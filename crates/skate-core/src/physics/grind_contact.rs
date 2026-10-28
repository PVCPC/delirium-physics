use super::native_arithmetic::dot3;
type V = [f32; 4];
#[path = "grind_contact/families.rs"]
pub mod families;
#[path = "grind_contact/entry.rs"]
pub mod entry;
#[path = "grind_contact/control.rs"]
pub mod control;
#[path = "grind_contact/admission.rs"]
pub mod admission;
#[path = "grind_contact/arithmetic.rs"]
mod arithmetic;
#[path = "grind_contact/investigator.rs"]
pub mod investigator;
#[path = "grind_contact/manager.rs"]
pub mod manager;
#[path = "grind_contact/balance.rs"]
pub mod balance;

#[derive(Clone, Copy, Debug)]
pub struct Primitive {
    pub start: V,
    pub end: V,
    pub owner: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct TruckContact {
    pub position: V,
    pub primitive: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct FiftyFiftyCandidate {
    pub direction: V,
    pub centre: V,
    pub front: V,
    pub rear: V,
    pub primitive: usize,
}

pub fn deck_contact(
    board: [V; 4],
    flags_2484: u32,
    above: f32,
    below: f32,
    deck_center_to_truck: f32,
    primitives: &[Primitive],
) -> Option<TruckContact> {
    if flags_2484 & 0x0020_0000 != 0 {
        return None;
    }
    let centre = add(board[3], scale(board[1], above));
    let along = scale(board[2], deck_center_to_truck);
    let down = scale(board[1], -(above + below));
    let a = add(centre, along);
    let b = sub(centre, along);
    let c = add(a, down);
    let d = add(b, down);
    let mut best = None;
    let mut distance = 1_000_000.0;
    for (primitive, edge) in primitives.iter().enumerate() {
        let first = segment_triangle(edge.start, edge.end, [a, b, c]);
        let second = segment_triangle(edge.start, edge.end, [d, b, c]);
        if let Some(position) = first.or(second) {
            let delta = sub(position, board[3]);
            let square = dot3(delta, delta);
            if square < distance {
                distance = square;
                best = Some(TruckContact {
                    position,
                    primitive,
                });
            }
        }
    }
    best
}

pub fn boardslide_candidate(
    board: [V; 4],
    contact: TruckContact,
    edge: Primitive,
    velocity: V,
    category: u32,
    manager_frames: i32,
    flags_2476: u32,
    deck_center_to_truck: f32,
    truck_to_wheel: f32,
    admission: &admission::Admission<'_>,
) -> Option<FiftyFiftyCandidate> {
    if flags_2476 & 0x4000_0000 != 0 {
        return None;
    }
    let delta = sub(edge.end, edge.start);
    let direction = scale(delta, arithmetic::reciprocal(arithmetic::square_root(dot3(delta, delta))));
    if dot3(upright_normal(direction), board[1]) <= 0.65 {
        return None;
    }
    if !admission.test(400, direction, false).allowed { return None; }
    let depth = dot3(sub(board[3], contact.position), board[1]);
    let projected = scale(board[1], depth);
    if dot3(projected, projected) >= f32::from_bits(0x3b6b_edfa) {
        return None;
    }
    if category == 100 && dot3(velocity, direction).abs() <= 0.75 && manager_frames <= 10 {
        return None;
    }
    let horizontal = sub(delta, scale(board[1], dot3(delta, board[1])));
    let length = arithmetic::square_root(dot3(horizontal, horizontal));
    if !(length > 0.0) {
        return None;
    }
    let horizontal = scale(horizontal, arithmetic::reciprocal(length));
    let alignment = dot3(horizontal, board[2]).abs();
    let from = sub(contact.position, board[3]);
    let across = sub(from, scale(board[1], dot3(from, board[1])));
    let remaining = deck_center_to_truck - arithmetic::square_root(dot3(across, across));
    let clearance = if alignment > 0.0 {
        remaining / alignment * arithmetic::square_root(1.0 - alignment * alignment)
    } else {
        999.0
    };
    if !(clearance > truck_to_wheel) {
        return None;
    }
    Some(FiftyFiftyCandidate {
        direction,
        centre: contact.position,
        front: edge.end,
        rear: edge.start,
        primitive: contact.primitive,
    })
}

pub fn fifty_fifty_candidate(
    board: [V; 4],
    balance: [f32; 2],
    contacts: [Option<TruckContact>; 2],
) -> Option<FiftyFiftyCandidate> {
    fifty_fifty_candidate_on_splines(board, balance, contacts, &[])
}

pub fn fifty_fifty_candidate_on_splines(
    board: [V; 4],
    balance: [f32; 2],
    contacts: [Option<TruckContact>; 2],
    primitives: &[Primitive],
) -> Option<FiftyFiftyCandidate> {
    let [Some(front), Some(rear)] = contacts else {
        return None;
    };
    if balance[0].abs().max(balance[1].abs()) >= 0.9 {
        return None;
    }
    if front.primitive != rear.primitive {
        let edge = primitives.get(front.primitive)?;
        let delta = sub(edge.end, edge.start);
        let direction = scale(delta, arithmetic::inverse_square_root(dot3(delta, delta)));
        let across = cross(upright_normal(direction), direction);
        if dot3(sub(front.position, rear.position), across) > 0.1 {
            return None;
        }
    }
    let front_depth = dot3(sub(board[3], front.position), board[1]);
    let rear_depth = dot3(sub(board[3], rear.position), board[1]);
    if front_depth.max(rear_depth) >= 0.13 {
        return None;
    }
    let difference = sub(front.position, rear.position);
    Some(FiftyFiftyCandidate {
        direction: scale(difference, arithmetic::inverse_square_root(dot3(difference, difference))),
        centre: scale(add(front.position, rear.position), 0.5),
        front: front.position,
        rear: rear.position,
        primitive: front.primitive,
    })
}

pub fn upright_normal(direction: V) -> V {
    let cross_up = cross([0.0, 1.0, 0.0, 0.0], direction);
    let normal = cross(cross_up, direction);
    let mut length = arithmetic::square_root(dot3(normal, normal));
    if length <= 0.0 {
        return [1.0, 0.0, 0.0, 0.0];
    }
    if normal[1] < 0.0 {
        length = -length;
    }
    scale(normal, arithmetic::reciprocal(length))
}

pub fn within_approach_angle(direction: V, velocity: V, degrees: f32) -> bool {
    let normal = upright_normal(direction);
    let projected = sub(velocity, scale(normal, dot3(velocity, normal)));
    let length = arithmetic::square_root(dot3(projected, projected));
    if length <= 0.001 {
        return true;
    }
    let alignment = dot3(scale(projected, arithmetic::reciprocal(length)), direction).abs();
    alignment > crate::trigonometry::cos(degrees * f32::from_bits(0x3c8e_fa35))
}

pub fn truck_contacts(
    board: [V; 4],
    flags_2484: u32,
    truck_to_wheel: f32,
    deck_center_to_truck: f32,
    primitives: &[Primitive],
) -> [Option<TruckContact>; 2] {
    if flags_2484 & 0x0020_0000 != 0 {
        return [None; 2];
    }
    let [right, up, forward, position] = board;
    let centre = core::array::from_fn(|i| up[i].mul_add(-0.02, position[i]));
    let side = scale(right, truck_to_wheel);
    let along = scale(forward, deck_center_to_truck);
    let down = scale(up, -0.2);
    let centres = [add(centre, along), sub(centre, along)];
    let mut result = [None; 2];
    let mut distance = [1_000_000.0; 2];
    for (primitive, edge) in primitives.iter().enumerate() {
        for truck in 0..2 {
            let a = add(centres[truck], side);
            let b = sub(centres[truck], side);
            let c = add(b, down);
            let d = add(a, down);
            let first = segment_triangle(edge.start, edge.end, [a, b, c]);
            let second = segment_triangle(edge.start, edge.end, [a, d, c]);
            if let Some(position) = first.or(second) {
                let delta = sub(position, centres[truck]);
                let squared = dot3(delta, delta);
                if distance[truck] > squared {
                    distance[truck] = squared;
                    result[truck] = Some(TruckContact {
                        position,
                        primitive,
                    });
                }
            }
        }
    }
    result
}

pub fn segment_triangle(start: V, end: V, triangle: [V; 3]) -> Option<V> {
    let [a, b, c] = triangle;
    let ac = sub(c, a);
    let ab = sub(b, a);
    let direction = sub(end, start);
    let normal = cross(ab, ac);
    let denominator = -dot3(direction, normal);
    if denominator == 0.0 {
        return None;
    }
    let from = sub(start, a);
    let inverse = 1.0 / denominator;
    let t = inverse * dot3(from, normal);
    if t < 0.0 || t > 1.0 {
        return None;
    }
    let side = cross(from, direction);
    let u = inverse * dot3(ac, side);
    let v = (1.0 / -denominator) * dot3(ab, side);
    if u + v > 1.0 || u < 0.0 || v < 0.0 {
        return None;
    }
    Some(core::array::from_fn(|i| direction[i].mul_add(t, start[i])))
}

fn sub(a: V, b: V) -> V {
    core::array::from_fn(|i| a[i] - b[i])
}
fn add(a: V, b: V) -> V {
    core::array::from_fn(|i| a[i] + b[i])
}
fn scale(a: V, s: f32) -> V {
    a.map(|v| v * s)
}
fn cross(a: V, b: V) -> V {
    [
        (-a[2]).mul_add(b[1], a[1] * b[2]),
        (-a[0]).mul_add(b[2], a[2] * b[0]),
        (-a[1]).mul_add(b[0], a[0] * b[1]),
        0.0,
    ]
}

