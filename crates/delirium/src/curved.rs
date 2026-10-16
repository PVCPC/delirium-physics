use crate::prelude::*;
use crate::estimate::vrsqrtefp;
use crate::{Fma, Vd, Vx};

#[derive(Clone, Copy, Debug)]
struct Face {
    centre: Vec3,
    axis: Vec3,
    radius: f32,
    from: usize,
    to: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Curved {
    verts: Vec<Vec3>,
    faces: Vec<Face>,
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or_else(|| "Read out of range".to_string())
}
fn bef(d: &[u8], at: usize) -> Result<f32, String> {
    be32(d, at).map(f32::from_bits)
}
fn vec3(d: &[u8], at: usize) -> Result<Vec3, String> {
    Ok(Vec3::new(bef(d, at)?, bef(d, at + 4)?, bef(d, at + 8)?))
}
fn ptr(p: u32) -> Result<usize, String> {
    if p >> 28 == 5 { Ok((p & 0x0FFF_FFFF) as usize) } else { Err(format!("Bad bounds pointer {p:08X}")) }
}

pub fn read(d: &[u8], at: usize) -> Result<Curved, String> {
    let (scale, offset) = (vec3(d, at + 144)?, vec3(d, at + 160)?);
    let (nv, nf) = (be32(d, at + 200)? as usize, be32(d, at + 236)? as usize);
    let mut c = Curved::default();
    if nv > 0 {
        let vp = ptr(be32(d, at + 176)?)?;
        for i in 0..nv {
            let q = |k: usize| d.get(vp + i * 6 + k * 2..vp + i * 6 + k * 2 + 2).map(|b| i16::from_be_bytes([b[0], b[1]]) as f32).ok_or("Read out of range");
            c.verts.push(Vec3::new(q(0)?.mul_add(scale.x, offset.x), q(1)?.mul_add(scale.y, offset.y), q(2)?.mul_add(scale.z, offset.z)));
        }
    }
    if nf > 0 {
        let fp = ptr(be32(d, at + 228)?)?;
        for i in 0..nf {
            let p = fp + i * 48;
            let idx = |o: usize| {
                let v = be32(d, p + o)? as i32;
                if v >= 0 && (v as usize) < nv { Ok(v as usize) } else { Err(format!("Face {i} indexes vertex {v} of {nv}")) }
            };
            c.faces.push(Face { centre: vec3(d, p)?, axis: vec3(d, p + 16)?, radius: bef(d, p + 32)?, from: idx(36)?, to: idx(40)? });
        }
    }
    Ok(c)
}

impl Curved {
    pub fn support(&self, d: Vec3) -> Vec3 {
        let dd = d.vd(d);
        let est = vrsqrtefp(dd);
        let est = if est == f32::INFINITY { 0.0 } else { est };
        let u = d * Fma::fma(Fma::fma(-dd, est * est, 1.0), 0.5 * est, est);

        let (mut best, mut pt) = (-f32::MAX, Vec3::ZERO);
        for &p in &self.verts {
            let k = u.vd(p);
            if k > best {
                (best, pt) = (k, p);
            }
        }
        let (best0, pt0) = (best, pt);
        for f in &self.faces {
            let perp = f.axis.vx(u).vx(f.axis);
            let len2 = perp.vd(perp);
            if len2 > f32::from_bits(0x358637bd) {
                let est = vrsqrtefp(len2);
                let est = if est == f32::INFINITY { 0.0 } else { est };
                let n = Fma::fma(Fma::fma(-len2, est * est, 1.0), 0.5 * est, est);
                let e = perp * f.radius * n;
                let (a, b) = (self.verts[f.from] - f.centre, self.verts[f.to] - f.centre);
                let neg = |s: f32| 0.0 > s;
                let c1 = neg(f.axis.vd(a.vx(e)));
                let c2 = neg(f.axis.vd(a.vx(b)));
                let c3 = neg(f.axis.vd(e.vx(b)));
                let on = f.from == f.to || (!c1 && !c2 && !c3) || (c2 && !(c1 && c3));
                let cand = e + f.centre;
                let k = u.vd(cand);
                if on && k > best {
                    (best, pt) = (k, cand);
                }
            }
            if !(f.radius > 0.0) {
                (best, pt) = (best0, pt0);
            }
        }
        pt
    }
}

