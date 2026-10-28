pub fn closest_on_segment(a: [f32; 3], b: [f32; 3], p: [f32; 3]) -> (f32, [f32; 3]) {
    let d: [f32; 3] = std::array::from_fn(|l| b[l] - a[l]);
    let q: [f32; 3] = std::array::from_fn(|l| p[l] - a[l]);
    let num = q[0].mul_add(d[0], q[1].mul_add(d[1], q[2] * d[2]));
    let den = d[0].mul_add(d[0], d[2].mul_add(d[2], d[1] * d[1]));
    let t = num / den;
    let c = if t >= 0.0 { t } else { 0.0 };
    let t = if c as f64 - 1.0 >= 0.0 { 1.0 } else { c };
    (t, std::array::from_fn(|l| d[l].mul_add(t, a[l])))
}

pub fn in_front(c: &[[f32; 3]; 4], p: [f32; 3]) -> bool {
    let e1: [f32; 3] = std::array::from_fn(|l| c[0][l] - c[1][l]);
    let e2: [f32; 3] = std::array::from_fn(|l| c[1][l] - c[2][l]);
    let d: [f32; 3] = std::array::from_fn(|l| p[l] - c[1][l]);
    let yzx = |v: [f32; 3]| [v[1], v[2], v[0]];
    let zxy = |v: [f32; 3]| [v[2], v[0], v[1]];
    let m: [f32; 3] = std::array::from_fn(|l| zxy(e2)[l] * yzx(e1)[l]);
    let n: [f32; 3] = std::array::from_fn(|l| -(yzx(e2)[l].mul_add(zxy(e1)[l], -m[l])));
    let dot = d[2].mul_add(n[2], d[1].mul_add(n[1], d[0] * n[0]));
    !(dot < 0.0)
}

pub fn portal_distance(c: &[[f32; 3]; 4], p: [f32; 3]) -> Option<f32> {
    let ok = |t: f32| t > 1.0e-5 && t < 0.99999;
    let (t1, q1) = closest_on_segment(c[0], c[1], p);
    if !ok(t1) {
        return None;
    }
    let (t2, q2) = closest_on_segment(c[3], c[2], p);
    if !ok(t2) {
        return None;
    }
    let (t3, q3) = closest_on_segment(q1, q2, p);
    if !ok(t3) {
        return None;
    }
    let d: [f32; 3] = std::array::from_fn(|l| q3[l] - p[l]);
    let len = d[2].mul_add(d[2], d[1].mul_add(d[1], d[0] * d[0])).sqrt();
    Some(if in_front(c, p) { len } else { -len })
}
