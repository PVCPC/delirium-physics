// Real water's spill as Clear spill's film in raster (fluid_gpu.rs): premultiplied, its own light only added (its
// reflection: sky and sun) and the floor under it darkened by its alpha (wet). Mirror smooth where it's still, the fine
// ripples' map (TILE square) as strong as the water's quick there (uv_b.x, m/s: full from FLOW); white where water lands
// on it (uv_b.y: Impact foam), lit as foam.
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var ripples: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var ripples_sampler: sampler;
// x: the ripples on, y: Impact foam on.
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<uniform> k: vec4<f32>;

// The map's tile (m: fluid_gpu.rs's RIPPLE_TILE), the speed the ripples are full at (m/s), how white the foam.
const TILE: f32 = 2.0;
const FLOW: f32 = 0.8;
const FOAM: f32 = 0.85;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, is_front);
    var s = 0.0;
    var w = 0.0;
    @if(VERTEX_UVS_B) {
        s = smoothstep(0.0, 1.0, in.uv_b.x / FLOW) * k.x;
        w = clamp(in.uv_b.y, 0.0, 1.0) * k.y;
    }
    // (The map's normal in its own uv: u along x, v along -y.)
    let r = textureSample(ripples, ripples_sampler, vec2(in.world_position.x, -in.world_position.y) / TILE).xy * 2.0 - 1.0;
    pbr.N = normalize(pbr.N + vec3(r.x, -r.y, 0.0) * s);
    let a = pbr.material.base_color.a;
    pbr.material.base_color = vec4(vec3(w * FOAM), max(a, w * FOAM));
    pbr.material.perceptual_roughness = mix(pbr.material.perceptual_roughness, 0.8, w);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr);
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
}
