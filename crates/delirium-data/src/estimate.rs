const RSQRTE: [u32; 32] = [
    0x0568B4FD, 0x04F3AF97, 0x048DAAA5, 0x0435A618, 0x03E7A1E4, 0x03A29DFE, 0x03659A5C, 0x032E96F8,
    0x02FC93CA, 0x02D090CE, 0x02A88DFE, 0x02838B57, 0x026188D4, 0x02438673, 0x02268431, 0x020B820B,
    0x03D27FFA, 0x03807C29, 0x033878AA, 0x02F97572, 0x02C27279, 0x02926FB7, 0x02666D26, 0x023F6AC0,
    0x021D6881, 0x01FD6665, 0x01E16468, 0x01C76287, 0x01AF60C1, 0x01995F12, 0x01855D79, 0x01735BF4,
];

pub fn vmsum3(p: [f64; 3]) -> f32 {
    ((p[0] + p[2]) + p[1]) as f32
}

pub fn vrsqrtefp(x: f32) -> f32 {
    let b = x.to_bits();
    let (neg, e, m) = (b >> 31 != 0, (b >> 23) & 0xFF, b & 0x7F_FFFF);
    let r = if e == 0xFF {
        if m != 0 { b | 0x40_0000 } else if neg { 0x7FC0_0000 } else { 0 }
    } else if e == 0 {
        if neg { 0xFF80_0000 } else { 0x7F80_0000 }
    } else if neg {
        0x7FC0_0000
    } else {
        let c = RSQRTE[(((e & 1) << 4) | (m >> 19)) as usize];
        let mut est = ((c << 10) & 0x3FF_FC00).wrapping_sub(((m >> 9) & 1023) * (c >> 16));
        let mut adj = 0i32;
        if est & 0x200_0000 == 0 {
            let lz = (est & 0x1FF_FFFF).leading_zeros();
            adj += 6 - lz as i32;
            est <<= lz - 6;
        }
        if est & 5 != 0 && est & 2 != 0 {
            est += 4;
        }
        let r = 0x3F80_0000u32.wrapping_add((adj as u32).wrapping_mul(0x80_0000)) | ((est >> 2) & 0x7F_FFFF);
        r.wrapping_sub(((b >> 24).wrapping_sub(63)) << 23)
    };
    f32::from_bits(r)
}

const VREFP: [[u32; 2]; 32] = [
    [0x7FF800, 0x3E1], [0x783800, 0x3A7], [0x70EA00, 0x371], [0x6A0800, 0x340], [0x638800, 0x313], [0x5D6200, 0x2EA],
    [0x579000, 0x2C4], [0x520800, 0x2A0], [0x4CC800, 0x27F], [0x47CA00, 0x261], [0x430800, 0x245], [0x3E8000, 0x22A],
    [0x3A2C00, 0x212], [0x360800, 0x1FB], [0x321400, 0x1E5], [0x2E4A00, 0x1D1], [0x2AA800, 0x1BE], [0x272C00, 0x1AC],
    [0x23D600, 0x19B], [0x209E00, 0x18B], [0x1D8800, 0x17C], [0x1A9000, 0x16E], [0x17AE00, 0x15B], [0x14F800, 0x15B],
    [0x124400, 0x143], [0x0FBE00, 0x143], [0x0D3800, 0x12D], [0x0ADE00, 0x12D], [0x088400, 0x11A], [0x065000, 0x11A],
    [0x041C00, 0x108], [0x020C00, 0x106],
];

pub fn fneg<T: Fneg>(x: T) -> T {
    x.fneg()
}

pub trait Fneg {
    fn fneg(self) -> Self;
}
impl Fneg for f32 {
    #[inline(always)]
    fn fneg(self) -> f32 {
        #[cfg(target_arch = "aarch64")]
        {
            let r: f32;
            unsafe { core::arch::asm!("fneg {0:s}, {1:s}", out(vreg) r, in(vreg) self, options(pure, nomem, nostack)) };
            r
        }
        #[cfg(not(target_arch = "aarch64"))]
        f32::from_bits(std::hint::black_box(self).to_bits() ^ 0x8000_0000)
    }
}
impl Fneg for f64 {
    #[inline(always)]
    fn fneg(self) -> f64 {
        #[cfg(target_arch = "aarch64")]
        {
            let r: f64;
            unsafe { core::arch::asm!("fneg {0:d}, {1:d}", out(vreg) r, in(vreg) self, options(pure, nomem, nostack)) };
            r
        }
        #[cfg(not(target_arch = "aarch64"))]
        f64::from_bits(std::hint::black_box(self).to_bits() ^ 0x8000_0000_0000_0000)
    }
}
impl<T: Fneg, const N: usize> Fneg for [T; N] {
    fn fneg(self) -> Self {
        self.map(Fneg::fneg)
    }
}

