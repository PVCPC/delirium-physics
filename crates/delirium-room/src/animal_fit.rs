use bevy::prelude::*;
use crate::rdr2frag::{self, Frag};

pub struct Imported {
    pub name: String,
    pub text: String,
    pub tailless: bool,
    pub colour: Color,
    pub look: std::sync::Arc<Look>,
    pub stiff: f32,
    pub lock: bool,
    pub object: bool,
}

#[derive(Default)]
pub struct Piece {
    pub mat: Option<usize>,
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
}

#[derive(Default)]
pub struct Look {
    pub pieces: Vec<Piece>,
    pub mats: Vec<crate::usd::Mat>,
    pub bind: Vec<Mat4>,
    pub farm: bool,
}

pub static IMPORTED: std::sync::Mutex<Vec<Imported>> = std::sync::Mutex::new(vec![]);
pub const MOST: usize = 7;

pub fn count() -> usize {
    rdr2frag::ANIMALS.len() + IMPORTED.lock().unwrap().len()
}

pub fn name(i: usize) -> String {
    rdr2frag::ANIMALS.get(i).map(|a| a.0.to_string())
        .or_else(|| IMPORTED.lock().unwrap().get(i - rdr2frag::ANIMALS.len()).map(|a| a.name.clone())).unwrap_or_default()
}

pub fn text(i: usize) -> Option<String> {
    rdr2frag::ANIMALS.get(i).map(|a| a.1.to_string()).or_else(|| IMPORTED.lock().unwrap().get(i - rdr2frag::ANIMALS.len()).map(|a| a.text.clone()))
}

pub fn tailless(i: usize) -> bool {
    (3..=6).contains(&i) || IMPORTED.lock().unwrap().get(i.wrapping_sub(rdr2frag::ANIMALS.len())).is_some_and(|a| a.tailless)
}

pub fn colour(i: usize) -> Option<Color> {
    IMPORTED.lock().unwrap().get(i.wrapping_sub(rdr2frag::ANIMALS.len())).map(|a| a.colour)
}

pub fn look(i: usize) -> Option<std::sync::Arc<Look>> {
    IMPORTED.lock().unwrap().get(i.wrapping_sub(rdr2frag::ANIMALS.len())).map(|a| a.look.clone())
}

pub fn legs(i: usize) -> (f32, bool) {
    IMPORTED.lock().unwrap().get(i.wrapping_sub(rdr2frag::ANIMALS.len())).map_or((0.0, false), |a| (a.stiff, a.lock))
}

pub fn object(i: usize) -> bool {
    IMPORTED.lock().unwrap().get(i.wrapping_sub(rdr2frag::ANIMALS.len())).is_some_and(|a| a.object)
}

pub fn set_object(i: usize, on: bool) {
    if let Some(a) = IMPORTED.lock().unwrap().get_mut(i.wrapping_sub(rdr2frag::ANIMALS.len())) {
        a.object = on;
    }
}

pub fn set_legs(i: usize, stiff: f32, lock: bool) {
    if let Some(a) = IMPORTED.lock().unwrap().get_mut(i.wrapping_sub(rdr2frag::ANIMALS.len())) {
        (a.stiff, a.lock) = (stiff.clamp(0.0, 1.0), lock);
    }
}

pub fn import(path: &str) -> Result<(usize, String), String> {
    let m = read(path)?;
    let (fr, tailless, look, log) = fit(&m)?;
    let name = std::path::Path::new(path).file_stem().map_or("Model".into(), |s| s.to_string_lossy().into_owned());
    let n = m.tris.len().max(1) as f32;
    let c = m.tris.iter().fold([0.0f32; 3], |s, t| [s[0] + t.1[0], s[1] + t.1[1], s[2] + t.1[2]]);
    let farm = look.farm;
    let mut all = IMPORTED.lock().unwrap();
    if all.len() >= MOST {
        all.pop();
    }
    all.push(Imported { name, text: fr.to_text(), tailless, colour: Color::srgb(c[0] / n, c[1] / n, c[2] / n), look: std::sync::Arc::new(look),
                          stiff: 0.0, lock: false, object: !farm });
    Ok((rdr2frag::ANIMALS.len() + all.len() - 1, log))
}

pub struct Model {
    pub bones: Vec<(String, Option<usize>, Vec3, Vec3)>,
    pub verts: Vec<Vec3>,
    pub tris: Vec<([Vec3; 3], [f32; 4])>,
    pub corners: Vec<(usize, Vec3, Option<[f32; 2]>)>,
    pub tri_mat: Vec<Option<usize>>,
    pub mats: Vec<crate::usd::Mat>,
    pub skin: Vec<Vec<(usize, f32)>>,
}

