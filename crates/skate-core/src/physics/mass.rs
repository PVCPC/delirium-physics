use super::{
    native_arithmetic,
    rigid_body::{RetailBodyMassProperties, RetailInertiaDynamics, RetailLocalMassFrame},
};
use crate::math::{Basis3, Vector3};

#[path = "construction/primitive_mass.rs"]
mod primitive_mass;
pub use primitive_mass::{MassShape, PrimitiveMass, primitive_mass};
#[path = "construction/board_parts.rs"]
mod board_parts;
pub use board_parts::{
    ForwardMassProperties, PartMassInput, TruckMassSettings, WheelMassSettings,
    forward_mass_properties, truck_mass_input, wheel_mass_input,
};

#[path = "construction/mass_moments.rs"]
mod mass_moments;
#[path = "construction/principal_axes.rs"]
mod principal_axes;
pub use mass_moments::{AggregateMassProperties, MassMoments};
#[path = "construction/deck_geometry.rs"]
mod deck_geometry;
pub use deck_geometry::{DeckChild, DeckGeometry, DeckGeometrySettings, DeckShape};

pub fn primitive_mass_properties(
    input: PartMassInput,
    maximum_angular_velocity: f32,
    angular_drag: f32,
) -> Option<RetailBodyMassProperties> {
    let primitive = primitive_mass(input.shape)?;
    let mass = if input.requested_mass < f32::MIN_POSITIVE {
        primitive.volume
    } else {
        input.requested_mass
    };
    let inertia = primitive.moments_per_unit_mass;
    Some(finalize_principal_mass(
        RetailLocalMassFrame::IDENTITY,
        mass,
        Vector3::new(inertia.x * mass, inertia.y * mass, inertia.z * mass),
        maximum_angular_velocity,
        angular_drag,
    ))
}

pub fn aggregate_mass_properties(
    mut moments: MassMoments,
    requested_mass: f32,
    maximum_angular_velocity: f32,
    angular_drag: f32,
) -> RetailBodyMassProperties {
    let properties = moments.principal_properties();
    let mass = if requested_mass < f32::MIN_POSITIVE {
        properties.volume
    } else {
        requested_mass
    };
    let inertia = properties.moments_per_unit_mass;
    finalize_principal_mass(
        inverse_mass_frame(properties.local_mass_frame, inertia),
        mass,
        Vector3::new(inertia.x * mass, inertia.y * mass, inertia.z * mass),
        maximum_angular_velocity,
        angular_drag,
    )
}

fn inverse_mass_frame(forward: RetailLocalMassFrame, inertia: Vector3) -> RetailLocalMassFrame {
    let center = forward.translation;
    let center_squared = native_arithmetic::dot3(
        [center.x, center.y, center.z, 0.0],
        [center.x, center.y, center.z, 0.0],
    );
    let minimum_offset_squared =
        ((inertia.y + inertia.z) + inertia.x) * f32::from_bits(0x3586_37BE);
    let translated = !(center_squared < minimum_offset_squared);
    let rotated = forward
        .basis
        .columns
        .iter()
        .enumerate()
        .any(|(column, axis)| {
            axis.iter().enumerate().any(|(row, value)| {
                let identity = if column == row { 1.0 } else { 0.0 };
                (*value - identity).abs() > f32::from_bits(0x3A83_126F)
            })
        });
    if !translated && !rotated {
        return RetailLocalMassFrame::IDENTITY;
    }
    let columns = core::array::from_fn::<_, 3, _>(|column| {
        core::array::from_fn(|row| forward.basis.columns[row][column])
    });
    let negative_center = [0.0 - center.x, 0.0 - center.y, 0.0 - center.z];
    let translation = core::array::from_fn::<_, 3, _>(|row| {
        negative_center[0].mul_add(
            columns[0][row],
            negative_center[1].mul_add(columns[1][row], negative_center[2] * columns[2][row]),
        )
    });
    RetailLocalMassFrame {
        basis: Basis3 { columns },
        translation: Vector3::new(translation[0], translation[1], translation[2]),
    }
}

