use super::motion_animation::MotionAnimation;
use skate_core::animation::{
    channel_playback::ChannelSettings,
    playback::TransitionSettings,
    playback_parameters::{AttributeSink, SettableAttribute},
    skeleton_input::name::encode,
};
use skate_core::graph::intents::IntentMap;

const CHANNELS: [&str; 3] = ["GestureBoth", "GestureRight", "GestureLeft"];
const TENTH: f32 = f32::from_bits(0x3DCC_CCCD);

pub struct CharacterGestureInputs<'a> {
    pub motion_intents: &'a IntentMap,
    pub busy_hands: [u32; 2],
    pub filtered_state: u32,
    pub ground321: bool,
    pub board_held311: bool,
    pub state_offboard75: bool,
    pub distance_to_cog: f32,
    pub selections: Option<[u32; 4]>,
    pub suppress_up: bool,
    pub force_brake_bypass: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct GesturePublication {
    pub gesture: u32,
    pub down: bool,
}

pub struct CharacterGesture {
    active: bool,
    stage: i32,
    direction: i32,
    hands: i32,
}
impl Default for CharacterGesture {
    fn default() -> Self {
        Self::new()
    }
}
impl CharacterGesture {
    pub fn new() -> Self {
        Self {
            active: false,
            stage: -1,
            direction: -1,
            hands: -1,
        }
    }

    pub fn end(&mut self, animation: &mut MotionAnimation) {
        end_all(animation);
        self.clear_active();
        self.stage = -1;
    }

    pub fn update(
        &mut self,
        animation: &mut MotionAnimation,
        input: CharacterGestureInputs<'_>,
    ) -> Result<Option<GesturePublication>, String> {
        let any_channel = CHANNELS.iter().any(|name| animation.channels.has(name));
        if self.active && !any_channel {
            self.stage = -1;
            self.clear_active();
        } else if !self.active && any_channel {
            end_all(animation);
            self.clear_active();
        }
        let held = [
            "GestureLeftHeld",
            "GestureRightHeld",
            "GestureUpHeld",
            "GestureDownHeld",
        ]
        .map(|name| has(input.motion_intents, name));
        let mut selected = select_start(input.motion_intents);
        let hands = select_hands(animation, &input);
        if selected == -1 && self.direction != -1 && !held[self.direction as usize] {
            selected = [3, 0, 2, 1]
                .into_iter()
                .find(|&direction| held[direction as usize])
                .unwrap_or(-1);
        }
        let braking = has(input.motion_intents, "ForceBrake") && !input.force_brake_bypass;
        let start = selected != -1
            && hands != -1
            && !braking
            && (!input.suppress_up || selected != 2)
            && (self.direction == -1 || !held[self.direction as usize])
            && (!self.active || selected != self.direction);
        if start {
            let name = animation_name(selected, hands, 0, input.selections)?;
            self.direction = selected;
            self.stage = 0;
            let transition = TransitionSettings {
                kind: 2,
                seconds: f32::from_bits(0x3E4C_CCCD),
                under: 0,
                matching: 0,
                use_channels_from_weights: false,
            };
            animation.transition_channel(
                CHANNELS[hands as usize],
                &name,
                settings(false, true),
                transition,
                true,
                true,
            )?;
            match hands {
                0 => {
                    animation.channels.end(CHANNELS[2]);
                    animation.channels.end(CHANNELS[1]);
                }
                1 => {
                    animation.channels.end(CHANNELS[2]);
                    animation.channels.end(CHANNELS[0]);
                }
                2 => {
                    animation.channels.end(CHANNELS[0]);
                    animation.channels.end(CHANNELS[1]);
                }
                _ => unreachable!(),
            }
            self.hands = hands;
            self.active = true;
        } else if self.active {
            if self.hands != hands {
                end_all(animation);
                self.clear_active();
            } else {
                let channel = CHANNELS[self.hands as usize];
                if !animation.channels.in_transition(channel)
                    && animation.channels.remaining(channel) < TENTH
                {
                    match self.stage {
                        0 => self.next_stage(animation, &input, 1)?,
                        1 if !held[self.direction as usize] => {
                            self.next_stage(animation, &input, 2)?
                        }
                        2 => {
                            self.stage = -1;
                            self.clear_active();
                        }
                        _ => {}
                    }
                }
            }
        }
        if !self.active {
            return Ok(None);
        }
        let gesture = selection(self.direction, input.selections)?;
        animation.set_attribute(SettableAttribute {
            name: encode(b"GstrDistToCog"),
            value: if input.state_offboard75 {
                1.0
            } else {
                input.distance_to_cog
            },
            normalized: false,
            sequence_id: -1,
        });
        Ok(Some(GesturePublication {
            gesture,
            down: self.direction == 3,
        }))
    }

