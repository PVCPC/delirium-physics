#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int { bits: u64, size: u8 },
    Float(f32),
    Double(f64),
    Bytes(Vec<u8>),
    Chars(Vec<u8>),
    Wide(Vec<u16>),
    WideChars(Vec<u16>),
    Untouched,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scanned {
    pub count: i32,
    pub values: Vec<Value>,
}

impl Scanned {
    pub fn int(&self, k: usize) -> Option<i32> {
        match self.values.get(k)? {
            Value::Int { bits, .. } => Some(*bits as u32 as i32),
            _ => None,
        }
    }
    pub fn float(&self, k: usize) -> Option<f32> {
        match self.values.get(k)? {
            Value::Float(v) => Some(*v),
            _ => None,
        }
    }
    pub fn bytes(&self, k: usize) -> Option<&[u8]> {
        match self.values.get(k)? {
            Value::Bytes(v) => Some(v),
            _ => None,
        }
    }
    pub fn text(&self, k: usize) -> Option<String> {
        self.bytes(k).map(|b| String::from_utf8_lossy(b).into_owned())
    }
}

const EOF: i32 = -1;

fn is_space(c: i32) -> bool {
    (9..=13).contains(&c) || c == 32
}

fn is_digit(c: i32) -> bool {
    (0x30..=0x39).contains(&c)
}

fn is_xdigit(c: i32) -> bool {
    is_digit(c) || (0x41..=0x46).contains(&c) || (0x61..=0x66).contains(&c)
}

struct Input<'a> {
    memory: &'a [u8],
    at: usize,
    count: u32,
}

impl Input<'_> {
    fn inc(&mut self) -> i32 {
        let was = self.count;
        self.count = was.wrapping_sub(1);
        if (was as u64).wrapping_sub(1) as i64 >= 0 {
            let c = self.memory.get(self.at).copied().unwrap_or(0);
            self.at += 1;
            c as i32
        } else {
            EOF
        }
    }
    fn un_inc(&mut self, c: i32) {
        if c == EOF {
            return;
        }
        let mut at = self.at;
        if at == 0 {
            if self.count != 0 {
                return;
            }
            at += 1;
        }
        if self.memory.get(at - 1).copied().unwrap_or(0) != c as u8 {
            return;
        }
        self.at = at - 1;
        self.count = self.count.wrapping_add(1);
    }
    fn eat_white(&mut self) -> i32 {
        loop {
            let c = self.inc();
            if !is_space(c) {
                return c;
            }
        }
    }
}

