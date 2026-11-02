use crate::blockedspot::{Bound, Rows};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extents {
    pub bound: Bound,
    pub reach: f32,
    pub off_centre: bool,
}

pub fn extents(kids: &[Option<[[f32; 3]; 2]>], cur: &[Rows], last: &[Rows]) -> Extents {
    let (mut mn, mut mx) = ([f32::MAX; 3], [-f32::MAX; 3]);
    let mut any = false;
    for i in (0..kids.len()).rev() {
        let Some([low, high]) = kids[i] else { continue };
        any = true;
        let (c, l) = (&cur[i], &last[i]);
        let d = sub(l[3], c[3]);
        let r: [[f32; 3]; 3] = std::array::from_fn(|a| std::array::from_fn(|k| vmsum3(l[a], c[k])));
        let t: [f32; 3] = std::array::from_fn(|k| vmsum3(d, c[k]));
        let mid = mul(add(low, high), 0.5);
        let half = sub(high, mid);
        let p: [f32; 3] = std::array::from_fn(|j| vmsum4([mid[0], mid[1], mid[2], 1.0], [r[0][j], r[1][j], r[2][j], t[j]]));
        let e: [f32; 3] = std::array::from_fn(|j| vmsum3(half, [r[0][j].abs(), r[1][j].abs(), r[2][j].abs()]));
        let hi: [f32; 3] = std::array::from_fn(|j| vmax(half[j] + mid[j], e[j] + p[j]));
        let lo: [f32; 3] = std::array::from_fn(|j| vmin(mid[j] - half[j], p[j] - e[j]));
        let mid = mul(add(lo, hi), 0.5);
        let half = sub(hi, mid);
        let w: [f32; 3] = std::array::from_fn(|j| vmsum4([mid[0], mid[1], mid[2], 1.0], [c[0][j], c[1][j], c[2][j], c[3][j]]));
        let e: [f32; 3] = std::array::from_fn(|j| vmsum3(half, [c[0][j].abs(), c[1][j].abs(), c[2][j].abs()]));
        for j in 0..3 {
            mn[j] = vmin(mn[j], w[j] - e[j]);
            mx[j] = vmax(mx[j], e[j] + w[j]);
        }
    }
    if !any {
        (mn, mx) = ([0.0; 3], [0.0; 3]);
    }
    let len2 = |v: [f32; 3]| v[0].mul_add(v[0], v[1].mul_add(v[1], v[2] * v[2]));
    let (lo2, hi2) = (len2(mn), len2(mx));
    let reach = if hi2 - lo2 >= 0.0 { hi2 } else { lo2 }.sqrt();
    let centroid = mul(add(mx, mn), 0.5);
    let off_centre = centroid.iter().any(|&v| v != 0.0);
    let o = sub(mx, centroid);
    let radius = o[2].mul_add(o[2], o[1].mul_add(o[1], o[0] * o[0])).sqrt();
    Extents { bound: Bound { radius, high: mx, low: mn, centroid }, reach, off_centre }
}

fn vmsum3(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0.0f64 + a[0] as f64 * b[0] as f64 + a[1] as f64 * b[1] as f64 + a[2] as f64 * b[2] as f64) as f32
}

fn vmsum4(a: [f32; 4], b: [f32; 4]) -> f32 {
    (0.0f64 + a[0] as f64 * b[0] as f64 + a[1] as f64 * b[1] as f64 + a[2] as f64 * b[2] as f64 + a[3] as f64 * b[3] as f64) as f32
}

fn vmax(p: f32, q: f32) -> f32 {
    if p.is_nan() || q.is_nan() {
        return if p.is_nan() { p } else { q };
    }
    if p == q { f32::from_bits(p.to_bits() & q.to_bits()) } else if p > q { p } else { q }
}

fn vmin(p: f32, q: f32) -> f32 {
    if p.is_nan() || q.is_nan() {
        return if p.is_nan() { p } else { q };
    }
    if p == q { f32::from_bits(p.to_bits() | q.to_bits()) } else if p < q { p } else { q }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}
