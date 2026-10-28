use crate::physics::rigid_body::RetailReactionCorrections;

pub mod tu3 {
    pub const SIMULATION_SOLVE: u32 = 0x82AE_27D0;
    pub const JOINT_SOLVE_LOOP: u32 = 0x82AE_2BC8;
    pub const DRIVE_SOLVE_LOOP: u32 = 0x82AE_2E78;
    pub const JOINT_BATCH_BUILD: u32 = 0x82AE_39D0;
    pub const JOINT_JACOBIAN_BUILD: u32 = 0x82AE_3BC8;
    pub const JACOBIAN_BYTES: usize = 384;
    pub const REACTION_BYTES: usize = 64;
}

const JACOBIAN_VECTOR_COUNT: usize = tu3::JACOBIAN_BYTES / 16;
const REACTION_VECTOR_COUNT: usize = tu3::REACTION_BYTES / 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct RetailJointJacobian {
    pub words: [u32; JACOBIAN_VECTOR_COUNT * 4],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct RetailJointReactionBlock {
    pub words: [u32; REACTION_VECTOR_COUNT * 4],
}

impl RetailJointReactionBlock {
    pub fn from_reactions(reactions: RetailReactionCorrections) -> Self {
        let mut words = [0; REACTION_VECTOR_COUNT * 4];
        words[0] = reactions.linear_displacement.x.to_bits();
        words[1] = reactions.linear_displacement.y.to_bits();
        words[2] = reactions.linear_displacement.z.to_bits();
        words[8] = reactions.angular_displacement.x.to_bits();
        words[9] = reactions.angular_displacement.y.to_bits();
        words[10] = reactions.angular_displacement.z.to_bits();
        Self { words }
    }

    pub fn write_velocity_reactions(self, reactions: &mut RetailReactionCorrections) {
        reactions.linear_displacement.x = f32::from_bits(self.words[0]);
        reactions.linear_displacement.y = f32::from_bits(self.words[1]);
        reactions.linear_displacement.z = f32::from_bits(self.words[2]);
        reactions.angular_displacement.x = f32::from_bits(self.words[8]);
        reactions.angular_displacement.y = f32::from_bits(self.words[9]);
        reactions.angular_displacement.z = f32::from_bits(self.words[10]);
    }
}

pub fn solve_joint_iteration(
    jacobian: &mut RetailJointJacobian,
    reaction_a: &mut RetailJointReactionBlock,
    reaction_b: &mut RetailJointReactionBlock,
) {
    super::solver::solve_joint_record(
        &mut jacobian.words,
        &mut reaction_a.words,
        &mut reaction_b.words,
    );
}

