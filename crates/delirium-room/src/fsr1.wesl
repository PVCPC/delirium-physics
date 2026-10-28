// The 3D view drawn below full resolution, stretched over the window: plainly (bilinear), or with AMD FidelityFX
// Super Resolution 1 (EASU's upscale, then RCAS's sharpening). EASU and RCAS ported from AMD's ffx_fsr1.h (FP32 paths):
//
// Copyright (c) 2021 Advanced Micro Devices, Inc. All rights reserved.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the following conditions:
// The above copyright notice and this permission notice shall be included in all copies or substantial portions of the
// Software.
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
// WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
// COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;

// xy: the image's size in pixels; z: RCAS's sharpness (2^-stops, 0 = no RCAS); w: 1 = FSR, 0 = bilinear.
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> p: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var img: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var img_sampler: sampler;

// A texel, clamped to the image, in gamma 2 (EASU and RCAS want perceptual values; the image reads back linear).
fn tap(t: vec2<i32>) -> vec3<f32> {
    return sqrt(max(textureLoad(img, clamp(t, vec2(0), vec2<i32>(p.xy) - 1), 0).rgb, vec3(0.0)));
}

fn luma(c: vec3<f32>) -> f32 {
    return c.b * 0.5 + (c.r * 0.5 + c.g);
}

fn easu_set(w: f32, la: f32, lb: f32, lc: f32, ld: f32, le: f32, dir: ptr<function, vec2<f32>>, len: ptr<function, f32>) {
    let dc = ld - lc;
    let cb = lc - lb;
    let dir_x = ld - lb;
    let len_x = saturate(abs(dir_x) / max(max(abs(dc), abs(cb)), 1e-8));
    let ec = le - lc;
    let ca = lc - la;
    let dir_y = le - la;
    let len_y = saturate(abs(dir_y) / max(max(abs(ec), abs(ca)), 1e-8));
    *dir += vec2(dir_x, dir_y) * w;
    *len += (len_x * len_x + len_y * len_y) * w;
}

fn easu_tap(off: vec2<f32>, dir: vec2<f32>, len: vec2<f32>, lob: f32, clp: f32, c: vec3<f32>, ac: ptr<function, vec3<f32>>, aw: ptr<function, f32>) {
    let v = vec2(off.x * dir.x + off.y * dir.y, off.x * -dir.y + off.y * dir.x) * len;
    let d2 = min(dot(v, v), clp);
    var wb = 2.0 / 5.0 * d2 - 1.0;
    var wa = lob * d2 - 1.0;
    wb *= wb;
    wa *= wa;
    wb = 25.0 / 16.0 * wb - (25.0 / 16.0 - 1.0);
    let w = wb * wa;
    *ac += c * w;
    *aw += w;
}

