use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy_egui::egui;

pub const COLLIDE_TRIS: usize = 4000;

pub const LEVEL_CHUNK: u64 = 0x4C45_5600_0000;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Shape {
    Box,
    Ramp,
    Cylinder,
    Sphere,
    Pyramid,
    Cone,
    Disc,
    Model,
    Pool,
}

pub const SHAPES: [(Shape, &str); 8] = [(Shape::Box, "box"), (Shape::Ramp, "ramp"), (Shape::Cylinder, "cylinder"), (Shape::Sphere, "sphere"),
                                        (Shape::Pyramid, "pyramid"), (Shape::Cone, "cone"), (Shape::Disc, "disc"), (Shape::Pool, "pool")];

impl Shape {
    pub fn name(self) -> &'static str {
        SHAPES.iter().find(|s| s.0 == self).map_or("model", |s| s.1)
    }
    fn from_name(n: &str) -> Option<Shape> {
        SHAPES.iter().find(|s| s.1 == n).map(|s| s.0).or((n == "model").then_some(Shape::Model))
    }
    pub fn detail(self) -> Option<(&'static str, u32, std::ops::RangeInclusive<u32>)> {
        match self {
            Shape::Cylinder | Shape::Disc | Shape::Cone => Some(("Sides", 32, 3..=128)),
            Shape::Pyramid => Some(("Sides", 4, 3..=64)),
            Shape::Sphere => Some(("Segments", 24, 4..=96)),
            _ => None,
        }
    }
    pub fn mesh(self, n: u32) -> Mesh {
        let n = self.detail().map_or(0, |d| if n == 0 { d.1 } else { n.clamp(*d.2.start(), *d.2.end()) });
        let up = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        match self {
            Shape::Box | Shape::Model => Cuboid::new(1.0, 1.0, 1.0).into(),
            Shape::Cylinder | Shape::Disc => Cylinder::new(0.5, 1.0).mesh().resolution(n).build().rotated_by(up),
            Shape::Cone => Cone::new(0.5, 1.0).mesh().resolution(n).build().rotated_by(up),
            Shape::Pyramid => {
                let r = if n == 4 { std::f32::consts::FRAC_1_SQRT_2 } else { 0.5 };
                let a0 = -std::f32::consts::FRAC_PI_2 - std::f32::consts::PI / n as f32;
                let mut v: Vec<Vec3> = (0..n).map(|k| { let a = a0 + k as f32 / n as f32 * std::f32::consts::TAU; Vec3::new(a.cos() * r, a.sin() * r, -0.5) }).collect();
                v.push(Vec3::new(0.0, 0.0, 0.5));
                let mut t: Vec<[u32; 3]> = (1..n - 1).map(|k| [0, k + 1, k]).collect();
                t.extend((0..n).map(|k| [k, (k + 1) % n, n]));
                flat_mesh(&v, &t)
            }
            Shape::Sphere => Sphere::new(0.5).mesh().uv(n, (n * 2 / 3).max(3)),
            Shape::Pool => {
                let v: Vec<Vec3> = (0..8).map(|i| Vec3::new(if i & 1 == 0 { -0.5 } else { 0.5 }, if i & 2 == 0 { -0.5 } else { 0.5 }, if i & 4 == 0 { -0.5 } else { 0.5 })).collect();
                flat_mesh(&v, &[[0, 1, 3], [0, 3, 2], [0, 4, 5], [0, 5, 1], [2, 3, 7], [2, 7, 6], [0, 2, 6], [0, 6, 4], [1, 5, 7], [1, 7, 3]])
            }
            Shape::Ramp => {
                let c = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * 0.5;
                let (a, b, cc, d) = (c(-1., -1., -1.), c(1., -1., -1.), c(1., 1., -1.), c(-1., 1., -1.));
                let (e, f) = (c(-1., 1., 1.), c(1., 1., 1.));
                let tris = [[a, cc, b], [a, d, cc], [d, f, cc], [d, e, f], [a, b, f], [a, f, e], [a, e, d], [b, cc, f]];
                let mut pos = vec![];
                let mut nrm = vec![];
                for t in tris {
                    let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize();
                    for p in t {
                        pos.push(p.to_array());
                        nrm.push(n.to_array());
                    }
                }
                Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
            }
        }
    }
    fn triangles(self, n: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
        let m = self.mesh(n);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) = m.attribute(Mesh::ATTRIBUTE_POSITION) else { return (vec![], vec![]) };
        let p: Vec<Vec3> = p.iter().map(|&v| Vec3::from(v)).collect();
        let idx: Vec<u32> = match m.indices() {
            Some(i) => i.iter().map(|k| k as u32).collect(),
            None => (0..p.len() as u32).collect(),
        };
        (p, idx.chunks(3).map(|t| [t[0], t[1], t[2]]).collect())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loose {
    pub kg: f32,
    pub friction: f32,
    pub bounce: f32,
    pub float: f32,
    pub alive: bool,
}

impl Default for Loose {
    fn default() -> Self {
        Loose { kg: 20.0, friction: 0.6, bounce: 0.2, float: 2.0, alive: false }
    }
}

#[derive(Clone, Debug)]
pub struct Piece {
    pub shape: Shape,
    pub pos: Vec3,
    pub rot: Vec3,
    pub size: Vec3,
    pub material: String,
    pub model: String,
    pub color: Option<[f32; 4]>,
    pub glass: bool,
    pub detail: u32,
    pub loose: Option<Loose>,
}

impl Piece {
    pub fn transform(&self) -> Transform {
        let r = self.rot * (std::f32::consts::PI / 180.0);
        Transform { translation: self.pos, rotation: Quat::from_euler(EulerRot::ZYX, r.z, r.y, r.x), scale: self.size.max(Vec3::splat(0.01)) }
    }
    pub fn world_triangles(&self, local: &(Vec<Vec3>, Vec<[u32; 3]>)) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
        let m = self.transform().to_matrix();
        (local.0.iter().map(|&v| m.transform_point3(v).to_array()).collect(), local.1.clone())
    }
    pub fn hit(&self, o: Vec3, d: Vec3, bounds: (Vec3, Vec3)) -> Option<f32> {
        let inv = self.transform().to_matrix().inverse();
        let (lo, ld) = (inv.transform_point3(o), inv.transform_vector3(d));
        let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
        for k in 0..3 {
            let (a, b) = ((bounds.0[k] - lo[k]) / ld[k], (bounds.1[k] - lo[k]) / ld[k]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        (t0 <= t1).then_some(t0)
    }
    fn json(&self) -> serde_json::Value {
        serde_json::json!({"shape": self.shape.name(), "pos": self.pos.to_array(), "rot": self.rot.to_array(), "size": self.size.to_array(), "material": self.material,
                           "model": self.model, "color": self.color, "glass": self.glass, "detail": self.detail,
                           "loose": self.loose.map(|l| [l.kg, l.friction, l.bounce, l.float, l.alive as u8 as f32])})
    }
    fn from_json(v: &serde_json::Value) -> Option<Piece> {
        let v3 = |k: &str| -> Option<Vec3> {
            let a = v.get(k)?.as_array()?;
            Some(Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32))
        };
        Some(Piece { shape: Shape::from_name(v.get("shape")?.as_str()?)?, pos: v3("pos")?, rot: v3("rot").unwrap_or(Vec3::ZERO), size: v3("size")?,
                     material: v.get("material").and_then(|m| m.as_str()).unwrap_or("CONCRETE").to_string(),
                     model: v.get("model").and_then(|m| m.as_str()).unwrap_or("").to_string(),
                     color: v.get("color").and_then(|c| serde_json::from_value(c.clone()).ok()),
                     glass: v.get("glass").and_then(|g| g.as_bool()).unwrap_or(false),
                     loose: v.get("loose").and_then(|l| serde_json::from_value::<Vec<f32>>(l.clone()).ok()).filter(|l| l.len() >= 3)
                         .map(|l| Loose { kg: l[0], friction: l[1], bounce: l[2], float: l.get(3).copied().unwrap_or(2.0), alive: l.get(4).is_some_and(|&a| a > 0.5) }),
                     detail: v.get("detail").and_then(|d| d.as_u64()).unwrap_or(0) as u32 })
    }
}

pub struct Model {
    pub tris: Option<(Vec<Vec3>, Vec<[u32; 3]>)>,
    obj: Option<(Handle<Mesh>, Color)>,
    pub scene: Option<Handle<bevy::world_serialization::WorldAsset>>,
    usd: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
    splat: Option<(Handle<bevy_gaussian_splatting::PlanarGaussian3d>, Transform)>,
    seen: usize,
    gltf: Vec<(Handle<Mesh>, Option<Handle<StandardMaterial>>, Mat4)>,
    pub rig: Vec<Vec3>,
    pub skin: Vec<Vec<([u16; 4], [f32; 4])>>,
    pub floppy: Vec<usize>,
    pub splats: Vec<bevy_gaussian_splatting::Gaussian3d>,
    pub splat_at: Transform,
    pub error: Option<String>,
}

pub fn set_joint(level: &mut Level, k: usize, v: Vec3) {
    if level.fig_moved.len() <= k {
        level.fig_moved.resize(k + 1, Vec3::NAN);
    }
    (level.fig_moved[k], level.fig_joints[k]) = (v, v);
}

impl Model {
    pub fn drawn(&self) -> Vec<(Handle<Mesh>, Option<Handle<StandardMaterial>>, Mat4)> {
        if !self.usd.is_empty() {
            return self.usd.iter().map(|(m, q)| (m.clone(), Some(q.clone()), Mat4::IDENTITY)).collect();
        }
        if !self.gltf.is_empty() {
            return self.gltf.iter().map(|(m, q, at)| (m.clone(), q.clone(), *at)).collect();
        }
        self.obj.iter().map(|(m, _)| (m.clone(), None, Mat4::IDENTITY)).collect()
    }
}

#[derive(Resource, Default)]
pub struct Level {
    pub pieces: Vec<Piece>,
    pub open: bool,
    pub selected: Option<usize>,
    pub name: String,
    pub note: String,
    pub change: u64,
    drag: Option<(usize, f32, Vec3)>,
    bodies: Vec<crate::world::PieceBody>,
    pub models: std::collections::HashMap<String, Model>,
    pub path: String,
    pub stage: String,
    pub cut: Vec<(bool, usize)>,
    pub start: Option<Vec3>,
    pub placing_start: bool,
    pub rebuild: bool,
    pub ahead: Vec3,
    pub tab: u8,
    pub mode: u8,
    grip: Option<(usize, usize, f32, f32, Piece)>,
    pub menu_ask: bool,
    pub menu: Option<egui::Pos2>,
    pub thing: Option<(u8, usize)>,
    pub thing_name: String,
    pub thing_at: (Vec3, Vec3, f32),
    pub thing_set: bool,
    pub thing_drag: Option<(f32, Vec3)>,
    pub figure: Option<(String, Quat)>,
    pub fig_joints: Vec<Vec3>,
    pub fig_moved: Vec<Vec3>,
    pub fig_pick: Option<usize>,
    pub fig_drag: Option<(usize, Vec3)>,
    pub fig_auto: bool,
}

