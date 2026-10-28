use super::{GamePhysics, SkaterRuntime, ground_runtime::GroundInputObservations};
use skate_core::{
    math::Vector3,
    physics::{force_queue::QueuedPointForce, manual::controller},
    point_graph::PointGraph,
    riding::{anti_flip, collision_response::signed_angle, pumping},
};
use skate_data::collections::Collections;
type V = [f32; 4];
pub(crate) struct RevertState {
    speed: PointGraph<8>,
    tolerance: PointGraph<4>,
    gain: f32,
    delay: f32,
    duration: f32,
    direction: f32,
    travel_sign: f32,
    velocity: V,
    elapsed: f32,
    captured: bool,
    pub active: bool,
}
impl RevertState {
    pub fn load(d: &Collections) -> Result<Self, String> {
        let class = "Hash_018E8A2E5028AB3F";
        let a = d
            .words::<20>(class, "default", "Hash_DAA990E803A782A1")?
            .map(f32::from_bits);
        let b = d
            .words::<12>(class, "default", "Hash_3387562E9BEA0AF6")?
            .map(f32::from_bits);
        Ok(Self {
            speed: PointGraph {
                x: a[4..12].try_into().unwrap(),
                y: a[12..20].try_into().unwrap(),
            },
            tolerance: PointGraph {
                x: b[4..8].try_into().unwrap(),
                y: b[8..12].try_into().unwrap(),
            },
            gain: d.float(class, "default", "Hash_0E5B939FD4ECDBAA")?,
            delay: d.float(class, "default", "Hash_6BF6544475D2A5AD")?,
            duration: d.float(class, "default", "Hash_035D9805EB9E2EE2")?,
            direction: 0.0,
            travel_sign: 1.0,
            velocity: [0.0; 4],
            elapsed: 0.0,
            captured: false,
            active: false,
        })
    }
    fn correction(&mut self, forward: V, normal: V, angular: V) -> V {
        if !self.captured {
            return [0.0; 4];
        }
        let desired = forward.map(|v| v * self.travel_sign);
        let project = |v: V| {
            let d = dot(v, normal);
            v.map(|x| x - x * d)
        };
        let from = project(self.velocity);
        let to = project(desired);
        if dot(from, from) * dot(to, to) <= f32::from_bits(0x37800000) {
            return [0.0; 4];
        }
        let normalize = |v: V| {
            let length = dot(v, v).sqrt();
            v.map(|x| x / length)
        };
        let mut angle = signed_angle(xyz(normalize(from)), xyz(normalize(to)), xyz(normal));
        if angle >= std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        if angle * self.direction > 0.0 {
            angle -= self.direction * std::f32::consts::TAU;
        }
        let target = self.speed.evaluate(angle.abs()) * self.direction;
        if angle.abs() < self.tolerance.evaluate(normal[1]) * f32::from_bits(0x3c8efa35)
            || angle.abs() > 4.5
        {
            self.active = false;
        }
        let current = dot(angular, normal);
        let correction = if current * target > 0.0 && current.abs() > target.abs() {
            0.0
        } else {
            (target - current) * self.gain
        };
        normal.map(|v| v * correction)
    }
}
pub(super) fn enter(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    skater.ground_lifecycle.skeleton_elapsed_16505 = true;
    physics
        .board
        .hook_mut()
        .drive
        .disable_animation(&mut skater.ground_lifecycle.board_animated_290);
    let r = &mut skater.revert_state;
    r.direction = skater.animation_input.extra.revert_direction;
    r.velocity = [0.0; 4];
    r.elapsed = 0.0;
    r.captured = false;
    r.active = true;
    bevy::log::info!(tick = physics.ticks, direction = r.direction,
        speed = skater.player_input.processed.scalar_2656,
        "REVERT_ENTER");
    skater.wipeout.state.enter_ground();
    Ok(())
}
pub(super) fn update(physics: &mut GamePhysics, skater: &mut SkaterRuntime) -> Result<(), String> {
    let p = &skater.player_input.processed;
    let t = skater
        .player_input
        .toolkit
        .as_ref()
        .ok_or("Revert requires current BoardToolkit")?;
    let r = &mut skater.revert_state;
    if !r.captured && r.elapsed >= r.delay {
        r.captured = true;
        r.velocity = p.vectors_400_416[0].map(f32::from_bits);
        r.travel_sign = if dot(r.velocity, t.deck[2]) > 0.0 {
            -1.0
        } else {
            1.0
        };
    }
    physics.riding.update_slide_reckoning(
        p,
        t,
        skater.animation_input.extra.physical_body_spin,
        skater.animation_input.fields.balance,
    );
    super::input_phase::update_ground(physics, skater)?;
    let p = &skater.player_input.processed;
    let t = skater
        .player_input
        .toolkit
        .as_ref()
        .ok_or("Revert requires current BoardToolkit")?;
    skater.ground.pumping_settings.update_revert(
        &mut skater.ground.pumping,
        t,
        &physics.riding,
        &skater.animated_skeleton,
        p,
        skater.animation_input.fields.balance,
    )?;
    let input = skater.ground_settings.input(
        t,
        p,
        &skater.animation_input,
        &skater.ground.pumping,
        skater
            .ground
            .pumping_settings
            .mode(p.state_variant_index_2528)?
            .unintentional_scalar,
        &physics.riding,
        &skater.animated_skeleton,
        physics.settings.step.base_truck_transforms,
        GroundInputObservations {
            manual_drag_2724: skater.ground_lifecycle.manual_drag_2724,
            trajectory_state_bits: p.external_physics_1616.flags,
            edge_flags: 0,
            edge_point: [0.0; 4],
        },
    );
    let settings = skater.ground_settings.board();
    let correction = skater.revert_state.correction(
        t.deck[2],
        p.vectors_464_480_496_512_528[0].map(f32::from_bits),
        p.vectors_720_784_800_816_832_864[0].map(f32::from_bits),
    );
    for body in physics.board.bodies_mut() {
        body.inertia.linear_drag = 0.0;
    }
    let anti_flip = anti_flip::calculate(settings.anti_flip, &input.anti_flip);
    let pump = pumping::calculate(&input.pump_force);
    let manual = controller::calculate(
        &mut skater.ground.manual,
        settings.manual,
        settings.manual_mode,
        &input.manual,
        &mut skater.ground_runtime,
    )
    .map_err(|e| format!("Revert manual: {e:?}"))?;
    for value in [correction, manual.angular_displacement, anti_flip] {
        skater
            .ground_runtime
            .apply_angular_displacement(&mut physics.board, value);
    }
    physics.board.forces_mut().append(QueuedPointForce {
        tag: 8,
        force_world: Vector3::new(pump[0], pump[1], pump[2]),
        point_body: Vector3::new(pump[4], pump[5], pump[6]),
    });
    skater.revert_state.elapsed += p.timestep_2604;
    if skater.revert_state.elapsed > skater.revert_state.duration {
        skater.revert_state.active = false;
    }
    Ok(())
}
fn dot(a: V, b: V) -> f32 {
    a[2].mul_add(b[2], a[1].mul_add(b[1], a[0] * b[0]))
}
fn xyz(v: V) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}

