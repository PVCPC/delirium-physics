// Delirium: the sun's caustics, photon mapped (after NVIDIA's ray traced caustics, Ray Tracing Gems II ch. 30): the
// light the sun sends through water (or off it) to where it lands, focused by its waves. A square of tiles round the
// camera in the sun's plane: which have water or glass under them; photons down from those (and their neighbours), bent or
// reflected by the water (split the first time, either after), soaked up in it, summed where they land in bindings.wesl's
// cells; each cell then eased toward this frame's sum.
import bevy_render::view::View;
import bevy_render::maths::orthonormalize;
import bevy_pbr::render::pbr_functions::calculate_tbn_mikktspace;
import bevy_pbr::render::utils::{rand_f, rand_vec2f};
import package::scene::bindings::{
    caustics, caustic_owns, caustic_cell_size, caustic_axis, caustic_slot, CausticHeader, CAUSTIC_CELLS, CAUSTIC_SCALE,
    trace_ray, resolve_ray_hit_full, ResolvedRayHitFull, textures, samplers, TEXTURE_MAP_NONE, materials, material_ids, directional_lights, ray_cone, ray_cone_dir, RAY_T_MIN, RAY_T_MAX,
};
import package::scene::water_beams::water_sun;

enable wgpu_ray_query;

@group(1) @binding(0) var<uniform> view: View;

// The square's tiles a side, half its side (m), a tile's photons a side (a thread's 2 by 2 of them), and how high above
// it they start (m).
const TILES = 256u;
const HALF = 32.0;
const PHOTONS = 32u;
const HEIGHT = 1000.0;
// How much of this frame's sum a cell takes, and how many frames it keeps with none before it's let go.
const EASE = 0.3;
const IDLE = 60u;

@compute @workgroup_size(8, 8, 1)
fn tiles(@builtin(global_invocation_id) id: vec3<u32>) {
    let sun = water_sun();
    let l = select(vec3(0.0, 0.0, 1.0), directional_lights[u32(max(sun, 0))].direction_to_light, sun >= 0);
    let basis = orthonormalize(l);
    let tile = 2.0 * HALF / f32(TILES);
    let cam = view.world_position;
    // (On the tiles' grid, so they don't swim as the camera moves.)
    let center = basis[0] * floor(dot(cam, basis[0]) / tile) * tile + basis[1] * floor(dot(cam, basis[1]) / tile) * tile + l * dot(cam, l);
    if all(id.xy == vec2(0u)) {
        caustics.header = CausticHeader(select(0u, 1u, sun >= 0), u32(max(sun, 0)), HALF, tile, vec4(center, 0.0), vec4(cam, 0.0), vec4(basis[0], 0.0), vec4(basis[1], 0.0));
    }
    var water = 0u;
    if sun >= 0 {
        let o = center + basis[0] * ((f32(id.x) + 0.5) * tile - HALF) + basis[1] * ((f32(id.y) + 0.5) * tile - HALF) + l * HEIGHT;
        let ray = trace_ray(o, -l, 0.0, RAY_T_MAX, RAY_FLAG_NONE);
        if ray.kind != RAY_QUERY_INTERSECTION_NONE {
            let m = materials[material_ids[ray.instance_custom_data]];
            water = select(0u, 1u, m.glass > 0.0);
        }
    }
    caustics.tiles[id.y * TILES + id.x] = water;
}

// A tile's photons (a workgroup), if it or a neighbour has water.
@compute @workgroup_size(16, 16, 1)
fn photons(@builtin(workgroup_id) tile: vec3<u32>, @builtin(local_invocation_id) id: vec3<u32>) {
    let h = caustics.header;
    if h.on == 0u { return; }
    var water = 0u;
    for (var y = max(i32(tile.y) - 1, 0); y <= min(i32(tile.y) + 1, i32(TILES) - 1); y++) {
        for (var x = max(i32(tile.x) - 1, 0); x <= min(i32(tile.x) + 1, i32(TILES) - 1); x++) {
            water |= caustics.tiles[u32(y) * TILES + u32(x)];
        }
    }
    if water == 0u { return; }

    var rng = ((tile.y * TILES + tile.x) * 256u + id.y * 16u + id.x) + view.frame_count * 5782582u;
    let side = h.tile / f32(PHOTONS);
    let l = directional_lights[h.sun].direction_to_light;
    for (var k = 0u; k < 4u; k++) {
        let j = rand_vec2f(&rng);
        let x = f32(tile.x) * h.tile - h.half + (f32(id.x * 2u + k % 2u) + j.x) * side;
        let y = f32(tile.y) * h.tile - h.half + (f32(id.y * 2u + k / 2u) + j.y) * side;
        photon(h.center.xyz + h.u.xyz * x + h.v.xyz * y + l * HEIGHT, -l, vec3(side * side), &rng);
    }
}

