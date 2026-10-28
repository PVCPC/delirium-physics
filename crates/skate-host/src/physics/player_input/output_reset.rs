use skate_core::player::input_phase::*;
const UP: RawVector = [0, 0x3f800000, 0, 0];
pub(crate) fn reset_outputs(out: &mut PhysicalPlayerInput) {
    let surface = out.surface_default_mode;
    let world_grab = out.component_1832_word_1876;
    let anim_to_world = out.skeleton.anim_to_world_11920;
    let handplant_flags = out.air.handplant_flags_324 & !0xb000_0000;
    *out = PhysicalPlayerInput {
        air: AirOutputFields {
            handplant_flags_324: handplant_flags,
            landing_normal_144: UP,
            selected_trajectory_240: [[0; 4], [0; 4], [0; 4], [0xbf80_0000; 4]],
            ..Default::default()
        },
        reckoning: SystemReckoningFields {
            vector_96: UP,
            ..Default::default()
        },
        ground: GroundOutputFields {
            vector_64: UP,
            vector_80: UP,
            vector_96: UP,
            scalar_276: -1.,
            ..Default::default()
        },
        grinds: GrindOutputFields {
            words_136_140: [u32::MAX, 0],
            ..Default::default()
        },
        skeleton: SkeletonOutputFields {
            anim_to_world_11920: anim_to_world,
            ..Default::default()
        },
        surface_default_mode: surface,
        component_1832_word_1876: world_grab,
        ..Default::default()
    };
}
