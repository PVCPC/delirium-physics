use crate::prelude::*;
use crate::{vm4, Fma, Vd};
pub use delirium_data::bounds::Bvh;

pub const CAP: usize = 0x2000;

#[derive(Clone, Copy, Debug)]
pub struct Obb {
    pub rows: [Vec3; 3],
    pub ext: Vec3,
    pub centre: Vec3,
}

fn quant(b: &Bvh, p: Vec3) -> [u16; 3] {
    std::array::from_fn(|i| (((p[i] - b.origin[i]) * b.scale[i] + 0.5) as i32).clamp(0, 65535) as u16)
}

pub fn query_box(b: &Bvh, q: &Obb) -> ([u16; 3], [u16; 3]) {
    let e = q.ext + Vec3::from(b.margin);
    let a = [q.rows[0] * e.x, q.rows[1] * e.y, q.rows[2] * e.z];
    let h = Vec3::new((a[1].x.abs() + a[0].x.abs()) + a[2].x.abs(), (a[2].y.abs() + a[1].y.abs()) + a[0].y.abs(),
                      (a[2].z.abs() + a[1].z.abs()) + a[0].z.abs());
    (quant(b, q.centre - h), quant(b, q.centre + h))
}

fn be16(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
}

fn meets(d: &[u8], at: usize, lo: [u16; 3], hi: [u16; 3]) -> bool {
    (0..3).all(|i| {
        let (nl, nh) = (be16(d, at + i * 2).unwrap_or(u16::MAX), be16(d, at + 6 + i * 2).unwrap_or(0));
        nl <= hi[i] && lo[i] <= nh
    })
}

pub fn walk(b: &Bvh, lo: [u16; 3], hi: [u16; 3]) -> Vec<u32> {
    let mut out = vec![];
    for s in 0..b.subs.len() / 16 {
        let h = s * 16;
        if !meets(&b.subs, h, lo, hi) {
            continue;
        }
        let (mut i, end) = (be16(&b.subs, h + 12).unwrap_or(0) as usize, be16(&b.subs, h + 14).unwrap_or(0) as usize);
        while i < end {
            let n = i * 16;
            let Some(count) = b.nodes.get(n + 14).copied() else { break };
            let step = if meets(&b.nodes, n, lo, hi) {
                let first = be16(&b.nodes, n + 12).unwrap_or(0) as u32;
                for j in 0..count as u32 {
                    if out.len() < CAP {
                        out.push(first + j);
                    }
                }
                1
            } else if count != 0 {
                1
            } else {
                be16(&b.nodes, n + 12).unwrap_or(1).max(1) as usize
            };
            i += step;
        }
    }
    out
}

pub fn polygons(b: &Bvh, q: &Obb) -> Vec<u32> {
    let (lo, hi) = query_box(b, q);
    walk(b, lo, hi)
}

pub struct Tree {
    pub bvh: Bvh,
    pub tris: Vec<Vec<u32>>,
    pub count: usize,
}

impl Tree {
    pub fn new(m: &delirium_data::bounds::CollisionMesh) -> Option<std::sync::Arc<Tree>> {
        let bvh = m.bvh.clone()?;
        let mut tris = vec![vec![]; m.poly.iter().max().map_or(0, |&p| p as usize + 1)];
        for (k, &p) in m.poly.iter().enumerate() {
            tris[p as usize].push(k as u32);
        }
        Some(std::sync::Arc::new(Tree { bvh, tris, count: m.poly.len() }))
    }
}

impl PartialEq for Tree {
    fn eq(&self, o: &Tree) -> bool {
        std::ptr::eq(self, o)
    }
}

impl std::fmt::Debug for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Tree({} polygons)", self.tris.len())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub rows: [Vec3; 3],
    pub t: Vec3,
}

fn fm(a: Vec3, k: f32, c: Vec3) -> Vec3 {
    Vec3::new(a.x.fma(k, c.x), a.y.fma(k, c.y), a.z.fma(k, c.z))
}

fn rel(a: &Frame, b: &Frame) -> [Vec3; 3] {
    std::array::from_fn(|i| Vec3::new(a.rows[i].vd(b.rows[0]), a.rows[i].vd(b.rows[1]), a.rows[i].vd(b.rows[2])))
}

pub fn swept_obb(a: &Frame, a_last: &Frame, b: &Frame, b_last: &Frame, bmin: Vec3, bmax: Vec3) -> Obb {
    let (e, c) = ((bmax - bmin) * 0.5, (bmin + bmax) * 0.5);
    let rl = rel(a_last, b_last);
    let dl = a_last.t - b_last.t;
    let tl = Vec3::new(dl.vd(b_last.rows[0]), dl.vd(b_last.rows[1]), dl.vd(b_last.rows[2]));
    let w: [Vec3; 3] = std::array::from_fn(|i| fm(b.rows[2], rl[i].z, fm(b.rows[1], rl[i].y, b.rows[0] * rl[i].x)));
    let off = fm(b.rows[2], tl.z, fm(b.rows[1], tl.y, fm(b.rows[0], tl.x, b.t))) - a.t;
    let m: [Vec3; 3] = std::array::from_fn(|i| Vec3::new(w[i].vd(a.rows[0]), w[i].vd(a.rows[1]), w[i].vd(a.rows[2])));
    let t = Vec3::new(off.vd(a.rows[0]), off.vd(a.rows[1]), off.vd(a.rows[2]));
    let cl = vm4(m[0], m[1], m[2], t, c);
    let el = Vec3::new(e.vd(Vec3::new(m[0].x.abs(), m[1].x.abs(), m[2].x.abs())),
                       e.vd(Vec3::new(m[0].y.abs(), m[1].y.abs(), m[2].y.abs())),
                       e.vd(Vec3::new(m[0].z.abs(), m[1].z.abs(), m[2].z.abs())));
    let (lo, hi) = ((c - e).min(cl - el), (c + e).max(cl + el));
    let ctr = (lo + hi) * 0.5;
    let d = vm4(a.rows[0], a.rows[1], a.rows[2], a.t, ctr) - b.t;
    Obb { rows: rel(a, b), ext: hi - ctr, centre: Vec3::new(d.vd(b.rows[0]), d.vd(b.rows[1]), d.vd(b.rows[2])) }
}

