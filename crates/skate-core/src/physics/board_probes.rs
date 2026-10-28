use super::{
    board_ground::WheelLine,
    board_motion_output::{dot, inverse_length_squared},
};
use crate::math::Vector3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardProbeHit {
    pub point: Vector3,
    pub normal: Vector3,
    pub surface_tag: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardProbeState {
    pub start: Vector3,
    pub point: Vector3,
    pub normal: Vector3,
    pub surface_tag: u32,
    pub hit: bool,
}
impl Default for BoardProbeState {
    fn default() -> Self {
        Self {
            start: Vector3::ZERO,
            point: Vector3::ZERO,
            normal: Vector3::ZERO,
            surface_tag: 0,
            hit: false,
        }
    }
}
impl BoardProbeState {
    pub fn start(&mut self, position: Vector3) {
        self.start = position;
        self.hit = false;
    }
    pub fn publish(&mut self, hit: Option<BoardProbeHit>) {
        self.hit = hit.is_some();
        if let Some(hit) = hit {
            self.point = hit.point;
            self.normal = hit.normal;
            self.surface_tag = hit.surface_tag;
        }
    }
    pub fn disable(&mut self) {
        *self = Self::default();
    }
}

pub const DECK_PROBE_RADIUS: f32 = f32::from_bits(0x3e19_999a);
pub fn deck_probe(position: Vector3) -> WheelLine {
    WheelLine {
        start: position,
        end: Vector3::new(
            position.x,
            position.y + f32::from_bits(0xc2c8_0000),
            position.z,
        ),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WallLineInput {
    pub state: u32,
    pub contact_normal: Vector3,
    pub skater_up: Vector3,
    pub deck_position: Vector3,
    pub time: f32,
}
pub fn wall_probe(input: WallLineInput) -> Option<WheelLine> {
    if !matches!(input.state, 100 | 101)
        || !(input.contact_normal.y.abs() < 0.5)
        || !(dot(input.skater_up, input.contact_normal) > f32::from_bits(0x3f35_c28f))
        || !(input.time > f32::from_bits(0x3c23_d70a))
    {
        return None;
    }
    let normal = input.contact_normal;
    let start = Vector3::new(
        normal.x.mul_add(0.05, input.deck_position.x),
        normal.y.mul_add(0.05, input.deck_position.y),
        normal.z.mul_add(0.05, input.deck_position.z),
    );
    let first = cross(Vector3::new(0., 1., 0.), normal);
    let down = cross(first, normal);
    let squared = dot(down, down);
    let inverse = inverse_length_squared(squared, 2);
    let magnitude = if squared == 0. { 0. } else { squared * inverse };
    let down = if magnitude > f32::from_bits(0x3586_37bd) {
        Vector3::new(down.x * inverse, down.y * inverse, down.z * inverse)
    } else {
        Vector3::ZERO
    };
    Some(WheelLine {
        start,
        end: Vector3::new(
            down.x.mul_add(4., start.x),
            down.y.mul_add(4., start.y),
            down.z.mul_add(4., start.z),
        ),
    })
}
fn cross(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(
        (-a.z).mul_add(b.y, a.y * b.z),
        (-a.x).mul_add(b.z, a.z * b.x),
        (-a.y).mul_add(b.x, a.x * b.y),
    )
}

