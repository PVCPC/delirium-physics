use super::input_phase::RawMatrix;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub transform: RawMatrix,
    pub on_board: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Update {
    RequestCheckpoint,
    Captured,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub transform: RawMatrix,
    pub next_state: u32,
    pub state_61: u8,
    pub board_272: u8,
}

#[derive(Clone, Debug)]
pub struct TeleportState {
    target: RawMatrix,
    ready: bool,
    received: bool,
    on_board: bool,
}
impl Default for TeleportState {
    fn default() -> Self {
        Self {
            target: [[0; 4]; 4],
            ready: false,
            received: false,
            on_board: true,
        }
    }
}
impl TeleportState {
    pub fn enter(&mut self) {
        self.ready = false;
        self.received = false;
        self.on_board = true;
    }
    pub fn update(&mut self, flags_2468: u32, matrix_1536: RawMatrix, byte_1600: u8) -> Update {
        if flags_2468 & 2 == 0 {
            return Update::RequestCheckpoint;
        }
        self.received = true;
        self.target = matrix_1536;
        self.ready = true;
        self.on_board = byte_1600 & 1 != 0;
        Update::Captured
    }
    pub fn output(&self) -> Option<Output> {
        self.ready.then_some(Output {
            transform: self.target,
            next_state: if self.on_board { 100 } else { 500 },
            state_61: 1,
            board_272: 1,
        })
    }
}
