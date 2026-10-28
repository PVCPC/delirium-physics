use crate::carscan;
use crate::ped::Outcome;
use crate::tasks::{kind as task, Manager, NewTask, Task, ABORT_IMMEDIATE, ABORT_LEAVE, ABORT_URGENT, EVENT_RESPONSE_NONTEMP, EVENT_RESPONSE_TEMP,
                   PHYSICAL_RESPONSE, PRIMARY};

#[path = "eventpass.rs"]
mod pass;

pub mod kind {
    pub const DAMAGE: u32 = 9;
    pub const DEATH: u32 = 10;
    pub const SCRIPT_COMMAND: u32 = 32;
    pub const STATIC_COUNT_REACHED_MAX: u32 = 121;
    pub const GIVE_PED_TASK: u32 = 124;
    pub const ON_FIRE: u32 = 58;
    pub const IN_WATER: u32 = 66;
    pub const GUN_SHOT: u32 = 15;
    pub const BULLET_IMPACT: u32 = 116;
    pub const GUN_AIMED_AT: u32 = 31;
    pub const DRAGGED_OUT_CAR: u32 = 7;
    pub const DISLIKE: u32 = 37;
    pub const GUN_SHOT_WHIZZED_BY: u32 = 49;
    pub const SWITCH2NM: u32 = 130;
    pub const POTENTIAL_GET_RUN_OVER: u32 = 12;
    pub const IMMINENT_DANGER: u32 = 139;
    pub const STUCK_IN_AIR: u32 = 93;
    pub const POTENTIAL_BE_WALKED_INTO: u32 = 123;
    pub const MELEE_ACTION: u32 = 125;
}

pub const MELEE_REACH: f32 = 2.0;

pub const RESPONSE_FIGHT: i32 = 1900;

pub const RESPONSE_DODGE: i32 = 603;

pub const MAX: usize = 16;
pub const NO_RESPONSE: i32 = 200;
pub const APPLIED: u8 = 0x80;
pub const LANDED: u8 = 0x40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: u32,
    pub player: bool,
}

pub struct Damage<P> {
    pub real: bool,
    pub source: Option<Source>,
    pub outcome: Outcome,
    pub task: Option<Box<dyn Task<P>>>,
    pub nm: bool,
    pub dir: [f32; 3],
}

