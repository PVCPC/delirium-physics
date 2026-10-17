use crate::prelude::*;
use crate::Fma;

pub const MOVING: f32 = 0.5625;
pub const MOVING_FAST: f32 = 100.0;
pub const HANDS: [u16; 2] = [1232, 1219];
pub const PROBE_RADIUS: f32 = 0.2;
pub const BALANCE_MS: [u32; 2] = [1000, 10000];
pub const LEAN: f32 = 2.5;
pub const LEAN_TIME: f32 = 1.0;

pub fn climbing(state: u32) -> bool {
    matches!(state, 2..=5) || state > 8
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Grab,
    Other,
}

pub fn moving_climb(kind: u32, status: i32, state: u32, vel: Vec3) -> Option<Next> {
    if !(2..5).contains(&kind) || !climbing(state) {
        return None;
    }
    let fast = kind == 2 && status == 2;
    if vel.z.fma(vel.z, vel.x.fma(vel.x, vel.y * vel.y)) <= if fast { MOVING_FAST } else { MOVING } {
        return None;
    }
    Some(if kind == 2 && status == 3 { Next::Other } else { Next::Grab })
}

pub fn in_frame(axes: [Vec3; 3], at: Vec3, p: Vec3, n: Vec3) -> (Vec3, Vec3) {
    let d = p - at;
    (Vec3::new(axes[0].dot(d), axes[1].dot(d), axes[2].dot(d)), Vec3::new(axes[0].dot(n), axes[1].dot(n), axes[2].dot(n)))
}

pub fn lean_dir(hand: Vec3, at: Vec3) -> Vec3 {
    use crate::Vd;
    let d = hand - at;
    let x = d.vd(d);
    let e = crate::estimate::vrsqrtefp(x);
    let e = if e.is_infinite() { 0.0 } else { e };
    d * (-x).mul_add(e * e, 1.0).mul_add(0.5 * e, e)
}

pub fn jump_grab(instance: i32, part: i32, points: &[(Vec3, Vec3)], rand: &mut delirium_data::rng::Rand, player: bool) -> crate::grabhelper::GrabHelper {
    let mut g = crate::grabhelper::GrabHelper::new();
    let p = |k: usize| points.get(k).map(|x| x.0);
    let n = |k: usize| points.get(k).map(|x| x.1);
    g.set_points(Some((instance, part)), [p(0), n(0), p(1), n(1), None, None]);
    g.strength = rand.float_in_range(80.0, 110.0);
    g.find_left = 1.0;
    if player {
        g.hold_left = 5.0;
    }
    g
}

pub fn door_dir(at: Vec3, hit: Option<Vec3>, forward: Vec3) -> Vec3 {
    hit.map_or(forward, |h| Vec3::new(h.x - at.x, h.y - at.y, 0.0).normalize_or_zero())
}

pub fn door_grab(instance: i32, part: i32, hit: Option<(Vec3, Vec3)>, rand: &mut delirium_data::rng::Rand, player: bool) -> crate::grabhelper::GrabHelper {
    jump_grab(instance, part, hit.as_slice(), rand, player)
}

