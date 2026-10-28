// Delirium: water in ray-traced lighting as the path tracer sees it (from Extra High it's in the scene, glass): what's seen
// through its surface bent (Snell's, by its ripples' normal map), its reflection by Fresnel, what those rays hit lit from
// the world cache; the light soaked up on its way through; and the pool's caustics at their full brightness (the G-buffer
// holds albedo only up to 1).
import bevy_core_pipeline::tonemapping::tonemapping_luminance;
import bevy_render::maths::PI;
import package::realtime::bindings::{constants, view, water_fog};
import package::scene::bindings::{trace_ray, resolve_ray_hit_full, ResolvedRayHitFull, ResolvedMaterial, caustic_photons, caustic_sun, RAY_T_MIN, RAY_T_MAX};
import package::scene::sampling::sample_environment_map_light as _env;
import bevy_pbr::render::utils::rand_vec2f;
import bevy_pbr::render::pbr_functions::calculate_diffuse_color;
import package::scene::sampling::sample_environment_map_light;
import package::realtime::world_cache_query::{query_world_cache, world_cache_found, WORLD_CACHE_CELL_LIFETIME};
import package::realtime::gbuffer_utils::reconstruct_world_position;
import package::scene::water_beams::{water_beams, sun_on, sun_disk};

enable wgpu_ray_query;

const WATER_IOR: f32 = 1.333;

// What the eye sees of a G-buffer pixel (`color` its light before exposure, `albedo` the G-buffer's), after exposure.
fn water_view(color: vec3<f32>, world_position: vec3<f32>, albedo: vec3<f32>, rng: ptr<function, u32>) -> vec3<f32> {
    let sigma = constants.water_fog.xyz;
    let under = constants.water_fog.w != 0.0;
    let eye = view.world_position;
    let to = world_position - eye;
    let dir = normalize(to);
    let ray = trace_ray(eye, dir, 0.0, length(to) + 0.01, RAY_FLAG_NONE);
    if ray.kind == RAY_QUERY_INTERSECTION_NONE {
        return water_fog(color * view.exposure, world_position);
    }
    let hit = resolve_ray_hit_full(ray);
    // (The pool's way only where the way to it is in or through its water; other glass the path tracer's.)
    if all(sigma == vec3(0.0)) || (!under && world_position.z >= constants.water_fog_color.w) {
        if hit.material.glass > 0.0 { return through_glass(eye, dir, ray, rng) * view.exposure; }
        return water_fog(color * view.exposure, world_position);
    }
    if hit.material.glass > 0.0 {
        return through_surface(hit.world_position, hit.world_normal, hit.geometric_world_normal, dir, ray.t, rng) * view.exposure;
    }
    // The caustics' albedo past 1 back.
    var c = color;
    if distance(hit.world_position, world_position) < 0.05 {
        c *= select(vec3(1.0), hit.material.base_color / albedo, albedo > vec3(0.01));
    }
    // (The sunbeams on the way, under the water.)
    var beams = vec3(0.0);
    if under { beams = water_beams(eye, dir, length(to), sigma, rng); }
    return water_fog(c * view.exposure, world_position) + beams * view.exposure;
}

// The sky's pixels (no G-buffer), under the water: the surface over them (the raster sky's off then: water.rs). After
// exposure, w 0 when there's no water in the way.
fn water_sky(pixel_id: vec2<u32>, rng: ptr<function, u32>) -> vec4<f32> {
    let far = reconstruct_world_position(pixel_id, 1e-6, view.main_pass_viewport.zw, view.world_from_clip);
    let dir = normalize(far - view.world_position);
    let ray = trace_ray(view.world_position, dir, 0.0, RAY_T_MAX, RAY_FLAG_NONE);
    // (Out of the water: glass in the way, the path tracer's way through it.)
    if constants.water_fog.w == 0.0 {
        if ray.kind == RAY_QUERY_INTERSECTION_NONE || resolve_ray_hit_full(ray).material.glass == 0.0 { return vec4(0.0); }
        return vec4(through_glass(view.world_position, dir, ray, rng) * view.exposure, 1.0);
    }
    // (No surface in the scene below Extra High: the sky through the water up to where it'd be.)
    let up = (constants.water_fog_color.w - view.world_position.z) / max(dir.z, 1e-3);
    var l = medium(sample_environment_map_light(dir), select(RAY_T_MAX, up, dir.z > 0.0));
    if ray.kind != RAY_QUERY_INTERSECTION_NONE {
        let hit = resolve_ray_hit_full(ray);
        if hit.material.glass > 0.0 {
            l = through_surface(hit.world_position, hit.world_normal, hit.geometric_world_normal, dir, ray.t, rng);
        }
    }
    return vec4(l * view.exposure, 1.0);
}

