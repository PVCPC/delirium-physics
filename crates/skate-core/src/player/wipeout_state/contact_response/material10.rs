use super::{apply_velocity_delta, reject_positive};
use crate::{physics::skeleton_body::SkeletonBody, player::wipeout_state::math::*};

const STEP: f32 = f32::from_bits(0x3c88_8889);
const PHASE_DURATION: f32 = f32::from_bits(0x3d07_2b02);

pub(super) struct Response {
    previous_velocity: V,
    normal: V,
    target_velocity: V,
    time: f32,
    pub phase: u32,
    pub finished: bool,
}
impl Response {
    pub fn new() -> Self {
        Self {
            previous_velocity: [0.0; 4],
            normal: [0.0, 1.0, 0.0, 0.0],
            target_velocity: [0.0; 4],
            time: 0.0,
            phase: 0,
            finished: false,
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    pub fn update(&mut self, body: &mut SkeletonBody, velocity: V, contact: Option<V>) {
        if let Some(normal) = contact {
            self.normal = normal;
        }
        self.finished = false;
        self.select(contact.is_some());
        match self.phase {
            0 => {
                let difference = sub(velocity, self.previous_velocity);
                self.previous_velocity = if self.time > f32::from_bits(0x3c75_c28f)
                    && dot(difference, difference) > 1.0
                {
                    madd(
                        velocity,
                        f32::from_bits(0x3d4c_cccd),
                        scale(self.previous_velocity, f32::from_bits(0x3f73_3333)),
                    )
                } else {
                    velocity
                };
            }
            1 => {
                let rejected = reject_positive(scale(velocity, -0.5), self.normal);
                apply_velocity_delta(body, madd(self.normal, -1.0, rejected));
            }
            2 => {
                apply_velocity_delta(body, scale(sub(self.target_velocity, velocity), 1.0));
                self.target_velocity = madd(
                    [0.0, f32::from_bits(0xc11c_cccd), 0.0, 0.0],
                    STEP,
                    self.target_velocity,
                );
            }
            _ => {}
        }
        self.time += STEP;
    }
    fn select(&mut self, contact: bool) {
        let next = match self.phase {
            0 if contact => {
                if -dot(self.previous_velocity, self.normal) > 2.0 {
                    1
                } else {
                    4
                }
            }
            1 if self.time > PHASE_DURATION => {
                let twice_normal = scale(self.normal, 2.0);
                self.target_velocity = scale(
                    sub(
                        self.previous_velocity,
                        scale(twice_normal, dot(self.previous_velocity, self.normal)),
                    ),
                    f32::from_bits(0x3f66_6666),
                );
                2
            }
            2 if self.time > PHASE_DURATION => {
                self.finished = true;
                3
            }
            3 if !contact => 0,
            phase => phase,
        };
        if next != self.phase {
            self.phase = next;
            self.time = 0.0;
        }
    }
}
