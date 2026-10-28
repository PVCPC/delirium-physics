import bevy_render::view::View;
import bevy_pbr::prepass::bindings::PreviousViewUniforms;
import bevy_core_pipeline::tonemapping::tonemapping_luminance;
import package::scene::sampling::{LightSample, NULL_LIGHT_ID, sample_environment_map_light};

enable wgpu_ray_query;

@group(1) @binding(0) var view_output: texture_storage_2d<rgba16float, read_write>;
@group(1) @binding(1) var<storage, read_write> light_tile_samples: array<LightSample>;
@group(1) @binding(2) var<storage, read_write> light_tile_resolved_samples: array<ResolvedLightSamplePacked>;
@group(1) @binding(3) var gbuffer: texture_2d<u32>;
@group(1) @binding(4) var depth_buffer: texture_depth_2d;
@if(MOTION_VECTORS_READ_WRITE)
@group(1) @binding(5) var motion_vectors: texture_storage_2d<rg16float, read_write>;
@if(!MOTION_VECTORS_READ_WRITE)
@group(1) @binding(5) var motion_vectors: texture_2d<f32>;
@group(1) @binding(6) var<uniform> view: View;
@group(1) @binding(7) var<uniform> previous_view: PreviousViewUniforms;
@group(1) @binding(8) var<storage, read_write> world_cache: WorldCache;
@group(1) @binding(9) var<uniform> constants: SolariLightingSettings;

@if(RESTIR) {
@group(1) @binding(10) var previous_gbuffer: texture_2d<u32>;
@group(1) @binding(11) var previous_depth_buffer: texture_depth_2d;
@group(1) @binding(12) var<storage, read_write> reservoirs_a: array<Reservoir>;
@group(1) @binding(13) var<storage, read_write> reservoirs_b: array<Reservoir>;
}

@if(DLSS_RR_GUIDE_BUFFERS) {
@group(2) @binding(0) var diffuse_albedo: texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(1) var specular_albedo: texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(2) var normal_roughness: texture_storage_2d<rgba16float, write>;
@group(2) @binding(3) var dlss_rr_depth: texture_storage_2d<r32float, write>;
@group(2) @binding(4) var specular_motion_vectors: texture_storage_2d<rg16float, write>;
}

/// Delirium: a pixel of the view in the window-sized screen textures (the view's viewport is off the corner).
fn screen(pixel_id: vec2<u32>) -> vec2<u32> {
    return pixel_id + vec2<u32>(view.main_pass_viewport.xy);
}

fn load_motion_vector(pixel_id: vec2<u32>) -> vec2<f32> {
    @if(MOTION_VECTORS_READ_WRITE)
    return textureLoad(motion_vectors, screen(pixel_id)).xy;
    @if(!MOTION_VECTORS_READ_WRITE)
    return textureLoad(motion_vectors, screen(pixel_id), 0).xy;
}

/// User-configurable settings from the `SolariLighting` component, plus per-frame
/// state. Field order and types must match `SolariLightingUniforms` in `prepare.rs`.
struct SolariLightingSettings {
    confidence_weight_cap: f32,
    primary_di_samples: u32,
    secondary_di_samples: u32,
    max_bounces: u32,
    world_cache_max_temporal_samples: f32,
    world_cache_direct_light_sample_count: u32,
    world_cache_max_gi_ray_distance: f32,
    world_cache_cell_updates_soft_target: u32,
    world_cache_position_base_cell_size: f32,
    world_cache_position_lod_scale: f32,
    frame_rng: u32,
    reset: u32,
    water_fog: vec4<f32>,
    water_fog_color: vec4<f32>,
}

// Delirium: how much of the way from `a` to `b` is under the water's surface (m).
fn in_water(a: vec3<f32>, b: vec3<f32>) -> f32 {
    let s = constants.water_fog_color.w;
    let lo = min(a.z, b.z);
    let hi = max(a.z, b.z);
    return distance(a, b) * select(clamp((s - lo) / (hi - lo), 0.0, 1.0), select(0.0, 1.0, lo < s), hi - lo < 1e-4);
}

