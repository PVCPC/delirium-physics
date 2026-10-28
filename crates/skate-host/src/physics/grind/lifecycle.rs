use skate_core::physics::{
    board::BodyId, board_runtime::BoardRuntime, skeleton_body::SkeletonCollisionMode,
};
const FEET: [usize; 4] = [19, 15, 20, 16];

pub(crate) fn enter(
    board: &mut BoardRuntime,
    animated_290: &mut u8,
    skeleton_16505: &mut bool,
    collision: &mut SkeletonCollisionMode,
) {
    board.hook_mut().drive.disable_animation(animated_290);
    *skeleton_16505 = true;
    for part in FEET {
        collision.parts[part].volume_group = 4;
        collision.parts[part].enabled = false;
        collision.disable_count[part] = 0;
    }
    board.bodies_mut()[BodyId::Deck.index()]
        .inertia
        .angular_drag = f32::from_bits(0x4066_6665);
}

pub(crate) fn exit(
    board: &mut BoardRuntime,
    animated_290: &mut u8,
    collision: &mut SkeletonCollisionMode,
    bone_has_collision: [bool; 4],
    standard_angular_drag: f32,
) {
    for (part, enabled) in FEET.into_iter().zip(bone_has_collision) {
        collision.normal_bone(part, enabled);
    }
    board.bodies_mut()[BodyId::Deck.index()]
        .inertia
        .angular_drag = standard_angular_drag;
    board.hook_mut().drive.disable_animation(animated_290);
    clear_wheel_spin(board);
}

pub(crate) fn manage_wheel_spin(board: &mut BoardRuntime, contact: [bool; 4]) {
    for (wheel, contact) in board.bodies_mut()[..4].iter_mut().zip(contact) {
        wheel.inertia.angular_drag =
            (if contact { 0.5 } else { 0.0 }) * f32::from_bits(0x426f_ffff);
    }
}

pub(crate) fn clear_wheel_spin(board: &mut BoardRuntime) {
    for wheel in &mut board.bodies_mut()[..4] {
        wheel.inertia.angular_drag = 0.0;
    }
}
