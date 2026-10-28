use super::{GamePhysics, PlayerControls, SkaterRuntime};
use crate::{camera::CameraRuntime, graph_runtime::StockGraphs, input::ControllerInput};
use bevy::prelude::*;
use skate_data::skate_map::{Collision, Geometry, Rail, SkateMap};
use std::path::Path;

#[derive(Clone, Copy, Default)]
pub struct Controls {
    pub buttons: u16,
    pub triggers: [u8; 2],
    pub left: [i16; 2],
    pub right: [i16; 2],
}
pub struct Session {
    physics: GamePhysics,
    skater: SkaterRuntime,
    controls: PlayerControls,
    graphs: StockGraphs,
    input: ControllerInput,
    camera: CameraRuntime,
    markers: crate::session_marker::Runtime,
}
pub struct Pose {
    pub root: Mat4,
    pub bones: Vec<Mat4>,
    pub names: Vec<String>,
    pub camera: Option<(Vec3, Mat3, f32)>,
    pub velocity: Vec3,
    pub spin: Vec3,
    pub tick: u64,
    pub state: String,
    pub trick: String,
    pub board: Vec<Mat4>,
}
impl Session {
    pub fn new(
        root: &Path,
        triangles: Vec<[[f32; 3]; 3]>,
        rails: Vec<Vec<[f32; 3]>>,
        spawn: [f32; 3],
        heading: f32,
    ) -> Result<Self, String> {
        let started = std::time::Instant::now();
        eprintln!("IW4L_SKATE_LOAD begin");
        skate_data::input_config::StockGameplayConfig::load(root).map_err(|e| e.to_string())?;
        let assets = skate_data::GameAssets::load(root).map_err(|e| e.to_string())?;
        let graphs = StockGraphs::load(root, &assets)?;
        let map = collision_map(triangles, rails, spawn, heading);
        eprintln!("IW4L_SKATE_LOAD graphs {}ms", started.elapsed().as_millis());
        let physics = GamePhysics::load_with_map(root, Some(&map))?;
        eprintln!(
            "IW4L_SKATE_LOAD physics {}ms",
            started.elapsed().as_millis()
        );
        let skater = SkaterRuntime::load(root, &graphs, &physics, "easy")?;
        eprintln!("IW4L_SKATE_LOAD skater {}ms", started.elapsed().as_millis());
        Ok(Self {
            physics,
            skater,
            controls: PlayerControls::load(root)?,
            graphs,
            input: ControllerInput::default(),
            camera: CameraRuntime::load(root)?,
            markers: crate::session_marker::Runtime::load(root)?,
        })
    }
    pub fn collision_builder(&self) -> CollisionBuilder {
        CollisionBuilder {
            material: self.physics.floor_material(),
        }
    }
    pub fn install_collision(&mut self, prepared: PreparedCollision) -> Result<(), String> {
        self.physics
            .install_world(prepared.world, std::sync::Arc::clone(&prepared.grind))?;
        self.skater.trajectory.bind_grind_world(prepared.grind);
        Ok(())
    }
    pub fn period(&self) -> f32 {
        self.physics.period().as_secs_f32()
    }
    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        if aspect_ratio.is_finite() && aspect_ratio > 0. {
            self.camera.set_aspect_ratio(aspect_ratio);
        }
    }
    pub fn preload(root: &Path) -> Result<(), String> {
        crate::skater_animation::AnimationSource::load(root).map(|_| ())
    }
    pub fn activate(&mut self, spawn: [f32; 3], heading: f32) -> Result<Pose, String> {
        self.input = ControllerInput::default();
        self.markers.suspend();
        if self.physics.ticks == 0 {
            self.tick(Controls::default())?;
        }
        let mut transform = Mat4::from_rotation_translation(
            Quat::from_rotation_y(heading),
            Vec3::from_array(spawn),
        )
        .to_cols_array_2d();
        transform[3][3] = 0.;
        self.skater.travel_to(transform)?;
        for _ in 0..4 {
            self.tick(Controls::default())?;
        }
        self.input = ControllerInput::default();
        Ok(self.pose())
    }
    pub fn collect(&mut self, frame: InputFrame, dt: f32) {
        self.input.collect(frame.samples);
        self.markers.collect_time(f64::from(dt));
    }
    pub fn suspend_input(&mut self) {
        self.input = ControllerInput::default();
        self.markers.suspend();
    }
    pub fn advance(&mut self) -> Result<(), String> {
        self.input.publish_actions();
        self.advance_published()
    }
    fn advance_published(&mut self) -> Result<(), String> {
        let published = self.input.tick_input();
        self.markers
            .advance(&self.input, &self.physics, &mut self.skater);
        let mut actions = published.actions();
        self.controls.update_for_physics(
            &mut actions,
            &self.physics,
            &self.skater,
            &self.camera,
        )?;
        self.controls.publish_gestures(
            self.physics.animation_profile.physics_mode,
            self.skater.player_input.physical.state.state_16,
        );
        super::frame::advance(
            &mut self.physics,
            &mut self.skater,
            &mut self.controls,
            &self.graphs,
            &mut actions,
            published.controller_available(),
            &mut self.camera,
        )
    }
    pub fn tick(&mut self, input: Controls) -> Result<(), String> {
        crate::input::sample(
            &mut self.input,
            skate_core::input::gamepad::GamepadState {
                buttons: input.buttons,
                triggers: input.triggers,
                left: input.left,
                right: input.right,
            },
        );
        self.advance_published()
    }
    pub fn pose(&self) -> Pose {
        let v = self.physics.board.bodies()[skate_core::physics::board::BodyId::Deck.index()]
            .rates
            .linear_velocity;
        Pose {
            root: crate::animation::native_matrix(
                self.skater.animated_skeleton.roots.animation_to_world,
            ),
            bones: self
                .skater
                .render_pose
                .iter()
                .map(|m| crate::animation::native_matrix(*m))
                .collect(),
            names: self.skater.animation.evaluator.frames.bone_names.clone(),
            camera: self.camera.frame.as_ref().map(|f| {
                (
                    Vec3::new(f.position[0], f.position[1], f.position[2]),
                    Mat3::from_cols_array_2d(&f.basis.columns),
                    f.field_of_view_degrees,
                )
            }),
            velocity: Vec3::new(v.x, v.y, v.z),
            spin: {
                let w = self.physics.board.bodies()[skate_core::physics::board::BodyId::Deck.index()].rates.angular_velocity;
                Vec3::new(w.x, w.y, w.z)
            },
            tick: self.physics.ticks,
            state: format!("{:?}", self.skater.player_state.current()),
            trick: self.skater.scoring.trick().to_string(),
            board: self
                .physics
                .board
                .bodies()
                .iter()
                .map(|b| {
                    let (c, p) = (b.rates.basis.columns, b.rates.position);
                    Mat4::from_cols(
                        Vec3::from(c[0]).extend(0.),
                        Vec3::from(c[1]).extend(0.),
                        Vec3::from(c[2]).extend(0.),
                        Vec4::new(p.x, p.y, p.z, 1.),
                    )
                })
                .collect(),
        }
    }
}