pub fn sscanf(memory: &[u8], format: &[u8]) -> Scanned {
    let len = memory.iter().position(|&c| c == 0).unwrap_or(memory.len());
    let format = &format[..format.iter().position(|&c| c == 0).unwrap_or(format.len())];
    let f = |i: usize| format.get(i).copied().unwrap_or(0) as i32;
    let mut s = Input { memory, at: 0, count: len.min(0x7FFF_FFFF) as u32 };
    let mut values = vec![];
    let mut assigned = 0;
    let mut matched = 0;
    let mut ch: i32 = 0;
    let mut p = 0;
    let done = |ch: i32, assigned: i32, matched: i32, values: Vec<Value>| Scanned {
        count: if ch == EOF && assigned == 0 && matched == 0 { -1 } else { assigned },
        values,
    };
    while f(p) != 0 {
        if is_space(f(p)) {
            let c = s.eat_white();
            s.un_inc(c);
            p += 1;
            while is_space(f(p)) {
                p += 1;
            }
            continue;
        }
        if f(p) != b'%' as i32 {
            ch = s.inc();
            if f(p) != ch {
                s.un_inc(ch);
                return done(ch, assigned, matched, values);
            }
            p += 1;
        } else {
            let (mut width, mut widthset, mut suppress, mut widechar, mut longone, mut integer64) = (0i32, 0, 0, 0i32, 1i32, 0);
            loop {
                p += 1;
                let c = f(p);
                if is_digit(c) {
                    widthset += 1;
                    width = width.wrapping_mul(10).wrapping_add(c - 0x30);
                    continue;
                }
                match c as u8 {
                    b'F' | b'N' => {}
                    b'*' => suppress += 1,
                    b'I' if f(p + 1) == b'6' as i32 && f(p + 2) == b'4' as i32 => {
                        p += 2;
                        integer64 += 1;
                    }
                    b'I' if f(p + 1) == b'3' as i32 && f(p + 2) == b'2' as i32 => p += 2,
                    b'I' if b"dioxX".contains(&(f(p + 1) as u8)) => {}
                    b'L' => longone += 1,
                    b'h' => {
                        longone -= 1;
                        widechar -= 1;
                    }
                    b'l' if f(p + 1) == b'l' as i32 => {
                        p += 1;
                        integer64 += 1;
                    }
                    b'l' => {
                        longone += 1;
                        widechar += 1;
                    }
                    b'w' => widechar += 1,
                    _ => break,
                }
            }
            if widechar == 0 {
                widechar = if f(p) == b'S' as i32 || f(p) == b'C' as i32 { 1 } else { -1 };
            }
            let mut comchr = (f(p) | 0x20) as u8;
            if comchr != b'n' {
                ch = if comchr == b'c' || comchr == b'{' { s.inc() } else { s.eat_white() };
                if ch == EOF {
                    return done(ch, assigned, matched, values);
                }
            }
            if widthset != 0 && width == 0 {
                s.un_inc(ch);
                return done(ch, assigned, matched, values);
            }
            match comchr {
                b'c' | b's' | b'{' => {
                    if comchr == b'c' && widthset == 0 {
                        widthset = 1;
                        width = 1;
                    }
                    let mut table = [0u8; 32];
                    let mut reject = 0u8;
                    if comchr == b'{' {
                        p += 1;
                        if f(p) == b'^' as i32 {
                            p += 1;
                            reject = 0xFF;
                        }
                        let mut prev = 0i32;
                        if f(p) == b']' as i32 {
                            prev = b']' as i32;
                            p += 1;
                            table[(b']' >> 3) as usize] |= 1 << (b']' & 7);
                        }
                        while f(p) != b']' as i32 {
                            if f(p) == 0 {
                                return done(ch, assigned, matched, values);
                            }
                            let c = f(p);
                            p += 1;
                            if c == b'-' as i32 && prev != 0 && f(p) != b']' as i32 {
                                let end = f(p);
                                p += 1;
                                let (lo, hi) = if prev < end { (prev, end) } else { (end, prev) };
                                for r in lo..=hi {
                                    table[(r >> 3) as usize] |= 1 << (r & 7);
                                }
                                prev = 0;
                            } else {
                                table[(c >> 3) as usize] |= 1 << (c & 7);
                                prev = c;
                            }
                        }
                    }
                    s.un_inc(ch);
                    let mut got = vec![];
                    loop {
                        if widthset != 0 {
                            if width == 0 {
                                break;
                            }
                            width -= 1;
                        }
                        ch = s.inc();
                        let takes = ch != EOF
                            && match comchr {
                                b'c' => true,
                                b's' => !(9..=13).contains(&ch) && ch != 32,
                                _ => (table[(ch >> 3) as usize] ^ reject) & (1 << (ch & 7)) != 0,
                            };
                        if !takes {
                            s.un_inc(ch);
                            break;
                        }
                        got.push(ch as u8);
                    }
                    if got.is_empty() {
                        return done(ch, assigned, matched, values);
                    }
                    if suppress == 0 {
                        assigned += 1;
                        let wide = || got.iter().map(|&b| b as u16).collect();
                        values.push(match (widechar > 0, comchr == b'c') {
                            (true, true) => Value::WideChars(wide()),
                            (true, false) => Value::Wide(wide()),
                            (false, true) => Value::Chars(got),
                            (false, false) => Value::Bytes(got),
                        });
                    }
                }
                b'd' | b'o' | b'u' | b'p' | b'i' | b'x' => {
                    if comchr == b'p' {
                        longone = 1;
                    }
                    if comchr == b'i' {
                        comchr = b'd';
                    }
                    let hex_prefix = comchr == b'x' || f(p) | 0x20 == b'i' as i32;
                    let (mut flag_done, mut negative, mut started) = (false, false, 0);
                    if ch == b'-' as i32 || ch == b'+' as i32 {
                        negative = ch == b'-' as i32;
                        width -= 1;
                        if width == 0 && widthset != 0 {
                            flag_done = true;
                        } else {
                            ch = s.inc();
                        }
                    }
                    if hex_prefix && ch == b'0' as i32 {
                        ch = s.inc();
                        if ch == b'x' as i32 || ch == b'X' as i32 {
                            ch = s.inc();
                            if widthset != 0 {
                                width -= 2;
                                if width < 1 {
                                    flag_done = true;
                                }
                            }
                            comchr = b'x';
                        } else {
                            started = 1;
                            if comchr == b'x' {
                                s.un_inc(ch);
                                ch = b'0' as i32;
                            } else {
                                if widthset != 0 {
                                    width -= 1;
                                    if width == 0 {
                                        flag_done = true;
                                    }
                                }
                                comchr = b'o';
                            }
                        }
                    }
                    let mut num: u64 = 0;
                    while !flag_done {
                        let mut digit = ch;
                        if comchr == b'x' || comchr == b'p' {
                            if is_xdigit(ch) {
                                num <<= 4;
                                if !is_digit(ch) {
                                    digit = (ch & !0x20) - 7;
                                }
                            } else {
                                flag_done = true;
                            }
                        } else if is_digit(ch) {
                            if comchr == b'o' {
                                if ch < b'8' as i32 {
                                    num <<= 3;
                                } else {
                                    flag_done = true;
                                }
                            } else {
                                num = num.wrapping_mul(10);
                            }
                        } else {
                            flag_done = true;
                        }
                        if !flag_done {
                            started += 1;
                            num = num.wrapping_add((digit - 0x30) as i64 as u64);
                            if integer64 == 0 {
                                num &= 0xFFFF_FFFF;
                            }
                            if widthset != 0 {
                                width -= 1;
                                if width == 0 {
                                    flag_done = true;
                                    continue;
                                }
                            }
                            ch = s.inc();
                        } else {
                            s.un_inc(ch);
                        }
                    }
                    if negative {
                        num = num.wrapping_neg();
                    }
                    if started == 0 {
                        return done(ch, assigned, matched, values);
                    }
                    if suppress == 0 {
                        assigned += 1;
                        values.push(if integer64 != 0 {
                            Value::Int { bits: num, size: 8 }
                        } else if longone != 0 {
                            Value::Int { bits: num & 0xFFFF_FFFF, size: 4 }
                        } else {
                            Value::Int { bits: num & 0xFFFF, size: 2 }
                        });
                    }
                }
                b'n' => {
                    if suppress == 0 {
                        values.push(Value::Untouched);
                    }
                }
                b'e' | b'f' | b'g' => {
                    let mut buf = vec![];
                    let mut started = 0;
                    if ch == b'-' as i32 || ch == b'+' as i32 {
                        if ch == b'-' as i32 {
                            buf.push(b'-');
                        }
                        width -= 1;
                        ch = s.inc();
                    }
                    if widthset == 0 {
                        width = -1;
                    }
                    let take = |width: &mut i32| {
                        let w = *width;
                        *width -= 1;
                        w != 0
                    };
                    while is_digit(ch) && take(&mut width) {
                        started += 1;
                        buf.push(ch as u8);
                        ch = s.inc();
                    }
                    if ch == b'.' as i32 && take(&mut width) {
                        ch = s.inc();
                        buf.push(b'.');
                        while is_digit(ch) && take(&mut width) {
                            started += 1;
                            buf.push(ch as u8);
                            ch = s.inc();
                        }
                    }
                    if started != 0 && (ch == b'e' as i32 || ch == b'E' as i32) && take(&mut width) {
                        buf.push(b'e');
                        ch = s.inc();
                        if ch == b'-' as i32 || ch == b'+' as i32 {
                            if ch == b'-' as i32 {
                                buf.push(b'-');
                            }
                            if take(&mut width) {
                                ch = s.inc();
                            } else {
                                width += 1;
                            }
                        }
                        while is_digit(ch) && take(&mut width) {
                            started += 1;
                            buf.push(ch as u8);
                            ch = s.inc();
                        }
                    }
                    s.un_inc(ch);
                    if started == 0 {
                        return done(ch, assigned, matched, values);
                    }
                    if suppress == 0 {
                        assigned += 1;
                        values.push(if longone - 1 != 0 { Value::Double(atodbl(&buf)) } else { Value::Float(atoflt(&buf)) });
                    }
                }
                _ => {
                    if f(p) != ch {
                        s.un_inc(ch);
                        return done(ch, assigned, matched, values);
                    }
                    matched -= 1;
                }
            }
            matched += 1;
            p += 1;
        }
        if ch == EOF && (f(p) != b'%' as i32 || f(p + 1) != b'n' as i32) {
            break;
        }
    }
    done(ch, assigned, matched, values)
}

