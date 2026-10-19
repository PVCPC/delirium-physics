use crate::rigid::{col, ldot, row_mul, rows_mul};
use crate::estimate::vrefp;
use crate::Vd;
use bevy_math::{DVec3, Vec3};

pub type V6 = [f64; 6];

pub fn v6(l: DVec3, a: DVec3) -> V6 {
    [l.x, l.y, l.z, a.x, a.y, a.z]
}

pub fn parts(v: V6) -> (DVec3, DVec3) {
    (DVec3::new(v[0], v[1], v[2]), DVec3::new(v[3], v[4], v[5]))
}

type M3 = [Vec3; 3];
pub type P2 = (Vec3, Vec3);

const E: M3 = [Vec3::X, Vec3::Y, Vec3::Z];
const ZERO2: P2 = (Vec3::ZERO, Vec3::ZERO);

fn yzx(v: Vec3) -> Vec3 {
    Vec3::new(v.y, v.z, v.x)
}

fn zxy(v: Vec3) -> Vec3 {
    Vec3::new(v.z, v.x, v.y)
}

fn neg(v: Vec3) -> Vec3 {
    Vec3::ZERO - v
}

fn neg3(m: &M3) -> M3 {
    m.map(neg)
}

fn add3(a: &M3, b: &M3) -> M3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn cols(m: &M3) -> M3 {
    [col(m, 0), col(m, 1), col(m, 2)]
}

fn dcols(m: &M3, v: Vec3) -> Vec3 {
    Vec3::new(v.vd(col(m, 0)), v.vd(col(m, 1)), v.vd(col(m, 2)))
}

fn padd(a: P2, b: P2) -> P2 {
    (a.0 + b.0, a.1 + b.1)
}

#[derive(Clone, Copy, Default)]
struct Sym {
    a: M3,
    b: M3,
    d: M3,
}

impl Sym {
    fn neg_add(&self, o: &Sym) -> Sym {
        Sym { a: add3(&neg3(&self.a), &o.a), b: add3(&neg3(&self.b), &o.b), d: add3(&neg3(&self.d), &o.d) }
    }

    fn add(&self, o: &Sym) -> Sym {
        Sym { a: add3(&self.a, &o.a), b: add3(&self.b, &o.b), d: add3(&self.d, &o.d) }
    }

    fn answer(&self, (f, t): P2) -> P2 {
        (dcols(&self.b, f) + ldot(&self.a, t), ldot(&self.b, t) + ldot(&self.d, f))
    }

    fn momentum(&self, (w, v): P2) -> P2 {
        (dcols(&self.b, w) + ldot(&self.a, v), ldot(&self.b, v) + ldot(&self.d, w))
    }
}

fn nms(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    Vec3::new(-a.x.mul_add(b.x, -c.x), -a.y.mul_add(b.y, -c.y), -a.z.mul_add(b.z, -c.z))
}

fn vx(a: Vec3, b: Vec3) -> Vec3 {
    nms(zxy(a), yzx(b), yzx(a) * zxy(b))
}

fn fmsub(a: f32, b: f32, c: f32) -> f32 {
    a.mul_add(b, -c)
}

fn fnmsub(a: f32, b: f32, c: f32) -> f32 {
    -a.mul_add(b, -c)
}

#[derive(Clone, Copy)]
pub struct Link {
    pub r: [Vec3; 3],
    pub x: Vec3,
    pub m: f32,
    pub i: Vec3,
}

#[derive(Clone, Copy)]
pub struct Joint {
    pub parent: usize,
    pub ca: Vec3,
    pub axis: Option<Vec3>,
    pub stiff: f32,
    pub mode: u32,
}

fn link_sym(l: &Link) -> Sym {
    let m = l.m;
    let b = E.map(|e| vx(e, l.x * m));
    let mm = l.r.map(|r| {
        let ir = l.i * r;
        Vec3::new(ir.vd(l.r[0]), ir.vd(l.r[1]), ir.vd(l.r[2]))
    });
    Sym {
        a: [Vec3::new(m, 0.0, 0.0), Vec3::new(0.0, m, 0.0), Vec3::new(0.0, 0.0, m)],
        b,
        d: [0, 1, 2].map(|i| mm[i] - row_mul(vx(E[i], l.x), &b)),
    }
}

