pub const TYPE: u32 = 2112;

#[derive(Clone, Copy, Debug)]
pub struct Table<'a> {
    pub names: &'a [String],
    pub messages: &'a [i32],
}

impl Table<'_> {
    pub fn bits(&self, msg: &str) -> u32 {
        self.messages.iter().enumerate().filter(|&(_, &m)| self.names.get(m as usize + 1).is_some_and(|n| !n.is_empty() && n == msg)).fold(0, |b, (i, _)| b | 1 << i)
    }

    pub fn probe(&self, id: i32) -> u32 {
        1 << self.messages.iter().position(|&m| m == id.wrapping_sub(1)).unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Script {
    pub flag: bool,
    pub min: u16,
    pub max: u16,
    pub started: Option<u32>,
    pub success: u32,
    pub failure: u32,
    pub finish: u32,
}

pub const FAILURE: u8 = 8;
pub const SUCCESS: u8 = 12;
pub const FINISH: u8 = 16;

impl Script {
    pub fn new(ms: u32, flag: bool) -> Self {
        Script { flag, min: ms as u16, max: ms as u16, ..Default::default() }
    }

    pub fn update(&mut self, now: u32, ped_flags: &mut u32) -> bool {
        crate::taskctor::script_start(self.flag as u8, ped_flags);
        let Some(t0) = self.started else {
            self.started = Some(now);
            return false;
        };
        self.max != 0 && now > t0.wrapping_add(self.max as u32)
    }

    pub fn feedback(&mut self, slot: u8, t: &Table, msg: &str) {
        let b = t.bits(msg);
        match slot {
            FAILURE => self.failure |= b,
            SUCCESS => self.success |= b,
            _ => self.finish |= b,
        }
    }

    pub fn seen(&self, slot: u8, t: &Table, id: i32) -> bool {
        let m = match slot {
            FAILURE => self.failure,
            SUCCESS => self.success,
            _ => self.finish,
        };
        m & t.probe(id) != 0
    }
}