impl Level {
    pub fn local(&self, p: &Piece) -> (Vec<Vec3>, Vec<[u32; 3]>) {
        match p.shape {
            Shape::Model => self.models.get(&p.model).and_then(|m| m.tris.clone()).unwrap_or_default(),
            s => s.triangles(p.detail),
        }
    }
    pub fn bounds(&self, p: &Piece) -> (Vec3, Vec3) {
        let v = self.models.get(&p.model).filter(|_| p.shape == Shape::Model).and_then(|m| m.tris.as_ref()).map_or(&[][..], |t| &t.0[..]);
        if v.is_empty() {
            return (Vec3::splat(-0.5), Vec3::splat(0.5));
        }
        v.iter().fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |b, &q| (b.0.min(q), b.1.max(q)))
    }
    pub fn framed(&self, p: &Piece) -> (Vec3, f32) {
        let (lo, hi) = self.bounds(p);
        let t = p.transform();
        (t.transform_point((lo + hi) * 0.5), (((hi - lo) * t.scale).max_element() * 1.6).max(2.5))
    }
    pub fn import(&mut self, path: &str, at: Vec3) {
        let path = path.trim().trim_matches('"').to_string();
        let ext = std::path::Path::new(&path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !["glb", "gltf", "obj", "fbx", "usdz", "usdc", "usda", "usd", "ply"].contains(&ext.as_str()) {
            self.note = format!("Can't import .{ext}: .glb, .gltf, .obj, .fbx, .usdz/.usdc/.usda or a splat .ply");
            return;
        }
        if !std::path::Path::new(&path).is_file() {
            self.note = format!("No file {path}");
            return;
        }
        self.pieces.push(Piece { shape: Shape::Model, pos: at, rot: Vec3::ZERO, size: Vec3::ONE, material: "CONCRETE".into(), model: path.clone(), color: None, glass: false, loose: None, detail: 0 });
        self.selected = Some(self.pieces.len() - 1);
        self.change += 1;
        self.note = format!("Imported {path}");
    }
    pub fn dir() -> std::path::PathBuf {
        crate::getup::config_dir().join("levels")
    }
    pub fn save(&mut self) {
        let name = if self.name.trim().is_empty() { "level".to_string() } else { self.name.trim().to_string() };
        let dir = Level::dir().join(&name);
        let mut err = std::fs::create_dir_all(&dir).err();
        let mut pieces = self.pieces.clone();
        for p in pieces.iter_mut().filter(|p| p.shape == Shape::Model) {
            let src = std::path::Path::new(&p.model);
            let Some(file) = src.file_name() else { continue };
            let to = dir.join(file);
            if src != to && !to.is_file() {
                err = err.or(std::fs::copy(src, &to).err());
                let mtl = std::fs::read_to_string(src.with_extension("mtl")).unwrap_or_default();
                let named: Vec<String> = match std::fs::read(src).ok().filter(|_| p.model.ends_with(".gltf")).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
                    Some(j) => ["buffers", "images"].iter().flat_map(|k| j.get(*k).and_then(|a| a.as_array()).cloned().unwrap_or_default())
                        .filter_map(|b| Some(b.get("uri")?.as_str()?.to_string())).filter(|u| !u.starts_with("data:")).collect(),
                    None => mtl.lines().filter(|l| l.trim_start().starts_with("map_") || l.trim_start().starts_with("bump"))
                        .filter_map(|l| l.split_whitespace().last().map(str::to_string)).chain([src.with_extension("mtl").file_name().unwrap().to_string_lossy().into_owned()]).collect(),
                };
                for u in named.iter().filter(|u| !u.contains("..")) {
                    let (from, into) = (src.with_file_name(u), dir.join(u));
                    if from.is_file() {
                        let _ = into.parent().map(std::fs::create_dir_all);
                        let _ = std::fs::copy(from, into);
                    }
                }
            }
            p.model = file.to_string_lossy().into_owned();
        }
        let rel = |m: &str| std::path::Path::new(m).file_name().map_or(m.to_string(), |f| f.to_string_lossy().into_owned());
        let figure = self.figure.as_ref().map(|(m, q)| serde_json::json!({"model": rel(m), "turn": q.to_array(),
            "moved": self.fig_moved.iter().map(|v| v.is_finite().then_some(v.to_array())).collect::<Vec<_>>()}));
        let v = serde_json::json!({"pieces": pieces.iter().map(Piece::json).collect::<Vec<_>>(), "stage": self.stage, "cut": self.cut,
                                   "start": self.start.map(|s| s.to_array()), "figure": figure});
        let p = dir.join(format!("{name}.json"));
        self.note = match err.map_or_else(|| std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap()), Err) {
            Ok(()) => format!("Saved {} (share the whole folder)", dir.display()),
            Err(e) => format!("Couldn't save {}: {e}", p.display()),
        };
    }
    pub fn load(&mut self, name: &str) {
        let dir = Level::dir().join(name);
        let p = if dir.join(format!("{name}.json")).is_file() { dir.join(format!("{name}.json")) } else { Level::dir().join(format!("{name}.json")) };
        let here = |m: &str| if m.is_empty() || std::path::Path::new(m).is_absolute() { m.to_string() } else { dir.join(m).to_string_lossy().into_owned() };
        let v = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
        let got = v.as_ref().and_then(|v| v.get("pieces")?.as_array().map(|a| a.iter().filter_map(Piece::from_json).collect::<Vec<_>>()));
        match got {
            Some(mut pieces) => {
                let v = v.unwrap();
                for p in pieces.iter_mut().filter(|p| p.shape == Shape::Model) {
                    p.model = here(&p.model);
                }
                self.stage = v.get("stage").and_then(|s| s.as_str()).unwrap_or("").to_string();
                self.cut = v.get("cut").and_then(|c| serde_json::from_value(c.clone()).ok()).unwrap_or_default();
                self.start = v.get("start").and_then(|c| serde_json::from_value::<[f32; 3]>(c.clone()).ok()).map(Vec3::from);
                let f = v.get("figure");
                self.figure = f.and_then(|f| Some((here(f.get("model")?.as_str()?), Quat::from_array(serde_json::from_value(f.get("turn")?.clone()).ok()?))));
                self.fig_moved = f.and_then(|f| serde_json::from_value::<Vec<Option<[f32; 3]>>>(f.get("moved")?.clone()).ok()).unwrap_or_default()
                    .into_iter().map(|m| m.map_or(Vec3::NAN, Vec3::from)).collect();
                self.fig_pick = None;
                self.rebuild = true;
                self.pieces = pieces;
                self.selected = None;
                self.name = name.to_string();
                self.change += 1;
                self.note = format!("Loaded {name}");
            }
            None => self.note = format!("Couldn't read {}", p.display()),
        }
    }
    pub fn saved() -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(Level::dir()).into_iter().flatten().flatten().filter_map(|e| {
            let n = e.file_name().to_str()?.to_string();
            if e.path().is_dir() { e.path().join(format!("{n}.json")).is_file().then_some(n) } else { n.strip_suffix(".json").map(str::to_string) }
        }).collect();
        v.sort();
        v.dedup();
        v
    }
}

pub fn load(path: &str, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>,
            clouds: &mut Assets<bevy_gaussian_splatting::PlanarGaussian3d>, assets: &AssetServer,
            registry: &bevy::asset::io::embedded::EmbeddedAssetRegistry) -> Model {
    let mut m = Model { tris: None, obj: None, scene: None, usd: vec![], splat: None, seen: 0, gltf: vec![], rig: vec![], skin: vec![], floppy: vec![], splats: vec![], splat_at: Transform::IDENTITY, error: None };
    let file = std::path::Path::new(path);
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    let dir = format!("level-{:016x}", h.finish());
    if ext.starts_with("usd") || ext == "fbx" {
        let scene = match if ext == "fbx" { fbx(path) } else { crate::usd::read(path) } {
            Ok(s) => s,
            Err(e) => {
                m.error = Some(format!("Couldn't read {path}: {e}"));
                return m;
            }
        };
        let mut texs: std::collections::HashMap<String, Handle<Image>> = Default::default();
        let mut tex = |t: &Option<crate::usd::Tex>, srgb: bool| -> Option<Handle<Image>> {
            let t = t.as_ref()?;
            let key = format!("{dir}/{}", t.name.trim_start_matches("./"));
            if let Some(h) = texs.get(&key) {
                return Some(h.clone());
            }
            registry.insert_asset(std::path::PathBuf::new(), std::path::Path::new(&key), t.bytes.clone()?);
            let h: Handle<Image> = assets.load_builder().with_settings(move |s: &mut bevy::image::ImageLoaderSettings| s.is_srgb = srgb).load(format!("embedded://{key}"));
            texs.insert(key, h.clone());
            Some(h)
        };
        let made: Vec<Handle<StandardMaterial>> = scene.mats.iter().map(|q| {
            let c = Color::srgba(q.base[0], q.base[1], q.base[2], q.base[3]);
            mats.add(StandardMaterial {
                base_color: c,
                base_color_texture: tex(&q.base_tex, true),
                normal_map_texture: tex(&q.normal_tex, false),
                occlusion_texture: tex(&q.occlusion_tex, false),
                emissive: LinearRgba::rgb(q.emissive[0], q.emissive[1], q.emissive[2]) * if q.emissive_tex.is_some() { 1.0 } else { 1.0 },
                emissive_texture: tex(&q.emissive_tex, true),
                metallic: if q.metallic_tex.is_some() { 0.0 } else { q.metallic },
                perceptual_roughness: if q.roughness_tex.is_some() { 0.7 } else { q.roughness },
                alpha_mode: if q.base[3] < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        }).collect();
        let plain = mats.add(StandardMaterial { base_color: Color::srgb(0.7, 0.7, 0.7), perceptual_roughness: 0.8, ..default() });
        let (mut v, mut t) = (vec![], vec![]);
        for part in &scene.parts {
            let o = v.len() as u32;
            v.extend(part.pos.iter().copied());
            t.extend((0..part.pos.len() as u32 / 3).map(|k| [o + k * 3, o + k * 3 + 1, o + k * 3 + 2]));
            let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, part.pos.iter().map(|p| p.to_array()).collect::<Vec<_>>());
            if part.nrm.is_empty() {
                mesh.compute_flat_normals();
            } else {
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, part.nrm.iter().map(|p| p.to_array()).collect::<Vec<_>>());
            }
            if !part.uv.is_empty() {
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, part.uv.iter().map(|p| p.to_array()).collect::<Vec<_>>());
                let _ = mesh.generate_tangents();
            }
            m.usd.push((meshes.add(mesh), part.mat.and_then(|k| made.get(k).cloned()).unwrap_or(plain.clone())));
            m.skin.push(part.skin.clone());
        }
        m.rig = scene.joints.clone();
        m.floppy = scene.floppy.clone();
        if t.is_empty() {
            m.error = Some(format!("No meshes in {path}"));
        }
        m.tris = Some(simplify(v, t, COLLIDE_TRIS));
        return m;
    }
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(e) => {
            m.error = Some(format!("Couldn't read {path}: {e}"));
            return m;
        }
    };
    if ext == "obj" {
        let (v, t, c) = obj(&String::from_utf8_lossy(&bytes), file.parent().unwrap_or(std::path::Path::new(".")));
        if t.is_empty() {
            m.error = Some(format!("No faces in {path}"));
        }
        m.obj = Some((meshes.add(flat_mesh(&v, &t)), c));
        m.tris = Some(simplify(v, t, COLLIDE_TRIS));
        return m;
    }
    if ext == "ply" {
        let (splats, drawn) = match crate::splat::read_ply(&bytes) {
            Ok(s) => s,
            Err(e) => {
                m.error = Some(format!("Couldn't read {path}: {e}"));
                return m;
            }
        };
        let turn = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let (v, t) = crate::splat::surface(&splats, 0.5, 0.5, 120, 3);
        let v: Vec<Vec3> = v.iter().map(|&q| turn * q).collect();
        let (lo, hi) = v.iter().fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |b, &q| (b.0.min(q), b.1.max(q)));
        let off = -Vec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, lo.z);
        if t.is_empty() {
            m.error = Some(format!("No solid splats in {path}"));
        }
        m.splat_at = Transform::from_translation(off).with_rotation(turn);
        m.splats = drawn.clone();
        m.splat = Some((clouds.add(bevy_gaussian_splatting::PlanarGaussian3d::from(drawn)), m.splat_at));
        m.tris = Some(simplify(v.iter().map(|&q| q + off).collect(), t, COLLIDE_TRIS * 5));
        return m;
    }
    let name = file.file_name().map_or("model".into(), |f| f.to_string_lossy().into_owned());
    if ext == "gltf" && let Ok(j) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        for k in ["buffers", "images"] {
            for u in j.get(k).and_then(|a| a.as_array()).into_iter().flatten().filter_map(|b| b.get("uri")?.as_str()) {
                if !u.starts_with("data:") && let Ok(b) = std::fs::read(file.with_file_name(u)) {
                    registry.insert_asset(std::path::PathBuf::new(), std::path::Path::new(&format!("{dir}/{u}")), b);
                }
            }
        }
    }
    registry.insert_asset(std::path::PathBuf::new(), std::path::Path::new(&format!("{dir}/{name}")), bytes);
    m.scene = Some(assets.load(bevy::gltf::GltfAssetLabel::Scene(0).from_asset(format!("embedded://{dir}/{name}"))));
    m
}

