use crate::rng::Rand;

pub const KIND: u32 = 431;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Fight,
    Seek,
    Route,
    Quit,
}

pub const SEEK_BEYOND: f32 = 5.5;
pub const SEEK_NEAR: f32 = 3.3;
pub const ROUTE_WITHIN: f32 = 1.5;
pub const ROUTE_PAST: f32 = 5.0;

pub fn step(target_in_vehicle: bool, target_boarding: bool, dist: f32, moving: bool) -> Step {
    if target_in_vehicle || target_boarding {
        Step::Quit
    } else if moving && dist * dist < ROUTE_WITHIN * ROUTE_WITHIN {
        Step::Route
    } else if dist * dist > SEEK_BEYOND * SEEK_BEYOND {
        Step::Seek
    } else {
        Step::Fight
    }
}

const NEAR: f32 = 0.85;
const FAR: f32 = 3.0;
const POWER: f32 = 7.0;
const FLOOR: f32 = 0.01;
const TOP: f32 = 0.75;
const PLAYING: f32 = 0.28;
const WAIT_MS: (i32, i32) = (2000, 7000);

pub fn chance(dist: f32, playing: bool) -> f32 {
    if playing {
        return PLAYING;
    }
    let t = ((dist - NEAR) / (FAR - NEAR)).clamp(0.0, 1.0);
    (TOP * (1.0 - t).powf(POWER)).clamp(FLOOR, TOP)
}

pub fn wants_move(left: Option<f32>, dist: f32, playing: bool, roll: f32) -> bool {
    left.is_some_and(|l| l <= 0.0) || roll < chance(dist, playing)
}

pub fn wait(rng: &mut Rand) -> f32 {
    rng.int_in_range(WAIT_MS.0, WAIT_MS.1) as f32 / 1000.0
}

pub const KEEP: f32 = 0.707;
pub const DEAD: f32 = 0.6;
pub const KEEP_FAR: f32 = 2.5;
pub const TURN_SPEED: f32 = 22.0;
const CLOSE_POW: f32 = 1.0;
const FAR_POW: f32 = 2.0;
const LEAST: f32 = 0.1;
const STANCE_RATIO: f32 = 2.0;

pub fn keep_distance(ped: [f32; 3], right: [f32; 3], forward: [f32; 3], target: [f32; 3]) -> (f32, f32) {
    let d = [target[0] - ped[0], target[1] - ped[1], target[2] - ped[2]];
    let len = d[2].mul_add(d[2], d[1].mul_add(d[1], d[0] * d[0])).sqrt();
    let e = KEEP - len;
    let a = e.abs().min(KEEP_FAR);
    let speed = if a < DEAD {
        0.0
    } else {
        let p = ((a - DEAD) / (KEEP_FAR - DEAD)).powf(if e < 0.0 { FAR_POW } else { CLOSE_POW });
        let s = -(e.signum() * p);
        if s.abs() < LEAST { s.signum() * LEAST } else { s }
    };
    let way = if len > 0.0 { d.map(|v| v / len) } else { [0.0; 3] };
    let dot = |r: [f32; 3]| r[2].mul_add(way[2], r[1].mul_add(way[1], r[0] * way[0]));
    (dot(right) * speed, dot(forward) * speed)
}

pub fn stance(ped: [f32; 3], right: [f32; 3], forward: [f32; 3], target: [f32; 3]) -> (f32, f32) {
    let (s, f) = keep_distance(ped, right, forward, target);
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

