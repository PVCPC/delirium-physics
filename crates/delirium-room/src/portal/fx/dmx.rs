use std::collections::HashMap;

use anyhow::{bail, Context, Result};

#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
    Element(Option<usize>),
    Int(i32),
    Float(f32),
    Bool(bool),
    String(String),
    Binary(Vec<u8>),
    Time(f32),
    Color([u8; 4]),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    QAngle([f32; 3]),
    Quat([f32; 4]),
    Matrix([f32; 16]),
    Array(Vec<Attr>),
}

impl Attr {
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Attr::Float(v) | Attr::Time(v) => Some(*v),
            Attr::Int(v) => Some(*v as f32),
            Attr::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Attr::Int(v) => Some(*v),
            Attr::Float(v) => Some(*v as i32),
            Attr::Bool(b) => Some(*b as i32),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Attr::Bool(b) => Some(*b),
            Attr::Int(v) => Some(*v != 0),
            Attr::Float(v) => Some(*v != 0.0),
            _ => None,
        }
    }
    pub fn as_vec3(&self) -> Option<[f32; 3]> {
        match self {
            Attr::Vec3(v) | Attr::QAngle(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_color(&self) -> Option<[u8; 4]> {
        match self {
            Attr::Color(c) => Some(*c),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Attr::String(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Element {
    pub ty: String,
    pub name: String,
    pub attrs: HashMap<String, Attr>,
}

impl Element {
    pub fn get(&self, k: &str) -> Option<&Attr> {
        self.attrs.get(k)
    }
    pub fn elements(&self, k: &str) -> Vec<usize> {
        match self.attrs.get(k) {
            Some(Attr::Array(v)) => v
                .iter()
                .filter_map(|a| match a {
                    Attr::Element(Some(i)) => Some(*i),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }
    pub fn element(&self, k: &str) -> Option<usize> {
        match self.attrs.get(k) {
            Some(Attr::Element(i)) => *i,
            _ => None,
        }
    }
}

struct Reader<'a> {
    d: &'a [u8],
    o: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        if self.o + n > self.d.len() {
            bail!("dmx: unexpected end of file at {}", self.o);
        }
        let s = &self.d[self.o..self.o + n];
        self.o += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn cstr(&mut self) -> Result<String> {
        let rest = &self.d[self.o..];
        let end = rest.iter().position(|&b| b == 0).context("dmx: unterminated string")?;
        let s = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.o += end + 1;
        Ok(s)
    }
    fn floats<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Ok(out)
    }
}

pub fn parse(data: &[u8]) -> Result<Vec<Element>> {
    let mut r = Reader { d: data, o: 0 };
    let header = r.cstr()?;
    let words: Vec<&str> = header.split_whitespace().collect();
    let enc_pos = words.iter().position(|w| *w == "binary").context("dmx: not a binary dmx")?;
    let version: i32 = words.get(enc_pos + 1).and_then(|v| v.parse().ok()).context("dmx: bad header")?;
    if !(2..=5).contains(&version) {
        bail!("dmx: unsupported binary encoding {version}");
    }

    let strings: Vec<String> = {
        let n = if version >= 4 { r.i32()? as usize } else { r.i16()? as u16 as usize };
        (0..n).map(|_| r.cstr()).collect::<Result<_>>()?
    };
    let idx = |r: &mut Reader| -> Result<usize> {
        Ok(if version >= 5 { r.i32()? as usize } else { r.i16()? as u16 as usize })
    };
    let s = |i: usize| -> Result<String> { strings.get(i).cloned().context("dmx: bad string index") };

    let n_elements = r.i32()? as usize;
    let mut elements = Vec::with_capacity(n_elements);
    for _ in 0..n_elements {
        let ty = s(idx(&mut r)?)?;
        let name = if version >= 4 { s(idx(&mut r)?)? } else { r.cstr()? };
        r.take(16)?;
        elements.push(Element { ty, name, attrs: HashMap::new() });
    }

    fn value(r: &mut Reader, t: u8, version: i32, strings: &[String]) -> Result<Attr> {
        Ok(match t {
            1 => {
                let i = r.i32()?;
                match i {
                    -1 => Attr::Element(None),
                    -2 => {
                        r.cstr()?;
                        Attr::Element(None)
                    }
                    i => Attr::Element(Some(i as usize)),
                }
            }
            2 => Attr::Int(r.i32()?),
            3 => Attr::Float(r.f32()?),
            4 => Attr::Bool(r.u8()? != 0),
            5 => {
                if version >= 4 {
                    let i = if version >= 5 { r.i32()? as usize } else { r.i16()? as u16 as usize };
                    Attr::String(strings.get(i).cloned().unwrap_or_default())
                } else {
                    Attr::String(r.cstr()?)
                }
            }
            6 => {
                let n = r.i32()? as usize;
                Attr::Binary(r.take(n)?.to_vec())
            }
            7 => Attr::Time(r.i32()? as f32 / 10000.0),
            8 => {
                let c = r.take(4)?;
                Attr::Color([c[0], c[1], c[2], c[3]])
            }
            9 => Attr::Vec2(r.floats::<2>()?),
            10 => Attr::Vec3(r.floats::<3>()?),
            11 => Attr::Vec4(r.floats::<4>()?),
            12 => Attr::QAngle(r.floats::<3>()?),
            13 => Attr::Quat(r.floats::<4>()?),
            14 => Attr::Matrix(r.floats::<16>()?),
            _ => bail!("dmx: unknown attribute type {t}"),
        })
    }

    for e in elements.iter_mut() {
        let n = r.i32()? as usize;
        for _ in 0..n {
            let name = s(idx(&mut r)?)?;
            let t = r.u8()?;
            let v = if t >= 15 {
                let count = r.i32()? as usize;
                let inner = t - 14;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(if inner == 5 { Attr::String(r.cstr()?) } else { value(&mut r, inner, version, &strings)? });
                }
                Attr::Array(items)
            } else {
                value(&mut r, t, version, &strings)?
            };
            e.attrs.insert(name, v);
        }
    }
    Ok(elements)
}
