use crate::world::{over_pool, POOL, POOL_DEEP, WATER};
use crate::{HeroMesh, OursCam, PartMesh, PropMesh, Sim, Water};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{CameraProjection, RenderTarget};
use bevy::light::NotShadowCaster;
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::pbr::{DistanceFog, ExtendedMaterial, FogFalloff, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

const DT: f32 = 1.0 / 60.0;
const SPEED: f32 = 0.45;
const FADE: f32 = 2.5;
const VERT: f32 = 1.0 / 16.0;
const THREADS: usize = 8;
const SURFACE_MAX: f32 = 0.1;
const PIXEL: f32 = 1.0 / 16.0;
const SLOPE_MAX: f32 = 0.5;
const BREEZE: [(f32, f32, f32, f32); 4] = [(0.9, 0.004, 0.3, 0.55), (0.55, 0.0025, 1.4, 0.4), (0.33, 0.0015, -0.7, 0.32), (0.21, 0.0008, 2.4, 0.25)];

fn breeze_waves(cell: f32) -> &'static [(f32, f32, f32, f32)] {
    static FINE: std::sync::OnceLock<Vec<(f32, f32, f32, f32)>> = std::sync::OnceLock::new();
    if cell > 1.0 / 40.0 {
        return &BREEZE;
    }
    FINE.get_or_init(|| BREEZE[..2].iter().copied().chain((0..20).map(|i| {
        let len = 0.2 * 4f32.powf(i as f32 / 19.0);
        let k = TAU / len;
        (len, 0.2 / (k * k), (i as f32 * 2.39996 + 0.3) % TAU - PI, (9.81 / k).sqrt() * 0.5)
    })).collect())
}

fn cell_for(q: u8) -> f32 {
    if q >= 3 { 1.0 / 64.0 } else { 1.0 / 32.0 }
}

pub fn plugin(app: &mut App) {
    let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(&MIRROR_SHADER, Shader::from_wesl(include_str!("mirror.wesl"), "mirror.wesl"));
    app.add_plugins(MaterialPlugin::<MirrorMat>::default())
        .init_resource::<Under>()
        .add_systems(Startup, spawn_mirror)
        .add_systems(Update, (ripple, underwater))
        .add_systems(PostUpdate, mirror.before(bevy::camera::CameraUpdateSystems));
}

const MIRROR_SHADER: Handle<Shader> = bevy::asset::uuid_handle!("5c9d2e71-0a4b-4f3e-8d16-b7e2c4a9f013");

#[derive(Component)]
pub struct Mirror(Handle<Image>);

#[derive(Clone, Default, Debug, Reflect, ShaderType)]
struct MirrorU {
    clip_from_world: Mat4,
    k: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
struct MirrorExt {
    #[uniform(100)]
    u: MirrorU,
    #[texture(101)]
    #[sampler(102)]
    picture: Handle<Image>,
}
impl MaterialExtension for MirrorExt {
    fn fragment_shader() -> ShaderRef {
        MIRROR_SHADER.into()
    }
}
type MirrorMat = ExtendedMaterial<StandardMaterial, MirrorExt>;

const MIRROR_WOBBLE: f32 = 0.4;

const REFRACT_UP: f32 = 2.0;

fn spawn_mirror(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let img = images.add(Image::new_target_texture(16, 16, TextureFormat::Rgba16Float, None));
    commands.spawn((Camera3d::default(), Camera { order: -1, is_active: false, ..default() }, RenderTarget::Image(img.clone().into()), bevy::camera::Hdr,
                    bevy::render::view::Tonemapping::None, Msaa::Off, RenderLayers::from_layers(&[0, 1]), Mirror(img)));
}

fn mirror(sim: NonSend<Sim>, ours: Query<(&Camera, &Transform, &Projection), (With<OursCam>, Without<Mirror>)>,
          mut cam: Query<(&mut Camera, &mut Transform, &mut Projection, &mut RenderTarget, &mut Mirror), Without<OursCam>>,
          mut skins: Query<(&MeshMaterial3d<MirrorMat>, &mut Visibility)>, mut mats: ResMut<Assets<MirrorMat>>, mut images: ResMut<Assets<Image>>) {
    let (Some((oc, ot, Projection::Perspective(op))), Ok((mut c, mut t, mut proj, mut target, mut m))) = (ours.iter().next(), cam.single_mut()) else { return };
    let above = ot.translation.z > WATER;
    let on = sim.water_q >= 3 && sim.rt_now() == 0 && !skins.is_empty() && (ot.translation.z - WATER).abs() > 0.05 && (above || over_pool(ot.translation));
    for (_, mut v) in &mut skins {
        v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
    }
    c.is_active = on;
    let Some(size) = oc.physical_viewport_size().filter(|_| on).map(|s| (s / 2).max(UVec2::ONE)) else { return };
    if images.get(&m.0).is_none_or(|i| i.size() != size) {
        m.0 = images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Rgba16Float, None));
        *target = RenderTarget::Image(m.0.clone().into());
    }
    let flip = |v: Vec3| v.with_z(-v.z);
    *t = Transform::from_translation(ot.translation.with_z(2.0 * WATER - ot.translation.z)).looking_to(flip(*ot.forward()), flip(*ot.up()));
    let side = if above { 1.0 } else { -1.0 };
    let view = t.compute_affine().inverse();
    let (n, at) = (view.transform_vector3(Vec3::Z * side), view.transform_point3(Vec3::new(0.0, 0.0, WATER + 0.03 * side)));
    let p = PerspectiveProjection { aspect_ratio: size.x as f32 / size.y as f32, near_clip_plane: n.extend(-n.dot(at)), ..op.clone() };
    let u = MirrorU { clip_from_world: p.get_clip_from_view() * Mat4::from(view), k: Vec4::new(MIRROR_WOBBLE, REFRACT_UP, 0.0, 0.0) };
    *proj = Projection::Perspective(p);
    for (h, _) in &skins {
        if let Some(mut s) = mats.get_mut(&h.0) {
            (s.extension.u, s.extension.picture) = (u.clone(), m.0.clone());
        }
    }
}

