use crate::physics::SkaterRuntime;

pub(crate) fn publish(skater: &mut SkaterRuntime) {
    if let Some(impulse) = skater.grind.pending_wipeout_impulse.take() {
        let output = &mut skater.player_input.physical.physics;
        output.vector_16 = impulse.map(f32::to_bits);
        output.flag_32 = 1;
    }
}
