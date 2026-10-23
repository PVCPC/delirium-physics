// Portal surface: an oval hole that grows open, shows the view through the portal, and burns at its
// edge in the portal's colour (the game's own noise and colour-ramp textures, read from the user's
// install). Our own simple look; the room's camera and portal views are set up in portal/render.rs.
//
// The view through the portal is a render target from the portal's view camera, sampled at this
// pixel's screen position (it uses the main view's viewport, so the pixels line up).
//
// Variants (from `flags` and the pipeline key):
//   * live:  the normal surface; opaque while opening (reads the opaque-pass copy for the wall
//            around the rim), alpha-blended once fully open;
//   * cap:   the box behind the opening used when the eye is inside the near plane: just the view.
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::{view, globals, view_transmission_texture, view_transmission_sampler};
@if(TONEMAP_IN_SHADER)
import bevy_core_pipeline::tonemapping::tone_mapping;

struct PortalSurfaceParams {
    // 0 closed .. 1 open.
    open_amount: f32,
    // 0 while unlinked (a coloured membrane covers the hole) .. 1 linked.
    portal_active: f32,
    // Brightness of the edge colour.
    color_scale: f32,
    // bit 0: has a view texture; bit 1: cap; bit 3: depth doubler; bit 4: blended.
    flags: u32,
    // Depth doubler: view UV = screen UV * xy + zw.
    uv_remap: vec4<f32>,
};

const FLAG_HAS_VIEW: u32 = 1u;
const FLAG_CAP: u32 = 2u;
const FLAG_DOUBLER: u32 = 8u;
const FLAG_BLEND: u32 = 16u;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> params: PortalSurfaceParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var view_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var view_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var noise_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var noise_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var ramp_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var ramp_sampler: sampler;

fn finish(c: vec4<f32>) -> vec4<f32> {
    @if(TONEMAP_IN_SHADER) {
    return tone_mapping(c, view.color_grading);
    } @else {
    return c;
    }
}

fn ramp(u: f32) -> vec3<f32> {
    return saturate(textureSampleLevel(ramp_texture, ramp_sampler, vec2(saturate(u), 0.5), 0.0).rgb * params.color_scale);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // The main pass's own size: with an upscaler it's drawn smaller than the viewport.
    let screen_uv = (in.position.xy - view.main_pass_viewport.xy) / view.main_pass_viewport.zw;
    let has_view = (params.flags & FLAG_HAS_VIEW) != 0u;
    var view_uv = screen_uv;
    if ((params.flags & FLAG_DOUBLER) != 0u) {
        view_uv = screen_uv * params.uv_remap.xy + params.uv_remap.zw;
    }
    if ((params.flags & FLAG_CAP) != 0u) {
        if (!has_view) {
            discard;
        }
        return finish(vec4(textureSampleLevel(view_texture, view_sampler, view_uv, 0.0).rgb, 1.0));
    }

    @if(VERTEX_UVS_A)
    let uv = in.uv;
    @else
    let uv = vec2(0.5);
    // Distance from the centre of the oval: 1 at the quad's edge.
    let d = length((uv - 0.5) * 2.0);
    let open = saturate(params.open_amount);
    let t = globals.time;
    // Two scrolling reads of the noise make the edge flicker.
    let n = 0.5 * (textureSampleLevel(noise_texture, noise_sampler, uv * 1.5 + vec2(0.0, -t * 0.2), 0.0).r
        + textureSampleLevel(noise_texture, noise_sampler, uv * 2.3 + vec2(t * 0.13, 0.0), 0.0).r);
    let hole = 0.9 * open;
    let edge = hole + 0.06 + 0.06 * n;

    if (d < hole) {
        var c = ramp(0.3 + 0.4 * n);
        if (has_view) {
            let through = textureSampleLevel(view_texture, view_sampler, view_uv, 0.0).rgb;
            c = mix(c, through, saturate(params.portal_active));
        }
        // A thin glow just inside the edge.
        let glow = smoothstep(hole * 0.85, hole, d) * 0.6;
        return finish(vec4(mix(c, ramp(0.9), glow), 1.0));
    }
    if (d > edge) {
        discard;
    }
    // The burning edge: brightest at the hole, fading out.
    let k = 1.0 - (d - hole) / (edge - hole);
    let flame = finish(vec4(ramp(k * (0.6 + 0.4 * n)), 1.0)).rgb;
    let a = saturate(k * 1.2);
    if ((params.flags & FLAG_BLEND) != 0u) {
        return vec4(flame, a);
    }
    let wall = textureSampleLevel(view_transmission_texture, view_transmission_sampler, in.position.xy / vec2<f32>(textureDimensions(view_transmission_texture)), 0.0).rgb;
    return vec4(mix(wall, flame, a), 1.0);
}
