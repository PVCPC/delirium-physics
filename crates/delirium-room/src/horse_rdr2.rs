use super::{Horse, LAME, LEGS, MID, WELD, WELDED};
use bevy::prelude::*;
use delirium::collision::Collision;
use delirium::ragdoll::{Capsule, Drive, Limit, LimitRow, Muscle, Obstacle, Rigid, Rules};
use std::collections::HashSet;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const LEG: [usize; 4] = [3, 2, 1, 0];
const DIAGONAL: [usize; 4] = [3, 2, 1, 0];
const FORE_HIND: [usize; 4] = [2, 3, 0, 1];
const SIDE: [usize; 4] = [1, 0, 3, 2];
const LEG_INIT: [(f32, f32, f32, f32, f32); 4] = [
    (1.0, -1.0, 1.2, 0.8, 1.0), (1.0, 1.0, 1.2, 0.8, 1.0), (-1.0, -1.0, 1.36, 0.9, 1.2), (-1.0, 1.0, 1.36, 0.9, 1.2),
];
const RDR2_LEG: [f32; 4] = [1.188, 1.188, 1.453, 1.453];
const PUSHED: [usize; 5] = [0, 5, 11, 12, 15];
const CHAIN: [usize; 8] = [1, 5, 11, 12, 15, 21, 23, 24];
const POSTURE: [f32; 8] = [0.01, 0.19, 0.0, -0.15, -0.08, -0.19, -0.10, 0.0];
const SNAP_JOINT: [f32; 22] = [15.0, 9.0, 9.0, 1.0, 15.0, 2.0, 2.0, 1.0, 1.0, 1.0, 15.0, 15.0, 9.0, 9.0, 15.0, 2.0, 1.0, 2.0, 1.0, 12.0, 10.0, 8.0];
const PART_JOINTS: [[i32; 3]; 23] = [
    [-1, -1, -1], [2, 1, 4], [1, -1, 5], [5, -1, 6], [-1, -1, -1], [2, -1, 7], [7, -1, 8], [-1, -1, -1], [-1, -1, -1], [-1, -1, -1],
    [4, -1, 10], [10, -1, 11], [11, -1, 14], [12, -1, 15], [15, -1, 16], [-1, -1, -1], [13, -1, 17], [17, -1, 18], [-1, -1, -1],
    [14, -1, 19], [19, -1, 20], [20, -1, 21], [21, -1, -1],
];
const WEAK_MASK: u32 = 0xfff5_f96e;
const LYING_MASK: u32 = 0x70f82e;
const DOUBLE_MASK: u32 = 0x1822;
const GROUND_HIT_MASK: u32 = 0x782e;
const HOOF_MASK: u32 = 0xa0280;
const TAIL_MASK: u32 = 0x410;
const TAIL_FRICTION: f32 = 0.01;
const HOLD: f32 = 0.025;
const FLAIL_STIFF: f32 = 14.0;
const KNEE_LEAN: f32 = 0.07;
const LEG_LEN: f32 = 1.0;

#[derive(Clone, Copy, Default)]
struct GravPart { moment: Vec3, torque: Vec3, mass: f32, fixed: bool }

fn support_on(com: Vec3, up: Vec3, q: [Vec3; 4]) -> Vec3 {
    let edge = |a: Vec3, b: Vec3| {
        let n = up.cross(b - a).normalize_or_zero();
        (com - a).dot(n)
    };
    let (d01, d13, d32, d20) = (edge(q[0], q[1]), edge(q[1], q[3]), edge(q[3], q[2]), edge(q[2], q[0]));
    let mean = (q[0] + q[1] + q[2] + q[3]) * 0.25;
    let on = com + up * (mean - com).dot(up);
    if d01.min(d13).min(d32).min(d20) >= 0.0 {
        return on;
    }
    let (a, b) = if d20 < 0.0 && d20 < d01 && d20 < d32 { (q[0], q[2]) }
        else if d13 < 0.0 && d13 < d01 && d13 < d32 { (q[3], q[1]) }
        else if d01 < 0.0 { (q[0], q[1]) }
        else { (q[3], q[2]) };
    let e = a - b;
    let s = ((on - b).dot(e) / e.length_squared().max(1e-10)).clamp(0.0, 1.0);
    b + e * s
}

#[allow(clippy::too_many_arguments)]
fn gravity_chain(p: &mut [GravPart; 23], list: &[usize], root: bool, planted: bool, acc: Vec3, scale: f32, total: f32, joint: impl Fn(usize) -> Vec3) {
    if acc.length_squared() < 1e-6 {
        return;
    }
    let last = list.len() as i32 - 1;
    let mut order: Vec<(i32, bool)> = vec![];
    if root {
        let (mut hi, mut found) = (last, false);
        while hi > 0 {
            if p[list[hi as usize]].fixed {
                found = true;
                break;
            }
            order.push((hi, true));
            hi -= 1;
        }
        if found {
            let mut lo = 0;
            while lo < hi && !p[list[lo as usize]].fixed {
                order.push((lo, false));
                lo += 1;
            }
            let mid = (hi + lo) / 2;
            for a in mid..hi {
                order.push((a, false));
            }
            let mut a = mid;
            while a > lo {
                order.push((a, true));
                a -= 1;
            }
        }
    } else if !planted {
        for a in (1..=last).rev() {
            order.push((a, true));
        }
    } else {
        for a in 0..last {
            order.push((a, false));
        }
    }
    for &(a, up) in &order {
        let from = list[a as usize];
        let to = list[(a + if up { -1 } else { 1 }) as usize];
        let at = joint(if up { from } else { to });
        let (m, moment) = (p[from].mass, p[from].moment);
        let centre = moment / m.max(1e-6);
        let t = (centre - at).cross(acc * m);
        p[to].mass += m;
        p[to].moment += moment;
        if scale > 0.0 {
            let lim = (total - m) * m / total.max(1e-3) * 5.0 * scale;
            let t = t.clamp_length_max(lim.max(0.0));
            p[from].torque -= t;
            p[to].torque += t;
        }
    }
}

fn link_part(l: usize) -> usize {
    match l {
        0 => 1,
        1 => 0,
        2..=15 => l,
        16..=20 => l + 1,
        21 => 23,
        _ => 24,
    }
}

fn parts_of(mask: u32) -> impl Iterator<Item = usize> {
    (0..23usize).filter(move |b| mask >> b & 1 != 0).map(link_part)
}

pub const TABLE: [(&str, f32, f32, f32); 86] = [
    ("legStiffness", 15.0, 6.0, 17.0), ("legStiffnessMin", 7.0, 3.0, 30.0), ("legDamping", 1.0, 0.0, 2.0),
    ("0x2128bd54", 1.0, 0.0, 10.0), ("spineStiffness", 15.0, 6.0, 30.0), ("spinestiffnessmin", 7.0, 3.0, 30.0),
    ("spineDamping", 1.0, 0.0, 2.0), ("0xe4adac13", 4.0, 0.0, 30.0), ("pitchoffset", 0.0, -1.0, 1.0),
    ("rolloffset", 0.0, -1.0, 1.0), ("height", 1.0, 0.0, 2.0), ("strengthdecay", 0.5, 0.0, 10.0),
    ("0xd5842c66", 1.0, 0.0, 10.0), ("0x1689ff1e", 0.1, 0.0, 1.0), ("rampheighttime", -1.0, -1.0, 10.0),
    ("maxsupporttime", -1.0, -1.0, 10.0), ("frictionScale", 1.0, 0.0, 10.0), ("frictionscaleend", 1.0, 0.0, 10.0),
    ("0x1f3617d5", 0.1, 0.0, 10.0), ("maxsteplength", 0.6, 0.0, 1.0), ("steptimescale", 1.0, 0.0, 10.0),
    ("stepheightscale", 1.0, 0.0, 10.0), ("stepthreshold", 0.0, -1.0, 1.0), ("gaitphase", 0.0, 0.0, 1.0),
    ("0x6ce93cc5", 0.24, 0.0, 2.0), ("0xe6200ae9", 0.67, 0.0, 2.0), ("0x60b32079", -0.15, -2.0, 2.0),
    ("0x78748c58", 1.0, 0.0, 10.0), ("0xdcaa8fcc", 0.1, 0.0, 10.0), ("leanAgainstVelocity", 0.0, -4.0, 4.0),
    ("0x3ba69a5e", 0.0, 0.0, 1.0), ("0xbf41a9ff", 1.0, -1.0, 2.0), ("0x0a364789", 1.0, 0.0, 10.0),
    ("0x14c", 5.0, 0.0, 100.0), ("anklestrengthscale", 1.0, 0.0, 2.0), ("snapMag", 1.0, -5.0, 5.0),
    ("initialWeaknessZeroDuration", 0.0, 0.0, 10.0), ("initialWeaknessRampDuration", 0.4, 0.0, 10.0),
    ("floorWeaknessTime", 0.1, 0.0, 10.0), ("floorStrength", 0.1, 0.0, 1.0),
    ("floorJointWeaknessTime", 0.1, 0.0, 10.0), ("floorJointStrength", 0.01, 0.0, 1.0), ("footslipcompensation", 0.0, 0.0, 100.0),
    ("maxBalanceTime", 1.0, -1.0, f32::MAX), ("restepdistance", 0.35, 0.0, 10.0), ("writhespeed", 0.5, 0.0, 10.0),
    ("minscrabblespeed", 3.0, 0.0, 20.0), ("maxscrabblespeed", 11.0, 0.0, 20.0), ("angVelScale", 1.0, 0.0, 1.0),
    ("rightingTorque", 0.0, 0.0, 10.0), ("failangle", 0.6, -1.0, 1.0), ("0xbda62333", 0.25, 0.0, 0.6),
    ("rollvelocity", 0.7, 0.0, 10.0), ("rollscale", 0.7, 0.0, 10.0), ("rollpredictiontime", 0.0, 0.0, 10.0),
    ("0x18b9c869", 1.0, 0.0, 10.0), ("0x75a875d8", 0.5, 0.0, 10.0), ("0x65b9a730", 2.0, 0.0, 10.0),
    ("0x84cf43be", 4.0, 0.0, 10.0), ("0x6ded80a2", 1.0, 0.0, 10.0), ("0x33a9d629", 0.3, 0.0, 10.0),
    ("0xc55d4193", 10.0, 0.0, 100.0), ("0x1179b8be", 2.0, 0.0, 100.0), ("lookahead", 0.5, 0.1, 10.0),
    ("0xcdd11768", 0.0, -100.0, 100.0), ("0x463334cb", 0.0, -10000.0, 10000.0), ("0xf08a586f", 0.3, 0.0, 1.57),
    ("0x491b9b33", 0.0, 0.0, 10000.0), ("0x741b990a", 0.0, 0.0, 10.0), ("gaitstate", 0.0, 0.0, 4.0),
    ("0x8c66dab4", -1.0, -1.0, 4.0), ("initialstep", -1.0, -1.0, 3.0), ("alwayslifthead", 0.0, 0.0, 1.0),
    ("scrabbleonside", 1.0, 0.0, 1.0), ("forcescrabble", 0.0, 0.0, 1.0), ("measuresupport", 0.0, 0.0, 1.0),
    ("newHit", 0.0, 0.0, 1.0), ("Snap", 0.0, 0.0, 1.0), ("Writhe", 0.0, 0.0, 1.0), ("writheonground", 1.0, 0.0, 1.0),
    ("rollonground", 1.0, 0.0, 1.0), ("0x839ff632", 0.0, 0.0, 1.0), ("step", 1.0, 0.0, 1.0),
    ("0xdf7a800d", 1.0, 0.0, 1.0), ("OnFire", 0.0, 0.0, 1.0), ("0x76e322dc", 0.0, -f32::MAX, f32::MAX),
];

fn at(name: &str) -> usize {
    TABLE.iter().position(|t| t.0 == name).unwrap_or_else(|| panic!("no quadruped setting {name}"))
}

macro_rules! say {
    ($c:expr, $tag:expr, $($a:tt)*) => {
        if $c.tr && $c.said.insert($tag) {
            eprintln!("quadruped {}: {}", $tag, format!($($a)*));
        }
    };
}

macro_rules! p {
    ($c:expr, $name:literal) => {{
        static SLOT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
        $c.v[*SLOT.get_or_init(|| at($name))]
    }};
}