fn fbx(path: &str) -> Result<crate::usd::Scene, String> {
    use ufbx::CoordinateAxis as A;
    let opts = ufbx::LoadOpts { target_axes: ufbx::CoordinateAxes { right: A::PositiveX, up: A::PositiveZ, front: A::NegativeY }, target_unit_meters: 1.0,
                                generate_missing_normals: true, ..Default::default() };
    let scene = ufbx::load_file(path, opts).map_err(|e| e.description.to_string())?;
    let v3 = |v: ufbx::Vec3| Vec3::new(v.x as f32, v.y as f32, v.z as f32);
    let mut out = crate::usd::Scene::default();
    let names = delirium_data::nm_asset::PARTS;
    let mut on: std::collections::HashMap<u32, usize> = Default::default();
    let skinned: Vec<u32> = scene.skin_clusters.iter().filter_map(|c| c.bone_node.as_ref().map(|b| b.element.element_id)).collect();
    let mut bones: Vec<&ufbx::Node> = scene.nodes.iter().filter(|b| b.bone.is_some() || skinned.contains(&b.element.element_id)).map(|b| &**b).collect();
    bones.sort_by_key(|b| b.node_depth);
    let ids: Vec<u32> = bones.iter().map(|b| b.element.element_id).collect();
    let named: Vec<(&str, Option<usize>)> = bones.iter().map(|b| (b.element.name.as_ref(), b.parent.as_ref().and_then(|p| ids.iter().position(|&i| i == p.element.element_id)))).collect();
    let mut of = bone_parts(&named);
    let up = floppy(&named, &mut of);
    let mut joints = vec![Vec3::NAN; names.len() + up.len()];
    for (b, k) in bones.iter().zip(of) {
        if let Some(k) = k {
            on.insert(b.element.element_id, k);
            if joints[k].is_nan() {
                let m = &b.node_to_world;
                joints[k] = Vec3::new(m.m03 as f32, m.m13 as f32, m.m23 as f32);
            }
        }
    }
    if joints[..names.len()].iter().filter(|j| !j.is_nan()).count() >= 6 {
        out.joints = joints;
        out.floppy = up;
    }
    for node in scene.nodes.iter() {
        let Some(mesh) = node.mesh.as_ref() else { continue };
        let skin: Vec<([u16; 4], [f32; 4])> = match mesh.skin_deformers.first().filter(|_| !out.joints.is_empty()) {
            Some(d) => {
                let of: Vec<usize> = d.clusters.iter().map(|c| c.bone_node.as_ref().and_then(|b| on.get(&b.element.element_id)).copied().unwrap_or(0)).collect();
                d.vertices.iter().map(|v| {
                    let mut w = vec![0.0f32; out.joints.len()];
                    for x in d.weights.iter().skip(v.weight_begin as usize).take(v.num_weights as usize) {
                        w[of.get(x.cluster_index as usize).copied().unwrap_or(0)] += x.weight as f32;
                    }
                    top4(&w)
                }).collect()
            }
            None => vec![],
        };
        let (m, nm) = (&node.geometry_to_world, ufbx::matrix_for_normals(&node.geometry_to_world));
        let first = out.mats.len();
        out.mats.extend(mesh.materials.iter().map(|q| {
            let (c, r, k) = (q.pbr.base_color.value_vec4, q.pbr.roughness.value_vec4.x, q.pbr.metalness.value_vec4.x);
            let base_tex = fbx_tex(&q.pbr.base_color, path);
            let c = if base_tex.is_some() { [1.0; 4] } else { [c.x as f32, c.y as f32, c.z as f32, 1.0] };
            crate::usd::Mat { base: c, base_tex, normal_tex: fbx_tex(&q.pbr.normal_map, path), roughness: if q.pbr.roughness.has_value { r as f32 } else { 0.7 },
                              metallic: k as f32, ..Default::default() }
        }));
        let mut parts: Vec<crate::usd::Part> = (0..=mesh.materials.len()).map(|k| crate::usd::Part { mat: (k < mesh.materials.len()).then_some(first + k), ..Default::default() }).collect();
        let mut idx = vec![0u32; mesh.max_face_triangles * 3];
        for (f, face) in mesh.faces.iter().enumerate() {
            let k = mesh.face_material.get(f).map_or(mesh.materials.len(), |&k| (k as usize).min(mesh.materials.len()));
            let n = mesh.triangulate_face(&mut idx, *face) as usize;
            let part = &mut parts[k];
            for &i in &idx[..n * 3] {
                let i = i as usize;
                part.pos.push(v3(ufbx::transform_position(m, mesh.vertex_position[i])));
                part.nrm.push(v3(ufbx::transform_direction(&nm, mesh.vertex_normal[i])).normalize_or_zero());
                if let Some(s) = skin.get(mesh.vertex_indices[i] as usize) {
                    part.skin.push(*s);
                }
                if mesh.vertex_uv.exists {
                    let uv = mesh.vertex_uv[i];
                    part.uv.push(Vec2::new(uv.x as f32, 1.0 - uv.y as f32));
                }
            }
        }
        out.parts.extend(parts.into_iter().filter(|p| !p.pos.is_empty()));
    }
    if out.parts.iter().flat_map(|p| &p.pos).map(|p| p.z).fold(f32::MIN, f32::max) - out.parts.iter().flat_map(|p| &p.pos).map(|p| p.z).fold(f32::MAX, f32::min) > 50.0 {
        out.parts.iter_mut().for_each(|p| p.pos.iter_mut().for_each(|q| *q *= 0.01));
        out.joints.iter_mut().for_each(|q| *q *= 0.01);
    }
    Ok(out)
}

fn fbx_tex(map: &ufbx::MaterialMap, fbx: &str) -> Option<crate::usd::Tex> {
    let t = map.texture.as_ref()?;
    let name = std::path::Path::new(t.filename.as_ref()).file_name()?.to_owned();
    if !t.content.is_empty() {
        return Some(crate::usd::Tex { name: name.to_string_lossy().into_owned(), bytes: Some(t.content.to_vec()) });
    }
    let here = std::path::Path::new(fbx).parent()?;
    fn find(d: &std::path::Path, name: &std::ffi::OsStr, depth: u32) -> Option<std::path::PathBuf> {
        let mut dirs = vec![];
        for e in std::fs::read_dir(d).ok()?.flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.file_name() == Some(name) {
                return Some(p);
            }
        }
        dirs.iter().filter(|_| depth > 0).find_map(|p| find(p, name, depth - 1))
    }
    let got = [std::path::PathBuf::from(t.absolute_filename.as_ref()), here.join(t.relative_filename.as_ref())].into_iter().find(|p| p.is_file())
        .or_else(|| find(here.ancestors().nth(2).unwrap_or(here), &name, 3))?;
    Some(crate::usd::Tex { name: name.to_string_lossy().into_owned(), bytes: std::fs::read(got).ok() })
}

fn floppy(bones: &[(&str, Option<usize>)], of: &mut [Option<usize>]) -> Vec<usize> {
    const NAMED: [&str; 13] = ["tail", "hair", "cape", "cloak", "pony", "braid", "skirt", "scarf", "jiggle", "tentacle", "antenna", "ear", "wing"];
    let (base, mut up, mut is) = (delirium_data::nm_asset::PARTS.len(), vec![], vec![false; bones.len()]);
    for (i, &(n, p)) in bones.iter().enumerate() {
        let low = n.rsplit(':').next().unwrap_or(n).to_ascii_lowercase();
        if bone_part(n).is_some() || !(NAMED.iter().any(|k| low.contains(k)) || p.is_some_and(|p| is[p])) {
            continue;
        }
        let Some(parent) = p.and_then(|p| of[p]) else { continue };
        is[i] = true;
        of[i] = Some(base + up.len());
        up.push(parent);
    }
    up
}

#[test]
fn floppy_bones() {
    let b = [("Hips", None), ("Head", Some(0)), ("Tail1", Some(0)), ("Tail2", Some(2)), ("Hair_01", Some(1)), ("LeftHand", Some(0)), ("LeftHandIndex1", Some(5))];
    let mut of = bone_parts(&b);
    let up = floppy(&b, &mut of);
    let (base, part) = (delirium_data::nm_asset::PARTS.len(), |n: &str| delirium_data::nm_asset::PARTS.iter().position(|p| *p == n));
    assert_eq!(up, vec![part("Buttocks").unwrap(), base, part("Head").unwrap()]);
    assert_eq!((of[2], of[3], of[4], of[6]), (Some(base), Some(base + 1), Some(base + 2), of[5]));
}

fn top4(w: &[f32]) -> ([u16; 4], [f32; 4]) {
    let mut o: Vec<usize> = (0..w.len()).collect();
    o.sort_by(|a, b| w[*b].total_cmp(&w[*a]));
    let sum = o[..4].iter().map(|&k| w[k]).sum::<f32>();
    if sum <= 0.0 {
        return ([0; 4], [1.0, 0.0, 0.0, 0.0]);
    }
    (std::array::from_fn(|i| o[i] as u16), std::array::from_fn(|i| w[o[i]] / sum))
}

fn bone_parts(bones: &[(&str, Option<usize>)]) -> Vec<Option<usize>> {
    let names = delirium_data::nm_asset::PARTS;
    let (mut out, mut spine) = (vec![], 0);
    for &(n, up) in bones {
        let k = match bone_part(n) {
            Some("Spine") => {
                spine += 1;
                names.iter().position(|q| *q == format!("Spine{}", (spine - 1).min(3)))
            }
            Some(p) => names.iter().position(|q| *q == p),
            None => None,
        };
        let k = k.or_else(|| up.and_then(|u: usize| out.get(u).copied().flatten()));
        out.push(k);
    }
    out
}

fn bone_part(name: &str) -> Option<&'static str> {
    let low = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
    let toks: Vec<&str> = low.split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty()).collect();
    let side = if low.contains("left") || toks.contains(&"l") { "Left" } else if low.contains("right") || toks.contains(&"r") { "Right" } else { "" };
    let base: String = toks.iter().filter(|t| **t != "l" && **t != "r").copied().collect::<String>().replace("left", "").replace("right", "")
        .chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let limb = match base.as_str() {
        "hips" | "pelvis" => return Some("Buttocks"),
        "neck" => return Some("Neck"),
        "head" => return Some("Head"),
        b if b.starts_with("spine") || b.ends_with("chest") => return Some("Spine"),
        "upleg" | "thigh" | "upperleg" => "Thigh",
        "leg" | "calf" | "shin" | "lowerleg" | "knee" => "Shin",
        "foot" | "ankle" => "Foot",
        "shoulder" | "clavicle" | "collar" => "Clavicle",
        "arm" | "upperarm" => "Upper_Arm",
        "forearm" | "lowerarm" | "elbow" => "Lower_Arm",
        "hand" | "wrist" => "Hand",
        _ => return None,
    };
    let full = format!("{limb}_{side}");
    delirium_data::nm_asset::PARTS.iter().copied().find(|p| !side.is_empty() && *p == full)
}

#[test]
fn bone_names() {
    for (n, p) in [("mixamorig:LeftUpLeg", Some("Thigh_Left")), ("thigh_l", Some("Thigh_Left")), ("upper_arm.R", Some("Upper_Arm_Right")),
                   ("mixamorig:RightForeArm", Some("Lower_Arm_Right")), ("spine_02", Some("Spine")), ("mixamorig:LeftHandIndex1", None),
                   ("Hips", Some("Buttocks")), ("mixamorig:LeftArm", Some("Upper_Arm_Left")), ("calf_r", Some("Shin_Right"))] {
        assert_eq!(bone_part(n), p, "{n}");
    }
}

pub fn model_triangles(level: &mut Level, path: &str, root: Entity, children: &Query<&Children>, parts: &Query<(&Transform, Option<&Mesh3d>, Option<&ChildOf>), impl bevy::ecs::query::QueryFilter>,
                       rigs: &Query<(Option<&bevy::mesh::skinning::SkinnedMesh>, Option<&Name>, Option<&MeshMaterial3d<StandardMaterial>>)>, meshes: &Assets<Mesh>, assets: &AssetServer) -> bool {
    let Some(m) = level.models.get_mut(path) else { return false };
    let Some(scene) = m.scene.as_ref().filter(|_| m.tris.is_none()) else { return false };
    if !assets.is_loaded_with_dependencies(scene) {
        return false;
    }
    let found: Vec<(Entity, Handle<Mesh>)> = children.iter_descendants(root).filter_map(|e| Some((e, parts.get(e).ok()?.1?.0.clone()))).collect();
    let seen = std::mem::replace(&mut m.seen, found.len());
    if found.is_empty() || seen != found.len() {
        return false;
    }
    let (mut v, mut t) = (vec![], vec![]);
    m.gltf.clear();
    let place = |e: Entity| {
        let (mut at, mut k) = (Mat4::IDENTITY, e);
        while k != root && let Ok((tr, _, up)) = parts.get(k) {
            at = tr.to_matrix() * at;
            let Some(up) = up else { break };
            k = up.parent();
        }
        at
    };
    let mut bones: Vec<Entity> = found.iter().filter_map(|(e, _)| rigs.get(*e).ok()?.0).flat_map(|s| s.joints.clone()).collect();
    bones.sort();
    bones.dedup();
    let depth = |e: Entity| std::iter::successors(Some(e), |&k| parts.get(k).ok()?.2.map(|u| u.parent())).count();
    bones.sort_by_key(|&b| depth(b));
    let named: Vec<(String, Option<usize>)> = bones.iter().map(|&b| (rigs.get(b).ok().and_then(|r| r.1).map_or(String::new(), |n| n.to_string()),
        parts.get(b).ok().and_then(|p| p.2).and_then(|u| bones.iter().position(|&x| x == u.parent())))).collect();
    let named: Vec<(&str, Option<usize>)> = named.iter().map(|(n, u)| (n.as_str(), *u)).collect();
    let mut of = bone_parts(&named);
    let up = floppy(&named, &mut of);
    let mut joints = vec![Vec3::NAN; delirium_data::nm_asset::PARTS.len() + up.len()];
    for (&b, k) in bones.iter().zip(&of) {
        if let Some(k) = *k && joints[k].is_nan() {
            joints[k] = place(b).transform_point3(Vec3::ZERO);
        }
    }
    let rigged = joints[..delirium_data::nm_asset::PARTS.len()].iter().filter(|j| !j.is_nan()).count() >= 6;
    m.floppy = if rigged { up } else { vec![] };
    let all = joints.len();
    m.rig = if rigged { joints } else { vec![] };
    m.skin.clear();
    for (e, h) in found {
        let Some(mesh) = meshes.get(&h) else { return false };
        let at = place(e);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { continue };
        m.gltf.push((h.clone(), rigs.get(e).ok().and_then(|r| r.2).map(|q| q.0.clone()), at));
        let skin = rigs.get(e).ok().and_then(|r| r.0).filter(|_| rigged);
        m.skin.push(match (skin, mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX), mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)) {
            (Some(s), Some(bevy::mesh::VertexAttributeValues::Uint16x4(j)), Some(bevy::mesh::VertexAttributeValues::Float32x4(w))) => j.iter().zip(w).map(|(j, w)| {
                let mut p = vec![0.0f32; all];
                for (&j, &w) in j.iter().zip(w) {
                    let k = s.joints.get(j as usize).and_then(|b| bones.iter().position(|x| x == b)).and_then(|i| of[i]).unwrap_or(0);
                    p[k] += w;
                }
                top4(&p)
            }).collect(),
            _ => vec![],
        });
        let o = v.len() as u32;
        v.extend(p.iter().map(|&q| at.transform_point3(Vec3::from(q))));
        let idx: Vec<u32> = mesh.indices().map_or_else(|| (0..p.len() as u32).collect(), |i| i.iter().map(|x| x as u32).collect());
        t.extend(idx.chunks_exact(3).map(|c| [o + c[0], o + c[1], o + c[2]]));
    }
    m.tris = Some(simplify(v, t, COLLIDE_TRIS));
    level.change += 1;
    true
}

