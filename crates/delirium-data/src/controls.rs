#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Button(u32),
    Analog(u32),
    Axis(u32),
}

impl Source {
    pub fn code(self) -> (u32, u32) {
        match self {
            Source::Button(b) => (crate::kv!(6 int), b),
            Source::Analog(i) => (crate::kv!(23 int), i),
            Source::Axis(i) => (crate::kv!(93 int), i),
        }
    }
}

pub type Mapper = u32;
pub const M0: Mapper = 0x000;
#[allow(non_snake_case)]
#[inline]
pub fn M1() -> &'static Mapper { static V: std::sync::OnceLock<Mapper> = std::sync::OnceLock::new(); let f = || crate::kv!(629 int); if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn M2() -> &'static Mapper { static V: std::sync::OnceLock<Mapper> = std::sync::OnceLock::new(); let f = || crate::kv!(630 int); if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn M3() -> &'static Mapper { static V: std::sync::OnceLock<Mapper> = std::sync::OnceLock::new(); let f = || crate::kv!(631 int); if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
pub fn MAPPERS() -> &'static [Mapper; 4] { static V: std::sync::OnceLock<[Mapper; 4]> = std::sync::OnceLock::new(); let f = || [M0, (*M1()), (*M2()), (*M3())]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

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

#[allow(non_snake_case)]
#[inline]
fn NO_PAD() -> &'static [Binding] { static V: std::sync::OnceLock<Box<[Binding]>> = std::sync::OnceLock::new(); let f = || Box::new([
    b(M0, Button(crate::kv!(172 int)), crate::kv!(632 int)),
    b(M0, Button(crate::kv!(251 int)), crate::kv!(633 int)),
    b(M0, Button(crate::kv!(249 int)), crate::kv!(634 int)),
    b(M0, Button(crate::kv!(7 int)), crate::kv!(635 int)),
    b(M0, Button(crate::kv!(1 int)), crate::kv!(636 int)),
    b(M0, Button(crate::kv!(51 int)), crate::kv!(637 int)),
    b(M0, Button(crate::kv!(23 int)), crate::kv!(638 int)),
    b(M0, Button(crate::kv!(1 int)), crate::kv!(639 int)),
    b(M0, Button(0x2), crate::kv!(640 int)),
    b(M0, Button(0x1), crate::kv!(641 int)),
    b(M0, Button(crate::kv!(174 int)), crate::kv!(642 int)),
    b(M0, Button(crate::kv!(175 int)), crate::kv!(643 int)),
    b(M0, Button(crate::kv!(23 int)), crate::kv!(644 int)),
    b(M0, Button(crate::kv!(7 int)), crate::kv!(645 int)),
    b(M0, Button(crate::kv!(173 int)), crate::kv!(646 int)),
    b(M0, Button(crate::kv!(647 int)), crate::kv!(648 int)),
    b(M0, Axis(0x1), crate::kv!(649 int)),
    b(M0, Axis(0x1), crate::kv!(650 int)),
    b(M0, Axis(crate::kv!(0 int)), crate::kv!(651 int)),
    b(M0, Axis(crate::kv!(0 int)), crate::kv!(652 int)),
    b(M0, Axis(0x0), crate::kv!(653 int)),
    b(M0, Axis(0x0), crate::kv!(654 int)),
    b(M0, Axis(0x1), crate::kv!(655 int)),
    b(M0, Axis(0x1), crate::kv!(656 int)),
    b(M0, Axis(0x2), crate::kv!(657 int)),
    b(M0, Axis(0x2), crate::kv!(658 int)),
    b(M0, Axis(crate::kv!(0 int)), crate::kv!(659 int)),
    b(M0, Axis(crate::kv!(0 int)), crate::kv!(660 int)),
    b((*M1()), Button(crate::kv!(7 int)), crate::kv!(661 int)),
    b((*M1()), Button(crate::kv!(51 int)), crate::kv!(662 int)),
    b((*M1()), Button(0x2), crate::kv!(663 int)),
    b(M0, Button(crate::kv!(1 int)), crate::kv!(664 int)),
    b((*M1()), Button(crate::kv!(249 int)), crate::kv!(665 int)),
    b((*M1()), Button(crate::kv!(251 int)), crate::kv!(666 int)),
    b((*M2()), Axis(0x0), crate::kv!(667 int)),
    b((*M2()), Axis(0x0), crate::kv!(668 int)),
    b((*M2()), Axis(0x1), crate::kv!(669 int)),
    b((*M2()), Axis(0x1), crate::kv!(670 int)),
    b((*M2()), Axis(0x2), crate::kv!(671 int)),
    b((*M2()), Axis(0x2), crate::kv!(672 int)),
    b((*M2()), Axis(crate::kv!(0 int)), crate::kv!(673 int)),
    b((*M2()), Axis(crate::kv!(0 int)), crate::kv!(674 int)),
    b((*M2()), Button(crate::kv!(1 int)), crate::kv!(675 int)),
    b((*M2()), Button(crate::kv!(1 int)), crate::kv!(676 int)),
    b((*M2()), Button(crate::kv!(51 int)), crate::kv!(677 int)),
    b((*M2()), Button(crate::kv!(249 int)), crate::kv!(678 int)),
    b((*M2()), Button(crate::kv!(23 int)), crate::kv!(679 int)),
    b((*M2()), Button(crate::kv!(7 int)), crate::kv!(680 int)),
    b((*M2()), Button(crate::kv!(175 int)), crate::kv!(681 int)),
    b((*M2()), Button(crate::kv!(49 int)), crate::kv!(682 int)),
    b((*M2()), Button(crate::kv!(683 int)), crate::kv!(684 int)),
    b((*M2()), Button(crate::kv!(174 int)), crate::kv!(685 int)),
    b((*M2()), Button(crate::kv!(251 int)), crate::kv!(686 int)),
    b((*M2()), Button(crate::kv!(249 int)), crate::kv!(687 int)),
    b((*M2()), Button(crate::kv!(251 int)), crate::kv!(688 int)),
    b((*M2()), Button(crate::kv!(249 int)), crate::kv!(39 int)),
    b((*M2()), Button(0x1), crate::kv!(689 int)),
]) as Box<[Binding]>; if crate::kv::open() { &**V.get_or_init(f) } else { Box::leak(f()) } }

