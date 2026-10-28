// The water's reflection (water.rs, Extra High in raster): a skin over the surface showing the mirror camera's
// picture of the room, as much of it as Fresnel says (little looking down, nearly all at a glance), shifted by the
// ripples' tilt so it wobbles. Its normals the water's own (the same normal map). From under the water, the pool mirrored
// outside Snell's window.
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::main_pass_post_lighting_processing,
    mesh_view_bindings as view_bindings,
};

struct Mirror {
    // The mirror camera's clip from world.
    clip_from_world: mat4x4<f32>,
    // x: how far (m) the ripples' tilt shifts what's seen; y: how far over the water what's seen through it from below is
    // taken to be (its bend's).
    k: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> mirror: Mirror;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var picture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var picture_sampler: sampler;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let pbr = pbr_input_from_standard_material(in, is_front);
    let n = pbr.N;
    var f = 0.02 + 0.98 * pow(1.0 - saturate(dot(n, pbr.V)), 5.0);
    if !is_front {
        // From below (n the eye's way): water into air bends away, past ~49° none gets out (all mirrored); inside Snell's
        // window Fresnel by the angle out.
        let c = saturate(dot(n, pbr.V));
        let s = 1.333 * sqrt(1.0 - c * c);
        let out = sqrt(max(1.0 - s * s, 0.0));
        f = select(0.02 + 0.98 * pow(1.0 - out, 5.0), 1.0, s >= 1.0);
        f = mix(f, 1.0, smoothstep(0.93, 1.0, s));
    }
    let at = mirror.clip_from_world * vec4(in.world_position.xyz + vec3(n.xy * mirror.k.x, 0.0), 1.0);
    let uv = at.xy / at.w * vec2(0.5, -0.5) + 0.5;
    var color = vec4(textureSample(picture, picture_sampler, uv).rgb, f);
    @if(VIEW_TRANSMISSION_TEXTURE) {
        if !is_front && f < 1.0 {
            color = vec4(mix(seen_above(in, n, pbr.V), color.rgb, f), 1.0);
        }
    }
    var out: FragmentOutput;
    out.color = main_pass_post_lighting_processing(pbr, color);
    return out;
}

// Through Snell's window: what's over the water, bent out of it (the room's picture before the water, shifted as far as
// the bent way's point a little up is from here on screen, easing off toward the picture's edge).
@if(VIEW_TRANSMISSION_TEXTURE)
fn seen_above(in: VertexOutput, n: vec3<f32>, V: vec3<f32>) -> vec3<f32> {
    let t = refract(-V, n, 1.333);
    let a = view_bindings::view.clip_from_world * vec4(in.world_position.xyz + t * mirror.k.y, 1.0);
    let b = view_bindings::view.clip_from_world * vec4(in.world_position.xyz, 1.0);
    let at = a.xy / a.w * vec2(0.5, -0.5) + 0.5;
    let edge = min(min(at.x, 1.0 - at.x), min(at.y, 1.0 - at.y));
    let shift = (a.xy / a.w - b.xy / b.w) * vec2(0.5, -0.5) * smoothstep(0.0, 0.08, edge);
    let uv = (in.position.xy + shift * view_bindings::view.viewport.zw) / vec2<f32>(textureDimensions(view_bindings::view_transmission_texture));
    return textureSampleLevel(view_bindings::view_transmission_texture, view_bindings::view_transmission_sampler, uv, 0.0).rgb;
}
