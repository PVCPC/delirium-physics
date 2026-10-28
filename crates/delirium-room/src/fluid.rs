use bevy::math::Vec3;
use bevy::tasks::{ComputeTaskPool, TaskPool};

const AIR: u8 = 0;
const FLUID: u8 = 1;
const SOLID: u8 = 2;
const G: f32 = 9.81;

#[derive(Clone, Copy, Default, PartialEq)]
pub struct Physics {
    pub relax: bool,
    pub ghost: bool,
    pub vort: bool,
    pub spray: bool,
}

impl Physics {
    pub fn bits(self) -> u32 {
        self.relax as u32 | (self.ghost as u32) << 1 | (self.vort as u32) << 2 | (self.spray as u32) << 3
    }
}

const VORT: f32 = 0.5;

const GHOST_MIN: f32 = 0.2;

const KEEP: f32 = 0.03;
const KEEP_BAND: f32 = 0.1;

#[derive(Clone, Copy)]
pub struct Particle {
    pub x: Vec3,
    pub v: Vec3,
    c: [Vec3; 3],
}

impl Particle {
    pub fn grad(&self) -> [Vec3; 3] {
        self.c
    }
}

pub struct Fluid {
    pub n: [usize; 3],
    pub h: f32,
    pub lo: Vec3,
    cell: Vec<u8>,
    vel: [Vec<f32>; 3],
    known: [Vec<bool>; 3],
    pub p: Vec<Particle>,
    start: Vec<u32>,
    order: Vec<u32>,
    q: Vec<f32>,
    pub iters: usize,
    pub phys: Physics,
}

fn par<T: Send>(out: &mut [T], f: impl Fn(usize, &mut T) + Sync) {
    let pool = ComputeTaskPool::get_or_init(TaskPool::default);
    let chunk = out.len().div_ceil(pool.thread_num().max(1)).max(2048);
    let f = &f;
    pool.scope(|s| {
        for (k, c) in out.chunks_mut(chunk).enumerate() {
            s.spawn(async move {
                for (j, x) in c.iter_mut().enumerate() {
                    f(k * chunk + j, x);
                }
            });
        }
    });
}

fn sum(len: usize, f: impl Fn(usize) -> f64 + Sync) -> f64 {
    let pool = ComputeTaskPool::get_or_init(TaskPool::default);
    let chunk = len.div_ceil(pool.thread_num().max(1)).max(2048);
    let f = &f;
    pool.scope(|s| {
        for k in 0..len.div_ceil(chunk) {
            s.spawn(async move { (k * chunk..((k + 1) * chunk).min(len)).map(f).sum::<f64>() });
        }
    }).into_iter().sum()
}

fn at(d: [usize; 3], i: usize, j: usize, k: usize) -> usize {
    i + d[0] * (j + d[1] * k)
}

fn ijk(d: [usize; 3], f: usize) -> [usize; 3] {
    [f % d[0], f / d[0] % d[1], f / (d[0] * d[1])]
}

fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFF_FFFF) as f32 / 16_777_216.0
}

impl Fluid {
    pub fn new(n: [usize; 3], h: f32, lo: Vec3) -> Fluid {
        let cells = n[0] * n[1] * n[2];
        let cell = (0..cells).map(|c| {
            let i = ijk(n, c);
            if (0..3).any(|a| i[a] == 0 || i[a] == n[a] - 1) { SOLID } else { AIR }
        }).collect();
        let faces = |a: usize| { let mut d = n; d[a] += 1; d[0] * d[1] * d[2] };
        Fluid { n, h, lo, cell, vel: [0, 1, 2].map(|a| vec![0.0; faces(a)]), known: [0, 1, 2].map(|a| vec![false; faces(a)]), p: vec![],
                start: vec![0; cells + 1], order: vec![], q: vec![0.0; cells], iters: 0, phys: Physics::default() }
    }

    fn dims(&self, a: usize) -> [usize; 3] {
        let mut d = self.n;
        d[a] += 1;
        d
    }

