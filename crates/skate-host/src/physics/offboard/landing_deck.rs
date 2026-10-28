mod input;
mod output;
mod query;
use super::contact_toolkit::StaticScene;
use crate::physics::{animated_skeleton::AnimatedSkeleton, player_input::PlayerInputRuntime};
pub(crate) use output::PublishTargets;
use skate_core::player::offboard::landing_deck::{
    self as native, AssistInput, IkInput, Manager, QueryResult, Settings, UpdateOutput, Vector,
};
use skate_data::collections::Collections;

pub(crate) struct Owner {
    pub manager: Manager,
    pub settings: Settings,
    completion: Option<QueryResult>,
}
impl Owner {
    pub(crate) fn load(data: &Collections) -> Result<Self, String> {
        Ok(Self {
            manager: Manager::default(),
            settings: Settings {
                deck_min_uprightness: data.float(
                    "physics_landingondeck",
                    "default",
                    "DeckMinUprightness",
                )?,
                approximate_com_height: data.float(
                    "physics_landingondeck",
                    "default",
                    "ApproxCOMHeightOnLanding",
                )?,
            },
            completion: None,
        })
    }

    pub(crate) fn reset(&mut self) {
        self.manager.reset();
    }

    pub(crate) fn assist(
        &mut self,
        scene: &StaticScene<'_>,
        input: &PlayerInputRuntime,
        maximum_velocity_change: f32,
    ) -> Result<(), String> {
        let mut next = self.manager;
        let request = next.assist(
            &AssistInput {
                processed: input::processed(input)?,
                position: input::position(input),
                velocity: input::velocity(input),
                maximum_velocity_change,
            },
            &self.settings,
        );
        let completion = request
            .map(|request| query::execute(scene, request, &input.processed))
            .transpose()?;
        if let Some(completion) = completion {
            self.completion = Some(completion);
            next.query_submitted();
        }
        self.manager = next;
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        scene: &StaticScene<'_>,
        input: &PlayerInputRuntime,
    ) -> Result<UpdateOutput, String> {
        let mut next = self.manager;
        let output = next.update(&input::processed(input)?, &self.settings);
        let completion = output
            .query
            .map(|request| query::execute(scene, request, &input.processed))
            .transpose()?;
        if let Some(completion) = completion {
            self.completion = Some(completion);
            next.query_submitted();
        }
        self.manager = next;
        Ok(output)
    }

    pub(crate) fn calculate_accurate_ik_offset(
        &mut self,
        input: &PlayerInputRuntime,
        animation: &AnimatedSkeleton,
        mapped_position_12608: Vector,
        time_to_land: f32,
    ) -> Result<Vector, String> {
        Ok(self.manager.calculate_accurate_ik_offset(&IkInput {
            processed: input::processed(input)?,
            time_to_land,
            animation_root: animation.roots.animation_to_world,
            animation_com_10960: animation.record.centre_of_mass,
            mapped_position_12608,
        }))
    }

    pub(crate) fn fill(&self) -> native::FillOutput {
        self.manager.fill()
    }
}
