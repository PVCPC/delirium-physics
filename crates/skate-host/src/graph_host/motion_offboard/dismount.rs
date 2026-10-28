use skate_core::graph::conditions::PushBrakeInputs;
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Condition;

impl Condition {
    pub fn parse(attributes: &Attributes<'_>) -> Option<Self> {
        (attributes.text("name") == Some("DisableDismount")).then_some(Self)
    }

    pub fn evaluate(self, physical: Option<&PushBrakeInputs>) -> Result<bool, &'static str> {
        let physical = physical.ok_or("DisableDismount requires actual Ground80 and Skeleton598")?;
        Ok(f32::from_bits(0x3F23_D70A) > physical.ground_axis_y
            || physical.skeleton_disables_push_brake)
    }
}
