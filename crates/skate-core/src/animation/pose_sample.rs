use super::{output::Sqt, pose_blend::blend_sample};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSelection {
    pub first: usize,
    pub second: usize,
    pub coefficient: f32,
}

pub fn select_frames(
    time: f32,
    fps: f32,
    frames: usize,
    blend_frames: bool,
    truncation_offset: f32,
) -> Result<FrameSelection, &'static str> {
    if frames == 0 || !time.is_finite() || time < 0.0 || !fps.is_finite() || fps <= 0.0 {
        return Err("Invalid stock animation sampling input");
    }
    let last = (frames - 1) as f32;
    let position = time * fps;
    let position = if position - last >= 0.0 {
        last
    } else {
        position
    };
    let offset = if blend_frames { 0.0 } else { truncation_offset };
    let first = (position + offset) as usize;
    if first >= frames {
        return Err("Stock frame selection exceeds decoded samples");
    }
    let second = if blend_frames {
        (first + 1).min(frames - 1)
    } else {
        first
    };
    let fraction = ((position - first as f32) * 65535.0) as u16;
    Ok(FrameSelection {
        first,
        second,
        coefficient: f32::from(fraction) * f32::from_bits(0x3780_0080),
    })
}

pub fn sample_key(first: Sqt, second: Sqt, selection: FrameSelection) -> Sqt {
    if selection.first == selection.second {
        first
    } else {
        blend_sample(first, second, selection.coefficient)
    }
}

pub fn sample_key_vbr(first: Sqt, second: Sqt, selection: FrameSelection) -> Sqt {
    let crosses_block = selection.first / 8 != selection.second / 8;
    let copies_second = crosses_block && (selection.first % 8 == 7 || selection.second % 8 == 0);
    sample_key(
        if copies_second { second } else { first },
        second,
        selection,
    )
}