type Ld12 = u128;
const M96: u128 = (1 << 96) - 1;

fn exp16(x: Ld12) -> u16 {
    (x >> 80) as u16
}

fn inf12(sign: u16) -> Ld12 {
    ((0x7FFF | sign) as u128) << 80 | 1 << 79
}

fn mtold12(digits: &[u8]) -> Ld12 {
    let mut v: u128 = 0;
    for &d in digits {
        v = (v * 10 + d as u128) & M96;
    }
    let mut e: u16 = 0x404E;
    while v >> 64 == 0 {
        v = (v << 16) & M96;
        e = e.wrapping_sub(16);
    }
    while v & 1 << 79 == 0 {
        v = (v << 1) & M96;
        e = e.wrapping_sub(1);
    }
    (v & ((1 << 80) - 1)) | (e as u128) << 80
}

fn ld12mul(x: Ld12, y: Ld12) -> Ld12 {
    let (ex, ey) = (exp16(x), exp16(y));
    let sign = (ex ^ ey) & 0x8000;
    let (ex, ey) = (ex & 0x7FFF, ey & 0x7FFF);
    let mut expsum = ex.wrapping_add(ey);
    if ex >= 0x7FFF || ey >= 0x7FFF || expsum > 0xBFFD {
        return inf12(sign);
    }
    if expsum <= 0x3FBF {
        return 0;
    }
    let is_zero = |v: Ld12| v & ((1 << 95) - 1) == 0;
    if ex == 0 {
        expsum = expsum.wrapping_add(1);
        if is_zero(x) {
            return 0;
        }
    }
    if ey == 0 {
        expsum = expsum.wrapping_add(1);
        if is_zero(y) {
            return 0;
        }
    }
    let piece = |v: Ld12, i: usize| (v >> (16 * i)) as u16;
    let mut t = [0u16; 6];
    for i in 0..5 {
        for k in 0..5 - i {
            let prod = piece(x, i + k) as u32 * piece(y, 4 - k) as u32;
            let r = t[i] as u32 | (t[i + 1] as u32) << 16;
            let sum = r.wrapping_add(prod);
            t[i] = sum as u16;
            t[i + 1] = (sum >> 16) as u16;
            if sum < r || sum < prod {
                t[i + 2] = t[i + 2].wrapping_add(1);
            }
        }
    }
    let mut m: u128 = t.iter().enumerate().map(|(i, &d)| (d as u128) << (16 * i)).sum();
    expsum = expsum.wrapping_sub(0x3FFE);
    while (expsum as i16) > 0 && m & 1 << 95 == 0 {
        m = (m << 1) & M96;
        expsum = expsum.wrapping_sub(1);
    }
    if (expsum as i16) <= 0 {
        expsum = expsum.wrapping_sub(1);
        if (expsum as i16) < 0 {
            let mut sticky = false;
            while (expsum as i16) < 0 {
                sticky |= m & 1 != 0;
                m >>= 1;
                expsum = expsum.wrapping_add(1);
            }
            if sticky {
                m |= 1;
            }
        }
    }
    if (m & 0xFFFF) > 0x8000 || (m & 0x1FFFF) == 0x18000 {
        let field = |m: u128, at: u32, bits: u32| (m >> at) as u64 & ((1u64 << bits) - 1);
        let set = |m: u128, at: u32, bits: u32, v: u64| (m & !((((1u128 << bits) - 1) as u128) << at)) | (v as u128) << at;
        if field(m, 16, 32) == 0xFFFF_FFFF {
            m = set(m, 16, 32, 0);
            if field(m, 48, 32) == 0xFFFF_FFFF {
                m = set(m, 48, 32, 0);
                if field(m, 80, 16) == 0xFFFF {
                    m = set(m, 80, 16, 0x8000);
                    expsum = expsum.wrapping_add(1);
                } else {
                    m = set(m, 80, 16, field(m, 80, 16) + 1);
                }
            } else {
                m = set(m, 48, 32, field(m, 48, 32) + 1);
            }
        } else {
            m = set(m, 16, 32, field(m, 16, 32) + 1);
        }
    }
    if expsum >= 0x7FFF {
        return inf12(sign);
    }
    (m >> 16) | ((expsum | sign) as u128) << 80
}