// At the surface: the way bent through it and the way reflected, by Fresnel (all reflected past the critical angle from
// below), the eye's way there through the water if it's under.
fn through_surface(p: vec3<f32>, mapped: vec3<f32>, geometric: vec3<f32>, d: vec3<f32>, t: f32, rng: ptr<function, u32>) -> vec3<f32> {
    let entering = dot(d, geometric) < 0.0;
    var n = select(-mapped, mapped, entering);
    if dot(d, n) >= 0.0 { n = select(-geometric, geometric, entering); }
    let bent = refract(d, n, select(WATER_IOR, 1.0 / WATER_IOR, entering));
    let tir = all(bent == vec3(0.0));
    // (Schlick's, by the angle on the air's side.)
    let c = select(saturate(-dot(bent, n)), saturate(-dot(d, n)), entering);
    let f = select(0.02 + 0.98 * pow(1.0 - c, 5.0), 1.0, tir);
    var l = f * seen(p + n * 0.002, reflect(d, n), !entering, rng);
    if !tir {
        l += (1.0 - f) * seen(p - n * 0.002, bent, entering, rng);
    }
    if !entering {
        l = medium(l, t) + water_beams(view.world_position, d, t, constants.water_fog.xyz, rng);
    }
    return l;
}

// What a ray sees (before exposure): its hit lit from the world cache, the sky if none.
fn seen(o: vec3<f32>, d: vec3<f32>, in_water: bool, rng: ptr<function, u32>) -> vec3<f32> {
    let ray = trace_ray(o, d, RAY_T_MIN, RAY_T_MAX, RAY_FLAG_NONE);
    // (Off water or glass: the sun's glitter too.)
    var l = sample_environment_map_light(d) + glint(d);
    var t = RAY_T_MAX;
    if ray.kind != RAY_QUERY_INTERSECTION_NONE {
        let hit = resolve_ray_hit_full(ray);
        t = ray.t;
        if hit.material.glass == 0.0 {
            l = lit(hit, ray.t, rng);
        }
    }
    if !in_water { return l; }
    return medium(l, t) + water_beams(o, d, t, constants.water_fog.xyz, rng);
}

// A ray's hit `t` off, lit from the world cache (before exposure).
fn lit(hit: ResolvedRayHitFull, t: f32, rng: ptr<function, u32>) -> vec3<f32> {
    world_cache_found = hit.world_position;
    var l = query_world_cache(hit.world_position, hit.geometric_world_normal, view.world_position, t, WORLD_CACHE_CELL_LIFETIME, rng);
    // The sun's share the cell's (one point's for all of it) for this point's own: sharp shadows.
    // (Where photons light it, the sun through water theirs, soaked up already.)
    let n = hit.geometric_world_normal;
    l = max(l - sun_on(world_cache_found, n), vec3(0.0));
    caustic_photons = true;
    l += sun_on(hit.world_position, n);
    caustic_photons = false;
    // (The light's way down through the water, as `water_fog`'s.)
    let down = exp(-constants.water_fog.xyz * max(constants.water_fog_color.w - hit.world_position.z, 0.0) * 1.3);
    let facing = select(-n, n, dot(n, view.world_position - hit.world_position) > 0.0);
    return hit.material.base_color / PI * (l * down + caustic_sun(hit.world_position, facing, rand_vec2f(rng))) + hit.material.emissive;
}

