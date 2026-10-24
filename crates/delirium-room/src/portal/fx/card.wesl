// Particle cards (render.rs `SpriteCardMaterial`). Every vertex carries its particle's centre; the vertex shader spreads
// the corners out for the view being drawn, so a card faces every camera, the portal views' too.
//   shape = (corner x, corner y, radius, rotation): the corner's offset is (x, y) * radius, turned by rotation.
//   axis = (vector, mode): 0 faces the camera; 1 lies flat across the vector (a normal); 2 a strip along the vector,
//   widened across the view; 3 widened along the vector.
// The fragment shader draws texture x vertex colour x overbright, decoded from sRGB, premultiplied: additive cards add
// (alpha 0), others cover. A distance-field card thresholds its alpha smoothly. A refraction card bends the opaque-pass
// copy by its normal map.
import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings as vb,
    view_transformations::position_world_to_clip,
};

struct SpriteCardParams {
    overbright: f32,
    add_self: f32,
    // bit 0 additive, bit 1 distance-field alpha, bit 2 refraction.
    flags: u32,
    refract: f32,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> card: SpriteCardParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var card_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var card_smp: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
    @location(8) shape: vec4<f32>,
    @location(9) axis: vec4<f32>,
};

struct Varyings {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vertex(v: Vertex) -> Varyings {
    let centre = (mesh_functions::get_world_from_local(v.instance_index) * vec4(v.position, 1.0)).xyz;
    let rad = v.shape.z;
    let mode = i32(round(v.axis.w));
    let cam_right = vb::view.world_from_view[0].xyz;
    let cam_up = vb::view.world_from_view[1].xyz;
    var off: vec3<f32>;
    if mode == 2 {
        let side = cross(v.axis.xyz, vb::view.world_position - centre);
        let l = length(side);
        off = select(cam_right, side / l, l > 1e-6) * v.shape.x * rad;
    } else if mode == 3 {
        off = v.axis.xyz * v.shape.x * rad;
    } else {
        var right = cam_right;
        var up = cam_up;
        if mode == 1 {
            let n = normalize(v.axis.xyz);
            let refv = select(vec3(0.0, 0.0, 1.0), vec3(1.0, 0.0, 0.0), abs(n.z) > 0.9);
            right = normalize(cross(refv, n));
            up = cross(n, right);
        }
        let c = cos(v.shape.w);
        let s = sin(v.shape.w);
        let k = vec2(c * v.shape.x - s * v.shape.y, s * v.shape.x + c * v.shape.y) * rad;
        off = right * k.x + up * k.y;
    }
    var out: Varyings;
    out.position = position_world_to_clip(centre + off);
    out.uv = v.uv;
    out.color = v.color;
    return out;
}

@fragment
fn fragment(in: Varyings) -> @location(0) vec4<f32> {
    let tex = textureSample(card_tex, card_smp, in.uv);
    if (card.flags & 4u) != 0u {
        // Refraction: the scene behind, shifted by the normal map's tilt, blended with the unshifted scene by alpha.
        var scene = vec4(0.0);
        @if(VIEW_TRANSMISSION_TEXTURE) {
            let dims = vec2<f32>(textureDimensions(vb::view_transmission_texture));
            let at = in.position.xy / dims;
            let n = tex.xy * 2.0 - 1.0;
            let bent = at + vec2(n.x, -n.y) * card.refract * 0.1 * in.color.a;
            scene = vec4(textureSampleLevel(vb::view_transmission_texture, vb::view_transmission_sampler, bent, 0.0).rgb, 1.0);
        }
        @if(!VIEW_TRANSMISSION_TEXTURE) {
            discard;
        }
        return scene;
    }
    var a = tex.a;
    if (card.flags & 2u) != 0u {
        let w = clamp(fwidth(tex.a), 0.01, 0.25);
        a = smoothstep(0.5 - w, 0.5 + w, tex.a);
    }
    a = a * clamp(in.color.a, 0.0, 1.0);
    let srgb = tex.rgb * in.color.rgb * card.overbright + tex.rgb * card.add_self;
    let rgb = pow(max(srgb, vec3(0.0)), vec3(2.2)) * a;
    return vec4(rgb, select(a, 0.0, (card.flags & 1u) != 0u));
}
