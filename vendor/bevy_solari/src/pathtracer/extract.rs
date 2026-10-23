use super::{prepare::PathtracerAccumulationTexture, Pathtracer};
use bevy_camera::Camera;
use bevy_ecs::{
    change_detection::DetectChanges,
    system::{Commands, Query},
    world::Ref,
};
use bevy_ecs::system::Local;
use bevy_render::{sync_world::RenderEntity, Extract};
use bevy_transform::components::GlobalTransform;

pub fn extract_pathtracer(
    cameras_3d: Extract<
        Query<(
            RenderEntity,
            &Camera,
            Ref<GlobalTransform>,
            Option<&Pathtracer>,
        )>,
    >,
    mut commands: Commands,
    mut was: Local<bevy_platform::collections::HashMap<bevy_ecs::entity::Entity, GlobalTransform>>,
) {
    for (entity, camera, global_transform, pathtracer) in &cameras_3d {
        let mut entity_commands = commands
            .get_entity(entity)
            .expect("Camera entity wasn't synced.");
        if let Some(pathtracer) = pathtracer
            && camera.is_active
        {
            let mut pathtracer = pathtracer.clone();
            // Delirium's patch: only when the camera really moved (over a tenth of a millimeter), not whenever its
            // transform was written.
            if global_transform.is_changed() && !was.get(&entity).is_some_and(|w| w.affine().abs_diff_eq(global_transform.affine(), 1e-4)) {
                was.insert(entity, *global_transform);
                pathtracer.reset = true;
            }
            entity_commands.insert(pathtracer);
        } else {
            entity_commands.remove::<(Pathtracer, PathtracerAccumulationTexture)>();
        }
    }
}
