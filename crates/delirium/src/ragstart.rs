use crate::prelude::*;

pub fn start_part(now: [Vec3; 4], prev: Option<&[Vec3; 4]>, cen: Vec3, dt: f32, e: [f32; 3], v_ref: Vec3) -> ([Vec3; 4], Vec3, Vec3) {
    let mut m = now;
    let mut r = [m[0], m[1], m[2]];
    crate::ragdoll::orthonormalise(&mut r);
    m[..3].copy_from_slice(&r);
    let (v, w) = match prev {
        Some(p) => crate::ragdoll::start_motion(p, &m, cen, 1.0 / dt, e[0], e[1], e[2], v_ref),
        None => (Vec3::ZERO, Vec3::ZERO),
    };
    (m, v, w)
}

#[derive(Clone, Copy, Debug)]
pub struct ProbePart {
    pub m: [Vec3; 4],
    pub cen: Vec3,
    pub shape: ProbeShape,
}

#[derive(Clone, Copy, Debug)]
pub enum ProbeShape {
    Capsule { len: f32, radius: f32 },
    Box(Vec3),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeHit {
    pub k: usize,
    pub at: Vec3,
    pub n: Vec3,
    pub t: f32,
    pub depth: f32,
    pub id: i32,
}

fn into(m: &[Vec3; 4], p: Vec3) -> Vec3 {
    use crate::Vd;
    let q = p - m[3];
    Vec3::new(m[0].vd(q), m[1].vd(q), m[2].vd(q))
}

fn out_of(m: &[Vec3; 4], p: Vec3) -> Vec3 {
    crate::vm4(m[0], m[1], m[2], m[3], p)
}

fn turn_out(m: &[Vec3; 4], n: Vec3) -> Vec3 {
    use crate::Vd;
    let c = |k: usize| Vec3::new(m[0][k], m[1][k], m[2][k]).vd(n);
    Vec3::new(c(0), c(1), c(2))
}

pub fn probe_parts(inst: &[Vec3; 4], parts: &[ProbePart], a: Vec3, b: Vec3) -> Option<ProbeHit> {
    let (mut sa, mut sb) = (into(inst, a), into(inst, b));
    let mut best: Option<ProbeHit> = None;
    for (k, p) in parts.iter().enumerate() {
        (sa, sb) = (into(&p.m, sa), into(&p.m, sb));
        if let Some(h) = primitive(p, sa, sb).filter(|h| h.depth > 0.0 && best.is_none_or(|b| h.depth > b.depth)) {
            best = Some(ProbeHit { k, at: out_of(&p.m, h.at), n: turn_out(&p.m, h.n), ..h });
        }
        (sa, sb) = (out_of(&p.m, sa), out_of(&p.m, sb));
    }
    best.map(|h| ProbeHit { at: out_of(inst, h.at), n: turn_out(inst, h.n), ..h })
}

fn primitive(p: &ProbePart, a: Vec3, b: Vec3) -> Option<ProbeHit> {
    let (s, d) = (a - p.cen, b - a);
    let (t, id, n) = match p.shape {
        ProbeShape::Capsule { len, radius } => {
            let (t, id) = seg_capsule(s, b - p.cen, len, radius)?;
            let mut q = d.mul_add(Vec3::splat(t), a) - p.cen;
            q.y = match id {
                2 => 0.0,
                1 => (-len).mul_add(0.5, q.y),
                _ => len.mul_add(0.5, q.y),
            };
            (t, id, crate::rigid::nrm(q))
        }
        ProbeShape::Box(size) => {
            let (t, id) = seg_box(s, d, size * 0.5)?;
            let mut n = Vec3::ZERO;
            n[id as usize / 2] = if id % 2 == 0 { 1.0 } else { -1.0 };
            (t, id, n)
        }
    };
    if n.x.mul_add(d.x, n.y.mul_add(d.y, d.z * n.z)) > 0.0 {
        return None;
    }
    let at = d.mul_add(Vec3::splat(t), a);
    let e = at - b;
    let depth = e.x.mul_add(e.x, e.z.mul_add(e.z, e.y * e.y)).sqrt();
    Some(ProbeHit { k: 0, at, n, t, depth, id })
}

fn seg_capsule(s: Vec3, e: Vec3, len: f32, r: f32) -> Option<(f32, i32)> {
    const EPS: f32 = 1e-6;
    let (r2, h, d) = (r * r, len * 0.5, e - s);
    let (mut t1, mut t2): (f32, f32);
    let (mut id1, mut id2) = (-1i32, -1i32);
    let a = d.x.mul_add(d.x, d.z * d.z);
    if a > EPS {
        let cr = s.z.mul_add(d.x, -(s.x * d.z));
        if cr * cr > r2 * a {
            return None;
        }
        let b = s.z.mul_add(d.z, s.x * d.x);
        let c = (s.z.mul_add(s.z, s.x * s.x) - r2) * a;
        let disc = b.mul_add(b, -c);
        let q = if disc > 0.0 { disc.sqrt() } else { 0.0 };
        let ia = 1.0 / a;
        (t1, t2) = ((-b - q) * ia, (q - b) * ia);
        let (y1, y2) = (t1.mul_add(d.y, s.y), t2.mul_add(d.y, s.y));
        let side = |y: f32| !(y < -h) && !(y > h);
        if side(y1) {
            id1 = 2;
        }
        let both = side(y2) && {
            id2 = 2;
            id1 == 2
        };
        if !both {
            let dd = d.y.mul_add(d.y, a);
            let idd = 1.0 / dd;
            let mut cap = |o: f32, id: i32, below: bool| {
                let bb = s.x.mul_add(d.x, o.mul_add(d.y, s.z * d.z));
                let cc = (s.x.mul_add(s.x, s.z.mul_add(s.z, o * o)) - r2) * dd;
                let disc = bb.mul_add(bb, -cc);
                if disc > 0.0 {
                    let q = disc.sqrt();
                    let out = |y: f32| if below { y < -h } else { y > h };
                    if out(y1) {
                        (id1, t1) = (id, (-bb - q) * idd);
                    }
                    if out(y2) {
                        (id2, t2) = (id, (q - bb) * idd);
                    }
                }
            };
            if y1 < -h || y2 < -h {
                cap(s.y + h, 0, true);
            }
            if y1 > h || y2 > h {
                cap(s.y - h, 1, false);
            }
        }
    } else {
        if !(d.y.abs() > EPS) {
            return None;
        }
        let rr = r2 - e.x.mul_add(e.x, e.z * e.z);
        if !(rr > 0.0) {
            return None;
        }
        let top = rr.sqrt() + h;
        let (lo, hi);
        (lo, hi, id1, id2) = if s.y > e.y { (top, -top, 1, 0) } else { (-top, top, 0, 1) };
        let k = 1.0 / d.y;
        (t1, t2) = ((lo - s.y) * k, (hi - s.y) * k);
    }
    let within = |t: f32| !(t < 0.0) && !(t > 1.0);
    if id1 != -1 && within(t1) {
        return Some(if id2 != -1 && within(t2) && t1 > t2 { (t2, id2) } else { (t1, id1) });
    }
    (id2 != -1 && within(t2)).then_some((t2, id2))
}

fn seg_box(s: Vec3, d: Vec3, ext: Vec3) -> Option<(f32, i32)> {
    let (mut t_in, mut t_out, mut f_in, mut f_out) = (-1.0f32, 2.0f32, -1i32, -1i32);
    for i in 0..3 {
        let (p, v, x, f) = (s[i], d[i], ext[i], 2 * i as i32);
        let away = !(v * p < 0.0);
        let av = v.abs();
        let m = x - p.abs();
        if m < 0.0 {
            if away || m < -av {
                return None;
            }
            let k = 1.0 / av;
            let t = -(k * m);
            if t > t_in {
                (f_in, t_in) = (if p > 0.0 { f } else { f + 1 }, t);
            }
            let t = x.mul_add(2.0, -m) * k;
            if t < t_out && t < 1.0 {
                (f_out, t_out) = (if p > 0.0 { f + 1 } else { f }, t);
            }
        } else {
            let far = if away { m } else { x.mul_add(2.0, -m) };
            if far < av && far < av * t_out {
                f_out = if p > 0.0 {
                    if away { f } else { f + 1 }
                } else if p < 0.0 {
                    if away { f + 1 } else { f }
                } else if v > 0.0 {
                    f
                } else {
                    f + 1
                };
                t_out = far / av;
            }
        }
        if t_in > t_out {
            return None;
        }
    }
    if f_in >= 0 {
        Some((t_in, f_in))
    } else {
        (f_out >= 0).then_some((t_out, f_out))
    }
}