const POW10_POS: [[u128; 7]; 5] = [
    [0x4002a0000000000000000000, 0x4005c8000000000000000000, 0x4008fa000000000000000000, 0x400c9c400000000000000000, 0x400fc3500000000000000000, 0x4012f4240000000000000000, 0x401698968000000000000000],
    [0x4019bebc2000000000000000, 0x40348e1bc9bf040000000000, 0x404ed3c21bcecceda1000000, 0x40699dc5ada82b70b59ef020, 0x4083eb194f8e1ae525fd5dd0, 0x409eaf298d050e4395d79671, 0x40b982818f1281ed44a0bff9],
    [0x40d3c2781f49ffcfa6d53cbf, 0x41a893ba47c980e98ce0c66f, 0x427ce070f78d3927556b85bc, 0x4351aa7eebfb9df9de8eddbc, 0x442681842f29f2cce376e6a1, 0x44fac4c5e310aef8aa171028, 0x45cf957a4ae1ebf7f3d4a7eb],
    [0x46a3e319a0aea60e91c7cc65, 0x4d48c976758681750c17650d, 0x53edb2b8353b3993a7e44258, 0x5a929e8b3b5dc53d5de5a74d, 0x61378ca554c020a1f0a65dff, 0x67dbf9895d25d88b5a8bfdd1, 0x6e80dd5dc8a2bf27f3f895aa],
    [0x7525c46052028a20979bc94c, 0x000000003ffbcccccccccccc, 0xcccdcccd3ff8a3d70a3d70a3, 0xd70a3d713ff583126e978d4f, 0xdf3b645a3ff1d1b71758e219, 0x652cd3c33feea7c5ac471b47, 0x84230fd03feb8637bd05af6c],
];
const POW10_NEG: [[u128; 7]; 5] = [
    [0x3ffbcccccccccccccccdcccd, 0x3ff8a3d70a3d70a3d70a3d71, 0x3ff583126e978d4fdf3b645a, 0x3ff1d1b71758e219652cd3c3, 0x3feea7c5ac471b4784230fd0, 0x3feb8637bd05af6c69b6a640, 0x3fe7d6bf94d5e57a42bc3d33],
    [0x3fe4abcc77118461cefdfdc2, 0x3fc9e69594bec44de15b4c2f, 0x3faf9abe14cd44753b53c492, 0x3f94cfb11ead453994ba67de, 0x3f7a8b61313bbabce2c62324, 0x3f5fbb127c53b17ec1595561, 0x3f44fb158592be068d2feed7],
    [0x3f2aa87fea27a539e9a53f24, 0x3e55ddd0467c64bce4a1ac7d, 0x3d8191ff83775423cc067b63, 0x3cacc0314325637a193afa91, 0x3bd7fd00b897478238d18921, 0x3b03a686e3e8b11b085888dc, 0x3a2edb377599b607424584c6],
    [0x395a9049ee32db23d21c7133, 0x32b5a2a682a5da57c0be87a6, 0x2c10b759449f52a711b268e2, 0x256bceae534f34362de44925, 0x1ec6e8fb7dc2dec0a404598f, 0x18228350bf3c91575a88e79e, 0x117d9406af8f83fd62654b4e],
    [0x0ad8a6dd04c8d2ce9fde2de4, 0x000026941993052000000000, 0, 0, 0, 0, 0],
];

