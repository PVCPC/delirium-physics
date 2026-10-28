use super::motion::MotionHost;
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug)]
pub struct GameplayConditions {
    pub state: u32,
    pub wants_runout: bool,
    pub physics_wiping: bool,
    pub body_flipping: bool,
    pub wants_wipeout: bool,
    pub bumped: bool,
    pub grabbing_object: bool,
    pub retrieving_board: bool,
    pub dropping_board: bool,
    pub in_biped_air: bool,
    pub hippy_hurdling: bool,
    pub handplant_flags: u32,
    pub handplant_time: f32,
    pub handplant_thresholds: [f32; 3],
    pub footplant_active: bool,
    pub footplant_duration: f32,
    pub footplant_contact_time: f32,
    pub time_to_skitch: f32,
    pub skitch_transition_time: f32,
    pub time_to_land: f32,
    pub time_to_land_valid: bool,
    pub offboard_time_to_land: f32,
    pub offboard_air_scalar_92: f32,
    pub offboard_air_translation: [f32; 4],
    pub offboard_landing_normal: [f32; 4],
    pub offboard_committed_to_motion: bool,
    pub offboard_obstacle_distance: f32,
    pub offboard_edge_distance: f32,
    pub offboard_trajectory_time: f32,
    pub offboard_trajectory_valid: bool,
    pub reached_apex: bool,
    pub can_land_on_board: bool,
    pub landing_turning: bool,
    pub grind_contact: bool,
    pub wheel_contact: bool,
    pub trucks_or_deck_contact: bool,
    pub moving_object: bool,
    pub tricks_blocked_on_stairs: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GameplayCondition {
    RetrievingBoard,
    DroppingBoard,
    InBipedAir,
    HippyHurdling,
    WantsRunout,
    PhysicsWiping,
    BodyFlipping,
    WantsWipeout,
    Bumped,
    CanEnterSlide { right: bool },
    GrabbingObject,
    Dark,
    UnderflipRequested,
    DarkCatchRequested,
    OkToDoTrickOnStairs,
    HandPlanting { state: u8, direction: u8 },
    FootPlanting,
    PrepareFootplant,
    NewHandplantPosition,
    PlayHandplant { phase: usize },
    EnteringSkitch,
    Skitching,
    IsMovingObject,
}
impl GameplayCondition {
    pub fn recognizes(name: &str) -> bool {
        matches!(
            name,
            "IsRetrievingSkateboard"
                | "IsDroppingSkateboard"
                | "IsInBipedAir"
                | "IsHippyHurdling"
                | "PhysicsWantsRunout"
                | "IsPhysicsWiping"
                | "IsBodyFlipping"
                | "PhysicsWantsWipeOut"
                | "IsBumped"
                | "CanEnterSlide"
                | "IsGrabbingObject"
                | "IsDark"
                | "IsUnderflipRequested"
                | "IsDarkCatchRequested"
                | "OkToDoTrickOnStairs"
                | "IsHandPlanting"
                | "IsFootPlanting"
                | "ShouldPrepareOneFootAirForFootplant"
                | "HasNewHandPlantPos"
                | "ShouldPlayHandPlantAnim"
                | "IsEnteringSkitch"
                | "IsSkitching"
                | "IsMovingObject"
        )
    }
    pub fn parse(a: &Attributes<'_>) -> Result<Self, String> {
        Ok(match a.text("name").unwrap_or("") {
            "IsRetrievingSkateboard" => Self::RetrievingBoard,
            "IsDroppingSkateboard" => Self::DroppingBoard,
            "IsInBipedAir" => Self::InBipedAir,
            "IsHippyHurdling" => Self::HippyHurdling,
            "PhysicsWantsRunout" => Self::WantsRunout,
            "IsPhysicsWiping" => Self::PhysicsWiping,
            "IsBodyFlipping" => Self::BodyFlipping,
            "PhysicsWantsWipeOut" => Self::WantsWipeout,
            "IsBumped" => Self::Bumped,
            "CanEnterSlide" => Self::CanEnterSlide {
                right: a.boolean_byte("right", 1) != 0,
            },
            "IsGrabbingObject" => Self::GrabbingObject,
            "IsDark" => Self::Dark,
            "IsUnderflipRequested" => Self::UnderflipRequested,
            "IsDarkCatchRequested" => Self::DarkCatchRequested,
            "OkToDoTrickOnStairs" => Self::OkToDoTrickOnStairs,
            "IsEnteringSkitch" => Self::EnteringSkitch,
            "IsSkitching" => Self::Skitching,
            "IsFootPlanting" => Self::FootPlanting,
            "ShouldPrepareOneFootAirForFootplant" => Self::PrepareFootplant,
            "HasNewHandPlantPos" => Self::NewHandplantPosition,
            "ShouldPlayHandPlantAnim" => Self::PlayHandplant { phase: match a.text("anim") {
                Some("antic") => 2, Some("into") => 1, Some("out") => 0,
                value => return Err(format!("Unauthored handplant animation phase {value:?}")),
            } },
            "IsMovingObject" => Self::IsMovingObject,
            "IsHandPlanting" => Self::HandPlanting {
                state: match a.text("state").unwrap_or("") {
                    "air" => 0,
                    "ground" => 1,
                    value => return Err(format!("IsHandPlanting has unauthored state {value:?}")),
                },
                direction: match a.text("dir").unwrap_or("") {
                    "FS" => 1,
                    "BS" => 0,
                    _ => 2,
                },
            },
            name => return Err(format!("Unknown gameplay condition {name}")),
        })
    }
    pub fn evaluate_physical(&self, p: &GameplayConditions) -> Option<bool> {
        Some(match self {
            Self::OkToDoTrickOnStairs => !p.tricks_blocked_on_stairs,
            Self::RetrievingBoard => p.retrieving_board,
            Self::InBipedAir => p.in_biped_air,
            Self::HippyHurdling => p.hippy_hurdling,
            Self::WantsRunout => p.wants_runout,
            Self::PhysicsWiping => p.physics_wiping,
            Self::BodyFlipping => p.body_flipping,
            Self::WantsWipeout => p.wants_wipeout,
            Self::Bumped => p.bumped,
            Self::GrabbingObject => p.grabbing_object,
            Self::Skitching => p.state == 104,
            Self::IsMovingObject => p.moving_object,
            Self::EnteringSkitch => {
                !(p.time_to_skitch < 0.0) && p.time_to_skitch <= p.skitch_transition_time
            }
            Self::HandPlanting { state, direction } => {
                let active = if *state == 0 {
                    p.state == 600
                } else {
                    p.handplant_flags & 0x8000_0000 != 0
                };
                active
                    && match direction {
                        2 => true,
                        1 => p.handplant_flags & 0x2000_0000 != 0,
                        0 => p.handplant_flags & 0x2000_0000 == 0,
                        _ => false,
                    }
            }
            Self::FootPlanting => p.footplant_active && p.footplant_duration >= 0.0,
            Self::PrepareFootplant => p.footplant_contact_time >= 0.1 && p.footplant_contact_time <= 0.5,
            Self::NewHandplantPosition => p.handplant_flags & 0x4000_0000 != 0,
            Self::PlayHandplant { phase } => {
                let threshold = p.handplant_thresholds[*phase];
                p.handplant_time - f32::from_bits(0x3c888889) < if *phase == 0 { -threshold } else { threshold }
            }
            Self::DroppingBoard
            | Self::Dark
            | Self::UnderflipRequested
            | Self::DarkCatchRequested
            | Self::CanEnterSlide { .. } => return None,
        })
    }

    pub fn evaluate(&self, host: &MotionHost) -> Result<bool, String> {
        let p = host
            .gameplay_conditions
            .as_ref()
            .ok_or("MotionGraph requires the actual physical condition publication")?;
        if let Some(result) = self.evaluate_physical(p) {
            return Ok(result);
        }
        Ok(match self {
            Self::DroppingBoard => p.dropping_board || host.animation.channels.has("RetrieveBoard"),
            Self::Dark => host.riding.dark,
            Self::UnderflipRequested => host.trick_requests.underflip,
            Self::DarkCatchRequested => host.trick_requests.dark_catch,
            Self::CanEnterSlide { right } => {
                if p.state == 102 {
                    false
                } else {
                    let fakie = host
                        .animation
                        .skater_animation_flags
                        .ok_or("CanEnterSlide requires actual SkaterAnim stance flags")?
                        & 0x2000_0000
                        != 0;
                    if *right ^ fakie {
                        host.slide_latch.start(true)
                    } else {
                        host.slide_latch.start(false)
                    }
                }
            }
            _ => unreachable!("Physical condition handled above"),
        })
    }
}
