use crate::vehicles::Handling;

#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    pub throttle: f32,
    pub brake: f32,
    pub handbrake: bool,
    pub engine_off: bool,
    pub bike: bool,
    pub flags_f80: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transmission {
    pub gear: i16,
    flags: u16,
    pub revs: f32,
    pub clutch: f32,
    pub throttle: f32,
    last_shift_ms: u32,
}

impl Default for Transmission {
    fn default() -> Self {
        Transmission { gear: 1, flags: 0, revs: 0.2, clutch: 1.0, throttle: 0.0, last_shift_ms: 0 }
    }
}

const IDLE_REVS: f32 = 0.2;

fn sel(c: f32, a: f32, b: f32) -> f32 {
    if c >= 0.0 { a } else { b }
}

impl Transmission {
    #[allow(clippy::too_many_arguments)]
    pub fn process(&mut self, h: &Handling, c: Controls, wheel_speeds: &[f32], forward_speed: f32, grounded: usize,
                   dt: f32, now_ms: u32) -> f32 {
        if c.engine_off {
            (self.revs, self.clutch, self.throttle, self.gear) = (0.0, 0.0, 0.0, 1);
            return 0.0;
        }
        let n = wheel_speeds.len();
        let mean = wheel_speeds.iter().fold(0.0f32, |a, &w| a + w) / n as f32;
        let forward = if mean > 0.0 { sel(-forward_speed, 0.0, forward_speed) } else { sel(-forward_speed, forward_speed, 0.0) };
        self.gear_change(h, c, mean, forward, n, grounded, dt, now_ms);
        self.drive_force(h, c, mean, forward, forward_speed, dt, n)
    }

    fn ratio(&self, h: &Handling, gear: i16) -> f32 {
        h.gear_ratios.get(gear.max(0) as usize).copied().unwrap_or(1.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn gear_change(&mut self, h: &Handling, c: Controls, s1: f32, s2: f32, drive_wheels: usize, grounded: usize,
                   dt: f32, now_ms: u32) {
        let thr = c.throttle;
        let load = c.brake.abs().mul_add(0.9, thr.abs());
        let f = sel(1.0 - load, load, 1.0).mul_add(0.3, 0.7);
        if thr.abs() < 0.7 {
            self.flags &= !4;
        }
        let locked = |bit: u16, ms: u32| {
            self.flags & bit != 0 && self.flags & 4 != 0 && now_ms < self.last_shift_ms.wrapping_add(ms)
        };
        let (can_up, can_down) = (!locked(2, 2000), !locked(1, 3000));
        let d = s1 - s2;
        let (slow, fast) = if s1 > 0.0 { (sel(d, s2, s1), sel(d, s1, s2)) } else { (sel(d, s1, s2), sel(d, s2, s1)) };
        if self.clutch < 1.0 {
            let rate: f32 = if h.drive_inertia < 1.0 { 0.6 } else { 1.3 };
            self.clutch = rate.mul_add(dt, self.clutch);
            if self.clutch > 1.0 {
                self.clutch = 1.0;
            }
        }
        let mfv = h.max_flat_velocity;
        if thr.abs() < 0.01 && self.ratio(h, 1) / mfv * s1 < 0.2 {
            self.clutch = 0.1;
            return;
        }
        if thr < 0.0 && self.ratio(h, 0) / mfv * slow > -0.5 {
            if self.gear != 0 {
                (self.clutch, self.gear) = (0.1, 0);
            }
            return;
        }
        if thr > 0.0 && self.gear == 0 && self.ratio(h, 1) / mfv * slow < 0.5 {
            (self.clutch, self.gear) = (0.1, 1);
            return;
        }
        let per = 1.0 / mfv;
        let g = self.gear;
        let (to, flags) = if self.ratio(h, g) * per * slow > f * 0.93 {
            if g >= h.gears as i16 || thr <= 0.0 || grounded != drive_wheels || !can_up {
                return;
            }
            (g + 1, 5)
        } else {
            if g <= 1 || self.ratio(h, g - 1) * per * fast >= f * 0.75 || !can_down {
                return;
            }
            (g - 1, 6)
        };
        (self.gear, self.flags, self.clutch, self.last_shift_ms) = (to, flags, 0.1, now_ms);
    }

    #[allow(clippy::too_many_arguments)]
    fn drive_force(&mut self, h: &Handling, c: Controls, s1: f32, s2: f32, along: f32, dt: f32,
                   drive_wheels: usize) -> f32 {
        let mut thr = c.throttle;
        self.throttle = thr;
        if (thr < 0.0 && self.gear > 0) || (thr > 0.0 && self.gear == 0) {
            self.throttle = 0.0;
            return 0.0;
        }
        if !c.handbrake && self.clutch < 0.6 && self.gear > 1 {
            (self.throttle, thr) = (0.0, 0.0);
        }
        let clutch = if c.handbrake {
            0.0
        } else {
            let k = sel(1.0 - self.clutch, self.clutch, 1.0);
            sel(-k, 0.0, k)
        };
        let ratio = self.ratio(h, self.gear);
        let mut target = thr.abs().mul_add(1.0 - clutch, clutch * (ratio / h.max_flat_velocity * s1));
        let push = c.bike && thr < 0.0 && self.gear == 0;
        let mut pushed = 0.0;
        if push {
            if along > -2.0 {
                let d = along + 2.0;
                pushed = -sel(1.0 - d, d, 1.0);
            }
            (self.clutch, target) = (0.0, 0.0);
        }
        if target < IDLE_REVS {
            target = IDLE_REVS;
        }
        if self.revs < target {
            self.revs = h.drive_inertia.mul_add(dt, self.revs);
            if self.revs > target {
                self.revs = target;
            }
        } else if self.revs > target {
            self.revs = -h.drive_inertia.mul_add(dt, -self.revs);
            if self.revs < target {
                self.revs = target;
            }
        }
        if push {
            return pushed;
        }
        if self.revs > 1.0 {
            (self.throttle, self.revs) = (self.throttle * 0.0, 0.9);
            thr *= 0.99;
        } else if ratio / h.max_flat_velocity * s2 > 1.0 {
            thr = 0.0;
        }
        if self.gear < 2 && self.revs < 0.5 {
            let r = self.revs * 2.0;
            let r = sel(0.2 - r, 0.2, r);
            self.clutch = sel(self.clutch - r, r, self.clutch);
        }
        let k = clutch * 2.0;
        let force = thr.abs() * ratio * sel(1.0 - k, k, 1.0) * h.drive_force * (self.revs / target) / drive_wheels as f32;
        if c.flags_f80 & 0x0080_0040 != 0 {
            force * 1.5
        } else if c.flags_f80 & 0x0040_0020 != 0 {
            force * 1.2
        } else {
            force
        }
    }
}

