use bevy::prelude::*;

mod controllers;
pub(crate) mod gesture_catalog;
mod gesture_mapping_data;
pub(crate) mod gesture_mapping;
pub(crate) mod gesture_input;
pub(crate) mod platform;
pub(crate) use controllers::{ControllerInput, ControllerStatus};
use skate_core::input::tick::TickInput;

#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct PublishedTickInput(pub TickInput);

impl Default for PublishedTickInput {
    fn default() -> Self {
        Self(TickInput::new(
            0,
            skate_core::input::gameplay_map::GameplayActions::from_values([0.0; 18]),
            false,
        ))
    }
}

pub(crate) fn sample(input: &mut ControllerInput, state: skate_core::input::gamepad::GamepadState) {
 input.collect([Ok(platform::DevicePacket {number:input.publications as u32+1,state,subtype:1}),Err(platform::DeviceError::Disconnected),Err(platform::DeviceError::Disconnected),Err(platform::DeviceError::Disconnected)]);
 input.publish_actions();
}
