use bevy::prelude::*;
use delirium::{collision::Collision, nm::Agent, ragdoll::Build};
use crate::learn::{self, Net};
use std::io::{self, Read, Write};
use std::sync::{OnceLock, atomic::{AtomicU32, Ordering}};

mod tracking;
mod support;
mod alignment;
mod audit;
mod phase_support;
mod stance_reward;
mod engine_drive;
mod contact_fidelity;
mod walk_expert;
mod practice_schedule;
mod ankle_residual;
mod ankle_fidelity;
mod foot_feedback;
mod turn;
mod jog_stop;
mod turn_torso;
mod whole_body;
mod turn_joints;
pub mod preview;
pub const ACTIONS: usize = 60;
const WALK_ACTIONS: usize = 28;
const WALK_JOG_ACTIONS: usize = 46;
const LIMIT: usize = 480;
const WALK_LIMIT: usize = 3600;
const WALK_EVALUATION: usize = 3600;
const DT: f32 = 1.0 / 60.0;
const METRICS: usize = 6;
static BRISK: OnceLock<Motion> = OnceLock::new();
static JOG_LIMIT: AtomicU32 = AtomicU32::new(1.0f32.to_bits());
static JOG_STIFFNESS: OnceLock<f32> = OnceLock::new();
static FLIGHT_HEIGHT: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
const FLIGHT_SAMPLES: usize=240;

#[derive(Clone)]
pub struct Motion {
    frames: Vec<Vec<Transform>>,
    pub duration: f32,
    stride: Vec3,
    anchors: Vec<(Option<usize>, Vec3, Vec3)>,
}

impl Motion {
    fn heading(&self, frames: &[Transform]) -> Quat {
        pelvis_yaw(frames[0].rotation, self.frames[0][0].rotation, self.stride.normalize_or(Vec3::X))
    }

    pub fn load(path: &str, build: &Build) -> Result<Self, String> {
        let mut motion = Self::decode(&std::fs::read(path).map_err(|e| e.to_string())?, &build.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>())?;
        motion.anchors = build.parts.iter().map(|p| (p.parent, p.anchor_parent, p.anchor)).collect();
        Ok(motion)
    }

    fn decode(data: &[u8], names: &[&str]) -> Result<Self, String> {
        fn take<'a>(b: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
            if b.len() < n { return Err("truncated motion reference".into()); }
            let (a, rest) = b.split_at(n); *b = rest; Ok(a)
        }
        fn u(b: &mut &[u8]) -> Result<usize, String> { Ok(u32::from_le_bytes(take(b, 4)?.try_into().unwrap()) as usize) }
        let mut b = data;
        if take(&mut b, 8)? != b"DMOTION1" { return Err("unsupported motion format".into()); }
        let (fps, n, parts) = (u(&mut b)?, u(&mut b)?, u(&mut b)?);
        if fps != 60 || !(2..=6001).contains(&n) || parts == 0 || parts != names.len() || parts > 256 {
            return Err("motion frame rate, frame count or skeleton does not match".into());
        }
        for name in names {
            let len = u(&mut b)?;
            if len > 256 || take(&mut b, len)? != name.as_bytes() { return Err(format!("motion joint does not match {name}")); }
        }
        if b.len() != n * parts * 28 { return Err("motion data length does not match header".into()); }
        let mut frames = Vec::with_capacity(n);
        for _ in 0..n {
            let mut fr = Vec::with_capacity(parts);
            for _ in 0..parts {
                let mut f = [0f32; 7];
                for x in &mut f { *x = f32::from_le_bytes(take(&mut b, 4)?.try_into().unwrap()); }
                let q = Quat::from_xyzw(f[3], f[4], f[5], f[6]);
                if f.iter().any(|x| !x.is_finite() || x.abs() > 1000.0) || (q.length_squared() - 1.0).abs() > 0.01 {
                    return Err("motion contains invalid positions or rotations".into());
                }
                fr.push(Transform::from_translation(Vec3::new(f[0], f[1], f[2])).with_rotation(q.normalize()));
            }
            frames.push(fr);
        }
        let stride = (frames[n - 1][0].translation - frames[0][0].translation).with_z(0.0);
        Ok(Self { duration: (n - 1) as f32 * DT, frames, stride, anchors: Vec::new() })
    }

    fn aligned_to(&self, direction: Vec3) -> Self {
        let turn = Quat::from_rotation_z(direction.y.atan2(direction.x) - self.stride.y.atan2(self.stride.x));
        let mut aligned = self.clone();
        aligned.stride = turn * self.stride;
        for frame in &mut aligned.frames {
            for body in frame {
                body.translation = turn * body.translation;
                body.rotation = turn * body.rotation;
            }
        }
        aligned
    }

    pub fn sample(&self, time: f32) -> Vec<Transform> {
        static HOLD: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let time = if *HOLD.get_or_init(|| std::env::var("DLM_MOTION_HOLD").as_deref() == Ok("1")) { time.min(self.duration) } else { time };
        let cycles = (time / self.duration).floor();
        let t = time.rem_euclid(self.duration) / DT;
        let i = (t.floor() as usize).min(self.frames.len() - 2);
        let blend = (t - i as f32).clamp(0.0, 1.0);
        let mut result: Vec<_> = self.frames[i].iter().zip(&self.frames[i + 1]).map(|(a, b)| {
            Transform::from_translation(a.translation.lerp(b.translation, blend) + self.stride * cycles)
                .with_rotation(a.rotation.slerp(b.rotation, blend))
        }).collect();
        for (k, &(parent, ap, ac)) in self.anchors.iter().enumerate() {
            if let Some(parent) = parent {
                let a = self.frames[i][parent].rotation.inverse() * self.frames[i][k].rotation;
                let b = self.frames[i + 1][parent].rotation.inverse() * self.frames[i + 1][k].rotation;
                result[k].rotation = result[parent].rotation * a.slerp(b, blend);
                result[k].translation = result[parent].translation + result[parent].rotation * ap - result[k].rotation * ac;
            }
        }
        result
    }
}

pub fn net_path() -> String {
    let old = std::path::Path::new(&learn::path()).with_file_name("human-walk.net");
    let behavior = old.with_file_name("human-behavior-walk.net");
    let natural = old.with_file_name("human-natural-walk.net");
    let assisted = old.with_file_name("human-assisted-walk.net");
    let long = old.with_file_name("human-assisted-long-walk.net");
    if long.exists() { long } else if assisted.exists() { assisted } else if natural.exists() { natural } else if behavior.exists() { behavior } else { old }.to_string_lossy().into_owned()
}
pub fn motion_path() -> Result<String, String> {
    std::fs::read_to_string(std::path::Path::new(&net_path()).with_extension("path")).map(|s| s.trim().to_string()).map_err(|e| e.to_string())
}
pub fn pretrained_path(run: bool) -> String {
    std::path::Path::new(&learn::path()).with_file_name(if run { "pretrained-run.net" } else { "pretrained-walk.net" }).to_string_lossy().into_owned()
}

pub fn dump_rig(a: &Agent, build: &Build, path: &str) -> Result<(), String> {
    let mut text = String::new();
    for (k, joint) in a.joints(build).into_iter().enumerate() {
        if let Some(j) = joint {
            let f: Vec<_> = j.in_parent.to_array().into_iter().chain(j.in_child.to_array()).chain(j.limits).map(|x| x.to_string()).collect();
            text += &format!("{k} {} {} {}\n", j.parent, j.hinge as u8, f.join(" "));
        }
    }
    std::fs::OpenOptions::new().write(true).create_new(true).open(path).and_then(|mut f| f.write_all(text.as_bytes())).map_err(|e| e.to_string())
}

pub struct Human {
    net: Net,
    motion: Motion,
    control: Controller,
    action: Vec<f32>,
    pub quality: Quality,
}

impl Human {
    pub fn load(build: &Build, world: &Collision, start: &[Transform]) -> Result<(Self, Agent), String> {
        let motion = Motion::load(&motion_path()?, build)?;
        let net = Net::load(&net_path()).ok_or("no human walking model yet")?;
        let mut control = if net.sizes[3] == WALK_ACTIONS { Controller::walking(&motion, 0.0, start) } else { Controller::new(&motion, 0.0) };
        let a = control.agent_in(&motion, build, world);
        if net.sizes[3] == ACTIONS + crate::motion_tools::choices().len() {
            let catalog = std::fs::read_to_string(std::path::Path::new(&net_path()).with_extension("tools.tsv")).map_err(|e| format!("missing behavior catalog: {e}"))?;
            if catalog != crate::motion_tools::catalog() { return Err("behavior registry changed; model must be retrained".into()); }
            control.tools = Some(Default::default());
        }
        let obs = control.observation(&a, build, &motion).len();
        let actions = control.actions();
        if net.sizes[0] != obs || net.sizes[3] != actions { return Err("incompatible human walking model".into()); }
        Ok((Self { net, motion, control, action: vec![0.0; actions], quality: Quality::default() }, a))
    }
    pub fn drive(&mut self, a: &mut Agent, build: &Build, world: &Collision) {
        if self.control.elapsed % 2 == 0 {
            self.action = self.net.forward(&self.net.w, &self.control.observation(a, build, &self.motion));
        }
        self.control.drive(a, build, world, &self.motion, &self.action);
    }
    pub fn measure(&mut self, a: &Agent, build: &Build) -> bool {
        let (q, fallen) = self.control.measure(a, build, &self.motion); self.quality.add(q); fallen || self.control.elapsed >= self.control.limit()
    }
}

pub(crate) fn yaw(fr: &[Transform]) -> Quat {
    let f = Vec3::Z.cross(fr[4].translation - fr[1].translation).with_z(0.0).normalize_or(Vec3::X);
    Quat::from_rotation_z(f.y.atan2(f.x))
}

fn pelvis_yaw(rotation: Quat, standing: Quat, forward: Vec3) -> Quat {
    let f = (rotation * standing.inverse() * forward).with_z(0.0).normalize_or(forward);
    Quat::from_rotation_z(f.y.atan2(f.x))
}

fn bottom(build: &Build, fr: &[Transform], k: usize) -> f32 {
    let p = &build.parts[k];
    let local_up = fr[k].rotation.inverse() * Vec3::Z;
    fr[k].translation.z - p.boxed.map_or(p.radius + p.half * local_up.y.abs(), |half| local_up.abs().dot(half))
}

#[derive(Clone, Copy, Default)]
pub struct Quality {
    pub pose: f32,
    pub slip: f32,
    pub jitter: f32,
    pub speed: f32,
    pub reward: f32,
    pub steps: usize,
    pub tracking: [f32; 4],
    pub gait: [f32; 4],
    pub alignment: [f32;4],
    pub interaction:[f32;6],
    pub turn_origin:Vec3,
}

impl Quality {
    fn add(&mut self, q: Self) {
        self.pose += q.pose; self.slip += q.slip; self.jitter += q.jitter;
        self.speed += q.speed; self.reward += q.reward; self.steps += q.steps;
        self.turn_origin=q.turn_origin;
        for i in 0..4 { self.tracking[i] += q.tracking[i]; self.gait[i] += q.gait[i]; self.alignment[i]+=q.alignment[i]; }
    }
    fn values(&self, limit: usize) -> [f32; METRICS] {
        let n = self.steps.max(1) as f32;
        [self.pose / n, self.slip / n, self.jitter / n, self.speed / n, self.reward / limit as f32, self.steps as f32 * DT]
    }
}

pub struct Controller {
    pub time: f32,
    pub elapsed: usize,
    origin: Vec3,
    heading: Quat,
    filtered: Vec<f32>,
    gait_anchor: Vec<f32>,
    transition_anchor:Vec<f32>,
    walk_anchor:Vec<f32>,
    balance_anchor:Vec<f32>,
    ankle_anchor:Vec<f32>,
    ankle_feedback:[Vec3;2],
    leg_muscle_base: Option<([f32;6],[f32;6])>,
    walking_expert_action: Vec<f32>,
    walking_ankle_action: Vec<f32>,
    previous: Vec<f32>,
    last_velocity: Vec3,
    turn_origin:Option<Vec3>,
    tools: Option<crate::motion_tools::Tools>,
    walk: Option<Walking>,
    jog: Option<Jogging>,
    tracking: u8,
    observed: Option<Vec<f32>>,
    poses: std::cell::RefCell<Option<Vec<(u32, Vec<Transform>)>>>,
}

#[derive(Clone)]
struct Jogging { motion: Motion, feet: Vec<(f32, f32)>, brisk: Option<(Motion, Vec<(f32, f32)>)>, rehearsal: bool, finite:bool, practice: bool, practice_limit:f32, offset: f32, practice_duration:f32, steer: Option<preview::Steer> }

