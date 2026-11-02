use crate::fneg;
use crate::prelude::*;
use crate::estimate::{vrefp, vrsqrtefp};
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
    d.get(at..at + delirium_data::kv!(1 int)).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or_else(|| "Read out of range".to_string())
}
fn bef(d: &[u8], at: usize) -> Result<f32, String> {
    be32(d, at).map(f32::from_bits)
}
fn vec3(d: &[u8], at: usize) -> Result<Vec3, String> {
    Ok(Vec3::new(bef(d, at)?, bef(d, at + delirium_data::kv!(1 int))?, bef(d, at + delirium_data::kv!(23 int))?))
}
fn ptr(p: u32) -> Result<usize, String> {
    if p >> delirium_data::kv!(85 int) == delirium_data::kv!(2 int) { Ok((p & delirium_data::kv!(86 int)) as usize) } else { Err(format!("Bad bounds pointer {p:08X}")) }
}

pub fn read(d: &[u8], at: usize) -> Result<Curved, String> {
    let (scale, offset) = (vec3(d, at + delirium_data::kv!(87 int))?, vec3(d, at + delirium_data::kv!(88 int))?);
    let (nv, nf) = (be32(d, at + delirium_data::kv!(89 int))? as usize, be32(d, at + delirium_data::kv!(90 int))? as usize);
    let mut c = Curved::default();
    if nv > 0 {
        let vp = ptr(be32(d, at + delirium_data::kv!(91 int))?)?;
        for i in 0..nv {
            let q = |k: usize| d.get(vp + i * delirium_data::kv!(6 int) + k * 2..vp + i * delirium_data::kv!(6 int) + k * 2 + 2).map(|b| i16::from_be_bytes([b[0], b[1]]) as f32).ok_or("Read out of range");
            c.verts.push(Vec3::new(q(0)?.mul_add(scale.x, offset.x), q(1)?.mul_add(scale.y, offset.y), q(2)?.mul_add(scale.z, offset.z)));
        }
    }
    if nf > 0 {
        let fp = ptr(be32(d, at + delirium_data::kv!(92 int))?)?;
        for i in 0..nf {
            let p = fp + i * delirium_data::kv!(93 int);
            let idx = |o: usize| {
                let v = be32(d, p + o)? as i32;
                if v >= 0 && (v as usize) < nv { Ok(v as usize) } else { Err(format!("Face {i} indexes vertex {v} of {nv}")) }
            };
            c.faces.push(Face { centre: vec3(d, p)?, axis: vec3(d, p + delirium_data::kv!(54 int))?, radius: bef(d, p + delirium_data::kv!(7 int))?, from: idx(delirium_data::kv!(94 int))?, to: idx(delirium_data::kv!(95 int))? });
        }
    }
    Ok(c)
}

pub fn from_blobs(b: &[u8], v: &[u8], f: &[u8]) -> Result<Curved, String> {
    let (vo, fo) = (b.len().max(delirium_data::kv!(96 int)), b.len().max(delirium_data::kv!(96 int)) + v.len());
    let mut d = vec![0u8; fo + f.len()];
    d[..b.len()].copy_from_slice(b);
    d[vo..fo].copy_from_slice(v);
    d[fo..].copy_from_slice(f);
    d[delirium_data::kv!(91 int)..delirium_data::kv!(97 int)].copy_from_slice(&(delirium_data::kv!(98 int) | vo as u32).to_be_bytes());
    d[delirium_data::kv!(92 int)..delirium_data::kv!(99 int)].copy_from_slice(&(delirium_data::kv!(98 int) | fo as u32).to_be_bytes());
    read(&d, 0)
}

fn vmax(a: f32, b: f32) -> f32 {
    if a > b || (a == b && a.is_sign_positive()) { a } else { b }
}
fn vmin(a: f32, b: f32) -> f32 {
    if a < b || (a == b && a.is_sign_negative()) { a } else { b }
}

pub fn ped_part(width: f32, height: f32, margin: f32) -> Curved {
    let (lo, hi) = (height * -0.5, height * 0.5);
    let max = Vec3::new(width + margin, width + margin, hi + margin);
    let min = Vec3::new(-width - margin, -width - margin, lo - margin);
    let scale = (max - min) * 0.5 * delirium_data::kv!(100 f32);
    let offset = (min + max) * 0.5;

    let quant = |p: Vec3| {
        let lane = |i: usize| {
            let c = vmin(max[i], vmax(min[i], p[i])) - offset[i];
            let est = vrefp(scale[i]);
            let inv = est.mul_add(fneg(est.mul_add(scale[i], -1.0)), est);
            let q = ((c * inv + 0.5) as i32) as i16;
            (q as f32).mul_add(scale[i], offset[i])
        };
        Vec3::new(lane(0), lane(1), lane(2))
    };
    let verts: Vec<Vec3> = [(width, lo), (-width, lo), (width, hi), (-width, hi)].iter().map(|&(x, z)| quant(Vec3::new(x, 0.0, z))).collect();

    let face = |from: usize, to: usize, axis: Vec3, up: Vec3| {
        let (a, b) = (verts[from], verts[to]);
        let mid = (b + a) * 0.5;
        if from == to {
            let c = Vec3::new(up.x.mul_add(width, mid.x), up.y.mul_add(width, mid.y), up.z.mul_add(width, mid.z));
            return Face { centre: c, axis, radius: width, from, to };
        }
        let d = b - a;
        let q = d.x.mul_add(d.x, d.z.mul_add(d.z, d.y * d.y)) * delirium_data::kv!(17);
        let s2 = width.mul_add(width, -q);
        let s = if s2 > 0.0 { s2.sqrt() } else { 0.0 };
        let centre = Vec3::new(fneg(up.x.mul_add(s, -mid.x)), fneg(up.y.mul_add(s, -mid.y)), fneg(up.z.mul_add(s, -mid.z)));
        let c = Vec3::new(
            fneg(d.y.mul_add(up.z, -(d.z * up.y))),
            fneg(d.z.mul_add(up.x, -(d.x * up.z))),
            fneg(d.x.mul_add(up.y, -(d.y * up.x))),
        );
        let k = c.x.mul_add(axis.x, c.y.mul_add(axis.y, c.z * axis.z));
        let (from, to) = if k >= 0.0 { (from, to) } else { (to, from) };
        Face { centre, axis, radius: width, from, to }
    };
    let (z, y) = (Vec3::Z, Vec3::Y);
    let faces = vec![face(0, 1, Vec3::ZERO - z, y), face(1, 0, Vec3::ZERO - z, Vec3::ZERO - y), face(2, delirium_data::kv!(0 int), z, Vec3::ZERO - y), face(delirium_data::kv!(0 int), 2, z, y)];
    Curved { verts, faces }
}

impl Curved {
    pub fn bits(&self) -> Vec<u32> {
        let v3 = |v: Vec3| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
        let mut out: Vec<u32> = self.verts.iter().flat_map(|&v| v3(v)).collect();
        for f in &self.faces {
            out.extend(v3(f.centre));
            out.extend(v3(f.axis));
            out.extend([f.radius.to_bits(), f.from as u32, f.to as u32]);
        }
        out
    }

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
            if len2 > delirium_data::kv!(101 f32) {
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