pub fn read(path: &str) -> Result<Model, String> {
    use ufbx::CoordinateAxis as A;
    let ext = std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext != "fbx" && ext != "obj" {
        return Err(format!("an animal from .{ext}: only .fbx and .obj for now"));
    }
    let opts = ufbx::LoadOpts { target_axes: ufbx::CoordinateAxes { right: A::PositiveX, up: A::PositiveZ, front: A::NegativeY }, target_unit_meters: 1.0,
                                generate_missing_normals: true, ..Default::default() };
    let scene = ufbx::load_file(path, opts).map_err(|e| e.description.to_string())?;
    let v3 = |v: ufbx::Vec3| Vec3::new(v.x as f32, v.y as f32, v.z as f32);
    let skinned: Vec<u32> = scene.skin_clusters.iter().filter_map(|c| c.bone_node.as_ref().map(|b| b.element.element_id)).collect();
    let nodes: Vec<&ufbx::Node> = scene.nodes.iter().filter(|b| b.bone.is_some() || skinned.contains(&b.element.element_id)).map(|b| &**b).collect();
    let ids: Vec<u32> = nodes.iter().map(|b| b.element.element_id).collect();
    let at = |b: &ufbx::Node| { let m = &b.node_to_world; Vec3::new(m.m03 as f32, m.m13 as f32, m.m23 as f32) };
    let mut bones = vec![];
    for b in &nodes {
        let head = at(b);
        let kids: Vec<&ufbx::Node> = b.children.iter().filter(|c| ids.contains(&c.element.element_id)).map(|c| &**c).collect();
        let m = &b.node_to_world;
        let y = Vec3::new(m.m01 as f32, m.m11 as f32, m.m21 as f32).normalize_or(Vec3::Z);
        let tail = match kids.len() {
            1 => at(kids[0]),
            0 => head + y * 0.1,
            n => head + y * kids.iter().map(|k| at(k).distance(head)).sum::<f32>() / n as f32,
        };
        bones.push((b.element.name.to_string(), b.parent.as_ref().and_then(|p| ids.iter().position(|&i| i == p.element.element_id)), head, tail));
    }
    let (mut verts, mut tris, mut corners, mut tri_mat, mut mats, mut skin) = (vec![], vec![], vec![], vec![], vec![], vec![]);
    for node in scene.nodes.iter() {
        let Some(mesh) = node.mesh.as_ref() else { continue };
        let m = &node.geometry_to_world;
        let nm = ufbx::matrix_for_normals(m);
        let first = verts.len();
        verts.extend(mesh.vertices.iter().map(|&v| v3(ufbx::transform_position(m, v))));
        skin.extend((0..mesh.vertices.len()).map(|v| match mesh.skin_deformers.first().and_then(|d| d.vertices.get(v).map(|x| (d, x))) {
            Some((d, x)) => d.weights.iter().skip(x.weight_begin as usize).take(x.num_weights as usize).filter_map(|w| {
                let b = d.clusters.get(w.cluster_index as usize)?.bone_node.as_ref()?;
                Some((ids.iter().position(|&i| i == b.element.element_id)?, w.weight as f32))
            }).collect(),
            None => vec![],
        }));
        let mat0 = mats.len();
        mats.extend(mesh.materials.iter().map(|q| crate::editor::fbx_mat(q, path)));
        let cols: Vec<[f32; 4]> = mesh.materials.iter().map(|q| {
            let c = q.pbr.base_color.value_vec4;
            if q.pbr.base_color.texture.is_some() { [0.7, 0.7, 0.7, 1.0] } else { [c.x as f32, c.y as f32, c.z as f32, 1.0] }
        }).collect();
        let mut idx = vec![0u32; mesh.max_face_triangles * 3];
        for (f, face) in mesh.faces.iter().enumerate() {
            let col = mesh.face_material.get(f).and_then(|&k| cols.get(k as usize)).copied().unwrap_or([0.7, 0.7, 0.7, 1.0]);
            let mat = mesh.face_material.get(f).map(|&k| k as usize).filter(|&k| k < mesh.materials.len()).map(|k| mat0 + k);
            let n = mesh.triangulate_face(&mut idx, *face) as usize;
            for t in idx[..n * 3].chunks(3) {
                let p = |i: u32| verts[first + mesh.vertex_indices[i as usize] as usize];
                tris.push(([p(t[0]), p(t[1]), p(t[2])], col));
                tri_mat.push(mat);
                for &i in t {
                    let i = i as usize;
                    let uv = mesh.vertex_uv.exists.then(|| { let u = mesh.vertex_uv[i]; [u.x as f32, 1.0 - u.y as f32] });
                    corners.push((first + mesh.vertex_indices[i] as usize, v3(ufbx::transform_direction(&nm, mesh.vertex_normal[i])).normalize_or_zero(), uv));
                }
            }
        }
    }
    if verts.len() < 8 {
        return Err("no mesh in it".into());
    }
    Ok(Model { bones, verts, tris, corners, tri_mat, mats, skin })
}

const FARM_S: f32 = 1.55 / 5.799826;
const FARM_HIPS: [Vec3; 4] = [Vec3::new(0.870984, -1.855739, 3.14008), Vec3::new(-0.921861, -1.855739, 3.14008),
                              Vec3::new(0.738009, 2.195882, 3.140079), Vec3::new(-0.788886, 2.195882, 3.140079)];

struct Quad {
    legs: [Vec<Vec3>; 4],
    neck: Vec<Vec3>,
    head: [Vec3; 2],
    tail: Vec<Vec3>,
    tailless: bool,
    farm: bool,
    yaw: Quat,
    bones: Vec<(String, Option<usize>, Vec3, Vec3)>,
    leg_bones: [Vec<usize>; 4],
}

fn lower(s: &str) -> String {
    s.to_lowercase()
}