    pub fn fill(&mut self, a: Vec3, b: Vec3) {
        let n = self.n;
        for c in 0..self.cell.len() {
            let i = ijk(n, c);
            let o = self.lo + Vec3::new(i[0] as f32, i[1] as f32, i[2] as f32) * self.h;
            let mid = o + Vec3::splat(0.5 * self.h);
            if self.cell[c] == SOLID || mid.cmplt(a).any() || mid.cmpgt(b).any() {
                continue;
            }
            for s in 0..8u32 {
                let k = (c as u32) * 8 + s;
                let j = Vec3::new(hash(3 * k), hash(3 * k + 1), hash(3 * k + 2));
                let sub = Vec3::new((s & 1) as f32, (s >> 1 & 1) as f32, (s >> 2) as f32);
                self.p.push(Particle { x: o + (sub + 0.25 + (j - 0.5) * 0.5) * 0.5 * self.h, v: Vec3::ZERO, c: [Vec3::ZERO; 3] });
            }
        }
    }

    fn cell_of(&self, x: Vec3) -> usize {
        let g = ((x - self.lo) / self.h).floor();
        let c = |a: usize| (g[a].max(0.0) as usize).min(self.n[a] - 1);
        at(self.n, c(0), c(1), c(2))
    }

    pub fn step(&mut self, dt: f32) {
        let fastest = self.p.iter().map(|p| p.v.length()).fold(0.0, f32::max) + (2.0 * G * self.h).sqrt();
        let k = (fastest * dt / self.h).ceil().max(1.0) as usize;
        for _ in 0..k {
            self.substep(dt / k as f32);
        }
    }

    pub fn steps(&mut self, dt: f32, k: usize) {
        for _ in 0..k {
            self.substep(dt / k as f32);
        }
    }

    fn substep(&mut self, dt: f32) {
        self.sort();
        self.to_grid();
        for (v, k) in self.vel[2].iter_mut().zip(&self.known[2]) {
            if *k {
                *v -= G * dt;
            }
        }
        self.walls();
        if self.phys.vort {
            self.vorticity(dt);
        }
        self.project(dt);
        self.extrapolate();
        self.to_particles();
        self.advect(dt);
    }

    fn sort(&mut self) {
        let cells = self.cell.len();
        let of: Vec<u32> = self.p.iter().map(|p| self.cell_of(p.x) as u32).collect();
        self.start.iter_mut().for_each(|s| *s = 0);
        for &c in &of {
            self.start[c as usize + 1] += 1;
        }
        for c in 0..cells {
            self.start[c + 1] += self.start[c];
        }
        let mut next = self.start.clone();
        self.order.resize(self.p.len(), 0);
        for (i, &c) in of.iter().enumerate() {
            self.order[next[c as usize] as usize] = i as u32;
            next[c as usize] += 1;
        }
        for c in 0..cells {
            if self.cell[c] != SOLID {
                self.cell[c] = if self.start[c + 1] > self.start[c] { FLUID } else { AIR };
            }
        }
    }

    fn to_grid(&mut self) {
        let (n, h, lo) = (self.n, self.h, self.lo);
        for a in 0..3 {
            let d = self.dims(a);
            let (p, start, order) = (&self.p, &self.start, &self.order);
            let mut out: Vec<(f32, bool)> = vec![(0.0, false); self.vel[a].len()];
            par(&mut out, |f, o| {
                let i = ijk(d, f);
                let mut fp = lo + Vec3::new(i[0] as f32 + 0.5, i[1] as f32 + 0.5, i[2] as f32 + 0.5) * h;
                fp[a] -= 0.5 * h;
                let r = |b: usize| if b == a { i[b].saturating_sub(1)..=i[b].min(n[b] - 1) } else { i[b].saturating_sub(1)..=(i[b] + 1).min(n[b] - 1) };
                let (mut s, mut ws) = (0.0, 0.0);
                for ck in r(2) {
                    for cj in r(1) {
                        for ci in r(0) {
                            let c = at(n, ci, cj, ck);
                            for &pi in &order[start[c] as usize..start[c + 1] as usize] {
                                let q = &p[pi as usize];
                                let dd = (q.x - fp).abs() / h;
                                if dd.cmpge(Vec3::ONE).any() {
                                    continue;
                                }
                                let w = (1.0 - dd.x) * (1.0 - dd.y) * (1.0 - dd.z);
                                s += w * (q.v[a] + q.c[a].dot(fp - q.x));
                                ws += w;
                            }
                        }
                    }
                }
                if ws > 0.0 {
                    *o = (s / ws, true);
                }
            });
            for (f, (v, k)) in out.into_iter().enumerate() {
                (self.vel[a][f], self.known[a][f]) = (v, k);
            }
        }
    }

