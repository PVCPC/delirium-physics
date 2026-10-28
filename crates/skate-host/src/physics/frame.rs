use super::{
    GamePhysics, PlayerControls, SkaterRuntime, animation_phase, camera_output, ground_phase,
    input_phase, player_state, solve,
};
use crate::{camera::CameraRuntime, graph_runtime::StockGraphs};
use skate_core::{
    input::controller::ActionMap,
    math::Vector3,
    physics::{
        board::BodyId,
        phase::{PhysicalOutputSnapshot, PhysicsEvent},
    },
};

pub(super) fn advance(
    physics: &mut GamePhysics,
    skater: &mut SkaterRuntime,
    controls: &mut PlayerControls,
    graphs: &StockGraphs,
    actions: &mut dyn ActionMap,
    input_available: bool,
    camera: &mut CameraRuntime,
) -> Result<(), String> {
    let tick = physics.ticks;
    super::offboard_audit_trace::stage(tick, "begin", physics, skater, controls);
    physics.exchange = super::SimulationExchange::new(tick);
    for request in camera.simulation_rate_requests.drain(..) {
        if !physics.network_active { physics.clock.apply(request)?; }
    }
    if physics.network_active { physics.clock = super::clock::SimulationClock::default(); }
    if physics.ticks == 0 {
        player_state::initialize(physics, skater)?;
    }
    if super::climbing::advance(physics, skater, controls, camera)? { return Ok(()); }
    super::biped_air::consume_selector(physics, skater)?;
    physics.board.clear_forces();
    physics
        .riding
        .start_wheel_queries(&physics.board, &physics.world)?;
    let skeleton_queries = super::foot_ik_queries::query(&physics.world, &skater.skeleton)?;
    let animation = bevy::log::info_span!("fixed_animation_graphs").in_scope(|| animation_phase::advance(
        physics,
        skater,
        controls,
        graphs,
        &physics.animation_profile,
    ))
    .map_err(|e| format!("Animation tick{}: {e}", physics.ticks))?;
    super::offboard_audit_trace::stage(tick, "animation", physics, skater, controls);
    if let Some(mode) = skater.animation.motion.riding.force_mode.take() {
        skater.skeleton_input.force_mode = mode.native_value();
    }
    let mut simulation_actions = controls.simulation_actions(actions);
    let teleported = input_phase::advance(
        physics,
        skater,
        &animation.packet(),
        &mut simulation_actions,
        input_available,
    )?;
    skater.ground_settings = skater.ground_profiles.select(skater.player_input.processed.state_variant_index_2528, skater.player_input.processed.surface_mode_2540)?;
    if physics.trainer != crate::tuning::TrainerTuning::default() {
        skater.ground_settings = std::sync::Arc::new(skater.ground_settings.tuned(physics.trainer));
    }
    let mut vehicle_ejected = false;
    if teleported {
        skater.respawn.reset_measurements();
        skater.offboard_contact.reset_history();
        skater.player_state.reset_for_teleport();
        skater.centre_of_mass_filter.reset();
        skater.animation_feedback.reset();
        player_state::enter_after_teleport(physics, skater)?;
        super::offboard::board_manager::runtime::finish_teleport(physics, skater);
        skater.wipeout_state.ragdoll.restore_normal(
            &mut skater.skeleton,
            &mut skater.skeleton_joints,
            &mut skater.skeleton_collision,
            &mut skater.collision_feedback,
        );
        vehicle_ejected = player_state::apply_vehicle_ejection(physics, skater)?;
    }
    physics.riding.finish_wheel_queries()?;
    {
        let scene =
            super::offboard::grab_scene::Scene::new(&physics.world, &physics.offboard_grab_scene);
        skater.offboard_grab.execute_queries(&scene)?;
    }
    let state_before_selection = skater.player_state.current();
    if vehicle_ejected {
        player_state::complete_post_input(physics, skater)?;
    } else {
        player_state::post_input_and_select(physics, skater)?;
    }
    let state_after_selection = skater.player_state.current();
    super::offboard_audit_trace::stage(tick, "selected", physics, skater, controls);
    if state_before_selection != state_after_selection {
        if skater.player_input.processed.flags_2476 & (1 << 22) != 0
            || state_before_selection == skate_core::player::state::PhysicalStateId::HandPlant
            || state_after_selection == skate_core::player::state::PhysicalStateId::HandPlant {
            bevy::log::info!("HANDPLANT_STATE tick={tick} from={state_before_selection:?} to={state_after_selection:?} flags={:08x} phase={} processed={:08x}/{:08x}/{:08x}",
                skater.handplant.flags, skater.handplant.phase,
                skater.player_input.processed.flags_2468, skater.player_input.processed.flags_2476,
                skater.player_input.processed.flags_2480);
        }
        physics.exchange.emit_event(
            tick,
            PhysicsEvent::StateChanged {
                from: state_before_selection,
                to: state_after_selection,
            },
        )?;
    }
    player_state::pre_state(physics, skater)?;
    match skater.player_state.current() {
        skate_core::player::state::PhysicalStateId::RevertGround => super::revert_state::update(physics,skater)?,
        skate_core::player::state::PhysicalStateId::HandPlant => super::handplant::update(physics,skater)?,
        skate_core::player::state::PhysicalStateId::FootPlant => super::footplant::ground::update(physics, skater)?,
        skate_core::player::state::PhysicalStateId::Boneless => super::boneless::update(physics, skater)?,
        skate_core::player::state::PhysicalStateId::PhysicsGround => {
            let p = &skater.player_input.processed;
            let com = p.animation_com_to_deck_752.map(f32::from_bits);
            physics.riding.update_ground_reckoning(
                &physics.board,
                super::riding_outputs::RidingPoseInputs {
                    com_to_deck: Vector3::new(com[0], com[1], com[2]),
                    body_spin: skater.animation_input.extra.physical_body_spin,
                },
                p.flags_2468,
                skater.animation_input.fields.balance,
                p.flags_2476 & 0x4000_0000 != 0,
                p,
            );
            skater.ground.state.push_suppressed_2730 = false;
            ground_phase::advance(physics, skater)?;
            super::handplant::ground_update(physics,skater)?;
            input_phase::update_ground(physics, skater)?;
        }
        skate_core::player::state::PhysicalStateId::PhysicsAir => {
            super::air_phase::advance(physics, skater)?
        }
        skate_core::player::state::PhysicalStateId::KnownAir => {
            super::known_air::update(physics, skater)?
        }
        skate_core::player::state::PhysicalStateId::BipedAir => {
            super::biped_air::update(physics, skater, controls)?
        }
        skate_core::player::state::PhysicalStateId::BipedGround
        | skate_core::player::state::PhysicalStateId::OffBoardPushing => {
            let contact = skate_core::player::offboard::ground_job::ContactSnapshot {
                readiness: skater.offboard_contact.readiness() as i32,
                prefix: skater.offboard_contact.prefix(),
            };
            let input = super::biped_ground::update(physics, skater, controls, contact)?;
            skater.offboard_contact.submit(
                input,
                skater.player_input.processed.actor_query_2952 as i32,
                &super::offboard::contact_toolkit::StaticScene::new(&physics.world)?,
            )?;
            super::biped_ground::submit_geometry(physics, skater)?;
        }
        skate_core::player::state::PhysicalStateId::GroundAnimation => {
            super::ground_animation::advance(physics, skater)?
        }
        skate_core::player::state::PhysicalStateId::LandingOnDeck => {
            super::landing_on_deck::advance(physics, skater)?
        }
        skate_core::player::state::PhysicalStateId::SlideGround => {
            super::slide_state::update(physics, skater)?
        }
        skate_core::player::state::PhysicalStateId::WipeoutGround => {
            super::wipeout_states::advance(physics, skater)?
        }
        skate_core::player::state::PhysicalStateId::Teleporting => {
            if skater.teleport_state.update(&skater.player_input.processed) {
                super::respawn::request(physics, skater)?;
            }
        }
        state
            if state.is_grind()
                || state == skate_core::player::state::PhysicalStateId::Nonspecific =>
        {
            super::grind::advance(physics, skater)?
        }
        state => return Err(format!("Selected {state:?} requires its physical update")),
    }
    skater.player_input.player.state_timer_1344 += skater.player_input.processed.timestep_2604;
    super::offboard_audit_trace::stage(tick, "state", physics, skater, controls);
    super::offboard::board_manager::runtime::update(physics, skater);
    super::offboard_audit_trace::stage(tick, "possession", physics, skater, controls);
    physics.processed_flags_2468 = skater.player_input.processed.flags_2468;
    skeleton_queries.publish(&mut skater.player_input.player);
    bevy::log::info_span!("fixed_collision_and_solve").in_scope(|| solve::advance(physics, skater, skater.ground.steering.targets))?;
    super::offboard_audit_trace::stage(tick, "solve", physics, skater, controls);
    super::player_input::reset_outputs(&mut skater.player_input.physical);
    bevy::log::info_span!("fixed_finish_skater").in_scope(|| physics.finish_skater(skater))?;
    super::offboard_audit_trace::stage(tick, "finish", physics, skater, controls);
    let simulation = physics.settings.step.simulation;
    skater
        .player_input
        .update_dynamic_normal(&physics.riding, simulation.gravity_acceleration);
    skater.player_input.publish_board(&physics.riding)?;
    skater.player_input.physical.skeleton.publish_deck_angles(
        skater.animated_skeleton.record.pose[0][2],
        skater.animation.packet.board_flipped,
    );
    let up = physics.riding.reckoning.up;
    skater.player_input.publish_grind_graph_outputs(
        &skater.skeleton.record,
        [up.x, up.y, up.z, 0.0],
        &skater.trajectory.selector,
    )?;
    player_state::publish(physics, skater)?;
    skater
        .grind_camera
        .condition_fields(&mut skater.player_input.physical.grinds);
    let physical = &skater.player_input.physical;
    skater.centre_of_mass_output = skater.centre_of_mass_filter.update(
        physical.reckoning.vector_64.map(f32::from_bits),
        physical.reckoning.vector_16.map(f32::from_bits),
    );
    let feedback = animation_phase::publish_feedback(physics, skater);
    super::respawn::observe(physics, skater)?;
    let deck = physics.board.bodies()[BodyId::Deck.index()];
    let rider = skater
        .skeleton
        .bodies()
        .first()
        .ok_or("Physical output requires the constructed rider body")?;
    let ground_normal = physics.riding.ground.wheel_normal;
    let predicted = skater.player_input.physical.collision.predicted_position_64;
    let predicted_position = Vector3::new(
        f32::from_bits(predicted[0]),
        f32::from_bits(predicted[1]),
        f32::from_bits(predicted[2]),
    );
    let events = physics.exchange.events().to_vec();
    physics.exchange.publish_output(PhysicalOutputSnapshot {
        tick,
        state: skater.player_state.current(),
        board_position: deck.rates.position,
        board_linear_velocity: deck.rates.linear_velocity,
        rider_root_position: rider.rates.position,
        rider_linear_velocity: rider.rates.linear_velocity,
        ground_normal,
        contact_count: physics.riding.ground.part_contact_count as u32,
        predicted_position,
        grounded: matches!(
            skater.player_state.current(),
            skate_core::player::state::PhysicalStateId::PhysicsGround
        ),
        wiping_out: skater.skeleton_collision.is_ragdoll,
        landed: events
            .iter()
            .any(|event| matches!(event, PhysicsEvent::Landing)),
        events,
    });
    camera_output::advance(physics, skater, &feedback, camera)?;
    let score = &skater.animation.motion.score_packet;
    let deck_frame = physics.board.part_transforms()[BodyId::Deck.index()];
    let position = deck.rates.position;
    let velocity = deck.rates.linear_velocity;
    let filtered = skater.player_state.filtered_output;
    skater.scoring.advance(crate::scoring_runtime::Frame {
        tick: tick as u32, dt: simulation.time_step,
        category: filtered.map_or(Default::default(), |f|f.category),
        state: skater.player_state.current() as u32,
        descriptor: score.trick_names.first.or_else(||score.grab.map(|g|g.0)),
        grind_id: filtered.map_or(-1, |f|f.grind.scorable_id), flags:score.flags,
        position:[position.x,position.y,position.z], velocity:[velocity.x,velocity.y,velocity.z],
        forward:deck_frame.basis.columns[2],
        switch:skater.animation.packet.riding_switch,fakie:skater.animation.packet.riding_fakie,
        nollie:skater.animation.packet.weight_forwards,body_flip:skater.player_input.physical.air.flag_441!=0,
        suspend_air:skater.player_input.physical.air.use_air_reckoning_452!=0,
        landing:skater.landing_quality,teleported,
        reverting:skater.player_input.physical.state.flag_66 != 0,
    })?;
    super::climbing::approach::advance(physics, skater, controls);
    skater.animation_input.finish_output_publication();
    skater.player_input.player.update_count_1316 =
        skater.player_input.player.update_count_1316.wrapping_add(1);
    physics.clock.finish_tick();
    Ok(())
}
