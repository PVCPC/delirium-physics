use super::{motion_animation::MotionAnimation, motion_riding::RidingState};
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug)]
pub struct Physical {
    pub centre_of_mass_velocity: [f32; 4],
    pub system_up: [f32; 4],
    pub board_reckoning_z: [f32; 4],
    pub board_reckoning: [[f32; 4]; 4],
}

#[derive(Clone, Copy, Debug)]
pub struct GesturePhysical {
    pub ground321: bool,
    pub state_offboard75: bool,
    pub selections: Option<[u32; 4]>,
    pub suppress_up: bool,
    pub force_brake_bypass: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    StoreLandingData,
    EndShimmy(f32),
    Score { regular: u32, mirrored: u32 },
}
impl Operation {
    pub fn parse(a: &Attributes<'_>) -> Result<Option<Self>, String> {
        Ok(Some(match a.text("name").unwrap_or("") {
            "StoreLandingData" => Self::StoreLandingData,
            "EndShimmy" => Self::EndShimmy(f32::from_bits(a.float_bits("blendtime", 0x3e4ccccd))),
            "SetScoreAugmentation" => {
                let decode = |name: &str| {
                    [
                        "FSPowerslide",
                        "BSPowerslide",
                        "FSRevert",
                        "BSRevert",
                        "NoseManual",
                        "TailManual",
                        "Wipeout",
                        "Landing",
                        "RideIdle",
                        "Switching",
                    ]
                    .iter()
                    .position(|v| *v == name)
                    .map(|n| n as u32)
                    .ok_or_else(|| format!("Unknown authored score augmentation {name}"))
                };
                let regular = decode(a.text("augment").unwrap_or(""))?;
                let mirrored = match a.text("mirrorAugment") {
                    Some("Switching") => {
                        return Err(
                            "Native mirrorAugment=Switching has no initialized mirrored value"
                                .into(),
                        );
                    }
                    Some(name) => decode(name)?,
                    None => regular,
                };
                Self::Score { regular, mirrored }
            }
            _ => return Ok(None),
        }))
    }
}

#[derive(Default)]
pub struct ScorePacket {
    pub handplant: Option<(skate_core::animation::output::attributes::AttributeName, [f32; 2])>,
    pub grab: Option<(skate_core::animation::output::attributes::AttributeName, [f32; 2])>,
    pub trick_names: super::motion_scoring_trick::Names,
    pub name: Option<u32>,
    pub flags: u32,
}
impl ScorePacket {
    pub fn set(&mut self, value: u32) {
        let bit = match value {
            0 => 31,
            1 => 30,
            2 => 29,
            3 => 28,
            4 => 27,
            5 => 26,
            6 => 23,
            7 => 21,
            8 => 22,
            9 => 20,
            _ => return,
        };
        self.flags |= 1 << bit;
        if value != 2 && value != 3 {
            self.name = Some(value);
        }
    }
}

#[derive(Default)]
pub struct LandingData {
    previous_velocity: f32,
    was_air: bool,
}
impl LandingData {
    pub fn update(&mut self, category: u32, p: Physical, riding: &mut RidingState) {
        let air = category == 2;
        if !self.was_air && air {
            riding.last_good_landing_velocity = 0.0;
        }
        self.was_air = air;
        let velocity = skate_core::riding::ground_correction_math::dot_product(
            p.centre_of_mass_velocity,
            p.system_up,
        );
        if velocity < 0.0 && velocity < self.previous_velocity {
            riding.last_good_landing_velocity = velocity.abs();
        }
        self.previous_velocity = velocity;
    }
}
pub fn end_shimmy(animation: &mut MotionAnimation, seconds: f32) {
    for name in [
        "SKCH_2H_SHIMMY_LEFT_CHANNEL",
        "SKCH_2H_SHIMMY_RIGHT_CHANNEL",
    ] {
        animation.channels.end_with(name, seconds, false);
    }
}
