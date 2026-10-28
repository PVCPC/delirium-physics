use crate::{
    math::Vector3,
    physics::{
        force_queue::{BoardForceQueue, QueuedPointForce},
        manual::{
            controller::{ManualAngleMeasurement, ManualInput},
            settings::{ManualMode, ManualSettings},
            state::ManualState,
        },
    },
    riding::{
        anti_flip::{AntiFlipInput, AntiFlipSettings},
        braking::LinearDragSettings,
        ground_force::GroundForceSettings,
        grounded::{
            drag::{BodyInertias, GroundDragInput},
            propulsion::{GroundPropulsionInput, GroundPropulsionSettings},
        },
        heading::{HeadingInput, HeadingSettings},
        pumping::PumpForceInput,
        slide_friction::{SlideFrictionInput, SlideFrictionSettings},
        speed_model::{SpeedModelInput, SpeedModelSettings, SpeedModelState},
        speed_wobble::{SpeedWobbleInput, SpeedWobbleSettings, SpeedWobbleState},
        steering::{SteeringInput, SteeringSettings, TruckSteeringState},
        straighten::{StraightenInput, StraightenSettings},
    },
};

use super::corrections::{
    AntiFlipNudgeInput, AntiFlipNudgeMath, HalfpipeWheelCatchInput, HalfpipeWheelCatchServices,
    HangUpInput, HangUpServices, PinningInput, PinningServices,
};

pub struct GroundBoardSettings<'a> {
    pub steering: &'a SteeringSettings,
    pub speed_wobble: &'a SpeedWobbleSettings,
    pub propulsion: GroundPropulsionSettings,
    pub ground_force: &'a GroundForceSettings,
    pub speed_model: &'a SpeedModelSettings,
    pub slide_friction: &'a SlideFrictionSettings,
    pub straighten: &'a StraightenSettings,
    pub heading: &'a HeadingSettings,
    pub anti_flip: &'a AntiFlipSettings,
    pub manual: &'a ManualSettings,
    pub manual_mode: ManualMode,
    pub linear_drag: LinearDragSettings,
    pub collision_response_duration: f32,
    pub collision_response_scale: f32,
    pub ground_force_contact_time_limit: f32,
    pub speed_model_reset_force_squared: f32,
}

