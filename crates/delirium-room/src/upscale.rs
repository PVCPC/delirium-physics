use bevy::camera::{CameraMainTextureUsages, MainPassResolutionOverride, Projection};
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, ViewPrepassTextures};
use bevy::diagnostic::FrameCount;
use bevy::ecs::query::QueryItem;
use bevy::prelude::*;
use bevy::render::camera::{MipBias, TemporalJitter};
use bevy::render::render_graph::{NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue};
use bevy::render::sync_world::RenderEntity;
use bevy::render::view::{prepare_view_targets, ExtractedView, Hdr, ViewTarget};
use bevy::render::{ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems};
use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use ash::vk::Handle;
use wgpu::hal::api::Vulkan;

#[derive(Component, Clone)]
#[require(TemporalJitter, MipBias, DepthPrepass, MotionVectorPrepass, Hdr)]
pub struct Upscale {
    pub by: By,
    pub mode: u8,
    pub sharpness: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum By {
    Fsr3,
    Xess,
}

#[derive(Resource)]
pub struct Supported {
    pub fsr3: bool,
    pub xess: bool,
}

const RATIO: [f32; 5] = [1.0, 1.5, 1.7, 2.0, 3.0];

pub fn vulkan_init(app: &mut App) {
    use bevy::render::renderer::raw_vulkan_init::RawVulkanInitSettings;
    let mut settings = app.world_mut().get_resource_or_init::<RawVulkanInitSettings>();
    unsafe {
        settings.add_create_device_callback(|args, adapter, _| {
            let ext = ash::ext::mutable_descriptor_type::NAME;
            let instance = adapter.shared_instance().raw_instance();
            let has = instance.enumerate_device_extension_properties(adapter.raw_physical_device())
                .is_ok_and(|l| l.iter().any(|e| e.extension_name_as_c_str() == Ok(ext)));
            if has && !args.extensions.contains(&ext) {
                args.extensions.push(ext);
                let f = Box::leak(Box::new(ash::vk::PhysicalDeviceMutableDescriptorTypeFeaturesEXT::default().mutable_descriptor_type(true)));
                *args.create_info = std::mem::take(args.create_info).push_next(f);
            }
        });
    }
}

pub struct UpscalePlugin;

impl Plugin for UpscalePlugin {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        let device = app.sub_app(RenderApp).world().resource::<RenderDevice>().wgpu_device().clone();
        let (ffx, xess) = (Ffx::load(&device).map(Arc::new), Xess::load(&device).map(Arc::new));
        if ffx.is_none() {
            info!("FSR 3: not on this system (needs amd_fidelityfx_vk.dll and Vulkan)");
        }
        if xess.is_none() {
            info!("XeSS: not on this system (needs libxess.dll and Vulkan)");
        }
        app.insert_resource(Supported { fsr3: ffx.is_some(), xess: xess.is_some() });
        if ffx.is_none() && xess.is_none() {
            return;
        }
        let copy = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fsr3_copy_in"),
            layout: None,
            module: &device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("fsr3_copy_in"), source: wgpu::ShaderSource::Wgsl(COPY_IN.into()) }),
            entry_point: Some("main"),
            compilation_options: default(),
            cache: None,
        });
        app.sub_app_mut(RenderApp)
            .insert_resource(Libs { ffx, xess, copy })
            .add_systems(ExtractSchedule, extract)
            .add_systems(Render, prepare.in_set(RenderSystems::ManageViews).before(prepare_view_targets))
            .add_render_graph_node::<ViewNodeRunner<UpscaleNode>>(Core3d, UpscaleLabel)
            .add_render_graph_edges(Core3d, (Node3d::MotionBlur, UpscaleLabel, Node3d::Bloom));
    }
}

