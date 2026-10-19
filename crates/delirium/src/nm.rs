use crate::prelude::*;
use delirium_data::taskparams::TaskParams;
#[derive(Clone, Debug)]
pub struct Random(u64);

impl Random {
    pub fn new(seed: u32) -> Self {
        Random(seed as u64 | 1 << 32)
    }

    pub fn next(&mut self) -> f32 {
        (self.bits() & 0x7F_FFFF) as f32 / (1 << 23) as f32
    }

    pub fn bits(&mut self) -> u32 {
        self.0 = (self.0 & 0xFFFF_FFFF) * 0x5CDC_FAA7 + (self.0 >> 32);
        self.0 as u32
    }
}

use crate::collision::Collision;
use crate::ragdoll::{Build, Capsule, Drive, Limit, Muscle, Obstacle, Ragdoll};

const G: f32 = 9.8;
const FOOT: f32 = 0.09;
const ANKLE_STIFFNESS: f32 = 40.0;
const ANKLE_DAMPING: f32 = 10.0;
const STEP_TIME: f32 = 0.28;
const STEP_REACH: f32 = 0.7;
const STEP_SLACK: f32 = 0.06;
const ABORT_TILT: f32 = 0.6;
const ABORT_SINK: f32 = 0.6;

#[derive(Clone, Default)]
struct Roles {
    pelvis: usize,
    spine: Vec<usize>,
    neck: Option<usize>,
    head: Option<usize>,
    upper_arm: [Option<usize>; 2],
    forearm: [Option<usize>; 2],
    thigh: [Option<usize>; 2],
    calf: [Option<usize>; 2],
    foot: [Option<usize>; 2],
}

impl Roles {
    fn new(build: &Build) -> Roles {
        let find = |s: &str| build.parts.iter().position(|p| p.name.ends_with(s));
        let side = |s: &str| [find(&format!("L_{s}")), find(&format!("R_{s}"))];
        Roles {
            pelvis: build.parts.iter().position(|p| p.parent.is_none()).unwrap_or(0),
            spine: build.parts.iter().enumerate().filter(|(_, p)| p.name.contains("Spine")).map(|(k, _)| k).collect(),
            neck: find("Neck"),
            head: find("Head"),
            upper_arm: side("UpperArm"),
            forearm: side("Forearm"),
            thigh: side("Thigh"),
            calf: side("Calf"),
            foot: side("Foot"),
        }
    }
    fn arms(&self) -> impl Iterator<Item = usize> + '_ {
        self.upper_arm.iter().chain(&self.forearm).flatten().copied()
    }
    fn legs(&self) -> impl Iterator<Item = usize> + '_ {
        self.thigh.iter().chain(&self.calf).chain(&self.foot).flatten().copied()
    }
}

#[derive(Clone, Copy)]
pub struct ConfigureBalance {
    pub step_height: f32,
    pub leg_stiffness: f32,
    pub balance_abort_threshold: f32,
    pub step_clamp_scale: f32,
    pub taper_knee_strength: bool,
}

impl Default for ConfigureBalance {
    fn default() -> Self {
        ConfigureBalance { step_height: 0.1, leg_stiffness: 12.0, balance_abort_threshold: ABORT_TILT, step_clamp_scale: 1.0, taper_knee_strength: true }
    }
}

#[derive(Clone, Copy)]
pub struct BodyBalance {
    pub body_stiffness: f32,
    pub damping: f32,
    pub elbow: f32,
    pub shoulder: f32,
    pub max_steps: u32,
    pub spine_stiffness: f32,
    pub spine_damping: f32,
}

pub type Shot = crate::nmshot::Params;

#[derive(Clone, Copy)]
pub struct CatchFall {
    pub torso_stiffness: f32,
    pub legs_stiffness: f32,
    pub arms_stiffness: f32,
    pub z_axis_spin_reduction: f32,
}

#[derive(Clone, Copy)]
pub struct UpperBodyFlinch {
    pub pos: Vec3,
    pub body_stiffness: f32,
    pub body_damping: f32,
    pub back_bend_amount: f32,
    pub left_handed: bool,
    pub right_handed: bool,
    pub use_left_arm: bool,
    pub use_right_arm: bool,
    pub new_hit: Option<bool>,
    pub hand_distance_vertical: f32,
    pub hand_distance_left_right: f32,
    pub hand_distance_front_back: f32,
    pub noise_scale: f32,
    pub turn_towards: i32,
}

#[derive(Clone, Copy)]
pub struct BodyWrithe {
    pub arm_stiffness: f32,
    pub back_stiffness: f32,
    pub leg_stiffness: f32,
    pub arm_period: f32,
    pub back_period: f32,
    pub leg_period: f32,
    pub arm_amplitude: f32,
    pub back_amplitude: f32,
    pub leg_amplitude: f32,
    pub knee_amplitude: f32,
    pub elbow_amplitude: f32,
    pub arm_damping: f32,
    pub back_damping: f32,
    pub leg_damping: f32,
    pub roll_over: bool,
    pub mask: &'static str,
}

impl Default for BodyWrithe {
    fn default() -> BodyWrithe {
        BodyWrithe { arm_stiffness: 13.0, back_stiffness: 13.0, leg_stiffness: 13.0, arm_damping: 0.5, back_damping: 0.5, leg_damping: 0.5,
                     arm_period: 1.0, back_period: 1.0, leg_period: 1.0, arm_amplitude: 1.0, back_amplitude: 1.0, leg_amplitude: 1.0,
                     elbow_amplitude: 1.0, knee_amplitude: 1.0, roll_over: false, mask: "fb" }
    }
}

#[derive(Clone, Copy)]
pub struct ArmsWindmill {
    pub arm_stiffness: f32,
    pub body_stiffness: f32,
    pub period: f32,
    pub amplitude: f32,
    pub phase: f32,
    pub disable_on_impact: bool,
}

#[derive(Clone, Copy)]
pub struct PedalLegs {
    pub leg_stiffness: f32,
    pub radius: f32,
    pub angular_speed: f32,
    pub pedal_offset: f32,
    pub speed_asymmetry: f32,
    pub back_pedal: bool,
    pub adaptive: bool,
    pub drag_multiplier: f32,
    pub left: bool,
    pub right: bool,
    pub radius_variance: f32,
    pub random_seed: u32,
    pub leg_angle_variance: f32,
    pub centre_forwards: f32,
    pub centre_up: f32,
}

#[derive(Clone, Copy)]
pub struct BodyRollUp {
    pub stiffness: f32,
    pub leg_push: f32,
    pub arm_reach_amount: f32,
    pub use_arm_to_slow_down: f32,
    pub asymmetrical_legs: f32,
    pub mask: Option<&'static str>,
    pub hips_from_vel: Option<bool>,
}

impl Default for UpperBodyFlinch {
    fn default() -> UpperBodyFlinch {
        UpperBodyFlinch { pos: Vec3::ZERO, body_stiffness: 11.0, body_damping: 1.0, back_bend_amount: -0.55, left_handed: false, right_handed: false,
                          use_left_arm: true, use_right_arm: true, new_hit: None,
                          hand_distance_vertical: 0.1, hand_distance_left_right: 0.1, hand_distance_front_back: 0.06, noise_scale: 0.1, turn_towards: 1 }
    }
}

impl Default for ArmsWindmill {
    fn default() -> ArmsWindmill {
        ArmsWindmill { arm_stiffness: 14.142, body_stiffness: 11.0, period: 1.0, amplitude: 0.6, phase: 0.0, disable_on_impact: true }
    }
}

impl Default for PedalLegs {
    fn default() -> PedalLegs {
        PedalLegs { leg_stiffness: 10.0, radius: 0.25, angular_speed: 10.0, pedal_offset: 0.15, speed_asymmetry: 8.0, back_pedal: false,
                    adaptive: false, drag_multiplier: 0.3, left: true, right: true, radius_variance: 0.4, random_seed: 100,
                    leg_angle_variance: 0.5, centre_forwards: 0.0, centre_up: 0.0 }
    }
}

impl Default for BodyRollUp {
    fn default() -> BodyRollUp {
        BodyRollUp { stiffness: 10.0, leg_push: 0.0, arm_reach_amount: 1.4, use_arm_to_slow_down: 1.3, asymmetrical_legs: 0.0, mask: None,
                     hips_from_vel: None }
    }
}

pub type BraceForImpact = crate::nmbr::BraceParams;
pub type HighFall = crate::nmhf::HighFallParams;
pub type FallOverWall = crate::nmfow::FallOverWallParams;

fn dbm_config(c: ConfigureBalance, params: &BodyBalance, lean_bias: f32, hip_yaw: (f32, f32), legs: (f32, f32)) -> crate::nmdbm::Config {
    crate::nmdbm::Config { step_height: c.step_height, leg_stiffness: c.leg_stiffness, abort: c.balance_abort_threshold,
                           step_clamp: c.step_clamp_scale, max_steps: params.max_steps as i32, taper: c.taper_knee_strength,
                           lean_bias, hip_yaw, legs }
}

impl BodyBalance {
    fn from(p: crate::nmbb::Params) -> BodyBalance {
        BodyBalance { body_stiffness: p.body_stiffness, damping: p.damping, elbow: p.elbow, shoulder: p.shoulder, max_steps: p.max_steps,
                      spine_stiffness: p.head_look_always_timeout, spine_damping: p.body_turn_timeout }
    }

    fn onto(self, p: &mut crate::nmbb::Params) {
        (p.body_stiffness, p.damping, p.elbow, p.shoulder, p.max_steps) = (self.body_stiffness, self.damping, self.elbow, self.shoulder, self.max_steps);
        (p.head_look_always_timeout, p.body_turn_timeout) = (self.spine_stiffness, self.spine_damping);
    }
}

#[derive(Clone, Copy, Debug)]
enum Foot {
    Planted(Transform),
    Stepping(Transform, Transform, f32),
}

pub(crate) struct Balance {
    params: BodyBalance,
    pub(crate) dbm: Option<crate::nmdbm::Dbm>,
    bb: Option<crate::nmbb::BodyBalance>,
    stepping: u32,
    feet: [Foot; 2],
    steps: u32,
    stand_height: f32,
    still: f32,
}

impl Balance {
    fn on(&self) -> bool {
        self.dbm.is_some() || self.bb.is_none()
    }
}

pub struct Agent {
    pub doll: Ragdoll,
    pub push: Vec<(usize, Vec3, Vec3)>,
    pub contact_wake: bool,
    roles: Roles,
    zero: Vec<Quat>,
    pelvis_up: Quat,
    pub facing: Quat,
    random: Random,
    time: f32,
    stiffness: (f32, f32),
    relax: Option<(f32, f32)>,
    config: ConfigureBalance,
    pub feedback: Vec<crate::nmshot::Feedback>,
    pub(crate) balance: Option<Balance>,
    lean: Option<(Vec3, f32, f32, bool)>,
    pub task_look: Option<bool>,
    shot: Option<(Shot, f32)>,
    nm_shot: Option<crate::nmshot::Shot>,
    nm_hit: Option<Shot>,
    nm_hit_next: Vec<(Shot, bool)>,
    nm_relax: Option<f32>,
    nm_head: crate::nmhl::HeadLook,
    nm_cf: Option<crate::nmcf::CatchFall>,
    nm_cf_msg: Option<crate::nmcf::Params>,
    nm_twist: crate::nmst::SpineTwist,
    nm_fl: crate::nmfl::Flinch,
    hands_back: bool,
    nm_fl_msg: Option<UpperBodyFlinch>,
    pub(crate) nm_bw: Option<crate::nmbw::Writhe>,
    nm_bw_msg: Option<BodyWrithe>,
    pub extras: crate::extras::Extras,
    nm_aw: crate::nmaw::Windmill,
    nm_aw_msg: Option<ArmsWindmill>,
    nm_pd: crate::nmpd::Pedal,
    nm_pd_msg: Option<PedalLegs>,
    nm_bf: crate::nmbf::Foetal,
    nm_bf_msg: Option<Option<crate::nmbf::Foetal>>,
    pub incoming: Vec<Transform>,
    pub(crate) incoming_prev: Vec<Transform>,
    pub incoming_v_ref: Vec3,
    pub(crate) incoming_state: u8,
    nm_ru: crate::nmru::RollUp,
    nm_ru_msg: Option<BodyRollUp>,
    nm_rds: crate::nmrds::RollDownStairs,
    nm_rds_msg: Option<crate::nmrds::RollDownStairs>,
    rds_on: bool,
    bb_on: bool,
    nm_pa: crate::nmpa::PointArm,
    nm_gr: crate::nmgr::Grab,
    obstacles: Vec<Obstacle>,
    nm_br: crate::nmbr::Brace,
    nm_br_msg: Option<Option<BraceForImpact>>,
    nm_br_started: Option<crate::nmbr::BraceOut>,
    nm_hf: crate::nmhf::HighFall,
    nm_fow: crate::nmfow::FallOverWall,
    nm_fow_msg: Option<Option<FallOverWall>>,
    nm_hf_msg: Option<Option<HighFall>>,
    nm_gr_msg: Option<Option<crate::nmgr::GrabParams>>,
    nm_pa_msg: Option<Option<crate::nmpa::PointArmParams>>,
    catch_fall: Option<CatchFall>,
    flinch: Option<UpperBodyFlinch>,
    writhe: Option<BodyWrithe>,
    windmill: Option<ArmsWindmill>,
    pedal: Option<PedalLegs>,
    roll_up: Option<BodyRollUp>,
    brace: Option<BraceForImpact>,
    head_look: Vec<(Vec3, f32)>,
    shot_look: bool,
    pub(crate) eff_override: Vec<(usize, [f32; 3], f32, f32, f32, Option<f32>)>,
    nm_dt: Option<f32>,
    held: Option<crate::ragdoll::Kept>,
    held_eff: Vec<[f32; 6]>,
    nm_shot_started: Option<crate::nmshot::Out>,
    pub fallen: bool,
    pub msgs: crate::msg::State,
    pub(crate) ch: Option<crate::nmrs::Character>,
    pub lean_bias: f32,
    pub hip_yaw: (f32, f32),
    pub legs: (f32, f32),
    pub strength: f32,
    pub most: f32,
}

pub(crate) const LEG_PAIRS: [(usize, usize); 7] = [(1, 4), (1, 5), (2, 4), (2, 5), (2, 6), (3, 5), (3, 6)];
pub(crate) const HAND_PAIRS: [(usize, usize); 3] = [(20, 16), (20, 15), (19, 16)];
pub(crate) fn pair_on((a, b): (usize, usize), legs: bool, hands: bool) -> bool {
    !(legs && LEG_PAIRS.contains(&(a.min(b), a.max(b))) || hands && HAND_PAIRS.contains(&(a.max(b), a.min(b))))
}

pub fn empty_pair_table(n: usize) -> Vec<i32> {
    vec![-1; n]
}

pub(crate) fn game_pairs(pairs: impl Iterator<Item = (usize, usize)>, hands_back: bool) -> Vec<(usize, usize)> {
    let mut v: Vec<_> = pairs.map(|(a, b)| if LEG_PAIRS.contains(&(b, a)) || hands_back && HAND_PAIRS.contains(&(a, b)) { (b, a) } else { (a, b) }).collect();
    v.sort();
    v
}

impl Agent {
    pub fn limp_rage(&self) -> Vec<crate::ragdoll::Rigid> {
        let mut rigid = self.ch.as_ref().map_or(vec![], |ch| ch.rigid(1.0 / 60.0, self.strength, self.most));
        rigid.iter_mut().for_each(|r| (r.max, r.gain) = ([0.0; 2], [0.0; 2]));
        rigid
    }

    pub fn set_rng(&mut self, rng: (u32, u32)) {
        if let Some(ch) = self.ch.as_mut() {
            ch.rng = rng;
        }
    }

