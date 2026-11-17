use crate::estimate::fneg;

pub mod state {
    pub const STILL: i32 = 1;
    pub const WALK: i32 = 2;
    pub const RUN: i32 = 3;
    pub const SPRINT: i32 = 4;
}

pub const IN_FORMATION: u32 = 913;
pub const MARGIN: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Nothing,
    Shove,
    HitResponse(u32),
    FallAndGetUp(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bump {
    pub me: i32,
    pub other: i32,
    pub my_forward: [f32; 3],
    pub other_forward: [f32; 3],
    pub d: [f32; 3],
    pub in_formation: bool,
    pub shoved_last: bool,
    pub friend: bool,
    pub health: f32,
    pub my_weakness: f32,
    pub other_weakness: f32,
}

fn dot_yzx(a: [f32; 3], d: [f32; 3]) -> f32 {
    d[0].mul_add(a[0], a[2].mul_add(d[2], a[1] * d[1]))
}

pub fn answer(b: &Bump, side: impl FnOnce() -> u32) -> Answer {
    let front = dot_yzx(b.my_forward, b.d) >= 0.0;
    let faces = b.other_forward[0].mul_add(b.d[0], b.other_forward[2].mul_add(b.d[2], b.other_forward[1] * b.d[1])) <= 0.0;
    if b.in_formation {
        return Answer::Nothing;
    }
    let shove = || if b.shoved_last || b.friend || b.health < 100.0 { Answer::Nothing } else { Answer::Shove };
    let moving = |s: i32| matches!(s, 2..=4);
    let still = |s: i32| matches!(s, 0 | 1);
    let (me, other) = (b.me, b.other);
    if me == state::WALK && other == state::WALK {
        return if front { shove() } else { Answer::Nothing };
    }
    if matches!(me, 3 | 4) && moving(other) {
        if !front {
            return Answer::Nothing;
        }
        if !faces {
            return shove();
        }
        let weaker = other != state::WALK && if b.my_weakness > 1.5 {
            if b.other_weakness > 1.5 { b.my_weakness > b.other_weakness } else { true }
        } else {
            false
        };
        return if weaker {
            Answer::FallAndGetUp(side())
        } else if me == state::SPRINT {
            Answer::HitResponse(side())
        } else {
            Answer::Nothing
        };
    }
    if still(me) && moving(other) && faces {
        let s = side();
        return if other == state::WALK { Answer::Nothing } else { Answer::HitResponse(s) };
    }
    if !matches!(me, 2..=4) || !still(other) || !front {
        return Answer::Nothing;
    }
    shove()
}

pub fn side(centre: [f32; 3], corners: [[f32; 3]; 4], p: [f32; 3]) -> u32 {
    let lines: [([f32; 3], f32); 4] = corners.map(|c| {
        let v = [c[0] - centre[0], c[1] - centre[1], c[2] - centre[2]];
        let n = [-v[1], v[0], 0.0f32];
        let off = fneg(n[1].mul_add(c[1], n[2].mul_add(c[2], n[0] * c[0])));
        (n, off)
    });
    let ahead = |(n, off): ([f32; 3], f32)| n[0].mul_add(p[0], n[1].mul_add(p[1], n[2] * p[2]) + off);
    for k in 4..8 {
        let (prev, cur) = ((k - 1) % 4, k % 4);
        if ahead(lines[prev]) < 0.0 {
            continue;
        }
        if ahead(lines[cur]) < 0.0 {
            return cur as u32;
        }
    }
    0
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Concerns {
    pub dead: bool,
    pub attached: bool,
    pub melee_move: bool,
    pub in_vehicle: bool,
    pub other_in_vehicle: bool,
    pub other_static: i32,
    pub hates: bool,
    pub me: i32,
    pub forward: [f32; 3],
    pub normal: [f32; 3],
    pub walking_round: bool,
    pub both_1202: bool,
    pub both_1203: bool,
}

pub const MOST_STATIC: i32 = 30;

pub fn concerns(c: &Concerns) -> bool {
    let d = c.forward[0].mul_add(c.normal[0], c.forward[2].mul_add(c.normal[2], c.forward[1] * c.normal[1]));
    !(c.dead || c.attached || c.melee_move || c.in_vehicle || c.other_in_vehicle || c.other_static > MOST_STATIC || c.hates || c.me < 2
      || d > 1.0 || c.walking_round || c.both_1202 || c.both_1203)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VehicleConcerns {
    pub dead: bool,
    pub forced: bool,
    pub in_vehicle: bool,
    pub boat: bool,
    pub any_normal: bool,
    pub normal: [f32; 3],
    pub move_dir: [f32; 3],
    pub cover_at_it: bool,
    pub door: Option<bool>,
    pub door_on_way: bool,
    pub sectors: Option<(u32, u32)>,
    pub heading_for_it: bool,
    pub at_its_door: bool,
    pub first_505_at_it: bool,
    pub walking_round: Option<(bool, bool)>,
}

pub const AGAINST: f32 = -0.35;

pub fn vehicle_concerns(c: &VehicleConcerns) -> bool {
    if c.dead {
        return false;
    }
    if c.forced {
        return true;
    }
    let d = c.normal[0].mul_add(c.move_dir[0], c.normal[1].mul_add(c.move_dir[1], c.normal[2] * c.move_dir[2]));
    if c.in_vehicle || c.boat || !c.any_normal && d > AGAINST || c.cover_at_it {
        return false;
    }
    if let Some((me, target)) = c.sectors {
        if c.door_on_way || me == target && c.door.is_none_or(|open| open) {
            return false;
        }
    }
    if c.heading_for_it && c.door.is_none() || c.at_its_door || c.first_505_at_it {
        return false;
    }
    let held = c.door == Some(false);
    !c.walking_round.is_some_and(|(hold, same)| !held || hold && same)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RateCount {
    pub count: u8,
    pub start: u32,
    pub length: u32,
    pub started: bool,
    pub restart: bool,
}

impl RateCount {
    pub const WINDOW: u32 = 3000;

    pub fn new() -> RateCount {
        RateCount { started: true, ..Default::default() }
    }

    pub fn add(&mut self, n: u8, now: u32) {
        let over = if self.started {
            if self.restart {
                (self.start, self.restart) = (now, false);
            }
            (self.length as i32).wrapping_add(self.start as i32) <= now as i32
        } else {
            false
        };
        if !over {
            return;
        }
        (self.start, self.length, self.started) = (now, Self::WINDOW, true);
        self.count = self.count.wrapping_add(n);
    }
}

pub fn walk_round(root: [f32; 3], rows: [[f32; 3]; 3], height: f32, margin: f32) -> ([f32; 3], [[f32; 3]; 4]) {
    let fma = |a: f32, x: f32, c: f32| (a as f64 * x as f64 + c as f64) as f32;
    let dot3 = |a: [f32; 3], c: [f32; 3]| crate::estimate::vmsum3(std::array::from_fn(|l| a[l] as f64 * c[l] as f64));
    let pick = |a: f32, c: f32, small: bool| if a == c { if (a.to_bits() >> 31 == 1) == small { a } else { c } } else if (a < c) == small { a } else { c };
    let m: [[f32; 4]; 4] = [[rows[0][0], rows[0][1], rows[0][2], 0.0], [rows[1][0], rows[1][1], rows[1][2], 0.0],
        [rows[2][0], rows[2][1], rows[2][2], 0.0], [root[0], root[1], root[2], 1.0]];
    let place = |mx: &[[f32; 4]; 4], v: [f32; 3]| -> [f32; 3] {
        let v = [v[0], v[1], v[2], 1.0];
        std::array::from_fn(|c| (0..4).fold(0.0f64, |acc, k| acc + v[k] as f64 * mx[k][c] as f64) as f32)
    };
    let (s, c) = std::f32::consts::FRAC_PI_2.sin_cos();
    let (below, above) = (height - 1.0, height + 1.0);
    let (mut lo, mut hi, mut found) = ([f32::MAX; 3], [-f32::MAX; 3], false);
    for &(half, z, turned) in &BOXES {
        let r = if turned { [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]] } else { [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] };
        let pm = [[r[0][0], r[0][1], r[0][2], 0.0], [r[1][0], r[1][1], r[1][2], 0.0], [r[2][0], r[2][1], r[2][2], 0.0], [0.0, 0.0, z, 1.0]];
        let cen: [f32; 3] = std::array::from_fn(|l| (half[l] + -half[l]) * 0.5);
        let e: [f32; 3] = std::array::from_fn(|l| (half[l] - -half[l]) * 0.5);
        let t = place(&pm, cen);
        let g: [f32; 3] = std::array::from_fn(|k| dot3(r[k].map(f32::abs), e));
        let w: [f32; 3] = std::array::from_fn(|l| fma(m[0][l], r[2][0], fma(m[1][l], r[2][1], m[2][l] * r[2][2])));
        let hz = dot3(e, w.map(f32::abs));
        let zc = fma(m[0][2], t[0], fma(m[1][2], t[1], m[2][2] * t[2]));
        if (zc + hz) + root[2] < below || (zc - hz) + root[2] > above {
            continue;
        }
        lo = std::array::from_fn(|k| pick(lo[k], t[k] - g[k], true));
        hi = std::array::from_fn(|k| pick(hi[k], t[k] + g[k], false));
        found = true;
    }
    if !found {
        (lo, hi) = ([-0.1; 3], [0.1; 3]);
    }
    let lo = lo.map(|v| v - margin);
    let hi = hi.map(|v| v + margin);
    let hs: [f32; 3] = std::array::from_fn(|k| (hi[k] - lo[k]) * 0.5);
    let sq: [f32; 3] = std::array::from_fn(|k| fma(m[k][0], m[k][0], m[k][1] * m[k][1]));
    let cen: [f32; 3] = std::array::from_fn(|k| (hi[k] + lo[k]) * 0.5);
    let w = place(&m, cen);
    let l: [f32; 3] = std::array::from_fn(|k| hs[k] * sq[k] * 2.0);
    let k = if l[0] > l[1] && l[0] > l[2] { 0 } else if l[1] > l[2] { 1 } else { 2 };
    let axis = [m[k][0], m[k][1], 0.0];
    let front: [f32; 3] = std::array::from_fn(|i| fma(axis[i], hs[k], w[i]));
    let back: [f32; 3] = std::array::from_fn(|i| w[i] - axis[i] * hs[k]);
    let n: [f32; 3] = {
        let sum = dot3(axis, axis);
        let e = crate::estimate::vrsqrtefp(sum);
        let e = if e == f32::INFINITY { 0.0 } else { e };
        let t = (-(sum as f64 * (e * e) as f64 - 1.0)) as f32;
        let e = (t as f64 * (0.5 * e) as f64 + e as f64) as f32;
        axis.map(|v| { let x = v * e; if x.is_nan() { f32::from_bits(0x7FC0_0000) } else { x } })
    };
    let others = match k { 0 => [1, 2], 1 => [0, 2], _ => [0, 1] };
    let (mut along, mut across) = (0.0f32, 0.0f32);
    for (i, &o) in others.iter().enumerate() {
        let r = m[o];
        let sel_abs = |x: f32| if x >= 0.0 { x } else { -x };
        let a = sel_abs(fma(r[1], n[1], r[0] * n[0]) * hs[o]);
        let b = sel_abs(((r[1] as f64 * n[0] as f64 - (r[0] * n[1]) as f64) as f32) * hs[o]);
        (along, across) = if i == 0 { (a, b) } else { (along + a, across + b) };
    }
    let p = [n[1], -n[0], 0.0];
    let fr: [f32; 3] = std::array::from_fn(|i| fma(n[i], along, front[i]));
    let bk: [f32; 3] = std::array::from_fn(|i| back[i] - n[i] * along);
    let corners: [[f32; 3]; 4] = [std::array::from_fn(|i| fr[i] - p[i] * across), std::array::from_fn(|i| bk[i] - p[i] * across),
        std::array::from_fn(|i| fma(p[i], across, bk[i])), std::array::from_fn(|i| fma(p[i], across, fr[i]))]
        .map(|v: [f32; 3]| [v[0], v[1], height]);
    let (mut cx, mut cy) = (corners[0][0], corners[0][1]);
    for v in &corners[1..] {
        (cx, cy) = (v[0] + cx, v[1] + cy);
    }
    ([cx * 0.25, cy * 0.25, height], corners)
}

const BOXES: [([f32; 3], f32, bool); 4] = [
    ([0.25, 0.25, 0.735], 0.085, false),
    ([0.1, 0.425, 0.1], -0.875, true),
    ([0.25, 0.735, 0.25], 0.085, true),
    ([0.25, 0.25, 0.25], -0.55, false),
];

pub fn bound_box() -> [[f32; 3]; 2] {
    let (s, c) = std::f32::consts::FRAC_PI_2.sin_cos();
    let kids: Vec<_> = BOXES.iter().map(|(h, ..)| Some([h.map(|v| -v), *h])).collect();
    let rows: Vec<crate::blockedspot::Rows> = BOXES.iter().map(|&(_, z, turned)| {
        let r = if turned { [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]] } else { [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] };
        [r[0], r[1], r[2], [0.0, 0.0, z]]
    }).collect();
    let b = crate::compositebox::extents(&kids, &rows, &rows).bound;
    [b.low, b.high]
}

pub fn shove_clip(right: [f32; 3], back: [f32; 3], running: bool, armed: bool) -> Option<u32> {
    let x = right[0].mul_add(back[0], right[1].mul_add(back[1], right[2] * back[2]));
    if armed {
        return if x < 0.0 { None } else { Some(0x99) };
    }
    let (x, run) = (x as f64, if running { 2 } else { 0 });
    Some(if !(x >= -0.2) { 0x98 + run } else if x < 0.2 { 0x96 } else { 0x97 + run })
}

pub const SHOVE_FLAGS: u32 = 0xC008;

pub const SHOVES: [(u32, &str); 5] = [
    (0x96, "ped/run_open_door_shove"),
    (0x97, "ped/open_door"),
    (0x98, "ped/open_door_r"),
    (0x99, "ped/run_open_door"),
    (0x9A, "ped/run_open_door_r"),
];

pub fn decide(mut b: Bump, me_root: [f32; 3], me_fwd: [f32; 3], other_root: [f32; 3], other_fwd: [f32; 3], armed: bool) -> (Answer, Option<usize>) {
    let flat = |v: [f32; 3], or: [f32; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
        if l > 1e-6 { [v[0] / l, v[1] / l, 0.0] } else { or }
    };
    let fwd = flat(me_fwd, [0.0, 1.0, 0.0]);
    let right = [fwd[1], -fwd[0], 0.0];
    b.my_forward = fwd;
    b.other_forward = flat(other_fwd, [0.0, 1.0, 0.0]);
    b.d = [other_root[0] - me_root[0], other_root[1] - me_root[1], other_root[2] - me_root[2]];
    let a = answer(&b, || {
        let (c, corners) = walk_round(me_root, [right, fwd, [0.0, 0.0, 1.0]], other_root[2], MARGIN);
        side(c, corners, other_root)
    });
    let clip = (a == Answer::Shove).then(|| shove_clip(right, [-b.d[0], -b.d[1], -b.d[2]], b.me >= state::RUN, armed)).flatten()
        .and_then(|c| SHOVES.iter().position(|s| s.0 == c));
    (a, clip)
}
