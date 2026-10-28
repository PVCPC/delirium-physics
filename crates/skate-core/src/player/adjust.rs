#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdjustFields {
    pub processed_word_2512: u32,
    pub timestep_2604: f32,
    pub controller_state_448: u32,
    pub output_flag_42: bool,
    pub output_scalar_144: f32,
    pub output_counter_200: u32,
}

pub trait AdjustServices {
    fn board_condition_n4(&mut self) -> u32;

    fn controller_mode_one_board_flag_868(&mut self) -> bool;

    fn adjust_skeleton_n5(&mut self, board_flag: bool, timestep: f32);

    fn update_current_state_post_physics_vtable_40(&mut self);

    fn player_output_selector_n6(&mut self) -> u32;

    fn finish_skeleton_adjustment_n7(&mut self, selector: u32);
}

pub fn run_adjust(fields: &mut AdjustFields, services: &mut impl AdjustServices) {
    if services.board_condition_n4() == 1 && fields.processed_word_2512 != 500 {
        fields.output_flag_42 = true;
        fields.output_scalar_144 = 0.0;
        fields.output_counter_200 = fields.output_counter_200.wrapping_add(1);
    }

    let board_flag = if fields.controller_state_448 == 1 {
        services.controller_mode_one_board_flag_868()
    } else {
        false
    };
    services.adjust_skeleton_n5(board_flag, fields.timestep_2604);
    services.update_current_state_post_physics_vtable_40();
    let selector = services.player_output_selector_n6();
    services.finish_skeleton_adjustment_n7(selector);
}

