use super::{
    motion_animation::MotionAnimation,
    motion_landing::{Flags, Operation, Physical, State},
    motion_riding_conditions::MotionRandom,
};
use skate_core::animation::{
    playback_parameters::{AttributeSink, SettableAttribute},
    skeleton_input::name::encode,
};

#[allow(clippy::too_many_arguments)]
pub fn execute(
    operation: &Operation,
    state: &mut State,
    flags: &mut Flags,
    animation: &mut MotionAnimation,
    random: &MotionRandom,
    physical: Option<Physical>,
    mirrored: Option<bool>,
    dt: f32,
    phase: u8,
) -> Result<(), String> {
    match *operation {
        Operation::IsAnticipating => {
            if phase != 1 {
                flags.anticipating = phase == 0;
            }
        }
        Operation::IsLanding => {
            if phase != 1 {
                flags.landing = phase == 0;
            }
        }
        Operation::IsManualing => {
            if phase != 1 {
                flags.manualing = phase == 0;
            }
        }
        Operation::IsDoingTrick => {
            if phase != 1 {
                flags.doing_trick = phase == 0;
            }
        }
        Operation::SetLandingData => match phase {
            0 => {
                let p = physical.ok_or("SetLandingData requires actual landing publication")?;
                let mirror = mirrored.ok_or("SetLandingData requires animation stance")?;
                set(animation, b"Spin", if mirror { p.spin } else { -p.spin });
                set(animation, b"AvgVelY", p.last_good_landing_velocity);
                state.value = p.height;
            }
            1 => set(animation, b"disttocog", state.value),
            _ => {}
        },
        Operation::DisableTricks { length } => match phase {
            0 => {
                flags.tricks_allowed = false;
                state.value = length;
                state.complete = false;
            }
            1 => {
                if !state.complete {
                    state.value -= dt;
                    if state.value < 0.0 {
                        flags.tricks_allowed = true;
                        state.complete = true;
                    }
                }
            }
            _ => flags.tricks_allowed = true,
        },
        Operation::ChooseRandomLanding { count } => match phase {
            0 => {
                if count <= 0 {
                    return Err("ChooseRandomLanding requires numlandings > 0".into());
                }
                let value = (random.next_u32()? as i32).wrapping_abs() % count;
                let name = encode(b"Random");
                let value = encode(value.to_string().as_bytes());
                if let Some(item) = animation
                    .construction_values
                    .iter_mut()
                    .find(|(key, _)| *key == name)
                {
                    item.1 = value;
                } else {
                    animation.construction_values.push((name, value));
                }
            }
            2 => animation
                .construction_values
                .retain(|(name, _)| *name != encode(b"Random")),
            _ => {}
        },
    }
    Ok(())
}
fn set(animation: &mut MotionAnimation, name: &[u8], value: f32) {
    animation.set_attribute(SettableAttribute {
        name: encode(name),
        value,
        normalized: false,
        sequence_id: -1,
    });
}
