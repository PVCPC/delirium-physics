use super::PlayerInputRuntime;
use skate_core::{
    air::trajectory::TrajectorySelector, physics::skeleton_body::SkeletonPhysicalRecord,
};

impl PlayerInputRuntime {
    pub(crate) fn publish_grind_graph_outputs(
        &mut self,
        record: &SkeletonPhysicalRecord,
        reckoning_up: [f32; 4],
        selector: &TrajectorySelector,
    ) -> Result<(), String> {
        let toolkit = self
            .toolkit
            .as_ref()
            .ok_or("Grind output requires the completed physical input toolkit")?;
        self.physical.air.flag_443 = u8::from(selector.grind_locked_to_middle());
        self.physical
            .skeleton
            .publish_twist(record, toolkit.forward, reckoning_up);
        Ok(())
    }
}
