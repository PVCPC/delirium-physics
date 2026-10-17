use std::mem::take;

use crate::tasks::{
    abort, build, control, kind, next, node, simple_leaf, Chain, Clock, DoNothing, End, Node, Status, Task,
    ABORT_IMMEDIATE, ABORT_URGENT,
};

pub const SLOTS: usize = 3;

pub fn mbr_for_state(state: u32) -> f32 {
    match state {
        2 => 1.0,
        3 => 2.0,
        4 => 3.0,
        _ => 0.0,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mover {
    mbr: f32,
    flagged: bool,
    owned: bool,
}

impl Mover {
    pub fn new(mbr: f32) -> Mover {
        Mover { mbr, flagged: false, owned: false }
    }

    pub fn set(&mut self, mbr: f32, flag: bool) {
        self.mbr = if mbr >= 3.0 { 3.0 } else if mbr >= 0.0 { mbr } else { 0.0 };
        self.flagged = flag;
    }

    pub fn set_state(&mut self, state: u32) {
        self.set(mbr_for_state(state), true);
    }

    pub fn mbr(&self) -> f32 {
        self.mbr
    }

    pub fn store(&mut self, mbr: f32) {
        self.mbr = mbr;
    }

    pub fn flagged(&self) -> bool {
        self.flagged
    }

    pub fn own(&mut self, on: bool) {
        self.owned = on;
    }

    pub fn owned(&self) -> bool {
        self.owned
    }
}

pub trait Moves: Clock + Sized + 'static {
    fn movement(&self) -> &Movement<Self>;
    fn movement_mut(&mut self) -> &mut Movement<Self>;
    fn stop_moving(&mut self) {}
}

struct Slot<P> {
    chain: Chain<P>,
    finished: bool,
}

impl<P> Default for Slot<P> {
    fn default() -> Self {
        Slot { chain: vec![], finished: false }
    }
}

pub struct Movement<P> {
    slot: [Slot<P>; SLOTS],
}

impl<P> Default for Movement<P> {
    fn default() -> Self {
        Movement { slot: Default::default() }
    }
}

impl<P> Movement<P> {
    pub fn root(&self, n: usize) -> Option<&dyn Task<P>> {
        self.slot[n].chain.first().map(|n| &*n.task)
    }

    pub fn finished(&self, n: usize) -> bool {
        self.slot[n].finished
    }

    pub fn active(&self) -> Option<usize> {
        self.slot.iter().position(|s| !s.chain.is_empty() && !s.finished)
    }

    pub(crate) fn chain(&self, n: usize) -> &[Node<P>] {
        &self.slot[n].chain
    }

    pub fn leaf(&self) -> Option<&dyn Task<P>> {
        self.active().and_then(|n| self.slot[n].chain.last()).map(|n| &*n.task)
    }

    pub fn find(&self, kind: u32) -> Option<&dyn Task<P>> {
        let chain = self.active().map(|n| self.chain(n)).unwrap_or(&[]);
        chain.iter().map(|n| &*n.task).find(|t| t.kind() == kind)
    }

    pub fn own(&mut self, n: usize, on: bool) {
        if let Some(m) = self.slot[n].chain.first_mut().and_then(|n| n.task.mover()) {
            m.own(on);
        }
    }

    pub fn finish(&mut self, n: usize) {
        self.slot[n].finished = true;
    }
}

pub fn with<P: Moves, R>(ped: &mut P, f: impl FnOnce(&mut Movement<P>, &mut P) -> R) -> R {
    let mut mv = take(ped.movement_mut());
    let r = f(&mut mv, ped);
    *ped.movement_mut() = mv;
    r
}

impl<P: Moves> Movement<P> {
    pub fn ask(&mut self, n: usize, ped: &mut P, urgency: u8) -> bool {
        let s = &mut self.slot[n];
        s.finished || abort(&mut s.chain, ped, urgency, None)
    }

    fn clear0(&mut self, ped: &mut P) {
        if !self.slot[0].chain.is_empty() {
            self.ask(0, ped, ABORT_IMMEDIATE);
            self.slot[0] = Slot::default();
        }
    }

    pub fn give(&mut self, ped: &mut P, task: Option<Box<dyn Task<P>>>, n: usize) {
        if !self.slot[n].chain.is_empty() {
            if !self.ask(n, ped, ABORT_URGENT) {
                self.ask(n, ped, ABORT_IMMEDIATE);
            }
            self.slot[n] = Slot::default();
            if n == 1 {
                self.clear0(ped);
            }
        }
        let Some(task) = task else { return };
        let chain = &mut self.slot[n].chain;
        chain.push(node(task));
        build(chain, ped);
        if !simple_leaf(chain) {
            self.slot[n] = Slot::default();
        }
    }

    pub fn run(&mut self, ped: &mut P) {
        for n in 0..SLOTS {
            if self.slot[n].chain.is_empty() {
                continue;
            }
            if self.slot[n].finished {
                self.slot[n] = Slot::default();
                if n == 1 {
                    self.clear0(ped);
                }
                continue;
            }
            if n == 1 && !self.slot[1].chain[0].task.mover().is_some_and(|m| m.owned()) {
                let s = &mut self.slot[1];
                if !s.chain[0].abortable {
                    abort(&mut s.chain, ped, ABORT_IMMEDIATE, None);
                }
                *s = Slot::default();
                self.clear0(ped);
                continue;
            }
            return self.run_slot(n, ped);
        }
    }

    fn run_slot(&mut self, n: usize, ped: &mut P) {
        let s = &mut self.slot[n];
        if let Some(m) = s.chain[0].task.mover() {
            m.own(false);
        }
        if !simple_leaf(&s.chain) {
            *s = Slot::default();
            if n == 1 {
                self.clear0(ped);
            }
            return;
        }
        propagate(&mut s.chain);
        let mut rounds = 0;
        loop {
            rounds += 1;
            control(&mut s.chain, ped);
            if !simple_leaf(&s.chain) {
                let k = s.chain.len() - 1;
                next(&mut s.chain, k.checked_sub(1), ped);
                if !simple_leaf(&s.chain) {
                    return finish(s);
                }
            }
            let leaf = s.chain.last_mut().unwrap();
            let done = leaf.task.process(ped);
            leaf.mark_ran();
            if !done {
                return;
            }
            let k = s.chain.len() - 1;
            next(&mut s.chain, k.checked_sub(1), ped);
            if s.chain.len() < 2 {
                return finish(s);
            }
            if rounds > 10 {
                return;
            }
        }
    }
}

fn finish<P: Moves>(s: &mut Slot<P>) {
    s.finished = true;
    if !s.chain[0].task.simple() {
        s.chain.push(node(Box::new(MoveDoNothing::new(0))));
    }
}

fn propagate<P>(chain: &mut Chain<P>) {
    let Some(at) = chain.iter_mut().position(|n| n.task.mover().is_some_and(|m| m.flagged())) else { return };
    let mbr = chain[at].task.mover().map_or(0.0, |m| m.mbr());
    for n in &mut chain[at..] {
        if let Some(m) = n.task.mover() {
            m.set(mbr, false);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MoveDoNothing {
    ms: i32,
    mover: Mover,
}

impl MoveDoNothing {
    pub fn new(ms: i32) -> MoveDoNothing {
        MoveDoNothing { ms, mover: Mover::new(0.0) }
    }
}

impl<P: Moves> Task<P> for MoveDoNothing {
    fn kind(&self) -> u32 {
        kind::MOVE_DO_NOTHING
    }

    fn process(&mut self, ped: &mut P) -> bool {
        if self.ms > 0 {
            self.ms -= (ped.dt() * 1000.0) as i32;
            if self.ms <= 0 {
                return true;
            }
        }
        ped.stop_moving();
        false
    }

    fn mover(&mut self) -> Option<&mut Mover> {
        Some(&mut self.mover)
    }
}

pub const ENDS_WITH_MOVEMENT: i32 = 0;
pub const ENDS_WITH_MAIN: i32 = 1;
pub const ENDS_WITH_EITHER: i32 = 2;
pub const ENDS_WITH_MOVEMENT_ALONE: i32 = 3;

pub struct ControlMovement<P> {
    time: f32,
    movement: Option<Box<dyn Task<P>>>,
    main: Option<Box<dyn Task<P>>>,
    mode: i32,
    done: bool,
    status: Status,
}

impl<P> ControlMovement<P> {
    pub fn new(time: f32, movement: Option<Box<dyn Task<P>>>, main: Option<Box<dyn Task<P>>>, mode: i32) -> Self {
        ControlMovement { time, movement, main, mode, done: false, status: Status::default() }
    }
}

impl<P> Clone for ControlMovement<P> {
    fn clone(&self) -> Self {
        ControlMovement::new(self.time, self.movement.clone(), self.main.clone(), self.mode)
    }
}

impl<P: Moves> ControlMovement<P> {
    fn ours(&self, ped: &P) -> bool {
        self.status.ran
            && !self.status.abortable
            && ped.movement().root(1).zip(self.movement.as_ref()).is_some_and(|(r, m)| r.kind() == m.kind())
    }

    fn default_main(&self) -> Option<Box<dyn Task<P>>> {
        Some(Box::new(DoNothing::new(0, 0)))
    }

    fn start(&mut self, ped: &mut P) -> Option<Box<dyn Task<P>>> {
        if let Some(m) = &self.movement {
            with(ped, |mv, ped| mv.give(ped, Some(m.clone()), 1));
            ped.movement().root(1)?;
        }
        ped.movement_mut().own(1, true);
        match &self.main {
            Some(m) => Some(m.clone()),
            None => self.default_main(),
        }
    }
}

impl<P: Moves> Task<P> for ControlMovement<P> {
    fn kind(&self) -> u32 {
        kind::CONTROL_MOVEMENT
    }

    fn simple(&self) -> bool {
        false
    }

    fn set_status(&mut self, status: Status) {
        self.status = status;
    }

    fn make_abortable(&mut self, ped: &mut P, urgency: u8) -> bool {
        if self.ours(ped) {
            if !with(ped, |mv, ped| mv.ask(1, ped, urgency)) {
                return false;
            }
            ped.movement_mut().own(1, false);
        }
        true
    }

    fn first(&mut self, ped: &mut P) -> Option<Box<dyn Task<P>>> {
        self.start(ped)
    }

    fn next(&mut self, ped: &mut P, done: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        let ends_with_main = matches!(self.mode, ENDS_WITH_MAIN | ENDS_WITH_EITHER);
        if !self.done && !ends_with_main && self.movement.is_some() {
            if self.main.as_ref().is_some_and(|m| m.kind() == done.kind()) {
                self.main = None;
            }
            return self.default_main();
        }
        if self.status.abortable || ped.movement().root(1).is_none() {
            return None;
        }
        with(ped, |mv, ped| mv.ask(1, ped, ABORT_IMMEDIATE));
        ped.movement_mut().finish(1);
        None
    }

    fn control(&mut self, ped: &mut P, _sub: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        if self.time > 0.0 {
            self.time -= ped.dt();
            if self.time <= 0.0 {
                return Some(Box::new(End));
            }
        }
        if !self.done {
            if let Some(m) = self.movement.as_ref().filter(|_| !self.ours(ped)) {
                with(ped, |mv, ped| mv.give(ped, Some(m.clone()), 1));
            }
            let same = ped.movement().root(1).zip(self.movement.as_ref()).is_some_and(|(r, m)| r.kind() == m.kind());
            if !same {
                return Some(Box::new(End));
            }
            ped.movement_mut().own(1, true);
            if ped.movement().finished(1) {
                let ends = match self.mode {
                    ENDS_WITH_MOVEMENT | ENDS_WITH_EITHER => true,
                    ENDS_WITH_MOVEMENT_ALONE => self.main.is_none(),
                    _ => false,
                };
                if ends {
                    return Some(Box::new(End));
                }
                self.done = true;
            }
        }
        None
    }
}

