use super::{
    super::{GamePhysics, SkaterRuntime, grind_chromosome::Input},
    Family,
};
use skate_core::physics::board::BodyId;

pub(crate) fn condition(physics: &GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    let out = &mut skater.player_input.physical.grinds;
    out.grinding_316 = u8::from(matches!(out.words_136_140[1], 1 | 2));
    let family = match out.words_136_140[0] {
        0 => Some(Family::FiftyFifty),
        1 => Some(Family::Boardslide),
        2 => Some(Family::Tipslide),
        3 => Some(Family::FiveO),
        4 => Some(Family::Backslash),
        5 => Some(Family::Darkslide),
        _ => None,
    };
    if out.grinding_316 != 0 && family.is_none() {
        return Err(format!(
            "Active grind has invalid native family {}",
            out.words_136_140[0]
        ));
    }
    let deck = physics.board.part_transforms()[BodyId::Deck.index()];
    let axis = |a: [f32; 3]| [a[0], a[1], a[2], 0.0];
    let effective_forward = axis(physics.riding.motion.effective_basis.columns[2]);
    let com = physics.board.bodies()[BodyId::Deck.index()].rates.position;
    let raw = |v: [u32; 4]| v.map(f32::from_bits);
    let input = Input {
        category: skater.player_input.physical.state.category_12,
        grinding_316: out.grinding_316 != 0,
        air_event_439: skater.player_input.processed.flags_2468 & 0x0020_0000 != 0,
        family,
        basic_right_0: axis(deck.basis.columns[0]),
        basic_forward_32: axis(deck.basis.columns[2]),
        basic_position_48: [
            deck.translation.x,
            deck.translation.y,
            deck.translation.z,
            0.0,
        ],
        basic_location_axis_96: effective_forward,
        basic_twist_axis_128: effective_forward,
        basic_location_position_144: [com.x, com.y, com.z, 0.0],
        feet_256_272: [
            skater.skeleton.record.pose[19][0],
            skater.skeleton.record.pose[15][0],
        ],
        fakie_155: skater.animation.packet.riding_fakie,
        point: raw(out.point_16),
        direction: raw(out.direction_0),
        normal: raw(out.normal_32),
        across: raw(out.across_48),
    };
    skater.grind.condition_outputs(input, out)
}
