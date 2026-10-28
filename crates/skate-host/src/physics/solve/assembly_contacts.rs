use skate_core::physics::{
    board_step::{BoardCollision, CollisionBody},
    board_world::BoardWorldVolume,
    contact::{RetailContactInput, combine_contact_materials},
    skeleton_body::SkeletonCollisionMode,
    world_contact::{PrimitivePairSettings, primitive_pair_contacts},
};

pub(super) fn append(
    contacts: &mut Vec<BoardCollision>,
    board: &[BoardWorldVolume],
    rider: &[BoardWorldVolume],
    board_group: u32,
    collision: &SkeletonCollisionMode,
) -> Result<(), String> {
    if board_group >= 21
        || collision.assembly_group >= 21
        || collision.parts.iter().any(|part| part.part_group >= 21)
    {
        return Err("Skater collision group is outside the original21x21 table".into());
    }
    if group_pair_allowed(board_group, collision.assembly_group) {
        for a in board {
            for b in rider {
                let CollisionBody::Attached(part) = b.body else {
                    unreachable!()
                };
                if !group_pair_allowed(board_group, collision.parts[part].part_group) {
                    continue;
                }
                append_pair(contacts, a, b);
            }
        }
    }
    for a in rider {
        let CollisionBody::Attached(part_a) = a.body else {
            unreachable!()
        };
        for b in rider {
            let CollisionBody::Attached(part_b) = b.body else {
                unreachable!()
            };
            if part_a != part_b && !collision.self_culling[part_a][part_b] {
                append_pair(contacts, a, b);
            }
        }
    }
    Ok(())
}

fn group_pair_allowed(a: u32, b: u32) -> bool {
    const ALLOWED: [u32; 21] = [
        0x1876fd, 0, 0x120001, 1, 0x79d1, 0x1079e1, 0x1079f1, 0x279f1, 0x1676f0, 0x7101, 0x5101,
        0x1008f0, 0x1d77f1, 0x233f1, 0xa57f1, 0, 0x1000, 0x6184, 0x1100, 0x5001, 0x1965,
    ];
    b < 21
        && ALLOWED
            .get(a as usize)
            .is_some_and(|row| row & (1 << b) != 0)
}

pub(super) fn append_pair(
    contacts: &mut Vec<BoardCollision>,
    a: &BoardWorldVolume,
    b: &BoardWorldVolume,
) {
    let Some(manifold) = primitive_pair_contacts(
        a.primitive,
        b.primitive,
        PrimitivePairSettings::skater_self_collision(),
    ) else {
        return;
    };
    let material = combine_contact_materials(a.material, b.material);
    for points in &manifold.points[..manifold.count] {
        contacts.push(BoardCollision {
            body_a: a.body,
            body_b: b.body,
            contact: RetailContactInput {
                position_on_a: points.a,
                position_on_b: points.b,
                normal: manifold.normal,
                restitution: material.restitution,
                static_friction: material.static_friction,
                dynamic_friction: material.dynamic_friction,
                tag: 0,
            },
        });
    }
}
