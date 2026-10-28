use super::{
    LaunchInfo, Prediction, QueryRequest, QueryResult, SelectorInput, SelectorSettings, SurfaceHit,
    Trajectory, launch,
    math::*,
    scoring::{self, Candidate},
};

#[derive(Clone, Copy, Debug)]
pub struct WorldWithoutGrindEdges;

#[derive(Clone, Copy, Debug)]
pub struct Selection {
    pub candidate_index: usize,
    pub prediction: Prediction,
    pub start_velocity: Vector,
    pub landing_normal: Vector,
    pub collision_velocity: Vector,
    pub collision_position: Vector,
    pub com_trajectory: Trajectory,
    pub surface_category: u32,
    pub wall_ride: bool,
    pub grind: Option<super::grind::GrindTarget>,
}
#[derive(Clone, Debug, Default)]
pub struct TrajectorySelector {
    launch_info: Option<LaunchInfo>,
    batch: Option<launch::LaunchBatch>,
    candidates: Vec<Candidate>,
    selection: Option<Selection>,
    selected_index: Option<usize>,
    grind_locked_to_middle: bool,
    grind_normal: Option<Vector>,
    pass: u16,
    adjusted_on_vert: bool,
    pending: bool,
    valid: bool,
    just_changed: bool,
    all_predictions_missed: bool,
    suggested_normal: Option<Vector>,
}
impl TrajectorySelector {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn valid(&self) -> bool {
        self.valid
    }
    pub fn just_changed(&self) -> bool {
        self.just_changed
    }
    pub fn all_predictions_missed(&self) -> bool {
        self.all_predictions_missed
    }
    pub fn suggested_normal(&self) -> Option<Vector> {
        self.suggested_normal
    }
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }
    pub fn grind_locked_to_middle(&self) -> bool {
        self.grind_locked_to_middle
    }
    pub fn grind_normal(&self) -> Option<Vector> {
        self.grind_normal
    }
    pub fn launch_info(&self) -> Option<&LaunchInfo> {
        self.launch_info.as_ref()
    }
    pub fn requests(&self) -> &[QueryRequest] {
        self.batch.as_ref().map_or(&[], |b| b.requests.as_slice())
    }
    pub fn com_displacement(&self) -> Option<Vector> {
        self.batch.as_ref().map(|b| b.com_displacement)
    }
    pub fn local_com_position(&self) -> Option<Vector> {
        self.batch.as_ref().map(|b| b.local_com_position)
    }
    pub fn local_board_position(&self) -> Option<Vector> {
        self.batch.as_ref().map(|b| b.local_board_position)
    }
    pub fn board_position(&self) -> Option<Vector> {
        self.batch.as_ref().map(|b| b.board_position)
    }
    pub fn query_origin(&self) -> Option<Vector> {
        self.batch.as_ref().map(|b| b.origin)
    }
    pub fn update_without_completion(&mut self) -> bool {
        self.just_changed = false;
        self.valid
    }
    pub fn cancel_pending(&mut self) {
        self.pending = false;
    }

    pub fn reset(&mut self) {
        self.valid = false;
        self.suggested_normal = None;
        self.grind_normal = None;
        self.just_changed = false;
        self.all_predictions_missed = false;
        self.pass = 0;
        self.grind_locked_to_middle = false;
        if let Some(selection) = &mut self.selection {
            selection.com_trajectory = Trajectory {
                position: ZERO,
                velocity: ZERO,
                acceleration: ZERO,
                duration: -1.0,
            };
        }
    }

    pub fn launch(
        &mut self,
        mut info: LaunchInfo,
        input: SelectorInput,
        s: &SelectorSettings,
    ) -> Result<bool, String> {
        if self.pending {
            return Ok(false);
        }
        if info.trajectory_count == 0 {
            return Err("GetLaunchInfo supplied zero trajectory candidates".into());
        }
        self.pass = 0;
        self.valid = false;
        self.selected_index = None;
        self.grind_normal = None;
        self.adjusted_on_vert = launch::adjust_velocity(&mut info, input, s);
        self.launch_info = Some(info);
        self.launch_pass(input, s);
        Ok(true)
    }
    fn launch_pass(&mut self, input: SelectorInput, s: &SelectorSettings) {
        let info = self
            .launch_info
            .expect("Launch owns the packet before its batch");
        self.all_predictions_missed = false;
        if let Some(selection) = &mut self.selection {
            selection.wall_ride = false;
        }
        let batch = launch::batch(info, input, s);
        self.candidates
            .resize_with(self.candidates.len().max(batch.requests.len()), || {
                Candidate {
                    prediction: Prediction {
                        request: batch.requests[0],
                        result: QueryResult::miss(),
                    },
                    start_velocity: ZERO,
                    normal: ZERO,
                    collision_velocity: ZERO,
                    collision_position: ZERO,
                    score: 0.0,
                    wall_score: 0.0,
                    wall_ride: false,
                    grind: None,
                }
            });
        for (index, c) in self
            .candidates
            .iter_mut()
            .take(batch.requests.len())
            .enumerate()
        {
            c.start_velocity = batch.velocities[index];
            c.prediction = Prediction {
                request: batch.requests[index],
                result: QueryResult::miss(),
            };
        }
        self.batch = Some(batch);
        self.pending = true;
    }

    pub fn complete_batch(
        &mut self,
        results: &[QueryResult],
        input: SelectorInput,
        s: &SelectorSettings,
        grind: impl FnMut(&mut Prediction, bool) -> Result<super::grind::GrindEvaluation, String>,
        line: impl FnMut(Vector, Vector, f32) -> Result<Option<SurfaceHit>, String>,
    ) -> Result<bool, String> {
        self.just_changed = false;
        if !self.pending {
            return Ok(self.valid);
        }
        let count = self.requests().len();
        if results.len() != count {
            return Err("Trajectory completion count differs from submitted batch".into());
        }
        self.just_changed = self.pass == 2;
        self.pending = false;
        self.pass += 1;
        for (c, &result) in self.candidates.iter_mut().zip(results) {
            c.prediction.result = result;
        }
        self.all_predictions_missed = scoring::score(
            &mut self.candidates[..count],
            self.pass,
            self.adjusted_on_vert,
            input,
            s,
            grind,
            line,
        )?;
        let mut index = 0;
        let mut best = -100_000_000.0;
        for (i, c) in self.candidates.iter().take(count).enumerate() {
            if c.score > best {
                best = c.score;
                index = i;
            }
        }
        let c = self.candidates[index];
        if let Some(target) = self.candidates.iter().take(count).find_map(|c| c.grind) {
            self.grind_normal = Some(target.vertical_normal);
        }
        self.selected_index = Some(index);
        let previous_com = self.selection.map(|v| v.com_trajectory);
        let mut selection = Selection {
            candidate_index: index,
            prediction: c.prediction,
            start_velocity: c.start_velocity,
            landing_normal: c.normal,
            collision_velocity: c.collision_velocity,
            collision_position: c.collision_position,
            com_trajectory: previous_com.unwrap_or(Trajectory {
                position: ZERO,
                velocity: ZERO,
                acceleration: ZERO,
                duration: -1.0,
            }),
            surface_category: self.selection.map_or(0, |v| v.surface_category),
            wall_ride: c.wall_ride,
            grind: c.grind,
        };
        if c.prediction.result.contact_time < s.minimum_valid_time {
            self.suggested_normal = Some(c.prediction.result.contact_normal);
            self.valid = false;
            self.just_changed = false;
            self.selection = Some(selection);
            return Ok(false);
        }
        selection.surface_category = (c.prediction.result.surface >> 7) & 31;
        selection.com_trajectory = self.com_trajectory(selection, input, s);
        self.grind_locked_to_middle = index == 0 && c.grind.is_some();
        let second_pass = self.pass == 1
            && count >= 2
            && !self.grind_locked_to_middle
            && !(angle_between(selection.landing_normal, input.ground_normal)
                < s.minimum_normal_delta_second_pass);
        self.selection = Some(selection);
        self.valid = true;
        if second_pass {
            let info = self.launch_info.as_mut().expect("completed launch packet");
            info.start_velocity = selection.start_velocity;
            info.cone_angle_x = s.cone_x_second_pass;
            info.cone_angle_z = s.cone_z_second_pass;
            self.launch_pass(input, s);
            self.pass = 2;
        }
        Ok(self.valid)
    }
    fn com_trajectory(
        &self,
        selection: Selection,
        input: SelectorInput,
        s: &SelectorSettings,
    ) -> Trajectory {
        let info = self.launch_info.expect("completed launch packet");
        let batch = self.batch.as_ref().expect("completed query batch");
        let mut trajectory = selection.prediction.request.trajectory;
        let scalar = s
            .landing_com_scalar_vs_slope
            .evaluate(selection.landing_normal[1]);
        let delta = sub(info.animation_com_position, trajectory.position);
        let height = dot(
            input.reference_up,
            sub(batch.origin, info.animation_com_position),
        )
        .abs()
            * scalar;
        let offset = scale(selection.landing_normal, height);
        if selection.prediction.result.contact_frame > 0 {
            trajectory.position = add(trajectory.position, delta);
            launch::adjust_trajectory(
                &mut trajectory,
                selection.prediction.result.contact_frame,
                sub(offset, delta),
                s.maximum_trajectory_adjust,
            );
        }
        trajectory
    }
}
