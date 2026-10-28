use crate::fluid::{Fluid, Physics};
use crate::world::{over_pool, POOL, POOL_DEEP, WATER};
use crate::{OursCam, PartMesh, Sim, SkyWater, Water};
use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::Aabb;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::platform::collections::HashMap;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

const CAUSTIC_SHADER: Handle<Shader> = bevy::asset::uuid_handle!("8b2f4c61-93d7-4e0a-b5c8-2a6e1f7d9c34");

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct SpillCausticExt {
    #[texture(100)]
    #[sampler(101)]
    net: Handle<Image>,
}
impl MaterialExtension for SpillCausticExt {
    fn fragment_shader() -> ShaderRef {
        CAUSTIC_SHADER.into()
    }
}
type SpillCaustic = ExtendedMaterial<StandardMaterial, SpillCausticExt>;

const FILM_SHADER: Handle<Shader> = bevy::asset::uuid_handle!("3c7e9a12-5b4d-4f86-a1e2-9d0c6b8f4a57");

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct SpillFilmExt {
    #[texture(100)]
    #[sampler(101)]
    ripples: Handle<Image>,
    #[uniform(102)]
    k: Vec4,
}
impl MaterialExtension for SpillFilmExt {
    fn fragment_shader() -> ShaderRef {
        FILM_SHADER.into()
    }
}
type SpillFilm = ExtendedMaterial<StandardMaterial, SpillFilmExt>;

pub fn plugin(app: &mut App) {
    let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
    let _ = shaders.insert(&CAUSTIC_SHADER, Shader::from_wesl(include_str!("spill_caustic.wesl"), "spill_caustic.wesl"));
    let _ = shaders.insert(&FILM_SHADER, Shader::from_wesl(include_str!("spill_film.wesl"), "spill_film.wesl"));
    app.add_plugins((MaterialPlugin::<SpillCaustic>::default(), MaterialPlugin::<SpillFilm>::default()));
}

const KERNELS: [&str; 22] = ["clear", "p2g", "cells", "tops", "faces", "cg_init", "fin_rz", "matvec", "fin_alpha", "update", "fin_beta", "p_update",
                             "project", "extrapolate", "extrapolate_back", "curl", "vort", "g2p", "ww_emit", "ww_step", "forces", "pour"];
const WW: usize = 32768;
const WG: u32 = 256;
const PF: usize = 20;

pub struct GpuFluid {
    pipes: Vec<wgpu::ComputePipeline>,
    bind: wgpu::BindGroup,
    params: wgpu::Buffer,
    parts: wgpu::Buffer,
    read: wgpu::Buffer,
    cells: wgpu::Buffer,
    solids: wgpu::Buffer,
    tops: wgpu::Buffer,
    ww: wgpu::Buffer,
    flow: wgpu::Buffer,
    nsolid: u32,
    ground: wgpu::Buffer,
    seed: AtomicU32,
    n: [usize; 3],
    h: f32,
    lo: Vec3,
    np: u32,
    ncell: u32,
    nface: u32,
    nred: u32,
    pub iters: u32,
    pub pour: f32,
    pub spout: Vec4,
    pub pour_v: f32,
    pub drain: bool,
    pub counts: bool,
    pub portals: [Vec4; 4],
    pub phys: Physics,
    vmax: AtomicU32,
}

