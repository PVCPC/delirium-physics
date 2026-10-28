import bevy_pbr::render::pbr_functions::calculate_tbn_mikktspace;
import bevy_render::maths::{affine3_to_square, orthonormalize};

enable wgpu_ray_query;
enable wgpu_binding_array;

struct InstanceGeometryIds {
    vertex_buffer_id: u32,
    vertex_buffer_offset: u32,
    index_buffer_id: u32,
    index_buffer_offset: u32,
    triangle_count: u32,
}

struct VertexBuffer { vertices: array<PackedVertex> }

struct IndexBuffer { indices: array<u32> }

struct PackedVertex {
    a: vec4<f32>,
    b: vec4<f32>,
    tangent: vec4<f32>,
}

struct Vertex {
    position: vec3<f32>,
    normal: vec3<f32>,
    uv: vec2<f32>,
    tangent: vec4<f32>,
}

fn unpack_vertex(packed: PackedVertex) -> Vertex {
    var vertex: Vertex;
    vertex.position = packed.a.xyz;
    vertex.normal = vec3(packed.a.w, packed.b.xy);
    vertex.uv = packed.b.zw;
    vertex.tangent = packed.tangent;
    return vertex;
}

struct Material {
    normal_map_texture_id: u32,
    base_color_texture_id: u32,
    emissive_texture_id: u32,
    metallic_roughness_texture_id: u32,

    base_color: vec3<f32>,
    perceptual_roughness: f32,
    emissive: vec3<f32>,
    metallic: f32,
    glass: f32,
    ior: f32,
    alpha_cutoff: f32,
    reflectance: f32,
    absorption: vec3<f32>,
    clear_light: f32,
}

const TEXTURE_MAP_NONE = 0xFFFFFFFFu;

const MIRROR_ROUGHNESS_THRESHOLD = 0.001f;

struct LightSource {
    kind: u32, // 1 bit for kind, 31 bits for extra data
    id: u32,
}

const LIGHT_SOURCE_KIND_EMISSIVE_MESH = 0u;
const LIGHT_SOURCE_KIND_DIRECTIONAL = 1u;

struct DirectionalLight {
    direction_to_light: vec3<f32>,
    cos_theta_max: f32,
    luminance: vec3<f32>,
    inverse_pdf: f32,
}

struct EnvironmentMapLight {
    light_from_world: mat3x3<f32>,
    intensity: f32,
}

const LIGHT_NOT_PRESENT_THIS_FRAME = 0xFFFFFFFFu;

@group(0) @binding(0) var<storage> vertex_buffers: binding_array<VertexBuffer>;
@group(0) @binding(1) var<storage> index_buffers: binding_array<IndexBuffer>;
@group(0) @binding(2) var textures: binding_array<texture_2d<f32>>;
@group(0) @binding(3) var samplers: binding_array<sampler>;
@group(0) @binding(4) var<storage> materials: array<Material>;
@group(0) @binding(5) var tlas: acceleration_structure;
@group(0) @binding(6) var previous_frame_tlas: acceleration_structure;
@group(0) @binding(7) var<storage> transforms: array<mat3x4<f32>>;
@group(0) @binding(8) var<storage> previous_frame_transforms: array<mat3x4<f32>>;
@group(0) @binding(9) var<storage> geometry_ids: array<InstanceGeometryIds>;
@group(0) @binding(10) var<storage> material_ids: array<u32>;
@group(0) @binding(11) var<storage> light_sources: array<LightSource>;
@group(0) @binding(12) var<storage> directional_lights: array<DirectionalLight>;
@group(0) @binding(13) var<storage> previous_frame_light_id_translations: array<u32>;
@group(0) @binding(14) var brdf_dfg_lut: texture_2d<f32>;
@group(0) @binding(15) var brdf_dfg_lut_sampler: sampler;
@group(0) @binding(16) var environment_map: texture_cube<f32>;
@group(0) @binding(17) var environment_map_sampler: sampler;
@group(0) @binding(18) var<storage> environment_map_light: EnvironmentMapLight;

