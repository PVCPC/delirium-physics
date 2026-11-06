use crate::estimate::fneg;
use crate::{mathrt, vehicles::Handling, wheelstep::{fsel, unit, TyreIn}};

#[allow(non_snake_case)]
#[inline]
pub fn GRAVITY() -> f32 { crate::kv!(34) }

#[allow(non_snake_case)]
#[inline]
pub fn F80_MASK() -> u32 { crate::kv!(1058 int) }

pub fn f80_flags(f80: u32, ai_driven: bool) -> u32 {
    let mut f = if f80 & crate::kv!(969 int) != 0 {
        flags::CAP_SLIDE()
    } else if f80 & crate::kv!(648 int) != 0 {
        flags::NO_LOCK()
    } else if ai_driven {
        flags::CAP_SLIDE()
    } else {
        0
    };
    for (bits, flag) in [(crate::kv!(1059 int), flags::CAP_SPIN()), (crate::kv!(1060 int), flags::CLAMP_SLIP()), (crate::kv!(1061 int), flags::GRIP_115()),
                         (crate::kv!(1062 int), flags::GRIP_130())] {
        if f80 & bits != 0 {
            f |= flag;
        }
    }
    f
}

pub fn abs_bits(model_flags: u32, draw: &mut impl FnMut() -> f32) -> u32 {
    if model_flags & crate::kv!(54 int) != 0 || model_flags & crate::kv!(7 int) != 0 && draw() > 0.5 {
        return crate::kv!(184 int);
    }
    if model_flags & crate::kv!(188 int) != 0 || model_flags & crate::kv!(257 int) != 0 && draw() > 0.5 {
        return crate::kv!(648 int);
    }
    0
}

