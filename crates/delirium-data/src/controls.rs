#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Button(u32),
    Analog(u32),
    Axis(u32),
}

impl Source {
    pub fn code(self) -> (u32, u32) {
        match self {
            Source::Button(b) => (6, b),
            Source::Analog(i) => (8, i),
            Source::Axis(i) => (9, i),
        }
    }
}

pub type Mapper = u32;
pub const M0: Mapper = 0x000;
pub const M1: Mapper = 0x308;
pub const M2: Mapper = 0x610;
pub const M3: Mapper = 0xD58;
pub const MAPPERS: [Mapper; 4] = [M0, M1, M2, M3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub mapper: Mapper,
    pub source: Source,
    pub control: u32,
}

const fn b(mapper: Mapper, source: Source, control: u32) -> Binding {
    Binding { mapper, source, control }
}
use Source::{Analog, Axis, Button};

const NO_PAD: &[Binding] = &[
    b(M0, Button(0x100), 0x918),
    b(M0, Button(0x40), 0x924),
    b(M0, Button(0x80), 0x930),
    b(M0, Button(0x20), 0x948),
    b(M0, Button(0x4), 0x954),
    b(M0, Button(0x10), 0x93C),
    b(M0, Button(0x8), 0x960),
    b(M0, Button(0x4), 0xA2C),
    b(M0, Button(0x2), 0x978),
    b(M0, Button(0x1), 0x984),
    b(M0, Button(0x200), 0xA08),
    b(M0, Button(0x400), 0x96C),
    b(M0, Button(0x8), 0xA68),
    b(M0, Button(0x20), 0xA74),
    b(M0, Button(0x1000), 0xA14),
    b(M0, Button(0x4000), 0xA20),
    b(M0, Axis(0x1), 0xA38),
    b(M0, Axis(0x1), 0xA44),
    b(M0, Axis(0x3), 0xA50),
    b(M0, Axis(0x3), 0xA5C),
    b(M0, Axis(0x0), 0x9A8),
    b(M0, Axis(0x0), 0x9B4),
    b(M0, Axis(0x1), 0x9C0),
    b(M0, Axis(0x1), 0x9CC),
    b(M0, Axis(0x2), 0x9D8),
    b(M0, Axis(0x2), 0x9E4),
    b(M0, Axis(0x3), 0x9F0),
    b(M0, Axis(0x3), 0x9FC),
    b(M1, Button(0x20), 0xBDC),
    b(M1, Button(0x10), 0xBE8),
    b(M1, Button(0x2), 0xBF4),
    b(M0, Button(0x4), 0xD14),
    b(M1, Button(0x80), 0xC00),
    b(M1, Button(0x40), 0xC0C),
    b(M2, Axis(0x0), 0xA80),
    b(M2, Axis(0x0), 0xA8C),
    b(M2, Axis(0x1), 0xA98),
    b(M2, Axis(0x1), 0xAA4),
    b(M2, Axis(0x2), 0xAB0),
    b(M2, Axis(0x2), 0xABC),
    b(M2, Axis(0x3), 0xAC8),
    b(M2, Axis(0x3), 0xAD4),
    b(M2, Button(0x4), 0xAE0),
    b(M2, Button(0x4), 0xAEC),
    b(M2, Button(0x10), 0xB1C),
    b(M2, Button(0x80), 0xB10),
    b(M2, Button(0x8), 0xB28),
    b(M2, Button(0x20), 0xB7C),
    b(M2, Button(0x400), 0xB70),
    b(M2, Button(0x2000), 0xB88),
    b(M2, Button(0x8000), 0xB94),
    b(M2, Button(0x200), 0xBA0),
    b(M2, Button(0x40), 0xAF8),
    b(M2, Button(0x80), 0xB04),
    b(M2, Button(0x40), 0xBAC),
    b(M2, Button(0x80), 0xBB8),
    b(M2, Button(0x1), 0xBC4),
];

