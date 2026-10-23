import bevy_core_pipeline::tonemapping::tonemapping_luminance as luminance;
import bevy_pbr::render::utils::{rand_f, rand_vec2f};
import bevy_render::view::View;
import bevy_pbr::prepass::bindings::PreviousViewUniforms;
import bevy_pbr::render::pbr_functions::{calculate_diffuse_color, calculate_F0};
import package::scene::brdf::{evaluate_brdf, evaluate_and_sample_brdf, brdf_pdf, F_AB};
import package::scene::sampling::{sample_random_light, random_emissive_light_pdf, power_heuristic, sample_environment_map_light};
import package::scene::bindings::{trace_ray, resolve_ray_hit_full, RAY_T_MIN, RAY_T_MAX, MIRROR_ROUGHNESS_THRESHOLD};

enable wgpu_ray_query;

@group(1) @binding(0) var accumulation_texture: texture_storage_2d<rgba32float, read_write>;
@group(1) @binding(1) var view_output: texture_storage_2d<rgba16float, write>;
@group(1) @binding(2) var<uniform> view: View;
@group(1) @binding(3) var<uniform> previous_view: PreviousViewUniforms;

// Delirium: DLSS Ray Reconstruction cleans up one sample a frame (as games' path tracing modes do), so the picture holds
// up while the camera moves; it needs what the first surface is (albedos, normal, roughness, depth, motion).
@if(DLSS_RR) {
@group(2) @binding(0) var diffuse_albedo: texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(1) var specular_albedo: texture_storage_2d<rgba8unorm, write>;
@group(2) @binding(2) var normal_roughness: texture_storage_2d<rgba16float, write>;
@group(2) @binding(3) var dlss_rr_depth: texture_storage_2d<r32float, write>;
@group(2) @binding(4) var specular_motion_vectors: texture_storage_2d<rg16float, write>;
}

