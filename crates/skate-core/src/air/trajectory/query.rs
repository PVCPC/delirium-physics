use super::{QueryRequest, QueryResult, math::*};
#[derive(Clone, Copy, Debug)]
pub struct SurfaceHit {
    pub position: Vector,
    pub normal: Vector,
    pub transform: Transform,
    pub surface: u32,
    pub geometry: u32,
}

pub fn query_trajectory<E>(
    request: QueryRequest,
    mut line: impl FnMut(Vector, Vector, f32) -> Result<Option<SurfaceHit>, E>,
    mut nearby: impl FnMut(Vector, f32) -> Result<Vec<[Vector; 3]>, E>,
) -> Result<QueryResult, E> {
    let trajectory = request.trajectory;
    let mut start_step = trajectory.duration;
    let mut end_step = trajectory.duration;
    if trajectory.acceleration[1] < 0.0 {
        start_step = step_size(
            trajectory.acceleration[1],
            request.start_error * request.radius,
        );
        end_step = step_size(
            trajectory.acceleration[1],
            request.end_error * request.radius,
        );
    }
    let minimum_square =
        ((request.start_error * request.radius) * request.start_error) * request.radius;
    let mut time = 0.0;
    let mut start = trajectory.position_at(time);
    let mut end = trajectory.position_at(time + start_step);
    while trajectory.duration > time {
        let delta = sub(end, start);
        if dot(delta, delta) > minimum_square {
            if delta[..3]
                .iter()
                .any(|v| v.abs() > f32::from_bits(0x3780_0000))
            {
                if let Some(hit) = line(start, end, request.radius)? {
                    let (contact_time, contact_frame) =
                        time_at_contact(request, hit.position, time);
                    let triangles = nearby(hit.position, request.radius)?;
                    let velocity = trajectory.velocity_at(contact_frame as f32 * STEP);
                    return Ok(QueryResult {
                        contact_position: hit.position,
                        contact_normal: hit.normal,
                        landing_normal: average_landing_normal(&triangles, velocity),
                        contact_time,
                        contact_frame,
                        contact_transform: hit.transform,
                        surface: hit.surface,
                        geometry: hit.geometry,
                    });
                }
            }
            start = end;
        }
        let fraction = reciprocal(trajectory.duration) * time;
        let step = end_step.mul_add(fraction, (1.0 - fraction) * start_step);
        time += step;
        end = trajectory.position_at(time + step);
    }
    Ok(QueryResult::miss())
}

fn reciprocal(value: f32) -> f32 {
    let mut inverse = crate::physics::native_arithmetic::reciprocal_estimate(value);
    for _ in 0..2 {
        inverse = inverse.mul_add((-inverse).mul_add(value, 1.0), inverse);
    }
    inverse
}
fn step_size(gravity_y: f32, error_radius: f32) -> f32 {
    let square = reciprocal(gravity_y) * (-8.0 * error_radius);
    if square == 0.0 {
        0.0
    } else {
        square * inverse_length(square)
    }
}
fn time_at_contact(request: QueryRequest, position: Vector, mut time: f32) -> (f32, i32) {
    const EPSILON: f32 = f32::from_bits(0x38d1_b717);
    let trajectory = request.trajectory;
    if EPSILON >= (reciprocal(2.0) * trajectory.acceleration[1]).abs() {
        let distance = length(sub(position, trajectory.position));
        if !(distance.abs() > EPSILON) {
            return (0.0, 0);
        }
        let speed = length(trajectory.velocity);
        if !(speed.abs() > EPSILON) {
            return (0.0, 0);
        }
        let frames = reciprocal(speed) * distance;
        return (frames * STEP, frames.ceil() as i32);
    }
    let mut closest_square = f32::MAX;
    let mut found = false;
    while trajectory.duration >= time {
        let delta = sub(position, trajectory.position_at(time));
        let square = dot(delta, delta);
        if closest_square >= square {
            closest_square = square;
        } else if square > closest_square {
            time -= STEP;
            found = true;
            break;
        }
        time += STEP;
    }
    if !found {
        time = trajectory.duration;
    }
    (time, (reciprocal(STEP) * time).ceil() as i32)
}
fn average_landing_normal(triangles: &[[Vector; 3]], velocity: Vector) -> Vector {
    let mut accepted = Vec::with_capacity(64);
    let mut most_up = ZERO;
    let mut highest = -1.0;
    for triangle in triangles.iter().take(64) {
        let normal = cross(sub(triangle[1], triangle[0]), sub(triangle[2], triangle[0]));
        let normal = scale(normal, inverse_length(dot(normal, normal)));
        if normal[1] > 0.7 || dot(normal, velocity) <= 0.0 {
            let up = dot(normal, UP);
            if up > highest {
                highest = up;
                most_up = normal;
            }
            accepted.push(normal);
        }
    }
    if accepted.is_empty() {
        return UP;
    }
    let mut total = ZERO;
    for normal in accepted {
        if dot(most_up, normal) > 0.5 {
            total = add(total, normal);
        }
    }
    normalize(total)
}