fn multtenpow12(mut x: Ld12, pow: i32) -> Ld12 {
    if pow == 0 {
        return x;
    }
    let (mut pow, table) = if pow < 0 { (-pow, &POW10_NEG) } else { (pow, &POW10_POS) };
    x &= !0xFFFF;
    let mut row = 0;
    while pow != 0 {
        let last3 = (pow & 7) as usize;
        pow >>= 3;
        if last3 != 0 {
            let mut y = table[row][last3 - 1];
            if y & 0xFFFF >= 0x8000 {
                let lo = ((y >> 16) as u32).wrapping_sub(1);
                y = (y & !(0xFFFF_FFFFu128 << 16)) | (lo as u128) << 16;
            }
            x = ld12mul(x, y);
        }
        row += 1;
    }
    x
}

fn strgtold12(text: &[u8]) -> Ld12 {
    strgtold12_flags(text).0
}

fn strgtold12_flags(text: &[u8]) -> (Ld12, bool) {
    let c_at = |i: usize| text.get(i).copied().unwrap_or(0) as i8 as i32;
    let mut p = 0;
    while matches!(c_at(p), 0x20 | 9 | 10 | 13) {
        p += 1;
    }
    let nz = |c: i32| (0x31..=0x39).contains(&c);
    let digit = |c: i32| (0x30..=0x39).contains(&c);
    let is_e = |c: i32| matches!(c as u8, b'D' | b'E' | b'd' | b'e') && c > 0;
    let (mut sign, mut exp_sign) = (0u16, 1i32);
    let (mut found_digit, mut found_exponent) = (false, false);
    let (mut manlen, mut man_exp, mut exp) = (0usize, 0i32, 0i32);
    let mut buf = [0u8; 25];
    let mut state = 0;
    loop {
        let mut c = c_at(p);
        p += 1;
        state = match state {
            0 => match c {
                _ if nz(c) => {
                    p -= 1;
                    3
                }
                0x2E => 5,
                0x2B => 2,
                0x2D => {
                    sign = 0x8000;
                    2
                }
                0x30 => 1,
                _ => break,
            },
            1 => {
                found_digit = true;
                match c {
                    _ if nz(c) => {
                        p -= 1;
                        3
                    }
                    0x2E => 4,
                    0x2B | 0x2D => break,
                    0x30 => 1,
                    _ if is_e(c) => 6,
                    _ => break,
                }
            }
            2 => match c {
                _ if nz(c) => {
                    p -= 1;
                    3
                }
                0x2E => 5,
                0x30 => 1,
                _ => break,
            },
            3 => {
                found_digit = true;
                while digit(c) {
                    if manlen < 25 {
                        buf[manlen] = (c - 0x30) as u8;
                        manlen += 1;
                    } else {
                        man_exp += 1;
                    }
                    c = c_at(p);
                    p += 1;
                }
                match c {
                    0x2E => 4,
                    _ if is_e(c) => 6,
                    _ => break,
                }
            }
            4 => {
                found_digit = true;
                if manlen == 0 && c == 0x30 {
                    while c == 0x30 {
                        c = c_at(p);
                        p += 1;
                        man_exp -= 1;
                    }
                }
                while digit(c) {
                    if manlen < 25 {
                        buf[manlen] = (c - 0x30) as u8;
                        manlen += 1;
                        man_exp -= 1;
                    }
                    c = c_at(p);
                    p += 1;
                }
                if is_e(c) { 6 } else { break }
            }
            5 => {
                if digit(c) {
                    p -= 1;
                    4
                } else {
                    break;
                }
            }
            6 => match c {
                _ if nz(c) => {
                    p -= 1;
                    9
                }
                0x2B => 7,
                0x2D => {
                    exp_sign = -1;
                    7
                }
                0x30 => 8,
                _ => break,
            },
            7 => match c {
                _ if nz(c) => {
                    p -= 1;
                    9
                }
                0x30 => 8,
                _ => break,
            },
            8 => {
                found_exponent = true;
                while c == 0x30 {
                    c = c_at(p);
                    p += 1;
                }
                if nz(c) {
                    p -= 1;
                    9
                } else {
                    break;
                }
            }
            _ => {
                found_exponent = true;
                let mut e = 0i32;
                while digit(c) {
                    e = e * 10 + c - 0x30;
                    if e > 0x1450 {
                        e = 0x1451;
                        break;
                    }
                    c = c_at(p);
                    p += 1;
                }
                exp = e;
                break;
            }
        };
    }
    let _ = found_exponent;
    let signed = |v: Ld12| v | (sign as u128) << 80;
    if !found_digit {
        return (signed(0), true);
    }
    if manlen > 24 {
        if buf[23] >= 5 {
            buf[23] += 1;
        }
        manlen = 24;
        man_exp += 1;
    }
    if manlen == 0 {
        return (signed(0), false);
    }
    while buf[manlen - 1] == 0 {
        manlen -= 1;
        man_exp += 1;
    }
    let ld = mtold12(&buf[..manlen]);
    if exp_sign < 0 {
        exp = -exp;
    }
    let total = man_exp + exp;
    if total > 0x1450 {
        return (signed((0x7FFFu128 << 80) | 1 << 79), false);
    }
    if total < -0x1450 {
        return (signed(0), false);
    }
    (signed(multtenpow12(ld, total)), false)
}