    pub fn new(build: &Build, mut doll: Ragdoll, facing: Quat, seed: f32) -> Agent {
        doll.fresh = true;
        doll.sleep_tol = [0.0; 3];
        if let Some(pairs) = crate::nmrs::SELF[build.nm].get().filter(|_| build.parts.len() == delirium_data::nm_asset::PARTS.len()) {
            doll.pairs = game_pairs(pairs.iter().copied().filter(|&(a, b)| a.max(b) < build.parts.len()), false);
        }
        let frames: Vec<Transform> = doll.frames().collect();
        let zero = build.parts.iter().enumerate()
            .map(|(i, p)| p.parent.map_or(frames[i].rotation, |j| frames[j].rotation.inverse() * frames[i].rotation)).collect();
        let roles = Roles::new(build);
        let pelvis_up = facing.inverse() * frames[roles.pelvis].rotation;
        let mut a = Agent { doll, msgs: Default::default(), push: vec![], contact_wake: false, roles, zero, pelvis_up, facing, random: Random::new((seed * 1000.0) as u32 + 1), time: 0.0,
                stiffness: (10.0, 1.0), relax: None, config: ConfigureBalance::default(), feedback: vec![], balance: None, lean: None,
                shot: None, nm_shot: None, nm_hit: None, nm_hit_next: vec![], nm_relax: None, nm_head: Default::default(), nm_cf: None, nm_cf_msg: None, nm_twist: Default::default(), hands_back: false, nm_fl: Default::default(), nm_fl_msg: None, nm_bw: None, nm_bw_msg: None, extras: Default::default(), nm_aw: Default::default(), nm_aw_msg: None, nm_pd: Default::default(), nm_pd_msg: None, nm_bf: Default::default(), nm_bf_msg: None, incoming: vec![], incoming_prev: vec![], incoming_state: 0, incoming_v_ref: Vec3::ZERO, nm_ru: Default::default(), nm_ru_msg: None, nm_rds: Default::default(), nm_rds_msg: None, rds_on: false, bb_on: false, nm_pa: Default::default(), nm_pa_msg: None, nm_gr: Default::default(), obstacles: vec![], nm_gr_msg: None, nm_br: Default::default(), nm_br_msg: None, nm_br_started: None, nm_hf: Default::default(), nm_hf_msg: None, nm_fow: Default::default(), nm_fow_msg: None, catch_fall: None, flinch: None, writhe: None, windmill: None, pedal: None, roll_up: None,
                brace: None, head_look: vec![], shot_look: false, eff_override: vec![], nm_dt: None, held: None, held_eff: vec![], nm_shot_started: None, fallen: false, task_look: None, lean_bias: -0.22, hip_yaw: crate::nmdbm::CHAR_YAW, legs: (crate::nmdbm::CHAR_LEGS.0 * build.parts[0].grown, crate::nmdbm::CHAR_LEGS.1 * build.parts[0].grown), strength: 1.0, most: build.parts.first().filter(|p| p.heft != 1.0 || p.grown != 1.0).map_or(1.0, |p| p.heft * p.size.powi(3) * p.grown),
                ch: crate::nmrs::ASSET[build.nm].get().and_then(|a| crate::nmrs::joints(build, a)).map(|j| {
                    let mut c = crate::nmrs::Character::new(j, &frames);
                    c.mode = if build.nm == crate::nmrs::MARGE { 2 } else { 1 };
                    c
                }) };
        a.nm_bw = a.ch.as_mut().map(crate::nmbw::Writhe::new);
        if build.nm == crate::nmrs::MARGE {
            (a.legs, a.hip_yaw) = (crate::nmdbm::MARGE_LEGS, crate::nmdbm::MARGE_YAW);
            if let Some(ch) = a.ch.as_mut() {
                ch.stance_yaw = (crate::nmdbm::MARGE_YAW.1, crate::nmdbm::MARGE_HEAD);
            }
        }
        a
    }

    pub fn stop_all_behaviours(&mut self) {
        self.extras = Default::default();
        if let Some(ch) = self.ch.as_mut().filter(|_| self.bb_on) {
            crate::nmbb::BodyBalance::deactivate(ch);
        }
        self.bb_on = false;
        (self.balance, self.shot, self.catch_fall, self.flinch) = (None, None, None, None);
        (self.nm_shot, self.nm_hit) = (None, None);
        self.nm_hit_next.clear();
        self.nm_head.deactivate();
        (self.writhe, self.windmill, self.pedal, self.roll_up, self.brace) = (None, None, None, None, None);
        self.rds_on = false;
        self.nm_pa_msg = Some(None);
        self.nm_hf_msg = Some(None);
        self.nm_fow_msg = Some(None);
        self.nm_br_msg = Some(None);
        self.nm_bf_msg = Some(None);
        match self.ch.as_mut() {
            Some(ch) => self.nm_gr.deactivate(ch, &mut self.nm_head, &mut self.nm_twist, self.balance.as_mut().and_then(|b| b.dbm.as_mut())),
            None => self.nm_gr_msg = Some(None),
        }
        self.nm_twist.deactivate();
        self.head_look.clear();
        self.shot_look = false;
        self.relax = None;
        self.doll.unpin();
        if let Some(ch) = self.ch.as_mut() {
            let frames: Vec<Transform> = self.doll.frames().collect();
            for i in 0..ch.eff.len() {
                let now = ch.actual(i, &frames);
                ch.eff[i].reset(&ch.joints[i]);
                ch.eff[i].desired = now;
            }
        }
        self.set_stiffness(5.0, 0.5);
        self.doll.restiffen();
    }

    pub fn set_stiffness(&mut self, body_stiffness: f32, damping: f32) {
        self.stiffness = (body_stiffness, damping);
        if let Some(ch) = self.ch.as_mut() {
            ch.set_stiffness(body_stiffness, damping, "fb", None);
        }
    }

    pub fn active_pose(&mut self, mask: Option<&str>) {
        let Some(ch) = self.ch.as_mut().filter(|_| !self.incoming.is_empty()) else { return };
        let n = ch.eff.len();
        let m = crate::nmrs::incoming_rows(self.doll.nm, &self.incoming);
        for j in mask.map_or_else(|| (0..n).collect(), crate::nmrs::Character::mask).into_iter().filter(|&j| j < n) {
            ch.eff[j].desired = ch.incoming_angle(j, &m);
        }
    }

    pub(crate) fn incoming_pose(&mut self, dt: f32) {
        if self.incoming_state != 0 && !self.incoming.is_empty() {
            let moving = self.incoming_state != 3 && self.incoming_prev.len() == self.incoming.len();
            let n = self.doll.frames().count();
            let rows = |t: &Transform| { let m = Mat3::from_quat(t.rotation); [m.x_axis, m.y_axis, m.z_axis, t.translation] };
            for (k, t) in self.incoming.iter().enumerate().take(n) {
                let (v, w) = if moving {
                    crate::ragdoll::start_motion(&rows(&self.incoming_prev[k]), &rows(t), Vec3::ZERO, 1.0 / dt, 0.5, 3.0, 2.0, self.incoming_v_ref)
                } else {
                    (Vec3::ZERO, Vec3::ZERO)
                };
                self.doll.set_body(k, t.translation, t.rotation, v, w);
            }
        }
        self.incoming_state = match self.incoming_state { 2 | 3 => 3, _ => 0 };
        self.incoming_prev.clone_from(&self.incoming);
    }

    pub fn body_relax(&mut self, relaxation: f32, damping: f32) {
        self.body_relax_full(relaxation, Some(damping), None, false);
    }

    pub fn body_relax_full(&mut self, relaxation: f32, damping: Option<f32>, mask: Option<&str>, hold_pose: bool) {
        if let Some(ch) = self.ch.as_mut() {
            let frames: Vec<Transform> = self.doll.frames().collect();
            let x = (100.0 - relaxation) * 0.01;
            let n = ch.eff.len();
            let js = mask.map_or_else(|| (0..n).collect(), crate::nmrs::Character::mask);
            for j in js.into_iter().filter(|&j| j < n) {
                if hold_pose {
                    ch.eff[j].desired = ch.angle(j, &frames);
                }
                let def = ch.joints[j].clone();
                ch.eff[j].set_relaxation(&def, x);
                if let Some(d) = damping {
                    ch.eff[j].set_damping_relaxation(&def, d);
                }
            }
        }
        self.relax = Some((relaxation / 100.0, damping.unwrap_or(1.0)));
        if self.ch.is_none() {
            (self.balance, self.lean, self.catch_fall, self.flinch, self.brace) = (None, None, None, None, None);
            self.nm_br_msg = Some(None);
            self.nm_hf_msg = Some(None);
            self.doll.unpin();
        }
    }

    pub fn configure_balance(&mut self, c: ConfigureBalance) {
        if let Some(d) = self.balance.as_mut().and_then(|b| b.dbm.as_mut()) {
            d.p.taper = c.taper_knee_strength;
        }
        self.config = c;
    }

    pub fn body_balance(&mut self, build: &Build, world: &Collision, params: BodyBalance) {
        self.body_balance_with(build, world, params, None);
    }

    pub fn body_balance_full(&mut self, build: &Build, world: &Collision, p: crate::nmbb::Params) {
        let params = BodyBalance { body_stiffness: p.body_stiffness, damping: p.damping, elbow: p.elbow, shoulder: p.shoulder,
                                   max_steps: p.max_steps, spine_stiffness: p.head_look_always_timeout, spine_damping: p.body_turn_timeout };
        self.body_balance_with(build, world, params, Some(p));
    }

