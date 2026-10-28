use crate::resource::Resource;

#[derive(Default, Clone)]
pub struct CollisionMesh {
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub materials: Vec<u8>,
    pub flags: Vec<u8>,
    pub normals: Vec<[f32; 3]>,
    pub neighbours: Vec<[u32; 3]>,
    pub corners: Vec<[u32; 3]>,
    pub halves: Vec<u8>,
    pub order: Vec<u32>,
    pub poly: Vec<u32>,
    pub bvh: Option<Bvh>,
}

#[derive(Default, Clone)]
pub struct Bvh {
    pub origin: [f32; 3],
    pub scale: [f32; 3],
    pub margin: [f32; 3],
    pub subs: Vec<u8>,
    pub nodes: Vec<u8>,
}

const COMPOSITE: u8 = 12;
const BVH: u8 = 10;
const GEOMETRY: u8 = 4;
const SPHERE: u8 = 0;
const CAPSULE: u8 = 1;
const BOX: u8 = 3;

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
}
fn bef(d: &[u8], at: usize) -> Result<f32, String> {
    be32(d, at).map(f32::from_bits)
}
fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2).map(|b| crate::u16_of([b[0], b[1]])).ok_or("Read out of range".into())
}
fn vec3(d: &[u8], at: usize) -> Result<[f32; 3], String> {
    Ok([bef(d, at)?, bef(d, at + 4)?, bef(d, at + 8)?])
}

fn ptr(p: u32) -> Result<usize, String> {
    if p >> 28 == 5 { Ok((p & 0x0FFF_FFFF) as usize) } else { Err(format!("Bad bounds pointer {p:08X}")) }
}

pub fn read(r: &Resource) -> Result<(Vec<CollisionMesh>, usize), String> {
    let mut out = vec![];
    let mut skipped = 0;
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]];
    read_bound(&r.data, ptr(be32(&r.data, 8)?)?, identity, &mut out, &mut skipped, 0, false, false)?;
    Ok((out, skipped))
}

pub fn fragment(r: &Resource, prims: bool) -> Result<(Vec<CollisionMesh>, usize), String> {
    let mut out = vec![];
    let mut skipped = 0;
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]];
    let bound = ptr(be32(&r.data, ptr(be32(&r.data, 0xE4)?)? + 0x0C)?)?;
    read_bound(&r.data, bound, identity, &mut out, &mut skipped, 0, true, prims)?;
    Ok((out, skipped))
}

#[derive(Clone, Debug)]
pub struct Part {
    pub kind: u8,
    pub at: usize,
    pub m: [[f32; 3]; 4],
    pub lo: [f32; 3],
    pub hi: [f32; 3],
    pub margin: f32,
    pub hull: Vec<[f32; 3]>,
}

pub fn parts(r: &Resource) -> Result<Vec<Part>, String> {
    let d = &r.data;
    let at = ptr(be32(d, ptr(be32(d, 0xE4)?)? + 0x0C)?)?;
    if d.get(at + 4) != Some(&COMPOSITE) {
        return Err("Fragment bound not a composite".into());
    }
    let (kids, mats, boxes, count) = (ptr(be32(d, at + 0x80)?)?, ptr(be32(d, at + 0x84)?)?, ptr(be32(d, at + 0x8C)?)?, be16(d, at + 0x92)? as usize);
    (0..count).map(|i| {
        let k = ptr(be32(d, kids + i * 4)?)?;
        let kind = *d.get(k + 4).ok_or("Truncated bound")?;
        let hull = if kind == GEOMETRY {
            let list = match be32(d, k + 0x84)? { 0 => be32(d, k + 0xB0)?, p => p };
            let (verts, scale, offset) = (ptr(list)?, vec3(d, k + 0x90)?, vec3(d, k + 0xA0)?);
            (0..be32(d, k + 0xC8)? as usize).map(|v| {
                let q = |j: usize| be16(d, verts + v * 6 + j * 2).map(|q| q as i16 as f32 * scale[j] + offset[j]);
                Ok([q(0)?, q(1)?, q(2)?])
            }).collect::<Result<Vec<_>, String>>()?
        } else {
            vec![]
        };
        Ok(Part { kind, at: k, m: [vec3(d, mats + i * 64)?, vec3(d, mats + i * 64 + 16)?, vec3(d, mats + i * 64 + 32)?, vec3(d, mats + i * 64 + 48)?],
                  lo: vec3(d, boxes + i * 32)?, hi: vec3(d, boxes + i * 32 + 16)?, margin: bef(d, k + 0x70)?, hull })
    }).collect()
}

