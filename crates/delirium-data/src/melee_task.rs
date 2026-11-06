use crate::mathrt;
use crate::rng::Rand;

#[allow(non_snake_case)]
#[inline]
pub fn KIND() -> u32 { crate::kv!(927 int) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Fight,
    Seek,
    Route,
    Quit,
}

#[allow(non_snake_case)]
#[inline]
pub fn SEEK_BEYOND() -> f32 { crate::kv!(504) }
#[allow(non_snake_case)]
#[inline]
pub fn SEEK_NEAR() -> f32 { crate::kv!(928) }
#[allow(non_snake_case)]
#[inline]
pub fn ROUTE_WITHIN() -> f32 { crate::kv!(12) }
#[allow(non_snake_case)]
#[inline]
pub fn ROUTE_PAST() -> f32 { crate::kv!(64) }

#[allow(non_snake_case)]
#[inline]
pub fn SEEK_STAGES() -> &'static [(f32, f32); 3] { static V: std::sync::OnceLock<[(f32, f32); 3]> = std::sync::OnceLock::new(); let f = || [(SEEK_NEAR(), crate::kv!(128)), (crate::kv!(9), crate::kv!(140)), (crate::kv!(28), crate::kv!(80))]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn SEEK_LOOK_MS() -> u32 { crate::kv!(43 int) }
#[allow(non_snake_case)]
#[inline]
pub fn SEEK_FIRST_LOOK_MS() -> u32 { crate::kv!(40 int) }
#[allow(non_snake_case)]
#[inline]
fn AWAY_BEYOND() -> f32 { crate::kv!(33) }
#[allow(non_snake_case)]
#[inline]
fn AWAY_WITHIN() -> f32 { crate::kv!(80) }

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeekStage {
    pub stage: u32,
    pub used: u32,
    timer: Option<(u32, u32)>,
    out: Option<(bool, bool)>,
    pub changed: bool,
}

impl SeekStage {
    pub fn start(now: u32) -> Self {
        SeekStage { timer: Some((now, SEEK_FIRST_LOOK_MS())), ..Default::default() }
    }

    pub fn stop(&self) -> (f32, f32) {
        (*SEEK_STAGES()).get(self.stage as usize).copied().unwrap_or((0.0, 0.0))
    }