#[derive(Resource, Default)]
pub struct Under(pub bool);

#[derive(Component)]
pub struct Haze;

#[derive(Component)]
pub struct Caustic;

pub const TRACED_ONLY: usize = 3;

#[derive(Component)]
pub struct Fx;

pub struct Ripples {
    w: usize,
    h: usize,
    cell: f32,
    now: Vec<f32>,
    was: Vec<f32>,
    slope: Vec<Vec2>,
}

impl Ripples {
    pub fn new(cell: f32) -> Self {
        let size = (POOL[1] - POOL[0]) / cell;
        let (w, h) = (size.x.round() as usize, size.y.round() as usize);
        Ripples { w, h, cell, now: vec![0.0; w * h], was: vec![0.0; w * h], slope: vec![Vec2::ZERO; w * h] }
    }

    fn cell(&self, p: Vec2) -> Option<(usize, usize)> {
        let c = ((p.x - POOL[0].x) / self.cell).floor();
        let r = ((POOL[1].y - p.y) / self.cell).floor();
        (c >= 0.0 && r >= 0.0 && (c as usize) < self.w && (r as usize) < self.h).then_some((c as usize, r as usize))
    }

    fn height(&self, p: Vec2) -> f32 {
        self.cell(p).map_or(0.0, |(c, r)| self.now[r * self.w + c])
    }

    pub fn poke(&mut self, p: Vec2, d: f32, r: f32) {
        let n = (r / self.cell / 2.0).ceil() as isize;
        let Some((c, w)) = self.cell(p) else { return };
        for y in -n..=n {
            for x in -n..=n {
                let (cx, cy) = (c as isize + x, w as isize + y);
                let f = 1.0 - ((x * x + y * y) as f32).sqrt() / (n as f32 + 1.0);
                if f > 0.0 && cx >= 0 && cy >= 0 && (cx as usize) < self.w && (cy as usize) < self.h {
                    self.now[cy as usize * self.w + cx as usize] += d * f;
                }
            }
        }
    }

    pub fn step(&mut self) {
        let k = (SPEED * DT / self.cell).powi(2);
        let keep = (-2.0 * DT / FADE).exp();
        let (w, h, now) = (self.w, self.h, &self.now);
        let rows = h.div_ceil(THREADS);
        std::thread::scope(|s| {
            for (ch, out) in self.was.chunks_mut(rows * w).enumerate() {
                s.spawn(move || {
                    for (j, o) in out.iter_mut().enumerate() {
                        let (x, y) = (j % w, ch * rows + j / w);
                        let i = y * w + x;
                        let at = |xx: usize, yy: usize| now[yy * w + xx];
                        let lap = at(x.saturating_sub(1), y) + at((x + 1).min(w - 1), y) + at(x, y.saturating_sub(1)) + at(x, (y + 1).min(h - 1)) - 4.0 * now[i];
                        *o = now[i] + (now[i] - *o) * keep + k * lap;
                    }
                });
            }
        });
        std::mem::swap(&mut self.now, &mut self.was);
    }

    pub fn pixels(&self, w: usize, out: &mut [u8]) {
        let k = self.w / w;
        for (j, px) in out.chunks_mut(4).enumerate() {
            let (x, y) = (j % w * k, j / w * k);
            let g = (y..y + k).flat_map(|yy| (x..x + k).map(move |xx| (xx, yy))).map(|(xx, yy)| self.slope[yy * self.w + xx]).sum::<Vec2>() / (k * k) as f32;
            let n = Vec3::new(-g.x, -g.y, 1.0).normalize();
            px.copy_from_slice(&[(n.x * 127.5 + 127.5) as u8, (n.y * 127.5 + 127.5) as u8, (n.z * 127.5 + 127.5) as u8, 255]);
        }
    }

    pub fn normals(&mut self, t: f32, out: &mut [u8]) {
        let (w, h, c) = (self.w, self.h, self.cell);
        let waves: Vec<_> = breeze_waves(c).iter().map(|&(len, amp, dir, speed)| {
            let (d, k) = (Vec2::from_angle(dir), TAU / len);
            let col: Vec<(f32, f32)> = (0..w).map(|x| (k * d.x * c * x as f32).sin_cos()).collect();
            let row: Vec<(f32, f32)> = (0..h).map(|y| (-k * d.y * c * y as f32 - k * speed * t).sin_cos()).collect();
            (amp * k * d, col, row)
        }).collect();
        let (now, waves) = (&self.now, &waves);
        let rows = h.div_ceil(THREADS);
        std::thread::scope(|s| {
            for (ch, (px, sl)) in out.chunks_mut(rows * w * 4).zip(self.slope.chunks_mut(rows * w)).enumerate() {
                s.spawn(move || {
                    for (j, sl) in sl.iter_mut().enumerate() {
                        let (x, y) = (j % w, ch * rows + j / w);
                        let at = |xx: usize, yy: usize| SURFACE_MAX * (now[yy * w + xx] / SURFACE_MAX).tanh();
                        let mut g = Vec2::new(at((x + 1).min(w - 1), y) - at(x.saturating_sub(1), y), at(x, y.saturating_sub(1)) - at(x, (y + 1).min(h - 1))) / (2.0 * c);
                        for (ak, col, row) in waves {
                            let ((sa, ca), (sb, cb)) = (col[x], row[y]);
                            g += *ak * (ca * cb - sa * sb);
                        }
                        let gl = g.length();
                        if gl > 1e-6 {
                            g *= SLOPE_MAX * (gl / SLOPE_MAX).tanh() / gl;
                        }
                        *sl = g;
                        let n = Vec3::new(-g.x, -g.y, 1.0).normalize();
                        px[j * 4..j * 4 + 4].copy_from_slice(&[(n.x * 127.5 + 127.5) as u8, (n.y * 127.5 + 127.5) as u8, (n.z * 127.5 + 127.5) as u8, 255]);
                    }
                });
            }
        });
    }
}

