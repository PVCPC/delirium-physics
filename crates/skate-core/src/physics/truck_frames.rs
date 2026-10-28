use super::drive_frames::{RetailAffineTransform, RetailDriveFrames, set_drive_frames_2};
use crate::{math::Basis3, trigonometry};

pub fn steering_drive_frames(
    base: [RetailAffineTransform; 2],
    targets: [f32; 2],
) -> [RetailDriveFrames; 2] {
    let [front, back] = steering_truck_transforms(base, targets);
    let transformed = [back, front];
    transformed.map(|target| set_drive_frames_2(RetailAffineTransform::IDENTITY, target))
}

pub fn steering_truck_transforms(
    base: [RetailAffineTransform; 2],
    targets: [f32; 2],
) -> [RetailAffineTransform; 2] {
    [
        rotate_truck(base[0], -targets[0]),
        rotate_truck(base[1], targets[1]),
    ]
}

fn rotate_truck(base: RetailAffineTransform, angle: f32) -> RetailAffineTransform {
    let (sin, cos) = trigonometry::sin_cos(angle);
    let rotation = [[1.0, 0.0, 0.0], [0.0, cos, sin], [0.0, -sin, cos]];
    let columns = rotation.map(|column| {
        core::array::from_fn(|lane| {
            let first = column[0] * base.basis.columns[0][lane];
            let second = column[1].mul_add(base.basis.columns[1][lane], first);
            column[2].mul_add(base.basis.columns[2][lane], second)
        })
    });
    RetailAffineTransform {
        basis: Basis3 { columns },
        translation: base.translation,
    }
}