const COPY_IN: &str = "
@group(0) @binding(0) var color: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_depth_2d;
@group(0) @binding(2) var mv: texture_2d<f32>;
@group(0) @binding(3) var o_color: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var o_depth: texture_storage_2d<r32float, write>;
@group(0) @binding(5) var o_mv: texture_storage_2d<rg32float, write>;
@group(0) @binding(6) var<uniform> at: vec4<u32>; // the view's corner, its size
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= at.zw) { return; }
    let p = vec2<i32>(id.xy + at.xy);
    textureStore(o_color, id.xy, textureLoad(color, p, 0));
    textureStore(o_depth, id.xy, vec4(textureLoad(depth, p, 0)));
    textureStore(o_mv, id.xy, textureLoad(mv, p, 0));
}
";

#[repr(C)]
struct Header {
    ty: u64,
    next: *mut Header,
}
#[repr(C)]
struct BackendVk {
    header: Header,
    device: ash::vk::Device,
    physical: ash::vk::PhysicalDevice,
    proc_addr: ash::vk::PFN_vkGetDeviceProcAddr,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Dims {
    w: u32,
    h: u32,
}
#[repr(C)]
struct CreateUpscale {
    header: Header,
    flags: u32,
    max_render: Dims,
    max_upscale: Dims,
    message: Option<unsafe extern "C" fn(u32, *const u16)>,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Resource {
    resource: *mut c_void,
    ty: u32,
    format: u32,
    width: u32,
    height: u32,
    depth: u32,
    mips: u32,
    flags: u32,
    usage: u32,
    state: u32,
}
const NONE: Resource = Resource { resource: null_mut(), ty: 0, format: 0, width: 0, height: 0, depth: 0, mips: 0, flags: 0, usage: 0, state: 0 };
#[repr(C)]
struct DispatchUpscale {
    header: Header,
    command_list: *mut c_void,
    color: Resource,
    depth: Resource,
    motion_vectors: Resource,
    exposure: Resource,
    reactive: Resource,
    transparency: Resource,
    output: Resource,
    jitter: [f32; 2],
    mv_scale: [f32; 2],
    render_size: Dims,
    upscale_size: Dims,
    sharpen: bool,
    sharpness: f32,
    frame_ms: f32,
    pre_exposure: f32,
    reset: bool,
    near: f32,
    far: f32,
    fov_y: f32,
    meters: f32,
    flags: u32,
}
const TYPE_BACKEND_VK: u64 = 3;
const TYPE_CREATE_UPSCALE: u64 = 0x10000;
const TYPE_DISPATCH_UPSCALE: u64 = 0x10001;
const FLAGS: u32 = 1 << 0 | 1 << 3 | 1 << 4 | 1 << 5;
const FLAG_DEBUG: u32 = 1 << 7;
const TEXTURE2D: u32 = 2;
const RGBA16F: u32 = 4;
const RG32F: u32 = 6;
const R32F: u32 = 28;
const USAGE_UAV: u32 = 2;
const STATE_UAV: u32 = 2;
const STATE_COMPUTE_READ: u32 = 4;

type PfnCreate = unsafe extern "C" fn(*mut *mut c_void, *mut Header, *const c_void) -> u32;
type PfnDestroy = unsafe extern "C" fn(*mut *mut c_void, *const c_void) -> u32;
type PfnDispatch = unsafe extern "C" fn(*mut *mut c_void, *const Header) -> u32;

struct Ffx {
    _lib: libloading::Library,
    create: PfnCreate,
    destroy: PfnDestroy,
    dispatch: PfnDispatch,
    device: wgpu::Device,
    backend: (ash::vk::Device, ash::vk::PhysicalDevice, ash::vk::PFN_vkGetDeviceProcAddr),
}

impl Ffx {
    fn load(device: &wgpu::Device) -> Option<Self> {
        let backend = unsafe {
            let hal = device.as_hal::<Vulkan>()?;
            (hal.raw_device().handle(), hal.raw_physical_device(), hal.shared_instance().raw_instance().fp_v1_0().get_device_proc_addr)
        };
        unsafe {
            let lib = libloading::Library::new("amd_fidelityfx_vk.dll").ok()?;
            Some(Self {
                create: *lib.get::<PfnCreate>(b"ffxCreateContext\0").ok()?,
                destroy: *lib.get::<PfnDestroy>(b"ffxDestroyContext\0").ok()?,
                dispatch: *lib.get::<PfnDispatch>(b"ffxDispatch\0").ok()?,
                _lib: lib,
                device: device.clone(),
                backend,
            })
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct XessView {
    view: ash::vk::ImageView,
    image: ash::vk::Image,
    range: ash::vk::ImageSubresourceRange,
    format: ash::vk::Format,
    width: u32,
    height: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct X2 {
    x: u32,
    y: u32,
}
#[repr(C)]
struct XessExecute {
    color: XessView,
    velocity: XessView,
    depth: XessView,
    exposure_scale: XessView,
    responsive: XessView,
    output: XessView,
    jitter: [f32; 2],
    exposure: f32,
    reset: u32,
    input_size: X2,
    bases: [X2; 6],
}
#[repr(C)]
struct XessInit {
    output: X2,
    quality: i32,
    flags: u32,
    creation_mask: u32,
    visible_mask: u32,
    buffer_heap: ash::vk::DeviceMemory,
    buffer_offset: u64,
    texture_heap: ash::vk::DeviceMemory,
    texture_offset: u64,
    pipeline_cache: ash::vk::PipelineCache,
}
const XESS_FLAGS: u32 = 1 << 1 | 1 << 8;
const XESS_QUALITY: [i32; 5] = [106, 103, 102, 101, 100];

type PfnXessCreate = unsafe extern "C" fn(ash::vk::Instance, ash::vk::PhysicalDevice, ash::vk::Device, *mut *mut c_void) -> i32;
type PfnXessInit = unsafe extern "C" fn(*mut c_void, *const XessInit) -> i32;
type PfnXessExecute = unsafe extern "C" fn(*mut c_void, ash::vk::CommandBuffer, *const XessExecute) -> i32;
type PfnXessDestroy = unsafe extern "C" fn(*mut c_void) -> i32;
type PfnXessOptimal = unsafe extern "C" fn(*mut c_void, *const X2, i32, *mut X2, *mut X2, *mut X2) -> i32;
type PfnXessScale = unsafe extern "C" fn(*mut c_void, f32, f32) -> i32;

struct Xess {
    _lib: libloading::Library,
    create: PfnXessCreate,
    init: PfnXessInit,
    execute: PfnXessExecute,
    destroy: PfnXessDestroy,
    optimal: PfnXessOptimal,
    velocity_scale: PfnXessScale,
    device: wgpu::Device,
    raw: (ash::vk::Instance, ash::vk::PhysicalDevice, ash::vk::Device),
}

impl Xess {
    fn load(device: &wgpu::Device) -> Option<Self> {
        let raw = unsafe {
            let hal = device.as_hal::<Vulkan>()?;
            if !hal.enabled_device_extensions().contains(&ash::ext::mutable_descriptor_type::NAME) {
                return None;
            }
            (hal.shared_instance().raw_instance().handle(), hal.raw_physical_device(), hal.raw_device().handle())
        };
        unsafe {
            let lib = libloading::Library::new("libxess.dll").ok()?;
            Some(Self {
                create: *lib.get(b"xessVKCreateContext\0").ok()?,
                init: *lib.get(b"xessVKInit\0").ok()?,
                execute: *lib.get(b"xessVKExecute\0").ok()?,
                destroy: *lib.get(b"xessDestroyContext\0").ok()?,
                optimal: *lib.get(b"xessGetOptimalInputResolution\0").ok()?,
                velocity_scale: *lib.get(b"xessSetVelocityScale\0").ok()?,
                _lib: lib,
                device: device.clone(),
                raw,
            })
        }
    }
}

unsafe extern "C" fn message(ty: u32, text: *const u16) {
    let n = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
    warn!("FSR 3 ({ty}): {}", String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, n) }));
}

#[derive(Resource)]
struct Libs {
    ffx: Option<Arc<Ffx>>,
    xess: Option<Arc<Xess>>,
    copy: wgpu::ComputePipeline,
}

enum Lib {
    Ffx(Arc<Ffx>),
    Xess(Arc<Xess>),
}

#[derive(Component)]
struct UpscaleCtx {
    lib: Lib,
    ctx: Mutex<Ctx>,
    mode: u8,
    render: UVec2,
    out_size: UVec2,
    color: wgpu::Texture,
    depth: wgpu::Texture,
    mv: wgpu::Texture,
    out: wgpu::Texture,
    views: [wgpu::TextureView; 4],
    at: wgpu::Buffer,
}
struct Ctx(*mut c_void);
unsafe impl Send for Ctx {}

impl Drop for UpscaleCtx {
    fn drop(&mut self) {
        let device = match &self.lib { Lib::Ffx(l) => &l.device, Lib::Xess(l) => &l.device };
        unsafe {
            if let Some(hal) = device.as_hal::<Vulkan>() {
                let _ = hal.raw_device().device_wait_idle();
            }
            let ctx = &mut self.ctx.get_mut().unwrap().0;
            match &self.lib {
                Lib::Ffx(l) => (l.destroy)(ctx, std::ptr::null()) as i32,
                Lib::Xess(l) => (l.destroy)(*ctx),
            };
        }
    }
}

impl UpscaleCtx {
    fn by(&self) -> By {
        match self.lib { Lib::Ffx(_) => By::Fsr3, Lib::Xess(_) => By::Xess }
    }
}

fn extract(mut commands: Commands, mut main: ResMut<MainWorld>, had: Query<Has<Upscale>>) {
    let mut cams = main.query::<(RenderEntity, &Camera, &Projection, Option<&Upscale>, Has<Hdr>)>();
    for (e, camera, projection, up, hdr) in cams.iter(&main) {
        let mut c = commands.entity(e);
        match up {
            Some(up) if hdr && camera.is_active && projection.is_perspective() => {
                c.insert(up.clone());
            }
            _ if had.get(e) == Ok(true) => {
                c.remove::<(Upscale, UpscaleCtx, MainPassResolutionOverride)>();
            }
            _ => {}
        }
    }
}

fn halton(mut i: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0, 0.0);
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

fn prepare(mut views: Query<(Entity, &ExtractedView, &Upscale, &mut CameraMainTextureUsages, &mut TemporalJitter, &mut MipBias, Option<&UpscaleCtx>)>,
           libs: Res<Libs>, device: Res<RenderDevice>, queue: Res<RenderQueue>, frame: Res<FrameCount>, mut commands: Commands) {
    for (e, view, up, mut usages, mut jitter, mut bias, ctx) in &mut views {
        usages.0 |= wgpu::TextureUsages::COPY_DST;
        let out_size = view.viewport.zw();
        let mode = up.mode.min(4);
        let at = |c: &UpscaleCtx| queue.write_buffer(&c.at, 0, bytes(&[view.viewport.x, view.viewport.y, c.render.x, c.render.y]));
        let render = match ctx {
            Some(c) if c.out_size == out_size && c.mode == mode && c.by() == up.by => {
                at(c);
                c.render
            }
            _ => {
                let lib = match up.by {
                    By::Fsr3 => libs.ffx.clone().map(Lib::Ffx),
                    By::Xess => libs.xess.clone().map(Lib::Xess),
                };
                let Some(c) = lib.and_then(|l| make(l, &device, out_size, mode)) else { continue };
                at(&c);
                let render = c.render;
                commands.entity(e).insert((c, MainPassResolutionOverride(render)));
                render
            }
        };
        bias.0 = (render.x as f32 / out_size.x as f32).log2() - 1.0;
        let phases = (8.0 * (out_size.x as f32 / render.x as f32).powi(2)) as u32;
        let i = frame.0 % phases.max(1) + 1;
        jitter.offset = Vec2::new(halton(i, 2), halton(i, 3)) - 0.5;
    }
}

fn bytes(v: &[u32; 4]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), 16) }
}

