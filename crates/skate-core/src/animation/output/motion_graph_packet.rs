#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrickIdentifier(pub [u32; 6]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrickData {
    pub identifiers: [TrickIdentifier; 2],
    pub word48: u32,
    pub scalars52_56: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GestureData {
    pub gesture: u32,
    pub flag_bytes: [u8; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionGraphPacket {
    pub trick: TrickData,
    pub gesture: GestureData,
    pub wipeout_gesture: [f32; 2],
}

pub fn publish(source: &MotionGraphPacket, destination: &mut MotionGraphPacket) {
    destination.trick = source.trick;
    destination.gesture.gesture = source.gesture.gesture;
    destination.gesture.flag_bytes = source.gesture.flag_bytes;
    destination.wipeout_gesture[0] = source.wipeout_gesture[0];
    destination.wipeout_gesture[1] = source.wipeout_gesture[1];
}
