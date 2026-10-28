#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathObstacle {
    pub position: [f32; 4],
    pub velocity: [f32; 4],
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PredictionPath {
    pub position: [f32; 4],
    pub velocity: [f32; 4],
    pub radius: f32,
    pub horizon: f32,
}

pub fn candidate_collision_time(path: PredictionPath, obstacles: &[PathObstacle]) -> f32 {
    let mut result = f32::MAX;
    for obstacle in obstacles {
        let separation = core::array::from_fn(|i| obstacle.position[i] - path.position[i]);
        let relative_velocity = core::array::from_fn(|i| obstacle.velocity[i] - path.velocity[i]);
        let radius = obstacle.radius + path.radius;
        let clearance = super::vector_tracker::dot(separation, separation) - radius * radius;
        let time = if clearance < 0.0 {
            0.0
        } else {
            let approach = super::vector_tracker::dot(separation, relative_velocity);
            if !(approach < 0.0) {
                continue;
            }
            let speed_squared = super::vector_tracker::dot(relative_velocity, relative_velocity);
            let time = (-1.0 / speed_squared) * approach;
            if !(approach.mul_add(time, clearance) < 0.0) {
                continue;
            }
            time
        };
        if time < path.horizon && time < result {
            result = time;
        }
    }
    result
}
