pub const MAX: usize = 16;
pub const NO_RESPONSE: i32 = 200;
pub const DAMAGE: u32 = 9;
pub const SCRIPT_COMMAND: u32 = 32;
pub const COMMUNICATE: u32 = 98;
pub const GIVE_PED_TASK: u32 = 124;
pub const MELEE_ACTION: u32 = 125;
pub const GUNSHOT_WAIT: f32 = 0.3;

#[derive(Clone, Debug)]
pub struct Ev<T> {
    pub kind: u32,
    pub priority: i32,
    pub lifetime: i32,
    pub age: i32,
    pub keep: bool,
    pub data: T,
}

impl<T> Ev<T> {
    pub fn new(kind: u32, priority: i32, data: T) -> Ev<T> {
        Ev { kind, priority, lifetime: 0, age: 0, keep: false, data }
    }

    pub fn still_valid(&self) -> bool {
        self.keep || self.age <= self.lifetime
    }
}

pub trait Asks<T> {
    fn ready(&mut self, e: &mut Ev<T>) -> bool;
    fn valid(&mut self, e: &Ev<T>) -> bool;
    fn concerns(&mut self, e: &Ev<T>) -> bool;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Admit {
    pub ped: bool,
    pub refused: bool,
    pub decision: bool,
    pub trials: [bool; 2],
    pub response: i32,
    pub byte_c: bool,
    pub bit38: bool,
    pub blocking: bool,
    pub temporary: bool,
    pub concerns: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Added {
    pub stored: bool,
    pub group: bool,
    pub friends: bool,
    pub vehicle: bool,
    pub heard: bool,
}

#[derive(Clone, Debug)]
pub struct Records<R> {
    pub note: u32,
    pub nontemp: Option<R>,
    pub temp: Option<R>,
    pub pending: Option<R>,
    pub other: Option<R>,
}

impl<R> Default for Records<R> {
    fn default() -> Self {
        Records { note: 0, nontemp: None, temp: None, pending: None, other: None }
    }
}

pub trait Record {
    fn kind(&self) -> u32;
    fn stacks(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Tree {
    pub slots: [u32; 5],
    pub secondary: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Reply {
    pub main: bool,
    pub secondary: bool,
}

pub trait Handler<T, R>: Asks<T> {
    fn overrides(&mut self, e: &Ev<T>, r: &R) -> bool;
    fn temporary(&mut self, e: &Ev<T>) -> bool;
    fn respond(&mut self, e: &Ev<T>, slot2: &mut bool) -> Reply;
    fn other(&mut self) -> bool;
    fn other_tree(&mut self) -> u32;
    fn ask(&mut self, task: u32, urgency: u32, e: &mut Ev<T>) -> bool;
    fn ragdoll(&mut self, e: &Ev<T>) -> bool;
    fn other_stop(&mut self);
    fn note(&mut self, recs: &mut Records<R>, e: &Ev<T>, slot2: bool);
    fn note_other(&mut self, recs: &mut Records<R>, e: &Ev<T>);
    fn give(&mut self, e: &Ev<T>);
    fn clear(&mut self);
}

#[derive(Clone, Debug)]
pub struct Queue<T> {
    pub evs: Vec<Ev<T>>,
}

impl<T> Default for Queue<T> {
    fn default() -> Self {
        Queue { evs: Vec::new() }
    }
}

impl<T> Queue<T> {
    pub fn add(&mut self, mut e: Ev<T>, keep: bool, replace: bool, a: &Admit) -> Added {
        let mut o = Added::default();
        if a.ped {
            if a.refused {
                return o;
            }
            let (mut store, mut pass) = (true, true);
            if a.decision && a.response == NO_RESPONSE && !(matches!(e.kind, DAMAGE | MELEE_ACTION) && a.byte_c) {
                store = false;
            }
            if a.blocking && !a.temporary && e.priority < 0x47 && !matches!(e.kind, SCRIPT_COMMAND | 54 | 55) {
                (store, pass) = (false, false);
            }
            if !a.concerns {
                (store, pass) = (false, false);
            }
            if pass {
                if a.decision {
                    o.group = a.trials[0];
                    o.friends = a.trials[1] && !(e.kind == DAMAGE && a.bit38);
                    o.vehicle = true;
                }
                o.heard = true;
            }
            if !store {
                return o;
            }
        }
        e.keep = keep;
        if self.evs.len() < MAX {
            if e.kind == GIVE_PED_TASK {
                self.evs.retain(|x| x.kind != GIVE_PED_TASK);
            }
        } else {
            let Some(i) = self.lowest().filter(|_| replace && a.ped) else { return o };
            if self.evs[i].priority >= e.priority {
                return o;
            }
            self.evs.remove(i);
        }
        self.evs.push(e);
        o.stored = true;
        o
    }

    pub fn lowest(&self) -> Option<usize> {
        let mut best = (None, 0x69);
        for (i, e) in self.evs.iter().enumerate() {
            if e.kind != SCRIPT_COMMAND && e.priority <= best.1 {
                best = (Some(i), e.priority);
            }
        }
        best.0
    }

    pub fn pick(&mut self, v: &mut impl Asks<T>) -> Option<usize> {
        let (mut best, mut at) = (None, -1);
        for (i, e) in self.evs.iter_mut().enumerate() {
            let p = e.priority;
            if e.kind == SCRIPT_COMMAND {
                if p <= at {
                    continue;
                }
            } else if p < at {
                v.ready(e);
                continue;
            }
            if v.ready(e) && v.valid(e) && v.concerns(e) {
                (best, at) = (Some(i), p);
            }
        }
        best
    }

    pub fn age(&mut self) {
        self.evs.iter_mut().for_each(|e| e.age = e.age.wrapping_add(1));
    }

    pub fn sweep(&mut self, v: &mut impl Asks<T>) {
        self.evs.retain(|e| v.valid(e));
    }

    pub fn drop_communicate(&mut self, v: &mut impl Asks<T>) {
        self.evs.retain_mut(|e| !(e.kind == COMMUNICATE && v.ready(e)));
    }

    pub fn handle<R: Record, H: Handler<T, R>>(&mut self, recs: &mut Records<R>, tree: &Tree, slot2: &mut bool, h: &mut H) {
        if recs.pending.is_some() && recs.temp.is_none() {
            recs.nontemp = recs.pending.take();
        }
        let note0 = recs.note;
        let first = |s: &[u32]| s.iter().copied().find(|&t| t != 0).unwrap_or(0);
        let root = first(&tree.slots[..3]);
        let nontemp_slot = if tree.slots[2] != 0 { tree.slots[2] } else { tree.slots[0] };
        self.drop_communicate(h);
        let picked = self.pick(h);
        self.age();
        recs.note = if note0 == root { note0 } else { 0 };
        if tree.slots[1] == 0 {
            recs.temp = None;
        }
        if nontemp_slot == 0 {
            recs.nontemp = None;
        }
        if tree.secondary == 0 {
            recs.other = None;
        }
        let drop_records = |q: &mut Self, recs: &mut Records<R>, h: &mut H| {
            if root == 0 {
                recs.other = None;
            }
            q.sweep(h);
        };
        let Some(i) = picked else { return drop_records(self, recs, h) };
        if let Some(r) = recs.temp.as_ref().or(recs.nontemp.as_ref()).or(recs.other.as_ref()) {
            if !r.stacks() && r.kind() == self.evs[i].kind {
                self.evs.remove(i);
                return drop_records(self, recs, h);
            }
        }
        if may_override(recs, &self.evs[i], *slot2, h) {
            let reply = h.respond(&self.evs[i], slot2);
            let mut ok = true;
            let top = first(&tree.slots);
            let built = reply.main || reply.secondary;
            if built {
                if top != 0 && !h.ask(top, 1, &mut self.evs[i]) {
                    ok = h.ragdoll(&self.evs[i]) && h.ask(top, 2, &mut self.evs[i]);
                }
            } else if h.other() {
                let t = h.other_tree();
                if t != 0 && !h.ask(t, 1, &mut self.evs[i]) {
                    h.other_stop();
                    ok = false;
                }
            }
            let accept = if root == 0 { note0 != 0 || ok } else { ok };
            if accept {
                recs.note = 0;
                if reply.main {
                    h.note(recs, &self.evs[i], *slot2);
                } else if h.other() {
                    h.note_other(recs, &self.evs[i]);
                }
                h.give(&self.evs[i]);
                self.evs.remove(i);
            } else if root == 0 {
                self.evs[i].age = self.evs[i].age.wrapping_sub(1);
                clear(h, slot2);
            } else if built {
                clear(h, slot2);
                self.evs[i].age = self.evs[i].age.wrapping_sub(1);
                h.ask(top, 0, &mut self.evs[i]);
                recs.note = root;
            }
        }
        clear(h, slot2);
        self.sweep(h);
    }
}

fn clear<T, R, H: Handler<T, R>>(h: &mut H, slot2: &mut bool) {
    h.clear();
    *slot2 = false;
}

fn may_override<T, R, H: Handler<T, R>>(recs: &Records<R>, e: &Ev<T>, slot2: bool, h: &mut H) -> bool {
    if let Some(r) = &recs.nontemp {
        return h.overrides(e, r);
    }
    let Some(t) = &recs.temp else { return true };
    if h.temporary(e) && !slot2 {
        return h.overrides(e, t);
    }
    h.overrides(e, t) && recs.pending.as_ref().is_none_or(|p| h.overrides(e, p))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wait {
    pub first: bool,
    pub ready: bool,
    pub aged: bool,
    pub left: f32,
}

impl Default for Wait {
    fn default() -> Wait {
        Wait { first: true, ready: false, aged: false, left: 0.0 }
    }
}

impl Wait {
    pub fn ready(&mut self, age: &mut i32, responding: bool, dt: f32, draw: impl FnOnce() -> f32) -> bool {
        if responding {
            (self.ready, self.aged) = (true, false);
            return true;
        }
        if self.ready {
            self.aged = false;
            return true;
        }
        if self.first {
            (self.left, self.first) = (draw(), false);
        }
        self.left -= dt;
        self.ready = !(self.left > 0.0);
        if self.ready && !self.aged {
            (*age, self.aged) = (0, true);
        }
        self.ready
    }

    pub fn valid<T>(&self, e: &Ev<T>) -> bool {
        !(self.ready && !self.aged) || e.still_valid()
    }
}
