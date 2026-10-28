use skate_core::{
    animation::{
        playback_parameters::{AttributeSink, SettableAttribute},
        skeleton_input::name::encode,
    },
    player::wipeout_state::{math, orientation},
};
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operation;
impl Operation {
    pub fn parse(attributes: &Attributes<'_>) -> Option<Self> {
        (attributes.text("name") == Some("AddRunoutAttribs")).then_some(Self)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub offboard_flag_331: bool,
    pub offboard_velocity_128: [f32; 4],
    pub reckoning_velocity_16: [f32; 4],
    pub reckoning_up_96: [f32; 4],
    pub skeleton_vector_0: [f32; 4],
    pub animation_mirrored: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    pub angle_degrees_8: f32,
    pub speed_12: Option<f32>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            angle_degrees_8: 0.0,
            speed_12: None,
        }
    }
}
impl State {
    pub fn begin(&mut self, observation: Option<Observation>) {
        let Some(observation) = observation else {
            return;
        };
        let velocity = if observation.offboard_flag_331 {
            observation.offboard_velocity_128
        } else {
            observation.reckoning_velocity_16
        };
        let mut angle = orientation::projected_angle(
            observation.skeleton_vector_0,
            velocity,
            observation.reckoning_up_96,
        );
        if observation.animation_mirrored {
            angle = -angle;
        }
        self.speed_12 = Some(math::length(velocity));
        self.angle_degrees_8 = math::wrap_angle(angle) * f32::from_bits(0x4265_2EE1);
    }

    pub fn update(&self, animation: Option<&mut impl AttributeSink>) -> Result<(), &'static str> {
        let Some(animation) = animation else {
            return Ok(());
        };
        let speed = self
            .speed_12
            .ok_or("AddRunoutAttribs speed12 has no successful Begin observation")?;
        for (name, value) in [
            ("BipedStartAngle", self.angle_degrees_8),
            ("BipedSpeed", speed),
        ] {
            animation.set_attribute(SettableAttribute {
                name: encode(name.as_bytes()),
                value,
                normalized: false,
                sequence_id: -1,
            });
        }
        Ok(())
    }
}

