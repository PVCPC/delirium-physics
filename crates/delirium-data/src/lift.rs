use crate::estimate::fneg;

fn f(w: u32) -> f32 {
    f32::from_bits(w)
}

fn fsel(t: f32, a: f32, b: f32) -> f32 {
    if t >= 0.0 { a } else { b }
}

fn fsel64(t: f64, a: f64, b: f64) -> f64 {
    if t >= 0.0 { a } else { b }
}

pub fn inverse_affine(m: [[f32; 4]; 4]) -> Option<[[f32; 3]; 4]> {
    let [a0, a1, a2] = [m[0][0], m[0][1], m[0][2]];
    let [b0, b1, b2] = [m[1][0], m[1][1], m[1][2]];
    let [c0, c1, c2] = [m[2][0], m[2][1], m[2][2]];
    let [t0, t1, t2] = [m[3][0], m[3][1], m[3][2]];
    let max2 = |p: f32, q: f32| fsel(p - q, p, q);
    let max3 = |m2: f32, r: f32| fsel64(m2 as f64 - r as f64, m2 as f64, r as f64) as f32;
    let mc = max3(max2(c0.abs(), c1.abs()), c2.abs());
    let mb = max3(max2(b0.abs(), b1.abs()), b2.abs());
    let ma = max3(max2(a0.abs(), a1.abs()), a2.abs());
    let mab = fsel(ma - mb, ma, mb);
    let big = fsel64(mab as f64 - mc as f64, mab as f64, mc as f64) as f32;
    let c00 = b1.mul_add(c2, -(b2 * c1));
    let c01 = b0.mul_add(c2, -(c0 * b2));
    let c02 = b0.mul_add(c1, -(c0 * b1));
    let det = a2.mul_add(c02, a0.mul_add(c00, fneg(a1 * c01)));
    if !(det.abs() > big * f(0x34C1_4606)) {
        return None;
    }
    let k = 1.0 / det;
    let (r00, r20, r10) = (k * c00, k * c02, fneg(k * c01));
    let r30 = fneg(t1.mul_add(r10, r20.mul_add(t2, r00 * t0)));
    let x1 = a1.mul_add(c2, -(c1 * a2));
    let y1 = c1.mul_add(a0, -(a1 * c0));
    let z1 = c2.mul_add(a0, -(a2 * c0));
    let (r01, r11, r21) = (fneg(x1 * k), k * z1, fneg(k * y1));
    let r31 = fneg(t1.mul_add(r11, t0.mul_add(r01, r21 * t2)));
    let x2 = b2.mul_add(a0, -(b0 * a2));
    let y2 = b1.mul_add(a0, -(b0 * a1));
    let z2 = b2.mul_add(a1, -(b1 * a2));
    let (r02, r22, r12) = (z2 * k, k * y2, fneg(k * x2));
    let r32 = fneg(t1.mul_add(r12, t0.mul_add(r02, r22 * t2)));
    Some([[r00, r01, r02], [r10, r11, r12], [r20, r21, r22], [r30, r31, r32]])
}

pub fn vmadd(a: f32, b: f32, c: f32) -> f32 {
    (a as f64 * b as f64 + c as f64) as f32
}

pub fn vrsq(n: f32) -> f32 {
    let mut e = crate::estimate::vrsqrtefp(n);
    if e == f32::INFINITY {
        e = 0.0;
    }
    let sq = e * e;
    let half = 0.5 * e;
    let t = (-(n as f64 * sq as f64 - 1.0)) as f32;
    vmadd(t, half, e)
}

