use crate::actiontable::wrap;
use crate::pedgroups::{Place, CROWD, LEAD};
use crate::movement::{with, ControlMovement, Moves, Mover, ENDS_WITH_MOVEMENT};
use crate::tasks::{kind, DoNothing, End, Pause, Task, Timer, ABORT_URGENT};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    pub pos: [f32; 3],
    pub heading: f32,
    pub speeds: [f32; 4],
    pub group: Option<usize>,
    pub leader: Option<Leader>,
    pub place: Option<Place>,
    pub flags: u32,
    pub catch_up: [f32; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Leader {
    pub id: u64,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub mbr: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Order {
    Go { target: [f32; 3], pace: f32 },
    Face(f32),
    Stand,
}

pub trait Follower: Moves {
    fn view(&self) -> &View;
    fn order(&mut self, order: Order);
}

pub fn move_state(mbr: f32) -> u32 {
    if mbr >= 3.0 {
        4
    } else if mbr >= 1.5 {
        3
    } else if mbr > 0.0 {
        2
    } else {
        1
    }
}

pub fn ratio_for_speed(speeds: &[f32; 4], v: f32) -> f32 {
    if v < speeds[0] {
        return 0.0;
    }
    for i in 0..3 {
        let (a, b) = (speeds[i], speeds[i + 1]);
        if !(v < a) && v <= b {
            return i as f32 + if a == b { 1.0 } else { (v - a) / (b - a) };
        }
    }
    3.0
}

pub fn pace(ped: [f32; 3], target: [f32; 3], leader: Option<&Leader>, flags: u32, catch_up: [f32; 2], speeds: &[f32; 4]) -> f32 {
    let to = leader.filter(|_| flags & LEAD != 0).map_or(target, |l| l.pos);
    let d = [to[0] - ped[0], to[1] - ped[1], to[2] - ped[2]];
    let dist2 = d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2]));
    let dist = dist2.sqrt();
    let [near, far] = catch_up;
    let hold = |r: f32| if r < 0.5 { 0.5 } else if r > 3.0 { 3.0 } else { r };
    if let Some(l) = leader {
        let v2 = l.vel[0].mul_add(l.vel[0], l.vel[1] * l.vel[1]);
        if v2 > 0.001 {
            let m = hold(ratio_for_speed(speeds, v2.sqrt()));
            let mut f = 0.0;
            if dist > near {
                f = (dist - near) / (far - near);
                f = if f - 1.0 >= 0.0 { 1.0 } else { f };
            }
            return hold(f + m);
        }
    }
    if dist2 > 64.0 {
        3.0
    } else if dist2 > 36.0 {
        2.0
    } else {
        1.0
    }
}