    pub fn update(&mut self, now: u32, look: impl FnOnce() -> (bool, bool), away: bool, dist2: f32) {
        let (mut blocked, mut off) = (false, false);
        if self.out.is_none() && (self.timer.is_none_or(|(t, len)| now.wrapping_sub(t) >= len) || self.used == 1) {
            self.out = Some(look());
        } else if let Some((chest, knee)) = self.out.take() {
            self.timer = Some((now, SEEK_LOOK_MS()));
            if !(chest && knee) {
                blocked = true;
                self.stage = 1;
            } else if self.used == 1 {
                self.stage = 0;
            }
        }
        if away && dist2 > AWAY_BEYOND() * AWAY_BEYOND() {
            off = true;
            self.stage = 2;
        }
        if self.used == 2 && (!away || dist2 < AWAY_WITHIN() * AWAY_WITHIN()) {
            self.stage = 0;
        }
        self.changed = blocked || off;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn step(target_in_vehicle: bool, target_boarding: bool, ped: [f32; 3], target: [f32; 3], moving: bool, waiting: bool,
            changed: bool, area: Option<[[f32; 3]; 2]>) -> Step {
    if target_in_vehicle || target_boarding {
        return Step::Quit;
    }
    let inside = |p: [f32; 3]| area.is_none_or(|[lo, hi]| (0..crate::kv!(0 int)).all(|i| !(lo[i] > p[i]) && !(p[i] > hi[i])));
    let d = [target[0] - ped[0], target[1] - ped[1], target[2] - ped[2]];
    let d2 = d[2].mul_add(d[2], d[1].mul_add(d[1], d[0] * d[0]));
    if inside(ped) && waiting && moving && d2 < ROUTE_WITHIN() * ROUTE_WITHIN() {
        Step::Route
    } else if inside(target) && (d2 > SEEK_BEYOND() * SEEK_BEYOND() || changed) {
        Step::Seek
    } else {
        Step::Fight
    }
}

#[allow(non_snake_case)]
#[inline]
fn NEAR() -> f32 { crate::kv!(124) }
#[allow(non_snake_case)]
#[inline]
fn FAR() -> f32 { crate::kv!(5) }
#[allow(non_snake_case)]
#[inline]
fn POWER() -> f32 { crate::kv!(32) }
#[allow(non_snake_case)]
#[inline]
fn FLOOR() -> f32 { crate::kv!(51) }
#[allow(non_snake_case)]
#[inline]
fn TOP() -> f32 { crate::kv!(18) }
#[allow(non_snake_case)]
#[inline]
fn PLAYING() -> f32 { crate::kv!(200) }
#[allow(non_snake_case)]
#[inline]
fn WAIT_MS() -> (i32, i32) { (crate::kv!(43 int), crate::kv!(929 int)) }

pub fn chance(dist: f32, playing: bool) -> f32 {
    if playing {
        return PLAYING();
    }
    let t = ((dist - NEAR()) / (FAR() - NEAR())).clamp(0.0, 1.0);
    (TOP() * mathrt::pow((1.0 - t) as f64, POWER() as f64) as f32).clamp(FLOOR(), TOP())
}

pub fn wants_move(left: Option<f32>, dist: f32, playing: bool, roll: f32, dt: f32) -> bool {
    left.is_some_and(|l| l <= 0.0) || roll < 1.0 - (1.0 - chance(dist, playing)).powf(crate::kv!(27) * dt)
}

pub fn wait(rng: &mut Rand) -> f32 {
    rng.int_in_range(WAIT_MS().0, WAIT_MS().1) as f32 / crate::kv!(82)
}

#[allow(non_snake_case)]
#[inline]
pub fn TURN_SPEED() -> f32 { crate::kv!(930) }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keep {
    pub keep: f32,
    pub dead: f32,
    pub far: f32,
    pub far_pow: f32,
    pub close_pow: f32,
    pub least: f32,
}

#[allow(non_snake_case)]
#[inline]
pub fn KEEP_WAITING() -> &'static Keep { static V: std::sync::OnceLock<Keep> = std::sync::OnceLock::new(); let f = || Keep { keep: crate::kv!(33), dead: crate::kv!(60), far: crate::kv!(5), far_pow: 2.0, close_pow: 1.0, least: crate::kv!(78) }; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn KEEP_FIGHTING() -> &'static Keep { static V: std::sync::OnceLock<Keep> = std::sync::OnceLock::new(); let f = || Keep { keep: 1.0, dead: crate::kv!(60), far: crate::kv!(12), ..(*KEEP_WAITING()) }; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn KEEP_OFF() -> &'static Keep { static V: std::sync::OnceLock<Keep> = std::sync::OnceLock::new(); let f = || Keep { keep: crate::kv!(491), dead: crate::kv!(60), far: crate::kv!(5), far_pow: 2.0, close_pow: 1.0, least: crate::kv!(78) }; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn CIRCLE() -> f32 { crate::kv!(17) }
const STANCE_RATIO: f32 = 2.0;

#[allow(clippy::too_many_arguments)]
pub fn keep_distance(k: &Keep, no_closer: bool, ped: [f32; 3], right: [f32; 3], forward: [f32; 3], target: [f32; 3],
                     area: Option<[[f32; 3]; 2]>) -> (f32, f32) {
    let d = [target[0] - ped[0], target[1] - ped[1], target[2] - ped[2]];
    let len = d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2])).sqrt();
    let e = k.keep - len;
    let a = e.abs().min(k.far);
    let speed = if a < k.dead {
        0.0
    } else {
        let p = mathrt::pow(((a - k.dead) / (k.far - k.dead)) as f64, if e < 0.0 { k.far_pow } else { k.close_pow } as f64) as f32;
        let s = -(e.signum() * p);
        if s == 0.0 { 0.0 } else if s.abs() < k.least { s.signum() * k.least } else { s }
    };
    let speed = if no_closer && speed > 0.0 { 0.0 } else { speed };
    let speed = match area {
        Some([lo, hi]) => {
            let inside = |p: [f32; 3]| (0..crate::kv!(0 int)).all(|i| !(lo[i] > p[i]) && !(p[i] > hi[i]));
            let (toward, away) = (inside(target), inside(ped));
            if (!toward && speed > 0.0) || (!away && speed < 0.0) { 0.0 } else { speed }
        }
        None => speed,
    };
    let way = crate::melee::vunit(d);
    let dot = |r: [f32; 3]| r[0].mul_add(way[0], r[1].mul_add(way[1], r[2] * way[2]));
    (dot(right) * speed, dot(forward) * speed)
}