fn inverse3(m: &M3) -> M3 {
    let (a00, a10, a11, a20, a21, a22) = (m[0].x, m[1].x, m[1].y, m[2].x, m[2].y, m[2].z);
    let f5 = fmsub(a00, a22, a20 * a20);
    let f8 = fmsub(a20, a21, a10 * a22);
    let f7 = fmsub(a10, a21, a20 * a11);
    let f6 = fmsub(a10, a20, a00 * a21);
    let f13 = a10 * f8;
    let f12 = fmsub(a11, a22, a21 * a21);
    let f9 = fmsub(a00, a11, a10 * a10);
    let det = a20.mul_add(f7, a00.mul_add(f12, f13));
    let inv = if det == 0.0 { 1e8 } else { 1.0 / det };
    [Vec3::new(inv * f12, inv * f8, inv * f7), Vec3::new(inv * f8, inv * f5, inv * f6), Vec3::new(inv * f7, inv * f6, inv * f9)]
}

fn fsel(x: f32, a: f32, b: f32) -> f32 {
    if x >= 0.0 { a } else { b }
}

fn ldl_inverse(m: &M3) -> M3 {
    let (a00, a10, a11, a20, a21, a22) = (m[0].x, m[1].x, m[1].y, m[2].x, m[2].y, m[2].z);
    let eps = ((a11 + a22) + a00) * 1e-5f32;
    let d0 = fsel(eps - a00, eps, a00);
    let i0 = 1.0 / d0;
    let (l10, l20) = (a10 * i0, a20 * i0);
    let d1r = fnmsub(a10, l10, a11);
    let c21 = fnmsub(a10, l20, a21);
    let c22 = fnmsub(a20, l20, a22);
    let d1 = fsel(eps - d1r, eps, d1r);
    let i1 = 1.0 / d1;
    let l21 = i1 * c21;
    let f7 = i1 * l10;
    let d2r = fnmsub(l21, c21, c22);
    let f4 = f7 * l10;
    let f9 = fmsub(l21, l10, l20);
    let d2 = fsel(eps - d2r, eps, d2r);
    let i2 = 1.0 / d2;
    let f11 = i2 * l21;
    let f12 = f9 * i2;
    let o12 = -f11;
    let o11 = fnmsub(o12, l21, i1);
    let o01 = -(f12 * l21) - f7;
    let o00 = f12.mul_add(f9, f4) + i0;
    [Vec3::new(o00, o01, f12), Vec3::new(o01, o11, o12), Vec3::new(f12, o12, i2)]
}

fn invert(m: &Sym) -> Sym {
    let dinv = inverse3(&m.d);
    let w = rows_mul(&cols(&m.b), &dinv);
    let x = rows_mul(&w, &m.b);
    let s = add3(&neg3(&x), &m.a);
    let sinv = ldl_inverse(&s);
    let yb = rows_mul(&sinv, &w);
    let z = [0, 1, 2].map(|i| row_mul(m.b[i], &yb) + E[i]);
    Sym { a: rows_mul(&dinv, &z), b: neg3(&yb), d: sinv }
}

#[derive(Clone, Copy, Default)]
struct Side3 {
    a: M3,
    t: M3,
    dm: M3,
    g: M3,
    h: M3,
}

fn ex(v: Vec3) -> M3 {
    E.map(|e| vx(e, v))
}

