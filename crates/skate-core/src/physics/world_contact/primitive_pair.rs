use super::ContactPrimitive;
use super::{
    PrimitiveContactManifold, PrimitiveKind, best_separating_direction,
    find_feature_intersection_prism,
    primitive_query::{
        box_box_sat,
        geometry::{pack, xyz},
        maximum, triangle_box_sat,
    },
    prism_math::*,
    project_direction,
};
use crate::{
    math::Vector3,
    physics::collision::{ContactPair, TriangleFixup, fix_up_triangle},
};

#[derive(Clone, Copy, Debug)]
pub struct PrimitivePairSettings {
    pub padding_a: f32,
    pub padding_b: f32,
    pub additional_padding: f32,
    pub edge_cos_bend_normal_threshold: f32,
    pub convexity_epsilon: f32,
}

impl PrimitivePairSettings {
    pub const fn skater_self_collision() -> Self {
        Self {
            padding_a: f32::from_bits(0x3D4C_CCCD),
            padding_b: f32::from_bits(0x3D4C_CCCD),
            additional_padding: 0.0,
            edge_cos_bend_normal_threshold: f32::from_bits(0x3F7F_BE77),
            convexity_epsilon: f32::from_bits(0x3C23_D70A),
        }
    }
}

pub fn primitive_pair_contacts(
    primitive_a: ContactPrimitive,
    primitive_b: ContactPrimitive,
    settings: PrimitivePairSettings,
) -> Option<PrimitiveContactManifold> {
    let (a, a_kind) = pack(primitive_a);
    let (b, b_kind) = pack(primitive_b);
    let (separation, initial_normal) = separating_direction(&a, a_kind, &b, b_kind);
    let limit = (settings.padding_b + settings.padding_a) + settings.additional_padding;
    let fat_a = f32::from_bits(a[32]);
    let fat_b = f32::from_bits(b[32]);
    if separation > (fat_b + limit) + fat_a {
        return None;
    }
    let mut feature_a = maximum(&a, a_kind, 1, initial_normal);
    let mut feature_b = maximum(&b, b_kind, 0, initial_normal.map(|w| w ^ 0x8000_0000));
    let mut prism = [0; 136];
    if find_feature_intersection_prism(&mut prism, &mut feature_a, &mut feature_b, initial_normal)
        == 0
    {
        return None;
    }
    let count = prism[132] as usize;
    let mut normal = initial_normal.map(f32::from_bits);
    if count == 1 {
        let delta = sub(load(&prism, 64), load(&prism, 0));
        let squared = dot(delta, delta);
        let inverse = inverse_length(squared);
        let length = if squared == 0.0 {
            0.0
        } else {
            squared * inverse
        };
        if length > f32::from_bits(0x3400_0000) {
            prism[128..132].copy_from_slice(&scale(delta, inverse).map(f32::to_bits));
            prism[133] = 1;
        }
    }
    if prism[133] != 0 {
        let direction: [u32; 4] = prism[128..132].try_into().unwrap();
        let mut interval_a = [0; 12];
        let mut interval_b = [0; 12];
        project_direction(&a, a_kind, direction, &mut interval_a);
        project_direction(&b, b_kind, direction, &mut interval_b);
        let forward = f32::from_bits(interval_a[0]) - f32::from_bits(interval_b[4]);
        let reverse = f32::from_bits(interval_b[0]) - f32::from_bits(interval_a[4]);
        let flip = forward > reverse;
        let refined = if flip { forward } else { reverse };
        if refined >= separation {
            normal = direction.map(|w| f32::from_bits(if flip { w ^ 0x8000_0000 } else { w }));
        }
    }
    let mut points = [ContactPair {
        a: Vector3::ZERO,
        b: Vector3::ZERO,
    }; 16];
    for (i, point) in points[..count].iter_mut().enumerate() {
        point.a = xyz(load(&prism, 4 * i));
        point.b = xyz(load(&prism, 64 + 4 * i));
    }
    let mut pair_normal = xyz(normal);
    for (primitive, reverse) in [(primitive_a, false), (primitive_b, true)] {
        if let ContactPrimitive::Triangle(triangle) = primitive {
            if !fix_up_triangle(
                triangle.feature,
                &mut pair_normal,
                &mut points[..count],
                TriangleFixup {
                    reverse,
                    edge_cos_bend_normal_threshold: settings.edge_cos_bend_normal_threshold,
                    convexity_epsilon: settings.convexity_epsilon,
                    is_object: false,
                },
            ) {
                return None;
            }
        }
    }
    let direction = [pair_normal.x, pair_normal.y, pair_normal.z, normal[3]];
    let offset_a = scale(direction, fat_a);
    let offset_b = scale(direction, fat_b);
    for point in &mut points[..count] {
        point.a = Vector3::new(
            point.a.x + offset_a[0],
            point.a.y + offset_a[1],
            point.a.z + offset_a[2],
        );
        point.b = Vector3::new(
            point.b.x - offset_b[0],
            point.b.y - offset_b[1],
            point.b.z - offset_b[2],
        );
    }
    Some(PrimitiveContactManifold {
        normal: Vector3::new(-pair_normal.x, -pair_normal.y, -pair_normal.z),
        points,
        count,
    })
}

fn separating_direction(
    a: &[u32; 48],
    a_kind: PrimitiveKind,
    b: &[u32; 48],
    b_kind: PrimitiveKind,
) -> (f32, [u32; 4]) {
    let (separation, direction) = match (a_kind, b_kind) {
        (PrimitiveKind::Sphere, PrimitiveKind::Sphere) => {
            let delta = sub(load(b, 0), load(a, 0));
            let squared = dot(delta, delta);
            let inverse = inverse_length(squared);
            let length = if squared == 0.0 {
                0.0
            } else {
                squared * inverse
            };
            (
                [length.to_bits(); 4],
                scale(delta, inverse).map(f32::to_bits),
            )
        }
        (PrimitiveKind::Box, PrimitiveKind::Box) => box_box_sat::box_box(a, b),
        (PrimitiveKind::Triangle, PrimitiveKind::Box) => triangle_box_sat::triangle_box(a, b),
        (PrimitiveKind::Box, PrimitiveKind::Triangle) => {
            let (separation, normal) = triangle_box_sat::triangle_box(b, a);
            (separation, normal.map(|w| w ^ 0x8000_0000))
        }
        _ => best_separating_direction(a, a_kind, b, b_kind),
    };
    (f32::from_bits(separation[0]), direction)
}

