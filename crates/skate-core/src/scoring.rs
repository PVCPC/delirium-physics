pub const SCORABLE_COUNT: usize = 332;
pub const SCORE_TYPE_COUNT: usize = 14;
pub mod carrier;
pub mod catalog;
pub mod conversions;
pub mod session;
pub mod timer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scorable {
    pub id: usize,
    pub class: u32,
    pub score_type: usize,
}

impl Scorable {
    pub fn valid(self) -> bool {
        self.id < SCORABLE_COUNT && self.score_type < SCORE_TYPE_COUNT
    }
    pub fn repetition_applies(self) -> bool {
        self.valid() && self.class != 5 && self.class != 6
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub completed_lines: f32,
    pub line: f32,
    pub accumulated: f32,
    pub last_reward: f32,
    pub general_pending: f32,
    pub fingerflip_pending: f32,
    pub grind_reward: f32,
}

#[derive(Clone, Debug)]
pub struct ScoreHolder {
    pub snapshot: Snapshot,
    repetitions: [i8; SCORABLE_COUNT],
    sequence_history: [i8; SCORABLE_COUNT],
    type_history: [i8; SCORE_TYPE_COUNT],
    pending_sequence: bool,
    suppressed: bool,
}

impl Default for ScoreHolder {
    fn default() -> Self {
        Self {
            snapshot: Snapshot::default(),
            repetitions: [0; SCORABLE_COUNT],
            sequence_history: [0; SCORABLE_COUNT],
            type_history: [0; SCORE_TYPE_COUNT],
            pending_sequence: false,
            suppressed: false,
        }
    }
}

impl ScoreHolder {
    pub fn has_pending_sequence(&self) -> bool {
        self.pending_sequence
    }
    pub fn credit_trick(&mut self, scorable: Scorable, reward: f32) {
        if !scorable.valid() || self.suppressed {
            return;
        }
        self.count_trick(scorable);
        self.snapshot.accumulated += reward;
    }
    fn count_trick(&mut self, scorable: Scorable) {
        self.repetitions[scorable.id] = self.repetitions[scorable.id].saturating_add(1);
        if scorable.score_type != 0 {
            self.sequence_history[scorable.id] =
                self.sequence_history[scorable.id].saturating_add(1);
            self.type_history[scorable.score_type] =
                self.type_history[scorable.score_type].saturating_add(1);
        }
    }
    pub fn set_suppressed(&mut self, suppressed: bool) {
        self.suppressed = suppressed;
    }
    pub fn repetition_count(&self, scorable: Scorable) -> Option<i8> {
        scorable.valid().then(|| self.repetitions[scorable.id])
    }
    pub fn end_trick(&mut self, scorable: Scorable, reward: f32) {
        if !scorable.valid() || self.suppressed {
            return;
        }
        self.count_trick(scorable);
        let s = &mut self.snapshot;
        if scorable.class != 5 {
            s.general_pending += s.fingerflip_pending;
            s.fingerflip_pending = 0.0;
        }
        if scorable.class == 3 {
            s.fingerflip_pending += reward;
        } else {
            s.general_pending += reward;
        }
    }
    pub fn finish_collector(&mut self) {
        self.pending_sequence = true;
    }
    pub fn cancel_pending(&mut self) {
        if self.pending_sequence {
            self.snapshot.general_pending = 0.0;
            self.snapshot.fingerflip_pending = 0.0;
        }
    }
    pub fn reward_sequence(&mut self, multiplier: f32) {
        if !self.pending_sequence {
            return;
        }
        let s = &mut self.snapshot;
        s.accumulated += (s.general_pending + s.fingerflip_pending) * multiplier;
        s.general_pending = 0.0;
        s.fingerflip_pending = 0.0;
        self.pending_sequence = false;
    }
    pub fn publish(&mut self, reward: f32, add_to_line: bool) {
        let s = &mut self.snapshot;
        if add_to_line {
            s.line += reward;
        }
        s.last_reward = reward;
        s.accumulated = 0.0;
        s.general_pending = 0.0;
        s.fingerflip_pending = 0.0;
        self.clear_sequence_history();
    }
    pub fn clear_sequence_history(&mut self) {
        self.sequence_history.fill(0);
        self.type_history.fill(0);
        self.snapshot.grind_reward = 0.0;
    }
    pub fn finish_line(&mut self) {
        self.bank_line(true);
    }
    pub fn bank_line(&mut self, clear_repetition: bool) {
        self.snapshot.completed_lines += self.snapshot.line;
        self.snapshot.line = 0.0;
        if clear_repetition {
            self.repetitions.fill(0);
        }
    }
    pub fn reset(&mut self) {
        let completed_lines = self.snapshot.completed_lines;
        *self = Self::default();
        self.snapshot.completed_lines = completed_lines;
    }
}

