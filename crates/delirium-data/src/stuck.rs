use crate::fall::{id, DAMAGE_KO};

pub const EVENT: u32 = 93;
pub const TASK: u32 = 515;

pub const RESTART: f32 = 5.0;
pub const SLOW_AFTER: f32 = 1.0;
pub const SLOW: f32 = 0.2;
pub const VERTICAL_AFTER: f32 = 2.0;
pub const VERTICAL: f32 = 0.1;
pub const SKIP: u8 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scanner {
    pub place: [f32; 3],
    pub time: f32,
    pub flag: u16,
    pub skip: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub in_world: bool,
    pub attached: bool,
    pub dying: bool,
    pub ragdoll: bool,
    pub grounded: bool,
    pub collided: bool,
    pub in_water: bool,
    pub at: [f32; 3],
    pub dt: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scan {
    Idle,
    Water,
    Stuck,
}

impl Scanner {
    pub fn update(&mut self, i: &Input) -> Scan {
        if !i.in_world || i.attached || i.dying || i.ragdoll {
            (self.flag, self.time) = (0, 0.0);
            return Scan::Idle;
        }
        if self.skip > 0 {
            self.skip -= 1;
            return Scan::Idle;
        }
        if i.grounded || !i.collided {
            (self.flag, self.time) = (0, 0.0);
            return Scan::Idle;
        }
        let moved = if self.time > 0.0 && self.time <= RESTART {
            self.time += i.dt;
            [i.at[0] - self.place[0], i.at[1] - self.place[1], i.at[2] - self.place[2]]
        } else {
            (self.flag, self.time, self.place) = (0, i.dt, i.at);
            [0.0; 3]
        };
        let t = self.time;
        let d2 = moved[0].mul_add(moved[0], moved[1].mul_add(moved[1], moved[2] * moved[2]));
        if t > SLOW_AFTER && d2 < (t * SLOW) * (t * SLOW) || t > VERTICAL_AFTER && moved[2].abs() < t * VERTICAL {
            self.flag = 1;
        }
        if self.flag == 0 {
            Scan::Idle
        } else if i.in_water {
            Scan::Water
        } else {
            Scan::Stuck
        }
    }
}

pub fn priority(queued: usize, answering: bool) -> i32 {
    if !answering && queued > 1 { 105 } else { 86 }
}

pub fn overrides(priority: i32, running_priority: i32, running_kind: u32) -> bool {
    running_priority < 104 && running_kind != EVENT || priority >= running_priority
}

pub fn concerns(ground_bit: bool, flag: u16) -> bool {
    !ground_bit && flag != 0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sub {
    Stand,
    Fall,
    Jump,
    StepAway,
}

impl Sub {
    pub fn of(kind: u32) -> Option<Sub> {
        Some(match kind {
            203 => Sub::Stand,
            208 => Sub::Fall,
            211 => Sub::Jump,
            910 => Sub::StepAway,
            _ => return None,
        })
    }
}

pub const STAND_MS: i32 = 5000;
pub const STAND_BLEND: f32 = 8.0;
pub const FALL: (i32, i32, u32) = (DAMAGE_KO, id::KO_FRONT, 1000);
pub const STEP_BACK: f32 = 0.5;
pub const STEP_DISTANCE: f32 = 5.0;
pub const STEP_MS: i32 = 10000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Keep,
    End,
    To(Sub),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub next: Next,
    pub clear: bool,
    pub ground: bool,
}

const fn go(sub: Sub, clear: bool, ground: bool) -> Step {
    Step { next: Next::To(sub), clear, ground }
}

pub fn first(flag: u16) -> Step {
    if flag == 2 { go(Sub::Fall, true, false) } else { go(Sub::Stand, false, true) }
}

pub fn next(done: Sub, flag: u16, player: bool) -> Step {
    let end = Step { next: Next::End, clear: false, ground: false };
    match done {
        Sub::Fall | Sub::StepAway if flag == 0 => end,
        Sub::Fall | Sub::StepAway => go(Sub::Stand, false, true),
        Sub::Jump if flag != 0 => go(Sub::Stand, false, true),
        Sub::Jump if player => end,
        Sub::Jump => go(Sub::StepAway, false, true),
        Sub::Stand if flag == 0 => end,
        Sub::Stand if player => go(Sub::Stand, false, true),
        Sub::Stand => go(Sub::Jump, true, true),
    }
}

pub fn control(sub: Sub, flag: u16, jump_in_air: bool) -> Step {
    let keep = Step { next: Next::Keep, clear: false, ground: false };
    match sub {
        Sub::Stand if flag == 0 => Step { next: Next::End, ..keep },
        Sub::Stand => Step { ground: true, ..keep },
        Sub::Jump if flag != 0 && jump_in_air => go(Sub::Stand, false, true),
        Sub::StepAway if flag != 0 => go(Sub::Stand, false, true),
        _ => keep,
    }
}

