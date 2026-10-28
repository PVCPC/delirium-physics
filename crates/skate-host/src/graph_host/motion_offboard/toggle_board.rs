use super::motion::MotionAnimation;
use skate_core::{
    animation::{
        channel_playback::ChannelSettings,
        playback::TransitionSettings,
        playback_parameters::{AttributeSink, SettableAttribute},
        skeleton_input::name::encode,
    },
    player::offboard::toggle_board::{Channel, Clip, Command, Input, State},
};
use crate::graph_host::outputs::BoardControls;

#[derive(Clone, Copy, Debug)]
pub struct Physical {
    pub grabbing_object: bool,
    pub holding_board: bool,
    pub free_board: bool,
    pub retrieval_blocked: bool,
    pub retrieval_active: bool,
    pub yaw_radians: f32,
    pub pitch_radians: f32,
}
const CHANNEL: &str = "RetrieveBoard";

pub fn execute(
    state: &mut State,
    animation: &mut MotionAnimation,
    actions: BoardControls,
    physical: Option<Physical>,
    mirrored: Option<bool>,
    phase: u8,
) -> Result<(), String> {
    if phase == 0 {
        state.begin();
        return Ok(());
    }
    if phase != 1 {
        return Ok(());
    }
    let physical = physical.ok_or("ToggleBoard requires completed board-possession output (OffBoard36/40/304/311/313 and bundle28 byte87)")?;
    let input = Input {
        grabbing_object: physical.grabbing_object,
        holding_board: physical.holding_board,
        retrieval_blocked: physical.retrieval_blocked,
        retrieval_active: physical.retrieval_active,
        yaw_radians: physical.yaw_radians,
        pitch_radians: physical.pitch_radians,
        mirrored: mirrored.ok_or("ToggleBoard requires the actual animation mirror state")?,
        drop_requested: actions.drop_requested,
        throw_requested: actions.throw_requested,
        retrieve_requested: actions.retrieve_requested,
    };
    let output = state.update(
        input,
        Channel {
            exists: animation.channels.has(CHANNEL),
            remaining: animation.channels.remaining(CHANNEL),
            elapsed: animation.channels.elapsed(CHANNEL),
        },
    );
    if output.retrieving {
        publish(animation, "OB_RetrievingBoard");
    }
    if output.dropping {
        publish(animation, "OB_DroppingBoard");
    }
    match output.channel {
        Some(Command::Stop { blend_seconds }) => {
            animation.channels.end_with(CHANNEL, blend_seconds, false)
        }
        Some(command) => {
            let (clip, sequence) = match command {
                Command::BlendTo(clip) => (clip, false),
                Command::SequenceTo(clip) => (clip, true),
                Command::Stop { .. } => unreachable!(),
            };
            let is_out = matches!(clip, Clip::FrontOut | Clip::BackOut);
            let hold = sequence || is_out;
            let settings = ChannelSettings {
                priority: 0,
                keep_alive: false,
                mirrored: false,
                speed: 1.0,
                blend_in: 0.25,
                blend_out: 0.25,
                hold_during_blend_in: hold,
                hold_during_blend_out: hold,
                use_attributes: true,
            };
            animation.transition_channel(
                CHANNEL,
                clip.name(),
                settings,
                TransitionSettings {
                    kind: if sequence { 4 } else { 2 },
                    seconds: if sequence { 0.0 } else { 0.25 },
                    under: 0,
                    matching: 0,
                    use_channels_from_weights: false,
                },
                true,
                !sequence && !is_out,
            )?;
        }
        None => {}
    }
    if output.retrieve {
        publish(animation, "OB_RetrieveBoard");
    }
    if let Some([yaw, pitch]) = output.yaw_pitch {
        attribute(animation, "yaw", yaw);
        attribute(animation, "pitch", pitch);
    }
    Ok(())
}
fn publish(animation: &mut MotionAnimation, name: &str) {
    animation.emit_packet(encode(name.as_bytes()), 1.0);
    attribute(animation, name, 1.0);
}
fn attribute(animation: &mut MotionAnimation, name: &str, value: f32) {
    animation.set_attribute(SettableAttribute {
        name: encode(name.as_bytes()),
        value,
        normalized: false,
        sequence_id: -1,
    });
}

