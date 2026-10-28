use crate::animation::{output::attributes::AttributeName, skeleton_input::name::encode};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum FilteredCategory {
    #[default]
    Invalid = 0,
    Ground = 1,
    Air = 2,
    Grind = 3,
    Wipeout = 4,
    Teleport = 5,
    Offboard = 6,
    OffboardAir = 7,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrindState {
    pub kind: i32,
    pub scorable_id: i32,
    pub name: AttributeName,
    pub scoring_name: AttributeName,
    pub on_front: bool,
    pub crouch: f32,
    pub pathed_guid: u64,
    pub local_guid: u64,
}
impl Default for GrindState {
    fn default() -> Self {
        Self {
            kind: -1, scorable_id: -1,
            name: encode(b""), scoring_name: encode(b""),
            on_front: false, crouch: 0.0,
            pathed_guid: u64::MAX, local_guid: u64::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FilteredStateInput {
    pub physics_category: i32,
    pub physics_state: i32,
    pub anything_in_contact: bool,
    pub physics_surface_type: i32,
    pub wall_ride_exit: bool,
    pub targeting_grind: bool,
    pub offboard_has_landed: bool,
    pub offboard_on_deck: bool,
    pub grind: GrindState,
    pub last_grind_distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilteredStateOutput {
    pub category: FilteredCategory,
    pub previous_category: FilteredCategory,
    pub grinding: bool,
    pub grind: GrindState,
    pub last_grind_distance: f32,
}

#[derive(Clone, Debug)]
pub struct FilteredState {
    pub category: FilteredCategory,
    pub previous_category: FilteredCategory,
    pub previous_physics_state: i32,
    pub air_count: i32,
    pub nonspecific_count: i32,
    pub nonspecific_collision_free_count: i32,
    pub nonspecific_collision_count: i32,
    pub frames_since_ground_stairs: i32,
    pub must_change: bool,
    cached_grind: GrindState,
}
impl Default for FilteredState {
    fn default() -> Self {
        Self {
            category: FilteredCategory::Invalid,
            previous_category: FilteredCategory::Invalid,
            previous_physics_state: 0,
            air_count: 0, nonspecific_count: 0,
            nonspecific_collision_free_count: 0, nonspecific_collision_count: 0,
            frames_since_ground_stairs: 0, must_change: true,
            cached_grind: GrindState::default(),
        }
    }
}
impl FilteredState {
    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn update(&mut self, input: FilteredStateInput) -> FilteredStateOutput {
        use FilteredCategory::*;
        let old = self.category;
        self.previous_category = old;
        self.must_change = old == Wipeout || old == Invalid
            || self.previous_physics_state == 702 && input.physics_state != 702;
        self.air_count = count(self.air_count, input.physics_category == 200);
        self.nonspecific_count = count(self.nonspecific_count, input.physics_state == 701);
        self.nonspecific_collision_free_count = count(self.nonspecific_collision_free_count,
            input.physics_state == 701 && !input.anything_in_contact);
        self.nonspecific_collision_count = count(self.nonspecific_collision_count,
            input.physics_state == 701 && input.anything_in_contact);
        self.frames_since_ground_stairs = count(self.frames_since_ground_stairs,
            !(input.physics_category == 100 && input.physics_surface_type == 8));
        self.category = match input.physics_state {
            100 => if input.wall_ride_exit { Air } else { Ground },
            101..=105 | 602 => Ground,
            200 => {
                let air_delay_passed = self.air_count > 3;
                if old == Air {
                    if air_delay_passed && input.anything_in_contact || self.must_change {
                        Ground
                    } else { Air }
                } else {
                    let may_enter_air = air_delay_passed
                        && (old != Ground || self.frames_since_ground_stairs > 9);
                    if may_enter_air || self.must_change { Air } else { old }
                }
            }
            201 => if input.anything_in_contact && !input.targeting_grind && self.air_count > 5 {
                Ground
            } else { Air },
            202 | 600 | 601 => Air,
            300 => Wipeout,
            400..=405 => { self.cached_grind = input.grind; Grind }
            500 | 502 => Offboard,
            501 => if input.offboard_has_landed { Offboard } else { OffboardAir },
            503 => if input.offboard_on_deck { Ground } else { Offboard },
            702 => Teleport,
            _ => old,
        };
        self.previous_physics_state = input.physics_state;
        let grinding = self.category == Grind;
        FilteredStateOutput {
            category: self.category,
            previous_category: self.previous_category,
            grinding,
            grind: if grinding { self.cached_grind } else { GrindState::default() },
            last_grind_distance: input.last_grind_distance,
        }
    }
}
fn count(previous: i32, active: bool) -> i32 {
    if active { previous.wrapping_add(1) } else { 0 }
}

