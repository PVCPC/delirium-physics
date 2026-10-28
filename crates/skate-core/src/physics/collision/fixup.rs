use super::{arithmetic::*, edge::edge_cos_test};
use crate::math::Vector3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriangleRegion {
    Face,
    Edge0,
    Edge1,
    Vertex1,
    Edge2,
    Vertex0,
    Vertex2,
}

#[derive(Clone, Copy, Debug)]
pub struct TriangleFeature {
    pub normal: Vector3,
    pub edges: [Vector3; 3],
    pub flags: u32,
    pub edge_cosines: [f32; 3],
}
impl TriangleFeature {
    pub const ONE_SIDED: u32 = 0x10;
    pub const USE_EDGE_COSINES: u32 = 0x100;
    pub const fn edge_convex(self, edge: usize) -> bool {
        self.flags & (0x20 << edge) != 0
    }
    pub const fn vertex_disabled(self, vertex: usize) -> bool {
        self.flags & (0x200 << vertex) != 0
    }

    pub fn classify(self, direction: Vector3) -> TriangleRegion {
        use TriangleRegion::*;
        let [a, b, c] = self.edges.map(|e| dot(direction, e));
        let t = f32::from_bits(0x3D4C_CCCD);
        if a > t && b < -t {
            Vertex2
        } else if b > t && c < -t {
            Vertex1
        } else if c > t && a < -t {
            Vertex0
        } else if a > -c && -b >= a {
            Edge2
        } else if b > -a && -c >= b {
            Edge1
        } else if c > -b && -a >= c {
            Edge0
        } else {
            Face
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactPair {
    pub a: Vector3,
    pub b: Vector3,
}

#[derive(Clone, Copy, Debug)]
pub struct TriangleFixup {
    pub reverse: bool,
    pub edge_cos_bend_normal_threshold: f32,
    pub convexity_epsilon: f32,
    pub is_object: bool,
}

pub fn fix_up_triangle(
    triangle: TriangleFeature,
    normal: &mut Vector3,
    contacts: &mut [ContactPair],
    settings: TriangleFixup,
) -> bool {
    assert!(contacts.len() <= 16);
    let toward = if settings.reverse {
        neg(*normal)
    } else {
        *normal
    };
    let projection = dot(toward, triangle.normal);
    let face_tolerance = f32::from_bits(0x3F7F_F62B);
    let region = if projection.abs() < face_tolerance {
        triangle.classify(normalize(
            sub(toward, scale(triangle.normal, projection)),
            2,
        ))
    } else {
        TriangleRegion::Face
    };
    let one_sided = triangle.flags & TriangleFeature::ONE_SIDED != 0;
    use TriangleRegion::*;
    let edge = match region {
        Edge0 => Some(0),
        Edge1 => Some(1),
        Edge2 => Some(2),
        _ => None,
    };
    let vertex = match region {
        Vertex0 => Some(0),
        Vertex1 => Some(1),
        Vertex2 => Some(2),
        _ => None,
    };
    if triangle.flags & TriangleFeature::USE_EDGE_COSINES == 0 {
        if region == Face {
            return !one_sided || projection > 0.0;
        }
        if one_sided {
            if projection > face_tolerance {
                return true;
            }
            if projection < 0.0 {
                return false;
            }
        } else if projection.abs() > face_tolerance {
            return true;
        }
        return edge
            .map(|i| triangle.edge_convex(i))
            .unwrap_or_else(|| !triangle.vertex_disabled(vertex.unwrap()));
    }
    if region == Face {
        return !one_sided || projection > 0.0;
    }
    if let Some(i) = edge {
        if !one_sided {
            return (if triangle.edge_convex(i) {
                projection
            } else {
                -projection
            }) >= triangle.edge_cosines[i];
        }
        return one_sided_edge(triangle, i, projection, normal, contacts, settings);
    }
    let vertex = vertex.unwrap();
    let first = (vertex + 2) % 3;
    let second = vertex;
    if !one_sided {
        return !triangle.vertex_disabled(vertex)
            && [first, second].into_iter().all(|i| {
                edge_cos_test(
                    neg(triangle.edges[2 - i]),
                    triangle.normal,
                    toward,
                    triangle.edge_cosines[i],
                    triangle.edge_convex(i),
                    true,
                )
            });
    }
    let disabled = triangle.vertex_disabled(vertex) && !settings.is_object;
    if projection > 0.0 && disabled {
        let edge = match (triangle.edge_convex(first), triangle.edge_convex(second)) {
            (true, false) => second,
            (false, true) => first,
            _ => return false,
        };
        let cosine = triangle.edge_cosines[edge];
        if !(cosine <= f32::from_bits(0x3F78_51EC) && cosine > f32::from_bits(0x3A83_126F)) {
            return false;
        }
        let direction = triangle.edges[2 - if edge == second { first } else { second }];
        let bent = normalize(cross(cross(direction, toward), direction), 1);
        bend(normal, contacts, bent, settings.reverse);
        return true;
    }
    if disabled {
        return false;
    }
    [first, second].into_iter().all(|i| {
        edge_cos_test(
            neg(triangle.edges[2 - i]),
            triangle.normal,
            toward,
            triangle.edge_cosines[i],
            triangle.edge_convex(i),
            false,
        )
    })
}

fn one_sided_edge(
    t: TriangleFeature,
    edge: usize,
    projection: f32,
    normal: &mut Vector3,
    contacts: &mut [ContactPair],
    s: TriangleFixup,
) -> bool {
    let cosine = t.edge_cosines[edge];
    if t.edge_convex(edge)
        || (s.is_object && ((cosine - 1.0) as f64).abs() < f64::from_bits(0x3EE4_F8B5_88E3_68F1))
    {
        return projection + s.convexity_epsilon >= cosine;
    }
    if cosine > s.edge_cos_bend_normal_threshold || cosine <= f32::from_bits(0x3A83_126F) {
        return false;
    }
    let squared = 1.0 - cosine * cosine;
    let threshold = if squared == 0.0 {
        0.0
    } else {
        squared * inverse_length_squared(squared, 2)
    };
    if projection > threshold {
        bend(normal, contacts, t.normal, s.reverse);
        true
    } else {
        false
    }
}

fn bend(normal: &mut Vector3, contacts: &mut [ContactPair], direction: Vector3, reverse: bool) {
    *normal = if reverse { neg(direction) } else { direction };
    for pair in contacts {
        if reverse {
            pair.b = madd(direction, dot(sub(pair.b, pair.a), direction), pair.a);
        } else {
            pair.a = madd(direction, dot(sub(pair.a, pair.b), direction), pair.b);
        }
    }
}
