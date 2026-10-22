// Particle cards: each vertex carries its particle's centre, and the vertex shader spreads the
// corners out per view so cards face every camera (including the virtual cameras behind portals).
//   shape = (corner x, corner y, radius, rotation)
//   axis  = (vector, mode): 0 faces the camera, 1 lies flat across the vector (a normal),
//           2 is a strip along the vector, widened across the view, 3 is widened along the vector.
#import bevy_pbr::mesh_view_bindings::{view, view_transmission_texture, view_transmission_sampler}
#import bevy_pbr::mesh_functions::get_world_from_local
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct CardParams {
    overbright: f32,
    add_self: f32,
    // bit 0: additive, bit 1: alpha is a distance field, bit 2: refraction (texture is a normal map).
    flags: u32,
    refract: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: CardParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var card_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var card_sampler: sampler;

struct CardIn {
    @builtin(instance_index) instance: u32,
    @location(0) centre: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(8) shape: vec4<f32>,
    @location(9) axis: vec4<f32>,
};

struct CardOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vertex(v: CardIn) -> CardOut {
    let centre = (get_world_from_local(v.instance) * vec4(v.centre, 1.0)).xyz;
    let to_eye = normalize(view.world_position - centre);
    let mode = u32(v.axis.w + 0.5);
    var across = view.world_from_view[0].xyz;
    var along = view.world_from_view[1].xyz;
    if (mode == 1u) {
        let n = normalize(v.axis.xyz);
        var helper = vec3(0.0, 1.0, 0.0);
        if (abs(n.y) > 0.9) {
            helper = vec3(1.0, 0.0, 0.0);
        }
        across = normalize(cross(helper, n));
        along = cross(n, across);
    } else if (mode == 2u) {
        across = normalize(cross(v.axis.xyz, to_eye));
        along = vec3(0.0);
    } else if (mode == 3u) {
        across = v.axis.xyz;
        along = vec3(0.0);
    }
    // Spin the corner by the particle's rotation (strips have no y corner and no rotation).
    let s = sin(v.shape.w);
    let c = cos(v.shape.w);
    let corner = vec2(v.shape.x * c - v.shape.y * s, v.shape.x * s + v.shape.y * c) * v.shape.z;
    let world = centre + across * corner.x + along * corner.y;
    var out: CardOut;
    out.clip = view.clip_from_world * vec4(world, 1.0);
    out.uv = v.uv;
    out.color = v.color;
    return out;
}

fn to_output(rgb: vec3<f32>) -> vec3<f32> {
#ifdef TONEMAP_IN_SHADER
    return tone_mapping(vec4(rgb, 1.0), view.color_grading).rgb;
#else
    return rgb;
#endif
}

@fragment
fn fragment(i: CardOut) -> @location(0) vec4<f32> {
    let t = textureSample(card_texture, card_sampler, i.uv);
    if ((params.flags & 4u) != 0u) {
        // Refraction: nudge where the scene behind is read by the normal map, faded by alpha.
        let screen_uv = (i.clip.xy - view.viewport.xy) / view.viewport.zw;
        let bend = (t.xy * 2.0 - 1.0) * params.refract * 0.1;
        let plain = textureSampleLevel(view_transmission_texture, view_transmission_sampler, screen_uv, 0.0).rgb;
        let bent = textureSampleLevel(view_transmission_texture, view_transmission_sampler, clamp(screen_uv + bend, vec2(0.0), vec2(1.0)), 0.0).rgb;
        return vec4(mix(plain, bent, saturate(t.a * i.color.a)), 1.0);
    }
    var a = t.a;
    if ((params.flags & 2u) != 0u) {
        a = smoothstep(0.4, 0.6, t.a);
    }
    a = saturate(a * i.color.a);
    // Colours are in gamma space: scale, then bring them to linear light.
    let gamma = t.rgb * i.color.rgb * params.overbright + t.rgb * params.add_self;
    let rgb = to_output(pow(max(gamma, vec3(0.0)), vec3(2.2)));
    if ((params.flags & 1u) != 0u) {
        // Additive: premultiplied with zero alpha adds onto what is there.
        return vec4(rgb * a, 0.0);
    }
    return vec4(rgb * a, a);
}
