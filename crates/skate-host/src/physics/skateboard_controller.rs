use skate_core::physics::board_ground::BoardGroundState;
pub(crate) use skate_core::physics::skateboard_controller::SkateboardController;

pub(crate) fn partial_request(
    controller: &SkateboardController,
    ground: &BoardGroundState,
) -> bool {
    controller.request_partial_ragdoll(ground.part_contact_count)
}