    fn body_balance_with(&mut self, build: &Build, world: &Collision, params: BodyBalance, full: Option<crate::nmbb::Params>) {
        if let Some(b) = self.balance.as_mut().filter(|b| b.on()) {
            b.params = params;
            if let Some(bb) = b.bb.as_mut() {
                match full {
                    Some(f) => bb.p = f,
                    None => params.onto(&mut bb.p),
                }
            }
            return;
        }
        let frames: Vec<Transform> = self.doll.frames().collect();
        if let Some(ch) = self.ch.as_mut() {
            let mut p = full.unwrap_or_default();
            if full.is_none() {
                params.onto(&mut p);
            }
            let snap = ch.clone();
            let bb = crate::nmbb::BodyBalance::activate(p, ch, &crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames });
            if p.use_head_look {
                let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
                let (h, (pos, vel)) = (&mut self.nm_head, crate::nmbb::look_at(&p, &body, &self.obstacles));
                (h.pos, h.vel, h.stiffness, h.damping, h.always_look) = (pos, vel, 11.0, 1.0, true);
                h.activate(ch, &body);
                self.head_look = vec![(pos, f32::INFINITY)];
            }
            self.config.leg_stiffness = 12.0;
            let body = crate::nmdbm::Body { ch, build, doll: &self.doll, frames: &frames };
            let dbm = crate::nmdbm::Dbm::new(&body, dbm_config(self.config, &params, self.lean_bias, self.hip_yaw, self.legs));
            dbm.activate(ch);
            self.lean_bias = -0.22;
            self.balance = Some(Balance { params, dbm: Some(dbm), bb: Some(bb), feet: [Foot::Planted(Transform::IDENTITY); 2], steps: 0,
                                          stand_height: dbm.p.pelvis_height.max(0.7), stepping: 0, still: 0.0 });
            return;
        }
        let mut feet = [Foot::Planted(Transform::IDENTITY); 2];
        let mut ground = f32::MAX;
        for (side, f) in self.roles.foot.iter().enumerate() {
            let Some(k) = *f else { self.fallen = true; return };
            let at = frames[k];
            let Some(z) = world.ground(at.translation + Vec3::Z * 0.3, 1.0) else { self.fallen = true; return };
            let rest = z + build.parts[k].radius;
            ground = ground.min(z);
            feet[side] = if at.translation.z - rest < 0.08 {
                Foot::Planted(at)
            } else {
                Foot::Stepping(at, Transform::from_translation(at.translation.with_z(rest)).with_rotation(at.rotation), 0.5)
            };
            self.doll.pin(k, at);
        }
        self.balance = Some(Balance { params, dbm: None, bb: None, feet, steps: 0, stand_height: (frames[self.roles.pelvis].translation.z - ground).max(0.7), stepping: 0, still: 0.0 });
    }

    pub fn set_zero(&mut self, build: &Build, frames: &[Transform]) {
        self.zero = build.parts.iter().enumerate()
            .map(|(i, p)| p.parent.map_or(frames[i].rotation, |j| frames[j].rotation.inverse() * frames[i].rotation)).collect();
        if let Some(ch) = self.ch.as_mut() {
            let m = crate::nmrs::incoming_rows(build.nm, frames);
            for i in 0..ch.eff.len() {
                ch.eff[i].zero = ch.incoming_angle(i, &m);
            }
            if let Some((legs, yaw, head)) = crate::nmdbm::zero_stance(ch, build, frames) {
                (self.legs, self.hip_yaw, ch.stance_yaw) = (legs, yaw, (yaw.1, head));
            }
        }
    }

    pub fn set_zero_rows(&mut self, build: &Build, m: &[[Vec3; 3]], d: &[Vec3]) {
        if let Some(ch) = self.ch.as_mut() {
            for i in 0..ch.eff.len() {
                ch.eff[i].zero = ch.incoming_angle(i, m);
            }
            let row = |k: usize| crate::nmdbm::M34 { a: m[k][0], b: m[k][1], c: m[k][2], d: d[k] };
            if let Some((legs, yaw, head)) = crate::nmdbm::zero_stance_rows(ch, build, &row) {
                (self.legs, self.hip_yaw, ch.stance_yaw) = (legs, yaw, (yaw.1, head));
            }
        }
    }

    pub fn set_zero_angles(&mut self, z: &[[f32; 3]]) {
        if let Some(ch) = self.ch.as_mut() {
            for (e, z) in ch.eff.iter_mut().zip(z) {
                e.zero = *z;
            }
        }
    }

    pub fn set_effectors(&mut self, m: &[([f32; 3], f32, f32, f32)]) {
        if let Some(ch) = self.ch.as_mut() {
            for (e, m) in ch.eff.iter_mut().zip(m) {
                (e.desired, e.muscle_stiffness, e.strength, e.damping) = *m;
            }
        }
    }

    pub fn set_last_step(&mut self, stiff: &[f32], twist: &[f32], block: [Vec3; 6], agent: (u32, u32), stab: Option<(f32, f32)>) {
        self.doll.set_stiff(std::iter::once(0.0).chain(stiff.iter().copied()).collect());
        self.doll.prep_twist = std::iter::once(None).chain(twist.iter().map(|&t| Some(t))).collect();
        if let Some(ch) = self.ch.as_mut() {
            let [a, b, c, d, vel, angvel] = block;
            ch.block = Some(crate::nmdbm::Block { com: d, vel, angvel, m: crate::nmdbm::M34 { a, b, c, d } });
            (ch.agent, ch.mode, ch.stabilise) = (agent.0, agent.1, stab);
        }
    }

    pub fn set_reads(&mut self, r: &[[f32; 3]]) {
        if let Some(ch) = self.ch.as_mut().filter(|ch| r.len() == ch.joints.len()) {
            ch.read = r.to_vec();
        }
    }

    pub fn hold(&mut self, weapon: bool, two_handed_gun: bool) {
        if let Some(ch) = self.ch.as_mut() {
            ch.busy = [weapon && two_handed_gun, weapon];
        }
    }

    pub fn set_agent(&mut self, i: u32) {
        if let Some(c) = self.ch.as_mut() {
            c.agent = i;
        }
    }

    pub fn set_inst_id(&mut self, i: i32) {
        if let Some(c) = self.ch.as_mut() {
            c.inst_id = i;
        }
    }

    pub fn agent_id(&self) -> Option<u32> {
        self.ch.as_ref().map(|c| c.agent)
    }

    pub fn body_balance_look(&mut self, pos: Vec3) {
        if let Some(bb) = self.balance.as_mut().and_then(|b| b.bb.as_mut()) {
            (bb.p.use_head_look, bb.p.head_look_pos) = (true, pos);
        }
    }

    pub fn shot_look(&mut self, pos: Vec3) {
        if let Some(s) = self.nm_hit.as_mut() {
            (s.use_head_look, s.head_look) = (true, pos);
        }
        if let Some(s) = self.nm_shot.as_mut() {
            (s.p.use_head_look, s.p.head_look) = (true, pos);
        }
    }

    pub fn flinch_at(&mut self, pos: Vec3) {
        if let Some(f) = self.flinch.as_mut() {
            f.pos = pos;
        }
        if let Some(m) = self.nm_fl_msg.as_mut() {
            m.pos = pos;
        }
        self.nm_fl.pos = pos;
    }

    pub fn lean_in_direction(&mut self, dir: Vec3, amount: f32, time: f32, as_force: bool) {
        self.lean = Some((crate::nmdbm::vnorm(dir), amount, time, as_force));
    }

    pub fn stay_upright(&mut self, on: bool, stiffness: f32, damping_scale: f32) {
        if let Some(ch) = self.ch.as_mut() {
            ch.stabilise = on.then_some((stiffness, damping_scale));
        }
    }

    pub fn shot_relax(&mut self, period: f32) {
        self.nm_relax = Some(period);
    }

    pub fn shot(&mut self, s: Shot) {
        if self.ch.is_some() {
            self.nm_hit = Some(s);
            return;
        }
        if s.add_shock_spin {
            let r = self.random.next();
            let spin = (s.shock_spin_min + (s.shock_spin_max - s.shock_spin_min) * r) * SHOCK_SPIN;
            let chest = self.roles.spine.last().copied().unwrap_or(self.roles.pelvis);
            let x = self.doll.frames().nth(chest).map_or(Vec3::ZERO, |f| f.translation);
            let sign = (s.hit_point - x).cross(s.bullet_vel).z.signum();
            for &k in &self.roles.spine {
                self.doll.spin(k, Vec3::Z * spin * sign);
            }
        }
        if self.ch.is_none() {
            self.head_look = vec![(s.hit_point, s.head_look_at_wound_max_timer)];
        }
        self.shot = Some((s, 0.0));
    }

    pub fn shot_start(&mut self, s: Shot) {
        if self.ch.is_none() || (self.nm_shot.is_none() && self.nm_hit.is_none()) {
            self.shot(s);
        }
    }

    pub(crate) fn held_params(&self, st: &mut crate::msg::State, msg: &str) {
        match msg {
            "headLook" => {
                let h = &self.nm_head;
                st.head_look = crate::msg::HeadLookMsg { pos: h.pos, vel: h.vel, stiffness: h.stiffness, damping: h.damping,
                                                         always_look: h.always_look, always_eyes_horizontal: h.always_eyes_horizontal };
            }
            "shot" => {
                if let Some(p) = self.nm_hit.or_else(|| self.nm_shot.as_ref().map(|s| s.p)) {
                    st.shot = p;
                }
            }
            "pointArm" => st.point_arm = match self.nm_pa_msg { Some(Some(p)) => p, _ => self.nm_pa.p },
            "bodyBalance" => {
                if let Some(bb) = self.balance.as_ref().and_then(|b| b.bb.as_ref()) {
                    st.body_balance = bb.p;
                }
            }
            _ => {}
        }
    }

    pub(crate) fn update_params(&mut self, build: &Build, world: &Collision, st: &crate::msg::State, msg: &str, body_part: bool) {
        match msg {
            "headLook" => {
                let (p, h) = (&st.head_look, &mut self.nm_head);
                (h.pos, h.vel, h.stiffness, h.damping) = (p.pos, p.vel, p.stiffness, p.damping);
                (h.always_look, h.always_eyes_horizontal) = (p.always_look, p.always_eyes_horizontal);
                if !self.head_look.is_empty() {
                    self.head_look = vec![(p.pos, f32::INFINITY)];
                }
            }
            "shot" if self.ch.is_none() => self.shot(st.shot),
            "shot" => {
                if self.nm_hit.is_some() {
                    self.nm_hit_next.push((st.shot, body_part));
                } else if let Some(s) = self.nm_shot.as_mut() {
                    if !body_part {
                        s.p = st.shot;
                    } else {
                        s.hit(st.shot);
                        if let Some(ch) = self.ch.as_mut().filter(|_| !s.failed()) {
                            hold_stiffness(ch);
                        }
                    }
                }
            }
            "pointArm" => {
                if let Some(Some(p)) = self.nm_pa_msg.as_mut() {
                    *p = st.point_arm;
                }
                self.nm_pa.p = st.point_arm;
            }
            "bodyBalance" if self.balance.as_ref().is_some_and(|b| b.bb.is_some()) => self.body_balance_full(build, world, st.body_balance),
            "upperBodyFlinch" => {
                let m = st.upper_body_flinch;
                flinch_params(m, &mut self.nm_fl);
                if let Some(f) = self.flinch.as_mut() {
                    *f = UpperBodyFlinch { new_hit: m.new_hit.or(f.new_hit), ..m };
                }
                if let Some(f) = self.nm_fl_msg.as_mut() {
                    *f = UpperBodyFlinch { new_hit: m.new_hit.or(f.new_hit), ..m };
                }
            }
            "braceForImpact" => { let p = &st.brace_for_impact; self.brace_at(p.pos, p.look, p.instance_index); }
            _ => {}
        }
    }

    pub fn body_balance_stop(&mut self) {
        if !self.balance.as_ref().is_some_and(|b| b.bb.is_some()) {
            return;
        }
        if let Some(ch) = self.ch.as_mut() {
            crate::nmbb::BodyBalance::deactivate(ch);
        }
        (self.bb_on, self.balance) = (false, None);
        self.doll.unpin();
        (self.catch_fall, self.nm_cf_msg) = (None, None);
        if let Some(mut f) = self.nm_cf.take() {
            f.deactivate(&mut self.nm_head, &mut self.nm_twist);
        }
        self.head_look.clear();
        self.nm_head.deactivate();
    }

    pub fn stop_roll_up(&mut self) {
        self.roll_up = None;
    }

    pub fn stop_roll_down_stairs(&mut self) {
        self.rds_on = false;
    }

    pub fn catch_fall(&mut self, c: CatchFall) {
        self.catch_fall_msg(c);
        (self.balance, self.flinch, self.windmill, self.pedal) = (None, None, None, None);
        self.doll.unpin();
    }

    fn catch_fall_msg(&mut self, c: CatchFall) {
        self.catch_fall = Some(c);
        if self.ch.is_some() {
            self.nm_cf_msg = Some(crate::nmcf::Params { torso_stiffness: c.torso_stiffness, legs_stiffness: c.legs_stiffness,
                                                        arms_stiffness: c.arms_stiffness, z_axis_spin_reduction: c.z_axis_spin_reduction, ..Default::default() });
        }
    }

    pub fn stop_catch_fall(&mut self) {
        self.catch_fall = None;
    }

    pub fn catch_fall_full(&mut self, build: &Build, p: crate::nmcf::Params) {
        self.catch_fall(CatchFall { torso_stiffness: p.torso_stiffness, legs_stiffness: p.legs_stiffness, arms_stiffness: p.arms_stiffness,
                                    z_axis_spin_reduction: p.z_axis_spin_reduction });
        if let Some(ch) = self.ch.as_mut() {
            let (snap, frames): (_, Vec<Transform>) = (ch.clone(), self.doll.frames().collect());
            let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
            let mut out = crate::nmshot::Out::default();
            let mut c = crate::nmshot::Ctx::new(ch, &body, &mut out, false);
            start_catch_fall(&mut self.nm_cf, p, &mut c, &mut self.nm_head, &mut self.nm_twist);
        }
    }

    pub fn head_look(&mut self, build: &Build, look: Option<(Vec3, Vec3)>, stiffness: f32, damping: f32, always_look: bool, always_eyes_horizontal: bool) {
        match look {
            Some((pos, vel)) => {
                self.head_look = vec![(pos, f32::INFINITY)];
                (self.nm_head.vel, self.nm_head.stiffness, self.nm_head.damping) = (vel, stiffness, damping);
                (self.nm_head.always_look, self.nm_head.always_eyes_horizontal) = (always_look, always_eyes_horizontal);
                let owned = self.nm_cf.as_ref().is_some_and(|f| f.p.use_head_look) || self.nm_fl.active || self.nm_gr.active || self.nm_br.active || self.nm_hf.active();
                if let Some(ch) = self.ch.as_mut().filter(|_| !owned) {
                    let (snap, frames): (_, Vec<Transform>) = (ch.clone(), self.doll.frames().collect());
                    self.nm_head.pos = pos;
                    self.nm_head.activate(ch, &crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames });
                }
            }
            None => {
                self.head_look.clear();
                self.nm_head.deactivate();
            }
        }
    }

    pub fn muscle_stiffness_x(&mut self, k: f32) {
        if let Some(ch) = self.ch.as_mut() {
            for (j, e) in ch.joints.iter().zip(ch.eff.iter_mut()) {
                e.muscle_stiffness = j.muscle_stiffness * k;
            }
        }
    }

    pub fn set_stiffness_mask(&mut self, body_stiffness: f32, damping: f32, mask: &str) {
        if mask == "fb" {
            self.stiffness = (body_stiffness, damping);
        }
        if let Some(ch) = self.ch.as_mut() {
            ch.set_stiffness(body_stiffness, damping, mask, None);
        }
    }

    pub fn stop_lean(&mut self) {
        self.lean = None;
    }

    pub fn upper_body_flinch(&mut self, build: &Build, f: UpperBodyFlinch) {
        self.flinch = Some(f);
        match self.ch.as_mut() {
            Some(ch) => {
                let (snap, frames): (_, Vec<Transform>) = (ch.clone(), self.doll.frames().collect());
                let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
                let mut out = crate::nmshot::Out::default();
                let mut c = crate::nmshot::Ctx::new(ch, &body, &mut out, false);
                start_flinch(f, &mut self.nm_fl, &mut c, &mut self.nm_head, &mut self.nm_twist);
            }
            None => self.nm_fl_msg = Some(f),
        }
    }

    pub fn body_writhe(&mut self, w: BodyWrithe) {
        self.writhe = Some(w);
        self.nm_bw_msg = Some(w);
    }

    pub fn arms_windmill(&mut self, w: ArmsWindmill) {
        self.windmill = Some(w);
        self.nm_aw_msg = Some(w);
    }

    pub fn pedal_legs(&mut self, p: PedalLegs) {
        self.pedal = Some(p);
        self.nm_pd_msg = Some(p);
    }

    pub fn stop_pedal(&mut self) {
        self.pedal = None;
    }

    pub fn body_roll_up(&mut self, r: BodyRollUp) {
        self.roll_up = Some(r);
        self.nm_ru_msg = Some(r);
    }

    pub fn body_foetal(&mut self, p: Option<crate::nmbf::Foetal>) {
        self.nm_bf_msg = Some(p);
    }

    pub fn roll_down_stairs(&mut self, r: crate::nmrds::RollDownStairs) {
        (self.nm_rds_msg, self.rds_on) = (Some(r), true);
    }

    pub fn grab(&mut self, build: &Build, p: Option<crate::nmgr::GrabParams>) {
        let Some(ch) = self.ch.as_mut() else {
            self.nm_gr_msg = Some(p);
            return;
        };
        let (snap, frames): (_, Vec<Transform>) = (ch.clone(), self.doll.frames().collect());
        let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
        let mut out = crate::nmshot::Out::default();
        let mut c = crate::nmshot::Ctx::new(ch, &body, &mut out, false);
        let dbm = self.balance.as_mut().and_then(|b| b.dbm.as_mut());
        match p {
            Some(p) => {
                self.nm_gr.p = p;
                self.nm_gr.place(&self.obstacles);
                self.nm_gr.activate(&mut c, &mut self.nm_head, &mut self.nm_twist, dbm);
            }
            None => self.nm_gr.deactivate(c.ch, &mut self.nm_head, &mut self.nm_twist, dbm),
        }
    }

    pub fn point_arm(&mut self, p: Option<crate::nmpa::PointArmParams>) {
        self.nm_pa_msg = Some(p);
    }

    pub fn fall_over_wall(&mut self, f: FallOverWall) {
        self.nm_fow_msg = Some(Some(f));
    }

    pub fn high_fall(&mut self, h: HighFall) {
        self.nm_hf_msg = Some(Some(h));
    }

    pub fn brace_for_impact(&mut self, build: &Build, b: BraceForImpact) {
        self.brace = Some(b);
        let Some(ch) = self.ch.as_mut().filter(|_| !self.nm_br.active) else {
            self.nm_br_msg = Some(Some(b));
            return;
        };
        let (snap, frames): (_, Vec<Transform>) = (ch.clone(), self.doll.frames().collect());
        let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
        let mut out = crate::nmshot::Out::default();
        let mut c = crate::nmshot::Ctx::new(ch, &body, &mut out, false);
        let mut k = crate::nmbr::Kids { head: &mut self.nm_head, twist: &mut self.nm_twist, pedal: &mut self.nm_pd,
                                        rds: &mut self.nm_rds, ru: &mut self.nm_ru, grab: &mut self.nm_gr,
                                        dbm: self.balance.as_mut().and_then(|b| b.dbm.as_mut()), fallen: self.fallen };
        self.nm_br_started = Some(self.nm_br.activate(b, &mut c, &mut k, &[]));
        self.nm_br_msg = None;
    }

    fn brace_at(&mut self, pos: Vec3, look: Vec3, inst: i32) {
        for p in [self.brace.as_mut(), Some(&mut self.nm_br.p), self.nm_br_msg.as_mut().and_then(|m| m.as_mut())].into_iter().flatten() {
            (p.pos, p.look, p.instance_index) = (pos, look, inst);
        }
    }

    #[allow(dead_code)]
    pub fn define_attached_object(&mut self, k: usize, pos: Vec3, mass: f32) {
        if let Some(ch) = self.ch.as_mut() {
            ch.attached = Some((k, pos, mass));
        }
    }

    pub fn joints(&self, build: &Build) -> Vec<Option<crate::ragdoll::JointInfo>> {
        let rigid = self.ch.as_ref().map_or(vec![], |ch| ch.rigid(1.0 / 60.0, self.strength, self.most));
        (0..build.parts.len()).map(|k| crate::ragdoll::joint_info(build, k, rigid.get(k))).collect()
    }

    pub fn balancing(&self) -> bool {
        self.balance.as_ref().is_some_and(Balance::on)
    }

    pub fn settled(&self) -> bool {
        self.balance.as_ref().is_some_and(|b| b.on() && b.still > 0.4)
    }

    pub fn balance_steps(&self) -> Option<(u32, u32)> {
        self.balance.as_ref().filter(|b| b.on()).map(|b| (b.steps, b.dbm.as_ref().map_or(0, |d| d.f.stepping)))
    }

    pub fn character(&self) -> Option<&crate::nmrs::Character> {
        self.ch.as_ref()
    }

    pub fn stance(&self) -> Option<Vec3> {
        let b = self.balance.as_ref().filter(|b| b.on())?;
        if let Some(d) = &b.dbm {
            return Some(d.f.centre);
        }
        let at = |f: &Foot| match f { Foot::Planted(t) => t.translation, Foot::Stepping(_, to, _) => to.translation };
        Some((at(&b.feet[0]) + at(&b.feet[1])) * 0.5)
    }

    pub fn relaxing(&self) -> bool {
        self.relax.is_some()
    }

    pub fn catching(&self) -> bool {
        self.catch_fall.is_some()
    }

    pub fn rolled_up(&self) -> bool {
        self.roll_up.is_some()
    }

    fn nm_shot_step(&mut self, build: &Build, frames: &[Transform], dbm_lost: bool, dt: f32, tick: bool) -> crate::nmshot::Out {
        let mut out = crate::nmshot::Out::default();
        let Some(ch) = self.ch.as_mut() else { return out };
        let snap = ch.clone();
        let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames };
        let mut c = crate::nmshot::Ctx::new(ch, &body, &mut out, dbm_lost);
        if let Some(p) = self.nm_hit.take() {
            match self.nm_shot.as_mut() {
                Some(s) => s.hit(p),
                None => self.nm_shot = Some(crate::nmshot::Shot::activate(p, &mut c, dt)),
            }
            for (p, body_part) in std::mem::take(&mut self.nm_hit_next) {
                let Some(s) = self.nm_shot.as_mut() else { break };
                if !body_part {
                    s.p = p;
                } else {
                    s.hit(p);
                    if !s.failed() {
                        hold_stiffness(c.ch);
                    }
                }
            }
        }
        let Some(s) = self.nm_shot.as_mut() else { return out };
        if tick {
            if let Some(r) = self.nm_relax.take() {
                s.t194 = r;
            }
            s.tick(&mut c, dt);
            if s.failed() {
                self.fallen = true;
            }
        }
        match out.dbm {
            Some(true) => {
                (self.lean_bias, self.config.leg_stiffness) = (0.0, 12.0);
                let ch = self.ch.as_mut().unwrap();
                let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames };
                let params = self.balance.as_ref().map_or(BodyBalance { body_stiffness: 0.0, damping: 0.0, elbow: 0.0, shoulder: 0.0,
                    max_steps: out.dbm_steps.unwrap_or(100).max(0) as u32, spine_stiffness: 0.0, spine_damping: 0.0 }, |b| b.params);
                let mut dbm = crate::nmdbm::Dbm::new(&body, dbm_config(self.config, &params, self.lean_bias, self.hip_yaw, self.legs));
                if let Some(old) = self.balance.as_ref().and_then(|b| b.dbm.as_ref()) {
                    dbm.restart_over(old);
                }
                dbm.activate(ch);
                let stand_height = dbm.p.pelvis_height.max(0.7);
                match self.balance.as_mut() {
                    Some(b) => b.dbm = Some(dbm),
                    None => self.balance = Some(Balance { params, dbm: Some(dbm), bb: None, feet: [Foot::Planted(Transform::IDENTITY); 2],
                                                          steps: 0, stand_height, stepping: 0, still: 0.0 }),
                }
            }
            Some(false) => match self.balance.as_mut() {
                Some(b) if b.bb.is_some() => {
                    if let Some(d) = b.dbm.take() {
                        b.stepping = d.f.stepping;
                        self.doll.unpin();
                    }
                }
                Some(_) => {
                    self.balance = None;
                    self.doll.unpin();
                }
                None => {}
            },
            None => {}
        }
        if out.hold_stiffness {
            hold_stiffness(self.ch.as_mut().unwrap());
        }
        if let Some(d) = self.balance.as_mut().and_then(|b| b.dbm.as_mut()) {
            if let Some(n) = out.dbm_steps {
                d.f.countdown = n;
            }
            if let Some(x) = out.dbm_leg_offset {
                d.leg_offset = x;
            }
            if let Some(x) = out.dbm_knee_scale {
                d.knee_scale = x;
            }
            if let Some(x) = out.dbm_acc {
                d.acc = x;
            }
            if let Some(v) = out.dbm_turn {
                (d.pv.fixed_vel, d.pv.vel) = (true, v);
            }
        }
        if out.head_look_stop {
            self.nm_head.deactivate();
            self.head_look.clear();
            self.shot_look = false;
        }
        if let Some((stiffness, damping)) = out.head_look_start {
            self.nm_head = crate::nmhl::HeadLook { stiffness, damping, always_look: true, ..Default::default() };
            self.shot_look = true;
        }
        if let Some(pos) = out.head_look_target {
            if self.shot_look || out.head_look_activate {
                self.head_look = vec![(pos, dt)];
            }
            (self.nm_head.pos, self.nm_head.vel) = (pos, Vec3::ZERO);
        }
        if out.fall_stop {
            (self.catch_fall, self.roll_up) = (None, None);
        }
        if let Some([legs, torso, arms]) = out.catch_fall {
            let c = CatchFall { torso_stiffness: torso, legs_stiffness: legs, arms_stiffness: arms, z_axis_spin_reduction: 0.0 };
            if self.catch_fall.is_none() {
                self.catch_fall(c);
            } else {
                self.catch_fall_msg(c);
            }
        }
        if let Some(k) = out.roll_up.filter(|_| self.roll_up.is_none()) {
            self.body_roll_up(BodyRollUp { stiffness: k, leg_push: 0.0, arm_reach_amount: 0.0, use_arm_to_slow_down: 2.0, asymmetrical_legs: 0.75, mask: Some("fb"), hips_from_vel: Some(true) });
        }
        out
    }

    pub fn step(&mut self, build: &Build, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        self.feedback.clear();
        let n = substeps(dt);
        for _ in 0..n {
            self.substep(build, world, dt / n as f32, obstacles, others);
        }
    }

    pub fn step_follow(&mut self, build: &Build, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        self.feedback.clear();
        let n = substeps(dt);
        for _ in 0..n {
            self.behave(build, world, dt / n as f32, obstacles);
            self.physics(build, world, dt / n as f32, obstacles, others);
        }
    }

    pub(crate) fn substep(&mut self, build: &Build, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        self.physics(build, world, dt, obstacles, others);
        self.behave(build, world, dt, obstacles);
    }

    pub(crate) fn physics(&mut self, build: &Build, world: &Collision, dt: f32, obstacles: &[Obstacle], others: &[Capsule]) {
        let Some(held) = self.physics_drive(build, dt) else { return };
        self.doll.step(build, world, dt, held.drive(), obstacles, others);
        self.physics_done(dt);
    }

    pub fn step_group(agents: &mut [(&mut Agent, usize, &Build, &[Obstacle], &[Capsule])], world: &Collision, dt: f32, group: &mut crate::ragdoll::Group) {
        agents.iter_mut().for_each(|a| a.0.feedback.clear());
        let n = substeps(dt);
        let h = dt / n as f32;
        for _ in 0..n {
            for (a, _, b, ob, _) in agents.iter_mut().filter(|a| a.0.contact_wake && a.0.held.is_none()) {
                a.doll.fresh = false;
                a.behave(b, world, h, ob);
            }
            agents.iter_mut().for_each(|a| a.0.contact_wake = false);
            let kept: Vec<Option<crate::ragdoll::Kept>> = agents.iter_mut().map(|(a, _, b, ..)| a.physics_drive(b, h)).collect();
            let mut ms: Vec<crate::ragdoll::Member> = agents.iter_mut().zip(&kept).filter_map(|((a, id, b, ob, ot), k)| {
                k.as_ref().map(|k| crate::ragdoll::Member { doll: &mut a.doll, id: *id, build: b, drive: k.drive(), obstacles: ob, others: ot })
            }).collect();
            Ragdoll::step_group(&mut ms, world, h, group);
            drop(ms);
            for ((a, ..), k) in agents.iter_mut().zip(&kept) {
                if k.is_some() {
                    a.physics_done(h);
                }
            }
            for (a, _, b, ob, _) in agents.iter_mut() {
                a.behave(b, world, h, ob);
            }
        }
    }

    fn physics_drive(&mut self, build: &Build, dt: f32) -> Option<crate::ragdoll::Kept> {
        let mut held = self.held.take()?;
        self.incoming_pose(dt);
        self.extras.hard_step(&mut self.doll, &self.incoming, dt);
        if self.nm_shot.is_none() && self.nm_hit.is_some() && self.doll.awake() && self.ch.is_some() {
            let frames: Vec<Transform> = self.doll.frames().collect();
            self.nm_shot_started = Some(self.nm_shot_step(build, &frames, false, self.nm_dt.unwrap_or(dt), false));
        }
        if let Some(ch) = self.ch.as_mut() {
            for (j, d, ms, st, dm, og) in std::mem::take(&mut self.eff_override) {
                (ch.eff[j].desired, ch.eff[j].muscle_stiffness, ch.eff[j].strength, ch.eff[j].damping) = (d, ms, st, dm);
                if let Some(og) = og {
                    ch.eff[j].oppose_gravity = og;
                }
            }
        }
        let pelvis = self.roles.pelvis;
        if let Some(ch) = self.ch.as_mut().filter(|ch| eff_words(ch) != self.held_eff) {
            held.muscles = ch.muscles(Muscle { target: self.zero[pelvis], stiffness: 0.0, damping: 0.0 });
            held.muscles[pelvis].stiffness = 0.0;
            held.rigid = ch.rigid(dt, self.strength, self.most);
        }
        Some(held)
    }

    fn physics_done(&mut self, dt: f32) {
        self.doll.fresh = false;
        self.nm_gr.snapped(&self.doll.snapped);
        self.nm_dt = Some(dt);
    }

    pub(crate) fn behave(&mut self, build: &Build, world: &Collision, dt: f32, obstacles: &[Obstacle]) {
        self.time += dt;
        self.obstacles.clear();
        self.obstacles.extend_from_slice(obstacles);
        let phys_dt = dt;
        let dt = self.nm_dt.unwrap_or(phys_dt);
        let frames: Vec<Transform> = self.doll.frames().collect();
        let n = build.parts.len();
        let (body_k, body_d) = self.stiffness;
        let mut catch = None;
        let mut dbm_lost = false;
        let awake = self.doll.awake();
        if self.doll.rigid.is_none() {
            self.doll.rigid = crate::nmdbm::own_rage(&self.doll);
        }
        let nm_tick = awake;
        let mut started = if let Some(o) = self.nm_shot_started.take() { o } else if awake && self.nm_hit.is_some() { self.nm_shot_step(build, &frames, false, dt, false) } else { Default::default() };
        let mut brace_out = crate::nmshot::Out::default();
        let mut brace_started = crate::nmbr::BraceOut::default();
        if let (Some(ch), true, Some(Some(_))) = (self.ch.as_mut(), nm_tick, &self.nm_br_msg) {
            let Some(Some(p)) = self.nm_br_msg.take() else { unreachable!() };
            let snap = ch.clone();
            let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
            let mut c = crate::nmshot::Ctx::new(&mut *ch, &body, &mut brace_out, false);
            let dbm = self.balance.as_mut().and_then(|b| b.dbm.as_mut());
            let mut k = crate::nmbr::Kids { head: &mut self.nm_head, twist: &mut self.nm_twist, pedal: &mut self.nm_pd,
                                            rds: &mut self.nm_rds, ru: &mut self.nm_ru, grab: &mut self.nm_gr, dbm,
                                            fallen: self.fallen };
            brace_started = self.nm_br.activate(p, &mut c, &mut k, obstacles);
        } else if let Some(o) = self.nm_br_started.take().filter(|_| nm_tick) {
            self.nm_br.place(obstacles);
            brace_started = o;
        }
        if let Some(s) = brace_started.leg_stiffness.take() {
            self.config.leg_stiffness = s;
            if let (false, Some(ch)) = (self.fallen, self.ch.as_mut()) {
                match self.balance.as_mut().and_then(|b| b.dbm.as_mut()) {
                    Some(d) => {
                        d.f.countdown = 100;
                        d.activate(ch);
                    }
                    None => {
                        let params = BodyBalance { max_steps: 100, ..BodyBalance::from(Default::default()) };
                        let body = crate::nmdbm::Body { ch, build, doll: &self.doll, frames: &frames };
                        let d = crate::nmdbm::Dbm::new(&body, crate::nmdbm::Config { lean_bias: -0.22, ..dbm_config(self.config, &params, 0.0, self.hip_yaw, self.legs) });
                        if d.held {
                            d.activate(ch);
                            self.balance = Some(Balance { params, dbm: Some(d), bb: None, feet: [Foot::Planted(Transform::IDENTITY); 2], steps: 0,
                                                          stand_height: d.p.pelvis_height.max(0.7), stepping: 0, still: 0.0 });
                        }
                    }
                }
            }
        }
        if let (Some(ch), true) = (self.ch.as_mut(), nm_tick) {
            ch.up = Vec3::Z;
        }
        if let (Some(b), Some(ch), true) = (self.balance.as_mut(), self.ch.as_mut(), nm_tick) {
            if let Some(dbm) = b.dbm.as_mut() {
                let c = dbm_config(self.config, &b.params, self.lean_bias, self.hip_yaw, self.legs);
                dbm.lean = match self.lean {
                    Some((dir, amount, _, as_force)) => crate::nmdbm::Lean { mode: crate::nmdbm::LeanMode::Dir(dir), amount, as_force },
                    None => Default::default(),
                };
                let was = dbm.f.stepping;
                let body = crate::nmdbm::Body { ch, build, doll: &self.doll, frames: &frames };
                let tick = dbm.tick(&body, world, c, dt);
                ch.up = dbm.p.up;
                let failed = match tick {
                    Some(s) => {
                        dbm.apply(&s, ch);
                        if was == 0 && dbm.f.stepping != 0 {
                            b.steps += 1;
                        }
                        if dbm.success {
                            self.feedback.push(crate::nmshot::Feedback { slot: 12, name: "balance", value: 0.0 });
                        }
                        if let Some(v) = dbm.event {
                            self.feedback.push(crate::nmshot::Feedback { slot: 20, name: "balance", value: v });
                        }
                        let vh = s.hvel.length();
                        b.still = if vh < 0.25 && dbm.f.stepping == 0 { b.still + dt } else { 0.0 };
                        false
                    }
                    None => {
                        debug!("nm: balance lost (dbm) t {:.2}", self.time);
                        self.feedback.push(crate::nmshot::Feedback { slot: 8, name: "balance", value: 0.0 });
                        self.fallen = true;
                        dbm_lost = true;
                        true
                    }
                };
                if let Some(bb) = b.bb.as_mut() {
                    let snap = ch.clone();
                    let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
                    let out = bb.tick(ch, &body, obstacles, dbm.f.stepping, failed, self.catch_fall.is_some(), dt);
                    (dbm.pv.fixed_vel, dbm.pv.vel) = (out.turn.is_some(), out.turn.unwrap_or(Vec3::ZERO));
                    if let Some(k) = out.step_clamp {
                        self.config.step_clamp_scale = k;
                    }
                    if let Some((pos, vel)) = out.head_look {
                        self.head_look = vec![(pos, dt)];
                        let h = &mut self.nm_head;
                        (h.pos, h.vel, h.stiffness, h.damping, h.always_look) = (pos, vel, out.head_look_stiffness, 1.0, true);
                        h.activate(ch, &body);
                    }
                    catch = out.catch_fall;
                }
            } else if let Some(bb) = b.bb.as_mut() {
                let snap = ch.clone();
                let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
                let out = bb.tick(ch, &body, obstacles, b.stepping, false, self.catch_fall.is_some(), dt);
                if let Some(k) = out.step_clamp {
                    self.config.step_clamp_scale = k;
                }
                if let Some((pos, vel)) = out.head_look {
                    self.head_look = vec![(pos, dt)];
                    let h = &mut self.nm_head;
                    (h.pos, h.vel, h.stiffness, h.damping, h.always_look) = (pos, vel, out.head_look_stiffness, 1.0, true);
                    h.activate(ch, &body);
                }
                catch = out.catch_fall;
            }
        }
        let mut bb_walked = 0u32;
        if let Some([legs, torso, arms]) = catch {
            self.catch_fall(CatchFall { torso_stiffness: torso, legs_stiffness: legs, arms_stiffness: arms, z_axis_spin_reduction: 0.0 });
            let before = walk_actives!(self);
            if let (Some(p), Some(ch)) = (self.nm_cf_msg.take(), self.ch.as_mut()) {
                let snap = ch.clone();
                let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
                let mut out = crate::nmshot::Out::default();
                let mut c = crate::nmshot::Ctx::new(&mut *ch, &body, &mut out, dbm_lost);
                start_catch_fall(&mut self.nm_cf, p, &mut c, &mut self.nm_head, &mut self.nm_twist);
            }
            bb_walked = walk_actives!(self) & !before;
        }
        let bb_on = self.balance.as_ref().is_some_and(|b| b.bb.is_some());
        if let Some(ch) = self.ch.as_mut().filter(|_| self.bb_on && !bb_on) {
            crate::nmbb::BodyBalance::deactivate(ch);
        }
        self.bb_on = bb_on;
        let mut walked = bb_walked;
        let mut stop_dbm = false;
        let mut hf_balance = None;
        if let Some(ch) = self.ch.as_mut().filter(|_| nm_tick) {
            let snap = ch.clone();
            let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
            let mut c = crate::nmshot::Ctx::new(&mut *ch, &body, &mut brace_out, dbm_lost);
            let before = walk_actives!(self);
            let dbm = self.balance.as_mut().and_then(|b| b.dbm.as_mut());
            let mut k = crate::nmbr::Kids { head: &mut self.nm_head, twist: &mut self.nm_twist, pedal: &mut self.nm_pd,
                                            rds: &mut self.nm_rds, ru: &mut self.nm_ru, grab: &mut self.nm_gr, dbm,
                                            fallen: self.fallen };
            let mut o = match self.nm_br_msg.take() {
                Some(None) => self.nm_br.deactivate(&mut c, &mut k),
                _ => std::mem::take(&mut brace_started),
            };
            let t = self.nm_br.tick(&mut c, &mut k, obstacles, dt);
            let (tick_cf, mid) = (t.catch_fall.is_some(), walk_actives!(self));
            walked |= mid & !before;
            o.catch_fall = t.catch_fall.or(o.catch_fall);
            (o.stop_catch_fall, o.stop_dbm) = (o.stop_catch_fall || t.stop_catch_fall, o.stop_dbm || t.stop_dbm);
            if let Some(p) = o.catch_fall {
                self.catch_fall = Some(CatchFall { torso_stiffness: p.torso_stiffness, legs_stiffness: p.legs_stiffness,
                                                   arms_stiffness: p.arms_stiffness, z_axis_spin_reduction: p.z_axis_spin_reduction });
                start_catch_fall(&mut self.nm_cf, p, &mut c, &mut self.nm_head, &mut self.nm_twist);
            }
            if o.stop_catch_fall {
                self.catch_fall = None;
                if let Some(mut f) = self.nm_cf.take() {
                    f.deactivate(&mut self.nm_head, &mut self.nm_twist);
                }
            }
            stop_dbm |= o.stop_dbm;
            if tick_cf {
                walked |= walk_actives!(self) & !mid;
            }
        }
        if let Some(ch) = self.ch.as_mut().filter(|_| nm_tick) {
            match self.nm_bf_msg.take() {
                Some(Some(p)) => {
                    self.nm_bf = crate::nmbf::Foetal { active: self.nm_bf.active, ..p };
                    self.nm_bf.activate(ch);
                }
                Some(None) => self.nm_bf.deactivate(),
                None => {}
            }
            self.nm_bf.tick(ch);
        }
        let hl_was = self.nm_head.active;
        let msgs = (self.nm_cf_msg.is_some() || self.nm_cf.as_ref().is_some_and(|f| f.active), self.nm_ru_msg.is_some() || self.nm_ru.active);
        let mut shot_out = if nm_tick { self.nm_shot_step(build, &frames, dbm_lost, dt, true) } else { Default::default() };
        let (shot_cf, shot_ru) = (!msgs.0 && self.nm_cf_msg.is_some(), !msgs.1 && self.nm_ru_msg.is_some());
        shot_out.feedback.extend(std::mem::take(&mut started.feedback));
        shot_out.impulse.extend(std::mem::take(&mut started.impulse));
        if let Some(d) = self.balance.as_ref().and_then(|b| b.dbm.as_ref()) {
            shot_out.force(self.roles.pelvis, d.push);
        }
        if let Some(ch) = self.ch.as_mut().filter(|_| nm_tick) {
            let snap = ch.clone();
            let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
            if let Some((pos, k)) = shot_out.spine_twist {
                if !self.nm_twist.active {
                    self.nm_twist = Default::default();
                    (self.nm_twist.stiffness, self.nm_twist.always) = (k, true);
                    self.nm_twist.activate(ch);
                    walked |= W_TWIST;
                }
                self.nm_twist.pos = pos;
            }
            if shot_out.spine_twist_stop {
                self.nm_twist.deactivate();
            }
            let mut out = crate::nmshot::Out::default();
            let mut c = crate::nmshot::Ctx::new(&mut *ch, &body, &mut out, dbm_lost);
            if let Some(p) = self.nm_cf_msg.take() {
                let b = walk_actives!(self);
                start_catch_fall(&mut self.nm_cf, p, &mut c, &mut self.nm_head, &mut self.nm_twist);
                if shot_cf {
                    walked |= walk_actives!(self) & !b;
                }
            }
            if self.catch_fall.is_none() {
                if let Some(mut f) = self.nm_cf.take() {
                    f.deactivate(&mut self.nm_head, &mut self.nm_twist);
                }
            }
            if let Some(m) = self.nm_fl_msg.take() {
                start_flinch(m, &mut self.nm_fl, &mut c, &mut self.nm_head, &mut self.nm_twist);
            }
            if self.flinch.is_none() {
                self.nm_fl.deactivate(c.ch, &mut self.nm_head, &mut self.nm_twist);
            }
            match self.nm_fow_msg.take() {
                Some(Some(p)) => {
                    self.nm_fow.p = p;
                    self.nm_fow.activate();
                }
                Some(None) => self.nm_fow.deactivate(),
                None => {}
            }
            let mut hf = match self.nm_hf_msg.take() {
                Some(Some(p)) => {
                    self.nm_hf.p = p;
                    self.nm_hf.activate(&mut c, world, obstacles, dt);
                    Default::default()
                }
                Some(None) => {
                    let balancing = self.balance.as_ref().is_some_and(Balance::on);
                    let fallen = self.fallen || !balancing;
                    let dbm = self.balance.as_mut().and_then(|b| b.dbm.as_mut());
                    let mut k = crate::nmhf::Kids { head: &mut self.nm_head, pedal: &mut self.nm_pd, windmill: &mut self.nm_aw,
                                                    ru: &mut self.nm_ru, rds: &mut self.nm_rds, dbm, fallen, balancing,
                                                    catching: self.nm_cf.as_ref().is_some_and(|f| f.active) };
                    self.nm_hf.deactivate(&mut k)
                }
                None => crate::nmhf::HighFallOut::default(),
            };
            if let Some(m) = self.nm_bw_msg.take() {
                let w = self.nm_bw.get_or_insert_with(|| crate::nmbw::Writhe::new(c.ch));
                (w.arm_stiffness, w.back_stiffness, w.leg_stiffness) = (m.arm_stiffness, m.back_stiffness, m.leg_stiffness);
                (w.arm_damping, w.back_damping, w.leg_damping) = (m.arm_damping, m.back_damping, m.leg_damping);
                (w.roll_over, w.mask) = (m.roll_over, m.mask);
                (w.arm_period, w.back_period, w.leg_period) = (m.arm_period, m.back_period, m.leg_period);
                (w.arm_amplitude, w.back_amplitude, w.leg_amplitude) = (m.arm_amplitude, m.back_amplitude, m.leg_amplitude);
                (w.elbow_amplitude, w.knee_amplitude) = (m.elbow_amplitude, m.knee_amplitude);
                w.activate(c.ch);
            }
            if self.writhe.is_none() {
                if let Some(w) = self.nm_bw.as_mut() {
                    w.deactivate(c.ch);
                }
            }
            if let Some(m) = self.nm_aw_msg.take() {
                let w = &mut self.nm_aw;
                (w.body_stiffness, w.period, w.amplitude, w.phase) = (m.body_stiffness, m.period, m.amplitude, m.phase);
                w.disable_on_impact = m.disable_on_impact;
                w.activate(c.ch);
            }
            if self.windmill.is_none() && !self.nm_hf.active() {
                self.nm_aw.deactivate();
            }
            if let Some(m) = self.nm_pd_msg.take() {
                let p = &mut self.nm_pd;
                (p.leg_stiffness, p.radius, p.angular_speed) = (m.leg_stiffness, m.radius, m.angular_speed);
                (p.pedal_offset, p.speed_asymmetry, p.back_pedal) = (m.pedal_offset, m.speed_asymmetry, m.back_pedal);
                (p.adaptive, p.drag_multiplier) = (m.adaptive, m.drag_multiplier);
                (p.left, p.right, p.radius_variance, p.random_seed) = (m.left, m.right, m.radius_variance, m.random_seed);
                (p.leg_angle_variance, p.centre_forwards, p.centre_up) = (m.leg_angle_variance, m.centre_forwards, m.centre_up);
                p.activate(&mut c);
            }
            if self.pedal.is_none() && !self.nm_br.active && !self.nm_hf.active() {
                self.nm_pd.deactivate();
            }
            if let Some(m) = self.nm_ru_msg.take() {
                let r = &mut self.nm_ru;
                (r.stiffness, r.leg_push, r.arm_reach_amount) = (m.stiffness, m.leg_push, m.arm_reach_amount);
                (r.use_arm_to_slow_down, r.asymmetrical_legs) = (m.use_arm_to_slow_down, m.asymmetrical_legs);
                if let Some(k) = m.mask {
                    r.mask = k;
                }
                if let Some(h) = m.hips_from_vel {
                    r.hips_from_vel = h;
                }
                r.activate(c.ch);
                if shot_ru {
                    walked |= W_RU;
                }
            }
            if self.roll_up.is_none() && !self.rds_on && !self.nm_br.active && !self.nm_hf.active() {
                self.nm_ru.deactivate();
            }
            if let Some(m) = self.nm_rds_msg.take() {
                let r = &mut self.nm_rds;
                (r.stiffness, r.damping, r.forcemag, r.asymmetry, r.asymmetrical_legs) = (m.stiffness, m.damping, m.forcemag, m.asymmetry, m.asymmetrical_legs);
                (r.use_arm_to_slow_down, r.arm_reach_amount, r.use_zero_pose, r.spin_when_in_air) = (m.use_arm_to_slow_down, m.arm_reach_amount, m.use_zero_pose, m.spin_when_in_air);
                (r.leg_push, r.arm_reach_length, r.stiffness_decay_time, r.stiffness_decay_target) = (m.leg_push, m.arm_reach_length, m.stiffness_decay_time, m.stiffness_decay_target);
                (r.use_custom_roll_dir, r.z_axis_spin_reduction, r.custom_roll_dir) = (m.use_custom_roll_dir, m.z_axis_spin_reduction, m.custom_roll_dir);
                self.nm_rds.activate(&mut c, &mut self.nm_ru);
            }
            if !self.rds_on && !self.nm_br.active && !self.nm_hf.active() {
                self.nm_rds.deactivate(&mut self.nm_ru);
            }
            {
                let dbm = self.balance.as_mut().and_then(|b| b.dbm.as_mut());
                match self.nm_gr_msg.take() {
                    Some(Some(p)) => {
                        self.nm_gr.p = p;
                        self.nm_gr.place(obstacles);
                        self.nm_gr.activate(&mut c, &mut self.nm_head, &mut self.nm_twist, dbm);
                    }
                    Some(None) => self.nm_gr.deactivate(c.ch, &mut self.nm_head, &mut self.nm_twist, dbm),
                    None => {}
                }
            }
            match self.nm_pa_msg.take() {
                Some(Some(p)) => self.nm_pa.activate(p),
                Some(None) => self.nm_pa.deactivate(),
                None => {}
            }
            if !self.nm_cf.as_ref().is_some_and(|f| f.p.use_head_look) && !self.nm_fl.active && !self.nm_gr.active && !self.nm_br.active && !self.nm_hf.active() {
                match self.head_look.first() {
                    Some(&(pos, _)) => {
                        self.nm_head.pos = pos;
                        self.nm_head.activate(c.ch, &body);
                    }
                    None => self.nm_head.deactivate(),
                }
            }
            let b0 = walk_actives!(self);
            let waits = move |now: u32, bit: u32| (walked | now & !b0) & bit != 0;
            if !waits(walk_actives!(self), W_TWIST) {
                self.nm_twist.tick(c.ch, &body);
            }
            if !waits(walk_actives!(self), W_CF) {
                if let Some(f) = self.nm_cf.as_mut() {
                    f.tick(&mut c, world, obstacles, &mut self.nm_head, &mut self.nm_twist, dt);
                }
            }
            if !waits(walk_actives!(self), W_RU) {
                self.nm_ru.tick(&mut c, self.nm_br.active, dt);
            }
            {
                let dbm = self.balance.as_mut().and_then(|b| b.dbm.as_mut()).filter(|_| !self.fallen);
                self.nm_fl.tick(&mut c, &mut self.nm_head, &mut self.nm_twist, dbm, self.fallen, dt);
            }
            self.nm_fow.tick(&mut c, dt);
            {
                let balancing = self.balance.as_ref().is_some_and(Balance::on);
                let fallen = self.fallen || !balancing;
                let dbm = self.balance.as_mut().and_then(|b| b.dbm.as_mut());
                let mut k = crate::nmhf::Kids { head: &mut self.nm_head, pedal: &mut self.nm_pd, windmill: &mut self.nm_aw,
                                                ru: &mut self.nm_ru, rds: &mut self.nm_rds, dbm, fallen, balancing,
                                                catching: self.nm_cf.as_ref().is_some_and(|f| f.active) };
                let t = self.nm_hf.tick(&mut c, &mut k, world, obstacles, dt);
                hf.catch_fall = t.catch_fall.or(hf.catch_fall);
                hf.balance = t.balance.or(hf.balance);
                hf.dbm_set = t.dbm_set.or(hf.dbm_set);
                (hf.stop_catch_fall, hf.stop_balance) = (hf.stop_catch_fall || t.stop_catch_fall, hf.stop_balance || t.stop_balance);
                if t.taper_off {
                    self.config.taper_knee_strength = false;
                }
            }
            if let Some(p) = hf.catch_fall {
                self.catch_fall = Some(CatchFall { torso_stiffness: p.torso_stiffness, legs_stiffness: p.legs_stiffness,
                                                   arms_stiffness: p.arms_stiffness, z_axis_spin_reduction: p.z_axis_spin_reduction });
                start_catch_fall(&mut self.nm_cf, p, &mut c, &mut self.nm_head, &mut self.nm_twist);
            }
            if hf.stop_catch_fall {
                self.catch_fall = None;
                if let Some(mut f) = self.nm_cf.take() {
                    f.deactivate(&mut self.nm_head, &mut self.nm_twist);
                }
            }
            stop_dbm |= hf.stop_balance;
            hf_balance = hf.balance.map(|p| (p, hf.dbm_set));
            if !waits(walk_actives!(self), W_RDS) {
                self.nm_rds.tick(&mut c, dt);
            }
            if let Some(w) = self.nm_bw.as_mut() {
                w.tick(&mut c, dt);
            }
            self.extras.tick(&mut c, obstacles, dt);
            if !waits(walk_actives!(self), W_PD) {
                self.nm_pd.tick(&mut c, dt);
            }
            if !waits(walk_actives!(self), W_AW) {
                self.nm_aw.tick(&mut c, dt);
            }
            if !waits(walk_actives!(self), W_GR) {
                let dbm = self.balance.as_mut().and_then(|b| b.dbm.as_mut());
                self.nm_gr.place(obstacles);
                self.nm_gr.tick(&mut c, &mut self.nm_head, &mut self.nm_twist, dbm, dt);
            }
            if (hl_was || (shot_out.head_look_start.is_none() && !shot_out.head_look_activate)) && !waits(walk_actives!(self), W_HEAD) {
                self.nm_head.tick(c.ch, &body);
            }
            self.nm_pa.tick(&mut c);
            for o in [brace_out, out] {
                for (k, &t) in o.torque.iter().enumerate() {
                    shot_out.torque(k, t);
                }
                for (k, &f) in o.force.iter().enumerate() {
                    shot_out.force(k, f);
                }
                shot_out.impulse.extend(o.impulse);
                shot_out.feedback.extend(o.feedback);
            }
        }
        if stop_dbm {
            self.balance = None;
        }
        if let Some((p, set)) = hf_balance {
            self.body_balance_with(build, world, BodyBalance::from(p), Some(p));
            if let (Some((acc, knee)), Some(d)) = (set, self.balance.as_mut().and_then(|b| b.dbm.as_mut())) {
                (d.p.taper, d.acc, d.knee_scale) = (false, acc, knee);
            }
        }
        let dt = phys_dt;
        let mut m: Vec<Muscle> = match self.ch.as_mut() {
            Some(ch) => ch.muscles(Muscle { target: self.zero[self.roles.pelvis], stiffness: 0.0, damping: 0.0 }),
            None => (0..n).map(|i| Muscle { target: self.zero[i], stiffness: body_k, damping: body_d }).collect(),
        };
        m[self.roles.pelvis].stiffness = 0.0;
        let r = self.roles.clone();
        let fwd = self.facing * Vec3::Y;
        let right = self.facing * Vec3::X;
        let aim = |i: usize, dir: Vec3| -> Quat {
            let p = &build.parts[i];
            let out = frames[i].rotation * -p.anchor.normalize_or(Vec3::NEG_Y);
            let want = Quat::from_rotation_arc(out, dir.normalize_or(out)) * frames[i].rotation;
            p.parent.map_or(want, |j| frames[j].rotation.inverse() * want)
        };
        let bend = |i: usize, a: f32| -> Quat {
            let p = &build.parts[i];
            match p.limit {
                Limit::Hinge { axis, lo, hi, .. } => p.rest.inverse() * Quat::from_axis_angle(axis, a.clamp(lo, hi)),
                Limit::BoneHinge { pf, cf, lo, hi } => pf * Quat::from_rotation_z(a.clamp(lo, hi)) * cf.inverse(),
                _ => self.zero[i],
            }
        };
        let rigid = self.ch.as_ref().map_or(vec![], |ch| ch.rigid(dt, self.strength, self.most));
        let (mut torque, mut force, mut push) = (vec![], vec![], vec![]);
        if let Some(ch) = self.ch.as_mut() {
            let mut yaw = ch.foot_yaw;
            let snap = ch.clone();
            let body = crate::nmdbm::Body { ch: &snap, build, doll: &self.doll, frames: &frames };
            torque = crate::nmdbm::foot_yaw(&body, &mut yaw, dt);
            let og = crate::nmdbm::oppose_gravity(&body, snap.attached);
            for (t, g) in torque.iter_mut().zip(og) {
                *t += g;
            }
            if let Some((k, _)) = ch.stabilise {
                push = crate::nmdbm::stabilise(&body, k, dt);
            }
            ch.foot_yaw = yaw;
            ch.attached = None;
        }
        for (k, p) in push.into_iter().enumerate().filter(|(_, p)| *p != Vec3::ZERO) {
            self.doll.impulse(k, self.doll.centre(k), p);
        }
        for (v, add) in [(&mut torque, &shot_out.torque), (&mut force, &shot_out.force)] {
            if v.len() < add.len() {
                v.resize(add.len(), Vec3::ZERO);
            }
            for (x, y) in v.iter_mut().zip(add) {
                *x += *y;
            }
        }
        for &(k, at, imp) in &shot_out.impulse {
            self.doll.impulse(k, at, imp);
        }
        for (k, at, f) in std::mem::take(&mut self.push) {
            for (v, x) in [(&mut force, f), (&mut torque, (at - self.doll.centre(k)).cross(f))] {
                if v.len() <= k {
                    v.resize(k + 1, Vec3::ZERO);
                }
                v[k] += x;
            }
        }
        self.feedback.append(&mut shot_out.feedback);
        let mut drive = Drive { muscles: &[], lift: 0.0, accel: Vec3::ZERO, rigid: &rigid, torque: &torque, force: &force };

        if let Some(b) = self.balance.as_mut().filter(|b| b.dbm.is_none() && b.bb.is_none()) {
            let (com, v) = self.doll.com();
            let mut planted = vec![];
            for (side, foot) in b.feet.iter_mut().enumerate() {
                let Some(k) = r.foot[side] else { continue };
                match *foot {
                    Foot::Planted(t) => planted.push(t.translation),
                    Foot::Stepping(from, to, s) => {
                        let s = (s + dt / STEP_TIME).min(1.0);
                        let mut at = from.translation.lerp(to.translation, s);
                        at.z += self.config.step_height * 4.0 * s * (1.0 - s);
                        self.doll.pin(k, Transform::from_translation(at).with_rotation(from.rotation.slerp(to.rotation, s)));
                        *foot = if s >= 1.0 { b.steps += 1; Foot::Planted(to) } else { Foot::Stepping(from, to, s) };
                    }
                }
            }
            let (a0, a1) = match planted.as_slice() { [a, c] => (*a, *c), [a] => (*a, *a), _ => (com, com) };
            let flat = |x: Vec3| x.with_z(0.0);
            let near = |x: Vec3| { let (s, _) = crate::ragdoll::nearest_point(flat(a0), flat(a1), flat(x)); s };
            let ground = (a0.z + a1.z) * 0.5;
            let h = (com.z - ground).max(0.3);
            let off = flat(com) - near(com);
            let outside = off - off.normalize_or_zero() * off.length().min(FOOT);
            let vh = flat(v);
            let inside = off - outside;
            let mid = flat(com) - flat((a0 + a1) * 0.5);
            let sway = (mid - off) * 0.3 + inside;
            drive.accel = outside * G / h - (sway * ANKLE_STIFFNESS + vh * ANKLE_DAMPING) * if outside == Vec3::ZERO { 1.0 } else { 0.3 };
            drive.lift = 1.0;
            let cp = flat(com) + vh * (h / G).sqrt();
            let miss = (cp - near(cp)).length();
            let stepping = b.feet.iter().any(|f| matches!(f, Foot::Stepping(..)));
            if !stepping && miss > FOOT + STEP_SLACK && b.steps < b.params.max_steps {
                let at = |f: &Foot| match f { Foot::Planted(t) => t.translation, Foot::Stepping(_, to, _) => to.translation };
                let side = if flat(at(&b.feet[0])).distance(cp) > flat(at(&b.feet[1])).distance(cp) { 0 } else { 1 };
                let stay = at(&b.feet[1 - side]);
                let going = (cp - flat(com)).normalize_or(fwd);
                let across = Vec3::Z.cross(going).normalize_or(right);
                let wide = across * (flat(at(&b.feet[side])) - flat(stay)).dot(across).signum() * 0.1;
                let reach = STEP_REACH * self.config.step_clamp_scale;
                let run = ((G / h).sqrt() * STEP_TIME).exp();
                let mut to = flat(stay) + (cp - flat(stay)) * run + wide;
                let d = to - flat(stay);
                if d.length() > reach {
                    to = flat(stay) + d.normalize() * reach;
                }
                let z = world.ground(to.with_z(ground + 0.5), 1.2).unwrap_or(ground);
                let Foot::Planted(from) = b.feet[side] else { unreachable!() };
                let dz = from.translation.z - ground;
                b.feet[side] = Foot::Stepping(from, Transform::from_translation(to.with_z(z + dz)).with_rotation(from.rotation), 0.0);
            }
            b.still = if vh.length() < 0.25 && !stepping && outside == Vec3::ZERO { b.still + dt } else { 0.0 };
            let top = r.head.or(r.neck).or(r.spine.last().copied()).map_or(frames[r.pelvis].translation + Vec3::Z, |k| frames[k].translation);
            let tilt = (top - frames[r.pelvis].translation).angle_between(Vec3::Z);
            let sunk = frames[r.pelvis].translation.z - ground < b.stand_height * ABORT_SINK;
            let out = b.steps >= b.params.max_steps && miss > FOOT + STEP_REACH * 0.5;
            if tilt > self.config.balance_abort_threshold.max(0.3) || sunk || out {
                debug!("nm: balance lost: tilt {tilt:.2} sunk {sunk} ({:.2} of {:.2}) out {out} steps {} miss {miss:.2} t {:.2}", frames[r.pelvis].translation.z - ground, b.stand_height, b.steps, self.time);
                self.fallen = true;
            } else {
                m[r.pelvis] = Muscle { target: self.facing * self.pelvis_up, stiffness: b.params.body_stiffness, damping: b.params.damping };
                for k in r.legs() {
                    m[k].stiffness = self.config.leg_stiffness;
                }
                for k in r.spine.iter().copied() {
                    (m[k].stiffness, m[k].damping) = (b.params.spine_stiffness, b.params.spine_damping);
                }
                for k in r.arms() {
                    m[k].stiffness = b.params.shoulder.max(b.params.elbow) * 5.0;
                }
            }
        }
        if self.fallen && self.balance.is_some() {
            self.balance = None;
            self.doll.unpin();
            drive.lift = 0.0;
            drive.accel = Vec3::ZERO;
        }
        if let Some((_, _, left, _)) = self.lean.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.lean = None;
            }
        }
        if let Some((s, t)) = self.shot.as_mut() {
            *t += dt;
            let (s, t) = (*s, *t);
            let loose = 1.0 - 0.3 * s.looseness_amount * (1.0 - t / 0.3).max(0.0);
            for mm in m.iter_mut() {
                mm.stiffness *= loose;
            }
            if t < s.spine_pain_time {
                let k = (1.0 - t / s.spine_pain_time) * 0.12;
                let side = (s.hit_point - frames[r.pelvis].translation).dot(right).signum();
                for &i in &r.spine {
                    m[i].target = Quat::from_axis_angle(Vec3::X, k * s.spine_pain_multiplier) * Quat::from_axis_angle(Vec3::Y, k * s.spine_pain_twist_multiplier * side) * m[i].target;
                    m[i].stiffness = m[i].stiffness.max(s.body_stiffness * loose);
                }
            }
            if s.reach_for_wound && t > s.time_before_reach_for_wound {
                let mut side = if (s.hit_point - frames[r.pelvis].translation).dot(right) > 0.0 { 1 } else { 0 };
                if r.upper_arm[side] == Some(s.body_part) || r.forearm[side] == Some(s.body_part) {
                    side = 1 - side;
                }
                if let (Some(u), Some(f)) = (r.upper_arm[side], r.forearm[side]) {
                    let shoulder = frames[u].translation;
                    let to = s.hit_point - s.normal * 0.05 - shoulder;
                    m[u] = Muscle { target: aim(u, to + Vec3::Z * 0.1), stiffness: s.body_stiffness, damping: 1.0 };
                    m[f] = Muscle { target: bend(f, -1.8 + to.length().min(0.6)), stiffness: s.body_stiffness, damping: 1.0 };
                }
            }
            if t < 1.0 {
                for &k in &r.spine {
                    let (_, w) = self.doll.motion(k);
                    self.doll.spin(k, -Vec3::Z * w.z * (s.shock_spin_decay_mult * dt).min(1.0) * 0.5);
                }
            }
            if t > 3.0 {
                self.shot = None;
            }
        }
        if let Some(f) = self.flinch.filter(|_| self.ch.is_none()) {
            let chest = r.spine.last().map_or(frames[r.pelvis].translation, |&k| frames[k].translation);
            let to = (f.pos - chest).with_z(0.0).normalize_or(fwd);
            let noise = (self.time * 7.0).sin() * f.noise_scale;
            for side in 0..2 {
                if let (Some(u), Some(fa)) = (r.upper_arm[side], r.forearm[side]) {
                    let s = if side == 0 { -1.0 } else { 1.0 };
                    let dir = to * (0.5 + f.hand_distance_front_back) + Vec3::Z * (0.35 + f.hand_distance_vertical + noise) + Vec3::Z.cross(to) * -s * 0.15;
                    m[u] = Muscle { target: aim(u, dir), stiffness: f.body_stiffness, damping: f.body_damping };
                    m[fa] = Muscle { target: bend(fa, -2.0), stiffness: f.body_stiffness, damping: f.body_damping };
                }
            }
            let away = Vec3::Z.cross(to).normalize_or(right);
            for &i in &r.spine {
                let cur = frames[i].rotation;
                let p = build.parts[i].parent.unwrap_or(r.pelvis);
                m[i] = Muscle { target: frames[p].rotation.inverse() * Quat::from_axis_angle(away, f.back_bend_amount * 0.15) * cur.slerp(frames[p].rotation * self.zero[i], 0.5),
                                stiffness: f.body_stiffness, damping: f.body_damping };
            }
        }
        if let Some(c) = self.catch_fall.filter(|_| self.ch.is_none()) {
            let (_, v) = self.doll.com();
            let going = v.with_z(0.0).normalize_or_zero();
            let to = (going + Vec3::NEG_Z).normalize();
            let down = frames[r.pelvis].translation.z - world.ground(frames[r.pelvis].translation + Vec3::Z * 0.3, 2.0).unwrap_or(-1e3) < 0.35;
            let soft = if down { 0.4 } else { 1.0 };
            for side in 0..2 {
                if let Some(u) = r.upper_arm[side] {
                    m[u] = Muscle { target: aim(u, to), stiffness: c.arms_stiffness * soft, damping: 1.0 };
                }
                if let Some(fa) = r.forearm[side] {
                    m[fa] = Muscle { target: bend(fa, -0.3), stiffness: c.arms_stiffness * soft, damping: 1.0 };
                }
            }
            for &i in &r.spine {
                (m[i].stiffness, m[i].damping) = (c.torso_stiffness * soft, 1.0);
            }
            for k in r.legs() {
                (m[k].stiffness, m[k].damping) = (c.legs_stiffness * soft, 1.0);
            }
            if let Some(h) = r.head {
                m[h] = Muscle { target: self.zero[h], stiffness: c.torso_stiffness, damping: 1.0 };
            }
        }
        if let Some(b) = self.brace.filter(|_| self.ch.is_none()) {
            for side in 0..2 {
                if let (Some(u), Some(fa)) = (r.upper_arm[side], r.forearm[side]) {
                    m[u] = Muscle { target: aim(u, b.pos - frames[u].translation), stiffness: b.body_stiffness, damping: 1.0 };
                    m[fa] = Muscle { target: bend(fa, -0.4), stiffness: b.body_stiffness, damping: 1.0 };
                }
            }
        }
        if let Some(w) = self.windmill.filter(|_| self.ch.is_none()) {
            for side in 0..2 {
                if let (Some(u), Some(fa)) = (r.upper_arm[side], r.forearm[side]) {
                    let a = self.time * std::f32::consts::TAU / w.period.max(0.1) + w.phase * std::f32::consts::TAU * side as f32;
                    let s = if side == 0 { -1.0 } else { 1.0 };
                    let dir = (fwd * a.cos() + Vec3::Z * a.sin()) * w.amplitude + right * s * 0.6;
                    m[u] = Muscle { target: aim(u, dir), stiffness: w.arm_stiffness, damping: 0.7 };
                    m[fa] = Muscle { target: bend(fa, -0.3), stiffness: w.arm_stiffness, damping: 0.7 };
                }
            }
            for &i in &r.spine {
                m[i].stiffness = w.body_stiffness;
            }
        }
        if let Some(p) = self.pedal.filter(|_| self.ch.is_none()) {
            for side in 0..2 {
                if let (Some(th), Some(ca)) = (r.thigh[side], r.calf[side]) {
                    let speed = p.angular_speed * (1.0 + p.speed_asymmetry * 0.1 * side as f32);
                    let a = self.time * speed + std::f32::consts::PI * side as f32;
                    let dir = Vec3::NEG_Z + (fwd * a.cos() + Vec3::Z * a.sin()) * p.radius * 2.0 + fwd * p.pedal_offset;
                    m[th] = Muscle { target: aim(th, dir), stiffness: p.leg_stiffness, damping: 0.8 };
                    m[ca] = Muscle { target: bend(ca, 0.9 + 0.7 * a.sin()), stiffness: p.leg_stiffness, damping: 0.8 };
                }
            }
        }
        if let Some(ru) = self.roll_up.filter(|_| self.ch.is_none()) {
            let up = frames[r.pelvis].rotation * Vec3::Z;
            for &i in &r.spine {
                m[i] = Muscle { target: Quat::from_rotation_x(0.35) * self.zero[i], stiffness: ru.stiffness, damping: 1.0 };
            }
            for side in 0..2 {
                if let (Some(th), Some(ca)) = (r.thigh[side], r.calf[side]) {
                    let chest = frames[r.pelvis].rotation * Vec3::Y;
                    m[th] = Muscle { target: aim(th, chest + up * (1.0 - ru.leg_push)), stiffness: ru.stiffness, damping: 1.0 };
                    m[ca] = Muscle { target: bend(ca, 2.0), stiffness: ru.stiffness, damping: 1.0 };
                }
                if let (Some(u), Some(fa)) = (r.upper_arm[side], r.forearm[side]) {
                    let _ = u;
                    m[fa] = Muscle { target: bend(fa, -1.5 * ru.arm_reach_amount.max(0.3)), stiffness: ru.stiffness, damping: 1.0 };
                }
            }
        }
        if let Some(w) = self.writhe.filter(|_| self.ch.is_none()) {
            let wave = |period: f32, k: usize| (self.time * std::f32::consts::TAU / period.max(0.1) + k as f32 * 1.7).sin();
            for &i in &r.spine {
                m[i] = Muscle { target: Quat::from_rotation_x(0.25 * w.back_amplitude * wave(w.back_period, i)) * Quat::from_rotation_z(0.2 * w.back_amplitude * wave(w.back_period * 1.3, i)) * self.zero[i],
                                stiffness: w.back_stiffness, damping: w.arm_damping };
            }
            for side in 0..2 {
                if let (Some(u), Some(fa)) = (r.upper_arm[side], r.forearm[side]) {
                    m[u] = Muscle { target: Quat::from_rotation_x(0.6 * w.arm_amplitude * wave(w.arm_period, u)) * Quat::from_rotation_z(0.4 * w.arm_amplitude * wave(w.arm_period * 0.7, u)) * self.zero[u],
                                    stiffness: w.arm_stiffness, damping: w.arm_damping };
                    m[fa] = Muscle { target: bend(fa, -0.8 - 0.6 * w.elbow_amplitude * (0.5 + 0.5 * wave(w.arm_period, fa))), stiffness: w.arm_stiffness, damping: w.arm_damping };
                }
                if let (Some(th), Some(ca)) = (r.thigh[side], r.calf[side]) {
                    m[th] = Muscle { target: Quat::from_rotation_x(0.4 * w.leg_amplitude * wave(w.leg_period, th)) * self.zero[th], stiffness: w.leg_stiffness, damping: w.arm_damping };
                    m[ca] = Muscle { target: bend(ca, 0.3 + 0.6 * w.knee_amplitude * (0.5 + 0.5 * wave(w.leg_period, ca))), stiffness: w.leg_stiffness, damping: w.arm_damping };
                }
            }
        }
        if let Some((pos, left)) = self.head_look.first_mut() {
            *left -= dt;
            for k in [r.neck, r.head].into_iter().flatten().filter(|_| self.ch.is_none()) {
                let look = (*pos - frames[k].translation).normalize_or(fwd);
                m[k] = Muscle { target: aim(k, Vec3::Z + look * 0.5), stiffness: m[k].stiffness.max(8.0), damping: 1.0 };
            }
            if *left <= 0.0 {
                self.head_look.remove(0);
            }
        }
        if let Some((share, damping)) = self.relax.as_mut().filter(|_| self.ch.is_none()) {
            let k = 1.0 - *share;
            for mm in m.iter_mut() {
                mm.stiffness *= k;
                mm.damping = mm.damping.max(*damping);
            }
            *share = (*share + dt * 2.0).min(1.0);
        }
        drive.muscles = &m;
        self.doll.grips = self.nm_gr.grips();
        self.doll.contact_off = self.nm_gr.contact_off;
        if let Some(pairs) = crate::nmrs::SELF[build.nm].get().filter(|_| build.parts.len() == delirium_data::nm_asset::PARTS.len()) {
            let legs = self.balance.as_ref().and_then(|b| b.dbm.as_ref()).is_some_and(|d| d.held);
            let hands = self.nm_fl.hands_off;
            self.hands_back |= hands;
            let on = |p: &(usize, usize)| pair_on(*p, legs, hands);
            if self.doll.pairs.len() != pairs.iter().filter(|p| on(p)).count() {
                self.doll.pairs = game_pairs(pairs.iter().copied().filter(on), self.hands_back);
            }
        }
        if let Some(ch) = self.ch.as_ref() {
            let body = crate::nmdbm::Body { ch, build, doll: &self.doll, frames: &frames };
            let block = body.block();
            let read = (0..ch.joints.len()).map(|j| body.read(j)).collect();
            let ch = self.ch.as_mut().unwrap();
            (ch.block, ch.read) = (Some(block), read);
        }
        let (lift, accel) = (drive.lift, drive.accel);
        self.held = Some(crate::ragdoll::Kept { muscles: m, lift, accel, rigid, torque, force });
        self.held_eff = self.ch.as_ref().map_or(vec![], eff_words);
    }
}