fn gait_ramp(x: f32) -> f32 { let x = x.clamp(0.0, 1.0); x * x * (3.0 - 2.0 * x) }
fn gait_ramp_integral(x: f32) -> f32 {
    if x <= 0.0 { 0.0 } else if x >= 1.0 { x - 0.5 } else { x.powi(3) - 0.5 * x.powi(4) }
}
fn jogging_mix(t: f32) -> f32 { gait_ramp((t - 8.0) / 4.0) - gait_ramp((t - 28.0) / 4.0) + gait_ramp((t - 44.0) / 4.0) }
fn jogging_integral(t: f32) -> f32 { 4.0 * (gait_ramp_integral((t - 8.0) / 4.0) - gait_ramp_integral((t - 28.0) / 4.0) + gait_ramp_integral((t - 44.0) / 4.0)) }
fn brisk_mix(t: f32) -> f32 { gait_ramp((t - 8.0) / 6.0) - gait_ramp((t - 48.0) / 6.0) }
fn brisk_integral(t: f32) -> f32 { 6.0 * (gait_ramp_integral((t - 8.0) / 6.0) - gait_ramp_integral((t - 48.0) / 6.0)) }
fn real_jog_mix(t: f32) -> f32 { gait_ramp((t - 20.0) / 8.0) - gait_ramp((t - 40.0) / 6.0) }
fn real_jog_integral(t: f32) -> f32 { 8.0 * gait_ramp_integral((t - 20.0) / 8.0) - 6.0 * gait_ramp_integral((t - 40.0) / 6.0) }

fn configure_bridge(build: &Build, walk: &Motion, jog: &Motion) -> Result<(), String> {
    if let Ok(path) = std::env::var("DLM_BRISK_REFERENCE") {
        if !path.is_empty() {
            let brisk = Motion::load(&path, build)?.aligned_to(walk.stride);
            let speed = brisk.stride.length() / brisk.duration;
            if !(walk.stride.length() / walk.duration < speed && speed < jog.stride.length() / jog.duration) {
                return Err("three-gait reference speeds must increase walk < brisk < jog".into());
            }
            BRISK.set(brisk).map_err(|_| "bridge already configured")?;
            let limit: f32 = std::env::var("DLM_JOG_LIMIT").unwrap_or("0.15".into()).parse().map_err(|_| "invalid jog limit")?;
            set_jog_limit(limit)?;
            if matches!(tracking::mode(),11|12|13|14|15|16|17) {
                let brisk=BRISK.get().unwrap();
                let temporary=Jogging{motion:jog.aligned_to(walk.stride),feet:vec![],brisk:Some((brisk.clone(),vec![])),rehearsal:false,finite:false,practice:false,practice_limit:1.,offset:0.0,practice_duration:turn::duration(),steer:None};
                let agent=learn::agent(build,&walk.frames[0]);
                let original:Vec<_>=agent.doll.frames().collect();let bounds:Vec<_>=agent.doll.capsules().collect();
                let calibrations:Vec<_>=original.iter().zip(bounds).map(|(a,b)|a.rotation.inverse()*b.rotation).collect();
                let masses:Vec<_>=(0..build.parts.len()).map(|k|agent.doll.part_mass(k)).collect();let mass:f32=masses.iter().sum();
                let mut table=vec![vec![0.0;FLIGHT_SAMPLES]];
                for i in 1..=20 {
                    let fraction=i as f32/20.0;
                    if fraction<0.5 {table.push(vec![0.0;FLIGHT_SAMPLES]);continue;}
                    let period=1.0/((1.0-fraction)/brisk.duration+fraction/jog.duration);
                    let mut com=vec![];let mut air=vec![];
                    for n in 0..FLIGHT_SAMPLES {
                        let frames=temporary.raw_high_pose(walk,walk.duration*n as f32/FLIGHT_SAMPLES as f32,fraction);
                        com.push(frames.iter().zip(&masses).map(|(f,m)|f.translation.z*m).sum::<f32>()/mass);
                        let shapes:Vec<_>=frames.iter().zip(&calibrations).map(|(f,q)|Transform::from_translation(f.translation).with_rotation(f.rotation*(*q))).collect();
                        air.push(bottom(build,&shapes,3).min(bottom(build,&shapes,6))>0.005);
                    }
                    let shifts=flight_projection(&com,&air,period/FLIGHT_SAMPLES as f32)?;
                    if shifts.iter().any(|x|!x.is_finite()||x.abs()>0.12) {return Err(format!("flight projection exceeds12cm; reject reference fraction={fraction} max={} grounded={}",shifts.iter().map(|x|x.abs()).fold(0.0,f32::max),air.iter().filter(|x|!**x).count()));}
                    table.push(shifts);
                }
                FLIGHT_HEIGHT.set(table).map_err(|_|"flight projection already configured")?;
            }
        }
    }
    turn::configure(build,walk)?;
    Ok(())
}

fn set_jog_limit(limit: f32) -> Result<(), String> {
    if !limit.is_finite() || !(0.0..=1.0).contains(&limit) { return Err("invalid jog curriculum limit".into()); }
    JOG_LIMIT.store(limit.to_bits(), Ordering::Relaxed);
    Ok(())
}

fn flight_projection(com:&[f32],air:&[bool],dt:f32)->Result<Vec<f32>,String> {
    if com.len()!=air.len()||com.is_empty()||dt<=0.0 {return Err("invalid flight samples".into());}
    let n=com.len();let origin=air.iter().position(|a|!*a).ok_or("cycle has no grounded support")?;
    let mut shifts=vec![0.0;n];let mut last=origin;
    for tick in origin+1..=origin+n {
        if !air[tick%n] {
            if tick>last+1 {
                let duration=(tick-last) as f32*dt;
                for k in last+1..tick {
                    let t=(k-last) as f32*dt;
                    let desired=com[last%n]+(com[tick%n]-com[last%n])*t/duration+0.5*delirium::ragdoll::GRAVITY*t*(duration-t);
                    shifts[k%n]=desired-com[k%n];
                }
            }
            last=tick;
        }
    }
    Ok(shifts)
}

impl Jogging {
    fn limit(&self)->f32 {
        if self.practice&&self.offset==28.0 {self.practice_limit} else {f32::from_bits(JOG_LIMIT.load(Ordering::Relaxed))}
    }
    fn fraction(&self, t: f32) -> f32 {
        if let Some(s)=&self.steer {return s.now[1]}
        if self.finite {turn::mix(t)} else if self.brisk.is_some() { real_jog_mix(t) * self.limit() } else { 0.0 }
    }
    fn raw_high_pose(&self, walk: &Motion, phase: f32, beta: f32) -> Vec<Transform> {
        let b = self.motion.sample(phase / walk.duration * self.motion.duration);
        let Some((brisk, _)) = &self.brisk else { return b; };
        let a = brisk.sample(phase / walk.duration * brisk.duration);
        let mut out: Vec<_> = a.iter().zip(&b).map(|(a,b)| Transform::from_translation(a.translation.lerp(b.translation,beta))
            .with_rotation(a.rotation.slerp(b.rotation,beta))).collect();
        for (k, &(parent, ap, ac)) in walk.anchors.iter().enumerate() {
            if let Some(p) = parent {
                out[k].rotation = out[p].rotation * (a[p].rotation.inverse()*a[k].rotation).slerp(b[p].rotation.inverse()*b[k].rotation,beta);
                out[k].translation = out[p].translation + out[p].rotation*ap - out[k].rotation*ac;
            }
        }
        out
    }
    fn high_pose(&self,walk:&Motion,phase:f32,t:f32)->Vec<Transform> {
        let fraction=self.fraction(t);let mut frames=self.raw_high_pose(walk,phase,fraction);
        if let Some(table)=FLIGHT_HEIGHT.get() {
            let x=fraction*20.0;let i=(x.floor() as usize).min(19);let blend=(x-i as f32).clamp(0.0,1.0);
            let p=(phase/walk.duration).rem_euclid(1.0)*FLIGHT_SAMPLES as f32;let k=p.floor() as usize%FLIGHT_SAMPLES;let a=p-p.floor();
            let height=|row:&Vec<f32>|row[k]+a*(row[(k+1)%FLIGHT_SAMPLES]-row[k]);
            let z=height(&table[i])+(height(&table[i+1])-height(&table[i]))*blend;
            for frame in &mut frames {frame.translation.z+=z;}
        }
        frames
    }
    fn high_speed(&self, t: f32) -> f32 {
        let speed = self.motion.stride.length()/self.motion.duration;
        self.brisk.as_ref().map_or(speed, |(b,_)| {
            let brisk = b.stride.length()/b.duration;
            brisk + self.fraction(t)*(speed-brisk)
        })
    }
}

struct Walking {
    start: Vec<Transform>,
    forward: Vec3,
    wander: delirium::playerctl::Wander,
    rand: delirium_data::rng::Rand,
    prior: Option<Net>,
    phase: f32,
    feet: Vec<(f32, f32)>,
    steer_yaw:f32,
}

fn cycle_difference(target: f32, current: f32, period: f32) -> f32 {
    (target - current + period * 0.5).rem_euclid(period) - period * 0.5
}

impl Controller {
    pub fn new(motion: &Motion, phase: f32) -> Self {
        let origin = motion.sample(phase)[0].translation.with_z(0.0);
        Self { time: phase, elapsed: 0, origin, heading: Quat::IDENTITY, filtered: vec![0.0; ACTIONS], gait_anchor:Vec::new(), transition_anchor:Vec::new(),walk_anchor:Vec::new(),balance_anchor:Vec::new(),ankle_anchor:Vec::new(),ankle_feedback:[Vec3::ZERO;2],leg_muscle_base:None,walking_expert_action:Vec::new(),walking_ankle_action:Vec::new(), previous: vec![0.0; ACTIONS], last_velocity: Vec3::ZERO, turn_origin:None, tools: None, walk: None, jog: None, tracking: tracking::mode(), observed: None, poses: Default::default() }
    }

    fn walking(motion: &Motion, phase: f32, start: &[Transform]) -> Self {
        let mut control = Self::new(motion, phase);
        control.heading = yaw(start) * motion.heading(&motion.sample(phase)).inverse();
        control.filtered = vec![0.0; WALK_ACTIONS]; control.previous = vec![0.0; WALK_ACTIONS];
        let feet = motion.frames.windows(2).map(|f| {
            let forward = motion.heading(&f[0]) * Vec3::X;
            let delta = f[0][6].translation - f[0][3].translation;
            let next = f[1][6].translation - f[1][3].translation;
            (delta.dot(forward), ((next - delta) / DT).dot(forward))
        }).collect();
        control.walk = Some(Walking { start: start.to_vec(), forward: yaw(start) * Vec3::X,
            wander: Default::default(), rand: delirium_data::rng::Rand::new(1),
            prior: Net::load(&learn::walk_path(false)).filter(|n| n.sizes[3] == 4), phase, feet, steer_yaw:0.0 });
        control
    }

    fn with_jog(&mut self, walk: &Motion, motion: &Motion, rehearsal: bool) {
        let motion = motion.aligned_to(walk.stride);
        let count = self.walk.as_ref().expect("jogging builds on native walking").feet.len();
        let feet = (0..count).map(|i| {
            let t = motion.duration * i as f32 / count as f32;
            let a = motion.sample(t); let b = motion.sample(t + DT);
            let forward = motion.heading(&a) * Vec3::X;
            let delta = a[6].translation - a[3].translation;
            let next = b[6].translation - b[3].translation;
            (delta.dot(forward), ((next - delta) / DT).dot(forward))
        }).collect();
        assert!(motion.stride.length() / motion.duration > walk.stride.length() / walk.duration);
        let brisk = BRISK.get().map(|brisk| {
            let brisk = brisk.aligned_to(walk.stride);
            let features = (0..count).map(|i| {
                let t = brisk.duration*i as f32/count as f32;
                let a=brisk.sample(t); let b=brisk.sample(t+DT); let f=brisk.heading(&a)*Vec3::X;
                let d=a[6].translation-a[3].translation; let n=b[6].translation-b[3].translation;
                (d.dot(f),((n-d)/DT).dot(f))
            }).collect();
            (brisk,features)
        });
        self.jog = Some(Jogging { motion, feet, brisk, rehearsal, finite:turn::enabled()&&!rehearsal, practice:false, practice_limit:1., offset: 0.0,practice_duration:turn::duration(),steer:None });
        self.filtered.resize(self.actions(), 0.0); self.previous.resize(self.actions(), 0.0);
    }

    fn gait(&self, ahead: f32) -> f32 {
        self.jog.as_ref().filter(|j| !j.rehearsal).map_or(0.0, |j| {
            let t=self.elapsed as f32*DT+ahead+j.offset;
            if let Some(s)=&j.steer {return s.now[0]}
            if j.finite{return turn::mix(t)}
            if j.brisk.is_some() { brisk_mix(t) } else { jogging_mix(t) }
        })
    }

    fn actions(&self) -> usize { if self.jog.is_some() { WALK_JOG_ACTIONS+if self.tracking==17 {4+if ankle_residual::enabled() {4} else {0}+if whole_body::enabled(){36}else{0}+if turn_joints::enabled(){14}else{0}} else if matches!(self.tracking,15|16) {2} else {0} } else if self.walk.is_some() { WALK_ACTIONS } else { ACTIONS + if self.tools.is_some() { crate::motion_tools::choices().len() } else { 0 } } }
    fn turning(&self)->bool{self.jog.as_ref().is_some_and(|j|j.finite)}
    fn task_forward(&self,target:&[Transform])->Vec3{
        if self.turning()&&self.elapsed as f32*DT+self.jog.as_ref().unwrap().offset>=8. {self.body_heading(target)*Vec3::X}else{self.walk.as_ref().map_or(Vec3::X,|w|w.forward)}
    }
    fn limit(&self) -> usize {if self.turning(){((60.-self.jog.as_ref().unwrap().offset)*60.).round() as usize}else if self.walk.is_some() { WALK_LIMIT } else { LIMIT } }

