use super::{controller::{DerivedControllerInput, magnitude}, riding_intentions::RidingIntent};

pub fn produce(controller: &DerivedControllerInput, actor_flags: u32) -> Vec<RidingIntent> {
    let mut intents = Vec::with_capacity(2);
    if actor_flags & (1 << 9) != 0 {
        return intents;
    }
    let words = controller.words();
    let [x, y] = [words[9], words[10]].map(f32::from_bits);
    let length = magnitude(x.mul_add(x, y * y));
    if y != 0.0 {
        let value = if y > 0.0 { length } else { -length };
        intents.push(RidingIntent { name: "Manual", value });
    }
    let threshold = f32::from_bits(0x3f66_6666);
    if length > threshold {
        let brake = (length - threshold) * f32::from_bits(0x411f_fffe);
        intents.push(RidingIntent {
            name: "ManualBrake",
            value: if y > 0.0 { brake } else { -brake },
        });
    }
    intents
}
