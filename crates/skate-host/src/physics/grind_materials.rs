use super::{player_input::grind::MaterialMode, settings::PhysicsSettings};
use skate_core::physics::{board_runtime::BoardRuntime, contact::RetailContactMaterial};

pub(crate) struct GrindMaterials {
    standard: [RetailContactMaterial; 3],
}

impl GrindMaterials {
    pub fn new(settings: &PhysicsSettings) -> Self {
        Self {
            standard: [
                settings.standard_wheel_material,
                settings.truck_material,
                settings.deck_material,
            ],
        }
    }

    pub fn apply(
        &self,
        mode: MaterialMode,
        board: &mut BoardRuntime,
        settings: &mut PhysicsSettings,
    ) {
        let materials = match mode {
            MaterialMode::Unchanged => return,
            MaterialMode::Standard => self.standard,
            MaterialMode::Grind => {
                [RetailContactMaterial {
                    static_friction: 0.,
                    dynamic_friction: 0.,
                    restitution: 0.,
                }; 3]
            }
        };
        board.set_collision_group(4);
        [
            settings.wheel_material,
            settings.truck_material,
            settings.deck_material,
        ] = materials;
    }
}