#[derive(Clone, Debug)]
pub struct Params {
    pub oppose: f32,
    pub v: [f32; 86],
    pub mask: String,
    pub normal: Vec3,
    pub hit_point: Vec3,
    pub bullet_vel: Vec3,
    pub local_hit: i32,
    pub limits: [Option<LimbLimit>; 4],
    pub stay: Option<Stay>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LimbLimit {
    pub kind: i32,
    pub dir: f32,
    pub min: f32,
    pub max: f32,
    pub a38: f32,
    pub a3c: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Stay {
    pub strength: Vec3,
    pub damping: Vec3,
    pub parts: bool,
    pub reduction: f32,
    pub lean_min: f32,
    pub lean_max: f32,
}

impl Default for Params {
    fn default() -> Self {
        let mut p = Params { oppose: 0.0, v: TABLE.map(|t| t.1), mask: "fb".into(), normal: Vec3::Z, hit_point: Vec3::ZERO, bullet_vel: Vec3::ZERO, local_hit: 0, limits: [None; 4], stay: None };
        p.env();
        p
    }
}

impl Params {
    pub fn env(&mut self) {
        for kv in format!("{};{}", if !tuned() { "" } else { SET }, std::env::var("HORSE_SET").unwrap_or_default()).split(';') {
            if let Some((k, v)) = kv.split_once('=') { self.set(k.trim(), v.trim()); }
        }
    }

    pub fn set(&mut self, name: &str, value: &str) -> bool {
        let vec = || {
            let v: Vec<f32> = value.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
        };
        match name.to_ascii_lowercase().as_str() {
            "angvelscalemask" => { self.mask = value.into(); return true }
            "normal" => return vec().map(|v| self.normal = v).is_some(),
            "hitpoint" => return vec().map(|v| self.hit_point = v).is_some(),
            "bulletvel" => return vec().map(|v| self.bullet_vel = v).is_some(),
            "localhitpointinfo" => return value.parse().map(|v| self.local_hit = v).is_ok(),
            _ => {}
        }
        let Some(i) = TABLE.iter().position(|t| t.0.eq_ignore_ascii_case(name)) else { return false };
        let v = match value {
            "true" => 1.0,
            "false" => 0.0,
            v => match v.parse::<f32>() {
                Ok(v) => v,
                Err(_) => return false,
            },
        };
        self.v[i] = v.clamp(TABLE[i].2, TABLE[i].3);
        true
    }

    pub fn message(&mut self, params: &[(u32, crate::gametask::Val)]) -> Vec<u32> {
        use crate::gametask::{joaat, Val};
        let mut left = vec![];
        for (h, val) in params {
            let num = match val {
                Val::Float(f) => *f,
                Val::Int(i) => *i as f32,
                Val::Bool(b) => *b as i32 as f32,
                _ => { left.push(*h); continue }
            };
            if *h == joaat("start") { continue; }
            let slot = TABLE.iter().position(|t| {
                t.0.strip_prefix("0x").and_then(|x| u32::from_str_radix(x, 16).ok()).map_or(joaat(t.0) == *h, |x| x == *h)
            });
            match slot {
                Some(i) => self.v[i] = num.clamp(TABLE[i].2, TABLE[i].3),
                None => left.push(*h),
            }
        }
        left
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Bend {
    sign: f32,
    zero: f32,
    lo: f32,
    hi: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct LegRig {
    row: usize,
    root: usize,
    a: f32,
    b: f32,
    c: f32,
    s: f32,
    side: f32,
    reach_max: f32,
    reach_min: f32,
    k: f32,
    unit: f32,
    sole: Vec3,
    knee: Bend,
    hock: Bend,
    knee_centre: f32,
    hock_centre: f32,
    knee_e: f32,
    hock_e: f32,
}

#[derive(Clone, Copy, Default)]
struct Leg {
    planted: bool,
    state: i32,
    start: f32,
    end: f32,
    hold: f32,
    left: f32,
    target: Vec3,
    neutral: Vec3,
    sole: Vec3,
    vel: Vec3,
    hip: Vec3,
    anchor: Option<Vec3>,
    aim: Option<Vec3>,
    gaps: u32,
}

pub struct Core {
    upright0: Option<(f32, f32)>,
    snap_floor: Vec<(usize, [f32; 3])>,
    t: f32,
    since_hit: f32,
    weak_clock: f32,
    blend: f32,
    state: i32,
    was_state: i32,
    first: bool,
    hit_on: bool,
    fresh3: bool,
    h: f32,
    com_h: f32,
    airborne: bool,
    free: bool,
    ground_t: f32,
    origin: Vec3,
    rows: [Vec3; 3],
    u: Vec3,
    tilt: f32,
    sign: f32,
    flail_sign: f32,
    flail: f32,
    flail_t: f32,
    lying_t: f32,
    on_parts: bool,
    lying: bool,
    support_t: f32,
    out_t: f32,
    fs: f32,
    fs2: f32,
    fl: f32,
    legs: [Leg; 4],
    gait: i32,
    last_leg: usize,
    first_gait: usize,
    phase: f32,
    sigma: f32,
    roll: f32,
    upright: f32,
    roll_go: f32,
    seed: [f32; 2],
    writhe_t: f32,
    fade: [f32; 2],
    push_lean: Option<(Vec3, f32)>,
    push_force: Option<(Vec3, f32)>,
    ground_n: Vec3,
    ground_p: Vec3,
    rng: [u64; 2],
    last: Vec<Quat>,
    locked: Vec<Quat>,
    shift: Option<(usize, f32)>,
    share_d: Vec3,
    remeasured: bool,
    hipw_done: bool,
    hit_part: usize,
    width: [f32; 2],
    fresh3_flag: bool,
    said: HashSet<&'static str>,
    tr: bool,
    rig: [LegRig; 4],
    scale: f32,
    unit: f32,
}

impl Core {
    fn rand(&mut self, which: usize) -> f32 {
        let r = &mut self.rng[which];
        *r = (*r & 0xffff_ffff) * 0x5cdc_faa7 + (*r >> 32);
        (*r & 0x7f_ffff) as f32 / 8_388_608.0
    }

    fn to_world(&self, l: Vec3) -> Vec3 {
        self.origin + self.rows[0] * l.x + self.rows[1] * l.y + self.rows[2] * l.z
    }

    fn to_local(&self, w: Vec3) -> Vec3 {
        let d = w - self.origin;
        Vec3::new(d.dot(self.rows[0]), d.dot(self.rows[1]), d.dot(self.rows[2]))
    }
}

fn hinge_of(pf: Quat, cf: Quat, rel: Quat) -> f32 {
    let r = pf.inverse() * rel * cf;
    let r = if r.w < 0.0 { -r } else { r };
    2.0 * r.z.atan2(r.w)
}

fn angles(pf: Quat, cf: Quat, rel: Quat) -> Vec3 {
    let r = pf.inverse() * rel * cf;
    let r = if r.w < 0.0 { -r } else { r };
    let tw = Quat::from_xyzw(0.0, 0.0, r.z, r.w).normalize();
    let sw = (r * tw.inverse()).to_scaled_axis();
    Vec3::new(sw.y, sw.x, 2.0 * tw.z.atan2(tw.w))
}

fn from_angles(pf: Quat, cf: Quat, a: Vec3) -> Quat {
    pf * Quat::from_scaled_axis(Vec3::new(a.y, a.x, 0.0)) * Quat::from_rotation_z(a.z) * cf.inverse()
}

fn wrap(a: f32) -> f32 {
    let a = (a + PI).rem_euclid(TAU) - PI;
    if a == -PI { PI } else { a }
}

fn flat(p: Vec3, n: Vec3, k: f32) -> Vec3 {
    p + n * (k - p.dot(n))
}

fn bend_f(a: f32, b: f32, c: f32, th: f32, d: f32, k: f32) -> (f32, f32) {
    let (kt, ot) = (k * th, th - k * th);
    let f = a * a + b * b + c * c - 2.0 * a * b * th.cos() - 2.0 * b * c * kt.cos() + 2.0 * a * c * ot.cos() - d * d;
    let g = 2.0 * a * b * th.sin() + 2.0 * b * c * k * kt.sin() - 2.0 * a * c * (1.0 - k) * ot.sin();
    (f, g)
}

fn bend_angle(a: f32, b: f32, c: f32, d: f32, k: f32, tol: f32) -> f32 {
    let d = d.clamp(0.0, a + b + c - 1e-6);
    let x = ((a + c) * (a + c) + b * b - d * d) / (2.0 * b * (a + c));
    let mut th = if x <= -1.0 { PI } else if x < 1.0 { x.acos() } else { 0.0 };
    for _ in 0..5 {
        let (f, g) = bend_f(a, b, c, th, d, k);
        if g.abs() < 1e-6 {
            break;
        }
        let next = th - f / g;
        let step = (next - th).abs();
        th = next;
        if step <= tol {
            break;
        }
    }
    th.clamp(0.0, PI / k.max(1.0))
}

fn tri_margin(t: [Vec3; 3], p: Vec3, up: Vec3) -> f32 {
    let s = if (t[1] - t[0]).cross(t[2] - t[0]).dot(up) >= 0.0 { 1.0 } else { -1.0 };
    (0..3).map(|i| {
        let (a, b) = (t[i], t[(i + 1) % 3]);
        (p - a).dot(up.cross(b - a).normalize_or_zero()) * s
    }).fold(f32::MAX, f32::min)
}

fn lock() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("HORSE_LOCK").is_some())
}

fn pin() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("HORSE_PIN").is_some())
}

const KNOBS: [(&str, &str); 9] = [("HORSE_KS", "6"), ("HORSE_JW", "0.4"), ("HORSE_CAP", "4"), ("HORSE_REACH", "0.98"),
    ("HORSE_STEP", "1"), ("HORSE_CATCH", "0.05"), ("HORSE_WORLDSWING", "1"), ("HORSE_SWINGMIN", "0.25"), ("HORSE_SHIFT", "1")];
const HYB_STEPTIME: f32 = 0.4;
const HYB_SWINGMIN: f32 = 0.2;
const STAY_REST: f32 = 3.0;
const WALK_LOOK: f32 = 0.15;
const WALK_SPEED: f32 = 0.4;
const SET: &str = "frictionScale=3;frictionscaleend=3;steptimescale=0.3;spineStiffness=18";
fn knob(name: &str) -> Option<String> {
    std::env::var(name).ok().or_else(|| share_on().then(|| "1".to_string()).filter(|_| matches!(name, "HORSE_STEP" | "HORSE_SHIFT"))).or_else(|| tuned().then(|| {
        if name == "HORSE_KS" && super::ours::hybrid() { return Some("1.5".to_string()); }
        KNOBS.iter().find(|k| k.0 == name).map(|k| k.1.to_string())
    }).flatten())
}

fn tuned() -> bool {
    std::env::var_os("HORSE_TUNE").is_some() || super::ours::hybrid()
}

fn share_on() -> bool {
    std::env::var("HORSE_SHARE").is_ok_and(|v| v != "0") && !super::ours::hybrid()
}

fn ours_step() -> bool {
    knob("HORSE_STEP").is_some_and(|v| v != "0")
}

fn support_margin(pts: &[Vec3; 4], p: Vec3, up: Vec3) -> f32 {
    let f = |v: Vec3| v - up * v.dot(up);
    let c = pts.iter().map(|&v| f(v)).sum::<Vec3>() / 4.0;
    let x = (f(pts[0]) - c).normalize_or(up.any_orthonormal_vector());
    let y = up.cross(x);
    let mut ring: Vec<Vec3> = pts.iter().map(|&v| f(v)).collect();
    ring.sort_by(|a, b| (a - c).dot(y).atan2((a - c).dot(x)).total_cmp(&(b - c).dot(y).atan2((b - c).dot(x))));
    let q = f(p);
    let (mut inside, mut out, mut all_in) = (f32::MAX, f32::MAX, true);
    for i in 0..4 {
        let (a, b) = (ring[i], ring[(i + 1) % 4]);
        let e = b - a;
        let d = (q - a).dot(up.cross(e).normalize_or_zero());
        inside = inside.min(d);
        all_in &= d >= 0.0;
        let s = ((q - a).dot(e) / e.length_squared().max(1e-9)).clamp(0.0, 1.0);
        out = out.min((a + e * s).distance(q));
    }
    if all_in { inside } else { -out }
}

fn noise(x: f32, seed: f32) -> f32 {
    let i = x.floor();
    let t = x - i;
    let hash = |n: f32| ((n * 12.9898 + seed * 78.233).sin() * 43758.547).rem_euclid(1.0) - 0.5;
    let u = t * t * (3.0 - 2.0 * t);
    hash(i) * (1.0 - u) + hash(i + 1.0) * u
}

impl Horse {
    fn joint_at(&self, fr: &[Transform], k: usize) -> Vec3 {
        let d = &self.build.parts[k];
        let p = d.parent.unwrap_or(0);
        fr[p].translation + fr[p].rotation * d.anchor_parent
    }

    fn leg_rig(&self, frames: &[Transform]) -> [LegRig; 4] {
        std::array::from_fn(|i| {
            let (s, side, cap, floor, k) = LEG_INIT[i];
            let row = LEG[i];
            let [p0, p1, p2] = LEGS[row];
            let (d0, d1, d2) = (&self.build.parts[p0], &self.build.parts[p1], &self.build.parts[p2]);
            let sole = Vec3::new(0.0, -(d2.half + d2.radius), 0.0);
            let (u0, u1, u2) = (d1.anchor_parent - d0.anchor, d2.anchor_parent - d1.anchor, sole - d2.anchor);
            let (a, b, c) = (u0.length(), u1.length(), u2.length());
            let sum = a + b + c;
            let unit = if (sum / RDR2_LEG[i] - 1.0).abs() < 0.03 { 1.0 } else { sum / RDR2_LEG[i] };
            let reach_max = sum.min(cap * unit);
            let reach_min = (reach_max * 0.66).max(floor * unit);
            let bend = |limit: Limit, up: Vec3, down: Vec3, rest: Quat, e_s: f32| -> (Bend, f32) {
                let Limit::BoneHinge { pf, cf, lo, hi } = limit else { return (Bend::default(), 0.0) };
                let (pj, cj) = (pf.inverse() * up, cf.inverse() * down);
                let (ap, ac) = (pj.y.atan2(pj.x), cj.y.atan2(cj.x));
                let t = hinge_of(pf, cf, rest);
                let sign = if wrap(ac + t - ap - PI) >= 0.0 { 1.0 } else { -1.0 };
                let zero = ap + PI - ac;
                (Bend { sign, zero, lo, hi }, e_s * sign)
            };
            let t1 = frames[self.build.parts[p1].parent.unwrap_or(0)].rotation.inverse() * frames[p1].rotation;
            let t2 = frames[self.build.parts[p2].parent.unwrap_or(0)].rotation.inverse() * frames[p2].rotation;
            let (knee, knee_e) = bend(d1.limit, u0, u1, t1, -s);
            let (hock, hock_e) = bend(d2.limit, u1, u2, t2, s);
            LegRig { row, root: d0.parent.unwrap_or(0), a, b, c, s, side, reach_max, reach_min, k, unit, sole, knee, hock,
                     knee_centre: self.centres.get(p1).map_or(0.0, |c| c.z), hock_centre: self.centres.get(p2).map_or(0.0, |c| c.z), knee_e, hock_e }
        })
    }

    fn com_rows(&self, fr: &[Transform]) -> [Vec3; 3] {
        let a = if std::env::var_os("HORSE_LEVEL").is_some() { Vec3::Z } else { (fr[MID].rotation * self.stand).normalize_or(Vec3::Z) };
        let f = fr[MID].rotation * self.fwd;
        let c = -(f - a * a.dot(f)).normalize_or(Vec3::X);
        [a, c.cross(a), c]
    }

    fn com_rider(&self) -> (Vec3, Vec3, f32) {
        let (x, v) = self.doll.com();
        let m = self.doll.mass();
        match self.rider {
            Some((p, pv, rm)) if rm > 0.0 => ((x * m + p * rm) / (m + rm), (v * m + pv * rm) / (m + rm), m + rm),
            _ => (x, v, m),
        }
    }

    fn spin_of(&self, com: Vec3) -> Vec3 {
        let (mut l, mut i) = (Vec3::ZERO, Mat3::ZERO);
        let (_, vc) = self.doll.com();
        for (k, f) in self.doll.frames().enumerate() {
            let m = self.doll.part_mass(k);
            let (v, _) = self.doll.vel(k);
            let r = f.translation - com;
            let t = self.doll.tensor(k).as_mat3();
            l += m * r.cross(v - vc);
            let s = Mat3::from_diagonal(Vec3::splat(r.length_squared())) - Mat3::from_cols(r * r.x, r * r.y, r * r.z);
            i += t + s * m;
        }
        if i.determinant().abs() < 1e-6 { Vec3::ZERO } else { i.inverse() * l }
    }

    pub(super) fn core_swinging(&self, j: usize) -> bool {
        self.core.as_ref().is_some_and(|c| (0..4).any(|i| LEG[i] == j && c.legs[i].state == 1))
    }

    fn sole_at(&self, c: &Core, fr: &[Transform], i: usize) -> Vec3 {
        let f = fr[LEGS[LEG[i]][2]];
        f.translation + f.rotation * c.rig[i].sole
    }