    fn target(&self, motion: &Motion, time: f32) -> Vec<Transform> {
        if let Some(poses) = self.poses.borrow_mut().as_mut() {
            if let Some((_, pose)) = poses.iter().find(|(t, _)| *t == time.to_bits()) { return pose.clone(); }
            let pose = self.target_(motion, time);
            poses.push((time.to_bits(), pose.clone()));
            return pose;
        }
        self.target_(motion, time)
    }
    fn target_(&self, motion: &Motion, time: f32) -> Vec<Transform> {
        if self.turning(){return turn::target(self,motion,time)}
        self.straight_target(motion,time)
    }
    fn straight_target(&self, motion: &Motion, time: f32) -> Vec<Transform> {
        let shift = self.walk.as_ref().map_or(Vec3::ZERO, |w| w.start[0].translation.with_z(0.0));
        if let Some(jog) = self.jog.as_ref().filter(|j| !j.rehearsal) {
            let ahead = time - self.time; let t = self.elapsed as f32 * DT + ahead + jog.offset;
            let mut phase = self.walk.as_ref().unwrap().phase + ahead;
            if matches!(self.tracking,11|12|13|14|15|16|17) {
                if let Some((brisk,_))=&jog.brisk {
                    let fraction=jog.fraction(t);let gait=self.gait(ahead);
                    let rate=(1.0-gait)+gait*motion.duration*((1.0-fraction)/brisk.duration+fraction/jog.motion.duration);
                    phase=self.walk.as_ref().unwrap().phase+ahead*rate;
                }
            }
            let beta = self.gait(ahead);
            let a = motion.sample(phase); let b = jog.high_pose(motion,phase,t);
            let walk_speed = motion.stride.length() / motion.duration;
            let jog_speed = jog.motion.stride.length() / jog.motion.duration;
            let distance = if let (Some(s),Some((brisk,_))) = (&jog.steer,&jog.brisk) {
                let brisk_speed=brisk.stride.length()/brisk.duration;
                walk_speed*(t-jog.offset)+(brisk_speed-walk_speed)*s.area(0,t)+(jog_speed-brisk_speed)*s.area(1,t)
            } else if let Some((brisk,_)) = &jog.brisk {
                let brisk_speed=brisk.stride.length()/brisk.duration;
                walk_speed*(t-jog.offset)+(brisk_speed-walk_speed)*(brisk_integral(t)-brisk_integral(jog.offset))
                    +(jog_speed-brisk_speed)*jog.limit()*(real_jog_integral(t)-real_jog_integral(jog.offset))
            } else { walk_speed*(t-jog.offset)+(jog_speed-walk_speed)*(jogging_integral(t)-jogging_integral(jog.offset)) };
            let progress = motion.stride.normalize() * distance;
            let root = a[0].translation.lerp(b[0].translation, beta).with_z(0.0);
            let mut result: Vec<Transform> = a.iter().zip(&b).map(|(a, b)| Transform::from_translation(self.heading * (a.translation.lerp(b.translation, beta) - root + progress) + shift)
                .with_rotation(self.heading * a.rotation.slerp(b.rotation, beta))).collect();
            if alignment::straight_route() {
                let delta=alignment::route_shift(self.walk.as_ref().unwrap().forward,self.heading*motion.stride.normalize(),distance);
                for frame in &mut result {frame.translation+=delta;}
            }
            if !motion.anchors.is_empty() {
                for (k, &(parent, ap, ac)) in motion.anchors.iter().enumerate() {
                    if let Some(parent) = parent {
                        assert!(parent < k, "motion rig must be parent-first");
                        let rel_a = a[parent].rotation.inverse() * a[k].rotation;
                        let rel_b = b[parent].rotation.inverse() * b[k].rotation;
                        result[k].rotation = result[parent].rotation * rel_a.slerp(rel_b, beta);
                        result[k].translation = result[parent].translation + result[parent].rotation * ap - result[k].rotation * ac;
                    }
                }
            }
            return result;
        }
        let mut frames = motion.sample(self.walk.as_ref().map_or(time, |w| w.phase + time - self.time));
        let advance = motion.sample(time)[0].translation - frames[0].translation;
        for f in &mut frames { f.translation += advance; }
        let route=self.walk.as_ref().filter(|_|alignment::straight_route()).map_or(Vec3::ZERO,|walk| {
            let distance=(self.elapsed as f32*DT+time-self.time)*motion.stride.length()/motion.duration;
            alignment::route_shift(walk.forward,self.heading*motion.stride.normalize(),distance)
        });
        frames.into_iter().map(|f| Transform::from_translation(self.heading * (f.translation - self.origin) + shift+route)
            .with_rotation(self.heading * f.rotation)).collect()
    }

    fn body_heading(&self, frames: &[Transform]) -> Quat {
        self.walk.as_ref().map_or_else(|| yaw(frames), |w| pelvis_yaw(frames[0].rotation, w.start[0].rotation, w.forward))
    }

    fn agent_in(&mut self, motion: &Motion, build: &Build, world: &Collision) -> Agent {
        self.agent_in_reset(motion,build,world,turn::reset_balance_enabled())
    }

    fn agent_in_reset(&mut self,motion:&Motion,build:&Build,world:&Collision,correct_reset:bool)->Agent {
        if let Some(walk) = &self.walk {
            let deferred=correct_reset&&self.turning()&&self.jog.as_ref().is_some_and(|j|j.practice&&j.offset>0.);
            let mut a = if deferred {learn::agent(build,&walk.start)} else {learn::walk_agent(build, world, &walk.start).expect("native bodyBalance available")};
            if self.jog.as_ref().is_some_and(|j| !j.rehearsal && j.offset > 0.0) {
                let pose = self.target(motion, self.time); let next = self.target(motion, self.time + DT);
                for k in 0..pose.len() {
                    let velocity = (next[k].translation - pose[k].translation) / DT;
                    let angular = (next[k].rotation * pose[k].rotation.inverse()).to_scaled_axis() / DT;
                    a.doll.set_body(k, pose[k].translation, pose[k].rotation, velocity, angular);
                }
                self.last_velocity = a.doll.motion(0).0;
            }
            if deferred {learn::start_walk_balance(&mut a,build,world).expect("native bodyBalance available after reference reset");}
            a
        } else { self.agent(motion, build) }
    }

    pub fn agent(&mut self, motion: &Motion, build: &Build) -> Agent {
        let fr = self.target(motion, self.time);
        let next = self.target(motion, self.time + DT);
        let mut a = learn::agent(build, &fr);
        for k in 0..fr.len() {
            let v = (next[k].translation - fr[k].translation) / DT;
            let w = (next[k].rotation * fr[k].rotation.inverse()).to_scaled_axis() / DT;
            a.doll.set_body(k, fr[k].translation, fr[k].rotation, v, w);
        }
        self.last_velocity = a.doll.motion(0).0;
        a
    }

    pub fn observation(&self, a: &Agent, build: &Build, motion: &Motion) -> Vec<f32> {
        let now: Vec<_> = a.doll.frames().collect();
        let target = self.target(motion, self.time + DT);
        let mut o = learn::body_obs(&a.doll, 0, true);
        o.truncate(o.len() - learn::MENU.len());
        let (s, c) = (self.walk.as_ref().map_or(self.time, |w| w.phase) / motion.duration * std::f32::consts::TAU).sin_cos();
        o.extend([s, c, 0.0]);
        let turned: Option<Vec<Quat>> = self.jog.as_ref().and_then(|j| j.steer.as_ref()).map(|_| a.doll.turned(preview::TRAINED * self.heading.inverse()).collect());
        for k in 1..now.len() {
            let p = build.parts[k].parent.unwrap();
            let actual = turned.as_ref().map_or(now[p].rotation.inverse() * now[k].rotation, |r| r[p].inverse() * r[k]);
            let desired = target[p].rotation.inverse() * target[k].rotation;
            o.extend((actual.inverse() * desired).to_scaled_axis().to_array());
        }
        let inv = self.body_heading(&now).inverse();
        o.extend((inv * (target[0].translation - now[0].translation)).clamp_length_max(2.0).to_array());
        let next = self.target(motion, self.time + 2.0 * DT);
        o.extend((inv * ((next[0].translation - target[0].translation) / DT - a.doll.motion(0).0) * 0.2).to_array());
        o.extend(self.filtered.iter().take(if matches!(self.tracking,15|16|17) {WALK_JOG_ACTIONS} else {self.filtered.len()}).copied());
        if let Some(jog) = &self.jog {
            let beta = self.gait(DT);
            let t=self.elapsed as f32*DT+DT+jog.offset;
            if jog.brisk.is_none() {
                o.extend([beta, ((1.0 - beta) * motion.stride.length() / motion.duration + beta * jog.motion.stride.length() / jog.motion.duration) / 3.0]);
            } else {
                o.extend([beta + if jog.rehearsal {0.0} else {jog.fraction(t)}, ((1.0 - beta) * motion.stride.length() / motion.duration + beta * jog.high_speed(t)) / 3.0]);
            }
        }
        if self.turning(){
            o[311]=2.*self.gait(DT);
            o[312]=(next[0].translation-target[0].translation).with_z(0.).length()/DT/3.;
        }
        if let Some(tools) = &self.tools {
            o.extend((0..crate::motion_tools::choices().len()).map(|k| f32::from(k == tools.selected)));
            o.push(f32::from(tools.failed || (crate::motion_tools::choices()[tools.selected] == "bodyBalance" && !a.balancing())));
        }
        if matches!(self.tracking,12|13|14|15|16|17) {
            let mut feedback=[0.0;10];
            if self.jog.as_ref().is_some_and(|j|!j.rehearsal&&j.fraction(self.elapsed as f32*DT+j.offset)>0.05) {
                let bounds:Vec<_>=a.doll.capsules().collect();let (com,v)=a.doll.com();
                let left=inv*(com-now[3].translation);let right=inv*(com-now[6].translation);
                let mass=a.doll.mass();
                let desired=(0..now.len()).map(|k|(next[k].translation.z-target[k].translation.z)/DT*a.doll.part_mass(k)).sum::<f32>()/mass;
                feedback=[f32::from(a.doll.touch[3]&0x20!=0),f32::from(a.doll.touch[6]&0x20!=0),
                    bottom(build,&bounds,3)*5.0,bottom(build,&bounds,6)*5.0,v.z*0.3,left.x,left.y,right.x,right.y,desired*0.3];
            }
            o.extend(feedback);
        }
        if matches!(self.tracking,13|14|15|16|17) {
            let actual_heading=self.body_heading(&now);let wanted=self.body_heading(&target);
            let task=self.task_forward(&target);let side=Vec3::Z.cross(task);
            let forward=actual_heading*Vec3::X;let goal=wanted*Vec3::X;
            o.extend([(target[0].translation-now[0].translation).dot(side),a.doll.motion(0).0.dot(side)*0.3,
                forward.cross(goal).z,forward.dot(goal),
                5.0*(alignment::knee(&now,1,2,3,actual_heading*Vec3::Y)-alignment::knee(&target,1,2,3,wanted*Vec3::Y)),
                5.0*(alignment::knee(&now,4,5,6,actual_heading*Vec3::Y)-alignment::knee(&target,4,5,6,wanted*Vec3::Y))]);
        }
        if matches!(self.tracking,15|16|17) {o.extend(self.filtered[WALK_JOG_ACTIONS..].iter().take(if self.tracking==17 {4} else {2}).copied());}
        if self.tracking==17&&support::native_observations() {
            let state=a.character().map_or([-2;2],|ch|ch.support);
            o.extend(support::native_feedback([a.doll.touch[3]&0x20!=0,a.doll.touch[6]&0x20!=0],state));
        }
          if self.tracking==17&&foot_feedback::enabled() {
              assert_eq!(o.len(),333,"foot-state feedback requires the legacy333 feature prefix");
              o.extend(foot_feedback::encode(inv,[a.doll.motion(3).1,a.doll.motion(6).1],
                  [a.doll.touch[3]&0x20!=0,a.doll.touch[6]&0x20!=0]));
          }
        if turn::preview_enabled(){
            assert_eq!(o.len(),341,"turn preview requires the retained341 foot-state prefix");
            o.extend(turn::preview(self,motion,inv));
        }
        if whole_body::enabled(){
            assert_eq!(o.len(),349,"whole-body turn requires planned-goal and foot-state inputs");
            o.push(whole_body::gate(self.turning(),self.elapsed as f32*DT+self.jog.as_ref().map_or(0.,|j|j.offset)));
        }
        if turn_joints::enabled(){assert_eq!(o.len(),350,"cut joints require whole-body inputs");o.push(turn_joints::gate(self.turning(),self.elapsed as f32*DT+self.jog.as_ref().map_or(0.,|j|j.offset)));}
        o.iter_mut().for_each(|x| *x = if x.is_finite() { x.clamp(-10.0, 10.0) } else { 0.0 });
        o
    }

