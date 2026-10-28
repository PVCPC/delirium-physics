use std::path::Path;
use bevy::prelude::Resource;
use skate_core::{camera::{CameraFrame, CameraMan, CompassSettings, ManagerSettings,
    MovingObstacleProvider, ShakeSamples, SimulationRateRequest, SlowMotionSettings},
    physics::board_world::BoardWorld, point_graph::PointGraph};
use skate_data::collections::Collections;
use super::{collision::CameraCollision, graph::CameraGraph, graph_subject::CameraGraphEnvironment,
    settings, shake_data, shot_data::StockShots, subject::{CameraSubjectSnapshot, SubjectPublisher},
    trajectory::{CameraTrajectory, TrajectoryResult}};

#[derive(Resource)]
pub(crate) struct CameraRuntime {
    manager: CameraMan,
    subject: SubjectPublisher,
    graph: CameraGraph,
    shots: StockShots,
    settings: ManagerSettings,
    compass_settings: CompassSettings,
    shakes: [ShakeSamples; 2],
    trajectories: [TrajectoryResult; 3],
    pub frame: Option<CameraFrame>,
    pub latest_subject: Option<CameraSubjectSnapshot>,
    pub simulation_rate_requests: Vec<SimulationRateRequest>,
}

impl CameraRuntime {
    pub fn load(root: &Path) -> Result<Self, String> {
        let data = Collections::load(root)?;
        let values = data.words::<32>("slowmotion_controller", "default", "timescale")?
            .map(f32::from_bits);
        let slow_motion = SlowMotionSettings {
            timescale: PointGraph {
                x: core::array::from_fn(|i| values[i]),
                y: core::array::from_fn(|i| values[i + 16]),
            },
            fps_at_scale_one: data.float("slowmotion_controller", "default", "fps_at_scale_one")?,
        };
        let graph = CameraGraph::load(&root.join(
            "private/stock/data/script/camera/Default_cameragraph.stategraph"), slow_motion)?;
        let samples = |name: &str| -> Result<ShakeSamples, String> {
            let path = root.join("private/stock/data/camera").join(name);
            shake_data::parse(&std::fs::read_to_string(&path)
                .map_err(|e| format!("{}: {e}", path.display()))?)
        };
        Ok(Self { manager: CameraMan::new(), subject: SubjectPublisher::new(), graph,
            shots: StockShots::from_collections(&data)?, settings: settings::manager_settings(&data)?,
            compass_settings: settings::compass_settings(&data)?, shakes: [samples("1.shk")?, samples("2.shk")?],
            trajectories: core::array::from_fn(|_| TrajectoryResult::new()), frame: None,
            latest_subject: None,
            simulation_rate_requests: Vec::new() })
    }

    pub fn set_aspect_ratio(&mut self, value: f32) {
        self.manager.state.aspect_ratio = value;
    }
    pub fn selected_shot(&self) -> &str { &self.manager.shots.current().name }

    pub fn advance(&mut self, dt: f32, snapshot: CameraSubjectSnapshot,
        world: &BoardWorld, query_gravity: [f32; 4], environment: &CameraGraphEnvironment,
        moving: &mut impl MovingObstacleProvider) -> Result<CameraFrame, String> {
        if let Some(previous) = self.latest_subject.as_ref()
            && snapshot.tick <= previous.tick
        {
            return Err(format!(
                "Camera received non-monotonic subject tick: previous={}, current={}",
                previous.tick, snapshot.tick,
            ));
        }
        self.latest_subject = Some(snapshot);
        let mut subject = self.subject.publish(snapshot, &self.manager, self.compass_settings)?;
        self.manager.prepare(&subject, self.settings);
        let requests = self.graph.update(dt, &mut self.manager, &subject,
            snapshot.graph, environment, &self.shots)?;
        self.simulation_rate_requests.extend(requests);
        let [a, b, c] = &mut self.trajectories;
        let mut trajectories = [a, b, c].map(|result| CameraTrajectory {
            world, gravity: query_gravity, result,
        });
        let mut collision = CameraCollision::new(world);
        let frame = self.manager.update(dt, &mut subject, self.settings,
            [&self.shakes[0], &self.shakes[1]], &mut trajectories, moving, &mut collision)?;
        if let Some(error) = collision.error { return Err(error); }
        if let Some(error) = self.trajectories.iter().find_map(|v| v.error.as_ref()) {
            return Err(error.clone());
        }
        if !frame.position.iter().all(|v| v.is_finite())
            || !frame.basis.columns.iter().flatten().all(|v| v.is_finite())
            || !frame.field_of_view_degrees.is_finite() {
            return Err(format!(
                "Normal gameplay camera produced a non-finite frame: frame={frame:?}; lens_length={:?}; aspect_ratio={:?}; subject_transform={:?}; skeleton_root={:?}; ground_normal={:?}; launch_position={:?}; landing_position={:?}",
                self.manager.shots.interpolated.lens_length,
                self.manager.state.aspect_ratio,
                subject.rig.transform,
                subject.rig.skeleton_root,
                subject.ground_normal,
                subject.launch_position,
                subject.landing_position,
            ));
        }
        self.frame = Some(frame);
        Ok(frame)
    }
}