    fn next_stage(
        &mut self,
        animation: &mut MotionAnimation,
        input: &CharacterGestureInputs<'_>,
        stage: i32,
    ) -> Result<(), String> {
        let name = animation_name(self.direction, self.hands, stage, input.selections)?;
        self.stage = stage;
        let transition = TransitionSettings {
            kind: 4,
            seconds: 0.0,
            under: 0,
            matching: 0,
            use_channels_from_weights: false,
        };
        animation.transition_channel(
            CHANNELS[self.hands as usize],
            &name,
            settings(stage == 1, false),
            transition,
            true,
            true,
        )?;
        Ok(())
    }
    fn clear_active(&mut self) {
        self.active = false;
        self.direction = -1;
        self.hands = -1;
    }
}

fn settings(keep_alive: bool, hold_in: bool) -> ChannelSettings {
    ChannelSettings {
        priority: 0,
        keep_alive,
        mirrored: false,
        speed: 1.0,
        blend_in: TENTH,
        hold_during_blend_in: hold_in,
        blend_out: TENTH,
        hold_during_blend_out: true,
        use_attributes: false,
    }
}
fn end_all(animation: &mut MotionAnimation) {
    for index in [2, 1, 0] {
        animation.channels.end(CHANNELS[index]);
    }
}
fn has(intents: &IntentMap, name: &str) -> bool {
    intents.contains_key(name)
}
fn select_start(intents: &IntentMap) -> i32 {
    [
        (3, "GestureDownStart"),
        (0, "GestureLeftStart"),
        (2, "GestureUpStart"),
        (1, "GestureRightStart"),
    ]
    .into_iter()
    .find(|(_, name)| has(intents, name))
    .map_or(-1, |(direction, _)| direction)
}
fn select_hands(animation: &MotionAnimation, input: &CharacterGestureInputs<'_>) -> i32 {
    if ["RetrieveBoard", "Shove", "WipeoutPushOff"]
        .iter()
        .any(|name| animation.channels.has(name))
        || has(input.motion_intents, "OB_Mount")
        || has(input.motion_intents, "OB_Dismount")
    {
        return -1;
    }
    if matches!(input.filtered_state, 6 | 7) && !input.ground321 {
        return if input.board_held311 { 2 } else { 0 };
    }
    match (input.busy_hands[0] != 0, input.busy_hands[1] != 0) {
        (false, false) => 0,
        (false, true) => 1,
        (true, false) => 2,
        (true, true) => -1,
    }
}
fn selection(direction: i32, selections: Option<[u32; 4]>) -> Result<u32, String> {
    let index = match direction {
        0 => 2,
        1 => 3,
        2 => 0,
        3 => 1,
        _ => return Ok(37),
    };
    Ok(selections.ok_or("CharacterGesture needs the actual skater gesture selections")?[index])
}
fn animation_name(
    direction: i32,
    hands: i32,
    stage: i32,
    selections: Option<[u32; 4]>,
) -> Result<String, String> {
    let id = selection(direction, selections)?;
    let prefix = NAMES
        .get(id as usize)
        .ok_or_else(|| format!("Invalid stock gesture selection {id}"))?;
    Ok(format!(
        "{prefix}_{}_{}",
        ["BOTH", "RIGHT", "LEFT"][hands as usize],
        ["INTO", "CYC", "OUT"][stage as usize]
    ))
}
const NAMES: [&str; 37] = [
    "B_GSTR_AIRGUITAR",
    "B_GSTR_AIRPLANE",
    "B_GSTR_BOXING",
    "B_GSTR_BRUCE_LEE",
    "B_GSTR_CHECKTIME",
    "B_GSTR_DEVIL",
    "B_GSTR_DBLE_GUN",
    "B_GSTR_DUNNO",
    "B_GSTR_FINGERWAG",
    "B_GSTR_FISTS",
    "B_GSTR_BICEPFLEX",
    "B_GSTR_FLIPTABLE",
    "B_GSTR_FONZ_EH",
    "B_GSTR_FREEDOM",
    "B_GSTR_FU_FIST",
    "B_GSTR_GETAWAY",
    "B_GSTR_GETOUTTAHERE",
    "B_GSTR_HANDCUFFS",
    "B_GSTR_HIGHPUMP",
    "B_GSTR_LOWPUMP",
    "B_GSTR_PREWIND",
    "B_GSTR_PEACE",
    "B_GSTR_POINT",
    "B_GSTR_RAISEROOF",
    "B_GSTR_SHAKA",
    "B_GSTR_SHRUG",
    "B_GSTR_POINT_SKY",
    "B_GSTR_SNAP",
    "B_GSTR_SOULARCH",
    "B_GSTR_SPOCK",
    "B_GSTR_SURFSUP",
    "B_GSTR_SWINGHIGH",
    "B_GSTR_SWINGLOW",
    "B_GSTR_THROWARMS",
    "B_GSTR_THMB_DWN",
    "B_GSTR_WINGS",
    "B_GSTR_YARDSALE",
];