#[allow(non_snake_case)]
#[inline]
fn PAD() -> &'static [Binding] { static V: std::sync::OnceLock<Box<[Binding]>> = std::sync::OnceLock::new(); let f = || Box::new([
    b(M0, Axis(0x0), crate::kv!(653 int)),
    b(M0, Axis(0x0), crate::kv!(654 int)),
    b(M0, Axis(0x1), crate::kv!(655 int)),
    b(M0, Axis(0x1), crate::kv!(656 int)),
    b(M0, Axis(0x2), crate::kv!(657 int)),
    b(M0, Axis(0x2), crate::kv!(658 int)),
    b(M0, Axis(crate::kv!(0 int)), crate::kv!(659 int)),
    b(M0, Axis(crate::kv!(0 int)), crate::kv!(660 int)),
    b(M0, Button(crate::kv!(172 int)), crate::kv!(632 int)),
    b(M0, Button(crate::kv!(251 int)), crate::kv!(633 int)),
    b(M0, Button(crate::kv!(249 int)), crate::kv!(634 int)),
    b(M0, Button(crate::kv!(51 int)), crate::kv!(637 int)),
    b(M0, Button(crate::kv!(174 int)), crate::kv!(642 int)),
    b(M0, Button(crate::kv!(175 int)), crate::kv!(643 int)),
    b(M0, Button(crate::kv!(49 int)), crate::kv!(640 int)),
    b(M0, Button(crate::kv!(683 int)), crate::kv!(641 int)),
    b(M0, Axis(0x1), crate::kv!(649 int)),
    b(M0, Axis(0x1), crate::kv!(650 int)),
    b(M0, Axis(crate::kv!(0 int)), crate::kv!(651 int)),
    b(M0, Axis(crate::kv!(0 int)), crate::kv!(652 int)),
    b(M0, Button(crate::kv!(173 int)), crate::kv!(646 int)),
    b(M0, Button(crate::kv!(647 int)), crate::kv!(648 int)),
    b((*M1()), Button(crate::kv!(7 int)), crate::kv!(661 int)),
    b((*M1()), Button(crate::kv!(51 int)), crate::kv!(662 int)),
    b((*M1()), Button(crate::kv!(249 int)), crate::kv!(665 int)),
    b((*M1()), Button(crate::kv!(251 int)), crate::kv!(666 int)),
    b((*M2()), Axis(0x0), crate::kv!(667 int)),
    b((*M2()), Axis(0x0), crate::kv!(668 int)),
    b((*M2()), Axis(0x1), crate::kv!(669 int)),
    b((*M2()), Axis(0x1), crate::kv!(670 int)),
    b((*M2()), Axis(0x2), crate::kv!(671 int)),
    b((*M2()), Axis(0x2), crate::kv!(672 int)),
    b((*M2()), Axis(crate::kv!(0 int)), crate::kv!(673 int)),
    b((*M2()), Axis(crate::kv!(0 int)), crate::kv!(674 int)),
    b((*M2()), Button(crate::kv!(51 int)), crate::kv!(677 int)),
    b((*M2()), Button(crate::kv!(175 int)), crate::kv!(681 int)),
    b((*M2()), Button(crate::kv!(49 int)), crate::kv!(682 int)),
    b((*M2()), Button(crate::kv!(683 int)), crate::kv!(684 int)),
    b((*M2()), Button(crate::kv!(174 int)), crate::kv!(685 int)),
]) as Box<[Binding]>; if crate::kv::open() { &**V.get_or_init(f) } else { Box::leak(f()) } }

