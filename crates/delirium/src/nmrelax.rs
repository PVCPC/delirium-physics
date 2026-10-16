use crate::msg::Value;
use crate::shottask::Msg;

pub fn relaxation(stiffness: f32) -> f32 {
    if stiffness >= 0.0 { (100.0 - stiffness).max(10.0) } else { 60.0 }
}

pub fn damping(damping: f32) -> Option<f32> {
    (damping >= 0.0).then_some(damping)
}

pub fn msgs(stiffness: f32, damp: f32, start: bool) -> Vec<Msg> {
    let mut p = vec![];
    if start {
        p.push(("start", Value::Bool(true)));
    }
    p.push(("holdPose", Value::Bool(true)));
    p.push(("relaxation", Value::Float(relaxation(stiffness))));
    if let Some(d) = damping(damp) {
        p.push(("damping", Value::Float(d)));
    }
    vec![("bodyRelax", p)]
}

#[derive(Clone, Copy, Debug)]
pub struct Relax {
    pub stiffness: f32,
    pub damping: f32,
    pub started: Option<u32>,
}

impl Relax {
    pub fn new(stiffness: f32, damping: f32) -> Self {
        Relax { stiffness, damping, started: None }
    }

    pub fn update(&mut self, now: u32) -> Vec<Msg> {
        let first = self.started.is_none();
        self.started.get_or_insert(now);
        let mut m = if first { vec![("stopAllBehaviours", vec![])] } else { vec![] };
        m.extend(msgs(self.stiffness, self.damping, first));
        m
    }
}

