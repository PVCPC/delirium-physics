use super::trajectory::{frames, shift};
use super::*;
use crate::player::wipeout_state::math::{add, dot, madd, reciprocal, scale, sub};

impl Manager {
    pub(super) fn prepare_query(&mut self, p: &Input, position: Vector, time: f32) -> QueryRequest {
        let target = add(
            madd(p.board_velocity_400, time, p.board_position_112),
            [0., f32::from_bits(0x3e4c_ccce), 0., 0.],
        );
        self.blocked_258 = false;
        self.obstruction_height_240 = target[1] + -0.1;
        QueryRequest {
            trajectory: Trajectory {
                position,
                velocity: sub(
                    scale(sub(target, position), reciprocal(time)),
                    scale(scale(GRAVITY, 0.5), time),
                ),
                acceleration: GRAVITY,
                duration: time,
            },
            radius: 0.3,
            start_error: 0.5,
            end_error: 0.5,
        }
    }

    pub fn query_submitted(&mut self) {
        self.pending_262 = true;
    }

    pub fn sync<E>(
        &mut self,
        hit: Option<QueryResult>,
        input: &SyncInput,
        hippy_velocity: impl FnOnce(&Manager) -> Result<Vector, E>,
    ) -> Result<(), E> {
        let mut next = *self;
        next.sync_completed(hit.filter(|hit| hit.valid()), input, hippy_velocity)?;
        *self = next;
        Ok(())
    }

    fn sync_completed<E>(
        &mut self,
        hit: Option<QueryResult>,
        input: &SyncInput,
        hippy_velocity: impl FnOnce(&Manager) -> Result<Vector, E>,
    ) -> Result<(), E> {
        if let Some(hit) = hit {
            if hit.contact_position[1] < self.obstruction_height_240 {
                self.blocked_258 = false;
                self.tested_259 = true;
            } else if self.tested_259 {
                self.blocked_258 = true;
            } else if self.completed_queries_252 == 0 {
                self.moving_contact_208 = hit.contact_transform[3];
                self.publish_moving_contact_261 = true;
            } else {
                let contact = hit.contact_transform[3];
                let velocity = scale(sub(contact, self.moving_contact_208), 60.);
                if dot(velocity, velocity) > f32::from_bits(0x3efa_e147) {
                    self.hippy_hurdling_260 = true;
                    if self.completed_queries_252 == 1 {
                        if input.flags_2488 & 0x0400_0000 != 0 {
                            let current = self.trajectory_32.velocity_at(self.elapsed_160);
                            let replacement = hippy_velocity(self)?;
                            if replacement[1] > current[1] {
                                let frame = frames(self.elapsed_160);
                                shift(&mut self.trajectory_32, frame);
                                self.trajectory_32.velocity = replacement;
                                shift(&mut self.trajectory_32, frame.wrapping_neg());
                            }
                        }
                    } else {
                        let landing_time = self.time_to_land_244 + self.elapsed_160;
                        let predicted_contact = madd(velocity, self.time_to_land_244, contact);
                        let mut separation = sub(
                            self.trajectory_32.position_at(landing_time),
                            predicted_contact,
                        );
                        separation[1] = 0.;
                        let square = dot(separation, separation);
                        if self.trajectory_32.velocity_at(self.elapsed_160)[1] < 0. {
                            if input.position_592[1] - hit.contact_position[1] < 0.7 {
                                self.blocked_258 = true;
                                self.tested_259 = true;
                            }
                        } else if square < 1. {
                            self.blocked_258 = true;
                            self.tested_259 = true;
                        } else if square > 6.25 {
                            self.blocked_258 = false;
                            self.tested_259 = true;
                        }
                    }
                } else {
                    self.hippy_hurdling_260 = false;
                    self.blocked_258 = true;
                    self.tested_259 = true;
                }
                self.moving_contact_208 = contact;
            }
        } else {
            self.blocked_258 = false;
            self.tested_259 = true;
        }
        self.pending_262 = false;
        self.completed_queries_252 = self.completed_queries_252.wrapping_add(1);
        Ok(())
    }
}