fn skeleton(m: &Model, log: &mut Vec<String>) -> Result<(Quad, Vec<Vec3>, Vec<([Vec3; 3], [f32; 4])>), String> {
    let (lo, hi) = m.verts.iter().fold((Vec3::MAX, Vec3::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    let height = hi.z - lo.z;
    let by = |n: &str| m.bones.iter().find(|b| b.0 == n);
    let farm = ["FrontUpLeg.L", "FrontUpLeg.R", "BackUpLeg.L", "BackUpLeg.R"].iter().all(|n| by(n).is_some());
    let (s, floor) = if farm { (FARM_S, 0.0) } else if (0.15..4.0).contains(&height) { (1.0, lo.z) } else {
        log.push(format!("guessed: its units (it was {height:.2} m tall; made 1.6 m)"));
        (1.6 / height, lo.z)
    };
    let to = |p: Vec3| (p - Vec3::Z * floor) * s - Vec3::Z;
    let v: Vec<Vec3> = m.verts.iter().map(|&p| to(p)).collect();
    let bones: Vec<(String, Option<usize>, Vec3, Vec3)> = m.bones.iter().map(|b| (b.0.clone(), b.1, to(b.2), to(b.3))).collect();
    let (top, len) = (height * s - 1.0, ((hi - lo) * s).with_z(0.0).max_element());

    let mut legs: Vec<(Vec<Vec3>, Vec<usize>)> = vec![];
    for (k, b) in bones.iter().enumerate().filter(|b| b.1.2.z < -1.0 + 0.12 * (top + 1.0)) {
        let (mut pts, mut ks) = (vec![b.2], vec![k]);
        let mut at = k;
        while let Some(p) = bones[at].1 {
            let d = bones[p].2 - *pts.last().unwrap();
            if d.z <= 0.5 * d.with_z(0.0).length() || d.z <= 0.0 {
                break;
            }
            pts.push(bones[p].2);
            ks.push(p);
            at = p;
        }
        if pts.last().unwrap().z - pts[0].z > 0.3 * (top + 1.0) && !legs.iter().any(|l| l.0[0] == *pts.last().unwrap()) {
            pts.reverse();
            ks.reverse();
            legs.push((pts, ks));
        }
    }
    legs.sort_by(|a, b| (b.0[0].z - b.0.last().unwrap().z).total_cmp(&(a.0[0].z - a.0.last().unwrap().z)));
    legs.truncate(4);
    let rigged = legs.len() == 4;
    if !rigged {
        if !bones.is_empty() {
            log.push(format!("guessed: its legs from its mesh ({} leg chains in its rig)", legs.len()));
        }
        legs = mesh_legs(&v, top).into_iter().map(|l| (l, vec![])).collect();
    }
    let front_named = |l: &(Vec<Vec3>, Vec<usize>)| -> Option<bool> {
        let n: Vec<String> = l.1.iter().map(|&k| lower(&bones[k].0)).collect();
        if n.iter().any(|n| ["front", "fore", "arm", "hand", "shoulder"].iter().any(|x| n.contains(x))) { Some(true) }
        else if n.iter().any(|n| ["back", "hind", "leg", "foot", "thigh"].iter().any(|x| n.contains(x))) { Some(false) } else { None }
    };
    let named: Vec<Option<bool>> = legs.iter().map(front_named).collect();
    let by_name = named.iter().filter(|x| **x == Some(true)).count() == 2 && named.iter().filter(|x| **x == Some(false)).count() == 2;
    let (legs, leg_bones): (Vec<Vec<Vec3>>, Vec<Vec<usize>>) = legs.into_iter().unzip();
    let mid = legs.iter().map(|l| l[0]).sum::<Vec3>() / 4.0;
    let front_mid = legs.iter().zip(&named).filter(|l| *l.1 == Some(true)).map(|l| l.0[0]).sum::<Vec3>() / 2.0;
    let hind_mid = legs.iter().zip(&named).filter(|l| *l.1 == Some(false)).map(|l| l.0[0]).sum::<Vec3>() / 2.0;
    let axis = if by_name { (front_mid - hind_mid).truncate().normalize_or(Vec2::Y) } else {
        let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
        for l in &legs {
            let d = l[0] - mid;
            (xx, xy, yy) = (xx + d.x * d.x, xy + d.x * d.y, yy + d.y * d.y);
        }
        let a = 0.5 * (2.0 * xy).atan2(xx - yy);
        Vec2::new(a.cos(), a.sin())
    };
    let head_bone = bones.iter().enumerate().filter(|b| { let n = lower(&b.1.0); n.contains("head") && !n.contains("end") })
        .min_by_key(|b| { let mut d = 0; let mut a = b.0; while let Some(p) = bones[a].1 { d += 1; a = p; } d }).map(|b| b.0);
    let along = |p: Vec3| (p - mid).truncate().dot(axis);
    let ends = legs.iter().map(|l| along(l[0])).fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(x), b.max(x)));
    let fwd = match head_bone {
        _ if by_name => if along(front_mid) > 0.0 { axis } else { -axis },
        Some(h) => if along(bones[h].2) > 0.0 { axis } else { -axis },
        None => {
            log.push("guessed: its front (the end more of its mesh stands out past its legs)".into());
            let (back, front) = v.iter().map(|&p| along(p)).fold((0, 0), |(b, f), x| (b + (x < ends.0) as usize, f + (x > ends.1) as usize));
            if front >= back { axis } else { -axis }
        }
    };
    let mut yaw = (-fwd.x).atan2(-fwd.y);
    let q = (yaw / std::f32::consts::FRAC_PI_2).round() * std::f32::consts::FRAC_PI_2;
    if (yaw - q).abs() < 10f32.to_radians() {
        yaw = q;
    }
    let turn = Quat::from_rotation_z(yaw);
    let o = Vec3::new(mid.x, mid.y, 0.0);
    let k = (ends.1 - ends.0) / ((FARM_HIPS[2].y - FARM_HIPS[0].y) * FARM_S);
    let target = (FARM_HIPS.iter().sum::<Vec3>() / 4.0 * FARM_S * k).with_z(0.0);
    let tr = |p: Vec3| if farm { p } else { turn * (p - o) + target };
    let v: Vec<Vec3> = v.iter().map(|&p| tr(p)).collect();
    let bones: Vec<(String, Option<usize>, Vec3, Vec3)> = bones.into_iter().map(|b| (b.0, b.1, tr(b.2), tr(b.3))).collect();
    let mut legs: Vec<(Vec<Vec3>, Vec<usize>)> = legs.into_iter().map(|l| l.into_iter().map(tr).collect()).zip(leg_bones).collect();
    let my = legs.iter().map(|l| l.0[0].y).sum::<f32>() / 4.0;
    legs.sort_by(|a, b| (a.0[0].y < my).cmp(&(b.0[0].y < my)).then(a.0[0].x.total_cmp(&b.0[0].x)));
    if rigged {
        let say = |l: &(Vec<Vec3>, Vec<usize>)| l.1.iter().map(|&k| bones[k].0.as_str()).collect::<Vec<_>>().join(">");
        log.push(format!("legs{}: hind right {}, hind left {}, front right {}, front left {}", if by_name { " (front and hind by their names)" } else { "" },
                         say(&legs[0]), say(&legs[1]), say(&legs[2]), say(&legs[3])));
    }
    let leg_bones: [Vec<usize>; 4] = std::array::from_fn(|i| legs.get(i).map_or(vec![], |l| l.1.clone()));
    let mut legs: Vec<Vec<Vec3>> = legs.into_iter().map(|l| l.0).collect();
    for l in &mut legs {
        let h = l.last_mut().unwrap();
        h.z = -1.0;
    }
    let legs: [Vec<Vec3>; 4] = legs.try_into().map_err(|_| "not four legs")?;
    let front_y = (legs[2][0].y + legs[3][0].y) / 2.0;
    let hind_y = (legs[0][0].y + legs[1][0].y) / 2.0;
    let belly = v.iter().filter(|p| p.y > front_y && p.y < hind_y && p.x.abs() < 0.25 * len).map(|p| p.z).fold(f32::MAX, f32::min).min(top);
    let top_at = |y: f32, w: f32| v.iter().filter(|p| (p.y - y).abs() < w).map(|p| p.z).fold(belly, f32::max);
    let (neck, head) = match head_bone {
        Some(h) => {
            let mut chain = vec![h];
            while let Some(p) = bones[*chain.last().unwrap()].1 {
                chain.push(p);
                if bones[p].2.y >= front_y {
                    break;
                }
            }
            chain.reverse();
            let (s, e) = (bones[h].2, bones[h].3);
            let e = if e.distance(s) < 0.02 { v.iter().copied().min_by(|a, b| a.y.total_cmp(&b.y)).unwrap_or(s) } else { e };
            (chain.iter().map(|&k| bones[k].2).collect::<Vec<_>>(), [s, e])
        }
        None => {
            log.push("guessed: its neck and head (its frontmost point the muzzle; the neck from over its front legs)".into());
            let tip = v.iter().filter(|p| p.z > belly).copied().min_by(|a, b| a.y.total_cmp(&b.y)).unwrap_or(Vec3::new(0.0, front_y - 0.5, 0.0)).with_x(0.0);
            let base = Vec3::new(0.0, front_y, (belly + top_at(front_y, 0.1 * len)) / 2.0 + 0.25 * (top_at(front_y, 0.1 * len) - belly));
            let start = tip + (base - tip) * 0.4;
            (vec![base, (base + start) / 2.0, start], [start, tip])
        }
    };
    if neck.len() < 2 {
        return Err("its head bone has no neck".into());
    }
    let tails: Vec<usize> = (0..bones.len()).filter(|&k| { let n = lower(&bones[k].0); n.contains("tail") && !n.contains("end") }).collect();
    let (tail, tailless) = if let Some(&t0) = tails.iter().find(|&&k| bones[k].1.is_none_or(|p| !tails.contains(&p))) {
        let mut c = vec![t0];
        while let Some(k) = tails.iter().copied().find(|&k| bones[k].1 == Some(*c.last().unwrap())) {
            c.push(k);
        }
        let mut p: Vec<Vec3> = c.iter().map(|&k| bones[k].2).collect();
        p.push(bones[*c.last().unwrap()].3);
        (p, false)
    } else if let Some(b) = bones.iter().find(|b| b.0 == "Back") {
        (vec![b.2, b.3], true)
    } else {
        let rear = v.iter().filter(|p| p.z > belly).copied().max_by(|a, b| a.y.total_cmp(&b.y)).unwrap_or(Vec3::new(0.0, hind_y, belly));
        let rump = Vec3::new(0.0, hind_y, top_at(hind_y, 0.1 * len) - 0.1 * (top_at(hind_y, 0.1 * len) - belly));
        let none = rear.y - hind_y < 0.15 * (hind_y - front_y);
        log.push(format!("guessed: its tail ({})", if none { "none: its tail links on its rump, not drawn" } else { "its mesh's rearmost point" }));
        (if none { vec![rump, rump + Vec3::new(0.0, 0.1, -0.1) * len] } else { vec![rump, rear] }, none)
    };
    if rigged {
        log.push(format!("rig: {} bones; legs, {}, {}", bones.len(), if head_bone.is_some() { "head bone" } else { "head guessed" }, if tails.is_empty() { "no tail bones" } else { "tail bones" }));
    }
    let tris = m.tris.iter().map(|t| (t.0.map(|p| tr(to(p))), t.1)).collect();
    Ok((Quad { legs, neck, head, tail, tailless, farm, yaw: if farm { Quat::IDENTITY } else { turn }, bones, leg_bones }, v, tris))
}