fn across3(i: &Sym, p: Vec3, s: f32, arm: Option<f32>) -> (Side3, f32) {
    let c = ex(p);
    let n = c.map(neg);
    let t = [0, 1, 2].map(|k| row_mul(n[k], &i.a) + i.b[k]);
    let u = rows_mul(&t, &c);
    let dm = [0, 1, 2].map(|k| (u[k] - Vec3::new(c[k].vd(i.b[0]), c[k].vd(i.b[1]), c[k].vd(i.b[2]))) + i.d[k]);
    let mut side = Side3 { a: i.a, t, dm, g: [Vec3::ZERO; 3], h: [Vec3::ZERO; 3] };
    if s == 1.0 {
        return (side, arm.unwrap_or(f32::MAX));
    }
    let tr = (dm[2].z + dm[1].y) + dm[0].x;
    let arm = arm.unwrap_or(((tr / (1.0 - s)) * s) * (1.0f32 / 3.0));
    let mut reg = dm;
    reg[0].x += arm;
    reg[1].y += arm;
    reg[2].z += arm;
    let inv = inverse3(&reg);
    side.h = rows_mul(&inv, &dm);
    side.g = rows_mul(&inv, &t);
    (side, arm)
}

fn free_pass(p: Vec3, a: M3) -> Sym {
    let c = ex(p);
    let b = rows_mul(&c, &a);
    let d = neg3(&rows_mul(&b, &c));
    Sym { a, b, d }
}

fn pass3(side: &Side3, p: Vec3, s: f32, mode: u32) -> Sym {
    let a = add3(&neg3(&rows_mul(&cols(&side.t), &side.g)), &side.a);
    if s == 0.0 && mode != 1 && mode != 3 {
        return free_pass(p, a);
    }
    let k = ex(neg(p));
    let y = rows_mul(&neg3(&k), &a);
    let w = rows_mul(&side.dm, &side.g);
    let z = [0, 1, 2].map(|i| neg(w[i]) + side.t[i]);
    let b = add3(&y, &z);
    let e = rows_mul(&b, &k);
    let f = [0, 1, 2].map(|i| Vec3::new(k[i].vd(z[0]), k[i].vd(z[1]), k[i].vd(z[2])));
    let q = rows_mul(&side.dm, &side.h);
    let d = [0, 1, 2].map(|i| (e[i] - f[i]) + (neg(q[i]) + side.dm[i]));
    Sym { a, b, d }
}

#[derive(Clone, Copy, Default)]
struct Side1 {
    v0: Vec3,
    v1: Vec3,
    c: f32,
    dd: f32,
}

fn newton(x: f32) -> f32 {
    let r = vrefp(x);
    r.mul_add((-r).mul_add(x, 1.0), r)
}

fn across1(i: &Sym, a: Vec3, n: Vec3, s: f32, child: Option<f32>) -> Side1 {
    let v0 = dcols(&i.b, a) + ldot(&i.a, n);
    let v1 = ldot(&i.b, n) + ldot(&i.d, a);
    let dd = v1.vd(a) + v0.vd(n);
    let one = 1.0 - s;
    let c = match child {
        None => one * if dd == 0.0 { f32::MAX } else { newton(dd) },
        Some(dc) => one * newton(one.mul_add(dd, s * dc)),
    };
    Side1 { v0, v1, c, dd }
}

fn pass1(side: &Side1, i: &Sym) -> Sym {
    let u = side.v0 * side.c;
    let w = side.v1 * side.c;
    let row = |v: Vec3, u: Vec3| [Vec3::splat(v.x) * u, Vec3::splat(v.y) * u, Vec3::splat(v.z) * u];
    Sym { a: row(side.v0, u), b: row(side.v1, u), d: row(side.v1, w) }.neg_add(i)
}

#[derive(Clone, Copy)]
enum Dof {
    Ball { child: Side3, rest: Side3, mode: u32 },
    Hinge { a: Vec3, n: Vec3, child: Side1, rest: Side1 },
}

#[derive(Clone, Copy)]
struct Cache {
    p: Vec3,
    dof: Dof,
}

fn project3(side: &Side3, p: Vec3, (x0, x1): P2) -> P2 {
    let tau = nms(yzx(p), zxy(x0), zxy(p).mul_add(yzx(x0), x1));
    let y = ldot(&side.g, tau) + ldot(&side.h, x0);
    let o0 = neg(y) + x0;
    (o0, nms(yzx(o0), zxy(p), zxy(o0).mul_add(yzx(p), tau)))
}

