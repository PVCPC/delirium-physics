use crate::movement::{self, Movement, Moves, Mover};

pub mod kind {
    #[allow(non_snake_case)]
    #[inline]
    pub fn PLAYER_ON_FOOT() -> u32 { crate::kv!(1 int) }
    pub const PAUSE: u32 = 202;
    pub const STAND_STILL: u32 = 203;
    #[allow(non_snake_case)]
    #[inline]
    pub fn SIMPLE_DIE() -> u32 { crate::kv!(389 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn DIE() -> u32 { crate::kv!(377 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn DEAD() -> u32 { crate::kv!(431 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn INJURED_ON_GROUND() -> u32 { crate::kv!(887 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn PLAY_RANDOM_AMBIENTS() -> u32 { crate::kv!(888 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn RUN_NAMED_ANIM() -> u32 { crate::kv!(889 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn HIT_WALL() -> u32 { crate::kv!(877 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn SEQUENCE() -> u32 { crate::kv!(349 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn USE_SEQUENCE() -> u32 { crate::kv!(890 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn DO_NOTHING() -> u32 { crate::kv!(891 int) }
    pub const MOVE_STAND_STILL: u32 = 282;
    #[allow(non_snake_case)]
    #[inline]
    pub fn MOVE_DO_NOTHING() -> u32 { crate::kv!(892 int) }
    pub const MOVE_NONE: u32 = 284;
    #[allow(non_snake_case)]
    #[inline]
    pub fn MOVE_PATHFIND_PROBLEM() -> u32 { crate::kv!(893 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn CONTROL_MOVEMENT() -> u32 { crate::kv!(894 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn MOVE_SWIM() -> u32 { crate::kv!(895 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn USE_MOBILE_PHONE() -> u32 { crate::kv!(896 int) }
    pub const CAR_DRIVE_MISSION: u32 = 724;
    pub const MOVE_GO_TO_POINT: u32 = 900;
    pub const MOVE_ACHIEVE_HEADING: u32 = 902;
    pub const MOVE_GO_TO_POINT_AND_STAND_STILL: u32 = 903;
    #[allow(non_snake_case)]
    #[inline]
    pub fn FOLLOW_POINT_ROUTE() -> u32 { crate::kv!(897 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn SMART_FLEE_POINT() -> u32 { crate::kv!(898 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn SMART_FLEE_ENTITY() -> u32 { crate::kv!(899 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn WANDER() -> u32 { crate::kv!(616 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn FOLLOW_LEADER_IN_FORMATION() -> u32 { crate::kv!(900 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn FOLLOW_LEADER_ANY_MEANS() -> u32 { crate::kv!(901 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn FLEE_ANY_MEANS() -> u32 { crate::kv!(902 int) }
    pub const MOVE_FOLLOW_NAV_MESH_ROUTE: u32 = 942;
    pub const MOVE_GO_TO_POINT_ON_ROUTE: u32 = 943;
    #[allow(non_snake_case)]
    #[inline]
    pub fn MOVE_WANDER() -> u32 { crate::kv!(903 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn MOVE_BE_IN_FORMATION() -> u32 { crate::kv!(904 int) }
    pub const MOVE_CROWD_AROUND_LOCATION: u32 = 951;
    #[allow(non_snake_case)]
    #[inline]
    pub fn GUN() -> u32 { crate::kv!(905 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn AIM_GUN() -> u32 { crate::kv!(906 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn FIRE_GUN() -> u32 { crate::kv!(907 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn RELOAD_GUN() -> u32 { crate::kv!(908 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn DUCK() -> u32 { crate::kv!(909 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn SAY_AUDIO() -> u32 { crate::kv!(910 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn GANG_HASSLE_PED() -> u32 { crate::kv!(911 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn COMBAT() -> u32 { crate::kv!(912 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_RELAX() -> u32 { crate::kv!(913 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_SHOT() -> u32 { crate::kv!(914 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_BALANCE() -> u32 { crate::kv!(765 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_EXPLOSION() -> u32 { crate::kv!(915 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_ON_FIRE() -> u32 { crate::kv!(916 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_BRACE() -> u32 { crate::kv!(917 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_HIGH_FALL() -> u32 { crate::kv!(918 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_FLINCH() -> u32 { crate::kv!(919 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_FALL_DOWN() -> u32 { crate::kv!(920 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_SCRIPT_CONTROL() -> u32 { crate::kv!(538 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn COMPLEX_NM() -> u32 { crate::kv!(921 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_POSE() -> u32 { crate::kv!(518 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NM_SIT() -> u32 { crate::kv!(559 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn REACT() -> u32 { crate::kv!(922 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn ON_FIRE() -> u32 { crate::kv!(36 int) }
    pub const END: u32 = u32::MAX;
}

pub const PHYSICAL_RESPONSE: usize = 0;
pub const EVENT_RESPONSE_TEMP: usize = 1;
pub const EVENT_RESPONSE_NONTEMP: usize = 2;
#[allow(non_snake_case)]
#[inline]
pub fn PRIMARY() -> usize { crate::kv!(0 int) }
#[allow(non_snake_case)]
#[inline]
pub fn DEFAULT() -> usize { crate::kv!(1 int) }

pub const ABORT_LEAVE: u8 = 0;
pub const ABORT_URGENT: u8 = 1;
pub const ABORT_IMMEDIATE: u8 = 2;

pub trait Clock {
    fn now(&self) -> u32;
    fn dt(&self) -> f32 {
        0.0
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Timer {
    pub start: u32,
    pub ms: i32,
    pub set: bool,
}

impl Timer {
    pub fn start(now: u32, ms: i32) -> Timer {
        Timer { start: now, ms, set: true }
    }

    pub fn done(&self, now: u32) -> bool {
        self.set && (self.start.wrapping_add(self.ms as u32) as i32) <= now as i32
    }
}

pub trait Task<P>: TaskClone<P> + Send + Sync {
    fn kind(&self) -> u32;
    fn simple(&self) -> bool {
        true
    }
    fn make_abortable(&mut self, _ped: &mut P, _urgency: u8) -> bool {
        true
    }
    fn clean_up(&mut self, _ped: &mut P) {}
    fn process(&mut self, _ped: &mut P) -> bool {
        true
    }
    fn first(&mut self, _ped: &mut P) -> Option<Box<dyn Task<P>>> {
        None
    }
    fn next(&mut self, _ped: &mut P, _done: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        None
    }
    fn control(&mut self, _ped: &mut P, _sub: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        None
    }
    fn control_ask(&self, _new: &dyn Task<P>) -> Option<(u8, NewTask)> {
        None
    }
    fn tune(&mut self, _ped: &mut P, _sub: &mut dyn Task<P>) {}
    fn abortable_for(&mut self, ped: &mut P, urgency: u8, _new: Option<&NewTask>) -> bool {
        self.make_abortable(ped, urgency)
    }
    fn set_status(&mut self, _status: Status) {}
    fn mover(&mut self) -> Option<&mut Mover> {
        None
    }
    fn progress(&self) -> Option<i32> {
        None
    }
    fn set_progress(&mut self, _step: i32) {}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub ran: bool,
    pub abortable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewTask {
    pub secondary: bool,
    pub drop_main: std::cell::Cell<bool>,
    pub kind: Option<u32>,
    pub priority: usize,
    pub event: u32,
    pub flag: bool,
    pub switched: Option<u32>,
}

#[allow(non_snake_case)]
#[inline]
pub fn NEW_TASK_EVENT() -> u32 { crate::kv!(297 int) }

#[allow(non_snake_case)]
#[inline]
pub fn SWITCH_PRIORITY() -> usize { crate::kv!(323 int) }

pub trait TaskClone<P> {
    fn clone_task(&self) -> Box<dyn Task<P>>;
}

impl<P, T: Task<P> + Clone + 'static> TaskClone<P> for T {
    fn clone_task(&self) -> Box<dyn Task<P>> {
        Box::new(self.clone())
    }
}

impl<P> Clone for Box<dyn Task<P>> {
    fn clone(&self) -> Self {
        self.clone_task()
    }
}

pub(crate) struct Node<P> {
    pub(crate) task: Box<dyn Task<P>>,
    pub(crate) abortable: bool,
    pub(crate) ran: bool,
    pub(crate) tag: u64,
}

pub(crate) type Chain<P> = Vec<Node<P>>;

pub(crate) fn node<P>(task: Box<dyn Task<P>>) -> Node<P> {
    Node { task, abortable: false, ran: false, tag: 0 }
}

impl<P> Node<P> {
    pub(crate) fn mark_ran(&mut self) {
        (self.ran, self.abortable) = (true, false);
    }

    fn tell(&mut self) {
        self.task.set_status(Status { ran: self.ran, abortable: self.abortable });
    }
}

pub(crate) fn abort<P>(chain: &mut [Node<P>], ped: &mut P, urgency: u8, new: Option<&NewTask>) -> bool {
    if chain.first().is_none_or(|n| n.abortable) {
        return true;
    }
    let ok = chain.iter_mut().rev().all(|n| {
        n.tell();
        n.task.abortable_for(ped, urgency, new)
    });
    if ok {
        chain.iter_mut().for_each(|n| n.abortable = true);
    }
    ok
}

pub(crate) fn build<P>(chain: &mut Chain<P>, ped: &mut P) {
    loop {
        let k = chain.len() - 1;
        if chain[k].task.simple() {
            return;
        }
        chain[k].tell();
        let t = chain[k].task.first(ped);
        chain[k].mark_ran();
        match t {
            Some(t) => chain.push(node(t)),
            None => return next(chain, k.checked_sub(1), ped),
        }
    }
}

pub(crate) fn next<P>(chain: &mut Chain<P>, mut at: Option<usize>, ped: &mut P) {
    while let Some(k) = at {
        let (head, tail) = chain.split_at_mut(k + 1);
        head[k].tell();
        let t = head[k].task.next(ped, &*tail[0].task);
        chain.truncate(k + 1);
        if let Some(t) = t {
            chain.push(node(t));
            return build(chain, ped);
        }
        at = k.checked_sub(1);
    }
}

pub(crate) fn control<P>(chain: &mut Chain<P>, ped: &mut P) {
    let mut k = 0;
    while k + 1 < chain.len() && !chain[k].task.simple() {
        let (head, tail) = chain.split_at_mut(k + 1);
        head[k].tell();
        head[k].task.tune(ped, &mut *tail[0].task);
        let new = head[k].task.control(ped, &*tail[0].task);
        head[k].mark_ran();
        if let Some(t) = new {
            if let Some((urgency, ev)) = chain[k].task.control_ask(&*t) {
                abort(&mut chain[k + 1..], ped, urgency, Some(&ev));
            }
            if !chain[k + 1].abortable {
                let ev = NewTask { secondary: false, drop_main: Default::default(), kind: (t.kind() != kind::END).then(|| t.kind()), priority: SWITCH_PRIORITY(), event: NEW_TASK_EVENT(), flag: false, switched: None };
                abort(&mut chain[k + 1..], ped, ABORT_IMMEDIATE, Some(&ev));
            }
            chain.truncate(k + 1);
            if t.kind() == kind::END {
                return;
            }
            chain.push(node(t));
            return build(chain, ped);
        }
        k += 1;
    }
}

pub(crate) fn simple_leaf<P>(chain: &Chain<P>) -> bool {
    chain.last().is_some_and(|n| n.task.simple())
}

pub(crate) fn clean_up<P>(chain: &mut Chain<P>, ped: &mut P) {
    chain.iter_mut().for_each(|n| n.task.clean_up(ped));
    chain.clear();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub kind: u32,
    pub running: bool,
    pub priority: usize,
    pub progress: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetObject {
    pub clone: bool,
    pub owner: i8,
    pub open: bool,
}

impl NetObject {
    pub fn owned(&self) -> bool {
        self.owner != -1
    }

    pub fn may_give(&self) -> bool {
        if !self.clone && !self.owned() { true } else { self.open }
    }

    pub fn lets_give(&self, priority: usize, default_busy: bool) -> bool {
        if self.clone {
            return self.may_give();
        }
        !self.owned() || priority == DEFAULT() && !default_busy
    }

    pub fn refuses_events(&self) -> bool {
        self.clone || self.owned()
    }
}

pub struct Manager<P> {
    net: Option<NetObject>,
    primary: [Chain<P>; 5],
    secondary: [Chain<P>; 6],
    running: Option<usize>,
    default: fn(&mut P) -> Option<Box<dyn Task<P>>>,
    list: Vec<Info>,
}

impl<P> Default for Manager<P> {
    fn default() -> Self {
        Manager::new(|_| None)
    }
}

impl<P> Manager<P> {
    pub fn new(default: fn(&mut P) -> Option<Box<dyn Task<P>>>) -> Self {
        Manager { net: None, primary: Default::default(), secondary: Default::default(), running: None, default, list: vec![] }
    }

    pub fn net(&self) -> Option<NetObject> {
        self.net
    }

    pub fn set_net(&mut self, net: Option<NetObject>) {
        self.net = net;
    }

    pub fn give(&mut self, ped: &mut P, task: Option<Box<dyn Task<P>>>, priority: usize) {
        if self.running == Some(DEFAULT()) && priority == DEFAULT() && !self.primary[DEFAULT()].is_empty() {
            return;
        }
        if self.net.is_some_and(|o| !o.lets_give(priority, !self.primary[DEFAULT()].is_empty())) {
            return;
        }
        let ev = NewTask { secondary: false, drop_main: Default::default(), kind: task.as_ref().map(|t| t.kind()), priority, event: NEW_TASK_EVENT(), flag: false, switched: None };
        for chain in &mut self.primary[priority..] {
            if !abort(chain, ped, ABORT_URGENT, Some(&ev)) {
                abort(chain, ped, ABORT_IMMEDIATE, Some(&ev));
            }
        }
        clean_up(&mut self.primary[priority], ped);
        let Some(task) = task else { return };
        let chain = &mut self.primary[priority];
        chain.push(node(task));
        build(chain, ped);
        if !simple_leaf(chain) {
            clean_up(chain, ped);
            if priority == DEFAULT() {
                let t = (self.default)(ped);
                self.give(ped, t, DEFAULT());
            }
        }
    }

    pub fn give_secondary(&mut self, ped: &mut P, task: Option<Box<dyn Task<P>>>, slot: usize) {
        let chain = &mut self.secondary[slot];
        chain.clear();
        if let Some(task) = task {
            chain.push(node(task));
            build(chain, ped);
            if !simple_leaf(chain) {
                chain.clear();
            }
        }
    }

    pub fn flush_secondary(&mut self, ped: &mut P) {
        for slot in (0..self.secondary.len()).filter(|&s| s != crate::kv!(0 int)) {
            if self.secondary[slot].is_empty() {
                continue;
            }
            if abort(&mut self.secondary[slot], ped, ABORT_URGENT, None) {
                self.give_secondary(ped, None, slot);
            } else {
                abort(&mut self.secondary[slot], ped, ABORT_LEAVE, None);
            }
        }
    }

    pub fn flush_primary(&mut self, ped: &mut P) {
        let ev = NewTask { secondary: false, drop_main: Default::default(), kind: None, priority: 0, event: NEW_TASK_EVENT(), flag: false, switched: None };
        for chain in &mut self.primary {
            if !chain.is_empty() && abort(chain, ped, ABORT_IMMEDIATE, Some(&ev)) {
                clean_up(chain, ped);
            }
        }
    }

    pub fn clear(&mut self, ped: &mut P) {
        for chain in self.primary.iter_mut().chain(self.secondary.iter_mut()) {
            clean_up(chain, ped);
        }
        self.list.clear();
    }

    fn ended(&mut self, s: usize, ped: &mut P) -> bool {
        self.primary[s].clear();
        if s == DEFAULT() {
            let t = (self.default)(ped);
            self.give(ped, t, DEFAULT());
        }
        !self.primary[s].is_empty()
    }

    pub fn leaf(&self) -> Option<&dyn Task<P>> {
        self.primary.iter().find(|c| !c.is_empty()).and_then(|c| c.last()).map(|n| &*n.task)
    }

    pub fn running(&self) -> Option<usize> {
        self.primary.iter().position(|c| !c.is_empty())
    }

    pub fn ask(&mut self, ped: &mut P, priority: usize, urgency: u8, new: &NewTask) -> bool {
        abort(&mut self.primary[priority], ped, urgency, Some(new))
    }

    pub fn root(&self, priority: usize) -> Option<&dyn Task<P>> {
        self.primary[priority].first().map(|n| &*n.task)
    }

    pub fn kinds(&self, priority: usize) -> Vec<u32> {
        self.primary[priority].iter().map(|n| n.task.kind()).collect()
    }

    pub fn secondary(&self, slot: usize) -> Option<&dyn Task<P>> {
        self.secondary[slot].first().map(|n| &*n.task)
    }

    pub fn find(&self, kind: u32) -> Option<&dyn Task<P>> {
        let running = self.primary.iter().find(|c| !c.is_empty()).into_iter();
        running.chain(self.secondary.iter()).flatten().map(|n| &*n.task).find(|t| t.kind() == kind)
    }

    pub fn find_task<'a>(&'a self, mv: &'a Movement<P>, kind: u32) -> Option<&'a dyn Task<P>> {
        let running = self.primary.iter().find(|c| !c.is_empty()).map(|c| c.as_slice()).unwrap_or(&[]);
        let moving = mv.active().map(|n| mv.chain(n)).unwrap_or(&[]);
        let all = running.iter().chain(moving).chain(self.secondary.iter().flatten());
        all.map(|n| &*n.task).find(|t| t.kind() == kind)
    }

    pub fn find_in(&self, priority: usize, kind: u32) -> Option<&dyn Task<P>> {
        self.primary[priority].iter().map(|n| &*n.task).find(|t| t.kind() == kind)
    }

    pub fn update_list(&mut self) {
        self.list.clear();
        let (mut running, mut roots_only) = (true, false);
        for (priority, chain) in self.primary.iter().enumerate() {
            if chain.is_empty() {
                continue;
            }
            for (i, n) in chain.iter().enumerate() {
                let below = chain.get(i + 1).and_then(|n| n.task.progress());
                let progress = n.task.progress().map(|p| (p, below.unwrap_or(-1)));
                self.list.push(Info { kind: n.task.kind(), running, priority, progress });
                if roots_only {
                    break;
                }
            }
            running = false;
            roots_only |= priority > 1;
        }
    }

    pub fn list(&self) -> &[Info] {
        &self.list
    }

    pub fn doing(&self, kind: u32, running_only: bool) -> bool {
        let Some(first) = self.list.first() else { return false };
        let mut at = first.priority;
        for e in &self.list {
            if at < e.priority && (at >= 2 || running_only) {
                return false;
            }
            at = e.priority;
            if e.kind == kind {
                return true;
            }
        }
        false
    }

    pub fn root_tag(&self, priority: usize) -> u64 {
        self.primary[priority].first().map_or(0, |n| n.tag)
    }

    pub fn secondary_tag(&self, slot: usize) -> u64 {
        self.secondary[slot].first().map_or(0, |n| n.tag)
    }

    pub fn tag_root(&mut self, priority: usize, tag: u64) {
        if let Some(n) = self.primary[priority].first_mut() {
            n.tag = tag;
        }
    }

    pub fn clear_responses(&mut self, ped: &mut P, priority: usize) {
        if priority >= 1 && !self.primary[EVENT_RESPONSE_TEMP].is_empty() {
            self.give(ped, None, EVENT_RESPONSE_TEMP);
        } else if priority >= 2 && !self.primary[EVENT_RESPONSE_NONTEMP].is_empty() {
            self.give(ped, None, EVENT_RESPONSE_NONTEMP);
        }
    }

    pub fn info(&self, kind: u32, priority: Option<usize>) -> Option<&Info> {
        self.list.iter().find(|e| e.kind == kind && priority.is_none_or(|p| p == e.priority))
    }
}

impl<P: Moves> Manager<P> {
    pub fn run(&mut self, ped: &mut P) {
        if self.primary[DEFAULT()].is_empty() {
            let t = (self.default)(ped);
            self.give(ped, t, DEFAULT());
        }
        if self.run_primary(ped) {
            movement::with(ped, |mv, ped| {
                if mv.root(2).is_none() {
                    mv.give(ped, Some(Box::new(movement::MoveDoNothing::new(0))), 2);
                }
            });
        }
        movement::with(ped, |mv, ped| mv.run(ped));
        for chain in &mut self.secondary {
            run_secondary(chain, ped);
        }
        self.update_list();
    }

    fn run_primary(&mut self, ped: &mut P) -> bool {
        let Some(s) = self.primary.iter().position(|c| !c.is_empty()) else {
            self.running = None;
            return true;
        };
        self.running = Some(s);
        if !simple_leaf(&self.primary[s]) {
            self.primary[s].clear();
            self.running = None;
            return false;
        }
        let mut rounds = 0;
        loop {
            rounds += 1;
            let last = rounds > crate::kv!(77 int);
            control(&mut self.primary[s], ped);
            if !simple_leaf(&self.primary[s]) {
                let k = self.primary[s].len() - 1;
                next(&mut self.primary[s], k.checked_sub(1), ped);
                if !simple_leaf(&self.primary[s]) && !self.ended(s, ped) {
                    break;
                }
            }
            let chain = &mut self.primary[s];
            let leaf = chain.last_mut().unwrap();
            let done = leaf.task.process(ped);
            leaf.mark_ran();
            if !done {
                break;
            }
            let k = chain.len() - 1;
            next(chain, k.checked_sub(1), ped);
            if self.primary[s].len() < 2 && !self.ended(s, ped) || last {
                break;
            }
        }
        self.running = None;
        true
    }
}

fn run_secondary<P>(chain: &mut Chain<P>, ped: &mut P) {
    while !chain.is_empty() {
        control(chain, ped);
        if !simple_leaf(chain) {
            return chain.clear();
        }
        let leaf = chain.last_mut().unwrap();
        let done = leaf.task.process(ped);
        leaf.mark_ran();
        if !done {
            return;
        }
        let k = chain.len() - 1;
        next(chain, k.checked_sub(1), ped);
        if chain.len() < 2 {
            return chain.clear();
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct End;

impl<P> Task<P> for End {
    fn kind(&self) -> u32 {
        kind::END
    }
}

#[derive(Debug)]
pub struct StandStill {
    pub ms: i32,
    pub blend: f32,
    pub forever: bool,
    pub flag45: u8,
    timer: Timer,
}

impl StandStill {
    pub fn new(ms: i32, blend: f32, forever: bool) -> StandStill {
        Self::with_flags(ms, blend, forever, 0)
    }

    pub fn with_flags(ms: i32, blend: f32, forever: bool, flag45: u8) -> StandStill {
        StandStill { ms, blend, forever, flag45, timer: Timer::default() }
    }
}

impl Clone for StandStill {
    fn clone(&self) -> Self {
        Self::with_flags(self.ms, self.blend, self.forever, self.flag45)
    }
}

impl<P: Clock> Task<P> for StandStill {
    fn kind(&self) -> u32 {
        kind::STAND_STILL
    }

    fn make_abortable(&mut self, ped: &mut P, urgency: u8) -> bool {
        if urgency == ABORT_LEAVE {
            self.timer = Timer::start(ped.now(), -1);
        }
        true
    }

    fn process(&mut self, ped: &mut P) -> bool {
        if !self.timer.set && self.ms >= 0 {
            self.timer = Timer::start(ped.now(), self.ms);
        }
        !self.forever && self.timer.done(ped.now())
    }
}

#[derive(Debug)]
pub struct Pause {
    pub ms: i32,
    timer: Timer,
}

impl Pause {
    pub fn new(ms: i32) -> Pause {
        Pause { ms, timer: Timer::default() }
    }
}

impl Clone for Pause {
    fn clone(&self) -> Self {
        Self::new(self.ms)
    }
}

impl<P: Clock> Task<P> for Pause {
    fn kind(&self) -> u32 {
        kind::PAUSE
    }

    fn make_abortable(&mut self, ped: &mut P, _: u8) -> bool {
        self.timer = Timer::start(ped.now(), -1);
        true
    }

    fn process(&mut self, ped: &mut P) -> bool {
        if !self.timer.set {
            self.timer = Timer::start(ped.now(), self.ms);
        }
        self.timer.done(ped.now())
    }
}

#[derive(Debug, Default)]
pub struct DoNothing {
    pub ms: i32,
    pub frames: i32,
    done: bool,
}

impl DoNothing {
    pub fn new(ms: i32, frames: i32) -> DoNothing {
        DoNothing { ms, frames, done: false }
    }
}

impl Clone for DoNothing {
    fn clone(&self) -> Self {
        Self::new(self.ms, self.frames)
    }
}

impl<P: Clock> Task<P> for DoNothing {
    fn kind(&self) -> u32 {
        kind::DO_NOTHING()
    }

    fn process(&mut self, ped: &mut P) -> bool {
        if self.frames > 0 {
            self.frames -= 1;
            self.done |= self.frames == 0;
        }
        if self.done {
            return true;
        }
        if self.ms > 0 {
            self.ms -= (ped.dt() * crate::kv!(78)) as i32;
            return self.ms <= 0;
        }
        false
    }
}

pub(crate) fn sequence_step<P>(tasks: &[Box<dyn Task<P>>], repeat: i32, step: &mut i32, loops: &mut i32) -> Option<Box<dyn Task<P>>> {
    *step += 1;
    if repeat != 0 && tasks.get(*step as usize).is_none() {
        (*step, *loops) = (0, *loops + 1);
        if repeat != 1 && *loops == repeat {
            return None;
        }
    }
    tasks.get(*step as usize).map(|t| t.clone_task())
}

pub struct Sequence<P> {
    tasks: Vec<Box<dyn Task<P>>>,
    pub repeat: i32,
    step: i32,
    loops: i32,
}

impl<P> Clone for Sequence<P> {
    fn clone(&self) -> Self {
        Sequence { tasks: self.tasks.clone(), repeat: self.repeat, step: self.step, loops: 0 }
    }
}

impl<P> Sequence<P> {
    pub fn new(repeat: i32) -> Sequence<P> {
        Sequence { tasks: vec![], repeat, step: 0, loops: 0 }
    }

    pub fn add(&mut self, task: Box<dyn Task<P>>) -> bool {
        let room = self.tasks.len() < crate::kv!(51 int);
        if room {
            self.tasks.push(task);
        }
        room
    }
}

impl<P: 'static> Task<P> for Sequence<P> {
    fn kind(&self) -> u32 {
        kind::SEQUENCE()
    }

    fn simple(&self) -> bool {
        false
    }

    fn first(&mut self, _: &mut P) -> Option<Box<dyn Task<P>>> {
        self.tasks.get(self.step as usize).map(|t| t.clone_task())
    }

    fn next(&mut self, _: &mut P, _: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        sequence_step(&self.tasks, self.repeat, &mut self.step, &mut self.loops)
    }
}

