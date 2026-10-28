use super::motion::MotionHost;
use skate_core::graph::controller::Frame;
use skate_core::{
    animation::{output::attributes::AttributeName, skeleton_input::name::encode},
    graph::conditions::{ActionCondition, Comparison, NumericCondition},
};
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Debug, PartialEq)]
pub enum MotionCondition {
    Grind(super::motion_grind::conditions::Condition),
    HasTweak(NumericCondition),
    CurrentGrabType(super::motion_stock_gameplay::GrabType),
    ManualOutTimerIsActive,
    Gesture(crate::input::gesture_catalog::Group),
    DisableDismount(super::motion_dismount::Condition),
    Landing(super::motion_landing::Condition),
    Wipeout(super::motion_wipeout::Condition),
    PushOff(super::motion_push_off::IsPushOffEnabled),
    Shared(ActionCondition),
    Intent {
        name: String,
        filtered: bool,
        numeric: NumericCondition,
    },
    AnimationAttribute {
        name: AttributeName,
        sequence_id: i32,
        numeric: NumericCondition,
    },
    ExpireInTime(NumericCondition),
    WillExpire {
        in_time: f32,
        tag: Option<String>,
        wait_for_transitions: bool,
    },
    InTimeWindow {
        start: f32,
        length: f32,
    },
    InStateForTime {
        name: String,
        target: Option<usize>,
        numeric: NumericCondition,
    },
    InParentStateForTime {
        target: Option<usize>,
        numeric: NumericCondition,
    },
    ProSkater(AttributeName),
    BreakOutOfPush,
    ShouldLeaveSlide {
        right: bool,
    },
    RidingGoofy,
    RidingSwitch,
    MongoPushFootTooFar,
    LastState {
        name: String,
        target: Option<usize>,
    },
    Gameplay(super::motion_gameplay_conditions::GameplayCondition),
    Riding(super::motion_riding_conditions::MotionRidingCondition),
    TimeToLand(NumericCondition),
    ObTimeToLand(NumericCondition),
    ObTrajTime(NumericCondition),
    LocoState(super::motion_offboard_cadence::LocoState),
    GroundSlopeType(super::motion_ground_slope::GroundSlopeType),
    IsBipedGroundThin,
    IsHoldingSkateboard,
    IsStandingOnMovingObject,
    DebugAnimationsMode,
    StockGameplay(super::motion_stock_conditions::Condition),
}
impl MotionCondition {
    pub fn parse(a: &Attributes<'_>) -> Result<Option<Self>, String> {
        if let Some(condition) = super::motion_grind::conditions::Condition::parse(a)? {
            return Ok(Some(Self::Grind(condition)));
        }
        if let Some(condition) = super::motion_landing::Condition::parse(a) {
            return Ok(Some(Self::Landing(condition)));
        }
        if let Some(condition) = super::motion_wipeout::Condition::parse(a)? {
            return Ok(Some(Self::Wipeout(condition)));
        }
        if let Some(condition) = super::motion_push_off::IsPushOffEnabled::parse(a) {
            return Ok(Some(Self::PushOff(condition)));
        }
        if let Some(condition) = super::motion_dismount::Condition::parse(a) {
            return Ok(Some(Self::DisableDismount(condition)));
        }
        let numeric = || super::condition_nodes::numeric(a);
        let name = a
            .text("name")
            .unwrap_or("")
            .trim_matches(|c: char| c.is_whitespace() || c == '\0');
        Ok(Some(match name {
            "HasTweak" => Self::HasTweak(numeric()),
            "CurrentGrabType" => Self::CurrentGrabType(
                super::motion_stock_gameplay::GrabType::parse(
                    a.text("grab").ok_or("CurrentGrabType requires grab")?,
                )?,
            ),
            "ManualOutTimerIsActive" => Self::ManualOutTimerIsActive,
            "HasGestureIntent" => Self::Gesture(crate::input::gesture_catalog::Group::parse(
                a.text("group").unwrap_or(""),
            )?),
            "HasIntent" | "HasFilteredMotionGraphIntent" => Self::Intent {
                name: a.text("intent").unwrap_or("").into(),
                filtered: a.text("name") == Some("HasFilteredMotionGraphIntent"),
                numeric: numeric(),
            },
            "HasAnimAttribute" => Self::AnimationAttribute {
                name: encode(a.text("attribute").unwrap_or("").as_bytes()),
                sequence_id: f32::from_bits(a.float_bits("sequenceid", (-1.0f32).to_bits())) as i32,
                numeric: numeric(),
            },
            "ExpireInTime" => Self::ExpireInTime(numeric()),
            "WillExpire" => Self::WillExpire {
                in_time: f32::from_bits(a.float_bits("InTime", 0)),
                tag: a
                    .text("InTimeTag")
                    .filter(|v| !v.is_empty())
                    .map(str::to_owned),
                wait_for_transitions: a.boolean_byte("waitForTransitions", 1) != 0,
            },
            "InTimeWindow" => Self::InTimeWindow {
                start: f32::from_bits(a.float_bits("StartTime", 0)),
                length: f32::from_bits(a.float_bits("WindowFrameLength", 0)),
            },
            "InStateForTime" => Self::InStateForTime {
                name: a.text("state").unwrap_or("").into(),
                target: None,
                numeric: numeric(),
            },
            "InParentStateForTime" => Self::InParentStateForTime {
                target: None,
                numeric: numeric(),
            },
            "IsProSkater" => Self::ProSkater(encode(a.text("skater").unwrap_or("").as_bytes())),
            "BreakOutOfPush" => Self::BreakOutOfPush,
            "ShouldLeaveSlide" => Self::ShouldLeaveSlide {
                right: a.boolean_byte("right", 1) != 0,
            },
            "IsRidingGoofy" => Self::RidingGoofy,
            "IsRidingSwitch" => Self::RidingSwitch,
            "MongoPushFootToFar" => Self::MongoPushFootTooFar,
            "LastState" => Self::LastState {
                name: a.text("state").unwrap_or("").into(),
                target: None,
            },
            name if super::motion_gameplay_conditions::GameplayCondition::recognizes(name) => {
                Self::Gameplay(super::motion_gameplay_conditions::GameplayCondition::parse(
                    a,
                )?)
            }
            name if super::motion_riding_conditions::MotionRidingCondition::recognizes(name) => {
                Self::Riding(super::motion_riding_conditions::MotionRidingCondition::parse(a)?)
            }
            "TimeToLand" => Self::TimeToLand(numeric()),
            "OBTimeToLand" => Self::ObTimeToLand(numeric()),
            "OBTrajTime" => Self::ObTrajTime(numeric()),
            "LocoState" => Self::LocoState(super::motion_offboard_cadence::LocoState::parse(a)?),
            "GroundSlopeType" => Self::GroundSlopeType(super::motion_ground_slope::GroundSlopeType::parse(a)?),
            "IsBipedGroundThin" => Self::IsBipedGroundThin,
            "IsHoldingSkateboard" => Self::IsHoldingSkateboard,
            "IsStandingOnMovingObject" => Self::IsStandingOnMovingObject,
            "IsInDebugAnimationsMode" => Self::DebugAnimationsMode,
            name if super::motion_stock_conditions::Condition::recognizes(name) => {
                Self::StockGameplay(super::motion_stock_conditions::Condition::parse(a))
            }
            _ => return Ok(super::condition_nodes::parse(a)?.map(Self::Shared)),
        }))
    }
    pub fn evaluate(&self, host: &MotionHost, frame: &Frame) -> Result<bool, String> {
        use skate_core::animation::playback_parameters::ParameterInputs;
        Ok(match self {
            Self::Grind(condition) => condition.evaluate(
                host.grind_conditions.as_ref()
                    .ok_or("Grind condition requires completed physical output")?
            ),
            Self::CurrentGrabType(grab) => host.animation.grab_type == Some(*grab),
            Self::HasTweak(numeric) => {
                let x = host.animation.filtered_intent("TweakX");
                let y = host.animation.filtered_intent("TweakY");
                (x.is_some() || y.is_some())
                    && (numeric.comparison == Comparison::None
                        || numeric.matches((x.unwrap_or(0.0).powi(2)
                            + y.unwrap_or(0.0).powi(2)).sqrt()))
            }
            Self::ManualOutTimerIsActive => host.riding.manual_out_timer > 0.0,
            Self::ShouldLeaveSlide { right } => host.slide_latch.should_leave(*right),
            Self::Gesture(group) => group.has_intent(|name| host.action_controls.has(name)),
            Self::DisableDismount(condition) => condition.evaluate(host.condition_inputs.push_brake.as_ref())?,
            Self::Shared(condition) => condition
                .evaluate(
                    &host.condition_inputs,
                    &host.action_intents,
                    frame.current,
                    &host.state_parents,
                )
                .map_err(str::to_owned)?,
            Self::Gameplay(condition) => condition.evaluate(host)?,
            Self::Riding(condition) => condition.evaluate(host)?,
            Self::TimeToLand(n) => {
                let p = host.gameplay_conditions.as_ref()
                    .ok_or("TimeToLand requires physical condition publication")?;
                p.time_to_land_valid && n.matches(p.time_to_land)
            }
            Self::ObTimeToLand(n) => n.matches(
                host.gameplay_conditions.as_ref()
                    .ok_or("OBTimeToLand requires physical condition publication")?
                    .offboard_time_to_land,
            ),
            Self::ObTrajTime(n) => {
                let p = host.gameplay_conditions.as_ref()
                    .ok_or("OBTrajTime requires physical condition publication")?;
                p.offboard_trajectory_valid && n.matches(p.offboard_trajectory_time)
            }
            Self::DebugAnimationsMode => false,
            Self::LocoState(condition) => condition.evaluate(
                host.offboard_locomotion_state
                    .ok_or("LocoState requires retained Biped locomotion through OffBoard84")?,
            ),
            Self::GroundSlopeType(condition) => condition.matches(
                host.ground_slope_type
                    .ok_or("GroundSlopeType requires the completed ground slope publication")?,
            ),
            Self::IsBipedGroundThin => host
                .biped_ground_thin
                .ok_or("IsBipedGroundThin requires the native ground geometry publication")?,
            Self::IsHoldingSkateboard => host.toggle_board_physical
                .is_some_and(|p| p.holding_board),
            Self::IsStandingOnMovingObject => host
                .gameplay_conditions
                .ok_or("IsStandingOnMovingObject requires the physical state publication")?
                .moving_object,
            Self::StockGameplay(condition) => condition.evaluate(host)?,
            Self::PushOff(condition) => condition.evaluate(),
            Self::Wipeout(condition) => condition.evaluate(host.wipeout_physical)?,
            Self::Landing(condition) => condition.evaluate(
                host.landing_physical,
                &host.flags,
                host.prelanding_physical
                    .map(|p| p.override_prelanding(&host.spin)),
            )?,
            Self::LastState { target, .. } => {
                let mut cursor = frame.last;
                let mut matched = false;
                if let Some(target) = target {
                    while let Some(state) = cursor {
                        if state == *target {
                            matched = true;
                            break;
                        }
                        cursor = host.state_parents.get(state).copied().flatten();
                    }
                }
                matched
            }
            Self::Intent {
                name,
                filtered,
                numeric,
            } => {
                let value = if *filtered {
                    host.animation.filtered_intent(name)
                } else {
                    host.animation.motion_intent(name)
                };
                value.is_some_and(|value| {
                    numeric.comparison == Comparison::None || numeric.matches(value)
                })
            }
            Self::AnimationAttribute {
                name,
                sequence_id,
                numeric,
            } => super::condition_nodes::animation_attribute(
                host.animation.tree_attributes(),
                *name,
                *sequence_id,
                *numeric,
            ),
            Self::ExpireInTime(numeric) => {
                numeric.matches(host.animation.property().remaining_before_wrap)
            }
            Self::WillExpire {
                in_time,
                tag,
                wait_for_transitions,
            } => {
                let time = match (tag, &host.time_tags) {
                    (Some(tag), Some(tags)) => *tags
                        .get(tag)
                        .ok_or_else(|| format!("Missing MotionGraph float tag {tag}"))?,
                    _ => *in_time,
                };
                if *wait_for_transitions && host.animation.in_transition() {
                    false
                } else {
                    let p = host.animation.property();
                    p.crossed_end || !(p.remaining_before_wrap > time)
                }
            }
            Self::InTimeWindow { start, length } => {
                if host.animation.in_transition() {
                    false
                } else {
                    let time = host.animation.current_time()?;
                    !(time < *start) && !(time > *start + *length)
                }
            }
            Self::InStateForTime {
                target, numeric, ..
            } => super::condition_nodes::state_time(frame, *target, *numeric),
            Self::InParentStateForTime { target, numeric } => {
                super::condition_nodes::state_time(frame, *target, *numeric)
            }
            Self::ProSkater(name) => host.playback_context.pro_skater == *name,
            Self::BreakOutOfPush => {
                let push = host
                    .push_state
                    .as_ref()
                    .ok_or("BreakOutOfPush requires initialized native push state")?;
                !(host.animation.current_time()?
                    < push.out_factor * host.animation.current_length()?)
                    && !push.continue_push
            }
            Self::RidingGoofy => {
                let (first, second) = host
                    .physical_stance
                    .ok_or("IsRidingGoofy requires PhysOutAnimation stance bytes")?;
                first == second
            }
            Self::RidingSwitch => host
                .playback_context
                .is_switch
                .ok_or("IsRidingSwitch requires actual relative stance")?,
            Self::MongoPushFootTooFar => {
                if let Some(event) = host
                    .animation
                    .tree_attributes()
                    .iter()
                    .find(|a| a.name == encode(b"push_contact"))
                {
                    let matches = |name| {
                        event.payload.0[..5]
                            .iter()
                            .zip(encode(name).0)
                            .all(|(word, key)| *word == Some(key))
                    };
                    let right = matches(b"RightToeBase");
                    let left = matches(b"LeftToeBase");
                    let mirrored = host
                        .playback_context
                        .is_mirrored
                        .ok_or("MongoPushFootToFar requires actual mirrored stance")?;
                    if (!mirrored && right) || (mirrored && left) {
                        host.physical
                            .and_then(|p| p.foot_frame)
                            .ok_or("MongoPushFootToFar requires actual foot frame")?
                            .out_distance(right)[0]
                            < -0.5
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        })
    }
}

pub fn bind_states(
    graph: &crate::graph_runtime::LoadedGraph,
    operations: &mut [super::motion_nodes::MotionOperation],
) {
    let mut parents = vec![None; graph.source.elements.len()];
    for (parent, element) in graph.source.elements.iter().enumerate() {
        for &child in &element.children {
            parents[child] = Some(parent);
        }
    }
    for &id in &graph.runtime.operations.conditions {
        let super::motion_nodes::MotionOperation::Condition(condition) = &mut operations[id] else {
            continue;
        };
        let (name, target) = match condition {
            MotionCondition::Shared(ActionCondition::CurrentState { name, target })
            | MotionCondition::InStateForTime { name, target, .. }
            | MotionCondition::LastState { name, target } => (Some(name), target),
            MotionCondition::InParentStateForTime { target, .. } => (None, target),
            _ => continue,
        };
        let mut element = parents[graph.binding.operations[id].element];
        while let Some(id) = element {
            if let Some(state) = graph.binding.states.iter().position(|s| s.element == id) {
                *target = name.as_ref().map_or(Some(state), |name| {
                    graph.binding.find_state(state, name, true)
                });
                break;
            }
            element = parents[id];
        }
    }
}