pub mod vmx {
    fn flush(x: f32) -> f32 {
        if x.is_subnormal() { 0.0f32.copysign(x) } else { x }
    }

    fn first_nan(xs: &[f32]) -> Option<f32> {
        xs.iter().find(|x| x.is_nan()).map(|x| f32::from_bits(x.to_bits() | 0x40_0000))
    }

    fn result(r: f32) -> f32 {
        if r.is_nan() { f32::from_bits(0x7FC0_0000) } else { flush(r) }
    }

    fn lanes<const N: usize>(a: [f32; N], b: [f32; N], op: impl Fn(f32, f32) -> f32) -> [f32; N] {
        std::array::from_fn(|i| {
            let (x, y) = (flush(a[i]), flush(b[i]));
            first_nan(&[x, y]).unwrap_or_else(|| result(op(x, y)))
        })
    }

    pub fn add<const N: usize>(a: [f32; N], b: [f32; N]) -> [f32; N] {
        lanes(a, b, |x, y| x + y)
    }

    pub fn sub<const N: usize>(a: [f32; N], b: [f32; N]) -> [f32; N] {
        lanes(a, b, |x, y| x - y)
    }

    pub fn scale<const N: usize>(a: [f32; N], s: f32) -> [f32; N] {
        lanes(a, [s; N], |x, y| x * y)
    }

    pub fn madd_lanes<const N: usize>(a: [f32; N], b: [f32; N], c: [f32; N]) -> [f32; N] {
        std::array::from_fn(|i| {
            let (x, y, z) = (flush(a[i]), flush(b[i]), flush(c[i]));
            first_nan(&[x, z, y]).unwrap_or_else(|| result(x.mul_add(y, z)))
        })
    }

    pub fn madd<const N: usize>(a: [f32; N], s: f32, c: [f32; N]) -> [f32; N] {
        madd_lanes(a, [s; N], c)
    }

    pub fn nmsub<const N: usize>(a: [f32; N], s: f32, c: [f32; N]) -> [f32; N] {
        let s = flush(s);
        std::array::from_fn(|i| {
            let (x, z) = (flush(a[i]), flush(c[i]));
            first_nan(&[x, z, s]).unwrap_or_else(|| result(super::fneg(x.mul_add(s, -z))))
        })
    }
}

pub fn ppc_nan(v: f32, a: f32, b: f32) -> f32 {
    if v.is_nan() && !a.is_nan() && !b.is_nan() { f32::from_bits(0x7FC0_0000) } else { v }
}

pub fn fmuls(a: f32, b: f32) -> f32 {
    match (a.is_nan(), b.is_nan()) {
        (true, _) => f32::from_bits(a.to_bits() | 0x40_0000),
        (_, true) => f32::from_bits(b.to_bits() | 0x40_0000),
        _ => ppc_nan(a * b, a, b),
    }
}

pub fn fdivs(a: f32, b: f32) -> f32 {
    match (a.is_nan(), b.is_nan()) {
        (true, _) => f32::from_bits(a.to_bits() | 0x40_0000),
        (_, true) => f32::from_bits(b.to_bits() | 0x40_0000),
        _ => ppc_nan(a / b, a, b),
    }
}

pub fn fsubs(a: f32, b: f32) -> f32 {
    match (a.is_nan(), b.is_nan()) {
        (true, _) => f32::from_bits(a.to_bits() | 0x40_0000),
        (_, true) => f32::from_bits(b.to_bits() | 0x40_0000),
        _ => ppc_nan(a - b, a, b),
    }
}

pub fn fadds(a: f32, b: f32) -> f32 {
    match (a.is_nan(), b.is_nan()) {
        (true, _) => f32::from_bits(a.to_bits() | 0x40_0000),
        (_, true) => f32::from_bits(b.to_bits() | 0x40_0000),
        _ => ppc_nan(a + b, a, b),
    }
}

pub fn fmadds(a: f32, c: f32, b: f32) -> f32 {
    for x in [a, b, c] {
        if x.is_nan() {
            return f32::from_bits(x.to_bits() | 0x40_0000);
        }
    }
    let r = a.mul_add(c, b);
    if r.is_nan() { f32::from_bits(0x7FC0_0000) } else { r }
}

pub fn vflush(x: f32) -> f32 {
    if x.is_subnormal() { 0.0f32.copysign(x) } else { x }
}

pub fn vmaddfp(a: f32, c: f32, b: f32) -> f32 {
    let r = a.mul_add(c, b);
    if r.is_nan() && !a.is_nan() && !c.is_nan() && !b.is_nan() {
        return f32::from_bits(0xFFC0_0000);
    }
    vflush(r)
}

pub fn fmuls_ordered(a: f32, b: f32) -> f32 {
    first_nan(&[a, b]).unwrap_or(a * b)
}

