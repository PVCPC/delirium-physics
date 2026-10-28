use super::{
    board_ground::BoardGroundState,
    board_motion_output::{add, dot, inverse_length_squared, scale, subtract},
};
use crate::{math::Vector3, point_graph::PointGraph};
const UP: Vector3 = Vector3::new(0., 1., 0.);
const ZERO: Vector3 = Vector3::ZERO;
#[derive(Clone, Debug)]
pub struct DynamicNormalSettings {
    pub speed_damping: f32,
    pub up_vector_damping: f32,
    pub maximum_delta: f32,
    pub speed_scale: f32,
    pub maximum_delta_vs_speed: PointGraph<8>,
}
#[derive(Clone, Debug)]
pub struct BoardDynamicNormal {
    pub normal: Vector3,
    pub delta: Vector3,
    pub acceleration: Vector3,
    pub last_contact_normal: Vector3,
}
impl BoardDynamicNormal {
    pub fn new() -> Self {
        Self {
            normal: UP,
            delta: UP,
            acceleration: ZERO,
            last_contact_normal: ZERO,
        }
    }
    pub fn update(
        &mut self,
        contacts: &BoardGroundState,
        gravity: Vector3,
        previous_ground_speed: f32,
        settings: &DynamicNormalSettings,
    ) {
        if contacts.wheel_contact_count == 0 {
            self.acceleration = ZERO;
            return;
        }
        let mut sum = ZERO;
        for i in 0..3 {
            if contacts.parts[i].in_contact {
                sum = subtract(add(sum, contacts.accelerations[i]), gravity);
            }
        }
        let target = normalize(sum);
        self.delta = scale(self.delta, settings.speed_damping);
        let error = scale(subtract(target, self.normal), settings.up_vector_damping);
        let (direction, magnitude) = normalize_length(error);
        let maximum = settings
            .maximum_delta_vs_speed
            .evaluate(previous_ground_speed.abs() / settings.speed_scale)
            * settings.maximum_delta;
        let nonnegative = if -magnitude >= -0.0 { 0.0 } else { magnitude };
        let amount = if maximum - nonnegative >= -0.0 {
            nonnegative
        } else {
            maximum
        };
        self.delta = Vector3::new(
            direction.x.mul_add(amount, self.delta.x),
            direction.y.mul_add(amount, self.delta.y),
            direction.z.mul_add(amount, self.delta.z),
        );
        self.normal = normalize(add(self.normal, self.delta));
        self.last_contact_normal = contacts.overall_normal;
    }
}

fn normalize(v: Vector3) -> Vector3 {
    normalize_length(v).0
}
fn normalize_length(v: Vector3) -> (Vector3, f32) {
    let squared = dot(v, v);
    let inverse = inverse_length_squared(squared, 2);
    let length = if squared == 0.0 {
        0.0
    } else {
        squared * inverse
    };
    let normal = if length > f32::from_bits(0x3586_37bd) {
        scale(v, inverse)
    } else {
        ZERO
    };
    (normal, length)
}
