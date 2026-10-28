use std::f32::consts::{FRAC_2_PI, FRAC_PI_4, TAU};

pub fn vunit3(v: [f32; 3]) -> [f32; 3] {
    let n = (((0.0 + v[0] as f64 * v[0] as f64) + v[1] as f64 * v[1] as f64) + v[2] as f64 * v[2] as f64) as f32;
    let mut e = crate::estimate::vrsqrtefp(n);
    if e == f32::INFINITY {
        e = 0.0;
    }
    let sq = e * e;
    let half = 0.5 * e;
    let t = -n.mul_add(sq, -1.0);
    let y = t.mul_add(half, e);
    v.map(|c| c * y)
}

pub fn mover_anim(pos: [f32; 3], velocity: [f32; 3], forward: [f32; 3], ped: [f32; 3], heading: f32, dive: bool) -> Option<&'static crate::runanim::Row> {
    let way = vunit3([pos[0] - ped[0], pos[1] - ped[1], pos[2] - ped[2]]);
    let v = velocity;
    let motion = if v[0].mul_add(v[0], v[1].mul_add(v[1], v[2] * v[2])) > 0.5 { v } else { forward };
    let rightward = way[1].mul_add(motion[0], -way[0] * motion[1]) > 0.0;
    let mut a = crate::fall::bearing([way[0], way[1]], heading) + FRAC_PI_4;
    if a < 0.0 {
        a += TAU;
    }
    let q = a * FRAC_2_PI;
    let id = match if q.is_nan() { -1 } else { q as i32 } {
        0 => 0xAF + rightward as i32,
        1 => 0xB4 - rightward as i32,
        2 => 0xB2 - rightward as i32,
        3 => 0xB5 + rightward as i32,
        _ => return None,
    };
    let rows = if dive { &(*crate::runanim::EV_DIVES()) } else { &(*crate::runanim::AVOIDS()) };
    rows.get((id - 0xAF) as usize)
}

#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn whizzes(start: [f32; 3], end: [f32; 3], root: [f32; 3]) -> bool {
    let u = vunit3(std::array::from_fn(|l| end[l] - start[l]));
    let to: [f32; 3] = std::array::from_fn(|l| root[l] - start[l]);
    let t = to[0].mul_add(u[0], to[1].mul_add(u[1], to[2] * u[2]));
    if !(t > 0.0) {
        return false;
    }
    let off: [f32; 3] = std::array::from_fn(|l| root[l] - u[l].mul_add(t, start[l]));
    off[1].mul_add(off[1], off[0] * off[0]) < crate::events::WHIZZED_REACH * crate::events::WHIZZED_REACH
}
