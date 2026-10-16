fn fsel(test: f64, a: f64, b: f64) -> f64 {
    if test >= 0.0 { a } else { b }
}

pub fn floor(x: f64) -> f64 {
    let whole = (x as i64) as f64;
    let r = fsel(x - whole, whole, whole - 1.0);
    let r = fsel(1e18 - x.abs(), r, x);
    fsel(-x.abs(), x, r)
}

pub fn ceil(x: f64) -> f64 {
    let whole = (x as i64) as f64;
    let r = fsel(whole - x, whole, whole + 1.0);
    let r = fsel(1e18 - x.abs(), r, x);
    fsel(-x.abs(), x, r)
}

const INV_PI: f64 = f64::from_bits(0x3FD4_5F30_6DC9_C883);
const PI_HIGH: f64 = f64::from_bits(0x4009_21FB_5440_0000);
const PI_LOW: f64 = f64::from_bits(0x3DE0_B461_1A60_0000);
const LIMIT: f64 = 220_000_000.0;
const NAN: f64 = f64::from_bits(0xFFF8_0000_0000_0000);
const SERIES: [u64; 8] = [
    0x3CE8_80FF_6993_DF95,
    0xBD6A_E420_DC08_499C,
    0x3DE6_123C_686A_D430,
    0xBE5A_E645_4B5D_C0AB,
    0x3EC7_1DE3_A524_F063,
    0xBF2A_01A0_1A01_3E1A,
    0x3F81_1111_1111_10B0,
    0xBFC5_5555_5555_5555,
];

fn series(r: f64) -> f64 {
    let z = r * r;
    let mut p = f64::from_bits(SERIES[0]);
    for &c in &SERIES[1..] {
        p = p.mul_add(z, f64::from_bits(c));
    }
    p.mul_add(z, 1.0) * r
}

pub fn sin(x: f64) -> f64 {
    let ax = x.abs();
    let sign = fsel(x, 1.0, -1.0);
    let turns = (INV_PI * ax).round_ties_even();
    let r = -(PI_HIGH.mul_add(turns, -ax));
    let r = -(PI_LOW.mul_add(turns, -r));
    let mut v = series(r);
    if (turns as i64) & 1 != 0 {
        v = -v;
    }
    if ax.to_bits() == 0 {
        return x;
    }
    fsel(ax - LIMIT, NAN, v * sign)
}

pub fn cos(x: f64) -> f64 {
    let ax = x.abs();
    let shifted = std::f64::consts::FRAC_PI_2 + ax;
    let turns = (INV_PI * shifted).round_ties_even();
    let half = turns - 0.5;
    let r = -(PI_HIGH.mul_add(half, -ax));
    let r = -(PI_LOW.mul_add(half, -r));
    let mut v = series(r);
    if (turns as i64) & 1 != 0 {
        v = -v;
    }
    if ax == 0.0 {
        return 1.0;
    }
    fsel(shifted - LIMIT, NAN, v)
}

pub fn atan2(y: f64, x: f64) -> f64 {
    const PI: f64 = 3.141592653589793;
    let r;
    if x == 0.0 {
        if y == 0.0 {
            if x.to_bits() >> 63 == 0 {
                return y;
            }
            return if y.to_bits() >> 63 == 0 { PI } else { -PI };
        }
        r = 1.5707963267948966;
    } else {
        let (mut num, mut den, mut quadrant) = (y.abs(), x.abs(), 0usize);
        if num > den {
            (num, den, quadrant) = (den, num, 2);
        }
        let mut t = num / den;
        if t > 0.2679491924311227 {
            quadrant += 1;
            let sqrt3: f64 = 1.7320508075688772;
            t = sqrt3.mul_add(t, -1.0) / (sqrt3 + t);
        }
        let z = t * t;
        let mut p = (-0.8375829936815006f64).mul_add(z, -8.494624035132068);
        let mut q = 15.024001160028575 + z;
        q = q.mul_add(z, 59.57843614259735);
        p = p.mul_add(z, -20.505855195861653);
        q = q.mul_add(z, 86.15734959713025);
        p = p.mul_add(z, -13.688768894191927);
        q = q.mul_add(z, 41.06630668257578);
        let mut v = p * z * t / q + t;
        if quadrant > 1 {
            v = -v;
        }
        r = [0.0, 0.5235987755982989, 1.5707963267948966, 1.0471975511965979][quadrant] + v;
    }
    let v = fsel(x, r, PI - r);
    if y.to_bits() >> 63 == 0 { v } else { -v }
}

pub fn acos(x: f64) -> f64 {
    let (r, small) = arc(x.abs());
    let (lo, hi) = if small { (HALF_PI, HALF_PI) } else { (0.0, 3.141592653589793) };
    if x >= 0.0 { lo - r } else { hi + r }
}

const HALF_PI: f64 = 1.5707963267948966;

fn arc(a: f64) -> (f64, bool) {
    const P: [u64; 5] = [0xC03B5E55A83A0A62, 0x404C9AA7360AD48A, 0xC043D82CA9A6DA9F, 0x40244E1764EC3927, 0xBFE64BBDB5E61E65];
    const Q: [u64; 5] = [0xC06486C03E2B87CC, 0x407A124F101EB843, 0xC077DDCEFC56A848, 0x4062DE7C96591C70, 0xC037D2E86EF9861F];
    let (p, q) = (P.map(f64::from_bits), Q.map(f64::from_bits));
    let (z, s, small) = if !(a > 0.5) { (a * a, a, true) } else {
        let z = (1.0 - a) * 0.5;
        (z, -(z.sqrt() * 2.0), false)
    };
    let top = p[4].mul_add(z, p[3]).mul_add(z, p[2]).mul_add(z, p[1]).mul_add(z, p[0]);
    let bottom = (q[4] + z).mul_add(z, q[3]).mul_add(z, q[2]).mul_add(z, q[1]).mul_add(z, q[0]);
    (top * z * s / bottom + s, small)
}

pub fn asin(x: f64) -> f64 {
    if x == 0.0 {
        return x;
    }
    let (r, small) = arc(x.abs());
    let r = r + if small { 0.0 } else { HALF_PI };
    if x >= 0.0 { r } else { -r }
}

