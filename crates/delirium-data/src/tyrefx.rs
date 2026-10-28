use crate::rng::Rand;
use crate::tyre::flags;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fire {
    pub burning: bool,
    pub time: f32,
}

impl Fire {
    pub fn put_out(&mut self) {
        if self.burning {
            *self = Fire::default();
        }
    }
}

pub fn ignite(surface: f32, fire: &mut Fire, proof: bool) {
    if !fire.burning && surface > 0.0 && !proof {
        fire.burning = true;
    }
}

pub fn heat(temp: f32, tyre_flags: u32, surface: f32, dt: f32, rand: &mut Rand) -> f32 {
    if tyre_flags & flags::HEATS() != 0 && surface > 0.0 {
        (rand.float_in_range(0.5, 1.5) * dt).mul_add(3.0, temp)
    } else {
        temp
    }
}

pub fn cool(temp: f32, dt: f32) -> f32 {
    let k: f32 = if temp > 30.0 { 50.0 } else { 6.0 };
    -k.mul_add(dt, -temp)
}

pub fn hold(temp: f32) -> f32 {
    let t = if temp >= 0.0 { temp } else { 0.0 };
    if t - 70.0 >= 0.0 { 70.0 } else { t }
}

pub fn air(temp: f32, dt: f32) -> f32 {
    let t = cool(temp, dt);
    if -t >= 0.0 { 0.0 } else { t }
}

#[allow(clippy::too_many_arguments, clippy::neg_cmp_op_on_partial_ord)]
pub fn burst(surface: &mut f32, fire: &mut Fire, temp: f32, proof: bool, doused: bool, dt: f32, roll: impl FnOnce() -> f32) -> bool {
    if !fire.burning && temp < 70.0 || !(*surface > 0.0) || proof {
        return false;
    }
    if fire.burning {
        if doused {
            fire.burning = false;
        }
        fire.time += dt;
    }
    if fire.time < 10.0 && temp < 70.0 || roll() >= f32::from_bits(0x3C23_D70A) {
        return false;
    }
    *surface = 0.0;
    *fire = Fire::default();
    true
}
