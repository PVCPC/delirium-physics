use std::collections::BTreeSet;
use std::f32::consts::PI;
use std::sync::Arc;

use bevy::math::{EulerRot, Mat3, Quat, Vec3};

use super::pcf::{OpDef, ParticleLibrary, SystemDef};

pub const MAX_CONTROL_POINTS: usize = 64;

const DRAG_REF_HZ: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlPoint {
    pub pos: Vec3,
    pub basis: Mat3,
    pub set: bool,
}

impl Default for ControlPoint {
    fn default() -> Self {
        Self { pos: Vec3::ZERO, basis: Mat3::IDENTITY, set: false }
    }
}

impl ControlPoint {
    pub fn new(pos: Vec3, basis: Mat3) -> Self {
        Self { pos, basis, set: true }
    }
    pub fn to_world(&self, local: Vec3) -> Vec3 {
        self.pos + self.basis * local
    }
    pub fn to_local(&self, world: Vec3) -> Vec3 {
        self.basis.transpose() * (world - self.pos)
    }
}

pub mod field {
    pub const XYZ: i32 = 0;
    pub const LIFE_DURATION: i32 = 1;
    pub const PREV_XYZ: i32 = 2;
    pub const RADIUS: i32 = 3;
    pub const ROTATION: i32 = 4;
    pub const ROTATION_SPEED: i32 = 5;
    pub const TINT_RGB: i32 = 6;
    pub const ALPHA: i32 = 7;
    pub const CREATION_TIME: i32 = 8;
    pub const SEQUENCE_NUMBER: i32 = 9;
    pub const TRAIL_LENGTH: i32 = 10;
    pub const YAW: i32 = 12;
    pub const SEQUENCE_NUMBER1: i32 = 13;
    pub const ALPHA2: i32 = 16;
    pub const NORMAL: i32 = 21;
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub pos: Vec3,
    pub prev: Vec3,
    pub lifetime: f32,
    pub radius: f32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub tint: Vec3,
    pub alpha: f32,
    pub alpha2: f32,
    pub created: f32,
    pub sequence: f32,
    pub sequence1: f32,
    pub trail_length: f32,
    pub yaw: f32,
    pub normal: Vec3,
    pub id: u32,
    pub init_radius: f32,
    pub init_alpha: f32,
    pub init_tint: Vec3,
    pub dead: bool,
}

impl Particle {
    pub fn scalar(&self, f: i32) -> f32 {
        match f {
            field::LIFE_DURATION => self.lifetime,
            field::RADIUS => self.radius,
            field::ROTATION => self.rotation,
            field::ROTATION_SPEED => self.rotation_speed,
            field::ALPHA => self.alpha,
            field::CREATION_TIME => self.created,
            field::SEQUENCE_NUMBER => self.sequence,
            field::TRAIL_LENGTH => self.trail_length,
            field::YAW => self.yaw,
            field::SEQUENCE_NUMBER1 => self.sequence1,
            field::ALPHA2 => self.alpha2,
            _ => 0.0,
        }
    }
    pub fn set_scalar(&mut self, f: i32, v: f32) {
        match f {
            field::LIFE_DURATION => self.lifetime = v,
            field::RADIUS => self.radius = v,
            field::ROTATION => self.rotation = v,
            field::ROTATION_SPEED => self.rotation_speed = v,
            field::ALPHA => self.alpha = v,
            field::CREATION_TIME => self.created = v,
            field::SEQUENCE_NUMBER => self.sequence = v,
            field::TRAIL_LENGTH => self.trail_length = v,
            field::YAW => self.yaw = v,
            field::SEQUENCE_NUMBER1 => self.sequence1 = v,
            field::ALPHA2 => self.alpha2 = v,
            _ => {}
        }
    }
    pub fn vector(&self, f: i32) -> Vec3 {
        match f {
            field::XYZ => self.pos,
            field::PREV_XYZ => self.prev,
            field::TINT_RGB => self.tint,
            field::NORMAL => self.normal,
            _ => Vec3::splat(self.scalar(f)),
        }
    }
    pub fn set_vector(&mut self, f: i32, v: Vec3) {
        match f {
            field::XYZ => self.pos = v,
            field::PREV_XYZ => self.prev = v,
            field::TINT_RGB => self.tint = v,
            field::NORMAL => self.normal = v,
            _ => self.set_scalar(f, v.x),
        }
    }
    fn is_vector_field(f: i32) -> bool {
        matches!(f, field::XYZ | field::PREV_XYZ | field::TINT_RGB | field::NORMAL)
    }
    fn age(&self, now: f32) -> f32 {
        now - self.created
    }
    fn life_fraction(&self, now: f32) -> f32 {
        if self.lifetime > 0.0 {
            self.age(now) / self.lifetime
        } else {
            0.0
        }
    }
}

fn hash(a: u32, b: u32, c: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

fn rnd(seed: u32, id: u32, salt: u32, k: u32) -> f32 {
    hash(id ^ seed, salt, k)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn remap_clamped(v: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    if (b - a).abs() < 1e-12 {
        return if v >= b { d } else { c };
    }
    c + (d - c) * ((v - a) / (b - a)).clamp(0.0, 1.0)
}
fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn bias(t: f32, b: f32) -> f32 {
    let b = b.clamp(1e-4, 1.0 - 1e-4);
    t.max(0.0).powf(b.ln() / 0.5f32.ln())
}
fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}
fn in_unit_sphere(r1: f32, r2: f32, r3: f32) -> Vec3 {
    let z = r1 * 2.0 - 1.0;
    let t = r2 * 2.0 * PI;
    let s = (1.0 - z * z).max(0.0).sqrt();
    Vec3::new(s * t.cos(), s * t.sin(), z) * r3.cbrt()
}

fn noise3(p: Vec3) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let grad = |c: Vec3| -> f32 {
        let h = hash(c.x as i32 as u32, c.y as i32 as u32, c.z as i32 as u32 ^ 0x5bd1);
        let theta = h * 2.0 * PI;
        let z = hash(c.x as i32 as u32 ^ 0x1234, c.y as i32 as u32, c.z as i32 as u32) * 2.0 - 1.0;
        let r = (1.0 - z * z).max(0.0).sqrt();
        let g = Vec3::new(r * theta.cos(), r * theta.sin(), z);
        g.dot(p - c)
    };
    let c000 = grad(i);
    let c100 = grad(i + Vec3::X);
    let c010 = grad(i + Vec3::Y);
    let c110 = grad(i + Vec3::new(1.0, 1.0, 0.0));
    let c001 = grad(i + Vec3::Z);
    let c101 = grad(i + Vec3::new(1.0, 0.0, 1.0));
    let c011 = grad(i + Vec3::new(0.0, 1.0, 1.0));
    let c111 = grad(i + Vec3::ONE);
    let x00 = lerp(c000, c100, u.x);
    let x10 = lerp(c010, c110, u.x);
    let x01 = lerp(c001, c101, u.x);
    let x11 = lerp(c011, c111, u.x);
    let y0 = lerp(x00, x10, u.y);
    let y1 = lerp(x01, x11, u.y);
    (lerp(y0, y1, u.z) * 1.6).clamp(-1.0, 1.0)
}

