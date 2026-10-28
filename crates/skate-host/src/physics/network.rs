use super::*;
use skate_core::physics::{assembly::BodySnapshot, board_world::BoardWorldVolume, world_contact::ContactPrimitive};

fn xyz(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}
pub(crate) fn bounds(p: ContactPrimitive) -> (Vec3, f32) {
    match p {
        ContactPrimitive::Sphere(s) => (Vec3::from_array(xyz(s.center)), s.radius),
        ContactPrimitive::Capsule {
            center,
            half_length,
            radius,
            ..
        } => (Vec3::from_array(xyz(center)), half_length + radius),
        ContactPrimitive::RoundedBox {
            center,
            half_extents,
            radius,
            ..
        } => (
            Vec3::from_array(xyz(center)),
            Vec3::from_array(xyz(half_extents)).length() + radius,
        ),
        ContactPrimitive::Triangle(t) => {
            let vertices = t.vertices.map(|v| Vec3::from_array(xyz(v)));
            let center = (vertices[0] + vertices[1] + vertices[2]) / 3.;
            (
                center,
                vertices
                    .iter()
                    .map(|v| v.distance(center))
                    .fold(0., f32::max)
                    + t.fatness,
            )
        }
    }
}
#[derive(Default)]
pub(crate) struct Proxies {
    pub bodies: Vec<BodySnapshot>,
    pub volumes: Vec<BoardWorldVolume>,
}
