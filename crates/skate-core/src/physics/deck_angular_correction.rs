use super::{
    board_motion_output::{dot, inverse_length_squared},
    native_arithmetic::reciprocal_estimate,
    rigid_body::RetailBodyRates,
};
use crate::math::{Basis3, Vector3};
const STEP: f32 = f32::from_bits(0x3c88_8889);
const NORMAL_MINIMUM: f32 = f32::from_bits(0x3586_37bd);

pub fn apply_axis_displacement(body: &mut RetailBodyRates, requested: Vector3) {
    let squared = dot(requested, requested);
    let inverse = inverse_length_squared(squared, 2);
    let length = if squared == 0.0 { 0.0 } else { squared * inverse };
    let direction = if length > NORMAL_MINIMUM {
        scale(requested, inverse)
    } else { Vector3::ZERO };
    let existing = scale(direction, dot(direction, scale(body.angular_velocity, STEP)));
    apply_angular_displacement(body, subtract(requested, existing));
}

pub fn apply_limited_displacement(body: &mut RetailBodyRates, requested: Vector3) {
    let squared = dot(requested, requested);
    let inverse = inverse_length_squared(squared, 2);
    let length = if squared == 0.0 {
        0.0
    } else {
        squared * inverse
    };
    let direction = if length > NORMAL_MINIMUM {
        scale(requested, inverse)
    } else {
        Vector3::ZERO
    };
    let existing = scale(
        direction,
        dot(direction, scale(body.angular_velocity, STEP)),
    );
    let remainder = subtract(requested, existing);
    let remainder = if dot(remainder, requested) < 0.0 {
        Vector3::ZERO
    } else {
        remainder
    };
    let selected = if dot(existing, requested) < 0.0 {
        requested
    } else {
        remainder
    };
    apply_angular_displacement(body, selected);
}

pub fn apply_angular_displacement(body: &mut RetailBodyRates, displacement: Vector3) {
    let tensor = body.world_inverse_inertia;
    let inverse = invert_symmetric(tensor);
    let inverse_step = reciprocal_refined(STEP);
    let target_rate = scale(displacement, inverse_step);
    let momentum = multiply(inverse, target_rate);
    let torque = scale(momentum, reciprocal_refined(STEP));
    let acceleration = multiply(tensor, torque);
    body.torque_acceleration = Vector3::new(
        body.torque_acceleration.x + acceleration.x,
        body.torque_acceleration.y + acceleration.y,
        body.torque_acceleration.z + acceleration.z,
    );
    body.cool_down = 0;
}

fn invert_symmetric(tensor: Basis3) -> Basis3 {
    let [a, b, c] = tensor.columns.map(|[x, y, z]| Vector3::new(x, y, z));
    let first = cross(b, c);
    let second = cross(c, a);
    let third = cross(a, b);
    let inverse_det = reciprocal_refined(dot(a, first));
    Basis3 {
        columns: [
            [first.x, second.x, third.x].map(|x| x * inverse_det),
            [first.y, second.y, third.y].map(|x| x * inverse_det),
            [first.z, second.z, third.z].map(|x| x * inverse_det),
        ],
    }
}
fn reciprocal_refined(value: f32) -> f32 {
    let estimate = reciprocal_estimate(value);
    let first = estimate.mul_add((-estimate).mul_add(value, 1.0), estimate);
    first.mul_add((-first).mul_add(value, 1.0), first)
}
fn cross(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(
        (-a.z).mul_add(b.y, a.y * b.z),
        (-a.x).mul_add(b.z, a.z * b.x),
        (-a.y).mul_add(b.x, a.x * b.y),
    )
}
fn multiply(matrix: Basis3, v: Vector3) -> Vector3 {
    let lane = |i: usize| {
        matrix.columns[2][i].mul_add(
            v.z,
            matrix.columns[1][i].mul_add(v.y, matrix.columns[0][i] * v.x),
        )
    };
    Vector3::new(lane(0), lane(1), lane(2))
}
fn scale(v: Vector3, x: f32) -> Vector3 {
    Vector3::new(v.x * x, v.y * x, v.z * x)
}
fn subtract(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

pub fn apply_ground_body_torque(body: &mut RetailBodyRates) {
    let local = Vector3::new(0.0, ground_torque_scalar(), 0.0);
    let world = multiply(body.basis, local);
    let acceleration = multiply(body.world_inverse_inertia, world);
    body.torque_acceleration = Vector3::new(
        acceleration.x + body.torque_acceleration.x,
        acceleration.y + body.torque_acceleration.y,
        acceleration.z + body.torque_acceleration.z,
    );
    body.cool_down = 0;
}
fn ground_torque_scalar() -> f32 {
    let x = 0.25_f32;
    let square = x * x;
    let cube = x * square;
    let fourth = square * square;
    let log = [
        0x3fb8aa0e, 0xbf389e52, 0x3ef5162d, 0xbeb1d204, 0x3e77adbd, 0xbe0cd4fb, 0x3d5541c6,
        0xbc188b0b,
    ]
    .map(f32::from_bits);
    let low = cube.mul_add(log[3], square.mul_add(log[2], x.mul_add(log[1], log[0])));
    let high = cube.mul_add(log[7], square.mul_add(log[6], x.mul_add(log[5], log[4])));
    let exponent = (-x).mul_add(fourth.mul_add(high, low), -3.0);
    let integral = exponent.floor();
    let fraction = exponent - integral;
    let square = fraction * fraction;
    let cube = fraction * square;
    let fourth = square * square;
    let exp = [
        0x3f800000, 0xbf317218, 0x3e75fded, 0xbd6357ca, 0x3c1d8c54, 0xbaae1854, 0x391aa7d7,
        0xb7364261,
    ]
    .map(f32::from_bits);
    let low = cube.mul_add(
        exp[3],
        square.mul_add(exp[2], fraction.mul_add(exp[1], exp[0])),
    );
    let high = cube.mul_add(
        exp[7],
        square.mul_add(exp[6], fraction.mul_add(exp[5], exp[4])),
    );
    let polynomial = fourth.mul_add(high, low);
    let power_of_two = f32::from_bits(((127 + integral as i32) as u32) << 23);
    (power_of_two * reciprocal_refined(polynomial)) * f32::from_bits(0xc0c1_999a)
}

