use skate_core::player::offboard::controller::ClipMetric;
use skate_data::animation_metadata::{AnimationMetadata, ClipMetadata};
const CLIPS: [&str; 3] = ["NB_WALK_FWD_CYC", "NB_RUN_FWD_CYC", "NB_SPRINT_FWD_CYC"];
pub(super) fn load(metadata: &AnimationMetadata) -> Result<[Option<ClipMetric>; 3], String> {
    let one = |name| -> Result<Option<ClipMetric>, String> {
        let clip = metadata.clip(name)?;
        let source = metadata
            .source_for(name)
            .ok_or_else(|| format!("Missing bank identity for {name}"))?;
        if !source.source_bank.eq_ignore_ascii_case("OffBoard.abin") {
            return Err(format!("{name}: expected actual OffBoard.abin source"));
        }
        metric(clip)
    };
    Ok([one(CLIPS[0])?, one(CLIPS[1])?, one(CLIPS[2])?])
}
pub(super) fn metric(clip: &ClipMetadata) -> Result<Option<ClipMetric>, String> {
    let Some(attribute) = clip.attributes.iter().find(|a| a.name == "ANIMTRANSZ") else {
        return Ok(None);
    };
    if attribute.type_id != 0 {
        return Err(format!(
            "{}: AnimTransZ expected scalar stock record",
            clip.name
        ));
    }
    let payload = attribute
        .payload_words
        .first()
        .ok_or_else(|| format!("{}: truncated AnimTransZ", clip.name))?;
    let translation_z = f32::from_bits(*payload);
    let length = f32::from_bits(clip.frames_bits) / f32::from_bits(clip.fps_bits);
    let end_time = f32::from_bits(attribute.end_bits) * length;
    if !translation_z.is_finite() || !end_time.is_finite() || end_time == 0.0 {
        return Err(format!("{}: invalid AnimTransZ metric", clip.name));
    }
    Ok(Some(ClipMetric {
        translation_z,
        end_time,
    }))
}
