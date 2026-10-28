use bevy::asset::RenderAssetUsages;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

const DRY: f32 = 1.0 / 24.0;
const POOLED: f32 = 1.6;
const RUNOFF: f32 = 0.17;
const DRIP_RATE: f32 = 8.0;

pub fn drain(name: &str) -> f32 {
    let n = name.to_lowercase();
    match () {
        _ if ["hand", "foot", "toe"].iter().any(|k| n.contains(k)) => 1.0,
        _ if ["forearm", "lower_arm", "shin", "calf"].iter().any(|k| n.contains(k)) => 0.6,
        _ => 0.2,
    }
}

pub const ROUGH: f32 = 0.6;

#[derive(Resource, Default)]
pub struct Wet {
    pub homer: Vec<Option<Skin>>,
    pub hero: Vec<Option<Skin>>,
    pub drips: Vec<Vec3>,
}

impl Wet {
    pub fn skin<'a>(skins: &'a mut Vec<Option<Skin>>, k: usize, shape: impl FnOnce() -> Mesh, extent: f32, res: usize) -> &'a mut Skin {
        if skins.len() <= k {
            skins.resize_with(k + 1, || None);
        }
        if skins[k].as_ref().is_none_or(|s| s.res != res || s.extent != extent) {
            skins[k] = Some(Skin::new(&shape(), extent, res, k as u32 + 1));
        }
        skins[k].as_mut().unwrap()
    }
}

pub struct Skin {
    res: usize,
    local: Vec<Option<Vec3>>,
    w: Vec<f32>,
    run: Vec<f32>,
    world: Vec<Vec3>,
    under: Vec<bool>,
    extent: f32,
    seed: u32,
    pub mean: f32,
    maps: Option<(Handle<Image>, Handle<Image>)>,
    last: (Vec<u8>, [u8; 4], bool, bool),
}