    fn sides(&self, a: usize, f: usize) -> [Option<usize>; 2] {
        let i = ijk(self.dims(a), f);
        let mut lo = i;
        let below = (i[a] > 0).then(|| { lo[a] -= 1; at(self.n, lo[0], lo[1], lo[2]) });
        let above = (i[a] < self.n[a]).then(|| at(self.n, i[0], i[1], i[2]));
        [below, above]
    }

    fn walls(&mut self) {
        for a in 0..3 {
            for f in 0..self.vel[a].len() {
                let solid = self.sides(a, f).map(|c| c.is_none_or(|c| self.cell[c] == SOLID));
                if solid[0] || solid[1] {
                    let v = self.vel[a][f];
                    let off = self.known[a][f] && !(solid[0] && solid[1]) && if solid[0] { v > 0.0 } else { v < 0.0 };
                    (self.vel[a][f], self.known[a][f]) = (if off { v } else { 0.0 }, true);
                }
            }
        }
    }

    fn inner(&self, c: usize) -> bool {
        self.cell[c] == FLUID && self.around(c).count() == 6 && self.around(c).all(|(_, w)| w)
    }

    fn centre_vel(&self, c: usize) -> Vec3 {
        let i = ijk(self.n, c);
        let mut v = Vec3::ZERO;
        for a in 0..3 {
            let d = self.dims(a);
            let mut j = i;
            let lo = at(d, j[0], j[1], j[2]);
            j[a] += 1;
            v[a] = 0.5 * (self.vel[a][lo] + self.vel[a][at(d, j[0], j[1], j[2])]);
        }
        v
    }

    fn vorticity(&mut self, dt: f32) {
        let (n, h) = (self.n, self.h);
        let cells = self.cell.len();
        let inner: Vec<bool> = (0..cells).map(|c| self.inner(c)).collect();
        let w: Vec<Vec3> = (0..cells).map(|c| {
            if !inner[c] {
                return Vec3::ZERO;
            }
            let i = ijk(n, c);
            let d = |a: usize| {
                let (mut lo, mut hi) = (i, i);
                lo[a] -= 1;
                hi[a] += 1;
                self.centre_vel(at(n, hi[0], hi[1], hi[2])) - self.centre_vel(at(n, lo[0], lo[1], lo[2]))
            };
            let (dx, dy, dz) = (d(0), d(1), d(2));
            Vec3::new(dy.z - dz.y, dz.x - dx.z, dx.y - dy.x) / (2.0 * h)
        }).collect();
        let mag: Vec<f32> = w.iter().map(|w| w.length()).collect();
        let force: Vec<Vec3> = (0..cells).map(|c| {
            if !inner[c] {
                return Vec3::ZERO;
            }
            let i = ijk(n, c);
            let mut g = Vec3::ZERO;
            for a in 0..3 {
                let (mut lo, mut hi) = (i, i);
                lo[a] -= 1;
                hi[a] += 1;
                g[a] = (mag[at(n, hi[0], hi[1], hi[2])] - mag[at(n, lo[0], lo[1], lo[2])]) / (2.0 * h);
            }
            VORT * h * (g / (g.length() + 1e-3)).cross(w[c])
        }).collect();
        for a in 0..3 {
            for f in 0..self.vel[a].len() {
                if let [Some(lo), Some(hi)] = self.sides(a, f) && self.known[a][f] && self.cell[lo] == FLUID && self.cell[hi] == FLUID {
                    self.vel[a][f] += dt * 0.5 * (force[lo][a] + force[hi][a]);
                }
            }
        }
    }

    fn div(&self, c: usize) -> f32 {
        let i = ijk(self.n, c);
        (0..3).map(|a| {
            let d = self.dims(a);
            let mut j = i;
            let lo = at(d, j[0], j[1], j[2]);
            j[a] += 1;
            self.vel[a][at(d, j[0], j[1], j[2])] - self.vel[a][lo]
        }).sum()
    }

