use super::*;
use std::path::Path;

const RAMP: f32 = 3.0;

pub(super) const TRAINED: Quat = Quat::from_xyzw(0.0, 0.0, -0.74354, 0.6686915);

#[derive(Clone)]
pub(super) struct Steer { from: [f32; 2], to: [f32; 2], s: f32, pub(super) now: [f32; 2], sum: [f32; 2], t: f32 }

impl Steer {
    pub(super) fn area(&self, i: usize, t: f32) -> f32 { self.sum[i] + self.now[i] * (t - self.t) }
    fn advance(&mut self, t: f32) {
        self.s = (self.s + DT / RAMP).min(1.0);
        for i in 0..2 {
            self.sum[i] += self.now[i] * (t - self.t);
            self.now[i] = self.from[i] + (self.to[i] - self.from[i]) * gait_ramp(self.s);
        }
        self.t = t;
    }
}

pub struct Preview { net: Net, walk: Motion, control: Controller, action: Vec<f32>, n: usize }

impl Preview {
    pub fn new(build: &Build, world: &Collision, start: &[Transform], dir: &Path) -> Result<(Self, Agent), String> {
        let f = |name: &str| dir.join(name).to_string_lossy().into_owned();
        let settings = [("DLM_MOCAP_TRACKING", "mocap-actuator-v2".into()), ("DLM_BRISK_REFERENCE", f("brisk.motion")), ("DLM_JOG_LIMIT", "0.15".into()),
            ("DLM_JOG_STIFFNESS", "12".into()), ("DLM_GAIT_PRIOR", f("gait-prior.net")), ("DLM_ALIGNMENT_PRIOR", f("alignment-prior.net")), ("DLM_PATH_GAIN", "1".into()),
            ("DLM_TRANSITION_PRIOR", f("transition-prior.net")), ("DLM_WALK_REFERENCE_PRIOR", f("walk-prior.net")), ("DLM_ANKLE_PRIOR", f("ankle-prior.net")),
            ("DLM_WALKING_EXPERT", f("walking-expert.net")), ("DLM_STRAIGHT_ROUTE", "1".into()), ("DLM_PHASE_SUPPORT", "0".into()),
            ("DLM_NATIVE_LEG_OWNERSHIP", "0.075".into()), ("DLM_CONTACT_FIDELITY", "1".into()), ("DLM_LONG_TRANSITIONS", "1".into()),
            ("DLM_JOG_NATIVE_OWNERSHIP", "0".into()), ("DLM_JOG_LEG_AUTHORITY", "0.25".into()), ("DLM_REFERENCE_FOOT_PLACEMENT", "1".into()),
            ("DLM_WALK_REFERENCE_AUTHORITY", "0.35".into()), ("DLM_BRISK_SUPPORT", "0".into()), ("DLM_JOG_ROUTE_GAIN", "1".into())];
        for (k, v) in settings { unsafe { std::env::set_var(k, v) } }
        let walk = Motion::load(&f("walk.motion"), build)?;
        let jog = Motion::load(&f("jog.motion"), build)?;
        if BRISK.get().is_none() { configure_bridge(build, &walk, &jog)?; }
        let net = Net::load(&f("policy.net")).ok_or("no preview policy.net")?;
        let mut control = Controller::walking(&walk, 0.0, start);
        control.with_jog(&walk, &jog, false);
        let j = control.jog.as_mut().unwrap();
        j.steer = Some(Steer { from: [0.0; 2], to: [0.0; 2], s: 1.0, now: [0.0; 2], sum: [0.0; 2], t: j.offset });
        let a = control.agent_in(&walk, build, world);
        if net.sizes[0] != control.observation(&a, build, &walk).len() || net.sizes[3] != control.actions() {
            return Err("preview policy doesn't match this build".into());
        }
        let action = vec![0.0; control.actions()];
        Ok((Self { net, walk, control, action, n: 0 }, a))
    }

    pub fn step(&mut self, a: &mut Agent, build: &Build, world: &Collision, beta: f32) -> bool {
        let j = self.control.jog.as_mut().unwrap();
        let t = self.control.elapsed as f32 * DT + j.offset;
        let s = j.steer.as_mut().unwrap();
        let to = [if s.now[1] > 0.0 { 1.0 } else { (2.0 * beta).clamp(0.0, 1.0) },
            if s.now[0] < 1.0 { 0.0 } else { (2.0 * beta - 1.0).clamp(0.0, 1.0) }];
        if s.to != to { s.from = s.now; s.to = to; s.s = 0.0; }
        s.advance(t);
        if self.n % 2 == 0 { self.action = self.net.forward(&self.net.w, &self.control.observation(a, build, &self.walk)); }
        self.n += 1;
        self.control.drive(a, build, world, &self.walk, &self.action);
        a.step(build, world, DT, &[], &[]);
        !self.control.measure(a, build, &self.walk).1
    }
}

pub fn check(build: &Build, world: &Collision, start: &[Transform], dir: &Path) -> Result<(), String> {
    let plans: [&[(f32, f32)]; 5] = [&[(0.0, 16.0)], &[(0.5, 16.0)], &[(1.0, 22.0)], &[(1.0, 22.0), (0.0, 22.0)], &[(0.0, 12.0), (0.5, 12.0), (1.0, 12.0), (0.5, 12.0), (0.0, 12.0)]];
    let mut failed = vec![];
    for plan in plans {
        let (mut p, mut a) = Preview::new(build, world, start, dir)?;
        let pelvis = |a: &Agent| a.doll.frames().next().unwrap().translation;
        let x0 = pelvis(&a); let mut steps = 0;
        'run: for &(beta, seconds) in plan {
            let x = pelvis(&a);
            for _ in 0..(seconds / DT) as usize {
                if !p.step(&mut a, build, world, beta) { failed.push(format!("{plan:?} fell at {:.1} s", steps as f32 * DT)); break 'run; }
                steps += 1;
            }
            println!("beta {beta}: {seconds} s at {:.2} m/s", (pelvis(&a) - x).with_z(0.0).length() / seconds);
        }
        println!("plan {plan:?}: {:.1} s, {:.1} m", steps as f32 * DT, (pelvis(&a) - x0).with_z(0.0).length());
    }
    if failed.is_empty() { println!("preview check: passed"); Ok(()) } else { Err(failed.join("; ")) }
}