fn hold_stiffness(ch: &mut crate::nmrs::Character) {
    ch.set_stiffness(6.0, 0.1, "ub", Some(0.0));
    ch.set_stiffness(6.0, 0.1, "lb", None);
    for j in crate::nmrs::ANKLE {
        ch.eff[j].set_stiffness(12.0, 1.0, None);
    }
}

fn eff_words(ch: &crate::nmrs::Character) -> Vec<[f32; 6]> {
    ch.eff.iter().map(|e| [e.desired[0], e.desired[1], e.desired[2], e.muscle_stiffness, e.strength, e.damping]).collect()
}

fn start_flinch(m: UpperBodyFlinch, f: &mut crate::nmfl::Flinch, c: &mut crate::nmshot::Ctx, head: &mut crate::nmhl::HeadLook,
                twist: &mut crate::nmst::SpineTwist) {
    flinch_params(m, f);
    f.activate(c, head, twist);
}

fn flinch_params(m: UpperBodyFlinch, f: &mut crate::nmfl::Flinch) {
    (f.pos, f.body_stiffness, f.body_damping, f.back_bend_amount) = (m.pos, m.body_stiffness, m.body_damping, m.back_bend_amount);
    (f.hand_distance_vertical, f.hand_distance_left_right, f.hand_distance_front_back, f.noise_scale) =
        (m.hand_distance_vertical, m.hand_distance_left_right, m.hand_distance_front_back, m.noise_scale);
    (f.use_left_arm, f.use_right_arm, f.turn_towards) = (m.use_left_arm, m.use_right_arm, m.turn_towards);
    if let Some(n) = m.new_hit {
        f.new_hit = n;
    }
}

