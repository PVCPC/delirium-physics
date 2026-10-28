mod frames;
pub use frames::{set_child_angular_frame, set_parent_angular_frame};

use super::drive_parameters::{RetailDriveDynamics, RetailDriveParams, RetailDriveType};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookDriveState {
    pub frames: [u32; 16],
    pub dynamics: [u32; 8],
}

impl HookDriveState {
    pub fn initial() -> Self {
        let mut state = Self {
            frames: [0; 16],
            dynamics: [0; 8],
        };
        let identity =
            [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0].map(f32::to_bits);
        set_child_angular_frame(&mut state.frames, identity);
        set_parent_angular_frame(&mut state.frames, identity);
        state.disable_linear();
        state.disable_angular();
        state
    }

    pub fn enable_animation_soft(&mut self, animated: &mut u8) {
        *animated = 1;
        self.dynamics[..4].copy_from_slice(&soft(5000.0, f32::from_bits(0x44e0_fffe)));
        self.enable_angular_soft();
    }

    pub fn enable_angular_soft(&mut self) {
        self.dynamics[4..].copy_from_slice(&soft(20000.0, f32::from_bits(0x470c_9fff)));
    }

    pub fn enable_angular_only(&mut self, animated: &mut u8) {
        *animated = 1;
        self.enable_angular_soft();
        self.disable_linear();
    }

    pub fn disable_animation(&mut self, animated: &mut u8) {
        *animated = 0;
        self.disable_linear();
        self.disable_angular();
    }

    pub fn disable_linear(&mut self) {
        self.dynamics[..4].copy_from_slice(&[0, 0, 0, 2]);
    }
    pub fn disable_angular(&mut self) {
        self.dynamics[4..].copy_from_slice(&[0, 0, 0, 2]);
    }

    pub fn solver_dynamics(&self) -> RetailDriveDynamics {
        let params = |i| RetailDriveParams {
            spring_or_max_velocity: f32::from_bits(self.dynamics[i]),
            damping: f32::from_bits(self.dynamics[i + 1]),
            max_strength: f32::from_bits(self.dynamics[i + 2]),
            drive_type: match self.dynamics[i + 3] {
                1 => RetailDriveType::SoftDrive,
                2 => RetailDriveType::HardDrive,
                _ => RetailDriveType::NoDrive,
            },
        };
        RetailDriveDynamics {
            linear: params(0),
            angular: params(4),
        }
    }
}

fn soft(spring: f32, strength: f32) -> [u32; 4] {
    let damping = (spring * frames::rsqrt(spring)) * 2.0;
    [spring.to_bits(), damping.to_bits(), strength.to_bits(), 1]
}
