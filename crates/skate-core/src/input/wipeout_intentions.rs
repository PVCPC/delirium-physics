use super::{controller::DerivedControllerInput, riding_intentions::RidingIntent};

pub fn produce(
    controller: &DerivedControllerInput,
    actor_flags_1908: u32,
    physical_flags_204: u32,
) -> Vec<RidingIntent> {
    let words = controller.words();
    let old_buttons = words[6];
    let buttons = words[13];
    let axis = |slot| f32::from_bits(words[slot]);
    let rising = |bit: u32| old_buttons & (1u32 << bit) == 0u32 && buttons & (1u32 << bit) != 0u32;
    let trigger_edge = |previous, current| axis(previous) != 1.0 && axis(current) == 1.0;
    let mut output = Vec::with_capacity(7);
    let mut emit = |name, value| output.push(RidingIntent { name, value });

    let gesture = physical_flags_204 & 0x80 != 0;
    emit("WipeoutGestureX", if gesture { axis(9) } else { 0.0 });
    emit("WipeoutGestureY", if gesture { axis(10) } else { 0.0 });
    let control = physical_flags_204 & 0x100 != 0;
    let x = if control { axis(7) } else { 0.0 };
    let y = if control { axis(8) } else { 0.0 };
    if x != 0.0 {
        emit("WipeoutControlX", x);
    }
    if y != 0.0 {
        emit("WipeoutControlY", y);
    }
    if rising(21) || rising(23) {
        emit("WipeOutRecover", 1.0);
    }
    if actor_flags_1908 & 0x40 == 0
        && axis(11) == 1.0
        && axis(12) == 1.0
        && buttons & 0xC000_0000 == 0xC000_0000
        && (trigger_edge(4, 11) || trigger_edge(5, 12) || rising(31) || rising(30))
    {
        emit("WipeOutRequest", 1.0);
    }
    if physical_flags_204 & 0x40 != 0 && trigger_edge(5, 12) {
        emit("WipeOutPushOff", 1.0);
    }
    output
}
