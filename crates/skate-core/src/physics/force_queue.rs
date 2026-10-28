use crate::math::{Basis3, Vector3};

use super::point_force::{RetailForceAccumulator, accumulate_point_force};

pub const FORCE_CAPACITY: usize = 21;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QueuedPointForce {
    pub tag: u32,
    pub force_world: Vector3,
    pub point_body: Vector3,
}

#[derive(Clone, Debug)]
pub struct BoardForceQueue {
    entries: [QueuedPointForce; FORCE_CAPACITY],
    count: usize,
}

impl Default for BoardForceQueue {
    fn default() -> Self {
        Self {
            entries: [QueuedPointForce::default(); FORCE_CAPACITY],
            count: 0,
        }
    }
}

impl BoardForceQueue {
    pub fn append(&mut self, entry: QueuedPointForce) -> bool {
        if self.count == FORCE_CAPACITY {
            return false;
        }
        self.entries[self.count] = entry;
        self.count += 1;
        true
    }

    pub fn entries(&self) -> &[QueuedPointForce] {
        &self.entries[..self.count]
    }

    pub fn apply_to_deck(
        &self,
        mut accumulator: RetailForceAccumulator,
        deck_basis: Basis3,
        inverse_mass: f32,
        world_inverse_inertia: Basis3,
        force_point_y_offset: f32,
    ) -> RetailForceAccumulator {
        for entry in self.entries() {
            let point = Vector3::new(
                entry.point_body.x + 0.0,
                entry.point_body.y + force_point_y_offset,
                entry.point_body.z + 0.0,
            );
            accumulator = accumulate_point_force(
                accumulator,
                entry.force_world,
                point,
                deck_basis,
                inverse_mass,
                world_inverse_inertia,
            );
        }
        accumulator
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }
}

pub fn total_body_mass(inverse_masses: &[f32]) -> f32 {
    let mut even = 0.0_f32;
    let mut odd = 0.0_f32;
    let mut pairs = inverse_masses.chunks_exact(2);
    for pair in &mut pairs {
        even = 1.0 / pair[0] + even;
        odd = 1.0 / pair[1] + odd;
    }
    let paired = odd + even;
    paired
        + pairs
            .remainder()
            .first()
            .map_or(0.0, |inverse| 1.0 / inverse)
}
