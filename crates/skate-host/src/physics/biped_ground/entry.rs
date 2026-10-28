type Frame = [[f32; 4]; 4];

pub(super) fn effective_root(mut frame: Frame, flags_2476: u32) -> Frame {
    if flags_2476 & 4 != 0 {
        for axis in [0, 2] {
            frame[axis] = frame[axis].map(|v| v * -1.);
        }
    }
    frame
}

