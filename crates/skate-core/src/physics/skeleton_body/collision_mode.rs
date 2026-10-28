use super::{ANIMATION_PART_COUNT, PART_COUNT, SkeletonBody, SkeletonCollisionFeedback};
use crate::physics::{contact::RetailContactMaterial, rigid_body::world_inverse_inertia};

#[derive(Clone, Copy, Debug)]
pub struct SkeletonCollisionSettings {
    pub enabled: bool,
    pub normal_material: RetailContactMaterial,
    pub compliant: [bool; ANIMATION_PART_COUNT],
    pub priority: [f32; ANIMATION_PART_COUNT],
    pub effect_time: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SkeletonPartCollision {
    pub enabled: bool,
    pub volume_group: u32,
    pub part_group: u32,
    pub material: RetailContactMaterial,
}

pub struct SkeletonCollisionMode {
    pub parts: [SkeletonPartCollision; PART_COUNT],
    pub disable_count: [u32; ANIMATION_PART_COUNT],
    pub pending_reenable: bool,
    pub assembly_group: u32,
    pub partial_ragdoll: bool,
    pub is_ragdoll: bool,
    pub settings: SkeletonCollisionSettings,
    pub self_culling: [[bool; PART_COUNT]; PART_COUNT],
    normal_self_culling: [[bool; PART_COUNT]; PART_COUNT],
}

impl SkeletonCollisionMode {
    pub fn disable_handplant_contacts(&mut self, frames: u32) {
        self.pending_reenable = true;
        for part in [7, 3, 8, 4, 1] {
            self.disable_count[part] = frames;
            self.parts[part].enabled = false;
        }
    }

    pub fn reset_body_state(&mut self, feedback: &mut SkeletonCollisionFeedback) {
        self.is_ragdoll = false;
        feedback.reset();
        self.disable_count = [0; ANIMATION_PART_COUNT];
        self.pending_reenable = false;
    }

    pub fn new_normal(settings: SkeletonCollisionSettings, cull_all_self_pairs: bool) -> Self {
        let zero = RetailContactMaterial {
            static_friction: 0.0,
            dynamic_friction: 0.0,
            restitution: 0.0,
        };
        let mut result = Self {
            parts: std::array::from_fn(|part| SkeletonPartCollision {
                enabled: false,
                volume_group: if part == 0 { 4 } else { 0 },
                part_group: 5,
                material: if (1..24).contains(&part) {
                    settings.normal_material
                } else {
                    zero
                },
            }),
            disable_count: [0; ANIMATION_PART_COUNT],
            pending_reenable: false,
            assembly_group: 5,
            partial_ragdoll: false,
            is_ragdoll: false,
            settings,
            self_culling: [[true; PART_COUNT]; PART_COUNT],
            normal_self_culling: [[true; PART_COUNT]; PART_COUNT],
        };
        if !cull_all_self_pairs {
            for (a, b) in [(14, 4), (14, 8), (23, 4), (23, 8), (4, 18), (8, 22)] {
                result.self_culling[a][b] = false;
                result.self_culling[b][a] = false;
            }
        }
        if settings.enabled {
            result.normal_collision();
        } else {
            result.disable_all(true);
        }
        result.normal_self_culling = result.self_culling;
        result
    }

    pub fn select_driven(&mut self, mode: u32) -> Result<(), &'static str> {
        match mode {
            3 => {
                self.select_biped();
                return Ok(());
            }
            5 | 6 => self.normal_collision(),
            1 => {
                self.enable_eligible_bones();
                self.set_group(3);
            }
            _ => return Err("Skeleton collision selector requires its non-driven physical setup"),
        }
        self.partial_ragdoll = false;
        self.disable_root();
        self.parts[24].enabled = false;
        self.parts[25].enabled = false;
        Ok(())
    }

    pub fn normal_collision(&mut self) {
        self.set_group(5);
        self.disable_root();
        self.enable_eligible_bones();
    }

    pub fn disable_all(&mut self, clear_counts: bool) {
        self.disable_root();
        self.parts[24].enabled = false;
        self.parts[25].enabled = false;
        for part in 1..24 {
            self.parts[part].enabled = false;
            if clear_counts {
                self.disable_count[part] = 0;
            }
        }
    }

    pub fn enable_bone(&mut self, part: usize) {
        if self.disable_count[part] == 0 {
            self.parts[part].enabled = true;
            self.parts[part].volume_group = 0;
        }
        self.parts[part].part_group = 5;
    }

    pub fn finish_contact_frame(&mut self) {
        if !self.pending_reenable {
            return;
        }
        let mut remaining = false;
        for part in 0..ANIMATION_PART_COUNT {
            if (self.disable_count[part] as i32) > 0 {
                self.disable_count[part] -= 1;
                let enabled = self.disable_count[part] == 0;
                self.parts[part].enabled = enabled;
                remaining |= !enabled;
            }
        }
        self.pending_reenable = remaining;
    }

    pub fn normal_bone(&mut self, part: usize, has_collision: bool) {
        if has_collision {
            if self.disable_count[part] == 0 {
                self.parts[part].enabled = true;
                self.parts[part].volume_group = 0;
            }
        } else {
            self.parts[part].enabled = false;
            self.parts[part].volume_group = 4;
            self.disable_count[part] = 0;
        }
    }