fn breeze(p: Vec2, t: f32) -> f32 {
    let p = p - Vec2::new(POOL[0].x, POOL[1].y);
    BREEZE.iter().map(|&(len, amp, dir, speed)| amp * (TAU / len * (p.dot(Vec2::from_angle(dir)) - speed * t)).sin()).sum()
}

pub fn surface(size: Vec2) -> Mesh {
    let (nx, ny) = ((size.x / VERT).round() as u32, (size.y / VERT).round() as u32);
    let (mut p, mut uv) = (vec![], vec![]);
    for y in 0..=ny {
        for x in 0..=nx {
            let (u, v) = (x as f32 / nx as f32, y as f32 / ny as f32);
            p.push([(u - 0.5) * size.x, (0.5 - v) * size.y, 0.0]);
            uv.push([u, v]);
        }
    }
    let mut i = vec![];
    for y in 0..ny {
        for x in 0..nx {
            let a = y * (nx + 1) + x;
            i.extend([a, a + nx + 1, a + 1, a + 1, a + nx + 1, a + nx + 2]);
        }
    }
    let n = p.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(i))
        .with_generated_tangents()
        .unwrap()
}

fn material(m: &mut StandardMaterial, q: u8) {
    (m.base_color, m.alpha_mode, m.specular_transmission) = if q < 2 {
        (Color::srgba(0.12, 0.42, 0.62, 0.55), AlphaMode::Blend, 0.0)
    } else {
        (Color::srgba(0.72, 0.9, 1.0, 0.7), AlphaMode::Opaque, 1.0)
    };
    m.thickness = if q >= 3 { REFRACT_DEPTH } else { 0.0 };
}

const REFRACT_DEPTH: f32 = 0.8;

struct Face {
    o: Vec3,
    u: Vec3,
    v: Vec3,
    n: Vec3,
    w: usize,
    h: usize,
    sum: Vec<f32>,
    img: Handle<Image>,
    mat: Handle<StandardMaterial>,
    e: Entity,
}

struct Drop {
    p: Vec3,
    v: Vec3,
}

struct Pool {
    water: Entity,
    q: u8,
    ripples: Ripples,
    img: Handle<Image>,
    pix: Handle<Image>,
    acc: f32,
    faces: Vec<Face>,
    tex: f32,
    drops: Vec<Drop>,
    spray: Option<(Handle<Mesh>, Entity)>,
    haze: Handle<Image>,
    vol: Vol,
    kids: Vec<Entity>,
    rng: u32,
}

struct Vol {
    img: Handle<Image>,
    w: usize,
    h: usize,
    tex: f32,
}

const SLICES: usize = 6;
const VOL_TEX: f32 = 1.0 / 16.0;

impl Pool {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
}

const HAZE: f32 = 2.0;
const HAZE_LIGHT: f32 = 20.0;
const HAZE_VOXEL: f32 = 0.1;
const SHAFT_BLUR: usize = 3;
const SHAFT_FEW: f32 = 1.0;

fn haze_size(size: Vec3) -> UVec3 {
    (size / HAZE_VOXEL).ceil().as_uvec3().max(UVec3::ONE)
}

fn shafts(floor: &Face, sun: Vec3, tex: f32, d: &mut [u8], n: Extent3d) {
    let (nx, ny, nz) = (n.width as usize, n.height as usize, n.depth_or_array_layers as usize);
    let down = refract(-sun, Vec3::Z, 1.0 / 1.33).unwrap_or(Vec3::NEG_Z);
    let lean = down.truncate() / -down.z;
    let k = (HAZE_VOXEL / tex).round().max(1.0) as usize;
    let (fx, fy) = (nx.min(floor.w / k), ny.min(floor.h / k));
    let mut map = vec![0.0f32; fx * fy];
    for y in 0..fy * k {
        for x in 0..fx * k {
            map[y / k * fx + x / k] += floor.sum[y * floor.w + x];
        }
    }
    map.iter_mut().for_each(|m| *m /= (k * k) as f32);
    let r = SHAFT_BLUR as isize;
    let blur: Vec<f32> = (0..fx * fy).map(|i| {
        let (x, y) = ((i % fx) as isize, (i / fx) as isize);
        let (mut t, mut c) = (0.0, 0.0);
        for yy in (y - r).max(0)..(y + r + 1).min(fy as isize) {
            for xx in (x - r).max(0)..(x + r + 1).min(fx as isize) {
                t += map[yy as usize * fx + xx as usize];
                c += 1.0;
            }
        }
        t / c
    }).collect();
    let mean = blur.iter().sum::<f32>() / blur.len().max(1) as f32;
    let sd = (blur.iter().map(|b| (b - mean).powi(2)).sum::<f32>() / blur.len().max(1) as f32).sqrt().max(1e-4);
    let map: Vec<f32> = blur.iter().map(|b| ((b - mean) / sd - SHAFT_FEW).clamp(0.0, 1.0)).collect();
    let depth = nz as f32 * HAZE_VOXEL;
    for z in 0..nz {
        let under = depth - (z as f32 + 0.5) * HAZE_VOXEL;
        let focus = (under / 0.5).min(1.0);
        let shift = lean * (z as f32 + 0.5);
        for y in 0..ny {
            for x in 0..nx {
                let (u, v) = ((x as f32 + shift.x).round(), (y as f32 + shift.y).round());
                let bright = if u >= 0.0 && v >= 0.0 && (u as usize) < fx && (v as usize) < fy { map[v as usize * fx + u as usize] } else { 0.0 };
                d[(z * ny + y) * nx + x] = ((0.03 + 0.97 * bright * focus) * 255.0) as u8;
            }
        }
    }
}