pub fn obj(text: &str, dir: &std::path::Path) -> (Vec<Vec3>, Vec<[u32; 3]>, Color) {
    let (mut v, mut t, mut c) = (vec![], vec![], None);
    let mut kd = std::collections::HashMap::new();
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("v") => {
                let n: Vec<f32> = w.take(3).filter_map(|x| x.parse().ok()).collect();
                if n.len() == 3 {
                    v.push(Vec3::new(n[0], n[1], n[2]));
                }
            }
            Some("f") => {
                let ix: Vec<u32> = w.filter_map(|x| x.split('/').next()?.parse::<i64>().ok())
                    .map(|i| if i < 0 { v.len() as i64 + i } else { i - 1 } as u32).collect();
                for k in 1..ix.len().saturating_sub(1) {
                    t.push([ix[0], ix[k], ix[k + 1]]);
                }
            }
            Some("mtllib") => {
                let mut name = String::new();
                for l in std::fs::read_to_string(dir.join(w.collect::<Vec<_>>().join(" "))).unwrap_or_default().lines() {
                    let mut m = l.split_whitespace();
                    match m.next() {
                        Some("newmtl") => name = m.collect::<Vec<_>>().join(" "),
                        Some("Kd") => {
                            let n: Vec<f32> = m.take(3).filter_map(|x| x.parse().ok()).collect();
                            if n.len() == 3 {
                                kd.insert(name.clone(), Color::srgb(n[0], n[1], n[2]));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some("usemtl") if c.is_none() => c = kd.get(&w.collect::<Vec<_>>().join(" ")).copied(),
            _ => {}
        }
    }
    let n = v.len() as u32;
    t.retain(|f| f.iter().all(|&i| i < n));
    for p in &mut v {
        *p = Vec3::new(p.x, -p.z, p.y);
    }
    (v, t, c.unwrap_or(Color::srgb(0.66, 0.65, 0.63)))
}

pub fn simplify(v: Vec<Vec3>, t: Vec<[u32; 3]>, most: usize) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    if t.len() <= most || v.is_empty() {
        return (v, t);
    }
    let (lo, hi) = v.iter().fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |b, &q| (b.0.min(q), b.1.max(q)));
    let size = (hi - lo).length().max(1e-6);
    let mut cell = size / (most as f32).sqrt();
    loop {
        let mut at: std::collections::HashMap<[i32; 3], u32> = Default::default();
        let (mut sum, mut n): (Vec<Vec3>, Vec<f32>) = (vec![], vec![]);
        let map: Vec<u32> = v.iter().map(|&q| {
            let k = ((q - lo) / cell).floor().as_ivec3().to_array();
            let i = *at.entry(k).or_insert_with(|| { sum.push(Vec3::ZERO); n.push(0.0); sum.len() as u32 - 1 });
            sum[i as usize] += q;
            n[i as usize] += 1.0;
            i
        }).collect();
        let mut seen = std::collections::HashSet::new();
        let out: Vec<[u32; 3]> = t.iter().map(|f| f.map(|i| map[i as usize])).filter(|f| f[0] != f[1] && f[1] != f[2] && f[0] != f[2])
            .filter(|f| { let mut s = *f; s.sort(); seen.insert(s) }).collect();
        if out.len() <= most || cell > size {
            return (sum.iter().zip(&n).map(|(s, n)| *s / *n).collect(), out);
        }
        cell *= 1.4;
    }
}

fn flat_mesh(v: &[Vec3], t: &[[u32; 3]]) -> Mesh {
    let (mut pos, mut nrm) = (vec![], vec![]);
    for f in t {
        let [a, b, c] = f.map(|i| v[i as usize]);
        let n = (b - a).cross(c - a).normalize_or_zero();
        for p in [a, b, c] {
            pos.push(p.to_array());
            nrm.push(n.to_array());
        }
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
}

#[derive(Component)]
pub struct LevelMesh(pub usize);

pub fn colour(material: &str) -> Color {
    match material {
        m if m.starts_with("BRICK") => Color::srgb(0.55, 0.27, 0.2),
        m if m.starts_with("METAL") => Color::srgb(0.55, 0.57, 0.6),
        m if m.contains("WOOD") => Color::srgb(0.5, 0.36, 0.22),
        m if m.contains("GRASS") => Color::srgb(0.3, 0.5, 0.22),
        m if m.contains("GLASS") => Color::srgba(0.7, 0.85, 0.9, 0.5),
        _ => Color::srgb(0.66, 0.65, 0.63),
    }
}

pub fn apply(level: &mut Level, room: &mut crate::world::Room, world: &mut delirium::collision::Collision, change: &mut u64,
             commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, drawn: &Query<(Entity, &LevelMesh)>,
             clouds: &mut Assets<bevy_gaussian_splatting::PlanarGaussian3d>, assets: &AssetServer,
             registry: &bevy::asset::io::embedded::EmbeddedAssetRegistry) -> bool {
    for p in level.pieces.iter_mut().filter(|p| p.shape == Shape::Pool) {
        let z = -p.size.z * 0.5;
        if p.pos.z != z || p.rot != Vec3::ZERO {
            (p.pos.z, p.rot) = (z, Vec3::ZERO);
            level.change += 1;
        }
    }
    for i in 0..level.pieces.len() {
        let p = &level.pieces[i];
        if p.shape == Shape::Model && !level.models.contains_key(&p.model) {
            let path = p.model.clone();
            let m = load(&path, meshes, mats, clouds, assets, registry);
            if let Some(e) = &m.error {
                level.note = e.clone();
            }
            level.models.insert(path, m);
        }
    }
    if *change == level.change && drawn.iter().count() == level.pieces.len() {
        return false;
    }
    let holes: Vec<_> = level.pieces.iter().filter(|p| p.shape == Shape::Pool && room.pool)
        .map(|p| (p.pos.truncate() - p.size.truncate() * 0.5, p.pos.truncate() + p.size.truncate() * 0.5, p.size.z)).collect();
    let recut = crate::world::HOLES.read().is_ok_and(|h| *h != holes);
    if recut {
        if let Ok(mut h) = crate::world::HOLES.write() {
            *h = holes;
        }
        room.recut_floor();
    }
    let fresh = *change == 0;
    room.loose.clear();
    for (i, b) in std::mem::take(&mut level.bodies).into_iter().enumerate() {
        if !fresh {
            room.remove_piece(b);
        }
        world.remove_chunk(LEVEL_CHUNK + i as u64);
    }
    if fresh {
        for i in 0..level.pieces.len() + 64 {
            world.remove_chunk(LEVEL_CHUNK + i as u64);
        }
    }
    for (e, _) in drawn.iter() {
        commands.entity(e).despawn();
    }
    for (i, p) in level.pieces.iter().enumerate() {
        let row = room.surfaces.iter().position(|s| s.0 == p.material).unwrap_or(0) as u8;
        let (pts, tris) = p.world_triangles(&level.local(p));
        let body = if let Some(l) = p.loose.filter(|_| p.shape != Shape::Pool) {
            let t = p.transform();
            let local: Vec<Vec3> = pts.iter().map(|&v| t.rotation.inverse() * (Vec3::from(v) - t.translation)).collect();
            let b = room.add_loose(&local, &tris, t.translation, t.rotation, l.kg, l.friction, l.bounce);
            room.loose.extend(b.map(|h| (h, i, false, l.float)));
            b
        } else {
            world.add_chunk(LEVEL_CHUNK + i as u64, &pts, &tris, &vec![row; tris.len()]);
            room.add_piece(&pts, &tris)
        };
        level.bodies.push(body);
        let picked = level.selected == Some(i) && level.open;
        let model = level.models.get(&p.model).filter(|_| p.shape == Shape::Model);
        if let Some(parts) = model.map(|m| m.usd.clone()).filter(|u| !u.is_empty()) {
            commands.spawn((p.transform(), Visibility::default(), RenderLayers::layer(1), LevelMesh(i))).with_children(|c| {
                for (mesh, mat) in parts {
                    c.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), RenderLayers::layer(1)));
                }
            });
            continue;
        }
        if let Some((cloud, at)) = model.and_then(|m| m.splat.clone()) {
            commands.spawn((p.transform(), Visibility::default(), RenderLayers::layer(1), LevelMesh(i)))
                .with_child((bevy_gaussian_splatting::PlanarGaussian3dHandle(cloud), bevy_gaussian_splatting::CloudSettings::default(), at, RenderLayers::layer(1)));
            continue;
        }
        if let Some(scene) = model.and_then(|m| m.scene.clone()) {
            commands.spawn((p.transform(), Visibility::default(), RenderLayers::layer(1), LevelMesh(i)))
                .with_child((bevy::world_serialization::WorldAssetRoot(scene), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), RenderLayers::layer(1)));
            continue;
        }
        let (mesh, c) = match model.and_then(|m| m.obj.clone()) {
            Some((h, c)) => (h, c),
            None => (meshes.add(p.shape.mesh(p.detail)), p.color.map_or(colour(&p.material), |c| Color::srgba(c[0], c[1], c[2], c[3]))),
        };
        commands.spawn((Mesh3d(mesh),
                        MeshMaterial3d(mats.add(if p.glass {
                            StandardMaterial { base_color: c.with_alpha(0.2), alpha_mode: AlphaMode::Blend, specular_transmission: 1.0, ior: 1.5, perceptual_roughness: 0.02,
                                               emissive: if picked { LinearRgba::rgb(0.25, 0.18, 0.0) } else { LinearRgba::BLACK }, ..default() }
                        } else {
                            StandardMaterial { base_color: c, perceptual_roughness: 0.9,
                                               emissive: if picked { LinearRgba::rgb(0.25, 0.18, 0.0) } else { LinearRgba::BLACK },
                                               alpha_mode: if c.alpha() < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque }, ..default() }
                        })),
                        p.transform(), RenderLayers::layer(1), LevelMesh(i)))
            .with_children(|c| if p.shape == Shape::Pool {
                c.spawn((Mesh3d(meshes.add(Plane3d::new(Vec3::Z, Vec2::splat(0.5)))),
                         MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgba(0.12, 0.42, 0.62, 0.55), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.02,
                                                                    reflectance: 0.5, ..default() })),
                         Transform::from_xyz(0.0, 0.0, 0.5 + crate::world::WATER() / p.size.z.max(0.01)), RenderLayers::layer(1), bevy::light::NotShadowCaster));
            });
    }
    *change = level.change;
    recut
}

