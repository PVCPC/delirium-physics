use crate::estimate::fneg;

const ZERO: [f32; 3] = [0.0; 3];
pub const GRAVITY: [f32; 3] = [0.0, 0.0, -9.8];
pub const MAX_SPEED: f32 = 80.0;
pub const MAX_SPIN: f32 = std::f32::consts::TAU;
pub const RENORM: u32 = 32;

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: [f32; 3], s: f32) -> [f32; 3] { a.map(|x| x * s) }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit(a: [f32; 3]) -> [f32; 3] { scale(a, 1.0 / dot(a, a).sqrt()) }
fn fused(a: [f32; 3], s: f32, c: [f32; 3]) -> [f32; 3] { std::array::from_fn(|k| a[k].mul_add(s, c[k])) }
fn vx(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [(-a[2]).mul_add(b[1], a[1] * b[2]), (-a[0]).mul_add(b[2], a[2] * b[0]), (-a[1]).mul_add(b[0], a[0] * b[1])]
}
fn vd(a: [f32; 3], b: [f32; 3]) -> f32 {
    let p = |i: usize| a[i] as f64 * b[i] as f64;
    ((p(0) + p(2)) + p(1)) as f32
}
fn col(m: &[[f32; 3]; 3], k: usize) -> [f32; 3] { [m[0][k], m[1][k], m[2][k]] }

