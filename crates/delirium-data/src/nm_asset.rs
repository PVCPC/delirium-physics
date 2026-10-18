#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub name: String,
    pub index: u32,
    pub three: bool,
    pub pos: [f32; 3],
    pub twist_axis: [f32; 3],
    pub lean_axis: [f32; 3],
    pub limits: [(f32, f32); 3],
    pub flip: [bool; 3],
    pub muscle_stiffness: f32,
    pub damping: f32,
    pub strength: f32,
    pub max_torque: [f32; 2],
}

const SHIFT: usize = 3;

pub struct Stream<'a> {
    d: &'a [u8],
    at: usize,
    size: usize,
    typed: bool,
    attached: bool,
    pub pos: usize,
    pub last: usize,
    pub peek: u8,
}

type Res<T> = Result<T, u8>;

impl<'a> Stream<'a> {
    pub fn empty(d: &'a [u8]) -> Self {
        Stream { d, at: SHIFT, size: 0, typed: false, attached: false, pos: 0, last: 0, peek: 0 }
    }

    pub fn set_typed(&mut self, on: bool) -> bool {
        if self.size > 8 {
            return false;
        }
        self.typed = on;
        true
    }

    pub fn attach(&mut self, size: usize, typed: bool) -> bool {
        if self.size > 8 {
            return false;
        }
        self.size = size;
        self.typed = typed;
        self.attached = true;
        true
    }

    pub fn open(d: &'a [u8]) -> Self {
        let mut s = Self::empty(d);
        s.set_typed(true);
        s.attach(d.len().saturating_sub(SHIFT), true);
        s
    }

    pub fn block(&self) -> (usize, bool, bool) {
        (self.size, self.typed, self.attached)
    }

    pub fn with(mut self, size: usize, typed: bool) -> Self {
        self.size = size;
        self.typed = typed;
        self
    }

    fn b(&self, i: usize) -> u8 {
        self.d.get(self.at + i).copied().unwrap_or(0)
    }

    fn w(&self, i: usize) -> u32 {
        crate::u32_of([self.b(i), self.b(i + 1), self.b(i + 2), self.b(i + 3)])
    }

    pub fn tag(&self) -> u8 {
        self.b(self.pos) & 0x7f
    }

    fn head(&mut self, kind: u8, width: usize, align: bool) -> Res<usize> {
        let t = self.typed as usize;
        if (self.size as u32).wrapping_sub(self.pos as u32) < (t + width) as u32 {
            return Err(1);
        }
        if self.typed {
            let tag = self.b(self.pos);
            if align && tag & 0x80 != 0 {
                let r = (self.pos + 1) % 4;
                if r > 0 {
                    self.pos = self.pos - r + 4;
                }
            }
            if tag & 0x7f != kind {
                return Err(2);
            }
        }
        Ok(self.pos + t)
    }

    fn done(&mut self, width: usize) {
        self.last = self.pos;
        self.peek = 0;
        self.pos += self.typed as usize + width;
    }

    pub fn byte(&mut self) -> Res<u8> {
        let a = self.head(1, 1, false)?;
        let v = self.b(a);
        self.done(1);
        Ok(v)
    }

    pub fn flag(&mut self) -> Res<u8> {
        let a = self.head(2, 1, false)?;
        let v = self.b(a);
        self.done(1);
        Ok(v)
    }

    pub fn int(&mut self) -> Res<u32> {
        let a = self.head(3, 4, true)?;
        let v = self.w(a);
        self.done(4);
        Ok(v)
    }

    pub fn uint(&mut self) -> Res<u32> {
        let a = self.head(4, 4, true)?;
        let v = self.w(a);
        self.done(4);
        Ok(v)
    }