#[derive(Clone, Debug, Default)]
struct EmitterState {
    accumulated: f32,
    remaining: f32,
    started: bool,
    done: bool,
}

#[derive(Clone, Debug)]
pub struct ChildInstance {
    pub delay: f32,
    pub end_cap_only: bool,
    pub system: Option<SystemInstance>,
    name: String,
}

#[derive(Clone, Debug)]
pub struct SystemInstance {
    pub def: Arc<SystemDef>,
    pub time: f32,
    pub cps: [ControlPoint; MAX_CONTROL_POINTS],
    prev_cps: [ControlPoint; MAX_CONTROL_POINTS],
    pub particles: Vec<Particle>,
    next_id: u32,
    emitters: Vec<EmitterState>,
    pub end_cap: bool,
    pub children: Vec<ChildInstance>,
    seed: u32,
    first_update: bool,
    pub last_dt: f32,
    cur_dt: f32,
    step_spawned: u32,
    ring_counters: Vec<u32>,
    pub emitted_last: u32,
    pub unsupported: BTreeSet<String>,
}

const KNOWN_EMITTERS: &[&str] = &["emit_instantaneously", "emit_continuously"];
const KNOWN_INITIALIZERS: &[&str] = &[
    "Position Within Sphere Random",
    "Position Along Ring",
    "Position Modify Warp Random",
    "Position Modify Offset Random",
    "Position Along Path Sequential",
    "Lifetime Random",
    "Alpha Random",
    "Color Random",
    "Radius Random",
    "Rotation Random",
    "Rotation Speed Random",
    "Sequence Random",
    "Trail Length Random",
    "Velocity Random",
    "Normal Modify Offset Random",
    "Velocity Repulse from World",
    "Remap Noise to Scalar",
    "Remap Particle Count to Scalar",
    "Remap Control Point to Vector",
    "Remap Initial Distance to Control Point to Scalar",
    "remap initial scalar",
];
const KNOWN_OPERATORS: &[&str] = &[
    "Set Control Point Rotation",
    "Set Control Point Positions",
    "Movement Basic",
    "Movement Lock to Control Point",
    "Movement Dampen Relative to Control Point",
    "Movement Max Velocity",
    "Lifespan Decay",
    "Lifespan Minimum Radius Decay",
    "Lifespan Minimum Alpha Decay",
    "Alpha Fade In Simple",
    "Alpha Fade Out Simple",
    "Alpha Fade In Random",
    "Alpha Fade Out Random",
    "Alpha Fade and Decay",
    "Color Fade",
    "Radius Scale",
    "Clamp Scalar",
    "Ramp Scalar Linear Simple",
    "Oscillate Scalar",
    "Oscillate Vector",
    "Remap Direction to CP to Vector",
    "Remap Control Point to Vector",
    "Rotation Basic",
    "Cull when crossing sphere",
    "Pull towards control point",
    "random force",
    "twist around axis",
    "Constrain distance to control point",
];

fn unsupported_ops(def: &SystemDef) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for e in &def.emitters {
        if !KNOWN_EMITTERS.contains(&e.function.as_str()) {
            out.insert(format!("emitter {}", e.function));
        }
    }
    for i in &def.initializers {
        if !KNOWN_INITIALIZERS.contains(&i.function.as_str()) {
            out.insert(format!("initializer {}", i.function));
        }
    }
    for o in &def.operators {
        if !KNOWN_OPERATORS.contains(&o.function.as_str()) {
            out.insert(format!("operator {}", o.function));
        }
    }
    for f in &def.forces {
        if !KNOWN_OPERATORS.contains(&f.function.as_str()) {
            out.insert(format!("force {}", f.function));
        }
    }
    for c in &def.constraints {
        if !KNOWN_OPERATORS.contains(&c.function.as_str()) {
            out.insert(format!("constraint {}", c.function));
        }
    }
    out
}

impl SystemInstance {
    pub fn carry(&mut self, pt: &dyn Fn(Vec3) -> Vec3, dir: &dyn Fn(Vec3) -> Vec3) {
        for p in &mut self.particles {
            p.pos = pt(p.pos);
            p.prev = pt(p.prev);
            p.normal = dir(p.normal);
        }
        let map_cp = |c: &mut ControlPoint| {
            c.pos = pt(c.pos);
            c.basis = Mat3::from_cols(dir(c.basis.x_axis), dir(c.basis.y_axis), dir(c.basis.z_axis));
        };
        self.cps.iter_mut().for_each(map_cp);
        self.prev_cps.iter_mut().for_each(map_cp);
        for child in &mut self.children {
            if let Some(sys) = child.system.as_mut() {
                sys.carry(pt, dir);
            }
        }
    }

    pub fn new(def: Arc<SystemDef>, seed: u32) -> Self {
        let children = def
            .children
            .iter()
            .map(|c| ChildInstance { delay: c.delay, end_cap_only: c.end_cap, system: None, name: c.name.clone() })
            .collect();
        let emitters = vec![EmitterState::default(); def.emitters.len()];
        let ring_counters = vec![0; def.initializers.len()];
        let unsupported = unsupported_ops(&def);
        Self {
            def,
            time: 0.0,
            cps: [ControlPoint::default(); MAX_CONTROL_POINTS],
            prev_cps: [ControlPoint::default(); MAX_CONTROL_POINTS],
            particles: Vec::new(),
            next_id: 0,
            emitters,
            end_cap: false,
            children,
            seed,
            first_update: true,
            last_dt: 1.0 / 60.0,
            cur_dt: 0.0,
            step_spawned: 0,
            ring_counters,
            emitted_last: 0,
            unsupported,
        }
    }

    pub fn set_cp(&mut self, i: usize, cp: ControlPoint) {
        if i < MAX_CONTROL_POINTS {
            self.cps[i] = cp;
        }
        for c in &mut self.children {
            if let Some(s) = &mut c.system {
                s.set_cp(i, cp);
            }
        }
    }

    pub fn stop(&mut self) {
        self.end_cap = true;
        for c in &mut self.children {
            if let Some(s) = &mut c.system {
                s.stop();
            }
        }
    }

