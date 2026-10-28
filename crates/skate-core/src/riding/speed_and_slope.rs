use crate::{physics::native_arithmetic, point_graph::PointGraph, trigonometry};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedAndSlopeSettings {
    pub turn_torque_vs_speed: PointGraph<8>,
    pub turn_torque_vs_slope: PointGraph<8>,
    pub heading_adjust_max_speed: f32,
}

impl SpeedAndSlopeSettings {
    pub fn calculate(&self, ground_normal_y: f32, forward_speed: f32) -> f32 {
        let normal_y = native_arithmetic::vector_min(
            1.0,
            native_arithmetic::vector_max(-1.0, ground_normal_y),
        );
        let half_pi = f32::from_bits(0x3FC9_0FDB);
        let mut inverse_half_pi = native_arithmetic::reciprocal_estimate(half_pi);
        for _ in 0..2 {
            let error = (-inverse_half_pi).mul_add(half_pi, 1.0);
            inverse_half_pi = inverse_half_pi.mul_add(error, inverse_half_pi);
        }
        let mut slope = inverse_half_pi * trigonometry::acos(normal_y);
        if slope > f32::from_bits(0x3F99_999A) {
            slope = 0.0;
        }

        let absolute_speed = forward_speed.abs();
        let nonnegative_speed = if -absolute_speed >= -0.0 {
            0.0
        } else {
            absolute_speed
        };
        let bounded_speed = if self.heading_adjust_max_speed - nonnegative_speed >= -0.0 {
            nonnegative_speed
        } else {
            self.heading_adjust_max_speed
        };
        let speed_fraction = bounded_speed / self.heading_adjust_max_speed;
        let speed_factor = self.turn_torque_vs_speed.evaluate(speed_fraction);
        let slope_factor = self.turn_torque_vs_slope.evaluate(slope);
        speed_factor * slope_factor
    }
}

