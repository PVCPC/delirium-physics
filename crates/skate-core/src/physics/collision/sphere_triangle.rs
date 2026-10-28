use super::{ContactPair, TriangleFeature, TriangleFixup, arithmetic::*, fix_up_triangle};
use crate::math::Vector3;

#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub center: Vector3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Triangle {
    pub vertices: [Vector3; 3],
    pub feature: TriangleFeature,
    pub edge_lengths: [f32; 3],
    pub fatness: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SphereTrianglePair {
    pub separating_direction: Vector3,
    pub separating_distance: f32,
    pub normal: Vector3,
    pub points: ContactPair,
    pub distance: f32,
    pub triangle_region: u32,
}

pub fn intersect_sphere_triangle(
    sphere: Sphere,
    triangle: Triangle,
    minimum_separating_distance: f32,
    triangle_is_a: bool,
) -> Option<SphereTrianglePair> {
    let prism = build_prism(
        sphere,
        triangle,
        minimum_separating_distance,
        triangle_is_a,
        false,
    )?;
    let SphereTrianglePrism {
        separating_direction,
        separating_distance,
        mut normal,
        raw_points,
        triangle_region,
    } = prism;
    let (fat_a, fat_b) = if triangle_is_a {
        (triangle.fatness, sphere.radius)
    } else {
        (sphere.radius, triangle.fatness)
    };
    let points = ContactPair {
        a: madd(normal, fat_a, raw_points.a),
        b: sub(raw_points.b, scale(normal, fat_b)),
    };
    let distance = dot(sub(points.b, points.a), normal);
    let settings = TriangleFixup {
        reverse: !triangle_is_a,
        edge_cos_bend_normal_threshold: -1.0,
        convexity_epsilon: 0.0,
        is_object: false,
    };
    if !fix_up_triangle(triangle.feature, &mut normal, &mut [raw_points], settings) {
        return None;
    }
    Some(SphereTrianglePair {
        separating_direction,
        separating_distance,
        normal,
        points,
        distance,
        triangle_region,
    })
}

pub(super) struct SphereTrianglePrism {
    pub separating_direction: Vector3,
    pub separating_distance: f32,
    pub normal: Vector3,
    pub raw_points: ContactPair,
    pub triangle_region: u32,
}

pub(super) fn build_prism(
    sphere: Sphere,
    triangle: Triangle,
    minimum_separating_distance: f32,
    triangle_is_a: bool,
    contact_query: bool,
) -> Option<SphereTrianglePrism> {
    let axis = triangle.feature.normal;
    let (a, b) = interval_gaps(sphere, triangle, axis, triangle_is_a);
    let (separating_direction, separating_distance) =
        if a > b { (neg(axis), a) } else { (axis, b) };
    let (fat_a, fat_b) = if triangle_is_a {
        (triangle.fatness, sphere.radius)
    } else {
        (sphere.radius, triangle.fatness)
    };
    if separating_distance > (fat_b + minimum_separating_distance) + fat_a {
        return None;
    }
    let (face_point, triangle_region) =
        point_face(sphere.center, triangle, separating_direction, triangle_is_a);
    let raw_points = if triangle_is_a {
        ContactPair {
            a: face_point,
            b: sphere.center,
        }
    } else {
        ContactPair {
            a: sphere.center,
            b: face_point,
        }
    };
    let delta = sub(raw_points.b, raw_points.a);
    let squared = dot(delta, delta);
    let inverse_length = inverse_length_squared(squared, 2);
    let gate = if contact_query {
        if squared == 0.0 {
            0.0
        } else {
            squared * inverse_length
        }
    } else {
        squared
    };
    let mut normal = separating_direction;
    if gate > f32::from_bits(0x3400_0000) {
        let candidate = scale(delta, inverse_length);
        let (a, b) = interval_gaps(sphere, triangle, candidate, triangle_is_a);
        let maximum = if a > b { a } else { b };
        if separating_distance <= maximum {
            normal = if a > b { neg(candidate) } else { candidate };
        }
    }
    Some(SphereTrianglePrism {
        separating_direction,
        separating_distance,
        normal,
        raw_points,
        triangle_region,
    })
}

fn interval_gaps(
    sphere: Sphere,
    triangle: Triangle,
    direction: Vector3,
    triangle_is_a: bool,
) -> (f32, f32) {
    let s = dot(direction, sphere.center);
    let p = triangle.vertices.map(|v| dot(direction, v));
    let lo = min(min(p[0], p[1]), p[2]);
    let hi = max(max(p[0], p[1]), p[2]);
    if triangle_is_a {
        (lo - s, s - hi)
    } else {
        (s - hi, lo - s)
    }
}
fn min(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}
fn max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

fn point_face(point: Vector3, t: Triangle, axis: Vector3, triangle_is_a: bool) -> (Vector3, u32) {
    let query = if triangle_is_a { axis } else { neg(axis) };
    let same = (dot(query, t.feature.normal) < 0.0) == triangle_is_a;
    let indices = if same { [0, 1, 2] } else { [2, 1, 0] };
    let vertices = if same { [0, 2, 1] } else { [0, 1, 2] };
    let mut region = if same { 8 } else { 0 };
    let mut projected = madd(axis, dot(axis, sub(t.vertices[0], point)), point);
    let mut best = 0usize;
    let mut violation = 0.0;
    let planes = indices.map(|i| {
        let direction = if same {
            t.feature.edges[i]
        } else {
            neg(t.feature.edges[i])
        };
        let plane = cross(direction, axis);
        let squared = dot(plane, plane);
        scale(
            plane,
            if squared > f32::from_bits(0x3400_0000) {
                inverse_length_squared(squared, 1)
            } else {
                0.0
            },
        )
    });
    for i in 0..3 {
        let d = dot(planes[i], sub(projected, t.vertices[vertices[i]]));
        if i == 0 || d > violation {
            best = i;
            violation = d;
        }
    }
    if violation > 0.0 {
        projected = sub(projected, scale(planes[best], violation));
        let i = indices[best];
        let base = t.vertices[vertices[best]];
        let direction = if same {
            t.feature.edges[i]
        } else {
            neg(t.feature.edges[i])
        };
        let along = dot(sub(projected, base), direction);
        let (p, r) = if along < 0.0 {
            (base, 1)
        } else if along > t.edge_lengths[i] {
            (madd(direction, t.edge_lengths[i], base), 3)
        } else {
            (madd(direction, along, base), 2)
        };
        projected = p;
        region += best as u32 * 2 + r;
    }
    (projected, region)
}