fn refract(i: Vec3, n: Vec3, eta: f32) -> Option<Vec3> {
    let c = -n.dot(i);
    let k = 1.0 - eta * eta * (1.0 - c * c);
    (k >= 0.0).then(|| eta * i + (eta * c - k.sqrt()) * n)
}

fn caustics(r: &Ripples, faces: &mut [Face], vol: Option<&Vol>, sun: Vec3, tex: f32, images: &mut Assets<Image>) {
    let (i, eta) = (-sun, 1.0 / 1.33);
    let centre = ((POOL[0] + POOL[1]) * 0.5).extend(WATER);
    let (a, b) = (POOL[0] - centre.truncate(), POOL[1] - centre.truncate());
    let deep = WATER + POOL_DEEP;
    let flat = refract(i, Vec3::Z, eta).unwrap_or(Vec3::NEG_Z);
    let (w, h, c) = (r.w, r.h, r.cell);
    let mut lean = vec![Vec2::ZERO; w * h];
    let rows = h.div_ceil(THREADS);
    std::thread::scope(|s| {
        for (ch, out) in lean.chunks_mut(rows * w).enumerate() {
            s.spawn(move || {
                for (j, o) in out.iter_mut().enumerate() {
                    let g = r.slope[ch * rows * w + j];
                    let d = refract(i, Vec3::new(-g.x, -g.y, 1.0).normalize(), eta).unwrap_or(flat);
                    *o = d.truncate() / -d.z;
                }
            });
        }
    });
    let n = 2;
    let e = (c / n as f32 / tex).powi(2);
    let geo: Vec<_> = faces.iter().map(|f| (f.o, f.u, f.v, f.w, f.h)).collect();
    let (lean, geo) = (&lean, &geo);
    let body = vol.map(|v| (v.w, v.h, v.tex, 4.0 * (c / n as f32 / v.tex).powi(2)));
    let sums: Vec<(Vec<Vec<f32>>, Vec<f32>)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..THREADS).map(|t| s.spawn(move || {
            let mut sum: Vec<Vec<f32>> = geo.iter().map(|g| vec![0.0; g.3 * g.4]).collect();
            let mut slices = vec![0.0; body.map_or(0, |b| b.0 * b.1 * SLICES)];
            for y in (t * rows..((t + 1) * rows).min(h)).flat_map(|y| (0..n).map(move |k| y as f32 + (k as f32 + 0.5) / n as f32 - 0.5)) {
                let (y0, ty) = (y.floor().clamp(0.0, (h - 2) as f32), y - y.floor().clamp(0.0, (h - 2) as f32));
                let y0 = y0 as usize;
                for x in (0..w).flat_map(|x| (0..n).map(move |k| x as f32 + (k as f32 + 0.5) / n as f32 - 0.5)) {
                    let x0 = x.floor().clamp(0.0, (w - 2) as f32);
                    let tx = x - x0;
                    let x0 = x0 as usize;
                    let at = |xx: usize, yy: usize| lean[yy * w + xx];
                    let l = at(x0, y0).lerp(at(x0 + 1, y0), tx).lerp(at(x0, y0 + 1).lerp(at(x0 + 1, y0 + 1), tx), ty);
                    let q = Vec2::new(a.x + (x + 0.5) * c, b.y - (y + 0.5) * c);
                    let p = q + l * deep;
                    let (mut f, mut t) = (0, deep);
                    for (k, edge, along, go) in [(1, a.x, q.x, l.x), (2, b.x, q.x, l.x), (3, a.y, q.y, l.y), (4, b.y, q.y, l.y)] {
                        let out = if k % 2 == 1 { (if k < 3 { p.x } else { p.y }) < edge } else { (if k < 3 { p.x } else { p.y }) > edge };
                        if out && go != 0.0 && (edge - along) / go < t {
                            (f, t) = (k, ((edge - along) / go).max(0.0));
                        }
                    }
                    if let Some((vw, vh, vt, ve)) = body && x < x.round() && y < y.round() {
                        for k in 0..SLICES {
                            let d = deep * (k + 1) as f32 / SLICES as f32;
                            if d > t + 1e-4 {
                                break;
                            }
                            let at = q + l * d;
                            let (fu, fv) = ((at.x - a.x) / vt - 0.5, (b.y - at.y) / vt - 0.5);
                            let (iu, iv) = (fu.floor(), fv.floor());
                            let (su, sv) = (fu - iu, fv - iv);
                            for (du, dv, wt) in [(0, 0, (1.0 - su) * (1.0 - sv)), (1, 0, su * (1.0 - sv)), (0, 1, (1.0 - su) * sv), (1, 1, su * sv)] {
                                let (xu, yv) = (iu as i64 + du, iv as i64 + dv);
                                if xu >= 0 && yv >= 0 && (xu as usize) < vw && (yv as usize) < vh {
                                    slices[(k * vh + yv as usize) * vw + xu as usize] += wt * ve;
                                }
                            }
                        }
                    }
                    let (o, u, v, fw, fh) = geo[f];
                    let hit = (q + l * t).extend(-t) - o;
                    let (fu, fv) = (hit.dot(u) / tex - 0.5, hit.dot(v) / tex - 0.5);
                    let (iu, iv) = (fu.floor(), fv.floor());
                    let (su, sv) = (fu - iu, fv - iv);
                    for (du, dv, k) in [(0, 0, (1.0 - su) * (1.0 - sv)), (1, 0, su * (1.0 - sv)), (0, 1, (1.0 - su) * sv), (1, 1, su * sv)] {
                        let (xu, yv) = (iu as i64 + du, iv as i64 + dv);
                        if xu >= 0 && yv >= 0 && (xu as usize) < fw && (yv as usize) < fh {
                            sum[f][yv as usize * fw + xu as usize] += k * e;
                        }
                    }
                }
            }
            (sum, slices)
        })).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (k, f) in faces.iter_mut().enumerate() {
        f.sum.fill(0.0);
        for t in &sums {
            f.sum.iter_mut().zip(&t.0[k]).for_each(|(s, x)| *s += x);
        }
        let flat_lit = (flat.dot(f.n) / flat.z).abs();
        if let Some(mut img) = images.get_mut(&f.img) && let Some(data) = img.data.as_mut() {
            for (px, e) in data.chunks_mut(4).zip(&f.sum) {
                let k = if flat_lit > 0.05 { e / flat_lit } else { 1.0 };
                px[..3].fill(((k / 3.0).clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    if let Some(v) = vol && let Some(mut img) = images.get_mut(&v.img) && let Some(data) = img.data.as_mut() {
        for (i, px) in data.chunks_mut(4).enumerate() {
            let e: f32 = sums.iter().map(|t| t.1[i]).sum();
            px[0] = ((e / 4.0).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

fn ripple(mut commands: Commands, time: Res<Time>, sim: NonSend<Sim>, water: Query<(Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform, &RenderLayers), With<Water>>,
          things: Query<(Entity, &GlobalTransform, &InheritedVisibility), Or<(With<PartMesh>, With<HeroMesh>, With<PropMesh>)>>,
          suns: Query<(&DirectionalLight, &GlobalTransform)>, cams: Query<(&Camera, &GlobalTransform), Without<Mirror>>,
          mut mats: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>, mut skins: ResMut<Assets<MirrorMat>>,
          (under, mut wc, mut vox): (Res<Under>, ResMut<bevy::pbr::WaterCaustics>, Local<Option<(Entity, Handle<Mesh>)>>), mut state: Local<Option<Pool>>, mut was: Local<HashMap<Entity, Vec3>>, mut flat: Local<bool>, mut block: Local<Option<(Entity, Entity)>>) {
    let Some((we, wmesh, mat, wt, layers)) = water.iter().next() else { return };
    let q = sim.water_q;
    let blocks = sim.voxel_water && q >= 1;
    if !blocks && let Some((e, _)) = vox.take() {
        commands.entity(e).try_despawn();
    }
    if block.is_some_and(|(w, _)| w != we || q != 0) {
        commands.entity(block.take().unwrap().1).try_despawn();
    }
    if q == 0 && block.is_none() {
        let (a, b) = ((POOL[0] + 0.01).extend(0.01 - POOL_DEEP), (POOL[1] - 0.01).extend(WATER - 0.002));
        let m = mats.add(StandardMaterial { base_color: Color::srgba(0.1, 0.45, 0.7, 0.4), alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, ..default() });
        let e = commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(b - a))), MeshMaterial3d(m), Transform::from_translation((a + b) * 0.5 - wt.translation()),
                                RenderLayers::layer(1), NotShadowCaster, Fx, ChildOf(we))).id();
        *block = Some((we, e));
    }
    if let Some(m) = mats.get(&mat.0) && (m.specular_transmission > 0.0) != (q >= 2) {
        material(&mut mats.get_mut(&mat.0).unwrap(), q);
    }
    if state.as_ref().is_some_and(|s| s.water != we || s.q != q) || q == 0 {
        if let Some(s) = state.take() {
            for e in s.kids {
                commands.entity(e).try_despawn();
            }
        }
    }
    if q == 0 {
        if mats.get(&mat.0).is_some_and(|m| m.normal_map_texture.is_some()) {
            mats.get_mut(&mat.0).unwrap().normal_map_texture = None;
        }
    }
    let rt = sim.rt_now();
    let traced = q >= 3 && rt > 0;
    let want = RenderLayers::layer(if traced || blocks { TRACED_ONLY } else { 1 });
    if *layers != want {
        commands.entity(we).insert(want);
    }
    let extra = q >= 3 && rt == 0;
    if !extra && !*flat {
        if let Some(mut mesh) = meshes.get_mut(&wmesh.0) && let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
            p.iter_mut().for_each(|v| v[2] = 0.0);
        }
        *flat = true;
    }
    if q == 0 {
        if wc.image.is_some() {
            wc.image = None;
        }
        return;
    }
    let pool = state.get_or_insert_with(|| {
        let r = Ripples::new(cell_for(q));
        let size = Extent3d { width: r.w as u32, height: r.h as u32, depth_or_array_layers: 1 };
        let img = images.add(Image::new_fill(size, TextureDimension::D2, &[128, 128, 255, 255], TextureFormat::Rgba8Unorm, RenderAssetUsages::default()));
        let pix = if q >= 3 {
            let k = (PIXEL / r.cell).round() as usize;
            let mut i = Image::new_fill(Extent3d { width: (r.w / k) as u32, height: (r.h / k) as u32, depth_or_array_layers: 1 }, TextureDimension::D2, &[128, 128, 255, 255],
                                        TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
            i.sampler = ImageSampler::nearest();
            images.add(i)
        } else {
            img.clone()
        };
        let mut p = Pool { water: we, q, ripples: r, img, pix, acc: 0.0, faces: vec![], tex: if q >= 4 { 1.0 / 64.0 } else { 1.0 / 32.0 }, drops: vec![], spray: None, haze: Handle::default(),
                              vol: Vol { img: Handle::default(), w: 0, h: 0, tex: 0.0 }, kids: vec![], rng: 0x2545_F491 };
        if q >= 3 {
            let centre = ((POOL[0] + POOL[1]) * 0.5).extend(WATER);
            let (a, b) = (POOL[0].extend(-POOL_DEEP) - centre, POOL[1].extend(WATER) - centre);
            let (sx, sy, sz) = (b.x - a.x, b.y - a.y, b.z - a.z);
            for (o, u, v, n, wu, wv) in [(a, Vec3::X, Vec3::Y, Vec3::Z, sx, sy), (a, Vec3::Y, Vec3::Z, Vec3::X, sy, sz), (Vec3::new(b.x, a.y, a.z), Vec3::Y, Vec3::Z, Vec3::NEG_X, sy, sz),
                                         (a, Vec3::X, Vec3::Z, Vec3::Y, sx, sz), (Vec3::new(a.x, b.y, a.z), Vec3::X, Vec3::Z, Vec3::NEG_Y, sx, sz)] {
                let (w, h) = ((wu / p.tex).ceil() as usize, (wv / p.tex).ceil() as usize);
                let mut img = Image::new_fill(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 }, TextureDimension::D2, &[85, 85, 85, 255], TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
                img.sampler = ImageSampler::linear();
                let img = images.add(img);
                let mat = mats.add(StandardMaterial { base_color: LinearRgba::rgb(0.787 * 3.0, 0.831 * 3.0, 0.849 * 3.0).into(), base_color_texture: Some(img.clone()),
                                                      perceptual_roughness: 0.9, cull_mode: None, ..default() });
                let c = o + n * 0.004;
                let quad = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![c, c + u * wu, c + u * wu + v * wv, c + v * wv])
                    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![n; 4])
                    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
                    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
                let e = commands.spawn((Mesh3d(meshes.add(quad)), MeshMaterial3d(mat.clone()), Transform::default(), RenderLayers::layer(1), NotShadowCaster, Fx, Caustic,
                                        ChildOf(we))).id();
                p.kids.push(e);
                p.faces.push(Face { o, u, v, n, w, h, sum: vec![0.0; w * h], img, mat, e });
            }
            let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![Vec3::ZERO; 3]).with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Z; 3])
                .with_inserted_indices(Indices::U32(vec![0, 1, 2])));
            let e = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgba(0.85, 0.93, 1.0, 0.55), alpha_mode: AlphaMode::Blend,
                                                                                                   perceptual_roughness: 0.15, cull_mode: None, ..default() })),
                                    Transform::default(), RenderLayers::layer(1), NotShadowCaster, Fx, ChildOf(we))).id();
            p.kids.push(e);
            p.spray = Some((mesh, e));
            let tex = VOL_TEX;
            let (w, h) = ((sx / tex).ceil() as usize, (sy / tex).ceil() as usize);
            let mut img = Image::new_fill(Extent3d { width: w as u32, height: (h * SLICES) as u32, depth_or_array_layers: 1 }, TextureDimension::D2,
                                          &[64, 255, 255, (POOL_DEEP + WATER) .div_euclid(0.02).min(255.0) as u8], TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
            img.sampler = ImageSampler::linear();
            p.vol = Vol { img: images.add(img), w, h, tex };
            let (lo, hi) = (-POOL_DEEP - WATER, -0.12);
            let n = haze_size(Vec3::new(sx, sy, hi - lo));
            let mut img = Image::new_fill(Extent3d { width: n.x, height: n.y, depth_or_array_layers: n.z }, TextureDimension::D3, &[0], TextureFormat::R8Unorm,
                                          RenderAssetUsages::default());
            img.sampler = ImageSampler::linear();
            p.haze = images.add(img);
            p.kids.push(commands.spawn((bevy::light::FogVolume { fog_color: Color::srgb(0.3, 0.75, 0.85), density_factor: HAZE, absorption: 0.0, scattering: 0.6,
                                                                 scattering_asymmetry: 0.3, light_tint: Color::srgb(0.75, 0.95, 1.0), light_intensity: HAZE_LIGHT,
                                                                 density_texture: Some(p.haze.clone()), ..default() },
                                        Transform::from_xyz(0.0, 0.0, (lo + hi) * 0.5).with_scale(Vec3::new(sx, sy, hi - lo)), Visibility::Hidden, RenderLayers::layer(1),
                                        Haze, ChildOf(we))).id());
            let skin = skins.add(MirrorMat { base: StandardMaterial { alpha_mode: AlphaMode::Blend, normal_map_texture: Some(p.pix.clone()), cull_mode: None,
                                                                       double_sided: true, specular_transmission: 1.0, ..default() },
                                             extension: MirrorExt { u: default(), picture: Handle::default() } });
            p.kids.push(commands.spawn((Mesh3d(wmesh.0.clone()), MeshMaterial3d(skin), Transform::from_xyz(0.0, 0.0, 0.002), RenderLayers::layer(1), NotShadowCaster, Fx,
                                        ChildOf(we))).id());
        }
        p
    });
    let dt = time.delta_secs().max(1e-4);
    for (e, t, shown) in &things {
        let p = t.translation();
        let before = was.insert(e, p);
        let Some(q0) = before.filter(|q| shown.get() && q.distance(p) < 0.5) else { continue };
        if !over_pool(p) || (p.z - WATER).abs() > 0.2 {
            continue;
        }
        let d = p - q0;
        pool.ripples.poke(p.truncate(), (d.z * 0.5 - d.truncate().length() * 0.15).clamp(-0.02, 0.02), 0.25);
        let speed = d.length() / dt;
        if extra && speed > 1.5 && pool.drops.len() < 6000 {
            let n = ((speed - 1.5) * if q >= 4 { 9.0 } else { 3.0 }) as usize;
            for _ in 0..n {
                let (ang, r, up, out) = (pool.rand() * TAU, pool.rand() * 0.12, 0.8 + pool.rand() * 2.2, 0.3 + pool.rand() * 1.2);
                let k = (speed / 4.0).min(1.5);
                let dir = Vec2::from_angle(ang);
                pool.drops.push(Drop { p: (p.truncate() + dir * r).extend(WATER + 0.01), v: (dir * out * k).extend(up * k) + d.with_z(0.0) / dt * 0.2 });
            }
        }
    }
    was.retain(|e, _| things.contains(*e));
    pool.acc = (pool.acc + dt).min(4.0 * DT);
    while pool.acc >= DT {
        pool.ripples.step();
        pool.acc -= DT;
    }
    if blocks {
        let (_, mesh) = vox.get_or_insert_with(|| {
            let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
            let m = mats.add(StandardMaterial { base_color: Color::srgba(0.17, 0.4, 0.9, 0.6), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.1, ..default() });
            (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), Transform::default(), RenderLayers::layer(1), NotShadowCaster, Fx)).id(), mesh)
        });
        if let Some(mut m) = meshes.get_mut(mesh) {
            voxels(&pool.ripples, &mut m);
        }
    }
    if let Some(mut m) = mats.get_mut(&mat.0) {
        m.normal_map_texture = Some(pool.pix.clone());
    }
    let t = time.elapsed_secs();
    if let Some(mut i) = images.get_mut(&pool.img) && let Some(data) = i.data.as_mut() {
        pool.ripples.normals(t, data);
    }
    if pool.pix != pool.img && let Some(mut i) = images.get_mut(&pool.pix) {
        let w = i.width() as usize;
        if let Some(data) = i.data.as_mut() {
            pool.ripples.pixels(w, data);
        }
    }
    for &e in &pool.kids {
        commands.entity(e).try_insert(if extra { Visibility::Inherited } else { Visibility::Hidden });
    }
    for f in pool.faces.iter().filter(|_| traced) {
        commands.entity(f.e).try_insert(Visibility::Inherited);
    }
    let vol = traced.then(|| pool.vol.img.clone());
    if mats.get(&mat.0).is_some_and(|m| m.metallic_roughness_texture != vol) && let Some(mut m) = mats.get_mut(&mat.0) {
        m.metallic_roughness_texture = vol;
    }
    let on = extra.then(|| pool.vol.img.clone());
    if wc.image != on {
        let (w, h) = (pool.vol.w as f32 * pool.vol.tex, pool.vol.h as f32 * pool.vol.tex);
        *wc = bevy::pbr::WaterCaustics { image: on, min: Vec2::new(POOL[0].x, POOL[1].y - h), max: Vec2::new(POOL[0].x + w, POOL[1].y), surface: WATER,
                                         deep: WATER + POOL_DEEP, layers: SLICES as u32 };
    }
    if !extra && !traced {
        return;
    }
    let centre = wt.translation().truncate();
    if extra && let Some(mut mesh) = meshes.get_mut(&wmesh.0) && let Some(VertexAttributeValues::Float32x3(p)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        for v in p.iter_mut() {
            let w = centre + Vec2::new(v[0], v[1]);
            v[2] = SURFACE_MAX * (pool.ripples.height(w) / SURFACE_MAX).tanh() + breeze(w, t);
        }
        *flat = false;
    }
    if let Some((_, st)) = suns.iter().max_by(|a, b| a.0.illuminance.total_cmp(&b.0.illuminance)) && st.back().z > 0.05 {
        caustics(&pool.ripples, &mut pool.faces, Some(&pool.vol), st.back().as_vec3(), pool.tex, &mut images);
        if under.0 && let Some(mut img) = images.get_mut(&pool.haze) {
            let n = img.texture_descriptor.size;
            if let Some(d) = img.data.as_mut() {
                shafts(&pool.faces[0], st.back().as_vec3(), pool.tex, d, n);
            }
        }
        for f in &pool.faces {
            if let Some(mut m) = mats.get_mut(&f.mat) {
                m.base_color_texture = Some(f.img.clone());
            }
        }
    }
    if !extra {
        return;
    }
    let g = Vec3::new(0.0, 0.0, -9.81);
    let mut landed = vec![];
    pool.drops.retain_mut(|d| {
        d.v += g * dt;
        d.p += d.v * dt;
        if d.p.z < WATER && d.v.z < 0.0 {
            if over_pool(d.p) {
                landed.push(d.p.truncate());
            }
            return false;
        }
        d.p.z > -0.5
    });
    for p in landed {
        pool.ripples.poke(p, -0.0015, 0.04);
    }
    let Some(eye) = cams.iter().filter(|(c, _)| c.is_active).min_by_key(|(c, _)| c.order).map(|(_, t)| t.translation()) else { return };
    let Some((mesh, _)) = &pool.spray else { return };
    if let Some(mut m) = meshes.get_mut(mesh) {
        let (mut p, mut n, mut i) = (vec![], vec![], vec![]);
        let o = wt.translation();
        for d in &pool.drops {
            let f = (eye - d.p).normalize_or(Vec3::Z);
            let along = (d.v - f * d.v.dot(f)).normalize_or(Vec3::Z);
            let side = f.cross(along).normalize_or(Vec3::X) * 0.006;
            let len = along * (0.008 + d.v.length() * 0.012);
            let c = d.p - o;
            let k = p.len() as u32;
            p.extend([c - side - len, c + side - len, c + side + len, c - side + len]);
            n.extend([f; 4]);
            i.extend([k, k + 1, k + 2, k, k + 2, k + 3]);
        }
        if p.is_empty() {
            (p, n, i) = (vec![Vec3::ZERO; 3], vec![Vec3::Z; 3], vec![0, 1, 2]);
        }
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
        m.insert_indices(Indices::U32(i));
    }
}

