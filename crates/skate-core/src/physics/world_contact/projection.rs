use super::arithmetic::{dot, scale, vector};
use super::separating_axis_candidates;

#[derive(Clone, Copy, Debug)]
pub enum PrimitiveKind {
    Sphere,
    Capsule,
    Triangle,
    Box,
}

pub fn project_direction(
    gp: &[u32; 48],
    kind: PrimitiveKind,
    direction: [u32; 4],
    output: &mut [u32; 12],
) {
    project(gp, kind, direction, output, false);
}

pub fn project_directions(
    gp: &[u32; 48],
    kind: PrimitiveKind,
    directions: &[[u32; 4]],
    output: &mut [[u32; 12]],
) {
    assert!(output.len() >= directions.len());
    for (&direction, interval) in directions.iter().zip(output) {
        project(gp, kind, direction, interval, true);
    }
}

fn project(
    gp: &[u32; 48],
    kind: PrimitiveKind,
    direction: [u32; 4],
    output: &mut [u32; 12],
    batch: bool,
) {
    let direction = vector(&direction, 0);
    let center = dot(vector(gp, 0), direction);
    let (min, max) = match kind {
        PrimitiveKind::Sphere => (center, center),
        PrimitiveKind::Capsule => {
            let radius = dot(vector(gp, 16), direction).abs() * f32::from_bits(gp[28]);
            (center - radius, center + radius)
        }
        PrimitiveKind::Triangle => {
            let second = dot(vector(gp, 8), direction);
            let third = dot(vector(gp, 12), direction);
            let min = if center < second { center } else { second };
            let max = if center > second { center } else { second };
            (
                if min < third { min } else { third },
                if max > third { max } else { third },
            )
        }
        PrimitiveKind::Box => {
            let extent: [f32; 3] = std::array::from_fn(|i| {
                let axis = vector(gp, 4 + i * 4);
                let half = f32::from_bits(gp[28 + i]);
                if batch {
                    dot(scale(axis, half), direction).abs()
                } else {
                    (dot(axis, direction) * half).abs()
                }
            });
            let radius = (extent[0] + extent[1]) + extent[2];
            (center - radius, center + radius)
        }
    };
    output[..4].fill(min.to_bits());
    output[4..8].fill(max.to_bits());
}

pub fn best_separating_direction(
    a: &[u32; 48],
    a_kind: PrimitiveKind,
    b: &[u32; 48],
    b_kind: PrimitiveKind,
) -> ([u32; 4], [u32; 4]) {
    let mut directions = [[0; 4]; 16];
    let mut count = separating_axis_candidates(a, b, &mut directions);
    if count == 0 {
        directions[0] = [0, 0, 1.0f32.to_bits(), 0];
        count = 1;
    }
    let mut a_intervals = [[0; 12]; 16];
    let mut b_intervals = [[0; 12]; 16];
    project_directions(a, a_kind, &directions[..count], &mut a_intervals);
    project_directions(b, b_kind, &directions[..count], &mut b_intervals);
    let mut selected = 0;
    let mut best = 0.0;
    let mut flip = false;
    for index in 0..count {
        let forward = f32::from_bits(a_intervals[index][0]) - f32::from_bits(b_intervals[index][4]);
        let reverse = f32::from_bits(b_intervals[index][0]) - f32::from_bits(a_intervals[index][4]);
        let (separation, reversed) = if forward > reverse {
            (forward, true)
        } else {
            (reverse, false)
        };
        if index == 0 || separation > best {
            best = separation;
            selected = index;
            flip = reversed;
        }
    }
    let mut normal = directions[selected];
    if flip {
        normal.iter_mut().for_each(|v| *v ^= 0x8000_0000);
    }
    ([best.to_bits(); 4], normal)
}
