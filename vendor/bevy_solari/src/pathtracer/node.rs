use super::{prepare::PathtracerAccumulationTexture, Pathtracer};
use crate::scene::RaytracingSceneBindings;
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy_anti_alias::dlss::ViewDlssRayReconstructionTextures;
use bevy_asset::{load_embedded_asset, AssetServer};
use bevy_core_pipeline::prepass::{PreviousViewData, PreviousViewUniformOffset, PreviousViewUniforms};
use bevy_ecs::{prelude::*, resource::Resource, system::Commands};
use bevy_render::{
    camera::ExtractedCamera,
    render_resource::{
        binding_types::{texture_storage_2d, uniform_buffer},
        BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
        CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor,
        ImageSubresourceRange, PipelineCache, ShaderStages, StorageTextureAccess, TextureFormat,
    },
    renderer::{RenderContext, RenderDevice, ViewQuery},
    view::{ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms},
};
use bevy_utils::default;

/// Resource holding the pathtracer pipeline configuration.
#[derive(Resource)]
pub struct PathtracerPipelines {
    bind_group_layout: BindGroupLayoutDescriptor,
    pipeline: CachedComputePipelineId,
    /// Delirium: one sample a frame into DLSS Ray Reconstruction, with its guides (group 2).
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
    dlss_rr: (BindGroupLayoutDescriptor, CachedComputePipelineId),
}

/// Initializes the pathtracer pipelines at render startup.
pub fn init_pathtracer_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    scene_bindings: Res<RaytracingSceneBindings>,
    asset_server: Res<AssetServer>,
) {
    let bind_group_layout = BindGroupLayoutDescriptor::new(
        "pathtracer_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_storage_2d(TextureFormat::Rgba32Float, StorageTextureAccess::ReadWrite),
                texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly),
                uniform_buffer::<ViewUniform>(true),
                uniform_buffer::<PreviousViewData>(true),
            ),
        ),
    );

    let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("pathtracer_pipeline".into()),
        layout: vec![
            scene_bindings.bind_group_layout.clone(),
            bind_group_layout.clone(),
        ],
        shader: load_embedded_asset!(asset_server.as_ref(), "pathtracer.wesl"),
        ..default()
    });

    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
    let dlss_rr = {
        let guides = BindGroupLayoutDescriptor::new(
            "pathtracer_dlss_rr_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::WriteOnly),
                    texture_storage_2d(TextureFormat::Rgba8Unorm, StorageTextureAccess::WriteOnly),
                    texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly),
                    texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly),
                    texture_storage_2d(TextureFormat::Rg16Float, StorageTextureAccess::WriteOnly),
                ),
            ),
        );
        let pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("pathtracer_dlss_rr_pipeline".into()),
            layout: vec![scene_bindings.bind_group_layout.clone(), bind_group_layout.clone(), guides.clone()],
            shader: load_embedded_asset!(asset_server.as_ref(), "pathtracer.wesl"),
            shader_defs: vec!["DLSS_RR".into()],
            ..default()
        });
        (guides, pipeline)
    };

    commands.insert_resource(PathtracerPipelines {
        bind_group_layout,
        pipeline,
        #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
        dlss_rr,
    });
}

#[cfg(any(not(feature = "dlss"), feature = "force_disable_dlss"))]
type PathtracerViewQuery = (
    &'static Pathtracer,
    &'static PathtracerAccumulationTexture,
    &'static ExtractedCamera,
    &'static ViewTarget,
    &'static ViewUniformOffset,
    &'static PreviousViewUniformOffset,
);

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
type PathtracerViewQuery = (
    &'static Pathtracer,
    &'static PathtracerAccumulationTexture,
    &'static ExtractedCamera,
    &'static ViewTarget,
    &'static ViewUniformOffset,
    &'static PreviousViewUniformOffset,
    Option<&'static ViewDlssRayReconstructionTextures>,
);

pub fn pathtracer(
    view: ViewQuery<PathtracerViewQuery>,
    pathtracer_pipelines: Option<Res<PathtracerPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    scene_bindings: Res<RaytracingSceneBindings>,
    view_uniforms: Res<ViewUniforms>,
    previous_view_uniforms: Res<PreviousViewUniforms>,
    render_device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    #[cfg(any(not(feature = "dlss"), feature = "force_disable_dlss"))]
    let (pathtracer_settings, accumulation_texture, camera, view_target, view_uniform_offset, previous_view_uniform_offset) =
        view.into_inner();
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
    let (pathtracer_settings, accumulation_texture, camera, view_target, view_uniform_offset, previous_view_uniform_offset, dlss_rr) =
        view.into_inner();

    let Some(pathtracer_pipelines) = pathtracer_pipelines else {
        return;
    };

    #[allow(unused_mut)]
    let mut pipeline_id = pathtracer_pipelines.pipeline;
    #[allow(unused_mut)]
    let mut guides: Option<bevy_render::render_resource::BindGroup> = None;
    #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
    if let Some(d) = dlss_rr {
        pipeline_id = pathtracer_pipelines.dlss_rr.1;
        guides = Some(render_device.create_bind_group(
            "pathtracer_dlss_rr_bind_group",
            &pipeline_cache.get_bind_group_layout(&pathtracer_pipelines.dlss_rr.0),
            &BindGroupEntries::sequential((
                &d.diffuse_albedo.default_view,
                &d.specular_albedo.default_view,
                &d.normal_roughness.default_view,
                &d.depth.default_view,
                &d.specular_motion_vectors.default_view,
            )),
        ));
    }

    let (Some(pipeline), Some(scene_bind_group), Some(viewport), Some(view_uniforms_binding), Some(previous_view_uniforms_binding)) = (
        pipeline_cache.get_compute_pipeline(pipeline_id),
        &scene_bindings.bind_group,
        camera.physical_viewport_size,
        view_uniforms.uniforms.binding(),
        previous_view_uniforms.uniforms.binding(),
    ) else {
        return;
    };

    let bind_group = render_device.create_bind_group(
        "pathtracer_bind_group",
        &pipeline_cache.get_bind_group_layout(&pathtracer_pipelines.bind_group_layout),
        &BindGroupEntries::sequential((
            &accumulation_texture.0.default_view,
            view_target.get_unsampled_color_attachment().view,
            view_uniforms_binding,
            previous_view_uniforms_binding,
        )),
    );

    let command_encoder = ctx.command_encoder();

    if pathtracer_settings.reset {
        command_encoder.clear_texture(
            &accumulation_texture.0.texture,
            &ImageSubresourceRange::default(),
        );
    }

    let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some("pathtracer"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, scene_bind_group, &[]);
    pass.set_bind_group(1, &bind_group, &[view_uniform_offset.offset, previous_view_uniform_offset.offset]);
    if let Some(guides) = &guides {
        pass.set_bind_group(2, guides, &[]);
    }
    pass.dispatch_workgroups(viewport.x.div_ceil(8), viewport.y.div_ceil(8), 1);
}
