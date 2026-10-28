use super::{Frame, QueryRequest, Vector};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub deck_min_uprightness: f32,
    pub approximate_com_height: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    pub board_up_80: Vector,
    pub board_position_112: Vector,
    pub board_velocity_400: Vector,
    pub up_544: Vector,
    pub support_1776: i32,
    pub flags_2480: u32,
    pub mode_2540: u32,
    pub wheel_contacts_2556: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AssistInput {
    pub processed: Input,
    pub position: Vector,
    pub velocity: Vector,
    pub maximum_velocity_change: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IkInput {
    pub processed: Input,
    pub time_to_land: f32,
    pub animation_root: Frame,
    pub animation_com_10960: Vector,
    pub mapped_position_12608: Vector,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncInput {
    pub position_592: Vector,
    pub flags_2488: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpdateOutput {
    pub can_land: bool,
    pub trajectory_valid: bool,
    pub time_to_land: f32,
    pub elapsed: f32,
    pub landing_time: f32,
    pub apex_time: f32,
    pub position: Vector,
    pub landing_velocity: Vector,
    pub launch_position: Vector,
    pub landing_position: Vector,
    pub normal: Vector,
    pub apex_position: Vector,
    pub direction: Vector,
    pub up: Vector,
    pub query: Option<QueryRequest>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillOutput {
    pub can_land_316: bool,
    pub hippy_hurdling_317: bool,
    pub moving_contact: Option<Vector>,
}
