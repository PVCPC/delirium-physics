use std::collections::HashMap;
use std::sync::Arc;

use crate::prelude::*;
use crate::Fma;

const CELL: f32 = 8.0;
const NEAR_SLACK: f32 = 2.0;
pub const EYE: f32 = 1.7;
const RADIUS: f32 = 0.35;
const STEP: f32 = 0.45;
const GRAVITY: f32 = 9.8;

#[derive(Default)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::prelude::Resource))]
pub struct Collision {
    chunks: Vec<Option<Chunk>>,
    keys: HashMap<u64, u32>,
    free: Vec<u32>,
    cells: HashMap<(i32, i32), Vec<(u32, u32)>>,
    count: usize,
    neighbours: std::sync::RwLock<HashMap<(u32, u32), [Option<(u32, u32)>; 3]>>,
    #[cfg(feature = "portal")]
    pub holes: Vec<Hole>,
}

#[cfg(feature = "portal")]
#[derive(Clone, Copy, Debug)]
pub struct Hole {
    pub at: Vec3,
    pub rot: Quat,
    pub half: Vec2,
}

#[cfg(feature = "portal")]
impl Hole {
    pub fn has(&self, p: Vec3) -> bool {
        let l = self.rot.inverse() * (p - self.at);
        l.z < 0.05 && l.z > -1.0 && (l.x / self.half.x).powi(2) + (l.y / self.half.y).powi(2) < 1.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub n: Vec3,
    pub across: [Option<Vec3>; 3],
    pub raw: Option<[Vec3; 3]>,
    pub half: u8,
    pub rank: u32,
    pub mesh: Option<(Arc<crate::bvh::Tree>, u32)>,
    pub place: Option<(Quat, Vec3)>,
}

impl Face {
    pub fn of(m: &delirium_data::bounds::CollisionMesh) -> Vec<Option<Face>> {
        let tree = crate::bvh::Tree::new(m);
        (0..m.triangles.len()).map(|k| {
            let across = m.neighbours.get(k)?.map(|j| m.normals.get(j as usize).map(|&v| Vec3::from(v)));
            let raw = m.corners.get(k).filter(|&&c| c != m.triangles[k]).map(|c| c.map(|i| Vec3::from(m.positions[i as usize])));
            Some(Face { n: Vec3::from(*m.normals.get(k)?), across, raw, half: m.halves.get(k).copied().unwrap_or(0), rank: m.order.get(k).copied().unwrap_or(k as u32), mesh: tree.clone().map(|t| (t, k as u32)), place: None })
        }).collect()
    }