#[allow(non_snake_case)]
#[inline]
fn PAD_ANALOG_TRIGGERS() -> &'static [Binding] { static V: std::sync::OnceLock<Box<[Binding]>> = std::sync::OnceLock::new(); let f = || Box::new([
    b(M0, Analog(crate::kv!(77 int)), crate::kv!(638 int)),
    b(M0, Analog(crate::kv!(102 int)), crate::kv!(635 int)),
    b(M0, Analog(crate::kv!(102 int)), crate::kv!(636 int)),
    b(M0, Button(crate::kv!(1 int)), crate::kv!(639 int)),
    b(M0, Button(crate::kv!(23 int)), crate::kv!(644 int)),
    b(M0, Button(crate::kv!(7 int)), crate::kv!(645 int)),
    b((*M1()), Analog(crate::kv!(102 int)), crate::kv!(663 int)),
    b((*M1()), Analog(crate::kv!(102 int)), crate::kv!(664 int)),
    b((*M2()), Analog(crate::kv!(102 int)), crate::kv!(686 int)),
    b((*M2()), Analog(crate::kv!(77 int)), crate::kv!(687 int)),
    b((*M2()), Button(crate::kv!(23 int)), crate::kv!(679 int)),
    b((*M2()), Button(crate::kv!(251 int)), crate::kv!(690 int)),
    b((*M2()), Button(crate::kv!(1 int)), crate::kv!(675 int)),
    b((*M2()), Button(crate::kv!(249 int)), crate::kv!(676 int)),
    b((*M2()), Button(crate::kv!(249 int)), crate::kv!(678 int)),
    b((*M2()), Button(crate::kv!(7 int)), crate::kv!(680 int)),
    b((*M2()), Analog(crate::kv!(77 int)), crate::kv!(691 int)),
    b((*M2()), Analog(crate::kv!(102 int)), crate::kv!(692 int)),
    b((*M2()), Analog(crate::kv!(102 int)), crate::kv!(688 int)),
    b((*M2()), Analog(crate::kv!(77 int)), crate::kv!(39 int)),
    b((*M2()), Button(crate::kv!(1 int)), crate::kv!(689 int)),
]) as Box<[Binding]>; if crate::kv::open() { &**V.get_or_init(f) } else { Box::leak(f()) } }