// A photon's way (`w`: the sun's light it carries, its beam's square metres straight on).
fn photon(origin: vec3<f32>, direction: vec3<f32>, power: vec3<f32>, rng: ptr<function, u32>) {
    var o = origin;
    var d = direction;
    var w = power;
    var medium = vec3(0.0);
    var bent = false;
    // (The first water's reflection, followed once the way through ends.)
    var later = false;
    var lo = vec3(0.0);
    var ld = vec3(0.0);
    var lw = vec3(0.0);
    var lm = vec3(0.0);
    ray_cone = vec2(0.0);
    for (var i = 0u; i < 16u; i++) {
        var done = true;
        let ray = trace_ray(o, d, RAY_T_MIN, RAY_T_MAX, RAY_FLAG_NONE);
        if ray.kind != RAY_QUERY_INTERSECTION_NONE {
            w *= exp(-medium * ray.t);
            ray_cone_dir = d;
            let hit = resolve_ray_hit_full(ray);
            let m = hit.material;
            let outward = hit.geometric_world_normal;
            let entering = dot(d, outward) < 0.0;
            if m.glass > 0.0 && any(m.absorption > vec3(0.0)) {
                // Water: as the path tracer's glass (its ripples' normal, smooth: the pixels' flat tilts don't focus).
                let mat = materials[material_ids[ray.instance_custom_data]];
                var mapped = hit.world_normal;
                if mat.metallic_roughness_texture_id != TEXTURE_MAP_NONE {
                    mapped = slope_normal(hit, mat.metallic_roughness_texture_id);
                } else if mat.normal_map_texture_id != TEXTURE_MAP_NONE {
                    mapped = smooth_normal(hit, mat.normal_map_texture_id);
                }
                var n = select(-mapped, mapped, entering);
                if dot(d, n) >= 0.0 { n = select(-outward, outward, entering); }
                let eta = select(m.ior, 1.0 / m.ior, entering);
                let through = refract(d, n, eta);
                let f0 = pow((1.0 - eta) / (1.0 + eta), 2.0);
                let fresnel = f0 + (1.0 - f0) * pow(1.0 - saturate(-dot(d, n)), 5.0);
                let back = reflect(d, n);
                if all(through == vec3(0.0)) {
                    o = hit.world_position + n * RAY_T_MIN;
                    d = back;
                } else if !bent {
                    later = true;
                    lo = hit.world_position + n * RAY_T_MIN;
                    ld = back;
                    lw = w * fresnel;
                    lm = medium;
                    w *= 1.0 - fresnel;
                    o = hit.world_position - n * RAY_T_MIN;
                    d = through;
                    medium = select(vec3(0.0), m.absorption, entering);
                } else if rand_f(rng) < fresnel {
                    o = hit.world_position + n * RAY_T_MIN;
                    d = back;
                } else {
                    o = hit.world_position - n * RAY_T_MIN;
                    d = through;
                    medium = select(vec3(0.0), m.absorption, entering);
                }
                bent = true;
                done = false;
            } else if m.glass > 0.0 {
                // Solid glass: bent in and out as the path tracer's, tinted on the way in, or reflected (Fresnel's share).
                var n = select(-hit.world_normal, hit.world_normal, entering);
                if dot(d, n) >= 0.0 { n = select(-outward, outward, entering); }
                let eta = select(m.ior, 1.0 / m.ior, entering);
                let through = refract(d, n, eta);
                let f0 = pow((1.0 - eta) / (1.0 + eta), 2.0);
                if all(through == vec3(0.0)) || rand_f(rng) < f0 + (1.0 - f0) * pow(1.0 - saturate(-dot(d, n)), 5.0) {
                    o = hit.world_position + n * RAY_T_MIN;
                    d = reflect(d, n);
                } else {
                    if entering { w *= mix(vec3(1.0), m.base_color, m.alpha); }
                    o = hit.world_position - n * RAY_T_MIN;
                    d = through;
                }
                done = false;
            } else if bent || dot(d, direction) < 0.9995 {
                // (Out of water, or turned by glass: a pane's two flat sides send it on as it came, the shadows' light.)
                land(hit.world_position, select(-outward, outward, entering), w);
            }
        }
        if done {
            if !later { break; }
            later = false;
            o = lo;
            d = ld;
            w = lw;
            medium = lm;
        }
    }
}

