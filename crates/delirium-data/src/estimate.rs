const RSQRTE: [u32; 32] = [
    0x0568B4FD, 0x04F3AF97, 0x048DAAA5, 0x0435A618, 0x03E7A1E4, 0x03A29DFE, 0x03659A5C, 0x032E96F8,
    0x02FC93CA, 0x02D090CE, 0x02A88DFE, 0x02838B57, 0x026188D4, 0x02438673, 0x02268431, 0x020B820B,
    0x03D27FFA, 0x03807C29, 0x033878AA, 0x02F97572, 0x02C27279, 0x02926FB7, 0x02666D26, 0x023F6AC0,
    0x021D6881, 0x01FD6665, 0x01E16468, 0x01C76287, 0x01AF60C1, 0x01995F12, 0x01855D79, 0x01735BF4,
];

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