#[allow(non_snake_case)]
#[inline]
fn PAD_BUTTON_TRIGGERS() -> &'static [Binding] { static V: std::sync::OnceLock<Box<[Binding]>> = std::sync::OnceLock::new(); let f = || Box::new([
    b(M0, Button(crate::kv!(23 int)), crate::kv!(638 int)),
    b(M0, Button(crate::kv!(1 int)), crate::kv!(635 int)),
    b(M0, Button(crate::kv!(7 int)), crate::kv!(636 int)),
    b(M0, Button(0x1), crate::kv!(639 int)),
    b(M0, Button(0x2), crate::kv!(644 int)),
    b((*M1()), Button(crate::kv!(1 int)), crate::kv!(663 int)),
    b((*M1()), Button(crate::kv!(7 int)), crate::kv!(664 int)),
    b((*M2()), Button(crate::kv!(251 int)), crate::kv!(686 int)),
    b((*M2()), Button(crate::kv!(249 int)), crate::kv!(687 int)),
    b((*M2()), Button(crate::kv!(23 int)), crate::kv!(679 int)),
    b((*M2()), Button(crate::kv!(23 int)), crate::kv!(690 int)),
    b((*M2()), Button(crate::kv!(1 int)), crate::kv!(675 int)),
    b((*M2()), Button(crate::kv!(1 int)), crate::kv!(676 int)),
    b((*M2()), Button(0x1), crate::kv!(678 int)),
    b((*M2()), Button(crate::kv!(7 int)), crate::kv!(680 int)),
    b((*M2()), Button(crate::kv!(251 int)), crate::kv!(691 int)),
    b((*M2()), Button(crate::kv!(249 int)), crate::kv!(692 int)),
    b((*M2()), Button(crate::kv!(251 int)), crate::kv!(688 int)),
    b((*M2()), Button(crate::kv!(249 int)), crate::kv!(39 int)),
    b((*M2()), Analog(crate::kv!(77 int)), crate::kv!(689 int)),
]) as Box<[Binding]>; if crate::kv::open() { &**V.get_or_init(f) } else { Box::leak(f()) } }

#[allow(non_snake_case)]
#[inline]
fn TAIL() -> &'static [Binding] { static V: std::sync::OnceLock<Box<[Binding]>> = std::sync::OnceLock::new(); let f = || Box::new([
    b((*M3()), Button(crate::kv!(693 int)), crate::kv!(694 int)),
    b((*M3()), Button(crate::kv!(173 int)), crate::kv!(695 int)),
    b((*M3()), Button(crate::kv!(647 int)), crate::kv!(696 int)),
    b((*M3()), Button(crate::kv!(683 int)), crate::kv!(697 int)),
    b((*M3()), Button(crate::kv!(49 int)), crate::kv!(698 int)),
    b((*M3()), Axis(0x0), crate::kv!(699 int)),
    b((*M3()), Axis(0x1), crate::kv!(700 int)),
    b((*M3()), Axis(0x2), crate::kv!(701 int)),
    b((*M3()), Axis(crate::kv!(0 int)), crate::kv!(702 int)),
    b((*M3()), Button(crate::kv!(251 int)), crate::kv!(703 int)),
    b((*M3()), Button(crate::kv!(7 int)), crate::kv!(704 int)),
    b((*M3()), Button(crate::kv!(249 int)), crate::kv!(705 int)),
    b((*M3()), Button(crate::kv!(51 int)), crate::kv!(706 int)),
    b((*M3()), Button(crate::kv!(1 int)), crate::kv!(707 int)),
    b((*M3()), Button(crate::kv!(23 int)), crate::kv!(708 int)),
    b((*M3()), Button(0x1), crate::kv!(709 int)),
    b((*M3()), Button(0x2), crate::kv!(710 int)),
]) as Box<[Binding]>; if crate::kv::open() { &**V.get_or_init(f) } else { Box::leak(f()) } }

#[allow(non_snake_case)]
#[inline]
pub fn SETUP_WORDS() -> &'static [u32; 2] { static V: std::sync::OnceLock<[u32; 2]> = std::sync::OnceLock::new(); let f = || [crate::kv!(251 int), crate::kv!(7 int)]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

