use skate_core::physics::{
    contact::RetailContactMaterial,
    skeleton_body::{SkeletonCollisionSettings, SkeletonFeedbackSettings},
};
use skate_data::{collections::Collections, physics_skeleton::PhysicsSkeleton};
use std::path::Path;

pub(super) fn load(
    asset_root: &Path,
    data: &Collections,
    bank_sha: &str,
) -> Result<SkeletonCollisionSettings, String> {
    let skeleton = PhysicsSkeleton::load(
        &asset_root.join("private/stock/physics-skeletons.json"),
        bank_sha,
        "PHYS_TPOSE",
    )?;
    if skeleton.bones.len() != 24 {
        return Err("Skeleton collision requires24 bone records".into());
    }
    let mut compliant = [false; 24];
    let mut priority = [0.0; 24];
    compliant[0] = true;
    for part in 1..24 {
        let words = data.words::<9>(
            "physics_skeleton_drives",
            "default",
            &format!("PART_{}", skeleton.bones[part].name),
        )?;
        compliant[part] = words[7] >> 24 != 0;
        priority[part] = f32::from_bits(words[8]);
    }
    let friction = data.float("physics_skeleton", "default", "FrictionNormal")?;
    Ok(SkeletonCollisionSettings {
        enabled: data.boolean("physics_skeleton", "default", "EnableCollision")?,
        normal_material: RetailContactMaterial {
            static_friction: friction,
            dynamic_friction: friction,
            restitution: data.float("physics_skeleton", "default", "RestitutionNormal")?,
        },
        compliant,
        priority,
        effect_time: data.float("physics_skeleton", "default", "CollisionEffectTime")?,
    })
}

pub(super) fn feedback(
    data: &Collections,
    body: SkeletonCollisionSettings,
) -> Result<SkeletonFeedbackSettings, String> {
    let value = |name| data.float("physics_collision", "default", name);
    let vector = |name| -> Result<[f32; 4], String> {
        Ok(data
            .words::<4>("physics_collision", "default", name)?
            .map(f32::from_bits))
    };
    Ok(SkeletonFeedbackSettings {
        body,
        small_object_mass: data.float("physics_skeleton", "default", "SmallObjectMassThreshold")?,
        ground_plane_max_distance: value("GroundPlaneMaxDist")?,
        ground_plane_max_angle: value("GroundPlaneMaxAngle")?,
        skater_scalar: value("SkaterSkeletonScalar")?,
        ai_scalar: value("AISkeletonScalar")?,
        groin_offset: vector("GroinLocalOffset")?,
        face_offset: vector("FaceLocalOffset")?,
        groin_radius: value("GroinRadius")?,
        face_radius: value("FaceRadius")?,
    })
}
