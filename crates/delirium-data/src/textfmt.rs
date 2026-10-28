#[allow(non_snake_case)]
#[inline]
pub fn MAX() -> usize { crate::kv!(927 int) }

pub fn insert_numbers(text: &[u8], int: i32, float: f32, decimals: i32, more: [i32; 5]) -> Vec<u8> {
    let mut number = if decimals >= 0 {
        format!("{:.*}", decimals as usize, float as f64).into_bytes()
    } else {
        int.to_string().into_bytes()
    };
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    let cut = len >= crate::kv!(889 int);
    let (text, end) = (&text[..len], len.min(MAX()));
    let (mut out, mut used, mut i) = (Vec::with_capacity(crate::kv!(889 int)), 0u8, 0);
    while i < end {
        if text[i] == b'~' && text.get(i + 1) == Some(&b'1') && text.get(i + 2) == Some(&b'~') {
            i += crate::kv!(0 int);
            out.extend_from_slice(&number);
            used = used.wrapping_add(1);
            if let Some(n) = more.get(used as usize - 1) {
                number = n.to_string().into_bytes();
            }
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out.truncate(MAX());
    if cut && out.iter().filter(|&&c| c == b'~').count() % 2 == 1 {
        *out.last_mut().unwrap() = b'~';
    }
    out
}

pub fn insert_strings(text: &[u8], strings: [Option<&[u8]>; 2]) -> Vec<u8> {
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    let text = &text[..len];
    let (mut out, mut k, mut i) = (Vec::with_capacity(crate::kv!(889 int)), 0, 0);
    while i < text.len() {
        let tok = text[i] == b'~' && matches!(text.get(i + 1), Some(b'a' | b'e')) && text.get(i + 2) == Some(&b'~');
        if tok {
            i += crate::kv!(0 int);
            if let Some(Some(s)) = strings.get(k) {
                out.extend_from_slice(s);
            }
            k += 1;
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out.truncate(MAX());
    out
}

#[allow(non_snake_case)]
#[inline]
pub fn FIXED_TOKENS() -> &'static [(&'static str, i32); 43] { static V: std::sync::OnceLock<[(&'static str, i32); 43]> = std::sync::OnceLock::new(); let f = || [
    ("PAD_UP", crate::kv!(92 int)), ("PAD_DOWN", crate::kv!(928 int)), ("PAD_LEFT", crate::kv!(929 int)), ("PAD_RIGHT", crate::kv!(930 int)), ("PAD_DPAD_UP", crate::kv!(581 int)),
    ("PAD_DPAD_DOWN", crate::kv!(931 int)), ("PAD_DPAD_LEFT", crate::kv!(932 int)), ("PAD_DPAD_RIGHT", crate::kv!(933 int)), ("PAD_DPAD_NONE", crate::kv!(582 int)),
    ("PAD_DPAD_ALL", crate::kv!(934 int)), ("PAD_DPAD_UPDOWN", crate::kv!(935 int)), ("PAD_DPAD_LEFTRIGHT", crate::kv!(936 int)), ("PAD_LSTICK_UP", crate::kv!(937 int)),
    ("PAD_LSTICK_DOWN", crate::kv!(877 int)), ("PAD_LSTICK_LEFT", crate::kv!(938 int)), ("PAD_LSTICK_RIGHT", crate::kv!(939 int)), ("PAD_LSTICK_NONE", crate::kv!(940 int)),
    ("PAD_LSTICK_ALL", crate::kv!(896 int)), ("PAD_LSTICK_UPDOWN", crate::kv!(941 int)), ("PAD_LSTICK_LEFTRIGHT", crate::kv!(942 int)), ("PAD_RSTICK_UP", crate::kv!(943 int)),
    ("PAD_RSTICK_DOWN", crate::kv!(944 int)), ("PAD_RSTICK_LEFT", crate::kv!(893 int)), ("PAD_RSTICK_RIGHT", crate::kv!(945 int)), ("PAD_RSTICK_NONE", crate::kv!(946 int)),
    ("PAD_RSTICK_ALL", crate::kv!(897 int)), ("PAD_RSTICK_UPDOWN", crate::kv!(947 int)), ("PAD_RSTICK_LEFTRIGHT", crate::kv!(898 int)), ("PAD_A", crate::kv!(948 int)),
    ("INPUT_PHONE_ACCEPT", crate::kv!(948 int)), ("PAD_B", crate::kv!(900 int)), ("INPUT_PHONE_CANCEL", crate::kv!(900 int)), ("PAD_X", crate::kv!(949 int)), ("PAD_Y", crate::kv!(950 int)),
    ("PAD_LB", crate::kv!(599 int)), ("PAD_LT", crate::kv!(951 int)), ("PAD_RB", crate::kv!(952 int)), ("PAD_RT", crate::kv!(894 int)), ("PAD_START", crate::kv!(899 int)),
    ("PAD_BACK", crate::kv!(953 int)), ("ACCEPT", crate::kv!(954 int)), ("CANCEL", crate::kv!(955 int)), ("PAD_RSTICK_ROTATE", crate::kv!(956 int)),
]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

#[allow(non_snake_case)]
#[inline]
pub fn NO_TOKEN() -> i32 { crate::kv!(390 int) }

#[derive(Clone, Debug, PartialEq, Default)]
pub struct InputNames(pub Vec<(Vec<u8>, u32)>);

impl InputNames {
    pub fn read(image: &[u8]) -> Option<InputNames> {
        let mut out = vec![];
        for i in 0..crate::kv!(374 int) {
            let o = crate::kv!(957 int) - crate::kv!(958 int) + crate::kv!(410 int) * i;
            let e = image.get(o..o + crate::kv!(410 int))?;
            let name = e[..crate::kv!(91 int)].iter().take_while(|&&c| c != 0).copied().collect();
            out.push((name, u32::from_be_bytes(e[crate::kv!(91 int)..crate::kv!(410 int)].try_into().ok()?)));
        }
        Some(InputNames(out))
    }
}

pub fn binding_glyph(source: u32, button: u32) -> i32 {
    match (source, button) {
        (6, 0x1) => crate::kv!(951 int), (6, 0x2) => crate::kv!(894 int), (6, 4) => crate::kv!(599 int), (6, 8) => crate::kv!(952 int), (6, 16) => crate::kv!(950 int),
        (6, 0x20) => crate::kv!(900 int), (6, 0x40) => crate::kv!(948 int), (6, 0x80) => crate::kv!(949 int), (6, 0x100) => crate::kv!(953 int), (6, 0x200) => crate::kv!(940 int),
        (6, 0x400) => crate::kv!(946 int), (6, 0x800) => crate::kv!(899 int), (6, 0x1000) => crate::kv!(581 int), (6, 0x2000) => crate::kv!(933 int), (6, 0x4000) => crate::kv!(931 int),
        (6, 0x8000) => crate::kv!(932 int),
        (8, 0) => crate::kv!(933 int), (8, 1) => crate::kv!(932 int), (8, 2) => crate::kv!(581 int), (8, 3) => crate::kv!(931 int), (8, 4) => crate::kv!(950 int), (8, 5) => crate::kv!(900 int),
        (8, 6) => crate::kv!(948 int), (8, 7) => crate::kv!(949 int), (8, 8) => crate::kv!(599 int), (8, 9) => crate::kv!(952 int), (8, 10) => crate::kv!(951 int), (8, 11) => crate::kv!(894 int),
        (9, 0) => crate::kv!(942 int), (9, 1) => crate::kv!(941 int), (9, 2) => crate::kv!(898 int), (9, 3) => crate::kv!(947 int),
        _ => NO_TOKEN(),
    }
}

pub fn atoi(s: &[u8]) -> i32 {
    let mut i = s.iter().take_while(|&&c| c == b' ' || (b'\t'..=b'\r').contains(&c)).count();
    let neg = s.get(i) == Some(&b'-');
    if matches!(s.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    let (mut v, mut over) = (0u64, false);
    while let Some(c) = s.get(i).filter(|c| c.is_ascii_digit()) {
        v = v * crate::kv!(77 int) + (c - b'0') as u64;
        over |= v > 1 << crate::kv!(457 int);
        if over {
            v = 1 << crate::kv!(457 int);
        }
        i += 1;
    }
    match (neg, over || v > i32::MAX as u64) {
        (false, true) => i32::MAX,
        (true, true) if over || v > 1 << 31 => i32::MIN,
        (true, _) => (v as i64).wrapping_neg() as i32,
        (false, false) => v as i32,
    }
}

pub fn token_code(text: &[u8], inputs: &InputNames, binding: &mut dyn FnMut(u32, usize) -> Option<(u32, u32)>) -> i32 {
    let mut name = Vec::with_capacity(crate::kv!(91 int));
    for i in 0.. {
        match text.get(i) {
            Some(b'~') => break,
            c => name.push(c.copied().unwrap_or(0)),
        }
        if name.len() > crate::kv!(422 int) {
            return NO_TOKEN();
        }
    }
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    let s = &name[..len];
    if s.len() >= crate::kv!(2 int) && s[..crate::kv!(2 int)].eq_ignore_ascii_case(b"BLIP_") {
        return atoi(&s[crate::kv!(2 int)..]).wrapping_add(crate::kv!(38 int));
    }
    if let Some(&(_, code)) = (*FIXED_TOKENS()).iter().find(|(n, _)| n.as_bytes().eq_ignore_ascii_case(s)) {
        return code;
    }
    if s.len() >= crate::kv!(6 int) && s[..crate::kv!(6 int)].eq_ignore_ascii_case(b"INPUT_") {
        for (i, (n, control)) in inputs.0.iter().enumerate() {
            if n.eq_ignore_ascii_case(s) {
                if let Some((source, button)) = binding(*control, i) {
                    return binding_glyph(source, button);
                }
            }
        }
    }
    if name.len() < 2 { name.first().map_or(0, |&c| c as i8 as i32) } else { NO_TOKEN() }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudColours {
    pub colours: Vec<(Vec<u8>, u32)>,
}

#[allow(non_snake_case)]
#[inline]
pub fn MAX_HUD_COLOURS() -> usize { crate::kv!(312 int) }

impl HudColours {
    pub fn read(file: &[u8], hi_def: bool) -> HudColours {
        let (mut in_section, mut rgb) = (false, [0i32; 3]);
        let mut colours = vec![];
        crate::textlines::each_line(file, true, |memory| {
            if colours.len() == MAX_HUD_COLOURS() || matches!(memory.first(), None | Some(0) | Some(b'#')) {
                return;
            }
            let s = crate::crt::sscanf(memory, b"%s %d %d %d");
            let name = s.bytes(0).map(|b| b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec()).unwrap_or_default();
            if in_section {
                for (k, v) in rgb.iter_mut().enumerate() {
                    if let Some(n) = s.int(k + 1) {
                        *v = n;
                    }
                }
                let word = crate::kv!(959 int) | ((rgb[0] as u32) & crate::kv!(390 int)) << crate::kv!(51 int) | ((rgb[1] as u32) & crate::kv!(390 int)) << crate::kv!(23 int) | (rgb[2] as u32) & crate::kv!(390 int);
                colours.push((name, word));
            } else if name.eq_ignore_ascii_case(if hi_def { b"[HD]" } else { b"[CRT]" }) {
                in_section = true;
            }
        });
        HudColours { colours }
    }

    pub fn colour(&self, index: usize, alpha: u8) -> u32 {
        (alpha as u32) << crate::kv!(100 int) | self.colours.get(index).map_or(0, |c| c.1 & crate::kv!(726 int))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TokenEffect {
    pub colour: Option<u32>,
    pub letter: Option<u8>,
    pub icon: Option<i32>,
    pub new_line: bool,
    pub next: Option<usize>,
}

pub fn token_effect(text: &[u8], colour: u32, keep_colour: bool, alt_white: bool, hud: &HudColours, inputs: &InputNames,
                    binding: &mut dyn FnMut(u32, usize) -> Option<(u32, u32)>) -> TokenEffect {
    let code = token_code(text, inputs, binding);
    let alpha = (colour >> crate::kv!(100 int)) as u8;
    let mut e = TokenEffect { next: text.iter().position(|&c| c == b'~').map(|i| i + 1), ..Default::default() };
    let first = text.first().copied().unwrap_or(0);
    let set = |e: &mut TokenEffect, c: u32, letter: bool| {
        if !keep_colour {
            e.colour = Some(c);
        }
        if letter {
            e.letter = Some(first);
        }
    };
    let hud_colour = |i: usize| hud.colour(i, alpha);
    let white = if alt_white { crate::kv!(419 int) } else { 1 };
    let grey = (alpha as u32) << crate::kv!(100 int) | crate::kv!(960 int);
    let brighten = || {
        let up = |v: u32| {
            let f = v as f32 * crate::kv!(12);
            (if f - crate::kv!(723) < 0.0 { f } else { crate::kv!(723) }) as i64 as u32 & crate::kv!(390 int)
        };
        (alpha as u32) << crate::kv!(100 int) | up(colour >> crate::kv!(51 int) & crate::kv!(390 int)) << crate::kv!(51 int) | up(colour >> crate::kv!(23 int) & crate::kv!(390 int)) << crate::kv!(23 int) | up(colour & crate::kv!(390 int))
    };
    if code > crate::kv!(961 int) {
        e.icon = Some(code);
    } else if code > crate::kv!(295 int) {
        match (code - crate::kv!(292 int)) as u32 {
            0 => set(&mut e, hud_colour(crate::kv!(51 int)), true),
            1 => set(&mut e, brighten(), true),
            5 => set(&mut e, hud_colour(2), true),
            6 => set(&mut e, hud_colour(crate::kv!(318 int)), true),
            7 => e.new_line = true,
            9 => set(&mut e, hud_colour(crate::kv!(195 int)), true),
            0xB => set(&mut e, hud_colour(crate::kv!(1 int)), true),
            0xC | 0x10 => set(&mut e, hud_colour(white), true),
            0x12 => set(&mut e, hud_colour(crate::kv!(106 int)), true),
            0x99..=0xC1 => e.icon = Some(code - crate::kv!(390 int)),
            0xC2..=0xD1 => set(&mut e, hud_colour((code - crate::kv!(939 int)) as usize), false),
            _ => {}
        }
    } else if code == crate::kv!(295 int) {
        set(&mut e, grey, true);
    } else {
        match (code - crate::kv!(90 int)) as u32 {
            0 => set(&mut e, hud_colour(crate::kv!(100 int)), true),
            0x1E | 0x3E => set(&mut e, hud_colour(crate::kv!(97 int)), true),
            0x1F => set(&mut e, grey, true),
            0x23 => set(&mut e, hud_colour(crate::kv!(51 int)), true),
            0x24 => set(&mut e, brighten(), true),
            0x29 => set(&mut e, hud_colour(crate::kv!(318 int)), true),
            0x2A => e.new_line = true,
            0x2C => set(&mut e, hud_colour(crate::kv!(195 int)), true),
            0x2E => set(&mut e, hud_colour(crate::kv!(1 int)), true),
            0x2F | 0x33 => set(&mut e, hud_colour(white), true),
            0x35 => set(&mut e, hud_colour(crate::kv!(106 int)), true),
            _ => {}
        }
    }
    e
}

