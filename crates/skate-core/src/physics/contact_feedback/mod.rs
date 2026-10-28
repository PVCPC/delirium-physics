pub(crate) mod board_reports;
mod collision_info;
mod compression;
mod reports;
mod spy;

pub use board_reports::BoardContactReport;
pub use collision_info::{choose_physics_surface, choose_surface, reset_frame_collision_info};
pub use compression::{
    average_wheel_compressions, calculate_wheel_compressions, part_mass_transform,
};
pub use reports::{
    ContactReportBackend, ContactReportBuffer, ContactReportDispatchBackend, ContactTarget,
    create_contact_reports, get_contact_reports, resolve_contact_reports,
};
pub use spy::spy_contact_jacobians;

pub type CollisionInfo = [u8; 812];
