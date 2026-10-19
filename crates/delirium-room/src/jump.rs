use bevy::prelude::*;
use delirium::nm::ConfigureBalance;

use crate::Sim;

const SOFT: f32 = 3.0;
const CROUCH_LEGS: f32 = 4.0;
const SINK: f32 = 1.5;
const SINK_TIME: f32 = 0.08;
const CROUCH_DEPTH: f32 = 0.15;
const CROUCH_TIME: f32 = 0.3;
const PUSH_TIME: f32 = 0.1;
const STIFF: f32 = 16.0;
const PUSH_LEGS: f32 = 20.0;
const APEX: f32 = 0.35;
const DIVE_APEX: f32 = 0.25;
const DIVE_SPEED: f32 = 3.5;
const NO_TAKEOFF: f32 = 0.3;
const LAND_SPEED: f32 = 3.0;
const DOWN: f32 = 0.3;
const FLIGHT: f32 = 1.5;
const OFF: f32 = 0.03;
const FEET: [usize; 2] = [3, 6];
const LEGS: std::ops::RangeInclusive<usize> = 1..=6;

pub struct Jump {
    dive: bool,
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

impl Sim {
    pub(crate) fn jump_start(&mut self, dive: bool) {
        if !self.agent.balancing() {
            return self.say("jump  Homer must be balancing".into());
        }
        let f: Vec<Transform> = self.agent.doll.frames().collect();
        let dir = Vec3::Z.cross(f[4].translation - f[1].translation).with_z(0.0).normalize_or(Vec3::X);
        self.agent.configure_balance(ConfigureBalance { leg_stiffness: CROUCH_LEGS, ..Default::default() });
        let base = self.agent.doll.com().0.z;
        let feet = FEET.map(|k| f[k].translation.z).into_iter().fold(f32::MAX, f32::min);
        self.jump = Some(Jump { dive, phase: Phase::Load, t: 0.0, dir, feet, base, top: base });
        self.say(format!("jump  {}", if dive { "dive" } else { "up" }));
    }

    pub(crate) fn jump_frame(&mut self, dt: f32) {
        let Some(mut j) = self.jump.take() else { return };
        j.t += dt;
        let (com, v) = self.agent.doll.com();
        j.top = j.top.max(com.z);
        let f: Vec<Transform> = self.agent.doll.frames().collect();
        let low = FEET.map(|k| f[k].translation.z).into_iter().fold(f32::MAX, f32::min);
        match j.phase {
            Phase::Load => {
                self.agent.set_stiffness_mask(SOFT, 0.5, "lb");
                if j.t <= SINK_TIME {
                    let dv = Vec3::NEG_Z * SINK * (dt / SINK_TIME);
                    for (k, x) in f.iter().enumerate().filter(|(k, _)| !LEGS.contains(k)) {
                        let m = self.agent.doll.part_mass(k);
                        self.agent.doll.impulse(k, x.translation, dv * m);
                    }
                }
                if j.base - com.z >= CROUCH_DEPTH || j.t >= CROUCH_TIME {
                    self.say(format!("jump  loaded: {:.2} m down in {:.2} s", j.base - com.z, j.t));
                    self.agent.set_stiffness_mask(STIFF, 1.0, "lb");
                    self.agent.configure_balance(ConfigureBalance { leg_stiffness: PUSH_LEGS, ..Default::default() });
                    (j.phase, j.t) = (Phase::Push, 0.0);
                }
            }
            Phase::Push => {
                let up = (2.0 * 9.8 * if j.dive { DIVE_APEX } else { APEX }).sqrt();
                let on = if j.dive { DIVE_SPEED } else { 0.0 };
                let gap = Vec3::Z * (up - v.z).max(0.0) + j.dir * (on - v.dot(j.dir)).max(0.0);
                let left = ((PUSH_TIME - j.t) / dt).max(0.0) + 1.0;
                if j.t <= PUSH_TIME + dt * 0.5 {
                    let dv = gap / left;
                    for (k, x) in f.iter().enumerate() {
                        let m = self.agent.doll.part_mass(k);
                        self.agent.doll.impulse(k, x.translation, dv * m);
                    }
                }
                if low > j.feet + OFF {
                    self.say(format!("jump  takeoff at {:.2} s: up {:.2} m/s, on {:.2} m/s", j.t, v.z, v.with_z(0.0).length()));
                    if j.dive {
                        self.send_to(0, "catchFall start=true");
                    }
                    (j.phase, j.t) = (Phase::Flight, 0.0);
                } else if j.t > PUSH_TIME + NO_TAKEOFF {
                    self.jump_end();
                    return self.say("jump  no takeoff".into());
                }
            }
            Phase::Flight if v.z < 0.0 && (low < j.feet + OFF || com.z < j.base - DOWN) || j.phase == Phase::Flight && j.t > FLIGHT => {
                self.say(format!("jump  lands: apex {:.2} m, down {:.2} m/s", j.top - j.base, -v.z));
                self.jump_end();
                if -v.z > LAND_SPEED && !j.dive {
                    self.send_to(0, "catchFall start=true");
                }
                (j.phase, j.t) = (Phase::Land, 0.0);
            }
            Phase::Land if j.t > 1.0 => return self.say(format!("jump  done: {}", if self.agent.balancing() { "standing" } else { "down" })),
            _ => {}
        }
        self.jump = Some(j);
    }

    fn jump_end(&mut self) {
        self.agent.configure_balance(Default::default());
        self.agent.muscle_stiffness_x(1.0);
    }
}