    pub fn drive(&mut self, a: &mut Agent, build: &Build, world: &Collision, motion: &Motion, action: &[f32]) {
        assert_eq!(action.len(), self.actions());
        *self.poses.get_mut() = None;
        let observed=self.observed.take().map_or_else(std::cell::OnceCell::new,std::cell::OnceCell::from);
        let mut ankle_mixed;
        let action=if let Some(expert)=walk_expert::ankle_expert(){
            if self.elapsed%2==0||self.walking_ankle_action.is_empty(){
                let obs=observed.get_or_init(||self.observation(a,build,motion));self.walking_ankle_action=expert.frozen(obs);
            }
            ankle_mixed=action.to_vec();for i in 0..4 {ankle_mixed[50+i]=walk_expert::ankle_action(action[50+i],self.walking_ankle_action[i],self.gait(0.));}
            ankle_mixed.as_slice()
        }else{action};
        let mixed;
        let action=if self.tracking==17 {
            if let Some(expert)=walk_expert::expert() {
                if self.elapsed%2==0||self.walking_expert_action.is_empty() {
                    let observation=observed.get_or_init(||self.observation(a,build,motion));
                    self.walking_expert_action=expert.frozen(&observation[..expert.sizes[0]]);
                }
                mixed=walk_expert::blend(action,&self.walking_expert_action,self.gait(0.));mixed.as_slice()
            } else {action}
        } else {action};
        let raw_action=action;
        if self.tracking==17 {
            if let Some((base,applied))=self.leg_muscle_base.take() {
                let character=a.character().expect("native joint effectors required");
                let effectors:Vec<_>=character.eff.iter().enumerate().map(|(i,e)|
                    (e.desired,if i<6 {support::restore_response(e.muscle_stiffness,base[i],applied[i])} else {e.muscle_stiffness},e.strength,e.damping)).collect();
                a.set_effectors(&effectors);
            }
        }
        let mut bounded;
        let action=if matches!(self.tracking,13|14|15|16|17) {
            let prior=alignment::prior();let observation=observed.get_or_init(||self.observation(a,build,motion));
            let base=prior.frozen(&observation[..323]);
            let fraction=self.jog.as_ref().filter(|j|!j.rehearsal).map_or(0.,|j|j.fraction(self.elapsed as f32*DT+j.offset));
            let mut protected;
            let action=if matches!(self.tracking,16|17) {
                if self.elapsed%2==0||self.gait_anchor.is_empty() {
                    let gait=alignment::gait_prior();self.gait_anchor=gait.frozen(&observation[..331]);
                    if let Some(prior)=alignment::transition_prior() {self.transition_anchor=prior.frozen(&observation[..333]);}
                    if let Some(prior)=alignment::walk_prior() {self.walk_anchor=prior.frozen(&observation[..333]);}
                    if let Some(prior)=alignment::balance_prior() {self.balance_anchor=prior.frozen(&observation[..prior.sizes[0]]);}
                    if let Some(prior)=alignment::ankle_prior() {self.ankle_anchor=prior.frozen(&observation[..333]);}
                }
                protected=self.gait_anchor.iter().zip(action).enumerate().map(|(i,(&p,&x))|alignment::protected_leg_action(i,p,x,fraction)).collect::<Vec<_>>();
                if !self.balance_anchor.is_empty() {
                    for i in 0..4 {protected[i]=alignment::balance_action(i,protected[i],action[i],self.balance_anchor[i],fraction);}
                }
                if !self.walk_anchor.is_empty() {
                    let brisk=self.gait(0.0);
                    for i in 28..46 {protected[i]=alignment::walking_action(i,protected[i],action[i],self.walk_anchor[i],brisk);}
                }
                if !self.transition_anchor.is_empty() {
                    let brisk=self.gait(0.0);
                    for i in 28..46 {protected[i]=alignment::transition_action(i,protected[i],action[i],self.transition_anchor[i],brisk,fraction);}
                }
                if self.tracking==17 {protected.extend_from_slice(&action[48..]);}
                protected.as_slice()
            } else {action};
            bounded=base.iter().zip(action).map(|(&p,&x)|alignment::bounded_action(p,x,fraction)).collect::<Vec<_>>();
            if matches!(self.tracking,15|16|17) {bounded.extend_from_slice(&action[WALK_JOG_ACTIONS..]);}
            bounded.as_slice()
        } else {action};
        if !self.ankle_anchor.is_empty() {
            for (leg,offset) in [34,43].into_iter().enumerate() {
                let target=alignment::ankle_correction(Vec3::from_slice(&raw_action[offset..offset+3]),Vec3::from_slice(&self.ankle_anchor[offset..offset+3]));
                self.ankle_feedback[leg]=self.ankle_feedback[leg].lerp(target,0.2);
            }
        }
        if let Some(tools) = &mut self.tools {
            tools.apply(self.elapsed, &action[ACTIONS..], a, build, world);
            for line in std::mem::take(&mut a.extras.outbox).iter().filter(|l| !l.starts_with("#say ")) {
                learn::send_line(a, build, world, line);
            }
            if !tools.supports_gait() { a.doll.kick(Vec3::ZERO); return; }
        }
        let smoothing = if self.walk.is_some() { 0.2 } else { plain_smoothing() };
        let upper_gate=whole_body::gate(self.turning(),self.elapsed as f32*DT+self.jog.as_ref().map_or(0.,|j|j.offset));
        let joint_gate=turn_joints::gate(self.turning(),self.elapsed as f32*DT+self.jog.as_ref().map_or(0.,|j|j.offset));
        for (i,((f, p), x)) in self.filtered.iter_mut().zip(&mut self.previous).zip(action).enumerate() {
            let x=if turn_joints::enabled()&&i>=90 {x*joint_gate}else if whole_body::enabled()&&i>=54 {x*upper_gate}else{*x};
            *p = *f; *f += smoothing * (x.clamp(-1.0, 1.0) - *f);
        }
        let gait = self.gait(0.0);
        let gait_time=self.elapsed as f32*DT+self.jog.as_ref().map_or(0.0,|j|j.offset);
        let real_jog=self.jog.as_ref().filter(|j|!j.rehearsal).map_or(0.0,|j|j.fraction(gait_time));
        let leg_support=if self.tracking==17 {support::brisk_support(gait,real_jog)} else {real_jog};
        let ownership=if self.tracking==17 {engine_drive::weight()*engine_drive::handover(self.elapsed as f32*DT,gait)
            +engine_drive::jog_weight()*engine_drive::jog_handover(gait,real_jog)} else {0.};
        let support_ownership=ownership+(1.-ownership)*if self.tracking==17 {engine_drive::support_only()*engine_drive::jog_handover(gait,real_jog)} else {0.};
        let cut_support=if turn_joints::enabled(){turn_joints::support_gain()*joint_gate}else{0.};
        let support_ownership=support_ownership+(1.-support_ownership)*cut_support;
        if let Some(walk) = &mut self.walk {
            let frames: Vec<_> = a.doll.frames().collect();
            let forward = pelvis_yaw(frames[0].rotation, walk.start[0].rotation, walk.forward) * Vec3::X;
            let position = (frames[6].translation - frames[3].translation).dot(forward);
            let velocity = (a.doll.motion(6).0 - a.doll.motion(3).0).dot(forward);
            let cost = |i: usize| {
                let (mut p, mut v) = walk.feet[i];
                if let Some(jog) = &self.jog {
                    let (mut high_p,mut high_v)=jog.feet[i];
                    if let Some((_,feet))=&jog.brisk {
                        let fraction=jog.fraction(gait_time);
                        high_p=feet[i].0+fraction*(high_p-feet[i].0); high_v=feet[i].1+fraction*(high_v-feet[i].1);
                    }
                    p += gait * (high_p - p); v += gait * (high_v - v);
                }
                (p - position).powi(2) + 0.01 * (v - velocity).powi(2)
                    + 0.001 * cycle_difference(i as f32 * DT, walk.phase, motion.duration).powi(2)
            };
            let best = (0..walk.feet.len()).min_by(|&i, &j| cost(i).total_cmp(&cost(j))).unwrap();
            let change = 0.2 * cycle_difference(best as f32 * DT, walk.phase, motion.duration);
            let advance = self.jog.as_ref().filter(|_| gait > 0.0).map_or(0.0, |j| {
                if j.brisk.is_none() { return DT*((1.0-gait)+gait*motion.duration/j.motion.duration); }
                let high_rate=j.brisk.as_ref().map_or(1.0/j.motion.duration,|(b,_)| (1.0-j.fraction(gait_time))/b.duration+j.fraction(gait_time)/j.motion.duration);
                DT*((1.0-gait)+gait*motion.duration*high_rate)
            });
            let fraction=self.jog.as_ref().filter(|j|!j.rehearsal).map_or(0.0,|j|j.fraction(gait_time));
            let correction = tracking::phase_correction(change,advance,fraction,self.tracking,DT);
            walk.phase = (walk.phase + advance + correction).rem_euclid(motion.duration);
        }
        if self.turning()&&turn::route_anchor()&&self.turn_origin.is_none()&&gait_time>=17. {
            let target=self.target(motion,self.time+DT);let actual=a.doll.frames().next().unwrap().translation;
            self.turn_origin=Some((actual-target[0].translation).with_z(0.).clamp_length_max(15.));
        }
        let target = self.target(motion, self.time + DT);
        let task_forward=self.task_forward(&target);
        let turning=self.turning();
        let entry_gain=if turning&&turn::entry_control(){turn::entry_gain(gait_time,self.body_heading(&a.doll.frames().collect::<Vec<_>>())*Vec3::X,task_forward)}else{1.};
        let task_velocity=if self.turning(){(self.target(motion,self.time+2.*DT)[0].translation-target[0].translation)/DT}else{Vec3::ZERO};
        if self.tracking==17 && !a.fallen {
            let handover=phase_support::handover(self.elapsed as f32*DT,self.gait(0.0));
            phase_support::apply(a,build,&target,DT,handover);
        }
        if let Some(walk) = &mut self.walk {
            if self.elapsed % 2 == 0 {
                let mut st = std::mem::take(&mut a.msgs);
                let direction=if matches!(self.tracking,14|15|16|17) {
                    let position=a.doll.frames().next().unwrap().translation;
                    let side=Vec3::Z.cross(task_forward);
                    walk.steer_yaw=alignment::steering(walk.steer_yaw,(position-target[0].translation).dot(side),(a.doll.motion(0).0-task_velocity).dot(side),alignment::path_gain()*alignment::jog_route_gain(real_jog));
                    Quat::from_rotation_z(walk.steer_yaw)*task_forward
                } else {walk.forward};
                if turning&&jog_stop::enabled()&&gait_time>18. {
                    turn::native_input(a,build,world,&mut st,direction,&mut walk.wander,&mut walk.rand,1.-jog_stop::brake_gate(gait_time));
                }
                else if turn_joints::enabled()&&joint_gate>0.&&turn_joints::push_release()>0. {
                    turn::native_input(a,build,world,&mut st,direction,&mut walk.wander,&mut walk.rand,1.-joint_gate*turn_joints::push_release());
                }
                else if turning&&turn::entry_control(){turn::native_input(a,build,world,&mut st,direction,&mut walk.wander,&mut walk.rand,entry_gain);}
                else{learn::walk_input(a, build, world, &mut st, direction, &mut walk.wander, &mut walk.rand);}
                a.msgs = st;
            }
            let input = learn::walk_obs(&a.doll, (self.time / motion.duration).rem_euclid(1.0));
            let base = walk.prior.as_ref().filter(|n| n.sizes[0] == input.len()).map_or_else(|| vec![0.0; 4], |n| n.frozen(&input));
            let settings: Vec<_> = base.iter().zip(&self.filtered).map(|(b, correction)| (b + tracking::balance_authority(self.tracking, gait) * correction).clamp(-1.0, 1.0)).collect();
            learn::tune_walk(a, &settings);
            let now: Vec<_> = a.doll.frames().collect();
            let mut rotations: Vec<_> = now.iter().map(|f| f.rotation).collect();
            if gait > 0.0 || self.tracking >= 2 {
                let jogging = &self.jog.as_ref().unwrap().motion;
                let jog_pose = jogging.sample((walk.phase + DT) / motion.duration * jogging.duration);
                  let native = a.limp_rage();
                for k in 1..7 {
                    let p = build.parts[k].parent.unwrap(); let offset = WALK_ACTIONS + (k - 1) * 3;
                    let delta = Quat::from_scaled_axis(Vec3::from_slice(&self.filtered[offset..offset + 3]) * 0.6);
                    let drive = &native[k];
                    let angles = if drive.hinge { [drive.desired, 0.0, 0.0] } else { [drive.target[2], drive.target[0], drive.target[1]] };
                    let base = drive.frame * delirium::nmrs::rotation(angles) * drive.child.inverse();
                    let local = if self.tracking >= 2 { target[p].rotation.inverse() * target[k].rotation }
                        else { jog_pose[p].rotation.inverse() * jog_pose[k].rotation };
                    let mut desired = tracking::leg_target(base, local, delta, gait, self.tracking);
                      if ownership>0. {
                          let residual=Quat::IDENTITY.slerp(delta,0.4+0.6*gait*gait);
                          desired=desired.slerp(local*residual,ownership);
                      }
                    if k==3||k==6 {
                        let ankle=self.ankle_feedback[usize::from(k==6)];
                        if ankle!=Vec3::ZERO {desired*=Quat::from_scaled_axis(ankle);}
                    }
                    rotations[k] = rotations[p] * desired;
                }
                let jog=self.jog.as_ref().unwrap();
                let desired_speed = if jog.brisk.is_none() {
                    (1.0 - gait) * motion.stride.length() / motion.duration + gait * jog.motion.stride.length() / jog.motion.duration
                } else {(1.0-gait)*motion.stride.length()/motion.duration+gait*jog.high_speed(gait_time)};
                let drive_direction=if turning{task_velocity.with_z(0.).normalize_or(task_forward)}else{walk.forward};
                let desired_speed=if turning{task_velocity.with_z(0.).length()}else{desired_speed};
                let error = (desired_speed - a.doll.motion(0).0.dot(drive_direction)).clamp(-1.5, 2.0);
                let original=drive_direction*(a.doll.mass()*1.5*error*gait);
                let force=if turning&&jog_stop::enabled(){
                    original.lerp(jog_stop::braking(task_velocity,a.doll.motion(0).0,a.doll.mass(),gait),jog_stop::brake_gate(gait_time))
                }else if turning&&turn::entry_control(){
                    let braking=(task_velocity.with_z(0.)*entry_gain-a.doll.motion(0).0.with_z(0.)).clamp_length_max(2.)*(a.doll.mass()*1.5*gait);
                    original.lerp(braking,turn::entry_window(gait_time))
                }else{original};
                a.push.push((0, now[0].translation, force));
            }
            let blend = (self.elapsed as f32 * DT).min(1.0);
            for k in 13..21 {
                let p = build.parts[k].parent.unwrap();
                let offset = 4 + (k - 13) * 3;
                let delta = Quat::from_scaled_axis(Vec3::from_slice(&self.filtered[offset..offset + 3]) * 0.12);
                let desired = target[p].rotation.inverse() * target[k].rotation * delta;
                let initial = walk.start[p].rotation.inverse() * walk.start[k].rotation;
                let mut local=initial.slerp(desired,blend);
                if whole_body::enabled(){local*=whole_body::arm(&self.filtered[66+(k-13)*3..69+(k-13)*3],whole_body::gate(turning,gait_time));}
                rotations[k] = rotations[p] * local;
            }
            a.set_stiffness_mask(6.0, 1.0, "ua");
            if self.tracking==10 && real_jog>0.05 {
                let stiffness=*JOG_STIFFNESS.get_or_init(||std::env::var("DLM_JOG_STIFFNESS").ok().and_then(|v|v.parse::<f32>().ok()).filter(|v|v.is_finite()&&(6.0..=16.0).contains(v)).unwrap_or(12.0));
                a.set_stiffness_mask(stiffness,1.0,"lb");
            }
            if matches!(self.tracking,12|13|14|15|16|17) && leg_support>0.05 {
                let mut desired=target.clone();let bounds:Vec<_>=a.doll.capsules().collect();
                for (i,k) in [3,6].into_iter().enumerate() {
                    desired[k].rotation*=now[k].rotation.inverse()*bounds[k].rotation;
                    let base=12.0+4.0*settings[0];
                    let strength=support::stiffness(base,self.filtered[0],a.doll.touch[k]&0x20!=0,bottom(build,&desired,k),leg_support);
                    let strength=if matches!(self.tracking,15|16|17) {support::leg_strength(strength,self.filtered[WALK_JOG_ACTIONS+i],leg_support)} else {strength};
                    a.set_stiffness_mask(strength,1.0,if i==0 {"ll"} else {"lr"});
                }
            }
              if self.tracking==17&&ankle_residual::enabled() {
                  for (k,i) in [(3,50),(6,52)] {
                      rotations[k]=ankle_residual::correction(rotations[k],task_forward,self.filtered[i],self.filtered[i+1],self.elapsed as f32*DT);
                  }
              }
              a.incoming = rotations.into_iter().map(Transform::from_rotation).collect();
            a.active_pose(Some("ua"));
              turn_torso::apply(a,&target,turning,gait_time);
              if whole_body::enabled(){whole_body::spine(a,&self.filtered[54..66],whole_body::gate(turning,gait_time));}
              if ownership==0. && (gait > 0.0 || self.tracking >= 2) { a.active_pose(Some("lb")); }
              if turn_joints::enabled(){turn_joints::apply(a,&self.filtered[90..104],turn_joints::gate(turning,gait_time));}
              if self.tracking==17 && engine_drive::configured() {
                  let ch=a.character().expect("native effectors required");
                  let rows=delirium::nmrs::incoming_rows(a.doll.nm,&a.incoming);
                  let targets=std::array::from_fn(|i|if cut_support>0. {ch.eff[i].desired}else{ch.incoming_angle(i,&rows)});
                  let strength=std::array::from_fn(|i|ch.eff[delirium::nmrs::HIP[i]].strength.sqrt().clamp(6.,24.));
                  let mut planned=target.clone();let bounds:Vec<_>=a.doll.capsules().collect();
                  for k in [3,6] {planned[k].rotation*=now[k].rotation.inverse()*bounds[k].rotation;}
                  a.set_locomotion_drive(Some(delirium::nm::LocomotionDrive{targets,stance:[bottom(build,&planned,3)<0.025,bottom(build,&planned,6)<0.025],
                      weight:support_ownership,gain_weight:ownership,leg_stiffness:strength,response:[support::response(self.filtered[48],leg_support),support::response(self.filtered[49],leg_support)]})).expect("valid native ownership command");
              }
              if self.tracking==17 {
                let scales=[support::response(self.filtered[48],leg_support),support::response(self.filtered[49],leg_support)];
                if scales!=[1.;2] {
                    let character=a.character().expect("native joint effectors required");
                    let base:[f32;6]=std::array::from_fn(|i|character.eff[i].muscle_stiffness);
                    let applied=std::array::from_fn(|i|base[i]*scales[i/3]);
                    let effectors:Vec<_>=character.eff.iter().enumerate().map(|(i,e)|
                        (e.desired,if i<6 {applied[i]} else {e.muscle_stiffness},e.strength,e.damping)).collect();
                    a.set_effectors(&effectors);self.leg_muscle_base=Some((base,applied));
                }
            }
            a.doll.kick(Vec3::ZERO);
            return;
        }
        let mut rotations = vec![a.doll.frames().next().unwrap().rotation];
        for k in 1..build.parts.len() {
            let p = build.parts[k].parent.unwrap();
            let delta = Quat::from_scaled_axis(Vec3::from_slice(&self.filtered[3 * (k - 1)..3 * k]) * residual_scale());
            rotations.push(rotations[p] * (target[p].rotation.inverse() * target[k].rotation) * delta);
        }
        a.set_stiffness(12.0, 1.0);
        delirium::nm::task_pose_control(a, rotations.into_iter().map(Transform::from_rotation).collect());
        a.doll.kick(Vec3::ZERO);
    }

