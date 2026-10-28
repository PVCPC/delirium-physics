use super::{
    AnchorTrackingSubject, AvoidanceSubject, ReferenceHeightSubject, RigModeSubject,
    RigPositioningSubject,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Subject {
    pub transform: [[f32; 4]; 4],
    pub skeleton_root: [[f32; 4]; 4],
    pub hips_position: [f32; 4],
    pub last_valid_ground_up: [f32; 4],
    pub reference_positions: [[f32; 4]; 10],
    pub context: u32,
    pub pumping_acceleration: f32,
    pub state_height_32: f32,
    pub in_ground_physics: u8,
    pub grinding: u8,
    pub trajectory_valid: u8,
    pub wiping_out: u8,
    pub physically_pushing: u8,
    pub at_pushable_speed: u8,
    pub off_board: u8,
    pub air_flag_452: u8,
    pub state_flag_81: u8,
    pub broken_bone_slowmo: u8,
    pub subject_flag_328: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferencePointInputs {
    pub head: [f32; 4],
    pub hips: [f32; 4],
    pub left_foot: [f32; 4],
    pub right_foot: [f32; 4],
    pub board: [f32; 4],
    pub centre_of_mass: [f32; 4],
    pub damped_centre_of_mass: [f32; 4],
    pub grind_position: [f32; 4],
    pub tracked_anchor: [f32; 4],
    pub incline_normal: [f32; 4],
    pub grinding: u8,
}

impl ReferencePointInputs {
    pub fn positions(self) -> [[f32; 4]; 10] {
        let feet = core::array::from_fn(|i| (self.left_foot[i] + self.right_foot[i]) * 0.5);
        [
            self.head,
            self.hips,
            if self.grinding != 0 {
                self.grind_position
            } else {
                self.board
            },
            self.centre_of_mass,
            self.board,
            self.centre_of_mass,
            self.tracked_anchor,
            feet,
            self.damped_centre_of_mass,
            self.incline_normal,
        ]
    }
}

impl RigModeSubject for Subject {
    fn flag_568(&mut self) -> u8 {
        self.in_ground_physics
    }
    fn flag_596(&mut self) -> u8 {
        self.grinding
    }
    fn flag_680(&mut self) -> u8 {
        self.subject_flag_328
    }
    fn flag_540(&mut self) -> u8 {
        self.trajectory_valid
    }
}
impl AvoidanceSubject for Subject {
    fn transform_376(&mut self) -> [[f32; 4]; 4] {
        self.transform
    }
    fn vector_452(&mut self) -> [f32; 4] {
        self.last_valid_ground_up
    }
    fn vector_444(&mut self) -> [f32; 4] {
        self.hips_position
    }
    fn context_532(&mut self) -> u32 {
        self.context
    }
    fn flag_608(&mut self) -> u8 {
        self.off_board
    }
    fn flag_548(&mut self) -> u8 {
        self.wiping_out
    }
}
impl AnchorTrackingSubject for Subject {
    fn flag_584(&mut self) -> u8 {
        self.physically_pushing
    }
    fn flag_588(&mut self) -> u8 {
        self.at_pushable_speed
    }
    fn value_476(&mut self) -> f32 {
        self.pumping_acceleration
    }
    fn flag_548(&mut self) -> u8 {
        self.wiping_out
    }
}
impl ReferenceHeightSubject for Subject {
    fn flag_656(&mut self) -> u8 {
        self.air_flag_452
    }
    fn flag_608(&mut self) -> u8 {
        self.off_board
    }
}
impl RigPositioningSubject for Subject {
    fn flag_668(&mut self) -> u8 {
        self.broken_bone_slowmo
    }
    fn flag_548(&mut self) -> u8 {
        self.wiping_out
    }
    fn flag_660(&mut self) -> u8 {
        self.state_flag_81
    }
    fn value_524(&mut self) -> f32 {
        self.state_height_32
    }
    fn transform_384(&mut self) -> [[f32; 4]; 4] {
        self.skeleton_root
    }
    fn position_360(&mut self, index: u32) -> [f32; 4] {
        self.reference_positions[index as usize]
    }
    fn context_532(&mut self) -> u32 {
        self.context
    }
}