@compute @workgroup_size(8, 8, 1)
fn pathtrace(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if any(global_id.xy >= vec2u(view.viewport.zw)) {
        return;
    }

    let old_color = textureLoad(accumulation_texture, global_id.xy);

    // Setup RNG
    let pixel_index = global_id.x + global_id.y * u32(view.viewport.z);
    let frame_index = u32(old_color.a) * 5782582u;
    var rng = pixel_index + frame_index;

    // Shoot the first ray from the camera (Ray Reconstruction: through the pixel's centre, the camera jitters for it)
    let pixel_center = vec2<f32>(global_id.xy) + 0.5;
    var jitter = rand_vec2f(&rng) - 0.5;
    @if(DLSS_RR) {
        jitter = vec2(0.0);
        rng = pixel_index + u32(view.frame_count) * 5782582u;
    }
    let pixel_uv = (pixel_center + jitter) / view.viewport.zw;
    let pixel_ndc = (pixel_uv * 2.0) - 1.0;
    let primary_ray_target = view.world_from_clip * vec4(pixel_ndc.x, -pixel_ndc.y, 1.0, 1.0);
    var ray_origin = view.world_position;
    var ray_direction = normalize((primary_ray_target.xyz / primary_ray_target.w) - ray_origin);
    var ray_t_min = 0.0;

    // Path trace
    var radiance = vec3(0.0);
    var throughput = vec3(1.0);
    var p_bounce = 0.0;
    var glass_hits = 0u;
    var first = true;
    var guided = false;
    loop {
        let ray = trace_ray(ray_origin, ray_direction, ray_t_min, RAY_T_MAX, RAY_FLAG_NONE);
        if ray.kind != RAY_QUERY_INTERSECTION_NONE {
            let ray_hit = resolve_ray_hit_full(ray);
            @if(DLSS_RR) {
                if first {
                    let clip = view.clip_from_world * vec4(ray_hit.world_position, 1.0);
                    textureStore(dlss_rr_depth, global_id.xy, vec4(clip.z / clip.w));
                }
                // The guides: the first surface that isn't glass (seen through it, or in it).
                if !guided && ray_hit.material.glass <= 0.0 {
                    guided = true;
                    let m = ray_hit.material;
                    let F0 = calculate_F0(m.base_color, m.metallic, vec3(m.reflectance));
                    textureStore(diffuse_albedo, global_id.xy, vec4(calculate_diffuse_color(m.base_color, m.metallic, 0.0, 0.0), 0.0));
                    textureStore(specular_albedo, global_id.xy, vec4(env_brdf_approx2(F0, m.roughness, ray_hit.world_normal, -ray_direction), 0.0));
                    textureStore(normal_roughness, global_id.xy, vec4(ray_hit.world_normal, m.perceptual_roughness));
                    textureStore(specular_motion_vectors, global_id.xy, vec4(motion_vector(ray_hit.world_position), vec2(0.0)));
                }
            }
            first = false;

            // Delirium: glass, smooth: reflected (Fresnel's share) or refracted through, tinted on the way in (as its
            // shadows are). What it shows next counts in full, like a mirror's.
            if ray_hit.material.glass > 0.0 {
                glass_hits += 1u;
                if glass_hits > 16u { break; }
                let outward = ray_hit.geometric_world_normal;
                let entering = dot(ray_direction, outward) < 0.0;
                let n = select(-outward, outward, entering);
                let eta = select(ray_hit.material.ior, 1.0 / ray_hit.material.ior, entering);
                let refracted = refract(ray_direction, n, eta);
                let f0 = pow((1.0 - eta) / (1.0 + eta), 2.0);
                let fresnel = select(f0 + (1.0 - f0) * pow(1.0 - saturate(-dot(ray_direction, n)), 5.0), 1.0, all(refracted == vec3(0.0)));
                if rand_f(&rng) < fresnel {
                    ray_direction = reflect(ray_direction, n);
                    ray_origin = ray_hit.world_position + n * RAY_T_MIN;
                } else {
                    ray_direction = refracted;
                    ray_origin = ray_hit.world_position - n * RAY_T_MIN;
                    if entering {
                        throughput *= mix(vec3(1.0), ray_hit.material.base_color, ray_hit.material.alpha);
                    }
                }
                ray_t_min = RAY_T_MIN;
                p_bounce = 0.0;
                continue;
            }
            let wo = -ray_direction;
            let NdotV = max(dot(ray_hit.world_normal, wo), 0.0001);
            let F_ab = F_AB(ray_hit.material.perceptual_roughness, NdotV);

            // Emissive contribution
            var mis_weight = 1.0;
            if p_bounce != 0.0 { // Not first bounce
                let p_light = random_emissive_light_pdf(ray_hit, ray.t, NdotV);
                mis_weight = power_heuristic(p_bounce, p_light);
            }
            if dot(ray_hit.world_normal, wo) > 0.0 {
                radiance += mis_weight * throughput * ray_hit.material.emissive;
            }

            // Sample direct lighting, but only if the surface is not mirror-like
            let is_perfectly_specular = ray_hit.material.roughness <= MIRROR_ROUGHNESS_THRESHOLD && ray_hit.material.metallic > 0.9999;
            if !is_perfectly_specular {
                let direct_lighting = sample_random_light(ray_hit.world_position, ray_hit.world_normal, &rng);

                mis_weight = 1.0;
                if direct_lighting.brdf_rays_can_hit {
                    let pdf_of_bounce = brdf_pdf(wo, direct_lighting.wi, ray_hit.world_normal, ray_hit.material, F_ab);
                    mis_weight = power_heuristic(1.0 / direct_lighting.inverse_solid_angle_pdf, pdf_of_bounce);
                }

                let direct_lighting_brdf = evaluate_brdf(wo, direct_lighting.wi, ray_hit.world_normal, ray_hit.material, F_ab);
                radiance += mis_weight * throughput * direct_lighting.radiance * direct_lighting.inverse_pdf * direct_lighting_brdf;
            }

            // Sample new ray direction from the material BRDF for next bounce and apply BRDF
            let next_bounce = evaluate_and_sample_brdf(wo, ray_hit.world_normal, ray_hit.material, F_ab, &rng);
            if next_bounce.pdf == 0.0 { break; }
            ray_direction = next_bounce.wi;
            ray_origin = ray_hit.world_position + (ray_hit.geometric_world_normal * RAY_T_MIN);
            ray_t_min = RAY_T_MIN;
            p_bounce = next_bounce.pdf;
            throughput *= next_bounce.throughput;

            // Russian roulette for early termination
            let rr = saturate(luminance(throughput));
            if rand_f(&rng) >= rr { break; }
            throughput /= rr;
        } else {
            radiance += throughput * sample_environment_map_light(ray_direction);
            @if(DLSS_RR) {
                if first {
                    textureStore(dlss_rr_depth, global_id.xy, vec4(0.0));
                }
                if !guided {
                    textureStore(diffuse_albedo, global_id.xy, vec4(0.0));
                    textureStore(specular_albedo, global_id.xy, vec4(0.5));
                    textureStore(normal_roughness, global_id.xy, vec4(0.0, 0.0, 1.0, 0.0));
                    textureStore(specular_motion_vectors, global_id.xy, vec4(motion_vector(ray_origin + ray_direction * 10000.0), vec2(0.0)));
                }
            }
            break;
        }
    }

    // Camera exposure
    radiance *= view.exposure;

    @if(DLSS_RR) {
        textureStore(view_output, global_id.xy + vec2<u32>(view.viewport.xy), vec4(radiance, 1.0));
        return;
    }

    // Accumulation over time via running average
    let new_color = mix(old_color.rgb, radiance, 1.0 / (old_color.a + 1.0));
    textureStore(accumulation_texture, global_id.xy, vec4(new_color, old_color.a + 1.0));
    textureStore(view_output, global_id.xy + vec2<u32>(view.viewport.xy), vec4(new_color, 1.0));
}