pub fn at(r: &Resource, at: usize) -> Result<(Vec<CollisionMesh>, usize), String> {
    let mut out = vec![];
    let mut skipped = 0;
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]];
    read_bound(&r.data, at, identity, &mut out, &mut skipped, 0, true, true)?;
    Ok((out, skipped))
}

fn read_bound(d: &[u8], at: usize, m: [[f32; 3]; 4], out: &mut Vec<CollisionMesh>, skipped: &mut usize, depth: u32,
              geometry: bool, prims: bool) -> Result<(), String> {
    match *d.get(at + 4).ok_or("Truncated bound")? {
        COMPOSITE if depth < 4 => {
            let (kids, mats, count) = (ptr(be32(d, at + 0x80)?)?, ptr(be32(d, at + 0x84)?)?, be16(d, at + 0x90)? as usize);
            for i in 0..count {
                let row = |k: usize| vec3(d, mats + i * 64 + k * 16);
                let local = [row(0)?, row(1)?, row(2)?, row(3)?];
                let world = std::array::from_fn(|k| {
                    let v = local[k];
                    let t = if k == 3 { m[3] } else { [0.0; 3] };
                    std::array::from_fn(|j| v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j] + t[j])
                });
                read_bound(d, ptr(be32(d, kids + i * 4)?)?, world, out, skipped, depth + 1, geometry, prims)?;
            }
        }
        BVH => out.push(read_polygons(d, at, &m)?),
        GEOMETRY if geometry => out.push(read_polygons(d, at, &m)?),
        SPHERE | CAPSULE | BOX if prims => out.push(read_box(d, at, &m)?),
        _ => *skipped += 1,
    }
    Ok(())
}

fn read_box(d: &[u8], at: usize, m: &[[f32; 3]; 4]) -> Result<CollisionMesh, String> {
    let (hi, lo) = (vec3(d, at + 0x10)?, vec3(d, at + 0x20)?);
    let mut mesh = CollisionMesh::default();
    for k in 0..8 {
        let p: [f32; 3] = std::array::from_fn(|j| if k >> j & 1 == 1 { hi[j] } else { lo[j] });
        mesh.positions.push(std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j]));
    }
    for [a, b, c, e] in [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]] {
        mesh.triangles.extend([[a, b, c], [a, c, e]]);
        mesh.materials.extend([0, 0]);
        mesh.flags.extend([0, 0]);
    }
    Ok(mesh)
}

