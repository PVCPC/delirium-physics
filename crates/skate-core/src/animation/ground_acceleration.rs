use crate::physics::{native_arithmetic, skeleton_animation_record::AnimationPartTransform};

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub deck: AnimationPartTransform,
    pub ground: AnimationPartTransform,
    pub world_acceleration: [f32; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub scale_x_acc: f32,
    pub min_bump_mag: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub acceleration: [f32; 4],
    pub bumped: bool,
}

pub fn publish(input: Input, settings: &Settings) -> Output {
    let acceleration = condition(input);
    Output {
        acceleration,
        bumped: is_bumped(acceleration, settings),
    }
}

pub fn condition(input: Input) -> [f32; 4] {
    let mut deck_local = inverse_rotate(input.deck, input.world_acceleration);
    deck_local[1] = 0.0;
    let world = rotate(input.deck, deck_local);
    let mut ground_local = inverse_rotate(input.ground, world);
    ground_local[1] = 0.0;
    ground_local
}

pub fn is_bumped(mut acceleration: [f32; 4], settings: &Settings) -> bool {
    acceleration[0] *= settings.scale_x_acc;
    let squared = native_arithmetic::dot3(acceleration, acceleration);
    let mut reciprocal = native_arithmetic::reciprocal_square_root_estimate(squared);
    for _ in 0..2 {
        let square = reciprocal * reciprocal;
        let half = reciprocal * 0.5;
        let error = (-squared).mul_add(square, 1.0);
        reciprocal = half.mul_add(error, reciprocal);
    }
    let magnitude = if squared == 0.0 {
        0.0
    } else {
        squared * reciprocal
    };
    magnitude > settings.min_bump_mag
}

fn inverse_rotate(frame: AnimationPartTransform, vector: [f32; 4]) -> [f32; 4] {
    core::array::from_fn(|lane| {
        frame[lane][2].mul_add(
            vector[2],
            frame[lane][1].mul_add(vector[1], frame[lane][0] * vector[0]),
        )
    })
}
fn rotate(frame: AnimationPartTransform, vector: [f32; 4]) -> [f32; 4] {
    core::array::from_fn(|lane| {
        frame[2][lane].mul_add(
            vector[2],
            frame[1][lane].mul_add(vector[1], frame[0][lane] * vector[0]),
        )
    })
}

