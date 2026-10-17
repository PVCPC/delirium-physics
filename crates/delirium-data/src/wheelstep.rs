use crate::{mathrt, suspension::{self, Spring}, vehicles::Handling};

pub type V4 = [f32; 4];
pub const MAX_WHEELS: usize = 8;
pub const GRAVITY: V4 = [0.0, 0.0, -9.8, 0.0];
const SIXTH: f32 = 1.0 / 6.0;
const G: f32 = 9.8;

fn add(a: V4, b: V4) -> V4 {
    std::array::from_fn(|l| a[l] + b[l])
}

fn mul(a: V4, s: f32) -> V4 {
    std::array::from_fn(|l| a[l] * s)
}

fn madd(a: V4, s: f32, c: V4) -> V4 {
    std::array::from_fn(|l| a[l].mul_add(s, c[l]))
}

pub(crate) fn dot3(a: V4, b: V4) -> f32 {
    (0.0 + a[0] as f64 * b[0] as f64 + a[1] as f64 * b[1] as f64 + a[2] as f64 * b[2] as f64) as f32
}

pub(crate) fn fsel(t: f32, a: f32, b: f32) -> f32 {
    if t >= 0.0 { a } else { b }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub m: [V4; 4],
    pub vel: V4,
    pub ang: V4,
    pub slip: [f32; MAX_WHEELS],
    pub push: [V4; MAX_WHEELS],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Deriv {
    pub vel: V4,
    pub ang: V4,
    pub acc: V4,
    pub ang_acc: V4,
    pub slip: [f32; MAX_WHEELS],
    pub push: [V4; MAX_WHEELS],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Times {
    pub dt: f32,
    pub inv_dt: f32,
    pub h: f32,
    pub inv_h: f32,
    pub t: f32,
    pub inv_t: f32,
    pub inv_k: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Out {
    pub push: V4,
    pub acc: V4,
    pub ang_acc: V4,
    pub value: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    pub flags: u32,
    pub slip: f32,
    pub timer: f32,
    pub sums: [V4; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub inv_inertia: V4,
    pub inv_mass: f32,
    pub m: [V4; 4],
    pub vel: V4,
    pub ang: V4,
}

pub(crate) fn axis_angle(axis: [f32; 3], angle: f32) -> [V4; 3] {
    let (c, s) = (mathrt::cos(angle as f64) as f32, mathrt::sin(angle as f64) as f32);
    let [x, y, z] = axis;
    let t = 1.0 - c;
    let (xt, tz) = (x * t, t * z);
    [
        [(x * x).mul_add(t, c), xt.mul_add(y, s * z), xt.mul_add(z, -(s * y)), 0.0],
        [xt.mul_add(y, -(s * z)), (y * y).mul_add(t, c), tz.mul_add(y, x * s), 0.0],
        [xt.mul_add(z, s * y), tz.mul_add(y, -(x * s)), (z * z).mul_add(t, c), 0.0],
    ]
}

pub fn advance(m: &[V4; 4], vel: V4, ang: V4, h: f32) -> [V4; 4] {
    let mut out = *m;
    out[3] = madd(vel, h, m[3]);
    let w = mul(ang, h);
    let sq = w[2].mul_add(w[2], w[1].mul_add(w[1], w[0] * w[0]));
    if sq > 1e-12 {
        let a = sq.sqrt();
        let e = 1.0 / a;
        let e = e.mul_add(-e.mul_add(a, -1.0), e);
        let axis = mul(w, e);
        let r = axis_angle([axis[0], axis[1], axis[2]], a);
        for row in &mut out[..3] {
            let v = *row;
            *row = madd(r[2], v[2], madd(r[1], v[1], mul(r[0], v[0])));
        }
    }
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Strut {
    pub x: f32,
    pub prev: f32,
    pub spring: Spring,
    pub axis: V4,
    pub contact: V4,
    pub start: V4,
    pub normal: V4,
    pub on_entity: bool,
    pub damage: f32,
    pub health: f32,
    pub flags: u32,
    pub drop: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mass {
    pub mass: f32,
    pub inv_mass: f32,
    pub inv_inertia: V4,
    pub m: [V4; 4],
}

fn at_least_zero(v: f32) -> f32 {
    fsel(-v, 0.0, v)
}

fn by_columns(m: &[V4; 4], v: V4) -> V4 {
    let col = |c: usize| [m[0][c], m[1][c], m[2][c], 0.0];
    let (x, y, z) = (dot3(v, col(0)), dot3(v, col(1)), dot3(v, col(2)));
    [x, y, z, x]
}

pub fn suspension(h: &Handling, b: &Mass, w: &Strut, m: &[V4; 4], vel: V4, t: &Times) -> Out {
    let s = &w.spring;
    let moved = w.x - w.prev;
    let blend = (t.t * t.inv_dt).mul_add(moved, w.prev);
    let o = &b.m;
    let r: V4 = std::array::from_fn(|l| w.start[l] - o[3][l]);
    let local = [dot3(o[0], r), dot3(o[1], r), dot3(o[2], r), 1.0];
    let world: V4 = std::array::from_fn(|c| {
        let col = [m[0][c], m[1][c], m[2][c], m[3][c]];
        (0.0 + local[0] as f64 * col[0] as f64 + local[1] as f64 * col[1] as f64 + local[2] as f64 * col[2] as f64
            + local[3] as f64 * col[3] as f64) as f32
    });
    let off: V4 = std::array::from_fn(|l| world[l] - w.start[l]);
    let along = mul(w.normal, dot3(off, w.normal));
    let axis = by_columns(m, w.axis);
    let delta = -along[2].mul_add(axis[2], along[0].mul_add(axis[0], axis[1] * along[1]));
    let mut x = fsel(blend - s.travel, s.travel, blend);
    if !(w.health >= 1000.0) {
        x = at_least_zero(x - suspension::sag(s, w.health, w.x, w.drop));
    }
    let sink = suspension::sink(w.flags);
    if sink != 0.0 {
        x = at_least_zero(x - sink);
    }
    let x = x + delta;
    let cap = if w.on_entity { 0.05 } else { s.travel * 0.5 };
    let step = (t.inv_t * t.dt).mul_add(delta, moved);
    let rate = fsel(step - cap, cap, step) * t.inv_dt;
    let share = if !(w.damage >= 100.0) { s.share * 0.1 } else if !(w.damage >= 500.0) { s.share * 0.5 } else { s.share };
    let top = if w.on_entity { share + 0.2 } else { 1.0 };
    let k = if s.front { 1.0 } else { 0.75 } * h.suspension_force;
    let f = k.mul_add(x - s.rest, share);
    let f = fsel(f, f, 0.0);
    let f = if f as f64 - top as f64 >= 0.0 { top } else { f };
    let spring = f * b.mass * G;
    let c = if !(rate <= 0.0) { h.suspension_comp_damp } else { h.suspension_rebound_damp };
    let mut damp = c * b.mass * G * rate;
    if !(w.damage >= 100.0) {
        damp *= 0.5;
    } else if !(w.damage >= 500.0) {
        damp *= 0.7;
    }
    let mut damp = h.suspension_bias[if s.front { 0 } else { 1 }] * damp;
    if !(damp >= -spring) {
        damp = -spring;
    }
    let fwd = m[1];
    let n_fwd = dot3(w.normal, fwd);
    let dir = if n_fwd * dot3(vel, fwd) >= 0.0 {
        w.normal
    } else {
        let p: V4 = std::array::from_fn(|l| w.normal[l] - n_fwd * fwd[l]);
        let len2 = dot3(p, p);
        let e = (1.0 / (len2 as f64).sqrt()) as f32;
        let e = if e == f32::INFINITY { 0.0 } else { e };
        let k = (-len2.mul_add(e * e, -1.0)).mul_add(0.5 * e, e);
        mul(p, k)
    };
    let push = mul(dir, damp + spring);
    let load = fsel(-damp, damp, 0.0) + spring;
    let acc = mul(push, b.inv_mass);
    let r: V4 = std::array::from_fn(|l| w.contact[l] - o[3][l]);
    let (a, c) = ([push[2], push[0], push[1], push[3]], [r[1], r[2], r[0], r[3]]);
    let (pa, rc) = ([push[1], push[2], push[0], push[3]], [r[2], r[0], r[1], r[3]]);
    let torque: V4 = std::array::from_fn(|l| -pa[l].mul_add(rc[l], -(a[l] * c[l])));
    let (b0, b1, b2) = (dot3(m[0], torque), dot3(m[1], torque), dot3(m[2], torque));
    let mut body = [b0, b1, b2, b0];
    if w.flags & 0x4_0000 != 0 {
        body[1] = 0.0;
    }
    let body: V4 = std::array::from_fn(|l| body[l] * b.inv_inertia[l]);
    Out { push, acc, ang_acc: by_columns(m, body), value: Some(load) }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tread {
    pub steer: f32,
    pub brake: f32,
    pub drive: f32,
    pub brake_prev: f32,
    pub drive_prev: f32,
    pub no_brake: f32,
    pub handbrake: f32,
    pub anchor: V4,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TyreIn {
    pub slip_speed: f32,
    pub along: f32,
    pub across: f32,
    pub hold_along: f32,
    pub hold_across: f32,
    pub slip: f32,
    pub brake: f32,
    pub drive: f32,
    pub load: f32,
    pub h: f32,
}

fn vpick(p: f32, q: f32, small: bool) -> f32 {
    if p.is_nan() {
        return p;
    }
    if q.is_nan() {
        return q;
    }
    if p == q {
        let (a, b) = (p.to_bits(), q.to_bits());
        return f32::from_bits(if small { a | b } else { a & b });
    }
    if (p < q) == small { p } else { q }
}

fn rsqrte(v: f32) -> f32 {
    if v.is_nan() || v < 0.0 {
        f32::NAN
    } else if v == 0.0 {
        if v.is_sign_negative() { f32::NEG_INFINITY } else { f32::INFINITY }
    } else {
        (1.0 / (v as f64).sqrt()) as f32
    }
}

fn cross(a: V4, b: V4) -> V4 {
    let (ay, az) = ([a[1], a[2], a[0], a[3]], [a[2], a[0], a[1], a[3]]);
    let (by, bz) = ([b[1], b[2], b[0], b[3]], [b[2], b[0], b[1], b[3]]);
    std::array::from_fn(|l| -ay[l].mul_add(bz[l], -(az[l] * by[l])))
}

pub(crate) fn unit(v: V4) -> V4 {
    let len2 = dot3(v, v);
    let e = rsqrte(len2);
    let e = if e == f32::INFINITY { 0.0 } else { e };
    mul(v, (-len2.mul_add(e * e, -1.0)).mul_add(0.5 * e, e))
}

pub fn axes(cache: &mut [V4; 2], axle: V4, normal: V4, flags: u32, m: &[V4; 4], steer: f32) -> (V4, V4) {
    let [fwd, side] = *cache;
    if !(fwd[0] == 0.0 && fwd[1] == 0.0 && fwd[2] == 0.0) {
        return (fwd, side);
    }
    let mut side = axle;
    if steer.abs() > 0.001 {
        let (c, s) = (mathrt::cos(steer as f64) as f32, mathrt::sin(steer as f64) as f32);
        side = by_columns(&[[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0; 4]], side);
    }
    let side = by_columns(m, side);
    let n = if flags & 3 != 0 { normal } else { m[2] };
    let d = side[0].mul_add(n[0], side[1].mul_add(n[1], side[2] * n[2]));
    let (fwd, side) = if d.abs() < 0.99 {
        let fwd = unit(cross(side, n));
        (fwd, unit(cross(n, fwd)))
    } else {
        let side = unit(cross(n, m[1]));
        (unit(cross(side, n)), side)
    };
    *cache = [fwd, side];
    (fwd, side)
}

#[allow(clippy::too_many_arguments)]
pub fn hold(h: &Handling, b: &Mass, w: &Strut, r: &mut Tread, cache: &mut [V4; 2], axle: V4, vel: V4, ang: V4,
            frame_dt: f32) -> f32 {
    if w.flags & 1 == 0 {
        r.anchor = [0.0; 4];
        return 0.0;
    }
    let mut brake = r.brake * b.mass * G;
    let drive = b.mass * G * r.drive;
    if w.flags & 4 == 0 || r.handbrake > 0.25 {
        brake = (fsel(0.05 - r.handbrake, 0.05, r.handbrake) * G).mul_add(b.mass, brake);
    }
    let brake = if brake > drive.abs() { brake } else { 0.0 };
    let o = &b.m;
    let arm: V4 = std::array::from_fn(|l| w.start[l] - o[3][l]);
    let v = add(cross(arm, ang), vel);
    let (fwd, _) = axes(cache, axle, w.normal, w.flags, o, r.steer);
    let along = v[0].mul_add(fwd[0], v[1].mul_add(fwd[1], v[2] * fwd[2]));
    let reach = h.traction_spring_delta_max;
    if !(reach > 0.0) {
        return along;
    }
    let none = r.anchor[0] == 0.0 && r.anchor[1] == 0.0 && r.anchor[2] == 0.0;
    let mut d: V4 = if none { [0.0; 4] } else { std::array::from_fn(|l| r.anchor[l] - w.start[l]) };
    let mut moved = false;
    if brake == 0.0 || brake.abs() * b.inv_mass * f32::from_bits(0x3DD0_FAC6) < along.abs() {
        let k = mul(fwd, d[0].mul_add(fwd[0], d[1].mul_add(fwd[1], d[2] * fwd[2])));
        d = std::array::from_fn(|l| d[l] - k[l]);
        moved = true;
    }
    if d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2])) > reach * reach {
        d = mul(mul(d, rsqrte(dot3(d, d))), reach);
        moved = true;
    }
    let speed2 = v[0].mul_add(v[0], v[1].mul_add(v[1], v[2] * v[2]));
    if moved && speed2 > 1.0 {
        let run = v[0].mul_add(o[1][0], o[1][1].mul_add(v[1], o[1][2] * v[2]));
        d = mul(d, (0.9f32 as f64).powf((run.abs() * frame_dt) as f64) as f32);
    }
    if none {
        r.anchor = w.start;
    } else if moved {
        r.anchor = add(w.start, d);
    }
    let n = w.normal;
    if speed2 < 1.0 && o[0][0].mul_add(n[0], o[0][2].mul_add(n[2], o[0][1] * n[1])).abs() > 0.7 && n[2] < 0.7 {
        r.anchor = madd(o[0], dot3(n, o[0]) * 0.02, r.anchor);
    }
    along
}

#[allow(clippy::too_many_arguments)]
pub fn tyre(h: &Handling, b: &Mass, w: &Strut, r: &Tread, load: f32, slip: f32, m: &[V4; 4], vel: V4, ang: V4,
            t: &Times, axes: impl FnOnce(f32) -> (V4, V4), force: impl FnOnce(&TyreIn) -> [f32; 3]) -> Out {
    let weight = b.mass * G;
    if !(load > weight * 0.01) {
        return Out::default();
    }
    let blend = t.t * t.inv_dt;
    let rest = 1.0 - blend;
    let drive = r.drive_prev.mul_add(rest, r.drive * blend) * b.mass * G;
    let mut brake = r.brake_prev.mul_add(rest, r.brake * blend) * b.mass * G;
    if w.flags & 4 == 0 || !(r.handbrake <= 0.25) {
        brake = (fsel(r.handbrake - 0.05, r.handbrake, 0.05) * b.mass).mul_add(G, brake);
    }
    let (drive, brake) = if !(brake <= drive.abs()) {
        (0.0, brake)
    } else if !(drive <= brake) {
        (drive - brake, 0.0)
    } else {
        (brake + drive, 0.0)
    };
    let brake = if !(r.no_brake <= 0.0) { 0.0 } else { brake };
    let (fwd, side) = axes(r.steer);
    let o = &b.m;
    let arm: V4 = std::array::from_fn(|l| w.start[l] - o[3][l]);
    let mut spin = cross(arm, ang);
    let s = dot3(fwd, vel);
    if s > 5.0 && w.flags & 0x400 != 0 {
        let k = |c: f32| -> [f32; 4] { [c, c, c, 0.0] };
        let (five, tenth, one, half) = (k(5.0), k(0.1), k(1.0), k(0.5));
        for l in 0..4 {
            let v = vpick(vpick((s - five[l]) * tenth[l], one[l], true), 0.0, false);
            spin[l] *= v.mul_add(half[l], one[l]);
        }
    }
    let v: V4 = std::array::from_fn(|l| spin[l] + vel[l]);
    let cap = weight * 4.0;
    let load = if !(load <= cap) { cap } else { load };
    let (along, across) = (dot3(fwd, v), dot3(side, v));
    let (mut hold_along, mut hold_across) = (0.0, 0.0);
    let reach = h.traction_spring_delta_max;
    if !(reach <= 0.0) {
        let mut d = r.anchor;
        if !(d[0] == 0.0 && d[1] == 0.0 && d[2] == 0.0) {
            d = std::array::from_fn(|l| d[l] - w.start[l]);
        }
        let local = [dot3(o[0], arm), dot3(o[1], arm), dot3(o[2], arm), 1.0];
        let world: V4 = std::array::from_fn(|c| {
            if c == 3 {
                return local[0];
            }
            (0.0 + local[0] as f64 * m[0][c] as f64 + local[1] as f64 * m[1][c] as f64
                + local[2] as f64 * m[2][c] as f64 + local[3] as f64 * m[3][c] as f64) as f32
        });
        let off: V4 = std::array::from_fn(|l| world[l] - w.start[l]);
        let n = w.normal;
        let along_n = mul(n, dot3(n, off));
        let off: V4 = std::array::from_fn(|l| off[l] - along_n[l]);
        d = std::array::from_fn(|l| d[l] - off[l]);
        let stop = brake.abs() * (b.inv_mass * f32::from_bits(0x3DD0_FAC6));
        if brake == 0.0 || along.abs() > stop {
            let k = mul(fwd, dot3(fwd, d));
            d = std::array::from_fn(|l| d[l] - k[l]);
        }
        let len2 = d[0].mul_add(d[0], d[2].mul_add(d[2], d[1] * d[1]));
        if !(len2 <= reach * reach * 6.25) {
            d = mul(mul(d, rsqrte(dot3(d, d))), reach * 2.5);
        }
        hold_along = fwd[0].mul_add(d[0], fwd[2].mul_add(d[2], fwd[1] * d[1]));
        hold_across = side[0].mul_add(d[0], side[2].mul_add(d[2], side[1] * d[1]));
    }
    let n = w.normal;
    let speed = v[0].mul_add(v[0], v[1].mul_add(v[1], v[2] * v[2])).sqrt();
    let up = v[0].mul_add(n[0], n[1].mul_add(v[1], n[2] * v[2]));
    let input = TyreIn { slip_speed: speed - up, along, across, hold_along, hold_across, slip, brake, drive, load,
                         h: t.h };
    let [f_side, f_along, next] = force(&input);
    let push: V4 = std::array::from_fn(|l| fwd[l].mul_add(f_along, side[l] * f_side));
    let acc = mul(push, b.inv_mass);
    let arm: V4 = std::array::from_fn(|l| w.contact[l] - o[3][l]);
    let torque = cross(push, arm);
    let (b0, b1, b2) = (dot3(m[0], torque), dot3(m[1], torque), dot3(m[2], torque));
    let mut body = [b0, b1, b2, b0];
    if w.flags & 0x4_0000 != 0 {
        body[1] = 0.0;
    }
    let body: V4 = std::array::from_fn(|l| body[l] * b.inv_inertia[l]);
    Out { push, acc, ang_acc: by_columns(m, body), value: Some((next - slip) * t.inv_h) }
}

pub trait Calls {
    fn advance(&mut self, m: &[V4; 4], vel: V4, ang: V4, h: f32) -> [V4; 4];
    fn suspension(&mut self, i: usize, m: &[V4; 4], vel: V4, ang: V4, t: &Times) -> Out;
    #[allow(clippy::too_many_arguments)]
    fn tyre(&mut self, i: usize, load: f32, slip: f32, timer: f32, m: &[V4; 4], vel: V4, ang: V4, t: &Times) -> Out;
}

struct Fixed {
    lin: V4,
    ang_acc: V4,
}

#[allow(clippy::too_many_arguments)]
fn derivative(out: &mut Deriv, s: &State, t: &Times, prev: Option<&Deriv>, h: f32, fixed: &Fixed, wheels: &[Wheel],
              calls: &mut impl Calls) {
    let n = wheels.len();
    let (m, vel, ang, mut slip) = (s.m, s.vel, s.ang, s.slip);
    let (m, vel, ang) = match prev {
        Some(p) => {
            for (i, v) in slip[..n].iter_mut().enumerate() {
                *v = p.slip[i].mul_add(h, *v);
            }
            (calls.advance(&m, p.vel, p.ang, h), madd(p.acc, h, vel), madd(p.ang_acc, h, ang))
        }
        None => (m, vel, ang),
    };
    out.vel = vel;
    out.ang = ang;
    out.acc = add(GRAVITY, fixed.lin);
    out.ang_acc = fixed.ang_acc;
    out.slip[..n].fill(0.0);
    let mut rate = 0.0;
    for (i, w) in wheels.iter().enumerate() {
        if w.flags & 1 == 0 {
            continue;
        }
        let sus = calls.suspension(i, &m, vel, ang, t);
        out.acc = add(out.acc, sus.acc);
        out.ang_acc = add(out.ang_acc, sus.ang_acc);
        out.push[i] = sus.push;
        let tyre = calls.tyre(i, sus.value.unwrap_or(0.0), slip[i], w.timer, &m, vel, ang, t);
        rate = tyre.value.unwrap_or(rate);
        out.acc = add(out.acc, tyre.acc);
        out.ang_acc = add(out.ang_acc, tyre.ang_acc);
        out.slip[i] = rate;
        out.push[i] = add(out.push[i], tyre.push);
    }
}

pub fn integrate(b: &Body, force: V4, torque: V4, wheels: &mut [Wheel], dt: f32, calls: &mut impl Calls) -> State {
    let n = wheels.len();
    assert!(n <= MAX_WHEELS);
    let d: [f32; 3] = std::array::from_fn(|r| dot3(b.m[r], torque));
    let local = [d[0] * b.inv_inertia[0], d[1] * b.inv_inertia[1], d[2] * b.inv_inertia[2], d[0] * b.inv_inertia[3]];
    let w: [f32; 3] = std::array::from_fn(|c| dot3(local, [b.m[0][c], b.m[1][c], b.m[2][c], b.m[3][c]]));
    let fixed = Fixed { lin: mul(force, b.inv_mass), ang_acc: [w[0], w[1], w[2], w[0]] };
    let c = mathrt::ceil((dt / 0.02) as f64) as f32;
    let steps = fsel(1.0 - c, 1.0, c);
    let steps = if steps.is_nan() { i32::MIN } else { steps as i32 };
    let h = dt / steps as f32;
    let mut s = State { m: b.m, vel: b.vel, ang: b.ang, ..Default::default() };
    for (i, w) in wheels.iter_mut().enumerate() {
        s.slip[i] = w.slip;
        w.sums = [[0.0; 4]; 2];
    }
    let mut t = Times { dt, inv_dt: 1.0 / dt, h, inv_h: 1.0 / h, ..Default::default() };
    let mut k = [Deriv::default(); 4];
    let mean = |a: V4, b: V4, c: V4, d: V4| mul(add(madd(add(b, c), 2.0, a), d), SIXTH);
    let mut end = h;
    for step in 0..steps.max(0) {
        for w in wheels.iter_mut() {
            let d = w.timer - h;
            w.timer = fsel(-d, 0.0, d);
        }
        let kf = step as f32;
        (t.t, t.inv_t, t.inv_k) = (end, 1.0 / end, 1.0 / fsel(1.0 - kf, 1.0, kf));
        let [k1, k2, k3, k4] = &mut k;
        derivative(k1, &s, &t, None, 0.0, &fixed, wheels, calls);
        derivative(k2, &s, &t, Some(&*k1), h * 0.5, &fixed, wheels, calls);
        derivative(k3, &s, &t, Some(&*k2), h * 0.5, &fixed, wheels, calls);
        derivative(k4, &s, &t, Some(&*k3), h, &fixed, wheels, calls);
        let (vel, ang) = (mean(k1.vel, k2.vel, k3.vel, k4.vel), mean(k1.ang, k2.ang, k3.ang, k4.ang));
        s.m = calls.advance(&s.m, vel, ang, h);
        s.vel = madd(mean(k1.acc, k2.acc, k3.acc, k4.acc), h, s.vel);
        s.ang = madd(mean(k1.ang_acc, k2.ang_acc, k3.ang_acc, k4.ang_acc), h, s.ang);
        let a = kf / (step + 1) as f32;
        let rest = 1.0 - a;
        for i in 0..n {
            let r = ((k3.slip[i] + k2.slip[i]).mul_add(2.0, k4.slip[i]) + k1.slip[i]) * SIXTH;
            s.slip[i] = r.mul_add(h, s.slip[i]);
            s.push[i] = madd(s.push[i], a, mul(mean(k1.push[i], k2.push[i], k3.push[i], k4.push[i]), rest));
        }
        end += h;
    }
    s
}
