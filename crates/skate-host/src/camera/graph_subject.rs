#[derive(Clone, Copy, Debug)]
pub(crate) struct CameraGraphSubject {
    pub time_since_player_input: f32,
    pub wipeout_tweak: u32,
    pub onboard_air: bool,
    pub preparing_to_jump: bool,
    pub manual: bool,
    pub offboard_air: bool,
    pub running_out: bool,
    pub footplant: bool,
    pub handplant: bool,
    pub hippy_jump: bool,
    pub hippy_hurdle: bool,
    pub boneless: bool,
    pub moving_object: bool,
    pub skitching: bool,
    pub dropping_in: bool,
    pub slow_motion_air_duration: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct CameraGraphEnvironment {
    pub camera_type: u32,
    pub on_road: bool,
    pub ledge_left: bool,
    pub ledge_right: bool,
    pub volumes: Vec<String>,
}
