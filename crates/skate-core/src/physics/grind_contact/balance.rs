use super::{V, admission::EntryKind, arithmetic, cross, dot3, scale, sub};
use crate::air::trajectory::grind_surface::{GeometryType, GrindSurface};
use crate::{point_graph::PointGraph, trigonometry};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BalanceState {
    pub elapsed: f32,
    pub exit_angle_degrees: f32,
    pub entry_delay: f32,
    pub frames_away: i32,
    pub previous_normal: V,
    pub exit_direction: V,
}

impl Default for BalanceState {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            exit_angle_degrees: 0.0,
            entry_delay: 2.0,
            frames_away: 21,
            previous_normal: [0.0, 1.0, 0.0, 0.0],
            exit_direction: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BalanceContact<'a> {
    pub surface: &'a GrindSurface,
    pub primitive_direction: V,
    pub directed_grind_direction: V,
    pub kind: u32,
    pub entry_kind: EntryKind,
}

#[derive(Clone, Copy, Debug)]
pub struct TargetUpInput {
    pub category: u32,
    pub previous_state: u32,
    pub board_up: V,
}

#[derive(Clone, Copy, Debug)]
pub struct ExitLeanInput {
    pub category: u32,
    pub current_state: u32,
    pub grind_substate: u32,
    pub timestep: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BalanceVectors {
    pub grind_normal: V,
    pub target_up: V,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForceExitProbe {
    pub start: V,
    pub end: V,
}

#[derive(Clone, Copy, Debug)]
pub struct ForceExitHit {
    pub normal: V,
}

impl BalanceState {
    pub fn update_target_up(
        &mut self,
        input: TargetUpInput,
        contact: Option<&BalanceContact<'_>>,
        vectors: &mut BalanceVectors,
    ) {
        let Some(contact) = contact else { return };
        let surface = contact.surface;
        let normal = if input.category != 400 {
            input.board_up
        } else if matches!(input.previous_state, 402 | 404) {
            blend(
                self.previous_normal,
                0.85,
                surface.upmost_normal,
                f32::from_bits(0x3e19_9998),
            )
        } else {
            blend(
                self.previous_normal,
                0.925,
                surface.tilted_upmost_normal,
                f32::from_bits(0x3d99_9998),
            )
        };
        let direction = contact.directed_grind_direction;
        let projected = cross(cross(direction, normal), direction);
        let length = magnitude(projected);
        vectors.grind_normal = if length <= 0.001 {
            surface.upmost_normal
        } else {
            scale(projected, arithmetic::reciprocal(length))
        };
        self.previous_normal = vectors.grind_normal;

        let target = blend(
            input.board_up,
            0.8,
            surface.upmost_normal,
            f32::from_bits(0x3e4c_cccc),
        );
        vectors.target_up = scale(target, arithmetic::reciprocal(magnitude(target)));
        let alignment = dot3(vectors.grind_normal, vectors.target_up)
            .max(-1.0)
            .min(1.0);
        let max_angle = f32::from_bits(0x3e7a_35dd);
        if trigonometry::acos(alignment) > max_angle {
            let axis = cross(vectors.grind_normal, vectors.target_up);
            let length = magnitude(axis);
            vectors.target_up = if length <= 0.001 {
                vectors.grind_normal
            } else {
                rotate(
                    scale(axis, arithmetic::reciprocal(length)),
                    vectors.grind_normal,
                    max_angle,
                )
            };
        }
    }

    pub fn update_exit_lean(
        &mut self,
        input: ExitLeanInput,
        contact: Option<&BalanceContact<'_>>,
        exit_lean_angle_vs_time: &PointGraph<8>,
        vectors: &mut BalanceVectors,
    ) -> f32 {
        let active = input.category == 400 || input.current_state == 701 || contact.is_some();
        self.frames_away = if active {
            0
        } else {
            self.frames_away.wrapping_add(1)
        };
        let droppable = contact.is_some_and(|contact| {
            if contact.kind == 2 {
                active && contact.surface.kind != GeometryType::ThinRail
            } else {
                contact.surface.kind == GeometryType::Ledge
            }
        });
        let mut started = false;
        if self.frames_away > 20 {
            self.exit_angle_degrees = 0.0;
            self.elapsed = 0.0;
            self.entry_delay = 2.0;
        } else if self.elapsed > 0.0 || (active && droppable && contact.is_some()) {
            if self.elapsed == 0.0 {
                if let Some(contact) = contact {
                    self.entry_delay = match contact.entry_kind {
                        EntryKind::RideFromBelow => 0.8,
                        EntryKind::RideIntoCoping => 0.1,
                        _ => 2.0,
                    };
                }
                started = true;
            }
            if input.grind_substate != 2 {
                self.elapsed = input.timestep + self.elapsed;
            }
        }
        if self.elapsed <= 0.0 {
            return self.exit_angle_degrees;
        }
        let time = self.elapsed - self.entry_delay;
        self.exit_angle_degrees = if time <= 0.0 {
            0.0
        } else {
            exit_lean_angle_vs_time.evaluate(time)
        };
        if let Some(contact) = contact {
            let surface = contact.surface;
            self.exit_direction = if surface.kind == GeometryType::ThinRail {
                retain_side(
                    self.exit_direction,
                    cross(surface.upmost_normal, contact.primitive_direction),
                )
            } else if started {
                surface.high_side
            } else {
                retain_side(self.exit_direction, surface.high_side)
            };
            if self.exit_angle_degrees > 0.0 {
                let direction = contact.directed_grind_direction;
                let axis = if cross(direction, self.exit_direction)[1] > 0.0 {
                    direction
                } else {
                    negate(direction)
                };
                let radians = self.exit_angle_degrees * f32::from_bits(0x3c8e_fa35);
                vectors.target_up = rotate(axis, vectors.target_up, radians);
                vectors.grind_normal = rotate(axis, vectors.grind_normal, radians);
            }
        }
        self.exit_angle_degrees
    }

    pub fn update_force_exit<E>(
        &self,
        primitive_location: V,
        investigation_flags: &mut u32,
        mut world_line: impl FnMut(ForceExitProbe) -> Result<Option<ForceExitHit>, E>,
    ) -> Result<(), E> {
        let force_exit = if self.exit_angle_degrees > 28.0 {
            true
        } else if self.exit_angle_degrees > 15.0 {
            let start = sub(primitive_location, scale(self.exit_direction, 0.1));
            let end = super::add(start, [0.0, -5.0, 0.0, 0.0]);
            match world_line(ForceExitProbe { start, end })? {
                Some(hit) => dot3(self.exit_direction, hit.normal) > -0.1,
                None => true,
            }
        } else {
            false
        };
        *investigation_flags = (*investigation_flags & 0x7fff_ffff) | ((force_exit as u32) << 31);
        Ok(())
    }
}

fn magnitude(value: V) -> f32 {
    arithmetic::square_root(dot3(value, value))
}

fn blend(a: V, a_weight: f32, b: V, b_weight: f32) -> V {
    core::array::from_fn(|i| a[i].mul_add(a_weight, b[i] * b_weight))
}

fn negate(value: V) -> V {
    value.map(|lane| -lane)
}

fn retain_side(previous: V, side: V) -> V {
    if dot3(previous, side) > 0.0 {
        side
    } else {
        negate(side)
    }
}

fn rotate(axis: V, value: V, angle: f32) -> V {
    let (sin, cos) = trigonometry::sin_cos(angle * 0.5);
    let q = scale(axis, sin);
    let first_cross = cross(q, value);
    let intermediate = core::array::from_fn(|i| value[i].mul_add(cos, first_cross[i]));
    let second_cross = cross(q, intermediate);
    core::array::from_fn(|i| second_cross[i].mul_add(2.0, value[i]))
}