fn storage(device: &wgpu::Device, label: &str, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: bytes.max(16), usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

impl GpuFluid {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, f: &Fluid, extra: usize) -> GpuFluid {
        let n = f.n;
        let ncell = n[0] * n[1] * n[2];
        let faces = |a: usize| { let mut d = n; d[a] += 1; d[0] * d[1] * d[2] };
        let nface = faces(0) + faces(1) + faces(2);
        let nred = ncell.div_ceil(WG as usize);
        let np = f.p.len() + extra;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("fluid"), source: wgpu::ShaderSource::Wgsl(include_str!("fluid.wesl").into()) });
        let params = device.create_buffer(&wgpu::BufferDescriptor { label: Some("fluid params"), size: 176, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let parts = storage(device, "fluid particles", (np * PF * 4) as u64);
        let p: Vec<f32> = f.p.iter().flat_map(|p| {
            let c = p.grad();
            [p.x.x, p.x.y, p.x.z, 0.0, p.v.x, p.v.y, p.v.z, 0.0, c[0].x, c[0].y, c[0].z, 0.0, c[1].x, c[1].y, c[1].z, 0.0, c[2].x, c[2].y, c[2].z, 0.0]
        }).chain((0..extra).flat_map(|_| { let mut d = [0.0; PF]; d[3] = 1.0; d })).collect();
        queue.write_buffer(&parts, 0, &floats(&p));
        let bufs = [
            storage(device, "fluid acc", (2 * nface * 4) as u64),
            storage(device, "fluid vel", (2 * nface * 4) as u64),
            storage(device, "fluid known", (2 * nface * 4) as u64),
            storage(device, "fluid cells", ((2 * ncell + 4 * n[0] * n[1]) * 4) as u64),
            storage(device, "fluid cg", (4 * ncell * 4) as u64),
            storage(device, "fluid red", ((nred + 4) * 4) as u64),
        ];
        let entry = |b: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding: b, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }, count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fluid"),
            entries: &std::iter::once(entry(0, wgpu::BufferBindingType::Uniform)).chain((1..13).map(|b| entry(b, wgpu::BufferBindingType::Storage { read_only: b == 8 || b == 12 }))).collect::<Vec<_>>(),
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("fluid"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipes = KERNELS.map(|k| device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(k), layout: Some(&pl), module: &shader, entry_point: Some(k), compilation_options: Default::default(), cache: None,
        })).to_vec();
        let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() }, wgpu::BindGroupEntry { binding: 1, resource: parts.as_entire_binding() }];
        entries.extend(bufs.iter().enumerate().map(|(k, b)| wgpu::BindGroupEntry { binding: k as u32 + 2, resource: b.as_entire_binding() }));
        let solids = storage(device, "fluid solids", (MAX_SOLIDS * 48) as u64);
        entries.push(wgpu::BindGroupEntry { binding: 8, resource: solids.as_entire_binding() });
        let ww = storage(device, "fluid whitewater", (WW * 32) as u64);
        let wwn = storage(device, "fluid whitewater next", 16);
        entries.push(wgpu::BindGroupEntry { binding: 9, resource: ww.as_entire_binding() });
        entries.push(wgpu::BindGroupEntry { binding: 10, resource: wwn.as_entire_binding() });
        let flow = storage(device, "fluid flow", (MAX_SOLIDS * 16) as u64);
        entries.push(wgpu::BindGroupEntry { binding: 11, resource: flow.as_entire_binding() });
        let ground = storage(device, "fluid ground", (n[0] * n[1] * 4) as u64);
        queue.write_buffer(&ground, 0, &floats(&vec![-1e9; n[0] * n[1]]));
        entries.push(wgpu::BindGroupEntry { binding: 12, resource: ground.as_entire_binding() });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("fluid"), layout: &layout, entries: &entries });
        let read = device.create_buffer(&wgpu::BufferDescriptor { label: Some("fluid read"), size: ((np * PF * 4) as u64).max(16), usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let tops = device.create_buffer(&wgpu::BufferDescriptor { label: Some("fluid tops"), size: (n[0] * n[1] * 16 + WW * 32 + MAX_SOLIDS * 16 + ncell * 4) as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let [_, _, _, cells, _, _] = bufs;
        GpuFluid { pipes, bind, params, parts, read, cells, solids, tops, ww, flow, nsolid: 0, ground, seed: AtomicU32::new(0), n, h: f.h, lo: f.lo, np: np as u32, ncell: ncell as u32,
                   nface: nface as u32, nred: nred as u32, iters: 60, pour: 0.0, spout: Vec4::ZERO, pour_v: 0.0, drain: false, counts: false, phys: Physics::default(), vmax: AtomicU32::new(0), portals: [Vec4::ZERO; 4] }
    }

    pub fn substeps(&self, dt: f32) -> u32 {
        if !self.phys.spray {
            return 2;
        }
        ((f32::from_bits(self.vmax.load(Ordering::Relaxed)) * dt / (0.8 * self.h)).ceil() as u32).clamp(2, 6)
    }

    pub fn step(&self, device: &wgpu::Device, queue: &wgpu::Queue, dt: f32, k: u32) {
        let n = self.n;
        let faces = |a: usize| { let mut d = n; d[a] += 1; (d[0] * d[1] * d[2]) as u32 };
        let pour = (self.pour * dt / k as f32 * 8.0 / self.h.powi(3)).round() as u32;
        let mut u: Vec<u32> = vec![n[0] as u32, n[1] as u32, n[2] as u32, self.np, self.lo.x.to_bits(), self.lo.y.to_bits(), self.lo.z.to_bits(), self.h.to_bits(),
                            (dt / k as f32).to_bits(), 9.81f32.to_bits(), self.ncell, self.nface, 0, faces(0), faces(0) + faces(1), 0, self.nred, (n[0] * n[1]) as u32, self.nsolid, self.seed.fetch_add(1, Ordering::Relaxed),
                            self.spout.x.to_bits(), self.spout.y.to_bits(), self.spout.z.to_bits(), self.spout.w.to_bits(), self.pour_v.to_bits(), pour, self.drain as u32, self.phys.bits()];
        u.extend(self.portals.iter().flat_map(|p| p.to_array().map(f32::to_bits)));
        queue.write_buffer(&self.params, 0, &u.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>());
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_bind_group(0, &self.bind, &[]);
            let k_of = |name: &str| KERNELS.iter().position(|k| *k == name).unwrap();
            let mut run = |name: &str, threads: u32| {
                pass.set_pipeline(&self.pipes[k_of(name)]);
                pass.dispatch_workgroups(threads.div_ceil(WG).max(1), 1, 1);
            };
            let (c, f, p) = (self.ncell, self.nface, self.np);
            for _ in 0..k {
                run("clear", (2 * f).max(c));
                run("pour", p);
                run("p2g", p);
                run("cells", c);
                run("tops", p);
                run("faces", f);
                if self.phys.vort {
                    run("curl", c);
                    run("vort", f);
                }
                run("cg_init", c);
                run("fin_rz", WG);
                for _ in 0..self.iters {
                    run("matvec", c);
                    run("fin_alpha", WG);
                    run("update", c);
                    run("fin_beta", WG);
                    run("p_update", c);
                }
                run("project", f);
                for _ in 0..2 {
                    run("extrapolate", f);
                    run("extrapolate_back", f);
                }
                run("g2p", p);
                run("ww_emit", p);
                run("ww_step", WW as u32);
            }
            run("forces", self.nsolid);
        }
        queue.submit([enc.finish()]);
    }

    pub fn particles(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<(Vec3, Vec3)> {
        self.request(device, queue, Arc::new(AtomicBool::new(false)));
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        self.take()
    }

    pub fn request(&self, device: &wgpu::Device, queue: &wgpu::Queue, ready: Arc<AtomicBool>) {
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&self.parts, 0, &self.read, 0, (self.np as usize * PF * 4) as u64);
        queue.submit([enc.finish()]);
        self.read.slice(..).map_async(wgpu::MapMode::Read, move |r| ready.store(r.is_ok(), Ordering::Release));
    }

    pub fn set_ground(&self, queue: &wgpu::Queue, ground: &[f32]) {
        queue.write_buffer(&self.ground, 0, &floats(ground));
    }

    pub fn solids(&mut self, queue: &wgpu::Queue, caps: &[Vec4]) {
        let caps = &caps[..caps.len().min((MAX_SOLIDS - 1) * 3)];
        self.nsolid = (caps.len() / 3) as u32;
        if !caps.is_empty() {
            queue.write_buffer(&self.solids, 0, &floats(&caps.iter().flat_map(|c| c.to_array()).collect::<Vec<_>>()));
        }
    }

    pub fn request_tops(&self, device: &wgpu::Device, queue: &wgpu::Queue, ready: Arc<AtomicBool>) {
        let mut enc = device.create_command_encoder(&Default::default());
        let n = (self.n[0] * self.n[1] * 16) as u64;
        enc.copy_buffer_to_buffer(&self.cells, (2 * self.ncell * 4) as u64, &self.tops, 0, n);
        enc.copy_buffer_to_buffer(&self.ww, 0, &self.tops, n, (WW * 32) as u64);
        enc.copy_buffer_to_buffer(&self.flow, 0, &self.tops, n + (WW * 32) as u64, (MAX_SOLIDS * 16) as u64);
        if self.counts {
            enc.copy_buffer_to_buffer(&self.cells, 0, &self.tops, self.tops_len(false), self.ncell as u64 * 4);
        }
        queue.submit([enc.finish()]);
        self.tops.slice(..self.tops_len(self.counts)).map_async(wgpu::MapMode::Read, move |r| ready.store(r.is_ok(), Ordering::Release));
    }

    fn tops_len(&self, counts: bool) -> u64 {
        ((self.n[0] * self.n[1] * 4 + WW * 8 + MAX_SOLIDS * 4) * 4) as u64 + if counts { self.ncell as u64 * 4 } else { 0 }
    }

    pub fn take_tops(&self) -> (Vec<u32>, Vec<(Vec3, Vec3, u8)>, Vec<Vec4>, Vec<u32>, Vec<Vec2>) {
        let out = {
            let v = self.tops.slice(..self.tops_len(self.counts)).get_mapped_range().unwrap();
            let w: Vec<u32> = v.chunks_exact(4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            let nc = self.n[0] * self.n[1];
            let (tops, ww) = w.split_at(nc);
            let (vel, ww) = ww.split_at(3 * nc);
            let vel = vel.chunks_exact(3).map(|c| if c[2] == 0 { Vec2::ZERO } else { Vec2::new(c[0] as i32 as f32, c[1] as i32 as f32) / (1024.0 * c[2] as f32) }).collect();
            let (ww, flow) = ww.split_at(WW * 8);
            let (flow, counts) = flow.split_at(MAX_SOLIDS * 4);
            let flow: Vec<Vec4> = flow.chunks_exact(4).map(|c| Vec4::new(f32::from_bits(c[0]), f32::from_bits(c[1]), f32::from_bits(c[2]), f32::from_bits(c[3]))).collect();
            self.vmax.store(flow[MAX_SOLIDS - 1].x.to_bits(), Ordering::Relaxed);
            let f = |k: usize| f32::from_bits(ww[k]);
            let white = (0..WW).filter(|&i| f(i * 4 + 3) > 0.0).map(|i| {
                let (a, b) = (i * 4, (WW + i) * 4);
                (Vec3::new(f(a), f(a + 1), f(a + 2)), Vec3::new(f(b), f(b + 1), f(b + 2)), f(b + 3) as u8)
            }).collect();
            (tops.to_vec(), white, flow, counts.to_vec(), vel)
        };
        self.tops.unmap();
        out
    }

    pub fn take(&self) -> Vec<(Vec3, Vec3)> {
        let out = {
            let v = self.read.slice(..).get_mapped_range().unwrap();
            let f: Vec<f32> = v.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
            f.chunks_exact(PF).take(self.np as usize).filter(|p| p[3] < 0.5).map(|p| (Vec3::new(p[0], p[1], p[2]), Vec3::new(p[4], p[5], p[6]))).collect()
        };
        self.read.unmap();
        out
    }
}

#[derive(Resource)]
pub struct RealWater {
    g: GpuFluid,
    ready: Arc<AtomicBool>,
    stepped: Arc<AtomicBool>,
    busy: bool,
    tops: Vec<f32>,
    rest: f32,
    was: HashMap<Entity, Vec3>,
    sky: Option<(Entity, Handle<Mesh>, Handle<Mesh>)>,
    basin: Basin,
    white: Vec<(Vec3, Vec3, u8)>,
    foam: (Entity, Handle<Mesh>),
    ripples: [Handle<Image>; 2],
    sent: Vec<Solid>,
    read: Vec<Solid>,
    flood: bool,
    ground: Vec<f32>,
    spill: Option<(Entity, Handle<Mesh>, Handle<StandardMaterial>)>,
    wet: Vec<f32>,
    trail: Option<(Entity, Handle<Mesh>)>,
    film: Option<(Entity, Handle<SpillFilm>, Handle<Mesh>)>,
    pipe: Option<Entity>,
    air: Option<(Entity, Handle<Mesh>, Handle<StandardMaterial>)>,
    under: Option<(Entity, Handle<Mesh>)>,
    caustic: Handle<Image>,
    vel: Vec<Vec2>,
    bubbles: (Entity, Handle<Mesh>),
    sheet: (Entity, crate::foam::Foam),
    sheet_t: f32,
    mist: (Entity, Handle<Mesh>, Vec<(Vec3, Vec3, f32)>, f32),
    ww_mats: [Handle<StandardMaterial>; 3],
    ww_rt: Option<bool>,
}

#[derive(Clone, Copy)]
enum Solid {
    Part(usize),
    Obj(usize),
    Hero(usize),
    Other,
}

const RHO: f32 = 1000.0;
const DRAG: f32 = 1.0;

pub fn push(flow: Vec4, (a, b, r): (Vec3, Vec3, f32), vol: f32, m: f32, v: Vec3, dt: f32) -> Vec3 {
    let under = (0..4).map(|i| {
        let d = ((flow.w - a.lerp(b, (i as f32 + 0.5) / 4.0).z + r) / (2.0 * r)).clamp(0.0, 1.0);
        d * d * (3.0 - 2.0 * d)
    }).sum::<f32>() / 4.0;
    if under <= 0.0 {
        return Vec3::ZERO;
    }
    let dv = flow.truncate() - v;
    let area = 2.0 * r * (a.distance(b) + 2.0 * r);
    let drag = (dv * (0.5 * RHO * DRAG * area * dv.length() + m) * under * dt).clamp_length_max(m * dv.length() * under);
    Vec3::Z * (RHO * 9.81 * vol * under * dt) + drag
}

#[derive(Clone, Copy)]
struct Basin {
    lo: Vec3,
    size: Vec3,
    cell: f32,
    head: f32,
}

const MAX_SOLIDS: usize = 1024;
const SPILL: f32 = 3.0;
const SPILL_CELL: f32 = 0.125;
const POUR: f32 = 2.0;
const SPOUT: Vec4 = Vec4::new(0.0, 6.6, 0.9, 0.4);
const POUR_V: f32 = 4.0;
pub(crate) const PIPE_R: f32 = 0.44;
const PIPE_ARC: usize = 48;
const PIPE_SIDES: u32 = 64;
const PIPE_BEND: f32 = 1.0;
const POUR_MORE: usize = 300_000;

pub(crate) fn pipe_path() -> Vec<(Vec3, Vec3)> {
    let top = SPOUT.z + 0.1;
    let c = Vec3::new(SPOUT.x, SPOUT.y - PIPE_BEND, top);
    let mut v = vec![(Vec3::new(SPOUT.x, SPOUT.y - 2.0 * PIPE_BEND, -0.05), Vec3::Z)];
    v.extend((0..=PIPE_ARC).map(|i| {
        let a = std::f32::consts::PI * i as f32 / PIPE_ARC as f32;
        (c + PIPE_BEND * Vec3::new(0.0, -a.cos(), a.sin()), Vec3::new(0.0, a.sin(), a.cos()))
    }));
    v
}

pub(crate) fn faucet_caps() -> [(Vec3, Vec3); 4] {
    let p = pipe_path();
    let at = |k: usize| p[1 + PIPE_ARC * k / 3].0;
    [(p[0].0, p[1].0), (at(0), at(1)), (at(1), at(2)), (at(2), at(3))]
}

fn pipe_mesh() -> Mesh {
    const SIDES: u32 = PIPE_SIDES;
    const ROUND: f32 = 3.0;
    let tile = std::f32::consts::TAU * PIPE_R / ROUND;
    let (mut pos, mut nor, mut uv, mut idx) = (vec![], vec![], vec![], vec![]);
    let mut along = vec![0.0; SIDES as usize + 1];
    for (i, (c, t)) in pipe_path().into_iter().enumerate() {
        let b = t.cross(Vec3::X);
        for k in 0..=SIDES {
            let a = std::f32::consts::TAU * k as f32 / SIDES as f32;
            let n = Vec3::X * a.cos() + b * a.sin();
            let p = c + n * PIPE_R;
            if i > 0 {
                along[k as usize] += p.distance(pos[pos.len() - (SIDES as usize + 1)]);
            }
            pos.push(p);
            nor.push(n);
            uv.push([along[k as usize] / tile, ROUND * k as f32 / SIDES as f32]);
        }
        if i > 0 {
            let (p, q) = ((i as u32 - 1) * (SIDES + 1), i as u32 * (SIDES + 1));
            for k in 0..SIDES {
                idx.extend([p + k, p + k + 1, q + k + 1, p + k, q + k + 1, q + k]);
            }
        }
    }
    let m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx));
    m.with_generated_tangents().expect("pipe tangents")
}

#[derive(Component)]
pub struct Faucet;

pub(crate) fn faucet(meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, assets: &AssetServer) -> (Mesh3d, MeshMaterial3d<StandardMaterial>, Faucet) {
    let [c, n, mr] = ["steel", "steel_n", "steel_mr"].map(|t| crate::pbr_tex(assets, t.into(), false));
    (Mesh3d(meshes.add(pipe_mesh())),
     MeshMaterial3d(mats.add(StandardMaterial { base_color_texture: Some(c), normal_map_texture: Some(n), metallic_roughness_texture: Some(mr),
                                                metallic: 1.0, perceptual_roughness: 0.75, cull_mode: None, double_sided: true, ..default() })),
     Faucet)
}

pub fn faucet_shown(mut sim: NonSendMut<Sim>, mut q: Query<&mut Visibility, With<Faucet>>) {
    let on = sim.water_faucet;
    for mut v in &mut q {
        v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
    }
    let room = &mut sim.room;
    for &h in &room.faucet {
        if let Some(b) = room.w[0].bodies.get_mut(h) {
            b.set_enabled(on);
        }
    }
}

