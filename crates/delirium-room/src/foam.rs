use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

const TEXEL: f32 = 0.015;
const CELL: f32 = 0.045;
const LIFE: f32 = 1.2;
const PHASE: f32 = 1.5;
const SPLAT: f32 = 0.15;
const PAT: usize = 128;
const THREADS: usize = 8;

pub struct Foam {
    w: usize,
    h: usize,
    lo: Vec2,
    d: Vec<f32>,
    v: Vec<Vec2>,
    t: f32,
    cells: Vec<f32>,
    pub img: Handle<Image>,
    pub mesh: Handle<Mesh>,
}

impl Foam {

    pub fn new(lo: Vec2, size: Vec2, images: &mut Assets<Image>, meshes: &mut Assets<Mesh>) -> Self {
        let (w, h) = ((size.x / TEXEL).ceil() as usize, (size.y / TEXEL).ceil() as usize);
        let n = 8;
        let pts: Vec<Vec2> = (0..n * n).map(|i| {
            let j = Vec2::new(hash(i as u32 * 2), hash(i as u32 * 2 + 1));
            (Vec2::new((i % n) as f32, (i / n) as f32) + 0.15 + 0.7 * j) * (PAT / n) as f32
        }).collect();
        let cells = (0..PAT * PAT).map(|i| {
            let p = Vec2::new((i % PAT) as f32 + 0.5, (i / PAT) as f32 + 0.5);
            let (mut f1, mut f2) = (f32::MAX, f32::MAX);
            for q in &pts {
                let mut d = (p - *q).abs();
                d = d.min(Vec2::splat(PAT as f32) - d);
                let d = d.length();
                if d < f1 {
                    (f1, f2) = (d, f1);
                } else if d < f2 {
                    f2 = d;
                }
            }
            (f2 - f1) / (PAT / n) as f32
        }).collect();
        let img = images.add(Image::new_fill(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 }, TextureDimension::D2,
                                             &[240, 243, 246, 0], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default()));
        let mesh = meshes.add(sheet(&[Vec3::ZERO; 4], 2, 2, lo, size));
        Foam { w, h, lo, d: vec![0.0; w * h], v: vec![Vec2::ZERO; w * h], t: 0.0, cells, img, mesh }
    }

    pub fn step(&mut self, dt: f32, white: &[(Vec3, Vec3, u8)], tops: &[f32], (nx, ny, clo, ch): (usize, usize, Vec2, f32),
                images: &mut Assets<Image>, meshes: &mut Assets<Mesh>) {
        let (w, h, lo) = (self.w, self.h, self.lo);
        self.t += dt;
        let (d0, v0) = (&self.d, &self.v);
        let at = |p: Vec2| -> (f32, Vec2) {
            let p = (p - 0.5).clamp(Vec2::ZERO, Vec2::new(w as f32 - 1.001, h as f32 - 1.001));
            let (x, y, f) = (p.x as usize, p.y as usize, p.fract());
            let k = x + w * y;
            let l = |a: usize, b: usize| (d0[a] + (d0[b] - d0[a]) * f.x, v0[a] + (v0[b] - v0[a]) * f.x);
            let ((a, va), (b, vb)) = (l(k, k + 1), l(k + w, k + w + 1));
            (a + (b - a) * f.y, va + (vb - va) * f.y)
        };
        let (mut d, mut v) = (vec![0.0; w * h], vec![Vec2::ZERO; w * h]);
        let rows = h.div_ceil(THREADS);
        std::thread::scope(|s| {
            for (ch_, (dd, vv)) in d.chunks_mut(rows * w).zip(v.chunks_mut(rows * w)).enumerate() {
                s.spawn(move || {
                    for (j, (dd, vv)) in dd.iter_mut().zip(vv.iter_mut()).enumerate() {
                        let i = ch_ * rows * w + j;
                        let p = Vec2::new((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
                        let (a, va) = at(p - v0[i] * dt / TEXEL);
                        *dd = a * (-dt / (LIFE * (1.0 + a.min(1.0)))).exp();
                        *vv = va * (-dt).exp();
                    }
                });
            }
        });
        let mut now = vec![0.0f32; w * h];
        for &(p, pv, kind) in white {
            if kind != 1 {
                continue;
            }
            let c = (p.truncate() - lo) / TEXEL;
            let (cx, cy) = (c.x as isize, c.y as isize);
            for yy in cy - 2..=cy + 2 {
                for xx in cx - 2..=cx + 2 {
                    if xx < 0 || yy < 0 || xx as usize >= w || yy as usize >= h {
                        continue;
                    }
                    let k = xx as usize + w * yy as usize;
                    let r2 = (Vec2::new(xx as f32 + 0.5, yy as f32 + 0.5) - c).length_squared();
                    let a = SPLAT * (-r2 / 2.0).exp();
                    now[k] += a;
                    v[k] = v[k].lerp(pv.truncate(), (a * 2.0).min(1.0));
                }
            }
        }
        for (d, n) in d.iter_mut().zip(now) {
            *d = d.max(n);
        }
        (self.d, self.v) = (d, v);
        let (t1, t2) = ((self.t / PHASE).fract(), (self.t / PHASE + 0.5).fract());
        let k1 = 1.0 - (2.0 * t1 - 1.0).abs();
        let (cells, fd, fv) = (&self.cells, &self.d, &self.v);
        let cell = |p: Vec2| -> f32 {
            let q = p / (8.0 * CELL) * PAT as f32 - 0.5;
            let (x, y, f) = (q.x.floor(), q.y.floor(), q.fract_gl());
            let g = |a: f32, b: f32| cells[(a.rem_euclid(PAT as f32) as usize) + PAT * (b.rem_euclid(PAT as f32) as usize)];
            let (a, b) = (g(x, y) + (g(x + 1.0, y) - g(x, y)) * f.x, g(x, y + 1.0) + (g(x + 1.0, y + 1.0) - g(x, y + 1.0)) * f.x);
            a + (b - a) * f.y
        };
        if let Some(mut img) = images.get_mut(&self.img) {
            let px = img.data.as_mut().unwrap();
            std::thread::scope(|s| {
                for (ch_, out) in px.chunks_mut(rows * w * 4).enumerate() {
                    s.spawn(move || {
                        for (j, o) in out.chunks_exact_mut(4).enumerate() {
                            let i = ch_ * rows * w + j;
                            let a = fd[i];
                            if a < 0.02 {
                                o[3] = 0;
                                continue;
                            }
                            let p = lo + Vec2::new((i % w) as f32 + 0.5, (i / w) as f32 + 0.5) * TEXEL;
                            let e = cell(p - fv[i] * t1 * PHASE) * k1 + cell(p - fv[i] * t2 * PHASE + 0.37) * (1.0 - k1);
                            let cover = 0.45 * (1.0 - (-a).exp());
                            o[3] = (((cover - e) / 0.08 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                            let g = (200.0 + 45.0 * a.min(1.0)) as u8;
                            (o[0], o[1], o[2]) = (g, g.saturating_add(2), g.saturating_add(4));
                        }
                    });
                }
            });
        }
        if let Some(mut m) = meshes.get_mut(&self.mesh) {
            let p: Vec<Vec3> = (0..nx * ny).map(|i| (clo + Vec2::new((i % nx) as f32 + 0.5, (i / nx) as f32 + 0.5) * ch).extend(tops[i] + 0.006)).collect();
            *m = sheet(&p, nx, ny, lo, Vec2::new(w as f32, h as f32) * TEXEL);
        }
    }
}

fn sheet(p: &[Vec3], nx: usize, ny: usize, lo: Vec2, size: Vec2) -> Mesh {
    let uv: Vec<[f32; 2]> = p.iter().map(|q| ((q.truncate() - lo) / size).to_array()).collect();
    let idx = (0..ny - 1).flat_map(|y| (0..nx - 1).flat_map(move |x| {
        let k = (x + nx * y) as u32;
        let n = nx as u32;
        [k, k + 1, k + n + 1, k, k + n + 1, k + n]
    })).collect();
    let len = p.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p.to_vec())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; len])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0f32, 0.0, 0.0, 1.0]; len])
        .with_inserted_indices(Indices::U32(idx))
}

fn hash(x: u32) -> f32 {
    let mut h = x.wrapping_mul(747796405).wrapping_add(2891336453);
    h = ((h >> ((h >> 28) + 4)) ^ h).wrapping_mul(277803737);
    ((h >> 22) ^ h) as f32 / 4294967296.0
}

