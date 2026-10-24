// Delirium: sunbeams in water (both ray tracers). The sun's light a point in the water scatters toward the eye: the sun
// bent down through the surface (by its ripples where it comes through), soaked up on its way down, as focused as the
// caustics where it lands show (only what's past flat water's: the beams, not a haze), none where anything's in its way.
import bevy_core_pipeline::tonemapping::tonemapping_luminance;
import bevy_render::maths::PI;
import bevy_pbr::render::utils::rand_f;
import package::scene::bindings::{trace_ray, resolve_ray_hit_full, trace_transmittance, directional_lights, RAY_T_MIN, RAY_T_MAX};

enable wgpu_ray_query;

// How much of the light a metre of the water scatters, and how much of that forward (Henyey-Greenstein's g).
const BEAM_SCATTER: f32 = 0.2;
const BEAM_FORWARD: f32 = 0.6;
// The pool's tiles' brightness (water.rs's caustics: their albedo the tiles' times how focused the light is there).
const TILE: f32 = 0.82;
// The longest way through water a beam's looked for along (m).
const BEAM_REACH: f32 = 20.0;

// The sun's disk seen the way `d` (the sky map hasn't it: the sun's a light of its own), before exposure: for a
// smooth surface's reflection (water's glitter) and the sky seen straight.
fn sun_disk(d: vec3<f32>) -> vec3<f32> {
    var l = vec3(0.0);
    for (var i = 0u; i < arrayLength(&directional_lights); i++) {
        let s = directional_lights[i];
        if dot(d, s.direction_to_light) >= s.cos_theta_max { l += s.luminance; }
    }
    return l;
}

// The beams' light along `t` of a ray from `o` the way `d` in water soaking up `sigma` (before exposure): one point
// along it picked at random (the denoiser evens it out).
fn water_beams(o: vec3<f32>, d: vec3<f32>, t: f32, sigma: vec3<f32>, rng: ptr<function, u32>) -> vec3<f32> {
    let reach = min(t, BEAM_REACH);
    if reach <= 0.0 { return vec3(0.0); }
    let s = rand_f(rng) * reach;
    return water_beam(o + d * s, d) * exp(-sigma * s) * reach;
}

// The light a metre of water at `p` scatters back along the way `d` came.
fn water_beam(p: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let sun = water_sun();
    if sun < 0 { return vec3(0.0); }
    let light = directional_lights[u32(sun)];
    let down = refract(-light.direction_to_light, vec3(0.0, 0.0, 1.0), 1.0 / 1.333);
    // Up to the surface (nothing in the way), then over the water to the sun.
    let up = trace_ray(p, -down, RAY_T_MIN, RAY_T_MAX, RAY_FLAG_NONE);
    if up.kind == RAY_QUERY_INTERSECTION_NONE { return vec3(0.0); }
    let top = resolve_ray_hit_full(up);
    if top.material.glass == 0.0 { return vec3(0.0); }
    let open = trace_transmittance(top.world_position + light.direction_to_light * 0.01, light.direction_to_light, RAY_T_MAX, false);
    // Bent by the ripple there (ponytail: from where the flat way meets the surface, not a search for the spot that
    // bends it through `p` exactly); down from there to where it lands: how focused.
    var n = top.world_normal;
    if n.z < 0.0 { n = -n; }
    var bent = refract(-light.direction_to_light, n, 1.0 / 1.333);
    if all(bent == vec3(0.0)) { bent = down; }
    let land = trace_ray(top.world_position - n * 0.002, bent, RAY_T_MIN, RAY_T_MAX, RAY_FLAG_NONE);
    if land.kind == RAY_QUERY_INTERSECTION_NONE { return vec3(0.0); }
    let focus = max(tonemapping_luminance(resolve_ray_hit_full(land).material.base_color) / TILE - 1.0, 0.0);
    let cos_theta = dot(bent, -d);
    let g = BEAM_FORWARD;
    let phase = (1.0 - g * g) / (4.0 * PI * pow(1.0 + g * g - 2.0 * g * cos_theta, 1.5));
    let sigma = top.material.absorption;
    return BEAM_SCATTER * phase * light.luminance * light.inverse_pdf * open * exp(-sigma * up.t) * focus;
}

// The sun's light straight on a surface at `p` facing `n` (through glass as it tints it).
fn sun_on(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let sun = water_sun();
    if sun < 0 { return vec3(0.0); }
    let light = directional_lights[u32(sun)];
    let c = dot(n, light.direction_to_light);
    if c <= 0.0 { return vec3(0.0); }
    return light.luminance * light.inverse_pdf * c * trace_transmittance(p + n * RAY_T_MIN, light.direction_to_light, RAY_T_MAX, false);
}

// The sun: the brightest directional light that's up (-1: none).
fn water_sun() -> i32 {
    var sun = -1;
    var best = 0.0;
    for (var i = 0u; i < arrayLength(&directional_lights); i++) {
        let l = directional_lights[i];
        let b = tonemapping_luminance(l.luminance) * l.inverse_pdf;
        if l.direction_to_light.z > 0.05 && b > best {
            best = b;
            sun = i32(i);
        }
    }
    return sun;
}