impl RealWater {
    pub fn bulk(&self, p: Vec2) -> f32 {
        let g = &self.g;
        let p = if self.room() { p.clamp((*POOL())[0] + g.h * 0.5, (*POOL())[1] - g.h * 0.5) } else { p };
        let (nx, ny) = (g.n[0], g.n[1]);
        let c = ((p - g.lo.truncate()) / g.h - 0.5).clamp(Vec2::ZERO, Vec2::new(nx as f32 - 1.001, ny as f32 - 1.001));
        let (x, y, f) = (c.x as usize, c.y as usize, c.fract());
        let at = |x: usize, y: usize| self.tops[x + nx * y];
        let a = at(x, y) + (at(x + 1, y) - at(x, y)) * f.x;
        let b = at(x, y + 1) + (at(x + 1, y + 1) - at(x, y + 1)) * f.x;
        a + (b - a) * f.y - self.rest
    }

    pub fn room(&self) -> bool {
        self.sky.is_none()
    }
}

const MIST_LIFE: f32 = 2.0;
const MIST_MAX: usize = 3000;

fn mist(puffs: &[(Vec3, Vec3, f32)], eye: Vec3, stub: Vec3) -> Mesh {
    let (mut p, mut c, mut n) = (vec![], vec![], vec![]);
    for (k, &(at, _, age)) in puffs.iter().enumerate() {
        let s = age / MIST_LIFE;
        let f = (eye - at).normalize_or(Vec3::Z);
        let u = Quat::from_axis_angle(f, k as f32 * 2.4) * f.any_orthonormal_vector();
        let r = 0.02 + 0.18 * s.sqrt();
        let a = 0.14 * (age * 10.0).min(1.0) * (1.0 - s) * (1.0 - s);
        let (u, w) = (u * r, f.cross(u) * r);
        p.extend([at - u - w, at + u - w, at + u + w, at - u + w]);
        c.extend([[1.0, 1.0, 1.0, a]; 4]);
        n.extend([f; 4]);
    }
    if p.is_empty() {
        p = vec![stub, stub + Vec3::X * 0.001, stub + Vec3::Y * 0.001, stub];
        c = vec![[1.0, 1.0, 1.0, 0.0]; 4];
        n = vec![Vec3::Z; 4];
    }
    let len = p.len();
    let q = len as u32 / 4;
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, n)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, c)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]].repeat(len / 4))
        .with_inserted_indices(Indices::U32((0..q).flat_map(|k| [0, 1, 2, 0, 2, 3].map(|i| k * 4 + i)).collect()))
}

fn whitewater(white: &[(Vec3, Vec3, u8)], eye: Vec3, stub: Vec3, keep: impl Fn(u8) -> bool) -> Mesh {
    let (mut p, mut n) = (vec![], vec![]);
    for (k, &(at, v, kind)) in white.iter().enumerate() {
        if !keep(kind) {
            continue;
        }
        let f = (eye - at).normalize_or(Vec3::Z);
        let size = (k as f32 * 0.618).fract();
        let (u, w, f) = match kind {
            0 => {
                let along = (v - f * v.dot(f)).normalize_or(Vec3::Z);
                let r = 0.002 + 0.003 * size;
                (f.cross(along).normalize_or(Vec3::X) * r, along * (r + v.length() / 120.0), f)
            }
            1 => {
                let d = Vec2::from_angle(k as f32 * 2.4) * (0.02 + 0.03 * size);
                (d.extend(0.0), d.perp().extend(0.0), Vec3::Z)
            }
            _ => {
                let u = f.any_orthonormal_vector() * (0.002 + 0.003 * size);
                (u, f.cross(u), f)
            }
        };
        p.extend([at - u - w, at + u - w, at + u + w, at - u + w]);
        n.extend([f; 4]);
    }
    if p.is_empty() {
        p = vec![stub, stub + Vec3::X * 0.001, stub + Vec3::Y * 0.001, stub];
        n = vec![Vec3::Z; 4];
    }
    let q = p.len() as u32 / 4;
    let len = p.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, n)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]].repeat(len / 4))
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0f32, 0.0, 0.0, 1.0]; len])
        .with_inserted_indices(Indices::U32((0..q).flat_map(|k| [0, 1, 2, 0, 2, 3].map(|i| k * 4 + i)).collect()))
}

fn breeze(p: Vec2, t: f32) -> f32 {
    (0..12).map(|i| {
        let k = std::f32::consts::TAU / (0.35 * (1.6f32 / 0.35).powf(i as f32 / 11.0));
        0.09 / (k * k) * (k * p.dot(Vec2::from_angle(i as f32 * 2.39996 + 0.3)) - (9.81 * k).sqrt() * t).sin()
    }).sum()
}

fn breeze_slope(p: Vec2, t: f32) -> Vec2 {
    (0..12).map(|i| {
        let k = std::f32::consts::TAU / (0.35 * (1.6f32 / 0.35).powf(i as f32 / 11.0));
        let dir = Vec2::from_angle(i as f32 * 2.39996 + 0.3);
        dir * (0.09 / k * (k * p.dot(dir) - (9.81 * k).sqrt() * t).cos())
    }).sum()
}

const SKY_RAY: f32 = 0.02;
const SKY_TEXEL: f32 = 0.04;
const SKY_EDGE: f32 = 1.5;
const SKY_EVERY: f32 = 1.0 / 30.0;
const SKY_SUN: f32 = 0.53;
const SKY_FILL: f32 = 0.35;

#[derive(Component)]
pub struct SkyCaustic;

pub fn sky_caustics(mut commands: Commands, sim: NonSend<Sim>, real: Option<Res<RealWater>>, time: Res<Time>, mut images: ResMut<Assets<Image>>,
                    mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
                    mut made: Local<Option<(Entity, Handle<Image>, Handle<Mesh>)>>, mut last: Local<f32>) {
    let on = sim.room.stage == "pttest" && sim.rt_now() == 0 && sim.water_caustics && sim.real_ripples
        && real.as_ref().is_some_and(|r| r.sky.is_some());
    if let Some((e, ..)) = &*made {
        commands.entity(*e).try_insert(if on { Visibility::Inherited } else { Visibility::Hidden });
    }
    let now = time.elapsed_secs();
    if !on || now - *last < SKY_EVERY {
        return;
    }
    *last = now;
    let r = real.unwrap();
    let b = r.basin;
    let (az, el) = (sim.sun_dir[0].to_radians(), sim.sun_dir[1].clamp(3.0, 89.0).to_radians());
    let d = -Vec3::new(az.cos() * el.cos(), az.sin() * el.cos(), el.sin());
    let (deep, high) = (b.size.z, b.lo.z);
    let land = |p: Vec2, g: Vec2| {
        let n = Vec3::new(-g.x, -g.y, 1.0).normalize();
        let t1 = crate::water::refract(d, n, 1.0 / 1.33).unwrap_or(d);
        let q = p + t1.truncate() * deep / -t1.z;
        let t2 = crate::water::refract(t1, Vec3::Z, 1.33).unwrap_or(d);
        q + t2.truncate() * high / (-t2.z).max(0.05)
    };
    let shift = land(Vec2::ZERO, Vec2::ZERO);
    let lo = b.lo.truncate() + shift - Vec2::splat(SKY_EDGE);
    let size = b.size.truncate() + Vec2::splat(2.0 * SKY_EDGE);
    let (w, h) = ((size.x / SKY_TEXEL).ceil() as usize, (size.y / SKY_TEXEL).ceil() as usize);
    let share = (SKY_RAY / SKY_TEXEL).powi(2);
    let (nx, ny) = ((b.size.x / SKY_RAY) as usize, (b.size.y / SKY_RAY) as usize);
    let threads = 12;
    let rows = ny.div_ceil(threads);
    let r = &*r;
    let sums: Vec<Vec<f32>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads).map(|k| s.spawn(move || {
            let mut sum = vec![0.0f32; w * h];
            for y in k * rows..((k + 1) * rows).min(ny) {
                for x in 0..nx {
                    let p = b.lo.truncate() + (Vec2::new(x as f32, y as f32) + 0.5) * SKY_RAY;
                    let e = 0.02;
                    let g = Vec2::new(r.bulk(p + Vec2::X * e) - r.bulk(p - Vec2::X * e), r.bulk(p + Vec2::Y * e) - r.bulk(p - Vec2::Y * e)) / (2.0 * e)
                        + breeze_slope(p, now);
                    let c = (land(p, g) - lo) / SKY_TEXEL - 0.5;
                    if c.x < 0.0 || c.y < 0.0 || c.x >= (w - 1) as f32 || c.y >= (h - 1) as f32 {
                        continue;
                    }
                    let (i, j, f) = (c.x as usize, c.y as usize, c.fract());
                    sum[i + w * j] += share * (1.0 - f.x) * (1.0 - f.y);
                    sum[i + 1 + w * j] += share * f.x * (1.0 - f.y);
                    sum[i + w * (j + 1)] += share * (1.0 - f.x) * f.y;
                    sum[i + 1 + w * (j + 1)] += share * f.x * f.y;
                }
            }
            sum
        })).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut v = sums[0].clone();
    for s in &sums[1..] {
        v.iter_mut().zip(s).for_each(|(a, b)| *a += b);
    }
    let rad = ((high * (SKY_SUN * 0.5).to_radians().tan() / SKY_TEXEL).round() as usize).max(1);
    for step in [1, w, 1, w] {
        let o = v.clone();
        for k in rad * (w + 1)..w * h - rad * (w + 1) {
            v[k] = (0..=2 * rad).map(|j| o[k + j * step - rad * step]).sum::<f32>() / (2 * rad + 1) as f32;
        }
    }
    let (f0, f1) = (b.lo.truncate() + shift, b.lo.truncate() + b.size.truncate() + shift);
    let cover = |a: f32, z0: f32, z1: f32| ((a + SKY_TEXEL).min(z1) - a.max(z0)).max(0.0) / SKY_TEXEL;
    let mut data = Vec::with_capacity(w * h * 8);
    for j in 0..h {
        for i in 0..w {
            let at = lo + Vec2::new(i as f32, j as f32) * SKY_TEXEL;
            let flat = cover(at.x, f0.x, f1.x) * cover(at.y, f0.y, f1.y);
            let k = crate::water::f16((1.0 - flat * (1.0 - SKY_FILL) + v[i + w * j] * (1.0 - SKY_FILL)).max(0.0));
            for c in [k, k, k, crate::water::f16(1.0)] {
                data.extend(c.to_le_bytes());
            }
        }
    }
    let img = Image::new(Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 }, TextureDimension::D2, data,
                         TextureFormat::Rgba16Float, RenderAssetUsages::default());
    let quad = || Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| (Vec2::new(x, y) * size * 0.5).extend(0.0)).to_vec())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 4])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    let centre = (lo + size * 0.5).extend(0.006);
    match &*made {
        Some((e, i, m)) => {
            if let Some(mut x) = images.get_mut(i) {
                *x = img;
            }
            if let Some(mut x) = meshes.get_mut(m) {
                *x = quad();
            }
            commands.entity(*e).try_insert(Transform::from_translation(centre));
        }
        None => {
            let (i, m) = (images.add(img), meshes.add(quad()));
            let mat = mats.add(StandardMaterial { base_color_texture: Some(i.clone()), unlit: true, alpha_mode: AlphaMode::Multiply,
                                                  double_sided: true, cull_mode: None, ..default() });
            let e = commands.spawn((Mesh3d(m.clone()), MeshMaterial3d(mat), Transform::from_translation(centre),
                                    bevy::camera::visibility::RenderLayers::layer(1), bevy::light::NotShadowCaster, crate::water::Fx, SkyCaustic)).id();
            *made = Some((e, i, m));
        }
    }
}