pub fn bindings(controller: bool, setting: u32) -> Vec<Binding> {
    let mut v = vec![];
    let last = if !controller {
        v.extend_from_slice(NO_PAD());
        Button(0x2)
    } else {
        v.extend_from_slice(PAD());
        if setting == 0 {
            v.extend_from_slice(PAD_ANALOG_TRIGGERS());
            Button(crate::kv!(23 int))
        } else {
            v.extend_from_slice(PAD_BUTTON_TRIGGERS());
            Analog(crate::kv!(102 int))
        }
    };
    v.push(b((*M2()), last, crate::kv!(711 int)));
    v.extend_from_slice(TAIL());
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
        let v = ((self.get() as f32 - crate::kv!(562 f32)) * crate::kv!(712 f32)).clamp(-1.0, 1.0);
        crate::pad::dead_zone(v, READ_DEAD_ZONE())
    }

    pub fn stick_int(&self) -> i32 {
        (self.stick() * crate::kv!(562 f32)) as i32
    }

    pub fn button(&self) -> f32 {
        crate::pad::dead_zone(self.get() as f32 * crate::kv!(713 f32), READ_DEAD_ZONE())
    }
}

#[allow(non_snake_case)]
#[inline]
pub fn READ_DEAD_ZONE() -> f32 { crate::kv!(17) }

#[allow(non_snake_case)]
#[inline]
pub fn AXIS_PAIRS() -> &'static [(u32, u32); 8] { static V: std::sync::OnceLock<[(u32, u32); 8]> = std::sync::OnceLock::new(); let f = || [(crate::kv!(653 int), crate::kv!(654 int)), (crate::kv!(655 int), crate::kv!(656 int)), (crate::kv!(657 int), crate::kv!(658 int)), (crate::kv!(659 int), crate::kv!(660 int)), (crate::kv!(667 int), crate::kv!(668 int)), (crate::kv!(669 int), crate::kv!(670 int)), (crate::kv!(671 int), crate::kv!(672 int)), (crate::kv!(673 int), crate::kv!(674 int))]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

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
    ((p.sticks[axis] as f32 - crate::kv!(562 f32)) * crate::kv!(714 f32)).clamp(-1.0, 1.0)
}

fn pair(first: bool, second: bool) -> u8 {
    match (second, first) {
        (false, true) => 0,
        (true, false) => crate::kv!(386 int),
        _ => crate::kv!(249 int),
    }
}

fn source_value(kind: u32, index: u32, pad: &crate::pad::RagePad, input: &RageInput) -> u8 {
    #[allow(non_snake_case)]
    #[inline]
    fn NEAR() -> f32 { crate::kv!(715 f32) }
    #[allow(non_snake_case)]
    #[inline]
    fn FAR() -> f32 { crate::kv!(716 f32) }
    let on = |b: bool| if b { crate::kv!(386 int) } else { 0 };
    match kind {
        0 => on(input.keys[index as usize & crate::kv!(386 int)] != 0),
        1 => {
            let k = match index { 0 => 0, 1 => 1, _ => return 0 };
            ((input.mouse_move[k] as f32 * input.mouse_scale[k]) as f64 * crate::kv!(717)) as i64 as u8
        }
        2 => match index { 0 => (input.mouse_axis[0] + crate::kv!(249 int)) as u8, 1 => (input.mouse_axis[1] + crate::kv!(249 int)) as u8, _ => 0 },
        3 => on(input.mouse_buttons & index == index),
        4 => on(input.mouse_buttons & index != 0),
        5 if input.wheel_valid => match index { 0 => on(input.wheel >= 1), 1 => on(input.wheel < 0), _ => 0 },
        6 => on(pad.buttons & index == index),
        7 => on(pad.buttons & index != 0),
        8 => pad.pressure.get(index as usize).copied().unwrap_or(0),
        9 => {
            let (a0, a1, a2, a3) = (stick(pad, 0), stick(pad, 1), stick(pad, 2), stick(pad, crate::kv!(0 int)));
            match index {
                0..=3 => pad.sticks[index as usize],
                4 => on(a1 < -FAR() && a0.abs() < NEAR()),
                5 => on(a1 > FAR() && a0.abs() < NEAR()),
                6 => on(a0 < -FAR() && a1.abs() < NEAR()),
                7 => on(a0 > FAR() && a1.abs() < NEAR()),
                8 => on(a1 < -NEAR() && a0 > NEAR()),
                9 => on(a1 < -NEAR() && a0 < -NEAR()),
                10 => on(a1 > NEAR() && a0 > NEAR()),
                11 => on(a1 > NEAR() && a0 < -NEAR()),
                12 => on(a3 < -FAR() && a2.abs() < NEAR()),
                13 => on(a3 > FAR() && a2.abs() < NEAR()),
                14 => on(a2 < -FAR() && a3.abs() < NEAR()),
                15 => on(a2 > FAR() && a3.abs() < NEAR()),
                16 => on(a3 < -NEAR() && a2 > NEAR()),
                17 => on(a3 < -NEAR() && a2 < -NEAR()),
                18 => on(a3 > NEAR() && a2 > NEAR()),
                19 => on(a3 > NEAR() && a2 < -NEAR()),
                0x14 => pair(pad.buttons & crate::kv!(718 int) == crate::kv!(683 int), pad.buttons & crate::kv!(718 int) == crate::kv!(49 int)),
                0x15 => pair(pad.buttons & crate::kv!(719 int) == crate::kv!(173 int), pad.buttons & crate::kv!(719 int) == crate::kv!(647 int)),
                _ => 0,
            }
        }
        14 => pair(input.keys[index as usize & crate::kv!(386 int)] != 0, input.keys[(index >> crate::kv!(23 int)) as usize & crate::kv!(386 int)] != 0),
        15 => {
            let bit = |n: u32| 1u32.checked_shl(n & crate::kv!(250 int)).unwrap_or(0);
            pair(pad.buttons & bit(index) != 0, pad.buttons & bit(index >> crate::kv!(23 int)) != 0)
        }
        _ => 0,
    }
}