fn start_catch_fall(cf: &mut Option<crate::nmcf::CatchFall>, p: crate::nmcf::Params, c: &mut crate::nmshot::Ctx,
                    head: &mut crate::nmhl::HeadLook, twist: &mut crate::nmst::SpineTwist) {
    match cf.as_mut().filter(|f| f.active) {
        Some(f) => f.p = p,
        None => *cf = Some(crate::nmcf::CatchFall::start(p, c, head, twist)),
    }
}

const W_TWIST: u32 = 1 << 5;
const W_CF: u32 = 1 << 6;
const W_RU: u32 = 1 << 7;
const W_RDS: u32 = 1 << 11;
const W_PD: u32 = 1 << 13;
const W_AW: u32 = 1 << 14;
const W_GR: u32 = 1 << 15;
const W_HEAD: u32 = 1 << 16;

macro_rules! walk_actives {
    ($s:ident) => {
        ($s.nm_twist.active as u32) * W_TWIST | ($s.nm_cf.as_ref().is_some_and(|f| f.active) as u32) * W_CF
            | ($s.nm_ru.active as u32) * W_RU | ($s.nm_rds.active as u32) * W_RDS | ($s.nm_pd.active as u32) * W_PD
            | ($s.nm_aw.active as u32) * W_AW | ($s.nm_gr.active as u32) * W_GR | ($s.nm_head.active as u32) * W_HEAD
    };
}
use walk_actives;

