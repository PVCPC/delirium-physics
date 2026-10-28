use skate_core::player::offboard::ground_sync::BoardSettings;
use skate_data::collections::Collections;
pub(super) fn load(data: &Collections) -> Result<BoardSettings, String> {
    let float = |name| data.float("physics_state_offboard", "default", name);
    let vector = |name| -> Result<[f32; 4], String> {
        let field = data.field("physics_state_offboard", "default", name)?;
        if field.type_name != "Math::Vector3" {
            return Err(format!("{name}: expected stock Vector3"));
        }
        let value = data
            .words::<4>("physics_state_offboard", "default", name)?
            .map(f32::from_bits);
        if value.iter().any(|v| !v.is_finite()) {
            return Err(format!("{name}: non-finite vector"));
        }
        Ok(value)
    };
    Ok(BoardSettings {
        extent_0: vector("GrabBoxSizeGrabbing")?,
        extent_16: vector("GrabBoxSize")?,
        offset_32: vector("GrabBoxOffset")?,
        angle_436: float("GrabSplineMaxAngleToHorizontalGrabbing")?,
        angle_440: float("GrabSplineMaxAngleToHorizontal")?,
        margin_444: float("GrabSplineEndExclusion")?,
        angle_452: float("GrabSplineAngleLimitGrabbing")?,
        angle_456: float("GrabSplineAngleLimit")?,
    })
}
