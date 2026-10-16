pub const STICK_DEAD_ZONE: f32 = f32::from_bits(0x3E4C_CCCD);
const HALF_RANGE: f32 = f32::from_bits(0x42FF_0000);
const STICK_SCALE: f32 = f32::from_bits(0x3C01_0204);

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
        self.motor_speed[motor] = (v * 65535.0) as i64 as u16;
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

const BUTTONS: [(u32, Option<usize>); 16] = [
    (0x0004, Some(0x08)),
    (0x0001, Some(0x0A)),
    (0x0200, None),
    (0x0008, Some(0x09)),
    (0x0002, Some(0x0B)),
    (0x0400, None),
    (0x0080, Some(0x07)),
    (0x0010, Some(0x04)),
    (0x0020, Some(0x05)),
    (0x0040, Some(0x06)),
    (0x1000, Some(0x02)),
    (0x4000, Some(0x03)),
    (0x8000, Some(0x01)),
    (0x2000, Some(0x00)),
    (0x0800, None),
    (0x0100, None),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadState(pub [i32; 20]);

impl PadState {
    pub fn from_rage(p: &RagePad) -> PadState {
        let mut w = [0i32; 20];
        for (k, &b) in p.sticks.iter().enumerate() {
            w[k] = stick(b);
        }
        for (k, &(bit, pressure)) in BUTTONS.iter().enumerate() {
            w[4 + k] = match pressure {
                Some(i) if p.has_pressure => p.pressure[i] as i32,
                _ => if p.buttons & bit != 0 { 255 } else { 0 },
            };
        }
        PadState(w)
    }
}

fn stick(b: u8) -> i32 {
    let v = ((b as f32 - HALF_RANGE) * STICK_SCALE).clamp(-1.0, 1.0);
    (dead_zone(v, STICK_DEAD_ZONE) * HALF_RANGE).floor() as i32
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
            let fifth = (self.rumble_strength as f64 * 0.2) as i32;
            if player_state_3 && fifth > 10 {
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
                rigid.set_motor((s as f64 * (1.0 / 256.0)) as f32, motor);
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

