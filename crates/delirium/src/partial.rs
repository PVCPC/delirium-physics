use crate::nm::Agent;
use crate::prelude::*;

#[allow(non_snake_case)]
#[inline]
pub fn STUMBLE_HOLD() -> f32 { delirium_data::kv!(126) }
#[allow(non_snake_case)]
#[inline]
pub fn STUMBLE_DIE() -> f32 { delirium_data::kv!(20) }
#[allow(non_snake_case)]
#[inline]
pub fn HARD_HIT() -> f32 { delirium_data::kv!(79) }
const PELVIS: usize = 0;
#[allow(non_snake_case)]
#[inline]
fn HEAD() -> usize { delirium_data::kv!(55 int) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Legs,
    Upper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Knocked,
    Stumbled,
    Fell,
    Recovered,
    Resumed,
    Dropped,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Off,
    On,
    Stumble { dying: bool },
    Down,
}

#[derive(Clone, Debug)]
pub struct Partial {
    pub mode: Mode,
    pub keep_running: bool,
    pub breaks: bool,
    state: State,
    t: f32,
    v: Vec3,
    last: Option<Vec3>,
}

impl Partial {
    pub fn new(mode: Mode) -> Self {
        Self { mode, keep_running: true, breaks: true, state: State::Off, t: 0.0, v: Vec3::ZERO, last: None }
    }

    pub fn animating(&self) -> bool {
        matches!(self.state, State::On | State::Stumble { .. })
    }

    pub fn active(&self) -> bool {
        self.state != State::Off
    }

    pub fn start(&mut self, a: &mut Agent) {
        self.release(a, false);
        let n = a.doll.frames().count();
        if self.mode == Mode::Legs {
            a.keyframe = (0..=delirium_data::kv!(6 int)).filter(|&k| k < n).collect();
        }
        a.hold.push(("ub".into(), if self.mode == Mode::Upper { delirium_data::kv!(27) } else { delirium_data::kv!(126) }, 1.0, 1.0));
        a.follow.push(("ub".into(), 1.0));
        (self.state, self.t, self.last) = (State::On, 0.0, None);
    }

    pub fn stop(&mut self, a: &mut Agent) {
        let was = self.animating();
        self.release(a, was);
        self.state = State::Off;
    }

    fn release(&mut self, a: &mut Agent, kick: bool) {
        for k in std::mem::take(&mut a.keyframe) {
            a.doll.unpin_part(k);
        }
        a.hold.retain(|h| h.0 != "ub" && h.0 != "lb");
        a.follow.retain(|f| f.0 != "ub" && f.0 != "lb");
        let v = std::mem::take(&mut self.v);
        if kick && v.length() > 0.5 {
            a.doll.kick((v - a.doll.vel(PELVIS).0).with_z(0.0));
        }
    }

    pub fn hit(&mut self, a: &mut Agent, ns: f32) -> Option<Event> {
        (self.breaks && ns >= HARD_HIT() && self.state == State::On).then(|| self.out(a, false))
    }

    pub fn die(&mut self, a: &mut Agent) -> Option<Event> {
        (self.active() && self.state != (State::Stumble { dying: true })).then(|| self.out(a, true))
    }

    fn out(&mut self, a: &mut Agent, dying: bool) -> Event {
        if self.mode != Mode::Legs || !self.keep_running || self.v.length() < 1.0 || self.state != State::On {
            if dying {
                self.stop(a);
                return Event::Dropped;
            }
            return self.down(a, Event::Knocked);
        }
        for k in std::mem::take(&mut a.keyframe) {
            a.doll.unpin_part(k);
        }
        let v = self.v;
        if v.length() > 0.5 {
            a.doll.kick((v - a.doll.vel(PELVIS).0).with_z(0.0));
        }
        a.hold.retain(|h| h.0 != "ub");
        a.follow.retain(|f| f.0 != "ub");
        a.hold.push(("lb".into(), STUMBLE_HOLD(), 1.0, 1.0));
        a.follow.push(("lb".into(), 1.0));
        (self.state, self.t) = (State::Stumble { dying }, 0.0);
        Event::Stumbled
    }

    fn down(&mut self, a: &mut Agent, e: Event) -> Event {
        let was = self.animating();
        self.release(a, was);
        (self.state, self.t) = (State::Down, 0.0);
        e
    }

    pub fn step(&mut self, a: &mut Agent, dt: f32, dead: bool) -> Option<Event> {
        if self.state == State::Off {
            return None;
        }
        self.t += dt;
        let t = self.t;
        if self.animating() {
            if let Some(p) = a.incoming.get(PELVIS).map(|f| f.translation) {
                if let Some(l) = self.last.filter(|_| dt > 0.0) {
                    self.v = (p - l) / dt;
                }
                self.last = Some(p);
            }
        }
        let f: Vec<Transform> = a.doll.frames().take(HEAD() + 1).collect();
        if f.len() <= HEAD() {
            return None;
        }
        let (pz, hz) = (f[PELVIS].translation.z, f[HEAD()].translation.z);
        let up = hz - pz > 0.5;
        match self.state {
            State::On => {
                let off = a.incoming.get(HEAD()).map_or(0.0, |b| f[HEAD()].translation.distance(b.translation));
                if self.breaks && t > 0.5 && (off > delirium_data::kv!(10) || self.mode == Mode::Upper && !a.balancing()) {
                    return Some(self.out(a, false));
                }
            }
            State::Stumble { dying: true } => {
                let k = (1.0 - t / STUMBLE_DIE()).max(0.0);
                a.hold.iter_mut().filter(|h| h.0 == "lb").for_each(|h| h.1 = STUMBLE_HOLD() * k);
                if t > STUMBLE_DIE() {
                    self.stop(a);
                    return Some(Event::Dropped);
                }
            }
            State::Stumble { dying: false } => {
                if pz < delirium_data::kv!(18) || hz - pz < delirium_data::kv!(140) || t > delirium_data::kv!(5) {
                    return Some(self.down(a, Event::Fell));
                }
                if t > delirium_data::kv!(20) && up && a.balancing() {
                    self.start(a);
                    return Some(Event::Recovered);
                }
            }
            State::Down => {
                if dead {
                    self.state = State::Off;
                } else if t > 1.0 && up && a.balancing() {
                    self.start(a);
                    return Some(Event::Resumed);
                }
            }
            State::Off => {}
        }
        None
    }
}
