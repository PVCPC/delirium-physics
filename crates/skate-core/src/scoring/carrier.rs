use super::Scorable;

pub fn delay_ticks(authored_seconds: f32, extra_seconds: f32) -> u32 {
    (((authored_seconds + extra_seconds) * f32::from_bits(0x42700000)) as i64) as u32
}

#[derive(Clone, Debug)]
pub struct Carrier {
    pub scorable: Scorable,
    pub points: i32,
    pub factor: f32,
    pub reward: f32,
    pub announcement_threshold: f32,
    pub start_tick: u32,
    pub delay_ticks: u32,
    pub announced: bool,
    pub completed: bool,
    pub unannounced: bool,
    pub switch: bool,
    pub fakie: bool,
}

impl Carrier {
    pub fn new(
        scorable: Scorable,
        points: i32,
        factor: f32,
        announcement_threshold: f32,
        start_tick: u32,
        delay_ticks: u32,
        switch: bool,
        fakie: bool,
    ) -> Self {
        Self {
            scorable,
            points,
            factor,
            reward: 0.0,
            announcement_threshold: announcement_threshold * factor,
            start_tick,
            delay_ticks,
            announced: false,
            completed: false,
            unannounced: false,
            switch,
            fakie,
        }
    }

    fn credit(&mut self, unannounced_factor: f32) {
        let mut reward = self.points as f32 * self.factor;
        if self.unannounced {
            reward *= unannounced_factor;
        }
        self.reward += reward;
    }

    pub fn announce(&mut self, tick: u32, unannounced_factor: f32) -> bool {
        if self.announced
            || !self.scorable.valid()
            || tick.wrapping_sub(self.start_tick) < self.delay_ticks
        {
            return false;
        }
        self.announced = true;
        self.credit(unannounced_factor);
        true
    }

    pub fn complete(&mut self, unannounced_factor: f32) -> bool {
        if !self.scorable.valid() {
            return false;
        }
        if !self.announced {
            self.unannounced = true;
            self.credit(unannounced_factor);
        }
        self.completed = true;
        true
    }

    pub fn convert_to(&mut self, replacement: &mut Self, unannounced_factor: f32) {
        self.completed = true;
        replacement.announced = true;
        replacement.credit(unannounced_factor);
    }
}