struct Format {
    max_exp: i32,
    min_exp: i32,
    precision: i32,
    exp_width: u32,
    bias: i32,
}

const FLOAT: Format = Format { max_exp: 128, min_exp: -127, precision: 24, exp_width: 8, bias: 127 };
const DOUBLE: Format = Format { max_exp: 1024, min_exp: -1023, precision: 53, exp_width: 11, bias: 1023 };

fn round_man(man: &mut [u32; 3], precision: i32) -> bool {
    let idx = (precision >> 5) as usize;
    let bit = 31 - (precision & 31) as u32;
    let mut carry = false;
    if man[idx] & 1 << bit != 0 && (man[idx] & !(u32::MAX << bit) != 0 || man[idx + 1..].iter().any(|&w| w != 0)) {
        let at = precision - 1;
        let (j, b) = ((at >> 5) as usize, 31 - (at & 31) as u32);
        let (v, c) = man[j].overflowing_add(1 << b);
        man[j] = v;
        carry = c;
        let mut k = j;
        while carry && k > 0 {
            k -= 1;
            let (v, c) = man[k].overflowing_add(1);
            man[k] = v;
            carry = c;
        }
    }
    man[idx] &= u32::MAX << bit;
    for w in &mut man[idx + 1..] {
        *w = 0;
    }
    carry
}