impl<P> Clone for Damage<P> {
    fn clone(&self) -> Self {
        Damage { real: self.real, source: self.source, outcome: self.outcome, task: self.task.clone(), nm: self.nm, dir: self.dir }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Whizzed {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub source: Option<Source>,
}

pub const WHIZZED_REACH: f32 = 2.0;
pub const IMPACT_REACH: f32 = 10.0;

pub fn sees(root: [f32; 3], forward: [f32; 3], at: [f32; 3], range: f32) -> bool {
    let d: [f32; 3] = std::array::from_fn(|l| at[l] - root[l]);
    let len = d.iter().map(|x| x * x).sum::<f32>().sqrt();
    let m = range.min(12.0);
    let least = if len < m { -(1.0 - len / m) } else { 0.0 };
    len < range && (0..3).map(|l| forward[l] * d[l] / len).sum::<f32>() > least
}

impl Whizzed {
    pub fn passes(&self, root: [f32; 3]) -> bool {
        crate::vecunit::whizzes(self.start, self.end, root)
    }
}

pub struct Switch<P> {
    pub task: Box<dyn Task<P>>,
    pub max_ms: u32,
    pub flag: bool,
}

impl<P> Clone for Switch<P> {
    fn clone(&self) -> Self {
        Switch { task: self.task.clone(), max_ms: self.max_ms, flag: self.flag }
    }
}

pub struct Script<P> {
    pub priority: usize,
    pub task: Option<Box<dyn Task<P>>>,
    pub urgent: bool,
}

impl<P> Clone for Script<P> {
    fn clone(&self) -> Self {
        Script { priority: self.priority, task: self.task.clone(), urgent: self.urgent }
    }
}

pub struct Event<P> {
    pub kind: u32,
    pub age: u32,
    pub keep: bool,
    pub response: i32,
    pub damage: Option<Damage<P>>,
    pub whizzed: Option<Whizzed>,
    pub switch: Option<Switch<P>>,
    pub script: Option<Script<P>>,
    pub bump: Option<u32>,
    pub tag: u64,
    pub run_over: Option<carscan::Vehicle>,
    pub stuck: Option<i32>,
    pub wait: Wait,
    pub danger: Option<Danger>,
    pub walked_into: Option<WalkedInto>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Danger {
    pub place: [f32; 3],
    pub reach: f32,
    pub dir: [f32; 3],
    pub kind: i32,
    pub agility: i32,
    pub quiet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkedInto {
    pub pos: [f32; 3],
    pub velocity: [f32; 3],
    pub forward: [f32; 3],
    pub state: u32,
}

pub fn walks_into(pos: [f32; 3], velocity: [f32; 3], forward: [f32; 3], other: [f32; 3]) -> bool {
    let ahead: [f32; 3] = std::array::from_fn(|l| velocity[l].mul_add(0.5, pos[l]));
    let (dx, dy) = (pos[0] - ahead[0], pos[1] - ahead[1]);
    let off: [f32; 3] = std::array::from_fn(|l| other[l] - pos[l]);
    let along = forward[0].mul_add(off[0], forward[1].mul_add(off[1], forward[2] * off[2]));
    let (_, near) = crate::portalgeom::closest_on_segment(pos, ahead, other);
    let d2: f32 = (0..3).map(|l| (near[l] - other[l]) * (near[l] - other[l])).sum();
    dx.mul_add(dx, dy * dy) > 0.1 && along > 0.25 && d2 <= 1.0
}

#[derive(Clone, Copy, Debug)]
pub struct Wait {
    first: bool,
    ready: bool,
    aged: bool,
    left: f32,
}

impl Default for Wait {
    fn default() -> Wait {
        Wait { first: true, ready: false, aged: false, left: 0.0 }
    }
}

pub const GUNSHOT_WAIT: f32 = 0.3;

impl<P> Clone for Event<P> {
    fn clone(&self) -> Self {
        Event {
            kind: self.kind, age: self.age, keep: self.keep, response: self.response, damage: self.damage.clone(), whizzed: self.whizzed,
            switch: self.switch.clone(), script: self.script.clone(), bump: self.bump, tag: self.tag, run_over: self.run_over,
            stuck: self.stuck, wait: self.wait, danger: self.danger, walked_into: self.walked_into,
        }
    }
}

pub trait Pedlike {
    fn is_dead(&self) -> bool;
    fn root(&self) -> [f32; 3] {
        [0.0; 3]
    }
    fn is_player(&self) -> bool {
        false
    }
    fn agility(&self) -> i32 {
        0
    }
    fn healthy(&self) -> bool {
        !self.is_dead()
    }
    fn on_ragdoll(&self) -> bool {
        false
    }
    fn stuck_in_air(&self) -> bool {
        false
    }
    fn frame_time(&self) -> f32 {
        0.0
    }
    fn float_in_range(&self, lo: f32, _hi: f32) -> f32 {
        lo
    }
}

impl<P: Pedlike> Event<P> {
    fn blank(kind: u32, response: i32) -> Event<P> {
        Event { kind, age: 0, keep: false, response, damage: None, whizzed: None, switch: None, script: None, bump: None, tag: 0, run_over: None, stuck: None, wait: Wait::default(), danger: None, walked_into: None }
    }

    pub fn damage(outcome: Outcome, source: Option<Source>, dir: [f32; 3], task: Option<Box<dyn Task<P>>>, nm: bool) -> Event<P> {
        let outcome = Outcome { flags: outcome.flags | APPLIED | LANDED, ..outcome };
        Event { damage: Some(Damage { real: true, source, outcome, task, nm, dir }), ..Event::blank(kind::DAMAGE, NO_RESPONSE) }
    }

    pub fn on_fire() -> Event<P> {
        Event::blank(kind::ON_FIRE, NO_RESPONSE)
    }

    pub fn whizzed_by(w: Whizzed) -> Event<P> {
        Event { whizzed: Some(w), ..Event::blank(kind::GUN_SHOT_WHIZZED_BY, RESPONSE_FIGHT) }
    }

    pub fn gun_aimed_at(w: Whizzed) -> Event<P> {
        Event { whizzed: Some(w), ..Event::blank(kind::GUN_AIMED_AT, RESPONSE_FIGHT) }
    }

    pub fn dragged_out_car(w: Whizzed) -> Event<P> {
        Event { whizzed: Some(w), ..Event::blank(kind::DRAGGED_OUT_CAR, RESPONSE_FIGHT) }
    }

    pub fn dislike(w: Whizzed) -> Event<P> {
        Event { whizzed: Some(w), ..Event::blank(kind::DISLIKE, RESPONSE_FIGHT) }
    }

    pub fn melee_action(w: Whizzed) -> Event<P> {
        Event { whizzed: Some(w), ..Event::blank(kind::MELEE_ACTION, NO_RESPONSE) }
    }

    pub fn bullet_impact(w: Whizzed) -> Event<P> {
        Event { whizzed: Some(w), ..Event::blank(kind::BULLET_IMPACT, RESPONSE_FIGHT) }
    }

    pub fn gun_shot(w: Whizzed) -> Event<P> {
        Event { whizzed: Some(w), ..Event::blank(kind::GUN_SHOT, RESPONSE_FIGHT) }
    }

    pub fn potential_get_run_over(car: carscan::Vehicle) -> Event<P> {
        Event { run_over: Some(car), ..Event::blank(kind::POTENTIAL_GET_RUN_OVER, RESPONSE_DODGE) }
    }

    pub fn imminent_danger(d: Danger) -> Event<P> {
        Event { danger: Some(d), ..Event::blank(kind::IMMINENT_DANGER, NO_RESPONSE) }
    }

    pub fn potential_be_walked_into(w: WalkedInto) -> Event<P> {
        Event { walked_into: Some(w), ..Event::blank(kind::POTENTIAL_BE_WALKED_INTO, NO_RESPONSE) }
    }

    pub fn stuck_in_air(priority: i32) -> Event<P> {
        Event { stuck: Some(priority), ..Event::blank(kind::STUCK_IN_AIR, NO_RESPONSE) }
    }

    pub fn switch2nm(task: Box<dyn Task<P>>, max_ms: u32, flag: bool) -> Event<P> {
        Event { switch: Some(Switch { task, max_ms, flag }), ..Event::blank(kind::SWITCH2NM, NO_RESPONSE) }
    }

    pub fn script(task: Box<dyn Task<P>>, priority: usize, urgent: bool, tag: u64) -> Event<P> {
        let script = Some(Script { priority, task: Some(task), urgent });
        Event { script, tag, ..Event::blank(kind::SCRIPT_COMMAND, NO_RESPONSE) }
    }

    pub fn give_task(task: Box<dyn Task<P>>, slot: usize, urgent: bool) -> Event<P> {
        let script = Some(Script { priority: slot, task: Some(task), urgent });
        Event { script, ..Event::blank(kind::GIVE_PED_TASK, NO_RESPONSE) }
    }

    pub fn static_count(task: u32) -> Event<P> {
        Event { bump: Some(task), ..Event::blank(kind::STATIC_COUNT_REACHED_MAX, NO_RESPONSE) }
    }

    pub fn priority(&self) -> i32 {
        if let Some(s) = &self.script {
            if self.kind == kind::GIVE_PED_TASK {
                return 76;
            }
            let kind = s.task.as_ref().map(|t| t.kind());
            let high = matches!(kind, Some(task::SIMPLE_DIE | task::DIE | task::MOVE_SWIM | task::USE_MOBILE_PHONE));
            return if s.urgent { 105 } else if high { 101 } else { 75 };
        }
        match (&self.damage, self.kind) {
            (Some(d), _) => 88 + if d.nm { 11 } else { 0 },
            (None, kind::DEATH) => 103,
            (None, kind::ON_FIRE) => 90,
            (None, kind::IN_WATER) => 85,
            (None, kind::SWITCH2NM) => 98 + if self.switch.as_ref().is_some_and(|s| s.flag) { 2 } else { 0 },
            (None, kind::GUN_SHOT_WHIZZED_BY) => 47,
            (None, kind::GUN_SHOT) => 46,
            (None, kind::BULLET_IMPACT) => 48,
            (None, kind::GUN_AIMED_AT) => 67,
            (None, kind::DRAGGED_OUT_CAR) => 52,
            (None, kind::DISLIKE) => 27,
            (None, kind::POTENTIAL_GET_RUN_OVER) => 68,
            (None, kind::STUCK_IN_AIR) => self.stuck.unwrap_or(86),
            (None, kind::IMMINENT_DANGER) => 69,
            (None, kind::STATIC_COUNT_REACHED_MAX) => 97,
            (None, kind::POTENTIAL_BE_WALKED_INTO) => 37,
            (None, kind::MELEE_ACTION) => 44,
            _ => 0,
        }
    }

    pub fn temporary(&self) -> bool {
        matches!(
            self.kind,
            kind::SWITCH2NM | kind::SCRIPT_COMMAND | kind::STATIC_COUNT_REACHED_MAX | kind::POTENTIAL_GET_RUN_OVER | kind::STUCK_IN_AIR | kind::IMMINENT_DANGER
                | kind::POTENTIAL_BE_WALKED_INTO
        )
    }

    pub fn takes_ragdoll(&self) -> bool {
        self.kind == kind::SWITCH2NM || self.damage.as_ref().is_some_and(|d| d.nm)
    }

    pub fn stacks(&self) -> bool {
        !matches!(
            self.kind,
            kind::ON_FIRE | kind::SCRIPT_COMMAND | kind::STATIC_COUNT_REACHED_MAX | kind::POTENTIAL_GET_RUN_OVER | kind::IMMINENT_DANGER | kind::POTENTIAL_BE_WALKED_INTO
        )
    }

    pub fn has_decision(&self) -> bool {
        self.damage.is_some() || self.whizzed.is_some() || self.run_over.is_some()
    }

    pub fn valid(&self, ped: &P) -> bool {
        match &self.script {
            Some(s) if self.kind == kind::GIVE_PED_TASK => ped.healthy() || s.urgent,
            Some(_) => ped.healthy(),
            None if self.whizzed.is_some() && !matches!(self.kind, kind::DISLIKE | kind::MELEE_ACTION) && (!self.wait.ready || self.wait.aged) => true,
            None => self.keep || self.age == 0,
        }
    }

    fn ready(&mut self, ped: &P, tasks: &Manager<P>) -> bool {
        if self.whizzed.is_none() || matches!(self.kind, kind::DISLIKE | kind::MELEE_ACTION) {
            return true;
        }
        let w = &mut self.wait;
        if [PHYSICAL_RESPONSE, EVENT_RESPONSE_TEMP, EVENT_RESPONSE_NONTEMP].iter().any(|&s| tasks.root(s).is_some()) {
            (w.ready, w.aged) = (true, false);
            return true;
        }
        if w.ready {
            w.aged = false;
            return true;
        }
        if w.first {
            (w.left, w.first) = (ped.float_in_range(0.0, GUNSHOT_WAIT), false);
        }
        w.left -= ped.frame_time();
        w.ready = w.left <= 0.0;
        if w.ready && !w.aged {
            (self.age, w.aged) = (0, true);
        }
        w.ready
    }

    pub fn concerns(&self, ped: &P, tasks: &Manager<P>) -> bool {
        if let Some(w) = &self.whizzed {
            let r = ped.root();
            let near = match self.kind {
                kind::GUN_SHOT_WHIZZED_BY => w.passes(r),
                kind::BULLET_IMPACT => (0..3).map(|l| (r[l] - w.end[l]) * (r[l] - w.end[l])).sum::<f32>() < IMPACT_REACH * IMPACT_REACH,
                kind::MELEE_ACTION => (0..3).map(|l| (r[l] - w.start[l]) * (r[l] - w.start[l])).sum::<f32>() <= MELEE_REACH * MELEE_REACH,
                _ => true,
            };
            return !ped.is_player() && !ped.is_dead() && near;
        }
        if let Some(s) = &self.script {
            return ped.healthy() || s.urgent && self.kind == kind::GIVE_PED_TASK;
        }
        if let Some(v) = &self.run_over {
            return !ped.is_dead() && v.class != 2;
        }
        if let Some(d) = &self.danger {
            let r = ped.root();
            let o = [r[0] - d.place[0], r[1] - d.place[1], r[2] - d.place[2]];
            return !ped.is_dead() && !ped.is_player() && d.reach * d.reach >= o[2].mul_add(o[2], o[1].mul_add(o[1], o[0] * o[0])) && d.agility <= ped.agility();
        }
        if let Some(w) = &self.walked_into {
            return !ped.is_dead() && w.state != 1;
        }
        if self.kind == kind::STATIC_COUNT_REACHED_MAX {
            return !ped.on_ragdoll();
        }
        if self.kind == kind::STUCK_IN_AIR {
            return ped.stuck_in_air();
        }
        let Some(d) = &self.damage else { return true };
        use crate::evdamage::{Hit, Victim};
        let root = tasks.running().and_then(|p| tasks.root(p)).map_or(0, |t| t.kind());
        let v = Victim { dead: ped.is_dead(), root, dying: tasks.find(task::DIE).is_some() };
        crate::evdamage::concerns(Hit { flags: d.outcome.flags, real: d.real }, &v, None).0
    }

    pub fn overrides(&self, running: &Event<P>) -> bool {
        if let Some(s) = &self.script {
            if running.kind == kind::DAMAGE || s.urgent && running.kind == kind::DEATH {
                return true;
            }
            return self.priority() >= running.priority();
        }
        if self.kind == kind::GUN_AIMED_AT {
            let src = |e: &Event<P>| e.whizzed.and_then(|w| w.source).or(e.damage.as_ref().and_then(|d| d.source));
            let by = src(self);
            return by.is_some_and(|s| s.player) && matches!(running.kind, kind::DAMAGE | 36 | 39) && src(running) != by
                || self.priority() >= running.priority();
        }
        if let Some(w) = &self.whizzed {
            let player = |w: Option<&Whizzed>| w.and_then(|w| w.source).is_some_and(|s| s.player);
            if running.kind == self.kind {
                return player(Some(w)) && !player(running.whizzed.as_ref()) && self.priority() >= running.priority();
            }
            if running.kind == kind::GUN_AIMED_AT && !matches!(self.kind, kind::DISLIKE | kind::MELEE_ACTION) {
                return true;
            }
        }
        if self.kind == kind::POTENTIAL_BE_WALKED_INTO && running.temporary() {
            return true;
        }
        if self.kind == kind::STUCK_IN_AIR {
            return crate::stuck::overrides(self.priority(), running.priority(), running.kind);
        }
        if let Some(d) = &self.damage {
            use crate::evdamage::{Hit, Running};
            let nm = d.task.as_ref().is_some_and(|t| (task::NM_RELAX()..=task::NM_FALL_DOWN()).contains(&t.kind()));
            let r = Running { kind: running.kind, same_source: running.damage.as_ref().and_then(|r| r.source) == d.source };
            let hit = Hit { flags: d.outcome.flags, real: d.real };
            return crate::evdamage::overrides(hit, d.source.is_some_and(|s| s.player), nm, r, self.priority() >= running.priority());
        }
        if self.kind == kind::ON_FIRE && (95..=96).contains(&running.priority()) {
            return true;
        }
        self.priority() >= running.priority()
    }

    fn record(&self) -> Event<P> {
        let mut e = self.clone();
        if let Some(d) = e.damage.as_mut() {
            d.task = None;
        }
        if let Some(s) = e.script.as_mut() {
            s.task = None;
        }
        e
    }
}

pub struct Response<P> {
    pub main: Option<Box<dyn Task<P>>>,
    pub secondary: Option<Box<dyn Task<P>>>,
}

impl<P> Default for Response<P> {
    fn default() -> Self {
        Response { main: None, secondary: None }
    }
}

pub trait Handler<P> {
    fn respond(&self, ped: &mut P, tasks: &Manager<P>, event: &Event<P>) -> Response<P>;
}

pub fn give<P>(r: Response<P>, temporary: bool, ped: &mut P, tasks: &mut Manager<P>) {
    if let Some(main) = r.main {
        if r.secondary.is_none() {
            tasks.give(ped, None, PHYSICAL_RESPONSE);
        }
        if temporary {
            tasks.give(ped, Some(main), EVENT_RESPONSE_TEMP);
        } else {
            tasks.give(ped, None, EVENT_RESPONSE_TEMP);
            tasks.give(ped, Some(main), EVENT_RESPONSE_NONTEMP);
        }
    }
    if let Some(sec) = r.secondary {
        tasks.give(ped, Some(sec), PHYSICAL_RESPONSE);
    }
}

pub struct Events<P> {
    queue: Vec<Event<P>>,
    recs: crate::evqueue::Records<Event<P>>,
    script: ScriptTask,
    serial: u64,
    slot2: bool,
}

impl<P> Default for Events<P> {
    fn default() -> Self {
        Events { queue: vec![], recs: Default::default(), script: ScriptTask::default(), serial: 0, slot2: false }
    }
}

pub const WAITING_TO_START: i32 = 0;
pub const PERFORMING: i32 = 1;
pub const DORMANT: i32 = 2;
pub const SECONDARY_TASK: i32 = 6;
pub const FINISHED: i32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptTask {
    pub id: i32,
    status: i32,
    event: Option<u64>,
    task: Option<u64>,
}

impl Default for ScriptTask {
    fn default() -> Self {
        ScriptTask { id: -1, status: 3, event: None, task: None }
    }
}

impl ScriptTask {
    fn queued(id: i32, tag: u64) -> ScriptTask {
        ScriptTask { id, status: WAITING_TO_START, event: Some(tag), task: None }
    }

    fn accepted(&mut self, tag: u64) {
        if self.event == Some(tag) {
            (self.event, self.task, self.status) = (None, Some(tag), PERFORMING);
        }
    }

    pub fn status(&self, id: i32) -> i32 {
        if self.id == id || id == -1 { self.status } else { FINISHED }
    }

    pub fn has_task(&self) -> bool {
        self.id != -1
    }

    pub fn update<P>(&mut self, queued: impl Fn(u64) -> bool, tasks: &Manager<P>) {
        match (self.event, self.task) {
            (None, None) => {}
            (Some(e), _) => {
                self.status = WAITING_TO_START;
                if !queued(e) {
                    *self = ScriptTask::default();
                }
            }
            (None, Some(t)) => {
                let running = (0..5).find(|&p| tasks.root(p).is_some());
                let response = (0..3).find(|&p| tasks.root(p).is_some());
                self.status = if running.is_some_and(|p| tasks.root_tag(p) == t) {
                    PERFORMING
                } else if response.is_some_and(|p| tasks.root_tag(p) == t) || tasks.root_tag(3) == t || tasks.root_tag(4) == t {
                    DORMANT
                } else if (0..6).any(|s| tasks.secondary_tag(s) == t) {
                    SECONDARY_TASK
                } else {
                    *self = ScriptTask::default();
                    return;
                };
            }
        }
    }
}

impl<P: Pedlike> Events<P> {
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn answering(&self, kind: u32) -> bool {
        let r = &self.recs;
        r.temp.as_ref().or(r.nontemp.as_ref()).or(r.other.as_ref()).is_some_and(|e| e.kind == kind)
    }

    pub fn has(&self, kind: u32) -> bool {
        self.queue.iter().any(|e| e.kind == kind)
    }

    pub fn clear(&mut self) {
        *self = Events::default();
    }

    pub fn has_script_task(&self, task: u32) -> bool {
        self.queue.iter().any(|e| {
            e.kind == kind::SCRIPT_COMMAND && e.script.as_ref().and_then(|s| s.task.as_ref()).is_some_and(|t| t.kind() == task)
        })
    }

    pub fn has_switch_task(&self, task: u32) -> bool {
        self.queue.iter().any(|e| e.switch.as_ref().is_some_and(|s| s.task.kind() == task))
    }

    pub fn has_shot(&self, kinds: &[u32], source: Option<Source>) -> bool {
        self.queue.iter().any(|e| kinds.contains(&e.kind) && e.whizzed.is_some_and(|w| w.source == source))
    }

    pub fn remove(&mut self, kind: u32) {
        self.queue.retain(|e| e.kind != kind);
    }

    pub fn script(&self) -> &ScriptTask {
        &self.script
    }

    pub fn add_script(&mut self, ped: &P, tasks: &Manager<P>, task: Box<dyn Task<P>>, id: i32) -> bool {
        self.remove(kind::SCRIPT_COMMAND);
        self.serial += 1;
        let tag = self.serial;
        let added = self.add(ped, tasks, Event::script(task, PRIMARY(), false, tag), true, true);
        if added {
            self.script = ScriptTask::queued(id, tag);
        }
        added
    }

    pub fn update_script(&mut self, tasks: &Manager<P>) {
        let queue = &self.queue;
        self.script.update(|tag| queue.iter().any(|e| e.tag == tag), tasks);
    }

    pub fn add(&mut self, ped: &P, tasks: &Manager<P>, mut event: Event<P>, keep: bool, replace: bool) -> bool {
        if tasks.net().is_some_and(|o| o.refuses_events()) {
            return false;
        }
        let real_damage = event.damage.as_ref().is_some_and(|d| d.real);
        if event.has_decision() && event.response == NO_RESPONSE && !real_damage {
            return false;
        }
        if !event.concerns(ped, tasks) {
            return false;
        }
        event.keep = keep;
        if self.queue.len() < MAX {
            if event.kind == kind::GIVE_PED_TASK {
                self.queue.retain(|e| e.kind != kind::GIVE_PED_TASK);
            }
        } else {
            let mut low = (None, 0x69);
            for (i, e) in self.queue.iter().enumerate() {
                if e.kind != kind::SCRIPT_COMMAND && e.priority() <= low.1 {
                    low = (Some(i), e.priority());
                }
            }
            let Some(i) = low.0.filter(|_| replace) else { return false };
            if self.queue[i].priority() >= event.priority() {
                return false;
            }
            self.queue.remove(i);
        }
        self.queue.push(event);
        true
    }

    pub fn reheap(&mut self) {
        self.slot2 = true;
    }

    pub fn handle(&mut self, ped: &mut P, tasks: &mut Manager<P>, handler: &dyn Handler<P>) {
        pass::handle(self, ped, tasks, handler)
    }
}

