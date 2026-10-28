#[derive(Clone, Debug, PartialEq)]
pub struct Requests {
    pub reasons: [bool; 34],
    pub values: [f32; 34],
    pub count: u32,
    pub cooldown: f32,
    pub contact_frames: i32,
    pub balance: f32,
    pub mode: u32,
}
impl Requests {
    pub fn new() -> Self {
        Self {
            reasons: [false; 34],
            values: [0.0; 34],
            count: 0,
            cooldown: 0.0,
            contact_frames: 0,
            balance: 0.0,
            mode: 0,
        }
    }
    pub fn initialize_player(&mut self) {
        self.cooldown = f32::from_bits(0x3d23_d70a);
    }
    pub fn teleport(&mut self) {
        self.cooldown = f32::from_bits(0x3ecc_cccd);
    }
    pub fn enter_ground(&mut self) {
        if self.mode != 1 {
            self.balance = 0.0;
            self.mode = 1;
        }
    }
    pub fn request(&mut self, index: usize, value: f32) {
        self.reasons[index] = true;
        self.values[index] = value;
        self.count = self.count.wrapping_add(1);
    }
    pub fn clear_after_selection(&mut self) {
        self.reasons.fill(false);
        self.values.fill(0.0);
        self.count = 0;
    }
    pub fn reset_systems(&mut self) {
        self.mode = 0;
        self.count = 0;
    }
    pub fn requests_runout(&self, frame: &super::RequestInput) -> bool {
        (self.count == 1
            && (self.reasons[2] || self.reasons[24] || self.reasons[10])
            && frame.flags_2468 & (1 << 5) == 0
            && frame.flags_2476 & (1 << 26) == 0
            && frame.animation_up_y.abs() > f32::from_bits(0x3f4f_5c29))
            || (frame.flags_2484 & (1 << 12) != 0 && frame.category == 400)
    }
    pub fn requests_wipeout(&self, frame: &super::RequestInput) -> bool {
        !self.requests_runout(frame)
            && (self.reasons.iter().any(|&v| v) || frame.flags_2480 & (1 << 3) != 0)
    }
}
