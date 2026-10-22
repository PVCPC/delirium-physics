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
pub struct Fsr3 {
    pub mode: u8,
    pub sharpness: f32,
}

#[derive(Resource)]
pub struct Fsr3Supported;

const RATIO: [f32; 5] = [1.0, 1.5, 1.7, 2.0, 3.0];

pub struct Fsr3Plugin;

impl Plugin for Fsr3Plugin {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        let device = app.sub_app(RenderApp).world().resource::<RenderDevice>().wgpu_device().clone();
        let Some(lib) = Ffx::load(&device) else {
            info!("FSR 3: not on this system (needs amd_fidelityfx_vk.dll and Vulkan)");
            return;
        };
        let copy = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fsr3_copy_in"),
            layout: None,
            module: &device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("fsr3_copy_in"), source: wgpu::ShaderSource::Wgsl(COPY_IN.into()) }),
            entry_point: Some("main"),
            compilation_options: default(),
            cache: None,
        });
        app.insert_resource(Fsr3Supported);
        app.sub_app_mut(RenderApp)
            .insert_resource(FfxLib(Arc::new(lib), copy))
            .add_systems(ExtractSchedule, extract)
            .add_systems(Render, prepare.in_set(RenderSystems::ManageViews).before(prepare_view_targets))
            .add_render_graph_node::<ViewNodeRunner<Fsr3Node>>(Core3d, Fsr3Label)
            .add_render_graph_edges(Core3d, (Node3d::MotionBlur, Fsr3Label, Node3d::Bloom));
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

unsafe extern "C" fn message(ty: u32, text: *const u16) {
    let n = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
    warn!("FSR 3 ({ty}): {}", String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, n) }));
}

#[derive(Resource)]
struct FfxLib(Arc<Ffx>, wgpu::ComputePipeline);

#[derive(Component)]
struct Fsr3Ctx {
    ffx: Arc<Ffx>,
    ctx: Mutex<Ctx>,
    mode: u8,
    out_size: UVec2,
    color: wgpu::Texture,
    depth: wgpu::Texture,
    mv: wgpu::Texture,
    out: wgpu::Texture,
    at: wgpu::Buffer,
}
struct Ctx(*mut c_void);
unsafe impl Send for Ctx {}

impl Drop for Fsr3Ctx {
    fn drop(&mut self) {
        unsafe {
            if let Some(hal) = self.ffx.device.as_hal::<Vulkan>() {
                let _ = hal.raw_device().device_wait_idle();
            }
            (self.ffx.destroy)(&mut self.ctx.get_mut().unwrap().0, std::ptr::null());
        }
    }
}

