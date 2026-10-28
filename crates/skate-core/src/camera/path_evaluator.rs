use super::{PathObstacle, PredictionPath, candidate_collision_time};

pub trait TrajectoryCollisionRequest {
    fn submit(&mut self, path: PredictionPath, context: u32, acceleration_flag: u8);
    fn is_ready(&mut self) -> bool;
    fn collision_time(&mut self) -> f32;
}

pub trait MovingObstacleProvider {
    fn collect(
        &mut self,
        position: [f32; 4],
        velocity: [f32; 4],
        radius: f32,
        output: &mut [PathObstacle; 50],
    ) -> usize;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathEvaluator {
    pub request_state: u32,
    pub path: PredictionPath,
    pub acceleration_flag: u8,
    pub collision_time: f32,
    pub last_valid_time: f32,
    pub found: u8,
    pub result_acceleration_flag: u8,
    pub submitted_acceleration_flag: u8,
    pub pending_polls: i32,
}

impl PathEvaluator {
    pub fn update_request(&mut self, context: u32, request: &mut impl TrajectoryCollisionRequest) {
        match self.request_state {
            0 => {
                request.submit(self.path, context, self.acceleration_flag);
                self.submitted_acceleration_flag = self.acceleration_flag;
                self.request_state = 1;
                self.pending_polls = 0;
            }
            1 => {
                if request.is_ready() {
                    let result = request.collision_time();
                    let result = if result < 0.0 { f32::MAX } else { result };
                    self.found = 1;
                    self.request_state = 0;
                    self.result_acceleration_flag = self.submitted_acceleration_flag;
                    self.collision_time = select_time(self.collision_time, result);
                } else {
                    self.pending_polls = self.pending_polls.wrapping_add(1);
                    if self.pending_polls > 3 {
                        self.request_state = 0;
                        self.last_valid_time = f32::MAX;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn update(
        &mut self,
        context: u32,
        request: &mut impl TrajectoryCollisionRequest,
        moving: &mut impl MovingObstacleProvider,
    ) {
        self.found = 0;
        self.collision_time = f32::MAX;
        self.update_request(context, request);

        let mut obstacles = [PathObstacle {
            position: [0.0; 4],
            velocity: [0.0; 4],
            radius: 0.0,
        }; 50];
        let count = moving.collect(
            self.path.position,
            self.path.velocity,
            self.path.radius,
            &mut obstacles,
        );
        assert!(
            count <= obstacles.len(),
            "native moving-sphere capacity exceeded"
        );
        let time = candidate_collision_time(self.path, &obstacles[..count]);
        if time < self.path.horizon {
            self.found = 1;
            self.collision_time = select_time(self.collision_time, time);
        }
        if self.found != 0 {
            self.last_valid_time = self.collision_time;
        }
    }
}

fn select_time(current: f32, candidate: f32) -> f32 {
    if current - candidate >= 0.0 {
        candidate
    } else {
        current
    }
}
