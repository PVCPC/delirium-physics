use skate_core::{input::{set_turning::Settings, turn_remap::TurnRemap}, point_graph::PointGraph};
use skate_data::collections::Collections;

pub fn load(data: &Collections) -> Result<Settings, String> {
    let float = |name| data.float("anim_carving", "default", name);
    Ok(Settings {
        remaps: [
            TurnRemap { magnitude: curve16(data, "quickMagMap")?, angle: curve16(data, "quickAngleMap")?, angle_offset: float("quickAngleOffset")? },
            TurnRemap { magnitude: curve16(data, "slowMagMap")?, angle: curve16(data, "slowAngleMap")?, angle_offset: float("slowAngleOffset")? },
        ],
        speed_tuck: curve8(data, "speed_tuck")?,
        blend: curve8(data, "blend_lean_in")?,
        speed_threshold: float("speed_tuck_start")?,
        maximum_delta: float("clamp_curr_lean")?,
        override_turn: data.float("anim_motion", "power_slide", "slide_turn_value")?,
    })
}
fn curve16(data: &Collections, name: &str) -> Result<PointGraph<16>, String> {
    let words = data.words::<36>("anim_carving", "default", name)?;
    Ok(PointGraph {
        x: std::array::from_fn(|i| f32::from_bits(words[4+i])),
        y: std::array::from_fn(|i| f32::from_bits(words[20+i])),
    })
}
fn curve8(data: &Collections, name: &str) -> Result<PointGraph<8>, String> {
    let words = data.words::<20>("anim_carving", "default", name)?;
    Ok(PointGraph {
        x: std::array::from_fn(|i| f32::from_bits(words[4+i])),
        y: std::array::from_fn(|i| f32::from_bits(words[12+i])),
    })
}
