use crate::prelude::*;
use crate::Fma;
use delirium_data::rng::Rand;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Building,
    Vehicle { player_driver: bool },
    Ped { player: bool, ragdoll: bool },
    Object,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thing<E> {
    pub id: E,
    pub kind: Kind,
}

impl<E> Thing<E> {
    fn playerish(&self) -> bool {
        matches!(self.kind, Kind::Vehicle { player_driver: true } | Kind::Ped { player: true, .. })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ped<E> {
    pub id: E,
    pub health: f32,
    pub inv_mass: f32,
    pub woke: bool,
    pub drop: Option<f32>,
    pub vz: f32,
    pub tough: bool,
    pub player_cut: bool,
    pub player: bool,
    pub mission: bool,
    pub quick_relax: bool,
    pub agility: u8,
    pub gun: bool,
    pub place: Vec3,
    pub vel: Vec3,
    pub standing_on: Option<E>,
}

#[derive(Clone, Copy, Debug)]
pub struct Impact<E> {
    pub impulse: f32,
    pub dt: f32,
    pub component: i32,
    pub other: Option<Thing<E>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Player<E> {
    pub thing: Thing<E>,
    pub place: Vec3,
    pub in_car: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Blend {
    pub state: u32,
    pub id: u32,
    pub group: u32,
    pub phase: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct Around<E> {
    pub touching: Vec<Thing<E>>,
    pub player: Option<Player<E>>,
    pub blend: Option<Blend>,
    pub sit: Option<[u32; 2]>,
    pub die: Option<u32>,
    pub accepts: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Task<E> {
    Relax { a: f32, b: f32 },
    RollUp { roll: i32 },
    Sit { a: u32, b: u32, with: E, at: Vec3 },
    Brace { with: E, weak: bool },
    Balance { with: Option<E>, point: Option<Vec3>, set: &'static str },
    Die(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wrap {
    ComplexNm(f32),
    Raw,
    Switch2Nm,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Made<E> {
    pub task: Task<E>,
    pub min: u16,
    pub max: u16,
    pub wrap: Option<Wrap>,
    pub say: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speech {
    Muted,
    NoLine,
    Said,
}

fn muted(health: f32, state: i32) -> bool {
    !(health > 0.0) && matches!(state, 1 | 2)
}

pub fn pain_line(health: f32, state: i32, pick: i32, has_line: bool, rand: &mut Rand) -> Speech {
    if muted(health, state) {
        return Speech::Muted;
    }
    if !(0..=2).contains(&pick) {
        rand.int_in_range(0, 3);
    }
    if has_line { Speech::Said } else { Speech::NoLine }
}

pub fn shocked_line(health: f32, state: i32, has_line: bool) -> Speech {
    if muted(health, state) {
        Speech::Muted
    } else if has_line {
        Speech::Said
    } else {
        Speech::NoLine
    }
}

pub fn damage<E: Copy>(ped: &Ped<E>, imp: &Impact<E>, rand: &mut Rand) -> Option<(f32, u32)> {
    let kind = imp.other.map(|o| o.kind);
    let car = matches!(kind, Some(Kind::Vehicle { .. }));
    if car && ped.woke {
        rand.next();
    }
    let cut = if ped.player_cut { 0.66 } else { 1.0 };
    let fall = if ped.tough { 15.0 } else { 10.0 };
    let amount = if ped.drop.is_some_and(|d| d > fall) && !(ped.vz < 0.0) {
        ped.health * 100.0
    } else {
        let k = match kind {
            Some(Kind::Ped { ragdoll: false, .. }) => None,
            Some(Kind::Ped { ragdoll: true, .. }) => Some(10.0),
            None | Some(Kind::Building) if matches!(imp.component, 2 | 3 | 5 | 6) => Some(7.0),
            _ => Some(2.5),
        };
        let f = k.map_or(0.0, |k| (-((9.8f64 * imp.dt as f64) as f32)).fma(k, ped.inv_mass * imp.impulse));
        cut * if f < 0.0 { 0.0 } else { f * if car { 6.0 } else { 3.0 } }
    };
    (amount > 0.0 || ped.woke).then_some((amount, if car { 50 } else { 54 }))
}

pub fn react<E: Copy + PartialEq>(ped: &Ped<E>, imp: &Impact<E>, around: &Around<E>, amount: f32, dying: bool, health: f32,
                                  rand: &mut Rand, probe: impl FnOnce() -> Option<Thing<E>>) -> Option<Made<E>> {
    if !around.accepts && !ped.woke {
        return None;
    }
    let made = |task, min, max, say| {
        let wrap = if !around.accepts { Wrap::Switch2Nm } else if dying { Wrap::Raw } else { Wrap::ComplexNm(amount) };
        Some(Made { task, min, max, wrap: Some(wrap), say })
    };
    let relax = Task::Relax { a: -1.0, b: 1.0 };
    if dying {
        if let Some(k) = around.die {
            return Some(Made { task: Task::Die(k), min: 0, max: 0, wrap: None, say: None });
        }
        if ped.quick_relax {
            return made(Task::Relax { a: 25.0, b: 0.75 }, 250, 500, None);
        }
        if imp.other.is_some_and(|o| o.playerish()) {
            return made(Task::RollUp { roll: rand.int_in_range(1000, 3000) }, 2000, 60000, None);
        }
        return made(relax, 2000, 60000, None);
    }
    if !ped.woke {
        return None;
    }
    let other = imp.other;
    let mut hard = other.is_some_and(|o| matches!(o.kind, Kind::Vehicle { .. } | Kind::Object) && Some(o.id) != ped.standing_on);
    let mut who = None;
    if other.is_some_and(|o| matches!(o.kind, Kind::Ped { player: true, .. })) || hard {
        who = other;
    } else if let Some(t) = around.touching.iter().find(|t| t.playerish() && matches!(t.kind, Kind::Ped { .. } | Kind::Vehicle { .. })) {
        (who, hard) = (Some(*t), matches!(t.kind, Kind::Vehicle { .. }));
    } else if let Some(pl) = around.player {
        let d2 = |a: Vec3, b: Vec3| {
            let d = b - a;
            d.x.fma(d.x, d.z.fma(d.z, d.y * d.y))
        };
        let d = d2(ped.place, pl.place);
        if pl.in_car && d < 25.0 {
            if let Some(t) = probe() {
                (who, hard) = (Some(t), true);
            }
        }
        if who.is_none() && pl.thing.id != ped.id && d < 0.64000005 {
            (who, hard) = (Some(pl.thing), false);
        }
    }
    let who = who.or(other);
    let weak = ped.agility < 3 || health < if ped.player || ped.mission { 110.0 } else { 140.0 };
    if let Some(b) = around.blend.filter(|b| b.state != 4) {
        let (go, roll) = match (b.phase, b.id, b.group) {
            (None, ..) => (true, false),
            (_, 98, _) => (false, false),
            (Some(p), _, 3 | 4) => (true, p >= 0.5),
            (Some(p), _, 5 | 6) => (if p < 0.5 { true } else { p < 0.8 && !who.is_some_and(|w| w.playerish()) }, p >= 0.15),
            _ => (true, false),
        };
        if go {
            return if roll {
                made(Task::RollUp { roll: rand.int_in_range(1000, 3000) }, 2000, 60000, Some(2))
            } else {
                made(relax, 2000, 60000, None)
            };
        }
    }
    if let (Some([a, b]), Some(w)) = (around.sit, who.filter(|w| matches!(w.kind, Kind::Ped { .. }))) {
        return made(Task::Sit { a, b, with: w.id, at: ped.place }, 2000, 60000, None);
    }
    if hard {
        let w = who.unwrap().id;
        return if imp.impulse > 400.0 {
            made(Task::RollUp { roll: rand.int_in_range(1000, 2000) }, 2000, 60000, None)
        } else {
            made(Task::Brace { with: w, weak }, 1000, 60000, None)
        };
    }
    let point = who.is_none().then(|| ped.place - Vec3::new(ped.vel.x, ped.vel.y, 0.0) * 3.0);
    let say = if ped.player {
        Some(1)
    } else if who.is_some_and(|w| matches!(w.kind, Kind::Ped { player: true, .. })) && rand.next() < 16383 {
        Some(if rand.next() < 16383 { 2 } else { 3 })
    } else {
        None
    };
    let set = crate::nm::balance_set(ped.agility, health, ped.player || ped.mission, ped.gun);
    made(Task::Balance { with: who.map(|w| w.id), point, set }, 2000, 60000, say)
}

#[derive(Clone, Copy, Debug)]
pub struct Gate {
    pub ragdoll: bool,
    pub flagged: bool,
    pub damages: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Contact<E> {
    pub other: Option<E>,
    pub other_ragdoll: bool,
    pub impulse: Vec3,
    pub normal: [f32; 4],
    pub point: [f32; 4],
    pub component: i32,
    pub extra: [u32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContactRecord<E> {
    pub other: Option<E>,
    pub strongest: f32,
    pub sum: f32,
    pub normal: [f32; 4],
    pub point: [f32; 4],
    pub component: i32,
    pub extra: [u32; 2],
    pub touched: Vec<E>,
    pub stamp: u32,
}

impl<E: Copy + PartialEq> ContactRecord<E> {
    pub fn new() -> Self {
        Self { other: None, strongest: 0.0, sum: 0.0, normal: [0.0; 4], point: [0.0; 4], component: 0, extra: [0; 2],
               touched: Vec::new(), stamp: 0 }
    }

    pub fn add(&mut self, gate: Gate, c: &Contact<E>, mass: f32, stamp: u32) -> Option<f32> {
        if !gate.ragdoll || (c.other.is_some() && !c.other_ragdoll) {
            return None;
        }
        let (x, y, z) = (c.impulse.x, c.impulse.y, c.impulse.z);
        let m2 = y.fma(y, z * z) + x * x;
        let mag = x.fma(x, y.fma(y, z * z)).sqrt();
        let old = self.strongest;
        if m2 > old * old || old == 0.0 {
            self.other = c.other;
            self.strongest = if 1e-6f32 - mag >= 0.0 { 1e-6 } else { mag };
            self.normal = c.normal;
            self.point = c.point;
            self.component = c.component;
            self.extra = c.extra;
            self.sum += self.strongest;
        } else if m2 > ((0.01f32 * 0.01) * mass) * mass {
            self.sum = mag + self.sum;
        }
        let other = c.other?;
        if gate.flagged {
            self.stamp = stamp;
            if !self.touched.contains(&other) && self.touched.len() < 6 {
                self.touched.push(other);
            }
        }
        gate.damages.then_some(mag)
    }

    pub fn consume(&mut self, state: u32, dt: f32, thing: impl FnOnce(E) -> Thing<E>) -> Option<Impact<E>> {
        if state <= 3 {
            return None;
        }
        let imp = if self.strongest > 0.0 && self.sum > 0.0 {
            Impact { impulse: self.sum, dt, component: self.component, other: self.other.map(thing) }
        } else if state == 4 {
            Impact { impulse: 0.0, dt, component: 0, other: None }
        } else {
            return None;
        };
        self.sum = 0.0;
        Some(imp)
    }
}

