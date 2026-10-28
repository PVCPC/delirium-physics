use crate::{
    math::Vector3,
    physics::{board_motion_output::inverse_length_squared, force_queue::QueuedPointForce},
    point_graph::PointGraph,
    riding::collision_response::signed_angle,
};
type V = [f32; 4];
#[derive(Clone, Debug)]
pub struct SlideSettings {
    pub input_remap: PointGraph<8>,
    pub remap_vs_speed: PointGraph<8>,
    pub force_vs_angle: PointGraph<8>,
    pub force_vs_speed: PointGraph<8>,
    pub softest_wheel_force: f32,
    pub softest_wheel_spin: f32,
    pub angular_force: f32,
    pub force_y_offset: f32,
}
#[derive(Clone, Debug)]
pub struct SlideSurface {
    pub speed_to_force: PointGraph<8>,
    pub yaw_strength: f32,
    pub yaw_damping: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct SlideInput {
    pub velocity: V,
    pub normal: V,
    pub side: V,
    pub effective_forward: V,
    pub reference_forward: V,
    pub angular_velocity: V,
    pub absolute_speed: f32,
    pub surface_speed: f32,
    pub slide: f32,
    pub elapsed: f32,
    pub wheel_hardness: f32,
}
pub fn angular_correction(s: &SlideSettings, surface: &SlideSurface, p: SlideInput) -> V {
    let remap = s.input_remap.evaluate(p.slide.abs());
    let age = p.elapsed - 0.5;
    let age = fsel(-age, 0.0, age);
    let age = fsel(1.0 - age, age, 1.0);
    let weight = s.remap_vs_speed.evaluate(p.surface_speed) * age;
    let pi = f32::from_bits(0x4049_0fdb);
    let signed = fsel(p.slide, remap, -remap);
    let angle = (p.slide * pi).mul_add(1.0 - weight, (signed * pi) * weight);
    let velocity = normalize(p.velocity);
    let side = normalize(cross(p.normal, velocity));
    let desired = madd(
        velocity,
        crate::trigonometry::cos(angle),
        scale(side, crate::trigonometry::sin(angle)),
    );
    let mut error = cross(p.reference_forward, desired);
    if dot(p.reference_forward, desired) < 0.0 {
        error = if error[1] < 0.0 {
            p.normal.map(|v| -v)
        } else {
            p.normal
        };
    }
    let strength = (1.0 - s.softest_wheel_spin).mul_add(p.wheel_hardness, s.softest_wheel_spin)
        * surface.yaw_strength;
    let displacement = scale(
        madd(
            error,
            strength,
            scale(p.angular_velocity, surface.yaw_damping),
        ),
        s.angular_force,
    );
    scale(p.normal, dot(displacement, p.normal))
}
pub fn sliding_force(s: &SlideSettings, surface: &SlideSurface, p: SlideInput) -> QueuedPointForce {
    let tangent = reject(p.velocity, p.normal);
    let lateral = scale(p.side, dot(tangent, p.side));
    let response = surface.speed_to_force.evaluate(p.absolute_speed);
    let hardness = if response > 0.0 {
        p.wheel_hardness
    } else {
        1.0
    };
    let strength =
        (1.0 - s.softest_wheel_force).mul_add(hardness, s.softest_wheel_force) * response;
    let side = normalize_safe(cross(p.normal, p.velocity));
    let longitudinal = reject(scale(lateral, strength), normalize_safe(side));
    let angle = if dot(p.velocity, p.velocity) * dot(p.effective_forward, p.effective_forward)
        > f32::from_bits(0x3780_0000)
    {
        signed_angle(
            xyz(reject(p.velocity, p.normal)),
            xyz(reject(p.effective_forward, p.normal)),
            xyz(p.normal),
        )
    } else {
        0.0
    };
    let turns = angle * f32::from_bits(0x3e22_f983);
    let fraction = turns - turns.floor();
    let wrapped = (fraction - if fraction > 0.5 { 1.0 } else { 0.0 }) * f32::from_bits(0x40c9_0fdb);
    let degrees = wrapped * f32::from_bits(0x4265_2ee1);
    let sign = if degrees > 0.0 { 1.0 } else { -1.0 };
    let angular = scale(
        scale(side, s.force_vs_angle.evaluate(degrees.abs())),
        s.force_vs_speed.evaluate(p.surface_speed),
    );
    let force = madd(angular, sign, longitudinal);
    let y = if dot(force, p.velocity) < 0.0 {
        -s.force_y_offset
    } else {
        s.force_y_offset
    };
    QueuedPointForce {
        tag: 9,
        force_world: xyz(force),
        point_body: Vector3::new(0.0, y, 0.0),
    }
}
fn dot(a: V, b: V) -> f32 {
    crate::physics::native_arithmetic::dot3(a, b)
}
fn scale(v: V, s: f32) -> V {
    v.map(|v| v * s)
}
fn madd(v: V, s: f32, b: V) -> V {
    std::array::from_fn(|i| v[i].mul_add(s, b[i]))
}
fn reject(v: V, n: V) -> V {
    let d = dot(v, n);
    std::array::from_fn(|i| v[i] - n[i] * d)
}
fn cross(a: V, b: V) -> V {
    [
        (-a[2]).mul_add(b[1], a[1] * b[2]),
        (-a[0]).mul_add(b[2], a[2] * b[0]),
        (-a[1]).mul_add(b[0], a[0] * b[1]),
        (-a[3]).mul_add(b[3], a[3] * b[3]),
    ]
}
fn normalize(v: V) -> V {
    scale(v, inverse_length_squared(dot(v, v), 2))
}
fn normalize_safe(v: V) -> V {
    let d = dot(v, v);
    let inv = inverse_length_squared(d, 2);
    let length = if d == 0.0 { 0.0 } else { d * inv };
    if length > f32::from_bits(0x3586_37bd) {
        scale(v, inv)
    } else {
        [0.0; 4]
    }
}
fn fsel(v: f32, a: f32, b: f32) -> f32 {
    if v >= -0.0 { a } else { b }
}
fn xyz(v: V) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}

