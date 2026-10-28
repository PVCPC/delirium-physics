#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointTimer {
    pub points: f32,
    pub expired: bool,
}
impl PointTimer {
    pub fn advance(&mut self, dt: f32, drain: f32, scale: f32, hold_near_one: bool) -> bool {
        self.expired = false;
        if self.points > 0.0 {
            let previous = self.points;
            self.points = -((drain * dt) * scale - self.points);
            if hold_near_one && self.points < 1.001 && previous > 1.000001 {
                self.points = previous;
                return true;
            }
            if self.points <= 0.0 {
                self.points = 0.0;
                self.expired = true;
            }
        }
        false
    }
    pub fn credit(&mut self, points: f32, capacity: f32) {
        self.points = (self.points + points).min(capacity);
    }
    pub fn seconds(&self, drain: f32) -> f32 {
        self.points / drain
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComboTimer {
    pub timer: PointTimer,
    pub multiplier: f32,
}
impl Default for ComboTimer {
    fn default() -> Self {
        Self {
            timer: PointTimer::default(),
            multiplier: 1.0,
        }
    }
}
impl ComboTimer {
    pub fn credit(&mut self, reward: f32, capacity: f32, levels: [(f32, f32); 3], refresh: f32) {
        self.timer.credit(reward, capacity);
        let mut next = 1.0;
        for (threshold, multiplier) in levels {
            if self.timer.points >= threshold {
                next = multiplier;
            }
        }
        if next > self.multiplier || reward > refresh {
            self.multiplier = next;
        }
    }
}

