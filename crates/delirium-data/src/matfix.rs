use std::array::from_fn;

const HALF: f32 = 0.5;
const ONE: f32 = 1.0;
const ESTIMATE_GUARD: f32 = f32::INFINITY;

fn unit(v: [f32; 4]) -> [f32; 4] {
    let p = |l: usize| v[l] as f64 * v[l] as f64;
    let s = ((p(0) + p(2)) + p(1)) as f32;
    let est = crate::estimate::vrsqrtefp(s);
    let e = if est == ESTIMATE_GUARD { 0.0 } else { est };
    let e2 = e * e;
    let half = HALF * e;
    let t = (-s).mul_add(e2, ONE);
    let k = t.mul_add(half, e);
    from_fn(|l| v[l] * k)
}

fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let yzx = |v: [f32; 4]| [v[1], v[2], v[0], v[3]];
    let zxy = |v: [f32; 4]| [v[2], v[0], v[1], v[3]];
    let (ya, za, yb, zb) = (yzx(a), zxy(a), yzx(b), zxy(b));
    from_fn(|l| {
        let p = za[l] * yb[l];
        (-(ya[l] as f64 * zb[l] as f64 - p as f64)) as f32
    })
}

pub fn matrix_fix(m: &mut [f32; 16]) {
    let row = |k: usize| -> [f32; 4] { from_fn(|l| m[4 * k + l]) };
    let (r1, r2) = (row(1), row(2));
    let n2 = unit(r2);
    let n0 = unit(cross(n2, r1));
    let r1_new = cross(n0, n2);
    for (k, r) in [n0, r1_new, n2].iter().enumerate() {
        m[4 * k..4 * k + 4].copy_from_slice(r);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyRecord {
    pub key: u32,
    pub bone: u32,
}

fn find(records: &[KeyRecord], x: u32) -> Option<usize> {
    let x = x as i32;
    let cmp = |i: i64| -> i32 {
        let e = records[i as usize].key as i32;
        if e > x {
            -1
        } else if e < x {
            1
        } else {
            0
        }
    };
    let mut base: i64 = 0;
    let mut last: i64 = records.len() as i64 - 1;
    let mut count = records.len() as i64;
    loop {
        let half = count >> 1;
        if half == 0 {
            if count == 0 {
                return None;
            }
            return (cmp(base) == 0).then_some(base as usize);
        }
        let odd = count & 1 != 0;
        let mid = base + if odd { half } else { half - 1 };
        let c = cmp(mid);
        if c == 0 {
            return Some(mid as usize);
        }
        if c > 0 {
            base = mid + 1;
            count = half;
        } else {
            last = mid - 1;
            count = if odd { half } else { half - 1 };
        }
        if base > last {
            return None;
        }
    }
}

pub fn bone_key(records: &[KeyRecord], bones: &[u32], x: u32, out: Option<&mut u32>) -> u32 {
    let Some(i) = find(records, x) else {
        return 0;
    };
    let bone = records[i].bone;
    if let Some(o) = out {
        *o = bone;
    }
    bones.get(bone as usize).copied().unwrap_or(0)
}

