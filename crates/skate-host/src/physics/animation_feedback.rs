use skate_core::{
    animation::{
        ground_acceleration,
        physical_feedback::{self, ControlFeedback, PhysicalFeedback, ReckoningFeedback},
    },
    input::turn_conditioner,
    physics::board_motion_output::BoardMotionOutput,
    riding::{pumping::state::PumpingState, speed_wobble::SpeedWobbleState},
};
use skate_data::collections::Collections;

pub(crate) struct AnimationFeedback {
    settings: turn_conditioner::Settings,
    bump_settings: ground_acceleration::Settings,
    state: turn_conditioner::State,
    previous_lateral_tilt: [f32; 4],
    pub published_previous_lateral_tilt: [f32; 4],
}

impl AnimationFeedback {
    pub fn load(data: &Collections) -> Result<Self, String> {
        Ok(Self {
            previous_lateral_tilt: [0.0; 4],
            published_previous_lateral_tilt: [0.0; 4],
            settings: super::animation_feedback_settings::load(data)?,
            bump_settings: ground_acceleration::Settings {
                scale_x_acc: data.float("anim_motion", "bumps", "scale_x_acc")?,
                min_bump_mag: data.float("anim_motion", "bumps", "min_bump_mag")?,
            },
            state: turn_conditioner::State {
                history: [0.0; 8],
                filters: [[0.0; 9]; 3],
            },
        })
    }

    pub fn reset(&mut self) {
        self.state.reset_history();
        self.previous_lateral_tilt = [0.0; 4];
        self.published_previous_lateral_tilt = [0.0; 4];
    }

    pub fn update(
        &mut self,
        motion: &BoardMotionOutput,
        pumping: &PumpingState,
        wobble: &SpeedWobbleState,
        reckoning: ReckoningFeedback,
        controls: ControlFeedback,
        acceleration: ground_acceleration::Input,
        lateral_tilt: [f32; 4],
    ) -> PhysicalFeedback {
        self.published_previous_lateral_tilt = self.previous_lateral_tilt;
        self.previous_lateral_tilt = lateral_tilt;
        physical_feedback::publish(
            &mut self.state,
            &self.settings,
            motion,
            pumping,
            wobble,
            reckoning,
            controls,
            ground_acceleration::publish(acceleration, &self.bump_settings),
        )
    }
}