pub fn fdivs_ordered(a: f32, b: f32) -> f32 {
    if let Some(n) = first_nan(&[a, b]) {
        return n;
    }
    if a == 0.0 && b == 0.0 { f32::from_bits(0x7FC0_0000) } else { a / b }
}

fn first_nan(xs: &[f32]) -> Option<f32> {
    xs.iter().find(|v| v.is_nan()).map(|v| f32::from_bits(v.to_bits() | 0x40_0000))
}

const VECTOR_INVALID: u32 = 0xFFC0_0000;
const CONSOLE_INVALID: u32 = 0x7FC0_0000;

pub fn vmulfp_lane(a: f32, b: f32) -> f32 {
    vmulfp_with(a, b, VECTOR_INVALID)
}

pub fn vmulfp_console(a: f32, b: f32) -> f32 {
    vmulfp_with(a, b, CONSOLE_INVALID)
}

fn vmulfp_with(a: f32, b: f32, invalid: u32) -> f32 {
    let (a, b) = (vflush(a), vflush(b));
    if let Some(n) = first_nan(&[a, b]) {
        return n;
    }
    let r = a * b;
    if r.is_nan() { f32::from_bits(invalid) } else { vflush(r) }
}

pub fn vmaddfp_lane(a: f32, c: f32, b: f32) -> f32 {
    vmaddfp_with(a, c, b, VECTOR_INVALID)
}

pub fn vmaddfp_console(a: f32, c: f32, b: f32) -> f32 {
    vmaddfp_with(a, c, b, CONSOLE_INVALID)
}

fn vmaddfp_with(a: f32, c: f32, b: f32, invalid: u32) -> f32 {
    let (a, c, b) = (vflush(a), vflush(c), vflush(b));
    if let Some(n) = first_nan(&[a, c]) {
        return n;
    }
    if (a * c).is_nan() {
        return f32::from_bits(invalid);
    }
    if let Some(n) = first_nan(&[b]) {
        return n;
    }
    let r = a.mul_add(c, b);
    if r.is_nan() { f32::from_bits(invalid) } else { vflush(r) }
}

pub fn vnmsubfp_lane(a: f32, c: f32, b: f32) -> f32 {
    let (a, c, b) = (vflush(a), vflush(c), vflush(b));
    if let Some(n) = first_nan(&[a, c]) {
        return -n;
    }
    if (a * c).is_nan() {
        return f32::from_bits(0x7FC0_0000);
    }
    if let Some(n) = first_nan(&[b]) {
        return n;
    }
    let r = a.mul_add(c, -b);
    if r.is_nan() { f32::from_bits(0x7FC0_0000) } else { vflush(-r) }
}

pub fn vsubfp(a: f32, b: f32) -> f32 {
    let flush = |x: f32| if x.is_subnormal() { 0.0f32.copysign(x) } else { x };
    match (a.is_nan(), b.is_nan()) {
        (true, _) => f32::from_bits(a.to_bits() | 0x40_0000),
        (_, true) => f32::from_bits(b.to_bits() | 0x40_0000),
        _ => {
            let (a, b) = (flush(a), flush(b));
            flush(ppc_nan(a - b, a, b))
        }
    }
}

pub fn vrefp(x: f32) -> f32 {
    let b = x.to_bits();
    let (s, e, m) = (b & 0x8000_0000, (b >> 23) & 0xFF, b & 0x7F_FFFF);
    let r = if e == 0xFF {
        if m != 0 { b | 0x40_0000 } else { s }
    } else if e == 0 {
        s | 0x7F80_0000
    } else if e >= 253 {
        s
    } else {
        let [base, slope] = VREFP[(m >> 18) as usize];
        s | (253 - e) << 23 | (base - slope * ((m >> 9) & 0x1FF))
    };
    f32::from_bits(r)
}

pub fn frsqrte(x: f64) -> f64 {
    const TABLE: [u64; 16] = [241, 216, 192, 168, 152, 136, 128, 112, 96, 76, 60, 48, 32, 24, 16, 8];
    const EXP: u64 = 0x7FF0_0000_0000_0000;
    const QNAN: u64 = 0x7FF8_0000_0000_0000;
    let b = x.to_bits();
    let r = if b << 1 == 0 {
        b & 1 << 63 | EXP
    } else if !b & EXP == 0 {
        if b == EXP { 0 } else if b << 12 == 0 { QNAN } else { b | QNAN }
    } else if b >> 63 != 0 {
        QNAN
    } else {
        let (mut e, mut m) = ((b >> 52) as i32, b & 0xF_FFFF_FFFF_FFFF);
        if e == 0 {
            let lz = m.leading_zeros();
            m <<= lz - 11;
            e = 12 - lz as i32;
        }
        let i = ((m >> 49) as usize & 7 | (e as usize & 1) << 3) ^ 8;
        TABLE[i] << 44 | ((1022 - ((e - 1023) >> 1)) as u64) << 52
    };
    f64::from_bits(r)
}

