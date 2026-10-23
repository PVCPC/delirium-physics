use super::{prepare::DlssRenderContext, Dlss, DlssFeature};
use bevy_camera::{Camera, Hdr, MainPassResolutionOverride, Projection};
use bevy_ecs::{
    query::Has,
    system::{Commands, Query, ResMut},
};
use bevy_render::{sync_world::RenderEntity, MainWorld};

pub fn extract_dlss<F: DlssFeature>(
    mut commands: Commands,
    mut main_world: ResMut<MainWorld>,
    cleanup_query: Query<Has<Dlss<F>>>,
) {
    let mut cameras_3d = main_world
        .query::<(RenderEntity, &Camera, &Projection, Option<&mut Dlss<F>>, Has<Hdr>)>();

    for (entity, camera, camera_projection, mut dlss, hdr) in cameras_3d.iter_mut(&mut main_world) {
        let mut entity_commands = commands
            .get_entity(entity)
            .expect("Camera entity wasn't synced.");
        // Every camera, not only HDR ones: one that lost HDR with DLSS still gets cleaned up.
        if dlss.is_some() && hdr && camera.is_active && camera_projection.is_perspective() {
            entity_commands.insert(dlss.as_deref().unwrap().clone());
            dlss.as_mut().unwrap().reset = false;
        } else if cleanup_query.get(entity) == Ok(true) {
            entity_commands.remove::<(Dlss<F>, DlssRenderContext<F>, MainPassResolutionOverride)>();
        }
    }
}
