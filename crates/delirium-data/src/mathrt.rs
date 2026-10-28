fn fsel(test: f64, a: f64, b: f64) -> f64 {
    if test >= 0.0 { a } else { b }
}

pub fn floor(x: f64) -> f64 {
    let whole = (x as i64) as f64;
    let r = fsel(x - whole, whole, whole - 1.0);
    let r = fsel(crate::kv!(800) - x.abs(), r, x);
    fsel(-x.abs(), x, r)
}

pub fn ceil(x: f64) -> f64 {
    let whole = (x as i64) as f64;
    let r = fsel(whole - x, whole, whole + 1.0);
    let r = fsel(crate::kv!(800) - x.abs(), r, x);
    fsel(-x.abs(), x, r)
}

#[allow(non_snake_case)]
#[inline]
fn INV_PI() -> f64 { crate::kv!(801 f64) }
#[allow(non_snake_case)]
#[inline]
fn PI_HIGH() -> f64 { crate::kv!(802 f64) }
#[allow(non_snake_case)]
#[inline]
fn PI_LOW() -> f64 { crate::kv!(803 f64) }
#[allow(non_snake_case)]
#[inline]
fn LIMIT() -> f64 { crate::kv!(804) }
const NAN: f64 = f64::from_bits(0xFFF8_0000_0000_0000);
#[allow(non_snake_case)]
#[inline]
fn SERIES() -> &'static [f64; 8] { static V: std::sync::OnceLock<[f64; 8]> = std::sync::OnceLock::new(); let f = || [
    crate::kv!(805 f64),
    crate::kv!(806 f64),
    crate::kv!(807 f64),
    crate::kv!(808 f64),
    crate::kv!(809 f64),
    crate::kv!(810 f64),
    crate::kv!(811 f64),
    crate::kv!(812 f64),
]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

fn series(r: f64) -> f64 {
    let z = r * r;
    let mut p = (*SERIES())[0];
    for &c in &(*SERIES())[1..] {
        p = p.mul_add(z, c);
    }
    p.mul_add(z, 1.0) * r
}

pub fn sin(x: f64) -> f64 {
    let ax = x.abs();
    let sign = fsel(x, 1.0, -1.0);
    let turns = (INV_PI() * ax).round_ties_even();
    let r = -(PI_HIGH().mul_add(turns, -ax));
    let r = -(PI_LOW().mul_add(turns, -r));
    let mut v = series(r);
    if (turns as i64) & 1 != 0 {
        v = -v;
    }
    if ax.to_bits() == 0 {
        return x;
    }
    fsel(ax - LIMIT(), NAN, v * sign)
}

pub fn cos(x: f64) -> f64 {
    let ax = x.abs();
    let shifted = std::f64::consts::FRAC_PI_2 + ax;
    let turns = (INV_PI() * shifted).round_ties_even();
    let half = turns - 0.5;
    let r = -(PI_HIGH().mul_add(half, -ax));
    let r = -(PI_LOW().mul_add(half, -r));
    let mut v = series(r);
    if (turns as i64) & 1 != 0 {
        v = -v;
    }
    if ax == 0.0 {
        return 1.0;
    }
    fsel(shifted - LIMIT(), NAN, v)
}

pub fn atan2(y: f64, x: f64) -> f64 {
    #[allow(non_snake_case)]
    #[inline]
    fn PI() -> f64 { crate::kv!(813) }
    let r;
    if x == 0.0 {
        if y == 0.0 {
            if x.to_bits() >> crate::kv!(253 int) == 0 {
                return y;
            }
            return if y.to_bits() >> crate::kv!(253 int) == 0 { PI() } else { -PI() };
        }
        r = crate::kv!(814);
    } else {
        let (mut num, mut den, mut quadrant) = (y.abs(), x.abs(), 0usize);
        if num > den {
            (num, den, quadrant) = (den, num, 2);
        }
        let mut t = num / den;
        if t > crate::kv!(815) {
            quadrant += 1;
            let sqrt3: f64 = crate::kv!(816);
            t = sqrt3.mul_add(t, -1.0) / (sqrt3 + t);
        }
        let z = t * t;
        let mut p = (-crate::kv!(817 f64)).mul_add(z, -crate::kv!(818));
        let mut q = crate::kv!(819) + z;
        q = q.mul_add(z, crate::kv!(820));
        p = p.mul_add(z, -crate::kv!(821));
        q = q.mul_add(z, crate::kv!(822));
        p = p.mul_add(z, -crate::kv!(823));
        q = q.mul_add(z, crate::kv!(824));
        let mut v = p * z * t / q + t;
        if quadrant > 1 {
            v = -v;
        }
        r = [0.0, crate::kv!(825), crate::kv!(814), crate::kv!(826)][quadrant] + v;
    }
    let v = fsel(x, r, PI() - r);
    if y.to_bits() >> crate::kv!(253 int) == 0 { v } else { -v }
}

pub fn acos(x: f64) -> f64 {
    let (r, small) = arc(x.abs());
    let (lo, hi) = if small { (HALF_PI(), HALF_PI()) } else { (0.0, crate::kv!(813)) };
    if x >= 0.0 { lo - r } else { hi + r }
}

#[allow(non_snake_case)]
#[inline]
fn HALF_PI() -> f64 { crate::kv!(814) }

fn arc(a: f64) -> (f64, bool) {
    #[allow(non_snake_case)]
    #[inline]
    fn P() -> &'static [f64; 5] { static V: std::sync::OnceLock<[f64; 5]> = std::sync::OnceLock::new(); let f = || [crate::kv!(827 f64), crate::kv!(828 f64), crate::kv!(829 f64), crate::kv!(830 f64), crate::kv!(831 f64)]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
    #[allow(non_snake_case)]
    #[inline]
    fn Q() -> &'static [f64; 5] { static V: std::sync::OnceLock<[f64; 5]> = std::sync::OnceLock::new(); let f = || [crate::kv!(832 f64), crate::kv!(833 f64), crate::kv!(834 f64), crate::kv!(835 f64), crate::kv!(836 f64)]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
    let (p, q) = ((*P()), (*Q()));
    let (z, s, small) = if !(a > 0.5) { (a * a, a, true) } else {
        let z = (1.0 - a) * 0.5;
        (z, -(z.sqrt() * 2.0), false)
    };
    let top = p[crate::kv!(1 int)].mul_add(z, p[crate::kv!(0 int)]).mul_add(z, p[2]).mul_add(z, p[1]).mul_add(z, p[0]);
    let bottom = (q[crate::kv!(1 int)] + z).mul_add(z, q[crate::kv!(0 int)]).mul_add(z, q[2]).mul_add(z, q[1]).mul_add(z, q[0]);
    (top * z * s / bottom + s, small)
}

pub fn asin(x: f64) -> f64 {
    if x == 0.0 {
        return x;
    }
    let (r, small) = arc(x.abs());
    let r = r + if small { 0.0 } else { HALF_PI() };
    if x >= 0.0 { r } else { -r }
}

