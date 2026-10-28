pub mod animation;
pub mod conditions;
mod settings;
pub use settings::Settings;

use animation::{Animation, endpoints, set};
use skate_core::animation::{
    grind_control::{Fade, crouch, facing, mirrored_twist},
    output::attributes::AttributeName,
    skeleton_input::name::encode,
};
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Attributes,
    Crouch {
        height: AttributeName,
    },
    Fade {
        height: AttributeName,
        twist: AttributeName,
        intent: String,
    },
}
impl Operation {
    pub fn parse(a: &Attributes<'_>) -> Result<Option<Self>, String> {
        let required = |name| {
            a.text(name)
                .ok_or_else(|| format!("GrindControlFade requires {name}"))
        };
        Ok(Some(match a.text("name") {
            Some("CreateGrindAttributes") => Self::Attributes,
            Some("ControlGrindCrouch") => Self::Crouch {
                height: encode(a.text("driveDistToCom").unwrap_or("disttocog").as_bytes()),
            },
            Some("GrindControlFade") => Self::Fade {
                height: encode(required("distBoardToCogAnimAttribute")?.as_bytes()),
                twist: encode(required("twistAnimAttribute")?.as_bytes()),
                intent: required("twistMGIntent")?.into(),
            },
            _ => return Ok(None),
        }))
    }
}

#[derive(Default)]
pub struct State {
    fade: Option<Fade>,
    height: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub struct Physical {
    pub grinding: bool,
    pub grind_name: AttributeName,
    pub ground_axis: [f32; 3],
    pub board_axis: [f32; 3],
    pub ground_flag_273: bool,
    pub animation_mirrored: bool,
    pub height: f32,
    pub crouch: f32,
    pub twist: f32,
}

pub fn execute(
    state: &mut State,
    op: &Operation,
    phase: u8,
    dt: f32,
    settings: &Settings,
    physical: &Physical,
    animation: &mut impl Animation,
) -> Result<(), String> {
    match op {
        Operation::Attributes if phase == 1 && physical.grinding => {
            animation.emit_packet(physical.grind_name, 1.0);
            let backwards = facing::backwards(
                physical.ground_axis,
                physical.board_axis,
                physical.ground_flag_273,
                physical.animation_mirrored,
            );
            animation.emit_packet(
                encode(if backwards {
                    b"GrindFacingBackwards"
                } else {
                    b"GrindFacingForwards"
                }),
                1.0,
            );
        }
        Operation::Crouch { .. } if phase == 0 => state.height = Some(physical.height),
        Operation::Crouch { height } if phase == 1 => {
            let previous = state
                .height
                .ok_or("ControlGrindCrouch Update before Begin")?;
            let [min, max, rate] = settings.height;
            let next = crouch(
                previous,
                animation.intent("Crouch").unwrap_or(0.0),
                physical.crouch,
                min,
                max,
                rate,
                dt,
            );
            state.height = Some(next);
            set(animation, *height, next, false);
        }
        Operation::Fade {
            height,
            twist,
            intent,
        } => {
            if phase == 0 {
                set(animation, *height, physical.height, false);
                let bounds = endpoints(animation, *twist)?;
                let mut fade = Fade::default();
                let value = fade.begin(
                    bounds[0],
                    bounds[1],
                    mirrored_twist(physical.twist, physical.animation_mirrored),
                );
                set(animation, *twist, value, false);
                state.fade = Some(fade);
            } else if phase == 1 {
                let fade = state
                    .fade
                    .as_mut()
                    .ok_or("GrindControlFade Update before Begin")?;
                if let Some(value) =
                    fade.update(dt, animation.intent(intent).unwrap_or(0.0), settings.fade)
                {
                    set(animation, *twist, value, false);
                }
            }
        }
        _ => {}
    }
    Ok(())
}