    pub fn measure(&mut self, a: &Agent, build: &Build, motion: &Motion) -> (Quality, bool) {
        self.time += DT; self.elapsed += 1;
        let fr: Vec<_> = a.doll.frames().collect();
        let target = self.target(motion, self.time);
        let next = self.target(motion, self.time + DT);
        let mut pose = 0.0;
        for k in 1..fr.len() {
            let p = build.parts[k].parent.unwrap();
            pose += (fr[p].rotation.inverse() * fr[k].rotation).angle_between(target[p].rotation.inverse() * target[k].rotation).powi(2);
        }
        pose /= (fr.len() - 1) as f32;
        let inv = self.body_heading(&fr).inverse();
        let rt = if self.walk.is_some() {
            pelvis_yaw(target[0].rotation, self.heading * motion.frames[0][0].rotation,
                self.heading * motion.stride.normalize_or(Vec3::X)).inverse()
        } else { yaw(&target).inverse() };
        let points = fr.iter().zip(&target).map(|(a, b)| (inv * (a.translation - fr[0].translation)).distance_squared(rt * (b.translation - target[0].translation))).sum::<f32>() / fr.len() as f32;
        let desired_velocity = (next[0].translation - target[0].translation) / DT;
        let actual_velocity = a.doll.motion(0).0;
        let velocity = (actual_velocity - desired_velocity).length_squared();
        let mut arm_points = 0.0; let mut arm_velocity = 0.0;
        for k in [16, 20] {
            arm_points += (inv * (fr[k].translation - fr[0].translation)).distance_squared(rt * (target[k].translation - target[0].translation)) * 0.5;
            let desired = rt * ((next[k].translation - target[k].translation) / DT - desired_velocity);
            let actual = inv * (a.doll.motion(k).0 - actual_velocity);
            arm_velocity += actual.distance_squared(desired) * 0.5;
        }
        let mut slip = 0.0;
        let bounds: Vec<_> = a.doll.capsules().collect();
        for k in [3, 6] {
            if self.walk.is_some() {
                if bottom(build, &bounds, k) < 0.025 {
                    let local_up = bounds[k].rotation.inverse() * Vec3::Z;
                    let r = bounds[k].rotation * build.parts[k].boxed.map_or(-Vec3::Y * local_up.y.signum() * build.parts[k].half - local_up * build.parts[k].radius,
                        |half| -local_up.signum() * half);
                    let (v, w) = a.doll.motion(k);
                    slip += (v + w.cross(r)).truncate().length_squared() * 0.5;
                }
            } else if bottom(build, &target, k) < 0.04 {
                slip += a.doll.motion(k).0.truncate().length_squared() * 0.5;
            }
        }
        let count = if self.jog.as_ref().is_some_and(|j| j.rehearsal) { WALK_ACTIONS } else if matches!(self.tracking,15|16|17) {WALK_JOG_ACTIONS} else { self.filtered.len() };
        let extra_jitter=if matches!(self.tracking,15|16|17)&&count==WALK_JOG_ACTIONS {
            self.filtered[WALK_JOG_ACTIONS..].iter().zip(&self.previous[WALK_JOG_ACTIONS..]).map(|(a,b)|(a-b).powi(2)).sum::<f32>()
        } else {0.};
        let jitter = (self.filtered[..count].iter().zip(&self.previous).map(|(a, b)| (a - b).powi(2)).sum::<f32>()+extra_jitter) / count as f32;
        let height = (fr[0].translation.z - target[0].translation.z).powi(2);
        let mut imitation = 0.25 * (-2.0 * pose).exp() + 0.2 * (-40.0 * points).exp() + 0.15 * (-0.5 * velocity).exp()
            + 0.1 * (-2.0 * slip).exp() + 0.05 * (-20.0 * height).exp()
            + 0.15 * (-40.0 * arm_points).exp() + 0.1 * (-2.0 * arm_velocity).exp();
        if self.gait(0.0) > 0.5 {
            let mut expected_bounds = target.clone();
            for k in [3, 6] { expected_bounds[k].rotation *= fr[k].rotation.inverse() * bounds[k].rotation; }
            let contacts = [3, 6].iter().map(|&k| f32::from((bottom(build, &bounds, k) < 0.025) == (bottom(build, &expected_bounds, k) < 0.025))).sum::<f32>() * 0.5;
            imitation = 0.85 * imitation + 0.15 * contacts;
        }
        let (tracking_terms, fall_factor) = if self.tracking > 0 {
            let mut positions = 0.0; let mut velocities = 0.0;
            for k in 0..fr.len() {
                let shape = &build.parts[k];
                let half = shape.boxed.unwrap_or(Vec3::new(shape.radius, shape.half + shape.radius, shape.radius));
                let collider = fr[k].rotation.inverse() * bounds[k].rotation;
                let (v, w) = a.doll.motion(k);
                for axis in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
                    let offset = collider * (axis * half);
                    let actual_offset = fr[k].rotation * offset;
                    let desired_offset = target[k].rotation * offset;
                    positions += (rt * (fr[k].translation - fr[0].translation + actual_offset))
                        .distance_squared(rt * (target[k].translation - target[0].translation + desired_offset));
                    let desired = (next[k].translation + next[k].rotation * offset - target[k].translation - desired_offset) / DT;
                    velocities += (rt * (v + w.cross(actual_offset))).distance_squared(rt * desired);
                }
            }
            let head = rt * (fr[12].translation - fr[0].translation)
                - rt * (target[12].translation - target[0].translation);
            (tracking::terms(positions / fr.len() as f32, velocities / fr.len() as f32, pose, velocity),
                (1.3 - 1.4 * head.length_squared()).clamp(0.0, 1.0))
        } else { ([0.0; 4], 1.0) };
        let side_error = if matches!(self.tracking,7|8|9|10|11|12|13|14|15|16|17) {
            let side = Vec3::Z.cross(self.task_forward(&target));
            [9, 12].iter().map(|&k| tracking::lateral_error(fr[k].translation - fr[0].translation,
                target[k].translation - target[0].translation, side)).sum::<f32>() * 0.5
        } else { 0.0 };
        let fraction=self.jog.as_ref().filter(|j|!j.rehearsal).map_or(0.0,|j|j.fraction(self.elapsed as f32*DT+j.offset));
        let mut gait=[0.0;4]; let mut contact_cost=0.0;let mut support_cost=0.0;
        if fraction>0.05 {
            let mut desired_bounds=target.clone();
            for k in [3,6] { desired_bounds[k].rotation*=fr[k].rotation.inverse()*bounds[k].rotation; }
            let actual=[bottom(build,&bounds,3),bottom(build,&bounds,6)];
            let desired=[bottom(build,&desired_bounds,3),bottom(build,&desired_bounds,6)];
            gait=[1.0,f32::from(actual.iter().all(|&h|h>0.025)),f32::from(desired.iter().all(|&h|h>0.025)),
                [0,1].iter().map(|&i|f32::from((actual[i]<0.025)==(desired[i]<0.025))).sum::<f32>()*0.5];
            if matches!(self.tracking,8|9|10|11|12|13|14|15|16|17) {
                contact_cost=fraction*tracking::contact_cost(actual,desired,[a.doll.motion(3).0.z,a.doll.motion(6).0.z],
                    [(next[3].translation.z-target[3].translation.z)/DT,(next[6].translation.z-target[6].translation.z)/DT],
                    fr[0].translation.z-target[0].translation.z);
            }
            if matches!(self.tracking,12|13|14|15|16|17) {
                let desired_vz=(0..fr.len()).map(|k|(next[k].translation.z-target[k].translation.z)/DT*a.doll.part_mass(k)).sum::<f32>()/a.doll.mass();
                support_cost=support::cost(a.doll.com().1.z,desired_vz,
                    [a.doll.touch[3]&0x20!=0,a.doll.touch[6]&0x20!=0],actual,desired,fr[12].translation-fr[0].translation);
            }
        }
        let reward = if matches!(self.tracking,12|13|14|15|16|17)&&fraction>0.05 {support::reward(fall_factor*tracking_terms.iter().sum::<f32>()*0.25,support_cost,contact_cost,side_error,jitter,fraction)}
            else if self.tracking > 0 { fall_factor * tracking_terms.iter().sum::<f32>() * 0.25 - 0.05 * jitter - 16.0 * side_error - contact_cost }
            else { movement_reward(imitation, actual_velocity, desired_velocity, jitter) };
        let aligned=if matches!(self.tracking,13|14|15|16|17) {alignment::terms(&fr,&target,self.body_heading(&fr),rt.inverse(),
            self.task_forward(&target),(actual_velocity-if self.turning(){desired_velocity}else{Vec3::ZERO}).dot(Vec3::Z.cross(self.task_forward(&target))),slip)} else {[0.;4]};
        let reward=if matches!(self.tracking,13|14|15|16|17) {(reward-if matches!(self.tracking,14|15|16|17) {0.15}else{0.10}*alignment::cost(aligned)).max(0.02)} else {reward};
        let placement_cost=if self.tracking==17&&fraction>0.05&&std::env::var("DLM_REFERENCE_FOOT_PLACEMENT").as_deref()==Ok("1") {
            let actual=(fr[3].translation-fr[6].translation).dot(self.body_heading(&fr)*Vec3::Y);
            let reference=(target[3].translation-target[6].translation).dot(rt.inverse()*Vec3::Y);
            alignment::placement_cost(actual,reference)
        } else {0.};
        let mut style_cost=0.;
        if self.tracking==17&&support::style_tracking() {
            let heading=self.body_heading(&fr);let expected_heading=rt.inverse();
            for (hip,calf,foot) in [(1,2,3),(4,5,6)] {
                let collider=fr[foot].rotation.inverse()*bounds[foot].rotation;
                let actual_up=heading.inverse()*(bounds[foot].rotation*Vec3::Y);
                let expected_up=rt*(target[foot].rotation*collider*Vec3::Y);
                let ankle=fr[calf].translation+fr[calf].rotation*build.parts[foot].anchor_parent;
                let expected_ankle=target[calf].translation+target[calf].rotation*build.parts[foot].anchor_parent;
                let hip_position=fr[0].translation+fr[0].rotation*build.parts[hip].anchor_parent;
                let expected_hip=target[0].translation+target[0].rotation*build.parts[hip].anchor_parent;
                let offset=fr[hip].rotation*build.parts[calf].anchor_parent;
                let knee=fr[hip].translation+offset;
                let expected_knee=target[hip].translation+target[hip].rotation*build.parts[calf].anchor_parent;
                let next_knee=next[hip].translation+next[hip].rotation*build.parts[calf].anchor_parent;
                let (v,w)=a.doll.motion(hip);let velocity=(v+w.cross(offset)-actual_velocity).dot(heading*Vec3::Y);
                let expected_velocity=((next_knee-expected_knee)/DT-desired_velocity).dot(expected_heading*Vec3::Y);
                let mut expected_bounds=target.clone();expected_bounds[foot].rotation*=collider;
                let stance=((0.05-bottom(build,&expected_bounds,foot))/0.03).clamp(0.,1.);
                style_cost+=0.5*support::style_cost(actual_up,expected_up,
                    alignment::joint_knee(hip_position,knee,ankle,heading*Vec3::Y),
                    alignment::joint_knee(expected_hip,expected_knee,expected_ankle,expected_heading*Vec3::Y),velocity,expected_velocity,stance);
            }
        }
        let reward=reward-placement_cost+if self.tracking==17&&support::style_tracking() {support::style_bonus(style_cost)} else {0.};
        if std::env::var_os("DLM_REWARD_TRACE").is_some() {
            let mut desired_bounds=target.clone();
            for k in [3,6] {desired_bounds[k].rotation*=fr[k].rotation.inverse()*bounds[k].rotation;}
            let desired_vz=(0..fr.len()).map(|k|(next[k].translation.z-target[k].translation.z)/DT*a.doll.part_mass(k)).sum::<f32>()/a.doll.mass();
            audit::trace([self.elapsed as f32,fraction,f32::from(self.jog.as_ref().is_some_and(|j|j.practice)),
                fall_factor*tracking_terms.iter().sum::<f32>()*0.25,support_cost,contact_cost,side_error,jitter,alignment::cost(aligned),reward,
                a.doll.com().1.z,desired_vz,bottom(build,&bounds,3),bottom(build,&bounds,6),bottom(build,&desired_bounds,3),bottom(build,&desired_bounds,6),
                f32::from(a.doll.touch[3]&0x20!=0),f32::from(a.doll.touch[6]&0x20!=0),(fr[12].translation-fr[0].translation).normalize_or_zero().z,placement_cost]);
        }
        let fallen = fr.iter().any(|f| !f.translation.is_finite()) || !reward.is_finite()
            || fr[12].translation.z < 0.8 || fr[0].translation.z < 0.4;
        let q = Quality { pose: pose.sqrt(), slip: slip.sqrt(), jitter: jitter.sqrt(), speed: actual_velocity.dot(desired_velocity.with_z(0.0).normalize_or(Vec3::X)),
            reward: if fallen { if matches!(self.tracking,12|13|14|15|16|17)&&fraction>0.05 {-2.0} else {-0.5} } else { reward }, steps: 1, tracking: tracking_terms, gait, alignment:aligned,interaction:[0.;6],turn_origin:self.turn_origin.unwrap_or(Vec3::ZERO) };
        self.last_velocity = a.doll.motion(0).0;
        (q, fallen)
    }
}