    pub fn turned(self, q: Quat, place: impl Fn(Vec3) -> Vec3) -> Face {
        let at = place(Vec3::ZERO);
        Face { n: q * self.n, across: self.across.map(|a| a.map(|a| q * a)), raw: self.raw.map(|r| r.map(place)), place: Some((q, at)), ..self }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poly {
    pub first: (u32, u32),
    pub n: Vec3,
    pub edges: [Option<(Vec3, Vec3, Vec3)>; 4],
}

impl Poly {
    pub fn clamp(&self, hits: &mut [(Vec3, Vec3)], margin: f32) {
        let tol = margin * margin * 1.0816;
        let (mut axis, mut cos) = (self.n, 1.0);
        for (a, b, m) in self.edges.into_iter().flatten() {
            if hits.iter().any(|&(p, _)| on_edge(a, b, p, tol)) {
                expand_cone(&mut axis, &mut cos, m);
            }
        }
        for (_, n) in hits.iter_mut() {
            *n = clamp_to_cone(*n, axis, cos);
        }
    }
}

fn on_edge(a: Vec3, b: Vec3, p: Vec3, tol: f32) -> bool {
    let (ap, e) = (p - a, b - a);
    let t = ap.dot(e);
    let c = if !(t > 1e-12) {
        -ap
    } else if e.dot(e) - t > 1e-12 {
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
    let (w, q) = (h.cos() as f32, axis * h.sin() as f32);
    let d = v.z.fma(q.z, v.x.fma(q.x, q.y * v.y)) * 2.0;
    let c = Vec3::new(q.y.fma(v.z, -(q.z * v.y)), q.z.fma(v.x, -(v.z * q.x)), v.y.fma(q.x, -(q.y * v.x)));
    c * (w * 2.0) + (v * (w * w).fma(2.0, -1.0) + q * d)
}

fn expand_cone(axis: &mut Vec3, cos: &mut f32, v: Vec3) {
    let d = v.x.fma(axis.x, v.y.fma(axis.y, v.z * axis.z));
    if d >= *cos {
        return;
    }
    if *cos >= 0.9999 {
        let h = (v + *axis) * 0.5;
        let l2 = h.dot(h);
        *axis = if l2 > 1e-6 { h * crate::gjk::rsq(l2) } else { Vec3::Z };
        let c = (d + 1.0) * 0.5;
        *cos = if c > 0.0 { c.sqrt() } else { 0.0 };
    } else {
        let (a, b) = (acos(d), acos(*cos));
        let p = v.cross(*axis);
        *axis = turn(p * crate::gjk::rsq(p.dot(p)), (b - a) * 0.5, *axis);
        *cos = (((a + b) * 0.5) as f64).cos() as f32;
    }
}

fn clamp_to_cone(n: Vec3, axis: Vec3, cos: f32) -> Vec3 {
    if n.x.fma(axis.x, n.z.fma(axis.z, n.y * axis.y)) >= cos - 1e-4 {
        n
    } else if cos >= 0.9999 {
        axis
    } else {
        let p = axis.cross(n);
        turn(p * crate::gjk::rsq(p.dot(p)), acos(cos), axis)
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
    (v / CELL).floor() as i32
}

impl Collision {
    pub fn add(&mut self, positions: &[[f32; 3]], triangles: &[[u32; 3]], materials: &[u8]) {
        self.add_chunk_flagged(FIXED, positions, triangles, materials, &[]);
    }

    pub fn add_floor(&mut self, x: f32, y: f32, z: f32, half: f32, material: u8) {
        let size = (half / 100.0).max(1.0);
        let n = (half / size).ceil().max(1.0) as i32;
        let (mut p, mut t) = (vec![], vec![]);
        for i in -n..n {
            for j in -n..n {
                let (a, b, k) = (x + i as f32 * size, y + j as f32 * size, p.len() as u32);
                p.extend([[a, b, z], [a + size, b, z], [a + size, b + size, z], [a, b + size, z]]);
                t.extend([[k, k + 1, k + 2], [k, k + 2, k + 3]]);
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
                let chunk = Chunk { triangles: vec![], faces: vec![], materials: vec![], flags: vec![], cells: vec![], order: vec![], listed: vec![], meshes: vec![], bounds: (Vec3::INFINITY, Vec3::NEG_INFINITY) };
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
            if (hi - lo).truncate().max_element() > 2000.0 || !lo.is_finite() || !hi.is_finite() {
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
                let (rot, pos) = place.unwrap_or((Quat::IDENTITY, Vec3::ZERO));
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
    }

    pub fn chunk_keys(&self) -> impl Iterator<Item = u64> + '_ {
        self.keys.keys().copied()
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
            if c.meshes.is_empty() || !(c.bounds.0 - Vec3::splat(0.5)).cmple(hi).all() || !(c.bounds.1 + Vec3::splat(0.5)).cmpge(lo).all() {
                continue;
            }
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
        ids.sort_by_key(|&(s, i)| (s, self.chunks[s as usize].as_ref().unwrap().order[i as usize]));
        self.tuples(ids)
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
                            let (p, q) = (t[k], t[(k + 1) % 3]);
                            self.cells.get(&(cell(p.x), cell(p.y))).into_iter().flatten().copied()
                                .filter(|&o| o.0 == e.0).filter(|&e| { let o = tri(e); o != t && o.contains(&p) && o.contains(&q) }).min()
                        });
                        self.neighbours.write().unwrap().insert(e, f);
                        f
                    });
                    (normal(t), found.map(|o| o.map(|o| normal(tri(o)))))
                }
            };
            let edge = |k: usize| across[k].map(|m| if n.dot(m.cross(t[(k + 1) % 3] - t[k])) > 0.0 {
                let h = n + m;
                h * crate::gjk::rsq(h.length_squared())
            } else {
                n
            });
            let edges = [edge(0), edge(1), edge(2)];
            let mask = (0..3).filter(|&k| edges[k].is_some()).fold(0, |m, k| m | 1 << k);
            let first = if e.1 > 0 && quad((e.0, e.1 - 1)) { (e.0, e.1 - 1) } else { e };
            let sides = |t: [Vec3; 3], across: [Option<Vec3>; 3], k: usize| across[k].map(|m| (t[k], t[(k + 1) % 3], m));
            let poly = Poly { first, n, edges: if quad(first) {
                let (a, b) = (first, (first.0, first.1 + 1));
                let (fa, fb) = (face(a).unwrap().across, face(b).unwrap().across);
                [sides(tri(a), fa, 0), sides(tri(a), fa, 1), sides(tri(b), fb, 1), sides(tri(b), fb, 2)]
            } else {
                [sides(t, across, 0), sides(t, across, 1), sides(t, across, 2), None]
            } };
            (t, chunk(e).materials[e.1 as usize], [n, edges[0].unwrap_or(n), edges[1].unwrap_or(n), edges[2].unwrap_or(n)], mask, e.0, poly)
        }).collect()
    }

    pub fn segment(&self, from: Vec3, to: Vec3) -> Option<(Vec3, Vec3)> {
        let (lo, hi) = (from.min(to), from.max(to));
        let mut best: Option<(f32, Vec3)> = None;
        for x in cell(lo.x)..=cell(hi.x) {
            for y in cell(lo.y)..=cell(hi.y) {
                for &(slot, id) in self.cells.get(&(x, y)).into_iter().flatten() {
                    let Some(chunk) = self.chunks[slot as usize].as_ref() else { continue };
                    let test = |id: u32| {
                        let face = chunk.faces[id as usize].as_ref();
                        let t = face.and_then(|f| f.raw).unwrap_or(chunk.triangles[id as usize]);
                        let n = face.map_or_else(|| (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero(), |f| f.n);
                        (segment_triangle(t, n, from, to), n)
                    };
                    if chunk.faces[id as usize].as_ref().is_some_and(|f| f.half == 2) && test(id - 1).0.err().is_none_or(|c| c < 4) {
                        continue;
                    }
                    if let (Ok(f), n) = test(id) {
                        if best.is_none_or(|b| f < b.0) {
                            best = Some((f, n));
                        }
                    }
                }
            }
        }
        best.map(|(f, n)| (from + (to - from) * f, n))
    }

    pub fn cast(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3)> {
        self.hit(from, dir, len, false).map(|(t, n, _)| (t, n))
    }

    pub fn contact(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3, u8)> {
        self.hit(from, dir, len, false).map(|(t, n, (slot, id))| (t, n, self.chunks[slot as usize].as_ref().map_or(0, |c| c.materials[id as usize])))
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
                        if ground && n.z.abs() < 0.7 {
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
        _ if !(t >= 0.0) || t > 1.0 => Err(3),
        _ if !(area >= 7.629_394_5e-6) => Err(1),
        _ if !(u >= 0.0) || u > area => Err(4),
        _ if !(v >= 0.0) || u + v > area => Err(5),
        _ => Ok(t),
    }
}

fn ray_triangle([a, b, c]: [Vec3; 3], from: Vec3, dir: Vec3, len: f32, best: Option<f32>) -> Option<(f32, Vec3)> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
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
pub struct Walker {
    pub feet: Vec3,
    pub fall: f32,
    pub grounded: bool,
}

impl Walker {
    pub fn step(&mut self, world: &Collision, wish: Vec2, jump: bool, dt: f32) {
        let mut motion = wish.extend(0.0) * dt;
        for _ in 0..2 {
            let len = motion.length();
            if len < 1e-6 {
                break;
            }
            let dir = motion / len;
            let hit = [STEP + 0.1, 1.2].into_iter().filter_map(|h| world.cast(self.feet + Vec3::Z * h, dir, len + RADIUS)).min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((t, n)) = hit else { break };
            let n = Vec3::new(n.x, n.y, 0.0).normalize_or_zero();
            if n == Vec3::ZERO {
                break;
            }
            let free = (t - RADIUS / dir.dot(n).abs().max(0.1)).clamp(0.0, len);
            let rest = motion - dir * free;
            motion = dir * free + (rest - n * rest.dot(n));
        }
        if self.grounded && let Some((_, n)) = world.cast(self.feet + Vec3::Z * 0.5, Vec3::NEG_Z, 0.8).filter(|(_, n)| n.z >= 0.5) {
            motion.z = -(n.x * motion.x + n.y * motion.y) / n.z;
        }
        self.feet += motion;

        if jump && self.grounded {
            self.fall = -5.0;
            self.grounded = false;
        }
        if self.grounded && let Some(z) = world.ground(self.feet + Vec3::Z * STEP, STEP + 0.3) {
            let up = -self.fall;
            let a = GRAVITY * (6.0 * (z - self.feet.z) + 1.0).min(10.0) - GRAVITY - (2.0 * 58.86f32.sqrt() * up).clamp(-50.0, 50.0);
            let up = up + a * dt;
            self.feet.z += up * dt;
            self.fall = -up;
            return;
        }
        self.fall += GRAVITY * dt;
        let drop = self.fall * dt;
        match world.ground(self.feet + Vec3::Z * STEP, STEP + drop.max(0.0) + 0.05) {
            Some(z) if self.fall >= 0.0 && z >= self.feet.z - drop.max(0.0) - 0.05 => {
                self.feet.z = z;
                self.fall = 0.0;
                self.grounded = true;
            }
            _ => {
                self.feet.z -= drop;
                self.grounded = false;
            }
        }
    }
}

pub fn street_spawn(world: &Collision, around: Vec2) -> Option<Vec3> {
    let tops: Vec<Vec3> = (-10..=10)
        .flat_map(|i| (-10..=10).map(move |j| around + Vec2::new(i as f32, j as f32) * 10.0))
        .filter_map(|p| world.ground(p.extend(1000.0), 1100.0).map(|z| p.extend(z)))
        .filter(|p| p.z > 1.0)
        .filter(|p| world.cast(*p + Vec3::Z * 0.5, Vec3::Z, 30.0).is_none())
        .filter(|p| (0..8).all(|i| {
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