fn shr_man(man: &mut [u32; 3], n: u32) {
    let (s, words) = (n % 32, (n / 32) as usize);
    let mut carry = 0;
    for w in man.iter_mut() {
        let v = *w;
        *w = v >> s | carry;
        carry = (v & !(u32::MAX << s)).checked_shl(32 - s).unwrap_or(0);
    }
    for i in (0..3).rev() {
        man[i] = if i < words { 0 } else { man[i - words] };
    }
}

fn ld12cvt(ld: Ld12, f: &Format) -> [u32; 2] {
    let e = exp16(ld) as i32;
    let sign = if e & 0x8000 != 0 { 1u32 << 31 } else { 0 };
    let exponent = (e & 0x7FFF) - 0x3FFF;
    let mut man = [(ld >> 48) as u32, (ld >> 16) as u32, ((ld & 0xFFFF) as u32) << 16];
    let bexp;
    if exponent == -0x3FFF {
        bexp = 0;
        man = [0; 3];
    } else {
        let saved = man;
        let e2 = if round_man(&mut man, f.precision) { exponent + 1 } else { exponent };
        if e2 < f.min_exp - f.precision {
            man = [0; 3];
            bexp = 0;
        } else if e2 <= f.min_exp {
            man = saved;
            shr_man(&mut man, (f.min_exp - exponent) as u32);
            round_man(&mut man, f.precision);
            shr_man(&mut man, f.exp_width + 1);
            bexp = 0;
        } else if e2 >= f.max_exp {
            man = [1 << 31, 0, 0];
            shr_man(&mut man, f.exp_width);
            bexp = f.max_exp + f.bias;
        } else {
            man[0] &= !(1 << 31);
            shr_man(&mut man, f.exp_width);
            bexp = f.bias + e2;
        }
    }
    [((bexp as u32) << (31 - f.exp_width)) | sign | man[0], man[1]]
}

