#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AngleError {
    IntegerConversionUnavailable,
}

pub fn normalize(angle: f32) -> Result<f32, AngleError> {
    let pi = f32::from_bits(0x4049_0fdb);
    if angle >= -pi && angle < pi {
        return Ok(angle);
    }
    let turns = angle * f32::from_bits(0x3e22_f983);
    if !turns.is_finite() || turns < i32::MIN as f32 || turns as f64 > i32::MAX as f64 {
        return Err(AngleError::IntegerConversionUnavailable);
    }
    let whole_turns = turns as i32;
    let tau = f32::from_bits(0x40c9_0fdb);
    let reduced = -((whole_turns as f32).mul_add(tau, -angle));
    Ok(if reduced >= pi {
        reduced - tau
    } else if reduced < -pi {
        reduced + tau
    } else {
        reduced
    })
}
