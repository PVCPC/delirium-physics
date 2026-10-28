use crate::route::Point;

#[allow(non_snake_case)]
#[inline]
pub fn MAX() -> i32 { crate::kv!(277 int) }
#[allow(non_snake_case)]
#[inline]
pub fn FRAME() -> f32 { 1.0 / crate::kv!(80) }
#[allow(non_snake_case)]
#[inline]
fn NEAR() -> f32 { crate::kv!(763) }
#[allow(non_snake_case)]
#[inline]
fn NEAR_WIDE() -> f32 { crate::kv!(182) }

#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    pub walking: bool,
    pub special: bool,
    pub simulated: bool,
    pub wide: bool,
    pub pos: Point,
    pub moved: bool,
}

#[allow(non_snake_case)]
#[inline]
pub fn FULL() -> f32 { crate::kv!(112) }
#[allow(non_snake_case)]
#[inline]
pub fn NO_MOVER_TRACK() -> u32 { crate::kv!(798 int) }
#[allow(non_snake_case)]
#[inline]
pub fn MOVER_BITS() -> u32 { crate::kv!(890 int) }

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anim {
    pub clip: bool,
    pub layer: i32,
    pub weight: f32,
    pub flags: u32,
    pub mover_track: bool,
}

pub fn anim_moves(anims: &[Anim]) -> bool {
    anims.iter().filter(|a| a.clip && a.layer > 0).any(|a| {
        if a.weight < FULL() {
            return false;
        }
        if a.flags & NO_MOVER_TRACK() == 0 && a.mover_track {
            let mask = (a.flags & crate::kv!(194 int)) as u8;
            if mask == 0 || crate::animblend::mask_passes(mask, 0) {
                return true;
            }
        }
        a.flags & MOVER_BITS() != 0
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StaticCount {
    count: i32,
    since: u32,
    sample: Point,
    owed: f32,
}

impl StaticCount {
    pub fn count(&self) -> i32 {
        self.count
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }

    pub fn update(&mut self, dt: f32, look: &Look) {
        self.owed += dt;
        while self.owed >= FRAME() {
            self.owed -= FRAME();
            self.step(look);
        }
    }

    pub fn step(&mut self, look: &Look) {
        if !(look.walking || look.special) {
            self.since = tick(self.since);
            self.count = 0;
            return;
        }
        if !look.special && look.moved {
            self.count = 0;
            return;
        }
        if look.simulated {
            if self.since > crate::kv!(1 int) {
                self.sample = look.pos;
            }
            self.since = 0;
        } else {
            self.since = tick(self.since);
        }
        if self.since > crate::kv!(1 int) {
            self.count = 0;
            return;
        }
        let (dx, dy) = (look.pos[0] - self.sample[0], look.pos[1] - self.sample[1]);
        let d2 = dx.mul_add(dx, dy * dy);
        if d2 < if look.wide { NEAR_WIDE() } else { NEAR() } {
            self.count += 1;
        } else {
            self.count = 0;
            self.sample = look.pos;
        }
    }

    pub fn state(&self) -> (i32, u32, Point) {
        (self.count, self.since, self.sample)
    }

    pub fn with_state(count: i32, since: u32, sample: Point) -> StaticCount {
        StaticCount { count, since, sample, owed: 0.0 }
    }
}

fn tick(n: u32) -> u32 {
    let n = n.wrapping_add(1);
    if n == u32::MAX { crate::kv!(1 int) } else { n }
}

