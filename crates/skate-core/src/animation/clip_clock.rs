#[derive(Clone, Copy, Debug)]
pub struct ClipClock {
    pub frames: f32,
    pub fps: f32,
    pub base_speed: f32,
    pub speed: f32,
    pub length: f32,
    pub time: f32,
    pub previous_time: f32,
    pub loops_since_evaluation: u32,
    pub looping: bool,
    pub phase_controlled: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AdvanceResult {
    pub crossed_end: bool,
    pub overshoot: f32,
    pub remaining_before_wrap: f32,
}

impl ClipClock {
    pub fn set_time(&mut self, time: f32) {
        self.time = time;
        self.previous_time = time;
        self.loops_since_evaluation = 0;
    }

    pub fn set_speed(&mut self, speed: f32) {
        let new_rate = self.base_speed * speed;
        let old_rate = self.speed * self.base_speed;
        let frames_per_second = self.fps * self.base_speed;
        let inverse_new_rate = 1.0 / new_rate;
        self.time = (self.time * old_rate) * inverse_new_rate;
        self.previous_time = (self.previous_time * old_rate) * inverse_new_rate;
        self.length = (self.frames - 1.0) / (frames_per_second * speed);
        self.speed = speed;
    }

    pub fn sample_time(&self) -> f32 {
        if self.length - self.time >= 0.0 {
            self.time
        } else {
            self.length
        }
    }

    pub fn advance(&mut self, delta_seconds: f32, phase: f32, result: &mut AdvanceResult) {
        if self.phase_controlled {
            self.previous_time = self.time;
            let lower = if -phase >= 0.0 { 0.0 } else { phase };
            let phase = if 1.0 - lower >= 0.0 { lower } else { 1.0 };
            self.time = self.length * phase;
            self.loops_since_evaluation = u32::from(self.time < self.previous_time);
            return;
        }
        self.time += delta_seconds;
        result.remaining_before_wrap = self.length - self.time;
        if self.time.partial_cmp(&self.length) != Some(core::cmp::Ordering::Greater) {
            result.crossed_end = false;
        } else if self.looping {
            assert!(
                self.length > 0.0 && self.time.is_finite(),
                "invalid looping clip clock"
            );
            while self.time > self.length {
                result.crossed_end = true;
                result.overshoot = self.time - self.length;
                let next = self.time - self.length;
                assert!(
                    next < self.time,
                    "clip clock cannot advance at this precision"
                );
                self.time = next;
                self.loops_since_evaluation = self.loops_since_evaluation.wrapping_add(1);
            }
        } else {
            result.crossed_end = true;
            result.overshoot = self.time - self.length;
        }
    }

    pub fn commit_evaluation(&mut self) {
        self.previous_time = self.time;
        self.loops_since_evaluation = 0;
    }

    pub fn attribute_status(&self, begin: f32, end: f32) -> u8 {
        if begin == -1.0 {
            return 6;
        }
        let begin = self.length * begin;
        let end = self.length * end;
        let intersects = if self.loops_since_evaluation == 0 {
            end > self.previous_time && begin <= self.time
        } else {
            end > self.previous_time || begin <= self.time
        };
        if !intersects {
            17
        } else if self.time < begin || self.time > end {
            9
        } else {
            5
        }
    }
}
