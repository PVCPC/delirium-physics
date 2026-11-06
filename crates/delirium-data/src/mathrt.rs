use crate::estimate::fneg;

fn fsel(test: f64, a: f64, b: f64) -> f64 {
    if test >= 0.0 { a } else { b }
}

pub fn floor(x: f64) -> f64 {
    let whole = (x as i64) as f64;
    let r = fsel(x - whole, whole, whole - 1.0);
    let r = fsel(crate::kv!(827) - x.abs(), r, x);
    fsel(-x.abs(), x, r)
}

pub fn ceil(x: f64) -> f64 {
    let whole = (x as i64) as f64;
    let r = fsel(whole - x, whole, whole + 1.0);
    let r = fsel(crate::kv!(827) - x.abs(), r, x);
    fsel(-x.abs(), x, r)
}

#[allow(non_snake_case)]
#[inline]
fn INV_PI() -> f64 { crate::kv!(828 f64) }
#[allow(non_snake_case)]
#[inline]
fn PI_HIGH() -> f64 { crate::kv!(829 f64) }
#[allow(non_snake_case)]
#[inline]
fn PI_LOW() -> f64 { crate::kv!(830 f64) }
#[allow(non_snake_case)]
#[inline]
fn LIMIT() -> f64 { crate::kv!(831) }
const NAN: f64 = f64::from_bits(0xFFF8_0000_0000_0000);
#[allow(non_snake_case)]
#[inline]
fn SERIES() -> &'static [f64; 8] { static V: std::sync::OnceLock<[f64; 8]> = std::sync::OnceLock::new(); let f = || [
    crate::kv!(832 f64),
    crate::kv!(833 f64),
    crate::kv!(834 f64),
    crate::kv!(835 f64),
    crate::kv!(836 f64),
    crate::kv!(837 f64),
    crate::kv!(838 f64),
    crate::kv!(839 f64),
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
    let r = fneg(PI_HIGH().mul_add(turns, -ax));
    let r = fneg(PI_LOW().mul_add(turns, -r));
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
    let r = fneg(PI_HIGH().mul_add(half, -ax));
    let r = fneg(PI_LOW().mul_add(half, -r));
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
    fn PI() -> f64 { crate::kv!(840) }
    let r;
    if x == 0.0 {
        if y == 0.0 {
            if x.to_bits() >> crate::kv!(258 int) == 0 {
                return y;
            }
            return if y.to_bits() >> crate::kv!(258 int) == 0 { PI() } else { -PI() };
        }
        r = crate::kv!(841);
    } else {
        let (mut num, mut den, mut quadrant) = (y.abs(), x.abs(), 0usize);
        if num > den {
            (num, den, quadrant) = (den, num, 2);
        }
        let mut t = num / den;
        if t > crate::kv!(842) {
            quadrant += 1;
            let sqrt3: f64 = crate::kv!(843);
            t = sqrt3.mul_add(t, -1.0) / (sqrt3 + t);
        }
        let z = t * t;
        let mut p = (-crate::kv!(844 f64)).mul_add(z, -crate::kv!(845));
        let mut q = crate::kv!(846) + z;
        q = q.mul_add(z, crate::kv!(847));
        p = p.mul_add(z, -crate::kv!(848));
        q = q.mul_add(z, crate::kv!(849));
        p = p.mul_add(z, -crate::kv!(850));
        q = q.mul_add(z, crate::kv!(851));
        let mut v = p * z * t / q + t;
        if quadrant > 1 {
            v = -v;
        }
        r = [0.0, crate::kv!(852), crate::kv!(841), crate::kv!(853)][quadrant] + v;
    }
    let v = fsel(x, r, PI() - r);
    if y.to_bits() >> crate::kv!(258 int) == 0 { v } else { -v }
}

pub fn acos(x: f64) -> f64 {
    let (r, small) = arc(x.abs());
    let (lo, hi) = if small { (HALF_PI(), HALF_PI()) } else { (0.0, crate::kv!(840)) };
    if x >= 0.0 { lo - r } else { hi + r }
}

#[allow(non_snake_case)]
#[inline]
fn HALF_PI() -> f64 { crate::kv!(841) }

