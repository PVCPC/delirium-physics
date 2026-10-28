use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::math::Mat3;
use bevy::mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, VertexFormat,
};
use bevy::shader::ShaderRef;
use super::fs::Fs as GameFsRes;
use super::pcf::ParticleLibrary;
use super::sim::{ControlPoint, Particle, SystemInstance};

const SOURCE_UNIT: f32 = 0.0254;
pub const MAIN_ONLY: usize = 7;

fn source_to_world(v: Vec3) -> Vec3 {
    v * SOURCE_UNIT
}
fn source_dir_to_world(v: Vec3) -> Vec3 {
    v
}

pub const PORTAL_PCF_FILES: &[&str] = &["particles/portals.pcf", "particles/portal_projectile.pcf", "particles/portalgun.pcf"];

#[derive(Resource, Default, Clone, Debug)]
pub struct SheetGrids(pub HashMap<String, (u32, u32)>);

#[derive(Resource, Clone, Default)]
pub struct Particles {
    pub library: Arc<ParticleLibrary>,
}

#[derive(Resource, Default, Clone, Debug)]
pub struct ParticleStats {
    pub emitted: HashMap<String, u64>,
    pub alive: HashMap<String, usize>,
}

#[derive(Component)]
pub struct ParticleEffect {
    pub name: String,
    pub instance: Option<SystemInstance>,
    cps: Vec<(usize, ControlPoint)>,
    pub despawn_when_done: bool,
    pub restart_when_done: bool,
    pub render_layer: Option<usize>,
    pub visible: bool,
    pub render_transform: Transform,
    stop: bool,
    meshes: HashMap<usize, Entity>,
    dying: u8,
}

impl ParticleEffect {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            instance: None,
            cps: Vec::new(),
            despawn_when_done: true,
            restart_when_done: false,
            render_layer: None,
            visible: true,
            render_transform: Transform::IDENTITY,
            stop: false,
            meshes: HashMap::new(),
            dying: 0,
        }
    }
    pub fn with_cp(mut self, i: usize, pos: Vec3, basis: Mat3) -> Self {
        self.set_cp(i, pos, basis);
        self
    }
    pub fn with_cp_vector(mut self, i: usize, v: Vec3) -> Self {
        self.set_cp_vector(i, v);
        self
    }
    pub fn set_cp(&mut self, i: usize, pos: Vec3, basis: Mat3) {
        let cp = ControlPoint::new(world_to_source(pos), basis_to_source(basis));
        self.put(i, cp);
    }
    pub fn set_cp_vector(&mut self, i: usize, v: Vec3) {
        self.put(i, ControlPoint::new(v, Mat3::IDENTITY));
    }
    fn put(&mut self, i: usize, cp: ControlPoint) {
        self.cps.retain(|(k, _)| *k != i);
        self.cps.push((i, cp));
    }
    pub fn stop(&mut self) {
        self.stop = true;
        self.restart_when_done = false;
        self.despawn_when_done = true;
    }
}

pub fn world_to_source(v: Vec3) -> Vec3 {
    v / SOURCE_UNIT
}