#[allow(clippy::too_many_arguments)]
pub fn stance(ped: [f32; 3], right: [f32; 3], forward: [f32; 3], target: [f32; 3], area: Option<[[f32; 3]; 2]>,
              waiting: bool, second: Option<[f32; 3]>) -> (f32, f32) {
    let (mut s, mut f) = keep_distance(if waiting { &(*KEEP_WAITING()) } else { &(*KEEP_FIGHTING()) }, false, ped, right, forward, target, area);
    if let Some(o) = second {
        let (mut s2, f2) = keep_distance(&(*KEEP_OFF()), true, ped, right, forward, o, area);
        let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let len2 = |v: [f32; 3]| v[2].mul_add(v[2], v[1].mul_add(v[1], v[0] * v[0]));
        let (theirs, ours) = (sub(target, o), sub(target, ped));
        if len2(theirs) < len2(ours) {
            let (a, b) = (crate::melee::vunit(theirs), crate::melee::vunit(ours));
            let dot = a[2].mul_add(b[2], a[0].mul_add(b[0], a[1] * b[1]));
            let z = (-a[0]).mul_add(b[1], a[1] * b[0]);
            let sign = if z > 0.0 { 1.0 } else if z == 0.0 { 0.0 } else { -1.0 };
            s2 -= dot * sign * CIRCLE();
        }
        (s, f) = (s + s2, f + f2);
    }
    let (mut s, mut f) = (s.clamp(-1.0, 1.0), f.clamp(-1.0, 1.0));
    let l2 = f.mul_add(f, s * s);
    if l2 > 1.0 {
        let k = 1.0 / l2.sqrt();
        (s, f) = (s * k, f * k);
    }
    if f.mul_add(f, s * s) > 0.0 {
        (s, f) = (s * STANCE_RATIO, f * STANCE_RATIO);
    }
    (s, f)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rival {
    pub d2: f32,
    pub waiting: bool,
    pub gang: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turns {
    pub waiting: bool,
    pub second: Option<usize>,
    pub relieve: Option<usize>,
}

pub fn take_turns(waiting: bool, gang: bool, d2: f32, rivals: &[Rival]) -> Turns {
    if rivals.is_empty() {
        return Turns { waiting: false, second: None, relieve: None };
    }
    let (mut near, mut near_d2, mut far, mut far_d2) = (None, f32::MAX, None, 0.0);
    for (i, r) in rivals.iter().enumerate() {
        if r.d2 < near_d2 {
            (near, near_d2) = (Some(i), r.d2);
        }
        if r.d2 > far_d2 {
            (far, far_d2) = (Some(i), r.d2);
        }
    }
    let fighting = rivals.iter().filter(|r| !r.waiting).count();
    let limit = if gang { 2 } else { 1 };
    let mut t = Turns { waiting, second: near, relieve: None };
    if fighting < limit {
        t.waiting = false;
    } else if d2 < near_d2 && !gang && !near.is_some_and(|i| rivals[i].gang) {
        (t.waiting, t.relieve) = (false, far);
    }
    t
}

pub fn request_way(start: [f32; 3], hit: [f32; 3]) -> [f32; 3] {
    crate::melee::vunit([start[0] - hit[0], start[1] - hit[1], start[2] - hit[2]])
}

pub fn request_takes(playing: Option<i32>, priority: i32) -> bool {
    playing.is_none_or(|p| priority as u32 >= p as u32)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drift {
    pub way: [f32; 2],
    pub momentum: f32,
    pub left: f32,
}

#[allow(non_snake_case)]
#[inline]
fn DRIFT_LEAST() -> f32 { crate::kv!(78) }
const DRIFT_RATIO: f32 = 2.0;
const DRIFT_FALL: f32 = 1.0;

impl Drift {
    pub fn start(way: [f32; 3], received: bool, blocking: bool, momentum: f32) -> Option<Drift> {
        let len = way[2].mul_add(way[2], way[0].mul_add(way[0], way[1] * way[1])).sqrt();
        (received && blocking && len > DRIFT_LEAST()).then_some(Drift { way: [way[0], way[1]], momentum, left: momentum })
    }

    pub fn ratios(&self, right: [f32; 3], forward: [f32; 3]) -> Option<(f32, f32)> {
        if !(self.left > 0.0) {
            return None;
        }
        let w = crate::melee::vunit([self.way[0], self.way[1], 0.0]);
        if !(w[0].mul_add(w[0], w[1].mul_add(w[1], w[2] * w[2])).sqrt() > 0.0) {
            return Some((0.0, 0.0));
        }
        let k = self.momentum * DRIFT_RATIO;
        let dot = |r: [f32; 3]| r[1].mul_add(w[1], r[0] * w[0]) + w[2] * 0.0;
        Some((dot(right) * k, -dot(forward) * k))
    }

    pub fn step(&mut self, dt: f32) {
        self.left = (self.left - dt * DRIFT_FALL).max(0.0);
    }
}

