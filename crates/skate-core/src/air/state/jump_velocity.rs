use super::data::{PhysicsAirFrame, Vector4};

const VERTICAL_ERROR_FLOOR: f32 = f32::from_bits(0xC040_0000);
const HORIZONTAL_ERROR_LIMIT_SQUARED: f32 = f32::from_bits(0x4110_0000);

pub trait PhysicsAirMath {
    fn minimum_vminfp(&mut self, left: f32, right: f32) -> f32;

    fn length_squared_vmsum3fp(&mut self, value: Vector4) -> f32;

    fn length_vmsum3fp_vrsqrte(&mut self, value: Vector4) -> f32;

    fn clamp_vector_within_max_length(&mut self, value: Vector4, maximum_length: f32) -> Vector4;
}

pub fn calculate_velocity_from_jump(
    frame: &PhysicsAirFrame,
    frames: i32,
    math: &mut impl PhysicsAirMath,
) -> Vector4 {
    let frame_seconds = frame.delta_time_2604 * frames as f32;
    let gravity_delta = frame_seconds * frame.gravity_y_2648;
    let mut predicted = frame.jump_velocity_848;
    predicted[0] += 0.0;
    predicted[1] += gravity_delta;
    predicted[2] += 0.0;
    predicted[3] += 0.0;

    let current = frame.current_velocity_400;
    let mut error = core::array::from_fn(|lane| current[lane] - predicted[lane]);
    let mut output = predicted;

    if !(error[1] > VERTICAL_ERROR_FLOOR) {
        output[1] = current[1];
    }

    error[1] = 0.0;
    let horizontal_error_squared = math.length_squared_vmsum3fp(error);
    if !(horizontal_error_squared < HORIZONTAL_ERROR_LIMIT_SQUARED) {
        let mut predicted_horizontal = output;
        predicted_horizontal[1] = 0.0;
        let maximum_length = math.length_vmsum3fp_vrsqrte(predicted_horizontal);

        let mut current_horizontal = current;
        current_horizontal[1] = 0.0;
        let clamped = math.clamp_vector_within_max_length(current_horizontal, maximum_length);
        output[0] = clamped[0];
        output[2] = clamped[2];
    }
    output
}