fn rotate(rows: &mut [[f32; 3]; 3], axis: [f32; 3], angle: f32) {
    let (s, c) = angle.sin_cos();
    for r in rows.iter_mut() {
        *r = add(add(scale(*r, c), scale(cross(axis, *r), s)), scale(axis, dot(axis, *r) * (1.0 - c)));
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Damping {
    pub linear: [[f32; 3]; 3],
    pub angular: [[f32; 3]; 3],
}

pub fn damp(v: [f32; 3], [c, l, q]: [[f32; 3]; 3], dt: f32) -> [f32; 3] {
    let n = dot(v, v).sqrt();
    std::array::from_fn(|i| {
        let x = v[i];
        let d = if x > 0.0 { c[i] } else { -c[i] } + l[i] * x + q[i] * x * n;
        -(if d.abs() * dt > x.abs() { x / dt } else { d })
    })
}

pub fn held(v: [f32; 3], cap: f32) -> Option<f32> {
    let f = |x: f32| x as f64;
    let r = |x: f64| x as f32 as f64;
    let n2 = r(f(v[2]).mul_add(f(v[2]), r(f(v[0]).mul_add(f(v[0]), r(f(v[1]) * f(v[1]))))));
    if n2 <= r(f(cap) * f(cap)) {
        return None;
    }
    let mut y = crate::estimate::frsqrte(n2);
    for _ in 0..2 {
        y = (fneg((y * y).mul_add(n2, -1.0))).mul_add(y * 0.5, y);
    }
    Some(y as f32 * cap)
}

fn inverse(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let [[a0, a1, a2], [b0, b1, b2], [c0, c1, c2]] = m;
    let c00 = b1.mul_add(c2, -(b2 * c1));
    let c01 = b0.mul_add(c2, -(c0 * b2));
    let c02 = b0.mul_add(c1, -(c0 * b1));
    let det = a2.mul_add(c02, a0.mul_add(c00, fneg(a1 * c01)));
    let big = m.iter().flatten().fold(0.0f32, |g, x| g.max(x.abs()));
    if !(det.abs() > big * 3.6e-7) {
        return m;
    }
    let k = 1.0 / det;
    [[k * c00, -(a1.mul_add(c2, -(c1 * a2)) * k), k * b2.mul_add(a1, -(b1 * a2))],
     [fneg(k * c01), k * c2.mul_add(a0, fneg(a2 * c0)), fneg(k * b2.mul_add(a0, fneg(b0 * a2)))],
     [k * c02, fneg(k * c1.mul_add(a0, fneg(a1 * c0))), k * b1.mul_add(a0, fneg(b0 * a1))]]
}

fn solve(n: [[f32; 3]; 3], rhs: [f32; 3]) -> Option<[f32; 3]> {
    let [[a0, a1, a2], [b0, b1, b2], [c0, c1, c2]] = n;
    let (mut big, mut at) = (0.0f32, None);
    for (i, r) in n.iter().enumerate() {
        for x in r {
            if big < x.abs() {
                (big, at) = (x.abs(), Some(i));
            }
        }
    }
    let at = at?;
    if big < f32::from_bits(0x322B_CC77) {
        return None;
    }
    let f4 = big * f32::from_bits(0x3A1D_4952);
    let k = [[b1.mul_add(c2, -(b2 * c1)), fneg(b0.mul_add(c2, -(c0 * b2))), b0.mul_add(c1, -(c0 * b1))],
             [fneg(a1.mul_add(c2, -(a2 * c1))), a0.mul_add(c2, -(a2 * c0)), fneg(a0.mul_add(c1, -(a1 * c0)))],
             [a1.mul_add(b2, -(a2 * b1)), fneg(a0.mul_add(b2, -(a2 * b0))), a0.mul_add(b1, -(a1 * b0))]];
    let row_max: [f32; 3] = k.map(|r| r.iter().fold(0.0f32, |g, x| if g < x.abs() { x.abs() } else { g }));
    let cof = row_max.iter().fold(0.0f32, |g, &x| if g < x { x } else { g });
    let det = k[0][1].mul_add(a1, a0.mul_add(k[0][0], k[0][2] * a2));
    let (f2, cube, edge) = (f4 * f4, f4 * f4 * f4, cof * f4);
    let past = if cube - edge >= 0.0 { cube } else { edge };
    if !(det.abs() > past && row_max[(at + 1) % 3] > f2 && row_max[(at + 2) % 3] > f2) {
        return None;
    }
    let r = crate::estimate::vrefp(det);
    let r = r.mul_add((-r).mul_add(det, 1.0), r);
    Some(k.map(|row| vd(row, rhs) * r))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Collider {
    pub pos: [f32; 3],
    pub rows: [[f32; 3]; 3],
    pub v: [f32; 3],
    pub w: [f32; 3],
    pub p: [f32; 3],
    pub l: [f32; 3],
    pub force: [f32; 3],
    pub torque: [f32; 3],
    pub impulse: [f32; 3],
    pub ang_impulse: [f32; 3],
    pub push: [f32; 3],
    pub turn: [f32; 3],
    pub mass: f32,
    pub inv_mass: f32,
    pub inertia: [f32; 3],
    pub inv_inertia: [f32; 3],
    pub max_speed: f32,
    pub max_spin: f32,
    pub m: [[f32; 3]; 3],
    pub sm: [[f32; 3]; 3],
    pub rsm: [[f32; 3]; 3],
    pub at_point: bool,
    pub steps: u32,
}

impl Collider {
    pub fn new(pos: [f32; 3], rows: [[f32; 3]; 3], mass: f32, inertia: [f32; 3]) -> Collider {
        Collider { pos, rows, v: ZERO, w: ZERO, p: ZERO, l: ZERO, force: ZERO, torque: ZERO, impulse: ZERO, ang_impulse: ZERO,
                   push: ZERO, turn: ZERO, mass, inv_mass: 1.0 / mass, inertia, inv_inertia: inertia.map(|i| 1.0 / i),
                   max_speed: MAX_SPEED, max_spin: MAX_SPIN, m: [ZERO; 3], sm: [ZERO; 3], rsm: [ZERO; 3], at_point: false, steps: 0 }
    }

    fn velocity(&mut self) {
        self.v = scale(self.p, self.inv_mass);
        if let Some(k) = held(self.v, self.max_speed) {
            (self.v, self.p) = (scale(self.v, k), scale(self.p, k));
        }
    }

    fn spin(&mut self) {
        let r = &self.rows;
        let body: [f32; 3] = std::array::from_fn(|i| vd(r[i], self.l) * self.inv_inertia[i]);
        self.w = std::array::from_fn(|k| vd(body, col(r, k)));
        if let Some(k) = held(self.w, self.max_spin) {
            (self.w, self.l) = (scale(self.w, k), scale(self.l, k));
        }
    }

    pub fn apply_impulses(&mut self) {
        if self.impulse != ZERO {
            (self.p, self.impulse) = (add(self.p, self.impulse), ZERO);
            self.velocity();
        }
        if self.ang_impulse != ZERO {
            (self.l, self.ang_impulse) = (add(self.l, self.ang_impulse), ZERO);
            self.spin();
        }
    }

    pub fn add_force(&mut self, f: [f32; 3]) { self.force = add(self.force, f); }
    pub fn add_torque(&mut self, t: [f32; 3]) { self.torque = add(self.torque, t); }

    pub fn add_impulse(&mut self, j: [f32; 3], ang: [f32; 3]) {
        (self.impulse, self.ang_impulse) = (add(self.impulse, j), add(self.ang_impulse, ang));
    }

    pub fn push(&mut self, d: [f32; 3], turn: [f32; 3]) {
        let held = |v: [f32; 3]| {
            let n = dot(v, v).sqrt();
            if n > 100.0 { scale(v, 100.0 / n) } else { v }
        };
        (self.push, self.turn) = (add(self.push, held(d)), add(self.turn, held(turn)));
    }

    fn take_push(&mut self) {
        (self.pos, self.push) = (add(self.pos, self.push), ZERO);
        let n2 = dot(self.turn, self.turn);
        if n2 > 1e-12 {
            let n = n2.sqrt();
            rotate(&mut self.rows, scale(self.turn, 1.0 / n), n);
        }
        self.turn = ZERO;
    }

    fn renormalise(&mut self, random: u32) {
        let [a, b, c] = self.rows;
        self.rows = if [a, b, c].contains(&ZERO) {
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        } else {
            let b = unit(b);
            let c = unit(cross(a, b));
            [cross(b, c), b, c]
        };
        self.steps = (random & 0x7FFF_FFFF) % 16;
    }

    pub fn update(&mut self, gravity: Option<f32>, damping: Option<&Damping>, dt: f32, random: u32) {
        self.take_push();
        self.apply_impulses();
        self.pos = add(self.pos, scale(self.v, dt));
        let turn = scale(self.w, dt);
        let n2 = dot(turn, turn);
        if n2 > 1e-12 {
            let n = n2.sqrt();
            rotate(&mut self.rows, scale(turn, 1.0 / n), n);
            self.spin();
        }
        if let Some(k) = gravity {
            self.add_force(scale(GRAVITY, k * self.mass));
        }
        if let Some(d) = damping {
            let body = |v: [f32; 3]| self.rows.map(|r| dot(r, v));
            let world = |v: [f32; 3]| (0..3).fold(ZERO, |a, i| add(a, scale(self.rows[i], v[i])));
            let lin = damp(body(self.v), d.linear, dt).map(|x| x * self.mass);
            let ang = damp(body(self.w), d.angular, dt);
            let ang = std::array::from_fn(|i| ang[i] * self.inertia[i]);
            let (f, t) = (world(lin), world(ang));
            self.add_force(f);
            self.add_torque(t);
        }
        self.steps += 1;
        if self.steps > RENORM {
            self.renormalise(random);
        }
    }

    pub fn add_force_at(&mut self, f: [f32; 3], point: [f32; 3], m: [[f32; 3]; 3]) {
        self.at_point = true;
        self.force = add(self.force, f);
        let r = [point[0] - self.pos[0], point[1] - self.pos[1], point[2] - self.pos[2]];
        self.torque = add(self.torque, vx(r, f));
        let s: [[f32; 3]; 3] = std::array::from_fn(|i| vx(std::array::from_fn(|k| (k == i) as u8 as f32), r));
        let sm: [[f32; 3]; 3] = s.map(|s| fused(m[2], s[2], fused(m[1], s[1], scale(m[0], s[0]))));
        for i in 0..3 {
            self.m[i] = add(self.m[i], m[i]);
            self.sm[i] = add(self.sm[i], sm[i]);
            self.rsm[i] = add(self.rsm[i], vx(r, sm[i]));
        }
    }

    pub fn forces_to_velocity(&mut self, dt: f32) {
        self.impulse = fused(self.force, dt, self.impulse);
        self.ang_impulse = fused(self.torque, dt, self.ang_impulse);
        (self.force, self.torque) = (ZERO, ZERO);
        if self.at_point {
            self.implicit(dt);
        } else {
            self.apply_impulses();
        }
    }

    fn world_inertia(&self) -> [[f32; 3]; 3] {
        let (r, i) = (&self.rows, self.inertia);
        std::array::from_fn(|k| {
            let x = [r[0][k] * i[0], r[1][k] * i[1], r[2][k] * i[2]];
            fused(r[2], x[2], fused(r[1], x[1], scale(r[0], x[0])))
        })
    }

    fn implicit(&mut self, dt: f32) {
        let k = self.inv_mass * dt;
        let mut q = self.m.map(|r| scale(r, k));
        for (i, r) in q.iter_mut().enumerate() {
            r[i] += 1.0;
        }
        let qi = inverse(q);
        let b = self.sm;
        let p: [[f32; 3]; 3] = b.map(|b| fused(qi[2], b[2], fused(qi[1], b[1], scale(qi[0], b[0]))));
        let z: [[f32; 3]; 3] = p.map(|p| std::array::from_fn(|j| vd(p, b[j])));
        let nk = -(k * dt);
        let iw = self.world_inertia();
        let n: [[f32; 3]; 3] = std::array::from_fn(|i| {
            let m = std::array::from_fn(|c| self.rsm[i][c].mul_add(dt, z[i][c] * nk));
            add(m, iw[i])
        });
        let u = scale(self.impulse, -k);
        let g: [f32; 3] = std::array::from_fn(|c| vd(u, col(&qi, c)));
        let h = add(b.map(|b| vd(b, g)), self.ang_impulse);
        let x = solve(n, h).unwrap_or(ZERO);
        self.l = add(self.l, std::array::from_fn(|c| vd(x, col(&iw, c))));
        self.spin();
        let y = scale(x, dt);
        let w = scale(add(std::array::from_fn(|c| vd(y, col(&b, c))), self.impulse), self.inv_mass);
        self.p = fused(std::array::from_fn(|c| vd(w, col(&qi, c))), self.mass, self.p);
        self.velocity();
        (self.impulse, self.ang_impulse) = (ZERO, ZERO);
        (self.m, self.sm, self.rsm, self.at_point) = ([ZERO; 3], [ZERO; 3], [ZERO; 3], false);
    }

    pub fn finish(&mut self) {
        self.take_push();
    }

    pub fn set_velocity(&mut self, v: [f32; 3], w: [f32; 3]) {
        self.p = scale(v, self.mass);
        self.velocity();
        let r = &self.rows;
        self.l = (0..3).fold(ZERO, |a, i| add(a, scale(r[i], self.inertia[i] * dot(r[i], w))));
        self.spin();
    }
}

