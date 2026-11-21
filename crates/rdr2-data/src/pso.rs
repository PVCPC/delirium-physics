use std::collections::HashMap;
use std::fmt::Write;

fn jcase(s: &str) -> u32 {
    let mut h: u32 = 0;
    for c in s.bytes() {
        h = h.wrapping_add(c as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

struct Entry {
    hash: u32,
    ty: u8,
    sub: u8,
    off: usize,
    rk: u32,
}

struct Pso<'a> {
    d: &'a [u8],
    blocks: Vec<(u32, usize, usize)>,
    structs: HashMap<u32, (usize, Vec<Entry>)>,
    names: HashMap<u32, &'a str>,
}

impl Pso<'_> {
    fn u8(&self, o: usize) -> u8 {
        self.d.get(o).copied().unwrap_or(0)
    }
    fn u16(&self, o: usize) -> u16 {
        u16::from_be_bytes([self.u8(o), self.u8(o + 1)])
    }
    fn u32(&self, o: usize) -> u32 {
        u32::from_be_bytes([self.u8(o), self.u8(o + 1), self.u8(o + 2), self.u8(o + 3)])
    }
    fn f32(&self, o: usize) -> f32 {
        f32::from_bits(self.u32(o))
    }
    fn name(&self, h: u32) -> String {
        self.names.get(&h).map_or(format!("0x{h:08X}"), |n| n.to_string())
    }
    fn tag(&self, h: u32) -> String {
        self.names.get(&h).map_or(format!("UNK_MEMBER_0x{h:08X}"), |n| n.to_string())
    }
    fn ptr(&self, o: usize) -> Option<(u32, usize)> {
        let (w0, w1) = (self.u32(o), self.u32(o + 4));
        let at = (w0 >> 20) as usize | ((w1 >> 24) as usize) << 12;
        let &(h, off, len) = self.blocks.get(((w0 & 0xfff) as usize).checked_sub(1)?)?;
        (at < len).then_some((h, off + at))
    }
    fn size(&self, h: u32) -> usize {
        self.structs.get(&h).map_or(0, |s| s.0)
    }

    fn field(&self, out: &mut String, tag: &str, e: &Entry, all: &[Entry], base: usize, depth: usize) {
        let at = base + e.off;
        let pad = "  ".repeat(depth);
        if depth > 40 || out.len() > 64 << 20 {
            return;
        }
        match e.ty {
            0x00 => _ = writeln!(out, "{pad}<{tag} value=\"{}\" />", self.u8(at) != 0),
            0x01 => _ = writeln!(out, "{pad}<{tag} value=\"{}\" />", self.u8(at) as i8),
            0x02 => _ = writeln!(out, "{pad}<{tag} value=\"{}\" />", self.u8(at)),
            0x03 => _ = writeln!(out, "{pad}<{tag} value=\"{}\" />", self.u16(at) as i16),
            0x04 => _ = writeln!(out, "{pad}<{tag} value=\"{}\" />", self.u16(at)),
            0x05 => _ = writeln!(out, "{pad}<{tag} value=\"{}\" />", self.u32(at) as i32),
            0x06 | 0x0e | 0x0f => _ = writeln!(out, "{pad}<{tag} value=\"{}\" />", self.u32(at)),
            0x07 => _ = writeln!(out, "{pad}<{tag} value=\"{}\" />", self.f32(at)),
            0x08 => _ = writeln!(out, "{pad}<{tag} x=\"{}\" y=\"{}\" />", self.f32(at), self.f32(at + 4)),
            0x09 | 0x13 | 0x14 => _ = writeln!(out, "{pad}<{tag} x=\"{}\" y=\"{}\" z=\"{}\" />", self.f32(at), self.f32(at + 4), self.f32(at + 8)),
            0x0a | 0x15 => _ = writeln!(out, "{pad}<{tag} x=\"{}\" y=\"{}\" z=\"{}\" w=\"{}\" />", self.f32(at), self.f32(at + 4), self.f32(at + 8), self.f32(at + 12)),
            0x0b => {
                let text = match e.sub {
                    7 | 8 | 9 => match self.u32(at) {
                        0 => String::new(),
                        h => self.name(h),
                    },
                    0 => {
                        let s = &self.d[at.min(self.d.len())..(at + e.rk as usize).min(self.d.len())];
                        String::from_utf8_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())]).into_owned()
                    }
                    _ => self.ptr(at).map_or(String::new(), |(_, o)| {
                        let s = &self.d[o.min(self.d.len())..];
                        String::from_utf8_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(0)]).into_owned()
                    }),
                };
                _ = writeln!(out, "{pad}<{tag}>{}</{tag}>", text.replace('&', "&amp;").replace('<', "&lt;"));
            }
            0x0c => match e.sub {
                0 => self.object(out, tag, None, e.rk, at, depth),
                _ => match self.ptr(at) {
                    Some((h, o)) => self.object(out, tag, Some(h), h, o, depth),
                    None => _ = writeln!(out, "{pad}<{tag} type=\"NULL\" />"),
                },
            },
            0x0d => {
                let el = all.get(e.rk as usize & 0xffff);
                let (start, n) = match e.sub {
                    1 | 2 | 4 => (Some(at), (e.rk >> 16) as usize),
                    _ => (self.ptr(at).map(|p| p.1), self.u16(at + 8) as usize),
                };
                let Some(el) = el else { return };
                let Some(start) = start.filter(|_| n > 0) else {
                    _ = writeln!(out, "{pad}<{tag} />");
                    return;
                };
                let step = match el.ty {
                    0x00..=0x02 => 1,
                    0x03 | 0x04 => 2,
                    0x05..=0x07 | 0x0e | 0x0f => 4,
                    0x08 => 8,
                    0x09 | 0x0a | 0x13 | 0x14 | 0x15 => 16,
                    0x0b => if matches!(el.sub, 7 | 8 | 9) { 4 } else { 8 },
                    0x0c if el.sub == 0 => self.size(el.rk),
                    _ => 8,
                };
                _ = writeln!(out, "{pad}<{tag}>");
                for i in 0..n {
                    let one = Entry { hash: 0, ty: el.ty, sub: el.sub, off: 0, rk: el.rk };
                    self.field(out, "Item", &one, all, start + i * step, depth + 1);
                }
                _ = writeln!(out, "{pad}</{tag}>");
            }
            0x10 => {
                let (Some((ph, start)), n) = (self.ptr(at + 8), self.u16(at + 0x10) as usize) else {
                    _ = writeln!(out, "{pad}<{tag} />");
                    return;
                };
                let Some((step, pair)) = self.structs.get(&ph) else { return };
                let (Some(k), Some(v)) = (pair.first(), pair.get(1)) else { return };
                _ = writeln!(out, "{pad}<{tag}>");
                for i in 0..n {
                    let at = start + i * step;
                    let key = match k.ty {
                        0x0b => self.name(self.u32(at + k.off)),
                        _ => self.u32(at + k.off).to_string(),
                    };
                    let mut one = String::new();
                    self.field(&mut one, "Item", v, pair, at, depth + 1);
                    _ = write!(out, "{}", one.replacen("<Item", &format!("<Item key=\"{key}\""), 1));
                }
                _ = writeln!(out, "{pad}</{tag}>");
            }
            _ => {
                let raw: String = (0..24).map(|i| format!("{:02x}", self.u8(at + i))).collect();
                _ = writeln!(out, "{pad}<{tag} unread_type=\"{:#x}\" raw=\"{raw}\" />", e.ty)
            }
        }
    }

    fn object(&self, out: &mut String, tag: &str, ty: Option<u32>, h: u32, at: usize, depth: usize) {
        let pad = "  ".repeat(depth);
        let t = ty.map_or(String::new(), |t| format!(" type=\"{}\"", self.name(t)));
        let Some((_, es)) = self.structs.get(&h) else {
            _ = writeln!(out, "{pad}<{tag}{t} />");
            return;
        };
        _ = writeln!(out, "{pad}<{tag}{t}>");
        for e in es.iter().filter(|e| e.hash != 0x100) {
            self.field(out, &self.tag(e.hash), e, es, at, depth + 1);
        }
        _ = writeln!(out, "{pad}</{tag}>");
    }
}

