use super::{contact_queries::{Input, V, accepts_side_support},
    contact_records::{Records, Source, Direction}, controller::Frame};
use crate::physics::native_arithmetic::dot3;

#[derive(Clone, Copy, Debug)]
pub struct SupportHit {
    pub position: V,
    pub normal: V,
    pub frame: Frame,
    pub support_id: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Packet {
    pub position: V,
    pub normal: V,
    pub support_frame: Frame,
    pub target_position: V,
    pub target_normal: V,
    pub edge_position: V,
    pub edge_normal: V,
    pub value_160: f32,
    pub kind_164: u32,
    pub distance_168: f32,
    pub distance_172: f32,
    pub flags: u32,
    pub support_id: u32,
}
impl Default for Packet {
    fn default() -> Self {
        Self { position: [0.;4], normal: [0.,1.,0.,0.],
            support_frame: [[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.;4]],
            target_position: [0.;4], target_normal: [0.,1.,0.,0.],
            edge_position: [0.;4], edge_normal: [0.;4], value_160: 0., kind_164: 0,
            distance_168: f32::from_bits(0x5015_02f9), distance_172: f32::from_bits(0x5015_02f9),
            flags: 0, support_id: 0 }
    }
}
impl Packet {
    pub fn consume_support(&mut self, input: Input, hits: [Option<SupportHit>;3],
        previous_flags_30416: u32, records: &mut Records) -> [bool;3]
    {
        let mut accepted = [false;3];
        if let Some(hit) = hits[0] {
            let mut projected_normal = hit.normal;
            records.insert(input,hit.position,&mut projected_normal,Source::Support,Direction::None,-1.);
            self.position = hit.position;
            self.normal = hit.normal;
            self.support_frame = hit.frame;
            self.flags |= 1;
            let delta = std::array::from_fn(|i| hit.position[i] - input.position[i]);
            let height = dot3(input.surface_up,delta);
            if !(height >= f32::from_bits(0xbdcc_cccd)) {
                self.flags |= 8;
                if previous_flags_30416 & 64 != 0 { self.flags |= 64; }
            } else { self.flags &= !8; }
            self.support_id = hit.support_id;
            accepted[0] = true;
        }
        for i in 1..3 {
            if let Some(hit) = hits[i] {
                if accepts_side_support(input,hit.position) {
                    accepted[i] = true;
                    self.flags |= if i == 1 { 16 } else { 32 };
                    if hits[0].is_some() && hit.normal[1] > self.normal[1] {
                        self.normal = hit.normal;
                    }
                }
            }
        }
        accepted
    }
}