fn substeps(dt: f32) -> usize {
    (dt * 30.0 * crate::ragdoll::SUBSTEPS as f32).round().clamp(1.0, 8.0) as usize
}

const SHOCK_SPIN: f32 = 0.05;

#[derive(Clone, Copy, Debug, Default)]
pub struct Hit {
    pub health: f32,
    pub dying: bool,
    pub moving: bool,
    pub running: bool,
    pub attacker_head: Option<Vec3>,
    pub attacker_neck: Option<Vec3>,
    pub look: Option<(Vec3, bool)>,
    pub attacker_at: Option<Vec3>,
    pub few_steps: bool,
    pub gun: bool,
    pub two_handed: bool,
    pub vel: Vec3,
    pub root: Option<Vec3>,
    pub pos: Vec3,
    pub fwd: Vec3,
    pub fire_period: u32,
    pub weapon_busy: bool,
}

impl Hit {
    pub fn ragdoll(a: &Agent) -> Hit {
        let r = a.doll.root();
        Hit { vel: a.doll.vel(0).0, root: Some(r), pos: r, fwd: forward(a), ..Default::default() }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn task_shot(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, rand: &mut delirium_data::rng::Rand, set: &str, hit: Hit,
                 part: usize, at: Vec3, vel: Vec3, normal: Vec3, next: bool) -> Option<f32> {
    let mut t = crate::shottask::ShotTask::new(&hit, set, part, at, vel, normal);
    let msgs = if next { t.next_hit(rand, tp, &hit, 0, set, part, at, vel, normal) } else { t.update(rand, tp, &hit, 0).msgs };
    let mut st = std::mem::take(&mut a.msgs);
    if next {
        a.hold(hit.gun, hit.two_handed);
    }
    for m in &msgs {
        if m.0 == "shot" && a.ch.is_none() {
            let st = |n: &str, or: f32| tp.get("NMShot_Standing", set, n, or);
            a.body_balance(build, world, BodyBalance { body_stiffness: st("shotBodyStiffness", 13.5), damping: 0.7, elbow: 1.75, shoulder: 1.0,
                                                      max_steps: st("shotMaxSteps", 6.0) as u32, spine_stiffness: 10.0, spine_damping: 1.0 });
        }
        crate::shottask::apply(a, build, world, &mut st, std::slice::from_ref(m));
        if m.0 == "stopAllBehaviours" {
            a.hold(hit.gun, hit.two_handed);
        }
    }
    a.msgs = st;
    if let Some(l) = a.lean.as_mut().filter(|_| t.lean_until != 0) {
        l.2 = t.lean_until as f32 * 1e-3;
    }
    a.task_look = t.look.then_some(t.balance);
    (t.blind_until != 0).then(|| t.blind_until as f32 * 1e-3)
}

pub fn task_balance(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, set: &str, look: Option<Vec3>, few_steps: bool) {
    let hit = Hit { attacker_head: look, few_steps, ..Default::default() };
    let mut msgs = vec![("stopAllBehaviours", vec![])];
    msgs.extend(crate::meleetask::balance_start(tp, set, &hit));
    let mut st = std::mem::take(&mut a.msgs);
    crate::shottask::apply(a, build, world, &mut st, &msgs);
    a.msgs = st;
    a.task_look = Some(true);
}

fn forward(a: &Agent) -> Vec3 {
    a.doll.frames().next().map_or(Vec3::Y, |t| (t.rotation * Vec3::Y).with_z(0.0).normalize_or(Vec3::Y))
}

pub fn weapon_set(w: &delirium_data::weapons::Weapon, agility: u8) -> &'static str {
    let weak = agility < 3;
    if w.damage_type == "MELEE" {
        return if w.has("MELEE_CLUB") { "MELEE_BASEBALL" } else { "MELEE" };
    }
    match (w.slot.as_str(), weak) {
        ("RIFLE", false) => "POWERFUL_WEAPON",
        ("RIFLE", true) => "WEAK_POWERFUL_WEAPON",
        ("SNIPER", false) => "SNIPER_RIFLE",
        ("SNIPER", true) => "WEAK_SNIPER_RIFLE",
        ("SHOTGUN", false) => "SHOTGUN",
        ("SHOTGUN", true) => "WEAK_SHOTGUN",
        (_, true) => "WEAK",
        _ => "*",
    }
}

pub fn balance_set(agility: u8, health: f32, player: bool, gun: bool) -> &'static str {
    if agility < 3 || health < if player { 110.0 } else { 140.0 } {
        "WEAK"
    } else if gun {
        "AGGRESSIVE"
    } else {
        "*"
    }
}