    pub fn sub(&mut self) -> Res<Stream<'a>> {
        let a = self.head(13, 5, true)?;
        let (n, typed) = (self.w(a) as usize, self.b(a + 4) != 0);
        let start = a + 5;
        if (self.size as u32).wrapping_sub(start as u32) < n as u32 {
            return Err(1);
        }
        let s = Stream { d: self.d, at: self.at + start, size: n, typed, attached: true, pos: 8, last: 0, peek: 0 };
        self.last = self.pos;
        self.peek = 0;
        self.pos = start + n;
        Ok(s)
    }

    pub fn float(&mut self) -> Res<f32> {
        let a = self.head(5, 4, true)?;
        let v = f32::from_bits(self.w(a));
        self.done(4);
        Ok(v)
    }

    pub fn vec3(&mut self) -> Res<[f32; 3]> {
        let a = self.head(16, 12, true)?;
        let v = [0, 4, 8].map(|k| f32::from_bits(self.w(a + k)));
        self.done(12);
        Ok(v)
    }

    pub fn matrix(&mut self) -> Res<[f32; 16]> {
        let a = self.head(22, 64, true)?;
        let v = std::array::from_fn(|k| f32::from_bits(self.w(a + 4 * k)));
        self.done(64);
        Ok(v)
    }

    pub fn string(&mut self, max: Option<usize>) -> Res<&'a [u8]> {
        let t = self.typed as usize;
        let room = (self.size as u32).wrapping_sub(self.pos as u32);
        if room < (t + 4) as u32 {
            return Err(1);
        }
        if self.typed && self.b(self.pos) & 0x7f != 7 {
            return Err(2);
        }
        let n = self.w(self.pos + t) as usize;
        if max.is_some_and(|m| n > m) {
            return Err(4);
        }
        if room.wrapping_sub((t + 4) as u32) < n as u32 {
            return Err(1);
        }
        let a = self.at + self.pos + t + 4;
        self.done(4);
        self.pos += n;
        Ok(self.d.get(a..a + n).unwrap_or(&[]))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssetSlot {
    pub slot: u32,
    pub n12: Option<u32>,
    pub n8: Option<u32>,
    pub name: Option<Vec<u8>>,
    pub items: Option<u32>,
    pub taken: Option<Vec<u32>>,
    pub walked: Option<bool>,
}

pub fn manager_load(d: &[u8], max: u32, held: &mut [bool]) -> (Vec<AssetSlot>, i32) {
    let mut out = vec![];
    let mut verdict = 1;
    let size = d.get(..4).map_or(0, |w| crate::u32_of([w[0], w[1], w[2], w[3]]) as usize);
    let mut s = Stream { d, at: 0, size, typed: true, attached: true, pos: 8, last: 0, peek: 0 };
    let ok = (|| -> Res<()> {
        let h = s.string(Some(11))?;
        let h = &h[..h.iter().position(|&c| c == 0).unwrap_or(h.len())];
        let version = s.byte()?;
        s.byte()?;
        if h != b"NMEUPHORIA" || version != 3 {
            return Err(0);
        }
        let n = s.uint()?;
        let mut r = s.sub()?;
        for _ in 0..n {
            let slot = r.uint()?;
            if slot >= max {
                return Err(0);
            }
            out.push(AssetSlot { slot, n12: None, n8: None, name: None, items: None, taken: None, walked: None });
            let e = out.last_mut().unwrap();
            e.n12 = Some(r.uint()?);
            e.n8 = Some(r.uint()?);
            let nm = r.string(None)?;
            let k = nm.len().min(7);
            let mut v: Vec<u8> = nm[..k].iter().copied().take_while(|&c| c != 0).collect();
            v.resize(k, 0);
            e.name = Some(v);
            let n = r.uint()?;
            e.items = Some(n);
            let mut b = r.sub()?;
            let mut taken = vec![];
            let walked = items(&mut b, n, slot as usize, held, &mut taken);
            (e.taken, e.walked) = (Some(taken), walked);
            if walked != Some(true) {
                verdict = -1;
            }
        }
        Ok(())
    })();
    (out, if ok.is_ok() { verdict } else { 0 })
}

