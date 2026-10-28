use super::{
    angle::left_stick_angle,
    controller::{DerivedControllerInput, magnitude},
    riding_intentions::RidingIntent,
};

pub fn produce(controller: &DerivedControllerInput) -> Vec<RidingIntent> {
    let words = controller.words();
    let x = f32::from_bits(words[9]);
    let y = f32::from_bits(words[10]);
    if x == 0.0 && y == 0.0 {
        return Vec::new();
    }
    vec![
        RidingIntent {
            name: "AnticMag",
            value: magnitude(x.mul_add(x, y * y)),
        },
        RidingIntent {
            name: "AnticAngle",
            value: left_stick_angle(x, y),
        },
    ]
}