const VOX: f32 = 0.125;
const VOX_STEP: f32 = 1.0 / 16.0;
const VOX_HIGH: f32 = 4.0;

fn voxels(r: &Ripples, m: &mut Mesh) {
    let (w, h) = (((POOL[1].x - POOL[0].x) / VOX) as usize, ((POOL[1].y - POOL[0].y) / VOX) as usize);
    let top = |x: usize, y: usize| {
        let c = Vec2::new(POOL[0].x + (x as f32 + 0.5) * VOX, POOL[0].y + (y as f32 + 0.5) * VOX);
        WATER + (r.height(c) * VOX_HIGH / VOX_STEP).round().clamp(-3.0, 4.0) * VOX_STEP
    };
    let tops: Vec<f32> = (0..w * h).map(|i| top(i % w, i / w)).collect();
    let (mut p, mut n, mut i) = (vec![], vec![], vec![]);
    let mut quad = |c: [Vec3; 4], f: Vec3| {
        let k = p.len() as u32;
        p.extend(c);
        n.extend([f; 4]);
        i.extend([k, k + 1, k + 2, k, k + 2, k + 3]);
    };
    for y in 0..h {
        for x in 0..w {
            let z = tops[y * w + x];
            let (a, b) = (POOL[0] + Vec2::new(x as f32, y as f32) * VOX, POOL[0] + Vec2::new(x as f32 + 1.0, y as f32 + 1.0) * VOX);
            quad([a.extend(z), Vec2::new(b.x, a.y).extend(z), b.extend(z), Vec2::new(a.x, b.y).extend(z)], Vec3::Z);
            for (dx, dy) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                    continue;
                }
                let lo = tops[ny as usize * w + nx as usize];
                if lo >= z {
                    continue;
                }
                let f = Vec3::new(dx as f32, dy as f32, 0.0);
                let (s, e) = match (dx, dy) {
                    (1, _) => (Vec2::new(b.x, a.y), b),
                    (-1, _) => (Vec2::new(a.x, b.y), a),
                    (_, 1) => (b, Vec2::new(a.x, b.y)),
                    _ => (a, Vec2::new(b.x, a.y)),
                };
                quad([s.extend(lo), e.extend(lo), e.extend(z), s.extend(z)], f);
            }
        }
    }
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
    m.insert_indices(Indices::U32(i));
}