pub fn items(s: &mut Stream, n: u32, slot: usize, held: &mut [bool], taken: &mut Vec<u32>) -> Option<bool> {
    let mut refused = false;
    for _ in 0..n {
        let Ok([ty, _, size, chunks]) = (|| Ok::<_, u8>([s.uint()?, s.uint()?, s.uint()?, s.uint()?]))() else { break };
        if chunks > 0 {
            if s.string(Some(size as usize)).is_err() {
                break;
            }
            for _ in 0..chunks {
                let Ok(2) = s.uint() else { return None };
                let Ok(b) = s.string(None) else { return None };
                if b.len() >> 2 != 2 {
                    return None;
                }
            }
        } else if s.string(None).is_err() {
            break;
        }
        match ty {
            3 if held[slot] => refused = true,
            3 | 0x79 => {
                held[slot] |= ty == 3;
                taken.push(ty);
            }
            _ => {}
        }
    }
    Some(!refused)
}

pub fn rigid(m: &[f32; 16], tol: f32) -> bool {
    let ok = |x: f32| !(x.abs() > tol);
    if !ok(m[15] - 1.0) || !(0..3).all(|i| ok(m[3 + 4 * i])) {
        return false;
    }
    let p = [m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10]];
    let t = [p[0], p[3], p[6], p[1], p[4], p[7], p[2], p[5], p[8]];
    let (a, b) = (&p, &t);
    let o = [
        b[6].mul_add(a[2], a[1].mul_add(b[3], a[0] * b[0])),
        b[1].mul_add(a[0], a[1].mul_add(b[4], b[7] * a[2])),
        b[2].mul_add(a[0], a[1].mul_add(b[5], b[8] * a[2])),
        a[3].mul_add(b[0], a[4].mul_add(b[3], a[5] * b[6])),
        a[3].mul_add(b[1], a[5].mul_add(b[7], a[4] * b[4])),
        a[3].mul_add(b[2], a[5].mul_add(b[8], a[4] * b[5])),
        a[7].mul_add(b[3], a[6].mul_add(b[0], b[6] * a[8])),
        b[1].mul_add(a[6], a[8].mul_add(b[7], b[4] * a[7])),
        b[2].mul_add(a[6], a[8].mul_add(b[8], b[5] * a[7])),
    ];
    for i in 0..3 {
        let d = |j: usize| (if i == j { 1.0 } else { 0.0 }) - o[3 * i + j];
        if (d(1).abs() + d(0).abs()) + d(2).abs() > tol {
            return false;
        }
    }
    let c2 = a[2].mul_add(a[7], -(a[1] * a[8]));
    let c0 = a[4].mul_add(a[8], -(a[5] * a[7]));
    let c1 = a[1].mul_add(a[5], -(a[2] * a[4]));
    let det = c1.mul_add(a[6], c0.mul_add(a[0], c2 * a[3]));
    ok(det - 1.0)
}

pub fn read(d: &[u8]) -> Vec<Joint> {
    joints(d).0
}

