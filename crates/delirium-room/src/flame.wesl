// Fire (fire.rs): flames, embers and smoke, all procedural. Each quad's colour carries its particle: r how far
// through its life (0..1), g a seed, b its kind (0 a lick of flame, 0.25 a sheet of flame, 0.5 an ember, 1 smoke),
// a how strongly it burns. Flames and embers add their light (alpha 0 under premultiplied blending); smoke covers.
// Voxel fire (flame.k.y 1): its flames and embers solid cubes, each flat in a fire palette's colour for its heat, its
// face's shade in uv.x.
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::{globals, view},
};

struct FlameK {
    // x: overall brightness; y: 1 for voxel fire.
    k: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> flame: FlameK;

// A flame's hottest light (before the camera's exposure), and smoke's shade.
const FLAME_NITS: f32 = 3500.0;
const SMOKE_NITS: f32 = 250.0;
// Voxel fire's flames' light as a part of realistic's.
const VOXEL_K: f32 = 0.5;
// How much of what's behind a voxel it covers.
const VOXEL_COVER: f32 = 0.45;

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var s = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i = i + 1) {
        s = s + a * noise(q);
        q = q * 2.03 + vec2(1.7, 9.2);
        a = a * 0.5;
    }
    return s;
}

// Heat 0..1+ to a flame's colour, as hot gas glows: deep red, orange, yellow, then near white in the core.
fn ramp(h: f32) -> vec3<f32> {
    let red = vec3(0.45, 0.03, 0.0);
    let orange = vec3(1.0, 0.3, 0.02);
    let yellow = vec3(1.0, 0.62, 0.15);
    let white = vec3(1.0, 0.85, 0.6);
    return mix(mix(mix(red, orange, smoothstep(0.05, 0.4, h)), yellow, smoothstep(0.4, 0.8, h)), white, smoothstep(1.0, 1.3, h));
}

// Voxel fire's palette: seven steps from smouldering red to white-hot (the old games' fire).
fn palette(h: f32) -> vec3<f32> {
    let i = clamp(i32(h * 7.0), 0, 6);
    var c = array<vec3<f32>, 7>(vec3(0.28, 0.04, 0.02), vec3(0.55, 0.07, 0.02), vec3(0.8, 0.18, 0.02), vec3(0.95, 0.35, 0.03),
                                vec3(1.0, 0.55, 0.08), vec3(1.0, 0.78, 0.25), vec3(1.0, 0.95, 0.7));
    return c[i] * c[i];
}

// A sheet of flame: rolling, warped noise rising through a body that narrows upward, burning off as it climbs.
fn sheet_heat(uv: vec2<f32>, seed: f32, time: f32) -> f32 {
    let p = vec2((uv.x - 0.5) * 2.0, uv.y);
    let q = vec2(p.x * 1.2 + seed * 10.0, p.y * 1.5 - time * 1.8);
    let warp = vec2(fbm(q * 1.5 + vec2(0.0, time * 0.5)), fbm(q * 1.5 + vec2(5.2, 1.3)));
    let n = fbm(q + warp * 0.9);
    let body = 1.0 - smoothstep(0.35, 1.0, abs(p.x) / max(1.0 - p.y * 0.85, 0.05));
    return clamp(body * (n * 1.6 - p.y * 1.05 + 0.25) * 1.4, 0.0, 1.3);
}

// A lick of flame: a teardrop narrowing to its tip, licked sideways and broken up by rising noise, cooling with age.
fn lick_heat(uv: vec2<f32>, seed: f32, time: f32, t: f32) -> f32 {
    let x = uv.x - 0.5;
    let y = uv.y;
    let n = fbm(vec2(x * 3.0 + seed, y * 2.2 - time * 2.8 + seed));
    let w = 0.5 * pow(max(1.0 - y, 0.0), 1.1) * (0.7 + 0.3 * smoothstep(0.0, 0.18, y));
    let d = abs(x + (n - 0.5) * 0.4 * y) / max(w, 0.001);
    return clamp((1.0 - d) * (0.35 + n) * (1.0 - 0.6 * y) * (1.0 - 0.75 * t) * 1.5, 0.0, 1.3);
}

@fragment
fn fragment(in: VertexOutput) -> FragmentOutput {
    let t = in.color.r;
    let seed = in.color.g * 37.0;
    let kind = in.color.b;
    let k = in.color.a;
    let light = view.exposure * flame.k.x * FLAME_NITS;
    var out: FragmentOutput;
    if (flame.k.y > 0.5 && kind < 0.6) {
        // A voxel: hottest young (an ember stays hot), cooling to red as it rises (smoke is as realistic's).
        let heat = select(1.1 * (1.0 - t), 1.0 - 0.5 * t, kind > 0.4);
        // See-through, glowing: its light more than it covers.
        out.color = vec4(palette(heat) * in.uv.x * k * VOXEL_K * light, VOXEL_COVER * k);
        return out;
    }
    let uv = in.uv;
    let time = globals.time;
    if (kind < 0.6) {
        var heat: f32;
        var foot = smoothstep(0.0, 0.08, uv.y);
        var bright = 1.0;
        if (kind < 0.1) {
            heat = lick_heat(uv, seed, time, t);
            foot = smoothstep(0.0, 0.18, uv.y);
        } else if (kind < 0.4) {
            heat = sheet_heat(uv, seed, time);
            bright = 0.4;
        } else {
            // An ember: a hot dot, cooling.
            let r = length(uv - 0.5) * 2.0;
            heat = (1.0 - smoothstep(0.1, 1.0, r)) * 1.2 * (1.0 - 0.6 * t);
            foot = 1.0 - t;
            bright = 1.5;
        }
        let a = smoothstep(0.02, 0.35, heat) * foot * k;
        out.color = vec4(ramp(heat) * a * bright * (0.15 + heat * heat) * light, 0.0);
    } else {
        // Smoke: a soft, churning puff, thinning as it spreads.
        let n = fbm(uv * 3.0 + vec2(seed, seed - time * 0.4));
        let r = length(uv - 0.5) * 2.0 + (n - 0.5) * 0.6;
        let a = (1.0 - smoothstep(0.0, 1.0, r)) * smoothstep(0.0, 0.2, t) * (1.0 - t) * 0.2 * k;
        let shade = (0.4 + 0.25 * n) * SMOKE_NITS * view.exposure;
        out.color = vec4(vec3(shade) * a, a);
    }
    return out;
}
