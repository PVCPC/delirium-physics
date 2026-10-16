use crate::explosion::Set;
use crate::msg::Value;
use crate::shottask::Msg;
use delirium_data::rng::Rand;

#[derive(Clone, Copy, Debug)]
pub struct RollUpAndRelax {
    pub stiffness: f32,
    pub damping: f32,
    pub roll: u32,
    pub relaxed: bool,
    pub started: Option<u32>,
}

fn unit(rand: &mut Rand) -> f32 {
    rand.next() as f32 * f32::from_bits(0x3800_0100)
}

impl RollUpAndRelax {
    pub fn new(stiffness: f32, damping: f32, roll: u32) -> Self {
        RollUpAndRelax { stiffness, damping, roll, relaxed: false, started: None }
    }

    pub fn update(&mut self, set: &Set, rand: &mut Rand, now: u32) -> Vec<Msg> {
        let Some(t0) = self.started else {
            self.started = Some(now);
            return vec![("stopAllBehaviours", vec![]), self.start(set, rand)];
        };
        self.control(now, t0)
    }

    fn start(&mut self, set: &Set, rand: &mut Rand) -> Msg {
        use Value::{Bool as B, Float as F};
        let stiffness = unit(rand).mul_add(2.0, 10.0);
        ("bodyRollUp", vec![("start", B(true)), ("mask", Value::Str("fb".into())), ("stiffness", F(stiffness)), ("legPush", F(set.f[23])),
                            ("armReachAmount", F(set.f[24])), ("useArmToSlowDown", F(set.f[25])),
                            ("asymmetricalLegs", F(unit(rand).mul_add(0.3, 0.1)))])
    }

    fn control(&mut self, now: u32, t0: u32) -> Vec<Msg> {
        if self.relaxed {
            return crate::nmrelax::msgs(self.stiffness, self.damping, false);
        }
        if now <= self.roll.wrapping_add(t0) {
            return vec![];
        }
        self.relaxed = true;
        let mut m = vec![("bodyRollUp", vec![("start", Value::Bool(false))])];
        m.extend(crate::nmrelax::msgs(self.stiffness, self.damping, true));
        m
    }
}