pub struct GroundForceFrame {
    pub argument_1_2752: f32,
    pub processed_2776: f32,
    pub processed_2780: f32,
    pub previous_state_scalar_56: f32,
    pub ground_scalar_1216: f32,
    pub ground_scalar_1232: f32,
    pub ground_scalar_1236: f32,
    pub ground_scalar_1240: f32,
    pub balance_2720: f32,
    pub surface_speed_2656: f32,
    pub axis_384: [f32; 4],
    pub velocity_400: [f32; 4],
    pub axis_544: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundContactFrame {
    pub vector_8032: [f32; 4],
    pub vector_8048: [f32; 4],
    pub vector_8064: [f32; 4],
    pub word_8080: u32,
    pub flag_8084: bool,
    pub scalar_2752: f32,
    pub scalar_2756: f32,
}

pub struct GroundBoardInput {
    pub steering: SteeringInput,
    pub speed_wobble: SpeedWobbleInput,
    pub truck_flags_2468: u32,
    pub truck_flags_2472: u32,
    pub contact: GroundContactFrame,
    pub ground_vector_1216: [f32; 4],
    pub propulsion: GroundPropulsionInput,
    pub ground_force: GroundForceFrame,
    pub speed_model: SpeedModelInput,
    pub slide_friction: SlideFrictionInput,
    pub pump_force: PumpForceInput,
    pub straighten: StraightenInput,
    pub heading: HeadingInput,
    pub anti_flip: AntiFlipInput,
    pub manual: ManualInput,
    pub ground_drag: GroundDragInput,
    pub contact_time_2756: f32,
    pub trajectory_state_1776_bits: u32,
    pub anti_flip_nudge: AntiFlipNudgeInput,
    pub hang_up: HangUpInput,
    pub halfpipe_wheel_catch: HalfpipeWheelCatchInput,
    pub pinning: PinningInput,
}

pub struct GroundBoardComponents<'a, 'body> {
    pub speed_wobble: &'a mut SpeedWobbleState,
    pub truck_steering: &'a mut TruckSteeringState,
    pub speed_model: &'a mut SpeedModelState,
    pub manual: &'a mut ManualState,
    pub heading_previous: &'a mut f32,
    pub force_queue: &'a mut BoardForceQueue,
    pub inertias: &'a mut BodyInertias<'body>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundContactResponse {
    pub active_2731: bool,
    pub tag_16_force: QueuedPointForce,
    pub vector_2688: [f32; 4],
    pub scalar_2704: f32,
    pub animated_board_2708: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionForceResponse {
    pub force_2528: [f32; 4],
    pub point_2544: [f32; 4],
    pub vector_2592: [f32; 4],
}

impl CollisionForceResponse {
    pub fn tagged_force(self) -> QueuedPointForce {
        QueuedPointForce {
            tag: 15,
            force_world: xyz(self.force_2528),
            point_body: xyz(self.point_2544),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundBoardStage {
    CenterOfMassHeight,
    SetWheelMaterials,
    ContactResponse,
    UpdateBodyAccumulator,
    SetAnimatedVelocity,
    WriteProcessedVelocity,
    BuildAnimatedPose,
    PublishAnimatedPose,
    UpdateExternalPlayer,
    CommitExternalPlayer,
    FinalizeAnimatedBoard,
    CollisionForce,
    ApplyCollisionVector,
    CollisionProjection,
    ManualEffect,
    PushForceSquared,
    AntiFlipNudge,
    ApplyCollisionDecay,
    ApplyStraighten,
    ApplyHeading,
    ApplyAntiFlip,
    ApplyManual,
    HangUps,
    HalfpipeWheelCatch,
    Pinning,
    StrongForceSquared,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroundBoardError<E> {
    Service { stage: GroundBoardStage, source: E },
    Manual(crate::physics::manual::controller::ManualError<E>),
    Drag(crate::riding::grounded::drag::DragBindingError),
}

pub trait GroundBoardServices:
    ManualAngleMeasurement<Error = Self::BoardError>
    + AntiFlipNudgeMath<Error = Self::BoardError>
    + HangUpServices<Error = Self::BoardError>
    + HalfpipeWheelCatchServices<Error = Self::BoardError>
    + PinningServices<Error = Self::BoardError>
{
    type BoardError;
    type AnimatedPose;

    fn center_of_mass_height_n34(&mut self) -> Result<f32, Self::BoardError>;
    fn set_contact_wheel_materials(&mut self) -> Result<(), Self::BoardError>;
    fn contact_response_n35(
        &mut self,
        frame: GroundContactFrame,
        previous_vector_2688: [f32; 4],
    ) -> Result<GroundContactResponse, Self::BoardError>;
    fn update_body_accumulator_n36(&mut self) -> Result<(), Self::BoardError>;
    fn set_animated_velocity_n40(
        &mut self,
        velocity: [f32; 4],
    ) -> Result<(), Self::BoardError>;
    fn write_processed_velocity_400(&mut self, velocity: [f32; 4]) -> Result<(), Self::BoardError>;
    fn build_animated_pose_n41(&mut self) -> Result<Self::AnimatedPose, Self::BoardError>;
    fn publish_animated_pose_n42(
        &mut self,
        pose: &Self::AnimatedPose,
    ) -> Result<(), Self::BoardError>;
    fn update_external_player_n43(
        &mut self,
        pose: &Self::AnimatedPose,
        velocity: [f32; 4],
    ) -> Result<(), Self::BoardError>;
    fn commit_external_player_n10(&mut self) -> Result<(), Self::BoardError>;
    fn finalize_animated_board_sk83_na_f_01a4(&mut self) -> Result<(), Self::BoardError>;
    fn collision_force_n37(
        &mut self,
        ground_vector_1216: [f32; 4],
    ) -> Result<Option<CollisionForceResponse>, Self::BoardError>;
    fn collision_force_dot_velocity_n39(
        &mut self,
        force: [f32; 4],
        velocity_400: [f32; 4],
    ) -> Result<f32, Self::BoardError>;
    fn apply_vector_n38(&mut self, vector: [f32; 4]) -> Result<(), Self::BoardError>;
    fn apply_angular_displacement_n44(
        &mut self,
        vector: [f32; 4],
    ) -> Result<(), Self::BoardError>;
}

fn xyz(value: [f32; 4]) -> Vector3 {
    Vector3::new(value[0], value[1], value[2])
}
