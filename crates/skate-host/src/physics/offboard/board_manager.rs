use super::board_possession::{self, Effects};
use skate_core::{
    physics::{assembly::BodySnapshot, drive_solver::RetailDriveRows},
    player::{
        lifecycle::{SkateboardControllerActions, SkateboardControllerFields},
        offboard::board_possession::{
            self as native, Frame,
            lifecycle::{Observation, Settings},
        },
    },
};
use skate_data::collections::Collections;
pub(crate) mod runtime;

pub(crate) struct Owner {
    pub state: native::State,
    settings: Settings,
}

impl Owner {
    pub(crate) fn load(data: &Collections) -> Result<Self, String> {
        Ok(Self {
            state: native::State::default(),
            settings: board_possession::settings::load(data)?,
        })
    }

    pub(crate) fn update(
        &mut self,
        fields: &mut SkateboardControllerFields,
        observation: &Observation,
        effects: &mut Effects<'_>,
    ) {
        self.state
            .update(fields, observation, &self.settings, effects);
    }

    pub(crate) fn hold(
        &mut self,
        fields: &mut SkateboardControllerFields,
        observation: &Observation,
        effects: &mut Effects<'_>,
    ) {
        self.state.hold(fields, observation, effects);
    }

    pub(crate) fn let_go(
        &mut self,
        fields: &mut SkateboardControllerFields,
        observation: &Observation,
        effects: &mut Effects<'_>,
    ) {
        self.state
            .let_go(fields, observation, &self.settings, effects);
    }

    pub(crate) fn stop(
        &mut self,
        fields: &mut SkateboardControllerFields,
        observation: &Observation,
        effects: &mut Effects<'_>,
    ) {
        self.state
            .stop(fields, observation, &self.settings, effects);
    }

    pub(crate) fn reset_for_teleport(
        &mut self,
        fields: &mut SkateboardControllerFields,
        observation: &Observation,
        effects: &mut Effects<'_>,
    ) {
        fields.word_444 = 0;
        self.stop(fields, observation, effects);
        self.state.retrieval = native::Retrieval::default();
    }

    pub(crate) fn append_drives(
        &self,
        deck: BodySnapshot,
        hands: [BodySnapshot; 2],
        deck_reaction: usize,
        hand_reactions: [usize; 2],
        dt: f32,
        rows: &mut Vec<RetailDriveRows>,
    ) {
        board_possession::drives::append(
            &self.state,
            deck,
            hands,
            deck_reaction,
            hand_reactions,
            dt,
            rows,
        );
    }

    pub(crate) fn fill(
        &self,
        fields: &SkateboardControllerFields,
        processed: &native::Processed,
        bone_11: Frame,
    ) -> native::Fill {
        native::fill(fields, &self.state, processed, bone_11)
    }
}

pub(crate) struct Transition<'a, 'board> {
    pub owner: &'a mut Owner,
    pub observation: &'a Observation,
    pub effects: &'a mut Effects<'board>,
    previous: SkateboardControllerFields,
    changed: bool,
}

impl<'a, 'board> Transition<'a, 'board> {
    pub(crate) fn new(
        owner: &'a mut Owner,
        observation: &'a Observation,
        effects: &'a mut Effects<'board>,
        previous: SkateboardControllerFields,
    ) -> Self {
        Self {
            owner,
            observation,
            effects,
            previous,
            changed: false,
        }
    }

    pub(crate) fn finish(self, fields: &mut SkateboardControllerFields) {
        if self.changed {
            fields.word_444 = self.previous.word_444;
        }
    }
}

impl SkateboardControllerActions for Transition<'_, '_> {
    fn hold_skateboard(&mut self) {
        self.previous.word_444 = 0;
        self.owner
            .hold(&mut self.previous, self.observation, self.effects);
        self.changed = true;
    }

    fn let_go_of_skateboard(&mut self) {
        self.previous.word_444 = 0;
        self.owner
            .let_go(&mut self.previous, self.observation, self.effects);
        self.changed = true;
    }
}
