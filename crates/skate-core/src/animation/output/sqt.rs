use super::{NativeMatrix, PoseBufferError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sqt {
    pub scale: [f32; 4],
    pub rotation: [f32; 4],
    pub translation: [f32; 4],
}

pub fn sqt_to_local(
    bone_count: usize,
    cache_offset: u8,
    source: &[Sqt],
    destination: &mut [NativeMatrix],
) -> Result<usize, PoseBufferError> {
    if cache_offset >= 128 || cache_offset % 16 != 0 {
        return Err(PoseBufferError::InvalidCacheOffset);
    }
    let padded = bone_count
        .checked_add(3)
        .ok_or(PoseBufferError::CountOverflow)?
        & !3;
    if source.len() < padded {
        return Err(PoseBufferError::ShortInput);
    }
    let trailing_words = if padded == 0 {
        0
    } else {
        (128 - usize::from(cache_offset)) / 4
    };
    let required = padded
        .checked_add(trailing_words.div_ceil(16))
        .ok_or(PoseBufferError::CountOverflow)?;
    if destination.len() < required {
        return Err(PoseBufferError::ShortOutput);
    }
    for word in destination[padded..required]
        .iter_mut()
        .flatten()
        .flatten()
        .take(trailing_words)
    {
        *word = 0.0;
    }
    for (input, output) in source[..padded].iter().zip(&mut destination[..padded]) {
        *output = sqt_to_matrix(*input);
    }
    Ok(padded)
}

pub fn sqt_to_matrix(input: Sqt) -> NativeMatrix {
        let [x, y, z, w] = input.rotation;
        let [sx, sy, sz, _] = input.scale;
        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let wz = w * z;
        let xz = x * z;
        let wy = w * y;
        let yz = y * z;
        let wx = w * x;
        [
            [
                (-(yy + zz)).mul_add(2.0, 1.0) * sx,
                ((xy + wz) * 2.0) * sx,
                ((xz - wy) * 2.0) * sx,
                0.0,
            ],
            [
                ((xy - wz) * 2.0) * sy,
                (-(xx + zz)).mul_add(2.0, 1.0) * sy,
                ((yz + wx) * 2.0) * sy,
                0.0,
            ],
            [
                ((xz + wy) * 2.0) * sz,
                ((yz - wx) * 2.0) * sz,
                (-(xx + yy)).mul_add(2.0, 1.0) * sz,
                0.0,
            ],
            [
                input.translation[0],
                input.translation[1],
                input.translation[2],
                1.0,
            ],
        ]
}
