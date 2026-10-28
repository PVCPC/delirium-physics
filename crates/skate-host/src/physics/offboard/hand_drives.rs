use skate_core::{
    math::{Basis3, Vector3},
    physics::{
        assembly::BodySnapshot,
        board::BodyId,
        board_step::ATTACHED_REACTION_BASE,
        drive_frames::{RetailDriveFrame, RetailDriveFrames, retail_quaternion_from_basis},
        drive_parameters::{RetailDriveDynamics, RetailDriveParams, RetailDriveType},
        drive_solver::{RetailDriveBodyState, RetailDriveRows, build_drive_rows},
        rigid_body::pack_world_inverse_inertia,
        skeleton_body::prepare_bone_drive_frames,
    },
    player::offboard::board_possession::{Frame, State},
};

pub(crate) fn append(
    rows: &mut Vec<RetailDriveRows>,
    state: &State,
    board: &[BodySnapshot; 7],
    skeleton: &[BodySnapshot; 26],
    dt: f32,
) {
    for (hand, part) in [3, 7].into_iter().enumerate() {
        let drive = state.hands[hand];
        let a = skeleton[part];
        let b = board[BodyId::Deck.index()];
        if (a.state_flags | b.state_flags) & 4 == 0 {
            continue;
        }
        let parameters = |words: [u32; 4]| RetailDriveParams {
            spring_or_max_velocity: f32::from_bits(words[0]),
            damping: f32::from_bits(words[1]),
            max_strength: f32::from_bits(words[2]),
            drive_type: match words[3] {
                1 => RetailDriveType::SoftDrive,
                2 => RetailDriveType::HardDrive,
                _ => RetailDriveType::NoDrive,
            },
        };
        rows.push(build_drive_rows(
            body(a, ATTACHED_REACTION_BASE + part),
            body(b, BodyId::Deck.index()),
            prepare_bone_drive_frames(RetailDriveFrames {
                body_a: frame(drive.child),
                body_b: frame(drive.parent),
            }),
            RetailDriveDynamics {
                linear: parameters(drive.dynamics[0]),
                angular: parameters(drive.dynamics[1]),
            },
            dt,
        ));
    }
}
fn frame(f: Frame) -> RetailDriveFrame {
    RetailDriveFrame {
        orientation: retail_quaternion_from_basis(Basis3 {
            columns: std::array::from_fn(|i| [f[i][0], f[i][1], f[i][2]]),
        }),
        translation: Vector3::new(f[3][0], f[3][1], f[3][2]),
    }
}
fn body(b: BodySnapshot, index: usize) -> RetailDriveBodyState {
    let r = b.rates;
    RetailDriveBodyState {
        reaction_index: index,
        state: b.state_flags,
        orientation: r.orientation,
        basis: r.basis,
        center_of_mass: r.position,
        linear_velocity: r.linear_velocity,
        angular_velocity: r.angular_velocity,
        force_acceleration: r.force_acceleration,
        torque_acceleration: r.torque_acceleration,
        inverse_mass: b.inertia.inverse_mass,
        world_inverse_inertia: pack_world_inverse_inertia(r.world_inverse_inertia),
    }
}