fn mesh_legs(v: &[Vec3], top: f32) -> Vec<Vec<Vec3>> {
    let low: Vec<Vec3> = v.iter().filter(|p| p.z < -1.0 + 0.25 * (top + 1.0)).copied().collect();
    let c = low.iter().sum::<Vec3>() / low.len().max(1) as f32;
    let (sx, sy) = low.iter().fold((0.0, 0.0), |s, p| (s.0 + (p.x - c.x).abs(), s.1 + (p.y - c.y).abs()));
    let long_y = sy >= sx;
    let mut bunch = [(Vec3::ZERO, 0usize); 4];
    for p in &low {
        let (a, b) = if long_y { (p.y > c.y, p.x > c.x) } else { (p.x > c.x, p.y > c.y) };
        let k = a as usize * 2 + b as usize;
        bunch[k] = (bunch[k].0 + *p, bunch[k].1 + 1);
    }
    let belly = -1.0 + 0.35 * (top + 1.0);
    bunch.iter().map(|&(s, n)| {
        let f = s / n.max(1) as f32;
        let hip = f.with_z(belly + 0.33 * (top - belly));
        vec![hip, f.with_z(-1.0)]
    }).collect()
}

fn frame_of(caps: &[rdr2frag::Capsule]) -> (Mat3, Vec3) {
    let m: f32 = caps.iter().map(|c| c.mass).sum();
    (caps[0].turn, caps.iter().map(|c| (c.at + c.turn * c.cg) * c.mass).sum::<Vec3>() / m)
}

