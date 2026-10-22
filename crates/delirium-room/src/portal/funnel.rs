//! Air funnelling into floor and ceiling portals: a port of `p2_movement/funnel.rs` from the Portal 2
//! Rust Engine (see CREDITS.md), for a falling ragdoll instead of the player. Portal 2 steers an
//! airborne player who is dropping toward (or rising at) a linked floor or ceiling portal so they go
//! in the opening instead of just missing it, and then drops their sideways speed as they leave it.
//!
//! The maths is Source's: units per second, Z up (this room's axes match), `sv_player_funnel_*` constants.
//! For a ragdoll the "eye forward" the engine tests is the direction it is travelling in, and the steering
//! is applied to every body's velocity alike (the centre of mass accelerates, the spin is left alone).
use bevy::prelude::*;

use super::Frame;

/// Metres per Source unit.
const U: f32 = 0.0254;
/// `sv_player_funnel_snap_threshold`: per-axis speed below which the funnel snaps the axis to zero (u/s).
const SNAP_THRESHOLD: f32 = 10.0;
/// `sv_player_funnel_speed_bonus`: steering boost when rising at full speed (1 to this factor).
const SPEED_BONUS: f32 = 2.0;
/// `sv_player_funnel_height_adjust`: height above the portal the funnel aims at (u).
const HEIGHT_ADJUST: f32 = 128.0;
/// `sv_player_funnel_gimme_dot`: minimum vertical share of the speed for the exit gimme.
const GIMME_DOT: f32 = 0.9;
/// Portal 2's air acceleration, speed cap and top wish speed (u/s).
const AIR_ACCELERATE: f32 = 5.0;
const AIR_SPEED_CAP: f32 = 60.0;
const MAX_SPEED: f32 = 175.0;

/// Source's `AirAccelerate`: the add-speed test uses `min(wish, cap)`, the step the full wish speed.
fn air_accelerate(v: Vec3, dir: Vec3, wish_speed: f32, dt: f32) -> Vec3 {
    let add = wish_speed.min(AIR_SPEED_CAP) - v.dot(dir);
    if add <= 0.0 {
        return v;
    }
    v + dir * (AIR_ACCELERATE * wish_speed * dt).min(add)
}

/// The velocity (m/s) a falling or rising body should have after one funnel step of `dt` seconds. `centre`
/// is its centre of mass (m), `velocity` its centre-of-mass velocity (m/s), `gravity` the world's (m/s^2).
/// Returns the velocity unchanged when no linked floor or ceiling portal catches it.
pub fn steer(portals: &[Frame], gravity: f32, centre: Vec3, velocity: Vec3, dt: f32) -> Vec3 {
    let mut v = velocity / U;
    if v.x * v.x + v.y * v.y > 300.0 * 300.0 {
        return velocity;
    }
    let fwd = v.normalize_or_zero();
    let g = gravity / U;
    let c = centre / U;

    let mut best: Option<(f32, Vec3)> = None;
    for p in portals {
        let (n, r, up) = (p.normal(), p.rot * Vec3::X, p.rot * Vec3::Y);
        let d = p.at / U - c;
        let vz = v.z;
        let ok = if vz <= 165.0 {
            fwd.z < -0.7 && vz < -165.0 && n.z > 0.8 && d.z >= -1024.0 && d.z < 0.0
        } else {
            n.z < -0.8 && d.z <= vz * vz / (2.0 * g) && d.z > 0.0 && d.z <= 1024.0
        };
        if !ok {
            continue;
        }
        let h = d.z.abs();
        let s = if h < 256.0 {
            1.5
        } else if h > 1024.0 {
            3.0
        } else {
            1.5 * (1.0 + (h - 256.0) / 768.0)
        };
        let r = (r - n * r.dot(n)).normalize_or_zero();
        let up = (up - n * up.dot(n)).normalize_or_zero();
        if d.dot(r).abs() > 32.0 * s || d.dot(up).abs() > 56.0 * s {
            continue;
        }
        if best.is_none_or(|(bd, _)| d.length() < bd) {
            best = Some((d.length(), d));
        }
    }
    let Some((_, d)) = best else { return velocity };

    // Time to reach 128 u above the portal centre: -g t^2 + 2 vz t - 2 (128 + d.z) = 0.
    let (a, b, cc) = (-g, 2.0 * v.z, -2.0 * (HEIGHT_ADJUST + d.z));
    let disc = b * b - 4.0 * a * cc;
    if disc < 0.0 {
        return velocity;
    }
    let sq = disc.sqrt();
    let Some(t) = [(-b + sq) / (2.0 * a), (-b - sq) / (2.0 * a)].into_iter().filter(|t| *t > 0.0).reduce(f32::min) else { return velocity };
    let extra = 1.0 + (v.z / 1065.0).clamp(0.0, 1.0) * (SPEED_BONUS - 1.0);

    let h = d.z.abs();
    let half_life = if h < 128.0 {
        0.01
    } else if h <= 1024.0 {
        0.01 + 0.14 * (h - 128.0) / 896.0
    } else {
        0.15
    };
    let decay = (-std::f32::consts::LN_2 / half_life * dt).exp();
    let mut funnel = Vec3::ZERO;
    for i in 0..2 {
        if v[i] == 0.0 || d[i] / v[i] >= t {
            funnel[i] = 6.0 * extra * d[i] - v[i];
        } else if v[i].abs() > SNAP_THRESHOLD {
            v[i] *= decay;
        } else {
            v[i] = 0.0;
        }
    }
    let len = funnel.length();
    if len > 1e-4 {
        v = air_accelerate(v, funnel / len, len.min(MAX_SPEED), dt);
    }
    v * U
}

/// The exit "gimme": out of an exactly horizontal portal, within about 26 degrees of vertical, drop all
/// horizontal speed. Returns the horizontal velocity to remove from every body (zero when it doesn't apply).
pub fn exit_gimme(exit: &Frame, velocity: Vec3) -> Vec3 {
    let len = velocity.length();
    if exit.normal().z.abs() > 0.999 && len > 0.0 && velocity.z.abs() / len > GIMME_DOT {
        Vec3::new(velocity.x, velocity.y, 0.0)
    } else {
        Vec3::ZERO
    }
}
