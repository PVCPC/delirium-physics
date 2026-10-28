use super::{MassShape, PrimitiveMass, primitive_mass};
use crate::math::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelMassSettings {
    pub radius: f32,
    pub mass: f32,
    pub mass_factor: f32,
}

impl WheelMassSettings {
    pub const STOCK: Self = Self {
        radius: f32::from_bits(0x3cfd_f3b6),
        mass: f32::from_bits(0x3da9_fbe7),
        mass_factor: f32::from_bits(0x40a0_0000),
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TruckMassSettings {
    pub wheel_radius: f32,
    pub wheel_x_distance: f32,
    pub radius_scalar: f32,
    pub half_height_scalar: f32,
    pub mass: f32,
    pub mass_factor: f32,
}

impl TruckMassSettings {
    pub const STOCK: Self = Self {
        wheel_radius: f32::from_bits(0x3cfd_f3b6),
        wheel_x_distance: f32::from_bits(0x3dc2_8f5c),
        radius_scalar: f32::from_bits(0x3f09_999a),
        half_height_scalar: f32::from_bits(0x3fac_cccd),
        mass: f32::from_bits(0x3ec7_ae14),
        mass_factor: f32::from_bits(0x40a0_0000),
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartMassInput {
    pub shape: MassShape,
    pub requested_mass: f32,
}

pub fn wheel_mass_input(settings: WheelMassSettings) -> PartMassInput {
    PartMassInput {
        shape: MassShape::Sphere {
            radius: settings.radius,
        },
        requested_mass: settings.mass * settings.mass_factor,
    }
}

pub fn truck_mass_input(settings: TruckMassSettings) -> PartMassInput {
    PartMassInput {
        shape: MassShape::Capsule {
            radius: (settings.wheel_radius * settings.radius_scalar) * 0.5,
            half_length: (settings.wheel_x_distance * settings.half_height_scalar) * 0.5,
        },
        requested_mass: settings.mass * settings.mass_factor,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForwardMassProperties {
    pub primitive: PrimitiveMass,
    pub mass: f32,
    pub principal_moments: Vector3,
}

pub fn forward_mass_properties(input: PartMassInput) -> Option<ForwardMassProperties> {
    let primitive = primitive_mass(input.shape)?;
    let mass = if input.requested_mass < f32::MIN_POSITIVE {
        primitive.volume
    } else {
        input.requested_mass
    };
    let moments = primitive.moments_per_unit_mass;
    Some(ForwardMassProperties {
        primitive,
        mass,
        principal_moments: Vector3::new(moments.x * mass, moments.y * mass, moments.z * mass),
    })
}