pub const RETAIL_UNBOUNDED_VELOCITY: f32 = f32::from_bits(0x7F7F_FFFF);
pub const RETAIL_WHEEL_MAXIMUM_ANGULAR_VELOCITY: f32 = f32::from_bits(0x476A_5FFF);

pub fn retail_wheel_mass_properties() -> RetailBodyMassProperties {
    wheel_mass_properties(WheelMassSettings::STOCK)
}

pub fn wheel_mass_properties(settings: WheelMassSettings) -> RetailBodyMassProperties {
    let forward = forward_mass_properties(wheel_mass_input(settings))
        .expect("the stock wheel is a supported sphere");
    finalize_principal_mass(
        RetailLocalMassFrame::IDENTITY,
        forward.mass,
        forward.principal_moments,
        RETAIL_WHEEL_MAXIMUM_ANGULAR_VELOCITY,
        0.0,
    )
}

pub fn retail_truck_mass_properties() -> RetailBodyMassProperties {
    truck_mass_properties(TruckMassSettings::STOCK)
}

pub fn truck_mass_properties(settings: TruckMassSettings) -> RetailBodyMassProperties {
    let forward = forward_mass_properties(truck_mass_input(settings))
        .expect("the stock truck is a supported capsule");
    finalize_principal_mass(
        RetailLocalMassFrame::IDENTITY,
        forward.mass,
        forward.principal_moments,
        RETAIL_UNBOUNDED_VELOCITY,
        0.0,
    )
}

pub fn deck_mass_properties(
    geometry: &DeckGeometry,
    mass: f32,
    angular_drag: f32,
) -> RetailBodyMassProperties {
    let mut properties = aggregate_mass_properties(
        geometry.mass_moments(),
        mass,
        RETAIL_UNBOUNDED_VELOCITY,
        angular_drag,
    );
    properties.local_mass_frame = RetailLocalMassFrame::IDENTITY;
    properties
}

pub fn retail_deck_mass_properties() -> RetailBodyMassProperties {
    let geometry = DeckGeometry::new(DeckGeometrySettings::STOCK);
    deck_mass_properties(
        &geometry,
        6.0,
        f32::from_bits(0x3EE6_6666) * f32::from_bits(0x426F_FFFF),
    )
}

pub fn default_skateboard_mass_properties() -> [RetailBodyMassProperties; 7] {
    [
        retail_wheel_mass_properties(),
        retail_wheel_mass_properties(),
        retail_wheel_mass_properties(),
        retail_wheel_mass_properties(),
        retail_truck_mass_properties(),
        retail_truck_mass_properties(),
        retail_deck_mass_properties(),
    ]
}

fn finalize_principal_mass(
    local_mass_frame: RetailLocalMassFrame,
    mass: f32,
    principal_moments: Vector3,
    maximum_angular_velocity: f32,
    angular_drag: f32,
) -> RetailBodyMassProperties {
    let inverse_tensor = Vector3::new(
        refined_reciprocal(principal_moments.x),
        refined_reciprocal(principal_moments.y),
        refined_reciprocal(principal_moments.z),
    );
    let smallest_xy = if inverse_tensor.x < inverse_tensor.y {
        inverse_tensor.x
    } else {
        inverse_tensor.y
    };
    let smallest_inverse = if smallest_xy < inverse_tensor.z {
        smallest_xy
    } else {
        inverse_tensor.z
    };
    RetailBodyMassProperties {
        local_mass_frame,
        dynamics: RetailInertiaDynamics {
            inverse_tensor,
            inverse_mass: 1.0 / mass,
            spherical: 1.0 / smallest_inverse,
            maximum_linear_velocity: RETAIL_UNBOUNDED_VELOCITY,
            maximum_angular_velocity,
            linear_drag: 0.0,
            angular_drag,
        },
    }
}

fn refined_reciprocal(value: f32) -> f32 {
    let mut estimate = native_arithmetic::reciprocal_estimate(value);
    for _ in 0..2 {
        let residual = (-estimate).mul_add(value, 1.0);
        estimate = estimate.mul_add(residual, estimate);
    }
    estimate
}