    fn activate(&mut self, fr: &[Transform]) -> Core {
        if self.held {
            self.held = false;
            self.doll.unpin();
            for (k, at, imp) in std::mem::take(&mut self.pushes) {
                self.doll.impulse(k, at, imp);
            }
        }
        let rig = self.leg_rig(fr);
        let tr = std::env::var_os("RDR2_TRACE").is_some();
        let v = &self.r;
        let (com, _, m) = self.com_rider();
        let rows = self.com_rows(fr);
        let unit = rig.iter().map(|r| r.unit).sum::<f32>() / 4.0;
        let h = unit * p!(v, "height");
        let (w, ln, o) = (unit * p!(v, "0x6ce93cc5"), unit * p!(v, "0xe6200ae9"), unit * p!(v, "0x60b32079"));
        let layout = [Vec3::new(0.0, w, -ln - o), Vec3::new(0.0, -w, -ln - o), Vec3::new(0.0, w, ln - o), Vec3::new(0.0, -w, ln - o)];
        let seed = (com.x.to_bits() as u64) ^ ((com.y.to_bits() as u64) << 21) ^ 0x1234_5678;
        let mut c = Core {
            upright0: None,
            snap_floor: vec![],
            t: 0.0, since_hit: 100_000.0, weak_clock: 1000.0, blend: 1.0, state: 0, was_state: -1, first: true, hit_on: false, fresh3: false, h,
            com_h: 10.0, airborne: false, free: false, ground_t: 0.0, origin: com - rows[0] * h, rows, u: Vec3::Z, tilt: 0.0, sign: 1.0, flail_sign: 2.0,
            flail: 0.0, flail_t: 0.0, lying_t: 0.0, on_parts: false, lying: false, support_t: 0.0, out_t: 0.0, fs: 1.0, fs2: 1.0, fl: 1.0,
            legs: [Leg::default(); 4], gait: -1, last_leg: (p!(v, "gaitstate") as usize).min(4), first_gait: 4, phase: 0.0, sigma: 0.0, roll: 0.0,
            upright: 1.0, roll_go: 1.0, seed: [123.0, 321.0], writhe_t: 0.0, fade: [1.0; 2], push_lean: None, push_force: None,
            ground_n: Vec3::Z, ground_p: com - Vec3::Z, rng: [seed | 1, (seed << 7) | 3], last: vec![], locked: vec![], shift: None, share_d: Vec3::ZERO, remeasured: false, hipw_done: false, hit_part: 0, width: [0.1, 0.65], fresh3_flag: false,
            said: HashSet::new(), tr, rig, scale: (m / 618.0).sqrt(), unit,
        };
        c.seed = [c.rand(0) * 1000.0, c.rand(0) * 1000.0];
        for i in 0..4 {
            c.legs[i].neutral = layout[i];
            c.legs[i].target = layout[i];
            c.legs[i].sole = self.sole_at(&c, fr, i);
        }
        if p!(self.r, "measuresupport") != 0.0 || unit != 1.0 {
            let mut hw = 0.0;
            let mut hl = 0.0;
            for i in 0..4 {
                let m = c.to_local(c.legs[i].sole);
                c.legs[i].neutral = Vec3::new(0.0, m.y, m.z);
                c.legs[i].target = c.legs[i].neutral;
                hw += m.y.abs();
                hl += m.z.abs();
            }
            let wide = std::env::var("HORSE_WIDE").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
            for i in 0..4 { c.legs[i].neutral.y *= wide; }
            hw *= wide;
            c.width = [hw * 0.25, hl * 0.25];
            say!(c, "0x2ad4518 measuresupport", "half width {:.3}, half length {:.3}", hw * 0.25, hl * 0.25);
        }
        if let Some(f) = std::env::var("HORSE_FIT").ok().and_then(|v| v.parse::<f32>().ok()) {
            for i in 0..2 {
                let r = c.rig[i];
                let hip = self.joint_at(fr, LEGS[LEG[i]][0]);
                let up = c.rows[0];
                let v = c.to_world(c.legs[i].neutral) - hip;
                let (vu, reach) = (v.dot(up), (r.a + r.b + r.c) * f);
                let vp2 = (v.length_squared() - vu * vu).max(0.0);
                if tr { eprintln!("fit? leg {i}: hip {hip:.3} place {:.3} up {up:.3} reach {reach:.3}", c.to_world(c.legs[i].neutral)); }
                if v.length() > reach && reach * reach > vp2 {
                    let x = -vu - (reach * reach - vp2).sqrt();
                    c.legs[i].neutral.x += x;
                    c.legs[i].target.x += x;
                    if tr { eprintln!("fit: leg {i} {:.3} m from its shoulder of {:.3}: raised {x:.3}", v.length(), r.a + r.b + r.c); }
                }
            }
        }
        let fs = p!(self.r, "frictionScale");
        let fric: Vec<(usize, f32)> = parts_of(HOOF_MASK).map(|k| (k, fs)).chain(parts_of(TAIL_MASK).map(|k| (k, TAIL_FRICTION))).collect();
        self.doll.set_part_scales(&fric, &[]);
        let scale = p!(self.r, "angVelScale");
        if (scale - 1.0).abs() >= 1.192_092_9e-7 {
            for (k, f) in fr.iter().enumerate() {
                let (v, w) = self.doll.vel(k);
                self.doll.set_body(k, f.translation, f.rotation, v, w * scale);
            }
            say!(c, "0x2a4b1ac", "the parts' spin scaled {scale}");
        }
        if tr {
            eprintln!("quadruped 0x2abf7dc: ctor's defaults: blend 1, H unit 1, seeds {:.1} {:.1}", c.seed[0], c.seed[1]);
            eprintln!("quadruped 0x2acea44: reset: legs stance, gait -1, last leg {}, widths {:?}", c.last_leg, c.width);
            let sole: Vec<String> = (0..4).map(|i| { let l = c.to_local(c.legs[i].sole); format!("({:.3} {:.3} {:.3})", l.x, l.y, l.z) }).collect();
            let y = fr[MID].rotation * self.spine_axis;
            eprintln!("quadruped 0x2ad4518: activate: H {h:.3}, mass {m:.1} (S {:.3}), soles in the frame {}, Spine2's second axis along a {:.3}, along b {:.3}, along c {:.3}", c.scale, sole.join(" "), y.dot(rows[0]), y.dot(rows[1]), y.dot(rows[2]));
            eprintln!("quadruped 0x2a3ba40: body init: unit 1.0, Spine2's part origin {:.3} above the first sole (its capsule's centre {:.3})", (fr[MID].translation + fr[MID].rotation * self.spine_cg).z - c.legs[0].sole.z, fr[MID].translation.z - c.legs[0].sole.z);
            for (i, r) in c.rig.iter().enumerate() {
                eprintln!("quadruped 0x2a5224c: leg {i} (row {}): a {:.3} b {:.3} c {:.3} sum {:.3} reach {:.3}..{:.3} s {} side {} k {}, knee sign {} (e {}), hock sign {} (e {})", r.row, r.a, r.b, r.c, r.a + r.b + r.c, r.reach_min, r.reach_max, r.s, r.side, r.k, r.knee.sign, r.knee_e, r.hock.sign, r.hock_e);
            }
        }
        c
    }

    #[allow(clippy::too_many_arguments)]
    fn leg_ik(&self, c: &mut Core, fr: &[Transform], i: usize, to: Vec3, vel: Vec3, gain: f32, psi: f32, lead: Option<(f32, f32)>, st: f32, ratio: f32) -> [Quat; 3] {
        let r = c.rig[i];
        let [p0, p1, p2] = LEGS[r.row];
        let hip = self.joint_at(fr, p0);
        let mut t = to;
        let (s_k, d_k) = (st * st, 2.0 * st * ratio);
        if gain > 0.0 {
            let kappa = gain * d_k / (s_k + 0.1);
            let rr = t - hip;
            let (vr, wr) = self.doll.vel(r.root);
            let len = rr.length();
            let dirn = rr.normalize_or_zero();
            let mut np = hip + (rr + wr.cross(rr) * kappa).normalize_or_zero() * len - (vr - vel) * kappa;
            let along = (np - hip).dot(dirn);
            if along < 0.1 {
                np += dirn * (0.1 - along);
            }
            t = np;
            say!(c, "0x2a37370", "leg {i}: the target led by {:.3} m", np.distance(to));
        }
        if c.tr && std::env::var_os("HORSE_HIPLOG").is_some() && (c.t * 60.0).round() as i32 % 6 == 0 {
            if let Limit::Bone { pf, cf, lean, .. } = self.build.parts[p0].limit {
                let now = fr[self.build.parts[p0].parent.unwrap_or(0)].rotation.inverse() * fr[p0].rotation;
                let a = angles(pf, cf, now);
                eprintln!("hip{i} t {:.1} lean ({:+.2} {:+.2}) of +-({:.2} {:.2})", c.t, a.y, a.z, lean[0], lean[1]);
            }
        }
        let foot = fr[p2].translation + fr[p2].rotation * r.sole;
        let mut ratio_x = None;
        if let Some((p, k)) = lead {
            let len = (t - foot).length().min(d_k * k / s_k.max(1e-3)) + 1e-4;
            let w = (1.0 - 0.1 / len).max(0.0);
            let p = if w < p { (w + p) * 0.5 } else { p };
            t = foot + (t - foot).normalize_or_zero() * (1.0 - p) * len;
            ratio_x = Some(1.0 / (1.0001 - p));
            say!(c, "0x2a1839c", "leg {i}: swing lead fraction {p:.3}, length {len:.3}");
        }
        if let Some(f) = knob("HORSE_REACH").and_then(|v| v.parse::<f32>().ok()) {
            let all = std::env::var_os("HORSE_REACHALL").is_some();
            let most = if i < 2 { r.a + r.b + r.c } else { r.reach_max } * f;
            if (i < 2 || all) && (t - hip).length() > most {
                t = hip + (t - hip).normalize_or_zero() * most;
            }
        }
        let d = (t - hip).length();
        let th = bend_angle(r.a, r.b, r.c, d, r.k, 1e-3);
        say!(c, "0x2a23434", "leg {i}: distance {d:.3} of {:.3}: theta {th:.3}", r.a + r.b + r.c);
        let (f, g) = bend_f(r.a, r.b, r.c, th, d.min(r.a + r.b + r.c), r.k);
        say!(c, "0x2a3eec8", "leg {i}: f {f:.5}, slope {g:.3}");
        let t1 = r.knee.sign * th + r.knee.zero;
        let t2 = r.hock.sign * (r.k * th) + r.hock.zero;
        let (Limit::BoneHinge { pf: f1, cf: g1, .. }, Limit::BoneHinge { pf: f2, cf: g2, .. }, Limit::Bone { pf: f0, cf: g0, .. }) = (self.build.parts[p1].limit, self.build.parts[p2].limit, self.build.parts[p0].limit) else {
            return [self.targets[p0], self.targets[p1], self.targets[p2]];
        };
        let hinge = |f: Quat, g: Quat, t: f32| f * Quat::from_rotation_z(wrap(t)) * g.inverse();
        let (mut q1, mut q2) = (hinge(f1, g1, t1), hinge(f2, g2, t2));
        let pq = fr[self.build.parts[p0].parent.unwrap_or(0)].rotation;
        let zero = f0 * g0.inverse();
        let sole_from = |q0: Quat, q1: Quat, q2: Quat| {
            let (d1, d2) = (&self.build.parts[p1], &self.build.parts[p2]);
            let r0 = pq * q0;
            let j1 = hip + r0 * (d1.anchor_parent - self.build.parts[p0].anchor);
            let r1 = r0 * q1;
            let j2 = j1 + r1 * (d2.anchor_parent - d1.anchor);
            let r2 = r1 * q2;
            j2 + r2 * (r.sole - d2.anchor)
        };
        let a = (sole_from(zero, q1, q2) - hip).normalize_or(Vec3::NEG_Z);
        let b = (t - hip).normalize_or(Vec3::NEG_Z);
        let arc = Quat::from_axis_angle(b, psi) * Quat::from_rotation_arc(a, b);
        let mut q0 = pq.inverse() * arc * pq * zero;
        if let Some(x) = ratio_x {
            let now = |k: usize| fr[self.build.parts[k].parent.unwrap_or(0)].rotation.inverse() * fr[k].rotation;
            let (a0, b0) = (angles(f0, g0, now(p0)), angles(f0, g0, q0));
            q0 = from_angles(f0, g0, a0 + (b0 - a0) * x);
            let (a1, b1) = (hinge_of(f1, g1, now(p1)), hinge_of(f1, g1, q1));
            q1 = hinge(f1, g1, a1 + wrap(b1 - a1) * x);
            let (a2, b2) = (hinge_of(f2, g2, now(p2)), hinge_of(f2, g2, q2));
            q2 = hinge(f2, g2, a2 + wrap(b2 - a2) * x);
        }
        if c.tr && std::env::var_os("HORSE_FK").is_some() && (i == 2 || i == 3) && (c.t * 60.0).round() as i32 % 30 == 0 {
            let now = |k: usize| fr[self.build.parts[k].parent.unwrap_or(0)].rotation.inverse() * fr[k].rotation;
            let (fk_now, fk_aim) = (sole_from(now(p0), now(p1), now(p2)), sole_from(q0, q1, q2));
            let (a0, b0) = (angles(f0, g0, now(p0)), angles(f0, g0, q0));
            eprintln!("ang{i} t {:.1} d {d:.3} reach {:.3}..{:.3} th {th:.3} zeros {:.3} {:.3} hip now ({:+.3} {:+.3} {:+.3}) want ({:+.3} {:+.3} {:+.3}) knee now {:+.3} want {:+.3} hock now {:+.3} want {:+.3}", c.t, r.reach_min, r.reach_max, r.knee.zero, r.hock.zero, a0.x, a0.y, a0.z, b0.x, b0.y, b0.z, hinge_of(f1, g1, now(p1)), hinge_of(f1, g1, q1), hinge_of(f2, g2, now(p2)), hinge_of(f2, g2, q2));
            eprintln!("fk{i} t {:.1} real ({:.3} {:.3} {:.3}) model-now ({:.3} {:.3} {:.3}) aim ({:.3} {:.3} {:.3}) model-of-pose ({:.3} {:.3} {:.3})", c.t, foot.x, foot.y, foot.z, fk_now.x, fk_now.y, fk_now.z, t.x, t.y, t.z, fk_aim.x, fk_aim.y, fk_aim.z);
        }
        say!(c, "0x2a2c398", "leg {i}: knee hinge {:.3} (limits {:.2} {:.2}), hock hinge {:.3} (limits {:.2} {:.2}), swivel {psi:.3}", wrap(t1), r.knee.lo, r.knee.hi, wrap(t2), r.hock.lo, r.hock.hi);
        [q0, q1, q2]
    }

    fn limb_limits(&mut self, c: &mut Core, fr: &[Transform]) {
        let jw = knob("HORSE_JW").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        for i in 0..4 {
            let mut r = c.rig[i];
            if !c.hipw_done {
                let hw = std::env::var("HORSE_HIPW").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                let p0 = LEGS[r.row][0];
                if let Limit::Bone { pf, cf, lean, twist } = self.build.parts[p0].limit {
                    if c.tr { eprintln!("hip {i}: lean {:.3} {:.3} twist {twist:.3}", lean[0], lean[1]); }
                    self.build.parts[p0].limit = Limit::Bone { pf, cf, lean: [lean[0] + hw, lean[1] + hw], twist };
                } else if c.tr { eprintln!("hip {i}: {:?}", self.build.parts[p0].limit); }
                if i == 3 { c.hipw_done = true; }
            }
            if jw != 0.0 {
                (r.hock.lo, r.hock.hi) = (r.hock.lo - jw, r.hock.hi + jw);
                let [_, p1, _] = LEGS[r.row];
                if let Limit::BoneHinge { pf, cf, .. } = self.build.parts[p1].limit {
                    self.build.parts[p1].limit = Limit::BoneHinge { pf, cf, lo: r.knee.lo - jw, hi: r.knee.hi + jw };
                }
            }
            let [_, p1, p2] = LEGS[r.row];
            let rel = |d: &Self, k: usize| fr[d.build.parts[k].parent.unwrap_or(0)].rotation.inverse() * fr[k].rotation;
            let Some(l) = self.r.limits[i].filter(|l| l.kind == 3) else {
                if let Limit::BoneHinge { pf, cf, .. } = self.build.parts[p2].limit {
                    self.build.parts[p2].limit = Limit::BoneHinge { pf, cf, lo: r.hock.lo, hi: r.hock.hi };
                }
                continue;
            };
            let (Limit::BoneHinge { pf: f1, cf: g1, .. }, Limit::BoneHinge { pf: f2, cf: g2, .. }) = (self.build.parts[p1].limit, self.build.parts[p2].limit) else { continue };
            let knee = r.knee_e * hinge_of(f1, g1, rel(self, p1)) + r.knee_centre;
            let v = (knee - l.a38) * l.dir + l.a3c;
            let (a, b) = ((v + l.min - r.hock_centre) * r.hock_e, (v + l.max - r.hock_centre) * r.hock_e);
            let (lo, hi) = ((a.min(b) - jw).max(r.hock.lo), (a.max(b) + jw).min(r.hock.hi));
            if lo < hi {
                self.build.parts[p2].limit = Limit::BoneHinge { pf: f2, cf: g2, lo, hi };
            }
            let now = hinge_of(f2, g2, rel(self, p2));
            if c.tr && std::env::var_os("HORSE_FK").is_some() && i == 2 && (c.t * 60.0).round() as i32 % 30 == 0 { eprintln!("win{i} t {:.1} hock window {lo:.3}..{hi:.3} (joint's own {:.3}..{:.3}) hock {now:.3}; knee {:.3} of {:.3}..{:.3}", c.t, r.hock.lo, r.hock.hi, hinge_of(f1, g1, rel(self, p1)), r.knee.lo, r.knee.hi); }
            say!(c, "0x2a5c448", "leg {i}: hock window {lo:.3}..{hi:.3}, hock at {now:.3}, knee in Delirium's numbers {knee:.3}");
            say!(c, "0x2a73d68", "leg {i}: (knee - {}) x {} + {} = {v:.3}", l.a38, l.dir, l.a3c);
        }
    }

