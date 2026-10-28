use super::controller::DerivedControllerInput;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrickIntent {
    pub name: &'static str,
    pub value: f32,
}

pub fn produce(controller: &DerivedControllerInput) -> Vec<TrickIntent> {
    let words = controller.words();
    let left_trigger = f32::from_bits(words[11]);
    let right_trigger = f32::from_bits(words[12]);
    let right_x = f32::from_bits(words[9]);
    let right_y = f32::from_bits(words[10]);

    let left_ground = left_trigger == 1.0;
    let right_ground = right_trigger == 1.0;
    let left_air = left_trigger > 0.0;
    let right_air = right_trigger > 0.0;

    let mut output = Vec::with_capacity(12);
    let mut emit = |name, value| output.push(TrickIntent { name, value });
    if left_ground {
        emit("LeftGroundGrab", 1.0);
    }
    if left_air {
        emit("LeftAirGrab", 1.0);
    }
    if right_ground {
        emit("RightGroundGrab", 1.0);
    }
    if right_air {
        emit("RightAirGrab", 1.0);
    }

    let previous_flags = words[6];
    let current_flags = words[13];
    let dismount_button = 1 << 20;
    if current_flags & dismount_button != 0 {
        emit("Dismount", 1.0);
        if previous_flags & dismount_button == 0 {
            emit("NewDismount", 1.0);
        }
    }
    let dark_flags = (1 << 20) | (1 << 28);
    if current_flags & dark_flags != 0 {
        emit("DarkCatch", 1.0);
    }
    if current_flags & dark_flags != 0 && previous_flags & dark_flags == 0 {
        emit("NewDarkCatch", 1.0);
    }
    let board_adjust_magnitude = (right_x.mul_add(right_x, right_y * right_y)).sqrt();
    if board_adjust_magnitude > 0.0 {
        emit("BoardAdjustAngle", right_x.atan2(-right_y));
        emit("BoardAdjustMag", board_adjust_magnitude);
    }
    if right_x != 0.0 {
        emit("TweakX", right_x);
    }
    if right_y != 0.0 {
        emit("TweakY", right_y);
    }
    if current_flags & (1 << 28) != 0 {
        emit("GrabWorld", 1.0);
    }
    if right_x != 0.0 {
        emit("HandPlantTweakX", right_x);
    }
    if right_y != 0.0 {
        emit("HandPlantTweakY", right_y);
    }
    for (bit, name) in [(20, "HandPlantDismount"), (21, "HandPlantOneFootRight"), (23, "HandPlantOneFootLeft")] {
        if current_flags & (1 << bit) != 0 {
            emit(name, 1.0);
        }
    }
    output
}

