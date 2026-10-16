#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rand {
    pub seed: u32,
}

impl Default for Rand {
    fn default() -> Self {
        Rand { seed: 1 }
    }
}

impl Rand {
    pub fn new(seed: u32) -> Self {
        Rand { seed }
    }

    pub fn next(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        (self.seed >> 16) & 0x7FFF
    }

    pub fn float_in_range(&mut self, min: f32, max: f32) -> f32 {
        float_from_draw(self.next(), min, max)
    }

    pub fn int_in_range(&mut self, min: i32, max: i32) -> i32 {
        int_from_draw(self.next(), min, max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptRand {
    pub low: u32,
    pub carry: u32,
}

impl ScriptRand {
    pub fn at_startup(time_base: u32) -> Self {
        ScriptRand { low: if time_base == 0 { 1 } else { time_base }, carry: time_base }
    }

    pub fn seeded(seed: u32) -> Self {
        ScriptRand { low: if seed == 0 { 1 } else { seed }, carry: seed.rotate_left(16) ^ seed }
    }

    fn step(&mut self) -> u32 {
        let t = u64::from(self.low) * 0x5CDC_FAA7 + u64::from(self.carry);
        self.low = t as u32;
        self.carry = (t >> 32) as u32;
        self.low
    }

    pub fn int(&mut self) -> i32 {
        (self.step() & 0x7FFF) as i32
    }

    pub fn float(&mut self) -> f32 {
        (self.step() & 0x7F_FFFF) as f32 * f32::from_bits(0x3400_0000)
    }

    pub fn int_in_range(&mut self, min: i32, max: i32) -> i32 {
        let bits = i64::from(self.step() & 0x7FFF_FFFF);
        let range = i64::from(max.wrapping_sub(min));
        ((bits.wrapping_mul(range) as u64 >> 31) as u32 as i32).wrapping_add(min)
    }

    pub fn float_in_range(&mut self, min: f32, max: f32) -> f32 {
        let unit = (self.step() & 0x7F_FFFF) as f32 * f32::from_bits(0x3400_0000);
        unit.mul_add(max - min, min)
    }
}

pub fn float_from_draw(draw: u32, min: f32, max: f32) -> f32 {
    let unit = draw as f32 * f32::from_bits(0x3800_0100);
    unit.mul_add(max - min, min)
}

pub fn int_from_draw(draw: u32, min: i32, max: i32) -> i32 {
    let unit = (draw & 0xFFFF) as f32 * f32::from_bits(0x3800_0000);
    let scaled = unit * max.wrapping_sub(min) as f32;
    (scaled as i32).wrapping_add(min)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioRand {
    pub value: u32,
    pub carry: u32,
}

impl Default for AudioRand {
    fn default() -> Self {
        AudioRand { value: 1, carry: 0x10001 }
    }
}

impl AudioRand {
    pub fn int_in_range(&mut self, min: i32, max: i32) -> i32 {
        let x = (self.value as u64).wrapping_mul(0x5CDC_FAA7).wrapping_add(self.carry as u64);
        (self.value, self.carry) = (x as u32, (x >> 32) as u32);
        let range = max.wrapping_sub(min).wrapping_add(1);
        if range == 0 {
            return min;
        }
        ((self.value & 0x7FFF_FFFF) as i32).wrapping_rem(range).wrapping_add(min)
    }

    pub fn float_in_range(&mut self, min: f32, max: f32) -> f32 {
        let x = (self.value as u64).wrapping_mul(0x5CDC_FAA7).wrapping_add(self.carry as u64);
        (self.value, self.carry) = (x as u32, (x >> 32) as u32);
        ((max - min) * (self.value & 0x7F_FFFF) as f32).mul_add(1.0 / 8_388_608.0, min)
    }
}