pub fn atoflt(text: &[u8]) -> f32 {
    f32::from_bits(ld12cvt(strgtold12(text), &FLOAT)[0])
}

pub fn atof(text: &[u8]) -> f64 {
    let text = &text[text.iter().take_while(|&&c| is_space(c as i32)).count()..];
    let (ld, no_digit) = strgtold12_flags(text);
    if no_digit {
        return 0.0;
    }
    let [hi, lo] = ld12cvt(ld, &DOUBLE);
    f64::from_bits((hi as u64) << 32 | lo as u64)
}

pub fn strtok(buf: &mut [u8], start: Option<usize>, saved: &mut usize, delimiters: &[u8]) -> Option<usize> {
    let mut map = [0u8; 32];
    for &c in delimiters.iter().take_while(|&&c| c != 0).chain([&0]) {
        map[(c >> 3) as usize] |= 1 << (c & 7);
    }
    let at = |buf: &[u8], i: usize| buf.get(i).copied().unwrap_or(0);
    let is_delim = |c: u8| map[(c >> 3) as usize] & 1 << (c & 7) != 0;
    let mut str = start.unwrap_or(*saved);
    while at(buf, str) != 0 && is_delim(at(buf, str)) {
        str += 1;
    }
    let token = str;
    let mut end = str;
    while at(buf, end) != 0 {
        if is_delim(at(buf, end)) {
            buf[end] = 0;
            end += 1;
            break;
        }
        end += 1;
    }
    *saved = end;
    (token != end).then_some(token)
}

pub fn strncpy(src: &[u8], n: usize) -> Vec<u8> {
    let len = src.iter().position(|&c| c == 0).unwrap_or(src.len()).min(n);
    let mut out = src[..len].to_vec();
    out.resize(n, 0);
    out
}

pub fn atodbl(text: &[u8]) -> f64 {
    let [hi, lo] = ld12cvt(strgtold12(text), &DOUBLE);
    f64::from_bits((hi as u64) << 32 | lo as u64)
}