fn arc(a: f64) -> (f64, bool) {
    #[allow(non_snake_case)]
    #[inline]
    fn P() -> &'static [f64; 5] { static V: std::sync::OnceLock<[f64; 5]> = std::sync::OnceLock::new(); let f = || [crate::kv!(854 f64), crate::kv!(855 f64), crate::kv!(856 f64), crate::kv!(857 f64), crate::kv!(858 f64)]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
    #[allow(non_snake_case)]
    #[inline]
    fn Q() -> &'static [f64; 5] { static V: std::sync::OnceLock<[f64; 5]> = std::sync::OnceLock::new(); let f = || [crate::kv!(859 f64), crate::kv!(860 f64), crate::kv!(861 f64), crate::kv!(862 f64), crate::kv!(863 f64)]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
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

fn whole_kind(y: f64) -> u8 {
    if ((y.round_ties_even() as i64) as f64) != y {
        return 0;
    }
    let h = y * 0.5;
    if ((h.round_ties_even() as i64) as f64) == h { 2 } else { 1 }
}

fn frexp(x: f64) -> (f64, i32) {
    let field = ((x.to_bits() >> crate::kv!(385 int)) & crate::kv!(864 int)) as i32;
    if field == 0 {
        let (m, e) = frexp(x * crate::kv!(865 f64));
        return (m, e - crate::kv!(37 int));
    }
    (f64::from_bits(x.to_bits() & crate::kv!(866 int) | crate::kv!(867 int)), field - crate::kv!(868 int))
}

fn exponent(x: f64) -> i32 {
    ((x.to_bits() >> crate::kv!(385 int)) & crate::kv!(864 int)) as i32 - crate::kv!(868 int)
}

fn set_exponent(x: f64, e: i32) -> f64 {
    f64::from_bits(x.to_bits() & crate::kv!(866 int) | ((e + crate::kv!(868 int)) as u64) << crate::kv!(385 int))
}

#[allow(non_snake_case)]
#[inline]
fn POW2() -> &'static [u64; 17] { static V: std::sync::OnceLock<[u64; 17]> = std::sync::OnceLock::new(); let f = || [
    crate::kv!(869 int), crate::kv!(870 int), crate::kv!(871 int), crate::kv!(872 int),
    crate::kv!(873 int), crate::kv!(874 int), crate::kv!(875 int), crate::kv!(876 int),
    crate::kv!(877 int), crate::kv!(878 int), crate::kv!(879 int), crate::kv!(880 int),
    crate::kv!(881 int), crate::kv!(882 int), crate::kv!(883 int), crate::kv!(884 int),
    crate::kv!(867 int),
]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn CORR() -> &'static [u64; 9] { static V: std::sync::OnceLock<[u64; 9]> = std::sync::OnceLock::new(); let f = || [
    0, crate::kv!(885 int), crate::kv!(886 int), crate::kv!(887 int), crate::kv!(888 int),
    crate::kv!(889 int), crate::kv!(890 int), crate::kv!(891 int), crate::kv!(892 int),
]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn LOG2E() -> f64 { crate::kv!(893 f64) }

fn ln(x: f64) -> f64 {
    if x == 1.0 {
        return 0.0;
    }
    let (x, bias) = if x < f64::MIN_POSITIVE { (x * crate::kv!(894 f64), crate::kv!(895 int)) } else { (x, crate::kv!(868 int)) };
    let mut k = ((x.to_bits() >> crate::kv!(385 int)) & crate::kv!(864 int)) as i32 - bias;
    let m = f64::from_bits(x.to_bits() & crate::kv!(866 int) | crate::kv!(867 int));
    let (num, den);
    if m > crate::kv!(896 f64) {
        num = (m - 0.5) - 0.5;
        den = (m + 1.0) * 0.5;
    } else {
        num = m - 0.5;
        k -= 1;
        den = (num + 1.0) * 0.5;
    }
    let w = num / den;
    let z = w * w;
    let a = crate::kv!(897 f64);
    let top = fneg(z.mul_add(crate::kv!(898 f64), -a));
    let sum = (z - crate::kv!(899 f64)).mul_add(z, crate::kv!(900 f64));
    let top = top.mul_add(z, -crate::kv!(901 f64));
    let bottom = sum.mul_add(z, -crate::kv!(902 f64));
    let r = (top * z / bottom + 1.0).mul_add(w, -(k as f64 * crate::kv!(903 f64)));
    (k as f64).mul_add(crate::kv!(904 f64), r)
}

fn pow_inf(x: f64, y: f64) -> f64 {
    let ax = x.abs();
    if y == f64::INFINITY {
        return if ax > 1.0 { f64::INFINITY } else if ax < 1.0 { 0.0 } else { 1.0 };
    }
    if y == f64::NEG_INFINITY {
        return if ax > 1.0 { 0.0 } else if ax < 1.0 { f64::INFINITY } else { NAN };
    }
    if x == f64::INFINITY {
        return if y > 0.0 { f64::INFINITY } else { 0.0 };
    }
    match (y > 0.0, whole_kind(y) == 1) {
        (true, true) => f64::NEG_INFINITY,
        (true, false) => f64::INFINITY,
        (false, true) => -0.0,
        (false, false) => 0.0,
    }
}