pub fn own_pieces(room: &crate::world::Room) -> Vec<(bool, usize, Piece)> {
    let name = |row: u8| room.surfaces.get(row as usize).map_or("CONCRETE".to_string(), |s| s.0.clone());
    let piece = |pos: Vec3, q: Quat, h: Vec3, row: u8| {
        let (z, y, x) = q.to_euler(EulerRot::ZYX);
        Piece { shape: Shape::Box, pos, rot: Vec3::new(x, y, z) * (180.0 / std::f32::consts::PI), size: h * 2.0, material: name(row), model: String::new(), color: None, glass: false, loose: None, detail: 0 }
    };
    let gone = crate::world::GONE_Z() + 1.0;
    room.blocks.iter().enumerate().filter(|b| b.1.0.z > gone && b.1.0.z + b.1.1.z > 0.01).map(|(i, &(c, h, r))| (false, i, piece(c, Quat::IDENTITY, h, r)))
        .chain(room.tilted.iter().enumerate().filter(|b| b.1.0.translation.z > gone).map(|(i, &(t, h, r))| (true, i, piece(t.translation, t.rotation, h, r))))
        .collect()
}

pub fn mouse(level: &mut Level, ray: Option<Ray3d>, press: bool, held: bool, shift: bool, own: &[(bool, usize, Piece)], stage: &str) -> bool {
    if !level.open {
        return false;
    }
    let Some(ray) = ray else { return false };
    let (o, d) = (ray.origin, *ray.direction);
    if press {
        let mut hit = level.pieces.iter().enumerate().filter_map(|(i, p)| p.hit(o, d, level.bounds(p)).map(|t| (i, t))).min_by(|a, b| a.1.total_cmp(&b.1));
        let unit = (Vec3::splat(-0.5), Vec3::splat(0.5));
        let theirs = own.iter().filter_map(|(k, i, p)| p.hit(o, d, unit).map(|t| (*k, *i, p, t))).min_by(|a, b| a.3.total_cmp(&b.3));
        if let Some((k, i, p, t)) = theirs.filter(|th| hit.is_none_or(|m| th.3 < m.1)) {
            if level.stage != stage {
                (level.stage, level.cut) = (stage.to_string(), vec![]);
            }
            level.cut.push((k, i));
            level.pieces.push(p.clone());
            level.rebuild = true;
            level.note = format!("Took the stage's {} out: it's the level's now, to move, size, colour or delete", p.material.to_lowercase());
            hit = Some((level.pieces.len() - 1, t));
        }
        let was = level.selected;
        level.selected = hit.map(|h| h.0);
        if level.selected != was {
            level.change += 1;
        }
        level.drag = hit.map(|(i, t)| {
            let at = o + d * t;
            (i, at.z, level.pieces[i].pos - at)
        });
        return hit.is_some();
    }
    if !held {
        level.drag = None;
        return false;
    }
    let Some((i, z, off)) = level.drag else { return false };
    let to = if shift {
        let n = Vec3::new(d.x, d.y, 0.0).normalize_or_zero();
        let p = level.pieces[i].pos - off;
        let den = n.dot(d);
        if den.abs() < 1e-4 { return true }
        let t = n.dot(p - o) / den;
        let at = o + d * t;
        Vec3::new(level.pieces[i].pos.x, level.pieces[i].pos.y, at.z + off.z)
    } else {
        if d.z.abs() < 1e-4 { return true }
        let t = (z - o.z) / d.z;
        if t <= 0.0 { return true }
        let at = o + d * t;
        Vec3::new(at.x + off.x, at.y + off.y, level.pieces[i].pos.z)
    };
    if to != level.pieces[i].pos {
        level.pieces[i].pos = to;
        level.change += 1;
    }
    true
}

pub const MODES: [(&str, &str); 3] = [("Move", "1"), ("Turn", "2"), ("Size", "3")];
const AXIS_COLOUR: [Color; 3] = [Color::srgb(0.92, 0.37, 0.33), Color::srgb(0.47, 0.78, 0.37), Color::srgb(0.35, 0.59, 0.94)];

fn grip_axes(p: &Piece, mode: u8, eye: Vec3) -> ([Vec3; 3], f32) {
    let q = if mode == 2 { p.transform().rotation } else { Quat::IDENTITY };
    ([q * Vec3::X, q * Vec3::Y, q * Vec3::Z], (p.pos.distance(eye) * 0.13).max(0.25))
}

fn along(o: Vec3, d: Vec3, at: Vec3, a: Vec3) -> Option<(f32, f32, f32)> {
    let (b, w) = (d.dot(a), o - at);
    let den = 1.0 - b * b;
    if den < 1e-4 {
        return None;
    }
    let (s, t) = ((b * a.dot(w) - d.dot(w)) / den, (a.dot(w) - b * d.dot(w)) / den);
    Some((t, s, (o + d * s).distance(at + a * t)))
}

fn round(o: Vec3, d: Vec3, at: Vec3, a: Vec3) -> Option<(f32, f32, f32)> {
    let den = d.dot(a);
    if den.abs() < 1e-4 {
        return None;
    }
    let s = (at - o).dot(a) / den;
    let h = o + d * s - at;
    let u = a.any_orthonormal_vector();
    Some((h.dot(a.cross(u)).atan2(h.dot(u)), h.length(), s))
}

pub fn grip_under(level: &Level, ray: Option<Ray3d>, eye: Vec3) -> Option<usize> {
    let (ray, p) = (ray?, level.pieces.get(level.selected?)?);
    let (o, d) = (ray.origin, *ray.direction);
    let (axes, s) = grip_axes(p, level.mode, eye);
    (0..3).filter_map(|k| {
        if level.mode == 1 {
            let (_, r, t) = round(o, d, p.pos, axes[k])?;
            (t > 0.0 && (r - s).abs() < s * 0.1).then_some((k, t))
        } else {
            let (t, ds, gap) = along(o, d, p.pos, axes[k])?;
            (ds > 0.0 && t > s * 0.15 && t < s * 1.1 && gap < s * 0.09).then_some((k, ds))
        }
    }).min_by(|a, b| a.1.total_cmp(&b.1)).map(|h| h.0)
}

pub fn grip(level: &mut Level, ray: Option<Ray3d>, press: bool, held: bool, eye: Vec3, ctrl: bool) -> bool {
    if !held {
        level.grip = None;
        return false;
    }
    let Some(ray) = ray else { return level.grip.is_some() };
    let (o, d) = (ray.origin, *ray.direction);
    let measure = |p: &Piece, k: usize, mode: u8| {
        let (axes, _) = grip_axes(p, mode, eye);
        if mode == 1 { round(o, d, p.pos, axes[k]).map(|r| r.0) } else { along(o, d, p.pos, axes[k]).map(|a| a.0) }
    };
    if press {
        let Some(k) = grip_under(level, Some(ray), eye) else { return false };
        let i = level.selected.expect("a handle is the pick's");
        let p = level.pieces[i].clone();
        let Some(m) = measure(&p, k, level.mode) else { return false };
        level.grip = Some((i, k, m, 0.0, p));
        return true;
    }
    let Some((i, k, last, gone, p0)) = level.grip.clone() else { return false };
    if i >= level.pieces.len() {
        level.grip = None;
        return false;
    }
    let Some(m) = measure(&p0, k, level.mode) else { return true };
    let step = if level.mode == 1 { (m - last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI } else { m - last };
    let gone = gone + step;
    level.grip = Some((i, k, m, gone, p0.clone()));
    let snap = |v: f32, by: f32| if ctrl { (v / by).round() * by } else { v };
    let mut p = p0.clone();
    let (axes, _) = grip_axes(&p0, level.mode, eye);
    match level.mode {
        0 => p.pos = p0.pos + axes[k] * snap(gone, 0.25),
        1 => {
            let q = Quat::from_axis_angle(axes[k], snap(gone.to_degrees(), 15.0).to_radians()) * p0.transform().rotation;
            let (z, y, x) = q.to_euler(EulerRot::ZYX);
            p.rot = Vec3::new(x, y, z) * (180.0 / std::f32::consts::PI);
        }
        _ => p.size[k] = snap(p0.size[k] + gone * 2.0, 0.25).max(0.05),
    }
    if p.pos != level.pieces[i].pos || p.rot != level.pieces[i].rot || p.size != level.pieces[i].size {
        level.pieces[i] = p;
        level.change += 1;
    }
    true
}

pub fn draw_grips(level: &Level, eye: Vec3, hot: Option<usize>, line: &mut impl FnMut(Vec3, Vec3, Color)) {
    let Some(p) = level.selected.and_then(|i| level.pieces.get(i)) else { return };
    let hot = level.grip.as_ref().map(|g| g.1).or(hot);
    let (axes, s) = grip_axes(p, level.mode, eye);
    for (k, a) in axes.into_iter().enumerate() {
        let c = if hot == Some(k) { Color::srgb(1.0, 0.85, 0.2) } else { AXIS_COLOUR[k] };
        let (u, v) = (a.any_orthonormal_vector(), a.cross(a.any_orthonormal_vector()));
        match level.mode {
            0 => {
                let tip = p.pos + a * s;
                line(p.pos + a * s * 0.15, tip, c);
                for e in [u, -u, v, -v] {
                    line(tip, tip - a * s * 0.22 + e * s * 0.07, c);
                }
            }
            1 => {
                let n = 64;
                for j in 0..n {
                    let at = |j: usize| { let t = j as f32 / n as f32 * std::f32::consts::TAU; p.pos + (u * t.cos() + v * t.sin()) * s };
                    line(at(j), at(j + 1), c);
                }
            }
            _ => {
                let end = p.pos + a * s * 0.9;
                line(p.pos + a * s * 0.15, end, c);
                let h = s * 0.07;
                let corner = |i: usize| end + a * h * if i & 1 == 0 { -1.0 } else { 1.0 } + u * h * if i & 2 == 0 { -1.0 } else { 1.0 } + v * h * if i & 4 == 0 { -1.0 } else { 1.0 };
                for (x, y) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                    line(corner(x), corner(y), c);
                }
            }
        }
    }
}

pub fn context_menu(ctx: &egui::Context, level: &mut Level) -> Option<&'static str> {
    if std::mem::take(&mut level.menu_ask) {
        level.menu = ctx.input(|i| i.pointer.latest_pos()).or(Some(ctx.content_rect().center()));
    }
    let (Some(at), Some(i)) = (level.menu, level.selected.filter(|&i| i < level.pieces.len())) else {
        level.menu = None;
        return None;
    };
    let mut asked = None;
    let mut done = false;
    let r = egui::Area::new("level menu".into()).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
        egui::Frame::menu(ui.style()).show(ui, |ui| {
            ui.set_width(200.0);
            ui.spacing_mut().interact_size.y = 24.0;
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.spacing_mut().button_padding = egui::vec2(8.0, 3.0);
            let mut item = |ui: &mut egui::Ui, text: &str, key: &str| {
                let r = ui.add(egui::Button::new(text).right_text(egui::RichText::new(key).weak()).frame(false).min_size(egui::vec2(200.0, 24.0)));
                done |= r.clicked();
                r.clicked()
            };
            let p = level.pieces[i].clone();
            if item(ui, "Frame it", "F") {
                asked = Some("frame");
            }
            if item(ui, "Duplicate", "") {
                let mut q = p.clone();
                q.pos += Vec3::new(p.size.x + 0.2, 0.0, 0.0);
                level.pieces.push(q);
                level.selected = Some(level.pieces.len() - 1);
                level.change += 1;
            }
            if item(ui, "Unturn", "") {
                level.pieces[i].rot = Vec3::ZERO;
                level.change += 1;
            }
            if item(ui, if p.glass { "Solid" } else { "Glass" }, "") {
                level.pieces[i].glass = !p.glass;
                level.change += 1;
            }
            if item(ui, "Homer's start on it", "") {
                level.start = Some(p.pos + Vec3::Z * p.size.z * 0.5);
                level.rebuild = true;
            }
            ui.separator();
            for (m, (name, key)) in MODES.into_iter().enumerate() {
                let on = if level.mode == m as u8 { "• " } else { "" };
                if item(ui, &format!("{on}{name} handles"), key) {
                    level.mode = m as u8;
                }
            }
            ui.separator();
            if item(ui, "Delete", "Del") {
                level.pieces.remove(i);
                level.selected = None;
                level.change += 1;
            }
        });
    }).response;
    if done || ctx.input(|k| k.key_pressed(egui::Key::Escape) || (k.pointer.any_pressed() && !k.pointer.latest_pos().is_some_and(|p| r.rect.contains(p)))) {
        level.menu = None;
    }
    asked
}

pub enum Act {
    Drop(usize),
    Taxi,
    Gas,
    Stage(String),
}

pub const STAMPS: [(&str, &str); 9] = [("room", "room stairs and wall"), ("bigstairs", "big stairs"), ("stairsdown", "stairs down"),
    ("rooftop", "rooftop"), ("highledge", "high ledge"), ("lowwall", "low wall"), ("kerb", "kerb"), ("rail", "rail"), ("skatepark", "skate ramps")];

