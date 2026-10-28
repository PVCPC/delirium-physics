use skate_core::animation::{output::attributes::AttributeName, skeleton_input::name::encode};
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Debug, PartialEq)]
pub struct Operation {
    pub trick: AttributeName,
}

impl Operation {
    pub fn parse(attributes: &Attributes<'_>) -> Result<Self, String> {
        let trick = attributes.text("trick").ok_or("ScoringTrick requires authored trick")?;
        Ok(Self { trick: encode(trick.as_bytes()) })
    }

    pub fn execute(&self, names: &mut Names, flags: &mut u32, phase: u8) {
        if phase != 1 {
            return;
        }
        *flags |= 0x0100_0000;
        names.first = Some(self.trick);
        names.second = Some(self.trick);
    }
}

#[derive(Default, Debug)]
pub struct Names {
    pub first: Option<AttributeName>,
    pub second: Option<AttributeName>,
}
