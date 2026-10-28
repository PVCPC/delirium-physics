use crate::{
    physics::rigid_body::RetailInertiaDynamics,
    riding::braking::{LinearDragInput, LinearDragSettings, calculate_linear_drag},
};

pub const DRAG_FREQUENCY: f32 = f32::from_bits(0x426f_ffff);

#[derive(Clone, Copy, Debug)]
pub struct GroundDragInput {
    pub flags_2468: u32,
    pub absolute_body_speed_2616: f32,
    pub balance_2720: f32,
    pub scalar_2724: f32,
    pub ground_normal_y: f32,
}

impl GroundDragInput {
    pub fn calculate(self, settings: LinearDragSettings) -> f32 {
        calculate_linear_drag(
            LinearDragInput {
                flags_2468: self.flags_2468,
                absolute_body_speed: self.absolute_body_speed_2616,
                balance_2720: self.balance_2720,
                scalar_2724: self.scalar_2724,
                comparison_scalar: self.ground_normal_y,
            },
            settings,
        )
    }
}

pub struct BodyInertias<'a> {
    pub part_inertia_indices: &'a [usize],
    pub inertias: &'a mut [RetailInertiaDynamics],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragSelection {
    AllParts,
    Part(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragBindingError {
    PartOutsideAssembly,
    InertiaOutsideStorage,
}

impl BodyInertias<'_> {
    pub fn set_linear_drag(
        &mut self,
        drag: f32,
        selection: DragSelection,
    ) -> Result<(), DragBindingError> {
        let selected = match selection {
            DragSelection::AllParts => self.part_inertia_indices,
            DragSelection::Part(part) => self
                .part_inertia_indices
                .get(part..part.saturating_add(1))
                .filter(|parts| parts.len() == 1)
                .ok_or(DragBindingError::PartOutsideAssembly)?,
        };
        if selected.iter().any(|&index| index >= self.inertias.len()) {
            return Err(DragBindingError::InertiaOutsideStorage);
        }
        let mut groups = selected.chunks_exact(4);
        for group in &mut groups {
            let [first, second, third, fourth] = [group[0], group[1], group[2], group[3]];
            let coefficient = drag * DRAG_FREQUENCY;
            self.inertias[first].linear_drag = coefficient;
            self.inertias[second].linear_drag = coefficient;
            self.inertias[third].linear_drag = coefficient;
            self.inertias[fourth].linear_drag = coefficient;
        }
        for &index in groups.remainder() {
            self.inertias[index].linear_drag = drag * DRAG_FREQUENCY;
        }
        Ok(())
    }

    pub fn apply_ground_drag(&mut self, calculated_drag: f32) -> Result<(), DragBindingError> {
        self.set_linear_drag(calculated_drag, DragSelection::AllParts)
    }
}