fn movement_reward(imitation: f32, actual: Vec3, desired: Vec3, jitter: f32) -> f32 {
    let speed_squared = desired.truncate().length_squared().max(0.01);
    let progress = (actual.truncate().dot(desired.truncate()) / speed_squared).clamp(-1.0, 1.0);
    progress.max(0.0) * imitation - 0.1 * (1.0 - progress) - 0.05 * jitter
}

pub fn pretrained_server(build: &Build, world: &Collision, path: &str, balance: bool, stiffness: f32) -> Result<(), String> {
    if build.parts.len() != 21 || !stiffness.is_finite() || !(1.0..=20.0).contains(&stiffness) { return Err("invalid pretrained rig or stiffness".into()); }
    let motion = Motion::load(path, build)?;
    let reset = |phase| -> Result<_, String> {
        let mut control = Controller::new(&motion, phase);
        let mut a = control.agent(&motion, build);
        if balance {
            let mut state = std::mem::take(&mut a.msgs);
            delirium::msg::send(&mut a, build, world, &mut state, "bodyBalance", &[("start", delirium::msg::Value::Bool(true))]).map_err(|e| format!("bodyBalance: {e:?}"))?;
            a.msgs = state;
        }
        Ok((control, a))
    };
    let (mut control, mut a) = reset(0.0)?;
    let mut output = io::BufWriter::new(io::stdout().lock()); let mut input = io::stdin().lock();
    writeln!(output, "PRETRAINENV1 {} {}", build.parts.len(), 4 + build.parts.len() * 13).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    let packet = |output: &mut io::BufWriter<_>, a: &Agent, control: &Controller, reward: f32, done: bool| -> Result<(), String> {
        let mut row = vec![control.time, control.elapsed as f32, reward, f32::from(done)];
        for (k, frame) in a.doll.frames().enumerate() {
            row.extend(frame.translation.to_array()); row.extend(frame.rotation.to_array());
            let (v, w) = a.doll.motion(k); row.extend(v.to_array()); row.extend(w.to_array());
        }
        for x in row { output.write_all(&x.to_le_bytes()).map_err(|e| e.to_string())?; }
        Ok(())
    };
    let mut done = false;
    loop {
        let mut command = [0u8];
        match input.read_exact(&mut command) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()), Ok(()) => {}
        }
        match command[0] {
            0 => packet(&mut output, &a, &control, 0.0, done)?,
            1 => {
                let mut bytes = vec![0u8; build.parts.len() * 16]; input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
                let mut target = Vec::new();
                for bytes in bytes.chunks_exact(16) {
                    let q = Quat::from_array(std::array::from_fn(|j| f32::from_le_bytes(bytes[4*j..4*j+4].try_into().unwrap())));
                    if !q.is_finite() || (q.length() - 1.0).abs() > 0.01 { return Err("invalid pretrained target quaternion".into()); }
                    target.push(Transform::from_rotation(q.normalize()));
                }
                for _ in 0..2 {
                    let mut reward = 0.0;
                    if !done {
                        a.set_stiffness(stiffness, 1.0);
                        delirium::nm::task_pose_control(&mut a, target.clone()); a.doll.kick(Vec3::ZERO);
                        a.step(build, world, DT, &[], &[]);
                        let (q, fallen) = control.measure(&a, build, &motion); reward = q.reward;
                        done = fallen || control.elapsed >= LIMIT;
                    }
                    packet(&mut output, &a, &control, reward, done)?;
                }
            }
            2 => {
                let mut bytes = [0u8; 4]; input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
                let phase = f32::from_le_bytes(bytes);
                if !phase.is_finite() || !(0.0..=motion.duration).contains(&phase) { return Err("invalid reset phase".into()); }
                (control, a) = reset(phase)?; done = false;
                packet(&mut output, &a, &control, 0.0, done)?;
            }
            3 => break,
            _ => return Err("unknown pretrained command".into()),
        }
        output.flush().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn evaluation_phase(duration:f32,index:usize,shift:f32)->f32 {
    assert!(shift.is_finite()&&(0.0..1.0).contains(&shift));
    if shift==0. {duration*index as f32/8.} else {((index as f32/8.+shift)%1.)*duration}
}

fn evaluate(motion: &Motion, build: &Build, world: &Collision, net: Option<&Net>, phase: f32, start: Option<&[Transform]>, limit: usize) -> (Quality, Vec<Vec<Transform>>) {
    evaluate_gait(motion, build, world, net, phase, start, limit, None)
}

fn evaluate_gait(motion: &Motion, build: &Build, world: &Collision, net: Option<&Net>, phase: f32, start: Option<&[Transform]>, limit: usize, jog: Option<(&Motion, bool)>) -> (Quality, Vec<Vec<Transform>>) {
    let mut controller = start.map_or_else(|| Controller::new(motion, phase), |s| Controller::walking(motion, phase, s));
    if let Some((jog, rehearsal)) = jog { controller.with_jog(motion, jog, rehearsal); }
    if start.is_none()&&net.is_some_and(|n| n.sizes[3] == ACTIONS+crate::motion_tools::choices().len()) { controller.tools = Some(Default::default()); }
    let mut a = controller.agent_in(motion, build, world);
    let mut quality = Quality::default(); let mut recording = Vec::with_capacity(limit);
    let mut action = vec![0.0; controller.actions()];
    for n in 0..limit {
        if n % 2 == 0 {
            if let Some(net) = net { action = net.forward(&net.w, &controller.observation(&a, build, motion)); }
        }
        let native=a.character().map(|ch|ch.eff.iter().take(6).map(|e|e.desired).collect::<Vec<_>>());
        controller.drive(&mut a, build, world, motion, &action);
        if let (Some(before),Some(ch))=(native,a.character()) {
            quality.interaction[0]+=before.iter().zip(&ch.eff).map(|(b,e)|(Vec3::from(*b)-Vec3::from(e.desired)).length()).sum::<f32>()/6.;
            let target=controller.target(motion,controller.time+DT);let now:Vec<_>=a.doll.frames().collect();let bounds:Vec<_>=a.doll.capsules().collect();
            let mut desired=target;
            for (i,k) in [3,6].into_iter().enumerate() {
                desired[k].rotation*=now[k].rotation.inverse()*bounds[k].rotation;
                let planned=bottom(build,&desired,k)<0.025;
                let native_support=match ch.support[i] {-2=>a.doll.touch[k]&0x20!=0,v=>v>=0};
                quality.interaction[1]+=0.5*f32::from(planned!=native_support);
                quality.interaction[2+i]+=f32::from(native_support);
                quality.interaction[4+i]+=f32::from(planned&&!native_support);
            }
        }
        a.step(build, world, DT, &[], &[]);
        let (q, done) = controller.measure(&a, build, motion); quality.add(q);
        let fr = a.doll.frames().collect::<Vec<_>>(); recording.push(fr.clone());
        if done { recording.resize(limit, fr); break; }
    }
    (quality, recording)
}

