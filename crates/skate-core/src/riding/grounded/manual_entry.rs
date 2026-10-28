use crate::physics::manual::state::{ManualEntryContinuation, ManualState};

#[derive(Clone, Copy, Debug)]
pub struct ManualGroundInput {
    pub balance: f32,
    pub ground_normal: [f32; 4],
    pub flags_2468: u32,
    pub flags_2472: u32,
}

pub trait ManualGroundBodies {
    fn linear_velocity(&mut self, part: usize) -> [f32; 4];
    fn set_linear_velocity(&mut self, part: usize, velocity: [f32; 4]);
}

pub trait ManualGroundProjection {
    type Error;
    fn normal_speed(
        &mut self,
        ground_normal: [f32; 4],
        linear_velocity: [f32; 4],
    ) -> Result<f32, Self::Error>;
}

pub fn enter_ground<B: ManualGroundBodies, P: ManualGroundProjection>(
    state: &mut ManualState,
    previous_category: u32,
    powerslide_exit_scale: f32,
    input: ManualGroundInput,
    bodies: &mut B,
    projection: &mut P,
) -> Result<(), P::Error> {
    if state.enter_ground(previous_category, powerslide_exit_scale)
        == ManualEntryContinuation::RemoveVelocityIntoGround
    {
        remove_velocity_into_ground(input, bodies, projection)?;
    }
    Ok(())
}

pub fn remove_velocity_into_ground<B: ManualGroundBodies, P: ManualGroundProjection>(
    input: ManualGroundInput,
    bodies: &mut B,
    projection: &mut P,
) -> Result<(), P::Error> {
    if input.balance == 0.0 {
        return Ok(());
    }
    remove_part(6, input.ground_normal, bodies, projection)?;

    let reversed = input.flags_2468 & (1 << 20) != 0;
    let contact26 = input.flags_2472 & (1 << 26) != 0;
    let contact27 = input.flags_2472 & (1 << 27) != 0;
    let (first_group, second_group) = if reversed {
        (contact26, contact27)
    } else {
        (contact27, contact26)
    };
    if first_group {
        for part in [0, 1, 4] {
            remove_part(part, input.ground_normal, bodies, projection)?;
        }
    }
    if second_group {
        for part in [2, 3, 5] {
            remove_part(part, input.ground_normal, bodies, projection)?;
        }
    }
    Ok(())
}

fn remove_part<B: ManualGroundBodies, P: ManualGroundProjection>(
    part: usize,
    ground_normal: [f32; 4],
    bodies: &mut B,
    projection: &mut P,
) -> Result<(), P::Error> {
    let velocity = bodies.linear_velocity(part);
    let normal_speed = projection.normal_speed(ground_normal, velocity)?;
    let mut remaining = velocity;
    for axis in 0..3 {
        let component = ground_normal[axis] * normal_speed;
        remaining[axis] = velocity[axis] - component;
    }
    bodies.set_linear_velocity(part, remaining);
    Ok(())
}
