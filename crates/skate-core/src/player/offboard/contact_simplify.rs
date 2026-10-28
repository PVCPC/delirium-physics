use super::{
    contact_queries::Input,
    contact_records::Record,
    contact_segments::{length_inverse, reciprocal},
};
use crate::physics::native_arithmetic::dot3;

pub struct Reduction {
    pub forward_limit: f32,
    pub inactive_records: Vec<Record>,
}
pub fn simplify(input: Input, records: &mut Vec<Record>) -> Reduction {
    let mut limit = f32::from_bits(0x5015_02f9);
    let original = records.len();
    if original < 2 {
        return Reduction {
            forward_limit: limit,
            inactive_records: Vec::new(),
        };
    }
    let sentinel = records[original - 1].distance + 1.;
    let mut retained = original;
    for i in 1..original - 1 {
        if records[i].flags & 8 != 0 {
            retained -= original - i - 1;
            limit = dot3(
                std::array::from_fn(|k| records[i].position[k] - input.position[k]),
                input.surface_forward,
            );
            break;
        }
        let a = records[i - 1].coordinates;
        let b = records[i].coordinates;
        let c = records[i + 1].coordinates;
        let incoming = [b[0] - a[0], b[1] - a[1], 0., 0.];
        let outgoing = [c[0] - b[0], c[1] - b[1], 0., 0.];
        let in_length = length_inverse(incoming).0;
        let out_length = length_inverse(outgoing).0;
        let mut remove = in_length < f32::from_bits(0x3ca3_d70a);
        if !remove && !(out_length < f32::from_bits(0x3ca3_d70a)) {
            if !(dot3(incoming, outgoing) < 0. && dot3(incoming, input.surface_up) >= 0.) {
                let a = incoming.map(|v| v * reciprocal(in_length));
                let b = outgoing.map(|v| v * reciprocal(out_length));
                let cross = [
                    (-a[2]).mul_add(b[1], a[1] * b[2]),
                    (-a[0]).mul_add(b[2], a[2] * b[0]),
                    (-a[1]).mul_add(b[0], a[0] * b[1]),
                    (-a[3]).mul_add(b[3], a[3] * b[3]),
                ];
                remove = !(length_inverse(cross).0 >= f32::from_bits(0x3d4c_cccd));
            }
        }
        if remove {
            records[i].distance = sentinel;
            retained -= 1;
        }
    }
    super::contact_sort::sort(records);
    let inactive_records = records.split_off(retained);
    Reduction {
        forward_limit: limit,
        inactive_records,
    }
}

