use super::super::wipeout::Observations;
use skate_core::{physics::grind_forces::post, player::wipeout::Requests};

pub(crate) fn check(
    state: &mut Requests,
    settings: post::Settings,
    observations: &Observations<'_>,
) -> Result<(), String> {
    post::check(state, settings, &observations.frame()?);
    Ok(())
}
