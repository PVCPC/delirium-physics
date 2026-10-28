use super::{
    FeaturePrism, MaximumFeature, closest_feature_segment, intersect_feature_segments,
    intersect_point_face, intersect_segment_face,
};

#[path = "face_face.rs"]
mod face_face;
#[path = "face_specialized.rs"]
mod face_specialized;

pub fn find_feature_intersection_prism(
    output: &mut FeaturePrism,
    a: &mut MaximumFeature,
    b: &mut MaximumFeature,
    normal: [u32; 4],
) -> u32 {
    let (a_edges, b_edges) = (a[140], b[140]);
    assert!(
        a_edges <= 8 && b_edges <= 8,
        "maximum-feature storage exceeded"
    );
    match (a_edges, b_edges) {
        (0, 0) => {
            output[132] = 1;
            output[..4].copy_from_slice(&a[136..140]);
            output[64..68].copy_from_slice(&b[136..140]);
            return 1;
        }
        (1, 0) => return segment_point(output, a, b, true),
        (0, 1) => return segment_point(output, b, a, false),
        (0, _) => return intersect_point_face(output, b, a, normal, false),
        (_, 0) => return intersect_point_face(output, a, b, normal, true),
        (1, 1) => return intersect_feature_segments(output, a, b, normal, true),
        (1, _) => return intersect_segment_face(output, b, a, normal, false),
        (_, 1) => return intersect_segment_face(output, a, b, normal, true),
        (4, 3) => {
            if face_specialized::quad_triangle(output, a, b, normal, true) != 0 {
                return 1;
            }
        }
        (3, 4) => {
            if face_specialized::quad_triangle(output, b, a, normal, false) != 0 {
                return 1;
            }
        }
        (4, 4) => {
            if face_specialized::quad_quad(output, a, b, normal) != 0 {
                return 1;
            }
        }
        _ => {}
    }
    face_face::intersect(output, a, b)
}

fn segment_point(
    output: &mut FeaturePrism,
    segment: &mut MaximumFeature,
    point: &MaximumFeature,
    segment_is_a: bool,
) -> u32 {
    output[132] = 1;
    output[..4].copy_from_slice(&point[136..140]);
    output[64..68].copy_from_slice(&point[136..140]);
    let offset = if segment_is_a { 0 } else { 64 };
    let region = closest_feature_segment(
        segment[4..20].try_into().unwrap(),
        (&mut output[offset..offset + 4]).try_into().unwrap(),
    );
    segment[0] = segment[0].wrapping_sub(region).wrapping_add(2);
    1
}