    pub fn restore_normal_properties(&mut self, skeleton: &mut SkeletonBody) {
        self.self_culling = self.normal_self_culling;
        self.set_group(5);
        for part in 0..PART_COUNT {
            let animated = skeleton.definition.parts[part];
            let body = &mut skeleton.bodies_mut()[part];
            body.inertia.linear_drag = 0.0;
            body.inertia.angular_drag = 0.0;
            if (1..24).contains(&part) {
                body.inertia.inverse_mass = animated.inverse_mass_animated;
                body.inertia.inverse_tensor = animated.animated.dynamics.inverse_tensor;
                let tensor = body.inertia.inverse_tensor;
                let mut minimum = tensor.x;
                if minimum >= tensor.y {
                    minimum = tensor.y;
                }
                if minimum >= tensor.z {
                    minimum = tensor.z;
                }
                body.inertia.spherical = 1.0 / minimum;
                body.rates.world_inverse_inertia = world_inverse_inertia(body.rates.basis, tensor);
                self.parts[part].material = self.settings.normal_material;
            }
        }
        if self.settings.enabled {
            self.normal_collision();
        } else {
            self.disable_all(true);
        }
        self.is_ragdoll = false;
    }

    pub fn apply_ragdoll_properties(
        &mut self,
        skeleton: &mut SkeletonBody,
        inverse_mass: bool,
        inverse_inertia: bool,
        drag: [f32; 2],
        materials: [RetailContactMaterial; 2],
    ) {
        const CULLED: [u32; PART_COUNT] = [
            0x03FFFFFF, 0x03FFF447, 0x03FFFC47, 0x039B8479, 0x03BB8479, 0x039F8479, 0x03FFFFFF,
            0x03B987C1, 0x03BB87C1, 0x03F987C1, 0x03FFFFFF, 0x03FFFC45, 0x03FFFC47, 0x03FFFC47,
            0x03FFFC47, 0x03FFFFFF, 0x038FFFFF, 0x038FFD7F, 0x03CFFC67, 0x03FFFFFF, 0x03F8FFFF,
            0x03F8FFD7, 0x03FCFE47, 0x03FFFFFF, 0x03FFFFFF, 0x03FFFFFF,
        ];
        if self
            .normal_self_culling
            .iter()
            .flatten()
            .all(|culled| *culled)
        {
            self.self_culling = [[true; PART_COUNT]; PART_COUNT];
        } else {
            self.self_culling =
                std::array::from_fn(|a| std::array::from_fn(|b| CULLED[a] & (1 << b) != 0));
        }
        self.set_group(6);
        for body in skeleton.bodies_mut() {
            body.inertia.linear_drag = drag[0];
            body.inertia.angular_drag = drag[1];
        }
        for part in 1..24 {
            let authored = skeleton.definition.parts[part];
            let body = &mut skeleton.bodies_mut()[part];
            self.disable_count[part] = 0;
            self.parts[part].enabled = true;
            self.pending_reenable = false;
            self.parts[part].material = materials[usize::from(matches!(part, 1 | 3 | 7))];
            if inverse_mass {
                body.inertia.inverse_mass = authored.inverse_mass_ragdoll;
            }
            if inverse_inertia {
                let tensor = authored.ragdoll.dynamics.inverse_tensor;
                body.inertia.inverse_tensor = tensor;
                let mut minimum = tensor.x;
                if minimum >= tensor.y {
                    minimum = tensor.y;
                }
                if minimum >= tensor.z {
                    minimum = tensor.z;
                }
                body.inertia.spherical = 1.0 / minimum;
            }
            body.rates.world_inverse_inertia =
                world_inverse_inertia(body.rates.basis, body.inertia.inverse_tensor);
        }
        self.is_ragdoll = true;
    }

    pub fn finish_ragdoll_request(&mut self, mode: u32) {
        match mode {
            7 | 10 => {
                self.partial_ragdoll = false;
                self.disable_root();
                self.parts[24].enabled = false;
                self.parts[25].enabled = false;
            }
            8 => {
                for part in 1..24 {
                    self.parts[part].material = RetailContactMaterial {
                        static_friction: 0.5,
                        dynamic_friction: 0.3,
                        restitution: 0.4,
                    };
                }
            }
            9 => self.set_group(7),
            _ => unreachable!("Only original Wipeout requests8..11 enter this adapter"),
        }
    }

    fn set_group(&mut self, group: u32) {
        self.assembly_group = group;
        for part in &mut self.parts {
            part.part_group = group;
        }
    }
    fn select_biped(&mut self) {
        self.set_group(6);
        self.disable_all(false);
        for part in [17, 21, 15, 19, 16, 20] {
            self.enable_bone(part);
            self.parts[part].part_group = 17;
        }
        for part in [5, 4, 3, 9, 8, 7, 2, 1] {
            self.enable_bone(part);
        }
        for part in [6, 10, 11, 12, 13, 14, 23] {
            self.enable_bone(part);
            self.parts[part].part_group = 18;
        }
        self.partial_ragdoll = true;
        for part in [24, 25] {
            self.parts[part].part_group = 20;
            self.parts[part].enabled = true;
        }
    }
    fn disable_root(&mut self) {
        self.parts[0].volume_group = 4;
        self.parts[0].enabled = false;
        self.disable_count[0] = 0;
    }
    fn enable_eligible_bones(&mut self) {
        for part in 1..24 {
            if self.disable_count[part] == 0 {
                self.parts[part].enabled = true;
                self.parts[part].volume_group = 0;
            }
        }
    }
}