fn rest_value(kind: u32, index: u32) -> u8 {
    match kind {
        1 | 2 | 14 | 15 => crate::kv!(249 int),
        9 if matches!(index & crate::kv!(720 int), 0..=3 | 0x14 | 0x15) => crate::kv!(249 int),
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
        self.bindings.push((kind, ((pad as u32) << crate::kv!(96 int)) | index, control));
    }

    pub fn reset(&self, values: &mut [IoValue]) {
        for &(kind, code, control) in &self.bindings {
            let v = &mut values[control];
            v.value = rest_value(kind, code & crate::kv!(720 int));
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
                    v.history_at = if next < crate::kv!(251 int) { next } else { 0 };
                    h[v.history_at as usize] = (0, time);
                }
                v.stamp = input.pass;
            }
        }
        for &(kind, code, control) in &self.bindings {
            let pad = if code >> crate::kv!(96 int) == crate::kv!(386 int) { self.default_pad } else { code >> crate::kv!(96 int) };
            let s = source_value(kind, code & crate::kv!(720 int), &input.pads[pad as usize], input);
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

#[allow(non_snake_case)]
#[inline]
pub fn CONTROLS() -> usize { crate::kv!(315 int) }

pub fn index(offset: u32) -> usize {
    ((offset - crate::kv!(632 int)) / crate::kv!(52 int)) as usize
}

#[allow(non_snake_case)]
#[inline]
fn KEEP_HISTORY() -> &'static [u32; 19] { static V: std::sync::OnceLock<[u32; 19]> = std::sync::OnceLock::new(); let f = || [crate::kv!(661 int), crate::kv!(662 int), crate::kv!(663 int), crate::kv!(664 int), crate::kv!(665 int), crate::kv!(666 int), crate::kv!(638 int), crate::kv!(657 int), crate::kv!(658 int), crate::kv!(653 int), crate::kv!(654 int), crate::kv!(655 int), crate::kv!(656 int), crate::kv!(677 int),
                                 crate::kv!(682 int), crate::kv!(684 int), crate::kv!(685 int), crate::kv!(648 int), crate::kv!(678 int)]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

