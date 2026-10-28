use skate_core::player::input_phase::ProcessedPhysicsInput;

pub(crate) fn reset_processed(input: &mut ProcessedPhysicsInput) {
    let external = input.external_physics_1616;
    let vector_832 = input.vectors_720_784_800_816_832_864[4];
    let mut reset = ProcessedPhysicsInput {
        effective_anim_transform_192: [
            [1.0f32.to_bits(), 0, 0, 0],
            [0, 1.0f32.to_bits(), 0, 0],
            [0, 0, 1.0f32.to_bits(), 0],
            [0; 4],
        ],
        vector_1520: input.vector_1520,
        matrix_1536: input.matrix_1536,
        byte_1600: input.byte_1600,
        external_physics_1616: external,
        probe_1792: input.probe_1792,
        flags_2468: 0x2000 | (input.flags_2468 & 8),
        flags_2488: input.flags_2488 & 0x001f_ffff,
        state_variant_index_2528: 1,
        grind_words_2532_2536: [u32::MAX, 0],
        timestep_2604: f32::from_bits(0x3c88_8889),
        gravity_2648: f32::from_bits(0xc11c_cccd),
        actor_query_2952: u32::MAX,
        ..ProcessedPhysicsInput::default()
    };
    reset.external_physics_1616.flags &= 0x01ff_ffff;
    reset.vectors_544_560_592_608[0] = [0, 1.0f32.to_bits(), 0, 0];
    reset.vectors_720_784_800_816_832_864[4] = vector_832;
    *input = reset;
}

