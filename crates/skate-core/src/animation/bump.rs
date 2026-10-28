use crate::physics::native_arithmetic;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub scale_x_acc: f32,
    pub min_bump_mag: f32,
    pub min_bump_blend_value: f32,
    pub max_bump_mag: f32,
}
pub fn coefficients(mut acceleration: [f32; 4], mirrored: bool, settings: &Settings) -> [f32; 2] {
    acceleration[0] *= settings.scale_x_acc;
    let squared = native_arithmetic::dot3(acceleration, acceleration);
    let mut inverse = native_arithmetic::reciprocal_square_root_estimate(squared);
    for _ in 0..2 {
        inverse = (inverse * 0.5).mul_add((-squared).mul_add(inverse * inverse, 1.), inverse);
    }
    let magnitude = if squared == 0. { 0. } else { squared * inverse };
    let t = ((magnitude - settings.min_bump_mag) / (settings.max_bump_mag - settings.min_bump_mag))
        .clamp(0., 1.);
    let weight = t.mul_add(
        1. - settings.min_bump_blend_value,
        settings.min_bump_blend_value,
    );
    let direction = if magnitude > f32::from_bits(0x3586_37bd) {
        [acceleration[0] * inverse, acceleration[2] * inverse]
    } else {
        [0.; 2]
    };
    direction.map(|v| {
        let value = v * weight;
        if mirrored { -value } else { value }
    })
}

