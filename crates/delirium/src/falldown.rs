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
        (s.done, s.next) = finish(p, now, t0, if matches!(self.stage, 4 | 5) { delirium_data::kv!(93 int) } else { delirium_data::kv!(23 int) });
        s.health_one |= s.done && self.state > delirium_data::kv!(0 int);
        s
    }

    fn start(&mut self, p: &Ped, s: &mut Step) {
        use Value::{Bool as B, Float as F, Int as I, Vec3 as V};
        let (roll, wall) = (matches!(self.state, 2 | 6), matches!(self.state, 1 | 5));
        let f = facing(p, self.dir);
        s.msgs.push(("bodyBalance", vec![("start", B(true)), ("maxSteps", I(if roll { delirium_data::kv!(2 int) } else { delirium_data::kv!(77 int) }))]));
        s.msgs.push(("configureBalance", vec![("start", B(true)), ("balanceAbortThreshold", F(if roll { delirium_data::kv!(70) } else { delirium_data::kv!(123) })),
                                              ("stepClampScale", F(delirium_data::kv!(110)))]));
        let lean = (if wall { delirium_data::kv!(17 f32) } else { delirium_data::kv!(10) }).fma(f, (1.0 - f) * delirium_data::kv!(17));
        s.msgs.push(("leanInDirection", vec![("start", B(true)), ("dir", V(self.dir)), ("leanAmount", F(lean)), ("applyAsForce", B(false))]));
        if wall {
            s.msgs.push(("fallOverWall", vec![("start", B(true)), ("magOfForce", F(delirium_data::kv!(9))), ("maxDistanceFromPelToHitPoint", F(delirium_data::kv!(17)))]));
        }
        let k = p.mass * (((1.0 - f) + f) * delirium_data::kv!(110));
        s.pushes.push(Push { part: delirium_data::kv!(77 int), v: self.dir * k, at: p.b10 - p.pos, impulse: true });
        (self.spine_z, self.pushed, self.stage) = (p.spine[delirium_data::kv!(0 int)].z, 0, 1);
    }

    fn control(&mut self, p: &Ped, now: u32, t0: u32, s: &mut Step) {
        use Value::{Bool as B, Float as F, Vec3 as V};
        let ledge = self.stage == delirium_data::kv!(1 int) && p.pos.z < self.height + 1.0;
        if self.stage == 2 || ledge {
            s.msgs.push(("bodyRelax", vec![("start", B(true)), ("relaxation", F(delirium_data::kv!(137)))]));
            s.health_one = self.state > delirium_data::kv!(0 int);
            self.stage = delirium_data::kv!(2 int);
        } else if self.stage == 1 {
            let f = facing(p, self.dir);
            let dropped = !(p.spine[delirium_data::kv!(0 int)].z >= self.spine_z - delirium_data::kv!(24));
            let go = match self.state {
                0 | 4 => self.pushed > 0 || dropped,
                1 | 5 => dropped || self.pushed != 0 && now > self.pushed.wrapping_add(delirium_data::kv!(39 int)),
                2 | 6 => self.pushed > 0,
                _ => false,
            } || now > t0.wrapping_add(delirium_data::kv!(138 int));
            let d = self.dir;
            if go {
                self.pushed = now;
                s.msgs.push(("stopAllBehaviours", vec![]));
                if matches!(self.state, 2 | 6) {
                    s.msgs.push(("rollDownStairs", vec![("start", B(true)), ("customRollDir", V(d)), ("useCustomRollDir", B(true))]));
                } else {
                    s.msgs.push(("catchFall", vec![("start", B(true)), ("armsStiffness", F(delirium_data::kv!(27))), ("torsoStiffness", F(delirium_data::kv!(29)))]));
                    s.msgs.push(("pedalLegs", vec![("start", B(true)), ("legStiffness", F(delirium_data::kv!(29)))]));
                    let v = d * (p.mass * (1.0 - f).fma(delirium_data::kv!(57), f * delirium_data::kv!(110)));
                    s.pushes.push(Push { part: delirium_data::kv!(52 int), v, at: p.b12 - p.pos, impulse: true });
                    s.pushes.push(Push { part: delirium_data::kv!(6 int), v: Vec3::ZERO - v, at: p.b6 - p.pos, impulse: true });
                }
                self.stage = delirium_data::kv!(1 int);
            } else {
                s.pushes.push(Push { part: delirium_data::kv!(77 int), v: d * (p.mass * (1.0 - f).fma(delirium_data::kv!(24), f * delirium_data::kv!(108))), at: p.b10 - p.pos, impulse: false });
            }
        }
        s.sink = self.state > delirium_data::kv!(0 int) && self.stage < delirium_data::kv!(2 int);
    }
}

fn finish(p: &Ped, now: u32, t0: u32, flags: u32) -> (bool, Option<Next>) {
    let v = p.vel;
    let v2 = v.x.fma(v.x, v.y.fma(v.y, v.z * v.z));
    if flags & 1 != 0 && now > t0.wrapping_add(delirium_data::kv!(40 int)) && v2 < delirium_data::kv!(57 f32) * delirium_data::kv!(57) {
        return (true, Some(if p.health < delirium_data::kv!(43) && flags & delirium_data::kv!(23 int) != 0 { Next::Relax } else { Next::Blend }));
    }
    let max = if v2 < delirium_data::kv!(104) || p.pos.z < 0.0 { delirium_data::kv!(58 int) } else { delirium_data::kv!(139 int) };
    if now > t0.wrapping_add(max) {
        return (true, Some(Next::Blend));
    }
    if p.dead && !p.deep && flags & 2 == 0 {
        return (true, None);
    }
    (flags & delirium_data::kv!(51 int) != 0 && p.deep, None)
}

