use crate::point_graph::PointGraph;
mod math;
use math::*;
pub type Vector = [f32; 4];

pub struct Settings {
    pub slope_speed_scalar: PointGraph<8>,
    pub slope_mode_speed: PointGraph<8>,
    pub turn_vs_speed: PointGraph<8>,
    pub turn_delta_vs_speed: PointGraph<8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub velocity: Vector,
    pub turn: f32,
    pub forward_delta: f32,
    pub right_delta: f32,
    pub speed: f32,
    pub override_remaining: f32,
}
impl Default for State {
    fn default() -> Self {
        Self { velocity: [0.0; 4], turn: 0.0, forward_delta: 0.0, right_delta: 0.0, speed: 0.0, override_remaining: -1.0 }
    }
}
pub struct Input {
    pub forward: Vector,
    pub right: Vector,
    pub plane_normal: Vector,
    pub desired_speed: f32,
    pub steering: f32,
    pub slope_mode: bool,
    pub obstacle: bool,
    pub obstacle_normal: Vector,
    pub override_gate: f32,
    pub override_duration: f32,
    pub override_velocity: Vector,
    pub flags: u32,
}

impl State {
    pub fn update(&mut self, settings: &Settings, input: &Input) {
        let dt = f32::from_bits(0x3c88_8889);
        let radians = f32::from_bits(0x3c8e_fa35);
        let projected = sub(input.forward, scale(input.plane_normal, dot(input.forward, input.plane_normal)));
        let direction = normalize_or(projected, input.forward);
        let slope_degrees = crate::trigonometry::asin(direction[1]) * f32::from_bits(0x4265_2ee1);
        let target_speed = if input.slope_mode {
            settings.slope_mode_speed.evaluate(slope_degrees.abs())
        } else {
            let mut target = settings.slope_speed_scalar.evaluate(slope_degrees) * input.desired_speed;
            if !(input.desired_speed <= 0.5) && !(target >= self.speed) {
                let gravity_delta = if self.velocity[1] > 0.0 {
                    (f32::from_bits(0xc11c_cccd) * dt) * select(-direction[1], 0.0, direction[1])
                } else { 0.0 };
                let delta = target - self.speed;
                let limited_delta = select(delta - -2.0, delta, -2.0) * dt;
                target = self.speed + select(gravity_delta - limited_delta, limited_delta, gravity_delta);
            }
            target
        };
        let original = self.velocity;
        let mut target = madd(original, 0.0, scale(direction, target_speed));
        let turn_error = (settings.turn_vs_speed.evaluate(self.speed) * input.steering).mul_add(radians, -self.turn);
        let mut maximum_delta = if input.slope_mode { 0.5 } else { f32::from_bits(0x3e4c_cccd) };
        if !(input.override_gate < 0.0) && !input.slope_mode {
            let first = self.override_remaining < 0.0;
            if first { self.override_remaining = input.override_duration; }
            let ratio = if input.override_duration < f32::from_bits(0x3a83_126f) { 0.0 } else { self.override_remaining / input.override_duration };
            let amount = ratio * 1.25;
            let mut blend = select(1.0 - amount, amount, 1.0);
            if !first && !(length(input.override_velocity) <= length(original) * f32::from_bits(0x3f8c_cccd)) {
                blend = select(blend - 0.5, 0.5, blend);
            }
            target = madd(original, blend, scale(input.override_velocity, 1.0 - blend));
            maximum_delta = 100.0;
            if input.flags & 8 != 0 {
                let speed = length(target);
                if !(speed >= f32::from_bits(0x3c23_d70a)) {
                    target = scale(direction, 1.0);
                } else if !(speed >= 1.0) {
                    target = scale(target, 1.0 / speed);
                }
            }
            let remaining = self.override_remaining - dt;
            self.override_remaining = select(-remaining, 0.0, remaining);
        } else {
            self.override_remaining = -1.0;
        }
        let turn_delta = settings.turn_delta_vs_speed.evaluate(self.speed) * radians;
        let mut velocity = limit_delta(target, original, maximum_delta);
        self.turn += clamp(turn_error, -turn_delta, turn_delta);
        if input.obstacle {
            let opposite = scale(input.obstacle_normal, -1.0);
            target = remove_positive(target, opposite);
            velocity = remove_positive(velocity, opposite);
        }
        self.right_delta = dot(sub(velocity, original), input.right);
        self.forward_delta = dot(sub(target, original), input.forward);
        self.velocity = velocity;
        self.speed = length(velocity);
    }
}