pub fn relax_period(tp: &TaskParams, set: &str, dead: bool) -> f32 {
    tp.get("NMShot_Standing", set, if dead { "shotRelaxPeriod_FatallyInjured" } else { "shotRelaxPeriod_Injured" }, if dead { 0.5 } else { 1.5 })
}

pub fn task_control(a: &mut Agent, task: Task, attacker: impl Fn(u16) -> Option<Vec3>) {
    match (task, attacker(1205), attacker(1204)) {
        (Task::Shot | Task::Balance, Some(pos), _) => match a.task_look {
            Some(true) => a.body_balance_look(pos),
            Some(false) => a.shot_look(pos),
            None => {}
        },
        (Task::Flinch, _, Some(pos)) => a.flinch_at(pos),
        (Task::Relax, ..) => task_relax(a, -1.0, -1.0),
        _ => {}
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PointGun {
    pub fire: bool,
    bone: usize,
    start: f32,
    pub next: f32,
    started: bool,
    gang: bool,
}

impl PointGun {
    const BONES: [u16; 4] = [417, 1203, 13985, 1205];

    pub fn pointing(&self) -> bool {
        self.started
    }

    pub fn bone(&self) -> u16 {
        Self::BONES[self.bone]
    }

    pub fn new(a: &mut Agent, fire: bool, times: [f32; 4], gang: bool) -> PointGun {
        let mut g = delirium_data::rng::Rand::new((a.random.next() * 65536.0) as u32);
        PointGun::draw(&mut g, fire, times.map(|t| t as i32), gang)
    }

    pub fn draw(g: &mut delirium_data::rng::Rand, fire: bool, t: [i32; 4], gang: bool) -> PointGun {
        let bone = g.int_in_range(0, 4) as usize;
        let start = g.int_in_range(t[0], t[1]) as f32 * 1e-3;
        let next = g.int_in_range(t[2], t[3]) as f32 * 1e-3;
        PointGun { fire, bone, start, next, started: false, gang }
    }

    pub fn control(&mut self, a: &mut Agent, dt: f32, target: Vec3) {
        self.start -= dt;
        self.next -= dt;
        if !self.started && self.start <= 0.0 {
            let (lo, hi) = if self.gang { (-0.4, 0.2) } else { (-1.37, 1.67) };
            let twist = lo + (hi - lo) * a.random.next();
            let arm_straightness = 0.75 + 0.2 * a.random.next();
            let p = crate::nmpa::PointArmParams { target, twist, arm_straightness, instance_index: -1, point_swing_limit: 1.0,
                                                  use_zero_pose_when_not_pointing: true, ..a.nm_pa.p };
            a.point_arm(Some(p));
            self.started = true;
        } else {
            a.nm_pa.p.target = target;
            if let Some(Some(p)) = a.nm_pa_msg.as_mut() {
                p.target = target;
            }
        }
    }

    pub fn ready(&self, health: f32) -> bool {
        self.fire && self.next <= 0.0 && health >= 100.0
    }

    pub fn stop(self, a: &mut Agent) {
        if self.started {
            a.point_arm(None);
        }
    }
}

pub fn task_catch_fall(a: &mut Agent, tp: &TaskParams, set: &str) {
    let k = tp.get("NMShot_Standing", set, "shotBodyStiffness", 13.5);
    a.catch_fall(CatchFall { torso_stiffness: k * 0.7, legs_stiffness: k * 0.5, arms_stiffness: k, z_axis_spin_reduction: 0.0 });
}

pub fn task_flinch(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, rand: &mut delirium_data::rng::Rand, set: &str, from: Vec3) {
    let hit = Hit { attacker_neck: Some(from), ..Hit::ragdoll(a) };
    let mut t = crate::meleetask::MeleeTask::Flinch { set: set.into(), started: false, lean_until: 0 };
    let msgs = t.update(rand, tp, &hit, 0, false);
    let mut st = std::mem::take(&mut a.msgs);
    crate::shottask::apply(a, build, world, &mut st, &msgs);
    a.msgs = st;
    if let (Some(l), crate::meleetask::MeleeTask::Flinch { lean_until: ms @ 1.., .. }) = (a.lean.as_mut(), &t) {
        l.2 = *ms as f32 * 1e-3;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Explosion {
    task: crate::explosion::Explosion,
    set: crate::explosion::Set,
    ms: f64,
}

impl Explosion {
    pub fn start(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, rand: &mut delirium_data::rng::Rand) -> Explosion {
        let mut ex = Explosion { task: Default::default(), set: crate::explosion::Set::read(tp), ms: 0.0 };
        ex.control(a, build, world, rand, 0.0, 0.0, false);
        ex
    }

    pub fn state(&self) -> u32 {
        self.task.state
    }

    #[allow(clippy::too_many_arguments)]
    pub fn control(&mut self, a: &mut Agent, build: &Build, world: &Collision, rand: &mut delirium_data::rng::Rand, dt: f32, vz: f32, near: bool) {
        self.ms += dt as f64 * 1000.0;
        let m = self.task.update(&self.set, rand, self.ms as u32, vz, near);
        let mut st = std::mem::take(&mut a.msgs);
        crate::shottask::apply(a, build, world, &mut st, &m);
        a.msgs = st;
    }
}

pub fn jump_roll_push(tp: &TaskParams, v: Vec3, v0: Vec3, rolled: bool) -> Vec3 {
    crate::jumproll::push(&crate::jumproll::Set::read(tp), v, v0, rolled).unwrap_or(Vec3::ZERO)
}

pub fn task_jump_roll(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams) {
    let mut m: Vec<crate::shottask::Msg> = vec![("stopAllBehaviours", vec![])];
    m.extend(crate::jumproll::start(&crate::jumproll::Set::read(tp)));
    send_task_msgs(a, build, world, &m);
}

pub fn jump_roll_up_time(tp: &TaskParams) -> f32 {
    crate::jumproll::Set::read(tp).roll_after as f32 * 1e-3
}

pub fn task_jump_roll_up(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, vel: Vec3) {
    send_task_msgs(a, build, world, &crate::jumproll::roll_up(&crate::jumproll::Set::read(tp), vel));
}

fn send_task_msgs(a: &mut Agent, build: &Build, world: &Collision, m: &[crate::shottask::Msg]) {
    let mut st = std::mem::take(&mut a.msgs);
    crate::shottask::apply(a, build, world, &mut st, m);
    a.msgs = st;
}

#[derive(Clone, Copy, Debug)]
pub struct OnFire {
    set: &'static str,
    task: crate::onfire::OnFire,
    params: crate::onfire::Set,
    ms: f64,
}

impl OnFire {
    pub fn new(set: &'static str) -> OnFire {
        OnFire { set, task: crate::onfire::OnFire::new(1000, 30000, 0.3, None), params: Default::default(), ms: 1000.0 }
    }

    pub fn start(&mut self, a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, rand: &mut delirium_data::rng::Rand) {
        self.params = crate::onfire::Set::read(tp, self.set);
        self.step(a, build, world, rand, false, 200.0, false);
    }

    pub fn lying(&self) -> bool {
        self.task.lying
    }

    pub fn balance_failed(&mut self) {
        self.task.feedback("balance");
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, a: &mut Agent, build: &Build, world: &Collision, rand: &mut delirium_data::rng::Rand, dt: f32, burning: bool,
                  health: f32, dead: bool) -> Option<bool> {
        self.ms += dt as f64 * 1000.0;
        self.step(a, build, world, rand, burning, health, dead)
    }

    #[allow(clippy::too_many_arguments)]
    fn step(&mut self, a: &mut Agent, build: &Build, world: &Collision, rand: &mut delirium_data::rng::Rand, burning: bool, health: f32,
            dead: bool) -> Option<bool> {
        let r = a.doll.frames().next().map_or(Quat::IDENTITY, |t| t.rotation);
        let p = crate::onfire::Ped { side: r * Vec3::X, fwd: r * Vec3::Y, vel: a.doll.com().1, health, dying: dead, burning, no_lie: false };
        let s = self.task.update(&p, &self.params, rand, self.ms as u32);
        let mut st = std::mem::take(&mut a.msgs);
        crate::shottask::apply(a, build, world, &mut st, &s.msgs);
        a.msgs = st;
        s.done.then_some(s.next == Some(Next::Relax))
    }
}

pub fn task_high_fall(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, set: &str, player: bool) {
    send_task_msgs(a, build, world, &crate::highfall::start(&crate::highfall::Set::read(tp, set), player));
}

pub fn task_relax(a: &mut Agent, stiffness: f32, damping: f32) {
    a.body_relax_full(crate::nmrelax::relaxation(stiffness), crate::nmrelax::damping(damping), None, true);
}

#[derive(Clone, Copy, Debug)]
pub struct RollUpAndRelax {
    task: crate::nmrollrelax::RollUpAndRelax,
    set: crate::explosion::Set,
    ms: f64,
}

impl RollUpAndRelax {
    #[allow(clippy::too_many_arguments)]
    pub fn start(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, rand: &mut delirium_data::rng::Rand, stiffness: f32,
                 damping: f32, roll_ms: u32) -> RollUpAndRelax {
        let mut t = RollUpAndRelax { task: crate::nmrollrelax::RollUpAndRelax::new(stiffness, damping, roll_ms), set: crate::explosion::Set::read(tp), ms: 0.0 };
        t.send(a, build, world, rand);
        t
    }

    pub fn control(&mut self, a: &mut Agent, build: &Build, world: &Collision, rand: &mut delirium_data::rng::Rand, dt: f32) {
        self.ms += dt as f64 * 1000.0;
        self.send(a, build, world, rand);
    }

    fn send(&mut self, a: &mut Agent, build: &Build, world: &Collision, rand: &mut delirium_data::rng::Rand) {
        let m = self.task.update(&self.set, rand, self.ms as u32);
        let mut st = std::mem::take(&mut a.msgs);
        crate::shottask::apply(a, build, world, &mut st, &m);
        a.msgs = st;
    }
}

pub fn task_pose(a: &mut Agent, build: &Build, world: &Collision, rand: &mut delirium_data::rng::Rand, low: f32, high: f32) {
    let m = crate::nmpose::Pose::new(low, high, 0, 0, false).update(rand, 0);
    let mut st = std::mem::take(&mut a.msgs);
    crate::shottask::apply(a, build, world, &mut st, &m);
    a.msgs = st;
}

pub fn task_pose_control(a: &mut Agent, pose: Vec<Transform>) {
    a.incoming = pose;
    a.active_pose(None);
}

pub fn task_sit(a: &mut Agent, build: &Build, world: &Collision, sit: &mut crate::nmsit::Sit, now: u32,
                entity: Option<&crate::nmsit::Entity>, pose: Option<Vec<Transform>>) {
    if let Some(p) = pose {
        a.incoming = p;
    }
    let m = sit.update(now, entity);
    let mut st = std::mem::take(&mut a.msgs);
    crate::shottask::apply(a, build, world, &mut st, &m);
    a.msgs = st;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Task {
    Shot,
    Balance,
    Brace,
    Flinch,
    HighFall,
    FallDown,
    OnFire,
    Relax,
    ScriptControl(f32),
    Sit(f32),
}

impl Task {
    pub fn min_time(self) -> f32 {
        if self == Task::FallDown { 2.0 } else { 1.0 }
    }

    pub fn max_time(self) -> f32 {
        if self == Task::Brace { 30.0 } else { 10.0 }
    }

    pub fn times(self) -> [f32; 2] {
        [self.min_time(), self.max_time()]
    }
}

pub fn task_failed(a: &mut Agent, world: &Collision, task: Task) {
    let Some(from) = a.doll.frames().nth(10).map(|t| t.translation).filter(|_| matches!(task, Task::Shot | Task::Balance | Task::Flinch | Task::Brace)) else { return };
    if world.ground_stairs(from, 2.5).is_some_and(|(_, stairs)| stairs) {
        a.roll_down_stairs(a.msgs.roll_down_stairs.clone());
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    Blend,
    Relax,
}

#[allow(clippy::too_many_arguments)]
pub fn task_finish(task: Task, since: f32, times: [f32; 2], v: f32, z: f32, failed: bool, fall: u32, health: f32) -> Option<Next> {
    task_finish_as(task, since, times, v, z, failed, fall, health, false)
}

#[allow(clippy::too_many_arguments)]
pub fn task_finish_as(task: Task, since: f32, [min, max]: [f32; 2], v: f32, z: f32, failed: bool, fall: u32, health: f32,
                      player: bool) -> Option<Next> {
    use Task::*;
    let (flags, row) = match task {
        Balance => (16 | (failed || player && since > 5.0) as u32, 1),
        Brace => (8 | (failed || player && since > 5.0) as u32, 1),
        Flinch => (9, 1),
        FallDown => (if matches!(fall, 4 | 5) { 9 } else { 8 }, 1),
        HighFall => (if failed { 9 } else { 0 } | (player && since > 10.0) as u32, 1),
        Shot => (18 | failed as u32, 1),
        Relax => (3, 0),
        OnFire => return None,
        ScriptControl(end) => return (end > 0.0 && since > end).then_some(Next::Blend),
        Sit(d2) if d2 > 4.0 => return Some(Next::Relax),
        Sit(_) => (9, 1),
    };
    let slow = [[0.1f32, 0.2], [0.2, 0.3]][player as usize][row];
    if flags & 1 != 0 && since > min && v * v < slow * slow {
        return Some(if health < 110.0 && !player && flags & 8 != 0 { Next::Relax } else { Next::Blend });
    }
    let limit = if max != 0.0 && (v * v < 4.0 || z < 0.0) { max } else { 100.0 };
    (since > limit).then_some(Next::Blend)
}

pub fn task_fails(task: Task, name: &str) -> bool {
    name.starts_with(if task == Task::HighFall { "HighFal" } else { "balance" })
}

pub fn gun_threat_failed(task: Task, name: &str, pointing: bool, rand: &mut delirium_data::rng::Rand) -> Option<u32> {
    (matches!(task, Task::Shot | Task::Brace) && pointing && task_fails(task, name)).then(|| rand.int_in_range(500, 1500) as u32)
}

pub fn balance_bar(player: bool, kind: u8, health: f32) -> Option<f32> {
    match (player, kind >= 3, health > 130.0) {
        (true, ..) | (false, true, true) => Some(1.0),
        (false, true, false) | (false, false, true) => Some(0.3),
        (false, false, false) => None,
    }
}

pub fn task_ends(task: Task, fb: &[crate::nmshot::Feedback], past_min: bool, bar: Option<f32>, at_once: bool) -> bool {
    use Task::*;
    fb.iter().any(|f| match (task, f.slot) {
        (HighFall, 12) if f.name.starts_with("HighFall") => past_min || at_once,
        (Shot | Brace, 12) => past_min,
        (Flinch, 12) => past_min && f.name == "balance",
        (Balance, 12) => past_min && f.name.starts_with("balance"),
        (Brace | Flinch | HighFall | Balance, 20) => past_min && f.name.starts_with("balance") && bar.is_some_and(|b| f.value < b),
        _ => false,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct FallDown {
    pub state: u32,
    task: crate::falldown::FallDown,
    ms: f64,
}

pub fn fall_down_try(world: &Collision, root: Vec3, r: f32) -> Option<(u32, Vec3, f32)> {
    let a = -std::f32::consts::PI + std::f32::consts::TAU * r;
    let d = Vec3::new(-a.sin(), a.cos(), 0.0);
    let from = root + Vec3::Z * 0.2;
    if world.cast(from, d, 2.0).is_some() {
        return None;
    }
    let end = from + d * 2.0;
    let (ground, stairs) = world.ground_stairs(end, 4.0).map_or((None, false), |(g, s)| (Some(g), s));
    let low = world.cast(from - Vec3::Z * 0.4, d, 2.0).is_some();
    let z = root.z;
    let state = match ground {
        _ if low && ground.is_none_or(|g| g < z - 1.0 - 0.5) => 5,
        None => 4,
        Some(g) if g < z - 1.0 - 2.0 || stairs && g < z - 2.0 => 6,
        _ => return None,
    };
    let height = match ground {
        Some(g) if state != 6 => g,
        _ => end.z - 4.0,
    };
    Some((state, d, height))
}

impl FallDown {
    pub fn start(a: &mut Agent, build: &Build, world: &Collision, state: u32, dir: Vec3, height: f32) -> FallDown {
        let mut f = FallDown { state, task: crate::falldown::FallDown::new(state, dir, height), ms: 1000.0 };
        f.step(a, build, world, 0.0);
        f
    }

    pub fn balance_failed(&mut self) {
        self.task.feedback("balance", self.ms as u32);
    }

    pub fn control(&mut self, a: &mut Agent, build: &Build, world: &Collision, dt: f32) {
        self.ms += dt as f64 * 1000.0;
        self.step(a, build, world, dt);
    }

    fn step(&mut self, a: &mut Agent, build: &Build, world: &Collision, dt: f32) {
        let fr: Vec<Transform> = a.doll.frames().collect();
        let at = |k: usize| fr.get(k).map_or(Vec3::ZERO, |t| t.translation);
        let r = fr.get(7).map_or(Quat::IDENTITY, |t| t.rotation);
        let pos = a.doll.root();
        let p = crate::falldown::Ped { spine: [r * Vec3::X, r * Vec3::Y, r * Vec3::Z, at(7)], b6: at(6), b10: at(10), b12: at(12), pos,
                                       mass: a.doll.mass(), vel: a.doll.com().1, health: 200.0, ..Default::default() };
        let s = self.task.update(&p, self.ms as u32);
        let mut st = std::mem::take(&mut a.msgs);
        crate::shottask::apply(a, build, world, &mut st, &s.msgs);
        a.msgs = st;
        for q in s.pushes {
            a.doll.impulse(q.part, pos + q.at, if q.impulse { q.v } else { q.v * dt });
        }
    }

    pub fn relaxed(&self) -> bool {
        self.task.stage == 5
    }

    pub fn stage(&self) -> u32 {
        self.task.stage
    }
}

pub fn task_brace(a: &mut Agent, build: &Build, world: &Collision, tp: &TaskParams, set: &str, rand: &mut delirium_data::rng::Rand, few_steps: bool) {
    send_task_msgs(a, build, world, &crate::brace::start(&crate::brace::Set::read(tp, set), rand, few_steps));
}

#[allow(clippy::too_many_arguments)]
pub fn task_brace_control(a: &mut Agent, build: &Build, world: &Collision, m: [Vec3; 4], hit: Option<Vec3>, place: Vec3, look: Vec3, inst: i32) {
    send_task_msgs(a, build, world, &crate::brace::control(m, hit, place, look, inst));
}

