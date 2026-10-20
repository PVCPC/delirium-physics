// Portal surface: a port of Portal 2's PortalRefract as drawn on
// `portalstaticoverlay_N` (stage 2: the burning rim), the stage-1 hole, and
// the stage-0 refraction of the wall while the opening grows. Valve's public
// SDK 2013 portal_refract_vs20 / portal_refract_ps2x were the starting point.
//
// Inside the hole we show the view through the portal: a render target
// written by the portal's view camera, sampled at this pixel's screen
// position (the view camera uses the same viewport as the main view, so the
// pixels line up exactly).
//
// Three variants share this shader (selected by `flags` and the pipeline key):
//   * live:   the normal surface (quad, alpha blended, depth tested);
//   * cap:    the shallow box behind the opening that takes over while the
//             eye is inside the near-plane distance (depth ignored, drawn
//             last, opaque output);
//   * writez: unlinked and opening: depth only, inside the hole, no colour
//             (PortalRefract's `portal_N_anims` material).
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{view, globals, view_transmission_texture, view_transmission_sampler}
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#endif

struct PortalSurfaceParams {
    // $PortalOpenAmount, 0..1.
    open_amount: f32,
    // 1 - $PortalStatic (0 while the membrane covers everything).
    portal_active: f32,
    // $PortalColorScale.
    color_scale: f32,
    // bit 0: has a view texture; bit 1: cap; bit 2: writez; bit 3: depth doubler.
    flags: u32,
    // Depth doubler: view UV = screen UV * xy + zw.
    uv_remap: vec4<f32>,
};

const FLAG_HAS_VIEW: u32 = 1u;
const FLAG_CAP: u32 = 2u;
const FLAG_WRITEZ: u32 = 4u;
const FLAG_DOUBLER: u32 = 8u;
// Fully open: the rim is drawn alpha-blended over the frame (the screen copy is not available to blended materials).
const FLAG_BLEND: u32 = 16u;

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: PortalSurfaceParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var view_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var view_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var noise_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var noise_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var ramp_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var ramp_sampler: sampler;

fn finish(c: vec4<f32>) -> vec4<f32> {
#ifdef TONEMAP_IN_SHADER
    return tone_mapping(c, view.color_grading);
#else
    return c;
#endif
}

fn linearstep(lo: f32, hi: f32, v: f32) -> f32 {
    return saturate((v - lo) / (hi - lo));
}

