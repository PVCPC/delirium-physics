use bevy::prelude::*;
pub use delirium::jump::Jump;

use crate::Sim;

impl Sim {
    pub(crate) fn jump_start(&mut self, dive: bool, toward: Option<Vec3>) {
        self.jump = Jump::start(&mut self.agent, dive, toward);
        self.say(if self.jump.is_none() { "jump  Homer must be balancing".into() } else { format!("jump  {}", if dive { "dive" } else { "up" }) });
    }

    pub(crate) fn jump_frame(&mut self, dt: f32) {
        let Some(mut j) = self.jump.take() else { return };
        let mut said = delirium::jump::Said::default();
        let on = j.frame(&mut self.agent, dt, &mut said);
        if let Some(l) = said.log {
            self.say(l);
        }
        if said.catch_fall {
            self.send_to(0, "catchFall start=true");
        }
        if on {
            self.jump = Some(j);
        }
    }
}
