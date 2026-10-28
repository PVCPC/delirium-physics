use super::MotionAnimation;
use crate::graph_host::motion_grind::animation::Animation;
use skate_core::animation::output::attributes::{AnimationAttribute, AttributeName};

impl Animation for MotionAnimation {
    fn apply_parameters(&mut self) -> Result<(), String> {
        MotionAnimation::apply_parameters(self)
    }

    fn query_current_attribute(
        &self,
        name: AttributeName,
        mask: u32,
        output: &mut AnimationAttribute,
    ) -> Result<bool, String> {
        self.current
            .as_ref()
            .ok_or("GrindControlFade requires a current animation tree")?
            .query_attribute(name, mask, output)
    }

    fn emit_packet(&mut self, name: AttributeName, value: f32) {
        MotionAnimation::emit_packet(self, name, value);
    }

    fn intent(&self, name: &str) -> Option<f32> {
        self.motion_intents.get(name).copied()
    }
}

