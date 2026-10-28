use skate_core::{
    physics::{
        contact::RetailContactMaterial,
        manual::settings::{ManualGains, ManualMode, ManualSettings},
    },
    point_graph::PointGraph,
    riding::{
        anti_flip::AntiFlipSettings,
        braking::{BrakeSettings, LinearDragSettings},
        ground_force::GroundForceSettings,
        grounded::{propulsion::GroundPropulsionSettings, state::board_types::GroundBoardSettings},
        heading::HeadingSettings,
        slide_friction::SlideFrictionSettings,
        speed_model::SpeedModelSettings,
        speed_wobble::SpeedWobbleSettings,
        steering::SteeringSettings,
        straighten::StraightenSettings,
    },
};
use skate_data::collections::Collections;

use std::sync::Arc;

pub(crate) struct GroundProfiles(Vec<Vec<Arc<GroundSettings>>>);
impl GroundProfiles {
    pub fn load(data: &Collections) -> Result<Self, String> {
        crate::difficulty::NATIVE_MODES.into_iter().map(|mode| {
            (1..=5).map(|surface| {
                GroundSettings::load(data, mode, super::surface_key(surface)?).map(Arc::new)
            }).collect::<Result<Vec<_>, String>>()
        }).collect::<Result<Vec<_>, String>>().map(Self)
    }
    pub fn select(&self, mode: u32, surface: u32) -> Result<Arc<GroundSettings>, String> {
        surface.checked_sub(1).and_then(|s| self.0.get(mode as usize)?.get(s as usize))
            .cloned().ok_or_else(|| format!("Invalid processed physics mode/surface {mode}/{surface}"))
    }
}