    pub fn finished(&self) -> bool {
        let emitting = !self.end_cap && self.emitters.iter().any(|e| !e.done);
        !emitting
            && self.particles.is_empty()
            && self.children.iter().all(|c| c.system.as_ref().is_none_or(SystemInstance::finished))
            && (self.end_cap || self.children.iter().all(|c| c.system.is_some() || c.end_cap_only))
            && (self.end_cap || !self.def.emitters.is_empty())
    }

    fn cp(&self, i: i32) -> ControlPoint {
        self.cps[i.clamp(0, 63) as usize]
    }

    pub fn update(&mut self, dt: f32, lib: &ParticleLibrary) {
        let def = self.def.clone();
        let dt = dt.clamp(0.0, def.maximum_time_step.max(1e-3));
        if dt <= 0.0 {
            return;
        }
        let cp0 = self.cps[0];
        for cp in self.cps.iter_mut().skip(1) {
            if !cp.set {
                *cp = ControlPoint { set: false, ..cp0 };
            }
        }
        if self.first_update {
            self.prev_cps = self.cps;
        }
        self.cur_dt = dt;
        self.time += dt;
        let now = self.time;
        self.emitted_last = 0;

        for op in &def.operators {
            if !self.op_active(op) {
                continue;
            }
            match op.function.as_str() {
                "Set Control Point Rotation" => self.op_cp_rotation(op, dt),
                "Set Control Point Positions" => self.op_cp_positions(op),
                _ => {}
            }
        }

        if !self.end_cap {
            let mut counts = Vec::with_capacity(def.emitters.len());
            for (ei, em) in def.emitters.iter().enumerate() {
                counts.push(self.emit_count(ei, em, dt));
            }
            self.step_spawned = counts.iter().sum::<usize>() as u32;
            for n in counts {
                for _ in 0..n {
                    self.spawn_particle(now - dt * 0.5);
                }
            }
        }

        self.run_operators(&def, dt, now);
        self.run_forces(&def, dt);
        self.run_constraints(&def);
        self.particles.retain(|p| !p.dead);

        for ci in 0..self.children.len() {
            let start = self.children[ci].delay;
            let only_end = self.children[ci].end_cap_only;
            if self.children[ci].system.is_none() && now >= start && (!only_end || self.end_cap) {
                if let Some(child_def) = lib.get(&self.children[ci].name) {
                    let mut s = SystemInstance::new(child_def, self.seed.wrapping_add(ci as u32 * 7777 + 1));
                    for (i, cp) in self.cps.iter().enumerate() {
                        if cp.set || i == 0 {
                            s.cps[i] = *cp;
                        }
                    }
                    s.cps[0].set = true;
                    if self.end_cap && !only_end {
                        s.end_cap = true;
                    }
                    self.children[ci].system = Some(s);
                }
            }
            let cps = self.cps;
            if let Some(s) = &mut self.children[ci].system {
                for (i, cp) in cps.iter().enumerate() {
                    if cp.set {
                        s.cps[i] = *cp;
                    }
                }
                s.update(dt, lib);
            }
        }
        self.last_dt = dt;
        self.prev_cps = self.cps;
        self.first_update = false;
    }

    fn op_active(&self, op: &OpDef) -> bool {
        match op.end_cap_state() {
            0 => !self.end_cap,
            1 => self.end_cap,
            _ => true,
        }
    }

    fn op_strength(op: &OpDef, now: f32) -> f32 {
        let (fin_s, fin_e) = (op.f("operator start fadein", 0.0), op.f("operator end fadein", 0.0));
        let (fout_s, fout_e) = (op.f("operator start fadeout", 0.0), op.f("operator end fadeout", 0.0));
        let mut strength = 1.0;
        if fin_e > fin_s {
            strength *= remap_clamped(now, fin_s, fin_e, 0.0, 1.0);
        }
        if fout_e > fout_s {
            strength *= remap_clamped(now, fout_s, fout_e, 1.0, 0.0);
        }
        strength
    }

    fn op_cp_rotation(&mut self, op: &OpDef, dt: f32) {
        let axis = v3(op.v3("Rotation Axis", [0.0, 0.0, 1.0])).normalize_or_zero();
        let rate = op.f("Rotation Rate", 180.0).to_radians();
        let i = op.i("Control Point", 0).clamp(0, 63) as usize;
        let local = op.i("Local Space Control Point", -1);
        let axis = if local >= 0 { self.cps[local.min(63) as usize].basis * axis } else { axis };
        if axis != Vec3::ZERO && rate != 0.0 {
            let q = Quat::from_axis_angle(axis.normalize(), rate * dt);
            self.cps[i].basis = Mat3::from_quat(q) * self.cps[i].basis;
            self.cps[i].set = true;
        }
    }

    fn op_cp_positions(&mut self, op: &OpDef) {
        let from = op.i("Control Point to offset positions from", 0).clamp(0, 63) as usize;
        let world = op.b("Set positions in world space", false);
        let base = self.cps[from];
        let entries = [
            ("First Control Point Number", "First Control Point Location", 1, [128.0, 0.0, 0.0]),
            ("Second Control Point Number", "Second Control Point Location", 2, [0.0, 128.0, 0.0]),
            ("Third Control Point Number", "Third Control Point Location", 3, [-128.0, 0.0, 0.0]),
            ("Fourth Control Point Number", "Fourth Control Point Location", 4, [0.0, -128.0, 0.0]),
        ];
        for (nk, lk, nd, ld) in entries {
            let n = op.i(nk, nd).clamp(0, 63) as usize;
            let l = v3(op.v3(lk, ld));
            let pos = if world { l } else { base.to_world(l) };
            self.cps[n] = ControlPoint { pos, basis: base.basis, set: true };
        }
    }

    fn emit_count(&mut self, ei: usize, em: &OpDef, dt: f32) -> usize {
        let now = self.time;
        let r = rnd(self.seed, ei as u32, 0xE1, 7);
        let start_max = em.f("emission_start_time max", -1.0);
        let start = if start_max >= 0.0 {
            lerp(em.f("emission_start_time", 0.0), start_max, r)
        } else {
            em.f("emission_start_time", 0.0)
        };
        let per_step_cap = match em.i("maximum emission per frame", -1) {
            m if m > 0 => m as usize,
            _ => self.def.max_particles,
        };
        let st = &mut self.emitters[ei];
        if st.done || now < start {
            return 0;
        }
        match em.function.as_str() {
            "emit_instantaneously" => {
                if !st.started {
                    st.started = true;
                    let num = em.i("num_to_emit", 100).max(0) as f32;
                    let min = em.i("num_to_emit_minimum", -1);
                    st.remaining = if min >= 0 { lerp(min as f32, num, r).round() } else { num };
                }
                let n = st.remaining.min(per_step_cap as f32).max(0.0).floor();
                st.remaining -= n;
                if st.remaining < 1.0 {
                    st.done = true;
                }
                n as usize
            }
            "emit_continuously" => {
                let duration = em.f("emission_duration", 0.0);
                if duration > 0.0 && now > start + duration {
                    st.done = true;
                    return 0;
                }
                st.accumulated += em.f("emission_rate", 100.0) * dt;
                let n = st.accumulated.floor();
                st.accumulated -= n;
                n as usize
            }
            _ => {
                st.done = true;
                0
            }
        }
    }