// Delirium: the sun's caustics: photons shot down from it through water (pathtracer's caustics.wesl), what lands summed
// in cells on what it lands on (hashed by where, how big, and which way the surface faces), its light over the sun's.
// The photons' square (in the sun's plane, round the camera): its centre, half its side, its tiles' side, its axes.
struct CausticHeader {
    on: u32,
    sun: u32,
    half: f32,
    tile: f32,
    center: vec4<f32>,
    cam: vec4<f32>,
    u: vec4<f32>,
    v: vec4<f32>,
}

struct CausticCell {
    checksum: atomic<u32>,
    flux: array<atomic<u32>, 3>,
    light: vec3<f32>,
    idle: u32,
}

struct Caustics {
    header: CausticHeader,
    tiles: array<u32, 65536>,
    cells: array<CausticCell>,
}

@group(0) @binding(19) var<storage, read_write> caustics: Caustics;

const CAUSTIC_CELLS = 2097152u;
// (A cell's sums' fixed point.)
const CAUSTIC_SCALE = 65536.0;

// Whether the photons light `p`: inside the square, short of its edge (photons coming in from past it).
fn caustic_owns(p: vec3<f32>) -> bool {
    let h = caustics.header;
    if h.on == 0u { return false; }
    let d = p - h.center.xyz;
    return max(abs(dot(d, h.u.xyz)), abs(dot(d, h.v.xyz))) < 0.85 * h.half;
}

// A cell's side at `p`: 2 cm near the camera, doubling every so far off.
fn caustic_cell_size(p: vec3<f32>) -> f32 {
    return 0.02 * exp2(floor(log2(1.0 + distance(p, caustics.header.cam.xyz) / 16.0)));
}

// Which way a surface faces most (an axis' each side).
fn caustic_axis(n: vec3<f32>) -> u32 {
    let a = abs(n);
    let i = select(select(2u, 1u, a.y >= a.z), 0u, a.x >= a.y && a.x >= a.z);
    return i * 2u + select(0u, 1u, n[i] < 0.0);
}

// The cell for `p` on a surface facing `n` (`create`: made if it isn't there); -1 when there's no room.
fn caustic_slot(p: vec3<f32>, n: vec3<f32>, s: f32, create: bool) -> i32 {
    let q = bitcast<vec3<u32>>(vec3<i32>(floor(p / s)));
    let a = caustic_axis(n) + bitcast<u32>(s);
    var key = caustic_hash(caustic_hash(caustic_hash(caustic_hash(q.x) + q.y) + q.z) + a);
    let sum = max(caustic_hash2(caustic_hash2(caustic_hash2(caustic_hash2(q.x) + q.y) + q.z) + a), 1u);
    for (var i = 0u; i < 4u; i++) {
        key &= CAUSTIC_CELLS - 1u;
        if create {
            let cas = atomicCompareExchangeWeak(&caustics.cells[key].checksum, 0u, sum);
            if cas.old_value == sum || cas.exchanged { return i32(key); }
        } else if atomicLoad(&caustics.cells[key].checksum) == sum {
            return i32(key);
        }
        key += 1u;
    }
    return -1;
}

// The caustics' light at `p` on a surface facing `n` (over the sun's straight on), looked up `xi` (0-1) off it
// within a cell (blurs the cells' squares away).
fn caustic_light(p: vec3<f32>, n: vec3<f32>, xi: vec2<f32>) -> vec3<f32> {
    if !caustic_owns(p) { return vec3(0.0); }
    let s = caustic_cell_size(p);
    let tbn = orthonormalize(n);
    let k = caustic_slot(p + (tbn[0] * (xi.x - 0.5) + tbn[1] * (xi.y - 0.5)) * s, n, s, false);
    if k < 0 { return vec3(0.0); }
    return caustics.cells[k].light;
}

