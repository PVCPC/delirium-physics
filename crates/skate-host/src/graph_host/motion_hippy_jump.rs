use skate_core::animation::skeleton_input::name::encode;
use skate_core::point_graph::PointGraph;
use skate_data::collections::Collections;

use super::motion_animation::MotionAnimation;
use skate_core::animation::playback_parameters::{AttributeSink, SettableAttribute};

pub(super) struct Settings {
    pub antic_length_to_hippy_height: PointGraph<8>,
}

impl Settings {
    pub(super) fn load(data: &Collections) -> Result<Self, String> {
        let words =
            data.words::<20>("anim_motion", "hippy_flip", "antic_length_to_hippy_height")?;
        Ok(Self {
            antic_length_to_hippy_height: PointGraph {
                x: std::array::from_fn(|i| f32::from_bits(words[4 + i])),
                y: std::array::from_fn(|i| f32::from_bits(words[12 + i])),
            },
        })
    }
}

#[derive(Default)]
pub(super) struct State {
    active: bool,
    elapsed: f32,
}

impl State {
    pub(super) fn begin(&mut self) {
        self.active = true;
        self.elapsed = 0.0;
    }

    pub(super) fn update(&mut self, animation: &mut MotionAnimation, settings: &Settings) {
        debug_assert!(self.active);
        self.elapsed += 1.0 / 60.0;
        animation.set_attribute(SettableAttribute {
            name: encode(b"TrickHeight"),
            value: settings.antic_length_to_hippy_height.evaluate(self.elapsed),
            normalized: false,
            sequence_id: -1,
        });
    }

    pub(super) fn end(&mut self) {
        self.active = false;
        self.elapsed = 0.0;
    }
}

