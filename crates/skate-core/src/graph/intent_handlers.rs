use crate::input::graph_intents::IntentMutation;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstMgIntent {
    pub value: f32,
    pub on_update: bool,
    created: bool,
}

impl ConstMgIntent {
    pub const fn new(value: f32, on_update: bool) -> Self {
        Self {
            value,
            on_update,
            created: false,
        }
    }

    pub fn begin(&mut self) -> IntentMutation {
        self.created = true;
        IntentMutation::Set(self.value)
    }

    pub fn update(&mut self) -> IntentMutation {
        let mutation = if self.on_update || self.created {
            IntentMutation::None
        } else {
            IntentMutation::Remove
        };
        self.created = false;
        mutation
    }

    pub const fn end(&self) -> IntentMutation {
        IntentMutation::Remove
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TimeMgIntent {
    pub elapsed: f32,
}

impl TimeMgIntent {
    pub fn update(&mut self, action_value: Option<f32>, dt: f32) -> IntentMutation {
        match action_value {
            Some(_) => {
                self.elapsed += dt;
                IntentMutation::Set(self.elapsed)
            }
            None => {
                self.elapsed = 0.0;
                IntentMutation::Remove
            }
        }
    }

    pub const fn end(&self) -> IntentMutation {
        IntentMutation::Remove
    }
}