#[allow(non_snake_case)]
#[inline]
fn RESET_TABLES() -> &'static [&'static [usize]; 3] { static V: std::sync::OnceLock<[&'static [usize]; 3]> = std::sync::OnceLock::new(); let f = || [
    Box::leak(Box::new([0x00, 0x01, 0x02, crate::kv!(0 int), crate::kv!(1 int), crate::kv!(2 int), crate::kv!(6 int), crate::kv!(22 int), crate::kv!(23 int), crate::kv!(93 int), crate::kv!(77 int), crate::kv!(102 int), crate::kv!(52 int), crate::kv!(103 int), crate::kv!(53 int), crate::kv!(191 int), crate::kv!(51 int), crate::kv!(99 int), crate::kv!(206 int), crate::kv!(192 int), crate::kv!(118 int),
      crate::kv!(100 int), crate::kv!(94 int), crate::kv!(95 int), crate::kv!(96 int), crate::kv!(97 int), crate::kv!(98 int), crate::kv!(328 int), crate::kv!(81 int), crate::kv!(474 int)])).as_slice(),
    Box::leak(Box::new([crate::kv!(392 int), crate::kv!(338 int), crate::kv!(482 int), crate::kv!(388 int), crate::kv!(301 int), crate::kv!(250 int)])).as_slice(),
    Box::leak(Box::new([crate::kv!(274 int), crate::kv!(454 int), crate::kv!(7 int), crate::kv!(310 int), crate::kv!(435 int), crate::kv!(307 int), crate::kv!(90 int), crate::kv!(292 int), crate::kv!(382 int), crate::kv!(418 int), crate::kv!(91 int), crate::kv!(345 int), crate::kv!(398 int), crate::kv!(415 int), crate::kv!(406 int), crate::kv!(464 int), crate::kv!(89 int), crate::kv!(451 int), crate::kv!(33 int), crate::kv!(445 int), crate::kv!(376 int),
      crate::kv!(283 int), crate::kv!(34 int), crate::kv!(346 int), crate::kv!(314 int), crate::kv!(308 int), crate::kv!(395 int)])).as_slice(),
]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

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
        Controls { mappers: Default::default(), values: vec![IoValue::default(); CONTROLS()], flag_d50: 0, pad: 0, setup_words: [0; 2],
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
            let k = (*MAPPERS()).iter().position(|&m| m == b.mapper).unwrap();
            let (kind, n) = b.source.code();
            self.mappers[k].map(kind, n, index(b.control), -1);
        }
        self.setup_words = (*SETUP_WORDS());
    }

    pub fn set_up_for_pad(&mut self, pad: u32, now: u32, controller: bool, setting: u32) {
        self.pad = pad;
        self.sticks = [0; 4];
        for m in &mut self.mappers {
            m.default_pad = pad;
        }
        self.last_used = now;
        self.set_up(controller, setting);
        for c in (*KEEP_HISTORY()) {
            let v = &mut self.values[index(c)];
            if v.history.is_none() {
                v.history = Some(vec![(0, 0); 64]);
            }
        }
    }

    pub fn stick_values(&self) -> [i32; 4] {
        [crate::kv!(699 int), crate::kv!(700 int), crate::kv!(702 int), crate::kv!(701 int)].map(|c| self.values[index(c)].stick_int())
    }

    pub fn update(&mut self, player: [i32; 4], input: &mut RageInput, now: u32, paused: bool) {
        self.sticks = player;
        for (k, m) in self.mappers.iter().enumerate() {
            if k == crate::kv!(0 int) || !paused {
                m.update(input, &mut self.values, now);
            }
        }
        self.flag_d50 = 0;
        for (i, v) in self.values.iter().enumerate() {
            if (crate::kv!(251 int)..=crate::kv!(460 int)).contains(&i) {
                continue;
            }
            let in_use = match i {
                0x0C..=0x25 if !matches!(i, 0x14..=0x17 | 0x1C | 0x1D) => v.stick_int() != 0,
                _ => v.get() >= crate::kv!(249 int),
            };
            if in_use {
                self.last_used = now;
            }
        }
    }

    pub fn reset(&mut self, now: u32) {
        for (m, table) in self.mappers.iter().zip((*RESET_TABLES())) {
            for &c in table {
                let kind = m.bindings.iter().find(|b| b.2 == c).map_or(m.default_pad, |b| b.0);
                let rest = if matches!(kind, 1 | 9 | 11 | 14 | 15) { crate::kv!(465 int) } else { 0 };
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

