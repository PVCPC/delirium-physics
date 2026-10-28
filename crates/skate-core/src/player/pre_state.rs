#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreStatePacket {
    pub words: [u32; 18],
}

impl PreStatePacket {
    pub const ZERO: Self = Self { words: [0; 18] };

    pub const fn vector_48(self) -> [u32; 4] {
        [
            self.words[12],
            self.words[13],
            self.words[14],
            self.words[15],
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreStatePlayerFields {
    pub frame_counter_1312: u32,
    pub component_1840_present: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreStateSkeletonFields {
    pub predicted_position_16112: [u32; 4],
    pub predicted_position_set_16416: bool,
    pub nested_flag_3184: bool,
}

pub trait PreStateServices {
    fn fill_packet_vtable_24(&mut self, packet: &mut PreStatePacket);
    fn update_component_1840_n17(&mut self);
    fn update_before_state_vtable_4(&mut self);
}

pub fn run_pre_state(
    player: &mut PreStatePlayerFields,
    skeleton: &mut PreStateSkeletonFields,
    services: &mut impl PreStateServices,
) {
    player.frame_counter_1312 = player.frame_counter_1312.wrapping_add(1);

    let mut packet = PreStatePacket::ZERO;
    services.fill_packet_vtable_24(&mut packet);
    skeleton.predicted_position_16112 = packet.vector_48();
    skeleton.predicted_position_set_16416 = true;
    skeleton.nested_flag_3184 = false;

    if player.component_1840_present {
        services.update_component_1840_n17();
    }
    services.update_before_state_vtable_4();
}