fn body(r: &RealWater, centre: Vec3, inset: f32, t: f32, fine: bool) -> Mesh {
    let b = r.basin;
    let (lo, hi) = (b.lo + Vec3::new(inset, inset, inset), b.lo + b.size - Vec3::new(inset, inset, 0.0));
    let m = (b.size.truncate() / b.cell * if fine { 2.0 } else { 1.0 }).round().as_uvec2();
    let level = b.lo.z + b.size.z;
    let top = |x: u32, y: u32| {
        let p = lo.truncate() + (hi - lo).truncate() * Vec2::new(x as f32 / m.x as f32, y as f32 / m.y as f32);
        p.extend((level + r.bulk(p) + if fine { breeze(p, t) } else { 0.0 }).clamp(lo.z + 0.01, level + b.head - 0.05))
    };
    let (mut pos, mut nor, mut idx) = (vec![], vec![], vec![]);
    let grid: Vec<Vec3> = (0..=m.y).flat_map(|y| (0..=m.x).map(move |x| (x, y))).map(|(x, y)| top(x, y)).collect();
    let at = |x: u32, y: u32| grid[(x + (m.x + 1) * y) as usize];
    let normal = |x: u32, y: u32| {
        let dx = at((x + 1).min(m.x), y) - at(x.saturating_sub(1), y);
        let dy = at(x, (y + 1).min(m.y)) - at(x, y.saturating_sub(1));
        dx.cross(dy).normalize()
    };
    let step = (hi - lo).truncate() / m.as_vec2();
    let mut tan = vec![];
    for y in 0..=m.y {
        for x in 0..=m.x {
            pos.push(at(x, y));
            nor.push(normal(x, y));
            let (dx, dy) = (normal((x + 1).min(m.x), y) - normal(x.saturating_sub(1), y), normal(x, (y + 1).min(m.y)) - normal(x, y.saturating_sub(1)));
            let (sx, sy) = (step.x * ((x + 1).min(m.x) - x.saturating_sub(1)) as f32, step.y * ((y + 1).min(m.y) - y.saturating_sub(1)) as f32);
            tan.push([dx.x / sx, dx.y / sx, dy.x / sy, dy.y / sy]);
        }
    }
    let k = |x: u32, y: u32| x + (m.x + 1) * y;
    for y in 0..m.y {
        for x in 0..m.x {
            idx.extend([k(x, y), k(x + 1, y), k(x, y + 1), k(x + 1, y), k(x + 1, y + 1), k(x, y + 1)]);
        }
    }
    let mut tri = |pos: &mut Vec<Vec3>, nor: &mut Vec<Vec3>, v: [Vec3; 3], n: [Vec3; 3]| {
        let k = pos.len() as u32;
        let flip = (v[1] - v[0]).cross(v[2] - v[0]).dot(n[0] + n[1] + n[2]) < 0.0;
        pos.extend(v);
        nor.extend(n);
        idx.extend(if flip { [k, k + 2, k + 1] } else { [k, k + 1, k + 2] });
    };
    let sides: [(Vec<(u32, u32)>, Vec3); 4] = [((0..=m.x).map(|x| (x, 0)).collect(), Vec3::NEG_Y), ((0..=m.x).map(|x| (x, m.y)).collect(), Vec3::Y),
                                               ((0..=m.y).map(|y| (0, y)).collect(), Vec3::NEG_X), ((0..=m.y).map(|y| (m.x, y)).collect(), Vec3::X)];
    for (edge, n) in sides {
        for w in edge.windows(2) {
            let (a, c) = (at(w[0].0, w[0].1), at(w[1].0, w[1].1));
            let (a0, c0) = (a.with_z(lo.z), c.with_z(lo.z));
            tri(&mut pos, &mut nor, [a, c, a0], [n; 3]);
            tri(&mut pos, &mut nor, [c, c0, a0], [n; 3]);
        }
    }
    let f = [lo, Vec3::new(hi.x, lo.y, lo.z), Vec3::new(lo.x, hi.y, lo.z), hi.with_z(lo.z)];
    tri(&mut pos, &mut nor, [f[0], f[1], f[2]], [Vec3::NEG_Z; 3]);
    tri(&mut pos, &mut nor, [f[1], f[3], f[2]], [Vec3::NEG_Z; 3]);
    let pos: Vec<Vec3> = pos.into_iter().map(|p| p - centre).collect();
    let (n, tops) = (pos.len(), tan.len());
    tan.resize(n, [0.0; 4]);
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, (0..n).map(|i| if i < tops { [2.0f32; 2] } else { [0.0; 2] }).collect::<Vec<_>>())
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tan)
        .with_inserted_indices(Indices::U32(idx))
}

const PORTAL_REACH: f32 = 1.5;
const PORTAL_TOP: f32 = 7.0;

const RIPPLE_SLOPE: f32 = 0.05;
const RIPPLE_TILE: f32 = 2.0;

const RIPPLE_PX: u32 = 64;

fn ripples(data: &mut [u8], w: u32, t: f32) {
    let w = w as usize;
    for (j, px) in data[..w * w * 4].chunks_exact_mut(4).enumerate() {
        let g = slope((Vec2::new((j % w) as f32, (j / w) as f32) + 0.5) / w as f32 * RIPPLE_TILE, t);
        let n = Vec3::new(-g.x, -g.y, 1.0).normalize();
        px[0] = ((n.x * 0.5 + 0.5) * 255.0) as u8;
        px[1] = ((n.y * 0.5 + 0.5) * 255.0) as u8;
        px[2] = ((n.z * 0.5 + 0.5) * 255.0) as u8;
    }
    mips(data, w);
}

fn mips(data: &mut [u8], mut w: usize) {
    let mut at = 0;
    while w > 1 && at + w * w * 4 + (w / 2) * (w / 2) * 4 <= data.len() {
        let (src, dst) = data[at..].split_at_mut(w * w * 4);
        let h = w / 2;
        for j in 0..h * h {
            let (x, y) = (j % h * 2, j / h * 2);
            for c in 0..4 {
                let px = |a: usize, b: usize| src[(a + w * b) * 4 + c] as u32;
                dst[j * 4 + c] = ((px(x, y) + px(x + 1, y) + px(x, y + 1) + px(x + 1, y + 1) + 2) / 4) as u8;
            }
        }
        at += w * w * 4;
        w = h;
    }
}

fn mipped(w: u32, fill: [u8; 4], f: bevy::image::ImageFilterMode) -> Image {
    use bevy::image::{ImageAddressMode, ImageSamplerDescriptor};
    let mut i = Image::new_fill(wgpu::Extent3d { width: w, height: w, depth_or_array_layers: 1 }, wgpu::TextureDimension::D2, &fill,
                                wgpu::TextureFormat::Rgba8Unorm, RenderAssetUsages::default());
    let levels = w.ilog2() + 1;
    let len: u32 = (0..levels).map(|l| (w >> l) * (w >> l) * 4).sum();
    i.data = Some(fill.iter().copied().cycle().take(len as usize).collect());
    i.texture_descriptor.mip_level_count = levels;
    i.sampler = bevy::image::ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::Repeat, address_mode_v: ImageAddressMode::Repeat,
                                                                              mag_filter: f, min_filter: f, mipmap_filter: f, ..ImageSamplerDescriptor::linear() });
    i
}

fn slope(p: Vec2, t: f32) -> Vec2 {
    const WAVES: [(f32, f32); 16] = [(9.0, 33.0), (0.0, 39.0), (-46.0, -4.0), (38.0, -24.0), (-19.0, -27.0), (27.0, -30.0), (10.0, 6.0), (24.0, 15.0),
                                     (-4.0, -34.0), (43.0, 14.0), (-43.0, 2.0), (39.0, 16.0), (-28.0, 0.0), (-15.0, -31.0), (27.0, -16.0), (14.0, 40.0)];
    WAVES.iter().enumerate().map(|(i, &(a, b))| {
        let k = std::f32::consts::TAU / RIPPLE_TILE * Vec2::new(a, b);
        let kl = k.length();
        let om = (9.81 * kl + 0.074 / 1000.0 * kl.powi(3)).sqrt();
        k / kl * RIPPLE_SLOPE * (k.dot(p) - om * t + i as f32 * 2.4).cos()
    }).sum()
}

const CAUSTIC_PX: usize = 256;
const CAUSTIC_FOCUS: f32 = 0.1;

fn caustic(data: &mut [u8], w: usize, t: f32) {
    let g: Vec<Vec2> = (0..w * w).map(|j| slope((Vec2::new((j % w) as f32, (j / w) as f32) + 0.5) / w as f32 * RIPPLE_TILE, t)).collect();
    let mut sum = vec![0.0f32; w * w];
    let wf = w as f32;
    for j in 0..w * w {
        let (x, y) = ((j % w) as f32, (j / w) as f32);
        for k in 0..4 {
            let (fx, fy) = (0.25 + 0.5 * (k % 2) as f32, 0.25 + 0.5 * (k / 2) as f32);
            let at = |a: usize, b: usize| g[(a % w) + w * (b % w)];
            let (i, jj) = (j % w, j / w);
            let s = at(i, jj).lerp(at(i + 1, jj), fx).lerp(at(i, jj + 1).lerp(at(i + 1, jj + 1), fx), fy);
            let q = Vec2::new(x + fx, y + fy) - s * 0.25 * CAUSTIC_FOCUS * wf / RIPPLE_TILE;
            let (qx, qy) = (q.x.rem_euclid(wf) as usize % w, q.y.rem_euclid(wf) as usize % w);
            sum[qx + w * qy] += 0.25;
        }
    }
    let soft = |j: usize| (0..9).map(|k| sum[(j % w + w + k % 3 - 1) % w + w * ((j / w + w + k / 3 - 1) % w)]).sum::<f32>() / 9.0;
    for (j, px) in data[..w * w * 4].chunks_exact_mut(4).enumerate() {
        let l = soft(j);
        let v = ((l * 0.25).clamp(0.0, 1.0) * 255.0) as u8;
        px.copy_from_slice(&[v, v, v, 255]);
    }
    mips(data, w);
}

const SHEET: f32 = 0.3;
const AIR: f32 = 0.45;
const EDGE: f32 = 0.01;
const ISO: f32 = 0.005;
const WET_DARK: f32 = 0.4;
const WET_DRY: f32 = 30.0;

fn trail(wet: &[f32], ground: &[f32], n: [usize; 3], lo: Vec3, h: f32) -> Mesh {
    let (nx, ny) = (n[0], n[1]);
    let dark = |k: usize| if wet[k] < 1.0 && ground[k] > -40.0 { wet[k] * WET_DARK } else { 0.0 };
    let (mut pos, mut col, mut idx) = (vec![], vec![], vec![]);
    for k in (0..wet.len()).filter(|&k| wet[k] > 0.02 && wet[k] < 1.0 && ground[k] > -40.0) {
        let (x, y) = (k % nx, k / nx);
        let base = pos.len() as u32;
        for (dx, dy) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
            let (mut s, mut c) = (0.0, 0.0);
            for (a, b) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (i, j) = ((x + dx) as isize - a, (y + dy) as isize - b);
                if i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < ny {
                    let q = i as usize + nx * j as usize;
                    if (ground[q] - ground[k]).abs() < 0.02 {
                        s += dark(q);
                        c += 1.0;
                    }
                }
            }
            pos.push([lo.x + (x + dx) as f32 * h, lo.y + (y + dy) as f32 * h, ground[k] + 0.001]);
            col.push([0.0, 0.0, 0.0, s / c]);
        }
        idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; pos.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