pub fn fmod(x: f64, y: f64) -> f64 {
    const NAN: f64 = f64::from_bits(0xFFF8_0000_0000_0000);
    let sld = |v: u64, n: i64| if (n as u64) & 127 >= 64 { 0 } else { v << ((n as u64) & 127) };
    let srd = |v: u64, n: i64| if (n as u64) & 127 >= 64 { 0 } else { v >> ((n as u64) & 127) };
    let (xb, yb) = (x.to_bits(), y.to_bits());
    if (xb >> 48) & 0x7FF0 == 0x7FF0 {
        return NAN;
    }
    if (yb >> 48) & 0x7FF0 == 0x7FF0 {
        return if yb & !(1 << 63) == 0x7FF0_0000_0000_0000 { x } else { NAN };
    }
    if xb & !(1 << 63) < yb & !(1 << 63) {
        return x;
    }
    let sign = xb & (1 << 63);
    let top = 1u64 << 63;
    let (mut ye, ym) = ((yb >> 52 & 0x7FF) as i64, (yb << 11) & !top);
    let ys = if ye == 0 {
        if ym == 0 {
            return NAN;
        }
        let z = ym.leading_zeros() as i64;
        ye = 1 - z;
        sld(ym, z)
    } else {
        ym | top
    };
    let (mut xe, mut r) = ((xb >> 52 & 0x7FF) as i64, (xb << 11) & !top);
    if xe == 0 {
        if r == 0 {
            return x;
        }
        let z = r.leading_zeros() as i64;
        r = sld(r, z);
        xe = 1 - z;
    } else {
        r |= top;
    }
    let half = ys >> 1;
    let mut step = |r: &mut u64, xe: &mut i64, by: u64| {
        *r = r.wrapping_sub(by);
        let z = r.leading_zeros() as i64;
        *xe -= z;
        *r = sld(*r, z);
    };
    while xe > ye {
        let by = if r >= ys { ys } else { half };
        step(&mut r, &mut xe, by);
    }
    if xe == ye && r >= ys {
        step(&mut r, &mut xe, ys);
    }
    if r == 0 {
        return f64::from_bits(sign);
    }
    if xe <= 0 {
        r = srd(r, 1 - xe);
        xe = 0;
    }
    f64::from_bits(((xe as u64) << 52).wrapping_add((r >> 11) & ((1 << 52) - 1)).wrapping_add(sign))
}

pub fn fctiwz(x: f32) -> i32 {
    if x.is_nan() {
        i32::MIN
    } else if x >= 2147483648.0 {
        i32::MAX
    } else if x < -2147483648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

pub fn fits_class(class: u32, slot: u32, flags: u32, mission_only: bool) -> bool {
    match class {
        12 => {
            if slot == 3 {
                flags & 0x0020_0000 != 0 || flags & 0x0080_0000 != 0 && mission_only
            } else {
                matches!(slot, 2 | 4 | 8)
            }
        }
        13 => matches!(slot, 5 | 2 | 4 | 8),
        14 => matches!(slot, 2 | 4 | 8),
        _ => matches!(slot, 2 | 4),
    }
}

pub fn heading_atan(x: f64) -> f64 {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_6};
    let mut t = x.abs();
    let mut q = 0usize;
    let r = 'r: {
        if t > 1.0 {
            if t > f64::from_bits(0x7FD0_0000_0000_0000) {
                break 'r FRAC_PI_2;
            }
            q = 2;
            t = 1.0 / t;
        }
        if t > 0.2679491924311227 {
            q += 1;
            let sqrt3: f64 = 1.7320508075688772;
            t = sqrt3.mul_add(t, -1.0) / (sqrt3 + t);
        }
        let z = t * t;
        let mut b = 15.024001160028575 + z;
        let mut a = (-0.8375829936815006f64).mul_add(z, -8.494624035132068);
        b = b.mul_add(z, 59.57843614259735);
        a = a.mul_add(z, -20.505855195861653);
        b = b.mul_add(z, 86.15734959713025);
        a = a.mul_add(z, -13.688768894191927);
        let b = b.mul_add(z, 41.06630668257578);
        let mut v = a * z * t / b + t;
        if q > 1 {
            v = -v;
        }
        [0.0, FRAC_PI_6, FRAC_PI_2, FRAC_PI_3][q] + v
    };
    if x.abs().to_bits() == 0 {
        x
    } else if x >= 0.0 {
        r
    } else {
        -r
    }
}

pub fn heading(x: f32, y: f32) -> f32 {
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let at = |v: f32| heading_atan(v as f64) as f32;
    let over = |d: f32, n: f32| (-1.0 / d) * n;
    let (quarter, three, half, whole) = (f(0x3FC9_0FDB), f(0x4096_CBE4), f(0x4049_0FDB), f(0x40C9_0FDB));
    if fsel(x, x, -x) < fsel(y, y, -y) {
        if y > 0.0 {
            if x > 0.0 { quarter - at(x / y) } else { at(over(y, x)) + quarter }
        } else if x > 0.0 {
            at(over(y, x)) + three
        } else {
            three - at(x / y)
        }
    } else if y > 0.0 {
        if x > 0.0 { at(y / x) } else { half - at(over(x, y)) }
    } else if x > 0.0 {
        whole - at(over(x, y))
    } else {
        at(y / x) + half
    }
}
