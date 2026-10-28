use crate::math::Vector3;

use super::{
    assembly::{BoardConstraints, BoardHook, BodySnapshot, prepare_drive_frames},
    board::{BODY_COUNT, BodyId},
    contact::{RetailContactBodyState, RetailContactInput, generate_contact},
    contact_feedback::{BoardContactReport, board_reports},
    contact_solver::{RetailContactJacobian, build_contact_jacobian},
    drive_frames::RetailAffineTransform,
    drive_parameters::RetailDriveDynamics,
    drive_solver::RetailDriveRows,
    force_queue::BoardForceQueue,
    point_force::RetailForceAccumulator,
    rigid_body::{
        RetailReactionCorrections, RetailSimulationStep, integrate_body_rates,
        pack_world_inverse_inertia,
    },
    solver::{JointConstraint, solve_constraints},
};

pub const ATTACHED_REACTION_BASE: usize = BODY_COUNT + 1;

pub struct AttachedStep<'a> {
    pub bodies: Vec<&'a mut BodySnapshot>,
    pub contacts: &'a mut [RetailContactJacobian],
    pub joints: &'a mut [JointConstraint],
    pub drives: &'a mut [RetailDriveRows],
}
impl AttachedStep<'_> {
    pub fn world_reaction(&self) -> usize {
        ATTACHED_REACTION_BASE + self.bodies.len()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BoardCollision {
    pub body_a: CollisionBody,
    pub body_b: CollisionBody,
    pub contact: RetailContactInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionBody {
    Board(BodyId),
    Attached(usize),
    StaticWorld,
}

impl CollisionBody {
    pub fn contact_id(self) -> u32 {
        match self {
            Self::Board(id) => id.index() as u32,
            Self::Attached(index) => u32::try_from(ATTACHED_REACTION_BASE + index)
                .expect("attached contact body index exceeds host storage"),
            Self::StaticWorld => u32::MAX,
        }
    }

    pub fn from_contact_id(id: u32) -> Self {
        if id == u32::MAX {
            Self::StaticWorld
        } else if id < BODY_COUNT as u32 {
            Self::Board(BodyId::ORDER[id as usize])
        } else {
            assert!(id >= ATTACHED_REACTION_BASE as u32, "board target has no collision volume");
            Self::Attached(id as usize - ATTACHED_REACTION_BASE)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BoardStepSettings {
    pub simulation: RetailSimulationStep,
    pub iterations: u32,
    pub base_truck_transforms: [RetailAffineTransform; 2],
    pub truck_dynamics: RetailDriveDynamics,
    pub force_point_y_offset: f32,
}

#[derive(Default)]
pub struct BoardStep {
    contacts: Vec<RetailContactJacobian>,
    reports: Vec<BoardContactReport>,
    reactions: Vec<RetailReactionCorrections>,
    pub diagnostic_capture: bool,
    diagnostic_tick: u64,
    pub diagnostic_snapshot: Option<String>,
}

impl BoardStep {
    pub fn contact_reports(&self) -> &[BoardContactReport] {
        &self.reports
    }
    pub fn solved_contacts(&self) -> &[RetailContactJacobian] {
        &self.contacts
    }
    pub fn advance(
        &mut self,
        bodies: &mut [BodySnapshot; BODY_COUNT],
        hook: &mut BoardHook,
        forces: &BoardForceQueue,
        collisions: &[BoardCollision],
        truck_targets: [f32; 2],
        settings: BoardStepSettings,
    ) {
        self.advance_attached(
            bodies,
            hook,
            forces,
            collisions,
            truck_targets,
            settings,
            AttachedStep {
                bodies: Vec::new(),
                contacts: &mut [],
                joints: &mut [],
                drives: &mut [],
            },
        );
    }

    pub fn advance_attached(
        &mut self,
        bodies: &mut [BodySnapshot; BODY_COUNT],
        hook: &mut BoardHook,
        forces: &BoardForceQueue,
        collisions: &[BoardCollision],
        truck_targets: [f32; 2],
        settings: BoardStepSettings,
        mut attached: AttachedStep<'_>,
    ) {
        let mut simulation = settings.simulation;
        assert!(simulation.time_step.is_finite() && simulation.time_step > 0.0);
        simulation.frequency = 1.0 / simulation.time_step;
        for collision in collisions {
            assert_ne!(collision.body_a, collision.body_b);
        }

        apply_deck_forces(bodies, forces, settings.force_point_y_offset);
        let frames = prepare_drive_frames(settings.base_truck_transforms, truck_targets, hook);
        let world_reaction = attached.world_reaction();
        self.reactions
            .resize(world_reaction + 1, RetailReactionCorrections::default());
        self.reactions.fill(RetailReactionCorrections::default());
        self.contacts.clear();
        self.reports.clear();
        if bodies
            .iter()
            .chain(core::iter::once(&hook.body))
            .chain(attached.bodies.iter().map(|body| &**body))
            .all(|body| body.state_flags & 4 == 0)
        {
            return;
        }
        for collision in collisions {
            let contact = generate_contact(
                collision.contact,
                contact_body(collision.body_a, bodies, &attached.bodies, world_reaction),
                contact_body(collision.body_b, bodies, &attached.bodies, world_reaction),
            );
            self.contacts
                .push(build_contact_jacobian(contact, simulation.time_step));
        }
        let mut constraints = BoardConstraints::build(
            bodies,
            hook,
            frames,
            settings.truck_dynamics,
            simulation.time_step,
        );
        let board_contacts = self.contacts.len();
        let board_joints = constraints.joints.len();
        let board_drives = constraints.drives.len();
        self.contacts.extend_from_slice(attached.contacts);
        constraints.joints.extend_from_slice(attached.joints);
        constraints.drives.extend_from_slice(attached.drives);
        solve_constraints(
            &mut self.contacts,
            &mut constraints.joints,
            &mut constraints.drives,
            &mut self.reactions,
            settings.iterations,
        );
        self.diagnostic_tick = self.diagnostic_tick.wrapping_add(1);
        if self.diagnostic_capture && self.diagnostic_tick % 6 == 0 {
            let deck = BodyId::Deck.index();
            self.diagnostic_snapshot = Some(format!(
                "tick={} body={:?} hook={:?} reaction={:?} joints={:?} drives={:?}",
                self.diagnostic_tick, bodies[deck], hook, self.reactions[deck],
                constraints.joints.iter().enumerate()
                    .filter(|(_, j)| j.reaction_a == deck || j.reaction_b == deck)
                    .map(|(i, j)| (i, j.reaction_a, j.reaction_b, j.jacobian.words.map(f32::from_bits)))
                    .collect::<Vec<_>>(),
                constraints.drives.iter().enumerate()
                    .filter(|(_, d)| d.frame_a_body.reaction_index == deck || d.frame_b_body.reaction_index == deck)
                    .collect::<Vec<_>>(),
            ));
        }
        for (body, reaction) in bodies
            .iter_mut()
            .chain(core::iter::once(&mut hook.body))
            .chain(attached.bodies.iter_mut().map(|body| &mut **body))
            .zip(&mut self.reactions)
        {
            if body.state_flags & 4 != 0 {
                body.rates =
                    integrate_body_rates(body.rates, body.inertia, simulation, *reaction).state;
            }
            *reaction = RetailReactionCorrections::default();
        }
        board_reports::collect(
            &mut self.reports,
            &self.contacts,
            bodies,
            simulation.frequency,
        );
        attached
            .contacts
            .clone_from_slice(&self.contacts[board_contacts..]);
        attached
            .joints
            .clone_from_slice(&constraints.joints[board_joints..]);
        attached
            .drives
            .clone_from_slice(&constraints.drives[board_drives..]);
    }
}

fn apply_deck_forces(
    bodies: &mut [BodySnapshot; BODY_COUNT],
    forces: &BoardForceQueue,
    y_offset: f32,
) {
    let deck = &mut bodies[BodyId::Deck.index()];
    let result = forces.apply_to_deck(
        RetailForceAccumulator {
            force_acceleration: deck.rates.force_acceleration,
            torque_acceleration: deck.rates.torque_acceleration,
            cool_down: deck.rates.cool_down,
        },
        deck.rates.basis,
        deck.inertia.inverse_mass,
        deck.rates.world_inverse_inertia,
        y_offset,
    );
    deck.rates.force_acceleration = result.force_acceleration;
    deck.rates.torque_acceleration = result.torque_acceleration;
    deck.rates.cool_down = result.cool_down;
}

fn contact_body(
    body: CollisionBody,
    bodies: &[BodySnapshot; BODY_COUNT],
    attached: &[&mut BodySnapshot],
    world_reaction: usize,
) -> RetailContactBodyState {
    if body == CollisionBody::StaticWorld {
        return RetailContactBodyState {
            contact_body_id: u32::MAX,
            reaction_id: world_reaction as u32,
            center_of_mass: Vector3::ZERO,
            inverse_inertia_full: Vector3::ZERO,
            inverse_inertia_split: Vector3::ZERO,
            inverse_mass: 0.0,
            state: 0,
            force_acceleration: Vector3::ZERO,
            torque_acceleration: Vector3::ZERO,
            linear_velocity: Vector3::ZERO,
            angular_velocity: Vector3::ZERO,
            kinetic_energy: 0.0,
            cool_down: 0,
        };
    };
    let contact_id = body.contact_id();
    let body = match body {
        CollisionBody::Board(id) => &bodies[id.index()],
        CollisionBody::Attached(index) => &*attached[index],
        CollisionBody::StaticWorld => unreachable!(),
    };
    let rates = body.rates;
    let inertia = pack_world_inverse_inertia(rates.world_inverse_inertia);
    RetailContactBodyState {
        contact_body_id: contact_id,
        reaction_id: contact_id,
        center_of_mass: rates.position,
        inverse_inertia_full: inertia.full,
        inverse_inertia_split: inertia.split,
        inverse_mass: body.inertia.inverse_mass,
        state: body.state_flags | 8,
        force_acceleration: rates.force_acceleration,
        torque_acceleration: rates.torque_acceleration,
        linear_velocity: rates.linear_velocity,
        angular_velocity: rates.angular_velocity,
        kinetic_energy: rates.kinetic_energy,
        cool_down: rates.cool_down,
    }
}