pub mod flags {
    pub const GROUNDED: u32 = 0x1;
    #[allow(non_snake_case)]
    #[inline]
    pub fn DRIVEN() -> u32 { crate::kv!(1 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn POWERED() -> u32 { crate::kv!(712 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn CLAMP_SLIP() -> u32 { crate::kv!(257 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn GRIP_115() -> u32 { crate::kv!(96 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn GRIP_130() -> u32 { crate::kv!(182 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn CAP_SPIN() -> u32 { crate::kv!(188 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn CAP_SLIDE() -> u32 { crate::kv!(825 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn BURNOUT() -> u32 { crate::kv!(816 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn HEATS() -> u32 { crate::kv!(821 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn LOCKED() -> u32 { crate::kv!(824 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn NO_LOCK() -> u32 { crate::kv!(826 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn SAND() -> u32 { crate::kv!(819 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn SOFT() -> u32 { crate::kv!(817 int) }
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
    pub vel: [f32; 4],
    pub roll: [f32; 4],
    pub temp: f32,
    pub fire: crate::tyrefx::Fire,
}

impl Tyre {
    pub fn new(front: bool, load_share: f32) -> Self {
        Tyre { front, load_share, grip: 1.0, wet: 1.0, resistance: 0.0, surface: crate::kv!(82), flags: 0, slip: 0.0, rate: 0.0,
              angle: 0.0, skid: [0.0; 3], vel: [0.0; 4], roll: [0.0; 4], temp: 0.0, fire: crate::tyrefx::Fire { burning: false, time: 0.0 } }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Contact {
    pub v_long: f32,
    pub v_roll: f32,
    pub drive: f32,
    pub brake: f32,
    pub frame: u32,
}

pub fn curve(h: &Handling, x: f32, b: f32, p: f32) -> f32 {
    let x = x.abs();
    let mu = if x < p {
        x / p * h.traction_max
    } else if x < p * crate::kv!(33) {
        (h.traction_min - h.traction_max) * (x - p) / (p * crate::kv!(33)) + h.traction_max
    } else {
        h.traction_min
    };
    mu * b * p
}

fn demands(t: &Tyre, c: &Contact, mass: f32) -> (f32, f32) {
    let mut brake = c.brake * mass * GRAVITY();
    if t.flags & flags::DRIVEN() == 0 || t.resistance > crate::kv!(17) {
        let k = if t.flags & flags::POWERED() != 0 { 2.0 } else { crate::kv!(78) };
        let r = GRAVITY() * mass * t.resistance;
        brake += fsel(k - r, k, r);
    }
    let drive = mass * GRAVITY() * c.drive;
    if brake > drive.abs() {
        (0.0, brake)
    } else if drive > brake {
        (drive - brake, 0.0)
    } else {
        (brake + drive, 0.0)
    }
}

pub fn rest(t: &mut Tyre, traction_max: f32, brake: f32, mass: f32, dt: f32) -> bool {
    #[allow(non_snake_case)]
    #[inline]
    fn G() -> f32 { crate::kv!(1063 f32) }
    let braking = brake * G() * mass;
    let (k, total, anchor) = if t.flags & 2 != 0 {
        let f = traction_max * t.load_share * G();
        (crate::kv!(26 f32), f.mul_add(mass, braking), false)
    } else {
        let least = if t.flags & flags::POWERED() != 0 { 2.0 } else { crate::kv!(1064 f32) };
        let roll = t.resistance * G() * mass;
        (crate::kv!(31), if least - roll >= 0.0 { least } else { roll } + braking, true)
    };
    let step = k * total * dt;
    t.skid = [0.0; 3];
    let rate = t.rate;
    t.rate = if step > rate.abs() { 0.0 } else if rate > 0.0 { rate - step } else { step + rate };
    let angle = dt.mul_add(t.rate, t.angle);
    let cool: f32 = if t.temp - crate::kv!(27) >= 0.0 { crate::kv!(79) } else { crate::kv!(21) };
    let temp = (-cool).mul_add(dt, t.temp);
    t.temp = if -temp >= 0.0 { 0.0 } else { temp };
    let (pi, two_pi) = (crate::kv!(774 f32), crate::kv!(618 f32));
    t.angle = if angle > pi { angle - two_pi } else if angle < -pi { angle + two_pi } else { angle };
    anchor
}

#[allow(clippy::too_many_arguments)]
pub fn spin(t: &mut Tyre, c: &Contact, grounded: bool, mass: f32, radius: f32, limit: f32, dir: f32, dt: f32,
            rand: &mut crate::rng::Rand) -> bool {
    let old = t.rate;
    let mut release = false;
    if grounded {
        if t.flags & flags::CAP_SLIDE() != 0 && t.flags & flags::LOCKED() != 0 && c.frame & crate::kv!(0 int) == 0 {
            t.slip = dir * crate::kv!(64);
        }
        t.skid[1] = t.slip * crate::kv!(33);
        let inv = 1.0 / radius;
        let s = t.slip * dir;
        if s < -crate::kv!(28) && c.drive * dir > 0.0 {
            let roll = -(inv * c.v_roll);
            let up = fneg((c.drive * dt).mul_add(crate::kv!(26), -old));
            t.rate = if dir > 0.0 {
                let r = fsel(roll - up, up, roll);
                if r < limit { limit * crate::kv!(138) } else { r }
            } else {
                let r = fsel(roll - up, roll, up);
                if r > limit { limit * crate::kv!(138) } else { r }
            };
            t.temp = crate::tyrefx::heat(t.temp, t.flags, t.surface, dt, rand);
        } else if s > crate::kv!(28) {
            t.rate = -((1.0 - fsel(1.0 - s, s, 1.0)) * inv * c.v_roll);
            release = t.flags & flags::NO_LOCK() != 0 && c.v_long.abs() > crate::kv!(64) && t.resistance < crate::kv!(17) && s > crate::kv!(12);
        } else {
            t.rate = -(fneg(s.mul_add(crate::kv!(78), -1.0)) * inv * c.v_roll);
            t.temp = crate::tyrefx::cool(t.temp, dt);
        }
        t.temp = crate::tyrefx::hold(t.temp);
    } else {
        let (drive, brake) = demands(t, c, mass);
        if drive != 0.0 {
            let r = fneg((drive * dt).mul_add(crate::kv!(249), -old));
            t.rate = if (limit > 0.0 && r > limit) || (limit < 0.0 && r < limit) { limit } else { r };
        } else if brake > 0.0 {
            let d = brake * dt * crate::kv!(31);
            t.rate = if d > old.abs() { 0.0 } else if old > 0.0 { old - d } else { d + old };
        }
        t.slip = 0.0;
        t.skid = [0.0; 3];
        t.temp = crate::tyrefx::air(t.temp, dt);
    }
    t.angle = dt.mul_add(t.rate, t.angle);
    if t.angle > std::f32::consts::PI {
        t.angle -= std::f32::consts::TAU;
    } else if t.angle < -std::f32::consts::PI {
        t.angle += std::f32::consts::TAU;
    }
    t.flags = t.flags & !crate::kv!(0 int) | (t.flags & 1) << 1;
    release
}

pub fn contact(t: &mut Tyre, held: &crate::wheelstep::Held, radius: f32, h: &Handling) {
    t.vel = held.vel;
    let k = radius * t.rate;
    t.roll = held.fwd.map(|f| f * k);
    let mut a = 0.0;
    if held.along.abs() > crate::kv!(78) {
        a = mathrt::atan2(-held.side as f64, held.along.abs() as f64) as f32;
    }
    if held.across.abs() > 0.0 {
        a += mathrt::atan2(held.across as f64, radius as f64) as f32;
    }
    t.skid[2] = 1.0 / h.traction_lateral * a;
}

#[derive(Clone, Debug)]
pub struct Random(u64);

impl Random {
    pub fn new(seed: u32) -> Self {
        Random(seed as u64 | 1 << crate::kv!(7 int))
    }

    pub fn from_state(low: u32, carry: u32) -> Self {
        Random(low as u64 | (carry as u64) << crate::kv!(7 int))
    }

    pub fn state(&self) -> (u32, u32) {
        (self.0 as u32, (self.0 >> crate::kv!(7 int)) as u32)
    }

    pub fn next(&mut self) -> f32 {
        (self.bits() & crate::kv!(196 int)) as f32 / (1 << crate::kv!(106 int)) as f32
    }

    pub fn bits(&mut self) -> u32 {
        self.0 = (self.0 & crate::kv!(197 int)) * crate::kv!(198 int) + (self.0 >> crate::kv!(7 int));
        self.0 as u32
    }
}

fn clamp(v: f32, cap: f32) -> f32 {
    let v = fsel(v - -cap, v, -cap);
    fsel(v - cap, cap, v)
}

pub fn tyre_force(h: &Handling, t: &mut Tyre, mass: f32, inv_mass: f32, i: &TyreIn) -> [f32; 3] {
    let mut l = inv_mass * (1.0 / t.load_share) * crate::kv!(1065 f32) * i.load;
    if i.load > 1.0 {
        let e = l - 1.0;
        l *= fneg(fsel(crate::kv!(64) - e, e, crate::kv!(64)).mul_add(crate::kv!(78), -1.0));
    }
    let n = t.load_share * GRAVITY() * l * mass;
    let inv_n = 1.0 / n;
    let s = i.slip_speed.abs() * crate::kv!(112);
    let speed_k = fsel(crate::kv!(64) - s, s, crate::kv!(64)) + 1.0;
    let mut b = h.traction_bias[if t.front { 0 } else { 1 }] * t.wet;
    let mut p = t.grip;
    if !(t.surface > 0.0) {
        p *= crate::kv!(119);
    } else if t.surface < crate::kv!(82) {
        p *= (t.surface * crate::kv!(134)).mul_add(crate::kv!(60), crate::kv!(20));
    }
    if t.resistance > 0.0 {
        let q = 1.0 - t.resistance;
        b = (fsel(-q, 0.0, q) + 1.0) * b * 0.5;
    }
    if t.flags & flags::BURNOUT() != 0 && h.drive_force > crate::kv!(125) {
        b *= crate::kv!(24);
    }
    if t.flags & flags::GRIP_130() != 0 {
        b *= crate::kv!(80);
    } else if t.flags & flags::GRIP_115() != 0 {
        b *= crate::kv!(552);
    }
    let (inv_p, inv_b) = (1.0 / p, 1.0 / b);
    let inv_max = 1.0 / h.traction_max;
    let inv_range = 1.0 / (h.traction_max - h.traction_min);
    let inv_spring = 1.0 / h.traction_spring_delta_max;
    let along = i.along.abs();
    let a = mathrt::atan2(-i.across as f64, fsel(1.0 - along, 1.0, along) as f64) as f32;
    let mut lat = 1.0 / h.traction_lateral * crate::kv!(116) * a * speed_k;
    let low = i.slip_speed.abs() * 0.5 < 1.0;
    let spring = |v: f32, k: f32, against: f32| {
        if against < 0.0 { k + v } else if v > 0.0 { fsel(v - k, v, k) } else { fsel(v - k, k, v) }
    };
    let ease = fneg(i.slip_speed.abs().mul_add(0.5, -1.0));
    if i.hold_across.abs() > 0.0 && low {
        lat = spring(lat, ease * (inv_spring * i.hold_across), lat * i.hold_across);
    }
    let s0 = i.slip;
    let lim = h.traction_min * p * b;
    let linear = |demand: f32, dir: f32| inv_max * inv_b * inv_p * crate::kv!(116) * demand * dir;
    let past = |demand: f32, dir: f32| ((h.traction_max - demand) * inv_range).mul_add(crate::kv!(12), 1.0) * dir * crate::kv!(116);
    let mut slip;
    if i.drive != 0.0 {
        let dir = if i.drive > 0.0 { -1.0 } else { 1.0 };
        let demand = i.drive.abs() * inv_n;
        let mut spinning = !(demand < lim || s0.abs() < p * crate::kv!(116) || dir * s0 < 0.0);
        slip = 0.0;
        if !spinning {
            slip = linear(demand, dir);
            if slip == 0.0 {
                spinning = true;
            } else if !(slip * s0 < 0.0) && slip.abs() < s0.abs() {
                slip = fsel(1.0 - i.h * crate::kv!(64), i.h * crate::kv!(64), 1.0).mul_add(slip - s0, s0);
            }
        }
        if spinning {
            let got = curve(h, s0 * crate::kv!(33), b, p);
            let s = if got < demand { ((demand - got) * dir * i.h).mul_add(2.0, s0) } else { past(demand, dir) };
            slip = clamp(s, if t.flags & flags::CAP_SPIN() != 0 { inv_p * crate::kv!(116) } else { crate::kv!(64) });
        }
    } else {
        let dir = if i.along > 0.0 { 1.0 } else { -1.0 };
        let stop = along * GRAVITY() * mass;
        let demand = fsel(i.brake - stop, stop, i.brake).abs() * inv_n;
        slip = if demand < lim || s0.abs() < p * crate::kv!(116) || dir * s0 < 0.0 {
            linear(demand, dir)
        } else {
            let got = curve(h, s0 * crate::kv!(33), b, p);
            if got < demand {
                (fsel(got - crate::kv!(28), crate::kv!(31), 2.0) * (demand - got) * dir).mul_add(i.h, s0)
            } else {
                past(demand, dir)
            }
        };
        if i.hold_along.abs() > 0.0 && low {
            let k = ease * -(inv_spring * i.hold_along);
            slip = spring(slip, k, k * slip);
        }
        let (mut cap, mut slide) = (crate::kv!(31), false);
        if t.flags & flags::CAP_SLIDE() != 0 && along > crate::kv!(64) && t.resistance < crate::kv!(17) {
            (cap, slide) = (inv_p * crate::kv!(116) * crate::kv!(12), true);
        }
        let to = if dir > 0.0 { (slip > cap).then_some(cap) } else { (slip < -cap).then_some(-cap) };
        if let Some(c) = to {
            slip = c;
            if slide {
                t.flags |= flags::LOCKED();
            }
        }
    }
    let mut long = -slip;
    if t.flags & flags::CLAMP_SLIP() != 0 {
        lat = clamp(lat, inv_p * crate::kv!(116));
        let lat2 = lat * lat;
        let c = crate::kv!(1066 f32);
        if long.mul_add(long, lat2) > c {
            let r = c - lat2;
            let m = fsel(crate::kv!(72) - r, crate::kv!(72), r).sqrt();
            long = clamp(long, fsel(crate::kv!(68) - m, crate::kv!(68), m));
        }
    }
    let sq = lat.mul_add(lat, long * long);
    let (mut total, mut out) = (0.0, [0.0; 2]);
    if sq > 0.0 {
        total = sq.sqrt();
        let got = curve(h, total * crate::kv!(33), b, p);
        out = if i.brake > 0.0 && i.slip_speed > 1.0 && total > p * crate::kv!(141)
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
    t.skid = [total.abs() * crate::kv!(33), slip.abs() * crate::kv!(33), lat.abs() * crate::kv!(33)];
    [out[0], out[1], slip]
}

