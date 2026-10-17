use crate::{mathrt, vehicles::Handling, wheelstep::{fsel, unit, TyreIn}};

pub const GRAVITY: f32 = 9.8;

pub mod flags {
    pub const GROUNDED: u32 = 0x1;
    pub const DRIVEN: u32 = 0x4;
    pub const POWERED: u32 = 0x8000;
    pub const CLAMP_SLIP: u32 = 0x80;
    pub const GRIP_115: u32 = 0x100;
    pub const GRIP_130: u32 = 0x200;
    pub const CAP_SPIN: u32 = 0x40;
    pub const CAP_SLIDE: u32 = 0x10_0000;
    pub const BURNOUT: u32 = 0x100_0000;
    pub const HEATS: u32 = 0x1000_0000;
    pub const LOCKED: u32 = 0x2000_0000;
    pub const NO_LOCK: u32 = 0x20_0000;
    pub const SAND: u32 = 0x800_0000;
    pub const SOFT: u32 = 0x400_0000;
}

#[derive(Clone, Debug)]
pub struct Tyre {
    pub front: bool,
    pub load_share: f32,
    pub grip: f32,
    pub wet: f32,
    pub resistance: f32,
    pub surface: f32,
    pub flags: u32,
    pub slip: f32,
    pub rate: f32,
    pub angle: f32,
    pub skid: [f32; 3],
    pub temp: f32,
    pub fire: Option<f32>,
}

