use crate::lift::{vmadd, vrsq};

pub const MASS: f32 = 25.0;
pub const MARGIN: f32 = 0.04;
pub const SLOP: f32 = 0.007;
pub const BREAK: f32 = 0.02;
pub const SWEEPS: usize = 8;
pub const SUBSTEPS: usize = 2;
pub const SOR: f32 = 1.0;
const FAR: f32 = 1e30;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub z: f32,
    pub half: f32,
    pub core: f32,
    pub round: f32,
    pub types: u32,
    pub include: u32,
}

pub const PARTS: [Part; 4] = [
    Part { z: 0.085, half: 1.27 * 0.5, core: 0.15, round: 0.1, types: u32::MAX, include: u32::MAX },
    Part { z: -0.875, half: 0.65 * 0.5, core: 0.0, round: 0.1, types: u32::MAX, include: u32::MAX },
    Part { z: 0.085, half: 0.97 * 0.5, core: 0.0, round: 0.25, types: 0x20, include: 0x7800 },
    Part { z: -0.55, half: 0.3 * 0.5, core: 0.15, round: 0.1, types: u32::MAX, include: u32::MAX },
];

pub fn meets(a: &Part, b: &Part) -> bool {
    a.types & b.include != 0 && b.types & a.include != 0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub root: [f32; 3],
    pub heading: f32,
    pub turn: Option<[[f32; 3]; 3]>,
    pub vel: [f32; 3],
    pub inv_mass: f32,
    pub state: u32,
    pub margin: [f32; 3],
    pub spin: [f32; 3],
    pub com: [f32; 3],
    pub inv_inertia: [[f32; 3]; 3],
}

impl Body {
    pub fn ped(root: [f32; 3], heading: f32, vel: [f32; 3]) -> Body {
        Body { root, heading, turn: None, vel, inv_mass: 1.0 / MASS, state: 2, margin: [MARGIN; 3], spin: [0.0; 3], com: root, inv_inertia: [[0.0; 3]; 3] }
    }

