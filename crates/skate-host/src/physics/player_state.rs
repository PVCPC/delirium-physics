mod post_input;
mod pre_state;
mod publication;
mod registry;
mod selection;
mod transition;
mod wipeout_output;
use super::{GamePhysics, skater::SkaterRuntime};
use skate_core::{
    physics::filtered_state::{FilteredState, FilteredStateOutput},
    player::{
        lifecycle::PhysicalPlayerStateLifecycle,
        selector::{StateSelector, conditions::TwoStageThresholds},
        state::PhysicalStateId,
    },
};
use skate_data::collections::Collections;

pub(crate) struct PlayerState {
    pub registry: registry::StateRegistry,
    pub lifecycle: PhysicalPlayerStateLifecycle,
    pub selector: StateSelector,
    pub requested_state: PhysicalStateId,
    pub filtered: FilteredState,
    pub filtered_output: Option<FilteredStateOutput>,
    pub ground_output: Option<skate_core::riding::grounded::state::output::PhysicsGroundOutput>,
    pub post: post_input::PostInputState,
    pub state_flags: [bool; 36],
    pub state_count: u32,
    pub update_count: u32,
    normal_off_ground: TwoStageThresholds,
    skitching_off_ground: TwoStageThresholds,
    animated_board_threshold: f32,
    initialized: bool,
}
impl PlayerState {
    pub fn load(data: &Collections, _mode: &str) -> Result<Self, String> {
        let thresholds = |class| -> Result<TwoStageThresholds, String> {
            Ok(TwoStageThresholds {
                field_856_primary: data.float(class, "default", "NaturalAirMaxDist")?,
                field_856_secondary: data.float(class, "default", "NaturalAirMinDist")?,
                field_7692: data.float(class, "default", "NaturalAirTime")?,
            })
        };
        Ok(Self {
            registry: registry::StateRegistry::new(),
            lifecycle: PhysicalPlayerStateLifecycle::new(PhysicalStateId::Sleeping),
            selector: StateSelector::default(),
            requested_state: PhysicalStateId::Sleeping,
            filtered: FilteredState::default(),
            filtered_output: None,
            ground_output: None,
            post: post_input::PostInputState::new(),
            state_flags: [false; 36],
            state_count: 0,
            update_count: 0,
            normal_off_ground: thresholds("physics_airstates")?,
            skitching_off_ground: thresholds("physics_state_skitching")?,
            animated_board_threshold: data.float(
                "physics_animation",
                "default",
                "MaxDeckZAxisYForAnimatedDeck",
            )?,
            initialized: false,
        })
    }
    pub fn current(&self) -> PhysicalStateId {
        self.lifecycle.active().state
    }
    pub fn reset_for_teleport(&mut self) {
        self.filtered.reset();
        self.filtered_output = None;
        self.ground_output = None;
        self.selector.nonspecific_collision_free_frames = 0;
        self.selector.nonspecific_collision_frames = 0;
        self.selector.something_colliding_frames = 0;
        self.selector.two_wheel_counter = 0;
        self.selector.three_wheel_counter = 0;
        self.selector.post_grind_jump_counter = 0;
        self.selector.air_frames = 0;
        self.selector.teleport_countdown = 10;
        self.selector.skitch_exit_countdown = 0;
        self.post.trajectory_available = false;
    }
}
pub(crate) fn initialize(
    physics: &mut GamePhysics,
    skater: &mut SkaterRuntime,
) -> Result<(), String> {
    if skater.player_state.initialized {
        return Ok(());
    }
    transition::set(physics, skater, PhysicalStateId::PhysicsGround)?;
    skater.player_state.initialized = true;
    Ok(())
}
pub(crate) fn post_input_and_select(
    physics: &mut GamePhysics,
    skater: &mut SkaterRuntime,
) -> Result<(), String> {
    complete_post_input(physics, skater)?;
    let processed = skater.player_input.processed_snapshot(physics.ticks);
    selection::advance(physics, skater, processed)
}

pub(crate) fn complete_post_input(
    physics: &mut GamePhysics,
    skater: &mut SkaterRuntime,
) -> Result<(), String> {
    post_input::advance(physics, skater)
}
pub(crate) fn publish(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    publication::publish(physics, skater)
}
pub(crate) fn enter_after_teleport(
    physics: &mut GamePhysics,
    skater: &mut SkaterRuntime,
) -> Result<(), String> {
    let target = if skater.teleport_state.take_manual_on_board() == Some(false) {
        PhysicalStateId::BipedGround
    } else { PhysicalStateId::PhysicsGround };
    transition::set(physics, skater, target)
}

pub(crate) fn pre_state(
    physics: &mut GamePhysics,
    skater: &mut SkaterRuntime,
) -> Result<(), String> {
    pre_state::advance(physics, skater)
}

pub(crate) fn resume_after_climb(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    super::biped_ground::exit(skater);
    skater.offboard_air_selector.reset();
    skater.collision_extra_errors = [[0.; 4]; 2];
    skater.animated_skeleton.motion.velocity_world = [0.; 4];
    skater.player_input.processed.vectors_544_560_592_608[3] = [0; 4];
    if skater.player_state.current() == PhysicalStateId::BipedGround {
        super::biped_ground::enter(physics, skater)?;
    } else {
        transition::set(physics, skater, PhysicalStateId::BipedGround)?;
    }
    super::foot_ik_queries::query(&physics.world, &skater.skeleton)?
        .publish(&mut skater.player_input.player);
    let frame = skater.biped_ground.ground.frame_80;
    skater.offboard_contact.submit(
        skate_core::player::offboard::contact_toolkit::Input {
            position: frame[3], right: frame[0], up: frame[1], forward: frame[2],
            animation_right: frame[0], animation_up: frame[1], velocity: [0.; 4],
        },
        skater.player_input.processed.actor_query_2952 as i32,
        &super::offboard::contact_toolkit::StaticScene::new(&physics.world)?,
    )?;

    Ok(())
}

pub(crate) fn apply_vehicle_ejection(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<bool, String> {
    let Some((velocity, angular)) = skater.teleport_state.take_vehicle_ejection() else { return Ok(false); };
    transition::set(physics, skater, PhysicalStateId::WipeoutGround)?;
    let velocity=skate_core::math::Vector3::new(velocity[0],velocity[1],velocity[2]);
    let angular=skate_core::math::Vector3::new(angular[0],angular[1],angular[2]);
    for body in skater.skeleton.bodies_mut() {
        body.rates.linear_velocity=velocity;
        body.rates.angular_velocity=angular;
    }
    for body in physics.board.bodies_mut() {
        body.rates.linear_velocity=velocity;
        body.rates.angular_velocity=angular;
    }
    skater.animated_skeleton.motion.velocity_world=[velocity.x,velocity.y,velocity.z,0.];
    skater.player_input.processed.vectors_544_560_592_608[3]=[velocity.x,velocity.y,velocity.z,0.].map(f32::to_bits);
    skater.wipeout_state.state.velocity=[velocity.x,velocity.y,velocity.z,0.];
    Ok(true)
}
