mod record;
pub use record::SkeletonPhysicalRecord;
mod construction;
pub use construction::{BoneSettings, HatGeometry, SkeletonBodyDefinition, SkeletonBodySettings, SkeletonPart};
mod runtime;
pub use runtime::SkeletonBody;
mod joints;
pub use joints::{JointBone, JointSettings, SkeletonJointSettings, SkeletonJoint, SkeletonJoints};
mod drive_dynamics;
pub use drive_dynamics::{AnimationDriveSettings, DriveInterpolation, BoneDriveSettings, BoneDriveDynamics};
mod drive_frames;
pub use drive_frames::{bone_drive_frames, prepare_bone_drive_frames};
mod targets;
pub use targets::{SkeletonTargets, TARGET_COUNT};
mod target_update;
pub use target_update::{SkeletonTargetInput, SkeletonTargetUpdate, ExtraTargetPositions};
mod drives;
pub use drives::{SkeletonDriveSettings, BoneDrives, SkeletonDrives, SkeletonDriveIdentity, SkeletonDriveBatch};
mod collision_mode;
pub use collision_mode::{SkeletonCollisionSettings, SkeletonPartCollision, SkeletonCollisionMode};
mod collision_feedback;
mod collision_update;
mod collision_vector;
mod collision_filter;
pub use collision_feedback::{SkeletonFeedbackSettings, SkeletonContactBody,
    SkeletonContactReport, SkeletonCollisionInput, BoneContact, ContactRegion,
    ContactPlane, SpecificContact, SkeletonContactFlags, SkeletonCollisionFeedback};
mod errors;
pub use errors::{SkeletonPoseErrors, SkeletonNormalError};

pub const PART_COUNT: usize = 26;
pub const ANIMATION_PART_COUNT: usize = 24;
pub const JOINT_COUNT: usize = 22;
