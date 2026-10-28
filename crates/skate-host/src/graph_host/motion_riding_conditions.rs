use super::motion::MotionHost;
use skate_core::{
    graph::conditions::NumericCondition,
    math::Vector3,
    physics::board_motion_output::{dot, length},
    trigonometry,
};
use skate_data::state_graph::attributes::Attributes;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug)]
pub struct RidingConditionInputs {
    pub com_velocity: [f32; 4],
    pub skeleton_x: [f32; 4],
    pub skeleton_z: [f32; 4],
    pub skate_up_y: f32,
    pub surface_up_y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VelocityAxis {
    X,
    Y,
    Z,
    All,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MotionRidingCondition {
    Random,
    ComVelocity {
        axis: VelocityAxis,
        numeric: NumericCondition,
    },
    SkateSlope(NumericCondition),
    SurfaceSlope(NumericCondition),
}

impl MotionRidingCondition {
    pub fn recognizes(name: &str) -> bool {
        matches!(
            name,
            "RandomCond" | "ComVelCompare" | "SkateSlope" | "SurfaceSlope"
        )
    }

    pub fn parse(a: &Attributes<'_>) -> Result<Self, String> {
        Ok(match a.text("name").unwrap_or("") {
            "RandomCond" => Self::Random,
            "ComVelCompare" => Self::ComVelocity {
                axis: match a.text("axis").unwrap_or("0").as_bytes().first() {
                    Some(b'x' | b'X') => VelocityAxis::X,
                    Some(b'y' | b'Y') => VelocityAxis::Y,
                    Some(b'z' | b'Z') => VelocityAxis::Z,
                    _ => VelocityAxis::All,
                },
                numeric: super::condition_nodes::numeric(a),
            },
            "SkateSlope" => Self::SkateSlope(super::condition_nodes::numeric(a)),
            "SurfaceSlope" => Self::SurfaceSlope(super::condition_nodes::numeric(a)),
            name => return Err(format!("Unknown riding condition {name}")),
        })
    }

    pub fn evaluate(&self, host: &MotionHost) -> Result<bool, String> {
        if matches!(self, Self::Random) {
            return Ok(host.condition_random.next_u32()? & 1 == 0);
        }
        let p = host
            .riding_conditions
            .as_ref()
            .ok_or("MotionGraph requires the actual COM and slope publication")?;
        Ok(match *self {
            Self::ComVelocity { axis, numeric } => {
                let velocity = xyz(p.com_velocity);
                let value = match axis {
                    VelocityAxis::X => dot(velocity, xyz(p.skeleton_x)),
                    VelocityAxis::Y => velocity.y,
                    VelocityAxis::Z => dot(velocity, xyz(p.skeleton_z)),
                    VelocityAxis::All => length(velocity),
                };
                numeric.matches(value)
            }
            Self::SkateSlope(numeric) => numeric.matches(slope(p.skate_up_y)),
            Self::SurfaceSlope(numeric) => numeric.matches(slope(p.surface_up_y)),
            Self::Random => unreachable!("Random condition evaluated above"),
        })
    }
}

fn xyz(v: [f32; 4]) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}

fn slope(up_y: f32) -> f32 {
    90.0 - trigonometry::asin(up_y) * f32::from_bits(0x4265_2ee1)
}

#[derive(Debug)]
pub struct MotionRandom(Mutex<[u32; 8]>);

impl MotionRandom {
    pub fn new() -> Self {
        Self(Mutex::new([
            0,
            0,
            0xf22d_0e56,
            0x8831_26e9,
            0xc624_dd2f,
            0x0702_c49c,
            0x9e35_3f7d,
            0x6fdf_3b64,
        ]))
    }

    pub fn next_u32(&self) -> Result<u32, String> {
        let mut words = self
            .0
            .lock()
            .map_err(|_| "MotionGraph random state lock poisoned")?;
        let last = words[7];
        let previous = words[6];
        let mut sum = previous.wrapping_add(last);
        let mut carry = u32::from(sum < last || sum < previous);
        words[6] = sum;
        for index in (2..=5).rev() {
            let previous = words[index];
            sum = previous.wrapping_add(sum).wrapping_add(carry);
            carry = u32::from(sum < previous);
            words[index] = sum;
        }
        words[7] = last.wrapping_add(1);
        if words[7] == 0 {
            for index in (2..=6).rev() {
                words[index] = words[index].wrapping_add(1);
                if words[index] != 0 {
                    break;
                }
            }
        }
        words[1] = words[1].wrapping_add(1);
        Ok(words[2])
    }
}
