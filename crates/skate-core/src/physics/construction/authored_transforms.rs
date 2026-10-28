use super::RetailAffineTransform;
use crate::math::{Basis3, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredTransformInputs {
    pub deck_mid_length: f32,
    pub wheel_x_distance: f32,
    pub truck_z_position_front: f32,
    pub truck_z_position_back: f32,
    pub truck_y_position: f32,
}

impl AuthoredTransformInputs {
    pub const STOCK: Self = Self {
        deck_mid_length: f32::from_bits(0x3f17_0a3d),
        wheel_x_distance: f32::from_bits(0x3dc2_8f5c),
        truck_z_position_front: f32::from_bits(0xbd54_fdf4),
        truck_z_position_back: f32::from_bits(0xbd54_fdf4),
        truck_y_position: f32::from_bits(0xbd67_6c8b),
    };
}

pub fn authored_body_transforms(input: AuthoredTransformInputs) -> [RetailAffineTransform; 7] {
    let front_z = input
        .deck_mid_length
        .mul_add(0.5, input.truck_z_position_front);
    let back_z = -input
        .deck_mid_length
        .mul_add(0.5, input.truck_z_position_back);
    let y = input.truck_y_position;
    let x = input.wheel_x_distance;
    let mut transforms = [RetailAffineTransform::IDENTITY; 7];
    for (part, translation) in [
        Vector3::new(-x, y, front_z),
        Vector3::new(x, y, front_z),
        Vector3::new(-x, y, back_z),
        Vector3::new(x, y, back_z),
        Vector3::new(0.0, y, front_z),
        Vector3::new(0.0, y, back_z),
    ]
    .into_iter()
    .enumerate()
    {
        transforms[part].translation = translation;
    }
    let (sin, cos) = crate::trigonometry::sin_cos(f32::from_bits(0x3fc9_0fdb));
    transforms[5].basis = Basis3 {
        columns: [[cos, 0.0, -sin], [0.0, 1.0, 0.0], [sin, 0.0, cos]],
    };
    transforms
}

pub fn authored_body_pose_records(input: AuthoredTransformInputs) -> [[u32; 16]; 7] {
    authored_body_transforms(input).map(|transform| {
        let mut words = [0; 16];
        for (axis, column) in transform.basis.columns.into_iter().enumerate() {
            for (component, value) in column.into_iter().enumerate() {
                words[axis * 4 + component] = value.to_bits();
            }
        }
        words[12] = transform.translation.x.to_bits();
        words[13] = transform.translation.y.to_bits();
        words[14] = transform.translation.z.to_bits();
        words
    })
}
