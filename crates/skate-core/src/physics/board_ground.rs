use crate::{math::Vector3, trigonometry};
use super::{
    board::{BodyId, BODY_COUNT},
    board_motion_output::{add, dot, inverse_length_squared, length, scale, subtract},
    board_runtime::BoardRuntime,
    board_step::CollisionBody,
    contact_feedback::BoardContactReport,
    native_arithmetic,
};

const UP: Vector3 = Vector3::new(0.0, 1.0, 0.0);
pub const WHEEL_LINE_LENGTH: f32 = f32::from_bits(0x3E4C_CCCD);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelLine {
    pub start: Vector3,
    pub end: Vector3,
}

pub fn wheel_lines(board: &BoardRuntime, reckoning_up: Vector3) -> [WheelLine; 4] {
    let poses = board.part_transforms();
    let delta = scale(reckoning_up, WHEEL_LINE_LENGTH);
    core::array::from_fn(|i| WheelLine {
        start: poses[i].translation,
        end: subtract(poses[i].translation, delta),
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelLineHit {
    pub fraction: f32,
    pub normal: Vector3,
    pub surface_tag: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelLineState {
    pub normals: [Vector3; 4],
    pub distances: [f32; 4],
    pub physics_surfaces: [u32; 4],
    pub minimum_distance: f32,
}
impl Default for WheelLineState {
    fn default() -> Self {
        Self {
            normals: [UP; 4],
            distances: [0.0; 4],
            physics_surfaces: [0; 4],
            minimum_distance: 0.0,
        }
    }
}
impl WheelLineState {
    pub fn publish(&mut self, hits: [Option<WheelLineHit>; 4]) {
        self.minimum_distance = WHEEL_LINE_LENGTH;
        for (i, hit) in hits.into_iter().enumerate() {
            self.physics_surfaces[i] = 0;
            if let Some(hit) = hit {
                let distance = hit.fraction * WHEEL_LINE_LENGTH;
                self.minimum_distance = if distance - self.minimum_distance >= -0.0 {
                    self.minimum_distance
                } else {
                    distance
                };
                self.normals[i] = hit.normal;
                self.distances[i] = distance;
                self.physics_surfaces[i] = (hit.surface_tag >> 7) & 31;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartGroundContact {
    pub in_contact: bool,
    pub normal: Vector3,
    pub point: Vector3,
    pub relative_velocity: Vector3,
}
const EMPTY_PART: PartGroundContact = PartGroundContact {
    in_contact: false, normal: UP, point: Vector3::ZERO, relative_velocity: Vector3::ZERO,
};

#[derive(Clone, Debug, PartialEq)]
pub struct BoardGroundState {
    pub parts: [PartGroundContact; BODY_COUNT],
    pub previous_velocities: [Vector3; BODY_COUNT],
    pub accelerations: [Vector3; BODY_COUNT],
    pub closing_velocity: Vector3,
    pub maximum_closing_speed: f32,
    pub opposing_contact: f32,
    pub surface_twelve_height: f32,
    pub collision_flags: u32,
    pub overall_normal: Vector3,
    pub wheel_normal: Vector3,
    pub valid_wheel_normals: [bool; 4],
    pub part_contact_count: u8,
    pub wheel_contact_count: u8,
    pub time_without_wheel_contact: f32,
    pub wheel_angular_drag: [f32; 4],
}
impl Default for BoardGroundState {
    fn default() -> Self {
        Self {
            parts: [EMPTY_PART; BODY_COUNT], overall_normal: UP, wheel_normal: UP,
            previous_velocities: [Vector3::ZERO; BODY_COUNT],
            accelerations: [Vector3::ZERO; BODY_COUNT],
            closing_velocity: Vector3::ZERO,
            maximum_closing_speed: 0.0,
            opposing_contact: 0.0,
            surface_twelve_height: 0.0,
            collision_flags: 0,
            valid_wheel_normals: [false; 4], part_contact_count: 0,
            wheel_contact_count: 0, time_without_wheel_contact: 0.0, wheel_angular_drag: [0.0; 4],
        }
    }
}
impl BoardGroundState {
    pub fn sample_accelerations(&mut self, velocities: [Vector3; BODY_COUNT], time_step: f32) {
        let inverse_dt = 1.0 / time_step;
        for (i, current) in velocities.into_iter().enumerate() {
            self.accelerations[i] = scale(subtract(current, self.previous_velocities[i]), inverse_dt);
            self.previous_velocities[i] = current;
        }
    }

    pub fn advance_contact_time(&mut self, time_step: f32) {
        self.time_without_wheel_contact = if self.wheel_contact_count == 0 {
            self.time_without_wheel_contact + time_step
        } else { 0.0 };
    }
    pub fn update(
        &mut self,
        reports: &[BoardContactReport],
        lines: &WheelLineState,
        reckoning_up: Vector3,
        maximum_ground_angle_degrees: f32,
        board_wiping_out: bool,
    ) {
        self.parts.fill(EMPTY_PART);
        self.closing_velocity = Vector3::ZERO;
        self.maximum_closing_speed = 0.0;
        self.surface_twelve_height = 0.0;
        self.collision_flags &= 0x01ff_ffff;
        self.overall_normal = UP;
        self.valid_wheel_normals.fill(true);
        let mut highest_y = -2.0;
        let mut highest_part = None;
        let mut minimum_projection = 1.0;
        let mut maximum_projection = -1.0;
        let mut surfaces = [0; BODY_COUNT];
        surfaces[..4].copy_from_slice(&lines.physics_surfaces);
        for report in reports {
            assert_eq!(report.other, CollisionBody::StaticWorld,
                "dynamic object contact classification needs its recovered owner");
            let i = report.part.index();
            if report.part == BodyId::Deck {
                let projection = dot(report.normal, reckoning_up);
                minimum_projection = if minimum_projection - projection >= 0.0 {
                    projection
                } else { minimum_projection };
                maximum_projection = if maximum_projection - projection >= 0.0 {
                    maximum_projection
                } else { projection };
            }
            let closing = -dot(report.normal, self.previous_velocities[i]);
            if !(closing <= self.maximum_closing_speed) {
                self.maximum_closing_speed = closing;
                self.closing_velocity = scale(report.normal, -closing);
            }
            let surface = (u32::from(report.other_surface) >> 7) & 31;
            if surface == 12 {
                self.collision_flags |= 1 << 25;
                self.surface_twelve_height = report.position.y;
            }
            if i >= 4 {
                surfaces[i] = surface;
            }
            let contact = &mut self.parts[i];
            if !contact.in_contact || report.normal.y > contact.normal.y {
                contact.normal = report.normal;
                contact.point = report.position;
                contact.relative_velocity = report.relative_linear_velocity;
            }
            if contact.normal.y > highest_y {
                highest_y = contact.normal.y;
                highest_part = Some(i);
            }
            contact.in_contact = true;
        }
        let range = maximum_projection - minimum_projection;
        self.opposing_contact = if -range >= 0.0 { 0.0 } else { range };
        for (part, surface) in self.parts.iter().zip(surfaces) {
            if part.in_contact && surface == 8 {
                self.collision_flags |= 1 << 31;
            }
        }
        let reference_normal = highest_part.map_or(self.wheel_normal, |i| self.parts[i].normal);
        for i in 0..4 {
            if !self.parts[i].in_contact {
                if lines.distances[i] < f32::from_bits(0x3D8F_5C29) {
                    self.parts[i].normal = lines.normals[i];
                } else {
                    self.valid_wheel_normals[i] = false;
                }
            }
        }
        self.part_contact_count = self.parts.iter().filter(|part| part.in_contact).count() as u8;
        self.wheel_contact_count = self.parts[..4].iter().filter(|part| part.in_contact).count() as u8;
        let angle_radians = maximum_ground_angle_degrees * f32::from_bits(0x3C8E_FA35);
        let minimum_up_dot = trigonometry::cos(angle_radians);
        let mut sum = Vector3::ZERO;
        for i in 0..4 {
            let contact = self.parts[i];
            if self.valid_wheel_normals[i]
                && dot(contact.normal, reckoning_up) > minimum_up_dot
                && angle_between(reference_normal, contact.normal) < f32::from_bits(0x3F49_0FDB)
            {
                sum = add(sum, contact.normal);
            }
            let drag = if contact.in_contact {
                if board_wiping_out { f32::from_bits(0x3D23_D70A) } else { 0.0 }
            } else {
                f32::from_bits(0x3BC4_9BA6)
            };
            self.wheel_angular_drag[i] = drag * f32::from_bits(0x426F_FFFF);
        }
        let squared = dot(sum, sum);
        if self.wheel_contact_count > 0 && squared > f32::from_bits(0x3780_0000) {
            self.overall_normal = scale(sum, inverse_length_squared(squared, 2));
            self.wheel_normal = self.overall_normal;
        } else {
            let mut support = Vector3::ZERO;
            for i in [BodyId::Deck.index(), BodyId::FrontTruck.index(), BodyId::BackTruck.index()] {
                if self.parts[i].in_contact { support = add(support, self.parts[i].normal); }
            }
            let magnitude = length(support);
            if magnitude > f32::from_bits(0x3C23_D70A) {
                let mut inverse = native_arithmetic::reciprocal_estimate(magnitude);
                for _ in 0..2 {
                    let error = (-inverse).mul_add(magnitude, 1.0);
                    inverse = inverse.mul_add(error, inverse);
                }
                self.overall_normal = scale(support, inverse);
            }
        }
    }
}

pub fn angle_between(a: Vector3, b: Vector3) -> f32 {
    let a_squared = dot(a, a);
    let b_squared = dot(b, b);
    let epsilon = f32::from_bits(0x38D1_B717);
    if !(a_squared > epsilon && b_squared > epsilon) { return 0.0; }
    let a = scale(a, inverse_length_squared(a_squared, 1));
    let b = scale(b, inverse_length_squared(b_squared, 1));
    let cosine = native_arithmetic::vector_min(
        native_arithmetic::vector_max(dot(a, b), -1.0), 1.0,
    );
    trigonometry::acos(cosine)
}