fn basis_to_source(b: Mat3) -> Mat3 {
    b
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(SpriteCardKey)]
pub struct SpriteCardMaterial {
    #[uniform(0)]
    pub params: SpriteCardParams,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
    pub ignore_z: bool,
    pub refract: bool,
}

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct SpriteCardParams {
    pub overbright: f32,
    pub add_self: f32,
    pub flags: u32,
    pub refract: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpriteCardKey {
    ignore_z: bool,
    refract: bool,
}

impl From<&SpriteCardMaterial> for SpriteCardKey {
    fn from(m: &SpriteCardMaterial) -> Self {
        Self { ignore_z: m.ignore_z, refract: m.refract }
    }
}

const SPRITE_SHADER: Handle<Shader> = bevy::asset::uuid_handle!("3b9e4a72-51c8-4d06-8f2a-7e1c9b3d5a40");

const ATTR_SHAPE: MeshVertexAttribute = MeshVertexAttribute::new("ParticleShape", 0x5057_0001, VertexFormat::Float32x4);
const ATTR_AXIS: MeshVertexAttribute = MeshVertexAttribute::new("ParticleAxis", 0x5057_0002, VertexFormat::Float32x4);

impl Material for SpriteCardMaterial {
    fn vertex_shader() -> ShaderRef {
        SPRITE_SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        SPRITE_SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        if self.refract { AlphaMode::Opaque } else { AlphaMode::Premultiplied }
    }
    fn reads_view_transmission_texture(&self) -> bool {
        self.refract
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(5),
            ATTR_SHAPE.at_shader_location(8),
            ATTR_AXIS.at_shader_location(9),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            if key.bind_group_data.ignore_z {
                ds.depth_compare = Some(CompareFunction::Always);
            }
            if key.bind_group_data.refract {
                ds.depth_write_enabled = Some(false);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct CardInfo {
    material: Handle<SpriteCardMaterial>,
    orientation: i32,
    grid: Option<(u32, u32)>,
}

#[derive(Resource, Default)]
struct CardCache {
    cards: HashMap<String, Option<CardInfo>>,
}

fn load_card(
    name: &str,
    game: &GameFsRes,
    images: &mut Assets<Image>,
    materials: &mut Assets<SpriteCardMaterial>,
    grids: &SheetGrids,
) -> Option<CardInfo> {
    let grid = grids.0.get(name).copied();
    let read = |p: &str| game.read(p);
    let (shader, kv) = super::vmt::load_vmt(&read, name).unwrap_or_default();
    let refract = shader.eq_ignore_ascii_case("refract");
    let g = |k: &str| kv.get(k).map(|s| s.trim().to_string());
    let f = |k: &str, d: f32| g(k).and_then(|s| s.parse::<f32>().ok()).unwrap_or(d);
    let texture = g(if refract { "$normalmap" } else { "$basetexture" }).and_then(|t| {
        let path = format!("materials/{}.vtf", super::vmt::canonical_name(&t));
        let data = read(&path)?;
        let tex = crate::p2vtf::VtfTexture::parse(&data).ok()?;
        Some(images.add(tex.into_image(false, false)))
    });
    if texture.is_none() {
        if !name.is_empty() {
            if shader.is_empty() {
                warn!("p2_fx: particle material {name} not found in the Portal 2 install; not drawn");
            } else {
                warn!("p2_fx: particle material {name} ({shader}) has no texture this renderer can use; not drawn");
            }
        }
        return None;
    }
    let additive = f("$additive", 0.0) != 0.0;
    let distance_alpha = f("$distancealpha", 0.0) != 0.0;
    let flags = (additive as u32) | ((distance_alpha as u32) << 1) | ((refract as u32) << 2);
    let material = materials.add(SpriteCardMaterial {
        params: SpriteCardParams {
            overbright: f("$overbrightfactor", 1.0),
            add_self: f("$addself", 0.0),
            flags,
            refract: if refract { f("$refractamount", 0.5) } else { 0.0 },
        },
        texture,
        ignore_z: f("$ignorez", 0.0) != 0.0,
        refract,
    });
    Some(CardInfo { material, orientation: f("$orientation", 0.0) as i32, grid })
}

pub fn plugin(app: &mut App) {
    let _ = app.world_mut().resource_mut::<Assets<Shader>>()
        .insert(&SPRITE_SHADER, Shader::from_wesl(include_str!("card.wesl"), "card.wesl"));
    app.add_plugins(MaterialPlugin::<SpriteCardMaterial>::default())
        .init_resource::<CardCache>()
        .init_resource::<SheetGrids>()
        .init_resource::<ParticleStats>()
        .init_resource::<Particles>()
        .init_resource::<GameFsRes>()
        .add_systems(Update, load_particles)
        .add_systems(PostUpdate, (super::events::drain, step_effects, build_meshes).chain()
            .before(bevy::transform::TransformSystems::Propagate)
            .before(bevy::pbr::check_entities_needing_specialization::<SpriteCardMaterial>))
        .add_observer(cleanup_meshes);
}

fn load_particles(mut sim: NonSendMut<crate::Sim>, mut fs: ResMut<GameFsRes>, mut particles: ResMut<Particles>, mut tried: Local<bool>) {
    let Some(game) = sim.portal.game.clone().filter(|_| !*tried) else { return };
    *tried = true;
    if !fs.load(&game) {
        eprintln!("portal  particles: couldn't read {game}/portal2/pak01_dir.vpk");
        return;
    }
    let mut lib = ParticleLibrary::default();
    for f in PORTAL_PCF_FILES {
        match fs.read(f).map(|d| lib.load_pcf(&d)) {
            Some(Ok(_)) => {}
            Some(Err(e)) => eprintln!("portal  particles: {f}: {e}"),
            None => eprintln!("portal  particles: {f} missing from the Portal 2 install"),
        }
    }
    particles.library = Arc::new(lib);
    sim.portal.fx_ready = true;
}

pub(crate) fn step_effects(
    time: Res<Time>,
    particles: Option<Res<Particles>>,
    mut stats: ResMut<ParticleStats>,
    mut effects: Query<(Entity, &mut ParticleEffect)>,
    mut missing: Local<HashSet<String>>,
    mut commands: Commands,
) {
    let Some(particles) = particles else { return };
    let lib = &particles.library;
    let dt = time.delta_secs();
    stats.alive.clear();
    for (e, mut fx) in &mut effects {
        let fx = &mut *fx;
        if fx.dying > 0 {
            fx.dying += 1;
            if fx.dying > 6 {
                commands.entity(e).despawn();
            }
            continue;
        }
        if !fx.visible {
            continue;
        }
        if fx.instance.is_none() {
            let Some(def) = lib.get(&fx.name) else {
                if missing.insert(fx.name.clone()) {
                    warn!("p2_fx: particle system {} not found in the Portal 2 install; not drawn", fx.name);
                }
                commands.entity(e).despawn();
                continue;
            };
            fx.instance = Some(SystemInstance::new(def, e.index().index().wrapping_mul(2654435761)));
        }
        let inst = fx.instance.as_mut().unwrap();
        for (i, cp) in &fx.cps {
            inst.set_cp(*i, *cp);
        }
        if fx.stop && !inst.end_cap {
            inst.stop();
        }
        inst.update(dt, lib);
        inst.for_each(&mut |s| {
            if s.emitted_last > 0 {
                *stats.emitted.entry(s.def.name.clone()).or_default() += s.emitted_last as u64;
            }
            *stats.alive.entry(s.def.name.clone()).or_default() += s.particles.len();
        });
        if inst.finished() {
            if fx.restart_when_done && !fx.stop {
                fx.instance = None;
            } else if fx.despawn_when_done && inst.time > 0.05 {
                fx.visible = false;
                fx.dying = 1;
            }
        }
    }
}

#[derive(Default)]
struct MeshBuf {
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    color: Vec<[f32; 4]>,
    shape: Vec<[f32; 4]>,
    axis: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl MeshBuf {
    fn vert(&mut self, p: Vec3, uv: [f32; 2], c: [f32; 4], shape: [f32; 4], axis: [f32; 4]) -> u32 {
        self.pos.push(p.to_array());
        self.uv.push(uv);
        self.color.push(c);
        self.shape.push(shape);
        self.axis.push(axis);
        (self.pos.len() - 1) as u32
    }
    fn into_mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.color)
            .with_inserted_attribute(ATTR_SHAPE, self.shape)
            .with_inserted_attribute(ATTR_AXIS, self.axis)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

fn to_world(p: Vec3) -> Vec3 {
    source_to_world(p)
}
fn dir_to_world(v: Vec3) -> Vec3 {
    source_dir_to_world(v)
}

fn catmull(a: Vec3, b: Vec3, c: Vec3, d: Vec3, t: f32) -> Vec3 {
    b + 0.5 * t * (c - a + t * (2.0 * a - 5.0 * b + 4.0 * c - d + t * (-a + 3.0 * b - 3.0 * c + d)))
}

fn sheet_rect(grid: Option<(u32, u32)>, p: &Particle, now: f32, rate: f32, as_fps: bool) -> [f32; 4] {
    let Some((cols, rows)) = grid else { return [0.0, 0.0, 1.0, 1.0] };
    let cols = cols.max(1);
    let rows = rows.max(1);
    let n = (cols * rows) as f32;
    let age = now - p.created;
    let lf = if p.lifetime > 0.0 { age / p.lifetime } else { 0.0 };
    let f = p.sequence + if as_fps { age * rate } else { lf * rate * n };
    let idx = f.floor().rem_euclid(n) as u32;
    let (col, row) = ((idx % cols) as f32, (idx / cols) as f32);
    let (cw, rh) = (1.0 / cols as f32, 1.0 / rows as f32);
    [col * cw, row * rh, (col + 1.0) * cw, (row + 1.0) * rh]
}

fn sheet_params(r: &super::pcf::OpDef) -> (f32, bool) {
    (r.f("animation rate", 0.1), r.b("use animation rate as FPS", false))
}

fn build_system_mesh(inst: &SystemInstance, card: &CardInfo, time: f32) -> Option<(Mesh, bool)> {
    let def = &inst.def;
    let mut buf = MeshBuf::default();
    let mut main_only = false;
    let mut parts: Vec<&Particle> = inst.particles.iter().collect();
    parts.sort_by_key(|p| p.id);
    let color = |p: &Particle| [p.tint.x, p.tint.y, p.tint.z, (p.alpha * p.alpha2).clamp(0.0, 1.0)];
    for r in &def.renderers {
        if r.i("cull system starting at this recursion depth", -1) >= 1 {
            main_only = true;
        }
        match r.function.as_str() {
            "render_animated_sprites" | "render_sprites" => {
                let oriented = card.orientation == 3 || r.i("orientation_type", 0) == 3;
                let (rate, as_fps) = sheet_params(r);
                for p in &parts {
                    let c = color(p);
                    if c[3] <= 0.0 || p.radius <= 0.0 {
                        continue;
                    }
                    let rect = sheet_rect(card.grid, p, time, rate, as_fps);
                    let centre = to_world(p.pos);
                    let rad = p.radius * SOURCE_UNIT;
                    let axis = if oriented {
                        let n = dir_to_world(p.normal).normalize_or(Vec3::Y);
                        [n.x, n.y, n.z, 1.0]
                    } else {
                        [0.0, 0.0, 0.0, 0.0]
                    };
                    let base = buf.pos.len() as u32;
                    let [u0, v0, u1, v1] = rect;
                    for (cx, cy, u, v) in [(-1.0, -1.0, u0, v1), (1.0, -1.0, u1, v1), (1.0, 1.0, u1, v0), (-1.0, 1.0, u0, v0)] {
                        buf.vert(centre, [u, v], c, [cx, cy, rad, p.rotation], axis);
                    }
                    buf.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                }
            }
            "render_rope" => {
                if parts.len() < 2 {
                    continue;
                }
                let subdiv = r.i("subdivision_count", 3).max(1);
                let texel = r.f("texel_size", 4.0);
                let scroll = r.f("texture_scroll_rate", 0.0);
                let oriented = card.orientation == 3;
                let pts: Vec<Vec3> = parts.iter().map(|p| p.pos).collect();
                let n = pts.len();
                let mut length = 0.0f32;
                let mut prev_point: Option<Vec3> = None;
                let mut prev_pair: Option<(u32, u32)> = None;
                for seg in 0..n - 1 {
                    let a = pts[seg.saturating_sub(1)];
                    let b = pts[seg];
                    let c = pts[seg + 1];
                    let d = pts[(seg + 2).min(n - 1)];
                    let steps = if seg == n - 2 { subdiv + 1 } else { subdiv };
                    let (pa, pb) = (parts[seg], parts[seg + 1]);
                    for s in 0..steps {
                        let t = s as f32 / subdiv as f32;
                        let pt = catmull(a, b, c, d, t);
                        let rad = (pa.radius + (pb.radius - pa.radius) * t) * SOURCE_UNIT;
                        let (ca, cb) = (color(pa), color(pb));
                        let col = [0, 1, 2, 3].map(|k| ca[k] + (cb[k] - ca[k]) * t);
                        if let Some(pp) = prev_point {
                            length += pt.distance(pp);
                        }
                        prev_point = Some(pt);
                        let tangent = dir_to_world((c - b).normalize_or_zero());
                        let v = length * texel + time * scroll;
                        let axis = if oriented {
                            let nrm = dir_to_world(pa.normal.lerp(pb.normal, t)).normalize_or(Vec3::Y);
                            let ofs = (nrm - tangent * nrm.dot(tangent)).normalize_or(Vec3::Y);
                            [ofs.x, ofs.y, ofs.z, 3.0]
                        } else {
                            [tangent.x, tangent.y, tangent.z, 2.0]
                        };
                        let w = to_world(pt);
                        let i0 = buf.vert(w, [1.0, v], col, [-0.5, 0.0, rad, 0.0], axis);
                        let i1 = buf.vert(w, [0.0, v], col, [0.5, 0.0, rad, 0.0], axis);
                        if let Some((p0, p1)) = prev_pair {
                            buf.idx.extend_from_slice(&[p0, p1, i1, p0, i1, i0]);
                        }
                        prev_pair = Some((i0, i1));
                    }
                }
            }
            "render_sprite_trail" => {
                let minl = r.f("min length", 0.0);
                let maxl = r.f("max length", 2000.0);
                for p in &parts {
                    let c = color(p);
                    if c[3] <= 0.0 {
                        continue;
                    }
                    let vel = (p.pos - p.prev) / inst.last_dt.max(1e-4);
                    let len = (vel.length() * p.trail_length).clamp(minl, maxl);
                    let dir = vel.normalize_or(Vec3::Z);
                    let head = to_world(p.pos);
                    let tail = to_world(p.pos - dir * len);
                    let t = (head - tail).normalize_or(Vec3::Y);
                    let axis = [t.x, t.y, t.z, 2.0];
                    let rad = p.radius * SOURCE_UNIT;
                    let a0 = buf.vert(tail, [1.0, 1.0], c, [-0.5, 0.0, rad, 0.0], axis);
                    let a1 = buf.vert(tail, [0.0, 1.0], c, [0.5, 0.0, rad, 0.0], axis);
                    let b0 = buf.vert(head, [1.0, 0.0], c, [-0.5, 0.0, rad, 0.0], axis);
                    let b1 = buf.vert(head, [0.0, 0.0], c, [0.5, 0.0, rad, 0.0], axis);
                    buf.idx.extend_from_slice(&[a0, a1, b1, a0, b1, b0]);
                }
            }
            _ => {}
        }
    }
    if buf.idx.is_empty() {
        return None;
    }
    Some((buf.into_mesh(), main_only))
}

#[allow(clippy::too_many_arguments)]
fn build_meshes(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: Query<(Entity, &mut ParticleEffect)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SpriteCardMaterial>>,
    mut cache: ResMut<CardCache>,
    grids: Res<SheetGrids>,
    game: Res<GameFsRes>,
    mut mesh_handles: Query<&mut Mesh3d>,
    mut vis: Query<&mut Visibility>,
    mut mesh_tfs: Query<&mut Transform, With<ParticleMeshOf>>,
) {
    if !game.ready() {
        return;
    }
    let now = time.elapsed_secs();
    for (e, mut fx) in &mut effects {
        let fx = &mut *fx;
        let Some(inst) = &fx.instance else { continue };
        let mut systems = Vec::new();
        inst.for_each(&mut |s| systems.push(s));
        for (i, s) in systems.iter().enumerate() {
            let mat_name = s.def.material.clone();
            let Some(card) = cache
                .cards
                .entry(mat_name.clone())
                .or_insert_with(|| load_card(&mat_name, &*game, &mut images, &mut materials, &grids))
                .clone()
            else {
                continue;
            };
            let built = build_system_mesh(s, &card, now);
            let visible = fx.visible;
            match (built, fx.meshes.get(&i).copied()) {
                (Some((mesh, main_only)), Some(child)) => {
                    if let Ok(mut m) = mesh_handles.get_mut(child) {
                        if meshes.contains(&m.0) {
                            let _ = meshes.insert(&m.0, mesh);
                        } else {
                            m.0 = meshes.add(mesh);
                        }
                    }
                    if let Ok(mut v) = vis.get_mut(child) {
                        *v = if visible { Visibility::Inherited } else { Visibility::Hidden };
                    }
                    if let Ok(mut t) = mesh_tfs.get_mut(child) {
                        if *t != fx.render_transform {
                            *t = fx.render_transform;
                        }
                    }
                    let _ = main_only;
                }
                (Some((mesh, main_only)), None) => {
                    let layers = if let Some(l) = fx.render_layer {
                        RenderLayers::layer(l)
                    } else if main_only {
                        RenderLayers::layer(MAIN_ONLY)
                    } else {
                        RenderLayers::layer(0)
                    };
                    let child = commands
                        .spawn((
                            Name::new(format!("particles {}", s.def.name)),
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(card.material.clone()),
                            fx.render_transform,
                            GlobalTransform::from(fx.render_transform),
                            NoFrustumCulling,
                            layers,
                            bevy::light::NotShadowCaster,
                            bevy::light::NotShadowReceiver,
                            ParticleMeshOf(e),
                        ))
                        .id();
                    fx.meshes.insert(i, child);
                }
                (None, Some(child)) => {
                    if let Ok(mut v) = vis.get_mut(child) {
                        *v = Visibility::Hidden;
                    }
                }
                (None, None) => {}
            }
        }
    }
}

#[derive(Component)]
#[allow(dead_code)]
pub struct ParticleMeshOf(pub Entity);

fn cleanup_meshes(remove: On<Remove<ParticleEffect>>, q: Query<&ParticleEffect>, mut commands: Commands) {
    if let Ok(fx) = q.get(remove.entity) {
        for &m in fx.meshes.values() {
            if let Ok(mut ec) = commands.get_entity(m) {
                ec.despawn();
            }
        }
    }
}

