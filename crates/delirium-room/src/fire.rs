use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

const SHADER: Handle<Shader> = bevy::asset::uuid_handle!("e3a1f7c2-58d4-4b9e-a06c-1d7f3b92c5e8");

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct FlameExt {
    #[uniform(100)]
    pub k: Vec4,
}
impl MaterialExtension for FlameExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}
pub type FlameMaterial = ExtendedMaterial<StandardMaterial, FlameExt>;

pub struct Source {
    pub at: Vec3,
    pub r: f32,
    pub k: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Sheet,
    Flame,
    Ember,
    Smoke,
    Steam,
}
struct P {
    at: Vec3,
    v: Vec3,
    age: f32,
    life: f32,
    size: f32,
    seed: f32,
    k: f32,
    kind: Kind,
}

#[derive(Component, Default)]
pub struct Fire {
    ps: Vec<P>,
    rng: u32,
    owed: Vec<f32>,
}
impl Fire {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.rand()
    }
}

pub fn plugin(app: &mut App) {
    let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(&SHADER, Shader::from_wesl(include_str!("flame.wesl"), "flame.wesl"));
    app.add_plugins(MaterialPlugin::<FlameMaterial>::default()).add_systems(Startup, spawn);
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<FlameMaterial>>) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    fill(&mut mesh, vec![[0.0; 3]; 3], vec![[0.0; 2]; 3], vec![[0.0; 4]; 3], vec![0, 1, 2]);
    let base = StandardMaterial { unlit: true, alpha_mode: AlphaMode::Premultiplied, double_sided: true, cull_mode: None, ..default() };
    commands.spawn((Fire { rng: 0x9E37_79B9, ..default() }, Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(FlameMaterial { base, extension: FlameExt { k: Vec4::ONE } })),
                    Transform::default(), Visibility::Hidden, NoFrustumCulling, bevy::light::NotShadowCaster, RenderLayers::layer(1)));
}

fn fill(mesh: &mut Mesh, p: Vec<[f32; 3]>, uv: Vec<[f32; 2]>, c: Vec<[f32; 4]>, i: Vec<u32>) {
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; p.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, c);
    mesh.insert_indices(Indices::U32(i));
}

fn rate(r: f32) -> f32 {
    12.0 + 40.0 * r * r
}
const EMBERS: f32 = 0.1;
const SMOKE: f32 = 0.12;
const STEAM: usize = 6;
const VOXEL: f32 = 0.07;
const VOXEL_RATE: f32 = 5.0;
const MAX: usize = 4000;

pub fn steam(fire: &mut Fire, at: &[Vec3]) {
    for &p in at {
        for _ in 0..STEAM {
            let v = Vec3::new(fire.range(-0.3, 0.3), fire.range(-0.3, 0.3), fire.range(0.5, 1.2));
            let (life, size, seed) = (fire.range(1.2, 2.2), fire.range(0.15, 0.3), fire.rand());
            fire.ps.push(P { at: p + v * 0.1, v, age: 0.0, life, size, seed, k: 1.0, kind: Kind::Steam });
        }
    }
}