fn bias3(side: &Side3, p: Vec3, (x0, x1): P2) -> P2 {
    let tau = nms(yzx(p), zxy(x0), zxy(p).mul_add(yzx(x0), x1));
    let u = neg(dcols(&side.h, tau)) + tau;
    let o0 = neg(dcols(&side.g, tau)) + x0;
    (o0, nms(yzx(o0), zxy(p), zxy(o0).mul_add(yzx(p), u)))
}

fn project1(side: &Side1, a: Vec3, n: Vec3, (x0, x1): P2, prior: Option<P2>) -> P2 {
    let q = x1.vd(side.v0) + x0.vd(side.v1);
    let y = neg(Vec3::splat(q * side.c));
    let (b0, b1) = prior.map_or((x0, x1), |(p0, p1)| (p0 + x0, p1 + x1));
    (a.mul_add(y, b0), n.mul_add(y, b1))
}

fn bias1(side: &Side1, a: Vec3, n: Vec3, (x0, x1): P2, prior: Option<P2>) -> P2 {
    let q = x1.vd(a) + x0.vd(n);
    let y = neg(Vec3::splat(q * side.c));
    let (b0, b1) = prior.map_or((x0, x1), |(p0, p1)| (p0 + x0, p1 + x1));
    (side.v0.mul_add(y, b0), side.v1.mul_add(y, b1))
}

impl Cache {
    fn rest(&self, x: P2) -> P2 {
        match &self.dof {
            Dof::Ball { rest, .. } => project3(rest, self.p, x),
            Dof::Hinge { a, n, rest, .. } => project1(rest, *a, *n, x, None),
        }
    }

    fn child(&self, x: P2) -> P2 {
        match &self.dof {
            Dof::Ball { child, .. } => project3(child, self.p, x),
            Dof::Hinge { a, n, child, .. } => project1(child, *a, *n, x, None),
        }
    }

    fn child_add(&self, x: P2, out: P2) -> P2 {
        match &self.dof {
            Dof::Ball { child, .. } => padd(out, project3(child, self.p, x)),
            Dof::Hinge { a, n, child, .. } => project1(child, *a, *n, x, Some(out)),
        }
    }

    fn bias_child(&self, x: P2) -> P2 {
        match &self.dof {
            Dof::Ball { child, .. } => bias3(child, self.p, x),
            Dof::Hinge { a, n, child, .. } => bias1(child, *a, *n, x, None),
        }
    }

    fn bias_rest_add(&self, x: P2, out: P2) -> P2 {
        match &self.dof {
            Dof::Ball { rest, .. } => padd(out, bias3(rest, self.p, x)),
            Dof::Hinge { a, n, rest, .. } => bias1(rest, *a, *n, x, Some(out)),
        }
    }
}

pub struct Artic {
    parent: Vec<Option<usize>>,
    order: Vec<usize>,
    link: Vec<Sym>,
    ic: Vec<Sym>,
    ir: Vec<Sym>,
    #[cfg_attr(not(test), allow(dead_code))]
    art: Vec<Sym>,
    y: Vec<Sym>,
    joint: Vec<Option<Cache>>,
    pub arm: Vec<f32>,
}