pub fn probe(build: &Build, world: &Collision, path: &str) -> Result<(), String> {
    let motion = Motion::load(path, build)?;
    for k in 0..8 {
        let (q, _) = evaluate(&motion, build, world, None, motion.duration * k as f32 / 8.0, None, LIMIT);
        let v = q.values(LIMIT);
        println!("reference motors phase {k}: pose {:.3} rad, stance slip {:.3} m/s, speed {:.3} m/s, score {:.3}, survived {:.2} s", v[0], v[1], v[3], v[4], v[5]);
    }
    Ok(())
}

pub fn walking_probe(build: &Build, world: &Collision, start: &[Transform], path: &str) -> Result<(), String> {
    let motion = Motion::load(path, build)?;
    let mut baseline = learn::walk_agent(build, world, start)?;
    let forward = yaw(start) * Vec3::X;
    let (mut wander, mut rand) = (Default::default(), delirium_data::rng::Rand::new(1));
    let mut survived = 0;
    for i in 0..WALK_EVALUATION {
        if i % 2 == 0 {
            let mut st = std::mem::take(&mut baseline.msgs);
            learn::walk_input(&mut baseline, build, world, &mut st, forward, &mut wander, &mut rand);
            baseline.msgs = st;
        }
        baseline.doll.kick(Vec3::ZERO); baseline.step(build, world, DT, &[], &[]);
        survived += 1;
        if baseline.doll.frames().nth(12).unwrap().translation.z < 0.8 { break; }
    }
    let distance = (baseline.doll.frames().next().unwrap().translation - start[0].translation).dot(forward);
    println!("native WASD: {:.2} seconds, {:.3} metres", survived as f32 * DT, distance);
    for k in 0..8 {
        let (q, rec) = evaluate(&motion, build, world, None, motion.duration * k as f32 / 8.0, Some(start), WALK_EVALUATION);
        let v = q.values(WALK_EVALUATION);
        let d = (rec[q.steps - 1][0].translation - start[0].translation).dot(forward);
        println!("native walk + captured arms phase {k}: {:.2} seconds, {d:.3} metres, score {:.3}", v[5], v[4]);
        if q.steps != WALK_EVALUATION || d < distance * 0.85 { return Err(format!("assisted walking regressed at phase {k}")); }
    }
    Ok(())
}

pub fn check_tools(build: &Build, world: &Collision, path: &str) -> Result<(), String> {
    let motion = Motion::load(path, build)?;
    let names = crate::motion_tools::choices();
    for (pick, name) in names.iter().enumerate() {
        let mut control = Controller::new(&motion, 0.0); control.tools = Some(Default::default());
        let mut a = control.agent(&motion, build);
        let mut action = vec![0.0; control.actions()]; action[ACTIONS + pick] = 1.0;
        action[0] = 0.1;
        for _ in 0..12 {
            control.drive(&mut a, build, world, &motion, &action);
            let tools = control.tools.as_ref().unwrap();
            if tools.failed || tools.selected != pick { return Err(format!("{name} did not activate")); }
            if tools.supports_gait() && control.filtered[0] <= 0.0 { return Err(format!("{name} suppressed gait corrections")); }
            a.step(build, world, DT, &[], &[]);
            if *name == "bodyBalance" && !a.balancing() { return Err("bodyBalance did not remain active with gait control".into()); }
            control.measure(&a, build, &motion);
            if a.doll.frames().any(|f| !f.translation.is_finite()) { return Err(format!("{name}: nonfinite pose")); }
        }
        action[ACTIONS + pick] = 0.0; action[ACTIONS] = 1.0;
        control.drive(&mut a, build, world, &motion, &action);
        if control.tools.as_ref().unwrap().selected != 0 { return Err(format!("{name} did not stop")); }
        println!("behavior {name}: dispatch, hold and return to reference motors passed");
    }
    println!("{} behavior choices checked; {} registered messages catalogued", names.len(), crate::motion_tools::registry().count());
    Ok(())
}

type Policy = Vec<(usize, usize, Vec<f32>, Vec<f32>)>;

