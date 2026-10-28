use super::{
    lifecycle::{
        PhysicalPlayerStateLifecycle, PhysicalStateCalls, SkateboardControllerActions,
        StateBinding, StateChangeData,
    },
    selector::{StateSelectionInput, StateSelector},
    state::UnknownPhysicalState,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsOutputStaging {
    pub bytes_20_through_53: [u8; 34],
    pub scalars_56_through_188: [f32; 34],
    pub word_200: u32,
}

impl PhysicsOutputStaging {
    pub fn clear_like_n8(&mut self) {
        self.word_200 = 0;
        self.bytes_20_through_53.fill(0);
        self.scalars_56_through_188.fill(0.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicResult {
    pub previous: StateBinding,
    pub selected: StateBinding,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalPlayerLogic {
    pub selector: StateSelector,
    pub lifecycle: PhysicalPlayerStateLifecycle,
    pub frames_since_jump_1304: u32,
}

impl PhysicalPlayerLogic {
    pub fn new(initial_state: super::state::PhysicalStateId) -> Self {
        Self {
            selector: StateSelector::default(),
            lifecycle: PhysicalPlayerStateLifecycle::new(initial_state),
            frames_since_jump_1304: 0,
        }
    }

    pub fn run<S, C>(
        &mut self,
        selection_input: &StateSelectionInput,
        state_change_data: &mut StateChangeData,
        output_staging: &mut PhysicsOutputStaging,
        states: &mut S,
        controller_actions: &mut C,
    ) -> Result<LogicResult, UnknownPhysicalState>
    where
        S: PhysicalStateCalls,
        C: SkateboardControllerActions,
    {
        let previous = self.lifecycle.active();
        let previous_type = states.get_type(previous);
        let suggested = self.selector.calculate(previous_type, selection_input);
        let changed = suggested != previous_type;
        let selected = if changed {
            self.frames_since_jump_1304 = 100;
            self.lifecycle.set_physics_state(
                suggested as u32,
                state_change_data,
                states,
                controller_actions,
            )?
        } else {
            previous
        };
        output_staging.clear_like_n8();
        Ok(LogicResult {
            previous,
            selected,
            changed,
        })
    }
}

