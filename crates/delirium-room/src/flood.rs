use crate::world::{over_pool, POOL, WATER};
use crate::{Clear, Sim, Water};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

const CELL: f32 = 0.125;
const LO: Vec2 = Vec2::new(-20.0, -12.5);
const W: usize = 320;
const H: usize = 400;
const POUR: f32 = 2.0;
const DRY: f32 = 0.002;
const STEPS: usize = 2;
const KEEP: f32 = 0.995;

pub struct Flood {
    ground: Vec<f32>,
    depth: Vec<f32>,
    out: Vec<[f32; 4]>,
    pool: Vec<bool>,
    pub level: f32,
    pub pouring: bool,
}

impl Flood {
    pub fn new(world: &delirium::collision::Collision) -> Flood {
        let at = |i: usize| LO + (Vec2::new((i % W) as f32, (i / W) as f32) + 0.5) * CELL;
        let pool: Vec<bool> = (0..W * H).map(|i| over_pool(at(i).extend(0.0))).collect();
        let ground = (0..W * H).map(|i| world.ground(at(i).extend(4.0), 8.0).unwrap_or(-50.0)).collect();
        let mut f = Flood { ground, depth: vec![0.0; W * H], out: vec![[0.0; 4]; W * H], pool, level: WATER, pouring: false };
        f.fill_pool();
        f
    }

    fn fill_pool(&mut self) {
        for i in 0..W * H {
            if self.pool[i] {
                self.depth[i] = (self.level - self.ground[i]).max(0.0);
            }
        }
    }

    pub fn depth_at(&self, p: Vec3) -> f32 {
        let c = ((p.truncate() - LO) / CELL).floor();
        if c.x < 0.0 || c.y < 0.0 || c.x >= W as f32 || c.y >= H as f32 {
            return 0.0;
        }
        let i = c.y as usize * W + c.x as usize;
        if self.pool[i] { 0.0 } else { self.depth[i] }
    }

    pub fn step(&mut self, dt: f32) {
        let area = POOL_AREA;
        if self.pouring {
            self.level += POUR * dt / area;
        }
        let h = dt / STEPS as f32;
        for _ in 0..STEPS {
            self.fill_pool();
            let before: f32 = (0..W * H).filter(|&i| self.pool[i]).map(|i| self.depth[i]).sum();
            for y in 0..H {
                for x in 0..W {
                    let i = y * W + x;
                    let top = self.ground[i] + self.depth[i];
                    let mut f = [0.0; 4];
                    for (k, n) in [(x > 0).then(|| i - 1), (x + 1 < W).then(|| i + 1), (y > 0).then(|| i - W), (y + 1 < H).then(|| i + W)].into_iter().enumerate() {
                        if let Some(j) = n {
                            f[k] = (self.out[i][k] * KEEP + h * CELL * 9.81 * (top - self.ground[j] - self.depth[j])).max(0.0);
                        }
                    }
                    let all = (f[0] + f[1] + f[2] + f[3]) * h;
                    let have = self.depth[i] * CELL * CELL;
                    if all > have {
                        let s = if all > 0.0 { have / all } else { 0.0 };
                        f.iter_mut().for_each(|x| *x *= s);
                    }
                    self.out[i] = f;
                }
            }
            for y in 0..H {
                for x in 0..W {
                    let i = y * W + x;
                    let mut net = -self.out[i].iter().sum::<f32>();
                    if x > 0 { net += self.out[i - 1][1]; }
                    if x + 1 < W { net += self.out[i + 1][0]; }
                    if y > 0 { net += self.out[i - W][3]; }
                    if y + 1 < H { net += self.out[i + W][2]; }
                    self.depth[i] = (self.depth[i] + net * h / (CELL * CELL)).max(0.0);
                    if self.ground[i] < -40.0 {
                        self.depth[i] = 0.0;
                    }
                }
            }
            let after: f32 = (0..W * H).filter(|&i| self.pool[i]).map(|i| self.depth[i]).sum();
            self.level += (after - before) * CELL * CELL / area;
        }
        self.fill_pool();
    }

    fn mesh(&self, m: &mut Mesh) {
        let (mut p, mut i) = (vec![], vec![]);
        for c in 0..W * H {
            if self.pool[c] || self.depth[c] < DRY {
                continue;
            }
            let a = LO + Vec2::new((c % W) as f32, (c / W) as f32) * CELL;
            let z = self.ground[c] + self.depth[c];
            let k = p.len() as u32;
            p.extend([a.extend(z), (a + Vec2::X * CELL).extend(z), (a + Vec2::splat(CELL)).extend(z), (a + Vec2::Y * CELL).extend(z)]);
            i.extend([k, k + 1, k + 2, k, k + 2, k + 3]);
        }
        if p.is_empty() {
            p = vec![Vec3::ZERO; 3];
            i = vec![0, 1, 2];
        }
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; p.len()]);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; p.len()]);
        m.insert_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0f32, 0.0, 0.0, 1.0]; p.len()]);
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
        m.insert_indices(Indices::U32(i));
    }
}

const POOL_AREA: f32 = (POOL[1].x - POOL[0].x) * (POOL[1].y - POOL[0].y);

pub fn view(mut commands: Commands, mut sim: NonSendMut<Sim>, time: Res<Time>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
            mut water: Query<&mut Transform, With<Water>>, mut spill: Local<Option<(Entity, Handle<Mesh>)>>) {
    if !sim.room.pool {
        sim.flood = None;
    }
    if let Some(f) = sim.flood.as_mut() {
        f.step(time.delta_secs().min(1.0 / 30.0));
    }
    let level = sim.flood.as_ref().map_or(WATER, |f| f.level);
    for mut t in &mut water {
        t.translation.z = level;
    }
    let Some(f) = sim.flood.as_ref() else {
        if let Some((e, _)) = spill.take() {
            commands.entity(e).try_despawn();
        }
        return;
    };
    let (_, mesh) = spill.get_or_insert_with(|| {
        let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
        let m = mats.add(StandardMaterial { base_color: Color::srgba(0.15, 0.4, 0.6, 0.55), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.05, reflectance: 0.5,
                                            specular_transmission: 1.0, ior: 1.33, ..default() });
        (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), Transform::default(), RenderLayers::layer(1), NotShadowCaster, Clear)).id(), mesh)
    });
    if let Some(mut m) = meshes.get_mut(mesh) {
        f.mesh(&mut m);
    }
}

#[test]
fn spills_and_keeps_its_water() {
    let mut f = Flood { ground: vec![0.0; W * H], depth: vec![0.0; W * H], out: vec![[0.0; 4]; W * H], pool: vec![false; W * H], level: WATER, pouring: true };
    for i in 0..W * H {
        let p = LO + (Vec2::new((i % W) as f32, (i / W) as f32) + 0.5) * CELL;
        f.pool[i] = over_pool(p.extend(0.0));
        if f.pool[i] {
            f.ground[i] = -1.5;
        }
    }
    f.fill_pool();
    for _ in 0..60 * 12 {
        f.step(1.0 / 60.0);
    }
    let spilt: f32 = (0..W * H).filter(|&i| !f.pool[i]).map(|i| f.depth[i]).sum::<f32>() * CELL * CELL;
    let poured = POUR * 12.0;
    let held = (f.level - WATER) * POOL_AREA + spilt;
    assert!(spilt > 1.0, "spilt {spilt}");
    assert!(f.level > 0.0 && f.level < 0.2, "level {}", f.level);
    assert!((held - poured).abs() < 0.05 * poured, "held {held} of {poured}");
}
