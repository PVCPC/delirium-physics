use crate::{
    math::Vector3,
    physics::{
        board_ground::angle_between,
        board_motion_output::{dot, inverse_length_squared, length},
        native_arithmetic::reciprocal_estimate,
    },
    point_graph::PointGraph,
};
#[derive(Clone, Debug)]
pub struct CollisionResponseSettings {
    pub maximum_velocity_delta: f32,
    pub force_y_offset: f32,
    pub force_scalar: f32,
    pub target_displacement_velocity: f32,
    pub torque_vs_angle: PointGraph<8>,
}
#[derive(Clone, Copy, Debug)]
pub struct CollisionResponsePhysical {
    pub flags_2472: u32,
    pub collision_displacement: [f32; 4],
    pub velocity: [f32; 4],
    pub forward: [f32; 4],
    pub up: [f32; 4],
    pub ground_normal: [f32; 4],
    pub time_step: f32,
    pub mass: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionResponse {
    pub applied: bool,
    pub force: [f32; 4],
    pub point: [f32; 4],
    pub angular_displacement: [f32; 4],
    pub target_velocity: [f32; 4],
}
pub fn collision_response(
    settings: &CollisionResponseSettings,
    input: CollisionResponsePhysical,
) -> Option<CollisionResponse> {
    if input.flags_2472 & 0x20000 == 0 {
        return None;
    }
    let projection = dot(xyz(input.collision_displacement), xyz(input.ground_normal));
    let displaced: [f32; 4] = std::array::from_fn(|i| {
        input.collision_displacement[i] - input.ground_normal[i] * projection
    });
    let distance = length(xyz(displaced));
    if distance < f32::from_bits(0x3780_0000) {
        return None;
    }
    let inverse_distance = 1.0 / distance;
    let direction: [f32; 4] = displaced.map(|v| v * inverse_distance);
    let speed_toward = dot(xyz(input.velocity), xyz(direction));
    let tangent: [f32; 4] =
        std::array::from_fn(|i| input.velocity[i] - direction[i] * speed_toward);
    let target_velocity = std::array::from_fn(|i| {
        direction[i].mul_add(settings.target_displacement_velocity, tangent[i])
    });
    let mut delta =
        std::array::from_fn(|i| (target_velocity[i] - input.velocity[i]) * settings.force_scalar);
    if speed_toward > settings.target_displacement_velocity {
        delta = [0.; 4];
    }
    let magnitude = length(xyz(delta));
    let inverse_dt = reciprocal_refined(input.time_step);
    let mut force = delta.map(|v| (v * input.mass) * inverse_dt);
    let applied = magnitude >= 0.01;
    if applied {
        let clamp = if magnitude <= settings.maximum_velocity_delta {
            1.0
        } else {
            settings.maximum_velocity_delta / magnitude
        };
        force = force.map(|v| v * clamp);
    } else {
        force = [0.; 4];
    }
    let angle = wrap_angle(signed_angle(
        xyz(input.forward),
        xyz(direction),
        xyz(input.up),
    ));
    let magnitude = settings
        .torque_vs_angle
        .evaluate(angle.abs() * f32::from_bits(0x3ea2_f983));
    let signed = if angle >= 0.0 { magnitude } else { -magnitude };
    Some(CollisionResponse {
        applied,
        force,
        point: [0.0, settings.force_y_offset, 0.0, 0.0],
        angular_displacement: input.up.map(|v| v * signed),
        target_velocity,
    })
}
pub fn signed_angle(a: Vector3, b: Vector3, up: Vector3) -> f32 {
    let a_squared = dot(a, a);
    let b_squared = dot(b, b);
    if !(a_squared > 0.0001 && b_squared > 0.0001) {
        return 0.0;
    }
    let angle = angle_between(a, b);
    let a = scale(a, inverse_length_squared(a_squared, 1));
    let b = scale(b, inverse_length_squared(b_squared, 1));
    let cross = Vector3::new(
        (-a.z).mul_add(b.y, a.y * b.z),
        (-a.x).mul_add(b.z, a.z * b.x),
        (-a.y).mul_add(b.x, a.x * b.y),
    );
    if dot(cross, up) < 0.0 {
        f32::from_bits(0x40c9_0fdb) - angle
    } else {
        angle
    }
}
fn wrap_angle(mut angle: f32) -> f32 {
    let pi = f32::from_bits(0x4049_0fdb);
    let tau = f32::from_bits(0x40c9_0fdb);
    if angle < -pi || !(angle < pi) {
        let turns = (angle * f32::from_bits(0x3e22_f983)) as i32;
        angle = (-(turns as f32)).mul_add(tau, angle);
        if !(angle < pi) {
            angle -= tau;
        } else if angle < -pi {
            angle += tau;
        }
    }
    angle
}
fn reciprocal_refined(value: f32) -> f32 {
    let estimate = reciprocal_estimate(value);
    let first = estimate.mul_add((-estimate).mul_add(value, 1.0), estimate);
    first.mul_add((-first).mul_add(value, 1.0), first)
}
fn scale(v: Vector3, k: f32) -> Vector3 {
    Vector3::new(v.x * k, v.y * k, v.z * k)
}
fn xyz(v: [f32; 4]) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}

