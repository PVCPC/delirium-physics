use std::collections::HashMap;
use std::sync::Arc;

use crate::prelude::*;
use crate::{Fma, Vd, Vx};

fn edge_dir(n: Vec3, m: Vec3, e: Vec3) -> Vec3 {
    if n.vd(m.vx(e)) > 0.0 {
        let h = n + m;
        h * crate::gjk::rsq(h.vd(h))
    } else {
        n
    }
}

#[allow(non_snake_case)]
#[inline]
fn CELL() -> f32 { delirium_data::kv!(65) }
const NEAR_SLACK: f32 = 2.0;
#[allow(non_snake_case)]
#[inline]
pub fn EYE() -> f32 { delirium_data::kv!(66) }
pub const ROOT: f32 = 1.0;
#[allow(non_snake_case)]
#[inline]
fn GRAVITY() -> f32 { delirium_data::kv!(34) }
#[allow(non_snake_case)]
#[inline]
fn SPRING() -> f32 { delirium_data::kv!(67) }

#[derive(Default)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::prelude::Resource))]
pub struct Collision {
    chunks: Vec<Option<Chunk>>,
    keys: HashMap<u64, u32>,
    free: Vec<u32>,
    cells: HashMap<(i32, i32), Vec<(u32, u32)>>,
    count: usize,
    neighbours: std::sync::RwLock<HashMap<(u32, u32), [Option<(u32, u32)>; 3]>>,
    pub level: delirium_data::levelslots::Slots,
    #[cfg(feature = "portal")]
    pub holes: Vec<Hole>,
}

#[cfg(feature = "portal")]
#[derive(Clone, Copy, Debug)]
pub struct Hole {
    pub at: Vec3,
    pub rot: Quat,
    pub half: Vec2,
    pub rect: bool,
}

