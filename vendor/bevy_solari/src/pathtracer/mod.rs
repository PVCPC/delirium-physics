mod caustics;
mod extract;
mod node;
mod prepare;

use crate::{scene::RaytracingSceneBindings, SolariPlugins};
use bevy_app::{App, Plugin};
use bevy_asset::embedded_asset;
use bevy_camera::Hdr;
use bevy_core_pipeline::{
    schedule::{Core3d, Core3dSystems},
    tonemapping::tonemapping,
};
use bevy_ecs::{component::Component, reflect::ReflectComponent, schedule::IntoScheduleConfigs};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::{
    init_gpu_resource, renderer::RenderDevice, ExtractSchedule, Render, RenderApp, RenderStartup,
    RenderSystems,
};
use extract::extract_pathtracer;
use caustics::{caustic_photons, init_caustic_pipelines};
pub use caustics::CAUSTIC_PHOTONS;
use node::{init_pathtracer_pipelines, pathtracer};
use prepare::prepare_pathtracer_accumulation_texture;
use tracing::warn;

/// Non-realtime pathtracing.
///
/// This plugin is meant to generate reference screenshots to compare against,
/// and is not intended to be used by games.
pub struct PathtracingPlugin;

impl Plugin for PathtracingPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "pathtracer.wesl");
        embedded_asset!(app, "caustics.wesl");
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);

        let render_device = render_app.world().resource::<RenderDevice>();
        let features = render_device.features();
        if !features.contains(SolariPlugins::required_wgpu_features()) {
            warn!(
                "PathtracingPlugin not loaded. GPU lacks support for required features: {:?}.",
                SolariPlugins::required_wgpu_features().difference(features)
            );
            return;
        }

        render_app
            .add_systems(
                RenderStartup,
                (init_pathtracer_pipelines, init_caustic_pipelines).after(init_gpu_resource::<RaytracingSceneBindings>),
            )
            .add_systems(ExtractSchedule, extract_pathtracer)
            .add_systems(
                Render,
                prepare_pathtracer_accumulation_texture.in_set(RenderSystems::PrepareResources),
            )
            .add_systems(Core3d, caustic_photons.before(pathtracer).before(crate::realtime::node::solari_lighting))
            .add_systems(
                Core3d,
                pathtracer
                    // Delirium: between the opaque and blended passes, so blended things (portals, particles) draw over its picture.
                    .after(bevy_core_pipeline::core_3d::main_opaque_pass_3d)
                    // (and after Bevy's sky, which would be blended over its sky)
                    .after(bevy_pbr::RenderSky)
                    .before(bevy_core_pipeline::core_3d::main_transparent_pass_3d)
                    // Delirium: before DLSS's Ray Reconstruction, which cleans up its picture.
                    .before(Core3dSystems::EarlyPostProcess)
                    .before(tonemapping),
            );
    }
}

#[derive(Component, Reflect, Default, Clone)]
#[reflect(Component, Default, Clone)]
#[require(Hdr)]
pub struct Pathtracer {
    pub reset: bool,
}
