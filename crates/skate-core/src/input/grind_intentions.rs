use super::{controller::DerivedControllerInput, riding_intentions::RidingIntent};

pub fn produce(controller: &DerivedControllerInput) -> Vec<RidingIntent> {
    let words = controller.words();
    let left_x = f32::from_bits(words[7]);
    let right_x = f32::from_bits(words[9]);
    let right_y = f32::from_bits(words[10]);
    let sum = left_x + right_x;
    let lower = if -1.0 - sum >= 0.0 { -1.0 } else { sum };
    let translation = if 1.0 - lower >= 0.0 { lower } else { 1.0 };
    [
        ("GrindBalanceX", -left_x),
        ("PhysGrindTranslation", translation),
        ("PhysGrindStabilityNudge", left_x),
        ("PhysGrindUpDown", right_y),
    ]
    .into_iter()
    .filter(|(_, value)| *value != 0.0)
    .map(|(name, value)| RidingIntent { name, value })
    .collect()
}
