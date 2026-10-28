// Real water's spill's caustics in raster (fluid_gpu.rs): the sun's net through its fine ripples (the map, TILE square:
// each texel the light there over flat water's, a quarter of it), lit as the floor under it and added, faint (a sunlit
// sheet's: soft, thin, bright wandering lines). Carried along by the water's own speed there (uv_b, m/s) as a flow map:
// each of two phases slides the net along and starts over, the two crossfaded half a period apart, so neither's restart
// shows.
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var net: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var net_sampler: sampler;

// The map's tile (m: fluid_gpu.rs's RIPPLE_TILE), a phase's length (s), how bright the net (its light over flat water's).
const TILE: f32 = 2.0;
const PERIOD: f32 = 1.5;
const GAIN: f32 = 0.2;

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr = pbr_input_from_standard_material(in, is_front);
    let p = vec2(in.world_position.x, -in.world_position.y);
    var v = vec2(0.0);
    @if(VERTEX_UVS_B) {
        v = vec2(in.uv_b.x, -in.uv_b.y);
    }
    let t = globals.time / PERIOD;
    var l = 0.0;
    for (var i = 0; i < 2; i++) {
        let tau = fract(t + 0.5 * f32(i));
        // (Each phase starts from its own place on the map: no two restarts alike.)
        let start = vec2(0.37, 0.61) * floor(t + 0.5 * f32(i));
        l += (1.0 - abs(2.0 * tau - 1.0)) * textureSample(net, net_sampler, (p - v * tau * PERIOD) / TILE + start).r * 4.0;
    }
    // Only the light over flat water's (the bright net).
    pbr.material.base_color = vec4(pbr.material.base_color.rgb * max(l - 1.0, 0.0) * GAIN, pbr.material.base_color.a);
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr);
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
}
