use super::{GamePhysics, skater::SkaterRuntime};
use crate::input::PublishedTickInput;
use bevy::prelude::*;
use skate_core::graph::intents::IntentMap;
use skate_core::input::{
    controller::{ActionMap, DerivedControllerInput, MagnitudeHeldSettings},
    riding_intentions::{self, PushPreferences, RidingIntent},
    wipeout_intentions,
};

#[derive(Resource)]
pub(crate) struct PlayerControls {
    pub controller: DerivedControllerInput,
    pub offboard_direction: Option<[f32; 4]>,
    pub intents: Vec<RidingIntent>,
    pub action_intents: IntentMap,
    pub ticks: u64,
    pub actor_flags: u32,
    pub bumper_state_502: bool,
    pub bumper_state_104: bool,
    pub preferences: PushPreferences,
    offboard_axes: Option<[f32; 2]>,
    gestures: Option<crate::input::gesture_input::GestureInput>,
}
impl Default for PlayerControls {
    fn default() -> Self {
        let mut controller = DerivedControllerInput::from_words([0; 26]);
        controller.initialize();
        Self {
            controller,
            offboard_direction: None,
            intents: Vec::new(),
            action_intents: IntentMap::new(),
            ticks: 0,
            actor_flags: 0,
            bumper_state_502: false,
            bumper_state_104: false,
            preferences: PushPreferences::default(),
            offboard_axes: None,
            gestures: None,
        }
    }
}

pub(super) fn sample(
    input: Res<PublishedTickInput>,
    physics: Res<GamePhysics>,
    skater: Res<SkaterRuntime>,
    camera: Res<crate::camera::CameraRuntime>,
    mut player: ResMut<PlayerControls>,
) {
    let mut map = input.0.actions();
    player
        .update_for_physics(&mut map, &physics, &skater, &camera)
        .unwrap_or_else(|error| panic!("Offboard controller publication: {error}"));
    player.publish_gestures(
        physics.animation_profile.physics_mode,
        skater.player_input.physical.state.state_16,
    );
}

impl PlayerControls {
    pub fn update_for_physics(
        &mut self,
        map: &mut impl ActionMap,
        physics: &GamePhysics,
        skater: &SkaterRuntime,
        camera: &crate::camera::CameraRuntime,
    ) -> Result<(), String> {
        let physical = &skater.player_input.physical;
        let remap = physical.state.category_12 == 500
            && physical.off_board.flag_304 == 0
            && physical.state.state_16 != 503;
        let axes = if remap {
            let frame = camera
                .frame
                .as_ref()
                .ok_or("Native offboard input requires a completed presentation camera frame")?;
            Some(camera_relative_axes(
                [map.value(64), map.value(65)],
                frame.basis.columns,
            ))
        } else {
            None
        };
        self.update(
            &mut SimulationActions {
                source: map,
                offboard_axes: axes,
            },
            physics.settings.step.simulation.time_step,
            physics.settings.input_magnitude_threshold,
            physical.scoring.capabilities_204,
        );
        self.offboard_axes = axes;
        self.offboard_direction = axes.map(|v| [v[0], 0., v[1], 0.]);
        Ok(())
    }