static REAL: std::sync::OnceLock<ash::vk::PFN_vkGetDeviceProcAddr> = std::sync::OnceLock::new();

unsafe extern "system" fn lookup(d: ash::vk::Device, name: *const std::ffi::c_char) -> ash::vk::PFN_vkVoidFunction {
    unsafe extern "system" fn nothing() -> i32 {
        0
    }
    let real = REAL.get().unwrap();
    let f = unsafe { real(d, name) };
    let n = unsafe { std::ffi::CStr::from_ptr(name) }.to_bytes();
    match n.strip_suffix(b"KHR") {
        _ if f.is_some() => f,
        Some(core) => unsafe { real(d, std::ffi::CString::new(core).unwrap().as_ptr()) },
        None if n.starts_with(b"vkSetDebugUtils") || n.starts_with(b"vkCmdBeginDebugUtils") || n.starts_with(b"vkCmdEndDebugUtils") => {
            Some(unsafe { std::mem::transmute::<unsafe extern "system" fn() -> i32, unsafe extern "system" fn()>(nothing) })
        }
        None => None,
    }
}

fn make(lib: Lib, device: &RenderDevice, out_size: UVec2, mode: u8) -> Option<UpscaleCtx> {
    let image = |label, size: UVec2, format, usage| device.wgpu_device().create_texture(&wgpu::TextureDescriptor {
        label: Some(label), size: wgpu::Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[],
    });
    use wgpu::{TextureFormat as F, TextureUsages as U};
    let input = U::STORAGE_BINDING | U::TEXTURE_BINDING;
    let mut ctx = null_mut();
    let render = match &lib {
        Lib::Ffx(ffx) => {
            let render = (out_size.as_vec2() / RATIO[mode as usize]).round().as_uvec2().max(UVec2::ONE);
            ctx = ffx_context(ffx, render, out_size)?;
            render
        }
        Lib::Xess(x) => {
            let (instance, physical, vk_device) = x.raw;
            let r = unsafe { (x.create)(instance, physical, vk_device, &mut ctx) };
            if r != 0 {
                error!("XeSS: couldn't start ({r})");
                return None;
            }
            let output = X2 { x: out_size.x, y: out_size.y };
            let quality = XESS_QUALITY[mode as usize];
            let init = XessInit {
                output, quality, flags: XESS_FLAGS, creation_mask: 0, visible_mask: 0, buffer_heap: default(), buffer_offset: 0,
                texture_heap: default(), texture_offset: 0, pipeline_cache: default(),
            };
            let (mut best, mut lo, mut hi) = (X2::default(), X2::default(), X2::default());
            let r = unsafe { (x.init)(ctx, &init) };
            let r = if r == 0 { unsafe { (x.optimal)(ctx, &output, quality, &mut best, &mut lo, &mut hi) } } else { r };
            if r != 0 {
                error!("XeSS: couldn't start ({r})");
                unsafe { (x.destroy)(ctx) };
                return None;
            }
            let render = UVec2::new(best.x, best.y).max(UVec2::ONE);
            unsafe { (x.velocity_scale)(ctx, -(render.x as f32), -(render.y as f32)) };
            render
        }
    };
    let color = image("upscale_color", render, F::Rgba16Float, input);
    let depth = image("upscale_depth", render, F::R32Float, input);
    let mv = image("upscale_mv", render, F::Rg32Float, input);
    let out = image("upscale_out", out_size, F::Rgba16Float, U::STORAGE_BINDING | U::TEXTURE_BINDING | U::COPY_SRC);
    Some(UpscaleCtx {
        views: [&color, &depth, &mv, &out].map(|t| t.create_view(&default())),
        lib,
        ctx: Mutex::new(Ctx(ctx)),
        mode,
        render,
        out_size,
        color,
        depth,
        mv,
        out,
        at: device.wgpu_device().create_buffer(&wgpu::BufferDescriptor { label: Some("upscale_at"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
    })
}

fn ffx_context(ffx: &Ffx, render: UVec2, out_size: UVec2) -> Option<*mut c_void> {
    let (vk_device, physical, proc_addr) = ffx.backend;
    let _ = REAL.set(proc_addr);
    let mut backend = BackendVk { header: Header { ty: TYPE_BACKEND_VK, next: null_mut() }, device: vk_device, physical, proc_addr: lookup };
    let debug = if std::env::var_os("FSR_DEBUG").is_some() { FLAG_DEBUG } else { 0 };
    let mut desc = CreateUpscale {
        header: Header { ty: TYPE_CREATE_UPSCALE, next: &mut backend.header },
        flags: FLAGS | debug,
        max_render: Dims { w: render.x, h: render.y },
        max_upscale: Dims { w: out_size.x, h: out_size.y },
        message: Some(message),
    };
    let mut ctx = null_mut();
    let r = unsafe { (ffx.create)(&mut ctx, &mut desc.header, std::ptr::null()) };
    if r != 0 {
        error!("FSR 3: couldn't start ({r})");
        return None;
    }
    Some(ctx)
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct UpscaleLabel;

#[derive(Default)]
struct UpscaleNode;

impl ViewNode for UpscaleNode {
    type ViewQuery = (&'static Upscale, &'static UpscaleCtx, &'static MainPassResolutionOverride, &'static TemporalJitter, &'static ViewTarget,
                      &'static ViewPrepassTextures, &'static ExtractedView);

    fn run(&self, _: &mut RenderGraphContext, render_context: &mut RenderContext, (fsr, c, render, jitter, target, prepass, view): QueryItem<Self::ViewQuery>,
           world: &World) -> Result<(), NodeRunError> {
        let (Some(depth), Some(mv)) = (&prepass.depth, &prepass.motion_vectors) else { return Ok(()) };
        let copy = &world.resource::<Libs>().copy;
        let write = target.post_process_write();
        let render = render.0;
        let bind = render_context.render_device().wgpu_device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("upscale_copy_in"),
            layout: &copy.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(write.source) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&depth.texture.default_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&mv.texture.default_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&c.views[0]) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&c.views[1]) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&c.views[2]) },
                wgpu::BindGroupEntry { binding: 6, resource: c.at.as_entire_binding() },
            ],
        });
        let encoder = render_context.command_encoder();
        encoder.push_debug_group("upscale");
        {
            let mut pass = encoder.begin_compute_pass(&default());
            pass.set_pipeline(copy);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(render.x.div_ceil(8), render.y.div_ceil(8), 1);
        }
        let state = |texture, state| wgpu::TextureTransition { texture, selector: None, state };
        encoder.transition_resources(std::iter::empty(), [
            state(&c.color, wgpu::TextureUses::RESOURCE),
            state(&c.depth, wgpu::TextureUses::RESOURCE),
            state(&c.mv, wgpu::TextureUses::RESOURCE),
            state(&c.out, wgpu::TextureUses::STORAGE_READ_WRITE),
        ].into_iter());
        encoder.pop_debug_group();
        let mut own = render_context.render_device().wgpu_device().create_command_encoder(&default());
        let mut ctx = c.ctx.lock().unwrap();
        let jitter = JITTER_SIGN * jitter.offset;
        let r = unsafe {
            own.as_hal_mut::<Vulkan, _, _>(|e| {
                let cmd = e.unwrap().raw_handle();
                match &c.lib {
                    Lib::Ffx(ffx) => ffx_dispatch(ffx, &mut ctx.0, cmd, c, fsr, render, jitter, view, world) as i32,
                    Lib::Xess(x) => xess_dispatch(x, ctx.0, cmd, c, render, jitter),
                }
            })
        };
        render_context.add_command_buffer(own.finish());
        if r != 0 {
            error!("{:?}: dispatch failed ({r})", c.by());
        }
        let at = wgpu::Origin3d { x: view.viewport.x, y: view.viewport.y, z: 0 };
        render_context.command_encoder().copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo { texture: &c.out, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyTextureInfo { texture: write.destination_texture, mip_level: 0, origin: at, aspect: wgpu::TextureAspect::All },
            wgpu::Extent3d { width: c.out_size.x, height: c.out_size.y, depth_or_array_layers: 1 },
        );
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn ffx_dispatch(ffx: &Ffx, ctx: &mut *mut c_void, cmd: ash::vk::CommandBuffer, c: &UpscaleCtx, fsr: &Upscale, render: UVec2, jitter: Vec2,
                       view: &ExtractedView, world: &World) -> u32 {
    let res = |t: &wgpu::Texture, format, usage, state| Resource {
        resource: unsafe { t.as_hal::<Vulkan>().unwrap().raw_handle() }.as_raw() as *mut c_void,
        ty: TEXTURE2D, format, width: t.width(), height: t.height(), depth: 1, mips: 1, flags: 0, usage, state,
    };
    let (near, fov_y) = (view.clip_from_view.w_axis.z, 2.0 * (1.0 / view.clip_from_view.y_axis.y).atan());
    let desc = DispatchUpscale {
        header: Header { ty: TYPE_DISPATCH_UPSCALE, next: null_mut() },
        command_list: cmd.as_raw() as *mut c_void,
        color: res(&c.color, RGBA16F, 0, STATE_COMPUTE_READ),
        depth: res(&c.depth, R32F, 0, STATE_COMPUTE_READ),
        motion_vectors: res(&c.mv, RG32F, 0, STATE_COMPUTE_READ),
        exposure: NONE,
        reactive: NONE,
        transparency: NONE,
        output: res(&c.out, RGBA16F, USAGE_UAV, STATE_UAV),
        jitter: jitter.to_array(),
        mv_scale: (-render.as_vec2()).to_array(),
        render_size: Dims { w: render.x, h: render.y },
        upscale_size: Dims { w: c.out_size.x, h: c.out_size.y },
        sharpen: fsr.sharpness > 0.0,
        sharpness: fsr.sharpness,
        frame_ms: world.get_resource::<Time>().map_or(16.7, |t| t.delta_secs() * 1000.0),
        pre_exposure: 1.0,
        reset: false,
        near: f32::MAX,
        far: near,
        fov_y,
        meters: 1.0,
        flags: 0,
    };
    unsafe { (ffx.dispatch)(ctx, &desc.header) }
}