pub fn stamp(level: &mut Level, surfaces: &[(String, f32, f32)], stage: &str, at: Vec3) {
    let (blocks, tilted, _) = crate::world::Room::stage_boxes(surfaces, stage);
    let name = |row: u8| surfaces.get(row as usize).map_or("CONCRETE".to_string(), |s| s.0.clone());
    let piece = |pos: Vec3, q: Quat, h: Vec3, row: u8| {
        let (z, y, x) = q.to_euler(EulerRot::ZYX);
        Piece { shape: Shape::Box, pos, rot: Vec3::new(x, y, z) * (180.0 / std::f32::consts::PI), size: h * 2.0, material: name(row), model: String::new(), color: None, glass: false, loose: None, detail: 0 }
    };
    let new: Vec<Piece> = blocks.iter().filter(|b| b.0.z + b.1.z > 0.01).map(|&(c, h, r)| piece(c, Quat::IDENTITY, h, r))
        .chain(tilted.iter().filter(|b| b.0.translation.z + b.1.max_element() > 0.01).map(|&(t, h, r)| piece(t.translation, t.rotation, h, r))).collect();
    if new.is_empty() {
        level.note = format!("The {stage} stage has no pieces of its own to stamp");
        return;
    }
    let (lo, hi) = new.iter().fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |b, p| (b.0.min(p.pos), b.1.max(p.pos)));
    let shift = (at - (lo + hi) * 0.5).with_z(0.0);
    let n = new.len();
    level.pieces.extend(new.into_iter().map(|mut p| { p.pos += shift; p }));
    level.selected = Some(level.pieces.len() - 1);
    level.change += 1;
    level.note = format!("Stamped the {stage} stage's {n} pieces");
}

pub fn panel(ui: &mut egui::Ui, level: &mut Level, materials: &[String], surfaces: &[(String, f32, f32)], ahead: Vec3, stages: &[(&str, &str)]) -> Option<Act> {
    let mut act = None;
    let accent = egui::Color32::from_rgb(130, 210, 255);
    ui.horizontal(|ui| {
        ui.heading("Level editor");
        crate::ui::wip(ui, "the level editor is still being built; expect rough edges");
        ui.label(egui::RichText::new(format!("{} pieces", level.pieces.len())).small().weak());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Done").on_hover_text("Back to the room's controls (the level stays)").clicked() {
                level.open = false;
                level.drag = None;
                level.change += 1;
            }
        });
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (k, name) in TABS.iter().enumerate() {
            let on = level.tab as usize == k;
            let text = egui::RichText::new(*name).size(13.5).color(if on { egui::Color32::BLACK } else { ui.visuals().text_color() });
            let b = egui::Button::new(text).fill(if on { accent } else { ui.visuals().widgets.inactive.weak_bg_fill }).corner_radius(6.0).min_size(egui::vec2(0.0, 26.0));
            if ui.add(b).clicked() {
                level.tab = k as u8;
            }
        }
    });
    ui.add_space(4.0);
    let grid_h = (ui.available_height() * 0.62).clamp(230.0, 560.0);
    egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(8.0).inner_margin(6.0).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("level tiles").max_height(grid_h).auto_shrink([false, true]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            if level.tab == 4 {
                ui.set_width(ui.available_width());
                return file_tab(ui, level, ahead);
            }
            ui.horizontal_wrapped(|ui| match level.tab {
                0 => {
                    for (s, n) in SHAPES {
                        if tile(ui, n, &format!("shape {n}"), || shape_pic(s), None).on_hover_text(format!("A {n} where the view looks")).clicked() {
                            let size = match s { Shape::Ramp => Vec3::new(2.0, 3.0, 1.0), Shape::Disc => Vec3::new(2.0, 2.0, 0.1), Shape::Pool => Vec3::new(4.0, 6.0, 1.5), _ => Vec3::ONE };
                            level.pieces.push(Piece { shape: s, pos: ahead.with_z(ahead.z + size.z * 0.5), rot: Vec3::ZERO, size, material: "CONCRETE".into(), model: String::new(), color: None, glass: false, loose: None, detail: 0 });
                            level.selected = Some(level.pieces.len() - 1);
                            level.change += 1;
                        }
                    }
                    if tile(ui, "glass pane", "shape glass", || boxes(&[(Vec3::ZERO, Vec3::new(1.0, 0.05, 0.75), Quat::IDENTITY, Color::srgb(0.65, 0.85, 0.95))]), None)
                        .on_hover_text("A glass pane, as the sky pool's").clicked() {
                        let size = Vec3::new(2.0, 0.1, 1.5);
                        level.pieces.push(Piece { shape: Shape::Box, pos: ahead.with_z(ahead.z + size.z * 0.5), rot: Vec3::ZERO, size, material: "GLASS".into(), model: String::new(),
                                                  color: Some([0.8, 0.95, 1.0, 1.0]), glass: true, loose: None, detail: 0 });
                        level.selected = Some(level.pieces.len() - 1);
                        level.change += 1;
                    }
                }
                1 => {
                    for (s, label) in STAMPS {
                        if tile(ui, label, &format!("stamp {s}"), || stage_pic(surfaces, s), None)
                            .on_hover_text("The stage's own pieces, put where the view looks: each one to move, size, colour or delete").clicked() {
                            stamp(level, surfaces, s, ahead);
                        }
                    }
                }
                2 => {
                    for (k, kind) in (*crate::world::KINDS()).iter().enumerate() {
                        if tile(ui, kind.0, &format!("kind {}", kind.0), || kind_pic(kind.0), None).on_hover_text("Dropped where the view looks").clicked() {
                            act = Some(Act::Drop(k));
                        }
                    }
                    if tile(ui, "taxi", "kind taxi", || kind_pic("taxi"), None).on_hover_text("A taxi where the view looks. Point at it and press F (or click it) to floor its gas").clicked() {
                        act = Some(Act::Taxi);
                    }
                    if tile(ui, "all taxis: gas", "kind gas", || kind_pic("taxi"), Some("⏩")).on_hover_text("Floor every empty taxi's gas (again: let off)").clicked() {
                        act = Some(Act::Gas);
                    }
                }
                3 => {
                    for (s, label) in stages {
                        if tile(ui, label, &format!("stage {s}"), || stage_pic(surfaces, s), None)
                            .on_hover_text("This stage as the level's base (its machines come with it); your pieces stay").clicked() {
                            act = Some(Act::Stage(s.to_string()));
                        }
                    }
                }
                _ => {}
            });
        });
    });
    ui.add_space(8.0);
    egui::ScrollArea::vertical().id_salt("level inspector").auto_shrink([false, false]).show(ui, |ui| {
        type_scale(ui);
        ui.horizontal(|ui| {
            let on = level.placing_start;
            if ui.add(egui::Button::new(if on { "Click where Homer starts… (Esc: cancel)" } else { "Set Homer's start" }).small().selected(on))
                .on_hover_text("Then click the ground: his start, and where Reset puts him").clicked() {
                level.placing_start = !on;
            }
            if level.start.is_some() && !on && ui.small_button("Stage's own").on_hover_text("The stage's own start again").clicked() {
                level.start = None;
                level.rebuild = true;
            }
        });
        ui.add_space(6.0);
        card(ui, |ui| inspector(ui, level, materials));
        if !level.note.is_empty() {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(&level.note).small().weak());
        }
    });
    act
}

const TABS: [&str; 5] = ["Shapes", "Pieces", "Objects", "Stages", "File"];

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(ui.visuals().faint_bg_color).stroke(ui.visuals().widgets.noninteractive.bg_stroke).corner_radius(8.0).inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
}

const TILE: f32 = 82.0;

fn tile(ui: &mut egui::Ui, label: &str, key: &str, make: impl FnOnce() -> Vec<([Vec3; 3], Color)>, mark: Option<&str>) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(TILE, TILE + 26.0), egui::Sense::click());
    if !ui.is_rect_visible(r) {
        return resp;
    }
    let v = ui.visuals();
    let (fill, stroke) = if resp.is_pointer_button_down_on() {
        (v.widgets.active.weak_bg_fill, egui::Stroke::new(1.5, egui::Color32::from_rgb(130, 210, 255)))
    } else if resp.hovered() {
        (v.widgets.hovered.weak_bg_fill, egui::Stroke::new(1.5, egui::Color32::from_rgb(130, 210, 255)))
    } else {
        (v.extreme_bg_color, v.widgets.noninteractive.bg_stroke)
    };
    let p = ui.painter_at(r);
    p.rect(r, 8.0, fill, stroke, egui::StrokeKind::Inside);
    let pic = egui::Rect::from_min_size(r.min + egui::vec2(8.0, 8.0), egui::vec2(TILE - 16.0, TILE - 16.0));
    THUMBS.with_borrow_mut(|t| {
        let tris = t.entry(key.to_string()).or_insert_with(make);
        thumb(&p, pic, tris);
    });
    if let Some(m) = mark {
        p.text(pic.right_top() + egui::vec2(-2.0, 2.0), egui::Align2::RIGHT_TOP, m, egui::FontId::proportional(20.0), v.text_color());
    }
    let galley = p.layout(label.to_string(), egui::FontId::proportional(11.5), v.text_color(), TILE - 8.0);
    let at = egui::pos2(r.center().x - galley.size().x / 2.0, r.max.y - 30.0 + (26.0 - galley.size().y).max(0.0) / 2.0);
    p.galley(at, galley, v.text_color());
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

thread_local! {
    static THUMBS: std::cell::RefCell<std::collections::HashMap<String, Vec<([Vec3; 3], Color)>>> = Default::default();
}

pub(crate) fn pic(p: &egui::Painter, r: egui::Rect, key: &str, make: impl FnOnce() -> Vec<([Vec3; 3], Color)>) {
    THUMBS.with_borrow_mut(|t| thumb(p, r, t.entry(key.to_string()).or_insert_with(make)));
}

pub(crate) fn thumb(p: &egui::Painter, r: egui::Rect, tris: &[([Vec3; 3], Color)]) {
    if tris.is_empty() {
        return;
    }
    let eye = Vec3::new(0.9, -1.3, 1.1).normalize();
    let right = Vec3::Z.cross(eye).normalize();
    let up = eye.cross(right);
    let light = Vec3::new(0.35, -0.55, 1.0).normalize();
    let flat = |v: Vec3| egui::vec2(v.dot(right), -v.dot(up));
    let (mut lo, mut hi) = (egui::vec2(f32::MAX, f32::MAX), egui::vec2(f32::MIN, f32::MIN));
    for t in tris {
        for &v in &t.0 {
            let s = flat(v);
            (lo, hi) = (lo.min(s), hi.max(s));
        }
    }
    let k = ((r.width() / (hi.x - lo.x).max(1e-3)).min(r.height() / (hi.y - lo.y).max(1e-3))) * 0.92;
    let mid = (lo + hi) / 2.0;
    let at = |v: Vec3| r.center() + (flat(v) - mid) * k;
    let mut order: Vec<(f32, usize)> = tris.iter().enumerate().map(|(i, t)| ((t.0[0] + t.0[1] + t.0[2]).dot(eye), i)).collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut mesh = egui::Mesh::default();
    for (_, i) in order {
        let ([a, b, c], col) = tris[i];
        let n = (b - a).cross(c - a).normalize_or_zero();
        if n.dot(eye) < -1e-4 {
            continue;
        }
        let shade = 0.5 + 0.5 * n.dot(light).max(0.0);
        let s = col.to_srgba();
        let fill = egui::Color32::from_rgb((s.red * shade * 255.0) as u8, (s.green * shade * 255.0) as u8, (s.blue * shade * 255.0) as u8);
        let k = mesh.vertices.len() as u32;
        for v in [a, b, c] {
            mesh.colored_vertex(at(v), fill);
        }
        mesh.add_triangle(k, k + 1, k + 2);
    }
    p.add(egui::Shape::mesh(mesh));
}

pub(crate) fn boxes(b: &[(Vec3, Vec3, Quat, Color)]) -> Vec<([Vec3; 3], Color)> {
    let mut out = vec![];
    for &(c, h, q, col) in b {
        let v = |x: f32, y: f32, z: f32| c + q * (h * Vec3::new(x, y, z));
        let faces = [[(-1., -1., 1.), (1., -1., 1.), (1., 1., 1.), (-1., 1., 1.)], [(-1., -1., -1.), (-1., 1., -1.), (1., 1., -1.), (1., -1., -1.)],
                     [(-1., -1., -1.), (1., -1., -1.), (1., -1., 1.), (-1., -1., 1.)], [(1., 1., -1.), (-1., 1., -1.), (-1., 1., 1.), (1., 1., 1.)],
                     [(1., -1., -1.), (1., 1., -1.), (1., 1., 1.), (1., -1., 1.)], [(-1., 1., -1.), (-1., -1., -1.), (-1., -1., 1.), (-1., 1., 1.)]];
        for f in faces {
            let [a, b, cc, d] = f.map(|(x, y, z)| v(x, y, z));
            out.push(([a, b, cc], col));
            out.push(([a, cc, d], col));
        }
    }
    out
}