    pub(super) fn step_rdr2(&mut self, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        let fr: Vec<Transform> = self.doll.frames().collect();
        if std::env::var_os("HORSE_MOMLOG").is_some() {
            let (_, v) = self.doll.com();
            let touching = self.core.as_ref().map_or(0, |c| c.legs.iter().filter(|l| l.planted).count());
            eprintln!("mom {:.3} {:.3} {:.3} {touching}", v.x, v.y, v.z);
        }
        let mut c = match self.core.take() {
            Some(c) => c,
            None => Box::new(self.activate(&fr)),
        };
        if p!(self.r, "newHit") != 0.0 {
            self.r.v[at("newHit")] = 0.0;
            let part = self.doll.nearest(&self.build, self.r.hit_point);
            self.rdr2_hits.push((part, self.r.hit_point, self.r.bullet_vel));
        }
        let n = self.build.parts.len();
        let (mut force, mut torque) = (vec![Vec3::ZERO; n], vec![Vec3::ZERO; n]);
        let w = Vec3::Z;
        let rows = self.com_rows(&fr);
        let [a, b, cc] = rows;
        let (com, vel, mass) = self.com_rider();
        let omega = self.spin_of(com);
        let (speed, spin) = (vel.length(), omega.length());
        c.scale = (mass / 618.0).sqrt();
        let pv = self.r.clone();

        c.t += dt;
        c.since_hit += dt;
        c.weak_clock += dt;
        let hits = std::mem::take(&mut self.rdr2_hits);
        c.hit_on = !hits.is_empty();
        let snap_hit = hits.first().copied();
        for &(part, _, imp) in &hits {
            self.new_hit(&mut c, &pv, part, imp, rows);
        }

        match world.contact(com, -w, 1.5 * LEG_LEN) {
            Some((d, ..)) => { c.airborne = false; c.com_h = d; say!(c, "0x2adea64", "the COM {d:.3} over the ground"); }
            None => c.airborne = true,
        }
        self.mean_contact(&mut c);
        if c.first {
            c.airborne = false;
        }
        let was_free = c.free;
        if c.airborne {
            let ahead = world.contact(com, vel.normalize_or(-w), speed.max(1e-3));
            say!(c, "0x2ade914", "the path ahead {:?}", ahead.map(|h| h.0 / speed.max(1e-3)));
            c.free = ahead.is_none();
            if c.free && !was_free {
                c.gait = 1;
                c.flail_sign = c.rand(0) * 4.0 - 2.0;
            }
        } else {
            c.ground_t += dt;
            c.free = false;
        }
        for i in 0..4 {
            let k = LEGS[LEG[i]][2];
            c.legs[i].sole = self.sole_at(&c, &fr, i);
            c.legs[i].vel = self.doll.vel(k).0;
            c.legs[i].hip = self.joint_at(&fr, LEGS[LEG[i]][0]);
        }
        let (t1, t2) = (b.dot(w), omega.dot(cc));
        let tilt = t1 + 0.2 * t2;
        c.tilt = if t2.abs() < 0.1 || tilt.abs() < 0.1 { 0.0 } else { tilt };
        c.sign = if c.tilt < 0.0 { -1.0 } else { 1.0 };
        let (lf, lt) = (p!(pv, "0x491b9b33"), p!(pv, "0x741b990a"));
        if c.t < lt && b.dot(omega) < 0.5 {
            let f = w * lf * (1.0 - c.t / lt) / 3.0;
            for k in [11, 12, 15] {
                force[k] += f;
            }
            say!(c, "0x2adb528 start lift", "{:.1} N on each of three parts", f.length());
        }
        let look = {
            let mut l = com + vel * c.unit.sqrt() * p!(pv, "lookahead");
            if let (Some((i, _)), Some(k)) = (c.shift, knob("HORSE_SHIFT").map(|v| v.parse::<f32>().unwrap_or(1.0))) {
                let mid = (0..4).filter(|&j| j != i).map(|j| c.legs[j].sole).sum::<Vec3>() / 3.0;
                let d = mid - com;
                l += (d - w * d.dot(w)) * k;
            }
            let ev = |n: &str, d: f32| std::env::var(n).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
            let top = ev("HORSE_WALKSPEED", WALK_SPEED) * c.unit.sqrt();
            let short = ((top - vel.dot(self.walk)) / top).clamp(-1.0, 1.0);
            let l = l + self.walk.with_z(0.0) * c.unit * ev("HORSE_WALKLOOK", WALK_LOOK) * if self.walk == Vec3::ZERO { 0.0 } else { short };
            Vec3::new(l.x, l.y, c.legs[0].sole.z)
        };
        self.support(&mut c, &pv, &fr, rows, com, look, dt, &mut force);
        let soles = [c.legs[0].sole, c.legs[1].sole, c.legs[2].sole, c.legs[3].sole];
        let mut m = support_margin(&soles, look, w);
        say!(c, "0x2a429c8", "look-ahead COM {m:.3} inside the hooves' hull");

        let on_ground = |d: &Self, k: usize| d.doll.touch[k] & 0x20 != 0;
        let up_hit = |d: &Self, k: usize| d.doll.hit[k].is_some_and(|h| h.1.dot(w) > 0.5);
        let mut count = 0;
        for bit in (0..23usize).filter(|b| LYING_MASK >> b & 1 != 0) {
            let k = link_part(bit);
            if on_ground(self, k) && up_hit(self, k) {
                count += if DOUBLE_MASK >> bit & 1 != 0 { 2 } else { 1 };
            }
        }
        c.on_parts = count >= 2;
        if c.on_parts && c.roll < 0.5 {
            c.lying_t = (c.lying_t + dt).min(0.3);
        } else {
            c.lying_t = (c.lying_t - dt).max(0.0);
        }
        c.lying = c.lying_t > 0.2;
        if c.on_parts { say!(c, "0x2adb528 parts on the ground", "count {count}"); }

        let up = w.dot(c.rows[0]);
        let fail = if c.on_parts { (p!(pv, "failangle") + 0.1).max(0.0) } else { p!(pv, "failangle") };
        if up < 0.0 { m = -1.0; }
        if c.state != 4 {
            let thr = if ours_step() { 0.04 * c.unit } else { p!(pv, "stepthreshold") };
            if up > 0.1 && m > thr {
                c.state = 0;
            } else if up < fail || c.airborne || c.lying {
                say!(c, "0x2a2b438", "the core told it's falling");
                c.state = 2;
            } else {
                c.state = (p!(pv, "step") != 0.0) as i32;
            }
            if c.state < 2 {
                c.h += (c.unit * p!(pv, "height") - c.h) * 0.1;
            }
        }
        c.fresh3 = false;
        if c.state == 2 {
            let hit = parts_of(GROUND_HIT_MASK).any(|k| on_ground(self, k));
            if hit {
                c.fresh3 = !c.was_ground3();
                c.state = 3;
                c.gait = 1;
                say!(c, "0x2adb528 ground hit", "state 3");
            }
            c.set_ground3(hit);
        }
        let any_step = c.legs.iter().any(|l| l.state == 1);
        if (c.state == 2 || c.state == 3) && speed <= 0.5 && spin <= 0.5 && c.lying {
            c.state = 4;
        }
        if c.state == 0 {
            c.out_t = 0.0;
            c.support_t += dt;
        } else {
            c.out_t += dt;
            if c.state == 1 { c.support_t += dt; } else { c.support_t = 0.0; }
        }
        let (leg_st, ankle_on) = self.strength(&mut c, &pv, dt);
        let kk = c.scale / (c.unit * c.unit);

        if let Some(at) = std::env::var("HORSE_REMEASURE").ok().and_then(|v| v.parse::<f32>().ok()) {
            if !c.remeasured && c.t >= at && c.state == 0 && c.legs.iter().all(|l| l.planted && l.state == 0) {
                c.remeasured = true;
                for i in 0..4 {
                    let m = c.to_local(c.legs[i].sole);
                    c.legs[i].neutral = Vec3::new(0.0, m.y, m.z);
                    c.legs[i].target = c.legs[i].neutral;
                }
                if c.tr { eprintln!("remeasured at {:.2}: {:?}", c.t, c.legs.iter().map(|l| (l.neutral.y, l.neutral.z)).collect::<Vec<_>>()); }
            }
        }
        let catch = ours_step() && c.state < 2 && !c.free && !any_step && {
            let soles = [c.legs[0].sole, c.legs[1].sole, c.legs[2].sole, c.legs[3].sole];
            let v = vel - w * vel.dot(w);
            let cap = com + v * (c.com_h.max(0.1) / 9.81).sqrt();
            let cap = Vec3::new(cap.x, cap.y, c.legs[0].sole.z);
            let mc = support_margin(&soles, cap, w);
            let edge = knob("HORSE_CATCH").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.04);
            let go = mc < edge * c.unit;
            if go {
                self.step_trigger(&mut c, &pv, cap, vel);
            }
            go
        };
        if catch {
        } else if c.free || (c.state == 1 && c.out_t > 0.0) {
            self.step_trigger(&mut c, &pv, look, vel);
        } else if c.state == 0 && p!(pv, "step") != 0.0 && !any_step && p!(pv, "restepdistance") > 0.0 {
            let u = c.u;
            let far = (0..4).map(|i| {
                let d = c.to_world(c.legs[i].neutral) - c.legs[i].sole;
                (i, (d - u * d.dot(u)).length())
            }).filter(|x| x.1 > c.unit * p!(pv, "restepdistance")).max_by(|x, y| x.1.total_cmp(&y.1));
            let far = far.or_else(|| c.shift.map(|(i, _)| (i, 0.0)));
            if let Some((i, d)) = far {
                let shifted = !knob("HORSE_SHIFT").is_some_and(|v| v != "0") || {
                    let t: Vec<Vec3> = (0..4).filter(|&j| j != i).map(|j| c.legs[j].sole).collect();
                    let waited = c.shift.filter(|s| s.0 == i).map_or(0.0, |s| s.1 + dt);
                    c.shift = Some((i, waited));
                    tri_margin([t[0], t[1], t[2]], com, w) > 0.03 * c.unit || waited > 1.5
                };
                if shifted {
                    if c.tr && c.shift.is_some() { eprintln!("shift: leg {i} lifts after {:.2} s at {:.2}", c.shift.map_or(0.0, |s| s.1), c.t); }
                    c.shift = None;
                    let to = c.to_world(c.legs[i].neutral);
                    self.step_to(&mut c, &pv, i, to);
                    say!(c, "0x2adb528 restep", "leg {i}, {d:.3} m off its place");
                }
            }
        }
        c.roll = ((spin - p!(pv, "rollvelocity")) * p!(pv, "rollscale")).clamp(0.0, 1.0);
        let pred = {
            let t = c.unit.sqrt() * p!(pv, "rollpredictiontime");
            Quat::from_scaled_axis(omega * t) * a
        };
        c.upright = (pred.dot(c.u) + 1.0) * 0.5;
        c.roll_go = (1.0 - c.roll) * c.upright;
        say!(c, "0x2ae2fd8", "roll {:.3}, upright {:.3}, go {:.3}", c.roll, c.upright, c.roll_go);
        say!(c, "0x2a35fb8", "predicted a {pred:.3}");
        let (f0, f1) = (p!(pv, "frictionScale"), p!(pv, "frictionscaleend"));
        let body_f = if c.state == 0 { f1 } else { f0 + (f1 - f0) * (c.ground_t * 0.5).min(1.0) };
        let mut hooves_f = [f0; 4];
        if ((c.state == 2 && !c.free) || c.state == 3) && !c.airborne {
            c.h += (c.unit * (p!(pv, "height") - (1.0 - c.roll_go) * 0.8) - c.h) * 0.1;
        }