#[derive(Clone)]
pub(crate) struct GroundSettings {
    steering: SteeringSettings,
    wobble: SpeedWobbleSettings,
    propulsion: GroundPropulsionSettings,
    force: GroundForceSettings,
    speed: SpeedModelSettings,
    slide: SlideFrictionSettings,
    straighten: StraightenSettings,
    heading: HeadingSettings,
    anti_flip: AntiFlipSettings,
    manual: ManualSettings,
    manual_mode: ManualMode,
    drag: LinearDragSettings,
    collision_duration: f32,
    collision_scale: f32,
    contact_force_time: f32,
    pub wheel_material: RetailContactMaterial,
    pub foot_force_offset: f32,
    pub push_target_multiplier: f32,
    pub absorption_front: f32,
    pub absorption_rear: f32,
    pub surface_braking_factor: f32,
    pub wobble_activation: f32,
    pub wobble_amplitude: f32,
}
impl GroundSettings {
    pub fn load(data: &Collections, mode: &str, surface: &str) -> Result<Self, String> {
        let f = |class, field| data.float(class, "default", field);
        let m = |field| data.float("physics_mode", mode, field);
        let s = |field| data.float("physics_surfaces", surface, field);
        let c8 = |class, field| curve8(data, class, "default", field);
        let c16 = |class, field| curve16(data, class, "default", field);
        let threshold = [f32::from_bits(0x3586_37bd); 4];
        Ok(Self {
            push_target_multiplier: 1.,
            foot_force_offset: f("physics_feet", "FootForceOffset")?,
            absorption_front: f("physics_feet", "AbsorptionFootForceScalar")?,
            absorption_rear: f("physics_feet", "AbsorptionFootForceRearScalar")?,
            surface_braking_factor: s("BrakingScalar")?,
            steering: SteeringSettings {
                hard_turn_increase: f("physics_steering", "HardTurnIncrease")?,
                damping: f("physics_steering", "Damping")?,
                speed_graph_max_speed: f("physics_steering", "SteeringVsSpeedMaxSpeed")?,
                push_scalar_increment: f("physics_steering", "PushScalarIncriment")?,
                push_scalar_decrement: f("physics_steering", "PushScalarDecriment")?,
                push_scalar_min: f("physics_steering", "PushScalarMin")?,
                manual_scalar: f("physics_manual", "Tilt_TiltScalar")?,
                general_scalar: f("physics_steering", "GeneralScalar")?,
                tight_trucks_scalar: f("physics_steering", "SteeringScalar_TightTrucks")?,
                speed_graph: c8("physics_steering", "SteeringVsSpeed")?,
                input_graph: c8("physics_steering", "SteeringVsInput")?,
                tilt_blending: f("physics_steering", "SteeringTiltBlending")?,
            },
            wobble: SpeedWobbleSettings {
                frequency_time: c8("physics_speed_wobble", "FrequencyVsTime")?,
                frequency_speed: c8("physics_speed_wobble", "FrequencyVsSpeed")?,
                amplitude_time: c8("physics_speed_wobble", "AmplitudeVsTime")?,
                amplitude_speed: c8("physics_speed_wobble", "AmplitudeVsSpeed")?,
                tightness_threshold: f("physics_speed_wobble", "TruckTightnessSpeedMaxAdjust")?,
                time_range: f("physics_speed_wobble", "TimeMax")?,
                speed_range: f("physics_speed_wobble", "SpeedRange")?,
                frequency_scale: f("physics_speed_wobble", "MaxFrequency")?,
                amplitude_scale: f("physics_speed_wobble", "MaxAmplitude")?,
                crouch_threshold: f("physics_speed_wobble", "CrouchStartSpeedMaxAdjust")?,
                height_min: f("physics_speed_wobble", "CrouchMin")?,
                height_max: f("physics_speed_wobble", "CrouchMax")?,
            },
            propulsion: GroundPropulsionSettings {
                braking: BrakeSettings {
                    input_force: f("physics_brakes", "FootBrakeForce")?,
                    override_force: f("physics_brakes", "TailBrakeForce")?,
                    minimum_speed: f("physics_brakes", "SlowSpeed")?,
                },
                maximum_pushable_speed: f("physics_push", "MaxPushableSpeed")?,
                mode_speed_changes: [m("MaxPushDVStart")?, m("MaxPushDVEnd")?],
            },
            force: GroundForceSettings {
                range_1220: f("physics_feet", "LandingForceTime")?,
                speed_scale_1224: f("physics_feet", "MinForce")?,
                scale_1228: f("physics_feet", "MaxLandingForce")?,
                normal_threshold: threshold,
            },
            speed: SpeedModelSettings {
                negative_gain: f("physics_speed_conservation", "NegativeGeneralAmount")?,
                maximum_gravity_acceleration: f(
                    "physics_speed_conservation",
                    "MaxGravityAcceleration",
                )?,
                gravity: f("physics_speed_conservation", "Gravity")?,
                positive_gain: f("physics_speed_conservation", "GeneralAmount")?,
                coffin_acceleration: f("physics_speed_conservation", "CoffinAcceleration")?,
                speed_error_bound: s("Friction_MaxSpeedChange")?,
                surface_friction: curve8(data, "physics_surfaces", surface, "FrictionVsSpeedNew")?,
                manual_acceleration: f("physics_manual", "SpinCorrectiveForce")?,
                negative_manual_angle_limit: f("physics_manual", "MinSpeedForCorrection")?,
                manual_angle_limit: f("physics_manual", "MaxSpeedForCorrection")?,
                no_input_delay: f("physics_friction", "NoInputTime")?,
                no_input_friction: c8("physics_friction", "FrictionVsSpeed_NoInput")?,
                manual_friction: c8("physics_friction", "FrictionVsSpeed_Manual")?,
                override_enabled: data.boolean("physics_mode", mode, "MotorEnabled")?,
                override_speed: m("MotorTopSpeed")?,
                normal_threshold: threshold,
                override_direction_threshold: [0.; 4],
            },
            slide: SlideFrictionSettings {
                angle_response: c16("physics_friction", "SlideVsNormal")?,
                speed_response: c8("physics_friction", "SlideVsSpeed")?,
                time_response: c16("physics_heading", "SideFrictionFactor")?,
                scalar_516: f("physicswheels", "SoftestWheelSideFrictionScalar")?,
                heading_time_limit: s("StraightenOut_ForceTime")?,
                friction: s("SideFriction_Scalar")?,
            },
            straighten: StraightenSettings {
                time_response: c16("physics_heading", "StraightenOutForce")?,
                opposite_turn_limit: f("physics_heading", "StraightenMaxTurnInput")?,
                time_scalar: f("physicswheels", "SoftestWheelStraightenOutTimeScalar")?,
                heading_time_limit: s("StraightenOut_ForceTime")?,
                strength: s("StraightenOut_Scalar")?,
            },
            heading: HeadingSettings {
                manual_wrong_wheel_scalar: f("physics_manual", "SpinNoLiftScalar")?,
                manual_damping: f("physics_manual", "ManualSpinBlendSpeed")?,
                speed_max: f("physics_heading", "HeadingAdjustMaxSpeed")?,
                heading_strength: f("physics_heading", "HeadingAdjustFactor")?,
                turn_strength: f("physics_heading", "TurnTorqueScalar")?,
                angular_response: c8("physics_heading", "TurnTorqueVsNegAngVel")?,
                manual_response: c8("physics_manual", "SpinVsSpeed")?,
                inclination_response: c8("physics_heading", "HeadingAdjustVsSlope")?,
                speed_response: c8("physics_heading", "HeadingAdjustVsSpeed")?,
                normal_threshold: threshold,
            },
            anti_flip: AntiFlipSettings {
                axis_96_response: c8("physics_feet", "AntiFlipTorqueX")?,
                axis_64_response: c8("physics_feet", "AntiFlipTorqueZ")?,
                normal_threshold: threshold,
            },
            manual: manual_settings(data)?,
            manual_mode: ManualMode {
                correction_angular_speed_threshold: m("Hash_84CB2F459F3FC811")?,
                corrective_force_enabled: data.boolean(
                    "physics_mode",
                    mode,
                    "Hash_F8CBC0F5FEF2240E",
                )?,
            },
            drag: LinearDragSettings {
                brake_speed: f("physics_brakes", "SlowSpeed")?,
                balance_speed: f("physics_brakes", "ManualSpinDragSpeed")?,
                comparison_threshold: f("physics_brakes", "ManualSpinDragNormal")?,
                balance_drag: f("physics_brakes", "ManualSpinDrag")?,
            },
            collision_duration: f("physics_collision", "CollisionTorqueMaxTime")?,
            collision_scale: f("physics_collision", "CollisionTorqueFadeoff")?,
            contact_force_time: f("physics_feet", "MaxTimeWallRidingForFootForce")?,
            wheel_material: RetailContactMaterial {
                static_friction: s("WheelStaticFriction")?,
                dynamic_friction: s("WheelDynamicFriction")?,
                restitution: f("physicswheels", "WheelRestitution")?,
            },
            wobble_activation: m("Hash_77AFCE78FE1206CA")?,
            wobble_amplitude: m("Hash_5B57F2CCCCEEF430")?,
        })
    }
    pub fn tuned(&self, tuning: crate::tuning::TrainerTuning) -> Self {
        let mut result=self.clone();
        result.push_target_multiplier = tuning.push_speed;
        result.propulsion.maximum_pushable_speed *= tuning.push_speed;
        for dv in &mut result.propulsion.mode_speed_changes { *dv *= tuning.push_power; }
        result.propulsion.braking.input_force *= tuning.braking;
        result.propulsion.braking.override_force *= tuning.braking;
        result.steering.general_scalar *= tuning.steering;
        result.wobble_amplitude *= tuning.wobble;
        result.slide.friction *= tuning.grip;
        result.wheel_material.static_friction *= tuning.grip;
        result.wheel_material.dynamic_friction *= tuning.grip;
        result.heading.turn_strength *= tuning.turn_power;
        result.drag.balance_drag *= tuning.manual_drag;
        result
    }
    pub fn board(&self) -> GroundBoardSettings<'_> {
        GroundBoardSettings {
            steering: &self.steering,
            speed_wobble: &self.wobble,
            propulsion: self.propulsion,
            ground_force: &self.force,
            speed_model: &self.speed,
            slide_friction: &self.slide,
            straighten: &self.straighten,
            heading: &self.heading,
            anti_flip: &self.anti_flip,
            manual: &self.manual,
            manual_mode: self.manual_mode,
            linear_drag: self.drag,
            collision_response_duration: self.collision_duration,
            collision_response_scale: self.collision_scale,
            ground_force_contact_time_limit: self.contact_force_time,
            speed_model_reset_force_squared: f32::from_bits(0x3780_0000),
        }
    }
}
fn manual_settings(d: &Collections) -> Result<ManualSettings, String> {
    let f = |field| d.float("physics_manual", "default", field);
    let gains = |p, i, v| -> Result<ManualGains, String> {
        Ok(ManualGains {
            proportional: f(p)?,
            integral: f(i)?,
            derivative: f(v)?,
        })
    };
    Ok(ManualSettings {
        noise_vs_speed: curve8(d, "physics_manual", "default", "NoiseVsSpeed")?,
        torque_scale_without_contact: f("TorqueScalarWithNoContact")?,
        torque_bleed_without_contact: f("TorqueBleedOffWithNoContact")?,
        start_torque_scale: f("StartTorqueScalar")?,
        procedural_noise_scale: f("ProceduralNoiseScalar")?,
        procedural_noise_frequency: f("ProceduralNoiseFreq")?,
        powerslide: gains("PowerScalar_P", "PowerScalar_I", "PowerScalar_D")?,
        manual: gains("ManualScalar_P", "ManualScalar_I", "ManualScalar_D")?,
        maximum_tilt_degrees: f("MaxTiltAngle")?,
        maximum_angle_error: f("MaxAngleDiff")?,
        derivative_limit: f("D_Scalar_Limit")?,
        brake_tilt_degrees: f("BrakeTiltAngle")?,
        animation_noise_scale: f("AnimationNoiseScalar")?,
    })
}
pub(super) fn curve8(d: &Collections, c: &str, k: &str, f: &str) -> Result<PointGraph<8>, String> {
    let data = &d.field(c, k, f)?.data;
    match data.len() {
        128 => {
            let w = d.words::<16>(c, k, f)?;
            Ok(PointGraph {
                x: std::array::from_fn(|i| f32::from_bits(w[i])),
                y: std::array::from_fn(|i| f32::from_bits(w[i + 8])),
            })
        }
        160 => {
            let w = d.words::<20>(c, k, f)?;
            Ok(PointGraph {
                x: std::array::from_fn(|i| f32::from_bits(w[i + 4])),
                y: std::array::from_fn(|i| f32::from_bits(w[i + 12])),
            })
        }
        _ => Err(format!("Invalid native eight-point graph {c}/{k}/{f}")),
    }
}
fn curve16(d: &Collections, c: &str, k: &str, f: &str) -> Result<PointGraph<16>, String> {
    let w = d.words::<32>(c, k, f)?;
    Ok(PointGraph {
        x: std::array::from_fn(|i| f32::from_bits(w[i])),
        y: std::array::from_fn(|i| f32::from_bits(w[i + 16])),
    })
}