fn joints(d: &[u8]) -> (Vec<Joint>, usize) {
    let mut out = vec![];
    let (mut o, mut end) = (SHIFT, 0);
    let mut s = Stream::open(d);
    while o + 5 < d.len() {
        s.pos = o - SHIFT;
        let Ok(nm) = s.string(None) else { o += 1; continue };
        let n = nm.len();
        let named = (2..40).contains(&n) && nm[n - 1] == 0 && nm[..n - 1].iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_');
        if !named || s.tag() != 3 {
            o += 1;
            continue;
        }
        let name = String::from_utf8_lossy(&nm[..n - 1]).into_owned();
        let (Ok(index), Ok(_), Ok(kind)) = (s.int(), s.int(), s.byte()) else { o += 1; continue };
        let three = kind == 0x33;
        let mut vecs = vec![];
        while s.tag() == 0x10 {
            let Ok(v) = s.vec3() else { break };
            vecs.push(v);
        }
        while s.tag() == 2 && s.flag().is_ok() {}
        if s.tag() != 7 || vecs.len() < 2 { o += 1; continue; }
        let Ok(blob) = s.string(None) else { o += 1; continue };
        let f = |k: usize| f32::from_bits(crate::u32_of(std::array::from_fn(|i| blob.get(k + i).copied().unwrap_or(0))));
        let j = if three {
            Joint { name, index, three, pos: vecs[0], twist_axis: vecs[1], lean_axis: vecs.get(2).copied().unwrap_or_default(),
                    limits: [(f(12), f(16)), (f(20), f(24)), (f(28), f(32))],
                    flip: [blob[8] != 0, blob[9] != 0, blob[10] != 0],
                    muscle_stiffness: f(56), damping: f(60), strength: f(64), max_torque: [f(48), f(52)] }
        } else {
            Joint { name, index, three, pos: vecs[0], twist_axis: vecs[1], lean_axis: [0.0; 3],
                    limits: [(f(8), f(12)), (0.0, 0.0), (0.0, 0.0)], flip: [false; 3],
                    muscle_stiffness: f(20), damping: f(24), strength: f(28), max_torque: [f(16), 0.0] }
        };
        o = s.pos + SHIFT;
        end = o;
        out.push(j);
    }
    out.sort_by_key(|j| j.index);
    (out, end)
}

pub fn self_collision(d: &[u8]) -> Vec<(usize, usize)> {
    let mut s = Stream::open(d);
    s.pos = joints(d).1.saturating_sub(SHIFT);
    let Ok(n) = s.int() else { return vec![] };
    (0..n).map_while(|_| Some((s.int().ok()? as usize, s.int().ok()? as usize))).collect()
}

pub const PARTS: [&str; 21] = ["Buttocks", "Thigh_Left", "Shin_Left", "Foot_Left", "Thigh_Right", "Shin_Right", "Foot_Right",
    "Spine0", "Spine1", "Spine2", "Spine3", "Neck", "Head", "Clavicle_Left", "Upper_Arm_Left", "Lower_Arm_Left", "Hand_Left",
    "Clavicle_Right", "Upper_Arm_Right", "Lower_Arm_Right", "Hand_Right"];
pub const PART_BONES: [u16; 21] = [417, 418, 419, 420, 423, 424, 425, 1202, 1203, 13984, 13985, 1204, 1205, 1216, 1217, 1218, 1219,
    1223, 1224, 1225, 1232];

pub fn parts(d: &[u8]) -> Vec<[[f32; 3]; 4]> {
    records(d).into_iter().map(|(rows, _)| rows).collect()
}

pub fn masses(d: &[u8]) -> Vec<f32> {
    records(d).into_iter().map(|(_, m)| m).collect()
}

fn records(d: &[u8]) -> Vec<([[f32; 3]; 4], f32)> {
    let find = |pat: &[u8]| d.windows(pat.len()).position(|w| w == pat);
    PARTS.iter().map(|name| {
        let n = name.len() as u32 + 1;
        let le = crate::LE.load(std::sync::atomic::Ordering::Relaxed);
        let mut pat = vec![if le { 0x07 } else { 0x87 }];
        pat.extend(if le { n.to_le_bytes() } else { n.to_be_bytes() });
        pat.extend(name.as_bytes());
        pat.push(0);
        let mut s = Stream::open(d);
        s.pos = find(&pat)?.checked_sub(SHIFT)?;
        s.string(None).ok()?;
        s.string(None).ok()?;
        s.flag().ok()?;
        for _ in 0..5 {
            s.float().ok()?;
        }
        let m1 = s.matrix().ok()?;
        let mass = s.float().ok()?;
        let m2 = s.matrix().ok()?;
        if !rigid(&m1, 1e-4) || !rigid(&m2, 1e-4) {
            return None;
        }
        let row = |r: usize| [m2[4 * r], m2[4 * r + 1], m2[4 * r + 2]];
        Some(([row(0), row(1), row(2), row(3)], mass))
    }).collect::<Option<Vec<_>>>().unwrap_or_default()
}

