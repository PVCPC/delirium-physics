mod frames;
mod state;
use crate::physics::{
    skeleton_air::SkeletonAir,
    skeleton_input_runtime::{CollisionInput, SkeletonInputRuntime, SkeletonOwners},
};
use skate_core::{
    animation::output::NativeMatrix,
    physics::{
        board_runtime::BoardRuntime, rigid_body::RetailSimulationStep,
        skeleton_animation_record::AnimationPartTransform as Transform,
    },
    player::input_phase::ProcessedPhysicsInput,
};
pub(crate) use state::State;
pub(crate) struct Input<'a> {
    pub world_frame: &'a Transform,
    pub centre_of_mass_1056: [f32; 4],
}
pub(crate) struct ReckoningUpdate {
    pub up: [f32; 4],
    pub forward: [f32; 4],
    pub blend: f32,
}
impl SkeletonInputRuntime {
    pub(crate) fn update_biped_ground<F>(
        &mut self,
        air: &mut SkeletonAir,
        board: &mut BoardRuntime,
        input: Input<'_>,
        state: &mut State,
        p: &mut ProcessedPhysicsInput,
        owners: &mut SkeletonOwners<'_>,
        globals: &[NativeMatrix],
        collision: &CollisionInput,
        simulation: RetailSimulationStep,
        finish_reckoning: F,
    ) -> Result<Transform, String>
    where
        F: FnOnce(ReckoningUpdate) -> Result<(), String>,
    {
        let s = &mut owners.animated;
        let target = frames::prepare_pose(
            &mut s.roots,
            &mut s.board_frames,
            &s.record.pose[0],
            &self.drive_frames[0],
            &mut state.retained_board,
            input,
            p.flags_2476,
            p.flags_2484,
            &mut p.flags_2468,
        );
        if p.flags_2480 & 0x8000 != 0 || p.flags_2484 & 1 != 0 {
            s.board_frames.physical_board = air.apply_board(board, &target, true);
        }
        self.general_update(p, owners, globals, collision, simulation)?;
        owners.animated.finish_ground();
        let root = owners.animated.roots.animation_to_world;
        finish_reckoning(ReckoningUpdate {
            up: root[1],
            forward: root[2],
            blend: 0.5,
        })?;
        owners.animation_input.fields.flags2468 = p.flags_2468;
        Ok(target)
    }
}
