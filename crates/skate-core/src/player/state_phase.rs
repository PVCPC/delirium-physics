#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatePhaseFields {
    pub elapsed_1344: f32,
    pub timestep_2604: f32,
    pub controller_state_448: u32,
    pub controller_system_on_452: bool,
}

pub trait StatePhaseServices {
    fn update_current_state_vtable_8(&mut self);

    fn update_controller_n18(&mut self);
    fn update_controller_mode_1_n19(&mut self);
    fn update_controller_mode_2_n20(&mut self);
    fn update_controller_mode_3_n21(&mut self);
    fn update_controller_mode_4_n22(&mut self);

    fn touch_skateboard_vtable_116(&mut self);
    fn apply_skateboard_force_queue_n23(&mut self);
    fn update_skateboard_fixed_step_cache_n24(&mut self);
}

pub fn run_state_phase(fields: &mut StatePhaseFields, services: &mut impl StatePhaseServices) {
    services.update_current_state_vtable_8();

    if fields.controller_system_on_452 {
        services.update_controller_n18();
        match fields.controller_state_448 {
            1 => services.update_controller_mode_1_n19(),
            2 => services.update_controller_mode_2_n20(),
            3 => services.update_controller_mode_3_n21(),
            4 => services.update_controller_mode_4_n22(),
            _ => {}
        }
    }

    services.touch_skateboard_vtable_116();
    services.apply_skateboard_force_queue_n23();
    services.update_skateboard_fixed_step_cache_n24();
    fields.elapsed_1344 += fields.timestep_2604;
}