#[cfg(feature = "portal")]
impl Hole {
    pub fn has(&self, p: Vec3) -> bool {
        let l = self.rot.inverse() * (p - self.at);
        let q = (l.truncate() / self.half).abs();
        l.z < delirium_data::kv!(68) && l.z > -1.0 && if self.rect { q.max_element() < 1.0 } else { q.length_squared() < 1.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPoly {
    pub corners: [Vec3; 4],
    pub count: usize,
    pub normal: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub n: Vec3,
    pub across: [Option<Vec3>; 3],
    pub raw: Option<[Vec3; 3]>,
    pub half: u8,
    pub rank: u32,
    pub mesh: Option<(Arc<crate::bvh::Tree>, u32)>,
    pub place: Option<Place>,
    pub frame: Option<Placed>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub turn: Quat,
    pub at: Vec3,
    pub tri: [Vec3; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub turn: Mat3,
    pub at: Vec3,
    pub tri: [Vec3; 3],
    pub own: Option<(Vec3, [Option<Vec3>; 3])>,
}

impl Face {
    pub fn of(m: &delirium_data::bounds::CollisionMesh) -> Vec<Option<Face>> {
        let tree = crate::bvh::Tree::new(m);
        (0..m.triangles.len()).map(|k| {
            let across = m.neighbours.get(k)?.map(|j| if j == delirium_data::bounds::PAST { Some(Vec3::Z) } else { m.normals.get(j as usize).map(|&v| Vec3::from(v)) });
            let raw = m.corners.get(k).filter(|&&c| c != m.triangles[k]).map(|c| c.map(|i| Vec3::from(m.positions[i as usize])));
            let tri = m.corners.get(k).copied().unwrap_or(m.triangles[k]).map(|i| Vec3::from(m.positions[i as usize]));
            let place = Some(Place { turn: Quat::IDENTITY, at: Vec3::ZERO, tri });
            Some(Face { n: Vec3::from(*m.normals.get(k)?), across, raw, half: m.halves.get(k).copied().unwrap_or(0), rank: m.order.get(k).copied().unwrap_or(k as u32), mesh: tree.clone().map(|t| (t, k as u32)), place, frame: None })
        }).collect()
    }

    pub fn turned(self, q: Quat, place: impl Fn(Vec3) -> Vec3) -> Face {
        let at = place(Vec3::ZERO);
        let own = self.place.map(|p| p.tri);
        let frame = own.map(|tri| Placed { turn: Mat3::from_quat(q), at, tri, own: Some((self.n, self.across)) });
        let placed = Some(Place { turn: q, at, tri: own.unwrap_or_default() });
        Face { n: q * self.n, across: self.across.map(|a| a.map(|a| q * a)), raw: self.raw.map(|r| r.map(place)), place: placed, frame, ..self }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poly {
    pub first: (u32, u32),
    pub n: Vec3,
    pub edges: [Option<(Vec3, Vec3, Vec3)>; 4],
    pub own_edges: [Option<Vec3>; 4],
    pub frame: Option<Placed>,
    pub pen: [Vec3; 4],
}

impl Poly {
    pub fn clamp(&self, hits: &mut [(Vec3, Vec3)], margin: f32) {
        let tol = margin * margin * delirium_data::kv!(69);
        let own = self.frame.and_then(|f| f.own.map(|o| (f, o.0)));
        let into = |f: &Placed, v: Vec3| Vec3::new(f.turn.x_axis.vd(v), f.turn.y_axis.vd(v), f.turn.z_axis.vd(v));
        let (mut axis, mut cos) = (own.map_or(self.n, |o| o.1), 1.0);
        for (k, (a, b, m)) in self.edges.into_iter().enumerate().filter_map(|(k, e)| e.map(|e| (k, e))) {
            if hits.iter().any(|&(p, _)| on_edge(a, b, p, tol)) {
                expand_cone(&mut axis, &mut cos, own.map_or(m, |o| self.own_edges[k].unwrap_or_else(|| into(&o.0, m))));
            }
        }
        for (_, n) in hits.iter_mut() {
            if let Some(f) = self.frame {
                let l = into(&f, *n);
                let l = if own.is_some() { clamp_to_cone(l, axis, cos) } else { l };
                *n = Vec3::new(f.turn.row(0).vd(l), f.turn.row(1).vd(l), f.turn.row(2).vd(l));
            }
            if own.is_none() {
                *n = clamp_to_cone(*n, axis, cos);
            }
        }
    }
}

fn to_frame(f: &Placed, p: Vec3) -> Vec3 {
    let q = p - f.at;
    Vec3::new(f.turn.x_axis.vd(q), f.turn.y_axis.vd(q), f.turn.z_axis.vd(q))
}

fn from_frame(f: &Placed, n: Vec3) -> Vec3 {
    Vec3::new(f.turn.row(0).vd(n), f.turn.row(1).vd(n), f.turn.row(2).vd(n))
}

fn on_edge(a: Vec3, b: Vec3, p: Vec3, tol: f32) -> bool {
    let (ap, e) = (p - a, b - a);
    let t = ap.dot(e);
    let c = if !(t > delirium_data::kv!(70)) {
        -ap
    } else if e.dot(e) - t > delirium_data::kv!(70) {
        e * (t * crate::gjk::recip(t + (e.dot(e) - t))) + -ap
    } else {
        e + -ap
    };
    let d = c + ap - ap;
    !(d.dot(d) > tol)
}

fn acos(x: f32) -> f32 {
    if x <= -1.0 { std::f32::consts::PI } else if x >= 1.0 { 0.0 } else { (x as f64).acos() as f32 }
}

fn turn(axis: Vec3, angle: f32, v: Vec3) -> Vec3 {
    let h = (angle * 0.5) as f64;
    let (w, q) = (delirium_data::mathrt::cos(h) as f32, axis * delirium_data::mathrt::sin(h) as f32);
    let d = v.z.fma(q.z, v.x.fma(q.x, q.y * v.y)) * 2.0;
    let c = Vec3::new(q.y.fma(v.z, -(q.z * v.y)), q.z.fma(v.x, -(v.z * q.x)), v.y.fma(q.x, -(q.y * v.x)));
    let (w2, ww) = (w * 2.0, (w * w).fma(2.0, -1.0));
    Vec3::new(c.x.fma(w2, v.x.fma(ww, q.x * d)), c.y.fma(w2, v.y.fma(ww, q.y * d)), c.z.fma(w2, v.z.fma(ww, q.z * d)))
}

fn expand_cone(axis: &mut Vec3, cos: &mut f32, v: Vec3) {
    let d = v.x.fma(axis.x, v.y.fma(axis.y, v.z * axis.z));
    if d >= *cos {
        return;
    }
    if *cos >= delirium_data::kv!(71) {
        let h = (v + *axis) * 0.5;
        let l2 = h.dot(h);
        *axis = if l2 > delirium_data::kv!(50) { h * crate::gjk::rsq(l2) } else { Vec3::Z };
        let c = (d + 1.0) * 0.5;
        *cos = if c > 0.0 { c.sqrt() } else { 0.0 };
    } else {
        let (a, b) = (acos(d), acos(*cos));
        let p = v.cross(*axis);
        *axis = turn(p * crate::gjk::rsq(p.dot(p)), (b - a) * 0.5, *axis);
        *cos = delirium_data::mathrt::cos(((a + b) * 0.5) as f64) as f32;
    }
}

fn clamp_to_cone(n: Vec3, axis: Vec3, cos: f32) -> Vec3 {
    if n.x.fma(axis.x, n.z.fma(axis.z, n.y * axis.y)) >= cos - delirium_data::kv!(72) {
        n
    } else if cos >= delirium_data::kv!(71) {
        axis
    } else {
        let p = crate::artic::at_point((axis, Vec3::ZERO), n);
        turn(p * crate::gjk::rsq(crate::Vd::vd(p, p)), acos(cos), axis)
    }
}

struct Chunk {
    triangles: Vec<[Vec3; 3]>,
    faces: Vec<Option<Face>>,
    materials: Vec<u8>,
    flags: Vec<u8>,
    cells: Vec<(i32, i32)>,
    order: Vec<u32>,
    listed: Vec<bool>,
    meshes: Vec<ChunkMesh>,
    index: u16,
    bounds: (Vec3, Vec3),
}

struct ChunkMesh {
    tree: Arc<crate::bvh::Tree>,
    ids: Vec<u32>,
    rot: Quat,
    pos: Vec3,
}

const FIXED: u64 = u64::MAX;

fn cell(v: f32) -> i32 {
    (v / CELL()).floor() as i32
}

impl Collision {
    pub fn add(&mut self, positions: &[[f32; 3]], triangles: &[[u32; 3]], materials: &[u8]) {
        self.add_chunk_flagged(FIXED, positions, triangles, materials, &[]);
    }

    pub fn add_floor(&mut self, x: f32, y: f32, z: f32, half: f32, material: u8) {
        assert!(half <= delirium_data::kv!(26), "a floor of 1 m squares {half} m each way");
        let size = 1.0;
        let n = half.ceil().max(1.0) as i32;
        let (mut p, mut t) = (vec![], vec![]);
        for i in -n..n {
            for j in -n..n {
                let (a, b, k) = (x + i as f32 * size, y + j as f32 * size, p.len() as u32);
                p.extend([[a, b, z], [a + size, b, z], [a + size, b + size, z], [a, b + size, z]]);
                t.extend([[k, k + 1, k + 2], [k, k + 2, k + delirium_data::kv!(0 int)]]);
            }
        }
        self.add(&p, &t, &vec![material; t.len()]);
    }

    pub fn add_flagged(&mut self, positions: &[[f32; 3]], triangles: &[[u32; 3]], materials: &[u8], flags: &[u8]) {
        self.add_chunk_flagged(FIXED, positions, triangles, materials, flags);
    }

    pub fn add_chunk(&mut self, key: u64, positions: &[[f32; 3]], triangles: &[[u32; 3]], materials: &[u8]) {
        self.add_chunk_flagged(key, positions, triangles, materials, &[]);
    }

    pub fn add_chunk_flagged(&mut self, key: u64, positions: &[[f32; 3]], triangles: &[[u32; 3]], materials: &[u8], flags: &[u8]) {
        self.add_chunk_faces(key, positions, triangles, materials, flags, &[]);
    }

    pub fn add_chunk_faces(&mut self, key: u64, positions: &[[f32; 3]], triangles: &[[u32; 3]], materials: &[u8], flags: &[u8],
                           faces: &[Option<Face>]) {
        let slot = match self.keys.get(&key) {
            Some(&s) => s,
            None => {
                let chunk = Chunk { triangles: vec![], faces: vec![], materials: vec![], flags: vec![], cells: vec![], order: vec![], listed: vec![], meshes: vec![], bounds: (Vec3::INFINITY, Vec3::NEG_INFINITY), index: self.level.take(false) };
                let s = match self.free.pop() {
                    Some(s) => { self.chunks[s as usize] = Some(chunk); s }
                    None => { self.chunks.push(Some(chunk)); self.chunks.len() as u32 - 1 }
                };
                self.keys.insert(key, s);
                s
            }
        };
        self.neighbours.get_mut().unwrap().clear();
        let chunk = self.chunks[slot as usize].as_mut().unwrap();
        let before = chunk.triangles.len();
        let mut touched = std::collections::HashSet::<(i32, i32)>::from_iter(chunk.cells.iter().copied());
        for (k, t) in triangles.iter().enumerate() {
            let Some(tri) = t.iter().map(|&i| positions.get(i as usize).map(|p| Vec3::from(*p))).collect::<Option<Vec<_>>>() else { continue };
            let tri = [tri[0], tri[1], tri[2]];
            let (lo, hi) = (tri[0].min(tri[1]).min(tri[2]), tri[0].max(tri[1]).max(tri[2]));
            if (hi - lo).truncate().max_element() > delirium_data::kv!(73) || !lo.is_finite() || !hi.is_finite() {
                continue;
            }
            let id = chunk.triangles.len() as u32;
            chunk.triangles.push(tri);
            chunk.faces.push(faces.get(k).cloned().flatten());
            chunk.order.push(faces.get(k).and_then(|f| f.as_ref()).map_or(id, |f| before as u32 + f.rank));
            let face = faces.get(k).and_then(|f| f.as_ref());
            chunk.listed.push(face.is_some_and(|f| f.mesh.is_some()));
            chunk.bounds = (chunk.bounds.0.min(lo), chunk.bounds.1.max(hi));
            if let Some(((tree, tri), place)) = face.and_then(|f| f.mesh.clone().map(|m| (m, f.place))) {
                let (rot, pos) = place.map_or((Quat::IDENTITY, Vec3::ZERO), |p| (p.turn, p.at));
                let m = match chunk.meshes.iter().position(|m| Arc::ptr_eq(&m.tree, &tree) && m.rot == rot && m.pos == pos) {
                    Some(m) => m,
                    None => {
                        chunk.meshes.push(ChunkMesh { ids: vec![u32::MAX; tree.count], tree, rot, pos });
                        chunk.meshes.len() - 1
                    }
                };
                if let Some(slot) = chunk.meshes[m].ids.get_mut(tri as usize) {
                    *slot = id;
                }
            }
            chunk.materials.push(materials.get(k).copied().unwrap_or(0));
            chunk.flags.push(flags.get(k).copied().unwrap_or(0));
            for x in cell(lo.x)..=cell(hi.x) {
                for y in cell(lo.y)..=cell(hi.y) {
                    self.cells.entry((x, y)).or_default().push((slot, id));
                    touched.insert((x, y));
                }
            }
        }
        chunk.cells = touched.into_iter().collect();
        self.count += chunk.triangles.len() - before;
    }

    pub fn has_chunk(&self, key: u64) -> bool {
        self.keys.contains_key(&key)
    }

    pub fn remove_chunk(&mut self, key: u64) {
        let Some(slot) = self.keys.remove(&key) else { return };
        let Some(chunk) = self.chunks[slot as usize].take() else { return };
        self.neighbours.get_mut().unwrap().clear();
        for c in &chunk.cells {
            if let Some(list) = self.cells.get_mut(c) {
                list.retain(|e| e.0 != slot);
                if list.is_empty() {
                    self.cells.remove(c);
                }
            }
        }
        self.count -= chunk.triangles.len();
        self.free.push(slot);
        self.level.free(chunk.index);
    }

    pub fn level_index(&self, slot: u32) -> u16 {
        self.chunks[slot as usize].as_ref().map_or(u16::MAX, |c| c.index)
    }

    pub fn set_level_index(&mut self, key: u64, i: u16) {
        if let Some(c) = self.keys.get(&key).and_then(|&s| self.chunks[s as usize].as_mut()) {
            c.index = i;
        }
    }

    pub fn chunk_keys(&self) -> impl Iterator<Item = u64> + '_ {
        self.keys.keys().copied()
    }

    pub fn slot_key(&self, slot: u32) -> Option<u64> {
        self.keys.iter().find(|(_, s)| **s == slot).map(|(&k, _)| k)
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn triangles(&self) -> Vec<[Vec3; 3]> {
        self.chunks.iter().flatten().flat_map(|c| c.triangles.iter().copied()).collect()
    }

    pub fn tris_near(&self, lo: Vec3, hi: Vec3) -> Vec<([Vec3; 3], u8, [Vec3; 4], u8, u32, Poly)> {
        self.tuples(self.grid_near(lo, hi, NEAR_SLACK, |_, _| true))
    }

    fn grid_near(&self, lo: Vec3, hi: Vec3, slack: f32, keep: impl Fn(&Chunk, u32) -> bool) -> Vec<(u32, u32)> {
        let chunk = |(s, _): (u32, u32)| self.chunks[s as usize].as_ref().unwrap();
        let mut ids: Vec<(u32, u32)> = (cell(lo.x)..=cell(hi.x)).flat_map(|x| (cell(lo.y)..=cell(hi.y)).map(move |y| (x, y)))
            .flat_map(|c| self.cells.get(&c).into_iter().flatten().copied()).filter(|&e| keep(chunk(e), e.1)).collect();
        let (blo, bhi) = (lo - Vec3::splat(slack), hi + Vec3::splat(slack));
        ids.retain(|&e| {
            let t = chunk(e).triangles[e.1 as usize];
            t[0].min(t[1]).min(t[2]).cmple(bhi).all() && t[0].max(t[1]).max(t[2]).cmpge(blo).all()
        });
        ids.sort_unstable();
        ids.dedup();
        ids.sort_by_key(|&(s, i)| (s, chunk((s, i)).order[i as usize]));
        ids
    }

    pub fn tris_in_part(&self, lo: Vec3, hi: Vec3, a: &crate::bvh::Frame, a_last: &crate::bvh::Frame, bmin: Vec3, bmax: Vec3)
        -> Vec<([Vec3; 3], u8, [Vec3; 4], u8, u32, Poly)> {
        let mut ids = self.grid_near(lo, hi, 0.0, |c, i| !c.listed[i as usize]);
        let mut listed = vec![];
        for (s, c) in self.chunks.iter().enumerate() {
            let Some(c) = c else { continue };
            for m in &c.meshes {
                let b = crate::bvh::Frame { rows: [m.rot * Vec3::X, m.rot * Vec3::Y, m.rot * Vec3::Z], t: m.pos };
                let q = crate::bvh::swept_obb(a, a_last, &b, &b, bmin, bmax);
                for p in crate::bvh::polygons(&m.tree.bvh, &q) {
                    listed.extend(m.tree.tris.get(p as usize).into_iter().flatten().filter_map(|&k| m.ids.get(k as usize).copied())
                        .filter(|&id| id != u32::MAX).map(|id| (s as u32, id)));
                }
            }
        }
        listed.sort_unstable();
        listed.dedup();
        ids.extend(listed);
        ids.sort_by_key(|&(s, i)| { let c = self.chunks[s as usize].as_ref().unwrap(); (c.index, c.order[i as usize]) });
        self.tuples(ids)
    }

    pub fn sphere_polys(&self, at: Vec3, r: f32) -> Vec<Vec<WorldPoly>> {
        let mut chunks: Vec<&Chunk> = self.chunks.iter().flatten().collect();
        chunks.sort_by_key(|c| c.index);
        let mut out = vec![];
        for c in chunks {
            for m in &c.meshes {
                let rows = [m.rot * Vec3::X, m.rot * Vec3::Y, m.rot * Vec3::Z];
                let d = at - m.pos;
                let q = crate::bvh::Obb { rows: [Vec3::X, Vec3::Y, Vec3::Z], ext: Vec3::splat(r), centre: Vec3::new(rows[0].dot(d), rows[1].dot(d), rows[2].dot(d)) };
                let polys: Vec<WorldPoly> = crate::bvh::polygons(&m.tree.bvh, &q).into_iter().filter_map(|p| {
                    let tris: Vec<u32> = m.tree.tris.get(p as usize)?.iter().filter_map(|&k| m.ids.get(k as usize).copied()).filter(|&id| id != u32::MAX).collect();
                    let a = *c.triangles.get(*tris.first()? as usize)?;
                    let corners = match tris.get(1).and_then(|&b| c.triangles.get(b as usize)) {
                        Some(b) => [a[0], a[1], a[2], b[2]],
                        None => [a[0], a[1], a[2], Vec3::ZERO],
                    };
                    let normal = c.faces[tris[0] as usize].as_ref().map_or_else(|| (a[1] - a[0]).cross(a[2] - a[0]).normalize_or_zero(), |f| f.n);
                    Some(WorldPoly { corners, count: if tris.len() > 1 { delirium_data::kv!(1 int) } else { delirium_data::kv!(0 int) }, normal })
                }).collect();
                if !polys.is_empty() {
                    out.push(polys);
                }
            }
        }
        out
    }

    fn tuples(&self, ids: Vec<(u32, u32)>) -> Vec<([Vec3; 3], u8, [Vec3; 4], u8, u32, Poly)> {
        let chunk = |(s, _): (u32, u32)| self.chunks[s as usize].as_ref().unwrap();
        let tri = |e: (u32, u32)| chunk(e).triangles[e.1 as usize];
        let normal = |[a, b, c]: [Vec3; 3]| (b - a).cross(c - a).normalize_or_zero();
        let near: Vec<((u32, u32), [Vec3; 3])> = ids.into_iter().map(|e| (e, tri(e))).collect();
        let face = |e: (u32, u32)| chunk(e).faces.get(e.1 as usize).and_then(|f| f.as_ref());
        let quad = |a: (u32, u32)| face(a).is_some_and(|f| f.half == 1) && face((a.0, a.1 + 1)).is_some_and(|f| f.half == 2);
        near.into_iter().map(|(e, t)| {
            let (n, across) = match &chunk(e).faces[e.1 as usize] {
                Some(f) => (f.n, f.across),
                None => {
                    let cached = self.neighbours.read().unwrap().get(&e).copied();
                    let found = cached.unwrap_or_else(|| {
                        let f = std::array::from_fn(|k| {
                            let (p, q) = (t[k], t[(k + 1) % delirium_data::kv!(0 int)]);
                            self.cells.get(&(cell(p.x), cell(p.y))).into_iter().flatten().copied()
                                .filter(|&o| o.0 == e.0).filter(|&e| { let o = tri(e); o != t && o.contains(&p) && o.contains(&q) }).min()
                        });
                        self.neighbours.write().unwrap().insert(e, f);
                        f
                    });
                    (normal(t), found.map(|o| o.map(|o| normal(tri(o)))))
                }
            };
            let diag = face(e).and_then(|f| match f.half { 2 => Some(0), 1 if quad(e) => Some(2), _ => None });
            let edge = |k: usize| across[k].map(|m| if diag == Some(k) { n } else { edge_dir(n, m, t[(k + 1) % delirium_data::kv!(0 int)] - t[k]) });
            let edges = [edge(0), edge(1), edge(2)];
            let mask = (0..delirium_data::kv!(0 int)).filter(|&k| edges[k].is_some()).fold(0, |m, k| m | 1 << k);
            let frame = face(e).and_then(|f| f.frame);
            let pen = match frame {
                Some(Placed { tri: t, own: Some((n, across)), .. }) => {
                    let edge = |k: usize| across[k].map(|m| if diag == Some(k) { n } else { edge_dir(n, m, t[(k + 1) % delirium_data::kv!(0 int)] - t[k]) });
                    [n, edge(0).unwrap_or(n), edge(1).unwrap_or(n), edge(2).unwrap_or(n)]
                }
                Some(p) => [n, edges[0].unwrap_or(n), edges[1].unwrap_or(n), edges[2].unwrap_or(n)].map(|d| p.turn.transpose() * d),
                None => [n, edges[0].unwrap_or(n), edges[1].unwrap_or(n), edges[2].unwrap_or(n)],
            };
            let first = if e.1 > 0 && quad((e.0, e.1 - 1)) { (e.0, e.1 - 1) } else { e };
            let sides = |t: [Vec3; 3], across: [Option<Vec3>; 3], k: usize| across[k].map(|m| (t[k], t[(k + 1) % delirium_data::kv!(0 int)], m));
            let own = |e: (u32, u32), k: usize| face(e).and_then(|f| f.frame).and_then(|p| p.own).and_then(|o| o.1[k]);
            let (sides4, own_edges) = if quad(first) {
                let (a, b) = (first, (first.0, first.1 + 1));
                let (fa, fb) = (face(a).unwrap().across, face(b).unwrap().across);
                ([sides(tri(a), fa, 0), sides(tri(a), fa, 1), sides(tri(b), fb, 1), sides(tri(b), fb, 2)], [own(a, 0), own(a, 1), own(b, 1), own(b, 2)])
            } else {
                ([sides(t, across, 0), sides(t, across, 1), sides(t, across, 2), None], [own(e, 0), own(e, 1), own(e, 2), None])
            };
            let poly = Poly { first, n, edges: sides4, own_edges, frame, pen };
            (t, chunk(e).materials[e.1 as usize], [n, edges[0].unwrap_or(n), edges[1].unwrap_or(n), edges[2].unwrap_or(n)], mask, e.0, poly)
        }).collect()
    }

    pub fn poly_flags(&self, (slot, id): (u32, u32)) -> u8 {
        self.chunks.get(slot as usize).and_then(|c| c.as_ref()).and_then(|c| c.flags.get(id as usize).copied()).unwrap_or(0)
    }

    pub fn segment(&self, from: Vec3, to: Vec3) -> Option<(Vec3, Vec3)> {
        let (lo, hi) = (from.min(to), from.max(to));
        let mut best: Option<(f32, (u16, u32), Vec3, Vec3)> = None;
        for x in cell(lo.x)..=cell(hi.x) {
            for y in cell(lo.y)..=cell(hi.y) {
                for &(slot, id) in self.cells.get(&(x, y)).into_iter().flatten() {
                    let Some(chunk) = self.chunks[slot as usize].as_ref() else { continue };
                    let test = |id: u32| {
                        let face = chunk.faces[id as usize].as_ref();
                        let frame = face.and_then(|f| f.frame).filter(|f| f.own.is_some());
                        let (s, e) = frame.map_or((from, to), |f| (to_frame(&f, from), to_frame(&f, to)));
                        let t = frame.map(|f| f.tri).or(face.and_then(|f| f.raw)).unwrap_or(chunk.triangles[id as usize]);
                        let n = match (frame, face) {
                            (Some(Placed { own: Some(o), .. }), _) => o.0,
                            (_, Some(f)) => f.n,
                            _ => (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero(),
                        };
                        (segment_triangle(t, n, s, e), n, s, e, frame)
                    };
                    if chunk.faces[id as usize].as_ref().is_some_and(|f| f.half == 2) && id.checked_sub(1).is_some_and(|p| test(p).0.err().is_none_or(|c| c < delirium_data::kv!(1 int))) {
                        continue;
                    }
                    if let (Ok(f), n, s, e, frame) = test(id) {
                        let at = (e - s).mul_add(Vec3::splat(f), s);
                        let r = at - e;
                        let d = r.x.mul_add(r.x, r.z.mul_add(r.z, r.y * r.y)).sqrt();
                        let order = (chunk.index, chunk.order[id as usize]);
                        if d > 0.0 && best.is_none_or(|b| d > b.0 || (d == b.0 && order < b.1)) {
                            let (at, n) = frame.map_or((at, n), |f| (crate::vm4(f.turn.x_axis, f.turn.y_axis, f.turn.z_axis, f.at, at), from_frame(&f, n)));
                            best = Some((d, order, at, n));
                        }
                    }
                }
            }
        }
        best.map(|(_, _, at, n)| (at, n))
    }

    pub fn cast(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3)> {
        self.hit(from, dir, len, false).map(|(t, n, _)| (t, n))
    }

    pub fn contact(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3, u8)> {
        self.hit(from, dir, len, false).map(|(t, n, (slot, id))| (t, n, self.chunks[slot as usize].as_ref().map_or(0, |c| c.materials[id as usize])))
    }

    pub fn contact_flags(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3, u8, u8, u64)> {
        let (t, n, (slot, id)) = self.hit(from, dir, len, false)?;
        let c = self.chunks[slot as usize].as_ref();
        Some((t, n, c.map_or(0, |c| c.materials[id as usize]), c.and_then(|c| c.flags.get(id as usize).copied()).unwrap_or(0),
              self.slot_key(slot).unwrap_or(u64::MAX)))
    }

    pub fn contact_chunk(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3, u8, u64)> {
        let (t, n, (slot, id)) = self.hit(from, dir, len, false)?;
        let chunk = self.chunks[slot as usize].as_ref()?;
        Some((t, n, chunk.materials[id as usize], self.slot_key(slot)?))
    }

    pub fn blocked(&self, from: Vec3, dir: Vec3, len: f32) -> bool {
        let d = dir * len;
        let (mut x, mut y, end) = (cell(from.x), cell(from.y), (cell(from.x + d.x), cell(from.y + d.y)));
        let axis = |p: f32, d: f32, c: i32| {
            if d == 0.0 {
                return (0, f32::INFINITY, f32::INFINITY);
            }
            let s = if d > 0.0 { 1 } else { -1 };
            (s, (((c + (s > 0) as i32) as f32 * CELL()) - p) / d, CELL() / d.abs())
        };
        let ((sx, mut tx, dx), (sy, mut ty, dy)) = (axis(from.x, d.x, x), axis(from.y, d.y, y));
        loop {
            if self.cells.get(&(x, y)).into_iter().flatten().any(|&(slot, id)| {
                self.chunks[slot as usize].as_ref().is_some_and(|c| ray_triangle(c.triangles[id as usize], from, dir, len, None).is_some())
            }) {
                return true;
            }
            if (x, y) == end || tx.min(ty) > 1.0 {
                return false;
            }
            if tx < ty {
                x += sx;
                tx += dx;
            } else {
                y += sy;
                ty += dy;
            }
        }
    }

    fn hit(&self, from: Vec3, dir: Vec3, len: f32, ground: bool) -> Option<(f32, Vec3, (u32, u32))> {
        let to = from + dir * len;
        let (lo, hi) = (from.min(to), from.max(to));
        let mut best: Option<(f32, Vec3, (u32, u32))> = None;
        for x in cell(lo.x)..=cell(hi.x) {
            for y in cell(lo.y)..=cell(hi.y) {
                for &(slot, id) in self.cells.get(&(x, y)).into_iter().flatten() {
                    let Some(chunk) = self.chunks[slot as usize].as_ref() else { continue };
                    if let Some((t, n)) = ray_triangle(chunk.triangles[id as usize], from, dir, len, best.map(|b| b.0)) {
                        if ground && n.z.abs() < delirium_data::kv!(74) {
                            continue;
                        }
                        #[cfg(feature = "portal")]
                        if self.in_hole(from + dir * t) {
                            continue;
                        }
                        best = Some((t, n, (slot, id)));
                    }
                }
            }
        }
        best
    }

    pub fn near(&self, lo: Vec3, hi: Vec3) -> Near {
        let mut tris = vec![];
        let mut seen = std::collections::HashSet::new();
        for x in cell(lo.x)..=cell(hi.x) {
            for y in cell(lo.y)..=cell(hi.y) {
                for &(slot, id) in self.cells.get(&(x, y)).into_iter().flatten() {
                    let Some(chunk) = self.chunks[slot as usize].as_ref() else { continue };
                    let t = chunk.triangles[id as usize];
                    let (tlo, thi) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
                    if tlo.cmple(hi).all() && thi.cmpge(lo).all() && seen.insert((slot, id)) {
                        tris.push(t);
                    }
                }
            }
        }
        Near { tris }
    }

    pub fn ground(&self, from: Vec3, len: f32) -> Option<f32> {
        self.hit(from, Vec3::NEG_Z, len, true).map(|(t, _, _)| from.z - t)
    }

    pub fn ground_material(&self, from: Vec3, len: f32) -> Option<(f32, u8)> {
        self.hit(from, Vec3::NEG_Z, len, true).map(|(t, _, (slot, id))| (from.z - t, self.chunks[slot as usize].as_ref().map_or(0, |c| c.materials[id as usize])))
    }

    pub fn ground_slot(&self, from: Vec3, len: f32) -> Option<u32> {
        self.hit(from, Vec3::NEG_Z, len, true).map(|(_, _, (slot, _))| slot)
    }

    pub fn ground_hit(&self, from: Vec3, len: f32) -> Option<(f32, u8, u64)> {
        let (t, _, (slot, id)) = self.hit(from, Vec3::NEG_Z, len, true)?;
        let key = self.keys.iter().find(|(_, s)| **s == slot).map(|(&k, _)| k)?;
        Some((from.z - t, self.chunks[slot as usize].as_ref().map_or(0, |c| c.materials[id as usize]), key))
    }

    #[cfg(feature = "portal")]
    pub fn in_hole(&self, p: Vec3) -> bool {
        self.holes.iter().any(|h| h.has(p))
    }

    pub fn chunks_near(&self, at: Vec3, r: f32) -> Vec<u64> {
        self.keys.iter().filter(|(_, s)| self.chunks[**s as usize].as_ref()
            .is_some_and(|c| c.bounds.0.cmple(c.bounds.1).all() && (at.clamp(c.bounds.0, c.bounds.1) - at).length_squared() <= r * r)).map(|(&k, _)| k).collect()
    }

    pub fn ground_stairs(&self, from: Vec3, len: f32) -> Option<(f32, bool)> {
        self.hit(from, Vec3::NEG_Z, len, true).map(|(t, _, (slot, id))| (from.z - t, self.chunks[slot as usize].as_ref().is_some_and(|c| c.flags[id as usize] & 1 != 0)))
    }
}

fn segment_triangle([a, b, c]: [Vec3; 3], n: Vec3, s: Vec3, e: Vec3) -> Result<f32, u8> {
    let (d, sa, ba, ca) = (e - s, s - a, b - a, c - a);
    let (ns, ne) = (n.dot(sa), n.dot(e - a));
    let w = d.cross(ca);
    let (u, v, area) = (w.dot(sa), d.dot(sa.cross(ba)), w.dot(ba));
    let den = ns - ne;
    let y = 1.0 / den;
    let t = ns * (y * (1.0 - y * den) + y);
    match () {
        _ if !(ns >= 0.0) || ne >= 0.0 => Err(2),
        _ if !(t >= 0.0) || t > 1.0 => Err(delirium_data::kv!(0 int)),
        _ if !(area >= delirium_data::kv!(75)) => Err(1),
        _ if !(u >= 0.0) || u > area => Err(delirium_data::kv!(1 int)),
        _ if !(v >= 0.0) || u + v > area => Err(delirium_data::kv!(2 int)),
        _ => Ok(t),
    }
}

fn ray_triangle([a, b, c]: [Vec3; 3], from: Vec3, dir: Vec3, len: f32, best: Option<f32>) -> Option<(f32, Vec3)> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < delirium_data::kv!(76) {
        return None;
    }
    let s = (from - a) / det;
    let u = s.dot(p);
    let q = s.cross(e1);
    let v = dir.dot(q);
    let t = e2.dot(q);
    if u < 0.0 || v < 0.0 || u + v > 1.0 || t < 0.0 || t > len || best.is_some_and(|bt| t >= bt) {
        return None;
    }
    let n = e1.cross(e2).normalize_or_zero();
    Some((t, if n.dot(dir) > 0.0 { -n } else { n }))
}

pub struct Near {
    tris: Vec<[Vec3; 3]>,
}

impl Near {
    pub fn cast(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3)> {
        let mut best: Option<(f32, Vec3)> = None;
        for &t in &self.tris {
            if let Some(hit) = ray_triangle(t, from, dir, len, best.map(|b| b.0)) {
                best = Some(hit);
            }
        }
        best
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WalkerStep {
    pub v0: Vec3,
    plane: f32,
    had: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Walker {
    pub feet: Vec3,
    pub fall: f32,
    pub grounded: bool,
    pub ground_vel: Vec3,
    pub ground: Option<(Vec3, Vec3)>,
    pub stairs: bool,
    pub anchors: [Vec3; 2],
    pub jumping: bool,
    pub before: bool,
    pub vel: Vec3,
    parts: [crate::ragdoll::MapPoints; 4],
}

impl Walker {
    pub fn at(feet: Vec3, grounded: bool) -> Walker {
        Walker { feet, grounded, ..Default::default() }
    }

    pub fn readd(&mut self) {
        self.parts = Default::default();
    }

    pub fn step(&mut self, world: &Collision, wish: Vec2, jump: bool, dt: f32) {
        use delirium_data::pedbodies::{step, Body};
        let pre = self.begin(world, wish, jump, dt);
        let mut bodies = [Body::ped((self.feet + Vec3::Z * ROOT).to_array(), 0.0, pre.v0.to_array()),
                          Body { inv_mass: 0.0, state: 1, ..Body::ped([0.0; 3], 0.0, [0.0; 3]) }];
        let mut contacts = vec![];
        let rows = self.rows(&bodies[0], 0, 1, &mut contacts);
        let (moves, pushes) = step(&mut bodies, &contacts, dt);
        self.set_pushes(&rows, &pushes);
        self.end(&pre, Vec3::from(bodies[0].vel), Vec3::from(moves[0]), dt);
    }

    pub fn begin(&mut self, world: &Collision, wish: Vec2, jump: bool, dt: f32) -> WalkerStep {
        let root = self.feet + Vec3::Z * ROOT;
        let had = self.grounded;
        if jump && self.grounded {
            self.fall = -delirium_data::kv!(64);
            self.grounded = false;
            self.jumping = true;
        }
        let plane = match self.ground.filter(|_| self.grounded) {
            Some((_, n)) => -(n.x * wish.x + n.y * wish.y) / n.z,
            None => 0.0,
        };
        let mut up = -self.fall;
        if self.grounded && let Some((p, n)) = self.ground && p.z - root.z > -2.0 {
            let lift = GRAVITY() * (delirium_data::kv!(35) * (p.z - root.z + 1.0) + 1.0).min(delirium_data::kv!(31));
            let len2 = |d: Vec3| d.x.mul_add(d.x, d.z.mul_add(d.z, d.y * d.y));
            let v = Vec3::new(wish.x, wish.y, up);
            let (dir, v) = if self.anchors[0] != Vec3::ZERO && self.stairs {
                let d = p - self.anchors[0];
                let d = if len2(d) < delirium_data::kv!(77) && self.anchors[1] != Vec3::ZERO { p - self.anchors[1] } else { d };
                (if d.y.mul_add(d.y, d.x * d.x) > delirium_data::kv!(68) { crate::nmdbm::vnorm(d.cross(Vec3::Z).cross(d)) } else { n }, v)
            } else {
                (n, v - self.ground_vel)
            };
            let rel = dir.z.mul_add(v.z, dir.y.mul_add(v.y, dir.x * v.x));
            let rel = if n.z > delirium_data::kv!(78) { rel / dir.z } else { rel };
            let damp = (-2.0 * SPRING().sqrt() * rel).clamp(-delirium_data::kv!(79), delirium_data::kv!(79));
            up += (lift + damp) * dt;
        }
        up -= GRAVITY() * dt;
        self.anchors = match self.ground.filter(|_| self.grounded && self.before && !self.jumping) {
            Some((p, _)) if self.anchors[0] == Vec3::ZERO => [p, p],
            Some((p, _)) => {
                let d = p - self.anchors[0];
                if d.x.mul_add(d.x, d.z.mul_add(d.z, d.y * d.y)) > delirium_data::kv!(77) { [p, self.anchors[0]] } else { self.anchors }
            }
            None => [Vec3::ZERO; 2],
        };

        let parts = crate::ragdoll::ped_parts();
        let place = |k: usize| Self::place(root, k);
        for (k, (p, m)) in parts.iter().zip(&mut self.parts).enumerate() {
            let (rot, centre) = place(k);
            m.update(world, p.shape, (p.hi - p.lo) * 0.5, rot, centre);
        }
        let (rot, centre) = place(1);
        let best = self.parts[1].points(rot, centre).into_iter().filter(|q| q.n.z >= 0.5 && q.ground.z <= root.z)
            .fold(None::<crate::ragdoll::MapPoint>, |b, q| if b.is_none_or(|b| q.ground.z > b.ground.z) { Some(q) } else { b });
        self.ground = best.map(|q| (q.ground, q.n));
        if let Some(q) = best {
            self.stairs = q.flags & 1 != 0;
        }
        self.parts[1].clear();
        self.parts[delirium_data::kv!(0 int)].set_off();
        WalkerStep { v0: Vec3::new(wish.x, wish.y, plane + up), plane, had }
    }

    fn place(root: Vec3, k: usize) -> (Mat3, Vec3) {
        let p = &crate::ragdoll::ped_parts()[k];
        (p.m, root + p.at + p.m * ((p.lo + p.hi) * 0.5))
    }

    pub fn rows(&self, body: &delirium_data::pedbodies::Body, me: usize, map: usize, out: &mut Vec<delirium_data::pedbodies::Contact>) -> Vec<(usize, usize)> {
        let root = self.feet + Vec3::Z * ROOT;
        let mut rows = vec![];
        for k in [0, 2] {
            let (rot, centre) = Self::place(root, k);
            for (i, q) in self.parts[k].points(rot, centre).into_iter().enumerate().filter(|(_, q)| !q.off) {
                rows.push((k, i));
                out.push(delirium_data::pedbodies::Contact { a: me, b: map, n: q.n.to_array(), depth: -q.gap, warm: q.push,
                                                         at_a: body.local(q.on_part.to_array()), at_b: q.ground.to_array() });
            }
        }
        rows
    }

    pub fn set_pushes(&mut self, rows: &[(usize, usize)], pushes: &[f32]) {
        for (&(k, i), &l) in rows.iter().zip(pushes) {
            self.parts[k].set_push(i, l);
        }
    }

    pub fn end(&mut self, pre: &WalkerStep, v: Vec3, moved: Vec3, dt: f32) {
        let (had, plane) = (pre.had, pre.plane);
        self.feet += pre.v0 * dt + moved;
        self.vel = v;
        self.fall = -(v.z - plane);
        self.before |= had;
        let root = self.feet + Vec3::Z * ROOT;
        self.grounded = self.ground.is_some_and(|(p, _)| p.z - root.z > -1.0 || self.before && p.z - root.z > -delirium_data::kv!(80));
        if self.grounded {
            self.jumping = false;
        }
    }
}

pub fn street_spawn(world: &Collision, around: Vec2) -> Option<Vec3> {
    let tops: Vec<Vec3> = (-delirium_data::kv!(81 int)..=delirium_data::kv!(81 int))
        .flat_map(|i| (-delirium_data::kv!(81 int)..=delirium_data::kv!(81 int)).map(move |j| around + Vec2::new(i as f32, j as f32) * delirium_data::kv!(31)))
        .filter_map(|p| world.ground(p.extend(delirium_data::kv!(82)), delirium_data::kv!(83)).map(|z| p.extend(z)))
        .filter(|p| p.z > 1.0)
        .filter(|p| world.cast(*p + Vec3::Z * 0.5, Vec3::Z, delirium_data::kv!(27)).is_none())
        .filter(|p| (0..delirium_data::kv!(23 int)).all(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            world.cast(*p + Vec3::Z * 1.0, Vec3::new(a.cos(), a.sin(), 0.0), 2.0).is_none()
        }))
        .collect();
    let mut bins: HashMap<i32, usize> = HashMap::new();
    tops.iter().for_each(|p| *bins.entry(p.z.round() as i32).or_default() += 1);
    let (&street, _) = bins.iter().max_by_key(|&(z, n)| (*n, -z))?;
    tops.into_iter()
        .filter(|p| p.z.round() as i32 == street)
        .min_by(|a, b| a.truncate().distance(around).total_cmp(&b.truncate().distance(around)))
}

pub fn sphere_down(tris: &[[Vec3; 3]], from: Vec3, r: f32, len: f32) -> Option<(f32, Vec3)> {
    sphere_sweep(tris, from, Vec3::NEG_Z, r, len)
}

pub fn sphere_sweep(tris: &[[Vec3; 3]], from: Vec3, d: Vec3, r: f32, len: f32) -> Option<(f32, Vec3)> {
    let mut best: Option<(f32, Vec3)> = None;
    let mut keep = |t: f32, q: Vec3| {
        let centre = from + d * t;
        if (0.0..=len).contains(&t) && (centre - q).dot(d) < 0.0 && best.is_none_or(|b| t < b.0) {
            best = Some((t, q));
        }
    };
    for &[a, b, c] in tris {
        let n0 = (b - a).cross(c - a).normalize_or_zero();
        if n0 == Vec3::ZERO {
            continue;
        }
        let n = if n0.dot(from - a) < 0.0 { -n0 } else { n0 };
        let (height, closing) = (n.dot(from - a), -n.dot(d));
        if closing > delirium_data::kv!(50) && height >= r {
            let t = (height - r) / closing;
            let q = from + d * t - n * r;
            let side = |q0: Vec3, q1: Vec3| (q1 - q0).cross(q - q0).dot(n0) >= 0.0;
            if side(a, b) && side(b, c) && side(c, a) {
                keep(t, q);
            }
        }
        for (p0, p1) in [(a, b), (b, c), (c, a)] {
            let e = p1 - p0;
            let u = e.normalize_or_zero();
            if u == Vec3::ZERO {
                continue;
            }
            let w = from - p0;
            let (w1, d1) = (w - u * w.dot(u), d - u * d.dot(u));
            let (qa, qb, qc) = (d1.dot(d1), 2.0 * w1.dot(d1), w1.dot(w1) - r * r);
            let disc = qb * qb - delirium_data::kv!(84) * qa * qc;
            if qa > delirium_data::kv!(76) && disc >= 0.0 && qc >= 0.0 {
                let t = (-qb - disc.sqrt()) / (2.0 * qa);
                let s = (w + d * t).dot(u);
                if (0.0..=e.length()).contains(&s) {
                    keep(t, p0 + u * s);
                }
            }
        }
        for p in [a, b, c] {
            let w = from - p;
            let (qb, qc) = (2.0 * w.dot(d), w.dot(w) - r * r);
            let disc = qb * qb - delirium_data::kv!(84) * qc;
            if disc >= 0.0 && qc >= 0.0 {
                keep((-qb - disc.sqrt()) / 2.0, p);
            }
        }
    }
    best
}
