use super::{GamePhysics, SkaterRuntime};

pub(crate) fn update_possession(physics: &mut GamePhysics, skater: &mut SkaterRuntime) {
    let flags = skater.player_input.processed.flags_2484;
    let state = skater.skateboard_controller.fields.state_448;
    let target = if flags & 1 != 0 {
        if matches!(state, 2 | 3 | 4) {
            return;
        }
        5
    } else {
        if state != 5 {
            return;
        }
        1
    };
    skater.skateboard_controller.fields.word_444 = 0;
    if state == target {
        return;
    }
    let observation = super::super::offboard::board_manager::runtime::observe(physics, skater);
    let mut effects = skater.board_possession_live.effects(
        physics,
        &mut skater.ground_lifecycle.board_animated_290,
        skater.player_input.processed.timestep_2604,
    );
    if target == 5 {
        skater.board_possession.stop(
            &mut skater.skateboard_controller.fields,
            &observation,
            &mut effects,
        );
    } else {
        skater.board_possession.hold(
            &mut skater.skateboard_controller.fields,
            &observation,
            &mut effects,
        );
    }
    skater.skateboard_controller.fields.state_448 = target;
    skater.board_possession_live.publish_volumes(physics);
}
