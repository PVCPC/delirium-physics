mod material10;
mod material11;
use super::math::{V, dot, normalize_or, scale, sub};
use crate::physics::skeleton_body::SkeletonBody;

#[derive(Clone, Copy)]
pub struct ContactResponseInput {
    pub com_velocity_608: V,
    pub material10_normal: Option<V>,
    pub material11_normal: Option<V>,
    pub effective_axis_224: V,
    pub wipeout_control_2824_2828: [f32; 2],
}
#[derive(Clone, Copy, Debug)]
pub struct ContactResponseOutput {
    pub material10_phase: u32,
    pub material10_finished_77: bool,
    pub material11_active_73: bool,
}
pub struct ContactResponse {
    material10: material10::Response,
    material11: material11::Response,
}
impl ContactResponse {
    pub fn new() -> Self {
        Self {
            material10: material10::Response::new(),
            material11: material11::Response::new(),
        }
    }
    pub fn reset(&mut self) {
        self.material10.reset();
        self.material11.reset();
    }
    pub fn update(
        &mut self,
        body: &mut SkeletonBody,
        input: ContactResponseInput,
    ) -> ContactResponseOutput {
        self.material10
            .update(body, input.com_velocity_608, input.material10_normal);
        self.material11.update(body, input);
        ContactResponseOutput {
            material10_phase: self.material10.phase,
            material10_finished_77: self.material10.finished,
            material11_active_73: self.material11.active,
        }
    }
}

fn reject(value: V, direction: V) -> V {
    let normal = normalize_or(direction, [0.0; 4]);
    sub(value, scale(normal, dot(value, normal)))
}
fn reject_positive(value: V, direction: V) -> V {
    let projection = dot(value, normalize_or(direction, [0.0; 4]));
    if projection > 0.0 {
        sub(value, scale(direction, projection))
    } else {
        value
    }
}
fn apply_velocity_delta(body: &mut SkeletonBody, delta: V) {
    for part in &mut body.bodies_mut()[1..24] {
        part.rates.linear_velocity.x += delta[0];
        part.rates.linear_velocity.y += delta[1];
        part.rates.linear_velocity.z += delta[2];
    }
}
