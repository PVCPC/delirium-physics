use super::{angle::left_stick_angle, controller::magnitude};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SteeringIntentions {
    pub turn: Option<f32>,
    pub hard_turn: Option<f32>,
    pub hard_turn_crouch: Option<f32>,
}

pub fn produce(left: [f32; 2], actor_flags_1908: u32) -> SteeringIntentions {
    let [x, y] = left;
    let angle = left_stick_angle(x, y);
    let absolute = angle.abs();
    let length = if x == 0.0 && y == 0.0 {
        0.0
    } else {
        magnitude(y.mul_add(y, x * x))
    };
    let signed_length = if x >= 0.0 { length } else { -length };
    let turn = if y > 0.0 {
        if absolute > f32::from_bits(0x40433333) {
            0.0
        } else if absolute > f32::from_bits(0x40166666) {
            ((f32::from_bits(0x40433333) - absolute) * f32::from_bits(0x3fb6db6d)) * signed_length
        } else {
            signed_length
        }
    } else {
        x
    };
    let active = length > f32::from_bits(0x3f666666)
        && !(absolute > f32::from_bits(0x3fb1eb85))
        && !(absolute < f32::from_bits(0x3f68f5c3));
    let hard = (absolute - f32::from_bits(0x3fb1eb85)) * f32::from_bits(0xc0055556);
    let minimum = f32::from_bits(0x3a83126f);
    let hard = if hard - minimum >= 0.0 { hard } else { minimum };
    let hard = if angle >= 0.0 { hard } else { -hard };
    let allowed = actor_flags_1908 & 1 == 0;
    SteeringIntentions {
        turn: (turn != 0.0 && allowed).then_some(turn),
        hard_turn: (active && allowed).then_some(hard),
        hard_turn_crouch: (active && allowed).then_some((hard * f32::from_bits(0x3f59999a)).abs()),
    }
}

