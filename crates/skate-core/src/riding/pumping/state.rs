#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PumpingState {
    pub previous_position: [f32; 4],
    pub previous_normal: [f32; 4],
    pub smoothed_height_change: f32,
    pub pumping_time: f32,
    pub previous_height: f32,
    pub pumping: f32,
    pub pump_acceleration: f32,
    pub angular_speed: f32,
    pub absorption: f32,
    pub ground_normal_absorption: f32,
    pub minimum_crouch: f32,
    pub deck_angle_absorption: f32,
    pub reset_only_scalar: f32,
    pub record_valid: bool,
    pub intentional_pumping: u8,
}

impl PumpingState {
    pub fn reset(&mut self) {
        *self = Self::reset_state();
    }

    pub const fn reset_state() -> Self {
        Self {
            previous_position: [0.0; 4],
            previous_normal: [0.0; 4],
            smoothed_height_change: 0.0,
            pumping_time: 0.0,
            previous_height: 0.0,
            pumping: 0.0,
            pump_acceleration: 0.0,
            angular_speed: 0.0,
            absorption: 0.0,
            ground_normal_absorption: 0.0,
            minimum_crouch: 0.0,
            deck_angle_absorption: 0.0,
            reset_only_scalar: 0.0,
            record_valid: false,
            intentional_pumping: 0,
        }
    }

    pub fn physics_output(&self) -> PumpingOutput {
        PumpingOutput {
            compression: 0.0,
            absorption: self.absorption,
            ground_normal_absorption: self.ground_normal_absorption,
            minimum_crouch: self.minimum_crouch,
            deck_angle_absorption: self.deck_angle_absorption,
            pump_acceleration: self.pump_acceleration,
            intentional_pumping: self.intentional_pumping,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PumpingOutput {
    pub compression: f32,
    pub absorption: f32,
    pub ground_normal_absorption: f32,
    pub minimum_crouch: f32,
    pub deck_angle_absorption: f32,
    pub pump_acceleration: f32,
    pub intentional_pumping: u8,
}