fn diff(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[derive(Clone, Copy, Debug)]
pub struct FollowLeaderAnyMeans {
    pub group: usize,
    pub leader: u64,
}

impl FollowLeaderAnyMeans {
    pub fn of(v: &View) -> Option<FollowLeaderAnyMeans> {
        let (group, leader) = (v.group?, v.leader?);
        v.place.map(|_| FollowLeaderAnyMeans { group, leader: leader.id })
    }
}

impl<P: Follower> Task<P> for FollowLeaderAnyMeans {
    fn kind(&self) -> u32 {
        kind::FOLLOW_LEADER_ANY_MEANS
    }

    fn simple(&self) -> bool {
        false
    }

    fn first(&mut self, _: &mut P) -> Option<Box<dyn Task<P>>> {
        Some(Box::new(Pause::new(1)))
    }

    fn next(&mut self, _: &mut P, done: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        matches!(done.kind(), kind::PAUSE | kind::STAND_STILL | kind::CAR_DRIVE_MISSION)
            .then(|| Box::new(FollowLeaderInFormation { group: self.group, leader: self.leader }) as Box<dyn Task<P>>)
    }

    fn control(&mut self, ped: &mut P, _: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        let v = ped.view();
        (v.group.is_some() && v.leader.map(|l| l.id) != Some(self.leader)).then(|| Box::new(End) as Box<dyn Task<P>>)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FollowLeaderInFormation {
    pub group: usize,
    pub leader: u64,
}

impl<P: Follower> Task<P> for FollowLeaderInFormation {
    fn kind(&self) -> u32 {
        kind::FOLLOW_LEADER_IN_FORMATION
    }

    fn simple(&self) -> bool {
        false
    }

    fn first(&mut self, ped: &mut P) -> Option<Box<dyn Task<P>>> {
        with(ped, |mv, ped| mv.active().map(|n| mv.ask(n, ped, ABORT_URGENT)));
        let movement: Box<dyn Task<P>> = Box::new(MoveBeInFormation::new(self.group));
        Some(Box::new(ControlMovement::new(0.0, Some(movement), Some(Box::new(DoNothing::new(0, 0))), ENDS_WITH_MOVEMENT)))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MoveBeInFormation {
    pub group: usize,
    target: [f32; 3],
    heading: f32,
    near: f32,
    last: [f32; 3],
    state: u32,
    stopped: bool,
    sub: Move,
    mover: Mover,
}

impl MoveBeInFormation {
    pub fn new(group: usize) -> MoveBeInFormation {
        let stand = Move { kind: kind::MOVE_STAND_STILL, target: [0.0; 3], pace: 0.0, near: 0.0, heading: 0.0, timer: Timer::default(), mover: Mover::new(0.0) };
        MoveBeInFormation { group, target: [0.0; 3], heading: 0.0, near: 0.0, last: [10000.0; 3], state: 1, stopped: false, sub: stand, mover: Mover::new(0.0) }
    }

    fn place(&mut self, v: &View) -> bool {
        let Some(p) = v.place else { return false };
        (self.target, self.heading, self.near) = (p.pos, p.heading, p.near);
        true
    }

    fn aim(&self, v: &View) -> [f32; 3] {
        match v.leader {
            Some(l) if v.flags & LEAD != 0 && move_state(l.mbr) > 1 => {
                [self.target[0] + l.vel[0], self.target[1] + l.vel[1], self.target[2] + l.vel[2]]
            }
            _ => self.target,
        }
    }

    fn pace(&self, v: &View, target: [f32; 3]) -> f32 {
        pace(v.pos, target, v.leader.as_ref(), v.flags, v.catch_up, &v.speeds)
    }

    fn make<P: Follower>(&mut self, kind: u32, v: &View) -> Box<dyn Task<P>> {
        let mut target = self.aim(v);
        let pace = self.pace(v, target);
        let mut near = self.near;
        if kind == kind::MOVE_GO_TO_POINT
            && v.leader.is_some_and(|l| l.vel[2].mul_add(l.vel[2], l.vel[0].mul_add(l.vel[0], l.vel[1] * l.vel[1])) > 0.01)
        {
            near = 0.1;
        }
        if kind == kind::MOVE_CROWD_AROUND_LOCATION {
            target = v.leader.map_or(target, |l| l.pos);
        }
        self.sub = Move { kind, target, pace, near, heading: self.heading, timer: Timer::default(), mover: Mover::new(0.0) };
        Box::new(self.sub)
    }

    fn retarget<P: Follower>(&mut self, m: Move) -> Option<Box<dyn Task<P>>> {
        if m.target == self.sub.target && m.pace == self.sub.pace {
            return None;
        }
        self.sub = m;
        Some(Box::new(m))
    }

    fn after<P: Follower>(&mut self, done: u32, v: &View) -> Option<Box<dyn Task<P>>> {
        let k = match done {
            kind::MOVE_GO_TO_POINT | kind::MOVE_FOLLOW_NAV_MESH_ROUTE => kind::MOVE_ACHIEVE_HEADING,
            kind::MOVE_STAND_STILL | kind::MOVE_ACHIEVE_HEADING | kind::MOVE_CROWD_AROUND_LOCATION => {
                match v.leader.filter(|l| v.flags & CROWD != 0 && l.pos[2] - v.pos[2] <= 2.0) {
                    Some(l) if l.mbr == 0.0 => kind::MOVE_CROWD_AROUND_LOCATION,
                    Some(_) => kind::MOVE_FOLLOW_NAV_MESH_ROUTE,
                    None if wrap(v.heading - self.heading).abs() > f32::from_bits(0x3D8E_FA35) => kind::MOVE_ACHIEVE_HEADING,
                    None => kind::MOVE_STAND_STILL,
                }
            }
            _ => return None,
        };
        Some(self.make(k, v))
    }

    fn standing<P: Follower>(&mut self, v: &View, sub: u32, target: [f32; 3]) -> Option<Box<dyn Task<P>>> {
        if !matches!(sub, kind::MOVE_ACHIEVE_HEADING | kind::MOVE_STAND_STILL | kind::MOVE_CROWD_AROUND_LOCATION) {
            return None;
        }
        let m = diff(target, self.last);
        if !(m[1].mul_add(m[1], m[0] * m[0]) > 1.0 / 1024.0) {
            return None;
        }
        self.last = target;
        let d = diff(v.pos, target);
        let (xy, near) = (d[1].mul_add(d[1], d[0] * d[0]), self.near * self.near);
        let k = if xy >= 6.0 {
            (xy > near || d[2].abs() > 4.0).then_some(kind::MOVE_FOLLOW_NAV_MESH_ROUTE)
        } else {
            (xy > 0.0625 && xy > near && d[2].abs() < 4.0).then_some(kind::MOVE_GO_TO_POINT)
        }?;
        Some(self.make(k, v))
    }

    fn moving<P: Follower>(&mut self, v: &View, sub: u32, target: [f32; 3]) -> Option<Box<dyn Task<P>>> {
        let mut m = self.sub;
        match sub {
            kind::MOVE_FOLLOW_NAV_MESH_ROUTE => {
                let d = diff(m.target, target);
                if self.stopped || d[1].mul_add(d[1], d[0] * d[0]) >= 6.0 || d[2].abs() > 4.0 {
                    m.target = target;
                }
            }
            kind::MOVE_GO_TO_POINT => {
                if let Some(l) = v.leader.filter(|_| self.state == 1) {
                    let d = diff(l.pos, m.target);
                    if d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2])) < 0.2 {
                        return self.after(sub, v);
                    }
                }
                let d = diff(target, v.pos);
                if d[1].mul_add(d[1], d[0] * d[0]) >= 8.0 || d[2].abs() > 4.0 {
                    return Some(self.make(kind::MOVE_FOLLOW_NAV_MESH_ROUTE, v));
                }
                m.target = target;
            }
            _ => return None,
        }
        m.pace = self.pace(v, target);
        self.retarget(m)
    }
}

impl<P: Follower> Task<P> for MoveBeInFormation {
    fn kind(&self) -> u32 {
        kind::MOVE_BE_IN_FORMATION
    }

    fn simple(&self) -> bool {
        false
    }

    fn first(&mut self, ped: &mut P) -> Option<Box<dyn Task<P>>> {
        let v = *ped.view();
        self.place(&v).then(|| self.make(kind::MOVE_FOLLOW_NAV_MESH_ROUTE, &v))
    }

    fn next(&mut self, ped: &mut P, done: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        let v = *ped.view();
        self.after(done.kind(), &v)
    }

    fn control(&mut self, ped: &mut P, sub: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        let v = *ped.view();
        if v.group != Some(self.group) {
            return Some(Box::new(End));
        }
        let state = v.leader.map_or(1, |l| move_state(l.mbr));
        (self.stopped, self.state) = (state == 1 && self.state > 1, state);
        if !self.place(&v) {
            return Some(Box::new(End));
        }
        let target = self.aim(&v);
        self.standing(&v, sub.kind(), target).or_else(|| self.moving(&v, sub.kind(), target))
    }

    fn mover(&mut self) -> Option<&mut Mover> {
        Some(&mut self.mover)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub kind: u32,
    pub target: [f32; 3],
    pub pace: f32,
    pub near: f32,
    pub heading: f32,
    timer: Timer,
    mover: Mover,
}

impl Move {
    pub fn go(kind: u32, target: [f32; 3], pace: f32, near: f32) -> Move {
        Move { kind, target, pace, near, heading: 0.0, timer: Timer::default(), mover: Mover::new(0.0) }
    }
}

impl<P: Follower> Task<P> for Move {
    fn kind(&self) -> u32 {
        self.kind
    }

    fn process(&mut self, ped: &mut P) -> bool {
        let v = *ped.view();
        match self.kind {
            kind::MOVE_GO_TO_POINT | kind::MOVE_FOLLOW_NAV_MESH_ROUTE => {
                ped.order(Order::Go { target: self.target, pace: self.pace });
                let d = diff(self.target, v.pos);
                d[1].mul_add(d[1], d[0] * d[0]) <= self.near * self.near
            }
            kind::MOVE_ACHIEVE_HEADING => {
                ped.order(Order::Face(self.heading));
                wrap(self.heading - v.heading).abs() <= 0.02
            }
            kind::MOVE_STAND_STILL => {
                ped.order(Order::Stand);
                if !self.timer.set {
                    self.timer = Timer::start(ped.now(), 2000);
                }
                self.timer.done(ped.now())
            }
            _ => {
                ped.order(Order::Stand);
                false
            }
        }
    }

    fn mover(&mut self) -> Option<&mut Mover> {
        Some(&mut self.mover)
    }
}

