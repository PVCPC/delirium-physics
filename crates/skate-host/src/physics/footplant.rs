use skate_core::air::trajectory::{QueryResult, Trajectory};
use skate_data::collections::Collections;
pub(super) mod math;
mod pose;
mod settings;
mod update;
pub(super) mod ground;
use settings::Settings;
pub(crate) use update::FootplantFrame;
pub(crate) type V = [f32; 4];
pub(crate) struct Footplant {
    pub enabled: bool,
    pub(super) result: QueryResult,
    pub(super) settings: Settings,
    pub selected_toe: Option<usize>,
    pub candidate: bool,
    pub hit: bool,
    pub perform: bool,
    pub flag_627: bool,
    pub launch_valid: bool,
    pub lock_valid: bool,
    pub contact_active: bool,
    pub requested: bool,
    pub contact_time: f32,
    pub scalar_596: f32,
    pub scalar_600: f32,
    pub target_blend: f32,
    pub active_elapsed: f32,
    pub scalar_612: f32,
    pub surface: u32,
    pub contact: V,
    pub adjusted_contact: V,
    pub(super) current_up: V,
    pub(super) selected_world: V,
    pub(super) selected_record: V,
    pub(super) leg_direction: V,
    pub(super) vectors_352_368: [V; 2],
    pub(super) curve: [V; 4],
    pub(super) physical_com: V,
    pub(super) animation_com: V,
    pub(super) launch_direction: V,
    pub(super) locked_target: V,
    pub(super) request: Trajectory,
    pub(super) completed_trajectory: Trajectory,
}
impl Footplant {
    pub fn load(data: &Collections) -> Result<Self, String> {
        let mut state = Self {
            enabled: false,
            result: QueryResult::miss(),
            settings: Settings::load(data)?,
            selected_toe: None,
            candidate: false,
            hit: false,
            perform: false,
            flag_627: false,
            launch_valid: false,
            lock_valid: false,
            contact_active: false,
            requested: false,
            contact_time: -1.0,
            scalar_596: -1.0,
            scalar_600: 0.0,
            target_blend: 0.0,
            active_elapsed: 0.0,
            scalar_612: 0.0,
            surface: 0,
            contact: [0.0; 4],
            adjusted_contact: [0.0; 4],
            current_up: [0.0; 4],
            selected_world: [0.0; 4],
            selected_record: [0.0; 4],
            leg_direction: [0.0; 4],
            vectors_352_368: [[0.0; 4]; 2],
            curve: [[0.0; 4]; 4],
            physical_com: [0.0; 4],
            animation_com: [0.0; 4],
            launch_direction: [0.0; 4],
            locked_target: [0.0; 4],
            request: Trajectory {
                position: [0.0; 4],
                velocity: [0.0; 4],
                acceleration: [0.0; 4],
                duration: -1.0,
            },
            completed_trajectory: Trajectory { position: [0.0;4], velocity: [0.0;4], acceleration: [0.0;4], duration: -1.0 },
        };
        state.reset();
        Ok(state)
    }
    pub fn reset(&mut self) {
        self.contact_time = -1.0;
        self.scalar_596 = -1.0;
        self.selected_toe = None;
        self.scalar_600 = 0.0;
        self.target_blend = 0.0;
        self.active_elapsed = 0.0;
        self.scalar_612 = 0.0;
        self.candidate = false;
        self.hit = false;
        self.perform = false;
        self.flag_627 = false;
        self.launch_valid = false;
        self.lock_valid = false;
        self.contact_active = false;
        self.requested = false;
        self.request = Trajectory {
            position: [0.0; 4],
            velocity: [0.0; 4],
            acceleration: [0.0; 4],
            duration: -1.0,
        };
        self.contact = [0.0; 4];
        self.adjusted_contact = [0.0; 4];
        self.vectors_352_368 = [[0.0; 4]; 2];
        self.animation_com = [0.0; 4];
        self.locked_target = [0.0; 4];
        self.launch_direction = [0.0; 4];
        self.physical_com = [0.0; 4];
    }
    pub fn full_reset(&mut self) {
        self.enabled = false;
        self.reset();
    }
    pub fn publish(&self, out: &mut skate_core::player::input_phase::AirOutputFields) {
        if self.hit {
            out.footplant_contact_time_208 = self.contact_time;
            out.footplant_duration_212 = self.scalar_596;
            out.flag_447 = u8::from(self.perform);
            out.flag_448 = u8::from(self.perform || self.flag_627);
            out.footplant_surface_224 = self.surface;
            out.footplant_left_449 = u8::from(self.flag_627 && self.selected_toe == Some(15));
            out.footplant_right_450 = u8::from(self.flag_627 && self.selected_toe == Some(19));
            out.footplant_surface_height_216 = self.contact[1];
        }
    }
}
