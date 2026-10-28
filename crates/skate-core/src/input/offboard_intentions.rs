use super::controller::DerivedControllerInput;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffboardIntent {
    pub name: &'static str,
    pub value: f32,
}

pub fn produce_discrete(
    controller: &DerivedControllerInput,
    actor_flags: u32,
    air_reckoning_active: bool,
) -> Vec<OffboardIntent> {
    let words = controller.words();
    let previous = words[6];
    let current = words[13];
    let axis = |slot: usize| f32::from_bits(words[slot]);
    let held = |bit: u32| current & (1u32 << bit) != 0;
    let rising = |bit: u32| held(bit) && previous & (1u32 << bit) == 0;
    let mut output = Vec::with_capacity(12);
    let mut emit = |name, value| output.push(OffboardIntent { name, value });

    if rising(23) {
        emit("OB_Jump", 1.0);
    }
    let right_x = axis(9);
    let right_y = axis(10);
    if right_x != 0.0 {
        emit("OB_LookAtX", right_x);
    }
    if right_y != 0.0 {
        emit("OB_LookAtY", right_y);
    }
    let drop_board = axis(4) != 1.0 && axis(11) == 1.0;
    let throw_board = axis(5) != 1.0 && axis(12) == 1.0;
    if drop_board {
        emit("OB_DropBoard", 1.0);
    }
    if throw_board {
        emit("OB_ThrowBoard", 1.0);
    }
    if drop_board || throw_board {
        emit("OB_RetrieveBoard", 1.0);
    }
    if held(28) || held(30) {
        emit("OB_DoAirBodyTweak", 1.0);
    }
    emit("OB_AirBodyTweakX", right_x);
    emit("OB_AirBodyTweakY", right_y);

    if !air_reckoning_active && !held(29) && held(22) {
        if (rising(22) || !(axis(23) >= f32::from_bits(0x3cf5_c28f)))
            && actor_flags & (1 << 10) == 0
        {
            emit("NewToggleOffBoardState", 1.0);
        }
        emit("ToggleOffBoardState", 1.0);
    }
    if !(axis(21) > 0.0) && axis(20) > 0.0 {
        emit("OB_Sprint", 1.0);
    }
    output
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalogObservation {
    pub effective_skeleton_z: [f32; 4],
    pub biped_correction: Option<[f32; 4]>,
}

pub fn produce_analog(
    controller: &DerivedControllerInput,
    observation: AnalogObservation,
) -> [OffboardIntent; 4] {
    let words = controller.words();
    let stick = [f32::from_bits(words[7]), 0.0, f32::from_bits(words[8]), 0.0];
    let mut horizontal = observation.effective_skeleton_z;
    horizontal[1] = 0.0;
    let forward = safe_unit(horizontal);
    let magnitude = if let Some(normal) = observation.biped_correction {
        let negative = normal.map(|value| f32::from_bits(value.to_bits() ^ 0x8000_0000));
        let projection = dot(stick, negative);
        let lower = if -projection >= 0.0 { 0.0 } else { projection };
        let coefficient = if 1.0 - lower >= 0.0 { lower } else { 1.0 };
        if !(dot(safe_unit(stick), negative) <= f32::from_bits(0x3f66_6666)) {
            0.0
        } else {
            let corrected = std::array::from_fn(|i| normal[i].mul_add(coefficient, stick[i]));
            dot(forward, corrected)
        }
    } else {
        dot(forward, stick)
    };
    [
        OffboardIntent { name: "OB_Mag", value: magnitude },
        OffboardIntent { name: "OB_BipedWorldZ", value: stick[2] },
        OffboardIntent { name: "OB_BipedWorldX", value: stick[0] },
        OffboardIntent { name: "OB_BipedStickMag", value: super::controller::magnitude(dot(stick, stick)) },
    ]
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn safe_unit(vector: [f32; 4]) -> [f32; 4] {
    let squared = dot(vector, vector);
    let mut inverse = crate::physics::reciprocal_sqrt::estimate(squared);
    for _ in 0..2 {
        let correction = (-squared).mul_add(inverse * inverse, 1.0);
        inverse = (inverse * 0.5).mul_add(correction, inverse);
    }
    let length = if squared == 0.0 { 0.0 } else { squared * inverse };
    if length > f32::from_bits(0x3586_37bd) {
        vector.map(|value| value * inverse)
    } else {
        [0.0; 4]
    }
}