fn surface(count: &[u32], ground: &[f32], tops: &[f32], lift: f32, vel: &[Vec2], n: [usize; 3], lo: Vec3, h: f32, air: bool, blocky: bool, pour: bool) -> (Mesh, Mesh, Mesh) {
    let (nx, ny, nz) = (n[0], n[1], n[2]);
    let floor = |k: usize| ground[k] > -40.0;
    let (mut depth, wtop): (Vec<f32>, Vec<f32>) = (0..nx * ny).map(|k| {
        let g = ground[k];
        if !floor(k) {
            return (0.0, f32::MIN);
        }
        let (mut s, mut top) = (0.0, g);
        for z in 0..nz {
            let m = lo.z + (z as f32 + 0.5) * h;
            let c = count[k + nx * ny * z] as f32;
            if m < g {
                continue;
            }
            if m >= g + SHEET && c < 2.0 {
                break;
            }
            s += c;
            top = m + 0.5 * h;
        }
        (s * h / 8.0, top)
    }).unzip();
    for _ in 0..2 {
        depth = (0..nx * ny).map(|k| {
            let (x, y) = ((k % nx) as isize, (k / nx) as isize);
            let mut s = 0.0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (a, b) = (x + dx, y + dy);
                    if a >= 0 && b >= 0 && a < nx as isize && b < ny as isize {
                        s += depth[a as usize + nx * b as usize];
                    }
                }
            }
            s / 9.0
        }).collect();
    }
    let corner: Vec<Option<(f32, f32)>> = (0..(nx + 1) * (ny + 1)).map(|c| {
        let (i, j) = (c % (nx + 1), c / (nx + 1));
        let round: Vec<usize> = [(0, 0), (1, 0), (0, 1), (1, 1)].iter().filter(|&&(a, b)| i >= a && j >= b && i - a < nx && j - b < ny)
            .map(|&(a, b)| (i - a) + nx * (j - b)).collect();
        let near: Vec<usize> = round.iter().copied().filter(|&k| floor(k)).collect();
        let low = near.iter().map(|&k| ground[k]).fold(f32::MAX, f32::min);
        let near: Vec<usize> = near.into_iter().filter(|&k| ground[k] < low + 0.05).collect();
        let pool = round.iter().filter(|&&k| ground[k] < -1e8).map(|&k| tops[k] + lift - low).fold(0.0, f32::max);
        (!near.is_empty()).then(|| (low, (near.iter().map(|&k| depth[k]).sum::<f32>() / near.len() as f32).max(pool)))
    }).collect();
    let speed = |c: usize| {
        let (i, j) = (c % (nx + 1), c / (nx + 1));
        let (mut s, mut w) = (Vec2::ZERO, 0.0);
        for (a, b) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            if i >= a && j >= b && i - a < nx && j - b < ny {
                let k = (i - a) + nx * (j - b);
                s += vel.get(k).copied().unwrap_or_default() * depth[k];
                w += depth[k];
            }
        }
        if w > 0.0 { s / w } else { Vec2::ZERO }
    };
    let z = |c: usize| corner[c].map(|(g, d)| g + if d > EDGE { 0.004 + d - EDGE } else { 0.002 + 0.002 * (d - ISO) / (EDGE - ISO) });
    let landing = |k: usize| (0..nz).filter(|&z| (ground[k] + SHEET..ground[k] + SHEET + 0.6).contains(&(lo.z + (z as f32 + 0.5) * h)))
        .map(|z| count[k + nx * ny * z] as f32).sum::<f32>() / 8.0;
    #[derive(Default)]
    struct Out { p: Vec<Vec3>, nor: Vec<Vec3>, sv: Vec<[f32; 2]>, fp: Vec<Vec3>, fc: Vec<[f32; 4]>, fv: Vec<[f32; 2]>, at: Vec<u32>, cut: HashMap<(u32, u32), u32> }
    impl Out {
        fn lerp(&mut self, a: u32, b: u32, t: f32) -> u32 {
            let key = (a.min(b), a.max(b));
            if let Some(&v) = self.cut.get(&key) {
                return v;
            }
            let (a, b) = (a as usize, b as usize);
            let l2 = |x: [f32; 2], y: [f32; 2]| [x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t];
            self.p.push(self.p[a].lerp(self.p[b], t));
            self.nor.push(self.nor[a].lerp(self.nor[b], t).normalize_or(Vec3::Z));
            self.sv.push(l2(self.sv[a], self.sv[b]));
            self.fp.push(self.fp[a].lerp(self.fp[b], t));
            self.fc.push(Vec4::from(self.fc[a]).lerp(Vec4::from(self.fc[b]), t).to_array());
            self.fv.push(l2(self.fv[a], self.fv[b]));
            self.cut.insert(key, self.p.len() as u32 - 1);
            self.p.len() as u32 - 1
        }
    }
    let mut o = Out { at: vec![u32::MAX; corner.len()], ..default() };
    let vert = |c: usize, o: &mut Out| {
        if o.at[c] == u32::MAX {
            let (i, j) = ((c % (nx + 1)) as isize, (c / (nx + 1)) as isize);
            let zc = z(c).unwrap_or(0.0);
            let hz = |a: isize, b: isize| if a < 0 || b < 0 || a > nx as isize || b > ny as isize { zc } else { z(a as usize + (nx + 1) * b as usize).unwrap_or(zc).max(zc - h) };
            o.at[c] = o.p.len() as u32;
            o.p.push((lo.truncate() + Vec2::new(i as f32, j as f32) * h).extend(zc));
            let d = corner[c].map_or(0.0, |c| c.1);
            let k = ((d - EDGE) / 0.04).clamp(0.0, 1.0);
            o.fp.push((lo.truncate() + Vec2::new(i as f32, j as f32) * h).extend(zc + 0.001));
            o.fc.push([k, k, k, 1.0]);
            o.fv.push(speed(c).to_array());
            o.nor.push(Vec3::new(hz(i - 1, j) - hz(i + 1, j), hz(i, j - 1) - hz(i, j + 1), 2.0 * h).normalize());
            let fall = [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().map(|&(a, b)| hz(i + a, j + b) - zc).fold(0.0, f32::max);
            let down = [(0, 0), (1, 0), (0, 1), (1, 1)].iter().filter(|&&(a, b)| i >= a && j >= b && ((i - a) as usize) < nx && ((j - b) as usize) < ny)
                .map(|&(a, b)| (i - a) as usize + nx * (j - b) as usize).filter(|&k| floor(k)).map(landing).fold(0.0, f32::max);
            let white = if d > EDGE { ((fall - 0.04) / 0.1).clamp(0.0, 1.0).max((down * 0.5).min(1.0)) } else { 0.0 };
            o.sv.push([speed(c).length(), white]);
        }
        o.at[c]
    };
    let mut idx = vec![];
    for k in (0..nx * ny).filter(|&k| floor(k)) {
        let (x, y) = (k % nx, k / nx);
        let cs = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)].map(|(i, j)| i + (nx + 1) * j);
        if blocky {
            if depth[k] > ISO {
                let q = cs.map(|c| vert(c, &mut o));
                idx.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
            }
            continue;
        }
        let f = cs.map(|c| corner[c].map_or(-1.0, |(_, d)| d - ISO));
        if f.iter().all(|&f| f <= 0.0) {
            continue;
        }
        for t in [[0, 1, 2], [0, 2, 3]] {
            let mut poly = vec![];
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                if f[a] > 0.0 {
                    poly.push(vert(cs[a], &mut o));
                }
                if (f[a] > 0.0) != (f[b] > 0.0) {
                    let (va, vb) = (vert(cs[a], &mut o), vert(cs[b], &mut o));
                    poly.push(o.lerp(va, vb, f[a] / (f[a] - f[b])));
                }
            }
            for e in 1..poly.len().saturating_sub(1) {
                idx.extend([poly[0], poly[e], poly[e + 1]]);
            }
        }
    }
    let Out { p, nor, sv, mut fp, mut fc, mut fv, .. } = o;
    let mut fi = idx.clone();
    let (mut ap, mut an, mut ai) = (vec![], vec![], vec![]);
    if air {
        let low = (ground.iter().copied().filter(|&g| g > -40.0).fold(f32::MAX, f32::min) + SHEET).min(AIR);
        let z0 = (((low - lo.z) / h) as usize).saturating_sub(2).min(nz - 2);
        let (n, lo) = ([nx, ny, nz - z0], lo + Vec3::Z * (z0 as f32 * h));
        let mut d: Vec<f32> = count[z0 * nx * ny..].iter().map(|&c| c as f32 / 8.0).collect();
        for (a, step) in [(0, 1), (1, nx), (2, nx * ny)] {
            let len = n[a];
            d = (0..d.len()).map(|i| {
                let k = i / step % len;
                let mut s = d[i];
                if k > 0 {
                    s += d[i - step];
                }
                if k + 1 < len {
                    s += d[i + step];
                }
                s / 3.0
            }).collect();
        }
        let keep = |q: Vec3| {
            let c = ((q.truncate() - lo.truncate()) / h).floor().as_ivec2().clamp(IVec2::ZERO, IVec2::new(nx as i32 - 1, ny as i32 - 1));
            let g = ground[c.x as usize + nx * c.y as usize];
            let k = c.x as usize + nx * c.y as usize;
            if g > -40.0 { q.z > (g + SHEET).max(wtop[k] + h) } else { q.z > AIR.max(tops[k] + lift + h) && !(pour && q.truncate().distance(SPOUT.truncate().truncate()) < SPOUT.w + 2.0 * h) }
        };
        (ap, an, ai) = nets(&d, n, lo, h, 0.3, keep);
    }
    if pour {
        const SIDES: u32 = 16;
        let base = ap.len() as u32;
        let c = ((SPOUT.truncate().truncate() - lo.truncate()) / h).as_uvec2().min(UVec2::new(nx as u32 - 1, ny as u32 - 1));
        let end = tops.get(c.x as usize + nx * c.y as usize).map_or(WATER(), |t| t + lift - EDGE) - 0.02;
        let fell = (POUR_V / (POUR_V * POUR_V + 2.0 * 9.81 * (SPOUT.z - end)).sqrt()).sqrt();
        for (z, w) in [(SPOUT.z + 0.1, SPOUT.w), (end, SPOUT.w * fell)] {
            for k in 0..SIDES {
                let a = std::f32::consts::TAU * k as f32 / SIDES as f32;
                let n = Vec3::new(a.cos(), a.sin(), 0.0);
                ap.push(SPOUT.truncate().with_z(z) + n * w);
                an.push(n);
            }
        }
        for k in 0..SIDES {
            let l = (k + 1) % SIDES;
            ai.extend([base + k, base + SIDES + k, base + SIDES + l, base + k, base + SIDES + l, base + l]);
        }
    }
    if fi.is_empty() {
        (fp, fc, fv, fi) = (vec![lo, lo + Vec3::X * 0.001, lo + Vec3::Y * 0.001], vec![[0.0, 0.0, 0.0, 1.0]; 3], vec![[0.0; 2]; 3], vec![0, 1, 2]);
    }
    let fn_ = vec![Vec3::Z; fp.len()];
    let na = ap.len();
    (water_mesh(p, nor, idx, sv), water_mesh(ap, an, ai, vec![[1.0, 0.0]; na]),
     Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, fp)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, fn_)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; fv.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, fv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, fc)
        .with_inserted_indices(Indices::U32(fi)))
}

