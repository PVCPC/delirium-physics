use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsPushOffEnabled;

impl IsPushOffEnabled {
    pub fn parse(attributes: &Attributes<'_>) -> Option<Self> {
        (attributes.text("name") == Some("IsPushOffEnabled")).then_some(Self)
    }

    pub fn evaluate(self) -> bool {
        true
    }
}