fn underwater(mut commands: Commands, sim: NonSend<Sim>,
              mut cams: Query<(Entity, &Camera, &GlobalTransform, Option<&DistanceFog>, Option<&mut bevy::solari::prelude::SolariLighting>, Option<&bevy::light::Skybox>,
                              Option<&mut bevy::pbr::AtmosphereSettings>),
                              Without<Mirror>>,
              mirrors: Query<Entity, With<Mirror>>, water: Query<&InheritedVisibility, With<Water>>, mut haze: Query<&mut Visibility, With<Haze>>,
              mut flag: ResMut<Under>, mut saved: Local<Option<Option<DistanceFog>>>, mut sky: Local<Option<bevy::light::Skybox>>) {
    let Some((e, _, t, fog, lit, skybox, atmosphere)) = cams.iter_mut().filter(|c| c.1.is_active).min_by_key(|c| c.1.order) else { return };
    let p = t.translation();
    let wet = sim.water_q >= 1 && sim.rt_now() != 2 && water.iter().any(|v| v.get()) && over_pool(p) && p.z < WATER;
    let under = wet && sim.water_q >= 3;
    if flag.0 != under {
        flag.0 = under;
    }
    let own_sky = wet && sim.rt_now() == 1;
    if own_sky && let Some(s) = skybox {
        *sky = Some(s.clone());
        commands.entity(e).remove::<bevy::light::Skybox>();
    } else if !own_sky && let Some(s) = sky.take() {
        commands.entity(e).insert(s);
    }
    if let Some(mut a) = atmosphere && a.sky_off != own_sky {
        a.sky_off = own_sky;
    }
    for mut v in &mut haze {
        v.set_if_neq(if under && sim.rt_now() == 0 { Visibility::Inherited } else { Visibility::Hidden });
    }
    if wet && saved.is_none() {
        *saved = Some(fog.cloned());
        let f = under_haze();
        commands.entity(e).insert(f.clone());
        mirrors.iter().for_each(|m| { commands.entity(m).insert(f.clone()); });
    } else if !wet && let Some(f) = saved.take() {
        match f {
            Some(f) => commands.entity(e).insert(f),
            None => commands.entity(e).remove::<DistanceFog>(),
        };
        mirrors.iter().for_each(|m| { commands.entity(m).remove::<DistanceFog>(); });
    }
    if let Some(mut l) = lit && let FogFalloff::Atmospheric { extinction: x, .. } = under_haze().falloff {
        let pool = sim.water_q >= 3 && sim.rt_now() == 1 && water.iter().any(|v| v.get());
        let want = if pool || (wet && sim.rt_now() == 1) { x.extend(wet as u8 as f32).to_array() } else { [0.0; 4] };
        if l.water_fog != want {
            l.water_fog = want;
            l.water_fog_color = [0.0, 0.0, 0.0, WATER];
        }
    }
}

fn under_haze() -> DistanceFog {
    DistanceFog { color: Color::srgb(0.07, 0.36, 0.48), directional_light_color: Color::srgba(0.55, 0.9, 1.0, 0.6), directional_light_exponent: 12.0,
                  falloff: FogFalloff::Atmospheric { extinction: Vec3::new(0.45, 0.12, 0.08), inscattering: Vec3::splat(0.12) },
                  surface: WATER }
}

