use skate_core::{
    animation::{
        playback_parameters::{AttributeSink, SettableAttribute},
        skeleton_input::name::encode,
    },
    player::offboard::twist_lean::{Observation, State},
};
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Operation {
    pub always: bool,
}
impl Operation {
    pub fn parse(attributes: &Attributes<'_>) -> Self {
        Self { always: attributes.text("update") == Some("always") }
    }

    pub fn execute(
        self,
        state: &mut State,
        observation: Option<Observation>,
        animation: Option<&mut impl AttributeSink>,
        phase: u8,
    ) {
        match phase {
            0 => state.begin(observation),
            1 => {
                if let Some(animation) = animation {
                    let values = state.update(self.always, observation);
                    for (name, value) in [("Twist", values.twist), ("Lean", values.lean)] {
                        animation.set_attribute(SettableAttribute {
                            name: encode(name.as_bytes()),
                            value,
                            normalized: false,
                            sequence_id: -1,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}
