#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManualState {
    pub filtered_angle_error: f32,
    pub target_angle: f32,
    pub measured_angle: f32,
    pub angular_correction: f32,
    pub elapsed: f32,
}

impl ManualState {
    pub fn reset(&mut self) {
        self.filtered_angle_error = 0.0;
        self.target_angle = 0.0;
        self.measured_angle = 0.0;
        self.elapsed = 0.0;
        self.angular_correction = 0.0;
    }

    pub fn enter_ground(
        &mut self,
        previous_category: u32,
        powerslide_exit_scale: f32,
    ) -> ManualEntryContinuation {
        if previous_category == 100 {
            self.angular_correction *= powerslide_exit_scale;
            self.filtered_angle_error *= powerslide_exit_scale;
            ManualEntryContinuation::Continue
        } else {
            self.filtered_angle_error = 0.0;
            self.target_angle = 0.0;
            self.measured_angle = 0.0;
            self.angular_correction = 0.0;
            self.elapsed = 0.0;
            ManualEntryContinuation::RemoveVelocityIntoGround
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum ManualEntryContinuation {
    Continue,
    RemoveVelocityIntoGround,
}
