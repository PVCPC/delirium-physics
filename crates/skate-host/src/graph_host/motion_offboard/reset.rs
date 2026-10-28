use skate_data::state_graph::attributes::Attributes;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    SkaterAnimation,
    GivenStance,
}
impl Operation {
    pub fn parse(a: &Attributes<'_>) -> Option<Self> {
        match a.text("name")? {
            "ResetSkaterAnimation" => Some(Self::SkaterAnimation),
            "ResetToGivenStance" => Some(Self::GivenStance),
            _ => None,
        }
    }
    pub fn begin<S: ResetOwners>(self, owners: &mut S) -> Result<(), S::Error> {
        match self {
            Self::SkaterAnimation => {
                owners.clear_action_intents()?;
                owners.reset_motion_graph_publications()?;
                owners.reset_skater_animation_playback()
            }
            Self::GivenStance => owners.reset_to_given_stance(),
        }
    }
}
pub trait ResetOwners {
    type Error;
    fn clear_action_intents(&mut self) -> Result<(), Self::Error>;
    fn reset_motion_graph_publications(&mut self) -> Result<(), Self::Error>;
    fn reset_skater_animation_playback(&mut self) -> Result<(), Self::Error>;
    fn reset_to_given_stance(&mut self) -> Result<(), Self::Error>;
}
