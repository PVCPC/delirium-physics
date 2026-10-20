// SpriteCard / SplineCard particle rendering, after Valve's public
// spritecard_vsxx / splinecard_vsxx / spritecard_ps2x shaders: sprites and
// Catmull-Rom ropes are expanded per view in the vertex shader, so they face
// every camera correctly (including the virtual cameras behind portals).
#import bevy_pbr::mesh_view_bindings::{view, view_transmission_texture, view_transmission_sampler}
#import bevy_pbr::mesh_functions::get_world_from_local
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct SpriteCardParams {
    overbright: f32,
    add_self: f32,
    // bit 0: additive, bit 1: distance alpha, bit 2: refraction
    flags: u32,
    refract: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: SpriteCardParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var base_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var base_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // Particle centre / rope point, in the effect's space (the mesh transform
    // places it in the world: identity for world effects, the eye for the
    // first-person gun effects).
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    // Sprites: (corner x, corner y, radius, rotation). Ropes: (side, 0, width, 0).
    @location(8) shape: vec4<f32>,
    // xyz: normal (oriented sprite), tangent (rope) or fixed offset direction; w: mode.
    @location(9) axis: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // View depth (metres), for the refraction offset.
    @location(2) depth: f32,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let mode = u32(v.axis.w + 0.5);
    let cam = view.world_position.xyz;
    let world_from_local = get_world_from_local(v.instance_index);
    let basis = mat3x3<f32>(world_from_local[0].xyz, world_from_local[1].xyz, world_from_local[2].xyz);
    var world = (world_from_local * vec4(v.position, 1.0)).xyz;
    let axis_dir = basis * v.axis.xyz;
    if (mode == 0u || mode == 1u) {
        // Sprite: 0 = face the camera, 1 = lie in the plane of `axis` (normal).
        var right: vec3<f32>;
        var up: vec3<f32>;
        if (mode == 0u) {
            right = view.world_from_view[0].xyz;
            up = view.world_from_view[1].xyz;
        } else {
            let n = normalize(axis_dir);
            var helper = vec3(0.0, 0.0, 1.0);
            if (abs(n.z) > 0.9) {
                helper = vec3(1.0, 0.0, 0.0);
            }
            right = normalize(cross(helper, n));
            up = cross(n, right);
        }
        let c = cos(v.shape.w);
        let s = sin(v.shape.w);
        let corner = vec2(v.shape.x * c - v.shape.y * s, v.shape.x * s + v.shape.y * c);
        world = world + (right * corner.x + up * corner.y) * v.shape.z;
    } else {
        // Rope: 2 = widen across the view (SplineCard), 3 = widen along a fixed direction.
        var ofs: vec3<f32>;
        if (mode == 2u) {
            ofs = normalize(cross(world - cam, normalize(axis_dir)));
        } else {
            ofs = normalize(axis_dir);
        }
        world = world + ofs * (v.shape.z * v.shape.x);
    }
    var out: VertexOutput;
    out.clip = view.clip_from_world * vec4(world, 1.0);
    out.uv = v.uv;
    out.color = v.color;
    out.depth = out.clip.w;
    return out;
}

// Source's sRGB transfer (piecewise), applied per channel.
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3(0.055)) / 1.055, vec3(2.4));
    return select(hi, lo, c <= vec3(0.04045));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // The texture and the tint are both sRGB-encoded values, as Source blends them.
    // Multiply in that space, then decode once, so the written light matches what
    // Source displays for the same values.
    let tex = textureSample(base_texture, base_sampler, in.uv);
    if ((params.flags & 4u) != 0u) {
        // Refract card: the texture is a normal map. Bend the scene behind the
        // card by the normal's xy times $refractamount, blended in by the
        // card's alpha (so its edges are the unbent scene).
        // Like Source's refract shader, the offset is added before the
        // perspective divide, so it shrinks with distance (depth in Source
        // units, 0.0254 m each); the card's vertex alpha blends it in.
        let screen_uv = (in.clip.xy - view.viewport.xy) / view.viewport.zw;
        let n = tex.xy * 2.0 - 1.0;
        let a = clamp(in.color.a, 0.0, 1.0);
        let depth_u = max(in.depth / 0.0254, 1.0);
        let bent_uv = clamp(screen_uv + n * vec2(0.5, -0.5) * params.refract / depth_u, vec2(0.0), vec2(1.0));
        let scene = textureSampleLevel(view_transmission_texture, view_transmission_sampler, screen_uv, 0.0).rgb;
        let bent = textureSampleLevel(view_transmission_texture, view_transmission_sampler, bent_uv, 0.0).rgb;
        return vec4(mix(scene, bent, a), 1.0);
    }
    var g = tex.rgb * in.color.rgb * params.overbright;
    if (params.add_self > 0.0) {
        g = g + tex.rgb * params.add_self * in.color.a;
    }
    var a = tex.a;
    if ((params.flags & 2u) != 0u) {
        // $distancealpha: the alpha channel is a distance field; threshold it.
        a = smoothstep(0.42, 0.58, tex.a);
    }
    a = a * in.color.a;
    let lin = srgb_to_linear(max(g, vec3(0.0)));
    // Premultiplied output: additive materials write alpha 0 (pure add).
    var out = vec4(lin * a, a);
    if ((params.flags & 1u) != 0u) {
        out = vec4(lin * a, 0.0);
    }
#ifdef TONEMAP_IN_SHADER
    out = vec4(tone_mapping(vec4(out.rgb, 1.0), view.color_grading).rgb, out.a);
#endif
    return out;
}
