#[allow(non_snake_case)]
#[inline]
pub fn STICK_DEAD_ZONE() -> f32 { crate::kv!(840 f32) }
#[allow(non_snake_case)]
#[inline]
fn HALF_RANGE() -> f32 { crate::kv!(562 f32) }
#[allow(non_snake_case)]
#[inline]
fn STICK_SCALE() -> f32 { crate::kv!(714 f32) }

pub fn dead_zone(v: f32, t: f32) -> f32 {
    if v > t {
        (v - t) / (1.0 - t)
    } else if v < -t {
        (v + t) / (1.0 - t)
    } else {
        0.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RagePad {
    pub buttons: u32,
    pub sticks: [u8; 4],
    pub pressure: [u8; 12],
    pub has_pressure: bool,
    pub motors_on: bool,
    pub motors_changed: bool,
    pub motor_speed: [u16; 2],
}

impl RagePad {
    pub fn set_motor(&mut self, v: f32, motor: usize) {
        let v = (v as f64).clamp(0.0, 1.0);
        self.motor_speed[motor] = (v * crate::kv!(841)) as i64 as u16;
        self.motors_changed = true;
    }

    pub fn set_motors_on(&mut self, on: bool) {
        self.motors_on = on;
        self.motors_changed = true;
    }

    pub fn stop_now(&mut self) {
        self.motor_speed = [0, 0];
    }
}

#[allow(non_snake_case)]
#[inline]
fn BUTTONS() -> &'static [(u32, Option<usize>); 16] { static V: std::sync::OnceLock<[(u32, Option<usize>); 16]> = std::sync::OnceLock::new(); let f = || [
    (crate::kv!(1 int), Some(crate::kv!(23 int))),
    (0x0001, Some(crate::kv!(77 int))),
    (crate::kv!(174 int), None),
    (crate::kv!(23 int), Some(crate::kv!(93 int))),
    (0x0002, Some(crate::kv!(102 int))),
    (crate::kv!(175 int), None),
    (crate::kv!(249 int), Some(crate::kv!(22 int))),
    (crate::kv!(51 int), Some(crate::kv!(1 int))),
    (crate::kv!(7 int), Some(crate::kv!(2 int))),
    (crate::kv!(251 int), Some(crate::kv!(6 int))),
    (crate::kv!(173 int), Some(0x02)),
    (crate::kv!(647 int), Some(crate::kv!(0 int))),
    (crate::kv!(683 int), Some(0x01)),
    (crate::kv!(49 int), Some(0x00)),
    (crate::kv!(693 int), None),
    (crate::kv!(172 int), None),
]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadState(pub [i32; 20]);

impl PadState {
    pub fn from_rage(p: &RagePad) -> PadState {
        let mut w = [0i32; 20];
        for (k, &b) in p.sticks.iter().enumerate() {
            w[k] = stick(b);
        }
        for (k, &(bit, pressure)) in (*BUTTONS()).iter().enumerate() {
            w[crate::kv!(1 int) + k] = match pressure {
                Some(i) if p.has_pressure => p.pressure[i] as i32,
                _ => if p.buttons & bit != 0 { crate::kv!(386 int) } else { 0 },
            };
        }
        PadState(w)
    }
}

fn stick(b: u8) -> i32 {
    let v = ((b as f32 - HALF_RANGE()) * STICK_SCALE()).clamp(-1.0, 1.0);
    (dead_zone(v, STICK_DEAD_ZONE()) * HALF_RANGE()).floor() as i32
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pad {
    pub now: PadState,
    pub last: PadState,
    pub rumble_start: u32,
    pub rumble_time: u32,
    pub rumble_strength: i32,
    pub hold_until: u32,
    pub hold_strength: i32,
    pub stopped: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShakeGate {
    pub vibration_on: bool,
    pub blocked: bool,
}

impl Pad {
    pub fn update(&mut self, mut rigid: Option<&mut RagePad>, now: u32, player_state_3: bool) {
        self.last = self.now;
        if let Some(p) = rigid.as_deref() {
            self.now = PadState::from_rage(p);
        }
        if self.rumble_time != 0 {
            let fifth = (self.rumble_strength as f64 * crate::kv!(57)) as i32;
            if player_state_3 && fifth > crate::kv!(77 int) {
                self.rumble_strength -= fifth;
                if let Some(p) = rigid.as_deref_mut() {
                    self.set_rumble(p, self.rumble_strength, 0, now);
                }
            } else if self.rumble_time < now.wrapping_sub(self.rumble_start) {
                self.stop_rumble(rigid, false);
            }
        }
        self.stopped = false;
    }

    pub fn set_rumble(&mut self, rigid: &mut RagePad, a: i32, b: i32, now: u32) {
        rigid.set_motor(0.0, 0);
        rigid.set_motor(0.0, 1);
        for (motor, s) in [(0, a), (1, b)] {
            if s > 0 {
                rigid.set_motor((s as f64 * (1.0 / crate::kv!(842))) as f32, motor);
                rigid.set_motors_on(true);
                self.rumble_start = now;
            }
        }
    }

    pub fn stop_rumble(&mut self, mut rigid: Option<&mut RagePad>, mark: bool) {
        if self.rumble_time != 0 || self.rumble_strength > 0 {
            if let Some(p) = rigid.as_deref_mut() {
                p.set_motors_on(false);
                p.set_motor(0.0, 0);
                p.set_motor(0.0, 1);
            }
            self.rumble_time = 0;
            self.rumble_strength = 0;
            self.rumble_start = 0;
            self.hold_strength = 1;
            self.hold_until = 0;
        }
        if mark && !self.stopped {
            if let Some(p) = rigid {
                p.stop_now();
                self.stopped = true;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn shake(&mut self, rigid: &mut RagePad, gate: ShakeGate, force: bool, time_a: u32, strength_a: i32, time_b: u32,
                 strength_b: i32, hold: u32, now: u32) {
        if !gate.vibration_on || (gate.blocked && !force) || self.stopped {
            return;
        }
        if strength_a == 0 && strength_b == 0 {
            self.stop_rumble(Some(rigid), false);
            return;
        }
        let top = if strength_a <= strength_b { strength_b } else { strength_a };
        if now < self.hold_until && top <= self.hold_strength {
            return;
        }
        let (time, strength) = if time_b <= time_a { (time_a, strength_a) } else { (time_b, strength_b) };
        if self.rumble_time < time {
            self.rumble_time = time;
            self.rumble_strength = strength;
        }
        self.hold_strength = top;
        self.hold_until = now.wrapping_add(hold);
        self.set_rumble(rigid, strength_a, strength_b, now);
    }
}