pub(crate) fn lathe(c: Vec3, h: f32, r: impl Fn(f32) -> f32, rings: usize, col: Color) -> Vec<([Vec3; 3], Color)> {
    let n = 20;
    let at = |i: usize, j: usize| {
        let (t, a) = (j as f32 / rings as f32, i as f32 / n as f32 * std::f32::consts::TAU);
        c + Vec3::new(a.cos() * r(t), a.sin() * r(t), (t - 0.5) * h)
    };
    let mut out = vec![];
    for j in 0..rings {
        for i in 0..n {
            let (a, b, cc, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            out.push(([a, b, cc], col));
            out.push(([a, cc, d], col));
        }
    }
    let (bot, top) = (c - Vec3::Z * h / 2.0, c + Vec3::Z * h / 2.0);
    for i in 0..n {
        if r(0.0) > 0.0 {
            out.push(([bot, at(i + 1, 0), at(i, 0)], col));
        }
        if r(1.0) > 0.0 {
            out.push(([top, at(i, rings), at(i + 1, rings)], col));
        }
    }
    out
}

pub(crate) fn ball(c: Vec3, r: f32, col: Color) -> Vec<([Vec3; 3], Color)> {
    let (v, t) = Shape::Sphere.triangles(16);
    t.iter().map(|t| (t.map(|i| c + v[i as usize] * (2.0 * r)), col)).collect()
}

fn shape_pic(s: Shape) -> Vec<([Vec3; 3], Color)> {
    let col = Color::srgb(0.72, 0.74, 0.78);
    match s {
        Shape::Sphere => ball(Vec3::ZERO, 0.5, col),
        Shape::Cylinder => lathe(Vec3::ZERO, 1.0, |_| 0.5, 1, col),
        Shape::Disc => lathe(Vec3::ZERO, 0.1, |_| 0.5, 1, col),
        Shape::Cone => lathe(Vec3::ZERO, 1.0, |t| 0.5 * (1.0 - t), 1, col),
        _ => {
            let (v, t) = s.triangles(0);
            t.iter().map(|t| (t.map(|i| v[i as usize]), col)).collect()
        }
    }
}

pub(crate) fn kind_pic(kind: &str) -> Vec<([Vec3; 3], Color)> {
    let wood = Color::srgb(0.62, 0.45, 0.26);
    match kind {
        "ball" => ball(Vec3::ZERO, 0.5, Color::srgb(0.9, 0.45, 0.2)),
        "box" => boxes(&[(Vec3::ZERO, Vec3::splat(0.4), Quat::from_rotation_z(0.3), Color::srgb(0.78, 0.62, 0.42))]),
        "crate" => {
            let mut b = boxes(&[(Vec3::ZERO, Vec3::splat(0.5), Quat::IDENTITY, wood)]);
            b.extend(boxes(&[(Vec3::new(0.0, -0.51, 0.0), Vec3::new(0.5, 0.01, 0.07), Quat::from_rotation_y(0.785), Color::srgb(0.45, 0.31, 0.17))]));
            b
        }
        "barrel" => {
            let mut b = lathe(Vec3::ZERO, 1.2, |t| 0.42 + 0.06 * (t * std::f32::consts::PI).sin(), 6, Color::srgb(0.7, 0.18, 0.15));
            b.extend(lathe(Vec3::Z * 0.3, 0.05, |_| 0.49, 1, Color::srgb(0.35, 0.35, 0.38)));
            b.extend(lathe(Vec3::Z * -0.3, 0.05, |_| 0.49, 1, Color::srgb(0.35, 0.35, 0.38)));
            b
        }
        "wrecking ball" => {
            let mut b = ball(Vec3::ZERO, 0.5, Color::srgb(0.25, 0.26, 0.28));
            b.extend(boxes(&[(Vec3::Z * 0.9, Vec3::new(0.03, 0.03, 0.45), Quat::IDENTITY, Color::srgb(0.5, 0.5, 0.52))]));
            b
        }
        _ => {
            let (y, d) = (Color::srgb(0.95, 0.75, 0.1), Color::srgb(0.08, 0.08, 0.09));
            let mut b = boxes(&[(Vec3::new(0.0, 0.0, 0.35), Vec3::new(0.9, 2.4, 0.35), Quat::IDENTITY, y),
                                (Vec3::new(0.0, -0.2, 0.95), Vec3::new(0.8, 1.2, 0.3), Quat::IDENTITY, y),
                                (Vec3::new(0.0, -0.2, 0.96), Vec3::new(0.82, 1.0, 0.2), Quat::IDENTITY, Color::srgb(0.25, 0.3, 0.38))]);
            for (x, yy) in [(-0.9, -1.4), (0.9, -1.4), (-0.9, 1.4), (0.9, 1.4)] {
                b.extend(boxes(&[(Vec3::new(x, yy, 0.0), Vec3::new(0.08, 0.32, 0.32), Quat::IDENTITY, d)]));
            }
            b
        }
    }
}

pub(crate) fn stage_pic(surfaces: &[(String, f32, f32)], stage: &str) -> Vec<([Vec3; 3], Color)> {
    let (blocks, tilted, _) = crate::world::Room::stage_boxes(surfaces, stage);
    let name = |row: u8| surfaces.get(row as usize).map_or("CONCRETE".to_string(), |s| s.0.clone());
    let mut b: Vec<(Vec3, Vec3, Quat, Color)> = blocks.iter().filter(|b| b.0.z + b.1.z > 0.01).map(|&(c, h, r)| (c, h, Quat::IDENTITY, colour(&name(r))))
        .chain(tilted.iter().filter(|b| b.0.translation.z + b.1.max_element() > 0.01).map(|&(t, h, r)| (t.translation, h, t.rotation, colour(&name(r))))).collect();
    let (lo, hi) = b.iter().fold((Vec3::splat(-2.0), Vec3::splat(2.0)), |a, b| (a.0.min(b.0 - b.1.abs()), a.1.max(b.0 + b.1.abs())));
    let (lo, hi) = if b.is_empty() { (Vec3::splat(-3.0), Vec3::splat(3.0)) } else { (lo, hi) };
    b.push((((lo + hi) / 2.0).with_z(-0.1), ((hi - lo) / 2.0 + Vec3::splat(0.5)).with_z(0.1), Quat::IDENTITY, Color::srgb(0.42, 0.44, 0.47)));
    let mut t = boxes(&b);
    t.extend(machine_pic(stage));
    t
}

fn machine_pic(stage: &str) -> Vec<([Vec3; 3], Color)> {
    let (wood, rope, metal) = (Color::srgb(0.6, 0.43, 0.25), Color::srgb(0.85, 0.75, 0.5), Color::srgb(0.55, 0.57, 0.6));
    match stage {
        "bull" => {
            let mut t = lathe(Vec3::Z * 0.4, 0.8, |_| 0.25, 1, metal);
            t.extend(boxes(&[(Vec3::Z * 1.1, Vec3::new(0.3, 0.75, 0.3), Quat::from_rotation_x(0.15), Color::srgb(0.42, 0.25, 0.15)),
                             (Vec3::new(0.0, 0.8, 1.35), Vec3::new(0.22, 0.25, 0.22), Quat::IDENTITY, Color::srgb(0.42, 0.25, 0.15))]));
            t.extend(lathe(Vec3::Z * 0.02, 0.04, |_| 1.6, 1, Color::srgb(0.75, 0.2, 0.2)));
            t
        }
        "bridge" => {
            let mut b = vec![(Vec3::new(0.0, -3.0, 0.5), Vec3::new(1.0, 0.8, 0.5), Quat::IDENTITY, metal), (Vec3::new(0.0, 3.0, 0.5), Vec3::new(1.0, 0.8, 0.5), Quat::IDENTITY, metal)];
            for k in 0..9 {
                let y = -2.0 + k as f32 * 0.5;
                b.push((Vec3::new(0.0, y, 0.95 - 0.25 * (1.0 - (y / 2.2).powi(2))), Vec3::new(0.6, 0.2, 0.03), Quat::IDENTITY, wood));
            }
            for x in [-0.65, 0.65] {
                b.push((Vec3::new(x, 0.0, 1.6), Vec3::new(0.03, 2.4, 0.03), Quat::IDENTITY, rope));
            }
            boxes(&b)
        }
        "trampoline" => {
            let mut t = lathe(Vec3::Z * 0.55, 0.06, |_| 1.0, 1, Color::srgb(0.15, 0.15, 0.17));
            t.extend(lathe(Vec3::Z * 0.55, 0.1, |_| 1.08, 1, Color::srgb(0.2, 0.45, 0.85)));
            for a in 0..4 {
                let q = a as f32 * std::f32::consts::FRAC_PI_2 + 0.785;
                t.extend(boxes(&[(Vec3::new(q.cos() * 0.95, q.sin() * 0.95, 0.27), Vec3::new(0.04, 0.04, 0.27), Quat::IDENTITY, metal)]));
            }
            t
        }
        "quake" => boxes(&[(Vec3::Z * 0.15, Vec3::new(1.5, 1.5, 0.15), Quat::from_rotation_y(0.08), Color::srgb(0.5, 0.42, 0.33)),
                           (Vec3::new(0.3, 0.0, 0.31), Vec3::new(0.05, 1.3, 0.01), Quat::from_rotation_z(0.4), Color::srgb(0.2, 0.17, 0.14))]),
        "cab" => kind_pic("taxi"),
        "lights" => boxes(&[(Vec3::new(-1.0, 0.0, 1.0), Vec3::splat(0.3), Quat::IDENTITY, Color::srgb(1.0, 0.25, 0.25)),
                            (Vec3::new(0.0, 0.5, 1.0), Vec3::splat(0.3), Quat::IDENTITY, Color::srgb(0.25, 1.0, 0.35)),
                            (Vec3::new(1.0, 0.0, 1.0), Vec3::splat(0.3), Quat::IDENTITY, Color::srgb(0.3, 0.45, 1.0))]),
        _ => vec![],
    }
}

fn file_tab(ui: &mut egui::Ui, level: &mut Level, ahead: Vec3) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Import").strong());
        crate::ui::wip(ui, "the model importer is still being built; some files may come in wrong");
    });
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut level.path).hint_text("Model or splat file (or drop it on the window)").desired_width((ui.available_width() - 70.0).max(80.0)));
        if ui.button("Import").on_hover_text("Work in progress: import the file named here").clicked() {
            let path = level.path.clone();
            level.import(&path, ahead);
        }
    });
    if ui.button("Browse...").on_hover_text("Work in progress: pick the file in Windows' Open dialog (it's imported when you pick it)").clicked() {
        crate::ui::pick_model(crate::ui::PICK_IMPORT);
    }
    ui.label(egui::RichText::new(".glb .gltf .obj .fbx .usd .usdz, and splats (.ply)").small().weak());
    ui.add_space(8.0);
    ui.label(egui::RichText::new("This level").strong());
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut level.name).hint_text("Name").desired_width((ui.available_width() - 60.0).max(80.0)));
        if ui.button("Save").clicked() {
            level.save();
        }
    });
    ui.horizontal(|ui| {
        let saved = Level::saved();
        egui::ComboBox::from_id_salt("level load").selected_text("Load a level…").width(160.0).show_ui(ui, |ui| {
            for n in &saved {
                if ui.selectable_label(false, n).clicked() {
                    level.load(n);
                }
            }
        });
        if ui.button("Folder").on_hover_text("The levels folder: share a level by sending its folder; put one you're sent in here").clicked() {
            let _ = std::fs::create_dir_all(Level::dir());
            let _ = std::process::Command::new("explorer").arg(Level::dir()).spawn();
        }
        if ui.button("Clear").on_hover_text("Every piece out, the stage's own back, Homer's start its own").clicked() {
            level.pieces.clear();
            level.selected = None;
            level.change += 1;
            (level.cut, level.start, level.rebuild) = (vec![], None, true);
        }
    });
}

fn thing_inspector(ui: &mut egui::Ui, level: &mut Level) {
    let Some((kind, _)) = level.thing else { return };
    ui.label(egui::RichText::new(capital(&level.thing_name)).strong().size(14.0));
    ui.label(egui::RichText::new("Moves with the physics; a change here sets it down still").small().weak());
    ui.add_space(8.0);
    section(ui, "TRANSFORM");
    let (mut at, mut rot, mut scale) = level.thing_at;
    vec_row(ui, "Position", &mut at, 0.05, f32::MIN, 2, "");
    vec_row(ui, "Rotation", &mut rot, 1.0, f32::MIN, 0, "°");
    if kind == 0 {
        ui.horizontal(|ui| {
            field_label(ui, "Size");
            ui.add(egui::DragValue::new(&mut scale).speed(0.01).range(0.2..=5.0).max_decimals(2).suffix(" ×"));
        });
    }
    if (at, rot, scale) != level.thing_at {
        level.thing_at = (at, rot, scale);
        level.thing_set = true;
    }
}