pub fn step(fire: &mut Fire, mesh: &mut Mesh, sources: &[Source], dt: f32, cam: &GlobalTransform, voxel: bool, smoke: bool) -> bool {
    fire.owed.resize(sources.len(), 0.0);
    for (j, s) in sources.iter().enumerate() {
        fire.owed[j] += rate(s.r) * if voxel { VOXEL_RATE } else { 1.0 } * s.k * dt;
        while fire.owed[j] >= 1.0 {
            fire.owed[j] -= 1.0;
            let (a, d) = (fire.range(0.0, std::f32::consts::TAU), s.r * fire.rand().sqrt());
            let at = s.at + Vec3::new(a.cos() * d, a.sin() * d, -0.03);
            let mid = 1.0 - 0.6 * d / s.r.max(1e-3);
            let size = (0.18 + 0.35 * s.r.min(1.0)) * mid * fire.range(0.6, 1.3) * (0.5 + 0.5 * s.k);
            let flame = P { at, v: Vec3::new(fire.range(-0.15, 0.15), fire.range(-0.15, 0.15), fire.range(0.8, 1.4)), age: 0.0, life: fire.range(0.45, 0.85),
                            size, seed: fire.rand(), k: s.k, kind: Kind::Flame };
            fire.ps.push(flame);
            if fire.rand() < EMBERS {
                let v = Vec3::new(fire.range(-0.5, 0.5), fire.range(-0.5, 0.5), fire.range(1.5, 3.0));
                let (life, seed) = (fire.range(1.0, 2.0), fire.rand());
                fire.ps.push(P { at: at + Vec3::Z * size * 0.5, v, age: 0.0, life, size: 0.03, seed, k: 1.0, kind: Kind::Ember });
            }
            if fire.rand() < SMOKE && smoke {
                let v = Vec3::new(fire.range(-0.1, 0.1), fire.range(-0.1, 0.1), fire.range(0.6, 1.0));
                let (life, seed) = (fire.range(2.5, 4.0), fire.rand());
                fire.ps.push(P { at: at + Vec3::Z * size * 1.2, v, age: 0.0, life, size: size * 1.2, seed, k: s.k, kind: Kind::Smoke });
            }
        }
    }
    if fire.ps.len() > MAX {
        let extra = fire.ps.len() - MAX;
        fire.ps.drain(..extra);
    }
    for k in 0..fire.ps.len() {
        let kick = Vec3::new(fire.range(-1.0, 1.0), fire.range(-1.0, 1.0), fire.range(-0.3, 0.6)) * 4.0 * dt;
        let p = &mut fire.ps[k];
        if p.kind == Kind::Ember {
            p.v = (p.v + kick) * (1.0 - 0.6 * dt);
        }
        p.at += p.v * dt;
        p.age += dt;
    }
    fire.ps.retain(|p| p.age < p.life);
    if voxel {
        return voxels(fire, mesh, cam);
    }
    let (right, up, eye) = (cam.right().as_vec3(), cam.up().as_vec3(), cam.translation());
    let mut sheets = vec![];
    for (j, s) in sources.iter().enumerate().filter(|s| s.1.k > 0.0) {
        let side = Vec3::Z.cross(eye - s.at).normalize_or(right);
        let n = (1 + (s.r * 2.5) as usize).min(5);
        for i in 0..n {
            let off = ((i as f32 + 0.5) / n as f32 * 2.0 - 1.0) * s.r * 0.75;
            let w = (2.0 * s.r / n as f32 * 1.8 + 0.3) * (0.6 + 0.4 * s.k);
            let tall = 1.0 - 0.35 * off.abs() / s.r.max(1e-3);
            sheets.push(P { at: s.at + side * off - Vec3::Z * 0.03, v: Vec3::ZERO, age: 0.0, life: 1.0, size: w * tall, seed: (j as f32 * 0.137 + i as f32 * 0.311).fract(),
                            k: s.k, kind: Kind::Sheet });
        }
    }
    if fire.ps.is_empty() && sheets.is_empty() {
        return false;
    }
    let n = fire.ps.len() + sheets.len();
    let (mut pos, mut uv, mut col, mut idx) = (Vec::with_capacity(n * 4), Vec::with_capacity(n * 4), Vec::with_capacity(n * 4), Vec::with_capacity(n * 6));
    for kind in [Kind::Sheet, Kind::Flame, Kind::Ember, Kind::Smoke, Kind::Steam] {
        for p in fire.ps.iter().chain(&sheets).filter(|p| p.kind == kind) {
            let t = p.age / p.life;
            let (x, y, w, h, base) = match kind {
                Kind::Sheet => (Vec3::Z.cross(eye - p.at).normalize_or(right), Vec3::Z, p.size, p.size * 1.4, p.at),
                Kind::Flame => {
                    let side = Vec3::Z.cross(eye - p.at).normalize_or(right);
                    let s = p.size * (1.0 - t).sqrt() * (t / 0.08).min(1.0);
                    (side, Vec3::Z, s, s * 1.9, p.at)
                }
                Kind::Ember => (right, up, p.size, p.size, p.at - up * p.size * 0.5),
                Kind::Smoke | Kind::Steam => {
                    let s = p.size * (1.0 + 2.5 * t);
                    (right, up, s, s, p.at - up * s * 0.5)
                }
            };
            let at = pos.len() as u32;
            for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                pos.push((base + x * (u - 0.5) * w + y * v * h).to_array());
                uv.push([u, v]);
                col.push([t, p.seed, match kind { Kind::Sheet => 0.25, Kind::Flame => 0.0, Kind::Ember => 0.5, Kind::Smoke => 1.0, Kind::Steam => 1.5 }, p.k]);
            }
            idx.extend([at, at + 1, at + 2, at, at + 2, at + 3]);
        }
    }
    fill(mesh, pos, uv, col, idx);
    true
}