fn water_mesh(mut p: Vec<Vec3>, mut nor: Vec<Vec3>, mut idx: Vec<u32>, mut sv: Vec<[f32; 2]>) -> Mesh {
    if idx.is_empty() {
        (p, nor, idx, sv) = (vec![Vec3::new(0.0, 0.0, -50.0), Vec3::new(0.001, 0.0, -50.0), Vec3::new(0.0, 0.001, -50.0)], vec![Vec3::Z; 3], vec![0, 1, 2], vec![[0.0; 2]; 3]);
    }
    let uv: Vec<[f32; 2]> = p.iter().map(|p| [p.x, -p.y].map(|x| x / RIPPLE_TILE)).collect();
    let tan: Vec<[f32; 4]> = nor.iter().map(|n| (Vec3::X - *n * n.x).normalize_or(Vec3::Y).extend(1.0).to_array()).collect();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, sv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, tan)
        .with_inserted_indices(Indices::U32(idx))
}

fn nets(f: &[f32], n: [usize; 3], lo: Vec3, h: f32, iso: f32, keep: impl Fn(Vec3) -> bool) -> (Vec<Vec3>, Vec<Vec3>, Vec<u32>) {
    let at = |x: usize, y: usize, z: usize| f[x + n[0] * (y + n[1] * z)];
    let mid = |x: usize, y: usize, z: usize| lo + (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * h;
    let (cx, cy) = (n[0] - 1, n[1] - 1);
    let mut vert = vec![u32::MAX; cx * cy * (n[2] - 1)];
    let (mut p, mut nor, mut idx) = (vec![], vec![], vec![]);
    for z in 0..n[2] - 1 {
        for y in 0..cy {
            for x in 0..cx {
                let c: [f32; 8] = std::array::from_fn(|k| at(x + (k & 1), y + (k >> 1 & 1), z + (k >> 2)));
                let inside = c.iter().filter(|&&v| v > iso).count();
                if inside == 0 || inside == 8 {
                    continue;
                }
                let (mut s, mut m) = (Vec3::ZERO, 0.0);
                for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                    if (c[a] > iso) != (c[b] > iso) {
                        let t = (iso - c[a]) / (c[b] - c[a]);
                        let o = |k: usize| Vec3::new((k & 1) as f32, (k >> 1 & 1) as f32, (k >> 2) as f32);
                        s += o(a).lerp(o(b), t);
                        m += 1.0;
                    }
                }
                vert[x + cx * (y + cy * z)] = p.len() as u32;
                p.push(mid(x, y, z) + s / m * h);
                let g = Vec3::new(c[1] + c[3] + c[5] + c[7] - c[0] - c[2] - c[4] - c[6], c[2] + c[3] + c[6] + c[7] - c[0] - c[1] - c[4] - c[5],
                                  c[4] + c[5] + c[6] + c[7] - c[0] - c[1] - c[2] - c[3]);
                nor.push(-g.normalize_or(Vec3::Z));
            }
        }
    }
    let v = |x: usize, y: usize, z: usize| vert[x + cx * (y + cy * z)];
    for z in 1..n[2] - 1 {
        for y in 1..cy {
            for x in 1..cx {
                let here = at(x, y, z) > iso;
                for a in 0..3 {
                    let e = [(a == 0) as usize, (a == 1) as usize, (a == 2) as usize];
                    let (ox, oy, oz) = (x + e[0], y + e[1], z + e[2]);
                    if ox > cx || oy > cy || oz >= n[2] || (at(ox, oy, oz) > iso) == here || !keep((mid(x, y, z) + mid(ox, oy, oz)) * 0.5) {
                        continue;
                    }
                    let (b, c) = ((a + 1) % 3, (a + 2) % 3);
                    let cube = |db: usize, dc: usize| {
                        let mut q = [x, y, z];
                        q[b] -= db;
                        q[c] -= dc;
                        v(q[0], q[1], q[2])
                    };
                    let q = [cube(1, 1), cube(0, 1), cube(0, 0), cube(1, 0)];
                    if q.contains(&u32::MAX) {
                        continue;
                    }
                    idx.extend(if here { [q[0], q[1], q[2], q[0], q[2], q[3]] } else { [q[0], q[2], q[1], q[0], q[3], q[2]] });
                }
            }
        }
    }
    (p, nor, idx)
}

fn capsule(t: &GlobalTransform, b: &Aabb) -> (Vec3, Vec3, f32) {
    let m = t.affine();
    let sides = [0, 1, 2].map(|k| Vec3::from(m.matrix3.col(k)) * b.half_extents[k]);
    let k = (0..3).max_by(|&a, &c| sides[a].length().total_cmp(&sides[c].length())).unwrap();
    let r = (0..3).filter(|&a| a != k).map(|a| sides[a].length()).sum::<f32>() * 0.5;
    let at = t.transform_point(b.center.into());
    let half = sides[k].normalize_or_zero() * (sides[k].length() - r).max(0.0);
    (at - half, at + half, r)
}

