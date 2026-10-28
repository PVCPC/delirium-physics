use super::{SkeletonBodyDefinition, SkeletonPhysicalRecord, PART_COUNT, ANIMATION_PART_COUNT};
use crate::physics::{
    assembly::BodySnapshot,
    board_pose::{PartPose, PoseMatrix, part_transform, set_part_transform, orthonormalize_part_basis},
    board_runtime::{BoardMotion, initialized_body},
    rigid_body::{RetailBodyRates, RetailInertiaDynamics, RetailLocalMassFrame, RetailSimulationStep},
    skeleton_animation_record::{AnimationPartTransform, IDENTITY, compose_affine},
    skeleton_root::inverse_rigid,
};

pub struct SkeletonBody {
    pub definition: SkeletonBodyDefinition,
    pub record: SkeletonPhysicalRecord,
    pub animation_to_world: AnimationPartTransform,
    bodies: [BodySnapshot; PART_COUNT],
}
impl SkeletonBody {
    pub fn new(definition: SkeletonBodyDefinition,
        authored: &[AnimationPartTransform; ANIMATION_PART_COUNT],
        spawn: AnimationPartTransform, simulation: RetailSimulationStep) -> Self {
        let authored = authored.map(|frame| floats(orthonormalize_part_basis(words(frame))));
        let animation_to_world = compose_affine(&spawn, &inverse_rigid(&authored[0]));
        let bodies = std::array::from_fn(|i| {
            let desired = if i < ANIMATION_PART_COUNT {
                compose_affine(&animation_to_world, &authored[i])
            } else { spawn };
            let mass = definition.parts[i].animated;
            let mut inertia = mass.dynamics;
            if (1..ANIMATION_PART_COUNT).contains(&i) {
                inertia.inverse_mass = definition.parts[i].inverse_mass_animated;
            }
            let mut part = PartPose { transform: words(desired),
                local_mass_frame: Some(mass_words(mass.local_mass_frame)),
                body: Some([0;44]), inertia: Some(inertia_words(inertia)) };
            set_part_transform(&mut part, words(desired));
            initialized_body(&part, inertia, simulation, BoardMotion::Active)
        });
        let mut result = Self { definition, bodies, animation_to_world,
            record: SkeletonPhysicalRecord::default() };
        let pose = result.part_transforms();
        result.record.update(&pose, pose[0], &result.definition.animation_masses.fractional);
        result.record.update(&pose, pose[0], &result.definition.animation_masses.fractional);
        result
    }
    pub fn bodies(&self) -> &[BodySnapshot; PART_COUNT] { &self.bodies }
    pub fn bodies_mut(&mut self) -> &mut [BodySnapshot; PART_COUNT] { &mut self.bodies }
    pub fn set_part_transform(&mut self, part: usize, frame: AnimationPartTransform) {
        use crate::{math::{Basis3, Vector3}, physics::rigid_body::RetailQuaternion};
        let body = &mut self.bodies[part];
        let mut pose = PartPose {
            transform: words(frame),
            local_mass_frame: Some(mass_words(self.definition.parts[part].animated.local_mass_frame)),
            body: Some(body_words(body.rates)),
            inertia: Some(inertia_words(body.inertia)),
        };
        set_part_transform(&mut pose, words(frame));
        let packed = pose.body.unwrap();
        let f = |i| f32::from_bits(packed[i]);
        body.rates.orientation = RetailQuaternion { x:f(0), y:f(1), z:f(2), w:f(3) };
        body.rates.position = Vector3::new(f(4),f(5),f(6));
        body.rates.basis = Basis3 { columns: [
            [f(16),f(17),f(18)], [f(20),f(21),f(22)], [f(24),f(25),f(26)]] };
        body.rates.world_inverse_inertia = Basis3 { columns: [
            [f(28),f(29),f(30)], [f(29),f(33),f(34)], [f(30),f(34),f(32)]] };
        self.record.pose[part] = frame;
    }
    pub fn part_transforms(&self) -> [AnimationPartTransform; PART_COUNT] {
        std::array::from_fn(|i| {
            let part = PartPose { transform: words(IDENTITY),
                local_mass_frame: Some(mass_words(self.definition.parts[i].animated.local_mass_frame)),
                body: Some(body_words(self.bodies[i].rates)), inertia: None };
            floats(part_transform(&part))
        })
    }
    pub fn publish_physical_record(&mut self, board: AnimationPartTransform) {
        let pose = self.part_transforms();
        self.record.update(&pose, board, &self.definition.animation_masses.fractional);
    }
    pub fn apply_part_displacement(&mut self, part: usize, displacement: [f32;4]) {
        use crate::physics::native_arithmetic;
        let step = f32::from_bits(0x3C88_8889);
        let mut inverse_step = native_arithmetic::reciprocal_estimate(step);
        for _ in 0..2 {
            inverse_step = inverse_step.mul_add((-inverse_step).mul_add(step, 1.0), inverse_step);
        }
        let body = &mut self.bodies[part];
        let value = displacement.map(|d| (d * inverse_step) * body.inertia.inverse_mass);
        body.rates.force_acceleration.x += value[0];
        body.rates.force_acceleration.y += value[1];
        body.rates.force_acceleration.z += value[2];
        body.rates.cool_down = 0;
    }
}

fn words(frame: AnimationPartTransform) -> PoseMatrix {
    std::array::from_fn(|i| frame[i/4][i%4].to_bits())
}
fn floats(frame: PoseMatrix) -> AnimationPartTransform {
    std::array::from_fn(|i| std::array::from_fn(|j| f32::from_bits(frame[i*4+j])))
}
fn mass_words(frame: RetailLocalMassFrame) -> PoseMatrix {
    let mut m = IDENTITY;
    for (column, axis) in frame.basis.columns.iter().enumerate() {
        m[column][..3].copy_from_slice(axis);
    }
    m[3] = [frame.translation.x,frame.translation.y,frame.translation.z,0.0];
    words(m)
}
fn inertia_words(d: RetailInertiaDynamics) -> [u32;10] {
    [d.inverse_tensor.x,d.inverse_tensor.y,d.inverse_tensor.z,0.0,d.inverse_mass,
        d.spherical,d.maximum_linear_velocity,d.maximum_angular_velocity,
        d.linear_drag,d.angular_drag].map(f32::to_bits)
}
fn body_words(rates: RetailBodyRates) -> [u32;44] {
    let mut w = [0;44];
    let q = rates.orientation;
    w[..4].copy_from_slice(&[q.x,q.y,q.z,q.w].map(f32::to_bits));
    w[4..7].copy_from_slice(&[rates.position.x,rates.position.y,rates.position.z].map(f32::to_bits));
    for (i,axis) in rates.basis.columns.iter().enumerate() {
        w[16+i*4..19+i*4].copy_from_slice(&axis.map(f32::to_bits));
    }
    w
}
