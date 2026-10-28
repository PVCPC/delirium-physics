use super::{data::PhysicsAirState, runtime::PhysicsAirRuntime};

pub fn update_post_physics(state: &mut PhysicsAirState, runtime: &mut impl PhysicsAirRuntime) {
    if !state.reached_apex {
        let board_velocity = runtime.board_body_velocity();
        if board_velocity[1] < 0.0 {
            state.reached_apex = true;
        }
    }
    runtime.check_for_air_wipeout(false);
}
