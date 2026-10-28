use super::*;
use crate::graph_host::motion_kickturn::{self, Operation as KickTurnOperation};
pub(super) fn execute(
    host: &mut MotionHost,
    behavior: BehaviorId,
    operation: MotionOperation,
    frame: &Frame,
    phase: u8,
) -> Result<(), String> {
    let instance = host.instances.get_mut(behavior)
        .ok_or("Unallocated MotionGraph behavior")?;
    match (operation, instance) {
            (MotionOperation::BipedCadence, _) => {
                if phase == 1 {
                    crate::graph_host::motion_offboard_cadence::biped_cadence(
                        host.offboard_cadence_phase,
                        Some(&mut host.animation_phase),
                        phase,
                    );
                }
            }
            (MotionOperation::MatchCadence, Instance::MatchCadence(state)) => {
                match phase {
                    0 => state.begin(host.offboard_cadence_phase, true),
                    1 => state.update(Some(&mut host.animation)),
                    _ => state.end(),
                }
            }
            (MotionOperation::OffboardBodyTweakBlend(settings), Instance::OffboardBodyTweak(state)) => {
                match phase {
                    0 => state.begin(),
                    1 => {
                        let physical = host.gameplay_conditions.as_ref()
                            .ok_or("OffboardBodyTweakBlend requires completed OffBoard output")?;
                        state.update(
                            &settings, &mut host.animation, &mut host.wipeout_controls,
                            body_tweak::Physical {
                                time_to_land: physical.offboard_time_to_land,
                                scalar_92: physical.offboard_air_scalar_92,
                                board_held: host.playback_context.board_available
                                    .ok_or("OffboardBodyTweakBlend requires OffBoard311")?,
                            },
                        )?;
                    }
                    _ => state.end(&mut host.animation),
                }
            }
            (MotionOperation::SetDeckPitchAndYaw { yaw, pitch }, _) => {
                if phase == 0 {
                    let values = host.deck_yaw_pitch
                        .ok_or("SetDeckPitchAndYaw requires completed Skeleton output")?;
                    for (name, value) in [yaw, pitch].into_iter().zip(values) {
                        host.animation.set_attribute(SettableAttribute {
                            name, value, normalized: false, sequence_id: -1,
                        });
                    }
                }
            }
            (MotionOperation::PrintText2D, _) => {}
            (MotionOperation::Shove(operation), Instance::Shove(state)) => match phase {
                0 => state.begin(),
                1 => state.update(
                    &operation,
                    &mut host.animation,
                    host.shove_physical
                        .ok_or("Shove requires actual interaction output")?,
                    host.hand_services.busy_hands,
                    host.playback_context
                        .is_mirrored
                        .ok_or("Shove requires animation stance")?,
                )?,
                _ => state.end(&mut host.animation, host.hand_services.keep_shove_channels),
            },
            (MotionOperation::CharacterGesture, Instance::CharacterGesture(state)) => {
                if phase == 1 {
                    let physical = host.gesture_physical.ok_or(
                        "CharacterGesture requires original physical and hostgesture publication",
                    )?;
                    let gesture_intents = host.animation.motion_intents.clone();
                    let inputs =
                        crate::graph_host::motion_character_gesture::CharacterGestureInputs {
                            motion_intents: &gesture_intents,
                            busy_hands: host.hand_services.busy_hands,
                            filtered_state: host
                                .condition_inputs
                                .physical_state
                                .as_ref()
                                .ok_or("CharacterGesture requires filteredcategory")?
                                .category,
                            ground321: physical.ground321,
                            board_held311: host
                                .playback_context
                                .board_available
                                .ok_or("CharacterGesture requires boardheldbyte311")?,
                            state_offboard75: physical.state_offboard75,
                            distance_to_cog: host
                                .crouching_physical
                                .ok_or("CharacterGesture requires actualheight")?
                                .animation_height_72,
                            selections: physical.selections,
                            suppress_up: physical.suppress_up,
                            force_brake_bypass: physical.force_brake_bypass,
                        };
                    if let Some(output) = state.update(&mut host.animation, inputs)? {
                        host.gesture_publication = Some(output);
                    }
                }
            }
            (MotionOperation::EndGesture, _) => {
                if phase == 0 {
                    host.end_gesture_channels();
                }
            }
            (
                MotionOperation::Native(
                    crate::graph_host::motion_native::Operation::StoreLandingData,
                ),
                Instance::LandingData(state),
            ) => {
                if phase == 1 {
                    state.update(
                        host.condition_inputs
                            .physical_state
                            .as_ref()
                            .ok_or("StoreLandingData requires physical category")?
                            .category,
                        host.native_physical
                            .ok_or("StoreLandingData requires raw physical COM velocity/up")?,
                        &mut host.riding,
                    );
                }
            }
            (
                MotionOperation::Native(crate::graph_host::motion_native::Operation::EndShimmy(
                    time,
                )),
                _,
            ) => {
                if phase == 0 {
                    crate::graph_host::motion_native::end_shimmy(&mut host.animation, time);
                }
            }
            (
                MotionOperation::Native(crate::graph_host::motion_native::Operation::Score {
                    regular,
                    mirrored,
                }),
                _,
            ) => {
                if phase == 1 {
                    host.score_packet.set(
                        if host
                            .playback_context
                            .is_mirrored
                            .ok_or("Score augmentation requires animation stance")?
                        {
                            mirrored
                        } else {
                            regular
                        },
                    );
                }
            }
            (MotionOperation::KickTurn(KickTurnOperation::ResetTimer), _) => {
                if phase == 0 || phase == 2 {
                    host.riding.time_since_kickturn = 0.0;
                }
            }
            (
                MotionOperation::KickTurn(KickTurnOperation::Steering(parameters)),
                Instance::KickTurn(state),
            ) => {
                motion_kickturn::execute(
                    state,
                    parameters,
                    &host.kickturn,
                    &mut host.animation,
                    frame.dt,
                    phase,
                )?;
            }
            (MotionOperation::FakieHeadChannel, Instance::FakieHead(state)) => {
                if phase == 1 {
                    let fakie = host
                        .animation
                        .skater_animation_flags
                        .ok_or("FakieHeadChannel requires actual animation flags")?
                        & 0x20000000
                        != 0;
                    state.update(
                        &mut host.animation,
                        host.flags.manualing,
                        host.is_power_sliding,
                        fakie,
                    )?;
                }
            }
            (MotionOperation::Pumping, Instance::Pumping(state)) => {
                const NAMES: [&str; 5] = ["PUMP0", "PUMP1", "PUMP2", "PUMP3", "PUMP4"];
                if phase == 2 {
                    for name in NAMES {
                        host.animation.channels.end(name);
                    }
                } else if phase == 1 && host.allow_pumping {
                    let occupied = std::array::from_fn(|i| host.animation.channels.has(NAMES[i]));
                    let update = state.update(
                        host.pumping_acceleration
                            .ok_or("Pumping requires actual ground pumping acceleration")?,
                        occupied,
                        &host.pumping,
                    );
                    if let Some(index) = update.start {
                        let settings = skate_core::animation::channel_playback::ChannelSettings {
                            priority: 0,
                            keep_alive: false,
                            mirrored: false,
                            speed: 1.0,
                            blend_in: host.pumping.blend_in,
                            hold_during_blend_in: false,
                            blend_out: host.pumping.blend_out,
                            hold_during_blend_out: true,
                            use_attributes: false,
                        };
                        host.animation
                            .new_channel(NAMES[index], "B_PUMP", settings)?;
                    }
                    if let Some((index, value)) = update.influence {
                        host.animation.channels.influence(NAMES[index], value);
                    }
                }
            }
            (MotionOperation::DisallowPumping, _) => {
                if phase == 0 {
                    for name in ["PUMP0", "PUMP1", "PUMP2", "PUMP3", "PUMP4"] {
                        host.animation
                            .channels
                            .end_with(name, f32::from_bits(0x3dcccccd), false);
                    }
                    host.allow_pumping = false;
                } else if phase == 2 {
                    host.allow_pumping = true;
                }
            }
            (MotionOperation::SetSpeed { name, value }, _) => {
                if phase == 1 {
                    let value = match value {
                        Some(value) => value,
                        None => {
                            host.crouching_physical
                                .ok_or("SetSpeed requires actual ground-projected speed")?
                                .body_164
                        }
                    };
                    host.animation.set_attribute(SettableAttribute {
                        name,
                        value: value.abs(),
                        normalized: false,
                        sequence_id: -1,
                    });
                }
            }
            (MotionOperation::UpdateRidingFakie(settings), Instance::RidingFakie(state)) => {
                if phase == 1 {
                    let mut physical = host
                        .fakie_physical
                        .ok_or("UpdateRidingFakie requires actual physical outputs")?;
                    physical.doing_trick = host.flags.doing_trick;
                    if let Some(fakie) = state.update(physical, frame.dt, settings) {
                        let flags = host
                            .animation
                            .skater_animation_flags
                            .as_mut()
                            .ok_or("UpdateRidingFakie requires actual animation flags")?;
                        *flags = (*flags & !0x20000000) | if fakie { 0x20000000 } else { 0 };
                    }
                }
            }
            (MotionOperation::StockGameplay(operation), instance) => {
                match operation {
                    crate::graph_host::motion_stock_gameplay::Operation::FingerFlipOut { grab_intent } => {
                        let Instance::FingerFlipOut(state) = instance else {
                            return Err("FingerFlipOut operation/instance mismatch".into());
                        };
                        if phase == 0 {
                            state.begin();
                        } else if phase == 1 {
                            let value = state.update(
                                host.animation.motion_intents.contains_key(&grab_intent),
                                frame.dt, &host.finger_flip,
                            );
                            host.animation.set_attribute(SettableAttribute {
                                name: encode(b"Grabbing"), value,
                                normalized: false, sequence_id: -1,
                            });
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::MatchAirTime => {
                        let Instance::MatchAirTime(state) = instance else {
                            return Err("MatchAirTime operation/instance mismatch".into());
                        };
                        if phase != 2 {
                            let p = host.gameplay_conditions
                                .ok_or("MatchAirTime requires completed OffBoard output")?;
                            if phase == 0 {
                                state.begin(host.offboard_cadence_phase
                                    .ok_or("MatchAirTime requires completed OffBoard80")?,
                                    p.offboard_air_scalar_92, p.offboard_air_translation);
                            } else {
                                state.update(&mut host.animation, p.offboard_time_to_land,
                                    p.offboard_air_scalar_92, p.offboard_air_translation);
                            }
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::InitMovingObjects { path } => {
                        if phase == 0 {
                            host.moving_objects.register(path);
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::MovingObject { path } => {
                        match phase {
                            0 => host.moving_objects.begin(&path)?,
                            1 => {
                                let active = host.moving_objects.update(&path)?;
                                if !active {
                                    return Err(format!("MovingObject is not active: {path}"));
                                }
                            }
                            _ => host.moving_objects.end(),
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::LandOnBoard => {
                        if phase == 0 && host.gameplay_conditions.as_ref()
                            .ok_or("LandOnBoard requires completed physical output")?.landing_turning {
                            *host.animation.skater_animation_flags.as_mut()
                                .ok_or("LandOnBoard requires SkaterAnim orientation")? ^= 0x80000000;
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::UpdateStandingOnCar => {
                        let _ = phase;
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::SetGrabType { grab } => {
                        let grab = crate::graph_host::motion_stock_gameplay::GrabType::parse(&grab)?;
                        if phase == 0 {
                            host.animation.set_grab_type(grab);
                        } else if phase == 2 {
                            host.animation.clear_grab_type();
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::ScoringGrabs {
                        grab_name, up, left, down, right, intent_x, intent_y, invert_y, polar,
                    } => {
                        if phase == 1 {
                            let first = host.animation.filtered_intent(&intent_x).unwrap_or(0.0);
                            let second = host.animation.filtered_intent(&intent_y).unwrap_or(0.0);
                            let (x, y) = if polar {
                                let x = second * first.sin();
                                let y = second * first.cos();
                                (x, if invert_y { -y } else { y })
                            } else {
                                (first, second)
                            };
                            let selected = crate::graph_host::motion_stock_gameplay::select_grab_score(
                                &grab_name, [up.as_deref(), left.as_deref(), down.as_deref(), right.as_deref()], x, y,
                            );
                            host.score_packet.grab = Some((encode(selected.as_bytes()), [x, y]));
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::TweakProject {
                        intent_x, intent_y, attribute_x, attribute_y,
                    } => {
                        if phase == 1 {
                            let source_x = host.animation.filtered_intent(&intent_x);
                            let source_y = host.animation.filtered_intent(&intent_y);
                            if source_x.is_none() && source_y.is_none() {
                                return Ok(());
                            }
                            let mut x = source_x.unwrap_or(0.0);
                            let mut y = source_y.unwrap_or(0.0);
                            let total = x.abs() + y.abs();
                            if total > 1.0 {
                                let scale = 1.0 / total;
                                x *= scale;
                                y *= scale;
                            }
                            host.animation.set_attribute(SettableAttribute {
                                name: attribute_x,
                                value: x,
                                normalized: false,
                                sequence_id: -1,
                            });
                            host.animation.set_attribute(SettableAttribute {
                                name: attribute_y,
                                value: y,
                                normalized: false,
                                sequence_id: -1,
                            });
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::JumpInto { attribute } => {
                        let Instance::JumpInto { first_update } = instance else {
                            return Err("JumpInto instance was not allocated".into());
                        };
                        if phase == 1 && *first_update {
                            host.animation.jump_into(attribute)?;
                            *first_update = false;
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::SetManualAngle { attribute } => {
                        let state = match instance {
                            Instance::SetManualAngle(state) => state,
                            _ => return Err("SetManualAngle instance was not allocated".into()),
                        };
                        match phase {
                            0 => state.begin(),
                            1 => {
                                let value = state.update(host.animation.motion_intent("Manual"), &host.manual);
                                host.animation.set_attribute(SettableAttribute {
                                    name: attribute,
                                    value,
                                    normalized: false,
                                    sequence_id: -1,
                                });
                            }
                            _ => {}
                        }
                    }
                    crate::graph_host::motion_stock_gameplay::Operation::HippyJumpAntic => {
                        let state = match instance {
                            Instance::HippyJumpAntic(state) => state,
                            _ => return Err("HippyJumpAntic instance was not allocated".into()),
                        };
                        match phase {
                            0 => {
                                state.begin();
                            }
                            1 => state.update(&mut host.animation, &host.hippy_jump),
                            2 => {
                                state.end();
                            }
                            _ => unreachable!(),
                        }
                    }
                    other => {
                        return Err(format!(
                            "MotionGraph stock gameplay producer {other:?} is not implemented"
                        ));
                    }
                }
            }
            (MotionOperation::SettingBodyTilt(name), Instance::BodyTilt(instance)) => {
                if phase == 1 && host.applying_body_tilt {
                    if let Some(value) = instance.update(
                        true,
                        host.playback_context
                            .is_mirrored
                            .ok_or("SettingBodyTilt needs actual mirrored stance")?,
                        host.body_tilt_physical
                            .ok_or("SettingBodyTilt needs actual PhysOut tilt and spin")?,
                        &host.body_tilt,
                    ) {
                        host.animation.set_attribute(SettableAttribute {
                            name,
                            value,
                            normalized: false,
                            sequence_id: -1,
                        });
                    }
                } else if phase == 1 {
                    instance.disable();
                }
            }
            (MotionOperation::Play(operation), Instance::Play(instance)) => match phase {
                0 => instance.begin(&operation, &mut host.playback_context, &mut host.animation)?,
                1 => instance.update(&operation, &mut host.animation)?,
                _ => instance.end(),
            },
            (
                MotionOperation::AttachIntent {
                    intent,
                    attribute,
                    set,
                },
                _,
            ) => {
                if phase == 1 {
                    host.animation.attach(&intent, attribute, set);
                }
            }
            (MotionOperation::Riding(operation), _) => host.riding.execute(
                operation,
                phase,
                frame,
                &mut host.animation,
                host.condition_inputs
                    .physical_state
                    .as_ref()
                    .map(|p| p.category),
                host.crouching_physical.map(|p| p.animation_height_72),
                host.flags.manualing,
            )?,
            (MotionOperation::ApplyingBodyTilt, _) => {
                if phase == 0 {
                    host.applying_body_tilt = true;
                } else if phase == 2 {
                    host.applying_body_tilt = false;
                }
            }
            (MotionOperation::Crouching(name), Instance::Crouching(state)) => {
                use skate_core::animation::crouching;
                if phase == 0 {
                    *state = Some(crouching::State::begin(
                        host.crouching_physical
                            .ok_or("Crouching Begin requires real physical height and speed")?,
                        &host.crouching,
                    ));
                } else if phase == 1 {
                    let intents = crouching::Intents {
                        auto_pump_angle: host.animation.motion_intent("AutoPumpAngle"),
                        auto_pump_magnitude: host.animation.motion_intent("AutoPumpMag"),
                        crouch: host.animation.motion_intent("Crouch"),
                        hard_turn_crouch: host.animation.motion_intent("HardTurnCrouch"),
                        manual: host.animation.motion_intent("Manual"),
                        motion_flag_108: host.is_power_sliding,
                    };
                    let result = state
                        .as_mut()
                        .ok_or("Crouching updated before Begin")?
                        .update(
                            host.crouching_physical
                                .ok_or("Crouching requires actual physical outputs")?,
                            intents,
                            frame.dt,
                            &host.crouching,
                        );
                    if result.new_auto_pump {
                        host.animation.emit_packet(encode(b"NewAutoPump"), 1.0);
                    }
                    if result.player_controlled_pump {
                        host.animation
                            .emit_packet(encode(b"PlayerControlledPump"), 1.0);
                    }
                    host.animation.set_attribute(SettableAttribute {
                        name,
                        value: result.height,
                        normalized: false,
                        sequence_id: -1,
                    });
                }
            }
            (MotionOperation::SetTurning(names), Instance::Turning(instance)) => {
                if phase == 0 {
                    instance.enter();
                } else if phase == 1 {
                    let physical = host
                        .physical
                        .ok_or("SetTurning requires actual PhysOutAnimation and stance outputs")?;
                    let intents = set_turning::Intents {
                        fakie_turn: host.animation.motion_intent("FakieTurn"),
                        mode_0_slide: host.animation.motion_intent("LeftSlide"),
                        mode_1_slide: host.animation.motion_intent("RightSlide"),
                    };
                    let flags = host
                        .animation
                        .skater_animation_flags
                        .ok_or("SetTurning requires actual animation stance flags")?;
                    set_turning::update(
                        instance,
                        &mut host.slide_latch,
                        physical.turning,
                        (flags & 0x2000_0000 != 0, flags & 0x4000_0000 != 0),
                        frame.dt,
                        &host.turning,
                        intents,
                        |attribute, value| {
                            let index = match attribute {
                                set_turning::Attribute::Angle => Some(0),
                                set_turning::Attribute::Direction => Some(1),
                                set_turning::Attribute::Quickness => Some(2),
                                set_turning::Attribute::Speed => Some(3),
                                set_turning::Attribute::Holding => Some(4),
                                _ => None,
                            };
                            if let Some(index) = index {
                                host.animation.set_attribute(SettableAttribute {
                                    name: names[index],
                                    value,
                                    normalized: false,
                                    sequence_id: -1,
                                });
                            } else {
                                host.animation.emit_packet(
                                    encode(if attribute == set_turning::Attribute::Turn {
                                        b"Turn"
                                    } else {
                                        b"Slide"
                                    }),
                                    value,
                                );
                            }
                        },
                    );
                }
            }
            (MotionOperation::Push(_), Instance::Push(instance)) => {
                let physical = host
                    .physical
                    .ok_or("Push behavior requires actual skater physical outputs")?;
                let intents = host.animation.motion_intents.clone();
                let mut context = PushContext {
                    settings: &host.pushing,
                    shared: host
                        .push_state
                        .as_mut()
                        .ok_or("Native push state initialization has not been published")?,
                    motion_intents: &intents,
                    forward_speed: physical.forward_speed,
                    delta_seconds: frame.dt,
                    time_since_teleport: host.riding.time_since_teleport,
                    is_switch: physical.is_switch,
                    foot_frame: physical.foot_frame,
                };
                match phase {
                    0 => instance.begin(&mut context, &mut host.animation),
                    1 => instance.update(&mut context, &mut host.animation)?,
                    _ => instance.end(&mut context),
                }
            }
            (MotionOperation::Unsupported { kind, name }, _) => {
                return Err(format!("Unsupported MotionGraph {kind:?} {name}"));
            }
            (operation, _) => {
                return Err(format!(
                    "Invalid MotionGraph behavior instance {operation:?}"
                ));
            }
        }

        Ok(())
}