// Delirium: what gets through the water on the way from `a` to `b`.
fn water_through(a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return exp(-constants.water_fog.xyz * in_water(a, b));
}

// Delirium: water (`SolariLighting::water_fog`: what a metre soaks up xyz, w 1 with the eye under it; `water_fog_color.w` its
// surface's height). As the path tracer: the light's way down through it to what's under it soaked up, and under it the
// haze between the eye and what's seen.
fn water_fog(color: vec3<f32>, world_position: vec3<f32>) -> vec3<f32> {
    let sigma = constants.water_fog.xyz;
    if all(sigma == vec3(0.0)) { return color; }
    let s = constants.water_fog_color.w;
    // (Its way down slanted, ~1.3 times the depth.) ponytail: anything below the surface's height counts as in it; pass the
    // pool's bounds if something outside it ever sits that low.
    var c = color * exp(-sigma * max(s - world_position.z, 0.0) * 1.3);
    if constants.water_fog.w == 0.0 { return c; }
    let d = in_water(world_position, view.world_position);
    // (Soaked up, and the sky's light scattered in from a few metres down.)
    let through = exp(-d * sigma);
    let glow = exp(-sigma * 5.0) * tonemapping_luminance(sample_environment_map_light(vec3(0.0, 0.0, 1.0))) * 0.3;
    return c * through + (1.0 - through) * glow * view.exposure;
}

/// Don't adjust the size of this struct without also adjusting `prepare::RESOLVED_LIGHT_SAMPLE_STRUCT_SIZE`.
struct ResolvedLightSamplePacked {
    world_position_x: f32,
    world_position_y: f32,
    world_position_z: f32,
    world_normal: u32,
    radiance: u32,
    inverse_pdf: f32,
}

/// Don't adjust the size of this struct without also adjusting `prepare::RESERVOIR_STRUCT_SIZE`.
struct Reservoir {
    sample_point_world_position: vec3<f32>,
    unbiased_contribution_weight: f32,
    radiance: vec3<f32>,
    confidence_weight: f32,
    sample_point_world_normal: vec2<f32>,
    light_sample: LightSample,
}

fn empty_reservoir() -> Reservoir {
    return Reservoir(
        vec3(0.0),
        0.0,
        vec3(0.0),
        0.0,
        vec2(0.0),
        LightSample(NULL_LIGHT_ID, 0u),
    );
}

struct WorldCacheGeometryData {
    world_position: vec3<f32>,
    padding_a: u32,
    world_normal: vec3<f32>,
    padding_b: u32,
}

/// Keep in sync with the packed world-cache layout constants in `prepare.rs`.
struct WorldCache {
    checksums: array<atomic<u32>, constants::WORLD_CACHE_SIZE>,
    @if(WORLD_CACHE_NON_ATOMIC_LIFE_BUFFER)
    life: array<u32, constants::WORLD_CACHE_SIZE>,
    @if(!WORLD_CACHE_NON_ATOMIC_LIFE_BUFFER)
    life: array<atomic<u32>, constants::WORLD_CACHE_SIZE>,
    radiance: array<vec4<f32>, constants::WORLD_CACHE_SIZE>,
    geometry_data: array<WorldCacheGeometryData, constants::WORLD_CACHE_SIZE>,
    luminance_deltas: array<f32, constants::WORLD_CACHE_SIZE>,
    active_cells_new_radiance: array<vec3<f32>, constants::WORLD_CACHE_SIZE>,
    a: array<u32, constants::WORLD_CACHE_SIZE>,
    b: array<u32, 1024u>,
    active_cell_indices: array<u32, constants::WORLD_CACHE_SIZE>,
    active_cells_count: u32,
}
