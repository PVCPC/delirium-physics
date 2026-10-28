use super::state::{PhysicalStateId, UnknownPhysicalState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateBinding {
    pub state: PhysicalStateId,
    pub owner_offset: u32,
}

impl StateBinding {
    pub const fn new(state: PhysicalStateId) -> Self {
        Self {
            state,
            owner_offset: state.native_owner_offset(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStateChangeFields {
    pub word_1312: u32,
    pub previous_category_latch_1336: u32,
    pub scalar_1344: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProcessedStateChangeFields {
    pub flags_2480: u32,
    pub requested_state_2500: u32,
    pub previous_state_2504: u32,
    pub current_state_2508: u32,
    pub current_category_2512: u32,
    pub previous_category_2516: u32,
    pub previous_category_latch_2520: u32,
    pub word_2564: u32,
    pub scalar_2664: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkateboardControllerFields {
    pub word_444: u32,
    pub state_448: u32,
    pub system_on_452: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateChangeData {
    pub player: PlayerStateChangeFields,
    pub processed: ProcessedStateChangeFields,
    pub skateboard_controller: SkateboardControllerFields,
}

pub trait SkateboardControllerActions {
    fn hold_skateboard(&mut self);
    fn let_go_of_skateboard(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateCall {
    pub state: StateBinding,
    pub active: StateBinding,
}

pub trait PhysicalStateCalls {
    fn get_type(&mut self, state: StateBinding) -> PhysicalStateId;
    fn exit(&mut self, call: StateCall);
    fn enter(&mut self, call: StateCall);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalPlayerStateLifecycle {
    active: StateBinding,
}

impl PhysicalPlayerStateLifecycle {
    pub const fn new(initial_state: PhysicalStateId) -> Self {
        Self {
            active: StateBinding::new(initial_state),
        }
    }

    pub const fn active(&self) -> StateBinding {
        self.active
    }

    pub fn set_physics_state<S, C>(
        &mut self,
        requested_raw: u32,
        data: &mut StateChangeData,
        states: &mut S,
        controller_actions: &mut C,
    ) -> Result<StateBinding, UnknownPhysicalState>
    where
        S: PhysicalStateCalls,
        C: SkateboardControllerActions,
    {
        let requested = PhysicalStateId::try_from(requested_raw)?;

        prepare_skateboard_controller(
            requested,
            &mut data.skateboard_controller,
            data.processed.flags_2480,
            controller_actions,
        );

        data.player.word_1312 = 0;
        data.player.scalar_1344 = 0.0;
        data.processed.scalar_2664 = 0.0;
        data.processed.word_2564 = 0;

        let previous = states.get_type(self.active);
        let previous_category = previous.category();
        let current_category = requested.category();
        data.processed.previous_state_2504 = previous as u32;
        data.processed.previous_category_2516 = previous_category;
        data.processed.requested_state_2500 = requested as u32;
        data.processed.current_state_2508 = requested as u32;
        data.processed.current_category_2512 = current_category;
        if current_category != previous_category {
            data.player.previous_category_latch_1336 = previous_category;
        }
        data.processed.previous_category_latch_2520 = data.player.previous_category_latch_1336;

        states.exit(StateCall {
            state: self.active,
            active: self.active,
        });
        self.active = StateBinding::new(requested);
        states.enter(StateCall {
            state: self.active,
            active: self.active,
        });
        Ok(self.active)
    }
}

fn prepare_skateboard_controller<C: SkateboardControllerActions>(
    requested: PhysicalStateId,
    controller: &mut SkateboardControllerFields,
    processed_flags_2480: u32,
    actions: &mut C,
) {
    if !matches!(
        requested,
        PhysicalStateId::BipedGround | PhysicalStateId::BipedAir | PhysicalStateId::OffBoardPushing
    ) {
        stop_skateboard_controller(controller, actions);
        return;
    }

    if controller.system_on_452 {
        return;
    }

    controller.word_444 = 0;
    if processed_flags_2480 & (0x80 | 0x100) != 0 {
        if controller.state_448 != 2 {
            actions.let_go_of_skateboard();
            controller.state_448 = 2;
        }
    } else if controller.state_448 != 1 {
        actions.hold_skateboard();
        controller.state_448 = 1;
    }
    controller.system_on_452 = true;
}

fn stop_skateboard_controller<C: SkateboardControllerActions>(
    controller: &mut SkateboardControllerFields,
    actions: &mut C,
) {
    if !controller.system_on_452 {
        return;
    }
    let previous_state = controller.state_448;
    controller.word_444 = 0;
    if previous_state != 0 {
        actions.let_go_of_skateboard();
        controller.state_448 = 0;
    }
    controller.system_on_452 = false;
}

