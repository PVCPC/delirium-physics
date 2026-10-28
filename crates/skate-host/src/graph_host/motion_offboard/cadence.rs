use skate_core::animation::{
    playback_parameters::{AttributeSink, SettableAttribute},
    skeleton_input::name::encode,
};
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    BipedCadence,
    MatchCadence,
}
impl Operation {
    pub(crate) fn parse(a: &Attributes<'_>) -> Option<Self> {
        match a.text("name") {
            Some("BipedCadence") => Some(Self::BipedCadence),
            Some("MatchCadence") => Some(Self::MatchCadence),
            _ => None,
        }
    }
}

pub(crate) fn biped_cadence(
    physical_phase: Option<f32>,
    animation_phase: Option<&mut f32>,
    phase: u8,
) {
    if phase == 1 {
        if let (Some(value), Some(output)) = (physical_phase, animation_phase) {
            *output = value;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MatchCadence {
    pub captured_phase: f32,
    pub pending: bool,
}
impl MatchCadence {
    pub(crate) fn begin(&mut self, physical_phase: Option<f32>, animation_present: bool) {
        if let (Some(value), true) = (physical_phase, animation_present) {
            self.pending = true;
            self.captured_phase = value;
        }
    }
    pub(crate) fn update(&mut self, animation: Option<&mut dyn AttributeSink>) {
        if let Some(animation) = animation {
            animation.set_attribute(SettableAttribute {
                name: encode(b"CadenceStartPercent"),
                value: self.captured_phase,
                normalized: false,
                sequence_id: -1,
            });
            self.pending = false;
        }
    }
    pub(crate) fn end(&mut self) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocoState {
    expected: u32,
}
impl LocoState {
    pub(crate) fn parse(a: &Attributes<'_>) -> Result<Self, String> {
        let expected = match a.text("locostate") {
            Some("Stand") => 0,
            Some("Walk") => 1,
            Some("Run") => 2,
            Some("Sprint") => 3,
            value => return Err(format!("Invalid stock LocoState locostate {value:?}")),
        };
        Ok(Self { expected })
    }
    pub(crate) fn evaluate(self, locomotion_state_84: u32) -> bool {
        locomotion_state_84 == self.expected
    }
}