    pub fn rows(&self) -> [[f32; 3]; 3] {
        if let Some(t) = self.turn {
            return t;
        }
        let (s, c) = self.heading.sin_cos();
        [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
    }

    pub fn lever(&self, local: [f32; 3]) -> [f32; 3] {
        let r = self.rows();
        [0, 1, 2].map(|i| r[0][i] * local[0] + r[1][i] * local[1] + r[2][i] * local[2] + self.root[i] - self.com[i])
    }

    fn turn_by(&self, v: [f32; 3]) -> [f32; 3] {
        self.inv_inertia.map(|r| dot(r, v))
    }

    pub fn local(&self, p: [f32; 3]) -> [f32; 3] {
        let d = [p[0] - self.root[0], p[1] - self.root[1], p[2] - self.root[2]];
        self.rows().map(|r| r[0] * d[0] + r[1] * d[1] + r[2] * d[2])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub a: usize,
    pub b: usize,
    pub n: [f32; 3],
    pub depth: f32,
    pub at_a: [f32; 3],
    pub at_b: [f32; 3],
    pub warm: f32,
}

pub const FRAGMENT: u32 = 0x40;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Side {
    pub rows: [[f32; 3]; 3],
    pub at: [f32; 3],
    pub n: [f32; 3],
    pub margin: [f32; 3],
    pub inv_mass: f32,
    pub state: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub speed: f32,
    pub moves: Option<([f32; 3], [f32; 3])>,
}

fn vdot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((0.0f64 + a[0] as f64 * b[0] as f64) + a[1] as f64 * b[1] as f64 + a[2] as f64 * b[2] as f64) as f32
}

fn support(d: [f32; 3], margin: [f32; 3]) -> [f32; 3] {
    let dd = vdot3(d, d);
    let r = vrsq(dd);
    let u = if dd > 0.0 { d.map(|v| r * v) } else { [1.0, 0.0, 0.0] };
    [0, 1, 2].map(|k| vmadd(u[k], margin[k], 0.0))
}

fn measure(s: &Side) -> f32 {
    let n = s.rows.map(|r| vdot3(r, s.n));
    let p = support(n, s.margin);
    let d = [p[0] - s.at[0], p[1] - s.at[1], p[2] - s.at[2]];
    let m = n[2].mul_add(d[2], n[0].mul_add(d[0], d[1] * n[1]));
    if m >= 0.0 { m } else { 0.0 }
}

pub fn target(depth: f32, inv_dt: f32, a: &Side, b: &Side) -> Target {
    let d = depth - SLOP;
    let half = d * 0.5;
    let keep = if half - SLOP >= 0.0 { SLOP } else { half };
    let keep = if -keep >= 0.0 { 0.0 } else { keep };
    let d = d - keep;
    let (ma, mb) = (measure(a), measure(b));
    let m = if ma - mb >= 0.0 { mb } else { ma };
    let speed = |d: f32| -(-(d * inv_dt)) + 0.0;
    if a.state == 1 || b.state == 1 || m == FAR || !(d > 0.0) || !(d > m) {
        return Target { speed: speed(d), moves: None };
    }
    let kept = m * 0.5;
    let total = d - kept;
    let by = a.n.map(|v| v * total);
    let (a2, b2) = (a.inv_mass * a.inv_mass, b.inv_mass * b.inv_mass);
    let (sa, sb) = if a2 > b2 {
        let s = 1.0 / ((b2 / a2).sqrt() + 1.0);
        (s, 1.0 - s)
    } else if b2 > a2 {
        let s = 1.0 / ((a2 / b2).sqrt() + 1.0);
        (1.0 - s, s)
    } else {
        (0.5, 0.5)
    };
    let nb = -sb;
    Target { speed: speed(kept), moves: Some((by.map(|v| v * sa), by.map(|v| v * nb))) }
}

pub fn step(bodies: &mut [Body], contacts: &[Contact], h: f32) -> (Vec<[f32; 3]>, Vec<f32>) {
    let mut moves = vec![[0.0f32; 3]; bodies.len()];
    let was: Vec<[f32; 3]> = bodies.iter().map(|b| b.vel).collect();
    let side = |b: &Body, at: [f32; 3], n: [f32; 3]| Side { rows: b.rows(), at, n, margin: b.margin, inv_mass: b.inv_mass, state: b.state };
    let rows: Vec<(usize, usize, [f32; 3], f32, f32, [f32; 3], [f32; 3])> = contacts.iter().map(|c| {
        let (ba, bb) = (&bodies[c.a], &bodies[c.b]);
        let t = target(c.depth, 1.0 / h, &side(ba, c.at_a, c.n), &side(bb, c.at_b, c.n.map(|v| -v)));
        if let Some((ma, mb)) = t.moves {
            moves[c.a] = add(moves[c.a], ma);
            moves[c.b] = add(moves[c.b], mb);
        }
        let (ra, rb) = (ba.lever(c.at_a), bb.lever(c.at_b));
        let (ta, tb) = (cross(ra, c.n), cross(rb, c.n));
        let k = ba.inv_mass + bb.inv_mass + dot(ta, ba.turn_by(ta)) + dot(tb, bb.turn_by(tb));
        (c.a, c.b, c.n, t.speed, if k > 0.0 { 1.0 / k } else { 0.0 }, ta, tb)
    }).collect();
    let mut sum: Vec<f32> = contacts.iter().map(|c| c.warm).collect();
    for (&(a, b, n, _, _, ta, tb), &l) in rows.iter().zip(&sum) {
        bodies[a].vel = add(bodies[a].vel, n.map(|x| x * l * bodies[a].inv_mass));
        bodies[b].vel = add(bodies[b].vel, n.map(|x| x * -l * bodies[b].inv_mass));
        let (wa, wb) = (bodies[a].turn_by(ta), bodies[b].turn_by(tb));
        bodies[a].spin = add(bodies[a].spin, wa.map(|x| x * l));
        bodies[b].spin = add(bodies[b].spin, wb.map(|x| x * -l));
    }
    for _ in 0..SWEEPS {
        let mut moved = 0.0f32;
        for (r, &(a, b, n, speed, mass, ta, tb)) in rows.iter().enumerate() {
            let v = dot(n, bodies[a].vel) + dot(ta, bodies[a].spin) - dot(n, bodies[b].vel) - dot(tb, bodies[b].spin);
            let total = (sum[r] + (speed - v) * mass * SOR).max(0.0);
            let l = total - sum[r];
            sum[r] = total;
            moved += l * l;
            bodies[a].vel = add(bodies[a].vel, n.map(|x| x * l * bodies[a].inv_mass));
            bodies[b].vel = add(bodies[b].vel, n.map(|x| x * -l * bodies[b].inv_mass));
            let (wa, wb) = (bodies[a].turn_by(ta), bodies[b].turn_by(tb));
            bodies[a].spin = add(bodies[a].spin, wa.map(|x| x * l));
            bodies[b].spin = add(bodies[b].spin, wb.map(|x| x * -l));
        }
        if moved == 0.0 {
            break;
        }
    }
    for (k, m) in moves.iter_mut().enumerate() {
        let dv = [0, 1, 2].map(|i| bodies[k].vel[i] - was[k][i]);
        *m = add(*m, dv.map(|v| v * h));
    }
    (moves, sum)
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