// EASU at an output position `uv` (0..1 over the window), in gamma 2.
fn easu(uv: vec2<f32>) -> vec3<f32> {
    var pp = uv * p.xy - 0.5;
    let fp = floor(pp);
    pp -= fp;
    let f0 = vec2<i32>(fp);
    //    b c
    //  e f g h
    //  i j k l
    //    n o
    let b = tap(f0 + vec2(0, -1));
    let c = tap(f0 + vec2(1, -1));
    let e = tap(f0 + vec2(-1, 0));
    let f = tap(f0);
    let g = tap(f0 + vec2(1, 0));
    let h = tap(f0 + vec2(2, 0));
    let i = tap(f0 + vec2(-1, 1));
    let j = tap(f0 + vec2(0, 1));
    let k = tap(f0 + vec2(1, 1));
    let l = tap(f0 + vec2(2, 1));
    let n = tap(f0 + vec2(0, 2));
    let o = tap(f0 + vec2(1, 2));
    let bl = luma(b); let cl = luma(c); let el = luma(e); let fl = luma(f); let gl = luma(g); let hl = luma(h);
    let il = luma(i); let jl = luma(j); let kl = luma(k); let ll = luma(l); let nl = luma(n); let ol = luma(o);
    var dir = vec2(0.0);
    var len = 0.0;
    easu_set((1.0 - pp.x) * (1.0 - pp.y), bl, el, fl, gl, jl, &dir, &len);
    easu_set(pp.x * (1.0 - pp.y), cl, fl, gl, hl, kl, &dir, &len);
    easu_set((1.0 - pp.x) * pp.y, fl, il, jl, kl, nl, &dir, &len);
    easu_set(pp.x * pp.y, gl, jl, kl, ll, ol, &dir, &len);
    let dir_r = dot(dir, dir);
    let zero = dir_r < 1.0 / 32768.0;
    dir = select(dir * inverseSqrt(dir_r), vec2(1.0, 0.0), zero);
    len *= 0.5;
    len *= len;
    let stretch = dot(dir, dir) / max(abs(dir.x), abs(dir.y));
    let len2 = vec2(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    let lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    let clp = 1.0 / lob;
    let mn = min(min(f, g), min(j, k));
    let mx = max(max(f, g), max(j, k));
    var ac = vec3(0.0);
    var aw = 0.0;
    easu_tap(vec2(0.0, -1.0) - pp, dir, len2, lob, clp, b, &ac, &aw);
    easu_tap(vec2(1.0, -1.0) - pp, dir, len2, lob, clp, c, &ac, &aw);
    easu_tap(vec2(-1.0, 1.0) - pp, dir, len2, lob, clp, i, &ac, &aw);
    easu_tap(vec2(0.0, 1.0) - pp, dir, len2, lob, clp, j, &ac, &aw);
    easu_tap(vec2(0.0, 0.0) - pp, dir, len2, lob, clp, f, &ac, &aw);
    easu_tap(vec2(-1.0, 0.0) - pp, dir, len2, lob, clp, e, &ac, &aw);
    easu_tap(vec2(1.0, 1.0) - pp, dir, len2, lob, clp, k, &ac, &aw);
    easu_tap(vec2(2.0, 1.0) - pp, dir, len2, lob, clp, l, &ac, &aw);
    easu_tap(vec2(2.0, 0.0) - pp, dir, len2, lob, clp, h, &ac, &aw);
    easu_tap(vec2(1.0, 0.0) - pp, dir, len2, lob, clp, g, &ac, &aw);
    easu_tap(vec2(1.0, 2.0) - pp, dir, len2, lob, clp, o, &ac, &aw);
    easu_tap(vec2(0.0, 2.0) - pp, dir, len2, lob, clp, n, &ac, &aw);
    return min(mx, max(mn, ac / aw));
}

const RCAS_LIMIT: f32 = 0.25 - 1.0 / 16.0;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    if p.w < 0.5 {
        return vec4(textureSample(img, img_sampler, in.uv).rgb, 1.0);
    }
    let e = easu(in.uv);
    if p.z <= 0.0 {
        return vec4(e * e, 1.0);
    }
    // RCAS on EASU's output: its four neighbours upscaled here too.
    // ponytail: 5 EASUs a pixel with sharpening on; an EASU target and a second pass if a slow card needs it.
    let px = fwidth(in.uv); // an output pixel
    let b = easu(in.uv + vec2(0.0, -px.y));
    let d = easu(in.uv + vec2(-px.x, 0.0));
    let f = easu(in.uv + vec2(px.x, 0.0));
    let h = easu(in.uv + vec2(0.0, px.y));
    let mn4 = min(min(b, d), min(f, h));
    let mx4 = max(max(b, d), max(f, h));
    let hit_min = min(mn4, e) / max(4.0 * mx4, vec3(1e-6));
    let hit_max = (1.0 - max(mx4, e)) / min(4.0 * mn4 - 4.0, vec3(-1e-6));
    let lobe3 = max(-hit_min, hit_max);
    let lobe = max(-RCAS_LIMIT, min(max(lobe3.r, max(lobe3.g, lobe3.b)), 0.0)) * p.z;
    let c = (lobe * (b + d + f + h) + e) / (4.0 * lobe + 1.0);
    return vec4(c * c, 1.0);
}