pub fn real_water(mut commands: Commands, mut sim: NonSendMut<Sim>, device: Option<Res<RenderDevice>>, queue: Option<Res<RenderQueue>>, real: Option<ResMut<RealWater>>,
                  water: Query<&MeshMaterial3d<StandardMaterial>, With<Water>>, sky: Query<(Entity, &Mesh3d, &GlobalTransform), With<SkyWater>>,
                  things: Query<(Entity, &GlobalTransform, &InheritedVisibility, &Aabb, Option<&PartMesh>), With<PartMesh>>,
                  time: Res<Time>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, eye: Query<&GlobalTransform, With<OursCam>>,
                  mut images: ResMut<Assets<Image>>, (mut cmats, mut fmats, assets): (ResMut<Assets<SpillCaustic>>, ResMut<Assets<SpillFilm>>, Res<AssetServer>)) {
    let (Some(device), Some(queue)) = (device, queue) else { return };
    let (device, queue): (&wgpu::Device, &wgpu::Queue) = (device.wgpu_device(), &queue);
    let sky_now = sky.iter().next();
    let on = sim.real_water && (sky_now.is_some() || !water.is_empty());
    if !on {
        sim.flow.clear();
        sim.room.flow.clear();
        sim.hero_flow.clear();
        sim.real_level = None;
    }
    #[cfg(feature = "portal")]
    let pair = sim.portal.pair();
    #[cfg(not(feature = "portal"))]
    let pair: Option<[(Vec3, Quat); 2]> = None;
    let reach = pair.map_or(1.0, |p| p.iter().map(|e| e.0.z + PORTAL_REACH).fold(1.0, f32::max)).min(PORTAL_TOP);
    let remake = real.as_ref().is_some_and(|r| sim.real_drain || r.sky.is_none() && (sim.pouring || pair.is_some()) && (!r.flood || WATER() + r.basin.head < reach - 0.01));
    if let Some(r) = &real && (!on || r.sky.as_ref().map(|s| s.0) != sky_now.map(|s| s.0) || remake) {
        if let Some((e, _, old)) = &r.sky && sky.contains(*e) {
            commands.entity(*e).insert(Mesh3d(old.clone()));
        }
        for e in [r.foam.0, r.bubbles.0, r.sheet.0, r.mist.0] {
            commands.entity(e).try_despawn();
        }
        for e in r.spill.as_ref().map(|s| s.0).into_iter().chain(r.under.as_ref().map(|u| u.0)).chain(r.film.as_ref().map(|f| f.0))
            .chain(r.trail.as_ref().map(|t| t.0)).chain(r.pipe).chain(r.air.as_ref().map(|a| a.0)) {
            commands.entity(e).try_despawn();
        }
        commands.remove_resource::<RealWater>();
        sim.real_drain = false;
        return;
    }
    let Some(mut r) = real else {
        if on {
            let flood = (sim.pouring || pair.is_some()) && sky_now.is_none();
            let basin = match sky_now {
                Some(_) => Basin { lo: crate::SKY_POOL - crate::SKY_HALF.extend(0.0), size: (crate::SKY_HALF * 2.0).extend(crate::SKY_DEEP), cell: 0.1, head: 0.25 },
                None => Basin { lo: (*POOL())[0].extend(-POOL_DEEP()), size: ((*POOL())[1] - (*POOL())[0]).extend(POOL_DEEP() + WATER()), cell: if flood { SPILL_CELL } else { 0.1 }, head: reach - WATER() },
            };
            let spill = if flood { Vec2::splat(SPILL).extend(0.0) } else { Vec3::ZERO };
            let lo = basin.lo - spill - Vec3::splat(basin.cell);
            let n = ((basin.size + 2.0 * spill + Vec3::Z * basin.head) / basin.cell).round().as_uvec3() + UVec3::splat(2);
            let mut f = Fluid::new([n.x as usize, n.y as usize, n.z as usize], basin.cell, lo);
            f.fill(basin.lo, basin.lo + basin.size);
            let mut tops = vec![f32::MIN; (n.x * n.y) as usize];
            for p in &f.p {
                let c = ((p.x - lo) / basin.cell).as_uvec3();
                let k = (c.x + n.x * c.y) as usize;
                tops[k] = tops[k].max(p.x.z);
            }
            let wet: Vec<f32> = tops.iter().copied().filter(|t| *t > f32::MIN).collect();
            let rest = wet.iter().sum::<f32>() / wet.len() as f32;
            let tops = tops.iter().map(|t| if *t > f32::MIN { *t } else { rest }).collect();
            let foam = meshes.add(whitewater(&[], Vec3::ZERO, basin.lo, |_| true));
            let mut disc = Image::new_fill(Extent3d { width: 32, height: 32, depth_or_array_layers: 1 }, TextureDimension::D2, &[255; 4],
                                       TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
            for (i, px) in disc.data.as_mut().unwrap().chunks_exact_mut(4).enumerate() {
                let d = Vec2::new((i % 32) as f32 - 15.5, (i / 32) as f32 - 15.5).length() / 16.0;
                px[3] = (255.0 * (1.0 - d * d).max(0.0)) as u8;
            }
            let disc = images.add(disc);
            let m = mats.add(StandardMaterial { base_color: Color::srgb(0.95, 0.97, 1.0), base_color_texture: Some(disc.clone()),
                                                alpha_mode: AlphaMode::Blend, emissive: LinearRgba::rgb(0.12, 0.13, 0.14),
                                                perceptual_roughness: 0.8, cull_mode: None, double_sided: true, ..default() });
            let mut spawn = |mesh: Handle<Mesh>, m: Handle<StandardMaterial>| (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), Transform::default(),
                                                       bevy::camera::visibility::RenderLayers::layer(1), bevy::light::NotShadowCaster)).id(), mesh);
            let mut ww_mats = vec![m.clone()];
            let foam = spawn(foam, m);
            let m = mats.add(StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(disc.clone()), alpha_mode: AlphaMode::Blend,
                                                emissive: LinearRgba::rgb(0.35, 0.37, 0.4), reflectance: 1.0, perceptual_roughness: 0.1,
                                                cull_mode: None, double_sided: true, ..default() });
            ww_mats.push(m.clone());
            let bubbles = spawn(meshes.add(whitewater(&[], Vec3::ZERO, basin.lo, |_| true)), m);
            let sf = crate::foam::Foam::new(basin.lo.truncate(), basin.size.truncate(), &mut images, &mut meshes);
            let m = mats.add(StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(sf.img.clone()), alpha_mode: AlphaMode::Mask(0.5),
                                                emissive: LinearRgba::rgb(0.04, 0.045, 0.05), perceptual_roughness: 0.7, cull_mode: None,
                                                double_sided: true, ..default() });
            ww_mats.push(m.clone());
            let ww_mats: [Handle<StandardMaterial>; 3] = ww_mats.try_into().unwrap();
            let sheet = (spawn(sf.mesh.clone(), m).0, sf);
            let m = mats.add(StandardMaterial { base_color: Color::WHITE, base_color_texture: Some(disc.clone()), alpha_mode: AlphaMode::Blend,
                                                emissive: LinearRgba::rgb(0.05, 0.055, 0.06), perceptual_roughness: 1.0, cull_mode: None,
                                                double_sided: true, ..default() });
            let mesh = meshes.add(mist(&[], Vec3::ZERO, basin.lo));
            let mist = (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), Transform::default(), bevy::camera::visibility::RenderLayers::layer(1),
                                        bevy::light::NotShadowCaster, crate::water::Fx)).id(), mesh, vec![], 0.0);
            let mut g = GpuFluid::new(device, queue, &f, if flood { POUR_MORE } else { 0 });
            let ground: Vec<f32> = (0..n.x * n.y).map(|k| {
                let at = lo.truncate() + (UVec2::new(k % n.x, k / n.x).as_vec2() + 0.5) * basin.cell;
                if !flood || over_pool(at.extend(0.0)) { -1e9 } else { sim.world.ground(at.extend(4.0), 8.0).unwrap_or(-50.0) }
            }).collect();
            if flood {
                g.set_ground(queue, &ground);
                (g.spout, g.pour_v, g.drain) = (SPOUT, POUR_V, true);
            }
            let pipe = (flood && sim.room.stage != "pttest").then(|| commands.spawn(faucet(&mut meshes, &mut mats, &assets)).id());
            let mut r = RealWater { g, ready: Arc::new(AtomicBool::new(false)), stepped: Arc::new(AtomicBool::new(true)), busy: false, tops, rest,
                                     was: HashMap::new(), sky: None, basin, white: vec![], foam, bubbles, sheet, sheet_t: 0.0, mist, ww_mats, ww_rt: None, sent: vec![], read: vec![], flood, ground,
                                     spill: None, film: None, pipe, air: None, wet: vec![], trail: None, under: None, vel: vec![], caustic: images.add(mipped(CAUSTIC_PX as u32, [0, 0, 0, 255], bevy::image::ImageFilterMode::Linear)),
                                     ripples: [RIPPLE_PX, RIPPLE_PX / 2].map(|w| {
                                         let f = if w == RIPPLE_PX { bevy::image::ImageFilterMode::Linear } else { bevy::image::ImageFilterMode::Nearest };
                                         images.add(mipped(w * RIPPLE_TILE as u32, [128, 128, 255, 255], f))
                                     }) };
            if let Some((e, old, t)) = sky_now {
                let m = meshes.add(body(&r, t.translation(), 0.003, 0.0, false));
                commands.entity(e).insert(Mesh3d(m.clone()));
                r.sky = Some((e, m, old.0.clone()));
            }
            commands.insert_resource(r);
        }
        return;
    };
    let dt = time.delta_secs().clamp(1e-4, 1.0 / 30.0);
    let (blo, bhi) = (r.basin.lo - Vec3::splat(0.5), r.basin.lo + r.basin.size + Vec3::new(0.5, 0.5, r.basin.head));
    let (mut caps, mut tags) = (vec![], vec![]);
    let r = &mut *r;
    let near = |at: Vec3| !at.cmplt(blo).any() && !at.cmpgt(bhi).any();
    for (e, t, shown, b, part) in &things {
        let at = t.transform_point(b.center.into());
        let before = r.was.insert(e, at);
        if !shown.get() || !near(at) || caps.len() >= MAX_SOLIDS * 3 {
            continue;
        }
        let v = before.map(|q| (at - q) / dt).filter(|v| v.length() < 30.0).unwrap_or(Vec3::ZERO);
        let (a, c, rad) = capsule(t, b);
        caps.extend([a.extend(rad), c.extend(0.0), v.extend(0.0)]);
        let np = sim.build.parts.len();
        tags.push(part.and_then(|p| p.homer().filter(|&h| h <= sim.bench.len()).map(|h| Solid::Part(h * np + p.k))).unwrap_or(Solid::Other));
    }
    r.was.retain(|e, _| things.contains(*e));
    for j in 0..sim.room.objs.len() {
        let ((a, c, rad), _, v) = sim.room.obj_body(j);
        if near(a) && caps.len() < MAX_SOLIDS * 3 {
            caps.extend([a.extend(rad), c.extend(0.0), v.extend(0.0)]);
            tags.push(Solid::Obj(j));
        }
    }
    if (r.pipe.is_some() || sim.room.stage == "pttest") && sim.water_faucet {
        for (a, c) in faucet_caps() {
            if caps.len() < MAX_SOLIDS * 3 {
                caps.extend([a.extend(PIPE_R), c.extend(0.0), Vec4::ZERO]);
                tags.push(Solid::Other);
            }
        }
    }
    for (k, (a, c, rad, v, _)) in sim.hero_caps().into_iter().enumerate() {
        if near(a) && caps.len() < MAX_SOLIDS * 3 {
            caps.extend([a.extend(rad), c.extend(0.0), v.extend(0.0)]);
            tags.push(Solid::Hero(k));
        }
    }
    r.g.solids(queue, &caps);
    if r.busy && r.ready.swap(false, Ordering::Acquire) {
        let (lo, h, rest, nx, ny) = (r.g.lo.z, r.g.h, r.rest, r.g.n[0], r.g.n[1]);
        let (raw, white, flow, count, vel) = r.g.take_tops();
        r.vel = vel;
        r.white = white;
        let (np, no) = (sim.build.parts.len(), sim.room.objs.len());
        sim.flow = vec![None; np * (sim.bench.len() + 1)];
        sim.room.flow = vec![None; no];
        sim.hero_flow = vec![None; r.read.iter().map(|t| if let Solid::Hero(k) = t { k + 1 } else { 0 }).max().unwrap_or(0)];
        for (tag, f) in r.read.iter().zip(flow) {
            match *tag {
                Solid::Part(k) if k < sim.flow.len() => sim.flow[k] = Some(f),
                Solid::Obj(j) if j < no => sim.room.flow[j] = Some(f),
                Solid::Hero(k) => sim.hero_flow[k] = Some(f),
                _ => {}
            }
        }
        let mut raw: Vec<Option<f32>> = raw.into_iter().map(|b| (b != 0).then(|| lo + f32::from_bits(b))).collect();
        if r.g.pour > 0.0 {
            let (glo, sp) = (r.g.lo.truncate(), r.g.spout);
            let off = |k: usize| (glo + (Vec2::new((k % nx) as f32, (k / nx) as f32) + 0.5) * h).distance(sp.truncate().truncate());
            let ring: Vec<f32> = (0..nx * ny).filter(|&k| (sp.w + 0.5..sp.w + 0.5 + 2.0 * h).contains(&off(k))).filter_map(|k| raw[k]).collect();
            if !ring.is_empty() {
                let level = ring.iter().sum::<f32>() / ring.len() as f32;
                for k in (0..nx * ny).filter(|&k| off(k) < sp.w + 0.5) {
                    raw[k] = Some(level);
                }
            }
        }
        let pool = |i: usize| r.ground.get(i).is_some_and(|g| *g < -1e8);
        let blur = |v: &[Option<f32>], d: (isize, isize)| -> Vec<Option<f32>> {
            (0..nx * ny).map(|i| {
                v[i]?;
                let (x, y) = ((i % nx) as isize, (i / nx) as isize);
                let near: Vec<f32> = (-2..=2).filter_map(|k| {
                    let (xx, yy) = (x + k * d.0, y + k * d.1);
                    (xx >= 0 && yy >= 0 && (xx as usize) < nx && (yy as usize) < ny && pool(xx as usize + nx * yy as usize) == pool(i))
                        .then(|| v[xx as usize + nx * yy as usize]).flatten()
                }).collect();
                Some(near.iter().sum::<f32>() / near.len() as f32)
            }).collect()
        };
        let smooth = blur(&blur(&raw, (1, 0)), (0, 1));
        for (t, s) in r.tops.iter_mut().zip(smooth) {
            *t += (s.unwrap_or(rest - 3.0 * h) - *t) * 0.5;
        }
        if r.room() && r.flood {
            let (glo, sp) = (r.g.lo.truncate(), r.g.spout.truncate().truncate());
            let at = |k: usize| glo + (Vec2::new((k % nx) as f32, (k / nx) as f32) + 0.5) * h;
            let wall = |k: usize| { let p = at(k); (p.x - (*POOL())[0].x).min((*POOL())[1].x - p.x).min(p.y - (*POOL())[0].y).min((*POOL())[1].y - p.y) };
            let mid: Vec<f32> = (0..nx * ny).filter(|&k| r.ground[k] < -1e8 && wall(k) > 1.0 && at(k).distance(sp) > 2.0).map(|k| r.tops[k]).collect();
            if !mid.is_empty() {
                let level = mid.iter().sum::<f32>() / mid.len() as f32;
                let floor = -WATER() + rest;
                let head = (level - floor).max(0.0);
                let brink = if head > 0.0 { floor + 0.715 * (2.0 / 3.0) * head } else { level };
                let reach = (4.0 * (2.0 / 3.0) * head).max(0.5);
                for k in (0..nx * ny).filter(|&k| r.ground[k] < -1e8 && at(k).distance(sp) > 1.2) {
                    let t = (wall(k) / reach).clamp(0.0, 1.0);
                    let cap = brink + (level - brink) * t * t * (3.0 - 2.0 * t);
                    if wall(k) < reach {
                        r.tops[k] = r.tops[k].min(cap);
                    }
                }
            }
        }
        r.busy = false;
        if r.room() {
            let pool: Vec<f32> = (0..nx * ny).filter(|&k| r.ground[k] < -1e8).map(|k| r.tops[k]).collect();
            sim.real_level = (!pool.is_empty()).then(|| WATER() + pool.iter().sum::<f32>() / pool.len() as f32 - rest);
        }
        let shown = sim.water_spray;
        let sheet = shown && sim.water_foam;
        for (e, on) in [(r.foam.0, shown), (r.bubbles.0, shown), (r.sheet.0, sheet)] {
            commands.entity(e).try_insert(if on { Visibility::Inherited } else { Visibility::Hidden });
        }
        if shown && let Ok(eye) = eye.single() {
            let (e, lo) = (eye.translation(), r.basin.lo);
            if let Some(mut mesh) = meshes.get_mut(&r.foam.1) {
                *mesh = whitewater(&r.white, e, lo, |k| k == 0 || k == 1 && !sheet);
            }
            if let Some(mut mesh) = meshes.get_mut(&r.bubbles.1) {
                *mesh = whitewater(&r.white, e, lo, |k| k == 2);
            }
        }
        let rt = sim.rt_now() > 0;
        if r.ww_rt != Some(rt) {
            r.ww_rt = Some(rt);
            for (k, h) in r.ww_mats.iter().enumerate() {
                if let Some(mut m) = mats.get_mut(h) {
                    m.alpha_mode = if rt { AlphaMode::Mask(if k == 2 { 0.5 } else { 0.35 }) } else { AlphaMode::Blend };
                    if k == 1 {
                        m.perceptual_roughness = if rt { 0.5 } else { 0.1 };
                    }
                }
            }
        }
        let misty = shown && sim.water_mist && !rt;
        commands.entity(r.mist.0).try_insert(if misty { Visibility::Inherited } else { Visibility::Hidden });
        if misty && let Ok(eye) = eye.single() {
            let now = time.elapsed_secs();
            let r = &mut *r;
            let dt = (now - r.mist.3).clamp(0.0, 0.1);
            r.mist.3 = now;
            let puffs = &mut r.mist.2;
            for (p, v, age) in puffs.iter_mut() {
                *v *= (-2.5 * dt).exp();
                *p += (*v + Vec3::Z * 0.06) * dt;
                *age += dt;
            }
            puffs.retain(|x| x.2 < MIST_LIFE);
            for (k, &(at, v, kind)) in r.white.iter().enumerate() {
                if puffs.len() >= MIST_MAX {
                    break;
                }
                if kind == 0 && v.length_squared() > 2.0 && (k + (now * 60.0) as usize) % 6 == 0 {
                    puffs.push((at, v * 0.4, 0.0));
                }
            }
            if let Some(mut mesh) = meshes.get_mut(&r.mist.1) {
                *mesh = mist(puffs, eye.translation(), r.basin.lo);
            }
        }
        if sheet {
            let now = time.elapsed_secs();
            let r = &mut *r;
            let dt = (now - r.sheet_t).clamp(0.0, 0.1);
            r.sheet_t = now;
            r.sheet.1.step(dt, &r.white, &r.tops, (r.g.n[0], r.g.n[1], r.g.lo.truncate(), r.g.h), &mut images, &mut meshes);
        }
        if let Some((e, m, _)) = &r.sky && let Ok((_, _, t)) = sky.get(*e) && let Some(mut mesh) = meshes.get_mut(m) {
            *mesh = body(r, t.translation(), 0.003, time.elapsed_secs(), sim.real_ripples);
        }
        if r.sky.is_none() && (!count.is_empty() || r.spill.is_some()) {
            let r = &mut *r;
            let (g, ground, tops) = (&r.g, &r.ground, &r.tops);
            let (se, mesh, mat) = r.spill.get_or_insert_with(|| {
                let mesh = meshes.add(surface(&[], &[], &[], 0.0, &[], [0; 3], g.lo, g.h, false, false, false).0);
                let m = mats.add(StandardMaterial::default());
                (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m.clone()), Transform::default(), bevy::camera::visibility::RenderLayers::layer(1),
                                 bevy::light::NotShadowCaster, crate::Clear)).id(), mesh, m)
            });
            let (sheet, air, floor) = if count.is_empty() { surface(&[], &[], &[], 0.0, &[], [0; 3], g.lo, g.h, false, false, false) } else { surface(&count, ground, tops, WATER() - r.rest + EDGE, &r.vel, g.n, g.lo, g.h, sim.real_surface, sim.voxel_water, g.pour > 0.0) };
            let (ue, umesh) = r.under.get_or_insert_with(|| {
                let mesh = meshes.add(surface(&[], &[], &[], 0.0, &[], [0; 3], g.lo, g.h, false, false, false).2);
                let m = cmats.add(SpillCaustic { base: StandardMaterial { base_color: Color::linear_rgb(0.4, 0.45, 0.5), alpha_mode: AlphaMode::Add, perceptual_roughness: 1.0,
                                                                          reflectance: 0.0, ..default() },
                                                 extension: SpillCausticExt { net: r.caustic.clone() } });
                (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), Transform::default(), bevy::camera::visibility::RenderLayers::layer(1),
                                 bevy::light::NotShadowCaster, crate::water::Fx)).id(), mesh)
            });
            let (_, amesh, amat) = r.air.get_or_insert_with(|| {
                let mesh = meshes.add(surface(&[], &[], &[], 0.0, &[], [0; 3], g.lo, g.h, false, false, false).1);
                let m = mats.add(StandardMaterial::default());
                (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m.clone()), Transform::default(), bevy::camera::visibility::RenderLayers::layer(1),
                                 bevy::light::NotShadowCaster, crate::Clear)).id(), mesh, m)
            });
            if let Some(mut m) = meshes.get_mut(&*amesh) {
                let mut air = air;
                air.remove_attribute(Mesh::ATTRIBUTE_UV_1);
                *m = air;
            }
            let amat = amat.clone();
            let lit = sim.water_caustics && sim.rt_now() == 0;
            commands.entity(*ue).try_insert(if lit { Visibility::Inherited } else { Visibility::Hidden });
            if lit {
                if let Some(mut m) = meshes.get_mut(&*umesh) {
                    *m = floor;
                }
                if let Some(mut i) = images.get_mut(&r.caustic) && let Some(data) = i.data.as_mut() {
                    caustic(data, CAUSTIC_PX, time.elapsed_secs());
                }
            }
            let trailing = sim.wet_trail && sim.rt_now() == 0 && !count.is_empty();
            let (te, tmesh) = r.trail.get_or_insert_with(|| {
                let mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
                let m = mats.add(StandardMaterial { base_color: Color::srgba(0.0, 0.0, 0.0, 1.0), alpha_mode: AlphaMode::Premultiplied, perceptual_roughness: 0.35,
                                                    reflectance: 0.3, ..default() });
                (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), Transform::default(), bevy::camera::visibility::RenderLayers::layer(1),
                                 bevy::light::NotShadowCaster, crate::water::Fx)).id(), mesh)
            });
            let mut drying = false;
            if trailing {
                let (nx, ny, nz) = (g.n[0], g.n[1], g.n[2]);
                if r.wet.len() != nx * ny {
                    r.wet = vec![0.0; nx * ny];
                }
                let dry = (-time.delta_secs() / WET_DRY).exp();
                for k in 0..nx * ny {
                    let gz = ground[k];
                    let wet = gz > -40.0 && (0..nz).any(|z| count[k + nx * ny * z] > 0 && (gz..gz + SHEET).contains(&(g.lo.z + (z as f32 + 0.5) * g.h)));
                    r.wet[k] = if wet { 1.0 } else { r.wet[k] * dry };
                }
                drying = r.wet.iter().any(|&w| w > 0.02 && w < 1.0);
                if drying && let Some(mut m) = meshes.get_mut(&*tmesh) {
                    *m = trail(&r.wet, ground, g.n, g.lo, g.h);
                }
            }
            commands.entity(*te).try_insert(if drying { Visibility::Inherited } else { Visibility::Hidden });
            let clear = sim.water_clear;
            let map = sim.real_ripples.then(|| r.ripples[sim.water_pixels as usize].clone());
            let film = clear && sim.rt_now() == 0;
            let (fe, fm, fmesh) = r.film.get_or_insert_with(|| {
                let mesh = meshes.add(surface(&[], &[], &[], 0.0, &[], [0; 3], g.lo, g.h, false, false, false).0);
                let m = fmats.add(SpillFilm { base: StandardMaterial { base_color: Color::srgba(0.0, 0.0, 0.0, WET_DARK), alpha_mode: AlphaMode::Premultiplied,
                                                                       perceptual_roughness: 0.04, reflectance: 0.5, ..default() },
                                              extension: SpillFilmExt { ripples: r.ripples[0].clone(), k: Vec4::ZERO } });
                (commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m.clone()), Transform::default(), bevy::camera::visibility::RenderLayers::layer(1),
                                 bevy::light::NotShadowCaster, crate::Clear)).id(), m, mesh)
            });
            let mut sheet = sheet;
            if !film {
                sheet.remove_attribute(Mesh::ATTRIBUTE_UV_1);
            }
            if let Some(mut m) = meshes.get_mut(if film { &*fmesh } else { &*mesh }) {
                *m = sheet;
            }
            let k = Vec4::new(map.is_some() as u8 as f32, sim.water_impact as u8 as f32, 0.0, 0.0);
            let px = r.ripples[sim.water_pixels as usize].clone();
            if fmats.get(&*fm).is_some_and(|m| m.extension.k != k || m.extension.ripples != px) && let Some(mut m) = fmats.get_mut(&*fm) {
                (m.extension.k, m.extension.ripples) = (k, px);
            }
            commands.entity(*fe).try_insert(if film { Visibility::Inherited } else { Visibility::Hidden });
            commands.entity(*se).try_insert(if film { Visibility::Hidden } else { Visibility::Inherited });
            if let Some(pool) = water.iter().next().and_then(|w| mats.get(&w.0)).cloned() {
                if let Some(mut m) = mats.get_mut(&amat) {
                    *m = StandardMaterial { normal_map_texture: map.clone(), metallic_roughness_texture: None, thickness: 0.0, ..pool.clone() };
                }
                if let Some(mut m) = mats.get_mut(&*mat) {
                    *m = if clear {
                        StandardMaterial { base_color: Color::WHITE, alpha_mode: AlphaMode::Opaque, specular_transmission: 1.0, normal_map_texture: None,
                                           metallic_roughness_texture: None, thickness: 0.0, ..pool }
                    } else {
                        StandardMaterial { normal_map_texture: map.clone(), metallic_roughness_texture: None, thickness: 0.0, ..pool }
                    };
                }
            }
            if let Some(map) = map && let Some(mut i) = images.get_mut(&map) {
                let w = i.width();
                if let Some(data) = i.data.as_mut() {
                    ripples(data, w, time.elapsed_secs());
                }
            }
        }
    }
    if !r.busy {
        r.g.counts = (r.flood || sim.real_surface) && r.sky.is_none();
        r.g.request_tops(device, queue, r.ready.clone());
        r.read = r.sent.clone();
        r.busy = true;
    }
    if r.stepped.swap(false, Ordering::Acquire) {
        r.g.pour = if sim.pouring && r.flood { POUR } else { 0.0 };
        r.g.phys = Physics { relax: sim.water_relax, ghost: sim.water_ghost, vort: sim.water_vort, spray: sim.water_spray_phys };
        r.g.portals = pair.filter(|_| r.flood).map_or([Vec4::ZERO; 4], |[(a, qa), (b, qb)]| [a.extend(1.0), Vec4::from(qa), b.extend(1.0), Vec4::from(qb)]);
        r.g.step(device, queue, dt, r.g.substeps(dt));
        r.sent = tags;
        let done = r.stepped.clone();
        queue.on_submitted_work_done(move || done.store(true, Ordering::Release));
    }
    let _ = device.poll(wgpu::PollType::Poll);
}