impl Tyre {
    pub fn new(front: bool, load_share: f32) -> Self {
        Tyre { front, load_share, grip: 1.0, wet: 1.0, resistance: 0.0, surface: 1000.0, flags: 0, slip: 0.0, rate: 0.0,
              angle: 0.0, skid: [0.0; 3], temp: 0.0, fire: None }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Contact {
    pub v_long: f32,
    pub drive: f32,
    pub brake: f32,
}

pub fn curve(h: &Handling, x: f32, b: f32, p: f32) -> f32 {
    let x = x.abs();
    let mu = if x < p {
        x / p * h.traction_max
    } else if x < p * 2.5 {
        (h.traction_min - h.traction_max) * (x - p) / (p * 2.5) + h.traction_max
    } else {
        h.traction_min
    };
    mu * b * p
}

fn demands(t: &Tyre, c: &Contact, weight: f32, grounded: bool) -> (f32, f32) {
    let mut brake = c.brake * weight;
    if t.flags & flags::DRIVEN == 0 || t.resistance > 0.25 {
        brake += if grounded {
            t.resistance.max(0.05) * weight
        } else {
            (t.resistance * weight).max(if t.flags & flags::POWERED != 0 { 2.0 } else { 0.1 })
        };
    }
    let drive = c.drive * weight;
    if brake > drive.abs() {
        (0.0, brake)
    } else if drive > brake {
        (drive - brake, 0.0)
    } else {
        (drive + brake, 0.0)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spin(t: &mut Tyre, c: &Contact, grounded: bool, mass: f32, radius: f32, limit: f32, dir: f32, dt: f32) -> bool {
    let old = t.rate;
    let mut release = false;
    let cool = |temp: f32| temp - (if temp > 30.0 { 50.0 } else { 6.0 }) * dt;
    if grounded {
        let roll = -c.v_long / radius;
        let s = t.slip * dir;
        t.temp = if s < -0.9 && c.drive * dir > 0.0 {
            if t.flags & flags::HEATS != 0 && t.surface > 0.0 { t.temp + 3.0 * dt } else { t.temp }
        } else if s > 0.9 {
            t.temp
        } else {
            cool(t.temp)
        }
        .clamp(0.0, 70.0);
        t.rate = if s < -0.9 && c.drive * dir > 0.0 {
            let up = old - c.drive * dt * 100.0;
            let r = if dir > 0.0 { roll.min(up) } else { roll.max(up) };
            if (dir > 0.0 && r < limit) || (dir < 0.0 && r > limit) { limit * 1.1 } else { r }
        } else if s > 0.9 {
            release = t.flags & flags::NO_LOCK != 0 && c.v_long.abs() > 5.0 && t.resistance < 0.25 && s > 1.5;
            (1.0 - s.min(1.0)) * roll
        } else {
            (1.0 - 0.1 * s) * roll
        };
    } else {
        let (drive, brake) = demands(t, c, mass * GRAVITY, false);
        t.rate = if drive != 0.0 {
            let r = old - drive * dt * 150.0;
            if limit > 0.0 { r.min(limit) } else if limit < 0.0 { r.max(limit) } else { r }
        } else if brake > 0.0 {
            let d = brake * dt * 10.0;
            if d > old.abs() { 0.0 } else { old - d * old.signum() }
        } else {
            old
        };
        t.slip = 0.0;
        t.temp = cool(t.temp).max(0.0);
    }
    t.angle += t.rate * dt;
    if t.angle > std::f32::consts::PI {
        t.angle -= std::f32::consts::TAU;
    } else if t.angle < -std::f32::consts::PI {
        t.angle += std::f32::consts::TAU;
    }
    release
}

#[derive(Clone, Debug)]
pub struct Random(u64);

impl Random {
    pub fn new(seed: u32) -> Self {
        Random(seed as u64 | 1 << 32)
    }

    pub fn next(&mut self) -> f32 {
        (self.bits() & 0x7F_FFFF) as f32 / (1 << 23) as f32
    }

    pub fn bits(&mut self) -> u32 {
        self.0 = (self.0 & 0xFFFF_FFFF) * 0x5CDC_FAA7 + (self.0 >> 32);
        self.0 as u32
    }
}

pub fn ignite(t: &mut Tyre) {
    if t.surface > 0.0 && t.fire.is_none() {
        t.fire = Some(0.0);
    }
}

pub fn burst(t: &mut Tyre, roll: f32, dt: f32) -> bool {
    if let Some(f) = &mut t.fire {
        *f += dt;
    }
    let hot = t.temp >= 70.0 || t.fire.unwrap_or(0.0) >= 10.0;
    let burst = t.surface > 0.0 && hot && roll < 1.0 - 0.99f32.powf(dt * 30.0);
    if burst {
        t.surface = 0.0;
        t.fire = None;
    }
    burst
}

fn clamp(v: f32, cap: f32) -> f32 {
    let v = fsel(v - -cap, v, -cap);
    fsel(v - cap, cap, v)
}

pub fn tyre_force(h: &Handling, t: &mut Tyre, mass: f32, inv_mass: f32, i: &TyreIn) -> [f32; 3] {
    let mut l = inv_mass * (1.0 / t.load_share) * f32::from_bits(0x3DD0_FAC6) * i.load;
    if i.load > 1.0 {
        let e = l - 1.0;
        l *= -fsel(5.0 - e, e, 5.0).mul_add(0.1, -1.0);
    }
    let n = t.load_share * GRAVITY * l * mass;
    let inv_n = 1.0 / n;
    let s = i.slip_speed.abs() * 0.07;
    let speed_k = fsel(5.0 - s, s, 5.0) + 1.0;
    let mut b = h.traction_bias[if t.front { 0 } else { 1 }] * t.wet;
    let mut p = t.grip;
    if !(t.surface > 0.0) {
        p *= 0.6;
    } else if t.surface < 1000.0 {
        p *= (t.surface * 0.001).mul_add(0.2, 0.8);
    }
    if t.resistance > 0.0 {
        let q = 1.0 - t.resistance;
        b = (fsel(-q, 0.0, q) + 1.0) * b * 0.5;
    }
    if t.flags & flags::BURNOUT != 0 && h.drive_force > 0.12 {
        b *= 0.3;
    }
    if t.flags & flags::GRIP_130 != 0 {
        b *= 1.3;
    } else if t.flags & flags::GRIP_115 != 0 {
        b *= 1.15;
    }
    let (inv_p, inv_b) = (1.0 / p, 1.0 / b);
    let inv_max = 1.0 / h.traction_max;
    let inv_range = 1.0 / (h.traction_max - h.traction_min);
    let inv_spring = 1.0 / h.traction_spring_delta_max;
    let along = i.along.abs();
    let a = mathrt::atan2(-i.across as f64, fsel(1.0 - along, 1.0, along) as f64) as f32;
    let mut lat = 1.0 / h.traction_lateral * 0.4 * a * speed_k;
    let low = i.slip_speed.abs() * 0.5 < 1.0;
    let spring = |v: f32, k: f32, against: f32| {
        if against < 0.0 { k + v } else if v > 0.0 { fsel(v - k, v, k) } else { fsel(v - k, k, v) }
    };
    let ease = -i.slip_speed.abs().mul_add(0.5, -1.0);
    if i.hold_across.abs() > 0.0 && low {
        lat = spring(lat, ease * (inv_spring * i.hold_across), lat * i.hold_across);
    }
    let s0 = i.slip;
    let lim = h.traction_min * p * b;
    let linear = |demand: f32, dir: f32| inv_max * inv_b * inv_p * 0.4 * demand * dir;
    let past = |demand: f32, dir: f32| ((h.traction_max - demand) * inv_range).mul_add(1.5, 1.0) * dir * 0.4;
    let mut slip;
    if i.drive != 0.0 {
        let dir = if i.drive > 0.0 { -1.0 } else { 1.0 };
        let demand = i.drive.abs() * inv_n;
        let mut spinning = !(demand < lim || s0.abs() < p * 0.4 || dir * s0 < 0.0);
        slip = 0.0;
        if !spinning {
            slip = linear(demand, dir);
            if slip == 0.0 {
                spinning = true;
            } else if !(slip * s0 < 0.0) && slip.abs() < s0.abs() {
                slip = fsel(1.0 - i.h * 5.0, i.h * 5.0, 1.0).mul_add(slip - s0, s0);
            }
        }
        if spinning {
            let got = curve(h, s0 * 2.5, b, p);
            let s = if got < demand { ((demand - got) * dir * i.h).mul_add(2.0, s0) } else { past(demand, dir) };
            slip = clamp(s, if t.flags & flags::CAP_SPIN != 0 { inv_p * 0.4 } else { 5.0 });
        }
    } else {
        let dir = if i.along > 0.0 { 1.0 } else { -1.0 };
        let stop = along * GRAVITY * mass;
        let demand = fsel(i.brake - stop, stop, i.brake).abs() * inv_n;
        slip = if demand < lim || s0.abs() < p * 0.4 || dir * s0 < 0.0 {
            linear(demand, dir)
        } else {
            let got = curve(h, s0 * 2.5, b, p);
            if got < demand {
                (fsel(got - 0.9, 10.0, 2.0) * (demand - got) * dir).mul_add(i.h, s0)
            } else {
                past(demand, dir)
            }
        };
        if i.hold_along.abs() > 0.0 && low {
            let k = ease * -(inv_spring * i.hold_along);
            slip = spring(slip, k, k * slip);
        }
        let (mut cap, mut slide) = (10.0, false);
        if t.flags & flags::CAP_SLIDE != 0 && along > 5.0 && t.resistance < 0.25 {
            (cap, slide) = (inv_p * 0.4 * 1.5, true);
        }
        let to = if dir > 0.0 { (slip > cap).then_some(cap) } else { (slip < -cap).then_some(-cap) };
        if let Some(c) = to {
            slip = c;
            if slide {
                t.flags |= flags::LOCKED;
            }
        }
    }
    let mut long = -slip;
    if t.flags & flags::CLAMP_SLIP != 0 {
        lat = clamp(lat, inv_p * 0.4);
        let lat2 = lat * lat;
        let c = f32::from_bits(0x3E23_D70B);
        if long.mul_add(long, lat2) > c {
            let r = c - lat2;
            let m = fsel(0.0001 - r, 0.0001, r).sqrt();
            long = clamp(long, fsel(0.05 - m, 0.05, m));
        }
    }
    let sq = lat.mul_add(lat, long * long);
    let (mut total, mut out) = (0.0, [0.0; 2]);
    if sq > 0.0 {
        total = sq.sqrt();
        let got = curve(h, total * 2.5, b, p);
        out = if i.brake > 0.0 && i.slip_speed > 1.0 && total > p * 2.25
            && (slip * i.across).abs() > (lat * i.along).abs()
        {
            let v = unit([-i.across, -i.along, 0.0, 0.0]);
            let k = got * n;
            [v[0] * k, v[1] * k]
        } else {
            let k = got / total * n;
            [lat * k, long * k]
        };
    }
    t.skid = [total.abs() * 2.5, slip.abs() * 2.5, lat.abs() * 2.5];
    [out[0], out[1], slip]
}

