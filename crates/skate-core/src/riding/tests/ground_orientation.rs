use super::*;
fn constant(value: f32) -> PointGraph<8> {
    PointGraph { x: [0.0,1.0,2.0,3.0,4.0,5.0,6.0,7.0], y: [value;8] }
}
fn settings() -> GroundOrientationSettings {
    GroundOrientationSettings {
        ground_normal_smoothing: [1.0,0.0,0.0,1.0],
        up_vector_smoothing_slow: [1.0,0.0,0.0,1.0],
        up_vector_smoothing_fast: [1.0,0.0,0.0,1.0],
        dynamic_up_vs_ground_y: constant(1.0), ground_vector_blend: constant(0.0),
        deck_angle_usage_vs_speed: constant(0.0), up_vector_smoothing_vs_speed: constant(0.0),
        up_vector_max_delta_vs_speed: constant(1.0), ground_blend_max_delta:0.1,
        up_vector_max_acceleration:2.0, anti_wobble_damping:0.5, extra_side_damping:0.6,
        minimum_wheels_for_ground_blend:0,
    }
}
fn input(normal: Vector3) -> GroundOrientationInput {
    GroundOrientationInput { com_to_deck: UP, ground_normal:normal,dynamic_up:normal,
        speed:0.0,wheel_contact_count:4,animation_balance:0.0,deck_angle_curve_input:0.0,
        board_up:UP,board_forward:Vector3::new(0.0,0.0,1.0),
        effective_board_forward:Vector3::new(0.0,0.0,1.0),
        previous_reckoning_right:Vector3::new(1.0,0.0,0.0),prevent_up_behind_board:false }
}
#[test]
fn stationary_filter_startup_has_no_false_tilt_or_acceleration() {
    let s=settings();let mut state=GroundOrientation::new(&s);
    for _ in 0..120 {state.update(&s,input(UP));}
    assert_eq!(state.up,UP);assert_eq!(state.ground_normal,UP);
    assert_eq!(state.up_velocity,Vector3::ZERO);
}
#[test]
fn changed_contact_normal_does_not_instantly_replace_the_skater_up() {
    let s=settings();let mut state=GroundOrientation::new(&s);
    let wall=Vector3::new(1.0,0.0,0.0);state.update(&s,input(wall));
    assert_eq!(state.ground_normal,wall);
    assert_ne!(state.up,state.ground_normal);
    assert!(state.up.y>0.99 && state.up.x>0.0 && state.up.x<0.04);
    assert_eq!(&state.slow_filter.words()[4..8],&lanes(state.up).map(f32::to_bits));
}