fn inspector(ui: &mut egui::Ui, level: &mut Level, materials: &[String]) {
    if level.thing.is_some() {
        return thing_inspector(ui, level);
    }
    let Some(i) = level.selected.filter(|&i| i < level.pieces.len()) else {
        ui.vertical_centered(|ui| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Nothing picked").strong());
            ui.label(egui::RichText::new("Click a piece in the view, or add one from the tabs above").small().weak());
            ui.add_space(6.0);
        });
        return;
    };
    let mut p = level.pieces[i].clone();
    let what = match p.shape {
        Shape::Model => std::path::Path::new(&p.model).file_name().map_or(p.model.clone(), |f| f.to_string_lossy().into_owned()),
        s => s.name().to_string(),
    };
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
        ui.painter().rect_filled(r, 6.0, ui.visuals().extreme_bg_color);
        if p.shape != Shape::Model {
            THUMBS.with_borrow_mut(|t| thumb(ui.painter(), r.shrink(4.0), t.entry(format!("shape {}", p.shape.name())).or_insert_with(|| shape_pic(p.shape))));
        }
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(capital(&what)).strong().size(14.0));
            ui.label(egui::RichText::new(format!("Piece {} of {}", i + 1, level.pieces.len())).small().weak());
        });
    });
    ui.add_space(8.0);
    section(ui, "TRANSFORM");
    vec_row(ui, "Position", &mut p.pos, 0.05, f32::MIN, 2, "");
    vec_row(ui, "Rotation", &mut p.rot, 1.0, f32::MIN, 0, "°");
    vec_row(ui, "Size", &mut p.size, 0.05, 0.05, 2, "");
    ui.horizontal(|ui| {
        field_label(ui, "");
        ui.spacing_mut().item_spacing.x = 3.0;
        for (t, d, tip) in [("↺ 15°", 15.0, "Turn left 15° (R)"), ("↻ 15°", -15.0, "Turn right 15° (Shift+R)"), ("↺ 90°", 90.0, "Turn a quarter")] {
            if ui.add(egui::Button::new(t).corner_radius(4.0).min_size(egui::vec2(0.0, ROW_H))).on_hover_text(tip).clicked() {
                p.rot.z = (p.rot.z + d).rem_euclid(360.0);
            }
        }
        if ui.add(egui::Button::new("Upright").corner_radius(4.0).min_size(egui::vec2(0.0, ROW_H))).on_hover_text("Its tilt off").clicked() {
            (p.rot.x, p.rot.y) = (0.0, 0.0);
        }
    });
    ui.add_space(10.0);
    section(ui, "LOOK");
    ui.horizontal(|ui| {
        field_label(ui, "Material");
        egui::ComboBox::from_id_salt("piece material").selected_text(capital(&p.material.to_lowercase().replace('_', " "))).width(ui.available_width() - 4.0).show_ui(ui, |ui| {
            for m in materials {
                ui.selectable_value(&mut p.material, m.clone(), capital(&m.to_lowercase().replace('_', " ")));
            }
        });
    });
    if let Some((name, usual, range)) = p.shape.detail() {
        ui.horizontal(|ui| {
            field_label(ui, name);
            let mut n = if p.detail == 0 { usual } else { p.detail };
            if ui.add(egui::Slider::new(&mut n, range).clamping(egui::SliderClamping::Always)).changed() {
                p.detail = n;
            }
        });
    }
    if p.shape != Shape::Pool {
        let fig = p.shape == Shape::Model && level.figure.as_ref().is_some_and(|f| f.0 == p.model);
        let was = if fig { 2 } else if p.loose.is_some() { 1 } else { 0 };
        let mut mode = was;
        ui.horizontal_wrapped(|ui| {
            field_label(ui, "Is");
            ui.selectable_value(&mut mode, 0, "Fixed").on_hover_text("Part of the level: it never moves");
            ui.selectable_value(&mut mode, 1, "Loose").on_hover_text("A physics object: it falls, tips and gets knocked about (held where you put it while the editor is open)");
            if p.shape == Shape::Model {
                ui.selectable_value(&mut mode, 2, "Homer's body").on_hover_text("Sized to him, each bit on the part of him nearest it, moving with it (turn the piece to face it his way)");
            }
        });
        if mode != was {
            p.loose = (mode == 1).then(|| p.loose.unwrap_or_default());
            if mode == 2 {
                level.figure = Some((p.model.clone(), p.transform().rotation));
            } else if fig {
                level.figure = None;
            }
            if mode == 2 || fig {
                (level.fig_moved, level.fig_pick) = (vec![], None);
            }
        }
        if let Some(l) = p.loose.as_mut() {
            ui.checkbox(&mut l.alive, "Alive").on_hover_text("It moves about on its own: breathes, rocks, waddles, hops away from Homer, shivers when hit, rights itself. Stays one solid piece (never bends)");
            for (name, v, lo, hi, step) in [("Weight kg", &mut l.kg, 0.1, 5000.0, 0.5), ("Friction", &mut l.friction, 0.0, 2.0, 0.01), ("Bounce", &mut l.bounce, 0.0, 1.0, 0.01), ("Float", &mut l.float, 0.0, 5.0, 0.05)] {
                ui.horizontal(|ui| {
                    field_label(ui, name);
                    let r = ui.add(egui::DragValue::new(v).range(lo..=hi).speed(step).max_decimals(2));
                    if name == "Float" {
                        r.on_hover_text("In water: 0 sinks, 1 hangs under the surface, 2 floats half in, more rides higher");
                    }
                });
            }
        }
    }
    if p.shape == Shape::Model {
        let on = level.figure.as_ref().is_some_and(|f| f.0 == p.model);
        if on && level.models.get(&p.model).is_some_and(|m| m.rig.iter().any(|j| j.is_finite())) {
            ui.horizontal(|ui| {
                field_label(ui, "Skeleton");
                ui.selectable_value(&mut level.fig_auto, false, "Its own rig").on_hover_text("The model's own bones and skin weights");
                ui.selectable_value(&mut level.fig_auto, true, "Fitted").on_hover_text("Joints guessed from its shape, as for a model with no rig");
            });
        }
        let n = level.fig_joints.len().min(delirium_data::nm_asset::PARTS.len());
        if on && n > 0 {
            let names = &delirium_data::nm_asset::PARTS;
            let mut k = level.fig_pick.unwrap_or(0).min(n - 1);
            ui.horizontal(|ui| {
                field_label(ui, "Joint");
                egui::ComboBox::from_id_salt("fig_joint").selected_text(names[k].replace('_', " ")).show_ui(ui, |ui| {
                    for (j, name) in names.iter().enumerate().take(n) {
                        ui.selectable_value(&mut k, j, name.replace('_', " "));
                    }
                });
                if ui.button("Guess again").on_hover_text("Every joint back where its shape puts it").clicked() {
                    level.fig_moved.clear();
                }
            });
            level.fig_pick = Some(k);
            let mut v = level.fig_joints[k];
            ui.horizontal(|ui| {
                field_label(ui, "At");
                let mut ch = false;
                for (a, x) in ["x ", "y ", "z "].iter().zip([&mut v.x, &mut v.y, &mut v.z]) {
                    ch |= ui.add(egui::DragValue::new(x).speed(0.002).max_decimals(3).prefix(*a)).changed();
                }
                if ch {
                    set_joint(level, k, v);
                }
            });
            ui.label(egui::RichText::new("Or drag its dot on the model (its own space, before the piece is turned or sized)").small().weak());
        }
    }
    if p.shape != Shape::Model {
        ui.horizontal(|ui| {
            field_label(ui, "Colour");
            let c = p.color.unwrap_or_else(|| colour(&p.material).to_srgba().to_f32_array());
            let b = c.map(|k| (k * 255.0).round() as u8);
            let mut e = egui::Color32::from_rgba_unmultiplied(b[0], b[1], b[2], b[3]);
            if egui::color_picker::color_edit_button_srgba(ui, &mut e, egui::color_picker::Alpha::OnlyBlend).changed() {
                p.color = Some(e.to_srgba_unmultiplied().map(|k| k as f32 / 255.0));
            }
            if p.color.is_some() && ui.add(egui::Button::new(egui::RichText::new("Reset").small()).frame(false)).on_hover_text("Back to its material's colour").clicked() {
                p.color = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut p.glass, "Glass").on_hover_text("Clear glass, as the sky pool's");
            });
        });
    }
    ui.add_space(12.0);
    ui.columns(2, |c| {
        if c[0].add_sized([c[0].available_width(), 28.0], egui::Button::new("Duplicate").corner_radius(6.0)).clicked() {
            let mut q = p.clone();
            q.pos += Vec3::new(p.size.x + 0.2, 0.0, 0.0);
            level.pieces.push(q);
            level.selected = Some(level.pieces.len() - 1);
            level.change += 1;
        }
        let red = egui::Color32::from_rgb(235, 95, 85);
        if c[1].add_sized([c[1].available_width(), 28.0], egui::Button::new(egui::RichText::new("Delete").color(red)).stroke(egui::Stroke::new(1.0, red.gamma_multiply(0.5))).corner_radius(6.0))
            .on_hover_text("Del").clicked() {
            level.pieces.remove(i);
            level.selected = None;
            level.change += 1;
        }
    });
    let changed = level.selected == Some(i) && level.pieces.get(i).is_some_and(|q| q.pos != p.pos || q.rot != p.rot || q.size != p.size || q.material != p.material || q.color != p.color || q.glass != p.glass || q.detail != p.detail || q.loose != p.loose);
    if changed {
        level.pieces[i] = p;
        level.change += 1;
    }
}

const LABEL_W: f32 = 66.0;

fn type_scale(ui: &mut egui::Ui) {
    let st = ui.style_mut();
    for (t, px) in [(egui::TextStyle::Body, 13.0), (egui::TextStyle::Button, 13.0), (egui::TextStyle::Small, 11.5)] {
        st.text_styles.insert(t, egui::FontId::proportional(px));
    }
    st.spacing.interact_size.y = ROW_H;
    st.spacing.item_spacing.y = 6.0;
}
const ROW_H: f32 = 24.0;

fn field_label(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    ui.allocate_ui_with_layout(egui::vec2(LABEL_W, ROW_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_min_width(LABEL_W);
        ui.add(egui::Label::new(text.into().color(ui.visuals().text_color().gamma_multiply(0.75))).selectable(false));
    });
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

fn section(ui: &mut egui::Ui, name: &str) {
    ui.label(egui::RichText::new(name).size(11.0).strong().extra_letter_spacing(0.8).color(ui.visuals().weak_text_color()));
    ui.add_space(2.0);
}

fn vec_row(ui: &mut egui::Ui, name: &str, v: &mut Vec3, speed: f64, min: f32, decimals: usize, unit: &str) {
    ui.horizontal(|ui| {
        field_label(ui, name);
        let (row, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), egui::Sense::hover());
        let gap = 6.0;
        let w = (row.width() - 2.0 * gap) / 3.0;
        for (k, (axis, col)) in [("X", egui::Color32::from_rgb(235, 95, 85)), ("Y", egui::Color32::from_rgb(120, 200, 95)), ("Z", egui::Color32::from_rgb(90, 150, 240))].into_iter().enumerate() {
            let r = egui::Rect::from_min_size(row.min + egui::vec2(k as f32 * (w + gap), 0.0), egui::vec2(w, ROW_H));
            let field = egui::Rect::from_min_max(r.min + egui::vec2(16.0, 0.0), r.max);
            let p = ui.painter();
            p.rect_filled(egui::Rect::from_min_size(r.min + egui::vec2(0.0, 4.0), egui::vec2(2.5, ROW_H - 8.0)), 1.0, col);
            p.text(egui::pos2(r.min.x + 9.0, r.center().y), egui::Align2::CENTER_CENTER, axis, egui::FontId::proportional(11.5), col);
            p.rect(field, 4.0, ui.visuals().widgets.inactive.weak_bg_fill, ui.visuals().widgets.inactive.bg_stroke, egui::StrokeKind::Inside);
            if v[k] == 0.0 {
                v[k] = 0.0;
            }
            let resp = ui.scope_builder(egui::UiBuilder::new().max_rect(field.translate(egui::vec2(0.0, -2.0))).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)), |ui| {
                let ws = &mut ui.visuals_mut().widgets;
                for w in [&mut ws.inactive, &mut ws.hovered, &mut ws.active] {
                    (w.weak_bg_fill, w.bg_fill, w.bg_stroke) = (egui::Color32::TRANSPARENT, egui::Color32::TRANSPARENT, egui::Stroke::NONE);
                }
                ui.add(egui::DragValue::new(&mut v[k]).speed(speed).range(min..=f32::MAX).max_decimals(decimals).min_decimals(decimals).suffix(unit))
            }).inner;
            if resp.hovered() || resp.has_focus() || resp.dragged() {
                ui.painter().rect_stroke(field, 4.0, egui::Stroke::new(1.0, col.gamma_multiply(0.8)), egui::StrokeKind::Inside);
            }
        }
    });
}

