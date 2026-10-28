mod state;
use crate::{
    animation_pose::PoseEvaluator,
    graph_host::{
        action::ActionHost,
        motion::{MotionHost, MotionPhysical},
        outputs::{ActionGraphInput, ActionInput, MotionGraphInput, PriorMotionState},
    },
    graph_runtime::StockGraphs,
};
use skate_core::graph::intents::IntentMap;
use skate_core::{
    animation::{
        body_tilt,
        output::{
            self, Sqt,
            attributes::PacketAttributes,
            packet_reset::{self, AdditionalResetFields, RESET_POSE},
            physics_packet::{self, PhysicsPosePacket},
        },
        physical_feedback::PhysicalFeedback,
        playback::PlaybackContext,
        playback_tree::{Evaluation, PoseCommand},
        riding_fakie,
        skeleton_input::name::encode,
    },
    graph::{conditions::ConditionInputs, controller::Controller},
    riding::push_behaviors::PushFootFrame,
};
use skate_data::collections::Collections;
use std::{path::Path, sync::Arc};

pub(crate) struct AnimationSource {
    banks: skate_data::animation_banks::AnimationBanks,
    evaluator: Arc<PoseEvaluator>,
}
impl AnimationSource {
    pub(crate) fn load(root: &Path) -> Result<Arc<Self>, String> {
        static CACHE: std::sync::Mutex<Option<(std::path::PathBuf, Arc<AnimationSource>)>> = std::sync::Mutex::new(None);
        let mut cache = CACHE.lock().map_err(|_| "Animation cache lock poisoned")?;
        if let Some((path, source)) = &*cache { if path == root { return Ok(source.clone()); } }
        let source = Self::load_uncached(root)?;
        *cache = Some((root.to_owned(), source.clone()));
        Ok(source)
    }
    fn load_uncached(root: &Path) -> Result<Arc<Self>, String> {
        let banks = skate_data::animation_banks::AnimationBanks::load(root)?;
        let mut evaluator = PoseEvaluator::from_banks(&banks)?;
        evaluator.load_authored_clips(root)?;
        Ok(Arc::new(Self { banks, evaluator: Arc::new(evaluator) }))
    }
}

pub(crate) struct AnimationPhysical {
    pub conditions: ConditionInputs,
    pub feedback: PhysicalFeedback,
    pub body_tilt: body_tilt::Physical,
    pub fakie: riding_fakie::Physical,
    pub physical_stance: (bool, bool),
    pub foot_frame: PushFootFrame,
    pub board_present: bool,
    pub physical_28_byte75: bool,
    pub time_since_teleport: f32,
}

pub(crate) struct SkaterAnimation {
    pub action: ActionHost,
    pub motion: MotionHost,
    pub evaluator: Arc<PoseEvaluator>,
    pub source: Arc<AnimationSource>,
    pub pose: Vec<Sqt>,
    pub packet: PhysicsPosePacket,
    pub attributes: PacketAttributes,
    pub action_controller: Controller,
    pub motion_controller: Controller,
    state: state::AnimationState,
    pub ticks: u64,
}

impl SkaterAnimation {
    pub fn checkpoint_stance(&self) -> u32 {
        let p = &self.state.publication;
        u32::from(((p.natural_stance == 0 && p.relative_stance == 0)
            || (p.natural_stance == 1 && p.relative_stance == 1)) ^ self.state.fakie())
    }
    pub fn request_checkpoint_stance(&mut self, foot: u32) {
        let natural = self.state.publication.natural_stance;
        self.motion.animation.requested_stance = u32::from(if foot == 0 { natural != 1 } else { natural == 1 });
    }
    pub fn foot_forward(&self) -> bool {
        self.checkpoint_stance() != 0
    }
    pub fn restore_foot_forward(&mut self, forward: bool) {
        self.request_checkpoint_stance(u32::from(forward));

    }
    pub(crate) fn set_customisation(&mut self, natural: u32, style: u32) {
        if natural <= 1 && self.state.publication.natural_stance != natural as i32 {
            self.state.publication.natural_stance = natural as i32;
            self.state.flags ^= 0xC000_0000;
        }
        let name: &[u8] = match style { 1 => b"Loose", 2 => b"Gonzo", 3 => b"Aggressive", _ => b"" };
        self.motion.playback_context.pro_skater = encode(name);
    }
    pub fn stance(&self) -> (bool, bool) {
        (self.state.fakie(), self.state.mirrored())
    }
    pub fn evaluate_initial_pose(&mut self) -> Result<Vec<output::NativeMatrix>, String> {
        self.pose = self.evaluator.evaluate(&[PoseCommand::Pose {
            name: "RIG_TPOSE".into(),
        }])?;
        self.evaluator.hierarchy(&self.pose)
    }

