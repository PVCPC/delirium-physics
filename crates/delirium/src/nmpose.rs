use crate::msg::Value;
use crate::shottask::Msg;
use delirium_data::rng::Rand;

pub const TYPE: u32 = 2105;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub clip: u32,
    pub table: u32,
    pub low: f32,
    pub high: f32,
    pub phase: f32,
    pub looped: bool,
    pub started: Option<u32>,
}

impl Pose {
    pub fn new(low: f32, high: f32, table: u32, clip: u32, looped: bool) -> Self {
        Pose { clip, table, low, high, phase: 0.0, looped, started: None }
    }

    pub fn update(&mut self, rand: &mut Rand, now: u32) -> Vec<Msg> {
        if self.started.is_some() {
            return vec![];
        }
        self.started = Some(now);
        vec![("stopAllBehaviours", vec![]), self.start(rand)]
    }

    fn start(&self, rand: &mut Rand) -> Msg {
        let k = rand.float_in_range(self.low, self.high);
        ("setStiffness", vec![("start", Value::Bool(true)), ("bodyStiffness", Value::Float(if k >= 0.0 { k } else { 4.0 }))])
    }
}

impl Pose {
    pub fn control(&mut self, step: f32, length: Option<f32>) -> Vec<Msg> {
        let Some(len) = length else { return vec![] };
        if self.looped {
            self.phase += step / len;
            if self.phase > 1.0 {
                self.phase -= 1.0;
            }
        }
        control_msgs()
    }
}

pub fn control_msgs() -> Vec<Msg> {
    vec![("activePose", vec![("start", Value::Bool(true))])]
}