// The normal by the water's own slopes (water.rs's, full size, in its metallic-roughness map's R and A).
fn slope_normal(hit: ResolvedRayHitFull, id: u32) -> vec3<f32> {
    let g = textureSampleLevel(textures[id], samplers[id], hit.uv, 0.0);
    let tbn = calculate_tbn_mikktspace(hit.geometric_world_normal, hit.world_tangent);
    return normalize(-g.r * tbn[0] - g.a * tbn[1] + tbn[2]);
}

// A normal-mapped hit's normal, its map's texels blended (whatever its sampler).
fn smooth_normal(hit: ResolvedRayHitFull, id: u32) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(textures[id], 0));
    let p = hit.uv * vec2<f32>(size) - 0.5;
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let a = textureLoad(textures[id], clamp(i, vec2(0), size - 1), 0).xy;
    let b = textureLoad(textures[id], clamp(i + vec2(1, 0), vec2(0), size - 1), 0).xy;
    let c = textureLoad(textures[id], clamp(i + vec2(0, 1), vec2(0), size - 1), 0).xy;
    let d = textureLoad(textures[id], clamp(i + vec2(1, 1), vec2(0), size - 1), 0).xy;
    let xy = mix(mix(a, b, f.x), mix(c, d, f.x), f.y) * 2.0 - 1.0;
    let tbn = calculate_tbn_mikktspace(hit.geometric_world_normal, hit.world_tangent);
    return normalize(xy.x * tbn[0] + xy.y * tbn[1] + sqrt(max(1.0 - dot(xy, xy), 0.0)) * tbn[2]);
}

// What a photon brings to where it lands (facing `n`), over the cell's area on the surface (its square's on the axis
// the surface faces most, over that axis' share of its normal).
fn land(p: vec3<f32>, n: vec3<f32>, w: vec3<f32>) {
    if !caustic_owns(p) { return; }
    let s = caustic_cell_size(p);
    let k = caustic_slot(p, n, s, true);
    if k < 0 { return; }
    let e = min(w * abs(n[caustic_axis(n) / 2u]) / (s * s) * CAUSTIC_SCALE, vec3(1e8));
    atomicAdd(&caustics.cells[k].flux[0], u32(e.x));
    atomicAdd(&caustics.cells[k].flux[1], u32(e.y));
    atomicAdd(&caustics.cells[k].flux[2], u32(e.z));
}

@compute @workgroup_size(256, 1, 1)
fn resolve(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= CAUSTIC_CELLS || atomicLoad(&caustics.cells[i].checksum) == 0u { return; }
    let f = vec3(
        f32(atomicExchange(&caustics.cells[i].flux[0], 0u)),
        f32(atomicExchange(&caustics.cells[i].flux[1], 0u)),
        f32(atomicExchange(&caustics.cells[i].flux[2], 0u)),
    ) / CAUSTIC_SCALE;
    caustics.cells[i].light = mix(caustics.cells[i].light, f, EASE);
    caustics.cells[i].idle = select(caustics.cells[i].idle + 1u, 0u, any(f > vec3(0.0)));
    if caustics.cells[i].idle > IDLE {
        atomicStore(&caustics.cells[i].checksum, 0u);
        caustics.cells[i].light = vec3(0.0);
        caustics.cells[i].idle = 0u;
    }
}
