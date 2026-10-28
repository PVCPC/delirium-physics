use bevy::prelude::*;

use super::Frame;

const U: f32 = 0.0254;
const SNAP_THRESHOLD: f32 = 10.0;
const SPEED_BONUS: f32 = 2.0;
const HEIGHT_ADJUST: f32 = 128.0;
const GIMME_DOT: f32 = 0.9;
const AIR_ACCELERATE: f32 = 5.0;
const AIR_SPEED_CAP: f32 = 60.0;
const MAX_SPEED: f32 = 175.0;

fn air_accelerate(v: Vec3, dir: Vec3, wish_speed: f32, dt: f32) -> Vec3 {
    let add = wish_speed.min(AIR_SPEED_CAP) - v.dot(dir);
    if add <= 0.0 {
        return v;
    }
    v + dir * (AIR_ACCELERATE * wish_speed * dt).min(add)
}

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

pub fn exit_gimme(exit: &Frame, velocity: Vec3) -> Vec3 {
    let len = velocity.length();
    if exit.normal().z.abs() > 0.999 && len > 0.0 && velocity.z.abs() / len > GIMME_DOT {
        Vec3::new(velocity.x, velocity.y, 0.0)
    } else {
        Vec3::ZERO
    }
}