unsafe fn xess_dispatch(x: &Xess, ctx: *mut c_void, cmd: ash::vk::CommandBuffer, c: &UpscaleCtx, render: UVec2, jitter: Vec2) -> i32 {
    use ash::vk::Format as F;
    let info = |i: usize, t: &wgpu::Texture, format| XessView {
        view: unsafe { c.views[i].as_hal::<Vulkan>().unwrap().raw_handle() },
        image: unsafe { t.as_hal::<Vulkan>().unwrap().raw_handle() },
        range: ash::vk::ImageSubresourceRange { aspect_mask: ash::vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1 },
        format,
        width: t.width(),
        height: t.height(),
    };
    let params = XessExecute {
        color: info(0, &c.color, F::R16G16B16A16_SFLOAT),
        velocity: info(2, &c.mv, F::R32G32_SFLOAT),
        depth: info(1, &c.depth, F::R32_SFLOAT),
        exposure_scale: default(),
        responsive: default(),
        output: info(3, &c.out, F::R16G16B16A16_SFLOAT),
        jitter: (XESS_JITTER_SIGN * jitter).to_array(),
        exposure: 1.0,
        reset: 0,
        input_size: X2 { x: render.x, y: render.y },
        bases: default(),
    };
    unsafe { (x.execute)(ctx, cmd, &params) }
}

const JITTER_SIGN: f32 = -1.0;
const XESS_JITTER_SIGN: f32 = 1.0;

