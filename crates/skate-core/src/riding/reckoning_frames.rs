use crate::{
    math::Vector3,
    physics::{
        board_motion_output::inverse_length_squared,
        native_arithmetic::dot3,
        skeleton_animation_record::{AnimationPartTransform, IDENTITY, compose_affine},
    },
    point_graph::PointGraph,
    riding::collision_response::signed_angle,
    trigonometry,
};

#[derive(Clone, Debug)]
pub struct ReckoningFrames {
    pub ground: AnimationPartTransform,
    pub system: AnimationPartTransform,
    pub unflipped: AnimationPartTransform,
    pub inverse_system: AnimationPartTransform,
    pub body_flip: AnimationPartTransform,
    pub heading: [f32; 4],
    pub target_lean_angle: f32,
    pub lateral_tilt: [f32; 4],
}

impl ReckoningFrames {
    pub fn new() -> Self {
        Self {
            ground: IDENTITY,
            system: IDENTITY,
            unflipped: IDENTITY,
            inverse_system: IDENTITY,
            body_flip: IDENTITY,
            heading: [1.0, 0.0, 0.0, 0.0],
            target_lean_angle: 0.0,
            lateral_tilt: [0.0; 4],
        }
    }

    pub fn calculate_transform(&mut self, up: [f32; 4], ground_normal: [f32; 4]) {
        let right = cross(up, self.heading);
        let forward = cross(right, up);
        self.heading = normalize(forward);
        self.system[0] = normalize(right);
        self.system[1] = up;
        self.system[2] = self.heading;
        self.unflipped = self.system;
        self.system = compose_affine(&self.body_flip, &self.system);
        self.inverse_system = inverse(&self.system);

        let ground_right = cross(ground_normal, self.heading);
        let ground_forward = cross(ground_right, ground_normal);
        self.ground[0] = normalize(ground_right);
        self.ground[1] = ground_normal;
        self.ground[2] = normalize(ground_forward);
    }

    pub fn calculate_dynamic_lean(&mut self, up: [f32; 4], dynamic_up: [f32; 4]) {
        let axis = self.system[2];
        let project = |value| {
            let amount = dot3(value, axis);
            std::array::from_fn(|i| value[i] - axis[i] * amount)
        };
        let from = project(up);
        let to = project(dynamic_up);
        let threshold = f32::from_bits(0x3727_c5ac);
        self.target_lean_angle = if dot3(from, from) > threshold && dot3(to, to) > threshold {
            wrap_fraction(signed_angle(xyz(from), xyz(to), xyz(axis)))
        } else {
            0.0
        };
    }

    pub fn calculate_tilt(
        &mut self,
        reverse_stance: bool,
        tilt_vs_angle: &PointGraph<8>,
        tilt_vs_up: &PointGraph<8>,
    ) {
        let axis = self.unflipped[1];
        let world_up = [0.0, 1.0, 0.0, 0.0];
        let dot = dot3(axis, world_up);
        let parallel = axis.map(|v| v * dot);
        let projected = std::array::from_fn(|i| world_up[i] - parallel[i]);
        let squared = dot3(projected, projected);
        if squared <= f32::from_bits(0x3a03_126f) {
            return;
        }
        let angle = if world_up == parallel {
            0.0
        } else {
            signed_angle(xyz(self.unflipped[2]), xyz(normalize(projected)), xyz(axis))
        };
        let half_pi = f32::from_bits(0x3fc9_0fdb);
        let scale = f32::from_bits(0x3f22_f983);
        let mut normalized_angle =
            ((wrap_fraction(angle - half_pi).abs() - half_pi) * -1.0) * scale;
        if reverse_stance {
            normalized_angle = -normalized_angle;
        }
        let normalized_up_angle = (half_pi - trigonometry::asin(axis[1])) * scale;
        let angle_tilt = tilt_vs_angle.evaluate(normalized_angle.abs());
        let up_tilt = tilt_vs_up.evaluate(normalized_up_angle.abs());
        let signed = if normalized_angle >= -0.0 {
            angle_tilt
        } else {
            -angle_tilt
        };
        self.lateral_tilt = [up_tilt * signed, 0.0, 0.0, 0.0];
    }
}

fn normalize(value: [f32; 4]) -> [f32; 4] {
    let reciprocal = inverse_length_squared(dot3(value, value), 2);
    value.map(|v| v * reciprocal)
}
fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        (-a[2]).mul_add(b[1], a[1] * b[2]),
        (-a[0]).mul_add(b[2], a[2] * b[0]),
        (-a[1]).mul_add(b[0], a[0] * b[1]),
        (-a[3]).mul_add(b[3], a[3] * b[3]),
    ]
}
fn inverse(frame: &AnimationPartTransform) -> AnimationPartTransform {
    let axes: [[f32; 4]; 3] = std::array::from_fn(|i| [frame[0][i], frame[1][i], frame[2][i], 0.0]);
    let p = frame[3].map(|v| 0.0 - v);
    let position = std::array::from_fn(|i| {
        let z = axes[2][i] * p[2];
        let y = axes[1][i].mul_add(p[1], z);
        axes[0][i].mul_add(p[0], y)
    });
    [axes[0], axes[1], axes[2], position]
}
fn wrap_fraction(angle: f32) -> f32 {
    let turns = angle * f32::from_bits(0x3e22_f983);
    let fraction = turns - turns.floor();
    let fraction = fraction - if fraction > 0.5 { 1.0 } else { 0.0 };
    fraction * f32::from_bits(0x40c9_0fdb)
}
fn xyz(v: [f32; 4]) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}