fn arc(pts: &[Vec3]) -> Vec<f32> {
    let mut l = vec![0.0];
    for w in pts.windows(2) {
        l.push(l.last().unwrap() + w[0].distance(w[1]));
    }
    let e = l.last().copied().unwrap_or(1.0).max(1e-9);
    l.iter().map(|x| x / e).collect()
}

fn project(pts: &[Vec3], p: Vec3) -> (f32, usize, Vec3, f32) {
    let t = arc(pts);
    let mut best = (0.0, 0, Vec3::ZERO, f32::MAX);
    for (i, w) in pts.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let u = ((p - a).dot(b - a) / (b - a).dot(b - a).max(1e-12)).clamp(0.0, 1.0);
        let q = a + u * (b - a);
        let d = p.distance(q);
        if d < best.3 {
            best = (t[i] + u * (t[i + 1] - t[i]), i, p - q, d);
        }
    }
    best
}

fn at_frac(pts: &[Vec3], f: f32) -> (Vec3, Vec3) {
    let t = arc(pts);
    let i = (t.partition_point(|&x| x <= f) as isize - 1).clamp(0, pts.len() as isize - 2) as usize;
    let u = (f - t[i]) / (t[i + 1] - t[i]).max(1e-12);
    (pts[i] + u * (pts[i + 1] - pts[i]), pts[i + 1] - pts[i])
}

fn rot(a: Vec3, b: Vec3) -> Mat3 {
    let (a, b) = (a.normalize(), b.normalize());
    let v = a.cross(b);
    let c = a.dot(b);
    if v.length() < 1e-9 {
        return if c > 0.0 { Mat3::IDENTITY } else { -Mat3::IDENTITY };
    }
    let k = Mat3::from_cols(Vec3::new(0.0, v.z, -v.y), Vec3::new(-v.z, 0.0, v.x), Vec3::new(v.y, -v.x, 0.0));
    Mat3::IDENTITY + k + k * k * (1.0 / (1.0 + c))
}

fn thick(pts: &[Vec3], cloud: &[Vec3]) -> Option<(f32, f32)> {
    let (mut s, mut o, mut n) = (0.0, 0.0, 0);
    for &p in cloud {
        let (f, _, off, _) = project(pts, p);
        if 0.02 < f && f < 0.98 {
            s += off.x.abs();
            o += Vec2::new(off.y, off.z).length();
            n += 1;
        }
    }
    (n > 0).then(|| (s / n as f32, o / n as f32))
}

fn vol(r: f32, h: f32) -> f32 {
    std::f32::consts::PI * r * r * 2.0 * h + 4.0 / 3.0 * std::f32::consts::PI * r.powi(3)
}

fn room(v: Vec3) -> Vec3 {
    Vec3::new(-v.y, v.x, v.z)
}