fn residual_scale() -> f32 {
    static S: OnceLock<f32> = OnceLock::new();
    *S.get_or_init(|| std::env::var("DLM_RESIDUAL_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(0.3))
}

fn plain_smoothing() -> f32 {
    static S: OnceLock<f32> = OnceLock::new();
    *S.get_or_init(|| std::env::var("DLM_ACTION_SMOOTHING").ok().and_then(|v| v.parse().ok()).unwrap_or(0.25))
}

fn le_bytes(v: &[f32]) -> Vec<u8> {
    let mut b = Vec::with_capacity(v.len() * 4);
    for x in v { b.extend_from_slice(&x.to_le_bytes()); }
    b
}

fn policy_forward(p: &Policy, x: &[f32]) -> Vec<f32> {
    let mut a = x.to_vec();
    for (n, m, wt, b) in p {
        let mut o = b.clone();
        for i in 0..*n { let ai = a[i]; for (o, w) in o.iter_mut().zip(&wt[i * m..(i + 1) * m]) { *o += w * ai; } }
        for v in &mut o { *v = v.tanh(); }
        a = o;
    }
    a
}

enum Job { Obs, Step(Vec<f32>), Roll(std::sync::Arc<(Policy, Vec<f32>)>, Vec<f32>, usize) }

struct Environment { a: Agent, control: Controller, quality: Quality, resets: usize }

impl Environment {
    fn new(motion: &Motion, build: &Build, world: &Collision, phase: f32, behaviors: bool, start: Option<&[Transform]>, jog: Option<(&Motion, bool)>) -> Self {
        let mut control = start.map_or_else(|| Controller::new(motion, phase), |s| Controller::walking(motion, phase, s));
        if let Some((jog, rehearsal)) = jog { control.with_jog(motion, jog, rehearsal); }
        if matches!(control.tracking,12|13|14|15|16|17) { if let Some(j)=control.jog.as_mut().filter(|j|!j.rehearsal) {j.offset=if turn::enabled(){turn::offset(0,0)}else{28.0};j.practice=true;j.practice_duration=turn::practice_duration(0,0);} }
        if behaviors { control.tools = Some(Default::default()); }
        let a = control.agent_in(motion, build, world);
        Self { a, control, quality: Quality::default(), resets: 0 }
    }

    fn step(&mut self, action: &[f32], motion: &Motion, build: &Build, world: &Collision, index: usize) -> Vec<f32> {
        let mut reward = 0.0; let mut done = false; let mut last = Quality::default();
        for _ in 0..2 {
            let t0 = std::time::Instant::now();
            self.control.drive(&mut self.a, build, world, motion, action);
            let t1 = std::time::Instant::now();
            self.a.step(build, world, DT, &[], &[]);
            let t2 = std::time::Instant::now();
            *self.control.poses.get_mut() = Some(Vec::new());
            let (q, fallen) = self.control.measure(&self.a, build, motion);
            let stance_bonus=if self.control.tracking==17 && stance_reward::enabled() && !fallen {
                let target=self.control.target(motion,self.control.time);
                let next=self.control.target(motion,self.control.time+DT);
                stance_reward::reward(&self.a,build,&target,(next[0].translation-target[0].translation)/DT)
            } else {0.};
            dbg_add(0, t1 - t0); dbg_add(1, t2 - t1); dbg_add(2, t2.elapsed());
            let fidelity=if self.control.tracking==17 && contact_fidelity::enabled() && !fallen && q.reward>0. {
                let target=self.control.target(motion,self.control.time);
                let next=self.control.target(motion,self.control.time+DT);
                contact_fidelity::factor(&self.a,build,&target,(next[0].translation-target[0].translation)/DT)
            } else {1.};
              let learning_reward=if !fallen {turn::learning_reward(&self.a,build,&self.control,motion,&q)} else {q.reward};
              reward += (learning_reward*fidelity+stance_bonus) * 0.5; self.quality.add(q); last.add(q);
            let skill_end=!turn::enabled()&&matches!(self.control.tracking,12|13|14|15|16|17)&&self.control.jog.as_ref().is_some_and(|j|!j.rehearsal&&j.offset==28.0)&&self.control.elapsed>=6*60;
            if fallen || skill_end || self.control.elapsed >= self.control.limit() { done = true; break; }
        }
        let t3 = std::time::Instant::now();
        if done {
            dbg_add(5, std::time::Duration::from_nanos(1));
            self.resets += 1;
            let phase = ((index * 37 + self.resets * 19) % 101) as f32 / 101.0 * motion.duration;
            let resets = self.resets;
            let start = self.control.walk.as_ref().map(|w| w.start.clone());
            let jog = self.control.jog.clone();
            let finite=jog.as_ref().is_some_and(|j|j.finite);
            *self = Self::new(motion, build, world, phase, self.control.tools.is_some(), start.as_deref(), jog.as_ref().map(|j| (&j.motion, j.rehearsal))); self.resets = resets;
            if let Some(jog) = self.control.jog.as_mut().filter(|j| !j.rehearsal) {
                jog.finite=finite;jog.practice_duration=turn::practice_duration(index,resets);
                let rehearsal_every=if alignment::transition_prior().is_some() {4} else {16};
                jog.offset = if finite{turn::offset(index,resets)}else if turn::enabled(){0.}else if matches!(self.control.tracking,12|13|14|15|16|17) {practice_schedule::offset(resets,rehearsal_every)} else {[0.0, 0.0, 12.0, 20.0, 28.0, 32.0, 44.0, 48.0][(index + resets) % 8]};
                jog.practice_limit=support::practice_blend(index,resets,f32::from_bits(JOG_LIMIT.load(Ordering::Relaxed)),
                    std::env::var("DLM_MIXED_JOG_PRACTICE").as_deref()==Ok("1"));
                self.a = self.control.agent_in(motion, build, world);
            }
        }
        let t4 = std::time::Instant::now(); dbg_add(3, t4 - t3);
        let mut row = self.control.observation(&self.a, build, motion);
        *self.control.poses.get_mut() = None;
        dbg_add(4, t4.elapsed());
        row.extend([reward, f32::from(done)]); row.extend(last.values(self.control.limit())); row
    }
}

pub fn server(build: &Build, world: &Collision, path: &str, model: &str, count: usize, behaviors: bool, start: Option<&[Transform]>, jog_path: Option<&str>) -> Result<(), String> {
    if build.parts.len() != 21 || !(1..=256).contains(&count) { return Err("motion environment requires Homer's 21-part rig and 1..256 environments".into()); }
    let motion = Motion::load(path, build)?;
    let jog = jog_path.map(|path| Motion::load(path, build)).transpose()?;
    if let Some(jog)=&jog { configure_bridge(build,&motion,jog)?; }
    if jog.is_some() && (start.is_none() || behaviors) { return Err("walk/jog transitions require native-assisted walking".into()); }
    if jog.as_ref().is_some_and(|j| j.stride.length() / j.duration <= motion.stride.length() / motion.duration) {
        return Err("jogging reference must have a genuinely higher captured speed".into());
    }
    if start.is_some() && behaviors { return Err("walking stage keeps native balance active; full-body skill selection is a later stage".into()); }
    let initial = Environment::new(&motion, build, world, 0.0, behaviors, start, jog.as_ref().map(|j| (j, false)));
    let actions = initial.control.actions();
    let obs = initial.control.observation(&initial.a, build, &motion).len();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(4).max(1)).min(count);
    let threads = std::env::var("WORKERS").ok().and_then(|w| w.parse().ok()).unwrap_or(threads).min(count);
    let mut output = io::BufWriter::new(io::stdout().lock()); let mut input = io::stdin().lock();
    let evaluation_limit = if start.is_some() { WALK_EVALUATION } else { LIMIT };
    let protocol = if initial.control.tracking > 0 { "MOTIONTRACK2" } else { "MOTIONENV2" };
    writeln!(output, "{protocol} {count} {obs} {actions} {METRICS} {} {evaluation_limit}", initial.control.limit()).map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())?;
    std::thread::scope(|scope| -> Result<(), String> {
        let mut workers = Vec::new();
        let standing = start;
        for start in (0..count).step_by(count.div_ceil(threads)) {
            let end = (start + count.div_ceil(threads)).min(count);
            let (tx, rx) = std::sync::mpsc::sync_channel::<Job>(1);
            let (out, result) = std::sync::mpsc::sync_channel::<Vec<f32>>(1);
            let motion = &motion;
            let jog = jog.as_ref();
            scope.spawn(move || {
                let mut envs: Vec<_> = (start..end).map(|i| {
                    let mut env=Environment::new(motion, build, world, motion.duration * i as f32 / count as f32, behaviors, standing, jog.map(|j| (j, i % 2 == 1)));
                    if turn::enabled()&&i%8==6 {if let Some(j)=env.control.jog.as_mut(){j.finite=false;j.offset=0.;}
                        env.a=env.control.agent_in(motion,build,world);}
                    env
                }).collect();
                while let Ok(job) = rx.recv() {
                    let tw = std::time::Instant::now();
                    let mut rows = Vec::new();
                    if let Job::Roll(p, noise, horizon) = &job {
                        let mut obs: Vec<_> = envs.iter().map(|env| env.control.observation(&env.a, build, motion)).collect();
                        let (n, obs_len) = (envs.len(), obs[0].len());
                        for t in 0..horizon.to_owned() {
                            for (j, env) in envs.iter_mut().enumerate() {
                                let z = &noise[(t * n + j) * actions..(t * n + j + 1) * actions];
                                let action: Vec<f32> = policy_forward(&p.0, &obs[j]).iter().zip(&p.1).zip(z).map(|((m, s), z)| m + s * z).collect();
                                env.control.observed = Some(std::mem::take(&mut obs[j]));
                                let row = env.step(&action, motion, build, world, start + j);
                                obs[j] = row[..obs_len].to_vec();
                                rows.extend(action); rows.extend(row);
                            }
                        }
                        dbg_add(6, tw.elapsed());
                        if out.send(rows).is_err() { break; }
                        continue;
                    }
                    let action = if let Job::Step(a) = job { Some(a) } else { None };
                    for (j, env) in envs.iter_mut().enumerate() {
                        if let Some(action) = &action {
                            rows.extend(env.step(&action[j * actions..(j + 1) * actions], motion, build, world, start + j));
                        } else {
                            rows.extend(env.control.observation(&env.a, build, motion));
                            rows.extend([0.0; 2 + METRICS]);
                        }
                    }
                    dbg_add(6, tw.elapsed());
                    if out.send(rows).is_err() { break; }
                }
            });
            workers.push((start, end, tx, result));
        }
        loop {
            let mut cmd = [0u8];
            match input.read_exact(&mut cmd) {
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.to_string()), Ok(()) => {}
            }
            match cmd[0] {
                0 | 1 => {
                    let action = if cmd[0] == 1 {
                        let mut bytes = vec![0u8; count * actions * 4]; input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
                        let action: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
                        if action.iter().any(|x| !x.is_finite()) { return Err("nonfinite action received".into()); }
                        Some(action)
                    } else { None };
                    let tr = std::time::Instant::now();
                    for (start, end, tx, _) in &workers {
                        tx.send(action.as_ref().map_or(Job::Obs, |a| Job::Step(a[start * actions..end * actions].to_vec()))).map_err(|e| e.to_string())?;
                    }
                    for (_, _, _, result) in &workers {
                        output.write_all(&le_bytes(&result.recv().map_err(|e| e.to_string())?)).map_err(|e| e.to_string())?;
                    }
                    dbg_add(7, tr.elapsed());
                }
                2 => {
                    let mut bytes = [0u8; 4]; input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
                    let generation = u32::from_le_bytes(bytes) as usize;
                    let net = Net::load(model).filter(|n| n.sizes[0] == obs && n.sizes[3] == actions).ok_or("incompatible motion policy")?;
                    let phase_shift=std::env::var("DLM_EVAL_PHASE_SHIFT").map_or(0.,|x|x.parse::<f32>().expect("invalid diagnostic phase shift"));
                    let results = std::thread::scope(|s| {
                        let handles: Vec<_> = (0..8).map(|i| { let (motion, net) = (&motion, &net);
                            let jog = jog.as_ref();
                            s.spawn(move || evaluate_gait(motion, build, world, Some(net), evaluation_phase(motion.duration,i,phase_shift), start, evaluation_limit, jog.map(|j| (j, i % 2 == 1)))) }).collect();
                        handles.into_iter().map(|h| h.join().unwrap()).collect::<Vec<_>>()
                    });
                    let scores: Vec<_> = results.iter().map(|r| r.0.values(evaluation_limit)[4]).collect();
                    let tries: Vec<_> = results.iter().map(|r| r.1.clone()).collect();
                    learn::save_live(&learn::live_path(model), generation, &scores, &tries);
                    if initial.control.tracking > 0 {
                        let terms: Vec<_> = results.iter().map(|r| r.0.tracking.map(|x| x / r.0.steps.max(1) as f32)).collect();
                        let gait:Vec<_>=results.iter().map(|r|r.0.gait).collect();
                        let alignment:Vec<_>=results.iter().map(|r|r.0.alignment.map(|v|v/r.0.steps.max(1) as f32)).collect();
                        let path:Vec<_>=results.iter().map(|r| {
                            let origin=start.map_or(r.1[0][0].translation,|s|s[0].translation);
                            let forward=start.map_or(motion.stride.normalize_or(Vec3::X),|s|yaw(s)*Vec3::X);
                            let side=Vec3::Z.cross(forward);let physical=&r.1[..r.0.steps];
                            let end=physical.last().unwrap()[0].translation-origin;
                            let max=physical.iter().map(|f|(f[0].translation-origin).dot(side).abs()).fold(0.0_f32,f32::max);
                            [end.dot(side),max,end.dot(forward)]
                        }).collect();
                        let interaction:Vec<_>=results.iter().map(|r|r.0.interaction.map(|v|v/r.0.steps.max(1) as f32)).collect();
                        let turn_report=if turn::enabled(){
                            let rows:Vec<_>=results.iter().enumerate().map(|(i,r)|{
                                if i%2==1{return [0.;6]}
                                let mut c=Controller::walking(&motion,evaluation_phase(motion.duration,i,phase_shift),start.unwrap());
                                c.with_jog(&motion,jog.as_ref().unwrap(),false);
                                let mut cross=0.0_f32;let mut position=0.;let mut heading=0.;let mut return_progress=0.;
                                let mut previous=turn::target(&c,&motion,0.)[0].translation;let mut arc=0.;let mut return_axis=Vec3::ZERO;let mut return_start=Vec3::ZERO;
                                for (n,fr) in r.1[..r.0.steps].iter().enumerate(){
                                    c.elapsed=n+1;if n>=17*60&&turn::route_anchor(){c.turn_origin=Some(r.0.turn_origin);}
                                    let target=c.target(&motion,c.time);let forward=c.task_forward(&target);
                                    let error=fr[0].translation-target[0].translation;
                                    cross=cross.max(error.dot(Vec3::Z.cross(forward)).abs());position+=error.with_z(0.).length_squared();
                                    let wanted=forward;let actual=c.body_heading(fr)*Vec3::X;
                                    if n>=turn::return_step(){heading+=(1.-actual.dot(wanted)).clamp(0.,2.);}
                                    if !turn::route_anchor()||n!=17*60{arc+=(target[0].translation-previous).with_z(0.).length();}previous=target[0].translation;
                                    if n==turn::return_step(){return_axis=wanted;return_start=fr[0].translation;}
                                    if n+1==r.0.steps&&n>turn::return_step(){return_progress=(fr[0].translation-return_start).dot(return_axis);}
                                }
                                [cross,(position/r.0.steps.max(1) as f32).sqrt(),heading/r.0.steps.saturating_sub(turn::return_step()).max(1) as f32,
                                    if r.0.steps>turn::return_step(){return_progress}else{0.},r.0.steps as f32*DT,arc]
                            }).collect();let anchors:Vec<_>=results.iter().map(|r|r.0.turn_origin.to_array()).collect();format!(",\"turn_return_start_seconds\":{},\"turn_reference_entry_shift\":{anchors:?},\"turn_columns\":[\"max_cross_track_m\",\"root_position_rmse_m\",\"return_heading_cost\",\"return_progress_m\",\"survival_s\",\"reference_arc_m\"],\"turn\":{rows:?}",turn::return_step() as f32*DT)
                        }else{String::new()};
                        let stop_report=if jog_stop::enabled(){let rows:Vec<_>=results.iter().enumerate().map(|(i,r)|if i%2==0{jog_stop::metrics(&r.1[..r.0.steps])}else{[0.;9]}).collect();format!(",\"stop_columns\":{:?},\"stop\":{rows:?}",jog_stop::COLUMNS)}else{String::new()};
                        let report = format!("{{\"update\":{generation},\"terms\":{terms:?},\"gait\":{gait:?},\"alignment\":{alignment:?},\"path_columns\":[\"signed_left_end_m\",\"max_abs_cross_track_m\",\"forward_end_m\"],\"path\":{path:?},\"interaction_columns\":[\"native_to_applied_joint_delta_rad\",\"native_vs_reference_support_mismatch_fraction\",\"native_left_support_fraction\",\"native_right_support_fraction\",\"reference_left_stance_native_excluded_fraction\",\"reference_right_stance_native_excluded_fraction\"],\"interaction\":{interaction:?}{turn_report}{stop_report}}}");
                        std::fs::write(format!("{model}.tracking.json"), report).map_err(|e| e.to_string())?;
                    }
                    for (q, _) in results { for x in q.values(evaluation_limit) { output.write_all(&x.to_le_bytes()).map_err(|e| e.to_string())?; } }
                }
                6 => {
                    let mut read = |n: usize| -> Result<Vec<f32>, String> {
                        let mut bytes = vec![0u8; n * 4]; input.read_exact(&mut bytes).map_err(|e| e.to_string())?;
                        let v: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
                        if v.iter().any(|x| !x.is_finite()) { return Err("nonfinite rollout input".into()); }
                        Ok(v)
                    };
                    let head = read(2)?; let (horizon, hidden) = (head[0] as usize, head[1] as usize);
                    let sizes = [obs, hidden, hidden, actions];
                    let mut p = Policy::new();
                    for l in 0..3 {
                        let (n, m) = (sizes[l], sizes[l + 1]);
                        let w = read(n * m)?; let b = read(m)?;
                        p.push((n, m, (0..n * m).map(|k| w[(k % m) * n + k / m]).collect(), b));
                    }
                    let shared = std::sync::Arc::new((p, read(actions)?));
                    let noise = read(horizon * count * actions)?;
                    for (start, end, tx, _) in &workers {
                        let mut z = Vec::with_capacity(horizon * (end - start) * actions);
                        for t in 0..horizon { z.extend_from_slice(&noise[(t * count + start) * actions..(t * count + end) * actions]); }
                        tx.send(Job::Roll(shared.clone(), z, horizon)).map_err(|e| e.to_string())?;
                    }
                    let blocks: Vec<Vec<f32>> = workers.iter().map(|w| w.3.recv().map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
                    let width = actions + obs + 2 + METRICS;
                    for t in 0..horizon {
                        for ((start, end, _, _), block) in workers.iter().zip(&blocks) {
                            let n = end - start;
                            output.write_all(&le_bytes(&block[t * n * width..(t + 1) * n * width])).map_err(|e| e.to_string())?;
                        }
                    }
                }
                3 => break,
                4 => {
                    let catalog = crate::motion_tools::catalog();
                    output.write_all(&(catalog.len() as u32).to_le_bytes()).map_err(|e| e.to_string())?;
                    output.write_all(catalog.as_bytes()).map_err(|e| e.to_string())?;
                }
                5 => {
                    for (frame, bounds) in initial.a.doll.frames().zip(initial.a.doll.capsules()) {
                        for x in (frame.rotation.inverse() * bounds.rotation).to_array() {
                            output.write_all(&x.to_le_bytes()).map_err(|e| e.to_string())?;
                        }
                    }
                }
                7 => {
                    if BRISK.get().is_none() { return Err("curriculum command requires three references".into()); }
                    let mut bytes=[0u8;4]; input.read_exact(&mut bytes).map_err(|e|e.to_string())?;
                    let limit=f32::from_le_bytes(bytes); set_jog_limit(limit)?;
                    output.write_all(&limit.to_le_bytes()).map_err(|e|e.to_string())?;
                }
                _ => return Err("unknown motion protocol command".into()),
            }
            output.flush().map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

static DBG: [std::sync::atomic::AtomicU64; 8] = [const { std::sync::atomic::AtomicU64::new(0) }; 8];
static DBGN: [std::sync::atomic::AtomicU64; 8] = [const { std::sync::atomic::AtomicU64::new(0) }; 8];
fn dbg_add(i: usize, d: std::time::Duration) {
    DBG[i].fetch_add(d.as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
    DBGN[i].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
pub fn dbg_print() {
    for (i, n) in ["drive", "step", "measure", "reset", "observation", "resets#", "worker batch", "round (main)"].iter().enumerate() {
        let (t, c) = (DBG[i].load(std::sync::atomic::Ordering::Relaxed), DBGN[i].load(std::sync::atomic::Ordering::Relaxed));
        eprintln!("DBG {n:14} calls {c:8} mean {:.1} us", t as f64 / 1e3 / c.max(1) as f64);
    }
}