impl Artic {
    pub fn new(links: &[Link], joints: &[Option<Joint>], order: &[usize]) -> Artic {
        let n = links.len();
        let link: Vec<Sym> = links.iter().map(link_sym).collect();
        let mut art = link.clone();
        let (mut ic, mut ir) = (vec![Sym::default(); n], vec![Sym::default(); n]);
        let mut arm = vec![0.0; n];
        let mut cache: Vec<Option<Cache>> = vec![None; n];
        let mut pass = vec![Sym::default(); n];
        for &k in order.iter().rev() {
            let Some(j) = &joints[k] else { continue };
            ic[k] = art[k];
            let p = ldot(&links[k].r, j.ca) + links[k].x;
            let (dof, ps) = match j.axis {
                None => {
                    let (child, a) = across3(&art[k], p, j.stiff, None);
                    arm[k] = a;
                    (Dof::Ball { child, rest: Side3::default(), mode: j.mode }, pass3(&child, p, j.stiff, j.mode))
                }
                Some(a) => {
                    let child = across1(&art[k], a, vx(p, a), j.stiff, None);
                    arm[k] = if j.stiff == 1.0 { f32::MAX } else { (j.stiff * child.dd) / (1.0 - j.stiff) };
                    (Dof::Hinge { a, n: vx(p, a), child, rest: Side1::default() }, pass1(&child, &art[k]))
                }
            };
            cache[k] = Some(Cache { p, dof });
            pass[k] = ps;
            art[j.parent] = art[j.parent].add(&ps);
        }
        for &k in order {
            let Some(j) = &joints[k] else { continue };
            ir[k] = pass[k].neg_add(&art[j.parent]);
            let c = cache[k].as_mut().unwrap();
            let out = match &mut c.dof {
                Dof::Ball { rest, mode, .. } => {
                    *rest = across3(&ir[k], c.p, j.stiff, Some(arm[k])).0;
                    pass3(rest, c.p, j.stiff, *mode)
                }
                Dof::Hinge { a, n, child, rest } => {
                    *rest = across1(&ir[k], *a, *n, j.stiff, Some(child.dd));
                    pass1(rest, &ir[k])
                }
            };
            art[k] = art[k].add(&out);
        }
        let y = art.iter().map(invert).collect();
        Artic { parent: joints.iter().map(|j| j.map(|j| j.parent)).collect(), order: order.to_vec(), link, ic, ir, art, y, joint: cache, arm }
    }

    fn own_f(&self, k: usize, x: P2) -> P2 {
        self.y[k].answer(x)
    }

    pub fn own(&self, k: usize, x: V6) -> V6 {
        let (f, t) = parts(x);
        let (w, v) = self.own_f(k, (f.as_vec3(), t.as_vec3()));
        v6(v.as_dvec3(), w.as_dvec3())
    }

    pub fn solve_f(&self, imp: &[P2]) -> Vec<P2> {
        let mut d: Vec<[P2; 1]> = imp.iter().map(|&x| [x]).collect();
        self.solve_in(&mut d, &mut vec![[ZERO2]; imp.len()]);
        d.into_iter().map(|[x]| x).collect()
    }

    fn solve_in<const M: usize>(&self, d: &mut [[P2; M]], up: &mut [[P2; M]]) {
        for (k, x) in d.iter_mut().enumerate() {
            for x in x {
                *x = if *x == ZERO2 { ZERO2 } else { self.own_f(k, *x) };
            }
        }
        for &k in self.order.iter().rev() {
            let (Some(p), Some(c)) = (self.parent[k], &self.joint[k]) else { continue };
            for r in 0..M {
                if d[k][r] == ZERO2 { continue }
                up[k][r] = c.rest(d[k][r]);
                d[p][r] = padd(d[p][r], up[k][r]);
            }
        }
        for &k in &self.order {
            let (Some(p), Some(c)) = (self.parent[k], &self.joint[k]) else { continue };
            for r in 0..M {
                let o = c.child((d[p][r].0 - up[k][r].0, d[p][r].1 - up[k][r].1));
                d[k][r] = padd(d[k][r], o);
            }
        }
    }

    pub fn solve_row(&self, j: &[f64]) -> Vec<f64> {
        let [r] = self.solve_rows([j]);
        r
    }

