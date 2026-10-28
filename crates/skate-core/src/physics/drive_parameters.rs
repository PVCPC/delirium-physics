const fn retail_f32(bits: u32) -> f32 {
    f32::from_bits(bits)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum RetailDriveType {
    NoDrive = 0,
    SoftDrive = 1,
    HardDrive = 2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct RetailDriveParams {
    pub spring_or_max_velocity: f32,
    pub damping: f32,
    pub max_strength: f32,
    pub drive_type: RetailDriveType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct RetailDriveDynamics {
    pub linear: RetailDriveParams,
    pub angular: RetailDriveParams,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailTruckDriveSettings {
    pub use_linear_drives: bool,
    pub use_hard_linear_drives: bool,
    pub angular_displacement: f32,
    pub angular_damping: f32,
    pub angular_strength: f32,
}

impl RetailTruckDriveSettings {
    pub const STOCK: Self = Self {
        use_linear_drives: false,
        use_hard_linear_drives: true,
        angular_displacement: 10.0,
        angular_damping: 0.0,
        angular_strength: 11.0,
    };
}

impl RetailDriveParams {
    pub const DISABLED: Self = Self {
        spring_or_max_velocity: 0.0,
        damping: 0.0,
        max_strength: 0.0,
        drive_type: RetailDriveType::NoDrive,
    };
}

pub fn retail_truck_drive_dynamics(settings: RetailTruckDriveSettings) -> RetailDriveDynamics {
    let linear = if !settings.use_linear_drives {
        RetailDriveParams::DISABLED
    } else if settings.use_hard_linear_drives {
        RetailDriveParams {
            spring_or_max_velocity: retail_f32(0x45BB_7FFF),
            damping: 0.0,
            max_strength: retail_f32(0x48AF_C7FF),
            drive_type: RetailDriveType::HardDrive,
        }
    } else {
        RetailDriveParams {
            spring_or_max_velocity: retail_f32(0x47C3_5000),
            damping: soft_linear_damping(),
            max_strength: retail_f32(0x48AF_C7FF),
            drive_type: RetailDriveType::SoftDrive,
        }
    };
    RetailDriveDynamics {
        linear,
        angular: RetailDriveParams {
            spring_or_max_velocity: settings.angular_displacement * retail_f32(0x426F_FFFF),
            damping: settings.angular_damping,
            max_strength: settings.angular_strength * retail_f32(0x4560_FFFE),
            drive_type: RetailDriveType::HardDrive,
        },
    }
}

pub const fn retail_wheel_drive_dynamics(use_hard_drives: bool) -> RetailDriveDynamics {
    if use_hard_drives {
        RetailDriveDynamics {
            linear: RetailDriveParams {
                spring_or_max_velocity: retail_f32(0x45BB_7FFF),
                damping: 0.0,
                max_strength: retail_f32(0x48AF_C7FF),
                drive_type: RetailDriveType::HardDrive,
            },
            angular: RetailDriveParams {
                spring_or_max_velocity: 0.0,
                damping: 0.0,
                max_strength: 0.0,
                drive_type: RetailDriveType::NoDrive,
            },
        }
    } else {
        RetailDriveDynamics {
            linear: RetailDriveParams {
                spring_or_max_velocity: retail_f32(0x4561_0000),
                damping: retail_f32(0x4270_0000),
                max_strength: retail_f32(0x470C_9FFF),
                drive_type: RetailDriveType::SoftDrive,
            },
            angular: RetailDriveParams {
                spring_or_max_velocity: 0.0,
                damping: 0.0,
                max_strength: 0.0,
                drive_type: RetailDriveType::NoDrive,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct RetailDriveFrameRaw {
    pub quaternion_lanes: [u32; 4],
    pub translation_lanes: [u32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct RetailDriveFramesRaw {
    pub body_a: RetailDriveFrameRaw,
    pub body_b: RetailDriveFrameRaw,
}

fn soft_linear_damping() -> f32 {
    let spring = retail_f32(0x47C3_5000);
    let mut reciprocal_root = super::native_arithmetic::reciprocal_square_root_estimate(spring);
    for _ in 0..2 {
        let squared = reciprocal_root * reciprocal_root;
        let half = reciprocal_root * 0.5;
        let residual = (-spring).mul_add(squared, 1.0);
        reciprocal_root = half.mul_add(residual, reciprocal_root);
    }
    let root = if spring == 0.0 {
        0.0
    } else {
        spring * reciprocal_root
    };
    root * retail_f32(0x4000_0000)
}

