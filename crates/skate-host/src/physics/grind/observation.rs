use super::Family;
type V = [f32; 4];

#[derive(Clone, Copy, Debug)]
pub(crate) struct ManagerObservation {
    pub geometry: GeometryObservation,
    pub surface: SurfaceObservation,
    pub control: ControlObservation,
    pub engagement: EngagementObservation,
    pub jumper: JumperObservation,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GeometryObservation {
    pub point_1120: V,
    pub direction_1136: V,
    pub normal_1152: V,
    pub target_up_1168: V,
    pub primitive_start_1264: V,
    pub primitive_end_1280: V,
    pub spline_guids_1296: Option<[u64; 2]>,
    pub upmost_normal_1408: V,
    pub high_side_1440: V,
    pub kind_1464: u32,
    pub flags_1476: u32,
    pub impact_speed_1492: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SurfaceObservation {
    pub audio_surface_1468: u32,
    pub material_1472: u32,
    pub friction_vs_time_1496: f32,
    pub reckon_blend_selector_1500: f32,
    pub gravity_relief_1512: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ControlObservation {
    pub family: Family,
    pub flags_1516: u32,
    pub flags_2468: u32,
    pub flags_2488: u32,
    pub translation_2796: f32,
    pub balance_2800: f32,
    pub exit_lean: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EngagementObservation {
    pub velocity_1184: V,
    pub kind_1248: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct JumperObservation {
    pub geometry_kind_16: u32,
    pub family_20: Family,
    pub energy_24: f32,
    pub high_side_32: V,
    pub normal_48: V,
    pub direction_64: V,
    pub upmost_normal_80: V,
    pub point_96: V,
}