    pub fn solve_rows<const M: usize>(&self, j: [&[f64]; M]) -> [Vec<f64>; M] {
        let (mut d, mut up) = ([[ZERO2; M]; 32], [[ZERO2; M]; 32]);
        let mut heap;
        let n = j[0].len() / 6;
        let (d, up) = if n <= 32 { (&mut d[..n], &mut up[..n]) } else { heap = (vec![[ZERO2; M]; n], vec![[ZERO2; M]; n]); (&mut heap.0[..], &mut heap.1[..]) };
        for (k, x) in d.iter_mut().enumerate() {
            for (x, j) in x.iter_mut().zip(j) {
                let c = &j[6 * k..6 * k + 6];
                *x = (Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32), Vec3::new(c[3] as f32, c[4] as f32, c[5] as f32));
            }
        }
        self.solve_in(d, up);
        std::array::from_fn(|r| d.iter().flat_map(|x| { let (w, v) = x[r]; [v.x, v.y, v.z, w.x, w.y, w.z].map(|x| x as f64) }).collect())
    }

    pub fn solve(&self, imp: &[V6]) -> Vec<V6> {
        let imp: Vec<P2> = imp.iter().map(|&x| { let (f, t) = parts(x); (f.as_vec3(), t.as_vec3()) }).collect();
        self.solve_f(&imp).into_iter().map(|(w, v)| v6(v.as_dvec3(), w.as_dvec3())).collect()
    }

    pub fn bias(&self, vel: &[P2]) -> Vec<P2> {
        let mut f: Vec<P2> = vel.iter().enumerate().map(|(k, &(w, v))| {
            let (h0, h1) = self.link[k].momentum((w, v));
            (vx(w, h0), nms(yzx(h1), zxy(w), zxy(h1).mul_add(yzx(w), vx(v, h0))))
        }).collect();
        let rel = |k: usize, p: usize| {
            let ((wp, vp), (wk, vk)) = (vel[p], vel[k]);
            (vx(wp, wk), nms(yzx(vk), zxy(wp), zxy(vk).mul_add(yzx(wp), vx(vp, wk))))
        };
        let mut up = vec![ZERO2; vel.len()];
        for &k in self.order.iter().rev() {
            let (Some(p), Some(c)) = (self.parent[k], &self.joint[k]) else { continue };
            up[k] = c.bias_child(padd(self.ic[k].momentum(rel(k, p)), f[k]));
            f[p] = padd(f[p], up[k]);
        }
        for &k in &self.order {
            let (Some(p), Some(c)) = (self.parent[k], &self.joint[k]) else { continue };
            let h = self.ir[k].momentum(rel(k, p));
            let x = ((neg(h.0) + f[p].0) - up[k].0, (neg(h.1) + f[p].1) - up[k].1);
            f[k] = c.bias_rest_add(x, f[k]);
        }
        f
    }

    pub fn velocities(&self, root: usize, x: &[Vec3], v: &[Vec3], w: &[Vec3], acc: &[P2], imp: &[P2], dt: f32, forces: bool) -> Vec<P2> {
        let n = x.len();
        let mut vel: Vec<P2> = (0..n).map(|k| (w[k], (v[k] - v[root]) - vx(w[k], x[k]))).collect();
        if !forces {
            let bias = self.bias(&vel);
            for (k, vk) in vel.iter_mut().enumerate() {
                let (o0, o1) = self.own_f(k, bias[k]);
                *vk = (vk.0 + o0 * -dt, vk.1 + o1 * -dt);
            }
        }
        let imp: Vec<P2> = (0..n).map(|k| {
            let i = if forces { ZERO2 } else { imp[k] };
            (acc[k].0.mul_add(Vec3::splat(dt), i.0), acc[k].1.mul_add(Vec3::splat(dt), i.1))
        }).collect();
        let d = self.solve_f(&imp);
        (0..n).map(|k| {
            let (w, l) = padd(vel[k], d[k]);
            (v[root] + nms(yzx(x[k]), zxy(w), zxy(x[k]).mul_add(yzx(w), l)), w)
        }).collect()
    }

    pub fn answer(&self, k: usize, t: Vec3) -> Vec3 {
        let (Some(p), Some(c)) = (self.parent[k], &self.joint[k]) else { return self.own_f(k, (Vec3::ZERO, t)).0 };
        let (yc, yp) = (&self.y[k], &self.y[p]);
        let cx = (ldot(&yc.a, t), ldot(&yc.b, t));
        let np = (neg(ldot(&yp.a, t)), neg(ldot(&yp.b, t)));
        let o1 = c.rest(cx);
        let o2 = c.child_add(np, cx);
        o2.0 - (np.0 + o1.0)
    }
}

