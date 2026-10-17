use crate::prelude::*;
use crate::Vd;

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub verts: Vec<Vec3>,
    pub polys: Vec<Poly>,
    pub mats: Vec<u32>,
    pub radius: f32,
    pub cen: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Poly {
    pub n: Vec3,
    pub mat: u8,
    pub v: [u16; 4],
    pub nb: [u16; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub k: usize,
    pub at: Vec3,
    pub n: Vec3,
    pub face: Vec3,
    pub depth: f32,
    pub mat: u32,
    pub poly: u16,
}

fn be(d: &[u8], at: usize, n: usize) -> Result<&[u8], String> {
    d.get(at..at + n).ok_or_else(|| "Read out of range".into())
}
fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    be(d, at, 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}
fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    be(d, at, 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}
fn vec3(d: &[u8], at: usize) -> Result<Vec3, String> {
    let f = |k: usize| be32(d, at + 4 * k).map(f32::from_bits);
    Ok(Vec3::new(f(0)?, f(1)?, f(2)?))
}
fn ptr(p: u32) -> Result<usize, String> {
    (p >> 28 == 5).then_some((p & 0x0FFF_FFFF) as usize).ok_or_else(|| format!("Bad resource pointer {p:08x}"))
}

impl Mesh {
    pub fn read(d: &[u8], at: usize) -> Result<Mesh, String> {
        let (scale, offset) = (vec3(d, at + 0x90)?, vec3(d, at + 0xA0)?);
        let (nv, np) = (be32(d, at + 0xC8)? as usize, be32(d, at + 0xCC)? as usize);
        let raw: Vec<u16> = match nv {
            0 => vec![],
            _ => { let vp = ptr(be32(d, at + 0xB0)?)?; (0..3 * nv).map(|k| be16(d, vp + 2 * k)).collect::<Result<_, _>>()? }
        };
        let mut polys = vec![];
        if np > 0 {
            let pp = ptr(be32(d, at + 0x8C)?)?;
            for i in 0..np {
                let p = pp + 32 * i;
                let h = |o: usize| be16(d, p + o);
                polys.push(Poly { n: vec3(d, p)?, mat: be(d, p + 15, 1)?[0], v: [h(16)?, h(18)?, h(20)?, h(22)?], nb: [h(24)?, h(26)?, h(28)?, h(30)?] });
            }
        }
        let mats = match be32(d, at + 0xD0)? {
            0 => vec![],
            p => (0..be(d, at + 0xD8, 1)?[0] as usize).map(|k| be32(d, ptr(p)? + 4 * k)).collect::<Result<_, _>>()?,
        };
        Ok(Mesh::new(&raw, scale, offset, polys, mats, f32::from_bits(be32(d, at + 8)?), vec3(d, at + 48)?))
    }

    pub fn new(raw: &[u16], scale: Vec3, offset: Vec3, polys: Vec<Poly>, mats: Vec<u32>, radius: f32, cen: Vec3) -> Mesh {
        let q = |k: usize| raw[k] as i16 as f32;
        let verts = (0..raw.len() / 3).map(|i| Vec3::new(q(3 * i).mul_add(scale.x, offset.x), q(3 * i + 1).mul_add(scale.y, offset.y),
                                                           q(3 * i + 2).mul_add(scale.z, offset.z))).collect();
        Mesh { verts, polys, mats, radius, cen }
    }
}

fn into(m: &[Vec3; 4], p: Vec3) -> Vec3 {
    let q = p - m[3];
    Vec3::new(m[0].vd(q), m[1].vd(q), m[2].vd(q))
}

fn out_of(m: &[Vec3; 4], p: Vec3) -> Vec3 {
    crate::vm4(m[0], m[1], m[2], m[3], p)
}

pub fn turn(m: &[Vec3; 4], n: Vec3) -> Vec3 {
    let c = |k: usize| Vec3::new(m[0][k], m[1][k], m[2][k]).vd(n);
    Vec3::new(c(0), c(1), c(2))
}

pub fn inverse(m: &[Vec3; 4]) -> [Vec3; 4] {
    let c = |k: usize| Vec3::new(m[0][k], m[1][k], m[2][k]);
    [c(0), c(1), c(2), -Vec3::new(m[0].vd(m[3]), m[1].vd(m[3]), m[2].vd(m[3]))]
}

fn recip(h: f32) -> f32 {
    let r = crate::estimate::vrefp(h);
    r.mul_add((-r).mul_add(h, 1.0), r)
}

struct Slot {
    hit: Option<Hit>,
    rank: f32,
}

impl Slot {
    fn offer(&mut self, depth: f32, r: f32) -> bool {
        if !(depth > 0.0) {
            return false;
        }
        let rank = if depth > r { r - depth } else { depth };
        if self.hit.is_some() && rank <= self.rank {
            return false;
        }
        self.rank = rank;
        true
    }
}

fn polygon(m: &Mesh, pi: usize, c: Vec3, r: f32, slot: &mut Slot, k: usize) {
    let p = &m.polys[pi];
    let nn = p.n;
    let dot = |a: Vec3, d: Vec3| a.x.mul_add(d.x, a.y.mul_add(d.y, a.z * d.z));
    let sq = |d: Vec3| d.x.mul_add(d.x, d.y.mul_add(d.y, d.z * d.z));
    let v = |i: usize| m.verts[p.v[i] as usize];
    let d0 = c - v(0);
    let dist = dot(nn, d0);
    if dist.abs() > r {
        return;
    }
    let n = if p.v[3] != 0 { 4 } else { 3 };
    let (mut inside, mut cls) = (true, [0i8; 4]);
    let mut found: Option<(Vec3, Vec3, f32)> = None;
    for i in 0..n {
        let (pi_, pj) = (v(i), v((i + 1) % n));
        let (d, e) = (c - pi_, pj - pi_);
        let w = Vec3::new((-e.y).mul_add(nn.z, e.z * nn.y), (-e.z).mul_add(nn.x, e.x * nn.z), (-e.x).mul_add(nn.y, e.y * nn.x));
        if dot(w, d) >= 0.0 {
            cls[i] = 1;
            continue;
        }
        inside = false;
        if dot(e, d) < 0.0 {
            cls[i] = -1;
            continue;
        }
        let d1 = c - pj;
        if d1.x.mul_add(e.x, d1.y.mul_add(e.y, d1.z * e.z)) > 0.0 {
            cls[i] = -2;
            continue;
        }
        if dot(nn, d) <= 0.0 {
            continue;
        }
        let (a, b, l2) = (sq(d), sq(d1), sq(e));
        let inv = if l2 == 0.0 { 1e8 } else { 1.0 / l2 };
        let ab = a - b;
        let h2 = (-(ab * ab)).mul_add(inv, (b + a).mul_add(2.0, -l2)) * 0.25;
        if h2 > r * r {
            return;
        }
        let h = if h2 > 0.0 { h2.sqrt() } else { 0.0 };
        let u = ((a - h2) * inv).max(0.0).sqrt();
        let at = e.mul_add(Vec3::splat(u), pi_);
        let nrm = if h > 1e-12 { (c - at) * recip(h) } else { nn };
        let across = match p.nb[i] {
            0xFFFF => None,
            q => Some(m.polys.get(q as usize).map_or(Vec3::Z, |q| q.n)),
        };
        let side = |s: Vec3| nrm.z.mul_add(s.z, nrm.x.mul_add(s.x, nrm.y * s.y));
        if across.is_none_or(|q| side(q) <= side(nn)) {
            found = Some((at, nrm, r - h));
        }
        break;
    }
    let (at, nrm, depth) = match found {
        Some(f) => f,
        None if inside => ((-nn).mul_add(Vec3::splat(dist), c), nn, r - dist),
        None => {
            let Some(i) = (0..n).find(|&i| {
                let (a, b) = (cls[i], cls[(i + 1) % n]);
                (a == -2 && b != -2) || (a != -2 && a != -1 && b == -1)
            }) else { return };
            let pm = v((i + 1) % n);
            let d = c - pm;
            let q = d.x.mul_add(d.x, d.y.mul_add(d.y, d.z * d.z));
            if q > r * r {
                return;
            }
            let (dd, nrm) = if q > 1e-12 { let s = q.sqrt(); (s, d * recip(s)) } else { (0.0, nn) };
            (pm, nrm, r - dd)
        }
    };
    if slot.offer(depth, r) {
        slot.hit = Some(Hit { k, at, n: nrm, face: nn, depth, mat: m.mats.get(p.mat as usize).copied().unwrap_or(0), poly: pi as u16 });
    }
}

pub fn probe(inst: &[Vec3; 4], parts: &[(&[Vec3; 4], Option<&Mesh>)], c: Vec3, r: f32) -> Option<Hit> {
    let mut cl = into(inst, c);
    let mut slot = Slot { hit: None, rank: 0.0 };
    for (k, (m, mesh)) in parts.iter().enumerate() {
        let Some(mesh) = mesh else { continue };
        cl = into(m, cl);
        let reach = mesh.radius + r;
        let d = cl - mesh.cen;
        if !(reach * reach < d.x.mul_add(d.x, d.z.mul_add(d.z, d.y * d.y))) {
            let before = slot.hit;
            for pi in 0..mesh.polys.len() {
                polygon(mesh, pi, cl, r, &mut slot, k);
            }
            if slot.hit != before {
                slot.hit = slot.hit.map(|h| Hit { at: out_of(m, h.at), n: turn(m, h.n), face: turn(m, h.face), ..h });
            }
        }
        cl = out_of(m, cl);
    }
    slot.hit.map(|h| Hit { at: out_of(inst, h.at), n: turn(inst, h.n), face: turn(inst, h.face), ..h })
}