pub fn to_xml(d: &[u8], names: &[&str]) -> Result<String, String> {
    let mut secs = HashMap::new();
    let mut o = 0;
    while o + 8 <= d.len() {
        let n = u32::from_be_bytes(d[o + 4..o + 8].try_into().unwrap()) as usize;
        secs.insert(&d[o..o + 4], o);
        if n == 0 {
            break;
        }
        o += n;
    }
    let (Some(_), Some(&pmap), Some(&psch)) = (secs.get(&b"PSIN"[..]), secs.get(&b"PMAP"[..]), secs.get(&b"PSCH"[..])) else {
        return Err("not a PSO file".into());
    };
    let mut p = Pso { d, blocks: vec![], structs: HashMap::new(), names: names.iter().map(|n| (jcase(n), *n)).collect() };
    let root = p.u32(pmap + 8) as usize;
    for i in 0..p.u16(pmap + 0x10) as usize {
        let e = pmap + 0x18 + i * 16;
        p.blocks.push((p.u32(e), p.u32(e + 4) as usize, p.u32(e + 12) as usize));
    }
    for i in 0..p.u32(psch + 8) as usize {
        let (h, off) = (p.u32(psch + 12 + i * 8), p.u32(psch + 16 + i * 8) as usize);
        let a = psch + off;
        let x = p.u32(a);
        if x >> 24 != 0 {
            continue;
        }
        let es = (0..(x & 0xffff) as usize)
            .map(|j| {
                let e = a + 12 + j * 12;
                Entry { hash: p.u32(e), ty: p.u8(e + 4), sub: p.u8(e + 5), off: p.u16(e + 6) as usize, rk: p.u32(e + 8) }
            })
            .collect();
        p.structs.insert(h, (p.u32(a + 4) as usize, es));
    }
    let &(h, off, _) = p.blocks.get(root.checked_sub(1).ok_or("no root")?).ok_or("no root block")?;
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    p.object(&mut out, &p.name(h), None, h, off, 0);
    Ok(out)
}
