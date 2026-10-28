use super::Pathtracer;
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy_anti_alias::dlss::{Dlss, DlssRayReconstructionFeature, ViewDlssRayReconstructionTextures};
#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
use bevy_ecs::query::Has;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    query::With,
    system::{Commands, Query, Res, ResMut},
};
use bevy_image::ToExtents;
use bevy_render::{
    camera::ExtractedCamera,
    render_resource::{TextureDescriptor, TextureDimension, TextureFormat, TextureUsages},
    renderer::RenderDevice,
    texture::{CachedTexture, TextureCache},
};

#[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
type HasDlssRr = Has<Dlss<DlssRayReconstructionFeature>>;
#[cfg(any(not(feature = "dlss"), feature = "force_disable_dlss"))]
type HasDlssRr = ();

#[derive(Component)]
pub struct PathtracerAccumulationTexture(pub CachedTexture);

pub fn prepare_pathtracer_accumulation_texture(
    query: Query<(Entity, &ExtractedCamera, HasDlssRr), With<Pathtracer>>,
    mut texture_cache: ResMut<TextureCache>,
    render_device: Res<RenderDevice>,
    mut commands: Commands,
) {
    for (entity, camera, _has_dlss_rr) in &query {
        let Some(viewport) = camera.physical_viewport_size else {
            continue;
        };

        // Delirium: Ray Reconstruction's guides, which the pathtracer writes from the first surface it hits.
        #[cfg(all(feature = "dlss", not(feature = "force_disable_dlss")))]
        if _has_dlss_rr {
            let mut guide = |label: &'static str, format: TextureFormat| {
                texture_cache.get(&render_device, TextureDescriptor {
                    label: Some(label),
                    size: viewport.to_extents(),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format,
                    usage: TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING,
                    view_formats: &[],
                })
            };
            let textures = ViewDlssRayReconstructionTextures {
                diffuse_albedo: guide("pathtracer_diffuse_albedo", TextureFormat::Rgba8Unorm),
                specular_albedo: guide("pathtracer_specular_albedo", TextureFormat::Rgba8Unorm),
                normal_roughness: guide("pathtracer_normal_roughness", TextureFormat::Rgba16Float),
                depth: guide("pathtracer_dlss_rr_depth", TextureFormat::R32Float),
                specular_motion_vectors: guide("pathtracer_specular_motion_vectors", TextureFormat::Rg16Float),
            };
            commands.entity(entity).insert(textures);
        } else {
            commands.entity(entity).remove::<ViewDlssRayReconstructionTextures>();
        }

        let descriptor = TextureDescriptor {
            label: Some("pathtracer_accumulation_texture"),
            size: viewport.to_extents(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        };

        commands
            .entity(entity)
            .insert(PathtracerAccumulationTexture(
                texture_cache.get(&render_device, descriptor),
            ));
    }
}
