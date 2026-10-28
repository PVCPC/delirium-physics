use crate::math::Vector3;

pub mod tu3 {
    pub const COMBINE_PHYSICAL_MATERIALS: u32 = 0x8276_3078;
    pub const GENERATE_CONTACT_A: u32 = 0x8277_A828;
    pub const GENERATE_CONTACT_A_END: u32 = 0x8277_AA88;
    pub const GENERATE_CONTACT_B: u32 = 0x8277_BFAC;
    pub const GENERATE_CONTACT_B_END: u32 = 0x8277_C21C;
    pub const CONTACT_BATCH_BUILD: u32 = 0x82AE_10C8;
}

pub const CONTACT_TANGENT_MINIMUM_SQUARED: f32 = f32::from_bits(0x0080_0000);
pub const CONTACT_FALLBACK_HALF: f32 = f32::from_bits(0x3F00_0000);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailContactBodyState {
    pub contact_body_id: u32,
    pub center_of_mass: Vector3,
    pub reaction_id: u32,
    pub inverse_inertia_full: Vector3,
    pub inverse_mass: f32,
    pub inverse_inertia_split: Vector3,
    pub state: u32,
    pub force_acceleration: Vector3,
    pub kinetic_energy: f32,
    pub torque_acceleration: Vector3,
    pub cool_down: u32,
    pub linear_velocity: Vector3,
    pub angular_velocity: Vector3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailContactInput {
    pub position_on_a: Vector3,
    pub position_on_b: Vector3,
    pub normal: Vector3,
    pub restitution: f32,
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub tag: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailContactMaterial {
    pub static_friction: f32,
    pub dynamic_friction: f32,
    pub restitution: f32,
}

pub fn combine_contact_materials(
    material_a: RetailContactMaterial,
    material_b: RetailContactMaterial,
) -> RetailContactMaterial {
    let static_friction = if material_a.static_friction > material_b.static_friction {
        material_a.static_friction
    } else {
        material_b.static_friction
    };
    let dynamic_friction = if material_a.dynamic_friction > material_b.dynamic_friction {
        material_a.dynamic_friction
    } else {
        material_b.dynamic_friction
    };
    let restitution = if material_a.restitution < material_b.restitution {
        material_a.restitution
    } else {
        material_b.restitution
    };
    RetailContactMaterial {
        static_friction: material_scalar_store(static_friction),
        dynamic_friction: material_scalar_store(dynamic_friction),
        restitution: material_scalar_store(restitution),
    }
}

fn material_scalar_store(value: f32) -> f32 {
    if value.is_nan() {
        f32::from_bits(value.to_bits() | 0x0040_0000)
    } else {
        value
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailContact {
    pub position_on_a: Vector3,
    pub body_a_id: u32,
    pub position_on_b: Vector3,
    pub body_b_id: u32,
    pub normal: Vector3,
    pub restitution: f32,
    pub tangent_0: Vector3,
    pub static_friction: f32,
    pub tangent_1: Vector3,
    pub dynamic_friction: f32,
    pub relative_velocity: Vector3,
    pub tag: u32,
    pub body_a_workspace: RetailContactWorkspace,
    pub body_b_workspace: RetailContactWorkspace,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RetailContactWorkspace {
    pub center_of_mass: Vector3,
    pub reaction_id: u32,
    pub inverse_inertia_full: Vector3,
    pub inverse_mass: f32,
    pub inverse_inertia_split: Vector3,
    pub state: u32,
    pub force_acceleration: Vector3,
    pub kinetic_energy: f32,
    pub torque_acceleration: Vector3,
    pub cool_down: u32,
}

impl From<RetailContactBodyState> for RetailContactWorkspace {
    fn from(body: RetailContactBodyState) -> Self {
        Self {
            center_of_mass: body.center_of_mass,
            reaction_id: body.reaction_id,
            inverse_inertia_full: body.inverse_inertia_full,
            inverse_mass: body.inverse_mass,
            inverse_inertia_split: body.inverse_inertia_split,
            state: body.state,
            force_acceleration: body.force_acceleration,
            kinetic_energy: body.kinetic_energy,
            torque_acceleration: body.torque_acceleration,
            cool_down: body.cool_down,
        }
    }
}

pub fn generate_contact(
    input: RetailContactInput,
    body_a: RetailContactBodyState,
    body_b: RetailContactBodyState,
) -> RetailContact {
    let arm_a = sub(input.position_on_a, body_a.center_of_mass);
    let arm_b = sub(input.position_on_b, body_b.center_of_mass);
    let point_velocity_a = point_velocity(body_a.linear_velocity, body_a.angular_velocity, arm_a);
    let point_velocity_b = point_velocity(body_b.linear_velocity, body_b.angular_velocity, arm_b);
    let relative_velocity = sub(point_velocity_b, point_velocity_a);

    let velocity_tangent = cross(relative_velocity, input.normal);
    let velocity_tangent_squared = length_squared(velocity_tangent);
    let (unnormalized_tangent, tangent_squared) =
        if velocity_tangent_squared >= CONTACT_TANGENT_MINIMUM_SQUARED {
            (velocity_tangent, velocity_tangent_squared)
        } else {
            fallback_tangent(input.normal)
        };

    let inverse_length = super::reciprocal_sqrt::estimate(tangent_squared);
    let tangent_0 = scale(unnormalized_tangent, inverse_length);
    let n = input.normal;
    let t = unnormalized_tangent;
    let tangent_1 = scale(
        Vector3::new(
            (-t.y).mul_add(n.z, t.z.mul_add(n.y, 0.0)),
            (-t.z).mul_add(n.x, t.x.mul_add(n.z, 0.0)),
            (-t.x).mul_add(n.y, t.y.mul_add(n.x, 0.0)),
        ),
        inverse_length,
    );

    RetailContact {
        position_on_a: input.position_on_a,
        body_a_id: body_a.contact_body_id,
        position_on_b: input.position_on_b,
        body_b_id: body_b.contact_body_id,
        normal: input.normal,
        restitution: input.restitution,
        tangent_0,
        static_friction: input.static_friction,
        tangent_1,
        dynamic_friction: input.dynamic_friction,
        relative_velocity,
        tag: input.tag,
        body_a_workspace: body_a.into(),
        body_b_workspace: body_b.into(),
    }
}

fn fallback_tangent(normal: Vector3) -> (Vector3, f32) {
    let candidate_x = Vector3::new(
        (-normal.x).mul_add(normal.x, 1.0),
        (-normal.y).mul_add(normal.x, 0.0),
        (-normal.z).mul_add(normal.x, 0.0),
    );
    let candidate_z = Vector3::new(
        (-normal.x).mul_add(normal.z, 0.0),
        (-normal.y).mul_add(normal.z, 0.0),
        (-normal.z).mul_add(normal.z, 1.0),
    );
    if (-normal.x).mul_add(normal.x, CONTACT_FALLBACK_HALF) >= -0.0 {
        (candidate_x, candidate_x.x)
    } else {
        (
            Vector3::new(-candidate_z.x, -candidate_z.y, -candidate_z.z),
            candidate_z.z,
        )
    }
}

fn point_velocity(linear: Vector3, omega: Vector3, arm: Vector3) -> Vector3 {
    Vector3::new(
        arm.z.mul_add(omega.y, (-arm.y).mul_add(omega.z, linear.x)),
        arm.x.mul_add(omega.z, (-arm.z).mul_add(omega.x, linear.y)),
        arm.y.mul_add(omega.x, (-arm.x).mul_add(omega.y, linear.z)),
    )
}

fn sub(left: Vector3, right: Vector3) -> Vector3 {
    Vector3::new(left.x - right.x, left.y - right.y, left.z - right.z)
}

fn scale(vector: Vector3, scalar: f32) -> Vector3 {
    Vector3::new(
        vector.x.mul_add(scalar, 0.0),
        vector.y.mul_add(scalar, 0.0),
        vector.z.mul_add(scalar, 0.0),
    )
}

fn cross(left: Vector3, right: Vector3) -> Vector3 {
    Vector3::new(
        left.y.mul_add(right.z, (-left.z).mul_add(right.y, 0.0)),
        left.z.mul_add(right.x, (-left.x).mul_add(right.z, 0.0)),
        left.x.mul_add(right.y, (-left.y).mul_add(right.x, 0.0)),
    )
}

fn dot(left: Vector3, right: Vector3) -> f32 {
    left.z.mul_add(
        right.z,
        left.y.mul_add(right.y, left.x.mul_add(right.x, 0.0)),
    )
}

fn length_squared(vector: Vector3) -> f32 {
    dot(vector, vector)
}

