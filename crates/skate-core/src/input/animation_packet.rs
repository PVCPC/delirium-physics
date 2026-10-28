pub struct AnimationPacketFields {
    pub stance_byte: u8,
    pub timestep: f32,
    pub scalar_10388: f32,
    pub flags_10375_10496_10784: [u8; 3],
    pub vector_10480: [u32; 4],
    pub matrix_10704: [[u32; 4]; 4],
    pub byte_10768: u8,
    pub truck_tightness: f32,
    pub scalar_10792: f32,
    pub flag_10371: u8,
}

pub struct ProcessedPacketFields {
    pub flags_2468: u32,
    pub flags_2476: u32,
    pub timestep: f32,
    pub scalar_2668: f32,
    pub vector_1520: [u32; 4],
    pub matrix_1536: [[u32; 4]; 4],
    pub byte_1600: u8,
    pub truck_tightness: f32,
    pub scalar_2764: f32,
}

pub fn publish(source: &AnimationPacketFields, output: &mut ProcessedPacketFields) {
    replace_bit(&mut output.flags_2468, 20, source.stance_byte);
    output.timestep = source.timestep;
    output.scalar_2668 = source.scalar_10388;
    replace_bit(&mut output.flags_2468, 3, source.flags_10375_10496_10784[0]);
    replace_bit(&mut output.flags_2468, 2, source.flags_10375_10496_10784[1]);
    output.vector_1520 = source.vector_10480;
    replace_bit(&mut output.flags_2468, 1, source.flags_10375_10496_10784[2]);
    output.matrix_1536 = source.matrix_10704;
    output.byte_1600 = source.byte_10768;
    output.truck_tightness = source.truck_tightness;
    output.scalar_2764 = source.scalar_10792;
    replace_bit(&mut output.flags_2476, 20, source.flag_10371);
}

fn replace_bit(word: &mut u32, bit: u32, byte: u8) {
    *word = (*word & !(1 << bit)) | (u32::from(byte & 1) << bit);
}
