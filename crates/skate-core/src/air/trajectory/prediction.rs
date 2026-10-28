use super::math::{IDENTITY, STEP, Transform, UP, Vector, ZERO, reciprocal};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trajectory {
    pub position: Vector,
    pub velocity: Vector,
    pub acceleration: Vector,
    pub duration: f32,
}
impl Trajectory {
    pub fn position_at(self, time: f32) -> Vector {
        let square = time * time;
        core::array::from_fn(|i| {
            (self.acceleration[i] * 0.5)
                .mul_add(square, self.velocity[i].mul_add(time, self.position[i]))
        })
    }
    pub fn velocity_at(self, time: f32) -> Vector {
        core::array::from_fn(|i| self.acceleration[i].mul_add(time, self.velocity[i]))
    }
    pub fn highest_position(self) -> (Vector, f32) {
        if self.velocity[1] < 0.0 || self.acceleration[1] >= 0.0 {
            (self.position, 0.0)
        } else {
            let t = self.velocity[1] * reciprocal(-self.acceleration[1]);
            (self.position_at(t), t)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryRequest {
    pub trajectory: Trajectory,
    pub radius: f32,
    pub start_error: f32,
    pub end_error: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QueryResult {
    pub contact_position: Vector,
    pub contact_normal: Vector,
    pub landing_normal: Vector,
    pub contact_time: f32,
    pub contact_transform: Transform,
    pub contact_frame: i32,
    pub surface: u32,
    pub geometry: u32,
}
impl QueryResult {
    pub const fn miss() -> Self {
        Self {
            contact_position: ZERO,
            contact_normal: UP,
            landing_normal: UP,
            contact_time: -1.0,
            contact_transform: IDENTITY,
            contact_frame: -1,
            surface: 0,
            geometry: 0,
        }
    }
    pub fn valid(self) -> bool {
        self.contact_time >= 0.0
    }
    pub fn suggested_normal(self) -> Vector {
        if self.valid() {
            self.landing_normal
        } else {
            UP
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prediction {
    pub result: QueryResult,
    pub request: QueryRequest,
}
impl Prediction {
    pub fn collision_position(self) -> Vector {
        self.request
            .trajectory
            .position_at(self.result.contact_time)
    }
    pub fn collision_velocity(self) -> Vector {
        self.request
            .trajectory
            .velocity_at(self.result.contact_frame as f32 * STEP)
    }
}