fn read_polygons(d: &[u8], at: usize, m: &[[f32; 3]; 4]) -> Result<CollisionMesh, String> {
    let (polys, verts) = (ptr(be32(d, at + 0x8C)?)?, ptr(be32(d, at + 0xB0)?)?);
    let (scale, offset) = (vec3(d, at + 0x90)?, vec3(d, at + 0xA0)?);
    let (vertex_count, poly_count) = (be32(d, at + 0xC8)? as usize, be32(d, at + 0xCC)? as usize);
    let list = match be32(d, at + 0xD0)? {
        0 => vec![],
        p => (0..*d.get(at + 0xD8).ok_or("Truncated bound")? as usize).map(|k| be32(d, ptr(p)? + k * 4)).collect::<Result<Vec<u32>, _>>()?,
    };
    let mut mesh = CollisionMesh::default();
    let turn = |p: [f32; 3]| std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j]);
    let place = |p: [f32; 3]| -> [f32; 3] { let q: [f32; 3] = turn(p); std::array::from_fn(|j| q[j] + m[3][j]) };
    let (mut first, mut k) = (Vec::with_capacity(poly_count), 0u32);
    for i in 0..poly_count {
        first.push(k);
        k += if be16(d, polys + i * 32 + 22)? != 0 { 2 } else { 1 };
    }
    let mut local = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let q = |k: usize| be16(d, verts + i * 6 + k * 2).map(|v| v as i16 as f32);
        local.push([q(0)? * scale[0] + offset[0], q(1)? * scale[1] + offset[1], q(2)? * scale[2] + offset[2]]);
    }
    mesh.positions = local.iter().map(|&p| place(p)).collect();
    for i in 0..poly_count {
        let p = polys + i * 32;
        let id = list.get(*d.get(p + 15).ok_or("Truncated polygon")? as usize).copied().unwrap_or(0);
        let (material, flags) = (id as u8, (id >> 24) as u8);
        let v = [be16(d, p + 16)?, be16(d, p + 18)?, be16(d, p + 20)?, be16(d, p + 22)?].map(u32::from);
        if v.iter().any(|&v| v as usize >= vertex_count) {
            return Err(format!("Polygon {i} indexes past {vertex_count} vertices"));
        }
        let nb = |k: usize| be16(d, p + 24 + k * 2).map(|q| if q == 0xFFFF { u32::MAX } else { first.get(q as usize).copied().unwrap_or(u32::MAX) });
        let n = turn(vec3(d, p)?);
        mesh.normals.push(n);
        if v[3] == 0 {
            mesh.triangles.push([v[0], v[1], v[2]]);
            mesh.corners.push([v[0], v[1], v[2]]);
            mesh.neighbours.push([nb(0)?, nb(1)?, nb(2)?]);
            mesh.halves.push(0);
            mesh.poly.push(i as u32);
        } else {
            let (n, t) = (vec3(d, p)?, first[i]);
            let flat = |k: usize| {
                let (a, b) = (local[v[0] as usize], local[v[k] as usize]);
                let e = (a[0] - b[0]) * n[0] + (a[1] - b[1]) * n[1] + (a[2] - b[2]) * n[2];
                place(b.map(|x| x + e))
            };
            let k = mesh.positions.len() as u32;
            mesh.positions.extend([flat(1), flat(3)]);
            mesh.triangles.extend([[v[0], k, v[2]], [v[0], v[2], k + 1]]);
            mesh.corners.extend([[v[0], v[1], v[2]], [v[0], v[2], v[3]]]);
            mesh.neighbours.extend([[nb(0)?, nb(1)?, t + 1], [t, nb(2)?, nb(3)?]]);
            mesh.halves.extend([1, 2]);
            mesh.poly.extend([i as u32, i as u32]);
            mesh.normals.push(turn(n));
            mesh.materials.push(material);
            mesh.flags.push(flags);
        }
        mesh.materials.push(material);
        mesh.flags.push(flags);
    }
    if d.get(at + 4) == Some(&BVH) {
        mesh.order = bvh_order(d, at, &first, mesh.triangles.len() as u32).unwrap_or_default();
        mesh.bvh = bvh(d, at).ok();
    }
    Ok(mesh)
}

fn bvh(d: &[u8], at: usize) -> Result<Bvh, String> {
    let v = ptr(be32(d, at + 0xE0)?)?;
    let (nodes, subs, count) = (ptr(be32(d, v)?)?, ptr(be32(d, v + 0x50)?)?, be16(d, v + 0x56)? as usize);
    let end = (0..count).map(|s| be16(d, subs + s * 16 + 14)).collect::<Result<Vec<_>, _>>()?.into_iter().max().unwrap_or(0) as usize;
    let bytes = |at: usize, n: usize| d.get(at..at + n).map(<[u8]>::to_vec).ok_or("Truncated BVH");
    Ok(Bvh { origin: vec3(d, v + 0x10)?, scale: vec3(d, v + 0x30)?, margin: vec3(d, at + 0x70)?, subs: bytes(subs, count * 16)?, nodes: bytes(nodes, end * 16)? })
}

fn bvh_order(d: &[u8], at: usize, first: &[u32], tris: u32) -> Option<Vec<u32>> {
    let bvh = ptr(be32(d, at + 0xE0).ok()?).ok()?;
    let (nodes, subs, count) = (ptr(be32(d, bvh).ok()?).ok()?, ptr(be32(d, bvh + 0x50).ok()?).ok()?, be16(d, bvh + 0x56).ok()?);
    let (mut rank, mut next) = (vec![u32::MAX; tris as usize], 0);
    for s in (0..count as usize).map(|s| subs + s * 16) {
        for n in (be16(d, s + 12).ok()?..be16(d, s + 14).ok()?).map(|k| nodes + k as usize * 16) {
            let start = be16(d, n + 12).ok()? as usize;
            for p in start..start + *d.get(n + 14)? as usize {
                for t in *first.get(p)?..first.get(p + 1).copied().unwrap_or(tris) {
                    *rank.get_mut(t as usize)? = next;
                    next += 1;
                }
            }
        }
    }
    (next == tris && !rank.contains(&u32::MAX) && rank.iter().enumerate().any(|(k, &r)| r != k as u32)).then_some(rank)
}

