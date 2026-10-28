use super::motion_animation::MotionAnimation;
use skate_core::{
    animation::{kickturn, playback_parameters::ParameterInputs, skeleton_input::name::encode},
    point_graph::PointGraph,
};
use skate_data::{collections::Collections, state_graph::attributes::Attributes};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    ResetTimer,
    Steering(kickturn::Parameters),
}

impl Operation {
    pub fn parse(a: &Attributes<'_>) -> Option<Self> {
        match a.text("name")? {
            "ResetTimeSinceKickturn" => Some(Self::ResetTimer),
            "KickTurnSteering" => Some(Self::Steering(kickturn::Parameters {
                max_height: f32::from_bits(a.float_bits("maxHeight", 0x3f000000)),
                spin_scale: f32::from_bits(a.float_bits("spinScale", 0x3f000000)),
            })),
            _ => None,
        }
    }
}

pub fn load_settings(data: &Collections) -> Result<kickturn::Settings, String> {
    let curve = |name| -> Result<PointGraph<8>, String> {
        let words = data.words::<20>("anim_motion", "kickturn", name)?;
        Ok(PointGraph {
            x: std::array::from_fn(|i| f32::from_bits(words[4 + i])),
            y: std::array::from_fn(|i| f32::from_bits(words[12 + i])),
        })
    };
    Ok(kickturn::Settings {
        spin: curve("kickturn_spin")?,
        height: curve("kickturn_balance")?,
    })
}

pub fn execute(
    state: &mut kickturn::State,
    parameters: kickturn::Parameters,
    settings: &kickturn::Settings,
    animation: &mut MotionAnimation,
    dt: f32,
    phase: u8,
) -> Result<(), String> {
    if phase == 0 {
        state.begin(settings);
        return Ok(());
    }
    if phase != 1 {
        return Ok(());
    }
    if state.needs_animation_length() {
        animation.apply_parameters()?;
        state.capture_animation_length(animation.current_length()?);
    }
    let output = state.update(
        dt,
        animation.motion_intent("KickTurn").unwrap_or(0.0),
        parameters,
        settings,
    );
    animation.emit_packet(encode(b"Balance"), output.balance);
    animation.emit_packet(encode(b"Spin"), output.spin);
    Ok(())
}
