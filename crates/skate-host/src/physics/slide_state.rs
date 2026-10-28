mod settings;
mod update;
use super::{GamePhysics, SkaterRuntime};
use skate_core::{
    math::Vector3,
    physics::contact::RetailContactMaterial,
    player::slide_state::{SlideSettings, SlideSurface},
};
pub(crate) struct SlideState {
    pub state: skate_core::player::slide_state::SlideState,
    settings: SlideSettings,
    surfaces: Vec<(SlideSurface, RetailContactMaterial)>,
    manual_scalar: f32,
}
pub(crate) fn enter(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    let p = &skater.player_input.processed;
    skater.ground_lifecycle.skeleton_elapsed_16505 = true;
    skater.air_reckoning.state.spin_angle = 0.0;
    skater.air_reckoning.state.spin_speed = 0.0;
    physics
        .board
        .hook_mut()
        .drive
        .disable_animation(&mut skater.ground_lifecycle.board_animated_290);
    physics.board.bodies_mut()[6].rates.angular_velocity = Vector3::ZERO;
    for body in physics.board.bodies_mut() {
        body.inertia.linear_drag = 0.0;
    }
    if p.category_2516 != 100 {
        skater.ground.manual.reset();
    }
    if skater.wipeout.state.mode != 1 {
        skater.wipeout.state.balance = 0.0;
        skater.wipeout.state.mode = 1;
    }
    skater.slide_state.state.enter(p.scalar_2656);
    Ok(())
}
pub(crate) fn exit(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    skater.slide_state.state.exit();
    physics.settings.wheel_material = physics.settings.standard_wheel_material;
    Ok(())
}
pub(crate) fn update(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    let p = &skater.player_input.processed;
    let t = skater
        .player_input
        .toolkit
        .as_ref()
        .ok_or("Slide requires current BoardToolkit")?;
    physics.riding.update_slide_reckoning(
        p,
        t,
        skater.animation_input.extra.physical_body_spin,
        skater.animation_input.fields.balance,
    );
    super::input_phase::update_ground(physics, skater)?;
    update::board(physics, skater)
}
