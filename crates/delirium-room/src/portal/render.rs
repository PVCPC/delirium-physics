//! Portal surfaces and views: a port of the rendering half of the Portal 2 Rust Engine
//! (`p2_render`; see CREDITS.md), adapted to this room's Z-up world and Bevy 0.18.
//!
//! * the surface is Portal 2's `PortalRefract` (`portal_surface.wgsl`): the burning rim, the opening
//!   animation, the static fade-in and the wall warp while it opens, with the game's own noise and
//!   colour-ramp textures read from your Portal 2 install;
//! * the view through a portal recurses: a tree of virtual cameras, one per sequence of portal
//!   crossings, down to `DEPTH`, and below that the "depth doubler" re-uses last frame's image;
//! * a near-plane cap keeps the opening drawn when the eye is closer to it than the near plane.
//!
//! A node is a sequence of crossings from the main camera. Node codes are binary: the leading 1 is
//! the root (the main camera) and each crossing appends a bit (0 = blue, 1 = orange).
use std::collections::BTreeMap;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{CameraProjection, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::math::Vec4Swizzles;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureFormat,
};
use bevy::shader::ShaderRef;

use super::{through, Frame, HALF};
use crate::{OursCam, Sim};

/// How many nested views carry real surfaces (Portal 2's `r_portal_stencil_depth`).
const DEPTH: u32 = 2;
/// Node codes 1..=NODES: root, levels 1..=DEPTH, and the plain level below with the depth doubler.
const NODES: usize = (1 << (DEPTH + 2)) - 1;
/// First render layer used by the surface quads (node `n` draws on `FIRST + n - 1`).
const FIRST: usize = 8;
/// A portal opens linearly over this long, seconds.
const OPEN_TIME: f32 = 0.5;
/// After linking, the static fades away over this long, seconds.
const STATIC_FADE: f32 = 1.0;
/// Distance in front of the wall at which the surface quad sits, metres.
const STANDOFF: f32 = 0.004;
/// Depth of the near-plane cap box behind the opening, metres.
const CAP_DEPTH: f32 = 0.35;
/// How far beyond the near plane the cap starts to take over, metres.
const CAP_MARGIN: f32 = 0.02;
/// The view's oblique near plane sits this far behind the exit plane (1 u).
const CLIP_BEHIND_EXIT: f32 = 0.0254;
/// `$PortalColorScale` of `portalstaticoverlay_N`.
const COLOR_SCALE: f32 = 4.0;

pub fn node_layer(code: u32) -> usize {
    FIRST + code as usize - 1
}
const fn node_level(code: u32) -> u32 {
    31 - code.leading_zeros()
}
const fn child_code(parent: u32, kind: usize) -> u32 {
    (parent << 1) | kind as u32
}

pub const LOOK_SHADER: Handle<Shader> = bevy::asset::uuid_handle!("6f1d2c3e-8a47-4b59-9e21-0c5d7a3b9f14");