    fn drift(&self, c: usize) -> f32 {
        let i = ijk(self.n, c);
        let (mut s, mut m, mut air) = (0.0, 0.0, false);
        for k in 0..27usize {
            let o = [k % 3, k / 3 % 3, k / 9];
            let j = [0, 1, 2].map(|a| (i[a] + o[a]).saturating_sub(1).min(self.n[a] - 1));
            let q = at(self.n, j[0], j[1], j[2]);
            match self.cell[q] {
                FLUID => (s, m) = (s + (self.start[q + 1] - self.start[q]) as f32, m + 1.0),
                AIR => air = true,
                _ => {}
            }
        }
        let e = s / m / 8.0 - 1.0;
        let e = if air { (e - KEEP_BAND).max(0.0) } else { e.signum() * (e.abs() - KEEP_BAND).max(0.0) };
        e.clamp(-0.5, 1.0)
    }

    fn theta_inv(&self, c: usize) -> f32 {
        if !self.phys.ghost {
            return 1.0;
        }
        let rho = ((self.start[c + 1] - self.start[c]) as f32 / 8.0).min(1.0);
        1.0 / ((rho - 0.5) / rho.max(1e-3)).clamp(GHOST_MIN, 1.0)
    }

    fn around(&self, c: usize) -> impl Iterator<Item = (usize, bool)> + '_ {
        let i = ijk(self.n, c);
        (0..6).filter_map(move |s| {
            let (a, up) = (s / 2, s % 2 == 1);
            let mut j = i;
            if up { j[a] += 1 } else { j[a] = j[a].checked_sub(1)? }
            (j[a] < self.n[a]).then(|| at(self.n, j[0], j[1], j[2]))
        }).filter(|&n| self.cell[n] != SOLID).map(|n| (n, self.cell[n] == FLUID))
    }

    fn project(&mut self, dt: f32) {
        let cells = self.cell.len();
        let fluid: Vec<usize> = (0..cells).filter(|&c| self.cell[c] == FLUID).collect();
        let diag: Vec<f32> = fluid.iter().map(|&c| self.around(c).map(|(_, w)| if w { 1.0 } else { self.theta_inv(c) }).sum()).collect();
        let mut slot = vec![u32::MAX; cells];
        for (k, &c) in fluid.iter().enumerate() {
            slot[c] = k as u32;
        }
        let nb: Vec<[u32; 6]> = fluid.iter().map(|&c| {
            let mut o = [u32::MAX; 6];
            for (k, (n, w)) in self.around(c).enumerate() {
                if w {
                    o[k] = slot[n];
                }
            }
            o
        }).collect();
        let m = fluid.len();
        let keep = self.phys.relax;
        let b: Vec<f32> = fluid.iter().map(|&c| if keep { KEEP * self.drift(c) * self.h / dt } else { 0.0 } - self.div(c)).collect();
        let apply = |x: &[f32], out: &mut Vec<f32>| par(out, |k, o| *o = diag[k] * x[k] - nb[k].iter().filter(|&&n| n != u32::MAX).map(|&n| x[n as usize]).sum::<f32>());
        let mut x = vec![0.0f32; m];
        let mut r = b.clone();
        let mut z: Vec<f32> = (0..m).map(|k| r[k] / diag[k]).collect();
        let mut p = z.clone();
        let mut ap = vec![0.0f32; m];
        let mut rz = sum(m, |k| (r[k] * z[k]) as f64);
        let tol = 1e-6 * sum(m, |k| (b[k] * b[k]) as f64).sqrt().max(1e-12);
        self.iters = 0;
        while self.iters < 400 && sum(m, |k| (r[k] * r[k]) as f64).sqrt() > tol {
            apply(&p, &mut ap);
            let alpha = (rz / sum(m, |k| (p[k] * ap[k]) as f64)) as f32;
            par(&mut x, |k, v| *v += alpha * p[k]);
            par(&mut r, |k, v| *v -= alpha * ap[k]);
            par(&mut z, |k, v| *v = r[k] / diag[k]);
            let rz2 = sum(m, |k| (r[k] * z[k]) as f64);
            let beta = (rz2 / rz) as f32;
            rz = rz2;
            par(&mut p, |k, v| *v = z[k] + beta * *v);
            self.iters += 1;
        }
        self.q.iter_mut().for_each(|q| *q = 0.0);
        for (k, &c) in fluid.iter().enumerate() {
            self.q[c] = x[k];
        }
        for a in 0..3 {
            for f in 0..self.vel[a].len() {
                let [Some(lo), Some(hi)] = self.sides(a, f) else { continue };
                if self.cell[lo] == SOLID || self.cell[hi] == SOLID || self.cell[lo] != FLUID && self.cell[hi] != FLUID {
                    continue;
                }
                let qe = |c: usize, o: usize| if self.cell[c] == FLUID { self.q[c] } else if self.phys.ghost { self.q[o] * (1.0 - self.theta_inv(o)) } else { 0.0 };
                let d = qe(hi, lo) - qe(lo, hi);
                self.vel[a][f] -= d;
                self.known[a][f] = true;
            }
        }
    }

    fn extrapolate(&mut self) {
        for _ in 0..2 {
            for a in 0..3 {
                let d = self.dims(a);
                let (vel, known) = (&self.vel[a], &self.known[a]);
                let mut out: Vec<Option<f32>> = vec![None; vel.len()];
                par(&mut out, |f, o| {
                    if known[f] {
                        return;
                    }
                    let i = ijk(d, f);
                    let (mut s, mut k) = (0.0, 0);
                    for b in 0..3 {
                        for up in [false, true] {
                            let mut j = i;
                            if up { j[b] += 1 } else if j[b] == 0 { continue } else { j[b] -= 1 }
                            if j[b] < d[b] {
                                let g = at(d, j[0], j[1], j[2]);
                                if known[g] {
                                    (s, k) = (s + vel[g], k + 1);
                                }
                            }
                        }
                    }
                    if k > 0 {
                        *o = Some(s / k as f32);
                    }
                });
                for (f, v) in out.into_iter().enumerate() {
                    if let Some(v) = v {
                        (self.vel[a][f], self.known[a][f]) = (v, true);
                    }
                }
            }
        }
    }

    fn sample(&self, a: usize, x: Vec3) -> (f32, Vec3) {
        let d = self.dims(a);
        let mut g = (x - self.lo) / self.h - 0.5;
        g[a] += 0.5;
        let mut base = [0usize; 3];
        let mut f = Vec3::ZERO;
        for b in 0..3 {
            let lo = g[b].floor().clamp(0.0, (d[b] - 2) as f32);
            base[b] = lo as usize;
            f[b] = (g[b] - lo).clamp(0.0, 1.0);
        }
        let (mut v, mut grad) = (0.0, Vec3::ZERO);
        for c in 0..8 {
            let o = [c & 1, c >> 1 & 1, c >> 2];
            let w = Vec3::new(if o[0] == 1 { f.x } else { 1.0 - f.x }, if o[1] == 1 { f.y } else { 1.0 - f.y }, if o[2] == 1 { f.z } else { 1.0 - f.z });
            let s = Vec3::new(if o[0] == 1 { 1.0 } else { -1.0 }, if o[1] == 1 { 1.0 } else { -1.0 }, if o[2] == 1 { 1.0 } else { -1.0 }) / self.h;
            let u = self.vel[a][at(d, base[0] + o[0], base[1] + o[1], base[2] + o[2])];
            v += w.x * w.y * w.z * u;
            grad += Vec3::new(s.x * w.y * w.z, w.x * s.y * w.z, w.x * w.y * s.z) * u;
        }
        (v, grad)
    }

    fn to_particles(&mut self) {
        let mut p = std::mem::take(&mut self.p);
        par(&mut p, |_, q| {
            for a in 0..3 {
                let (v, c) = self.sample(a, q.x);
                q.v[a] = v;
                q.c[a] = c;
            }
        });
        self.p = p;
    }

    fn advect(&mut self, dt: f32) {
        let lo = self.lo + Vec3::splat(self.h * 1.25);
        let hi = self.lo + Vec3::new(self.n[0] as f32 - 1.25, self.n[1] as f32 - 1.25, self.n[2] as f32 - 1.25) * self.h;
        par(&mut self.p, |_, q| {
            q.x += q.v * dt;
            for a in 0..3 {
                if q.x[a] < lo[a] {
                    (q.x[a], q.v[a]) = (lo[a], q.v[a].max(0.0));
                } else if q.x[a] > hi[a] {
                    (q.x[a], q.v[a]) = (hi[a], q.v[a].min(0.0));
                }
            }
        });
    }

    pub fn wet(&self) -> usize {
        self.cell.iter().filter(|&&c| c == FLUID).count()
    }
}

