use crate::physics::SkaterRuntime;
use skate_core::physics::skeleton_animation_record::compose_affine;

pub(crate) fn audit_render_parts(skater: &SkaterRuntime) -> [(usize, f32, f32); 24] {
    let live = skater.skeleton.part_transforms();
    std::array::from_fn(|part| {
        let expected = compose_affine(
            &skater.skeleton.record.pose[part],
            &skater.skeleton_output.pose.geometry.inverse_part_frames[part],
        );
        let rendered = compose_affine(
            &skater.animated_skeleton.roots.animation_to_world,
            &skater.render_pose[skater.animated_skeleton.bone_indices[part]],
        );
        let error = |a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]| {
            let mut maximum = 0.0_f32;
            for column in 0..4 {
                for lane in 0..3 {
                    let delta = (a[column][lane] - b[column][lane]).abs();
                    if !delta.is_finite() {
                        return f32::INFINITY;
                    }
                    maximum = maximum.max(delta);
                }
            }
            maximum
        };
        (
            part,
            error(&rendered, &expected),
            error(&skater.skeleton.record.pose[part], &live[part]),
        )
    })
}