    pub fn load(
        root: &Path,
        data: &Collections,
        graphs: &StockGraphs,
        pro_skater: &[u8],
    ) -> Result<Self, String> {
        Self::from_source(data, graphs, pro_skater, AnimationSource::load(root)?)
    }

    pub fn from_source(
        data: &Collections, graphs: &StockGraphs, pro_skater: &[u8], source: Arc<AnimationSource>,
    ) -> Result<Self, String> {
        let evaluator = source.evaluator.clone();
        let state = state::AnimationState::new(true);
        let count = evaluator.frames.bone_names.len();
        let mut motion = MotionHost::from_graph(
            &graphs.motion,
            data,
            source.banks.metadata()?,
            PlaybackContext {
                is_switch: Some(state.switch()),
                is_mirrored: Some(state.mirrored()),
                board_available: None,
                pro_skater: encode(pro_skater),
                transition_override: None,
            },
        )
        .map_err(|error| format!("Stock MotionGraph initialization: {error}"))?;
        motion.animation.set_hierarchy(
            &evaluator.frames.bone_names,
            &evaluator.frames.mirror_indices,
        )?;
        use skate_core::animation::posture::PosturePose;
        for posture in [PosturePose::Stiff, PosturePose::Slouch, PosturePose::Buff] {
            let pose = evaluator.frames.named_pose(posture.name())?;
            if pose.samples.len() != count {
                return Err(format!(
                    "Stock posture {} has {} bones; skater hierarchy has {count}",
                    posture.name(),
                    pose.samples.len()
                ));
            }
        }
        motion.animation.posture_bank_valid = true;
        motion.animation.skater_animation_flags = Some(state.flags);
        Ok(Self {
            action: ActionHost::from_graph(&graphs.action, data)
                .map_err(|error| format!("Stock ActionGraph initialization: {error}"))?,
            motion,
            evaluator,
            source,
            pose: Vec::new(),
            attributes: PacketAttributes::default(),
            state,
            action_controller: Controller::new(graphs.action.runtime.program.topology.states.len()),
            motion_controller: Controller::new(graphs.motion.runtime.program.topology.states.len()),
            packet: PhysicsPosePacket {
                bone_count: count as u32,
                hierarchy: vec![RESET_POSE; count],
                local: vec![RESET_POSE; count],
                timestep: f32::from_bits(0x3c88_8889),
                foot_surface_ids: [0; 2],
                flags: 0,
                board_flipped: false,
                mirrored: false,
                riding_switch: false,
                riding_fakie: false,
                weight_forwards: false,
                regular_stance: false,
                air_dismount_revert_frames: 0,
            },
            ticks: 0,
        })
    }