fn pow_end(p: f64, e: i32, sign: f64) -> f64 {
    if e > crate::kv!(183 int) {
        return f64::INFINITY * sign;
    }
    if e < -crate::kv!(905 int) {
        return sign * 0.0;
    }
    set_exponent(p, e) * sign
}

pub fn pow(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        return 1.0;
    }
    if x == 0.0 {
        let kind = whole_kind(y);
        if y < 0.0 {
            return if kind == 1 { f64::INFINITY.copysign(x) } else { f64::INFINITY };
        }
        if y > 0.0 {
            return if kind == 1 { x } else { 0.0 };
        }
    }
    if x.is_nan() || y.is_nan() {
        return x + y;
    }
    if x.is_infinite() || y.is_infinite() {
        return pow_inf(x, y);
    }
    let (mut x, mut sign) = (x, 1.0);
    if x < 0.0 {
        match whole_kind(y) {
            1 => sign = -1.0,
            2 => {}
            _ => return NAN,
        }
        x = -x;
    }
    if y.abs() > crate::kv!(906) {
        if y < 0.0 {
            x = 1.0 / x;
        }
        return if x > 1.0 { f64::INFINITY * sign } else if x < 1.0 { sign * 0.0 } else { sign };
    }
    let (m, e) = frexp(x);
    if y <= crate::kv!(907) && whole_kind(x) != 0 && whole_kind(y) != 0 && y > 0.0 {
        let (mut n, mut sq, mut p) = (y as i32, m, 1.0);
        let total = e.wrapping_mul(n);
        while n != 0 {
            if n & 1 != 0 {
                p *= sq;
            }
            sq *= sq;
            n >>= 1;
        }
        let r = exponent(p).wrapping_add(total);
        if r > crate::kv!(908 int) {
            return f64::INFINITY * p * sign;
        }
        if r < -crate::kv!(909 int) {
            return p * sign * 0.0;
        }
        return pow_end(p, r, sign);
    }
    let table = (*POW2()).map(f64::from_bits);
    let mut i = if m > table[crate::kv!(23 int)] { 1 } else { crate::kv!(104 int) };
    if !(m > table[i + crate::kv!(0 int)]) {
        i += crate::kv!(1 int);
    }
    if !(m > table[i + 1]) {
        i += 2;
    }
    let p = crate::kv!(54 int) * e - i as i32;
    let (hi, lo);
    if p.abs() == 1 {
        hi = ln(x) * LOG2E();
        lo = 0.0;
    } else {
        let c = table[i];
        let s = (m - c - f64::from_bits((*CORR())[(i + 1) / 2])) / (c + m);
        let w = s * 2.0;
        let z = w * w;
        let t = z.mul_add(crate::kv!(910 f64), crate::kv!(911 f64));
        let t = t.mul_add(z, crate::kv!(912 f64));
        let t = t.mul_add(z, crate::kv!(913 f64));
        hi = p as f64 * crate::kv!(77);
        lo = (t * z * w).mul_add(LOG2E(), w * crate::kv!(914 f64)) + w;
    }
    let sixteenth = |v: f64| (v * crate::kv!(139)).round_ties_even() as i64 as f64 * crate::kv!(77);
    let yh = sixteenth(y);
    let f11 = (y - yh).mul_add(hi, lo * y);
    let f10 = sixteenth(f11);
    let f13 = yh.mul_add(hi, f10);
    let f11 = f11 - f10;
    let f12 = sixteenth(f13);
    let f13 = f13 - f12 + f11;
    let f11 = sixteenth(f13);
    let f12 = f12 + f11;
    let mut f13 = f13 - f11;
    let sixteenths = f12 * crate::kv!(139);
    if sixteenths > crate::kv!(915) {
        return f64::INFINITY * sign;
    }
    if sixteenths < -crate::kv!(916) {
        return sign * 0.0;
    }
    let mut n = sixteenths as i32;
    if f13 > 0.0 {
        n += 1;
        f13 -= crate::kv!(77);
    }
    let r30 = (n >= 0) as i32 + n / crate::kv!(54 int);
    let idx = (crate::kv!(54 int) * r30 - n) as usize;
    let mut q = f13.mul_add(crate::kv!(917 f64), crate::kv!(918 f64));
    for c in [crate::kv!(919 int), crate::kv!(920 int), crate::kv!(921 int), crate::kv!(922 int), crate::kv!(923 int)] {
        q = q.mul_add(f13, f64::from_bits(c));
    }
    let v = q.mul_add(f13, 1.0) * table[idx];
    pow_end(v, exponent(v) + r30, sign)
}

