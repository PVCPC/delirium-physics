pub trait PostStateServices {
    fn update_current_state_vtable_12(&mut self);
}

pub fn run_post_state(services: &mut impl PostStateServices) {
    services.update_current_state_vtable_12();
}