        let share_k = if share_on() { std::env::var("HORSE_SHARE").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0) } else { 0.0 };
        if share_k != 0.0 {
            let stance: Vec<Vec3> = (0..4).filter(|&j| c.legs[j].state == 0 && c.legs[j].planted && c.shift.is_none_or(|s| s.0 != j)).map(|j| c.legs[j].sole).collect();
            let goal = if c.state < 2 && (c.legs.iter().any(|l| l.state == 1) || c.shift.is_some()) && stance.len() >= 2 {
                let d = com - stance.iter().sum::<Vec3>() / stance.len() as f32;
                (d - w * d.dot(w)).clamp_length_max(0.15 * c.unit) * share_k
            } else { Vec3::ZERO };
            c.share_d += (goal - c.share_d) * (dt / 0.1).min(1.0);
        }
        if let Some((i, t0)) = std::env::var("HORSE_LIFTTEST").ok().and_then(|v| v.split_once(',').and_then(|(a, b)| Some((a.parse::<usize>().ok()?.min(3), b.parse::<f32>().ok()?)))) {
            if c.legs[i].state == 0 && c.t >= t0 && c.t < t0 + 0.05 {
                let n = c.legs[i].neutral;
                let t: Vec<Vec3> = (0..4).filter(|&j| j != i).map(|j| c.legs[j].sole).collect();
                eprintln!("hlift {i} COM {:.3} m inside the other three", tri_margin([t[0], t[1], t[2]], com, w));
                if knob("HORSE_SHIFT").is_some_and(|v| v != "0") {
                    c.shift = c.shift.or(Some((i, 0.0)));
                } else {
                    self.begin_step(&mut c, &pv, i, Vec3::new(0.0, n.y, n.z));
                    c.legs[i].end = c.t + 0.4;
                }
            }
        }
        let mut targets = self.targets.clone();
        if ((c.state == 2 && !c.free) || c.state == 3) && !c.spine_gone() {
            c.sigma = 0.5 * (speed.min(1.0) + spin.min(1.0));
            c.phase += (p!(pv, "minscrabblespeed") + (p!(pv, "maxscrabblespeed") - p!(pv, "minscrabblespeed")) * c.sigma) * dt / c.unit.sqrt();
            say!(c, "0x2ae31d0", "scrabble speed {:.3}", c.sigma);
        }
        let force_sc = p!(pv, "forcescrabble") != 0.0;
        let p148 = p!(pv, "0x0a364789");
        let p194 = p!(pv, "0xbda62333");
        for i in 0..4 {
            let row = LEG[i];
            let psi = if i >= 2 { c.rig[i].side * p194 * std::env::var("HORSE_PSI").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0) } else { 0.0 };
            let st_j0 = leg_st[i] * std::env::var("HORSE_ST").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
            let ld = p!(pv, "legDamping");
            let mut request = None;
            let scrabble = if c.state == 4 {
                p!(pv, "scrabbleonside") != 0.0
            } else if force_sc {
                true
            } else if (c.state == 2 && !c.free) || c.state == 3 {
                !(c.legs[i].state == 0) && !(p!(pv, "rollonground") != 0.0 && spin > p!(pv, "rollvelocity") && !c.airborne)
            } else {
                false
            };
            if scrabble {
                request = Some((self.scrabble_target(&mut c, i), None));
                hooves_f[i] = 0.1;
            } else if c.state == 4 {
            } else if ((c.state == 2 && !c.free) || c.state == 3) && c.legs[i].state != 0 {
                let reach = c.rig[i].reach_min + (c.rig[i].reach_max - c.rig[i].reach_min) * c.roll_go;
                request = Some((c.legs[i].hip - c.rows[0] * reach, None));
                say!(c, "0x2ae3098", "leg {i}: reach {reach:.3}");
            } else if c.legs[i].state == 1 {
                let (to, lead) = self.swing(&mut c, &pv, i, &fr);
                request = Some((to, lead));
                hooves_f[i] = p!(pv, "0x1f3617d5");
            } else {
                let here = (ours_step() && c.legs[i].planted && std::env::var_os("HORSE_HOLD").is_some()).then(|| c.to_local(c.legs[i].sole));
                let anchored = (std::env::var_os("HORSE_ANCHOR").is_some() && c.legs[i].planted).then(|| {
                    let s0 = c.legs[i].sole;
                    let a = *c.legs[i].anchor.get_or_insert(s0);
                    c.to_local(a)
                });
                if std::env::var_os("HORSE_ANCHOR").is_some() && !c.legs[i].planted { c.legs[i].anchor = None; }
                let lg = &mut c.legs[i];
                if let Some(h) = anchored {
                    lg.target = Vec3::new(lg.target.x, h.y, h.z);
                } else if let Some(h) = here {
                    lg.target = Vec3::new(lg.target.x, h.y, h.z);
                } else if !(lg.planted && std::env::var_os("HORSE_NODRAG").is_some()) {
                    let d = lg.neutral - lg.target;
                    lg.target += d.clamp_length_max(1.5 * dt);
                }
                let k = LEGS[row][2];
                if pin() {
                    let lg = &mut c.legs[i];
                    if lg.planted {
                        let a = *lg.anchor.get_or_insert(lg.sole);
                        let m4 = self.doll.mass() * 0.25;
                        let om = std::f32::consts::TAU * 3.0;
                        let v = self.doll.vel(k).0;
                        let off = lg.sole - a;
                        let f = -(off - w * off.dot(w)) * m4 * om * om - (v - w * v.dot(w)) * 2.0 * m4 * om;
                        let cap = 0.8 * m4 * 9.81 * std::env::var("HORSE_PIN").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
                        if c.tr && (c.t * 60.0).round() as i32 % 60 == 0 { eprintln!("pin leg {i}: {:.0} N of {:.0} (weight share {:.0})", f.length(), cap, m4 * 9.81); }
                        if f.length() > cap {
                            lg.anchor = Some(lg.sole);
                        }
                        force[k] += f.clamp_length_max(cap);
                    } else {
                        lg.anchor = None;
                    }
                }
                if std::env::var_os("HORSE_GRIP").is_some() && c.legs[i].planted && c.legs[i].state == 0 {
                    let v = self.doll.vel(k).0;
                    self.doll.set_vel(k, w * v.dot(w));
                }
                let lg = &mut c.legs[i];
                let fsc = p!(pv, "footslipcompensation");
                if fsc > 0.0 && lg.planted && lg.state == 0 {
                    let v = self.doll.vel(k).0;
                    let vh = v - w * v.dot(w);
                    let f = -vh * vh.length() * fsc;
                    if f.length_squared() <= 810_000.0 {
                        force[k] += f;
                    }
                }
                let to = c.to_world(c.legs[i].target) + c.share_d;
                say!(c, "0x2ae3f1c", "leg {i}: stance, target {to:.3}, sole {:.3}", c.legs[i].sole);
                request = request.or(Some((to, None)));
            }
            let locked = lock() && c.state == 0 && c.legs.iter().all(|l| l.state == 0) && !c.last.is_empty();
            if !locked {
                c.locked.clear();
            } else {
                if c.locked.is_empty() {
                    c.locked = c.last.clone();
                }
                for &k in LEGS[row].iter() {
                    targets[k] = c.locked[k];
                }
            }
            if let Some((to, lead)) = request.filter(|_| !locked) {
                say!(c, "0x2a102a4", "the leg's request record: world target, flags 0x41, COM velocity, gain, swivel");
                let q = self.leg_ik(&mut c, &fr, i, to, vel, p148, psi, lead, st_j0 * kk, ld);
                for (j, &k) in LEGS[row].iter().enumerate() {
                    targets[k] = q[j];
                }
            }
        }
        let fric: Vec<(usize, f32)> = parts_of(0xfff5_fd7f).map(|k| (k, body_f)).chain(parts_of(TAIL_MASK).map(|k| (k, TAIL_FRICTION)))
            .chain((0..4).map(|i| (LEGS[LEG[i]][2], hooves_f[i]))).collect();
        self.doll.set_part_scales(&fric, &c.snap_floor);
        self.limb_limits(&mut c, &fr);

        let mut chain: Vec<Option<Vec3>> = CHAIN.iter().enumerate().map(|(i, &k)| match self.build.parts[k].limit {
            Limit::Bone { pf, cf, .. } if i > 0 => {
                let pose = angles(pf, cf, self.targets[k]);
                let centre = self.centres.get(k).map_or(0.0, |c| c.x);
                Some(Vec3::new(POSTURE[i] - centre, pose.y, pose.z))
            }
            _ => None,
        }).collect();
        say!(c, "0x2a36214", "accessor G: Neck2's record");
        say!(c, "0x2a36280", "accessor H: the head's record");
        say!(c, "0x2a10330", "the spine's request record");
        say!(c, "0x2a2b350", "the five limbs' records");
        say!(c, "0x2a5d118", "the spine's requests committed");
        let on_fire = p!(pv, "OnFire") != 0.0;
        let writhes = (p!(pv, "Writhe") != 0.0 || (p!(pv, "writheonground") != 0.0 && c.state == 4)) && !c.spine_gone();
        if writhes {
            self.writhe(&mut c, &pv, &fr, &mut chain);
        }
        let mut flail_on = false;
        if !c.free && !on_fire {
            let lift = p!(pv, "alwayslifthead") != 0.0 || c.state == 2 || c.state == 3 || (c.state == 4 && !writhes);
            if lift {
                self.head(&mut c, &pv, &fr, &mut chain, omega, rows);
            }
        } else {
            if c.flail_t < c.t {
                c.flail_t = c.t + 0.3 + c.rand(0) * 0.3;
                c.flail_sign = -c.flail_sign;
                c.flail = c.rand(0) * 3.0 - 1.0;
            }
            let spine = c.flail + 0.3;
            let (side, twist) = (c.flail_sign, c.flail_sign * 3.0);
            flail_on = true;
            for (i, x) in chain.iter_mut().enumerate().skip(1) {
                if let Some(x) = x {
                    let (n, lean) = if i < 4 { (3.0, spine) } else { (4.0, c.flail) };
                    let mid = self.centres.get(CHAIN[i]).copied().unwrap_or(Vec3::ZERO);
                    *x = Vec3::new(lean / n - mid.x, -(side / n - mid.y), twist / n - mid.z);
                }
            }
            say!(c, "0x2ae0318", "the spine bends {:.3}, {:.3}, {:.3} over three", spine, side, twist);
            say!(c, "0x2ae074c", "the neck bends {:.3}, {:.3}, {:.3} over four", c.flail, side, twist);
            say!(c, "0x2ae11f8", "one stiffness on all eight, {FLAIL_STIFF}");
        }
        for (i, &k) in CHAIN.iter().enumerate() {
            if let (Some(x), Limit::Bone { pf, cf, .. }) = (chain[i], self.build.parts[k].limit) {
                targets[k] = from_angles(pf, cf, x);
            }
        }
        let rt = p!(pv, "rightingTorque");
        if c.on_parts && rt > 1e-6 && spin <= 0.5 && speed <= 0.5 {
            let t = Quat::from_scaled_axis(omega * c.unit.sqrt() * p!(pv, "rollpredictiontime")) * b;
            let s = -a.dot(c.ground_n);
            if s > 0.0 {
                let side = w.dot(t);
                let s = if side < 0.0 { -s } else if side == 0.0 { 0.0 } else { s };
                for k in [0, 5, 11, 12] {
                    torque[k] += cc * s * rt;
                }
                say!(c, "0x2adb528 righting", "{:.3} about c", s * rt);
            }
        }
        if let (Some((part, _, imp)), true) = (snap_hit, p!(pv, "Snap") != 0.0) {
            self.body_snap(&mut c, &pv, part, imp, rows, &mut torque);
        }
        let (wz, wr) = (p!(pv, "initialWeaknessZeroDuration"), p!(pv, "initialWeaknessRampDuration"));
        let mut hit_scale = None;
        let mut joint_scale = None;
        if wz <= c.weak_clock && wr <= 1e-6 {
            c.blend = 1.0;
        }
        let (ft, fs) = (p!(pv, "floorWeaknessTime"), p!(pv, "floorStrength"));
        if ft > 0.0 && c.since_hit < ft && fs > 0.0 && fs < 1.0 {
            hit_scale = Some(fs);
            say!(c, "0x2a50ff4", "the joints but the ankles and tails scaled {fs}");
        }
        let (jt, js) = (p!(pv, "floorJointWeaknessTime"), p!(pv, "floorJointStrength"));
        if jt > 0.0 && c.since_hit < jt && c.since_hit < 100.0 {
            joint_scale = Some((part_rdr2(c.hit_part), js));
            say!(c, "0x2a3530c", "the joints round part {} at {js}", part_rdr2(c.hit_part));
        }
        if wz <= c.weak_clock && wr > 1e-6 {
            c.blend = (dt / wr + c.blend).min(1.0);
        }
        say!(c, "0x2ac7dac", "request blend {:.3} after {:.3} s", c.blend, c.weak_clock);
        c.writhe_t += dt;
        if !c.airborne && (2..5).contains(&c.state) {
            self.down_torques(&mut c, &pv, omega, rows, vel, dt, &mut torque);
        }
        if c.state != c.was_state {
            if c.tr {
                eprintln!("quadruped feedback: state {} at {:.2} s (hooves {}, up {:.3}, margin {:.3}, H {:.3}, COM h {:.3})", c.state, c.t, c.legs.iter().filter(|l| l.planted).count(), up, m, c.h, c.com_h);
            }
            c.was_state = c.state;
        }
        if c.tr && (c.t * 60.0).round() as i32 % 60 == 0 {
            let s: Vec<String> = (0..4).map(|i| { let l = c.to_local(c.legs[i].sole); let g = c.to_local(c.to_world(c.legs[i].target)); format!("{}{} g{} ({:+.2} {:+.2} {:+.2})>({:+.2} {:+.2}) w({:.3} {:.3})", if c.legs[i].planted { 'P' } else { '-' }, c.legs[i].state, c.legs[i].gaps, l.x, l.y, l.z, g.y, g.z, c.legs[i].sole.x, c.legs[i].sole.y) + &{ let (v, w) = self.doll.vel(LEGS[LEG[i]][2]); format!(" v{:.2} spin{:.2}", v.length(), w.length()) } }).collect();
            eprintln!("sample t {:.0} st {} up {:.3} m {:.3} com {:.2} {:.2} {:.2} v {:.3} w {:.3} fs {:.2} fl {:.2} mbt {:.1} sup {:.1} {}", c.t, c.state, a.dot(w), m, com.x, com.y, com.z, speed, spin, c.fs, c.fl, p!(pv, "maxBalanceTime"), c.support_t, s.join(" "));
            c.legs.iter_mut().for_each(|l| l.gaps = 0);
        }
        if !self.dead {
            let t0: Vec3 = torque.iter().sum();
            self.stay_upright(&mut c, &pv, &fr, com, omega, rows, vel, dt, &mut torque);
            if std::env::var_os("HORSE_HOOFLOG").is_some() { let t1: Vec3 = torque.iter().sum::<Vec3>() - t0; eprintln!("net upright {:.1} {:.1} {:.1}", t1.x, t1.y, t1.z); }
        }
        let hyb = super::ours::hybrid() && !self.dead;
        if c.state < 2 && !(hyb && std::env::var_os("HORSE_HYBNOGS").is_some()) {
            let t0: Vec3 = torque.iter().sum();
            self.gravity_solver(&mut c, &fr, com, &mut torque);
            if std::env::var_os("HORSE_HOOFLOG").is_some() { let t1: Vec3 = torque.iter().sum::<Vec3>() - t0; eprintln!("net gravity {:.1} {:.1} {:.1}", t1.x, t1.y, t1.z); }
        }
        if hyb && c.state < 2 {
            let planted = std::array::from_fn(|r| c.legs[LEG.iter().position(|&x| x == r).unwrap_or(0)].planted);
            self.hyb_support(&fr, planted, com, vel, &mut torque);
        }
        c.first = false;

        let leg_d = p!(pv, "legDamping");
        let dead = self.dead;
        let (sk, sd) = (p!(pv, "spineStiffness") * kk, p!(pv, "spineDamping"));
        if c.last.len() != n {
            c.last = targets.clone();
        }
        let blend = c.blend;
        let muscles: Vec<Muscle> = (0..n).map(|k| {
            let leg = LEGS.iter().position(|l| l.contains(&k));
            let (own, ratio) = self.muscles[k];
            let (mut st, mut dm) = if WELDED.contains(&k) {
                (WELD, 1.0)
            } else if dead {
                (0.0, ratio)
            } else if let Some(l) = leg {
                let i = LEG.iter().position(|&x| x == l).unwrap_or(0);
                if k == LEGS[l][2] && ankle_on[i] {
                    (p!(pv, "legStiffness") * kk * p!(pv, "anklestrengthscale"), leg_d)
                } else {
                    (leg_st[i] * kk, leg_d)
                }
            } else if CHAIN.contains(&k) && k != CHAIN[0] {
                if flail_on { (FLAIL_STIFF * kk, p!(pv, "spineDamping")) } else { (c.fs2 * sk, sd) }
            } else {
                (own, ratio)
            };
            let mut target = targets[k];
            if !WELDED.contains(&k) && !dead && (leg.is_some() || CHAIN.contains(&k)) {
                st = own + (st - own) * blend;
                dm = ratio + (dm - ratio) * blend;
                target = c.last[k].slerp(target, blend);
            }
            if !WELDED.contains(&k) {
                if let Some(s) = hit_scale.filter(|_| parts_of(WEAK_MASK).any(|p| p == k)) {
                    let f = s.sqrt();
                    st *= f;
                    dm *= f;
                }
                if let Some((p, s)) = joint_scale {
                    let joints = PART_JOINTS.get(p).copied().unwrap_or([-1; 3]);
                    if joints.iter().any(|&j| j >= 0 && link_part(j as usize + 1) == k) {
                        st *= s.sqrt();
                        dm *= s.sqrt();
                    }
                }
            }
            if let Some(l) = leg {
                st *= (1.0 - self.legs[l] / LAME).clamp(0.08, 1.0);
                if hyb && c.legs[LEG.iter().position(|&x| x == l).unwrap_or(0)].planted {
                    let x = std::env::var(if k == LEGS[l][0] { "HORSE_HYBHIP" } else { "HORSE_HYBLEG" }).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
                    st *= x;
                    dm *= x.sqrt();
                }
            }
            Muscle { target, stiffness: st, damping: dm.max(1e-3) }
        }).collect();
        c.last = muscles.iter().map(|m| m.target).collect();
        let ms = p!(pv, "0x2128bd54");
        let most = if c.unit == 1.0 { 1.0 } else { c.scale * c.scale * c.unit };
        if c.tr && c.said.insert("sizes") { eprintln!("sizes: mass {:.1} kg, unit {:.3}, scale {:.3}, most {most:.3}", self.doll.mass(), c.unit, c.scale); }
        if c.tr && c.said.insert("legmass") { for l in LEGS.iter() { eprintln!("legmass {:?}: {:?}", l.iter().map(|&k| self.build.parts[k].name.clone()).collect::<Vec<_>>(), l.iter().map(|&k| (self.doll.part_mass(k) * 100.0).round() / 100.0).collect::<Vec<_>>()); } }
        self.rage_drive(&muscles, ms, most, dead, dt);
        self.doll.rules = if std::env::var_os("HORSE_GTA4RULES").is_some() { Rules::Gta4 } else { Rules::Rdr2 };
        self.doll.limit_row = vec![None; n];
        for l in LEGS.iter() {
            for &k in &l[1..] {
                if let Limit::BoneHinge { lo, hi, .. } = self.build.parts[k].limit {
                    let hock = k == l[2] && (0..4).any(|i| LEGS[c.rig[i].row][2] == k && self.r.limits[i].is_some_and(|x| x.kind == 3));
                    self.doll.limit_row[k] = Some(if hock { LimitRow { lo, hi, strength: 10000.0, breaks: false, blend: 0.5, soft: 0.5, k: 0.01 } } else { LimitRow { lo, hi, strength: f32::MAX, breaks: false, blend: 1.0, soft: 0.5, k: 0.0 } });
                    if hock {
                        say!(c, "0x2a3bc68", "hock part {k}: window {lo:.3}..{hi:.3}, strength 10000, softness .5, .01");
                    }
                }
            }
        }
        self.doll.sleep_tol = [0.0; 3];
        self.kept.force = force;
        self.kept.torque = torque;
        self.kept.muscles = muscles;
        self.core = Some(c);
    }

    fn new_hit(&self, c: &mut Core, pv: &Params, part: usize, imp: Vec3, rows: [Vec3; 3]) {
        let [a, b, cc] = rows;
        if c.blend >= 1.0 {
            c.blend = 0.01;
            c.weak_clock = 0.0;
        }
        c.hit_part = part;
        let (mut y, z) = (imp.dot(b), imp.dot(cc));
        let lim = p!(pv, "0xf08a586f");
        let mut v = b * y + cc * z;
        let along = (z / y.hypot(z).max(1e-6)).abs();
        let angle = along.clamp(0.0, 1.0).acos();
        if angle < lim {
            let side = if y < 0.0 { -1.0 } else { 1.0 };
            let way = if z < 0.0 { -side } else { side };
            v = Quat::from_axis_angle(a, -(lim * way) - way * angle) * v;
            y = v.dot(b);
        }
        let u = c.u;
        let v = (v - u * v.dot(u)).normalize_or_zero();
        if p!(pv, "0xcdd11768").abs() > 1e-7 {
            c.push_lean = Some((v, p!(pv, "0xcdd11768")));
        }
        if p!(pv, "0x463334cb").abs() > 1e-7 {
            c.push_force = Some((v, p!(pv, "0x463334cb")));
        }
        c.since_hit = 0.0;
        say!(c, "0x2ad286c", "new hit on part {part}: direction {v:.3}, side {y:.3}, lean {}, push {}", p!(pv, "0xcdd11768"), p!(pv, "0x463334cb"));
    }

    fn mean_contact(&self, c: &mut Core) {
        let (mut nn, mut pp, mut count) = (Vec3::ZERO, Vec3::ZERO, 0);
        for (k, h) in self.doll.hit.iter().enumerate() {
            if let (Some(h), true) = (h, self.doll.touch[k] & 0x20 != 0) {
                nn += h.1;
                pp += h.0;
                count += 1;
            }
        }
        if count > 0 {
            c.ground_n = (nn / count as f32).normalize_or(Vec3::Z);
            c.ground_p = pp / count as f32;
            say!(c, "0x2ae2c10", "{count} contacts, normal {:.3}, at {:.3}", c.ground_n, c.ground_p);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn support(&self, c: &mut Core, pv: &Params, fr: &[Transform], rows: [Vec3; 3], com: Vec3, look: Vec3, dt: f32, force: &mut [Vec3]) {
        let w = Vec3::Z;
        let lean = c.push_lean.map_or(Vec3::ZERO, |(v, s)| v * s);
        let along = lean.dot(w);
        c.u = (w + lean - w * along).normalize_or(w);
        if c.state < 2 {
            if let Some((v, s)) = c.push_force {
                let f = flat(v, w, 0.0).normalize_or_zero() * s;
                for k in [5, 11, 12, 15] {
                    force[k] += f;
                }
                say!(c, "0x2ac70a4 push", "{:.1} N", f.length());
            }
        }
        for i in 0..4 {
            let k = LEGS[LEG[i]][2];
            let touching = self.doll.touch[k] & 0x20 != 0 && self.doll.hit[k].is_some_and(|h| h.1.dot(w) > 0.5);
            let lg = &mut c.legs[i];
            lg.hold = if touching { HOLD } else { (lg.hold - dt).max(0.0) };
            lg.gaps += (!touching) as u32;
            let was = lg.planted;
            lg.planted = lg.hold > 0.0;
            if lg.planted && !was && lg.state == 0 {
                say!(c, "0x2ac70a4 planted", "leg {i} just planted");
            }
        }
        let ramp = p!(pv, "rampheighttime");
        let h = if ramp > 0.1 { c.h - c.unit * 0.5 * ((c.support_t - 1.0).clamp(0.0, ramp) / ramp) } else { c.h };
        let (y, s) = (fr[MID].rotation * self.spine_axis, fr[MID].translation + fr[MID].rotation * self.spine_cg);
        let drop = h - (s - com).dot(y);
        let origin = com - y * drop;
        let q = Quat::from_rotation_arc(c.u, w);
        let mut fw = [q * rows[0], q * rows[1], q * rows[2]];
        let (mut roll, mut pitch) = (p!(pv, "rolloffset"), p!(pv, "pitchoffset"));
        if c.state == 0 {
            let most = p!(pv, "maxsupporttime");
            if (most > -1.0 && most < c.support_t) || c.com_h < 0.5 * LEG_LEN {
                roll += (c.support_t - most).clamp(0.0, 0.5) * 0.6 * c.sign;
                say!(c, "0x2ac70a4 sagging", "roll {roll:.3}, support {:.2}, COM h {:.3}", c.support_t, c.com_h);
            } else {
                let u = c.u;
                let k = origin.dot(u) - look.dot(u);
                let d = look + u * k - origin;
                let d = d - u * d.dot(u);
                let bh = (fw[1] - u * fw[1].dot(u)).normalize_or_zero();
                let ch = (fw[2] - u * fw[2].dot(u)).normalize_or_zero();
                let (x, z) = (bh.dot(d).clamp(-1.0, 1.0), ch.dot(d).clamp(-1.0, 1.0));
                let asin = |v: f32| v.clamp(-1.0, 1.0).asin();
                roll += asin(x * c.h) * p!(pv, "0xc55d4193");
                pitch -= asin(z * c.h) * p!(pv, "0x1179b8be");
            }
        }
        let tk = if std::env::var_os("HORSE_TILTSTEP").is_some() && (c.state != 0 || c.legs.iter().any(|l| l.state != 0)) || c.shift.is_some() { 1.0 }
                 else { std::env::var("HORSE_TILT").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0) };
        let (pitch, roll) = ((pitch * tk).clamp(-0.3, 0.3), (roll * tk).clamp(-0.3, 0.3));
        let corner = (if pitch > 0.0 { 0 } else { 2 }) + (roll <= 0.0) as usize;
        let n = c.legs[corner].neutral;
        let raise = roll.abs().sin() * n.y.abs() + pitch.abs().sin() * n.z.abs();
        let turn = Quat::from_axis_angle(fw[2], roll) * Quat::from_axis_angle(fw[1], pitch);
        c.origin = origin + fw[0] * raise;
        fw = fw.map(|v| turn * v);
        c.rows = fw;
        if c.tr && std::env::var_os("HORSE_FK").is_some() && (c.t * 60.0).round() as i32 % 30 == 0 {
            let zs: Vec<String> = (0..4).map(|i| format!("{i}:{:+.3}/z{:+.2}", c.to_world(c.legs[i].neutral).z - c.ground_p.z, c.legs[i].neutral.z)).collect();
            eprintln!("tilt t {:.1} pitch {pitch:+.3} roll {roll:+.3} corner {corner} raise {raise:.3} neutral heights {}", c.t, zs.join(" "));
        }
        say!(c, "0x2ac70a4", "frame origin {:.3}, a {:.3}, roll {roll:.3}, pitch {pitch:.3}, H {h:.3}", c.origin, c.rows[0]);
        say!(c, "0x2a3f08c", "the plane projection");
    }

    fn strength(&self, c: &mut Core, pv: &Params, dt: f32) -> ([f32; 4], [bool; 4]) {
        let (ls, ss) = (p!(pv, "legStiffness"), p!(pv, "spineStiffness"));
        if c.tr && c.said.insert("ss") { eprintln!("stiffness: legs {ls:.2} spine {ss:.2}"); }
        let (mut rate, mut floor) = (p!(pv, "strengthdecay"), p!(pv, "spinestiffnessmin") / ss);
        let leg_floor = p!(pv, "legStiffnessMin") / ls;
        let down_floor = p!(pv, "0xe4adac13") / ss;
        if c.state == 4 {
            rate = p!(pv, "0xd5842c66");
            floor = down_floor;
        }
        if !c.airborne {
            c.fs = (c.fs - rate * dt).max(floor);
            if c.fs == 0.0 { say!(c, "0x2ae39f0 gone", "the spine's strength is 0"); }
        }
        if c.fresh3 {
            c.fs2 = down_floor;
        }
        let tgt = c.fs.max(floor);
        c.fs2 += (tgt - c.fs2) * rate.max(0.2);
        let mut side = [1.0, 1.0];
        let mbt = p!(pv, "maxBalanceTime");
        if c.state >= 2 || mbt < 0.0 {
            c.fl = c.fs.min(c.fl).max(leg_floor);
        } else if mbt < c.support_t {
            let v = (1.0 - (c.support_t - mbt).clamp(0.0, 2.0) * 0.5).max(3.0 / ls);
            side[if c.tilt > 0.0 { 0 } else { 1 }] = v;
            say!(c, "0x2ae39f0 gives way", "{v:.3} on the side it leans to");
        }
        if !c.free && c.state > 1 {
            c.legs.iter_mut().for_each(|l| l.state = 2);
        }
        let by = [c.fl.min(side[1]), c.fl.min(side[0]), c.fl.min(side[1]), c.fl.min(side[0])];
        let mut st = [0.0; 4];
        let mut ankle = [false; 4];
        for i in 0..4 {
            let f = if c.legs[i].state == 1 { 1.0 } else { by[i] };
            st[i] = f * ls;
            ankle[i] = c.state < 2 && c.legs[i].state == 0;
        }
        say!(c, "0x2ae39f0", "spine {:.3}, legs {:.3}", c.fs2, c.fl);
        (st, ankle)
    }

    fn step_trigger(&self, c: &mut Core, pv: &Params, look: Vec3, vel: Vec3) {
        if c.legs.iter().any(|l| l.state == 1) {
            return;
        }
        if ours_step() {
            let u = c.u;
            let flat = |v: Vec3| v - u * v.dot(u);
            let soles = [c.legs[0].sole, c.legs[1].sole, c.legs[2].sole, c.legs[3].sole];
            let mid = soles.iter().sum::<Vec3>() / 4.0;
            let fall = flat(look - mid);
            if std::env::var_os("HORSE_KICK").is_some() && fall.length() > 1e-3 {
                let dir = fall.normalize();
                let best = (0..4).filter(|&i| (flat(c.legs[i].hip) - flat(mid)).dot(dir) > 0.0).map(|i| {
                    let hip = c.legs[i].hip;
                    let reach = c.rig[i].reach_max * 0.95;
                    let down = (hip - soles[i]).dot(u).max(0.1);
                    let out = (reach * reach - down * down).max(0.0).sqrt() * 0.7;
                    let to = flat(soles[i]) + u * soles[i].dot(u) + dir * (out - (flat(soles[i]) - flat(hip)).dot(dir)).max(0.05);
                    let mut q = soles;
                    q[i] = to;
                    (i, to, support_margin(&q, look, u))
                }).max_by(|a, b| a.2.total_cmp(&b.2));
                if let Some((i, to, m)) = best {
                    say!(c, "ours kick", "leg {i} out, margin then {m:.3}");
                    if c.tr { eprintln!("kick leg {i} to {:.3} {:.3} from {:.3} {:.3} at {:.2}", to.x, to.y, soles[i].x, soles[i].y, c.t); }
                    c.last_leg = i;
                    self.step_to(c, pv, i, to);
                }
                return;
            }
            let best = (0..4).map(|i| {
                let hip = c.legs[i].hip;
                let kx = std::env::var("HORSE_CATCHX").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.3);
                let mut to = soles[i] + fall.clamp_length_max(c.unit * p!(pv, "maxsteplength")) * kx;
                let dz = (hip - to).dot(u);
                let room = (c.rig[i].reach_max * 0.95).powi(2) - dz * dz;
                let off = flat(to - hip);
                if room > 0.0 && off.length() > room.sqrt() {
                    to = to - off + off.normalize() * room.sqrt();
                }
                let mut q = soles;
                q[i] = to;
                let o: Vec<Vec3> = (0..4).filter(|&k| k != i).map(|k| flat(soles[k])).collect();
                let lift = tri_margin([o[0], o[1], o[2]], flat(look), u);
                (i, to, if lift > 0.0 { support_margin(&q, look, u) } else { -10.0 + lift })
            }).max_by(|a, b| a.2.total_cmp(&b.2));
            if let Some((i, to, m)) = best {
                say!(c, "ours step", "leg {i}, margin then {m:.3}");
                c.last_leg = i;
                self.step_to(c, pv, i, to);

                if std::env::var_os("HORSE_PAIR").is_some() {
                    let j = FORE_HIND[i];
                    if (flat(c.legs[j].hip) - flat(mid)).dot(fall) > 0.0 {
                        let d = to - soles[i];
                        self.step_to(c, pv, j, soles[j] + d);
                    }
                }
            }
            return;
        }
        let leg = if c.last_leg < 4 {
            if c.gait == 1 || c.gait == 2 { SIDE[c.last_leg] } else { FORE_HIND[c.last_leg] }
        } else {
            let init = p!(pv, "initialstep");
            let pick = if init >= 0.0 {
                init as i32
            } else if c.first_gait == 0 || p!(pv, "0x839ff632") == 0.0 {
                let mut best = (-1, 10_000.0);
                for i in 0..4 {
                    let d = (look - c.legs[i].hip).length();
                    if d < best.1 { best = (i as i32, d); }
                }
                best.0
            } else {
                self.first_leg(c, pv, vel) as i32
            };
            pick.max(0) as usize
        };
        say!(c, "0x2ac39e4", "the first leg");
        if c.gait == -1 {
            c.gait = 1;
            let g = p!(pv, "0x8c66dab4") as i32;
            if !(g == 1 || g == 2) {
                let va = c.rows[2].dot(vel).abs();
                if va > 1.0 {
                    c.gait = 3;
                } else if c.rows[1].dot(vel).abs() > 0.5 {
                    c.gait = 2;
                }
            }
        }
        c.last_leg = leg;
        let u = c.u;
        let d = look - c.origin;
        let d = (d - u * d.dot(u)) * 1.3;
        let d = d.clamp_length_max(c.unit * p!(pv, "maxsteplength"));
        self.step_pair(c, pv, leg, d);
        let pair = match c.gait {
            0 => None,
            1 => Some(DIAGONAL[leg]),
            2 => Some(FORE_HIND[leg]),
            _ => Some(SIDE[leg]),
        };
        let pair = pair.filter(|_| std::env::var_os("HORSE_ONE").is_none());
        if let Some(o) = pair {
            self.step_pair(c, pv, o, d);
        }
        let weak = p!(pv, "0x1689ff1e");
        if weak > 0.0 {
            c.fl = (c.fl - weak).max(5.0 / p!(pv, "legStiffness"));
        }
        say!(c, "0x2acebbc", "leg {leg} with {pair:?}, gait {}, vector {d:.3}", c.gait);
    }

    fn first_leg(&self, c: &mut Core, pv: &Params, vel: Vec3) -> usize {
        let g = p!(pv, "gaitstate") as i32;
        if g == 1 || g == 2 {
            c.gait = 1;
            (0..4).max_by(|&x, &y| c.legs[x].vel.dot(vel).total_cmp(&c.legs[y].vel.dot(vel))).unwrap_or(1)
        } else if g == 4 {
            c.gait = 3;
            0
        } else {
            c.gait = 1;
            1
        }
    }

    fn step_pair(&self, c: &mut Core, pv: &Params, i: usize, d: Vec3) {
        let mut l = c.to_local(c.legs[i].hip + d);
        l.x = 0.0;
        if c.gait == 3 && l.y.abs() < c.width[0] {
            l.y = if i & !2 != 0 { -c.width[0] } else { c.width[0] };
        }
        self.begin_step(c, pv, i, l);
        say!(c, "0x2ae0dc8", "leg {i} to {l:.3}, gait {}", c.gait);
    }

    fn step_to(&self, c: &mut Core, pv: &Params, i: usize, to: Vec3) {
        let mut l = c.to_local(to);
        l.x = 0.0;
        self.begin_step(c, pv, i, l);
        say!(c, "0x2ae1008", "leg {i} to {l:.3}");
    }

    fn begin_step(&self, c: &mut Core, pv: &Params, i: usize, l: Vec3) {
        let f = c.to_local(c.legs[i].sole);
        let len = Vec3::new(0.0, l.y - f.y, l.z - f.z).length();
        let t = c.t;
        if std::env::var_os("HORSE_STEPLOG").is_some() { eprintln!("hstep {i} {t:.2} {len:.3}"); }
        let lg = &mut c.legs[i];
        lg.target = l;
        lg.state = 1;
        lg.anchor = None;
        lg.aim = None;
        lg.left = len;
        lg.start = t;
        let least = knob("HORSE_SWINGMIN").and_then(|v| v.parse::<f32>().ok()).unwrap_or(if super::ours::hybrid() { HYB_SWINGMIN } else { 0.0 }) * c.unit.sqrt();
        let lg = &mut c.legs[i];
        let sts = if super::ours::hybrid() && !std::env::var("HORSE_SET").is_ok_and(|v| v.contains("steptimescale")) { HYB_STEPTIME } else { p!(pv, "steptimescale") };
        lg.end = t + (len.max(0.1) * sts).max(least);
    }

    fn swing(&self, c: &mut Core, pv: &Params, i: usize, fr: &[Transform]) -> (Vec3, Option<(f32, f32)>) {
        let mut world = c.to_world(c.legs[i].target);
        if knob("HORSE_WORLDSWING").is_some_and(|v| v != "0") {
            world = *c.legs[i].aim.get_or_insert(world);
        }
        let on_ground = Vec3::new(world.x, world.y, c.ground_p.z);
        c.legs[i].target = c.to_local(on_ground);
        say!(c, "0x2ae3470", "leg {i}: target at the ground's height {:.3}", c.ground_p.z);
        let target = c.legs[i].target;
        let f = c.to_local(self.sole_at(c, fr, i));
        let t = c.t;
        let height = p!(pv, "stepheightscale");
        let lg = &mut c.legs[i];
        let mut lift = 0.0;
        let done;
        if t < lg.end {
            let d = Vec3::new(0.0, f.y - target.y, f.z - target.z).length().min(lg.left);
            lg.left = d;
            lift = (1.0 - (t - lg.start) / (lg.end - lg.start)) * height * d.clamp(0.0, 0.15) * 6.666_666_5;
            let early = knob("HORSE_SWINGMIN").is_none() || (t - lg.start) > (lg.end - lg.start) * 0.5;
            done = early && (d < 0.03 || ((t - lg.start) > (lg.end - lg.start) * 0.5 && lg.planted && d < 0.1));
        } else {
            done = true;
        }
        if done {
            lg.state = 0;
            if std::env::var_os("HORSE_STEPLOG").is_some() { eprintln!("hland {i} {t:.2} after {:.2} s, {:.3} m short", t - lg.start, Vec3::new(0.0, f.y - target.y, f.z - target.z).length()); }
        }
        let to = c.to_world(target) + Vec3::Z * lift * 0.5;
        let mut lead = None;
        if p!(pv, "0x3ba69a5e") != 0.0 {
            let p144 = p!(pv, "0xbf41a9ff");
            let frac = if p144 >= 0.0 {
                p144
            } else {
                let hoof = self.doll.vel(LEGS[LEG[i]][2]).0;
                let foot = self.sole_at(c, fr, i);
                ((to - (hoof + foot)).dot(Vec3::Z) * 4.0 + 0.1).clamp(0.15, 0.6)
            };
            lead = Some((frac, p!(pv, "0x14c")));
        }
        say!(c, "0x2ae3574", "leg {i}: swing, lift {lift:.3}, {:.3} m left", c.legs[i].left);
        (to, lead)
    }

    fn scrabble_target(&self, c: &mut Core, i: usize) -> Vec3 {
        let [a, b, _] = c.rows;
        let s = c.sigma;
        let centre = c.legs[i].hip - a * (0.8 + 0.2 * s);
        let r = 0.2 + 0.18 * (1.0 - s);
        let th = c.phase + i as f32 * FRAC_PI_2;
        say!(c, "0x2ae324c", "leg {i}: radius {r:.3}, angle {th:.3}");
        centre + Quat::from_axis_angle(b, th) * a * r
    }

    fn point_chain(&self, c: &mut Core, fr: &[Transform], chain: &mut [Option<Vec3>], mask: &[usize], dir: Vec3, frac: f32) {
        let share = frac / mask.len() as f32;
        for &i in mask {
            self.point_joint(fr, chain, i, dir, share);
        }
        say!(c, "0x2a41764", "{} joints at {dir:.3}, {share:.3} each", mask.len());
        say!(c, "0x2a42b64", "the turn from a joint's axis to the direction, its share, as lean and twist");
    }

    fn point_joint(&self, fr: &[Transform], chain: &mut [Option<Vec3>], i: usize, dir: Vec3, share: f32) {
        let k = CHAIN[i];
        let (Limit::Bone { pf, .. }, Some(p)) = (self.build.parts[k].limit, self.build.parts[k].parent) else { return };
        if chain[i].is_none() {
            return;
        }
        let local = (fr[p].rotation * pf).inverse() * dir;
        let q = Quat::IDENTITY.slerp(Quat::from_rotation_arc(Vec3::Z, local.normalize_or(Vec3::Z)), share.min(1.0));
        let tw = Quat::from_xyzw(0.0, 0.0, q.z, q.w).normalize();
        let sw = (q * tw.inverse()).to_scaled_axis();
        chain[i] = Some(Vec3::new(sw.y, sw.x, 2.0 * tw.z.atan2(tw.w)));
    }

    fn writhe(&self, c: &mut Core, pv: &Params, fr: &[Transform], chain: &mut [Option<Vec3>]) {
        let x = c.writhe_t * p!(pv, "writhespeed") / c.unit.sqrt();
        let (yaw, pit) = ((noise(x, c.seed[0]) + 0.5) * PI - FRAC_PI_2, (noise(x, c.seed[1]) + 0.5) * PI - FRAC_PI_2);
        let w = Vec3::Z;
        let e = c.rows[2].cross(w).normalize_or(Vec3::X);
        let f = e.cross(w);
        let v = Quat::from_euler(EulerRot::XYZ, yaw, pit, 0.0) * Vec3::Y;
        let dir = (f * v.y + e * v.x + w * v.z).normalize_or(w);
        self.point_chain(c, fr, chain, &[0, 1, 2, 4, 5, 6, 7], dir, 1.0);
        say!(c, "0x2aca3e4", "direction {dir:.3}");
    }

    fn head(&self, c: &mut Core, pv: &Params, fr: &[Transform], chain: &mut [Option<Vec3>], omega: Vec3, rows: [Vec3; 3]) {
        let q = Quat::from_scaled_axis(-omega * p!(pv, "0xdcaa8fcc"));
        let want = (q * c.ground_n).normalize_or(Vec3::Z);
        let fwd = -rows[2];
        let arc = Quat::from_rotation_arc(fwd, want);
        let (axis, ang) = arc.to_axis_angle();
        let dir = if ang > 1.2 { Quat::from_axis_angle(axis, 1.2) * fwd } else { want };
        self.point_chain(c, fr, chain, &[0, 1, 2, 3, 4, 5, 6, 7], dir, p!(pv, "0x78748c58"));
        say!(c, "0x2ade20c", "the head at {dir:.3}, {ang:.2} rad from forward");
    }

    #[allow(clippy::too_many_arguments)]
    fn down_torques(&self, c: &mut Core, pv: &Params, omega: Vec3, rows: [Vec3; 3], vel: Vec3, dt: f32, torque: &mut [Vec3]) {
        let w = Vec3::Z;
        let cc = rows[2];
        let ch = flat(cc, w, 0.0).normalize_or_zero();
        let d = c.ground_n;
        let mut dh = flat(d, w, 0.0);
        if p!(pv, "0xdf7a800d") != 0.0 && w.dot(d) >= 0.8 {
            dh = flat(vel, w, 0.0);
        }
        let dh = dh.normalize_or_zero();
        let speed = vel.length();
        let (fast, slow) = (p!(pv, "0x6ded80a2"), p!(pv, "0x33a9d629"));
        let r = if speed < p!(pv, "0x18b9c869") && fast > slow { fast } else { slow } * dt;
        c.fade[0] = (c.fade[0] - r).max(0.0);
        c.fade[1] = (c.fade[1] - 2.0 * r).max(0.0);
        if c.fade[0] < 1.192_092_9e-7 {
            return;
        }
        let k = 1.0 / 5.0;
        if c.fade[1] > 0.0 && dh != Vec3::ZERO && ch != Vec3::ZERO {
            let mut th = ch.dot(dh).clamp(-1.0, 1.0).acos();
            if th > FRAC_PI_2 {
                th -= PI;
            }
            if th.abs() > 0.1 {
                let way = ch.cross(dh).dot(w).signum();
                let lim = p!(pv, "0x75a875d8");
                let y = (th * dt * 10.0).clamp(-lim, lim) * k * c.fade[1] * 700.0 * way;
                for p in PUSHED {
                    torque[p] += w * y;
                }
                say!(c, "0x2ac3f28 yaw", "{y:.1} N m");
            }
        }
        let wc = omega.dot(ch);
        let lim = p!(pv, "0x65b9a730");
        let cd = ch.dot(dh);
        let rate = ((p!(pv, "0x84cf43be") * cd - wc) / dt).clamp(-lim, lim);
        let t = rate * k * c.scale * c.scale * c.unit * 500.0 * c.fade[0];
        if (cd <= 0.0 && t <= 0.0) || (cd > 0.0 && t >= 0.0) {
            for p in PUSHED {
                torque[p] += ch * t;
            }
            say!(c, "0x2ac3f28", "roll {t:.1} N m, fades {:.3} {:.3}", c.fade[0], c.fade[1]);
        }
    }

    fn body_snap(&self, c: &mut Core, pv: &Params, part: usize, imp: Vec3, rows: [Vec3; 3], torque: &mut [Vec3]) {
        let [_, b, cc] = rows;
        let d = imp.normalize_or_zero();
        let cross = if d.dot(cc).abs() >= 0.5 { d.cross(b) } else { d.cross(cc) };
        let rp = part_rdr2(part);
        let joints = PART_JOINTS.get(rp).copied().unwrap_or([-1; 3]);
        say!(c, "0x2a35358", "part {rp}: joints {joints:?}");
        let mut mag = p!(pv, "snapMag");
        if joints[1] >= 0 {
            mag *= 0.5;
        }
        for &j in joints.iter().filter(|&&j| j >= 0) {
            let j = j as usize;
            let v = cross + Vec3::new(c.rand(0) * 16.0 - 8.0, c.rand(0) * 16.0 - 8.0, c.rand(0) * 16.0 - 8.0);
            let t = v.normalize_or_zero() * mag * SNAP_JOINT[j];
            let k = link_part(j + 1);
            torque[k] += t;
            if let Some(p) = self.build.parts[k].parent {
                torque[p] -= t;
            }
            say!(c, "0x2a57164", "joint {j}: {:.1} N m", t.length());
            if matches!(self.build.parts[k].limit, Limit::Cone { .. } | Limit::Bone { .. }) && !c.snap_floor.iter().any(|f| f.0 == k) {
                c.snap_floor.push((k, [0.3, 0.3, 0.0]));
                say!(c, "0x2a57164 lean", "joint {j}: lean half ranges at least 0.3");
            }
        }
        say!(c, "0x2a56ee8", "snap on part {rp}, magnitude {mag:.2}");
    }

    fn gravity_solver(&self, c: &mut Core, fr: &[Transform], com: Vec3, torque: &mut [Vec3]) {
        const CHAINS: [&[usize]; 6] = [&[12, 15, 20, 21, 22], &[1, 5, 11, 12], &[12, 14, 18, 19], &[12, 13, 16, 17], &[1, 3, 8, 9], &[1, 2, 6, 7]];
        const SHARE: [f32; 6] = [1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        const ROOT: usize = 1;
        let g = Vec3::new(0.0, 0.0, -9.8);
        let up = Vec3::Z;
        let scale = self.r.oppose;
        let mut p = [GravPart::default(); 23];
        let mut mass = [0.0f32; 23];
        for k in 0..self.build.parts.len().min(fr.len()) {
            let mut o = k;
            let l = loop {
                match (0..23).find(|&l| link_part(l) == o) {
                    Some(l) => break l,
                    None => o = self.build.parts[o].parent.unwrap_or(0),
                }
            };
            let m = self.doll.part_mass(k);
            mass[l] += m;
            p[l].moment += fr[k].translation * m;
        }
        let own = p;
        for l in 0..23 {
            p[l].mass = mass[l];
        }
        p[1].mass += mass[0] + mass[4] + mass[10];
        p[1].moment += own[0].moment + own[4].moment + own[10].moment;
        if let Some((at, _, rm)) = self.rider.filter(|r| r.2 > 0.0) {
            p[12].mass += rm;
            p[12].moment += at * rm;
        }
        let total: f32 = p.iter().map(|x| x.mass).sum();
        let com_s = p.iter().map(|x| x.moment).sum::<Vec3>() / total.max(1e-3);
        let planted: [bool; 6] = [false, false, c.legs[0].planted, c.legs[1].planted, c.legs[2].planted, c.legs[3].planted];
        let feet: [Vec3; 6] = [Vec3::ZERO, Vec3::ZERO, c.legs[0].sole, c.legs[1].sole, c.legs[2].sole, c.legs[3].sole];
        let n = planted.iter().filter(|x| **x).count();
        let k = 1.0 / (g.length_squared() + 1e-10);
        let (mut weight, mut sum, mut foot_sum) = ([0.0f32; 6], 0.0, Vec3::ZERO);
        for i in 0..6 {
            if planted[i] {
                let d = feet[i] - com_s;
                foot_sum += feet[i];
                p[CHAINS[i][0]].fixed = true;
                let flat = d - g * (d.dot(g) * k);
                weight[i] = scale / (flat.length() + 1e-10);
                sum += weight[i];
            }
        }
        let mut f = Vec3::ZERO;
        if n != 0 {
            let on = support_on(com, up, [feet[2], feet[3], feet[4], feet[5]]);
            let a = on - com;
            let a = a - up * a.dot(up);
            let v = a + g * ((foot_sum / n as f32 - com_s).dot(g) * k);
            f = v * (v.dot(g) / v.length_squared().max(1e-10));
        }
        let mut acc = [Vec3::ZERO; 6];
        for i in 0..6 {
            if planted[i] {
                acc[i] = if sum != 0.0 { f * (weight[i] / sum) } else { f };
            }
        }
        for i in (0..6).rev() {
            if !planted[i] {
                f += (g - f) * SHARE[i];
                acc[i] = f;
            }
        }
        for i in (0..6).filter(|&i| i != ROOT && !planted[i]) {
            gravity_chain(&mut p, CHAINS[i], i == ROOT, planted[i], acc[i], scale, total, |j| self.joint_place(j, fr));
        }
        gravity_chain(&mut p, CHAINS[ROOT], true, planted[ROOT], acc[ROOT], scale, total, |j| self.joint_place(j, fr));
        for i in (0..6).filter(|&i| i != ROOT && planted[i]) {
            gravity_chain(&mut p, CHAINS[i], false, true, acc[i], scale, total, |j| self.joint_place(j, fr));
        }
        let mut most = 0.0f32;
        for l in 1..23 {
            let mut t = p[l].torque;
            if t == Vec3::ZERO {
                continue;
            }
            let k = link_part(l);
            let Some(pa) = self.build.parts[k].parent else { continue };
            if 0xf03c0u32 >> l & 1 == 1 {
                let pf = match self.build.parts[k].limit {
                    Limit::BoneHinge { pf, .. } | Limit::Bone { pf, .. } => pf,
                    _ => Quat::IDENTITY,
                };
                let z = fr[pa].rotation * pf * Vec3::Z;
                t = z * t.dot(z);
            }
            torque[k] += t;
            torque[pa] -= t;
            most = most.max(t.length());
        }
        say!(c, "0x2a37820", "gravity solver: oppose {scale}, {n} planted, mass {total:.0}, force {:.2?} on the planted, largest joint torque {most:.1} N m", f);
    }

    fn joint_place(&self, l: usize, fr: &[Transform]) -> Vec3 {
        let k = link_part(l);
        fr[k].translation + fr[k].rotation * self.build.parts[k].anchor
    }

    #[allow(clippy::too_many_arguments)]
    fn stay_upright(&self, c: &mut Core, pv: &Params, fr: &[Transform], com: Vec3, omega: Vec3, rows: [Vec3; 3], vel: Vec3, dt: f32, torque: &mut [Vec3]) {
        let Some(s) = pv.stay.filter(|_| c.state < 2 && std::env::var_os("HORSE_NOSTAY").is_none()) else { return };
        let u = c.u;
        let (a, b) = (rows[0], rows[1]);
        let mid = c.legs.iter().map(|l| l.sole).sum::<Vec3>() * 0.25;
        let tilt = u.dot((com - mid).normalize_or(u)).clamp(-1.0, 1.0).acos();
        let g = if tilt > s.lean_min && s.lean_max > s.lean_min { 1.0 - (tilt.min(s.lean_max) - s.lean_min) / (s.lean_max - s.lean_min) * s.reduction } else { 1.0 };
        let cc = if c.legs.iter().any(|l| !l.planted) { 0.99 } else { 1.0 };
        let h = flat(b, u, 0.0).normalize_or_zero();
        let e = u.cross(h);
        let asin = |v: f32| v.clamp(-1.0, 1.0).asin();
        let scale = g / (30.0 * dt).clamp(1.0, 30.0);
        let (kk, dd) = (s.strength, s.damping);
        let (r0, r1) = (asin(a.dot(e)), asin(a.dot(h)));
        let o = c.upright0.get_or_insert((r0, r1));
        if cc == 1.0 {
            let k = (dt / (STAY_REST * std::env::var("HORSE_STAYREST").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0))).min(1.0);
            *o = (o.0 + (r0 - o.0) * k, o.1 + (r1 - o.1) * k);
        }
        let (o0, o1) = if std::env::var_os("HORSE_STAYRAW").is_some() { (0.0, 0.0) } else { *o };
        let mut axes = vec![(h, (166.667 * (r0 - o0) * cc * kk.x - 4.0 * cc * dd.x * omega.dot(h)) * scale),
                            (e, (-166.667 * (r1 - o1) * cc * kk.y - 4.0 * cc * dd.y * omega.dot(e)) * scale)];
        let along = vel - u * u.dot(vel);
        if along.length() > 2.0 {
            axes.push((u, (-166.667 * asin(along.normalize().dot(h)) * cc * kk.z - 4.0 * cc * dd.z * omega.dot(u)) * scale));
        }
        for (axis, t) in axes.iter().copied() {
            if s.parts {
                let n = fr.len();
                let sum: f32 = (0..n).map(|k| self.doll.inertia(k, axis)).sum();
                if sum > 1e-6 {
                    for k in 0..n {
                        torque[k] += axis * (t * self.doll.inertia(k, axis) / sum);
                    }
                }
            } else {
                torque[12] += axis * t;
            }
        }
        let down: Vec<usize> = (0..4).filter(|&i| c.legs[i].planted).map(|i| LEGS[LEG[i]][1]).collect();
        if !down.is_empty() && std::env::var_os("HORSE_STAYINT").is_some() {
            let t: Vec3 = axes.iter().map(|&(x, t)| x * t).sum();
            for &k in &down { torque[k] -= t / down.len() as f32; }
        }
        say!(c, "0x2a5a238", "stayUpright: {:.1} {:.1} N m, fall-off {g:.3}", axes[0].1, axes[1].1);
        say!(c, "0x2a59608", "stayUpright on the quadruped");
    }

    pub(super) fn limb_message(&mut self, limb: usize, l: LimbLimit) {
        if let Some(s) = self.r.limits.get_mut(limb) {
            *s = Some(l);
        }
        if std::env::var_os("RDR2_TRACE").is_some() {
            eprintln!("quadruped 0x2a273ec: quadLimbLimit limb {limb} type {} direction {} [{}, {}]", l.kind, l.dir, l.min, l.max);
            eprintln!("quadruped 0x2a3b740: the record on leg {limb}'s knee and hock");
        }
    }

    pub(super) fn state_rdr2(&self) -> String {
        let Some(c) = self.core.as_ref() else { return "RDR2 quadruped (not started)".into() };
        let name = ["standing", "stepping", "falling", "falling, hit the ground", "down"][c.state.clamp(0, 4) as usize];
        let planted = c.legs.iter().filter(|l| l.planted).count();
        format!("RDR2 {name}  hooves {planted}  legs {:.0}%  spine {:.0}%  COM h {:.2}  H {:.2}{}", c.fl * 100.0, c.fs2 * 100.0, c.com_h, c.h,
                if c.legs.iter().any(|l| l.state == 1) { "  stepping" } else { "" })
    }

    pub(super) fn standing_rdr2(&self) -> bool {
        !self.dead && self.core.as_ref().is_none_or(|c| c.state < 2)
    }

    pub(super) fn down_rdr2(&self) -> bool {
        self.core.as_ref().is_some_and(|c| c.state >= 3)
    }

    pub(super) fn steady_rdr2(&self) -> bool {
        self.core.as_ref().is_some_and(|c| c.state == 0)
    }

    pub(super) fn rage_drive(&mut self, muscles: &[Muscle], ms: f32, most: f32, dead: bool, dt: f32) {
        let n = self.build.parts.len();
        self.kept.rigid = (0..n).map(|k| {
            let m = muscles[k];
            let kk = if LEGS.iter().any(|l| l.contains(&k)) { knob("HORSE_KS").and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0) * if std::env::var_os("HORSE_KSIZE").is_some() { most } else { 1.0 } } else { 1.0 };
            let (ks, ds) = (m.stiffness * m.stiffness * kk, (2.0 * m.damping * m.stiffness).max(0.5) * kk.sqrt());
            let driven = !dead && (LEGS.iter().any(|l| l.contains(&k)) || (CHAIN.contains(&k) && k != CHAIN[0]));
            let mus = if driven { ms } else { self.rig_ms[k] };
            let x = (8.0 * ds + ks) * mus * dt * 0.5;
            let blend = (6.0 * dt).max(x / (x + 1.0)).clamp(0.0, 0.9999);
            let (frame, child, limits, target, hinge) = match self.build.parts[k].limit {
                Limit::Bone { pf, cf, lean, twist } => {
                    let a = angles(pf, cf, m.target);
                    (pf, cf, [(-lean[0], lean[0]), (-lean[1], lean[1]), (-twist, twist)], [a.x, -a.y, a.z], false)
                }
                Limit::BoneHinge { pf, cf, lo, hi } if LEGS.iter().any(|l| l[1] == k || l[2] == k) => {
                    let big = lo.abs().max(hi.abs());
                    (pf, cf, [(-KNEE_LEAN, KNEE_LEAN), (-KNEE_LEAN, KNEE_LEAN), (-big, big)], [0.0, 0.0, hinge_of(pf, cf, m.target)], false)
                }
                Limit::BoneHinge { pf, cf, lo, hi } => (pf, cf, [(lo, hi), (0.0, 0.0), (0.0, 0.0)], [hinge_of(pf, cf, m.target), 0.0, 0.0], true),
                Limit::Cone { swing, twist } => {
                    let z = Quat::from_rotation_x(-FRAC_PI_2);
                    (self.build.parts[k].rest.inverse() * z, z, [(-swing, swing), (-swing, swing), (-twist, twist)], [0.0; 3], false)
                }
                _ => (Quat::IDENTITY, Quat::IDENTITY, [(0.0, 0.0); 3], [0.0; 3], false),
            };
            let leaned = LEGS.iter().any(|l| l[1] == k || l[2] == k) && !hinge && self.rig_caps[k][1] == 0.0;
            let capk = if LEGS.iter().any(|l| l.contains(&k)) { knob("HORSE_CAP").and_then(|v| v.parse::<f32>().ok()).unwrap_or(self.leg_cap) } else { 1.0 };
            let max = if leaned { [self.rig_caps[k][0]; 2] } else { self.rig_caps[k] }.map(|x| x * most * capk);
            Rigid { frame, child, limits, max, blend, hinge, desired: target[0], jc: None, target, gain: [ks, ds], soft: Default::default(),
                    cap: Some(if hinge { 27000.0 } else { 35000.0 } * most) }
        }).collect();
    }

    pub(super) fn ours_drive(&mut self, targets: &[Quat], ks: f32, most: f32, dt: f32, soft: Option<usize>) {
        let n = self.build.parts.len();
        let pv = &self.r;
        let (ls, ld, ss, sd, ms) = (p!(pv, "legStiffness") * ks, p!(pv, "legDamping"), p!(pv, "spineStiffness"), p!(pv, "spineDamping"), p!(pv, "0x2128bd54"));
        let muscles: Vec<Muscle> = (0..n).map(|k| {
            let (own, ratio) = self.muscles[k];
            let (st, dm) = if WELDED.contains(&k) {
                (WELD, 1.0)
            } else if soft.is_some_and(|i| LEGS[i].contains(&k)) {
                (ls * std::env::var("HORSE_OURSSOFT").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0), ld)
            } else if LEGS.iter().any(|l| l[0] == k) {
                (ls * std::env::var("HORSE_OURSHIP").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.2), ld)
            } else if LEGS.iter().any(|l| l.contains(&k)) {
                (ls, ld)
            } else if CHAIN.contains(&k) && k != CHAIN[0] {
                (ss, sd)
            } else {
                (own, ratio)
            };
            Muscle { target: targets[k], stiffness: st, damping: dm.max(1e-3) }
        }).collect();
        self.rage_drive(&muscles, ms, most, self.dead, dt);
        self.doll.rules = Rules::Rdr2;
        self.doll.limit_row = vec![None; n];
        for l in LEGS.iter() {
            for &k in &l[1..] {
                if let Limit::BoneHinge { lo, hi, .. } = self.build.parts[k].limit {
                let w = std::env::var("HORSE_OURSJW").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                self.doll.limit_row[k] = Some(LimitRow { lo: lo - w, hi: hi + w, strength: f32::MAX, breaks: false, blend: 1.0, soft: 0.5, k: 0.0 });
                }
            }
        }
        self.kept.muscles = muscles;
    }

    pub(super) fn rest_rdr2(&mut self) {
        if let Some(c) = self.core.take() {
            for r in &c.rig {
                let k = LEGS[r.row][2];
                if let Limit::BoneHinge { pf, cf, .. } = self.build.parts[k].limit {
                    self.build.parts[k].limit = Limit::BoneHinge { pf, cf, lo: r.hock.lo, hi: r.hock.hi };
                }
            }
        }
        self.doll.rules = Rules::Gta4;
        self.kept.rigid.clear();
        self.doll.set_part_scales(&[], &[]);
        self.pose = self.doll.frames().collect();
        (self.held, self.hold, self.calm) = (true, true, 0.0);
    }

    pub(super) fn stop_rdr2(&mut self) {
        if let Some(c) = self.core.take() {
            if c.tr {
                eprintln!("quadruped 0x2ad5514: deactivate at {:.2} s", c.t);
                eprintln!("quadruped 0x2abfb04: its state freed");
            }
            for r in &c.rig {
                let k = LEGS[r.row][2];
                if let Limit::BoneHinge { pf, cf, .. } = self.build.parts[k].limit {
                    self.build.parts[k].limit = Limit::BoneHinge { pf, cf, lo: r.hock.lo, hi: r.hock.hi };
                }
            }
        }
        self.held = false;
        self.fell = true;
        self.sunk = self.sunk.max(1.0);
        self.doll.rules = Rules::Gta4;
        self.kept.rigid.clear();
        self.doll.set_part_scales(&[], &[]);
    }
}

fn part_rdr2(k: usize) -> usize {
    (0..23).find(|&l| link_part(l) == k).unwrap_or(0)
}

impl Core {
    fn was_ground3(&self) -> bool {
        self.fresh3_flag
    }
    fn set_ground3(&mut self, v: bool) {
        self.fresh3_flag = v;
    }
    fn spine_gone(&self) -> bool {
        self.fs == 0.0
    }
}