pub struct PreparedCollision {
    world: skate_core::physics::board_world::BoardWorld,
    grind: std::sync::Arc<crate::grind_world::StaticProvider>,
}

#[derive(Clone, Copy)]
pub struct CollisionBuilder {
    material: skate_core::physics::contact::RetailContactMaterial,
}

impl CollisionBuilder {
    pub fn build(
        &self,
        triangles: Vec<[[f32; 3]; 3]>,
        rails: Vec<Vec<[f32; 3]>>,
    ) -> Result<PreparedCollision, String> {
        let map = collision_map(triangles, rails, [0.; 3], 0.);
        Ok(PreparedCollision {
            world: crate::skate_world::collision_world(&map, self.material)?,
            grind: std::sync::Arc::new(crate::grind_world::StaticProvider::new(Some(&map))?),
        })
    }
}

fn collision_map(
    triangles: Vec<[[f32; 3]; 3]>,
    rails: Vec<Vec<[f32; 3]>>,
    spawn: [f32; 3],
    heading: f32,
) -> SkateMap {
    SkateMap {
            version: 14,
            name: "IW4L collision".into(),
            spawn,
            heading,
            environment: vec![],
            materials: vec![skate_data::skate_map::Material {
                name: "MW2".into(),
                flags: 0,
                friction: 0.8,
                restitution: 0.,
                color: [1.; 3],
                roughness: 1.,
                emissive: 0.,
                textures: [0; 5],
                indirect_strength: 1.,
                alpha_mode: 0,
                alpha_cutoff: 0.5,
                audio: 0,
                physics: 0,
                pattern: 0,
                depth_layer: None,
                retail_definition: None,
            }],
            textures: vec![],
            geometry: Geometry {
                vertices: vec![],
                indices: vec![],
                collision: triangles
                    .into_iter()
                    .map(|points| Collision {
                        points,
                        surface: 0,
                        material: 1,
                        native_edges: None,
                    })
                    .collect(),
            },
            rails: rails
                .into_iter()
                .enumerate()
                .map(|(i, p)| Rail {
                    name: format!("iw4_edge_{i}"),
                    closed: false,
                    points: p,
                    native: None,
                })
                .collect(),
            doors: vec![],
            lights: vec![],
            routes: vec![],
            extensions: vec![],
    }
}

#[derive(Default)]
pub struct ControllerTransport {
    capabilities: [crate::input::platform::CapabilityCache; 4],
}
pub struct InputFrame {
    samples: [Result<crate::input::platform::DevicePacket, crate::input::platform::DeviceError>; 4],
}
impl ControllerTransport {
    pub fn poll(&mut self) -> InputFrame {
        InputFrame {
            samples: std::array::from_fn(|i| {
                crate::input::platform::poll_cached(i, &mut self.capabilities[i])
            }),
        }
    }
}
impl InputFrame {
    pub fn neutral() -> Self {
        Self {
            samples: std::array::from_fn(|_|Err(crate::input::platform::DeviceError::Disconnected)),
        }
    }
    pub fn from_pad(buttons: u16, triggers: [u8; 2], left: [i16; 2], right: [i16; 2], packet: u32) -> Self {
        let mut frame = Self::neutral();
        frame.samples[0] = Ok(crate::input::platform::DevicePacket {
            number: packet,
            state: skate_core::input::gamepad::GamepadState { buttons, triggers, left, right },
            subtype: 1,
        });
        frame
    }
    pub fn controller(&self) -> Option<usize> {
        self.samples.iter().position(Result::is_ok)
    }
    pub fn buttons(&self) -> u16 {
        self.samples
            .iter()
            .find_map(|s| s.as_ref().ok().map(|s| s.state.buttons))
            .unwrap_or(0)
    }
}
