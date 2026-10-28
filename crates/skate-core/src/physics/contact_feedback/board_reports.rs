use crate::math::Vector3;

use super::super::{
    assembly::BodySnapshot,
    board::{BODY_COUNT, BodyId},
    board_step::CollisionBody,
    contact_solver::RetailContactJacobian,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardContactReport {
    pub part: BodyId,
    pub other: CollisionBody,
    pub is_body_a: bool,
    pub normal: Vector3,
    pub position: Vector3,
    pub relative_linear_velocity: Vector3,
    pub other_surface: u16,
    pub normal_force_on_a: Vector3,
    pub friction_force_on_a: Vector3,
    pub tangents: [Vector3; 2],
}

pub(crate) fn collect(
    output: &mut Vec<BoardContactReport>,
    contacts: &[RetailContactJacobian],
    bodies: &[BodySnapshot; BODY_COUNT],
    frequency: f32,
) {
    output.clear();
    let frequency_squared = frequency * frequency;
    for contact in contacts {
        let words = contact.words();
        let impulse = contact.accumulated_impulse();
        if words[11] & 8 == 0 || !(impulse[0] > 0.0) {
            continue;
        }
        if !((words[31] < BODY_COUNT as u32 && words[43] == u32::MAX)
            || (words[43] < BODY_COUNT as u32 && words[31] == u32::MAX)) {
            continue;
        }
        let a = body_id(words[31]);
        let b = body_id(words[43]);
        let (part, other, is_body_a) = match (a, b) {
            (CollisionBody::Board(part), CollisionBody::StaticWorld) => (part, b, true),
            (CollisionBody::StaticWorld, CollisionBody::Board(part)) => (part, a, false),
            _ => continue,
        };
        if output.len() == 16 {
            break;
        }
        let normal = vector(words, 28);
        let tangents = [vector(words, 40), vector(words, 52)];
        let a_position = add(center(a, bodies), vector(words, 0));
        let b_position = add(center(b, bodies), vector(words, 4));
        let a_weight = f32::from_bits(words[35]);
        let b_weight = f32::from_bits(words[39]);
        let inverse_weight = 1.0 / (b_weight + a_weight);
        let position = combine(a_position, a_weight, scale(b_position, b_weight));
        let friction = combine(tangents[0], impulse[1], scale(tangents[1], impulse[2]));
        let sign = if is_body_a { 1.0 } else { -1.0 };
        let this = CollisionBody::Board(part);
        output.push(BoardContactReport {
            part,
            other,
            is_body_a,
            normal: scale(normal, sign),
            position: scale(position, inverse_weight),
            relative_linear_velocity: subtract(velocity(this, bodies), velocity(other, bodies)),
            other_surface: if is_body_a {
                words[55] as u16
            } else {
                (words[55] >> 16) as u16
            },
            normal_force_on_a: scale(scale(normal, impulse[0]), frequency_squared),
            friction_force_on_a: scale(friction, frequency_squared),
            tangents,
        });
    }
}

fn body_id(value: u32) -> CollisionBody {
    if value == u32::MAX {
        CollisionBody::StaticWorld
    } else {
        CollisionBody::Board(BodyId::ORDER[value as usize])
    }
}

fn center(body: CollisionBody, bodies: &[BodySnapshot; BODY_COUNT]) -> Vector3 {
    match body {
        CollisionBody::Board(id) => bodies[id.index()].rates.position,
        CollisionBody::StaticWorld => Vector3::ZERO,
        CollisionBody::Attached(_) => unreachable!("collector selects board/world contacts above"),
    }
}

fn velocity(body: CollisionBody, bodies: &[BodySnapshot; BODY_COUNT]) -> Vector3 {
    match body {
        CollisionBody::Board(id) => bodies[id.index()].rates.linear_velocity,
        CollisionBody::StaticWorld => Vector3::ZERO,
        CollisionBody::Attached(_) => unreachable!("collector selects board/world contacts above"),
    }
}

fn vector(words: &[u32; 64], offset: usize) -> Vector3 {
    Vector3::new(
        f32::from_bits(words[offset]),
        f32::from_bits(words[offset + 1]),
        f32::from_bits(words[offset + 2]),
    )
}
fn scale(v: Vector3, scale: f32) -> Vector3 {
    Vector3::new(v.x * scale, v.y * scale, v.z * scale)
}
fn add(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}
fn subtract(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}
fn combine(a: Vector3, weight: f32, b: Vector3) -> Vector3 {
    Vector3::new(
        a.x.mul_add(weight, b.x),
        a.y.mul_add(weight, b.y),
        a.z.mul_add(weight, b.z),
    )
}