fn smooth01(e0: f32, e1: f32, x: f32) -> f32 {
    // HLSL smoothstep; tolerate e0 == e1.
    let t = saturate((x - e0) / max(e1 - e0, 1e-6));
    return t * t * (3.0 - 2.0 * t);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let screen_uv = (in.position.xy - view.viewport.xy) / view.viewport.zw;
    let has_view = (params.flags & FLAG_HAS_VIEW) != 0u;
    // Where to read the view texture: the same screen position, or for the
    // depth doubler, last frame's image re-mapped into this opening.
    var view_uv = screen_uv;
    var view_scale = vec2(1.0);
    if ((params.flags & FLAG_DOUBLER) != 0u) {
        view_scale = params.uv_remap.xy;
        view_uv = screen_uv * params.uv_remap.xy + params.uv_remap.zw;
    }

    // The cap is the view and nothing else; it only exists to cover the
    // opening when the near plane cuts the quad.
    if ((params.flags & FLAG_CAP) != 0u) {
        if (!has_view) {
            discard;
        }
        return finish(vec4(textureSampleLevel(view_texture, view_sampler, view_uv, 0.0).rgb, 1.0));
    }

#ifdef VERTEX_UVS_A
    let base_uv = in.uv;
#else
    let base_uv = vec2(0.5);
#endif

    // ---- vertex stage (per pixel here; it is all linear) ----
    let kOuterBorder = 0.075;
    let kInnerBorder = kOuterBorder * 4.0;
    let uv0 = base_uv * (1.0 + kOuterBorder) - kOuterBorder * 0.5;
    let time = globals.time - floor(globals.time / 1000.0) * 1000.0;
    let open_vs = saturate(params.open_amount + 0.001);
    let noise_uv = ((base_uv - 0.5) / open_vs) + 0.5;
    let scroll = time * 0.0275;
    let noise_xy = noise_uv * 0.3 + vec2(scroll, 0.0);
    let noise_zw = noise_uv * 0.3 - vec2(scroll, 0.0);

    // ---- pixel stage ----
    let open = smooth01(0.0, 1.0, saturate(params.open_amount));
    let open2 = open * open;
    let stretch = uv0 * 2.0 - 1.0;
    let dist = length(stretch);
    let cutout = step(dist, open2);

    // writez (unlinked and opening): only the depth of the hole.
    if ((params.flags & FLAG_WRITEZ) != 0u) {
        if (cutout < 0.5) {
            discard;
        }
        return vec4(0.0);
    }

    let outer = (1.0 - linearstep(open2, open2 + kOuterBorder, dist)) * (1.0 - cutout);
    let inner = linearstep(open2 - kInnerBorder, open2, dist) * cutout;
    let p_active = saturate(params.portal_active);
    let fade_in = max(saturate(open * 2.5), 1.0 - p_active);
    var mask = (inner + outer) * fade_in;

    let n2 = textureSample(noise_texture, noise_sampler, noise_zw.yx - textureSample(noise_texture, noise_sampler, noise_xy).rg * 0.02);
    let n1 = textureSample(noise_texture, noise_sampler, noise_xy - n2.rg * 0.02);
    var noise = (n1.g + n2.g) * 0.5;
    let active_noise = smooth01(0.0, noise, p_active);
    let kBorderSoftness = 0.875;
    let border = 1.0 - smooth01(mask - kBorderSoftness, mask + kBorderSoftness, noise);
    noise = border;
    mask = mask * border;
    let transparency = saturate(mask + cutout * (1.0 - active_noise)) * 1.5;
    let shift = pow(abs(uv0.y), 1.5) * 0.8 + 0.2;
    let ramp_u = pow(noise, 0.5) * shift * transparency;
    // Portal 2 writes this colour (ramp x $PortalColorScale 4.0) into an
    // 8-bit target, so it is clamped to 1 before it is blended over the view;
    // unclamped, the 30%-deep inner glow turns into a solid band.
    let flame = saturate(textureSampleLevel(ramp_texture, ramp_sampler, vec2(ramp_u, 0.5), 0.0).rgb * params.color_scale);
    let a = min(transparency, 1.0);

    // ---- stage 0: the wall warps while the opening grows ----
    // PortalRefract's stage 0 refracts the frame buffer behind the rim. The
    // live surface reads Bevy's copy of the opaque pass (the "transmission"
    // texture) for that, offset by the noise, fading out as open -> 1.
    let warp_amt = 1.0 - open;
    let warp = (vec2(n1.r, n2.r) - 0.5) * 0.03 * warp_amt;
    let refr_uv = clamp(view_uv + warp * view_scale, vec2(0.0), vec2(1.0));
    let scene_uv = clamp(screen_uv + warp, vec2(0.0), vec2(1.0));
    let scene = textureSampleLevel(view_transmission_texture, view_transmission_sampler, screen_uv, 0.0).rgb;
    let scene_refr = textureSampleLevel(view_transmission_texture, view_transmission_sampler, scene_uv, 0.0).rgb;

    if (cutout > 0.5) {
        // The hole: the view through the portal, then the overlay on top.
        var base = vec3(0.0);
        if (has_view) {
            base = textureSampleLevel(view_texture, view_sampler, refr_uv, 0.0).rgb;
        }
        return finish(vec4(mix(base, flame, a), 1.0));
    }

    // The rim ring: stage 0 (refraction, under) then stage 2 (flame, over),
    // composited here over the scene copy and written opaque.
    let a0 = outer * warp_amt;
    if (a + a0 <= 1.0 / 255.0) {
        // Alpha test > 1/255 as in the game: leave the wall untouched.
        discard;
    }
    // The scene copy is already in the camera's output space (this app's main camera tone-maps in
    // each material), so only the flame is tone-mapped here; doing both darkens a halo round the rim.
    let flame_out = finish(vec4(flame, 1.0)).rgb;
    if ((params.flags & FLAG_BLEND) != 0u) {
        // Fully open: the rim is just the flame blended over what is already drawn. (Re-reading a
        // copy of the opaque pass here never matches the final image exactly and leaves a pale ring.)
        return vec4(flame_out, a);
    }
    let under = mix(scene, scene_refr, saturate(a0));
    return vec4(mix(under, flame_out, a), 1.0);
}
