use super::{arithmetic::dot3, board::apply_world_force};
use skate_core::physics::board_runtime::BoardRuntime;
type V = [f32; 4];

pub(super) fn apply(
    board: &mut BoardRuntime,
    position: V,
    point: V,
    across: V,
) -> Result<(), String> {
    let offset = core::array::from_fn(|i| position[i] - point[i]);
    let inward = across.map(|v| if dot3(across, offset) > 0. { -v } else { v });
    let angle = 45.0_f32 * f32::from_bits(0x3c8e_fa35);
    let (sine, cosine) = skate_core::trigonometry::sin_cos(angle);
    let mut force = inward.map(|v| (v * 100.) * cosine);
    force[1] -= sine;
    apply_world_force(board, force, position);
    Ok(())
}