// Delirium: the photons' light on a G-buffer pixel's surface (its diffuse share), before exposure.
fn caustics_on(p: vec3<f32>, n: vec3<f32>, m: ResolvedMaterial, rng: ptr<function, u32>) -> vec3<f32> {
    return calculate_diffuse_color(m.base_color, m.metallic, 0.0, 0.0) / PI * caustic_sun(p, n, rand_vec2f(rng));
}

// Through glass but the pool's water, as the path tracer goes (`first` the eye's ray's hit on it, `d` its way), before
// exposure: bent through surface after surface (all reflected past the critical angle), the first's reflection by
// Fresnel, tinted on the way into a pane, soaked up through a body of it (water); what's past lit from the world cache.
fn through_glass(o: vec3<f32>, d0: vec3<f32>, first: RayIntersection, rng: ptr<function, u32>) -> vec3<f32> {
    var ray = first;
    var d = d0;
    var through = vec3(1.0);
    var l = vec3(0.0);
    var medium = vec3(0.0);
    for (var i = 0u; i < 12u; i++) {
        if ray.kind == RAY_QUERY_INTERSECTION_NONE { return l + through * (sample_environment_map_light(d) + glint(d)); }
        // (In a body, the sky's light it scatters our way too: `medium`'s.)
        let k = exp(-medium * ray.t);
        l += through * (1.0 - k) * exp(-medium * 5.0) * tonemapping_luminance(sample_environment_map_light(vec3(0.0, 0.0, 1.0))) * 0.3;
        through *= k;
        let hit = resolve_ray_hit_full(ray);
        if hit.material.glass == 0.0 { return l + through * lit(hit, ray.t, rng); }
        let outward = hit.geometric_world_normal;
        let entering = dot(d, outward) < 0.0;
        var n = select(-hit.world_normal, hit.world_normal, entering);
        if dot(d, n) >= 0.0 { n = select(-outward, outward, entering); }
        let eta = select(hit.material.ior, 1.0 / hit.material.ior, entering);
        let bent = refract(d, n, eta);
        var p = hit.world_position;
        if all(bent == vec3(0.0)) {
            d = reflect(d, n);
            p += n * 0.002;
        } else {
            let f0 = pow((1.0 - eta) / (1.0 + eta), 2.0);
            let f = f0 + (1.0 - f0) * pow(1.0 - saturate(-dot(d, n)), 5.0);
            if i == 0u { l += f * seen(p + n * 0.002, reflect(d, n), false, rng); }
            through *= 1.0 - f;
            let body = any(hit.material.absorption > vec3(0.0));
            if entering && !body { through *= mix(vec3(1.0), hit.material.base_color, hit.material.alpha); }
            if body { medium = select(vec3(0.0), hit.material.absorption, entering); }
            d = bent;
            p -= n * 0.002;
        }
        ray = trace_ray(p, d, RAY_T_MIN, RAY_T_MAX, RAY_FLAG_NONE);
    }
    return l;
}

// The sun's disk the way `d`, before exposure, held to a picture's 50 (its real brightness overflows the half floats
// the picture's kept in, and the overflow spreads over the screen).
fn glint(d: vec3<f32>) -> vec3<f32> {
    return min(sun_disk(d), vec3(50.0 / view.exposure));
}

// Through `t` of water: soaked up, and the sky's light scattered in from a few metres down (the path tracer's).
fn medium(l: vec3<f32>, t: f32) -> vec3<f32> {
    let sigma = constants.water_fog.xyz;
    let through = exp(-sigma * t);
    let glow = exp(-sigma * 5.0) * tonemapping_luminance(sample_environment_map_light(vec3(0.0, 0.0, 1.0))) * 0.3;
    return l * through + (1.0 - through) * glow;
}