impl Skin {
    pub fn new(shape: &Mesh, extent: f32, res: usize, seed: u32) -> Skin {
        let pos = match shape.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => vec![],
        };
        let uv = match shape.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(v)) => v.clone(),
            _ => vec![],
        };
        let ix: Vec<usize> = shape.indices().map(|i| i.iter().collect()).unwrap_or_default();
        let mut local = vec![None; res * res];
        for t in ix.chunks_exact(3) {
            let (Some(a), Some(b), Some(c)) = (uv.get(t[0]), uv.get(t[1]), uv.get(t[2])) else { continue };
            let (a, b, c) = (Vec2::from(*a) * res as f32, Vec2::from(*b) * res as f32, Vec2::from(*c) * res as f32);
            let area = (b - a).perp_dot(c - a);
            if area.abs() < 1e-9 {
                continue;
            }
            let (lo, hi) = (a.min(b).min(c), a.max(b).max(c));
            for y in (lo.y.floor().max(0.0) as usize)..=(hi.y.ceil().max(0.0) as usize).min(res - 1) {
                for x in (lo.x.floor().max(0.0) as usize)..=(hi.x.ceil().max(0.0) as usize).min(res - 1) {
                    let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let (u, v) = ((p - a).perp_dot(c - a) / area, (b - a).perp_dot(p - a) / area);
                    if u < -0.02 || v < -0.02 || u + v > 1.02 {
                        continue;
                    }
                    let at = |i: usize| Vec3::from(pos[t[i]]);
                    local[y * res + x] = Some(at(0) + (at(1) - at(0)) * u + (at(2) - at(0)) * v);
                }
            }
        }
        let n = res * res;
        Skin { res, local, w: vec![0.0; n], run: vec![0.0; n], world: vec![Vec3::ZERO; n], under: vec![false; n], extent: extent.max(0.05), seed: seed.wrapping_mul(2654435761) | 1,
               mean: 0.0, maps: None, last: (vec![], [0; 4], false, false) }
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn luck(&self, i: usize) -> f32 {
        ((i as f32 * 12.9898 + self.res as f32).sin() * 43758.545).rem_euclid(1.0)
    }

    pub fn step(&mut self, at: &Transform, dt: f32, rich: bool, drain: f32, in_water: &dyn Fn(Vec3) -> bool, drips: &mut Vec<Vec3>) {
        let res = self.res;
        for i in 0..res * res {
            let Some(l) = self.local[i] else { continue };
            let p = at.transform_point(l);
            self.world[i] = p;
            self.under[i] = in_water(p);
            if self.under[i] {
                self.w[i] = self.w[i].max((self.w[i] + dt * 6.0).min(1.0));
                self.run[i] = 1.0;
                continue;
            }
            self.run[i] = (self.run[i] - dt * RUNOFF).max(0.0);
            let rate = if rich {
                let (hn, end) = (((p.z - at.translation.z) / self.extent).clamp(-1.0, 1.0), (l.length() / self.extent).min(1.0));
                DRY * (1.0 + 0.4 * hn + 0.4 * end) * (0.7 + 0.6 * self.luck(i))
            } else {
                DRY
            };
            self.w[i] = (self.w[i] - rate * dt).max(0.0);
        }
        let low = (0..res * res).filter(|&i| self.local[i].is_some() && !self.under[i] && self.run[i] > 0.02).min_by(|&a, &b| self.world[a].z.total_cmp(&self.world[b].z));
        if let Some(i) = low.filter(|_| drips.len() < 48) && self.rand() < dt * DRIP_RATE * drain * self.run[i] * self.run[i] {
            drips.push(self.world[i]);
        }
        if rich {
            for i in 0..res * res {
                if self.local[i].is_none() || self.under[i] || self.w[i] < 0.05 {
                    continue;
                }
                let (x, y) = ((i % res) as isize, (i / res) as isize);
                let mut low = None;
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = ((x + dx).rem_euclid(res as isize), y + dy);
                    if !(0..res as isize).contains(&ny) {
                        continue;
                    }
                    let j = ny as usize * res + nx as usize;
                    if self.local[j].is_some() && self.world[j].z + 0.002 < self.world[low.unwrap_or(i)].z {
                        low = Some(j);
                    }
                }
                match low {
                    Some(j) if self.w[i] > 0.6 => {
                        let m = ((self.w[i] - 0.5) * dt * 0.8).min(POOLED - self.w[j]).max(0.0);
                        self.w[i] -= m;
                        self.w[j] += m;
                    }
                    _ => {}
                }
            }
        }
        let (sum, n) = self.w.iter().zip(&self.local).filter(|(_, l)| l.is_some()).fold((0.0, 0), |(s, n), (w, _)| (s + w.min(1.0), n + 1));
        self.mean = sum / n.max(1) as f32;
    }

    pub fn maps(&mut self, images: &mut Assets<Image>, col: [u8; 4], rich: bool, nearest: bool) -> Option<(Handle<Image>, Handle<Image>, f32)> {
        if self.mean < 0.003 {
            return None;
        }
        let q: Vec<u8> = self.w.iter().map(|w| (w.min(POOLED) * 12.0).round() as u8).collect();
        let key = (q, col, rich, nearest);
        if self.maps.is_none() || self.last != key {
            let (mut base, mut mr) = (vec![], vec![]);
            let lum = (col[0] as f32 + col[1] as f32 + col[2] as f32) / 3.0;
            for i in 0..self.res * self.res {
                let w = self.w[i];
                let wv = w.min(1.0);
                let bead = rich && wv > 0.5 && self.luck(i + 7) > 0.9;
                let sat = if rich { 1.0 + 0.5 * wv } else { 1.0 };
                let dark = (if rich { 1.0 - 0.4 * wv } else { 1.0 - 0.3 * wv } - 0.2 * (w - 1.0).max(0.0)) * if bead { 1.35 } else { 1.0 };
                let px: [u8; 3] = [0, 1, 2].map(|c| ((lum + (col[c] as f32 - lum) * sat) * dark).clamp(0.0, 255.0) as u8);
                base.extend([px[0], px[1], px[2], 255]);
                let rough = if bead { 0.02 } else if w > 1.0 { 0.05 } else { ROUGH + (if rich { 0.14 } else { 0.25 } - ROUGH) * wv };
                mr.extend([255, (rough * 255.0) as u8, 0, 255]);
            }
            let image = |data, format| {
                let mut i = Image::new(Extent3d { width: self.res as u32, height: self.res as u32, depth_or_array_layers: 1 }, TextureDimension::D2, data, format,
                                       RenderAssetUsages::default());
                i.sampler = if nearest { bevy::image::ImageSampler::nearest() } else { bevy::image::ImageSampler::linear() };
                i
            };
            let (b, m) = (image(base, TextureFormat::Rgba8UnormSrgb), image(mr, TextureFormat::Rgba8Unorm));
            match &self.maps {
                Some((hb, hm)) => {
                    images.get_mut(hb).map(|mut i| *i = b);
                    images.get_mut(hm).map(|mut i| *i = m);
                }
                None => self.maps = Some((images.add(b), images.add(m))),
            }
            self.last = key;
        }
        let (b, m) = self.maps.as_ref()?;
        Some((b.clone(), m.clone(), if rich { (self.mean * 1.5).min(1.0) * 0.9 } else { 0.0 }))
    }
}