fn extract(mut commands: Commands, mut main: ResMut<MainWorld>, had: Query<Has<Fsr3>>) {
    let mut cams = main.query_filtered::<(RenderEntity, &Camera, &Projection, Option<&Fsr3>), With<Hdr>>();
    for (e, camera, projection, fsr) in cams.iter(&main) {
        let mut c = commands.entity(e);
        match fsr {
            Some(fsr) if camera.is_active && projection.is_perspective() => {
                c.insert(fsr.clone());
            }
            _ if had.get(e) == Ok(true) => {
                c.remove::<(Fsr3, Fsr3Ctx, MainPassResolutionOverride)>();
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

fn prepare(mut views: Query<(Entity, &ExtractedView, &Fsr3, &mut CameraMainTextureUsages, &mut TemporalJitter, &mut MipBias, Option<&Fsr3Ctx>)>,
           lib: Res<FfxLib>, device: Res<RenderDevice>, queue: Res<RenderQueue>, frame: Res<FrameCount>, mut commands: Commands) {
    for (e, view, fsr, mut usages, mut jitter, mut bias, ctx) in &mut views {
        usages.0 |= wgpu::TextureUsages::COPY_DST;
        let out_size = view.viewport.zw();
        let mode = fsr.mode.min(4);
        let render = (out_size.as_vec2() / RATIO[mode as usize]).round().as_uvec2().max(UVec2::ONE);
        let at = [view.viewport.x, view.viewport.y, render.x, render.y];
        match ctx {
            Some(c) if c.out_size == out_size && c.mode == mode => queue.write_buffer(&c.at, 0, bytes(&at)),
            _ => {
                let Some(c) = make(&lib.0, &device, render, out_size, mode) else { continue };
                queue.write_buffer(&c.at, 0, bytes(&at));
                commands.entity(e).insert((c, MainPassResolutionOverride(render)));
            }
        }
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

fn make(ffx: &Arc<Ffx>, device: &RenderDevice, render: UVec2, out_size: UVec2, mode: u8) -> Option<Fsr3Ctx> {
    let image = |label, size: UVec2, format, usage| device.wgpu_device().create_texture(&wgpu::TextureDescriptor {
        label: Some(label), size: wgpu::Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[],
    });
    use wgpu::{TextureFormat as F, TextureUsages as U};
    let input = U::STORAGE_BINDING | U::TEXTURE_BINDING;
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
    Some(Fsr3Ctx {
        ffx: ffx.clone(),
        ctx: Mutex::new(Ctx(ctx)),
        mode,
        out_size,
        color: image("fsr3_color", render, F::Rgba16Float, input),
        depth: image("fsr3_depth", render, F::R32Float, input),
        mv: image("fsr3_mv", render, F::Rg32Float, input),
        out: image("fsr3_out", out_size, F::Rgba16Float, U::STORAGE_BINDING | U::TEXTURE_BINDING | U::COPY_SRC),
        at: device.wgpu_device().create_buffer(&wgpu::BufferDescriptor { label: Some("fsr3_at"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }),
    })
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct Fsr3Label;

#[derive(Default)]
struct Fsr3Node;

impl ViewNode for Fsr3Node {
    type ViewQuery = (&'static Fsr3, &'static Fsr3Ctx, &'static MainPassResolutionOverride, &'static TemporalJitter, &'static ViewTarget,
                      &'static ViewPrepassTextures, &'static ExtractedView);

    fn run(&self, _: &mut RenderGraphContext, render_context: &mut RenderContext, (fsr, c, render, jitter, target, prepass, view): QueryItem<Self::ViewQuery>,
           world: &World) -> Result<(), NodeRunError> {
        let (Some(depth), Some(mv)) = (&prepass.depth, &prepass.motion_vectors) else { return Ok(()) };
        let lib = world.resource::<FfxLib>();
        let write = target.post_process_write();
        let render = render.0;
        let view_of = |t: &wgpu::Texture| t.create_view(&default());
        let bind = render_context.render_device().wgpu_device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fsr3_copy_in"),
            layout: &lib.1.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(write.source) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&depth.texture.default_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&mv.texture.default_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&view_of(&c.color)) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&view_of(&c.depth)) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&view_of(&c.mv)) },
                wgpu::BindGroupEntry { binding: 6, resource: c.at.as_entire_binding() },
            ],
        });
        let encoder = render_context.command_encoder();
        encoder.push_debug_group("fsr3");
        {
            let mut pass = encoder.begin_compute_pass(&default());
            pass.set_pipeline(&lib.1);
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
        let res = |t: &wgpu::Texture, format, usage, state| Resource {
            resource: unsafe { t.as_hal::<Vulkan>().unwrap().raw_handle() }.as_raw() as *mut c_void,
            ty: TEXTURE2D, format, width: t.width(), height: t.height(), depth: 1, mips: 1, flags: 0, usage, state,
        };
        let (near, fov_y) = (view.clip_from_view.w_axis.z, 2.0 * (1.0 / view.clip_from_view.y_axis.y).atan());
        let mut desc = DispatchUpscale {
            header: Header { ty: TYPE_DISPATCH_UPSCALE, next: null_mut() },
            command_list: null_mut(),
            color: res(&c.color, RGBA16F, 0, STATE_COMPUTE_READ),
            depth: res(&c.depth, R32F, 0, STATE_COMPUTE_READ),
            motion_vectors: res(&c.mv, RG32F, 0, STATE_COMPUTE_READ),
            exposure: NONE,
            reactive: NONE,
            transparency: NONE,
            output: res(&c.out, RGBA16F, USAGE_UAV, STATE_UAV),
            jitter: (JITTER_SIGN * jitter.offset).to_array(),
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
        encoder.pop_debug_group();
        let mut own = render_context.render_device().wgpu_device().create_command_encoder(&default());
        let mut ctx = c.ctx.lock().unwrap();
        let r = unsafe {
            own.as_hal_mut::<Vulkan, _, _>(|e| {
                desc.command_list = e.unwrap().raw_handle().as_raw() as *mut c_void;
                (c.ffx.dispatch)(&mut ctx.0, &desc.header)
            })
        };
        render_context.add_command_buffer(own.finish());
        if r != 0 {
            error!("FSR 3: dispatch failed ({r})");
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

const JITTER_SIGN: f32 = -1.0;