// The sun's light the photons bring to `p` (facing `n`), before exposure; none when they're off.
fn caustic_sun(p: vec3<f32>, n: vec3<f32>, xi: vec2<f32>) -> vec3<f32> {
    if caustics.header.on == 0u { return vec3(0.0); }
    let sun = directional_lights[caustics.header.sun];
    return caustic_light(p, n, xi) * sun.luminance * sun.inverse_pdf;
}

fn caustic_hash(input: u32) -> u32 {
    let state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn caustic_hash2(input: u32) -> u32 {
    let n = (input << 13u) ^ input;
    return n * (n * n * 15731u + 789221u) + 1376312589u;
}

// Whether shadow rays leave water to the photons (the first surface's sun, where they light).
var<private> caustic_photons: bool = false;

// Delirium: textures' mips by the ray's cone (ray cones). Its width at the ray's start and its spread (m a metre of
// the way; zero: the top mip, Solari's own way), the way the ray goes, and how far it went to the hit being resolved
// (-1: not a traced hit's); the mip its footprint there takes.
var<private> ray_cone: vec2<f32>;
var<private> ray_cone_dir: vec3<f32>;
var<private> cone_t: f32 = -1.0;
var<private> mip: f32;

const RAY_T_MIN = 0.001f;
const RAY_T_MAX = 100000.0f;

const RAY_NO_CULL = 0xFFu;

// Delirium: as `trace_ray`, but on through glass (water: ray-traced lighting's bounces don't bend through it).
fn trace_ray_past_glass(ray_origin: vec3<f32>, ray_direction: vec3<f32>, ray_t_min: f32, ray_t_max: f32) -> RayIntersection {
    var t = ray_t_min;
    var ray = trace_ray(ray_origin, ray_direction, t, ray_t_max, RAY_FLAG_NONE);
    for (var i = 0u; i < 8u && ray.kind != RAY_QUERY_INTERSECTION_NONE && materials[material_ids[ray.instance_custom_data]].glass > 0.0; i++) {
        t = ray.t + RAY_T_MIN;
        ray = trace_ray(ray_origin, ray_direction, t, ray_t_max, RAY_FLAG_NONE);
    }
    return ray;
}

fn trace_ray(ray_origin: vec3<f32>, ray_direction: vec3<f32>, ray_t_min: f32, ray_t_max: f32, ray_flag: u32) -> RayIntersection {
    var t = ray_t_min;
    var hit: RayIntersection;
    // Delirium: on through a cut-out's holes (up to 8 of them).
    for (var i = 0u; i < 8u; i++) {
        let ray = RayDesc(ray_flag, RAY_NO_CULL, t, ray_t_max, ray_origin, ray_direction);
        var rq: ray_query;
        rayQueryInitialize(&rq, tlas, ray);
        rayQueryProceed(&rq);
        hit = rayQueryGetCommittedIntersection(&rq);
        if hit.kind == RAY_QUERY_INTERSECTION_NONE || !cut_out(hit) { break; }
        t = hit.t + RAY_T_MIN;
    }
    return hit;
}

// Delirium: whether a hit's in a cut-out's hole (its colour texture's alpha below the material's cutoff).
fn cut_out(hit: RayIntersection) -> bool {
    let material = materials[material_ids[hit.instance_custom_data]];
    if material.alpha_cutoff <= 0.0 || material.base_color_texture_id == TEXTURE_MAP_NONE { return false; }
    let vertices = load_vertices(geometry_ids[hit.instance_custom_data], hit.primitive_index);
    let uv = mat3x2(vertices[0].uv, vertices[1].uv, vertices[2].uv) * vec3(1.0 - hit.barycentrics.x - hit.barycentrics.y, hit.barycentrics);
    let id = material.base_color_texture_id;
    return textureSampleLevel(textures[id], samplers[id], uv, 0.0).a < material.alpha_cutoff;
}

fn trace_ray_previous_frame(ray_origin: vec3<f32>, ray_direction: vec3<f32>, ray_t_min: f32, ray_t_max: f32, ray_flag: u32) -> RayIntersection {
    let ray = RayDesc(ray_flag, RAY_NO_CULL, ray_t_min, ray_t_max, ray_origin, ray_direction);
    var rq: ray_query;
    rayQueryInitialize(&rq, previous_frame_tlas, ray);
    rayQueryProceed(&rq);
    return rayQueryGetCommittedIntersection(&rq);
}

// Delirium: what a candidate hit lets through: nothing off an opaque surface, all through a cut-out's hole; glass its colour, the more the more opaque the
// texel (a stained pane's frame darkest), on the way in only so a block tints once.
// Delirium: a candidate hit's smooth normal (world).
fn hit_normal(hit: RayIntersection) -> vec3<f32> {
    let vertices = load_vertices(geometry_ids[hit.instance_custom_data], hit.primitive_index);
    let barycentrics = vec3(1.0 - hit.barycentrics.x - hit.barycentrics.y, hit.barycentrics);
    let transform = affine3_to_square(transforms[hit.instance_custom_data]);
    return mat3x3(transform[0].xyz, transform[1].xyz, transform[2].xyz) * (mat3x3(vertices[0].normal, vertices[1].normal, vertices[2].normal) * barycentrics);
}

// Delirium: how a candidate hit's smooth normal turns along the surface (its x and y over the ground's x and y). The real
// water's top (its UVs 2) has it in its tangents, smooth; elsewhere a triangle's own.
fn normal_turn(hit: RayIntersection) -> mat2x2<f32> {
    let vertices = load_vertices(geometry_ids[hit.instance_custom_data], hit.primitive_index);
    if vertices[0].uv.x > 1.5 {
        let b = vec3(1.0 - hit.barycentrics.x - hit.barycentrics.y, hit.barycentrics);
        let t = vertices[0].tangent * b.x + vertices[1].tangent * b.y + vertices[2].tangent * b.z;
        return mat2x2<f32>(t.xy, t.zw);
    }
    let transform = affine3_to_square(transforms[hit.instance_custom_data]);
    let w = transform_positions(transform, vertices);
    let r = mat3x3(transform[0].xyz, transform[1].xyz, transform[2].xyz);
    let n0 = (r * vertices[0].normal).xy;
    let edges = mat2x2<f32>((w[1] - w[0]).xy, (w[2] - w[0]).xy);
    let turns = mat2x2<f32>((r * vertices[1].normal).xy - n0, (r * vertices[2].normal).xy - n0);
    if abs(determinant(edges)) < 1e-8 { return mat2x2<f32>(vec2(0.0), vec2(0.0)); }
    return turns * inverse2(edges);
}

fn inverse2(m: mat2x2<f32>) -> mat2x2<f32> {
    return mat2x2<f32>(vec2(m[1].y, -m[0].y), vec2(-m[1].x, m[0].x)) * (1.0 / determinant(m));
}

fn glass_filter(hit: RayIntersection, ray_direction: vec3<f32>) -> vec3<f32> {
    let instance_id = hit.instance_custom_data;
    let material = materials[material_ids[instance_id]];
    if cut_out(hit) { return vec3(1.0); }
    if material.glass == 0.0 { return vec3(0.0); }
    let vertices = load_vertices(geometry_ids[instance_id], hit.primitive_index);
    let barycentrics = vec3(1.0 - hit.barycentrics.x - hit.barycentrics.y, hit.barycentrics);
    if dot(hit_normal(hit), ray_direction) > 0.0 {
        // Delirium: up out of water: the caustics where the ray starts (water.rs's `Vol`).
        // (Not with photons: the map's the water's slopes then.)
        if any(material.absorption > vec3(0.0)) && material.metallic_roughness_texture_id != TEXTURE_MAP_NONE && caustics.header.on == 0u {
            return vec3(water_caustic(instance_id, hit.primitive_index, barycentrics, ray_direction, hit.t));
        }
        return vec3(1.0);
    }
    // (Into water: what it soaks up on the way through, `trace_transmittance`'s, not a tint.)
    if any(material.absorption > vec3(0.0)) || material.clear_light > 0.0 { return vec3(1.0); }
    var c = vec4(material.base_color, 1.0);
    if material.base_color_texture_id != TEXTURE_MAP_NONE {
        let uv = mat3x2(vertices[0].uv, vertices[1].uv, vertices[2].uv) * barycentrics;
        c *= textureSampleLevel(textures[material.base_color_texture_id], samplers[material.base_color_texture_id], uv, 0.0);
    }
    return mix(vec3(1.0), c.rgb, c.a);
}

// Delirium: the caustics' light (over flat water's) at the point `t` back along `to_light` from the water's surface,
// from its layers through the water (water.rs's `Vol`). 1 on the pool's floor and walls: their own caustics' theirs.
fn water_caustic(instance_id: u32, triangle_id: u32, barycentrics: vec3<f32>, to_light: vec3<f32>, t: f32) -> f32 {
    let material = materials[material_ids[instance_id]];
    let id = material.metallic_roughness_texture_id;
    let vertices = load_vertices(geometry_ids[instance_id], triangle_id);
    let w = transform_positions(affine3_to_square(transforms[instance_id]), vertices);
    let uvs = mat3x2(vertices[0].uv, vertices[1].uv, vertices[2].uv);
    // The surface's metres a whole uv, and the point's uv (straight down the layers' grid from where it'd be under).
    let e1 = w[1] - w[0];
    let e2 = w[2] - w[0];
    let d1 = uvs[1] - uvs[0];
    let d2 = uvs[2] - uvs[0];
    let r = 1.0 / (d1.x * d2.y - d1.y * d2.x);
    let dpdu = ((e1 * d2.y - e2 * d1.y) * r).xy;
    let dpdv = ((e2 * d1.x - e1 * d2.x) * r).xy;
    let back = -to_light.xy * t;
    let k = 1.0 / (dpdu.x * dpdv.y - dpdu.y * dpdv.x);
    let uv = uvs * barycentrics + vec2(dpdv.y * back.x - dpdv.x * back.y, dpdu.x * back.y - dpdu.y * back.x) * k;
    let depth = t * to_light.z;
    let size = vec2<f32>(textureDimensions(textures[id]));
    let deep = textureSampleLevel(textures[id], samplers[id], vec2(0.5), 0.0).a * 255.0 * 0.02;
    let edge = 0.03 / vec2(length(dpdu), length(dpdv));
    if depth > deep - 0.03 || any(uv < edge) || any(uv > 1.0 - edge) { return 1.0; }
    // (The layers each as tall as the pool's long over its wide.)
    let layers = max(round(size.y / (size.x * length(dpdv) / length(dpdu))), 1.0);
    let f = depth / deep * layers - 1.0;
    let below = water_layer(id, uv, max(floor(f), 0.0), layers, size.y);
    if f < 0.0 { return mix(1.0, below, depth / (deep / layers)); }
    return mix(below, water_layer(id, uv, min(floor(f) + 1.0, layers - 1.0), layers, size.y), fract(f));
}

fn water_layer(id: u32, uv: vec2<f32>, k: f32, layers: f32, height: f32) -> f32 {
    let half = 0.5 * layers / height;
    let v = (k + clamp(uv.y, half, 1.0 - half)) / layers;
    return textureSampleLevel(textures[id], samplers[id], vec2(uv.x, v), 0.0).r * 4.0;
}

// Delirium: what reaches the end of a shadow ray (this frame's scene or the last's): the glass on the way's tint, none past
// anything opaque, and what water soaks up on its way through (its metres: where it leaves less where it goes in, in any
// order; started in it, from the start). Any hit, everything a candidate (`glass_filter` sorts them).
fn trace_transmittance(ray_origin: vec3<f32>, ray_direction: vec3<f32>, ray_t_max: f32, previous_frame: bool) -> vec3<f32> {
    let ray = RayDesc(RAY_FLAG_FORCE_NO_OPAQUE | RAY_FLAG_TERMINATE_ON_FIRST_HIT, RAY_NO_CULL, RAY_T_MIN, ray_t_max, ray_origin, ray_direction);
    var rq: ray_query;
    if previous_frame {
        rayQueryInitialize(&rq, previous_frame_tlas, ray);
    } else {
        rayQueryInitialize(&rq, tlas, ray);
    }
    var t = vec3(1.0);
    var wet = 0.0;
    var sigma = vec3(0.0);
    // (Where it left the water's wavy top: how far along, and how the normal turns along the surface there.)
    var top_t = -1.0;
    var bend = mat2x2<f32>(vec2(0.0), vec2(0.0));
    // (Water, where the photons light, the photons' to light through: a wall here.)
    let photons = caustic_photons && caustic_owns(ray_origin);
    while rayQueryProceed(&rq) {
        let hit = rayQueryGetCandidateIntersection(&rq);
        if photons {
            let m = materials[material_ids[hit.instance_custom_data]];
            if m.glass > 0.0 && any(m.absorption > vec3(0.0)) && !cut_out(hit) {
                rayQueryConfirmIntersection(&rq);
                continue;
            }
        }
        let f = glass_filter(hit, ray_direction);
        if all(f == vec3(0.0)) {
            rayQueryConfirmIntersection(&rq);
        } else {
            t *= f;
            let m = materials[material_ids[hit.instance_custom_data]];
            if m.glass > 0.0 && any(m.absorption > vec3(0.0)) && !cut_out(hit) {
                sigma = m.absorption;
                let n = hit_normal(hit);
                let out = dot(n, ray_direction) > 0.0;
                wet += select(-hit.t, hit.t, out);
                if out && n.z > 0.5 && m.metallic_roughness_texture_id == TEXTURE_MAP_NONE && hit.t > top_t {
                    top_t = hit.t;
                    bend = normal_turn(hit);
                }
            }
        }
    }
    t *= exp(-sigma * max(wet, 0.0));
    // Caustics: the waves' curves focus the light that came down through them, or spread it, more the further it went
    // on (in water, then in air past a flat bottom, where it bends 1.33 times as much). How much a patch of the surface
    // is squeezed to by the time it lands (`det`), the light over it.
    if top_t > 0.0 {
        let far = wet + 1.333 * max(top_t - wet, 0.0);
        let land = mat2x2<f32>(vec2(1.0, 0.0), vec2(0.0, 1.0)) - far * (1.0 - 1.0 / 1.333) * bend;
        t *= min(1.0 / max(abs(determinant(land)), 0.12), 6.0);
    }
    return select(t, vec3(0.0), rayQueryGetCommittedIntersection(&rq).kind != RAY_QUERY_INTERSECTION_NONE);
}

fn sample_texture(id: u32, uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(textures[id], samplers[id], uv, mip).rgb;
}

struct ResolvedMaterial {
    base_color: vec3<f32>,
    emissive: vec3<f32>,
    reflectance: f32,
    perceptual_roughness: f32,
    roughness: f32,
    metallic: f32,
    // Delirium: glass (0 or 1), its index of refraction, and the colour texture's alpha.
    glass: f32,
    ior: f32,
    alpha: f32,
    // Delirium: glass that's a body (water): what a metre of it soaks up.
    absorption: vec3<f32>,
}

struct ResolvedRayHitFull {
    world_position: vec3<f32>,
    previous_frame_world_position: vec3<f32>,
    world_normal: vec3<f32>,
    geometric_world_normal: vec3<f32>,
    world_tangent: vec4<f32>,
    uv: vec2<f32>,
    triangle_area: f32,
    triangle_count: u32,
    material: ResolvedMaterial,
}

fn resolve_material(material: Material, uv: vec2<f32>) -> ResolvedMaterial {
    var m: ResolvedMaterial;

    m.base_color = material.base_color.rgb;
    m.alpha = 1.0;
    if material.base_color_texture_id != TEXTURE_MAP_NONE {
        let c = textureSampleLevel(textures[material.base_color_texture_id], samplers[material.base_color_texture_id], uv, mip);
        m.base_color *= c.rgb;
        m.alpha = c.a;
    }
    m.glass = material.glass;
    m.ior = material.ior;
    m.absorption = material.absorption;

    m.emissive = material.emissive.rgb;
    if material.emissive_texture_id != TEXTURE_MAP_NONE {
        m.emissive *= sample_texture(material.emissive_texture_id, uv);
    }

    m.reflectance = material.reflectance;

    m.perceptual_roughness = material.perceptual_roughness;
    m.metallic = material.metallic;
    if material.metallic_roughness_texture_id != TEXTURE_MAP_NONE {
        let metallic_roughness = sample_texture(material.metallic_roughness_texture_id, uv);
        m.perceptual_roughness *= metallic_roughness.g;
        m.metallic *= metallic_roughness.b;
    }

    m.roughness = m.perceptual_roughness * m.perceptual_roughness;

    return m;
}

fn resolve_ray_hit_full(ray_hit: RayIntersection) -> ResolvedRayHitFull {
    let barycentrics = vec3(1.0 - ray_hit.barycentrics.x - ray_hit.barycentrics.y, ray_hit.barycentrics);
    cone_t = ray_hit.t;
    let hit = resolve_triangle_data_full(ray_hit.instance_custom_data, ray_hit.primitive_index, barycentrics);
    cone_t = -1.0;
    return hit;
}

fn load_vertices(instance_geometry_ids: InstanceGeometryIds, triangle_id: u32) -> array<Vertex, 3> {
    let index_buffer = &index_buffers[instance_geometry_ids.index_buffer_id].indices;
    let vertex_buffer = &vertex_buffers[instance_geometry_ids.vertex_buffer_id].vertices;

    let indices_i = (triangle_id * 3u) + vec3(0u, 1u, 2u) + instance_geometry_ids.index_buffer_offset;
    let indices = vec3((*index_buffer)[indices_i.x], (*index_buffer)[indices_i.y], (*index_buffer)[indices_i.z]) + instance_geometry_ids.vertex_buffer_offset;

    return array<Vertex, 3>(
        unpack_vertex((*vertex_buffer)[indices.x]),
        unpack_vertex((*vertex_buffer)[indices.y]),
        unpack_vertex((*vertex_buffer)[indices.z])
    );
}

fn transform_positions(transform: mat4x4<f32>, vertices: array<Vertex, 3>) -> array<vec3<f32>, 3> {
    return array<vec3<f32>, 3>(
        (transform * vec4(vertices[0].position, 1.0)).xyz,
        (transform * vec4(vertices[1].position, 1.0)).xyz,
        (transform * vec4(vertices[2].position, 1.0)).xyz
    );
}

fn resolve_triangle_data_full(instance_id: u32, triangle_id: u32, barycentrics: vec3<f32>) -> ResolvedRayHitFull {
    let material_id = material_ids[instance_id];
    let material = materials[material_id];

    let transform = affine3_to_square(transforms[instance_id]);
    let previous_frame_transform = affine3_to_square(previous_frame_transforms[instance_id]);

    let instance_geometry_ids = geometry_ids[instance_id];
    let vertices = load_vertices(instance_geometry_ids, triangle_id);

    let world_vertices = transform_positions(transform, vertices);
    let world_position = mat3x3(world_vertices[0], world_vertices[1], world_vertices[2]) * barycentrics;

    let previous_frame_world_vertices = transform_positions(previous_frame_transform, vertices);
    let previous_frame_world_position = mat3x3(previous_frame_world_vertices[0], previous_frame_world_vertices[1], previous_frame_world_vertices[2]) * barycentrics;

    let uv = mat3x2(vertices[0].uv, vertices[1].uv, vertices[2].uv) * barycentrics;

    let transform_3x3 = mat3x3(transform[0].xyz, transform[1].xyz, transform[2].xyz);

    let cofactor = mat3x3(cross(transform_3x3[1], transform_3x3[2]), cross(transform_3x3[2], transform_3x3[0]), cross(transform_3x3[0], transform_3x3[1]));
    let determinant = dot(transform_3x3[0], cofactor[0]);
    let determinant_sign = select(sign(determinant), 1.0, determinant == 0.0);

    let local_tangent = mat3x3(vertices[0].tangent.xyz, vertices[1].tangent.xyz, vertices[2].tangent.xyz) * barycentrics;
    let world_tangent = vec4(normalize(transform_3x3 * local_tangent), vertices[0].tangent.w * determinant_sign);

    let local_normal = mat3x3(vertices[0].normal, vertices[1].normal, vertices[2].normal) * barycentrics; // TODO: Use barycentric lerp, ray_hit.object_to_world, cross product geo normal
    var world_normal = normalize(determinant_sign * (cofactor * local_normal));

    let geometric_world_normal = world_normal;
    // Delirium: the mip the ray's cone's footprint takes: its width there squared, stretched by how slant it meets the
    // triangle, in the colour texture's texels (the normal map's without one; the triangle's a square metre).
    mip = 0.0;
    let tex = select(material.base_color_texture_id, material.normal_map_texture_id, material.base_color_texture_id == TEXTURE_MAP_NONE);
    if ray_cone.y > 0.0 && cone_t >= 0.0 && tex != TEXTURE_MAP_NONE {
        let duv0 = vertices[1].uv - vertices[0].uv;
        let duv1 = vertices[2].uv - vertices[0].uv;
        let size = vec2<f32>(textureDimensions(textures[tex], 0));
        let area = length(cross(world_vertices[1] - world_vertices[0], world_vertices[2] - world_vertices[0]));
        let texels = abs(duv0.x * duv1.y - duv0.y * duv1.x) * size.x * size.y / max(area, 1e-10);
        let width = ray_cone.x + ray_cone.y * cone_t;
        let slant = max(abs(dot(ray_cone_dir, geometric_world_normal)), 0.05);
        mip = max(0.5 * log2(width * width * texels / slant), 0.0);
    }
    if material.normal_map_texture_id != TEXTURE_MAP_NONE {
        let TBN = calculate_tbn_mikktspace(world_normal, world_tangent);
        let T = TBN[0];
        let B = TBN[1];
        let N = TBN[2];
        var Nt = sample_texture(material.normal_map_texture_id, uv) * 2.0 - 1.0;
        Nt.z = sqrt(max(1.0 - dot(Nt.xy, Nt.xy), 0.0)); // Reconstruct Z to support two-channel normal maps
        world_normal = normalize(Nt.x * T + Nt.y * B + Nt.z * N);
        // Delirium: a bump's normal turned from the eye (steep normal maps seen low went black) bent back till it just
        // faces it (the path tracer's rays; the others' direction unknown here, zero).
        let v = -ray_cone_dir;
        let nv = dot(world_normal, v);
        if nv < 0.05 && dot(geometric_world_normal, v) > 0.0 {
            world_normal = normalize(world_normal + v * (0.05 - nv));
        }
    }

    let triangle_edge0 = world_vertices[0] - world_vertices[1];
    let triangle_edge1 = world_vertices[0] - world_vertices[2];
    let triangle_area = length(cross(triangle_edge0, triangle_edge1)) / 2.0;

    let resolved_material = resolve_material(material, uv);

    return ResolvedRayHitFull(
        world_position,
        previous_frame_world_position,
        world_normal,
        geometric_world_normal,
        world_tangent,
        uv,
        triangle_area,
        instance_geometry_ids.triangle_count,
        resolved_material,
    );
}
