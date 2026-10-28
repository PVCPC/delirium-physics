use super::super::Family;
use skate_core::{point_graph::PointGraph, riding::collision_response::CollisionResponseSettings};
use skate_data::collections::Collections;

pub(crate) struct Settings {
    pub(super) collision: CollisionResponseSettings,
    pub(super) animated_board_threshold: f32,
    vertical: [[[f32; 2]; 3]; 5],
}

impl Settings {
    pub(crate) fn load(data: &Collections) -> Result<Self, String> {
        let f = |name| data.float("physics_collision", "default", name);
        let graph = data
            .words::<20>("physics_collision", "default", "CollisionTorqueVsAngle")?
            .map(f32::from_bits);
        let mut vertical = [[[0.; 2]; 3]; 5];
        for (index, mode) in ["easy", "normal", "hardcore", "motorized", "test"]
            .into_iter()
            .enumerate()
        {
            for (family, fields) in [
                ["Hash_3097A69281990652", "Hash_FA4CDBAE0DFD1FAD"],
                ["Hash_B2B1170AFFC8AC69", "Hash_1B3E9F9C836D287D"],
                ["Hash_703829BD711E54DE", "Hash_F2473E9125079F0"],
            ]
            .into_iter()
            .enumerate()
            {
                vertical[index][family] = [
                    data.float("physics_mode", mode, fields[0])?,
                    data.float("physics_mode", mode, fields[1])?,
                ];
            }
        }
        Ok(Self {
            animated_board_threshold: data.float(
                "physics_animation",
                "default",
                "MaxDeckZAxisYForAnimatedDeck",
            )?,
            collision: CollisionResponseSettings {
                maximum_velocity_delta: f("MaxVelDelta")?,
                force_y_offset: f("ForceYOffset")?,
                force_scalar: f("CollisionForceScalar")?,
                target_displacement_velocity: f("TargetDisplacementVel")?,
                torque_vs_angle: PointGraph {
                    x: graph[4..12].try_into().unwrap(),
                    y: graph[12..20].try_into().unwrap(),
                },
            },
            vertical,
        })
    }

    pub(super) fn vertical(&self, family: Family, mode: u32, strength: f32) -> Result<f32, String> {
        let modes = self
            .vertical
            .get(mode as usize)
            .ok_or_else(|| format!("Invalid grind jump physics mode {mode}"))?;
        let group = match family {
            Family::Boardslide => 1,
            Family::Tipslide => 2,
            _ => 0,
        };
        let [low, high] = modes[group];
        Ok((1. - strength).mul_add(low, high * strength))
    }
}

pub(super) fn side(family: Family, geometry_kind: u32) -> f32 {
    match family {
        Family::Boardslide | Family::Darkslide => {
            if geometry_kind == 2 {
                f32::from_bits(0x3f66_6666)
            } else {
                0.
            }
        }
        Family::Backslash => f32::from_bits(0x3f66_6666),
        _ => f32::from_bits(0x3f00_0000),
    }
}
