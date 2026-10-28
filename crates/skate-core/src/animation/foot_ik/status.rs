#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Disabled,
    OnDeck,
    External,
    Local,
}

#[derive(Clone, Copy, Debug)]
pub struct LimbStatus {
    pub mode: Mode,
    pub board_blend: f32,
    pub external_blend: f32,
    pub target_blend: f32,
    pub external_target_set: bool,
    pub local_target_set: bool,
    pub external_target_local_delta: [f32; 4],
    pub part_position: [f32; 4],
}
impl Default for LimbStatus {
    fn default() -> Self {
        Self {
            mode: Mode::Disabled,
            board_blend: 0.0,
            external_blend: 0.0,
            target_blend: 0.0,
            external_target_set: false,
            local_target_set: false,
            external_target_local_delta: [0.0; 4],
            part_position: [0.0; 4],
        }
    }
}

pub fn update_modes(limbs: &mut [LimbStatus; 4], feet_enabled: bool, flags_2472: u32) {
    let enabled = [
        feet_enabled,
        feet_enabled,
        flags_2472 & 0x80 != 0,
        flags_2472 & 0x100 != 0,
    ];
    for (limb, enabled) in limbs.iter_mut().zip(enabled) {
        let fallback = if enabled {
            Mode::OnDeck
        } else {
            Mode::Disabled
        };
        match limb.mode {
            Mode::Disabled => {
                limb.board_blend = 0.0;
                if limb.external_target_set {
                    limb.mode = Mode::External;
                    limb.external_blend = limb.target_blend;
                } else if limb.local_target_set {
                    limb.mode = Mode::Local;
                    limb.external_blend = limb.target_blend;
                    limb.external_target_local_delta = [0.0; 4];
                } else {
                    limb.mode = fallback;
                    limb.external_blend = 0.0;
                }
            }
            Mode::OnDeck => {
                if limb.external_target_set {
                    limb.mode = Mode::External;
                    limb.external_blend = limb.target_blend;
                } else if limb.local_target_set {
                    limb.mode = Mode::Local;
                    limb.external_blend = limb.target_blend;
                    limb.external_target_local_delta = [0.0; 4];
                } else if limb.board_blend == 0.0 {
                    limb.mode = fallback;
                    limb.external_blend = 0.0;
                }
            }
            Mode::External => {
                if !limb.external_target_set {
                    limb.mode = Mode::Local;
                }
            }
            Mode::Local => {
                if limb.external_target_set {
                    limb.mode = Mode::External;
                    limb.external_blend = limb.target_blend;
                }
                if !limb.local_target_set && limb.external_blend == 0.0 {
                    limb.mode = fallback;
                    limb.external_target_local_delta = [0.0; 4];
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BlendSettings {
    pub hand_inner_padding: [f32; 4],
    pub hand_outer_padding: [f32; 4],
    pub external_blend_step: f32,
    pub board_blend_step: f32,
}

pub fn update_blends(
    limbs: &mut [LimbStatus; 4],
    deck_half_width: f32,
    deck_total_half_length: f32,
    flags_2480: u32,
    settings: &BlendSettings,
) {
    let base = [deck_half_width, 0.0, deck_total_half_length, 0.0];
    let inner: [f32; 4] = core::array::from_fn(|i| base[i] + settings.hand_inner_padding[i]);
    let outer: [f32; 4] = core::array::from_fn(|i| inner[i] + settings.hand_outer_padding[i]);
    let step = settings.board_blend_step;
    for (index, limb) in limbs.iter_mut().enumerate() {
        match limb.mode {
            Mode::Disabled => {}
            Mode::External => {
                limb.external_blend = limb.target_blend;
                limb.board_blend = positive(limb.board_blend - step);
            }
            Mode::Local => {
                limb.external_blend = if limb.local_target_set {
                    1.0
                } else {
                    positive(limb.external_blend - settings.external_blend_step)
                };
                limb.board_blend = positive(limb.board_blend - step);
            }
            Mode::OnDeck if index < 2 => {
                let target = if flags_2480 & 0x0800_0000 != 0 {
                    0.0
                } else {
                    1.0
                };
                approach(&mut limb.board_blend, target, step);
            }
            Mode::OnDeck => {
                let p = limb.part_position.map(f32::abs);
                if limb.board_blend > 0.0 || p[0] < outer[0] && p[1] < outer[1] && p[2] < outer[2] {
                    let mut target = 1.0;
                    for axis in [0, 2, 1] {
                        if p[axis] > inner[axis] {
                            let candidate =
                                1.0 - (p[axis] - inner[axis]) / settings.hand_outer_padding[axis];
                            if axis == 0 {
                                target = candidate;
                            } else if axis == 2 {
                                target = select(target - candidate, candidate, target);
                            } else {
                                target = select(candidate - target, target, candidate);
                            }
                        }
                    }
                    target = positive(target);
                    target = select(1.0 - target, target, 1.0);
                    approach(&mut limb.board_blend, target, step);
                }
            }
        }
    }
}
fn select(condition: f32, nonnegative: f32, negative: f32) -> f32 {
    if condition >= 0.0 {
        nonnegative
    } else {
        negative
    }
}
fn positive(value: f32) -> f32 {
    select(-value, 0.0, value)
}
fn approach(value: &mut f32, target: f32, step: f32) {
    let delta = target - *value;
    let delta = select(-step - delta, -step, delta);
    *value = select(step - delta, delta, step) + *value;
}
