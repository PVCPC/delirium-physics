use super::super::riding_outputs::RidingOutputs;
use skate_core::physics::{board_runtime::BoardRuntime, contact_feedback::choose_surface};
pub(crate) fn active_surface(riding: &RidingOutputs, board: &BoardRuntime) -> u32 {
    let contacts = std::array::from_fn(|i| riding.ground.parts[i].in_contact);
    let forced = board
        .contact_reports()
        .iter()
        .any(|r| r.other_surface & 0x0f80 == 0x0600);
    choose_surface(riding.wheel_lines.physics_surfaces, contacts, forced)
}

pub(crate) fn surface_key(mode: u32) -> Result<&'static str, String> {
    match mode {
        1 => Ok("smooth"),
        2 => Ok("rough"),
        3 => Ok("slow"),
        4 => Ok("slippery"),
        5 => Ok("veryslow"),
        _ => Err(format!(
            "Processed surface mode {mode} was not normalized by SurfacePhysics"
        )),
    }
}
