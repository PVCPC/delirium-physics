use skate_core::{physics::board_dynamic_normal::DynamicNormalSettings, point_graph::PointGraph};
use skate_data::collections::Collections;
pub(crate) fn settings(data: &Collections) -> Result<DynamicNormalSettings, String> {
    let f = |field| data.float("physics_reckoning", "default", field);
    let values = data
        .words::<16>("physics_reckoning", "default", "DynamicSpeedMaxDeltaGraphZ")?
        .map(f32::from_bits);
    Ok(DynamicNormalSettings {
        speed_damping: f("DynamicSpeedDamping")?,
        up_vector_damping: f("DynamicUpVectorDamping")?,
        maximum_delta: f("DynamicSpeedMaxDelta")?,
        speed_scale: f("DynamicSpeedMaxDeltaXScale")?,
        maximum_delta_vs_speed: PointGraph {
            x: values[..8].try_into().unwrap(),
            y: values[8..].try_into().unwrap(),
        },
    })
}
