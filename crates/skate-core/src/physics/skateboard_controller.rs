use crate::player::lifecycle::{SkateboardControllerActions, SkateboardControllerFields};

pub struct SkateboardController {
    pub fields: SkateboardControllerFields,
}
impl Default for SkateboardController {
    fn default() -> Self {
        Self::new()
    }
}
impl SkateboardController {
    pub const fn new() -> Self {
        Self {
            fields: SkateboardControllerFields {
                word_444: 0,
                state_448: 0,
                system_on_452: false,
            },
        }
    }

    pub fn stop(&mut self, actions: &mut impl SkateboardControllerActions) {
        if !self.fields.system_on_452 {
            return;
        }
        let previous = self.fields.state_448;
        self.fields.word_444 = 0;
        if previous != 0 {
            actions.let_go_of_skateboard();
            self.fields.state_448 = 0;
        }
        self.fields.system_on_452 = false;
    }

    pub fn request_partial_ragdoll(&self, part_contact_count: u8) -> bool {
        self.fields.state_448 == 1 && part_contact_count != 0
    }
}