    fn spawn_particle(&mut self, created: f32) {
        if self.particles.len() >= self.def.max_particles {
            return;
        }
        let def = self.def.clone();
        let c = def.color;
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.emitted_last += 1;
        let cp0 = self.cps[0].pos;
        let mut p = Particle {
            pos: cp0,
            prev: cp0,
            lifetime: 1.0,
            radius: def.radius,
            rotation: def.rotation.to_radians(),
            rotation_speed: def.rotation_speed.to_radians(),
            tint: Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) / 255.0,
            alpha: c[3] as f32 / 255.0,
            alpha2: 1.0,
            created,
            sequence: def.sequence_number as f32,
            sequence1: 0.0,
            trail_length: 0.1,
            yaw: 0.0,
            normal: Vec3::Z,
            id,
            init_radius: 0.0,
            init_alpha: 0.0,
            init_tint: Vec3::ZERO,
            dead: false,
        };
        let rank = |f: &str| match f {
            "Position Within Sphere Random" | "Position Along Ring" | "Position Along Path Sequential" => 0,
            "Position Modify Warp Random" | "Position Modify Offset Random" | "Velocity Repulse from World" => 1,
            _ => 2,
        };
        let mut order: Vec<usize> = (0..def.initializers.len()).collect();
        order.sort_by_key(|&i| rank(&def.initializers[i].function));
        for oi in order {
            self.run_initializer(oi, &def.initializers[oi], &mut p);
        }
        p.init_radius = p.radius;
        p.init_alpha = p.alpha;
        p.init_tint = p.tint;
        self.particles.push(p);
    }

    fn set_initial(p: &mut Particle, f: i32, v: f32, scale_initial: bool) {
        let v = if f == field::ROTATION || f == field::YAW { v.to_radians() } else { v };
        if Particle::is_vector_field(f) {
            let cur = p.vector(f);
            p.set_vector(f, if scale_initial { cur * v } else { Vec3::splat(v) });
        } else {
            let cur = p.scalar(f);
            p.set_scalar(f, if scale_initial { cur * v } else { v });
        }
    }

    fn run_initializer(&mut self, oi: usize, op: &OpDef, p: &mut Particle) {
        let seed = self.seed;
        let id = p.id;
        let salt = oi as u32 * 7919 + 17;
        let r = |k: u32| rnd(seed, id, salt, k);
        let rexp = |k: u32, lo: f32, hi: f32, e: f32| lerp(lo, hi, r(k).powf(e.max(1e-3)));
        let dt = self.cur_dt;
        match op.function.as_str() {
            "Position Within Sphere Random" => {
                let cp = self.cp(op.i("control_point_number", 0));
                let dmin = op.f("distance_min", 0.0);
                let dmax = op.f("distance_max", 0.0);
                let bias_v = v3(op.v3("distance_bias", [1.0, 1.0, 1.0]));
                let abs = v3(op.v3("distance_bias_absolute_value", [0.0, 0.0, 0.0]));
                let local_bias = op.b("bias in local system", false);
                let d0 = in_unit_sphere(r(1), r(2), r(3));
                let mut d = d0 * bias_v;
                if abs.x != 0.0 {
                    d.x = d.x.abs();
                }
                if abs.y != 0.0 {
                    d.y = d.y.abs();
                }
                if abs.z != 0.0 {
                    d.z = d.z.abs();
                }
                let dir = d.normalize_or_zero();
                let dist = lerp(dmin, dmax, r(10));
                let world_dir = if local_bias { cp.basis * dir } else { dir };
                p.pos = cp.pos + world_dir * dist;
                let smin = op.f("speed_min", 0.0);
                let smax = op.f("speed_max", 0.0);
                let sexp = op.f("speed_random_exponent", 1.0);
                let speed = rexp(11, smin, smax, sexp);
                let lmin = v3(op.v3("speed_in_local_coordinate_system_min", [0.0; 3]));
                let lmax = v3(op.v3("speed_in_local_coordinate_system_max", [0.0; 3]));
                let local_v = Vec3::new(lerp(lmin.x, lmax.x, r(12)), lerp(lmin.y, lmax.y, r(13)), lerp(lmin.z, lmax.z, r(14)));
                let vel = world_dir * speed + cp.basis * local_v;
                p.prev = p.pos - vel * dt;
            }
            "Position Along Ring" => {
                let cp = self.cp(op.i("control point number", 0));
                let radius = op.f("initial radius", 0.0);
                let thick = op.f("thickness", 0.0);
                let even = op.b("even distribution", false);
                let count = op.f("even distribution count", -1.0);
                let n = if even && count > 0.0 { count } else { self.step_spawned.max(1) as f32 };
                let step = 2.0 * PI / n;
                let counter = self.ring_counters[oi];
                self.ring_counters[oi] = counter.wrapping_add(1);
                let angle = if even { counter as f32 * step } else { r(1) * 2.0 * PI };
                let ring = Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0);
                let rot = Quat::from_euler(
                    EulerRot::ZYX,
                    op.f("yaw", 0.0).to_radians(),
                    op.f("pitch", 0.0).to_radians(),
                    op.f("roll", 0.0).to_radians(),
                );
                let local = rot * ring + in_unit_sphere(r(2), r(3), r(4)) * thick;
                p.pos = cp.to_world(local);
                let speed = lerp(op.f("min initial speed", 0.0), op.f("max initial speed", 0.0), r(5));
                let outward = (p.pos - cp.pos).normalize_or_zero();
                p.prev = p.pos - outward * speed * dt;
                if op.b("XY velocity only", true) {
                    p.prev.z = p.pos.z;
                }
            }
            "Position Modify Warp Random" => {
                let cp = self.cp(op.i("control point number", 0));
                let wmin = v3(op.v3("warp min", [1.0; 3]));
                let wmax = v3(op.v3("warp max", [1.0; 3]));
                let w = Vec3::new(lerp(wmin.x, wmax.x, r(1)), lerp(wmin.y, wmax.y, r(2)), lerp(wmin.z, wmax.z, r(3)));
                p.pos = cp.to_world(cp.to_local(p.pos) * w);
                p.prev = cp.to_world(cp.to_local(p.prev) * w);
            }
            "Position Modify Offset Random" => {
                let cp = self.cp(op.i("control_point_number", 0));
                let omin = v3(op.v3("offset min", [0.0; 3]));
                let omax = v3(op.v3("offset max", [0.0; 3]));
                let mut o = Vec3::new(lerp(omin.x, omax.x, r(1)), lerp(omin.y, omax.y, r(2)), lerp(omin.z, omax.z, r(3)));
                if op.b("offset proportional to radius 0/1", false) {
                    o *= p.radius;
                }
                if op.b("offset in local space 0/1", false) {
                    o = cp.basis * o;
                }
                p.pos += o;
                p.prev += o;
            }
            "Velocity Repulse from World" => {
                if op.b("Offset instead of accelerate", false) {
                    let o = self.cp(0).basis * Vec3::new(0.0, 0.0, op.f("Trace Length", 64.0));
                    p.pos += o;
                    p.prev += o;
                }
            }
            "Position Along Path Sequential" => {
                let start = self.cp(op.i("start control point number", 0)).pos;
                let end = self.cp(op.i("end control point number", 0)).pos;
                let n = op.f("particles to map from start to end", 100.0).max(1.0);
                let loops = op.b("restart behavior (0 = bounce, 1 = loop )", true);
                let mut t = (id as f32 % n) / n;
                if !loops && (id as f32 / n) as u32 % 2 == 1 {
                    t = 1.0 - t;
                }
                let bulge = op.f("bulge", 0.0);
                let mut pos = start.lerp(end, t);
                if bulge != 0.0 {
                    let d = end - start;
                    let side = d.cross(Vec3::Z).normalize_or_zero();
                    pos += side * bulge * (t * PI).sin();
                }
                p.pos = pos;
                p.prev = pos;
            }
            "Lifetime Random" => {
                p.lifetime = rexp(1, op.f("lifetime_min", 0.0), op.f("lifetime_max", 0.0), op.f("lifetime_random_exponent", 1.0));
            }
            "Alpha Random" => {
                let a = rexp(1, op.f("alpha_min", 255.0), op.f("alpha_max", 255.0), op.f("alpha_random_exponent", 1.0));
                p.alpha = a / 255.0;
            }
            "Color Random" => {
                let c1 = op.color("color1", [255; 4]);
                let c2 = op.color("color2", [255; 4]);
                let t = r(1);
                p.tint = Vec3::new(
                    lerp(c1[0] as f32, c2[0] as f32, t),
                    lerp(c1[1] as f32, c2[1] as f32, t),
                    lerp(c1[2] as f32, c2[2] as f32, t),
                ) / 255.0;
            }
            "Radius Random" => {
                p.radius = rexp(1, op.f("radius_min", 1.0), op.f("radius_max", 1.0), op.f("radius_random_exponent", 1.0));
            }
            "Rotation Random" => {
                let init = op.f("rotation_initial", 0.0);
                let off = rexp(1, op.f("rotation_offset_min", 0.0), op.f("rotation_offset_max", 360.0), op.f("rotation_random_exponent", 1.0));
                p.rotation = (init + off).to_radians();
            }
            "Rotation Speed Random" => {
                let s = rexp(1, op.f("rotation_speed_random_min", 0.0), op.f("rotation_speed_random_max", 360.0), op.f("rotation_speed_random_exponent", 1.0));
                p.rotation_speed = (op.f("rotation_speed_constant", 0.0) + s).to_radians();
            }
            "Sequence Random" => {
                let lo = op.i("sequence_min", 0);
                let hi = op.i("sequence_max", 0).max(lo);
                p.sequence = (lo + ((hi - lo + 1) as f32 * r(1)) as i32).min(hi) as f32;
            }
            "Trail Length Random" => {
                p.trail_length = rexp(1, op.f("length_min", 0.1), op.f("length_max", 0.1), op.f("length_random_exponent", 1.0));
            }
            "Velocity Random" => {
                let cp = self.cp(op.i("control_point_number", 0));
                let smin = op.f("random_speed_min", 0.0);
                let smax = op.f("random_speed_max", 0.0);
                let lmin = v3(op.v3("speed_in_local_coordinate_system_min", [0.0; 3]));
                let lmax = v3(op.v3("speed_in_local_coordinate_system_max", [0.0; 3]));
                let local_v = Vec3::new(lerp(lmin.x, lmax.x, r(1)), lerp(lmin.y, lmax.y, r(2)), lerp(lmin.z, lmax.z, r(3)));
                let rand_v = in_unit_sphere(r(4), r(5), 1.0).normalize_or_zero() * lerp(smin, smax, r(6));
                let vel = cp.basis * local_v + rand_v;
                p.prev -= vel * dt;
            }
            "Normal Modify Offset Random" => {
                let cp = self.cp(op.i("control_point_number", 0));
                let omin = v3(op.v3("offset min", [0.0; 3]));
                let omax = v3(op.v3("offset max", [0.0; 3]));
                let mut o = Vec3::new(lerp(omin.x, omax.x, r(1)), lerp(omin.y, omax.y, r(2)), lerp(omin.z, omax.z, r(3)));
                if op.b("offset in local space 0/1", false) {
                    o = cp.basis * o;
                }
                p.normal += o;
                if op.b("normalize output 0/1", false) {
                    p.normal = p.normal.normalize_or_zero();
                }
            }
            "Remap Noise to Scalar" => {
                let f = op.i("output field", field::RADIUS);
                let omin = op.f("output minimum", 0.0);
                let omax = op.f("output maximum", 1.0);
                let tscale = op.f("time noise coordinate scale", 0.1);
                let sscale = op.f("spatial noise coordinate scale", 0.001);
                let soff = v3(op.v3("spatial coordinate offset", [0.0; 3]));
                let toff = op.f("time coordinate offset", 0.0);
                let real_ms = self.time * 1000.0;
                let q = (p.pos + soff) * sscale + Vec3::splat(real_ms * tscale + p.created + toff);
                let n = noise3(q);
                let mut t = if op.b("absolute value", false) { n.abs() } else { n * 0.5 + 0.5 };
                if op.b("invert absolute value", false) {
                    t = 1.0 - t;
                }
                Self::set_initial(p, f, lerp(omin, omax, t.clamp(0.0, 1.0)), false);
            }
            "Remap Particle Count to Scalar" => {
                let f = op.i("output field", field::RADIUS);
                let imin = op.f("input minimum", 0.0);
                let imax = op.f("input maximum", 10.0);
                let omin = op.f("output minimum", 0.0);
                let omax = op.f("output maximum", 1.0);
                let only = op.b("only active within specified input range", false);
                let c = id as f32;
                if only && (c < imin || c > imax) {
                    return;
                }
                let v = remap_clamped(c, imin, imax, omin, omax);
                Self::set_initial(p, f, v, op.b("output is scalar of initial random range", false));
            }
            "Remap Control Point to Vector" => {
                let cp = self.cp(op.i("input control point number", 0));
                let f = op.i("output field", field::XYZ);
                let imin = v3(op.v3("input minimum", [0.0; 3]));
                let imax = v3(op.v3("input maximum", [0.0; 3]));
                let omin = v3(op.v3("output minimum", [0.0; 3]));
                let omax = v3(op.v3("output maximum", [0.0; 3]));
                let v = Vec3::new(
                    remap_clamped(cp.pos.x, imin.x, imax.x, omin.x, omax.x),
                    remap_clamped(cp.pos.y, imin.y, imax.y, omin.y, omax.y),
                    remap_clamped(cp.pos.z, imin.z, imax.z, omin.z, omax.z),
                );
                if Particle::is_vector_field(f) {
                    p.set_vector(f, v);
                } else {
                    p.set_scalar(f, v.x);
                }
            }
            "Remap Initial Distance to Control Point to Scalar" => {
                let cp = self.cp(op.i("control point", 0));
                let f = op.i("output field", field::RADIUS);
                let dmin = op.f("distance minimum", 0.0);
                let dmax = op.f("distance maximum", 128.0);
                let omin = op.f("output minimum", 0.0);
                let omax = op.f("output maximum", 1.0);
                let d = p.pos.distance(cp.pos);
                if op.b("only active within specified distance", false) && (d < dmin || d > dmax) {
                    return;
                }
                let v = remap_clamped(d, dmin, dmax, omin, omax);
                Self::set_initial(p, f, v, op.b("output is scalar of initial random range", false));
            }
            "remap initial scalar" => {
                let fin = op.i("input field", field::CREATION_TIME);
                let f = op.i("output field", field::RADIUS);
                let v = p.scalar(fin);
                let imin = op.f("input minimum", 0.0);
                let imax = op.f("input maximum", 1.0);
                if op.b("only active within specified input range", false) && (v < imin || v > imax) {
                    return;
                }
                let out = remap_clamped(v, imin, imax, op.f("output minimum", 0.0), op.f("output maximum", 1.0));
                Self::set_initial(p, f, out, op.b("output is scalar of initial random range", false));
            }
            _ => {}
        }
    }

    fn run_operators(&mut self, def: &SystemDef, dt: f32, now: f32) {
        let cps = self.cps;
        let prev_cps = self.prev_cps;
        let seed = self.seed;
        let prev_dt = self.last_dt.max(1e-4);
        for (oi, op) in def.operators.iter().enumerate() {
            if !self.op_active(op) {
                continue;
            }
            let strength = Self::op_strength(op, now);
            if strength <= 0.0 {
                continue;
            }
            let salt = oi as u32 * 7919 + 101;
            let rand = |id: u32, k: u32| rnd(seed, id, salt, k);
            let cpn = |k: &str, d: i32| cps[op.i(k, d).clamp(0, 63) as usize];
            match op.function.as_str() {
                "Movement Basic" => {
                    let g = v3(op.v3("gravity", [0.0; 3]));
                    let drag = op.f("drag", 0.0);
                    let mut decay = if drag > 0.0 { (1.0 - drag).clamp(0.0, 1.0).powf(dt * DRAG_REF_HZ) } else { 1.0 };
                    decay *= dt / prev_dt;
                    for p in &mut self.particles {
                        let vel = p.pos - p.prev;
                        let new = p.pos + vel * decay + g * (dt * dt) * strength;
                        p.prev = p.pos;
                        p.pos = new;
                    }
                }
                "Movement Lock to Control Point" => {
                    let i = op.i("control_point_number", 0).clamp(0, 63) as usize;
                    let (a, b) = (prev_cps[i], cps[i]);
                    let lock_rot = op.b("lock rotation", false);
                    let (start_min, start_max) = (op.f("start_fadeout_min", 1.0), op.f("start_fadeout_max", 1.0));
                    let (end_min, end_max) = (op.f("end_fadeout_min", 1.0), op.f("end_fadeout_max", 1.0));
                    if a == b {
                        continue;
                    }
                    for p in &mut self.particles {
                        let lf = p.life_fraction(now);
                        let s = lerp(start_min, start_max, rand(p.id, 1));
                        let e = lerp(end_min, end_max, rand(p.id, 2));
                        let w = 1.0 - remap_clamped(lf, s, e, 0.0, 1.0);
                        if w <= 0.0 {
                            continue;
                        }
                        let moved = |x: Vec3| -> Vec3 {
                            if lock_rot {
                                b.to_world(a.to_local(x))
                            } else {
                                x + (b.pos - a.pos)
                            }
                        };
                        p.pos = p.pos.lerp(moved(p.pos), w);
                        p.prev = p.prev.lerp(moved(p.prev), w);
                        if lock_rot {
                            let rot_delta = b.basis * a.basis.transpose();
                            p.normal = rot_delta.mul_vec3(p.normal).normalize_or_zero();
                        }
                    }
                }
                "Movement Dampen Relative to Control Point" => {
                    let cp = cpn("control_point_number", 0);
                    let range = op.f("falloff range", 100.0).max(1e-3);
                    let scale = op.f("dampen scale", 1.0);
                    for p in &mut self.particles {
                        let d = p.pos.distance(cp.pos);
                        let k = (d / range).clamp(0.0, 1.0).powf(scale.max(1e-3));
                        let vel = (p.pos - p.prev) * lerp(1.0, k, strength);
                        p.prev = p.pos - vel;
                    }
                }
                "Movement Max Velocity" => {
                    let maxv = op.f("Maximum Velocity", 0.0) * dt;
                    for p in &mut self.particles {
                        let v = p.pos - p.prev;
                        if v.length() > maxv {
                            p.prev = p.pos - v.normalize_or_zero() * maxv;
                        }
                    }
                }
                "Lifespan Decay" => {
                    for p in &mut self.particles {
                        if p.lifetime <= 0.0 || p.age(now) >= p.lifetime {
                            p.dead = true;
                        }
                    }
                }
                "Lifespan Minimum Radius Decay" => {
                    let minr = op.f("minimum radius", 1.0);
                    for p in &mut self.particles {
                        if p.radius < minr {
                            p.dead = true;
                        }
                    }
                }
                "Lifespan Minimum Alpha Decay" => {
                    let mina = op.f("minimum alpha", 0.0);
                    for p in &mut self.particles {
                        if p.alpha * p.alpha2 <= mina {
                            p.dead = true;
                        }
                    }
                }
                "Alpha Fade In Simple" => {
                    let t = op.f("proportional fade in time", 0.25).max(1e-5);
                    for p in &mut self.particles {
                        let lf = p.life_fraction(now);
                        if lf < t {
                            p.alpha = p.init_alpha * smoothstep(lf / t);
                        }
                    }
                }
                "Alpha Fade Out Simple" => {
                    let t = op.f("proportional fade out time", 0.25).max(1e-5);
                    for p in &mut self.particles {
                        let lf = p.life_fraction(now);
                        if lf > 1.0 - t {
                            p.alpha = p.init_alpha * (1.0 - smoothstep((lf - (1.0 - t)) / t));
                        }
                    }
                }
                "Alpha Fade In Random" => {
                    let proportional = op.b("proportional 0/1", true);
                    let (tmin, tmax) = (op.f("fade in time min", 0.25), op.f("fade in time max", 0.25));
                    for p in &mut self.particles {
                        let t = lerp(tmin, tmax, rand(p.id, 1)).max(1e-5);
                        let a = if proportional { p.life_fraction(now) } else { p.age(now) };
                        if a < t {
                            p.alpha = p.init_alpha * smoothstep(a / t);
                        }
                    }
                }
                "Alpha Fade Out Random" => {
                    let proportional = op.b("proportional 0/1", true);
                    let ease = op.b("ease in and out", true);
                    let (tmin, tmax) = (op.f("fade out time min", 0.25), op.f("fade out time max", 0.25));
                    for p in &mut self.particles {
                        let t = lerp(tmin, tmax, rand(p.id, 1)).max(1e-5);
                        let (left, span) = if proportional {
                            (1.0 - p.life_fraction(now), 1.0)
                        } else {
                            (p.lifetime - p.age(now), 1.0)
                        };
                        if left < t {
                            let f = (left / t).clamp(0.0, 1.0);
                            let s = if ease { smoothstep(f) } else { f };
                            p.alpha = p.init_alpha * s;
                        }
                        let _ = span;
                    }
                }
                "Alpha Fade and Decay" => {
                    let sa = op.f("start_alpha", 1.0);
                    let ea = op.f("end_alpha", 0.0);
                    let sfi = op.f("start_fade_in_time", 0.0);
                    let efi = op.f("end_fade_in_time", 0.5);
                    let sfo = op.f("start_fade_out_time", 0.5);
                    let efo = op.f("end_fade_out_time", 1.0);
                    for p in &mut self.particles {
                        let a = p.life_fraction(now);
                        let s = if a < efi {
                            lerp(sa, 1.0, smoothstep(remap_clamped(a, sfi, efi, 0.0, 1.0)))
                        } else if a > sfo {
                            lerp(1.0, ea, smoothstep(remap_clamped(a, sfo, efo, 0.0, 1.0)))
                        } else {
                            1.0
                        };
                        p.alpha = p.init_alpha * s;
                        if a >= 1.0 {
                            p.dead = true;
                        }
                    }
                }
                "Color Fade" => {
                    let c = op.color("color_fade", [255; 4]);
                    let target = Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) / 255.0;
                    let st = op.f("fade_start_time", 0.0);
                    let et = op.f("fade_end_time", 1.0);
                    let ease = op.b("ease_in_and_out", true);
                    for p in &mut self.particles {
                        let mut t = remap_clamped(p.life_fraction(now), st, et, 0.0, 1.0);
                        if ease {
                            t = smoothstep(t);
                        }
                        p.tint = p.init_tint.lerp(target, t * strength);
                    }
                }
                "Radius Scale" => {
                    let st = op.f("start_time", 0.0);
                    let et = op.f("end_time", 1.0);
                    let rs = op.f("radius_start_scale", 1.0);
                    let re = op.f("radius_end_scale", 1.0);
                    let ease = op.b("ease_in_and_out", false);
                    let b = op.f("scale_bias", 0.5);
                    for p in &mut self.particles {
                        let t = remap_clamped(p.life_fraction(now), st, et, 0.0, 1.0);
                        let s = if ease { smoothstep(t) } else { bias(t, b) };
                        let target = p.init_radius * lerp(rs, re, s);
                        p.radius = lerp(p.radius, target, strength);
                    }
                }
                "Clamp Scalar" => {
                    let f = op.i("output field", field::RADIUS);
                    let lo = op.f("output minimum", 0.0);
                    let hi = op.f("output maximum", 1.0);
                    for p in &mut self.particles {
                        let v = p.scalar(f);
                        p.set_scalar(f, lerp(v, v.clamp(lo.min(hi), hi.max(lo)), strength));
                    }
                }
                "Ramp Scalar Linear Simple" => {
                    let f = op.i("ramp field", field::RADIUS);
                    let rate = op.f("ramp rate", 0.0);
                    let st = op.f("start time", 0.0);
                    let et = op.f("end time", 1.0);
                    for p in &mut self.particles {
                        let lf = p.life_fraction(now);
                        if lf >= st && lf < et && p.lifetime > 0.0 {
                            let v = p.scalar(f) + rate * dt * strength;
                            p.set_scalar(f, v);
                        }
                    }
                }
                "Oscillate Scalar" => {
                    let f = op.i("oscillation field", field::ALPHA);
                    let (rmin, rmax) = (op.f("oscillation rate min", 0.0), op.f("oscillation rate max", 0.0));
                    let (fmin, fmax) = (op.f("oscillation frequency min", 1.0), op.f("oscillation frequency max", 1.0));
                    let proportional = op.b("proportional 0/1", true);
                    let (stmin, stmax) = (op.f("start time min", 0.0), op.f("start time max", 0.0));
                    let (etmin, etmax) = (op.f("end time min", 1.0), op.f("end time max", 1.0));
                    let mult = op.f("oscillation multiplier", 2.0);
                    let phase = op.f("oscillation start phase", 0.5);
                    for p in &mut self.particles {
                        let t = if proportional { p.life_fraction(now) } else { p.age(now) };
                        let st = lerp(stmin, stmax, rand(p.id, 3));
                        let et = lerp(etmin, etmax, rand(p.id, 4));
                        if t < st || t > et {
                            continue;
                        }
                        let rate = lerp(rmin, rmax, rand(p.id, 1));
                        let freq = lerp(fmin, fmax, rand(p.id, 2));
                        let s = ((t * freq * mult + phase) * PI).sin();
                        let v = p.scalar(f) + rate * s * dt * strength;
                        p.set_scalar(f, v);
                    }
                }
                "Oscillate Vector" => {
                    let f = op.i("oscillation field", field::XYZ);
                    let rmin = v3(op.v3("oscillation rate min", [0.0; 3]));
                    let rmax = v3(op.v3("oscillation rate max", [0.0; 3]));
                    let fmin = v3(op.v3("oscillation frequency min", [1.0; 3]));
                    let fmax = v3(op.v3("oscillation frequency max", [1.0; 3]));
                    let proportional = op.b("proportional 0/1", true);
                    let (stmin, stmax) = (op.f("start time min", 0.0), op.f("start time max", 0.0));
                    let (etmin, etmax) = (op.f("end time min", 1.0), op.f("end time max", 1.0));
                    let mult = op.f("oscillation multiplier", 2.0);
                    let phase = op.f("oscillation start phase", 0.5);
                    for p in &mut self.particles {
                        let t = if proportional { p.life_fraction(now) } else { p.age(now) };
                        let st = lerp(stmin, stmax, rand(p.id, 7));
                        let et = lerp(etmin, etmax, rand(p.id, 8));
                        if t < st || t > et {
                            continue;
                        }
                        let rate = Vec3::new(
                            lerp(rmin.x, rmax.x, rand(p.id, 1)),
                            lerp(rmin.y, rmax.y, rand(p.id, 2)),
                            lerp(rmin.z, rmax.z, rand(p.id, 3)),
                        );
                        let freq = Vec3::new(
                            lerp(fmin.x, fmax.x, rand(p.id, 4)),
                            lerp(fmin.y, fmax.y, rand(p.id, 5)),
                            lerp(fmin.z, fmax.z, rand(p.id, 6)),
                        );
                        let s = Vec3::new(
                            ((t * freq.x * mult + phase) * PI).sin(),
                            ((t * freq.y * mult + phase) * PI).sin(),
                            ((t * freq.z * mult + phase) * PI).sin(),
                        );
                        let d = rate * s * dt * strength;
                        let v = p.vector(f) + d;
                        p.set_vector(f, v);
                        if f == field::XYZ {
                            p.prev += d;
                        }
                    }
                }
                "Remap Direction to CP to Vector" => {
                    let cp = cpn("control point", 0);
                    let f = op.i("output field", field::NORMAL);
                    let scale = op.f("scale factor", 1.0);
                    let rot = op.f("offset rotation", 0.0).to_radians();
                    let axis = v3(op.v3("offset axis", [0.0; 3])).normalize_or_zero();
                    let norm = op.b("normalize", false);
                    for p in &mut self.particles {
                        let mut d = p.pos - cp.pos;
                        if axis != Vec3::ZERO && rot != 0.0 {
                            d = Quat::from_axis_angle(axis, rot) * d;
                        }
                        let out = if norm { d.normalize_or_zero() * scale } else { d * scale / dt };
                        p.set_vector(f, out);
                    }
                }
                "Remap Control Point to Vector" => {
                    let cp = cpn("input control point number", 0);
                    let f = op.i("output field", field::XYZ);
                    let imin = v3(op.v3("input minimum", [0.0; 3]));
                    let imax = v3(op.v3("input maximum", [0.0; 3]));
                    let omin = v3(op.v3("output minimum", [0.0; 3]));
                    let omax = v3(op.v3("output maximum", [0.0; 3]));
                    let v = Vec3::new(
                        remap_clamped(cp.pos.x, imin.x, imax.x, omin.x, omax.x),
                        remap_clamped(cp.pos.y, imin.y, imax.y, omin.y, omax.y),
                        remap_clamped(cp.pos.z, imin.z, imax.z, omin.z, omax.z),
                    );
                    for p in &mut self.particles {
                        p.set_vector(f, v);
                    }
                }
                "Rotation Basic" => {
                    for p in &mut self.particles {
                        p.rotation += p.rotation_speed * dt * strength;
                    }
                }
                "Cull when crossing sphere" => {
                    let cp = cpn("Control Point", 0);
                    let dist = op.f("Cull Distance", 0.0);
                    let inside = op.b("Cull inside instead of outside", false);
                    for p in &mut self.particles {
                        let d = p.pos.distance(cp.pos);
                        if (inside && d < dist) || (!inside && d > dist) {
                            p.dead = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn run_forces(&mut self, def: &SystemDef, dt: f32) {
        let cps = self.cps;
        let seed = self.seed;
        for (fi, op) in def.forces.iter().enumerate() {
            if !self.op_active(op) {
                continue;
            }
            let strength = Self::op_strength(op, self.time);
            match op.function.as_str() {
                "Pull towards control point" => {
                    let cp = cps[op.i("control point number", 0).clamp(0, 63) as usize];
                    let amount = op.f("amount of force", 0.0);
                    let falloff = op.f("falloff power", 2.0);
                    for p in &mut self.particles {
                        let d = cp.pos - p.pos;
                        let len = d.length().max(1.0);
                        let force = d / len * amount / len.powf(falloff).max(1e-3);
                        p.prev -= force * (dt * dt) * strength;
                    }
                }
                "random force" => {
                    let lo = v3(op.v3("min force", [0.0; 3]));
                    let hi = v3(op.v3("max force", [0.0; 3]));
                    let k = (self.time * 30.0) as u32;
                    for p in &mut self.particles {
                        let f = Vec3::new(
                            lerp(lo.x, hi.x, hash(p.id ^ seed, fi as u32, k * 3)),
                            lerp(lo.y, hi.y, hash(p.id ^ seed, fi as u32, k * 3 + 1)),
                            lerp(lo.z, hi.z, hash(p.id ^ seed, fi as u32, k * 3 + 2)),
                        );
                        p.prev -= f * (dt * dt) * strength;
                    }
                }
                "twist around axis" => {
                    let amount = op.f("amount of force", 0.0);
                    let mut axis = v3(op.v3("twist axis", [0.0, 0.0, 1.0])).normalize_or(Vec3::Z);
                    let cp = cps[op.i("control point number", 0).clamp(0, 63) as usize];
                    if op.b("object local space axis 0/1", false) {
                        axis = cp.basis * axis;
                    }
                    for p in &mut self.particles {
                        let r = p.pos - cp.pos;
                        let tangent = axis.cross(r).normalize_or_zero();
                        p.prev -= tangent * amount * (dt * dt) * strength;
                    }
                }
                _ => {}
            }
        }
    }

    fn run_constraints(&mut self, def: &SystemDef) {
        let cps = self.cps;
        for op in &def.constraints {
            if op.function == "Constrain distance to control point" {
                let cp = cps[op.i("control point number", 0).clamp(0, 63) as usize];
                let mind = op.f("minimum distance", 0.0);
                let maxd = op.f("maximum distance", 100.0);
                let off = v3(op.v3("offset of center", [0.0; 3]));
                let center = cp.pos + off;
                for p in &mut self.particles {
                    let d = p.pos - center;
                    let len = d.length();
                    if len > maxd && len > 0.0 {
                        p.pos = center + d / len * maxd;
                    } else if len < mind && len > 0.0 {
                        p.pos = center + d / len * mind;
                    }
                }
            }
        }
    }

    pub fn for_each<'a>(&'a self, f: &mut dyn FnMut(&'a SystemInstance)) {
        f(self);
        for c in &self.children {
            if let Some(s) = &c.system {
                s.for_each(f);
            }
        }
    }
}

