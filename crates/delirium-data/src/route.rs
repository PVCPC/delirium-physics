#![allow(clippy::neg_cmp_op_on_partial_ord)]
use crate::follow::{Follower, Order};
use crate::mathrt;
use crate::movement::Mover;
use crate::tasks::{kind, End, Task, Timer, ABORT_LEAVE};

pub const MAX_POINTS: usize = 8;
pub const LEG_END: u32 = 1302;
pub const MAX_BUMPS: i32 = 30;
pub const ARRIVE: f32 = 0.5;
pub const SLOW: f32 = 5.0;

pub type Point = [f32; 3];

pub trait Walker: Follower {
    fn here(&self) -> Point;
    fn velocity(&self) -> Point;
    fn facing(&self) -> f32 {
        self.view().heading
    }
    fn in_vehicle(&self) -> bool {
        false
    }
    fn bumps(&self) -> i32 {
        0
    }
    fn pace_held(&self) -> bool {
        false
    }
    fn stopped(&self) -> bool {
        true
    }
    fn blocked(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Route {
    points: Vec<Point>,
}

impl Route {
    pub fn new() -> Route {
        Route::default()
    }

    pub fn extend(&mut self, p: Point) {
        if self.points.len() < MAX_POINTS {
            self.points.push(p);
        }
    }

    pub fn flush(&mut self) {
        self.points.clear();
    }

    pub fn points(&self) -> &[Point] {
        &self.points
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn same(&self, o: &Route) -> bool {
        self.points.len() == o.points.len() && self.points.iter().zip(&o.points).all(|(a, b)| a == b)
    }

    pub fn reverse(&mut self) {
        self.points.reverse();
    }
}

fn rsqrte(v: f32) -> f32 {
    if v.is_nan() || v < 0.0 {
        f32::NAN
    } else if v == 0.0 {
        if v.is_sign_negative() { f32::NEG_INFINITY } else { f32::INFINITY }
    } else if v == f32::INFINITY {
        0.0
    } else {
        (1.0 / (v as f64).sqrt()) as f32
    }
}

fn unit(d: Point) -> Point {
    unit_len(d).0
}

fn unit_len(d: Point) -> (Point, f32) {
    let s = (0.0 + d[0] as f64 * d[0] as f64 + d[1] as f64 * d[1] as f64 + d[2] as f64 * d[2] as f64) as f32;
    let e = rsqrte(s);
    let e = if e == f32::INFINITY { 0.0 } else { e };
    let e2 = e * e;
    let half = 0.5 * e;
    let t = (-(s as f64 * e2 as f64 - 1.0)) as f32;
    let r = (t as f64 * half as f64 + e as f64) as f32;
    ([d[0] * r, d[1] * r, d[2] * r], 1.0 / r)
}

const CORNER_ANGLE: f32 = std::f32::consts::FRAC_PI_8;
const CORNER_FLOOR: f32 = 1.1;

fn clamp01(v: f32) -> f32 {
    let v = if v >= 0.0 { v } else { 0.0 };
    if v as f64 - 1.0 >= 0.0 { 1.0 } else { v }
}

pub fn corner_pace(asked: f32, ped: Point, target: Point, next: Point, held: bool) -> f32 {
    if held || !(asked > 1.0) {
        return asked;
    }
    let (base, limit) = if asked > 2.0 { (2.0, 4.0) } else { (1.5, 3.0) };
    let (ahead, dist) = unit_len([target[0] - ped[0], target[1] - ped[1], 0.0]);
    if !(dist < limit) {
        return asked;
    }
    let (away, _) = unit_len([next[0] - target[0], next[1] - target[1], 0.0]);
    let cos = mathrt::cos(CORNER_ANGLE as f64) as f32;
    let c = if cos == 0.0 {
        1.0
    } else {
        let flat = away[1] * ahead[1];
        away[2].mul_add(ahead[2], away[0].mul_add(ahead[0], flat)) / cos
    };
    let span = asked - base;
    let c = clamp01(c);
    let p = (c * c).mul_add(span, asked - span);
    let p = if asked - p >= 0.0 { p } else { asked };
    if CORNER_FLOOR - p >= 0.0 { CORNER_FLOOR } else { p }
}

pub fn slowed(ratio: f32, slow: f32, max: f32, d2: f32) -> f32 {
    if !(slow > 0.0) {
        return ratio;
    }
    let to = if d2 < slow * slow {
        clamp01(d2.sqrt() / slow) + 1.0
    } else if d2 < max * max {
        clamp01((d2.sqrt() - slow) / (max - slow)) + 2.0
    } else {
        3.0
    };
    if to - ratio >= 0.0 { ratio } else { to }
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn len2(d: Point) -> f32 {
    d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2]))
}

pub fn join(route: &Route, ped: Point) -> i32 {
    let pts = route.points();
    let n = pts.len();
    let mut best = -1;
    let mut near = f32::MAX;
    for i in 0..n {
        let (a, b) = (pts[i % n], pts[(i + 1) % n]);
        let (d, w) = (sub(b, a), sub(ped, a));
        let dir = unit(d);
        let t = w[0].mul_add(dir[0], w[1].mul_add(dir[1], w[2] * dir[2]));
        if !(t > 0.0) || !(t < len2(d).sqrt()) {
            continue;
        }
        let foot: Point = std::array::from_fn(|l| (dir[l] as f64 * t as f64 + a[l] as f64) as f32);
        let off = len2(sub(ped, foot));
        if off < near * near {
            (best, near) = (((i + 1) % n) as i32, off);
        }
    }
    if best == -1 {
        let mut near = f32::MAX;
        for (i, p) in pts.iter().enumerate() {
            let d2 = len2(sub(ped, *p));
            if d2 < near * near {
                (best, near) = (i as i32, d2);
            }
        }
    }
    best
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Arrive {
    pub radius: f32,
    pub tolerance: f32,
    pub past: bool,
    pub held: bool,
    sides: u32,
    prev: Point,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    pub pos: Point,
    pub vel: Point,
    pub dt: f32,
    pub blend: f32,
    pub blocked: bool,
    pub floor: bool,
}

impl Arrive {
    pub fn new(radius: f32, past: bool) -> Arrive {
        Arrive { radius, tolerance: TOLERANCE, past, ..Default::default() }
    }

    pub fn set_state(&mut self, sides: u32, prev: Point) {
        (self.sides, self.prev) = (sides & 0xF, prev);
    }

    pub fn state(&self) -> (u32, Point) {
        (self.sides, self.prev)
    }

    fn crossed(&mut self, pos: Point, target: Point) -> bool {
        if pos[0] < target[0] {
            self.sides |= 1;
        } else if pos[0] > target[0] {
            self.sides |= 2;
        }
        if pos[1] < target[1] {
            self.sides |= 4;
        } else if pos[1] > target[1] {
            self.sides |= 8;
        }
        (pos[2] - target[2]).abs() < self.tolerance && self.sides == 0xF
    }

    pub fn test(&mut self, look: &Look, target: Point) -> bool {
        let pos = look.pos;
        if self.crossed(pos, target) && !self.held && !look.blocked {
            return true;
        }
        let d = sub(target, pos);
        let dz = (pos[2] - target[2]).abs();
        let reach = self.radius + look.blend;
        let within = d[1].mul_add(d[1], d[0] * d[0]) < reach * reach;
        let to = sub(target, self.prev);
        let passed = to[1].mul_add(d[1], to[0] * d[0]) < 0.0;
        self.prev = pos;
        let tol = if look.floor && !(self.tolerance - 4.0 >= 0.0) { 4.0 } else { self.tolerance };
        if within && !self.held && dz < tol && (!self.past || passed) && !look.blocked {
            return true;
        }
        if self.past {
            return false;
        }
        let next: Point = std::array::from_fn(|l| (look.vel[l] as f64 * look.dt as f64 + pos[l] as f64) as f32);
        let e = sub(target, next);
        let behind = e[1].mul_add(d[1], e[0] * d[0]);
        !(behind > 0.0) && !self.held && !look.blocked
    }
}

pub const TOLERANCE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlowStep {
    pub sub: f32,
    pub task: Option<f32>,
    pub keep: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn slow_step(ratio: f32, slow: f32, ped: Point, target: Point, radius: f32, ran: bool, stopped: bool, blocked: bool) -> SlowStep {
    let max = if ratio == 3.0 { SLOW_MAX } else { RUN_MAX };
    let (dx, dy) = (ped[0] - target[0], ped[1] - target[1]);
    let d2 = dy.mul_add(dy, dx * dx);
    let sub = slowed(ratio, slow, max, d2);
    if !(d2 < radius * radius) {
        return SlowStep { sub, task: None, keep: true };
    }
    SlowStep { sub: 0.0, task: Some(0.0), keep: !(ran && stopped && !blocked) }
}

pub const SLOW_MAX: f32 = 10.0;
const RUN_MAX: f32 = 100000000.0;

#[derive(Clone, Debug)]
pub struct GoToPoint {
    kind: u32,
    target: Point,
    next: Option<Point>,
    smooth: bool,
    arrive: Arrive,
    mover: Mover,
}

impl GoToPoint {
    pub fn new(mbr: f32, radius: f32, target: Point, past: bool) -> GoToPoint {
        GoToPoint {
            kind: kind::MOVE_GO_TO_POINT,
            target,
            next: None,
            smooth: false,
            arrive: Arrive::new(radius, past),
            mover: Mover::new(mbr),
        }
    }

    pub fn on_route(mbr: f32, radius: f32, target: Point, next: Point, smooth: bool, past: bool) -> GoToPoint {
        GoToPoint { kind: kind::MOVE_GO_TO_POINT_ON_ROUTE, next: Some(next), smooth, ..GoToPoint::new(mbr, radius, target, past) }
    }

    pub fn set_kind(&mut self, kind: u32) {
        self.kind = kind;
    }

    pub fn after(&self) -> Option<(Point, bool)> {
        self.next.map(|n| (n, self.smooth))
    }
}

impl<P: Walker> Task<P> for GoToPoint {
    fn kind(&self) -> u32 {
        self.kind
    }

    fn process(&mut self, ped: &mut P) -> bool {
        let look = Look { pos: ped.here(), vel: ped.velocity(), dt: ped.dt(), blocked: ped.blocked(), ..Default::default() };
        if self.arrive.test(&look, self.target) {
            return true;
        }
        let asked = self.mover.mbr();
        let pace = match self.next {
            Some(next) if self.smooth => corner_pace(asked, look.pos, self.target, next, ped.pace_held()),
            _ => asked,
        };
        ped.order(Order::Go { target: self.target, pace });
        false
    }

    fn mover(&mut self) -> Option<&mut Mover> {
        Some(&mut self.mover)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MoveStand {
    ms: i32,
    hold: bool,
    timer: Timer,
    mover: Mover,
}

impl MoveStand {
    pub fn new(ms: i32, hold: bool) -> MoveStand {
        MoveStand { ms, hold, timer: Timer::default(), mover: Mover::new(0.0) }
    }
}

impl<P: Walker> Task<P> for MoveStand {
    fn kind(&self) -> u32 {
        kind::MOVE_STAND_STILL
    }

    fn process(&mut self, ped: &mut P) -> bool {
        ped.order(Order::Stand);
        if !self.timer.set && self.ms >= 0 {
            self.timer = Timer::start(ped.now(), self.ms);
        }
        !self.hold && self.timer.done(ped.now())
    }

    fn mover(&mut self) -> Option<&mut Mover> {
        Some(&mut self.mover)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MoveNone {
    mover: Mover,
}

impl<P: Walker> Task<P> for MoveNone {
    fn kind(&self) -> u32 {
        kind::MOVE_NONE
    }

    fn mover(&mut self) -> Option<&mut Mover> {
        Some(&mut self.mover)
    }
}

#[derive(Clone, Debug)]
pub struct GoToPointAndStandStill {
    target: Point,
    radius: f32,
    pub slow: f32,
    mover: Mover,
    ran: bool,
    settled: bool,
}

impl GoToPointAndStandStill {
    pub fn new(mbr: f32, radius: f32, slow: f32, target: Point) -> Self {
        GoToPointAndStandStill { target, radius, slow, mover: Mover::new(mbr), ran: false, settled: false }
    }
}

impl<P: Walker> Task<P> for GoToPointAndStandStill {
    fn kind(&self) -> u32 {
        kind::MOVE_GO_TO_POINT_AND_STAND_STILL
    }

    fn simple(&self) -> bool {
        false
    }

    fn first(&mut self, ped: &mut P) -> Option<Box<dyn Task<P>>> {
        if ped.in_vehicle() {
            return None;
        }
        (self.ran, self.settled) = (false, false);
        Some(Box::new(GoToPoint::new(self.mover.mbr(), self.radius, self.target, false)))
    }

    fn next(&mut self, _: &mut P, done: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        (self.ran, self.settled) = (false, false);
        (done.kind() == kind::MOVE_GO_TO_POINT).then(|| Box::new(MoveStand::new(1, false)) as Box<dyn Task<P>>)
    }

    fn tune(&mut self, ped: &mut P, sub: &mut dyn Task<P>) {
        let ran = std::mem::replace(&mut self.ran, true);
        self.settled = false;
        if sub.kind() != kind::MOVE_GO_TO_POINT {
            return;
        }
        let ratio = self.mover.mbr();
        if !(ratio >= 1.0) {
            if let Some(m) = sub.mover() {
                m.store(ratio);
            }
            return;
        }
        let step = slow_step(ratio, self.slow, ped.here(), self.target, self.radius, ran, ped.stopped(), ped.blocked());
        if let Some(m) = sub.mover() {
            m.store(step.sub);
        }
        if let Some(t) = step.task {
            self.mover.store(t);
        }
        self.settled = !step.keep;
    }

    fn control(&mut self, _: &mut P, sub: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        if std::mem::take(&mut self.settled) && sub.kind() == kind::MOVE_GO_TO_POINT {
            self.ran = false;
            return Some(Box::new(MoveStand::new(1, false)));
        }
        None
    }

    fn mover(&mut self) -> Option<&mut Mover> {
        Some(&mut self.mover)
    }
}

#[derive(Debug)]
pub struct FollowPointRoute {
    mode: i32,
    counter: i32,
    radius: f32,
    last_radius: f32,
    slow: f32,
    route: Route,
    index: i32,
    past: bool,
    smooth: bool,
    stand: bool,
    changed: bool,
    mover: Mover,
}

impl Clone for FollowPointRoute {
    fn clone(&self) -> Self {
        FollowPointRoute { route: self.route.clone(), counter: 0, changed: true, ..*self }
    }
}

pub const ONCE: i32 = 0;
pub const BACK_ONCE: i32 = 1;
pub const BACK_AND_FORTH: i32 = 2;
pub const LOOP: i32 = 3;

#[derive(Clone, Copy, Debug)]
pub struct Course {
    pub arrive: f32,
    pub slow: f32,
    pub radius: f32,
    pub past: bool,
    pub smooth: bool,
    pub stand: bool,
    pub index: i32,
}

impl Course {
    pub const SCRIPT: Course = Course { arrive: ARRIVE, slow: SLOW, radius: -1.0, past: false, smooth: true, stand: true, index: 0 };
}

impl FollowPointRoute {
    pub fn new(state: u32, route: &Route, mode: i32, c: Course) -> FollowPointRoute {
        FollowPointRoute::with_ratio(crate::movement::mbr_for_state(state), route, mode, c)
    }

    pub fn with_ratio(mbr: f32, route: &Route, mode: i32, c: Course) -> FollowPointRoute {
        FollowPointRoute {
            mode,
            counter: 0,
            radius: if c.radius >= 0.0 { c.radius } else { 0.5 },
            last_radius: c.arrive,
            slow: c.slow,
            route: route.clone(),
            index: c.index,
            past: c.past,
            smooth: c.smooth,
            stand: c.stand,
            changed: true,
            mover: Mover::new(mbr),
        }
    }

    pub fn set_route(&mut self, route: &Route, arrive: f32, slow: f32, force: bool) {
        if force || !(self.route.same(route) && self.last_radius == arrive && self.slow == slow) {
            self.route = route.clone();
            (self.last_radius, self.slow, self.index, self.counter, self.changed) = (arrive, slow, 0, 0, true);
        }
    }

    pub fn route(&self) -> &Route {
        &self.route
    }

    pub fn index(&self) -> i32 {
        self.index
    }

    pub fn counter(&self) -> i32 {
        self.counter
    }

    pub fn with_state(mut self, index: i32, counter: i32) -> Self {
        (self.index, self.counter) = (index, counter);
        self
    }

    pub fn after(&mut self, done: u32) -> Option<u32> {
        if matches!(done, LEG_END | kind::MOVE_NONE | kind::MOVE_STAND_STILL) {
            return None;
        }
        if done == kind::MOVE_GO_TO_POINT && self.index + 1 == self.route.len() as i32 && !self.stand {
            return None;
        }
        self.index += 1;
        Some(self.leg_type())
    }

    pub fn leg_type(&mut self) -> u32 {
        let n = self.route.len() as i32;
        if n == 0 {
            return LEG_END;
        }
        let i = self.index;
        if i + 1 < n {
            return kind::MOVE_GO_TO_POINT_ON_ROUTE;
        }
        if i + 1 == n {
            return kind::MOVE_GO_TO_POINT + if self.stand { 3 } else { 0 };
        }
        if i == n {
            self.counter += 1;
            return match self.mode {
                ONCE => LEG_END,
                BACK_ONCE if self.counter != 1 => LEG_END,
                BACK_ONCE | BACK_AND_FORTH => {
                    self.route.reverse();
                    self.index = 0;
                    self.leg_type()
                }
                LOOP => {
                    self.index = 0;
                    self.leg_type()
                }
                _ => kind::MOVE_NONE,
            };
        }
        kind::MOVE_NONE
    }

    fn leg<P: Walker>(&self, ty: u32) -> Option<Box<dyn Task<P>>> {
        let at = |k: i32| self.route.points()[k as usize];
        let mbr = self.mover.mbr();
        match ty {
            kind::MOVE_GO_TO_POINT_ON_ROUTE => {
                Some(Box::new(GoToPoint::on_route(mbr, self.radius, at(self.index), at(self.index + 1), self.smooth, self.past)))
            }
            kind::MOVE_GO_TO_POINT => Some(Box::new(GoToPoint::new(mbr, self.radius, at(self.index), self.past))),
            kind::MOVE_GO_TO_POINT_AND_STAND_STILL => {
                let slow = match self.mode {
                    BACK_ONCE if self.counter == 0 => 0.0,
                    BACK_AND_FORTH | LOOP => 0.0,
                    _ => self.slow,
                };
                Some(Box::new(GoToPointAndStandStill::new(mbr, self.last_radius, slow, at(self.index))))
            }
            kind::MOVE_STAND_STILL => Some(Box::new(MoveStand::new(STAND_MS, false))),
            kind::MOVE_NONE => Some(Box::new(MoveNone::default())),
            _ => None,
        }
    }
}

const STAND_MS: i32 = 20000;

impl<P: Walker> Task<P> for FollowPointRoute {
    fn kind(&self) -> u32 {
        kind::FOLLOW_POINT_ROUTE
    }

    fn simple(&self) -> bool {
        false
    }

    fn make_abortable(&mut self, _: &mut P, urgency: u8) -> bool {
        if urgency == ABORT_LEAVE {
            self.route.flush();
        }
        true
    }

    fn first(&mut self, ped: &mut P) -> Option<Box<dyn Task<P>>> {
        self.changed = false;
        if self.route.len() as i32 == self.index {
            return Some(Box::new(MoveNone::default()));
        }
        if ped.in_vehicle() {
            return None;
        }
        if self.mode != ONCE {
            self.index = join(&self.route, ped.here());
        }
        let ty = self.leg_type();
        self.leg(ty)
    }

    fn next(&mut self, _: &mut P, done: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        let ty = self.after(done.kind())?;
        self.leg(ty)
    }

    fn control(&mut self, ped: &mut P, _: &dyn Task<P>) -> Option<Box<dyn Task<P>>> {
        if ped.bumps() > MAX_BUMPS || self.route.is_empty() {
            return Some(Box::new(End));
        }
        if self.changed {
            return Some(self.first(ped).unwrap_or_else(|| Box::new(End)));
        }
        None
    }

    fn mover(&mut self) -> Option<&mut Mover> {
        Some(&mut self.mover)
    }
}

