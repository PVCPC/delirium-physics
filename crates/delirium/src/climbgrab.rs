use crate::prelude::*;
use crate::Fma;

#[allow(non_snake_case)]
#[inline]
pub fn MOVING() -> f32 { delirium_data::kv!(54) }
#[allow(non_snake_case)]
#[inline]
pub fn MOVING_FAST() -> f32 { delirium_data::kv!(28) }
#[allow(non_snake_case)]
#[inline]
pub fn HANDS() -> &'static [u16; 2] { static V: std::sync::OnceLock<[u16; 2]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(55 int), delirium_data::kv!(56 int)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn PROBE_RADIUS() -> f32 { delirium_data::kv!(57) }
#[allow(non_snake_case)]
#[inline]
pub fn BALANCE_MS() -> &'static [u32; 2] { static V: std::sync::OnceLock<[u32; 2]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(38 int), delirium_data::kv!(58 int)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn LEAN() -> f32 { delirium_data::kv!(30) }
pub const LEAN_TIME: f32 = 1.0;

pub fn climbing(state: u32) -> bool {
    matches!(state, 2..=5) || state > delirium_data::kv!(23 int)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Grab,
    Other,
}

pub fn moving_climb(kind: u32, status: i32, state: u32, vel: Vec3) -> Option<Next> {
    if !(2..delirium_data::kv!(2 int)).contains(&kind) || !climbing(state) {
        return None;
    }
    let fast = kind == 2 && status == 2;
    if vel.z.fma(vel.z, vel.x.fma(vel.x, vel.y * vel.y)) <= if fast { MOVING_FAST() } else { MOVING() } {
        return None;
    }
    Some(if kind == 2 && status == delirium_data::kv!(0 int) { Next::Other } else { Next::Grab })
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
    g.strength = rand.float_in_range(delirium_data::kv!(59), delirium_data::kv!(43));
    g.find_left = 1.0;
    if player {
        g.hold_left = delirium_data::kv!(60);
    }
    g
}

pub fn door_dir(at: Vec3, hit: Option<Vec3>, forward: Vec3) -> Vec3 {
    hit.map_or(forward, |h| Vec3::new(h.x - at.x, h.y - at.y, 0.0).normalize_or_zero())
}

pub fn door_grab(instance: i32, part: i32, hit: Option<(Vec3, Vec3)>, rand: &mut delirium_data::rng::Rand, player: bool) -> crate::grabhelper::GrabHelper {
    jump_grab(instance, part, hit.as_slice(), rand, player)
}

