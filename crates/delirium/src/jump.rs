use crate::nm::{Agent, ConfigureBalance};
use crate::prelude::*;

#[allow(non_snake_case)]
#[inline]
fn SOFT() -> f32 { delirium_data::kv!(5) }
#[allow(non_snake_case)]
#[inline]
fn CROUCH_LEGS() -> f32 { delirium_data::kv!(84) }
#[allow(non_snake_case)]
#[inline]
fn SINK() -> f32 { delirium_data::kv!(12) }
#[allow(non_snake_case)]
#[inline]
fn SINK_TIME() -> f32 { delirium_data::kv!(130) }
#[allow(non_snake_case)]
#[inline]
fn CROUCH_DEPTH() -> f32 { delirium_data::kv!(9) }
#[allow(non_snake_case)]
#[inline]
fn CROUCH_TIME() -> f32 { delirium_data::kv!(24) }
#[allow(non_snake_case)]
#[inline]
fn PUSH_TIME() -> f32 { delirium_data::kv!(78) }
#[allow(non_snake_case)]
#[inline]
fn STIFF() -> f32 { delirium_data::kv!(139) }
#[allow(non_snake_case)]
#[inline]
fn PUSH_LEGS() -> f32 { delirium_data::kv!(25) }
#[allow(non_snake_case)]
#[inline]
fn APEX() -> f32 { delirium_data::kv!(140) }
#[allow(non_snake_case)]
#[inline]
fn DIVE_APEX() -> f32 { delirium_data::kv!(9) }
#[allow(non_snake_case)]
#[inline]
fn DIVE_SPEED() -> f32 { delirium_data::kv!(186) }
#[allow(non_snake_case)]
#[inline]
fn DIVE_SPIN() -> f32 { delirium_data::kv!(84) }
#[allow(non_snake_case)]
#[inline]
fn NO_TAKEOFF() -> f32 { delirium_data::kv!(24) }
#[allow(non_snake_case)]
#[inline]
fn LAND_SPEED() -> f32 { delirium_data::kv!(5) }
#[allow(non_snake_case)]
#[inline]
fn DOWN() -> f32 { delirium_data::kv!(24) }
#[allow(non_snake_case)]
#[inline]
fn FLIGHT() -> f32 { delirium_data::kv!(12) }
#[allow(non_snake_case)]
#[inline]
fn OFF() -> f32 { delirium_data::kv!(131) }
#[allow(non_snake_case)]
#[inline]
fn FEET() -> &'static [usize; 2] { static V: std::sync::OnceLock<[usize; 2]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(0 int), delirium_data::kv!(6 int)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn LEGS() -> &'static std::ops::RangeInclusive<usize> { static V: std::sync::OnceLock<std::ops::RangeInclusive<usize>> = std::sync::OnceLock::new(); let f = || 1..=delirium_data::kv!(6 int); if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

pub struct Jump {
    pub dive: bool,
    phase: Phase,
    t: f32,
    dir: Vec3,
    feet: f32,
    base: f32,
    top: f32,
}

#[derive(PartialEq, Clone, Copy)]
enum Phase {
    Load,
    Push,
    Flight,
    Land,
}

#[derive(Default)]
pub struct Said {
    pub catch_fall: bool,
    pub log: Option<String>,
}

impl Jump {
    pub fn start(a: &mut Agent, dive: bool, toward: Option<Vec3>) -> Option<Jump> {
        if !a.balancing() {
            return None;
        }
        let f: Vec<Transform> = a.doll.frames().collect();
        let dir = toward.unwrap_or_else(|| Vec3::Z.cross(f[delirium_data::kv!(1 int)].translation - f[1].translation).with_z(0.0).normalize_or(Vec3::X));
        a.configure_balance(ConfigureBalance { leg_stiffness: CROUCH_LEGS(), ..Default::default() });
        let base = a.doll.com().0.z;
        let feet = (*FEET()).map(|k| f[k].translation.z).into_iter().fold(f32::MAX, f32::min);
        Some(Jump { dive, phase: Phase::Load, t: 0.0, dir, feet, base, top: base })
    }

    pub fn frame(&mut self, a: &mut Agent, dt: f32, said: &mut Said) -> bool {
        let j = self;
        j.t += dt;
        let (com, v) = a.doll.com();
        j.top = j.top.max(com.z);
        let f: Vec<Transform> = a.doll.frames().collect();
        let low = (*FEET()).map(|k| f[k].translation.z).into_iter().fold(f32::MAX, f32::min);
        match j.phase {
            Phase::Load => {
                a.set_stiffness_mask(SOFT(), 0.5, "lb");
                if j.t <= SINK_TIME() {
                    let dv = Vec3::NEG_Z * SINK() * (dt / SINK_TIME());
                    for (k, x) in f.iter().enumerate().filter(|(k, _)| !(*LEGS()).contains(k)) {
                        let m = a.doll.part_mass(k);
                        a.doll.impulse(k, x.translation, dv * m);
                    }
                }
                if j.base - com.z >= CROUCH_DEPTH() || j.t >= CROUCH_TIME() {
                    said.log = Some(format!("jump  loaded: {:.2} m down in {:.2} s", j.base - com.z, j.t));
                    a.set_stiffness_mask(STIFF(), 1.0, "lb");
                    a.configure_balance(ConfigureBalance { leg_stiffness: PUSH_LEGS(), ..Default::default() });
                    (j.phase, j.t) = (Phase::Push, 0.0);
                }
            }
            Phase::Push => {
                let up = (2.0 * delirium_data::kv!(34) * if j.dive { DIVE_APEX() } else { APEX() }).sqrt();
                let on = if j.dive { DIVE_SPEED() } else { 0.0 };
                let gap = Vec3::Z * (up - v.z).max(0.0) + j.dir * (on - v.dot(j.dir)).max(0.0);
                let left = ((PUSH_TIME() - j.t) / dt).max(0.0) + 1.0;
                if j.t <= PUSH_TIME() + dt * 0.5 {
                    let dv = gap / left;
                    let spin = if j.dive { Vec3::Z.cross(j.dir) * DIVE_SPIN() * dt / (PUSH_TIME() + dt) } else { Vec3::ZERO };
                    for (k, x) in f.iter().enumerate() {
                        let m = a.doll.part_mass(k);
                        a.doll.impulse(k, x.translation, (dv + spin.cross(x.translation - com)) * m);
                    }
                }
                if low > j.feet + OFF() {
                    said.log = Some(format!("jump  takeoff at {:.2} s: up {:.2} m/s, on {:.2} m/s", j.t, v.z, v.with_z(0.0).length()));
                    said.catch_fall |= j.dive;
                    (j.phase, j.t) = (Phase::Flight, 0.0);
                } else if j.t > PUSH_TIME() + NO_TAKEOFF() {
                    end(a);
                    said.log = Some("jump  no takeoff".into());
                    return false;
                }
            }
            Phase::Flight if v.z < 0.0 && (low < j.feet + OFF() || com.z < j.base - DOWN()) || j.phase == Phase::Flight && j.t > FLIGHT() => {
                said.log = Some(format!("jump  lands: apex {:.2} m, down {:.2} m/s", j.top - j.base, -v.z));
                end(a);
                said.catch_fall |= -v.z > LAND_SPEED() && !j.dive;
                (j.phase, j.t) = (Phase::Land, 0.0);
            }
            Phase::Land if j.t > 1.0 => {
                said.log = Some(format!("jump  done: {}", if a.balancing() { "standing" } else { "down" }));
                return false;
            }
            _ => {}
        }
        true
    }
}

fn end(a: &mut Agent) {
    a.configure_balance(Default::default());
    a.muscle_stiffness_x(1.0);
}
