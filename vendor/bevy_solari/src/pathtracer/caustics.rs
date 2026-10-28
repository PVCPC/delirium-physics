//! Delirium: the sun's caustics through water, photon mapped (`caustics.wesl`), for the path tracer.
use super::Pathtracer;
use crate::realtime::SolariLighting;
use crate::scene::{RaytracingSceneBindings, CAUSTIC_CELLS};
use bevy_asset::{load_embedded_asset, AssetServer};
use bevy_ecs::{prelude::*, resource::Resource, system::Commands};
use bevy_render::{
    render_resource::{
        binding_types::uniform_buffer, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor, PipelineCache, ShaderStages,
    },
    renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
    view::{ViewUniform, ViewUniformOffset, ViewUniforms},
};
use bevy_utils::default;
use core::sync::atomic::{AtomicBool, Ordering};

/// Whether the sun lights through water by photons, path or ray traced (set by the app: its best settings).
pub static CAUSTIC_PHOTONS: AtomicBool = AtomicBool::new(false);

#[derive(Resource)]
pub struct CausticPipelines {
    layout: BindGroupLayoutDescriptor,
    passes: [CachedComputePipelineId; 3],
}

pub fn init_caustic_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    scene_bindings: Res<RaytracingSceneBindings>,
    asset_server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "caustics_bind_group_layout",
        &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, (uniform_buffer::<ViewUniform>(true),)),
    );
    let passes = ["tiles", "photons", "resolve"].map(|entry| {
        pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some(format!("caustics_{entry}").into()),
            layout: vec![scene_bindings.bind_group_layout.clone(), layout.clone()],
            shader: load_embedded_asset!(asset_server.as_ref(), "caustics.wesl"),
            entry_point: Some(entry.into()),
            ..default()
        })
    });
    commands.insert_resource(CausticPipelines { layout, passes });
}

pub fn caustic_photons(
    view: ViewQuery<&'static ViewUniformOffset, Or<(With<Pathtracer>, With<SolariLighting>)>>,
    pipelines: Option<Res<CausticPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    scene_bindings: Res<RaytracingSceneBindings>,
    view_uniforms: Res<ViewUniforms>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut was_on: Local<bool>,
    mut ctx: RenderContext,
) {
    let view_uniform_offset = view.into_inner();
    // (Off: the header's `on` cleared, once.)
    if !CAUSTIC_PHOTONS.load(Ordering::Relaxed) {
        if *was_on {
            render_queue.write_buffer(&scene_bindings.caustics, 0, &[0; 4]);
            *was_on = false;
        }
        return;
    }
    let Some(pipelines) = pipelines else { return };
    let (Some(tiles), Some(photons), Some(resolve), Some(scene_bind_group), Some(view_uniforms_binding)) = (
        pipeline_cache.get_compute_pipeline(pipelines.passes[0]),
        pipeline_cache.get_compute_pipeline(pipelines.passes[1]),
        pipeline_cache.get_compute_pipeline(pipelines.passes[2]),
        &scene_bindings.bind_group,
        view_uniforms.uniforms.binding(),
    ) else {
        return;
    };
    *was_on = true;
    let bind_group = render_device.create_bind_group(
        "caustics_bind_group",
        &pipeline_cache.get_bind_group_layout(&pipelines.layout),
        &BindGroupEntries::sequential((view_uniforms_binding,)),
    );
    // (A pass each, so each sees the last's writes.)
    let encoder = ctx.command_encoder();
    for (pipeline, groups, label) in [
        (tiles, [32, 32], "caustics_tiles"),
        (photons, [256, 256], "caustics_photons"),
        (resolve, [(CAUSTIC_CELLS / 256) as u32, 1], "caustics_resolve"),
    ] {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor { label: Some(label), timestamp_writes: None });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, scene_bind_group, &[]);
        pass.set_bind_group(1, &bind_group, &[view_uniform_offset.offset]);
        pass.dispatch_workgroups(groups[0], groups[1], 1);
    }
}