fn voxels(fire: &Fire, mesh: &mut Mesh, cam: &GlobalTransform) -> bool {
    let (right, up, eye) = (cam.right().as_vec3(), cam.up().as_vec3(), cam.translation());
    if fire.ps.is_empty() {
        return false;
    }
    let snap = |x: f32| ((x / VOXEL).round().max(1.0)) * VOXEL;
    let mut cubes: Vec<(f32, Vec3, f32, [f32; 4])> = fire.ps.iter().filter_map(|p| {
        let t = p.age / p.life;
        let (size, kind) = match p.kind {
            Kind::Flame => (snap(p.size * 0.8 * (1.0 - t)), 0.0),
            Kind::Ember => (VOXEL * 0.5, 0.5),
            Kind::Smoke | Kind::Steam | Kind::Sheet => return None,
        };
        let at = (p.at / VOXEL).round() * VOXEL + Vec3::Z * size * 0.5;
        Some((at.distance_squared(eye), at, size, [t, p.seed, kind, p.k]))
    }).collect();
    cubes.sort_by(|a, b| b.0.total_cmp(&a.0));
    let n = cubes.len();
    let (mut pos, mut uv, mut col, mut idx) = (Vec::with_capacity(n * 24), Vec::with_capacity(n * 24), Vec::with_capacity(n * 24), Vec::with_capacity(n * 36));
    let faces = [(Vec3::Z, Vec3::X, Vec3::Y, 1.0), (-Vec3::Z, Vec3::Y, Vec3::X, 0.45), (Vec3::X, Vec3::Y, Vec3::Z, 0.8), (-Vec3::X, Vec3::Z, Vec3::Y, 0.8),
                 (Vec3::Y, Vec3::Z, Vec3::X, 0.65), (-Vec3::Y, Vec3::X, Vec3::Z, 0.65)];
    for (_, at, size, c) in cubes {
        let h = size * 0.5;
        for (nrm, a, b, shade) in faces {
            let k = pos.len() as u32;
            for (u, v) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                pos.push((at + (nrm + a * u + b * v) * h).to_array());
                uv.push([shade, 0.0]);
                col.push(c);
            }
            idx.extend([k, k + 1, k + 2, k, k + 2, k + 3]);
        }
    }
    for p in fire.ps.iter().filter(|p| matches!(p.kind, Kind::Smoke | Kind::Steam)) {
        let (t, k) = (p.age / p.life, pos.len() as u32);
        let s = p.size * (1.0 + 2.5 * t);
        for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            pos.push((p.at - up * s * 0.5 + right * (u - 0.5) * s + up * v * s).to_array());
            uv.push([u, v]);
            col.push([t, p.seed, if p.kind == Kind::Steam { 1.5 } else { 1.0 }, p.k]);
        }
        idx.extend([k, k + 1, k + 2, k, k + 2, k + 3]);
    }
    fill(mesh, pos, uv, col, idx);
    true
}
