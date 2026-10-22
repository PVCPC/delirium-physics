// Ghost clip shading (p2_body): Bevy's standard PBR lighting, with up to four
// world-space clip planes. A fragment behind any enabled plane is discarded,
// so the original body is cut where it has gone through a portal and the
// ghost is cut where it has not yet come out.
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
}

struct ClipPlanes {
    // (normal.xyz, d): a point p is clipped when dot(normal, p) + d < 0.
    // A disabled plane is (0, 0, 0, 1), which never clips.
    planes: array<vec4<f32>, 4>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> clip: ClipPlanes;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    for (var i = 0u; i < 4u; i = i + 1u) {
        let p = clip.planes[i];
        if dot(p.xyz, in.world_position.xyz) + p.w < 0.0 {
            discard;
        }
    }
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
