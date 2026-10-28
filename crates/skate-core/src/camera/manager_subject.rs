use super::{AnchorState, Subject};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManagerSubject {
    pub rig: Subject,
    pub anchors: [AnchorState; 7],
    pub compass: [f32; 9],
    pub board_offset_direction: [f32; 4],
    pub ground_normal: [f32; 4],
    pub launch_position: [f32; 4],
    pub launch_normal: [f32; 4],
    pub landing_position: [f32; 4],
    pub landing_normal: [f32; 4],
    pub apex_position: [f32; 4],
    pub direction_424: [f32; 4],
    pub look: [f32; 2],
    pub steering: [f32; 4],
    pub trajectory_time: f32,
    pub trajectory_duration: f32,
    pub apex_time: f32,
    pub value_512: f32,
    pub value_516: f32,
    pub reset: u8,
    pub flag_556: u8,
    pub stance_560: u8,
    pub stance_592: u8,
    pub flag_652: u8,
    pub shake_variant: u8,
    pub special_effect: u8,
    pub flag_684: u8,
}

impl ManagerSubject {
    pub fn is_ground_camera(&self, height_mode: u32) -> bool {
        height_mode == 2 || (self.flag_556 == 0 && self.rig.trajectory_valid == 0
            && self.trajectory_duration <= f32::from_bits(0x3ecccccd))
    }

    pub fn steering_for_turn(&self, mirrored: bool) -> f32 {
        let [primary, _, first, second] = self.steering;
        let mut selected = if second.abs() > first.abs() { second } else { first };
        if sign(selected) != sign(primary) || selected.abs() < primary.abs() {
            selected = primary;
        }
        if u8::from(mirrored) == self.stance_560 { selected } else { -selected }
    }

    pub fn steering_for_blend(&self, mirrored: bool) -> f32 {
        let [_, primary, first, second] = self.steering;
        let secondary = if second.abs() > first.abs() { second } else { first };
        let selected = if sign(primary) != sign(secondary) || primary.abs() < secondary.abs() {
            secondary
        } else { primary };
        if u8::from(mirrored) == self.stance_560 { selected } else { -selected }
    }

    pub fn valid_trajectory_duration(&self) -> f32 {
        if self.rig.trajectory_valid != 0 { self.trajectory_duration } else { 0.0 }
    }

    pub fn look_heading(&self) -> f32 {
        let direction = super::orientation_math::normalize([self.look[0], 0.0, self.look[1], 0.0]);
        -super::direction_to_angles(direction)[1]
    }
}

fn sign(value: f32) -> i8 {
    if value >= 0.0 { if value > 0.0 { 1 } else { 0 } } else { -1 }
}
