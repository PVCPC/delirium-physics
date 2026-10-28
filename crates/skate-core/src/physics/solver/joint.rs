#[path = "joint_data.rs"]
pub(super) mod data;
use data::{
    ConstraintGeometry, ReactionState, project_correction, publish_reactions, read_column,
    write_column, xyz,
};

pub(super) fn solve(record: &mut [u32], a: &mut [u32], b: &mut [u32]) {
    let geometry = ConstraintGeometry::decode(record);
    let mut body_a = ReactionState::decode(a);
    let mut body_b = ReactionState::decode(b);
    let (relative_point, relative_angular) = geometry.relative_corrections(&body_a, &body_b);
    let old_linear = read_column(record, 32);
    let old_angular = read_column(record, 48);
    let linear_candidate =
        project_correction(&geometry.linear_projection, relative_point, old_linear);
    let angular_candidate =
        project_correction(&geometry.angular_projection, relative_angular, old_angular);

    let linear_low = read_column(record, 160);
    let linear_high = read_column(record, 176);
    let angular_low = [
        read_column(record, 96)[3],
        read_column(record, 112)[3],
        linear_low[3],
        linear_low[3],
    ];
    let angular_high = [
        read_column(record, 128)[3],
        read_column(record, 144)[3],
        linear_high[3],
        linear_high[3],
    ];
    let angular_impulse = core::array::from_fn(|axis| {
        joint_limit_correction(
            angular_candidate[axis],
            angular_low[axis],
            angular_high[axis],
        )
    });
    let linear_impulse = core::array::from_fn(|axis| {
        joint_limit_correction(linear_candidate[axis], linear_low[axis], linear_high[axis])
    });
    let angular_change = xyz(core::array::from_fn(|axis| {
        angular_impulse[axis] - old_angular[axis]
    }));
    let linear_change = xyz(core::array::from_fn(|axis| {
        linear_impulse[axis] - old_linear[axis]
    }));

    write_column(record, 48, angular_impulse);
    write_column(record, 32, linear_impulse);
    geometry.apply_impulse_changes(&mut body_a, &mut body_b, linear_change, angular_change);
    publish_reactions(&body_a, &body_b, a, b);
}

fn joint_limit_correction(candidate: f32, low: f32, high: f32) -> f32 {
    let high_error = high + candidate;
    let low_error = low + candidate;
    let negative_correction = if high_error < 0.0 { high_error } else { 0.0 };
    let positive_correction = if low_error > 0.0 { low_error } else { 0.0 };
    positive_correction + negative_correction
}
