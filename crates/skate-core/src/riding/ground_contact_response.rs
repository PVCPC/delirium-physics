use crate::{
    math::Vector3,
    physics::{board_motion_output::dot, force_queue::QueuedPointForce},
    point_graph::PointGraph,
    riding::grounded::state::board_types::{GroundContactFrame, GroundContactResponse},
};

#[derive(Clone, Debug)]
pub struct WallRideSettings {
    pub anti_gravity_vs_time: PointGraph<8>,
    pub max_dot_floor_wall: f32,
    pub foot_force_time: f32,
    pub auto_jump_height: f32,
    pub max_time: f32,
    pub velocity_time_to_consider: f32,
    pub auto_jump_y_down_scalar: f32,
    pub auto_jump_force: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct WallRidePhysical {
    pub board_normal: [f32; 4],
    pub up: [f32; 4],
    pub velocity: [f32; 4],
    pub board_mass: f32,
    pub gravity: f32,
    pub speed: f32,
    pub contact_count: i32,
}

pub fn wall_ride_response(
    settings: &WallRideSettings,
    physical: WallRidePhysical,
    frame: GroundContactFrame,
    previous_velocity: [f32; 4],
) -> GroundContactResponse {
    let mut response = GroundContactResponse {
        active_2731: false,
        tag_16_force: QueuedPointForce {
            tag: 16,
            ..Default::default()
        },
        vector_2688: previous_velocity,
        scalar_2704: 0.0,
        animated_board_2708: false,
    };
    if !frame.flag_8084
        || !(physical.board_normal[1].abs() < 0.5)
        || !(dot(xyz(physical.up), xyz(physical.board_normal)) > 0.71)
    {
        return response;
    }
    response.active_2731 = true;
    let acceleration = settings.anti_gravity_vs_time.evaluate(frame.scalar_2752);
    response.tag_16_force.force_world = Vector3::new(
        0.0,
        -((acceleration * physical.board_mass) * physical.gravity),
        0.0,
    );
    let time = frame.scalar_2756;
    let consider = (time < 0.0 && time > -1.0) || (physical.speed < 3.0 && time > 0.1);
    if !(consider
        || (frame.scalar_2752 > 0.01 && physical.contact_count > 0)
        || frame.scalar_2752 > settings.foot_force_time)
    {
        return response;
    }
    let floor_wall = dot(xyz(frame.vector_8064), xyz(physical.board_normal));
    let height = (frame.vector_8048[1] - frame.vector_8032[1]).abs();
    let predicted_height = physical.velocity[1].mul_add(settings.velocity_time_to_consider, height);
    if !(floor_wall < settings.max_dot_floor_wall
        && (consider || predicted_height < settings.auto_jump_height || time > settings.max_time))
    {
        return response;
    }
    response.animated_board_2708 = true;
    let into_normal = dot(xyz(physical.board_normal), xyz(physical.velocity));
    let mut tangent: [f32;4] =
        std::array::from_fn(|i| physical.velocity[i] - (physical.board_normal[i] * into_normal));
    if tangent[1] < 0.0 && tangent[1] > -6.0 {
        tangent[1] *= settings.auto_jump_y_down_scalar;
    }
    response.vector_2688 = std::array::from_fn(|i| {
        physical.board_normal[i].mul_add(settings.auto_jump_force, tangent[i])
    });
    response
}
fn xyz(v: [f32; 4]) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}

