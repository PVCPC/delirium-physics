use crate::prelude::*;
use crate::msg::Value;
use crate::nm::Agent;
use crate::shottask::Msg;
use delirium_data::rng::Rand;

const PUSH: f32 = 0.02;

pub fn stick_axis(raw: u8) -> i32 {
    let v = ((raw as f32 - 127.5) * 0.007843138).clamp(-1.0, 1.0);
    let v = if v > 0.25 { (v - 0.25) / (1.0 - 0.25) } else if v < -0.25 { (v + 0.25) / (1.0 - 0.25) } else { 0.0 };
    (v * 127.5) as i32
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Wander {
    x: f32,
    y: f32,
    frames: i32,
}

fn wrap(mut h: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    h = h.clamp(-25.0, 25.0);
    while h > PI {
        h -= TAU;
    }
    while h < -PI {
        h += TAU;
    }
    h
}

pub fn control(a: &mut Agent, stick: (i32, i32), camera: f32, ped: Vec3, wander: &mut Wander, rand: &mut Rand) -> Vec<Msg> {
    let (force, msg) = frame(stick, camera, ped, a.doll.mass(), wander, rand);
    a.push.push((0, ped, force));
    vec![msg]
}

pub fn frame(stick: (i32, i32), camera: f32, ped: Vec3, mass: f32, wander: &mut Wander, rand: &mut Rand) -> (Vec3, Msg) {
    let (mut x, mut y) = (stick.0 as f32, -(stick.1 as f32));
    if y.mul_add(y, x * x) <= 0.0 {
        if wander.frames <= 0 {
            wander.x = rand.float_in_range(-8.0, 8.0);
            wander.y = rand.float_in_range(-64.0, 64.0);
            wander.frames = rand.int_in_range(10, 120);
        }
        wander.frames -= 1;
        (x, y) = (wander.x, wander.y);
    }
    let h = wrap(((-x as f64).atan2(y as f64) as f32) + camera) as f64;
    let dir = Vec3::new(-(h.sin() as f32), h.cos() as f32, 0.0);
    let look = dir * 3.0;
    let push = y.mul_add(y, x * x).sqrt() * PUSH * mass;
    use Value::{Bool as B, Vec3 as V};
    (dir * push, ("bodyBalance", vec![("useBodyTurn", B(true)), ("useHeadLook", B(true)), ("headLookPos", V(ped + look))]))
}

