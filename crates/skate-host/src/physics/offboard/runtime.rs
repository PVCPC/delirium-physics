use skate_core::player::offboard::{
    contact_packet::Packet,
    contact_queries::Layout,
    contact_toolkit::Toolkit,
    controller::{Controller, GroundResult, PlacementInput},
    ground_entry, ground_job, ground_query, ground_sync,
};
use skate_core::point_graph::PointGraph;
use skate_data::{animation_metadata::AnimationMetadata, collections::Collections};

pub(crate) struct Runtime {
    pub air_diagnostics: super::air_diagnostics::History,
    pub air_state: skate_core::player::offboard::air_state::State,
    pub air_prediction: Option<skate_core::player::offboard::air_prediction::Prediction>,
    pub air_blend_curve: PointGraph<8>,
    pub air_query_settings: skate_core::player::offboard::air_queries::Settings,
    pub jump_height: f32,
    pub jump_speed_scalar: f32,
    pub geometry_position: [f32; 4],
    pub feet: skate_core::player::offboard::board_possession::manager::State,
    pub board_policy: super::board_effects::Policy,
    pub standard_deck_drag: f32,
    pub possession: skate_core::player::offboard::board_possession::State,
    pub possession_settings: skate_core::player::offboard::board_possession::lifecycle::Settings,
    pub controller: Controller,
    pub ground: ground_entry::State,
    pub contacts: Toolkit,
    pub layout: Layout,
    pub retained_contact: Packet,
    pub geometry: Option<ground_query::GroundGeometry>,
    pub geometry_flags: [bool; 3],
    pub board_settings: ground_sync::BoardSettings,
    movement_curve: PointGraph<8>,
    turn_curve: PointGraph<8>,
}

impl Runtime {
    pub(crate) fn load(data: &Collections, metadata: &AnimationMetadata) -> Result<Self, String> {
        let settings = super::settings::Settings::load(data, metadata)?;
        Ok(Self {
            air_diagnostics: Default::default(),
            air_state: Default::default(),
            air_prediction: None,
            air_blend_curve: settings.air_blend_curve,
            air_query_settings: settings.air_query,
            jump_height: settings.jump_height,
            jump_speed_scalar: settings.jump_speed_scalar,
            geometry_position: [0.; 4],
            feet: Default::default(),
            board_policy: Default::default(),
            standard_deck_drag: data.float("physicsdeck", "default", "DeckAngularDrag")?
                * f32::from_bits(0x426f_ffff),
            possession: Default::default(),
            possession_settings: settings.possession,
            controller: Controller::new(settings.controller, settings.metrics),
            ground: Default::default(),
            contacts: Default::default(),
            layout: Default::default(),
            retained_contact: Default::default(),
            geometry: None,
            geometry_flags: [false; 3],
            board_settings: settings.board,
            movement_curve: settings.movement_vs_stick_angle,
            turn_curve: settings.turn_vs_stick_angle,
        })
    }
    pub(crate) fn place_ground(
        &mut self,
        input: &ground_entry::Input,
        previous_frame: ground_entry::Frame,
    ) {
        self.contacts.reset_contacts(&self.layout);
        self.retained_contact = Packet::default();
        let placement = self.ground.enter(input);
        self.controller.place(PlacementInput {
            frame: placement.frame,
            velocity: placement.planar_velocity,
            body_position: placement.body_position,
            current_state: 500,
            previous_state: input.previous_state_2504,
            previous_frame,
        });
    }
    pub(crate) fn begin_player_update(&mut self) {
        self.contacts.begin_player_update(&self.layout);
    }
    pub(crate) fn step_ground(&mut self, input: &ground_job::Input) -> GroundResult {
        if let Some(geometry) = self.geometry {
            let p = geometry.frame.position;
            self.geometry_position = [p.x, p.y, p.z, 0.];
        }
        let completed = (self.contacts.contact_age > 0).then_some(self.contacts.packet);
        let prepared = ground_job::prepare(
            &mut self.ground,
            &mut self.retained_contact,
            completed,
            self.geometry.take(),
            input,
            &self.movement_curve,
            &self.turn_curve,
        );
        self.geometry_flags = prepared.geometry_flags_752_to_754;
        self.controller.step_ground(&prepared.job)
    }
    pub(crate) fn completed_motion(&self) -> ground_sync::CompletedMotion {
        let output = self.controller.output();
        ground_sync::CompletedMotion {
            contact_position_192: self.retained_contact.position,
            contact_flags_368: self.retained_contact.flags,
            physical_frame_848: output.physical_frame,
            animation_frame_912: output.animation_frame,
            vector_976: output.surface_frame[0],
            vector_992: output.surface_frame[1],
            vector_1008: output.surface_frame[2],
            vector_1040: output.velocity,
            vector_1056: output.position,
            launch_1076: output.alternate,
            flag_1077: output.sliding,
        }
    }
}
