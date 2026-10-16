pub const MAX: usize = 399;

pub fn insert_numbers(text: &[u8], int: i32, float: f32, decimals: i32, more: [i32; 5]) -> Vec<u8> {
    let mut number = if decimals >= 0 {
        format!("{:.*}", decimals as usize, float as f64).into_bytes()
    } else {
        int.to_string().into_bytes()
    };
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    let cut = len >= 400;
    let (text, end) = (&text[..len], len.min(MAX));
    let (mut out, mut used, mut i) = (Vec::with_capacity(400), 0u8, 0);
    while i < end {
        if text[i] == b'~' && text.get(i + 1) == Some(&b'1') && text.get(i + 2) == Some(&b'~') {
            i += 3;
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
    out.truncate(MAX);
    if cut && out.iter().filter(|&&c| c == b'~').count() % 2 == 1 {
        *out.last_mut().unwrap() = b'~';
    }
    out
}

pub fn insert_strings(text: &[u8], strings: [Option<&[u8]>; 2]) -> Vec<u8> {
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    let text = &text[..len];
    let (mut out, mut k, mut i) = (Vec::with_capacity(400), 0, 0);
    while i < text.len() {
        let tok = text[i] == b'~' && matches!(text.get(i + 1), Some(b'a' | b'e')) && text.get(i + 2) == Some(&b'~');
        if tok {
            i += 3;
            if let Some(Some(s)) = strings.get(k) {
                out.extend_from_slice(s);
            }
            k += 1;
        } else {
            out.push(text[i]);
            i += 1;
        }
    }
    out.truncate(MAX);
    out
}

pub const FIXED_TOKENS: [(&str, i32); 43] = [
    ("PAD_UP", 0x100), ("PAD_DOWN", 0x101), ("PAD_LEFT", 0x102), ("PAD_RIGHT", 0x103), ("PAD_DPAD_UP", 0x104),
    ("PAD_DPAD_DOWN", 0x105), ("PAD_DPAD_LEFT", 0x106), ("PAD_DPAD_RIGHT", 0x107), ("PAD_DPAD_NONE", 0x108),
    ("PAD_DPAD_ALL", 0x109), ("PAD_DPAD_UPDOWN", 0x10A), ("PAD_DPAD_LEFTRIGHT", 0x10B), ("PAD_LSTICK_UP", 0x10C),
    ("PAD_LSTICK_DOWN", 0x10D), ("PAD_LSTICK_LEFT", 0x10E), ("PAD_LSTICK_RIGHT", 0x10F), ("PAD_LSTICK_NONE", 0x110),
    ("PAD_LSTICK_ALL", 0x111), ("PAD_LSTICK_UPDOWN", 0x112), ("PAD_LSTICK_LEFTRIGHT", 0x113), ("PAD_RSTICK_UP", 0x114),
    ("PAD_RSTICK_DOWN", 0x115), ("PAD_RSTICK_LEFT", 0x116), ("PAD_RSTICK_RIGHT", 0x117), ("PAD_RSTICK_NONE", 0x118),
    ("PAD_RSTICK_ALL", 0x119), ("PAD_RSTICK_UPDOWN", 0x11A), ("PAD_RSTICK_LEFTRIGHT", 0x11B), ("PAD_A", 0x11C),
    ("INPUT_PHONE_ACCEPT", 0x11C), ("PAD_B", 0x11D), ("INPUT_PHONE_CANCEL", 0x11D), ("PAD_X", 0x11E), ("PAD_Y", 0x11F),
    ("PAD_LB", 0x120), ("PAD_LT", 0x121), ("PAD_RB", 0x122), ("PAD_RT", 0x123), ("PAD_START", 0x124),
    ("PAD_BACK", 0x125), ("ACCEPT", 0x126), ("CANCEL", 0x127), ("PAD_RSTICK_ROTATE", 0x128),
];

pub const NO_TOKEN: i32 = 0xFF;

#[derive(Clone, Debug, PartialEq)]
pub struct InputNames(pub Vec<(Vec<u8>, u32)>);

impl InputNames {
    pub fn read(image: &[u8]) -> Option<InputNames> {
        let mut out = vec![];
        for i in 0..86 {
            let o = 0x82A9_35B0 - 0x8200_0000 + 0x2C * i;
            let e = image.get(o..o + 0x2C)?;
            let name = e[..40].iter().take_while(|&&c| c != 0).copied().collect();
            out.push((name, u32::from_be_bytes(e[40..44].try_into().ok()?)));
        }
        Some(InputNames(out))
    }
}

pub fn binding_glyph(source: u32, button: u32) -> i32 {
    match (source, button) {
        (6, 0x1) => 0x121, (6, 0x2) => 0x123, (6, 0x4) => 0x120, (6, 0x8) => 0x122, (6, 0x10) => 0x11F,
        (6, 0x20) => 0x11D, (6, 0x40) => 0x11C, (6, 0x80) => 0x11E, (6, 0x100) => 0x125, (6, 0x200) => 0x110,
        (6, 0x400) => 0x118, (6, 0x800) => 0x124, (6, 0x1000) => 0x104, (6, 0x2000) => 0x107, (6, 0x4000) => 0x105,
        (6, 0x8000) => 0x106,
        (8, 0) => 0x107, (8, 1) => 0x106, (8, 2) => 0x104, (8, 3) => 0x105, (8, 4) => 0x11F, (8, 5) => 0x11D,
        (8, 6) => 0x11C, (8, 7) => 0x11E, (8, 8) => 0x120, (8, 9) => 0x122, (8, 10) => 0x121, (8, 11) => 0x123,
        (9, 0) => 0x113, (9, 1) => 0x112, (9, 2) => 0x11B, (9, 3) => 0x11A,
        _ => NO_TOKEN,
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
        v = v * 10 + (c - b'0') as u64;
        over |= v > 1 << 31;
        if over {
            v = 1 << 31;
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
    let mut name = Vec::with_capacity(40);
    for i in 0.. {
        match text.get(i) {
            Some(b'~') => break,
            c => name.push(c.copied().unwrap_or(0)),
        }
        if name.len() > 0x27 {
            return NO_TOKEN;
        }
    }
    let len = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    let s = &name[..len];
    if s.len() >= 5 && s[..5].eq_ignore_ascii_case(b"BLIP_") {
        return atoi(&s[5..]).wrapping_add(1000);
    }
    if let Some(&(_, code)) = FIXED_TOKENS.iter().find(|(n, _)| n.as_bytes().eq_ignore_ascii_case(s)) {
        return code;
    }
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case(b"INPUT_") {
        for (i, (n, control)) in inputs.0.iter().enumerate() {
            if n.eq_ignore_ascii_case(s) {
                if let Some((source, button)) = binding(*control, i) {
                    return binding_glyph(source, button);
                }
            }
        }
    }
    if name.len() < 2 { name.first().map_or(0, |&c| c as i8 as i32) } else { NO_TOKEN }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudColours {
    pub colours: Vec<(Vec<u8>, u32)>,
}

pub const MAX_HUD_COLOURS: usize = 57;

impl HudColours {
    pub fn read(file: &[u8], hi_def: bool) -> HudColours {
        let (mut in_section, mut rgb) = (false, [0i32; 3]);
        let mut colours = vec![];
        crate::textlines::each_line(file, true, |memory| {
            if colours.len() == MAX_HUD_COLOURS || matches!(memory.first(), None | Some(0) | Some(b'#')) {
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
                let word = 0xFF00_0000 | ((rgb[0] as u32) & 0xFF) << 16 | ((rgb[1] as u32) & 0xFF) << 8 | (rgb[2] as u32) & 0xFF;
                colours.push((name, word));
            } else if name.eq_ignore_ascii_case(if hi_def { b"[HD]" } else { b"[CRT]" }) {
                in_section = true;
            }
        });
        HudColours { colours }
    }

    pub fn colour(&self, index: usize, alpha: u8) -> u32 {
        (alpha as u32) << 24 | self.colours.get(index).map_or(0, |c| c.1 & 0xFF_FFFF)
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
    let alpha = (colour >> 24) as u8;
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
    let white = if alt_white { 0x2B } else { 1 };
    let grey = (alpha as u32) << 24 | 0x8C_8C8C;
    let brighten = || {
        let up = |v: u32| {
            let f = v as f32 * 1.5;
            (if f - 255.0 < 0.0 { f } else { 255.0 }) as i64 as u32 & 0xFF
        };
        (alpha as u32) << 24 | up(colour >> 16 & 0xFF) << 16 | up(colour >> 8 & 0xFF) << 8 | up(colour & 0xFF)
    };
    if code > 999 {
        e.icon = Some(code);
    } else if code > 99 {
        match (code - 0x67) as u32 {
            0 => set(&mut e, hud_colour(0x10), true),
            1 => set(&mut e, brighten(), true),
            5 => set(&mut e, hud_colour(2), true),
            6 => set(&mut e, hud_colour(0x38), true),
            7 => e.new_line = true,
            9 => set(&mut e, hud_colour(0x13), true),
            0xB => set(&mut e, hud_colour(4), true),
            0xC | 0x10 => set(&mut e, hud_colour(white), true),
            0x12 => set(&mut e, hud_colour(0xB), true),
            0x99..=0xC1 => e.icon = Some(code - 0xFF),
            0xC2..=0xD1 => set(&mut e, hud_colour((code - 0x10F) as usize), false),
            _ => {}
        }
    } else if code == 99 {
        set(&mut e, grey, true);
    } else {
        match (code - 0x24) as u32 {
            0 => set(&mut e, hud_colour(0x18), true),
            0x1E | 0x3E => set(&mut e, hud_colour(9), true),
            0x1F => set(&mut e, grey, true),
            0x23 => set(&mut e, hud_colour(0x10), true),
            0x24 => set(&mut e, brighten(), true),
            0x29 => set(&mut e, hud_colour(0x38), true),
            0x2A => e.new_line = true,
            0x2C => set(&mut e, hud_colour(0x13), true),
            0x2E => set(&mut e, hud_colour(4), true),
            0x2F | 0x33 => set(&mut e, hud_colour(white), true),
            0x35 => set(&mut e, hud_colour(0xB), true),
            _ => {}
        }
    }
    e
}

