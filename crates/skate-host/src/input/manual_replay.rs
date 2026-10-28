use skate_core::input::gamepad::GamepadState;

pub(crate) const END_TICK: u64 = 330;

pub(crate) fn sample(tick: u64, sign: i16) -> GamepadState {
    GamepadState {
        buttons: if (12..40).contains(&tick) { 0x1000 } else { 0 },
        triggers: [0; 2],
        left: [0; 2],
        right: if (45..195).contains(&tick) {
            [0, sign * 22000]
        } else {
            [0; 2]
        },
    }
}