const PAD: &[Binding] = &[
    b(M0, Axis(0x0), 0x9A8),
    b(M0, Axis(0x0), 0x9B4),
    b(M0, Axis(0x1), 0x9C0),
    b(M0, Axis(0x1), 0x9CC),
    b(M0, Axis(0x2), 0x9D8),
    b(M0, Axis(0x2), 0x9E4),
    b(M0, Axis(0x3), 0x9F0),
    b(M0, Axis(0x3), 0x9FC),
    b(M0, Button(0x100), 0x918),
    b(M0, Button(0x40), 0x924),
    b(M0, Button(0x80), 0x930),
    b(M0, Button(0x10), 0x93C),
    b(M0, Button(0x200), 0xA08),
    b(M0, Button(0x400), 0x96C),
    b(M0, Button(0x2000), 0x978),
    b(M0, Button(0x8000), 0x984),
    b(M0, Axis(0x1), 0xA38),
    b(M0, Axis(0x1), 0xA44),
    b(M0, Axis(0x3), 0xA50),
    b(M0, Axis(0x3), 0xA5C),
    b(M0, Button(0x1000), 0xA14),
    b(M0, Button(0x4000), 0xA20),
    b(M1, Button(0x20), 0xBDC),
    b(M1, Button(0x10), 0xBE8),
    b(M1, Button(0x80), 0xC00),
    b(M1, Button(0x40), 0xC0C),
    b(M2, Axis(0x0), 0xA80),
    b(M2, Axis(0x0), 0xA8C),
    b(M2, Axis(0x1), 0xA98),
    b(M2, Axis(0x1), 0xAA4),
    b(M2, Axis(0x2), 0xAB0),
    b(M2, Axis(0x2), 0xABC),
    b(M2, Axis(0x3), 0xAC8),
    b(M2, Axis(0x3), 0xAD4),
    b(M2, Button(0x10), 0xB1C),
    b(M2, Button(0x400), 0xB70),
    b(M2, Button(0x2000), 0xB88),
    b(M2, Button(0x8000), 0xB94),
    b(M2, Button(0x200), 0xBA0),
];

const PAD_ANALOG_TRIGGERS: &[Binding] = &[
    b(M0, Analog(0xA), 0x960),
    b(M0, Analog(0xB), 0x948),
    b(M0, Analog(0xB), 0x954),
    b(M0, Button(0x4), 0xA2C),
    b(M0, Button(0x8), 0xA68),
    b(M0, Button(0x20), 0xA74),
    b(M1, Analog(0xB), 0xBF4),
    b(M1, Analog(0xB), 0xD14),
    b(M2, Analog(0xB), 0xAF8),
    b(M2, Analog(0xA), 0xB04),
    b(M2, Button(0x8), 0xB28),
    b(M2, Button(0x40), 0xB34),
    b(M2, Button(0x4), 0xAE0),
    b(M2, Button(0x80), 0xAEC),
    b(M2, Button(0x80), 0xB10),
    b(M2, Button(0x20), 0xB7C),
    b(M2, Analog(0xA), 0xB40),
    b(M2, Analog(0xB), 0xB4C),
    b(M2, Analog(0xB), 0xBAC),
    b(M2, Analog(0xA), 0xBB8),
    b(M2, Button(0x4), 0xBC4),
];

const PAD_BUTTON_TRIGGERS: &[Binding] = &[
    b(M0, Button(0x8), 0x960),
    b(M0, Button(0x4), 0x948),
    b(M0, Button(0x20), 0x954),
    b(M0, Button(0x1), 0xA2C),
    b(M0, Button(0x2), 0xA68),
    b(M1, Button(0x4), 0xBF4),
    b(M1, Button(0x20), 0xD14),
    b(M2, Button(0x40), 0xAF8),
    b(M2, Button(0x80), 0xB04),
    b(M2, Button(0x8), 0xB28),
    b(M2, Button(0x8), 0xB34),
    b(M2, Button(0x4), 0xAE0),
    b(M2, Button(0x4), 0xAEC),
    b(M2, Button(0x1), 0xB10),
    b(M2, Button(0x20), 0xB7C),
    b(M2, Button(0x40), 0xB40),
    b(M2, Button(0x80), 0xB4C),
    b(M2, Button(0x40), 0xBAC),
    b(M2, Button(0x80), 0xBB8),
    b(M2, Analog(0xA), 0xBC4),
];

