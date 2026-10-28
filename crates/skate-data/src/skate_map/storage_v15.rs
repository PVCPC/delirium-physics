use super::{Reader, StoredBlock};

pub(super) fn decode(method: u32, bytes: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    let mut input = Reader { bytes, at: 0 };
    let size = input.u()? as usize;
    if size > expected.saturating_mul(3).saturating_add(16) || size > 2_147_483_648 {
        return Err("SKATE transformed block exceeds size limit".into());
    }
    let data = StoredBlock {
        expected: size, method: if method <= 6 { 2 } else { 1 },
        bytes: &bytes[4..],
    }.decode()?;
    let mut source = Reader { bytes: &data, at: 0 };
    let mut result = Vec::new();
    match method {
        3 | 7 => {
            let width = source.u()? as usize;
            let height = source.u()? as usize;
            let row = width.checked_mul(4).ok_or("SKATE filtered width overflow")?;
            if row.checked_mul(height) != Some(expected) || width > 16384 || height > 16384 {
                return Err("SKATE filtered dimensions mismatch".into());
            }
            result.resize(expected, 0u8);
            for y in 0..height {
                let filter = source.take(1)?[0];
                if filter > 4 { return Err("Invalid SKATE RGBA filter".into()); }
                for (x, &value) in source.take(row)?.iter().enumerate() {
                    let at = y * row + x;
                    let left = if x >= 4 { result[at - 4] as i32 } else { 0 };
                    let above = if y > 0 { result[at - row] as i32 } else { 0 };
                    let upper_left = if x >= 4 && y > 0 { result[at - row - 4] as i32 } else { 0 };
                    let predictor = match filter {
                        1 => left, 2 => above, 3 => (left + above) / 2,
                        4 => {
                            let p = left + above - upper_left;
                            let a = (p - left).abs();
                            let b = (p - above).abs();
                            let c = (p - upper_left).abs();
                            if a <= b && a <= c { left } else if b <= c { above } else { upper_left }
                        }
                        _ => 0,
                    };
                    result[at] = value.wrapping_add(predictor as u8);
                }
            }
        }
        4 | 8 => {
            if expected % 56 != 0 || data.len() != expected { return Err("Invalid SKATE vertex streams".into()); }
            result.resize(expected, 0);
            for (offset, length) in [(0,12), (12,12), (24,8), (32,8), (40,4), (44,8), (52,4)] {
                for vertex in 0..expected / 56 {
                    let at = vertex * 56 + offset;
                    result[at..at + length].copy_from_slice(source.take(length)?);
                }
            }
        }
        5 | 9 => {
            if expected % 4 != 0 { return Err("Invalid SKATE index block size".into()); }
            let mut previous = 0i64;
            for _ in 0..expected / 4 {
                let mut zigzag = 0u64;
                for shift in (0..=28).step_by(7) {
                    let byte = source.take(1)?[0];
                    if shift == 28 && byte > 31 { return Err("Invalid SKATE index varint".into()); }
                    zigzag |= u64::from(byte & 127) << shift;
                    if byte & 128 == 0 { break; }
                }
                let current = previous + ((zigzag >> 1) as i64 ^ -((zigzag & 1) as i64));
                let current = u32::try_from(current).map_err(|_| "SKATE index delta overflow")?;
                result.extend(current.to_le_bytes());
                previous = i64::from(current);
            }
        }
        6 | 10 => {
            if expected % 48 != 0 { return Err("Invalid SKATE collision block size".into()); }
            let count = source.u()? as usize;
            let vertices = source.take(count.checked_mul(12).ok_or("SKATE collision vertex overflow")?)?;
            for _ in 0..expected / 48 {
                for _ in 0..3 {
                    let index = source.u()? as usize;
                    if index >= count { return Err("Invalid SKATE collision vertex reference".into()); }
                    result.extend_from_slice(&vertices[index * 12..index * 12 + 12]);
                }
                result.extend_from_slice(source.take(12)?);
            }
        }
        _ => return Err("Invalid SKATE storage transform".into()),
    }
    if source.at != data.len() || result.len() != expected { return Err("SKATE transformed size mismatch or trailing data".into()); }
    Ok(result)
}

