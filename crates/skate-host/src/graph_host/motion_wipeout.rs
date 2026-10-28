use skate_core::graph::conditions::NumericCondition;
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    Wipeout,
    EnableGestures,
}

#[derive(Default)]
pub struct Controls {
    pub seed_from_air_tweak: bool,
    pub gestures_enabled: bool,
    pub gesture: [f32; 2],
}

#[derive(Default)]
pub struct State {
    pub(super) ticks: u32,
    pub(super) lean: f32,
    pub(super) twist: f32,
    pub(super) twist_velocity: f32,
    pub(super) gesture: [f32; 2],
    pub(super) released: bool,
}

pub struct Settings {
    pub(super) threshold: f32,
    pub(super) twist_velocity: f32,
    pub(super) twist_acceleration: f32,
    pub(super) lean_velocity: f32,
    pub(super) twist_blend: f32,
    pub(super) lean_blend: f32,
    pub(super) gesture_y_blend: f32,
    pub(super) gesture_x_blend: f32,
}
impl Settings {
    pub fn load(data: &skate_data::collections::Collections) -> Result<Self, String> {
        let f = |name| data.float("anim_wipeout", "default", name);
        Ok(Self {
            threshold: f("controlled_drives_thresh")?,
            twist_velocity: f("clamp_twist_vel")?,
            twist_acceleration: f("clamp_twist_acc")?,
            lean_velocity: f("clamp_lean_vel")?,
            twist_blend: f("blend_twist")?,
            lean_blend: f("blend_lean")?,
            gesture_y_blend: f("blend_gesture_y")?,
            gesture_x_blend: f("blend_gesture_x")?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Physical {
    pub over_599: bool,
    pub collision_time_144: f32,
    pub no_support_time_548: f32,
    pub profile_148: u32,
    pub below_surface_82: bool,
    pub orientation_y: Option<f32>,
    pub hips_right_angle_496: f32,
    pub hips_up_angle_500: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Condition {
    Done,
    TimeToLand(NumericCondition),
    TimeSinceContact(NumericCondition),
    GestureType(u32),
    InWater(u32),
}
impl Condition {
    pub fn parse(a: &Attributes<'_>) -> Result<Option<Self>, String> {
        Ok(Some(match a.text("name").unwrap_or("") {
            "IsDoneWipingOut" => Self::Done,
            "WipeoutTimeToLand" => Self::TimeToLand(super::condition_nodes::numeric(a)),
            "WipeoutTimeSinceContact" => Self::TimeSinceContact(super::condition_nodes::numeric(a)),
            "GestureType" => Self::GestureType(match a.text("gesture") {
                Some("Freefall") => 0,
                Some("CannonBall") => 1,
                Some("JudoKick") => 2,
                Some("SwanDive") => 3,
                Some("Torpedo") => 4,
                other => return Err(format!("Invalid Wipeout gesture type {other:?}")),
            }),
            "IsInWater" => Self::InWater(match a.text("orientation") {
                Some("onback") => 0,
                Some("onfront") => 1,
                Some("either") => 2,
                other => return Err(format!("Invalid water orientation {other:?}")),
            }),
            _ => return Ok(None),
        }))
    }

    pub fn evaluate(self, physical: Option<Physical>) -> Result<bool, String> {
        let p = physical.ok_or("Wipeout condition requires completed physical output")?;
        Ok(match self {
            Self::Done => p.over_599,
            Self::TimeToLand(n) => n.matches(p.collision_time_144),
            Self::TimeSinceContact(n) => n.matches(p.no_support_time_548),
            Self::GestureType(kind) => p.profile_148 == kind,
            Self::InWater(orientation) => {
                if !p.below_surface_82 {
                    false
                } else {
                    let y = p
                        .orientation_y
                        .ok_or("IsInWater requires skeleton output80.Y")?;
                    u32::from(!(y > 0.0)) == orientation || orientation == 2
                }
            }
        })
    }
}
