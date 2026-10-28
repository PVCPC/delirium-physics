use super::*;
use skate_core::physics::board::BodyId;
pub(super) fn advance(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    let player = &mut skater.player_input.player;
    player.state_count_1312 = player.state_count_1312.wrapping_add(1);
    if !skater.player_state.current().is_grind()
        && !matches!(
            skater.player_state.current(),
            PhysicalStateId::PhysicsGround
                | PhysicalStateId::PhysicsAir
                | PhysicalStateId::FootPlant | PhysicalStateId::Boneless | PhysicalStateId::HandPlant | PhysicalStateId::RevertGround
                | PhysicalStateId::KnownAir
                | PhysicalStateId::BipedAir
                | PhysicalStateId::BipedGround
                | PhysicalStateId::OffBoardPushing
                | PhysicalStateId::GroundAnimation
                | PhysicalStateId::SlideGround
                | PhysicalStateId::WipeoutGround
                | PhysicalStateId::Teleporting
                | PhysicalStateId::LandingOnDeck
                | PhysicalStateId::Nonspecific
        )
    {
        return Err("PreState requires the selected state's actual PredictFutureOfDeck".into());
    }
    let deck = physics.board.part_transforms()[BodyId::Deck.index()];
    let velocity = physics.board.bodies()[BodyId::Deck.index()]
        .rates
        .linear_velocity;
    let dt = skater.player_input.processed.timestep_2604;
    let prediction = [
        velocity.x.mul_add(dt, deck.translation.x),
        velocity.y.mul_add(dt, deck.translation.y),
        velocity.z.mul_add(dt, deck.translation.z),
        0.0,
    ];
    let roots = &mut skater.animated_skeleton.roots;
    roots.predicted_board_position = prediction;
    roots.supplied_prediction = Some(prediction);
    skater.foot_ik.state.contacts.support_failed = false;
    let p = &skater.player_input.processed;
    let context = skate_core::player::offboard::ground_query::QueryContext {
        selection_flags_2948: p.actor_query_2948,
        matching_id_2952: p.actor_query_2952 as i32,
    };
    let scene = super::super::offboard::grab_scene::Scene::new(
        &physics.world,
        &physics.offboard_grab_scene,
    );
    skater.offboard_grab.sync(&scene, context)?;
    Ok(())
}
