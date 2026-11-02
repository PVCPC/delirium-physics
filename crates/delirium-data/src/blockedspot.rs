pub type Rows = [[f32; 3]; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bound {
    pub radius: f32,
    pub high: [f32; 3],
    pub low: [f32; 3],
    pub centroid: [f32; 3],
}

impl Bound {
    pub fn read(d: &[u8], at: usize) -> Option<Bound> {
        let f = |o: usize| d.get(at + o..at + o + 4).map(|b| f32::from_bits(u32::from_be_bytes(b.try_into().unwrap())));
        let v = |o: usize| Some([f(o)?, f(o + 4)?, f(o + 8)?]);
        Some(Bound { radius: f(8)?, high: v(0x10)?, low: v(0x20)?, centroid: v(0x30)? })
    }

    pub fn fragment(d: &[u8]) -> Option<Bound> {
        let ptr = |at: usize| {
            let v = u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?);
            (v >> 28 == 5).then_some((v & 0x0FFF_FFFF) as usize)
        };
        Bound::read(d, ptr(ptr(0xE4)? + 0x0C)?)
    }
}

pub const VEHICLE: u32 = 8;
pub const CARS_AND_FRAGMENTS: u32 = 0xA8;

pub fn vehicle_includes(model_type: u32) -> u32 {
    if model_type == 2 { 0x6A8E } else { 0x7BFE }
}

#[derive(Clone, Copy, Debug)]
pub struct Object<'a> {
    pub state: u32,
    pub types: u32,
    pub includes: u32,
    pub entity: u32,
    pub bound: &'a Bound,
    pub matrix: Rows,
    pub last: Rows,
}

#[derive(Clone, Copy, Debug)]
pub struct Query<'a> {
    pub bound: &'a Bound,
    pub matrix: Rows,
    pub ignore: u32,
    pub types: u32,
    pub low_by: Option<[f32; 3]>,
    pub high_by: Option<[f32; 3]>,
}

pub fn entity(flags: u16, owner: u32) -> u32 {
    if flags & 0x40 != 0 { owner } else { 0 }
}

pub fn frag_last_from_owner(flags: u16, owner: u32, owner_28: u32, owner_12e4: i32) -> bool {
    flags & 0x4000 != 0 && owner != 0 && owner_28 & 0x03C0_0000 == 0x0080_0000 && owner_12e4 == 3
}

pub fn blocked<'a>(q: &Query, objects: impl IntoIterator<Item = Object<'a>>) -> bool {
    let centre = place(q.bound.centroid, &q.matrix);
    let mut objects = objects.into_iter();
    objects.any(|o| {
        if (1u32 << (o.state & 3)) & 7 == 0 || o.types & q.types == 0 || o.includes == 0 {
            return false;
        }
        let d = sub(centre, place(o.bound.centroid, &o.matrix));
        let r = q.bound.radius + o.bound.radius;
        if !(r * r > vmsum3(d, d)) {
            return false;
        }
        (q.ignore == 0 || o.entity != q.ignore) && boxes(q, &o)
    })
}

fn boxes(q: &Query, o: &Object) -> bool {
    let (b, m, i, l) = (q.bound, &q.matrix, &o.matrix, &o.last);
    let mut qh = mul(sub(b.high, b.low), 0.5);
    let mut qc = mul(add(b.low, b.high), 0.5);
    if let Some(w) = q.low_by {
        let w = mul(w, 0.5);
        qh = sub(qh, w);
        qc = add(qc, w);
    }
    if let Some(w) = q.high_by {
        let w = mul(w, 0.5);
        qh = add(qh, w);
        qc = add(qc, w);
    }
    let cw = place(qc, m);
    let ob = o.bound;
    let bh = mul(sub(ob.high, ob.low), 0.5);
    let bc = mul(add(ob.low, ob.high), 0.5);
    let r: [[f32; 3]; 3] = std::array::from_fn(|k| [vmsum3(l[k], i[0]), vmsum3(l[k], i[1]), vmsum3(l[k], i[2])]);
    let lt = sub(l[3], i[3]);
    let t = [vmsum3(lt, i[0]), vmsum3(lt, i[1]), vmsum3(lt, i[2])];
    let pl: [f32; 3] = std::array::from_fn(|j| vmsum4([bc[0], bc[1], bc[2], 1.0], [r[0][j], r[1][j], r[2][j], t[j]]));
    let e: [f32; 3] = std::array::from_fn(|j| vmsum3(bh, [r[0][j].abs(), r[1][j].abs(), r[2][j].abs()]));
    let umax: [f32; 3] = std::array::from_fn(|j| vmax(bh[j] + bc[j], e[j] + pl[j]));
    let umin: [f32; 3] = std::array::from_fn(|j| vmin(bc[j] - bh[j], pl[j] - e[j]));
    let uc = mul(add(umin, umax), 0.5);
    let uh = sub(umax, uc);
    let uw = place(uc, i);
    let dw = sub(cw, uw);
    let s: [[f32; 3]; 3] = std::array::from_fn(|k| [vmsum3(m[k], i[0]), vmsum3(m[k], i[1]), vmsum3(m[k], i[2])]);
    let d = [vmsum3(dw, i[0]), vmsum3(dw, i[1]), vmsum3(dw, i[2])];
    let ndq: [f32; 3] = std::array::from_fn(|k| 0.0 - vmsum3(s[k], d));
    let a: [[f32; 3]; 3] = std::array::from_fn(|k| std::array::from_fn(|j| s[k][j].abs() + f32::from_bits(0x3586_37BD)));
    (0..3).all(|k| vmsum3(a[k], uh) + qh[k] > ndq[k].abs())
        && (0..3).all(|j| {
            let col = |x: &[[f32; 3]; 3]| [x[0][j], x[1][j], x[2][j]];
            vmsum3(col(&a), qh) + uh[j] > vmsum3(col(&s), ndq).abs()
        })
}

fn place(p: [f32; 3], m: &Rows) -> [f32; 3] {
    std::array::from_fn(|j| vmsum4([p[0], p[1], p[2], 1.0], [m[0][j], m[1][j], m[2][j], m[3][j]]))
}

fn vmsum3(a: [f32; 3], b: [f32; 3]) -> f32 {
    let mut s = 0.0f64;
    for k in 0..3 {
        s += a[k] as f64 * b[k] as f64;
    }
    s as f32
}

fn vmsum4(a: [f32; 4], b: [f32; 4]) -> f32 {
    let mut s = 0.0f64;
    for k in 0..4 {
        s += a[k] as f64 * b[k] as f64;
    }
    s as f32
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
