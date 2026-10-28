use skate_core::animation::{
    output::attributes::{AnimationAttribute, AttributeName, MotionGraphAttribute},
    playback_parameters::{AttributeSink, SettableAttribute},
};

pub trait Animation: AttributeSink {
    fn apply_parameters(&mut self) -> Result<(), String>;
    fn query_current_attribute(
        &self,
        name: AttributeName,
        mask: u32,
        output: &mut AnimationAttribute,
    ) -> Result<bool, String>;
    fn emit_packet(&mut self, name: AttributeName, value: f32);
    fn intent(&self, name: &str) -> Option<f32>;
}

pub(super) fn set(
    animation: &mut impl AttributeSink,
    name: AttributeName,
    value: f32,
    normalized: bool,
) {
    animation.set_attribute(SettableAttribute {
        name,
        value,
        normalized,
        sequence_id: -1,
    });
}

pub fn endpoints(animation: &mut impl Animation, name: AttributeName) -> Result<[f32; 2], String> {
    let mut bounds = [0.0; 2];
    for (i, bound) in bounds.iter_mut().enumerate() {
        set(animation, name, i as f32, true);
        animation.apply_parameters()?;
        let mut attribute = MotionGraphAttribute { name, value: 0.0 }.to_animation();
        animation.query_current_attribute(name, 15, &mut attribute)?;
        *bound = f32::from_bits(
            attribute.payload.0[0].ok_or("Grind twist query returned an uninitialized scalar")?,
        );
    }
    Ok(bounds)
}
