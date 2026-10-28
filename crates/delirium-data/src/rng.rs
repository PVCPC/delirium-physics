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
        self.seed = self.seed.wrapping_mul(crate::kv!(872 int)).wrapping_add(crate::kv!(873 int));
        (self.seed >> crate::kv!(51 int)) & crate::kv!(561 int)
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
        ScriptRand { low: if seed == 0 { 1 } else { seed }, carry: seed.rotate_left(crate::kv!(51 int)) ^ seed }
    }

    fn step(&mut self) -> u32 {
        let t = u64::from(self.low) * crate::kv!(184 int) + u64::from(self.carry);
        self.low = t as u32;
        self.carry = (t >> crate::kv!(7 int)) as u32;
        self.low
    }

    pub fn int(&mut self) -> i32 {
        (self.step() & crate::kv!(561 int)) as i32
    }

    pub fn float(&mut self) -> f32 {
        (self.step() & crate::kv!(182 int)) as f32 * crate::kv!(738 f32)
    }

    pub fn int_in_range(&mut self, min: i32, max: i32) -> i32 {
        let bits = i64::from(self.step() & crate::kv!(527 int));
        let range = i64::from(max.wrapping_sub(min));
        ((bits.wrapping_mul(range) as u64 >> crate::kv!(454 int)) as u32 as i32).wrapping_add(min)
    }

    pub fn float_in_range(&mut self, min: f32, max: f32) -> f32 {
        let unit = (self.step() & crate::kv!(182 int)) as f32 * crate::kv!(738 f32);
        unit.mul_add(max - min, min)
    }
}

pub fn float_from_draw(draw: u32, min: f32, max: f32) -> f32 {
    let unit = draw as f32 * crate::kv!(519 f32);
    unit.mul_add(max - min, min)
}

pub fn int_from_draw(draw: u32, min: i32, max: i32) -> i32 {
    let unit = (draw & crate::kv!(50 int)) as f32 * crate::kv!(874 f32);
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
        AudioRand { value: 1, carry: crate::kv!(875 int) }
    }
}

impl AudioRand {
    pub fn int_in_range(&mut self, min: i32, max: i32) -> i32 {
        let range = max.wrapping_sub(min).wrapping_add(1);
        assert_ne!(range, 0, "audio random integer range has zero wrapping width");
        let x = (self.value as u64).wrapping_mul(crate::kv!(184 int)).wrapping_add(self.carry as u64);
        (self.value, self.carry) = (x as u32, (x >> crate::kv!(7 int)) as u32);
        ((self.value & crate::kv!(527 int)) as i32).wrapping_rem(range).wrapping_add(min)
    }

    pub fn float_in_range(&mut self, min: f32, max: f32) -> f32 {
        let x = (self.value as u64).wrapping_mul(crate::kv!(184 int)).wrapping_add(self.carry as u64);
        (self.value, self.carry) = (x as u32, (x >> crate::kv!(7 int)) as u32);
        ((max - min) * (self.value & crate::kv!(182 int)) as f32).mul_add(1.0 / crate::kv!(526), min)
    }
}