/// A still point's move on screen since last frame (UV offset, as the motion vector prepass writes them).
fn motion_vector(world_position: vec3<f32>) -> vec2<f32> {
    let now = view.unjittered_clip_from_world * vec4(world_position, 1.0);
    let was = previous_view.unjittered_clip_from_world * vec4(world_position, 1.0);
    return (now.xy / now.w - was.xy / was.w) * vec2(0.5, -0.5);
}

/// The specular albedo Ray Reconstruction wants (as Solari's lighting mode writes it).
fn env_brdf_approx2(specular_color: vec3<f32>, alpha: f32, N: vec3<f32>, V: vec3<f32>) -> vec3<f32> {
    let NoV = abs(dot(N, V));
    let X = vec4(1.0, NoV, NoV * NoV, NoV * NoV * NoV);
    let Y = vec4(1.0, alpha, alpha * alpha, alpha * alpha * alpha);
    let M1 = mat2x2<f32>(0.99044, 1.29678, -1.28514, -0.755907);
    let M2 = mat3x3<f32>(1.0, 20.3225, 121.563, 2.92338, -27.0302, 626.13, 59.4188, 222.592, 316.627);
    let M3 = mat2x2<f32>(0.0365463, 9.0632, 3.32707, -9.04756);
    let M4 = mat3x3<f32>(1.0, 9.04401, 5.56589, 3.59685, -16.3174, 19.7886, -1.36772, 9.22949, -20.2123);
    var bias = dot(M1 * X.xy, Y.xy) / dot(M2 * X.xyw, Y.xyw);
    let scale = dot(M3 * X.xy, Y.xy) / dot(M4 * X.xzw, Y.xyw);
    bias *= saturate(specular_color.g * 50.0);
    return fma(specular_color, vec3(max(0.0, scale)), vec3(max(0.0, bias)));
}