    pub fn advance(
        &mut self,
        graphs: &StockGraphs,
        dt: f32,
        action_intents: &IntentMap,
        physical: AnimationPhysical,
        reset_fields: &mut AdditionalResetFields,
    ) -> Result<(), String> {
        let tick = self.ticks;
        self.publish_physical(physical)?;
        self.action.prepare_input(ActionGraphInput {
            tick,
            controls: ActionInput::from_values(action_intents),
            prior_motion: PriorMotionState::from_values(&self.motion.animation.motion_intents),
            animation_attributes: self.motion.animation.tree_attributes().to_vec(),
        });
        self.action.is_tricking = Some(self.motion.flags.doing_trick);
        self.action_controller
            .update(&graphs.action.runtime.program, dt, &mut self.action);
        if !self.action.errors.is_empty() {
            return Err(self.action.errors.join("\n"));
        }
        let action_output = self.action.output();
        self.motion.animation.begin_graph_update();
        self.motion.accept_action_graph(MotionGraphInput {
            tick: action_output.tick,
            action: action_output,
        });
        self.motion.animation_phase = self.state.phase;
        self.motion_controller
            .update(&graphs.motion.runtime.program, dt, &mut self.motion);
        self.state.phase = self.motion.animation_phase;
        if !self.motion.errors.is_empty() {
            return Err(self.motion.errors.join("\n"));
        }
        self.state.flags = self
            .motion
            .animation
            .skater_animation_flags
            .ok_or("MotionGraph lost the SkaterAnim flag owner")?;
        self.state.publication.relative_stance = self.motion.animation.relative_stance as i32;
        if std::mem::take(&mut self.motion.animation.reset_action_intents) {
            self.action.action_intents.clear();
        }

        self.motion.animation.apply_parameters()?;
        self.motion.animation.advance(dt, self.state.phase);
        self.motion.animation.refresh_tree_attributes()?;
        self.state
            .apply_stance_events(self.motion.animation.tree_attributes());
        let commands = self.motion.animation.evaluate_pose(Evaluation {
            cull_threshold: self.state.cull_threshold(),
            update_history: true,
        })?;
        self.pose = self.evaluator.evaluate(&commands)?;
        let hierarchy = self.evaluator.hierarchy(&self.pose)?;
        let local: Vec<_> = self
            .pose
            .iter()
            .copied()
            .map(output::sqt_to_matrix)
            .collect();
        self.attributes.replace_from(
            &self.motion.animation.motion_attributes,
            self.motion.animation.tree_attributes(),
        );
        self.state.prepare_publication();
        packet_reset::reset(&mut self.packet, reset_fields)
            .map_err(|error| format!("Animation packet reset: {error:?}"))?;
        physics_packet::publish_evaluated(
            &mut self.state.publication,
            &hierarchy,
            &local,
            &mut self.packet,
            f64::from(f32::from_bits(0x3c888889)),
            b"signup",
        )
        .map_err(|e| format!("Skater pose publication: {e:?}"))?;
        self.state.finish_publication();
        self.motion.animation.skater_animation_flags = Some(self.state.flags);
        self.ticks += 1;
        Ok(())
    }

    fn publish_physical(&mut self, p: AnimationPhysical) -> Result<(), String> {
        self.motion.animation.natural_stance = self.state.publication.natural_stance as u32;
        self.motion.animation.relative_stance = self.state.publication.relative_stance as u32;
        let speed = p
            .conditions
            .speeds
            .ok_or("Animation requires completed board speed output")?;
        let stance = (self.state.fakie(), self.state.mirrored());
        self.action.stance = Some(stance);
        self.action.condition_inputs = p.conditions.clone();
        self.motion.condition_inputs = p.conditions;
        self.motion.playback_context.is_mirrored = Some(self.state.mirrored());
        self.motion.playback_context.is_switch = Some(self.state.switch());
        self.motion.playback_context.board_available = Some(p.board_present);
        self.motion.physical = Some(MotionPhysical {
            turning: p.feedback.turning,
            stance,
            forward_speed: speed.forward_speed,
            time_since_teleport: p.time_since_teleport,
            is_switch: self.state.switch(),
            foot_frame: Some(p.foot_frame),
        });
        self.motion.physical_stance = Some(p.physical_stance);
        self.motion.crouching_physical = Some(p.feedback.crouching);
        self.motion.body_tilt_physical = Some(p.body_tilt);
        self.motion.fakie_physical = Some(p.fakie);
        self.motion.pumping_acceleration = Some(p.feedback.pumping_acceleration);

        let board_attached_or_onboard = p.board_present || !p.physical_28_byte75;
        self.state.flags =
            (self.state.flags & !(1 << 17)) | (u32::from(board_attached_or_onboard) << 17);
        self.motion.animation.skater_animation_flags = Some(self.state.flags);
        Ok(())
    }
}
