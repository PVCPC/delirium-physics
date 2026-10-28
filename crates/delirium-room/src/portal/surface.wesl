// A portal's surface (render.rs `PortalSurface`), on a quad the size of the opening's oval (uv 0..1 across it).
// Inside the oval: the view through (the node camera's picture, sampled at this pixel's place on screen). On its edge: a
// burning band in the portal's colour (the colour ramp, read by brightness), swirled by the noise texture over time,
// glowing outward and with a thin dark line on the inner side. Unlinked, a swirling membrane of that colour fills it;
// just after linking it fades away (portal_active 0 -> 1). Opening (open_amount 0 -> 1), the oval grows from the
// centre with a flash on its edge; it's drawn opaque then, and the wall around it comes from the opaque-pass copy,
// rippled. The cap (the box behind the opening, when the eye is nearer than the near plane) shows just the view.
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings as vb,
};

struct SurfaceParams {
    open_amount: f32,
    portal_active: f32,
    color_scale: f32,
    flags: u32,
    // The view's uv = screen uv * xy + zw (the depth doubler's remap; identity otherwise).
    uv_remap: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: SurfaceParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var view_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var view_smp: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var noise_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var noise_smp: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var ramp_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var ramp_smp: sampler;

const FLAG_HAS_VIEW: u32 = 1u;
const FLAG_CAP: u32 = 2u;
const FLAG_BLEND: u32 = 16u;

// The oval's half size, metres (portal.rs HALF): the noise is laid out in metres so it isn't stretched.
const HALF: vec2<f32> = vec2(0.8128, 1.4224);
// Where the view ends, as a part of the oval's radius; the band burns from here out to the oval's edge.
const EDGE: f32 = 0.85;

fn nz(p: vec2<f32>) -> f32 {
    return textureSample(noise_tex, noise_smp, p).r;
}

// The noise in ring coordinates: x around the oval (turns * reps), y across it. atan2's seam is hidden by giving the
// sampler the angle's real screen-space change and repeating the tiling texture a whole number of times.
fn ring_nz(q: vec2<f32>, off: vec2<f32>, reps: f32, across: f32) -> f32 {
    let turns = atan2(q.y, q.x) * 0.15915494;
    let dx = dpdx(q);
    let dy = dpdy(q);
    let rr = max(dot(q, q), 1e-6);
    let dtx = (q.x * dx.y - q.y * dx.x) / rr * 0.15915494 * reps;
    let dty = (q.x * dy.y - q.y * dy.x) / rr * 0.15915494 * reps;
    let p = vec2(turns * reps, across) + off;
    return textureSampleGrad(noise_tex, noise_smp, p, vec2(dtx, dpdx(across)), vec2(dty, dpdy(across))).r;
}

fn ramp(x: f32) -> vec3<f32> {
    return textureSampleLevel(ramp_tex, ramp_smp, vec2(clamp(x, 0.002, 0.998), 0.5), 0.0).rgb;
}

fn rot(p: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2(c * p.x - s * p.y, s * p.x + c * p.y);
}

fn screen_uv(frag: vec4<f32>) -> vec2<f32> {
    return (frag.xy - vb::view.viewport.xy) / vb::view.viewport.zw;
}

fn through(frag: vec4<f32>) -> vec3<f32> {
    let uv = screen_uv(frag) * params.uv_remap.xy + params.uv_remap.zw;
    return textureSampleLevel(view_tex, view_smp, uv, 0.0).rgb;
}

// The membrane's swirl at q (metres from the centre): two broad layers turning opposite ways, twisted a little more
// toward the centre, about 0..3 (mean ~1).
fn swirl(q: vec2<f32>, r: f32, t: f32) -> f32 {
    let a = nz(rot(q, t * 0.7 + (1.0 - r) * 1.2) * 0.3 + vec2(0.11, 0.37));
    let b = nz(rot(q, -t * 0.45 - (1.0 - r) * 0.8) * 0.45 + vec2(0.53, 0.19));
    return a * b * 3.5;
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    let has_view = (params.flags & FLAG_HAS_VIEW) != 0u;
    if (params.flags & FLAG_CAP) != 0u {
        if !has_view {
            discard;
        }
        out.color = vec4(through(in.position), 1.0);
        return out;
    }

    let t = vb::globals.time;
    let q = (in.uv - 0.5) * vec2(2.0, -2.0) * HALF;
    let r = length(q / HALF);
    // Opening: the oval grows fast, then settles; its edge flashes.
    let s = clamp(params.open_amount, 0.0, 1.0);
    let size = max(1.0 - pow(1.0 - s, 3.0), 1e-3);
    let flash = 1.0 + 3.0 * (1.0 - s) * (1.0 - s);
    // Ring coordinates: around the oval, and across the band (0 at the view's edge, 1 at the oval's).
    let across = (r / size - EDGE) / (1.0 - EDGE);
    // The view's edge wanders a little with the noise.
    let d = r / size + (ring_nz(q, vec2(t * 0.03, 0.3), 6.0, r * 0.4 - t * 0.05) - 0.5) * 0.05;
    let outward = max(d - EDGE, 0.0) / (1.0 - EDGE);
    let inward = max(EDGE - d, 0.0);
    // Flames: two layers streaming outward across the band, drifting around it opposite ways (about 0..3, mean ~1).
    let f1 = ring_nz(q, vec2(t * 0.02, -t * 0.12), 3.0, across * 0.07);
    let f2 = ring_nz(q, vec2(-t * 0.03, 0.5 - t * 0.08), 2.0, across * 0.05);
    let n = f1 * f2 * 3.5;

    // The burning band: a bright line at the view's edge, tongues licking outward as far as the flames reach, a soft
    // glow, all gone by the oval's edge; a thin falloff inward.
    let line = exp(-outward * 7.0) * (0.75 + 0.35 * n);
    let tongues = smoothstep(outward * 1.6, outward * 1.6 + 0.5, n * 0.5) * 0.7;
    let glow = exp(-outward * 2.5) * 0.35;
    let fade = 1.0 - smoothstep(0.9, 1.0, across);
    let heat = select(exp(-inward / 0.008), line + tongues + glow, d >= EDGE) * fade * flash;
    let fire = ramp(min(heat, 1.0)) * params.color_scale * max(heat, 1.0);
    let fire_a = clamp(heat * 1.3, 0.0, 1.0);

    // Inside: the view (with a thin dark line along the edge), or the membrane while unlinked / fading.
    let inside = 1.0 - smoothstep(EDGE - 0.006, EDGE + 0.006, d);
    let membrane = ramp(0.55 + 0.25 * (swirl(q, r, t) - 1.0) + 0.3 * r * r) * params.color_scale * 0.7;
    let stat = 1.0 - clamp(params.portal_active, 0.0, 1.0);
    var interior = membrane;
    var interior_a = 0.9;
    if has_view {
        interior = mix(through(in.position) * (1.0 - 0.6 * exp(-inward / 0.04)), membrane, stat);
        interior_a = mix(1.0, 0.9, stat);
    }
    let base_a = inside * interior_a;
    // The fire adds its light over what's under it, covering little of it: the rim's own particles (drawn at the
    // wall, behind this quad) still show through.
    let lit = interior * base_a + fire * fire_a;
    let a = base_a + 0.3 * fire_a * (1.0 - base_a);

    if (params.flags & FLAG_BLEND) != 0u {
        // Fully open: blended over the frame (straight alpha: the light is divided by the coverage).
        if base_a + fire_a < 0.002 {
            discard;
        }
        let cover = max(a, 0.02);
        out.color = vec4(lit / cover, cover);
        return out;
    }
    // Opening: opaque; the wall around comes from the opaque-pass copy, rippled near the growing edge.
    var wall = vec3(0.0);
    @if(VIEW_TRANSMISSION_TEXTURE) {
        let dims = vec2<f32>(textureDimensions(vb::view_transmission_texture));
        let ring = exp(-abs(d - 1.0) / 0.25) * (1.0 - s);
        let push = (vec2(nz(q * 2.0 + vec2(t, 0.0)), nz(q * 2.0 + vec2(0.5, t))) - 0.5) * 0.03 * ring;
        wall = textureSampleLevel(vb::view_transmission_texture, vb::view_transmission_sampler, in.position.xy / dims + push, 0.0).rgb;
    }
    @if(!VIEW_TRANSMISSION_TEXTURE) {
        if a < 0.5 {
            discard;
        }
    }
    out.color = vec4(lit + wall * (1.0 - a), 1.0);
    return out;
}
