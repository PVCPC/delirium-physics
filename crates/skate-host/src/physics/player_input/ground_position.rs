use skate_core::physics::{
    board_runtime::BoardRuntime, skeleton_animation_record::AnimationPartTransform,
};

pub(crate) fn ground_position(board: &BoardRuntime, ground: &AnimationPartTransform) -> [u32; 4] {
    let wheels = std::array::from_fn(|i| {
        let p = board.bodies()[i].rates.position;
        [p.x, p.y, p.z, 0.0]
    });
    wheel_ground_position(wheels, ground).map(f32::to_bits)
}

fn wheel_ground_position(wheels: [[f32; 4]; 4], ground: &AnimationPartTransform) -> [f32; 4] {
    let transpose =
        std::array::from_fn(|axis| [ground[0][axis], ground[1][axis], ground[2][axis], 0.0]);
    let local = wheels.map(|position| rotate(&transpose, position));
    let mut mean =
        std::array::from_fn(|i| (((local[0][i] + local[1][i]) + local[2][i]) + local[3][i]) * 0.25);
    let first_minimum = if local[1][1] > local[0][1] {
        local[0][1]
    } else {
        local[1][1]
    };
    let next_minimum = if local[2][1] - first_minimum >= 0.0 {
        first_minimum
    } else {
        local[2][1]
    };
    mean[1] = if local[3][1] - next_minimum >= 0.0 {
        next_minimum
    } else {
        local[3][1]
    };
    rotate(&[ground[0], ground[1], ground[2]], mean)
}

fn rotate(basis: &[[f32; 4]; 3], point: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| {
        let x = basis[0][i] * point[0];
        let xy = basis[1][i].mul_add(point[1], x);
        basis[2][i].mul_add(point[2], xy)
    })
}