pub fn fit(m: &Model) -> Result<(Frag, bool, Look, String), String> {
    let mut log = vec![];
    let (quad, vf, tris) = skeleton(m, &mut log)?;
    let mut fr = Frag::from_text(rdr2frag::BUILT_IN_FRAG)?;
    let names: Vec<String> = fr.links.iter().map(|l| l.name.clone()).collect();
    let ix = |n: &str| names.iter().position(|x| x == n).unwrap();
    let mut jw = std::collections::HashMap::new();
    for j in &fr.joints {
        let (t, a) = frame_of(&fr.links[j.parent].capsules);
        jw.insert(names[j.child].clone(), a + t * j.at_parent);
    }
    let j = |n: &str| jw[n];
    let ground = |p: Vec3| p.with_z(-1.0);
    let mut ch: Vec<(&str, Vec<Vec3>, Vec<Vec3>)> = vec![];
    let leg_of = |c: &str, s: char| -> usize { match (c, s) { ("hind", 'r') => 0, ("hind", _) => 1, (_, 'r') => 2, _ => 3 } };
    let ya = [quad.legs[3][0].y, quad.legs[2][0].y, quad.legs[1][0].y, quad.legs[0][0].y];
    let yh = [FARM_HIPS[0].y * FARM_S, FARM_HIPS[1].y * FARM_S, FARM_HIPS[2].y * FARM_S, FARM_HIPS[3].y * FARM_S];
    let span = (ya[2] + ya[3] - ya[0] - ya[1]) / (yh[2] + yh[3] - yh[0] - yh[1]);
    let mut sz = vec![];
    for s in ['r', 'l'] {
        for c in ["hind", "front"] {
            let rp = if c == "hind" {
                let n = |b: &str| j(&format!("skel_{s}_{b}"));
                vec![n("thigh"), n("calf"), n("foot"), ground(n("foot"))]
            } else {
                let n = |b: &str| j(&format!("skel_{s}_{b}"));
                vec![n("upperarm"), n("forearm"), n("hand"), ground(n("hand"))]
            };
            let top = quad.legs[leg_of(c, s)][0];
            let f = (top.z + 1.0) / (FARM_HIPS[0].z * FARM_S);
            sz.push(f);
            let mut hip = Vec3::new(top.x, top.y, (rp[0].z + 1.0) * f - 1.0);
            if c == "front" {
                (hip.x, hip.y) = (rp[0].x * f, rp[0].y * span);
            }
            let fp = if quad.farm { rp.iter().map(|&p| hip + (p - rp[0]) * f).collect() } else {
                let f = (top.z + 1.0) / (rp[0].z + 1.0);
                rp.iter().map(|&p| top + (p - rp[0]) * f).collect()
            };
            ch.push((if c == "hind" { if s == 'r' { "hindr" } else { "hindl" } } else if s == 'r' { "frontr" } else { "frontl" }, rp, fp));
        }
    }
    ch.push(("tail", vec![j("skel_tail1"), j("skel_tail4"), j("skel_tail4") + (j("skel_tail4") - j("skel_tail1"))], quad.tail.clone()));
    ch.push(("neck", vec![j("skel_neck1"), j("skel_neck3"), j("skel_neck5"), j("skel_head")], quad.neck.clone()));
    let hc = &fr.links[ix("skel_head")].capsules[0];
    let muzzle = [1.0f32, -1.0].map(|s| hc.at + hc.turn.y_axis * s * (hc.half - hc.radius)).into_iter().min_by(|a, b| a.y.total_cmp(&b.y)).unwrap() - Vec3::Y * hc.radius;
    ch.push(("head", vec![j("skel_head"), muzzle], quad.head.to_vec()));
    let pel = &fr.links[ix("skel_pelvis")].capsules[0];
    let (rb_r, rb_f) = (Vec3::new(0.0, pel.at.y + pel.half, 0.25), Vec3::new(0.0, -0.80, 0.25));
    let (fr_, bk_) = (quad.legs[3][0], quad.legs[1][0]);
    let ln = bk_.y - fr_.y;
    let fb: Vec<Vec3> = vf.iter().filter(|p| p.z > fr_.z.min(bk_.z) - 0.133 * ln && p.y > fr_.y - 0.256 * ln && p.y < bk_.y + 0.222 * ln).copied().collect();
    if fb.is_empty() {
        return Err("no body found above its legs".into());
    }
    let (zlo, zhi) = fb.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
    let (ylo, yhi) = fb.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    let zc = (zlo + zhi) / 2.0;
    ch.push(("body", vec![rb_r, rb_f], vec![Vec3::new(0.0, yhi, zc), Vec3::new(0.0, ylo, zc)]));
    let of = |c: &str| -> Vec<String> {
        match c {
            "body" => ["skel_spine_root", "skel_pelvis", "skel_spine1", "skel_spine2", "skel_spine3"].map(String::from).to_vec(),
            "tail" => vec!["skel_tail1".into(), "skel_tail4".into()],
            "neck" => ["skel_neck1", "skel_neck3", "skel_neck5"].map(String::from).to_vec(),
            "head" => vec!["skel_head".into()],
            _ => {
                let s = &c[c.len() - 1..];
                if c.starts_with("hind") { ["thigh", "calf", "foot"] } else { ["upperarm", "forearm", "hand"] }.iter().map(|b| format!("skel_{s}_{b}")).collect()
            }
        }
    };
    let chain_of = |n: &str| ch.iter().position(|c| of(c.0).iter().any(|x| x == n)).unwrap();
    let samples = |c: &str| -> Vec<Vec3> {
        let mut out = vec![];
        for n in of(c) {
            for c in &fr.links[ix(&n)].capsules {
                for s in 0..5 {
                    let s = -1.0 + 0.5 * s as f32;
                    for a in 0..12 {
                        let a = a as f32 * std::f32::consts::TAU / 12.0;
                        out.push(c.at + c.turn * Vec3::new(a.cos() * c.radius, s * (c.half - c.radius), a.sin() * c.radius));
                    }
                }
            }
        }
        out
    };
    let owner: Vec<usize> = vf.iter().map(|&v| (0..ch.len()).min_by(|&a, &b| project(&ch[a].2, v).3.total_cmp(&project(&ch[b].2, v).3)).unwrap()).collect();
    let mut k: Vec<(f32, f32)> = ch.iter().enumerate().map(|(ci, (c, rp, fp))| {
        let rs = thick(rp, &samples(c)).unwrap_or((1.0, 1.0));
        let mine: Vec<Vec3> = vf.iter().zip(&owner).filter(|(_, o)| **o == ci).map(|(v, _)| *v).collect();
        let fs = if mine.len() > 3 { thick(fp, &mine) } else { None };
        fs.map_or((1.0, 1.0), |f| (f.0 / rs.0, f.1 / rs.1))
    }).collect();
    for (a, b) in [(0, 2), (1, 3)] {
        let m = ((k[a].0 + k[b].0) / 2.0, (k[a].1 + k[b].1) / 2.0);
        (k[a], k[b]) = (m, m);
    }
    let sz = sz.iter().sum::<f32>() / sz.len() as f32;
    let k: Vec<(f32, f32)> = k.iter().map(|&(a, b)| (a.clamp(0.7 * sz, sz), b.clamp(0.7 * sz, sz))).collect();
    let warp = |n: &str, p: Vec3| {
        let c = chain_of(n);
        let (_, rp, fp) = &ch[c];
        let (f, i, o, _) = project(rp, p);
        let (q, dseg) = at_frac(fp, f);
        q + rot(rp[i + 1] - rp[i], dseg) * Vec3::new(o.x * k[c].0, o.y * k[c].1, o.z * k[c].1)
    };
    const HORSE_VOL: f32 = 0.75726;
    let old: Vec<Vec<rdr2frag::Capsule>> = fr.links.iter().map(|l| l.capsules.iter().map(|c| rdr2frag::Capsule { boxed: c.boxed, ..*c }).collect()).collect();
    let mut spin = vec![Mat3::IDENTITY; fr.links.len()];
    let mut tot = [0.0f32; 2];
    for (i, l) in fr.links.iter_mut().enumerate() {
        let n = &names[i];
        let kc = k[chain_of(n)];
        let kk = (kc.0 * kc.1).sqrt();
        for (ci, c) in l.capsules.iter_mut().enumerate() {
            let ax = c.turn.y_axis;
            let sh = c.half - c.radius;
            let (e1, e2) = (warp(n, c.at + ax * sh), warp(n, c.at - ax * sh));
            let cgw = warp(n, c.at + c.turn * c.cg);
            let r = c.radius * kk;
            let at = (e1 + e2) / 2.0;
            let nsh = e1.distance(e2) / 2.0;
            let rr = if nsh > 1e-4 && sh > 1e-4 { rot(ax, e1 - e2) } else { Mat3::IDENTITY };
            let turn = rr * c.turn;
            let mass = c.mass * vol(r, nsh) / vol(c.radius, sh);
            let g = |a: f32, b: f32| (a / b).powi(2);
            let inertia = c.inertia * (mass / c.mass) * Vec3::new(g(nsh + r, c.half), g(r, c.radius), g(nsh + r, c.half));
            tot[0] += c.mass;
            tot[1] += mass;
            if ci == 0 {
                spin[i] = rr;
            }
            (c.at, c.turn, c.radius, c.half, c.mass, c.inertia, c.cg) = (at, turn, r, nsh + r, mass, inertia, turn.transpose() * (cgw - at));
        }
    }
    for (i, l) in fr.links.iter_mut().enumerate() {
        let f = old[i].iter().map(|c| c.mass).sum::<f32>() / l.capsules.iter().map(|c| c.mass).sum::<f32>() * tot[1] / tot[0] / HORSE_VOL;
        for c in &mut l.capsules {
            c.mass *= f;
            c.inertia *= f;
        }
    }
    let mut joints = std::mem::take(&mut fr.joints);
    for jt in &mut joints {
        let (pt0, pa0) = frame_of(&old[jt.parent]);
        let (ct0, _) = frame_of(&old[jt.child]);
        let w = warp(&names[jt.child], pa0 + pt0 * jt.at_parent);
        let (pt, pa) = frame_of(&fr.links[jt.parent].capsules);
        let (ct, ca) = frame_of(&fr.links[jt.child].capsules);
        let wp = spin[jt.parent] * pt0 * jt.in_parent;
        let wc = wp * (pt0 * jt.in_parent).inverse() * (ct0 * jt.in_child);
        (jt.in_parent, jt.in_child) = (pt.transpose() * wp, ct.transpose() * wc);
        (jt.at_parent, jt.at_child) = (pt.transpose() * (w - pa), ct.transpose() * (w - ca));
    }
    fr.joints = joints;
    let mass: f32 = fr.links.iter().flat_map(|l| &l.capsules).map(|c| c.mass).sum();
    log.push(format!("{mass:.0} kg, {:.2} x RDR2's horse's size", sz));

    let parts: Vec<(usize, &rdr2frag::Capsule)> = [1, 0].into_iter().chain(2..fr.links.len()).flat_map(|l| fr.links[l].capsules.iter().map(move |c| (l, c))).collect();
    let room_turn = |m: Mat3| Quat::from_mat3(&Mat3::from_cols(room(m.x_axis), room(m.y_axis), room(m.z_axis))).normalize();
    let bind: Vec<Mat4> = parts.iter().map(|(_, c)| Mat4::from_rotation_translation(room_turn(c.turn), room(c.at)).inverse()).collect();
    let dist = |k: usize, p: Vec3| {
        let c = parts[k].1;
        let e = c.turn.y_axis * (c.half - c.radius);
        project(&[c.at + e, c.at - e], p).3 - c.radius
    };
    let nearest = |p: Vec3, ok: &dyn Fn(usize) -> bool| (0..parts.len()).filter(|&k| ok(k)).min_by(|&a, &b| dist(a, p).total_cmp(&dist(b, p))).unwrap_or(0);
    let leg_links: [Vec<usize>; 4] = [("r", "thigh", "calf", "foot"), ("l", "thigh", "calf", "foot"), ("r", "upperarm", "forearm", "hand"), ("l", "upperarm", "forearm", "hand")]
        .map(|(s, a, b, c)| [a, b, c].iter().map(|n| ix(&format!("skel_{s}_{n}"))).collect());
    let mut bone_part = vec![0usize; quad.bones.len()];
    for (b, bone) in quad.bones.iter().enumerate() {
        bone_part[b] = match quad.leg_bones.iter().enumerate().find_map(|(l, c)| c.iter().position(|&x| x == b).map(|i| (l, i * 3 / c.len()))) {
            Some((l, i)) => parts.iter().position(|p| p.0 == leg_links[l][i]).unwrap_or(0),
            None => nearest((bone.2 + bone.3) / 2.0, &|_| true),
        };
    }
    let joined = |a: usize, b: usize| a == b || fr.joints.iter().any(|j| (j.parent, j.child) == (a, b) || (j.parent, j.child) == (b, a));
    const BLEND: f32 = 0.05;
    let (mut skinned, mut guessed) = (0, 0);
    let wts: Vec<([u16; 4], [f32; 4])> = vf.iter().zip(&m.skin).map(|(&p, sk)| {
        let mut w = vec![0.0f32; parts.len()];
        for &(b, x) in sk {
            w[bone_part[b]] += x;
        }
        if w.iter().sum::<f32>() > 1e-3 {
            skinned += 1;
        } else {
            guessed += 1;
            let best = nearest(p, &|_| true);
            let d0 = dist(best, p);
            for k in (0..parts.len()).filter(|&k| joined(parts[k].0, parts[best].0)) {
                w[k] = (1.0 - (dist(k, p) - d0) / BLEND).max(0.0);
            }
        }
        crate::editor::top4(&w)
    }).collect();
    log.push(format!("mesh: {} vertices ({skinned} by its skin, {guessed} by their nearest links), {} triangles, every one drawn", vf.len(), m.tris.len()));
    let mut on = std::collections::BTreeMap::<&str, usize>::new();
    for w in &wts {
        *on.entry(names[parts[w.0[0] as usize].0].as_str()).or_default() += 1;
    }
    log.push(format!("vertices mostly on: {}", on.iter().map(|(n, k)| format!("{} {k}", n.trim_start_matches("skel_"))).collect::<Vec<_>>().join(", ")));
    if !quad.bones.is_empty() {
        log.push(format!("bones on: {}", quad.bones.iter().zip(&bone_part).map(|(b, &k)| format!("{}>{}", b.0.rsplit(':').next().unwrap_or(&b.0), names[parts[k].0].trim_start_matches("skel_"))).collect::<Vec<_>>().join(", ")));
    }
    let yaw = quad.yaw;
    let mut look = Look { pieces: vec![], mats: m.mats.clone(), bind, farm: quad.farm };
    for (t, &mat) in m.tri_mat.iter().enumerate() {
        let k = match look.pieces.iter().position(|q| q.mat == mat) {
            Some(k) => k,
            None => { look.pieces.push(Piece { mat, ..Default::default() }); look.pieces.len() - 1 }
        };
        let q = &mut look.pieces[k];
        for &(v, n, uv) in &m.corners[t * 3..t * 3 + 3] {
            q.pos.push(room(vf[v]).to_array());
            q.nrm.push(room(yaw * n).to_array());
            if let Some(uv) = uv {
                q.uv.push(uv);
            }
            q.joints.push(wts[v].0);
            q.weights.push(wts[v].1);
        }
    }
    for q in &mut look.pieces {
        if q.uv.len() != q.pos.len() {
            q.uv.clear();
        }
    }
    Ok((fr, quad.tailless, look, log.join("; ")))
}

