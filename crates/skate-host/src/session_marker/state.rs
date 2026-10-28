#[derive(Default, Debug)]
pub(super) struct Hold {
    elapsed: f32,
    fired: bool,
    tail: u8,
}

#[derive(Default, Debug, PartialEq)]
pub(super) struct Step {
    pub progress: f32,
    pub relocate: bool,
}

impl Hold {
    pub fn cancel(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, held: bool, usable: bool, distance: f32, ready: bool) -> Step {
        let mut out = Step::default();
        if !held {
            self.elapsed = 0.;
            self.fired = false;
        } else if !self.fired && usable {
            self.elapsed += f32::from_bits(0x3c88_8889);
            if distance > 0.5 && distance.is_finite() {
                let duration = duration(distance);
                if ready && self.elapsed > duration {
                    self.fired = true;
                    self.tail = 3;
                    out.relocate = true;
                }
                out.progress = (self.elapsed / duration).clamp(0., 1.);
            }
        }
        if self.tail > 0 {
            out.progress = 1.;
            self.tail -= 1;
        }
        out
    }
}

pub(super) fn duration(distance: f32) -> f32 {
    if distance <= 100. {
        0.2
    } else if distance >= 1000. {
        1.
    } else {
        distance.mul_add(f32::from_bits(0x3a69_0453), f32::from_bits(0x3de3_8e39))
    }
}

