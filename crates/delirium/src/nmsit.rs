use crate::msg::Value;
use crate::prelude::Vec3;
use crate::shottask::Msg;

pub const TYPE: u32 = 2115;

#[derive(Clone, Copy, Debug)]
pub struct Entity {
    pub place: Vec3,
    pub head: Vec3,
    pub ped: Option<bool>,
}

pub fn look(point: Vec3, entity: Option<&Entity>) -> Vec3 {
    match entity {
        None => point,
        Some(e) => match e.ped {
            Some(true) => e.head,
            Some(false) => e.place + Vec3::new(0.0, 0.0, 0.7),
            None => e.place,
        },
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Sit {
    pub point: Vec3,
    pub seat: Vec3,
    pub clip: bool,
    pub started: Option<u32>,
}

impl Sit {
    pub fn new(point: Vec3, seat: Vec3, clip: bool) -> Self {
        Sit { point, seat, clip, started: None }
    }

    pub fn update(&mut self, now: u32, entity: Option<&Entity>) -> Vec<Msg> {
        let mut m = vec![];
        let start = self.started.is_none();
        if start {
            self.started = Some(now);
            m.push(("stopAllBehaviours", vec![]));
            if self.clip {
                m.push(("activePose", vec![("start", Value::Bool(true))]));
            }
            m.push(("catchFall", vec![("start", Value::Bool(true)), ("useHeadLook", Value::Bool(false))]));
        }
        let p = look(self.point, entity);
        if p != Vec3::ZERO {
            let mut ps = vec![];
            if start {
                ps.push(("start", Value::Bool(true)));
            }
            ps.push(("pos", Value::Vec3(p)));
            m.push(("headLook", ps));
        }
        m
    }

    pub fn d2(&self, place: Vec3) -> f32 {
        let d = place - self.seat;
        d.z.mul_add(d.z, d.y.mul_add(d.y, d.x * d.x))
    }
}