    pub fn simulation_actions<'a>(&self, source: &'a mut dyn ActionMap) -> SimulationActions<'a> {
        SimulationActions {
            source,
            offboard_axes: self.offboard_axes,
        }
    }

    pub fn load(root: &std::path::Path) -> Result<Self, String> {
        Ok(Self {
            gestures: Some(crate::input::gesture_input::GestureInput::load(root)?),
            ..Self::default()
        })
    }

    pub fn publish_gestures(&mut self, difficulty: u32, physical_state: u32) {
        if let Some(gestures) = &mut self.gestures {
            let words = self.controller.words();
            let axes = [[words[7], words[8]], [words[9], words[10]]].map(|p| p.map(f32::from_bits));
            gestures.publish(
                axes,
                difficulty,
                self.actor_flags,
                physical_state,
                &mut self.action_intents,
            );
        }
    }

    pub fn update(
        &mut self,
        map: &mut impl ActionMap,
        dt: f32,
        magnitude_threshold: f32,
        physical_capabilities: u32,
    ) {
        self.offboard_axes = None;
        self.controller.update(
            map,
            dt,
            self.bumper_state_502,
            self.bumper_state_104,
            &MagnitudeHeldSettings {
                attribute: Some(magnitude_threshold),
                missing_attribute_value: 0.0,
            },
        );
        self.intents =
            riding_intentions::produce(&self.controller, self.actor_flags, self.preferences);
        self.intents
            .extend(skate_core::input::manual_intentions::produce(
                &self.controller,
                self.actor_flags,
            ));
        self.intents.extend(wipeout_intentions::produce(
            &self.controller,
            self.actor_flags,
            physical_capabilities,
        ));
        self.intents
            .extend(skate_core::input::anticipation_intentions::produce(
                &self.controller,
            ));
        self.intents.extend(
            skate_core::input::trick_intentions::produce(&self.controller)
                .into_iter()
                .map(|intent| RidingIntent {
                    name: intent.name,
                    value: intent.value,
                }),
        );
        self.action_intents.clear();
        for intent in &self.intents {
            self.action_intents.insert(intent.name, intent.value);
        }
        self.ticks += 1;
    }
}

pub(crate) struct SimulationActions<'a> {
    source: &'a mut dyn ActionMap,
    offboard_axes: Option<[f32; 2]>,
}

impl ActionMap for SimulationActions<'_> {
    fn value(&mut self, action: u32) -> f32 {
        if let Some(axes) = self.offboard_axes {
            match action {
                64 => return axes[0],
                65 => return axes[1],
                _ => {}
            }
        }
        self.source.value(action)
    }

    fn state(&mut self, action: u32) -> u8 {
        if self.offboard_axes.is_some() && matches!(action, 64 | 65) {
            return u8::from(self.value(action) != 0.0);
        }
        self.source.state(action)
    }
}

fn camera_relative_axes(stick: [f32; 2], camera: [[f32; 3]; 3]) -> [f32; 2] {
    let [mut right, mut up, mut forward] = camera;
    if forward[1].abs() < f32::from_bits(0x3f7d_70a4) {
        right = normalize_camera_axis(cross_camera_axis([0.0, 1.0, 0.0], forward));
        forward = normalize_camera_axis(cross_camera_axis(right, [0.0, 1.0, 0.0]));
        up = normalize_camera_axis(cross_camera_axis(forward, right));
    }
    let local = [-stick[0], 0.0, stick[1]];
    let world: [f32; 3] = std::array::from_fn(|i| {
        forward[i].mul_add(local[2], up[i].mul_add(local[1], right[i] * local[0]))
    });
    let negative_x = if world[0] >= 0.0 { 0.0 } else { -world[0] };
    let positive_x = if world[0] > 0.0 { world[0] } else { 0.0 };
    let negative_z = if world[2] >= 0.0 { 0.0 } else { -world[2] };
    let positive_z = if world[2] <= 0.0 { 0.0 } else { world[2] };
    [positive_x - negative_x, positive_z - negative_z]
}

fn cross_camera_axis(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        (-a[2]).mul_add(b[1], a[1] * b[2]),
        (-a[0]).mul_add(b[2], a[2] * b[0]),
        (-a[1]).mul_add(b[0], a[0] * b[1]),
    ]
}

fn normalize_camera_axis(v: [f32; 3]) -> [f32; 3] {
    let squared = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    let mut inverse = squared.sqrt().recip();
    for _ in 0..2 {
        let correction = (-squared).mul_add(inverse * inverse, 1.0);
        inverse = (inverse * 0.5).mul_add(correction, inverse);
    }
    v.map(|component| component * inverse)
}

