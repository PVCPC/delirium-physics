use super::{
    ContactPair, Sphere, Triangle, TriangleFixup, arithmetic::*, fix_up_triangle,
    sphere_triangle::build_prism,
};
use crate::math::Vector3;

#[derive(Clone, Copy, Debug)]
pub struct WorldContactSettings {
    pub volume_padding: f32,
    pub maximum_separating_distance: f32,
    pub edge_cos_bend_normal_threshold: f32,
    pub convexity_epsilon: f32,
    pub is_object: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct WorldContact {
    pub normal: Vector3,
    pub points: ContactPair,
}

pub fn world_separation_limit(
    velocity: Vector3,
    triangle_normal: Vector3,
    padding: f32,
    maximum: f32,
) -> f32 {
    let approach = dot(
        neg(triangle_normal),
        scale(velocity, f32::from_bits(0x3C88_8889)),
    );
    let positive = if -approach >= 0.0 { 0.0 } else { approach };
    let limited = if maximum - positive >= 0.0 {
        positive
    } else {
        maximum
    };
    limited + padding
}

pub fn sphere_triangle_world_contact(
    sphere: Sphere,
    triangle: Triangle,
    velocity: Vector3,
    settings: WorldContactSettings,
) -> Option<WorldContact> {
    let separation = world_separation_limit(
        velocity,
        triangle.feature.normal,
        settings.volume_padding,
        settings.maximum_separating_distance,
    );
    let prism = build_prism(sphere, triangle, separation, false, true)?;
    let mut normal = prism.normal;
    let mut points = [prism.raw_points];
    if !fix_up_triangle(
        triangle.feature,
        &mut normal,
        &mut points,
        TriangleFixup {
            reverse: true,
            edge_cos_bend_normal_threshold: settings.edge_cos_bend_normal_threshold,
            convexity_epsilon: settings.convexity_epsilon,
            is_object: settings.is_object,
        },
    ) {
        return None;
    }
    let a = scale(normal, sphere.radius);
    Some(WorldContact {
        normal: neg(normal),
        points: ContactPair {
            a: Vector3::new(
                points[0].a.x + a.x,
                points[0].a.y + a.y,
                points[0].a.z + a.z,
            ),
            b: sub(points[0].b, scale(normal, triangle.fatness)),
        },
    })
}
