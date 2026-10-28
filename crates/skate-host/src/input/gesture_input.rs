use skate_core::{
    graph::intents::IntentMap,
    input::gesture::{Recognizer, Settings},
};
use skate_data::collections::Collections;
use std::path::Path;

pub(crate) struct GestureInput {
    recognizers: Vec<(usize, Recognizer)>,
    maximum_misses: [u8; 2],
    held_pattern: Option<String>,
}
impl GestureInput {
    pub fn load(root: &Path) -> Result<Self, String> {
        let data = Collections::load(root)?;
        let misses = |key| -> Result<u8, String> {
            let field = data.field("recognizer", key, "NumTicksPatternNotInRangeBeforeCulling")?;
            if field.type_name != "EA::Reflection::UInt8" {
                return Err("Recognizer culling field must be UInt8".into());
            }
            u8::from_str_radix(&field.data, 16).map_err(|e| e.to_string())
        };
        let mut recognizers = Vec::new();
        for (stick, file) in [
            (1, "skater.pat"),
            (1, "skater90.pat"),
            (1, "skaterN90.pat"),
            (1, "skater_air.pat"),
            (1, "skater_fingerflip.pat"),
            (0, "skaterls.pat"),
            (0, "skaterstep.pat"),
        ] {
            recognizers.push((
                stick,
                Recognizer::new(skate_data::gesture_patterns::load(
                    &root.join("private/stock/data/joystick").join(file),
                )?)?,
            ));
        }
        Ok(Self {
            recognizers,
            maximum_misses: [misses("left_stick")?, misses("right_stick")?],
            held_pattern: None,
        })
    }

    pub fn publish(
        &mut self,
        axes: [[f32; 2]; 2],
        difficulty: u32,
        flags: u32,
        physical_state: u32,
        ag: &mut IntentMap,
    ) {
        let samples = axes.map(|[x, y]| [x, -y].map(|v| if v.abs() < 0.1 { 0.0 } else { v }));
        let mut events = Vec::new();
        let mut held = false;
        for stick in [1, 0] {
            for (_, recognizer) in self.recognizers.iter_mut().filter(|r| r.0 == stick) {
                if let Some(index) = recognizer.held(samples[stick]) {
                    if self.held_pattern.as_deref().is_some_and(|name| {
                        name.eq_ignore_ascii_case(&recognizer.patterns()[index].name)
                    }) {
                        held = true;
                    }
                }
            }
            for (_, recognizer) in self.recognizers.iter_mut().filter(|r| r.0 == stick) {
                if let Some(result) = recognizer.sample(
                    samples[stick],
                    Settings {
                        maximum_misses: self.maximum_misses[stick],
                        difficulty,
                    },
                ) {
                    let name = recognizer.patterns()[result.pattern].name.clone();
                    if ["Kickflip", "Heelflip", "N_Kickflip", "N_Heelflip"].contains(&name.as_str())
                    {
                        self.held_pattern = Some(name.clone());
                    }
                    events.push((name, result.strength));
                }
            }
        }
        if held {
            ag.insert("HoldPattern", 1.0);
        }
        for (name, strength) in events {
            if permitted(&name, flags, physical_state, ag) {
                ag.insert("Trick", 1.0);
                ag.insert(&name, 1.0);
                ag.insert("GestureSpeed", strength);
            }
        }
    }
}

fn permitted(name: &str, flags: u32, state: u32, ag: &IntentMap) -> bool {
    let bit = |n: u32| (flags & (1u32 << n)) != 0u32;
    let member = |names: &[&str]| names.iter().any(|n| n.eq_ignore_ascii_case(name));
    if bit(5) && member(&["FingerFlip", "FS_Varial", "BS_Varial"]) {
        return false;
    }
    if bit(4)
        && (matches!(state, 200 | 201)
            || member(&[
                "L_F_Kickflip",
                "L_B_Kickflip",
                "L_F_Heelflip",
                "L_B_Heelflip",
                "L_FS_Shuvit",
                "L_BS_Shuvit",
            ]))
    {
        return false;
    }
    if member(&["FrontFlip", "BackFlip"])
        && (!(ag.contains_key("RightAirGrab") || ag.contains_key("LeftAirGrab"))
            || ag.contains_key("LeftPush")
            || ag.contains_key("RightPush")
            || bit(3))
    {
        return false;
    }
    if bit(8) && member(&["SlideFs180", "SlideBs180"]) {
        return false;
    }
    if bit(12)
        && !member(&[
            "Ollie",
            "Kickflip",
            "Heelflip",
            "Hardflip",
            "InwardHeelflip",
            "VarialKickflip",
            "VarialHeelflip",
            "PopShuvit",
            "FSPopShuvit",
            "360PopShuvit",
            "FS360PopShuvit",
        ])
    {
        return false;
    }
    true
}

