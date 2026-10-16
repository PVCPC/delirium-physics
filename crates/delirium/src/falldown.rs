use crate::msg::Value;
use crate::nm::Next;
use crate::prelude::*;
use crate::shottask::Msg;
use crate::Fma;

#[derive(Clone, Debug, Default)]
pub struct Ped {
    pub spine: [Vec3; 4],
    pub b6: Vec3,
    pub b10: Vec3,
    pub b12: Vec3,
    pub pos: Vec3,
    pub mass: f32,
    pub vel: Vec3,
    pub health: f32,
    pub dead: bool,
    pub deep: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Push {
    pub part: usize,
    pub v: Vec3,
    pub at: Vec3,
    pub impulse: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Step {
    pub msgs: Vec<Msg>,
    pub pushes: Vec<Push>,
    pub done: bool,
    pub next: Option<Next>,
    pub health_one: bool,
    pub sink: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct FallDown {
    pub state: u32,
    pub dir: Vec3,
    pub(crate) height: f32,
    pub stage: u32,
    pub(crate) spine_z: f32,
    pushed: u32,
    pub started: Option<u32>,
}

fn facing(p: &Ped, d: Vec3) -> f32 {
    let c = p.spine[2];
    let n = crate::rigid::nrm_or_y(Vec3::new(0.0 - c.x, 0.0 - c.y, 0.0));
    (n.x.fma(d.x, d.y.fma(n.y, d.z * n.z)) + 1.0) * 0.5
}

impl FallDown {
    pub fn new(state: u32, dir: Vec3, height: f32) -> Self {
        FallDown { state, dir, height, stage: 0, spine_z: 0.0, pushed: 0, started: None }
    }

    pub fn feedback(&mut self, name: &str, now: u32) {
        if name.starts_with("balance") {
            self.pushed = now;
        }
    }

    pub fn update(&mut self, p: &Ped, now: u32) -> Step {
        let Some(t0) = self.started else {
            self.started = Some(now);
            let mut s = Step { msgs: vec![("stopAllBehaviours", vec![])], ..Default::default() };
            self.start(p, &mut s);
            return s;
        };
        let mut s = Step::default();
        self.control(p, now, t0, &mut s);
        (s.done, s.next) = finish(p, now, t0, if matches!(self.stage, 4 | 5) { 9 } else { 8 });
        s.health_one |= s.done && self.state > 3;
        s
    }

    fn start(&mut self, p: &Ped, s: &mut Step) {
        use Value::{Bool as B, Float as F, Int as I, Vec3 as V};
        let (roll, wall) = (matches!(self.state, 2 | 6), matches!(self.state, 1 | 5));
        let f = facing(p, self.dir);
        s.msgs.push(("bodyBalance", vec![("start", B(true)), ("maxSteps", I(if roll { 5 } else { 10 }))]));
        s.msgs.push(("configureBalance", vec![("start", B(true)), ("balanceAbortThreshold", F(if roll { 0.7 } else { 0.55 })),
                                              ("stepClampScale", F(0.6))]));
        let lean = (if wall { 0.25f32 } else { 0.45 }).fma(f, (1.0 - f) * 0.25);
        s.msgs.push(("leanInDirection", vec![("start", B(true)), ("dir", V(self.dir)), ("leanAmount", F(lean)), ("applyAsForce", B(false))]));
        if wall {
            s.msgs.push(("fallOverWall", vec![("start", B(true)), ("magOfForce", F(0.15)), ("maxDistanceFromPelToHitPoint", F(0.25))]));
        }
        let k = p.mass * (((1.0 - f) + f) * 0.6);
        s.pushes.push(Push { part: 10, v: self.dir * k, at: p.b10 - p.pos, impulse: true });
        (self.spine_z, self.pushed, self.stage) = (p.spine[3].z, 0, 1);
    }

    fn control(&mut self, p: &Ped, now: u32, t0: u32, s: &mut Step) {
        use Value::{Bool as B, Float as F, Vec3 as V};
        let ledge = self.stage == 4 && p.pos.z < self.height + 1.0;
        if self.stage == 2 || ledge {
            s.msgs.push(("bodyRelax", vec![("start", B(true)), ("relaxation", F(70.0))]));
            s.health_one = self.state > 3;
            self.stage = 5;
        } else if self.stage == 1 {
            let f = facing(p, self.dir);
            let dropped = !(p.spine[3].z >= self.spine_z - 0.3);
            let go = match self.state {
                0 | 4 => self.pushed > 0 || dropped,
                1 | 5 => dropped || self.pushed != 0 && now > self.pushed.wrapping_add(3000),
                2 | 6 => self.pushed > 0,
                _ => false,
            } || now > t0.wrapping_add(5000);
            let d = self.dir;
            if go {
                self.pushed = now;
                s.msgs.push(("stopAllBehaviours", vec![]));
                if matches!(self.state, 2 | 6) {
                    s.msgs.push(("rollDownStairs", vec![("start", B(true)), ("customRollDir", V(d)), ("useCustomRollDir", B(true))]));
                } else {
                    s.msgs.push(("catchFall", vec![("start", B(true)), ("armsStiffness", F(10.0)), ("torsoStiffness", F(7.0))]));
                    s.msgs.push(("pedalLegs", vec![("start", B(true)), ("legStiffness", F(7.0))]));
                    let v = d * (p.mass * (1.0 - f).fma(0.2, f * 0.6));
                    s.pushes.push(Push { part: 12, v, at: p.b12 - p.pos, impulse: true });
                    s.pushes.push(Push { part: 6, v: Vec3::ZERO - v, at: p.b6 - p.pos, impulse: true });
                }
                self.stage = 4;
            } else {
                s.pushes.push(Push { part: 10, v: d * (p.mass * (1.0 - f).fma(0.3, f * 0.9)), at: p.b10 - p.pos, impulse: false });
            }
        }
        s.sink = self.state > 3 && self.stage < 5;
    }
}

fn finish(p: &Ped, now: u32, t0: u32, flags: u32) -> (bool, Option<Next>) {
    let v = p.vel;
    let v2 = v.x.fma(v.x, v.y.fma(v.y, v.z * v.z));
    if flags & 1 != 0 && now > t0.wrapping_add(2000) && v2 < 0.2f32 * 0.2 {
        return (true, Some(if p.health < 110.0 && flags & 8 != 0 { Next::Relax } else { Next::Blend }));
    }
    let max = if v2 < 4.0 || p.pos.z < 0.0 { 10000 } else { 100000 };
    if now > t0.wrapping_add(max) {
        return (true, Some(Next::Blend));
    }
    if p.dead && !p.deep && flags & 2 == 0 {
        return (true, None);
    }
    (flags & 16 != 0 && p.deep, None)
}