const FLAG_HAS_VIEW: u32 = 1;
const FLAG_CAP: u32 = 2;
const FLAG_DOUBLER: u32 = 8;
const FLAG_BLEND: u32 = 16;

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct SurfaceParams {
    pub open_amount: f32,
    pub portal_active: f32,
    pub color_scale: f32,
    pub flags: u32,
    pub uv_remap: Vec4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceKey {
    blend: bool,
    cap: bool,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(SurfaceKey)]
pub struct PortalSurface {
    #[uniform(0)]
    params: SurfaceParams,
    #[texture(1)]
    #[sampler(2)]
    view: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    noise: Option<Handle<Image>>,
    #[texture(5)]
    #[sampler(6)]
    ramp: Option<Handle<Image>>,
    cap: bool,
    /// Fully open: blend the rim over the frame. While opening it is drawn opaque and reads a copy of
    /// the opaque pass to warp the wall (blended materials don't get that copy).
    blend: bool,
}

impl From<&PortalSurface> for SurfaceKey {
    fn from(m: &PortalSurface) -> Self {
        SurfaceKey { blend: m.blend, cap: m.cap }
    }
}

impl Material for PortalSurface {
    fn fragment_shader() -> ShaderRef {
        LOOK_SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        if self.cap || self.blend { AlphaMode::Blend } else { AlphaMode::Opaque }
    }
    fn reads_view_transmission_texture(&self) -> bool {
        !self.cap && !self.blend
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
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.cap {
            // Drawn over the wall: depth neither tested nor written.
            if let Some(ds) = descriptor.depth_stencil.as_mut() {
                ds.depth_write_enabled = false;
                ds.depth_compare = CompareFunction::Always;
            }
        }
        Ok(())
    }
}

impl Frame {
    fn to_world(&self, p: Vec3) -> Vec3 {
        self.at + self.rot * p
    }
    fn plane_distance(&self, p: Vec3) -> f32 {
        self.normal().dot(p - self.at)
    }
}

/// One placed portal as seen this frame.
#[derive(Clone, Copy)]
struct Info {
    frame: Frame,
    linked: bool,
    open: f32,
    stat: f32,
}

#[derive(Resource, Default)]
struct Infos([Option<Info>; 2]);

#[derive(Clone, Copy, Default)]
struct Lens {
    fov: f32,
    aspect: f32,
    near: f32,
    far: f32,
}

impl Lens {
    fn projection(&self, clip: Option<Vec4>) -> PerspectiveProjection {
        PerspectiveProjection {
            fov: self.fov,
            aspect_ratio: self.aspect,
            near: self.near,
            far: self.far,
            near_clip_plane: clip.unwrap_or(Vec4::new(0.0, 0.0, -1.0, -self.near)),
        }
    }
}

#[derive(Clone, Copy)]
struct Node {
    level: u32,
    pos: Vec3,
    rot: Quat,
    clip: Option<Vec4>,
    /// Where the portal this node looks through sits on screen, as seen by the parent camera
    /// (min.x, min.y, max.x, max.y, screen UV, y down). Used by the depth doubler.
    via_rect: Option<Vec4>,
}

fn clip_from_world(lens: &Lens, n: &Node) -> Mat4 {
    lens.projection(n.clip).get_clip_from_view() * Mat4::from_rotation_translation(n.rot, n.pos).inverse()
}

fn screen_rect(clip_from_world: Mat4, frame: &Frame) -> Option<Vec4> {
    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let c = clip_from_world * frame.to_world(Vec3::new(sx * HALF.x, sy * HALF.y, 0.0)).extend(1.0);
        if c.w <= 1e-4 {
            return None;
        }
        let ndc = c.truncate().truncate() / c.w;
        let uv = Vec2::new(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        lo = lo.min(uv);
        hi = hi.max(uv);
    }
    (hi.x - lo.x > 1e-5 && hi.y - lo.y > 1e-5).then(|| Vec4::new(lo.x, lo.y, hi.x, hi.y))
}

/// Can this camera see the front face of this portal at all?
fn portal_visible(clip_from_world: Mat4, cam: Vec3, frame: &Frame) -> bool {
    if frame.plane_distance(cam) <= 0.0 {
        return false;
    }
    let clip: Vec<Vec4> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter().map(|(sx, sy)| clip_from_world * frame.to_world(Vec3::new(sx * HALF.x, sy * HALF.y, 0.0)).extend(1.0)).collect();
    if clip.iter().all(|c| c.w <= 1e-4) {
        return false;
    }
    if clip.iter().any(|c| c.w <= 1e-4) {
        return true;
    }
    let all = |f: &dyn Fn(&Vec4) -> bool| clip.iter().all(f);
    !(all(&|c| c.x < -c.w) || all(&|c| c.x > c.w) || all(&|c| c.y < -c.w) || all(&|c| c.y > c.w))
}

/// The tree of virtual cameras this frame, down to level `DEPTH + 1`. A child exists only if its
/// parent's camera can see the portal it looks through.
fn build_tree(root: (Vec3, Quat), lens: &Lens, infos: &[Option<Info>; 2]) -> BTreeMap<u32, Node> {
    let mut nodes = BTreeMap::new();
    nodes.insert(1, Node { level: 0, pos: root.0, rot: root.1, clip: None, via_rect: None });
    let mut frontier = vec![1u32];
    for _ in 0..=DEPTH {
        let mut next = vec![];
        for code in std::mem::take(&mut frontier) {
            let n = nodes[&code];
            let cfw = clip_from_world(lens, &n);
            for kind in 0..2 {
                let Some(x) = infos[kind].filter(|x| x.linked) else { continue };
                let Some(exit) = infos[1 - kind] else { continue };
                if !portal_visible(cfw, n.pos, &x.frame) {
                    continue;
                }
                let (turn, map) = through(&x.frame, &exit.frame);
                let (pos, rot) = (map(n.pos), (turn * n.rot).normalize());
                // Oblique near plane: the exit portal's plane, in the child's view space.
                let inv = rot.inverse();
                let normal = (inv * exit.frame.normal()).normalize();
                let at = inv * (exit.frame.at - exit.frame.normal() * CLIP_BEHIND_EXIT - pos);
                let plane = normal.extend(-normal.dot(at));
                let child = child_code(code, kind);
                nodes.insert(child, Node {
                    level: n.level + 1,
                    pos,
                    rot,
                    clip: (plane.w < 0.0).then_some(plane),
                    via_rect: screen_rect(cfw, &x.frame),
                });
                next.push(child);
            }
        }
        frontier = next;
    }
    nodes
}

#[derive(Resource, Default)]
struct Tree(BTreeMap<u32, Node>);

#[derive(Component)]
struct NodeCam(u32);

#[derive(Resource, Default)]
struct Cams {
    cameras: BTreeMap<u32, Entity>,
    dummy: Handle<Image>,
}

#[derive(Resource, Default)]
struct NodeImages {
    size: UVec2,
    images: BTreeMap<u32, Handle<Image>>,
    /// Last frame's image of each plain-level node (ping-pong with `images`), sampled by the doubler.
    prev: BTreeMap<u32, Handle<Image>>,
}

#[derive(Resource, Default)]
struct Main {
    near: f32,
    lens: Lens,
}

fn new_target(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Rgba16Float, None))
}