const TAIL: &[Binding] = &[
    b(M3, Button(0x800), 0xCA8),
    b(M3, Button(0x1000), 0xC24),
    b(M3, Button(0x4000), 0xC18),
    b(M3, Button(0x8000), 0xC30),
    b(M3, Button(0x2000), 0xC3C),
    b(M3, Axis(0x0), 0xC78),
    b(M3, Axis(0x1), 0xC84),
    b(M3, Axis(0x2), 0xC90),
    b(M3, Axis(0x3), 0xC9C),
    b(M3, Button(0x40), 0xCB4),
    b(M3, Button(0x20), 0xCC0),
    b(M3, Button(0x80), 0xCCC),
    b(M3, Button(0x10), 0xCD8),
    b(M3, Button(0x4), 0xCE4),
    b(M3, Button(0x8), 0xCF0),
    b(M3, Button(0x1), 0xCFC),
    b(M3, Button(0x2), 0xD08),
];

pub const SETUP_WORDS: [u32; 2] = [0x40, 0x20];

pub fn bindings(controller: bool, setting: u32) -> Vec<Binding> {
    let mut v = vec![];
    let last = if !controller {
        v.extend_from_slice(NO_PAD);
        Button(0x2)
    } else {
        v.extend_from_slice(PAD);
        if setting == 0 {
            v.extend_from_slice(PAD_ANALOG_TRIGGERS);
            Button(0x8)
        } else {
            v.extend_from_slice(PAD_BUTTON_TRIGGERS);
            Analog(0xB)
        }
    };
    v.push(b(M2, last, 0xBD0));
    v.extend_from_slice(TAIL);
    v
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IoValue {
    pub invert: u8,
    pub stamp: u8,
    pub value: u8,
    pub last: u8,
    pub history_at: u8,
    pub history: Option<Vec<(u8, u32)>>,
}

impl IoValue {
    pub fn get(&self) -> u8 {
        self.value ^ self.invert
    }

    pub fn stick(&self) -> f32 {
        let v = ((self.get() as f32 - f32::from_bits(0x42FF_0000)) * f32::from_bits(0x3C00_8081)).clamp(-1.0, 1.0);
        crate::pad::dead_zone(v, READ_DEAD_ZONE)
    }

    pub fn stick_int(&self) -> i32 {
        (self.stick() * f32::from_bits(0x42FF_0000)) as i32
    }

    pub fn button(&self) -> f32 {
        crate::pad::dead_zone(self.get() as f32 * f32::from_bits(0x3B80_8081), READ_DEAD_ZONE)
    }
}

pub const READ_DEAD_ZONE: f32 = 0.25;

pub const AXIS_PAIRS: [(u32, u32); 8] =
    [(0x9A8, 0x9B4), (0x9C0, 0x9CC), (0x9D8, 0x9E4), (0x9F0, 0x9FC), (0xA80, 0xA8C), (0xA98, 0xAA4), (0xAB0, 0xABC), (0xAC8, 0xAD4)];

pub fn pick(pair: (u32, u32), first: &IoValue) -> u32 {
    if first.stick_int() < 0 { pair.0 } else { pair.1 }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RageInput {
    pub pads: [crate::pad::RagePad; 4],
    pub keys: [u8; 256],
    pub mouse_move: [i32; 2],
    pub mouse_scale: [f32; 2],
    pub mouse_axis: [i32; 2],
    pub mouse_buttons: u32,
    pub wheel_valid: bool,
    pub wheel: i32,
    pub pass: u8,
}

impl Default for RageInput {
    fn default() -> Self {
        RageInput { pads: Default::default(), keys: [0; 256], mouse_move: [0; 2], mouse_scale: [0.0; 2], mouse_axis: [0; 2],
                    mouse_buttons: 0, wheel_valid: false, wheel: 0, pass: 0 }
    }
}

fn stick(p: &crate::pad::RagePad, axis: usize) -> f32 {
    ((p.sticks[axis] as f32 - f32::from_bits(0x42FF_0000)) * f32::from_bits(0x3C01_0204)).clamp(-1.0, 1.0)
}

fn pair(first: bool, second: bool) -> u8 {
    match (second, first) {
        (false, true) => 0,
        (true, false) => 0xFF,
        _ => 0x80,
    }
}

fn source_value(kind: u32, index: u32, pad: &crate::pad::RagePad, input: &RageInput) -> u8 {
    const NEAR: f32 = f32::from_bits(0x3F11_EB85);
    const FAR: f32 = f32::from_bits(0x3F4C_CCCD);
    let on = |b: bool| if b { 0xFF } else { 0 };
    match kind {
        0 => on(input.keys[index as usize & 0xFF] != 0),
        1 => {
            let k = match index { 0 => 0, 1 => 1, _ => return 0 };
            ((input.mouse_move[k] as f32 * input.mouse_scale[k]) as f64 * 255.0) as i64 as u8
        }
        2 => match index { 0 => (input.mouse_axis[0] + 0x80) as u8, 1 => (input.mouse_axis[1] + 0x80) as u8, _ => 0 },
        3 => on(input.mouse_buttons & index == index),
        4 => on(input.mouse_buttons & index != 0),
        5 if input.wheel_valid => match index { 0 => on(input.wheel >= 1), 1 => on(input.wheel < 0), _ => 0 },
        6 => on(pad.buttons & index == index),
        7 => on(pad.buttons & index != 0),
        8 => pad.pressure.get(index as usize).copied().unwrap_or(0),
        9 => {
            let (a0, a1, a2, a3) = (stick(pad, 0), stick(pad, 1), stick(pad, 2), stick(pad, 3));
            match index {
                0..=3 => pad.sticks[index as usize],
                4 => on(a1 < -FAR && a0.abs() < NEAR),
                5 => on(a1 > FAR && a0.abs() < NEAR),
                6 => on(a0 < -FAR && a1.abs() < NEAR),
                7 => on(a0 > FAR && a1.abs() < NEAR),
                8 => on(a1 < -NEAR && a0 > NEAR),
                9 => on(a1 < -NEAR && a0 < -NEAR),
                10 => on(a1 > NEAR && a0 > NEAR),
                11 => on(a1 > NEAR && a0 < -NEAR),
                12 => on(a3 < -FAR && a2.abs() < NEAR),
                13 => on(a3 > FAR && a2.abs() < NEAR),
                14 => on(a2 < -FAR && a3.abs() < NEAR),
                15 => on(a2 > FAR && a3.abs() < NEAR),
                16 => on(a3 < -NEAR && a2 > NEAR),
                17 => on(a3 < -NEAR && a2 < -NEAR),
                18 => on(a3 > NEAR && a2 > NEAR),
                19 => on(a3 > NEAR && a2 < -NEAR),
                0x14 => pair(pad.buttons & 0xA000 == 0x8000, pad.buttons & 0xA000 == 0x2000),
                0x15 => pair(pad.buttons & 0x5000 == 0x1000, pad.buttons & 0x5000 == 0x4000),
                _ => 0,
            }
        }
        14 => pair(input.keys[index as usize & 0xFF] != 0, input.keys[(index >> 8) as usize & 0xFF] != 0),
        15 => {
            let bit = |n: u32| 1u32.checked_shl(n & 0x3F).unwrap_or(0);
            pair(pad.buttons & bit(index) != 0, pad.buttons & bit(index >> 8) != 0)
        }
        _ => 0,
    }
}

fn rest_value(kind: u32, index: u32) -> u8 {
    match kind {
        1 | 2 | 14 | 15 => 0x80,
        9 if matches!(index & 0xFF_FFFF, 0..=3 | 0x14 | 0x15) => 0x80,
        _ => 0,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IoMapper {
    pub default_pad: u32,
    pub bindings: Vec<(u32, u32, usize)>,
}

impl IoMapper {
    pub fn map(&mut self, kind: u32, index: u32, control: usize, pad: i32) {
        self.bindings.push((kind, ((pad as u32) << 24) | index, control));
    }

    pub fn reset(&self, values: &mut [IoValue]) {
        for &(kind, code, control) in &self.bindings {
            let v = &mut values[control];
            v.value = rest_value(kind, code & 0xFF_FFFF);
            let at = v.history_at as usize;
            if let Some(h) = v.history.as_mut() {
                h[at].0 = v.value;
            }
        }
    }

    pub fn update(&self, input: &mut RageInput, values: &mut [IoValue], time: u32) {
        input.pass = input.pass.wrapping_add(1);
        for &(_, _, control) in &self.bindings {
            let v = &mut values[control];
            if v.stamp != input.pass {
                v.last = v.value;
                v.value = 0;
                if let Some(h) = v.history.as_mut() {
                    let next = v.history_at.wrapping_add(1);
                    v.history_at = if next < 0x40 { next } else { 0 };
                    h[v.history_at as usize] = (0, time);
                }
                v.stamp = input.pass;
            }
        }
        for &(kind, code, control) in &self.bindings {
            let pad = if code >> 24 == 0xFF { self.default_pad } else { code >> 24 };
            let s = source_value(kind, code & 0xFF_FFFF, &input.pads[pad as usize], input);
            let v = &mut values[control];
            if v.value ^ v.invert < s {
                v.value = s;
                let at = v.history_at as usize;
                if let Some(h) = v.history.as_mut() {
                    h[at].0 = s;
                }
            }
        }
    }
}

pub const CONTROLS: usize = 87;

pub const fn index(offset: u32) -> usize {
    ((offset - 0x918) / 12) as usize
}

const KEEP_HISTORY: [u32; 19] = [0xBDC, 0xBE8, 0xBF4, 0xD14, 0xC00, 0xC0C, 0x960, 0x9D8, 0x9E4, 0x9A8, 0x9B4, 0x9C0, 0x9CC, 0xB1C,
                                 0xB88, 0xB94, 0xBA0, 0xA20, 0xB10];

const RESET_TABLES: [&[usize]; 3] = [
    &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14,
      0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D],
    &[0x3B, 0x3C, 0x3D, 0x55, 0x3E, 0x3F],
    &[0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x30, 0x31, 0x32, 0x33, 0x34,
      0x35, 0x36, 0x37, 0x38, 0x39, 0x3A],
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Controls {
    pub mappers: [IoMapper; 4],
    pub values: Vec<IoValue>,
    pub flag_d50: u8,
    pub pad: u32,
    pub setup_words: [u32; 2],
    pub last_used: u32,
    pub sticks: [i32; 4],
}

impl Default for Controls {
    fn default() -> Self {
        Controls { mappers: Default::default(), values: vec![IoValue::default(); CONTROLS], flag_d50: 0, pad: 0, setup_words: [0; 2],
                   last_used: 0, sticks: [0; 4] }
    }
}

impl Controls {
    pub fn set_up(&mut self, controller: bool, setting: u32) {
        for m in &mut self.mappers {
            m.reset(&mut self.values);
            m.bindings.clear();
        }
        for b in bindings(controller, setting) {
            let k = MAPPERS.iter().position(|&m| m == b.mapper).unwrap();
            let (kind, n) = b.source.code();
            self.mappers[k].map(kind, n, index(b.control), -1);
        }
        self.setup_words = SETUP_WORDS;
    }

    pub fn set_up_for_pad(&mut self, pad: u32, now: u32, controller: bool, setting: u32) {
        self.pad = pad;
        self.sticks = [0; 4];
        for m in &mut self.mappers {
            m.default_pad = pad;
        }
        self.last_used = now;
        self.set_up(controller, setting);
        for c in KEEP_HISTORY {
            let v = &mut self.values[index(c)];
            if v.history.is_none() {
                v.history = Some(vec![(0, 0); 64]);
            }
        }
    }

    pub fn stick_values(&self) -> [i32; 4] {
        [0xC78, 0xC84, 0xC9C, 0xC90].map(|c| self.values[index(c)].stick_int())
    }

    pub fn update(&mut self, player: [i32; 4], input: &mut RageInput, now: u32, paused: bool) {
        self.sticks = player;
        for (k, m) in self.mappers.iter().enumerate() {
            if k == 3 || !paused {
                m.update(input, &mut self.values, now);
            }
        }
        self.flag_d50 = 0;
        for (i, v) in self.values.iter().enumerate() {
            if (0x40..=0x54).contains(&i) {
                continue;
            }
            let in_use = match i {
                0x0C..=0x25 if !matches!(i, 0x14..=0x17 | 0x1C | 0x1D) => v.stick_int() != 0,
                _ => v.get() >= 0x80,
            };
            if in_use {
                self.last_used = now;
            }
        }
    }

    pub fn reset(&mut self, now: u32) {
        for (m, table) in self.mappers.iter().zip(RESET_TABLES) {
            for &c in table {
                let kind = m.bindings.iter().find(|b| b.2 == c).map_or(m.default_pad, |b| b.0);
                let rest = if matches!(kind, 1 | 9 | 11 | 14 | 15) { 0x7F } else { 0 };
                let v = &mut self.values[c];
                v.value = rest;
                v.last = rest;
                if let Some(h) = v.history.as_mut() {
                    h.fill((rest, now));
                }
            }
        }
    }
}