#[derive(Component)]
struct Quad(usize, u32);
#[derive(Component)]
struct Cap(usize, u32);

#[derive(Resource)]
struct Parts {
    quads: [[Entity; NODES]; 2],
    caps: [[Entity; NODES]; 2],
    live: [[Handle<PortalSurface>; NODES]; 2],
    cap_mat: [[Handle<PortalSurface>; NODES]; 2],
    warm: u32,
}

fn cap_mesh(depth: f32) -> Mesh {
    let (x, y, z) = (HALF.x, HALF.y, -depth);
    let quads: [[Vec3; 4]; 5] = [
        [Vec3::new(-x, -y, z), Vec3::new(x, -y, z), Vec3::new(x, y, z), Vec3::new(-x, y, z)],
        [Vec3::new(-x, -y, 0.0), Vec3::new(-x, -y, z), Vec3::new(-x, y, z), Vec3::new(-x, y, 0.0)],
        [Vec3::new(x, -y, z), Vec3::new(x, -y, 0.0), Vec3::new(x, y, 0.0), Vec3::new(x, y, z)],
        [Vec3::new(-x, -y, 0.0), Vec3::new(x, -y, 0.0), Vec3::new(x, -y, z), Vec3::new(-x, -y, z)],
        [Vec3::new(-x, y, z), Vec3::new(x, y, z), Vec3::new(x, y, 0.0), Vec3::new(-x, y, 0.0)],
    ];
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    for q in quads {
        let n = (q[1] - q[0]).cross(q[2] - q[0]).normalize();
        let base = positions.len() as u32;
        for p in q {
            positions.push(p.to_array());
            normals.push(n.to_array());
            uvs.push([0.5, 0.5]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}

pub fn plugin(app: &mut App) {
    let _ = app.world_mut().resource_mut::<Assets<Shader>>()
        .insert(&LOOK_SHADER, Shader::from_wgsl(include_str!("../portal_surface.wgsl"), "portal_surface.wgsl"));
    app.add_plugins(MaterialPlugin::<PortalSurface>::default())
        .init_resource::<Infos>()
        .init_resource::<Tree>()
        .init_resource::<Cams>()
        .init_resource::<NodeImages>()
        .init_resource::<Main>()
        .add_systems(Startup, spawn)
        .configure_sets(PostUpdate, Render.before(bevy::camera::CameraUpdateSystems).before(bevy::transform::TransformSystems::Propagate))
        .add_systems(PostUpdate, (textures, update_views, sync_surfaces).chain().in_set(Render));
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Render;

/// The surface quads and caps (one pair per portal and node, hidden until needed), and the pool of
/// node cameras (all inactive to begin with).
fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<PortalSurface>>, mut images: ResMut<Assets<Image>>,
         mut cams: ResMut<Cams>) {
    let quad = meshes.add(Rectangle::new(HALF.x * 2.0, HALF.y * 2.0).mesh().build());
    let cap = meshes.add(cap_mesh(CAP_DEPTH));
    let mk = |flags: u32, cap: bool| PortalSurface {
        params: SurfaceParams { open_amount: 0.0, portal_active: 0.0, color_scale: COLOR_SCALE, flags, uv_remap: Vec4::new(1.0, 1.0, 0.0, 0.0) },
        view: None,
        noise: None,
        ramp: None,
        cap,
        blend: false,
    };
    let mut parts = Parts {
        quads: [[Entity::PLACEHOLDER; NODES]; 2],
        caps: [[Entity::PLACEHOLDER; NODES]; 2],
        live: Default::default(),
        cap_mat: Default::default(),
        warm: 120,
    };
    for kind in 0..2 {
        for i in 0..NODES {
            let node = i as u32 + 1;
            parts.live[kind][i] = mats.add(mk(0, false));
            parts.cap_mat[kind][i] = mats.add(mk(FLAG_CAP, true));
            parts.quads[kind][i] = commands.spawn((Mesh3d(quad.clone()), MeshMaterial3d(parts.live[kind][i].clone()), Transform::default(),
                Visibility::Hidden, RenderLayers::layer(node_layer(node)), NotShadowCaster, NotShadowReceiver, Quad(kind, node))).id();
            parts.caps[kind][i] = commands.spawn((Mesh3d(cap.clone()), MeshMaterial3d(parts.cap_mat[kind][i].clone()),
                Transform::from_scale(Vec3::splat(1e-4)), Visibility::Hidden, RenderLayers::layer(node_layer(node)), NotShadowCaster, NotShadowReceiver,
                Cap(kind, node))).id();
        }
    }
    commands.insert_resource(parts);
    cams.dummy = images.add(Image::new_target_texture(1, 1, TextureFormat::Rgba16Float, None));
    for code in 2..(1u32 << (DEPTH + 2)) {
        let level = node_level(code);
        let e = commands.spawn((
            Camera3d::default(),
            Camera { order: -((level as isize) * 100) - code as isize, is_active: false, ..default() },
            RenderTarget::Image(cams.dummy.clone().into()),
            Projection::Perspective(PerspectiveProjection::default()),
            bevy::render::view::Hdr,
            Tonemapping::None,
            DebandDither::Disabled,
            Msaa::Off,
            RenderLayers::layer(0),
            Transform::default(),
            NodeCam(code),
        )).id();
        cams.cameras.insert(code, e);
    }
}

/// Portal 2's own noise and colour-ramp textures, read from the linked install the first time.
fn textures(mut sim: NonSendMut<Sim>, mut images: ResMut<Assets<Image>>) {
    let p = &mut sim.portal;
    let Some(game) = p.game.clone().filter(|_| p.tex.is_none() && !p.tex_tried) else { return };
    p.tex_tried = true;
    const T: [(&str, bool, bool); 3] = [
        ("materials/models/portals/noise-blur-256x256.vtf", false, true),
        ("materials/models/portals/portal-blue-color.vtf", true, false),
        ("materials/models/portals/portal-orange-color.vtf", true, false),
    ];
    let Some(files) = super::vpk(&game, &T.map(|t| t.0)) else {
        eprintln!("portal  the portal textures aren't in {game}: portals won't be drawn");
        return;
    };
    let mut out = vec![];
    for (data, (path, srgb, wrap)) in files.iter().zip(T) {
        let tex = match crate::p2vtf::VtfTexture::parse(data) {
            Ok(t) => t,
            Err(e) => return eprintln!("portal  {path}: {e}"),
        };
        let mut img = tex.into_image(false, srgb);
        if let bevy::image::ImageSampler::Descriptor(d) = &mut img.sampler {
            let mode = if wrap { bevy::image::ImageAddressMode::Repeat } else { bevy::image::ImageAddressMode::ClampToEdge };
            d.address_mode_u = mode;
            d.address_mode_v = mode;
        }
        out.push(images.add(img));
    }
    p.tex = out.try_into().ok();
}

#[allow(clippy::too_many_arguments)]
fn update_views(mut sim: NonSendMut<Sim>, time: Res<Time>, mut commands: Commands, mut infos: ResMut<Infos>, mut tree: ResMut<Tree>,
                mut main: ResMut<Main>, cams: Res<Cams>, mut images: ResMut<Assets<Image>>, mut node_images: ResMut<NodeImages>,
                main_cam: Query<(Entity, &Camera, &Transform, &Projection, Option<&bevy::pbr::DistanceFog>, Option<&RenderLayers>), With<OursCam>>,
                mut cam_q: Query<(&mut Camera, &mut Transform, &mut Projection, &mut RenderLayers, &mut RenderTarget), (With<NodeCam>, Without<OursCam>)>) {
    // Open and static animation (PortalOpenAmount / PortalStatic).
    let (now, dt) = (time.elapsed_secs(), time.delta_secs());
    let ends = sim.portal.ends;
    for i in 0..2 {
        if ends[i].is_some() && sim.portal.placed[i].is_nan() {
            sim.portal.placed[i] = now;
        }
    }
    let linked = ends[0].is_some() && ends[1].is_some();
    if linked && !sim.portal.was_linked {
        sim.portal.stat = 1.0;
    }
    sim.portal.stat = if linked { (sim.portal.stat - dt / STATIC_FADE).max(0.0) } else { 1.0 };
    sim.portal.was_linked = linked;
    infos.0 = [0, 1].map(|i| ends[i].map(|frame| Info { frame, linked, open: ((now - sim.portal.placed[i]) / OPEN_TIME).clamp(0.0, 1.0), stat: sim.portal.stat }));

    let park = |cam_q: &mut Query<(&mut Camera, &mut Transform, &mut Projection, &mut RenderLayers, &mut RenderTarget), (With<NodeCam>, Without<OursCam>)>| {
        for &e in cams.cameras.values() {
            if let Ok((mut c, ..)) = cam_q.get_mut(e) {
                c.is_active = false;
            }
        }
    };
    let Ok((me, cam, at, Projection::Perspective(lens), fog, layers)) = main_cam.single() else { return park(&mut cam_q) };
    let want = RenderLayers::from_layers(&[0, 1, super::fx::MAIN_ONLY, node_layer(1)]);
    if layers != Some(&want) {
        commands.entity(me).insert(want);
    }
    let size = cam.physical_viewport_size().unwrap_or(UVec2::new(1280, 720)).max(UVec2::ONE);
    let lens = Lens { fov: lens.fov, aspect: size.x as f32 / size.y as f32, near: lens.near, far: lens.far };
    main.near = lens.near;
    main.lens = lens;
    let new = if infos.0.iter().any(Option::is_some) { build_tree((at.translation, at.rotation), &lens, &infos.0) } else { BTreeMap::new() };

    // One image per active child, sized like the main view.
    if node_images.size != size {
        node_images.size = size;
        node_images.images.clear();
        node_images.prev.clear();
    }
    for &code in new.keys().filter(|c| **c != 1) {
        if !node_images.images.contains_key(&code) {
            let h = new_target(&mut images, size);
            node_images.images.insert(code, h);
        }
        // Plain level: swap with last frame's image, which the doubler samples.
        if node_level(code) == DEPTH + 1 {
            let prev = node_images.prev.remove(&code).unwrap_or_else(|| new_target(&mut images, size));
            let cur = node_images.images.insert(code, prev).expect("inserted above");
            node_images.prev.insert(code, cur);
        }
    }

    for (&code, &entity) in cams.cameras.iter() {
        let Ok((mut c, mut tf, mut proj, mut layers, mut target)) = cam_q.get_mut(entity) else { continue };
        let Some(node) = new.get(&code) else {
            c.is_active = false;
            *target = RenderTarget::Image(cams.dummy.clone().into());
            continue;
        };
        let want = Transform::from_translation(node.pos).with_rotation(node.rot);
        if *tf != want {
            *tf = want;
        }
        *proj = Projection::Perspective(lens.projection(node.clip));
        let want = RenderLayers::from_layers(&[0, 1, node_layer(code)]);
        if *layers != want {
            *layers = want;
        }
        if let Some(image) = node_images.images.get(&code) {
            *target = RenderTarget::Image(image.clone().into());
        }
        c.is_active = true;
        c.order = -((node.level as isize) * 100) - code as isize;
        // A view should look like the room: same fog as the main camera.
        match fog {
            Some(f) => commands.entity(entity).insert(f.clone()),
            None => commands.entity(entity).remove::<bevy::pbr::DistanceFog>(),
        };
    }
    tree.0 = new;
}

fn set_material(mats: &mut Assets<PortalSurface>, h: &Handle<PortalSurface>, params: SurfaceParams, blend: bool, view: Option<Handle<Image>>, tex: Option<(&Handle<Image>, &Handle<Image>)>) {
    let Some(m) = mats.get(h) else { return };
    let same_view = match (&m.view, &view) {
        (Some(a), Some(b)) => a.id() == b.id(),
        (None, None) => true,
        _ => false,
    };
    let p = &m.params;
    let same = p.open_amount == params.open_amount && p.portal_active == params.portal_active && p.flags == params.flags && p.uv_remap == params.uv_remap;
    let same_tex = tex.is_none_or(|(n, r)| m.noise.as_ref().is_some_and(|x| x.id() == n.id()) && m.ramp.as_ref().is_some_and(|x| x.id() == r.id()));
    if same_view && same && same_tex && m.blend == blend {
        return;
    }
    if let Some(mut m) = mats.get_mut(h) {
        if !same_view {
            m.view = view;
        }
        m.params = params;
        m.blend = blend;
        if let Some((n, r)) = tex {
            m.noise = Some(n.clone());
            m.ramp = Some(r.clone());
        }
    }
}

/// Puts each portal's quads and caps on its wall and pushes this frame's view images, open and
/// static amounts into every surface.
#[allow(clippy::too_many_arguments)]
fn sync_surfaces(sim: NonSend<Sim>, infos: Res<Infos>, tree: Res<Tree>, node_images: Res<NodeImages>, main: Res<Main>, mut parts: ResMut<Parts>,
                 mut mats: ResMut<Assets<PortalSurface>>, mut tfs: Query<(&mut Transform, &mut Visibility, Has<Cap>)>) {
    let near_limit = main.near + CAP_MARGIN;
    let plain = DEPTH + 1;
    let warming = parts.warm > 0;
    parts.warm = parts.warm.saturating_sub(1);
    for kind in 0..2 {
        let info = infos.0[kind];
        let tex = sim.portal.tex.as_ref().map(|t| (&t[0], &t[1 + kind]));
        for i in 0..NODES {
            let node = i as u32 + 1;
            let (qe, ce) = (parts.quads[kind][i], parts.caps[kind][i]);
            let Some(info) = info else {
                for e in [qe, ce] {
                    if let Ok((_, mut v, _)) = tfs.get_mut(e) {
                        *v = Visibility::Hidden;
                    }
                }
                continue;
            };
            let view_node = tree.0.get(&node);
            let mut uv_remap = Vec4::new(1.0, 1.0, 0.0, 0.0);
            let mut doubler = false;
            let view = if node_level(node) == plain {
                // Depth doubler: this node has no child camera. Show last frame's image of this node,
                // scaled from where this node's own opening sits on screen into this portal's rectangle.
                match (info.linked, view_node) {
                    (true, Some(vn)) => {
                        let dst = screen_rect(clip_from_world(&main.lens, vn), &info.frame);
                        if let (Some(src), Some(dst)) = (vn.via_rect, dst) {
                            let scale = (src.zw() - src.xy()) / (dst.zw() - dst.xy());
                            let offset = src.xy() - dst.xy() * scale;
                            uv_remap = Vec4::new(scale.x, scale.y, offset.x, offset.y);
                            doubler = true;
                        }
                        node_images.prev.get(&node).cloned()
                    }
                    _ => None,
                }
            } else {
                match (info.linked, tree.0.get(&child_code(node, kind))) {
                    (true, Some(_)) => node_images.images.get(&child_code(node, kind)).cloned(),
                    _ => None,
                }
            };
            let has_view = view.is_some();
            let blend = info.open >= 1.0;
            let flags = if has_view { FLAG_HAS_VIEW } else { 0 } | if doubler { FLAG_DOUBLER } else { 0 } | if blend { FLAG_BLEND } else { 0 };
            let params = SurfaceParams { open_amount: info.open, portal_active: 1.0 - info.stat, color_scale: COLOR_SCALE, flags, uv_remap };
            set_material(&mut mats, &parts.live[kind][i], params, blend, view.clone(), tex);
            set_material(&mut mats, &parts.cap_mat[kind][i], SurfaceParams { flags: flags | FLAG_CAP, ..params }, false, view, tex);

            if let Ok((mut t, mut v, _)) = tfs.get_mut(qe) {
                *t = Transform::from_translation(info.frame.at + info.frame.normal() * STANDOFF).with_rotation(info.frame.rot);
                *v = Visibility::Visible;
            }
            // The cap shows only when this node's camera is just in front of the opening and the view exists.
            let cap_on = match view_node {
                Some(n) if has_view => {
                    let d = info.frame.plane_distance(n.pos);
                    d > 0.0 && d < near_limit
                }
                _ => false,
            };
            if let Ok((mut t, mut v, _)) = tfs.get_mut(ce) {
                *t = Transform::from_translation(info.frame.at).with_rotation(info.frame.rot).with_scale(Vec3::splat(if warming { 1e-4 } else { 1.0 }));
                let want = if cap_on || warming { Visibility::Inherited } else { Visibility::Hidden };
                if *v != want {
                    *v = want;
                }
            }
        }
    }
}
