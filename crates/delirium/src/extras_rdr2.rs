//! Behaviours from Red Dead Redemption 2's Euphoria, written from descriptions of how they behave (see the RDR2
//! Euphoria notes); none of RDR2's code or numbers. They are extra messages: nothing here runs unless one is sent with
//! start=true, so every existing behaviour is unchanged. Defaults are Delirium's own round values; a player's own
//! RDR2 values can be sent as parameters.
use crate::extras::{Extra, P};
use crate::msg::{Error, Kind, Message, Param};
use crate::nmrs::{HIP, KNEE, NECK, SPINE};
use crate::nmshot::Ctx;
use bevy_math::Vec3;

pub static MESSAGES: [Message; 4] = [
    Message { name: "shotFallToKnees", params: &[
        Param { name: "ftkBalanceTime", kind: Kind::Float, used: true, doc: "s he keeps balancing before sinking (0.1)" },
        Param { name: "ftkBendRate", kind: Kind::Float, used: true, doc: "how fast knees and hips bend, rad/s (3)" },
        Param { name: "ftkHelperForce", kind: Kind::Float, used: true, doc: "forward push on the pelvis while sinking, N (0)" },
        Param { name: "ftkSpineBend", kind: Kind::Float, used: true, doc: "forward bend of the whole spine while sinking, rad (0.1)" },
        Param { name: "ftkStiffness", kind: Kind::Float, used: true, doc: "legs and spine stiffness (9)" },
        Param { name: "ftkKneelTime", kind: Kind::Float, used: true, doc: "s he stays on his knees (2)" },
        Param { name: "ftkKneelHip", kind: Kind::Float, used: true, doc: "hip bend while kneeling, rad; more leans him forwards (0.3)" },
        Param { name: "ftkLeanHelp", kind: Kind::Float, used: true, doc: "pull of his centre over his knees while kneeling, N per m, damped (1500)" },
        Param { name: "ftkOnKneesArmType", kind: Kind::Int, used: true, doc: "0: arms as they are, 1: arms hang (1)" },
        Param { name: "ftkPitchForwards", kind: Kind::Bool, used: true, doc: "topple forwards (true) or back (false) at the end" },
        Param { name: "ftkPitchForce", kind: Kind::Float, used: true, doc: "push on the chest that tips him over, N (300)" },
    ] },
    Message { name: "shotSpin", params: &[
        Param { name: "spin", kind: Kind::Float, used: true, doc: "the hit's spin, m (none: from hitPoint and bulletDir: the lever arm about his centre across the bullet)" },
        Param { name: "hitPoint", kind: Kind::Vec3, used: true, doc: "where the bullet hit, world (zero)" },
        Param { name: "bulletDir", kind: Kind::Vec3, used: true, doc: "the bullet's direction (zero)" },
        Param { name: "shotSpinMin", kind: Kind::Float, used: true, doc: "spins smaller than this (m) do nothing (0.01)" },
        Param { name: "shotSpinTime", kind: Kind::Float, used: true, doc: "s the heading swings round over, with the yaw torque on (0.4)" },
        Param { name: "shotSpinSettle", kind: Kind::Float, used: true, doc: "s it then holds the new heading; the feet stay planted, so he eases part way back after (0.8)" },
        Param { name: "shotSpinTwistMult", kind: Kind::Float, used: true, doc: "rad of turn per m of spin (10)" },
        Param { name: "shotSpinTwist", kind: Kind::Float, used: true, doc: "most turn, rad; stops once he has turned this far (1.5)" },
        Param { name: "shotSpinTwistVel", kind: Kind::Float, used: true, doc: "stops while he already turns faster than this, rad/s (6)" },
        Param { name: "shotSpinTorque", kind: Kind::Float, used: true, doc: "yaw torque on pelvis and trunk per rad off the heading, N m (150)" },
        Param { name: "shotSpinTorqueMax", kind: Kind::Float, used: true, doc: "most yaw torque, N m (300)" },
        Param { name: "stepTurnType", kind: Kind::Int, used: true, doc: "after the spin: 0 nothing, 1 step round to face attackerPos (0)" },
        Param { name: "stepTurnWait", kind: Kind::Float, used: true, doc: "s after the spin before the step turn (0.3)" },
        Param { name: "stepTurnTime", kind: Kind::Float, used: true, doc: "s the step turn may take (2)" },
        Param { name: "attackerPos", kind: Kind::Vec3, used: true, doc: "where the shooter is, world (zero)" },
    ] },
    Message { name: "duck", params: &[
        Param { name: "magMin", kind: Kind::Float, used: true, doc: "least share of each joint's remaining range the duck bends into, 0..1 (0.25)" },
        Param { name: "magMax", kind: Kind::Float, used: true, doc: "most share (1); the share is random between the two" },
        Param { name: "lsRate", kind: Kind::Float, used: true, doc: "lower spine's bend rate, rad/s (0.5)" },
        Param { name: "usRate", kind: Kind::Float, used: true, doc: "upper spine's bend rate, rad/s (0.5)" },
        Param { name: "headRate", kind: Kind::Float, used: true, doc: "neck's bend rate, rad/s (0.5)" },
        Param { name: "spineTwistRate", kind: Kind::Float, used: true, doc: "spine's side-lean and twist rate, rad/s (0.5)" },
        Param { name: "neckTwistRate", kind: Kind::Float, used: true, doc: "neck's side-lean and twist rate, rad/s (0.5)" },
        Param { name: "timeBeforeDuck", kind: Kind::Float, used: true, doc: "s before the duck starts (0)" },
        Param { name: "duckTime", kind: Kind::Float, used: true, doc: "s spent bending in; then it unbends (0.5)" },
        Param { name: "blendTime", kind: Kind::Float, used: true, doc: "s over which the duck fades back out after duckTime (0.5)" },
        Param { name: "leanTwistStart", kind: Kind::Bool, used: true, doc: "also lean sideways and twist away (true)" },
        Param { name: "bendForward", kind: Kind::Bool, used: true, doc: "bend forwards (true) or back (false)" },
        Param { name: "duckLeft", kind: Kind::Bool, used: true, doc: "lean and twist to the left (false: right)" },
        Param { name: "crouch", kind: Kind::Float, used: true, doc: "how much the balancer bends the legs while ducking (0.05; 0: no crouch)" },
        Param { name: "stiffness", kind: Kind::Float, used: true, doc: "spine and neck stiffness while ducking (10)" },
    ] },
    Message { name: "shotSnap", params: &[
        Param { name: "snapMag", kind: Kind::Float, used: true, doc: "the kick's size; negative kicks the other way (0.5)" },
        Param { name: "snapDelay", kind: Kind::Float, used: true, doc: "s before the kick (0)" },
        Param { name: "snapDuration", kind: Kind::Float, used: true, doc: "s the kick is spread over; 0: all at once (0)" },
        Param { name: "unSnapInterval", kind: Kind::Float, used: true, doc: "s after the kick before the counter-kick (0.05)" },
        Param { name: "unSnapRatio", kind: Kind::Float, used: true, doc: "the counter-kick's size against the kick's, back the other way; 0: none (0.5)" },
        Param { name: "snapDirectionRandomness", kind: Kind::Float, used: true, doc: "0: about the axis across the bullet, 1: a random axis (0.25)" },
        Param { name: "snapMovingMult", kind: Kind::Float, used: true, doc: "kick times this when moving on his feet (1)" },
        Param { name: "snapBalancingMult", kind: Kind::Float, used: true, doc: "when standing still on his feet (1)" },
        Param { name: "snapAirborneMult", kind: Kind::Float, used: true, doc: "when nothing touches the ground (1)" },
        Param { name: "snapMovingThresh", kind: Kind::Float, used: true, doc: "m/s above which he is moving (1)" },
        Param { name: "snapUseBulletDir", kind: Kind::Bool, used: true, doc: "kick about the axis across the bullet (true); false: about his own side axis" },
        Param { name: "bulletDir", kind: Kind::Vec3, used: true, doc: "the bullet's direction (zero: coming from in front)" },
        Param { name: "snapHitPart", kind: Kind::Bool, used: true, doc: "also kick the joints either side of the part hit (true)" },
        Param { name: "bodyPart", kind: Kind::Int, used: true, doc: "the part hit, 0..20 (-1: none)" },
        Param { name: "snapHipType", kind: Kind::Int, used: true, doc: "legs: 0 no kick, 1 about the kick's axis, 2 about the bullet's direction (0)" },
        Param { name: "snapLeftLeg", kind: Kind::Bool, used: true, doc: "(false)" },
        Param { name: "snapRightLeg", kind: Kind::Bool, used: true, doc: "(false)" },
        Param { name: "snapLeftArm", kind: Kind::Bool, used: true, doc: "(false)" },
        Param { name: "snapRightArm", kind: Kind::Bool, used: true, doc: "(false)" },
        Param { name: "snapSpine", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "snapNeck", kind: Kind::Bool, used: true, doc: "(true)" },
        Param { name: "snapPhasedLegs", kind: Kind::Bool, used: true, doc: "the right leg kicks the other way (true)" },
    ] },
];

pub fn build(name: &str, p: &mut P) -> Option<Result<Box<dyn Extra>, Error>> {
    match name {
        "duck" => Some(Duck::new(p).map(|e| Box::new(e) as _)),
        "shotSnap" => Some(Snap::new(p).map(|e| Box::new(e) as _)),
        "shotSpin" => Some(Spin::new(p).map(|e| Box::new(e) as _)),
        "shotFallToKnees" => Some(Knees::new(p).map(|e| Box::new(e) as _)),
        _ => None,
    }
}

/// One rate-limited channel: a joint axis easing from where it was towards its duck target and back.
struct Channel { joint: usize, axis: usize, rate: f32, start: f32, goal: f32, x: f32 }

/// RDR2's DUCK: a short, fast flinch away from danger. After timeBeforeDuck the spine and neck bend into a random
/// share of what's left of each joint's range (forwards or back, and optionally sideways and twisting away), each at
/// its own rate, for duckTime; the legs crouch a little through the balancer; then it all fades out over blendTime.
pub struct Duck {
    mag: [f32; 2], rates: [f32; 5], wait: f32, time: f32, blend: f32, twist: bool, forward: bool, left: bool,
    crouch: f32, stiffness: f32, t: f32, channels: Vec<Channel>,
}

impl Duck {
    fn new(p: &mut P) -> Result<Duck, Error> {
        Ok(Duck {
            mag: [p.f("magMin", 0.25)?.clamp(0.0, 1.0), p.f("magMax", 1.0)?.clamp(0.0, 1.0)],
            rates: [p.f("lsRate", 0.5)?, p.f("usRate", 0.5)?, p.f("headRate", 0.5)?, p.f("spineTwistRate", 0.5)?, p.f("neckTwistRate", 0.5)?],
            wait: p.f("timeBeforeDuck", 0.0)?.max(0.0), time: p.f("duckTime", 0.5)?.max(0.0), blend: p.f("blendTime", 0.5)?.max(0.01),
            twist: p.b("leanTwistStart", true)?, forward: p.b("bendForward", true)?, left: p.b("duckLeft", false)?,
            crouch: p.f("crouch", 0.05)?.max(0.0), stiffness: p.f("stiffness", 10.0)?,
            t: 0.0, channels: vec![],
        })
    }

    /// Picks each channel's goal from the joints' current angles and limits.
    fn aim(&mut self, c: &mut Ctx) {
        let mag = self.mag[0] + (self.mag[1] - self.mag[0]) * c.ch.random();
        let (s, n) = (*SPINE(), *NECK());
        // (joint, axis, rate, direction): axis 0 lean forwards/back, 1 lean sideways, 2 twist
        // a spine joint's lean1 goes back as it rises, so forwards is towards its lower limit
        let bend = if self.forward { -1.0 } else { 1.0 };
        let side = if self.left { -1.0 } else { 1.0 };
        let mut want: Vec<(usize, usize, f32, f32)> = vec![
            (s[0], 0, self.rates[0], bend), (s[1], 0, self.rates[0], bend),
            (s[2], 0, self.rates[1], bend), (s[3], 0, self.rates[1], bend),
            (n[0], 0, self.rates[2], bend), (n[1], 0, self.rates[2], bend),
        ];
        if self.twist {
            for &j in &s {
                want.extend([(j, 1, self.rates[3], side), (j, 2, self.rates[3], side)]);
            }
            for &j in &n {
                want.extend([(j, 1, self.rates[4], side), (j, 2, self.rates[4], side)]);
            }
        }
        self.channels = want.into_iter().map(|(joint, axis, rate, dir)| {
            let now = c.ch.actual(joint, c.b.frames)[axis];
            let l = c.ch.limits(joint);
            let room = if dir > 0.0 { (l[2 * axis + 1] - now).max(0.0) } else { (l[2 * axis] - now).min(0.0) };
            Channel { joint, axis, rate, start: now, goal: room * mag, x: 0.0 }
        }).collect();
    }
}

impl Extra for Duck {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let t = self.t - self.wait;
        if t < 0.0 {
            return;
        }
        if self.channels.is_empty() {
            self.aim(c);
        }
        let bending = t <= self.time;
        // weight: full while bending, fading to nothing over blendTime after
        let w = if bending { 1.0 } else { (1.0 - (t - self.time) / self.blend).clamp(0.0, 1.0) };
        for ch in &mut self.channels {
            let target = if bending { ch.goal } else { 0.0 };
            let step = ch.rate * dt;
            ch.x += (target - ch.x).clamp(-step, step);
            let e = &mut c.ch.eff[ch.joint];
            if ch.axis == 0 && w > 0.0 {
                e.set_stiffness(self.stiffness, 1.0, None);
            }
            let d = &mut e.desired[ch.axis];
            *d += (ch.start + ch.x - *d) * w;
        }
        if self.crouch > 0.0 {
            c.out.dbm_leg_offset = Some(-self.crouch * w);
        }
    }

    fn done(&self) -> bool {
        self.t > self.wait + self.time + self.blend
    }

    fn stop(&mut self, c: &mut Ctx) {
        if self.crouch > 0.0 {
            c.out.dbm_leg_offset = Some(0.0);
        }
    }
}

/// Each Delirium part's parent part (joint j joins part j + 1 to this).
const PARENT: [usize; 21] = [0, 0, 1, 2, 0, 4, 5, 0, 7, 8, 9, 10, 11, 10, 13, 14, 15, 10, 17, 18, 19];
/// Delirium's own scale: a kick of size 1 turns a part by about this much angular velocity per kg of the part.
const KICK: f32 = 0.3;

/// RDR2's shot snap: a sharp kick across chosen joints at the hit, then a smaller counter-kick back. The kick turns
/// each joint about one axis (across the bullet's direction, or his side axis), sized by his state (moving,
/// standing, airborne) and by joint (spine hardest, neck lightest), spread over snapDuration (0: a single step).
pub struct Snap {
    mag: f32, delay: f32, duration: f32, interval: f32, ratio: f32, random: f32, mult: [f32; 3], moving: f32,
    use_dir: bool, dir: Vec3, part: i32, hip: i32, limbs: [bool; 6], phased: bool,
    t: f32, axis: Option<Vec3>, kicked: f32, countered: f32,
}

impl Snap {
    fn new(p: &mut P) -> Result<Snap, Error> {
        let mag = p.f("snapMag", 0.5)?;
        let (delay, duration) = (p.f("snapDelay", 0.0)?.max(0.0), p.f("snapDuration", 0.0)?.max(0.0));
        let (interval, ratio) = (p.f("unSnapInterval", 0.05)?.max(0.0), p.f("unSnapRatio", 0.5)?);
        let random = p.f("snapDirectionRandomness", 0.25)?.clamp(0.0, 1.0);
        let mult = [p.f("snapMovingMult", 1.0)?, p.f("snapBalancingMult", 1.0)?, p.f("snapAirborneMult", 1.0)?];
        let moving = p.f("snapMovingThresh", 1.0)?;
        let (use_dir, dir) = (p.b("snapUseBulletDir", true)?, p.v("bulletDir", Vec3::ZERO)?);
        let hit = p.b("snapHitPart", true)?;
        let part = p.i("bodyPart", -1)?;
        let hip = p.i("snapHipType", 0)?;
        let limbs = [p.b("snapLeftLeg", false)?, p.b("snapRightLeg", false)?, p.b("snapLeftArm", false)?,
                     p.b("snapRightArm", false)?, p.b("snapSpine", true)?, p.b("snapNeck", true)?];
        Ok(Snap { mag, delay, duration, interval, ratio, random, mult, moving, use_dir, dir, part: if hit { part } else { -1 },
                  hip, limbs, phased: p.b("snapPhasedLegs", true)?, t: 0.0, axis: None, kicked: 0.0, countered: 0.0 })
    }

    /// The share of a kick (spread over the duration) that falls in this step; all of it when the duration is 0.
    fn share(&self, since: f32, dt: f32, done: f32) -> f32 {
        if self.duration <= 0.0 {
            return 1.0 - done;
        }
        (((since + dt).min(self.duration) - since.max(0.0)) / self.duration).clamp(0.0, 1.0 - done)
    }

    /// Kicks joint `j` (the one driving part j + 1) about `axis` by `amount`: equal and opposite on its two parts.
    fn kick(c: &mut Ctx, j: usize, axis: Vec3, amount: f32, dt: f32) {
        let (child, parent) = (j + 1, PARENT[j + 1]);
        let t = axis * (amount * KICK * c.b.doll.part_mass(child) / dt);
        c.out.torque(child, t);
        c.out.torque(parent, -t);
    }

    fn apply(&mut self, c: &mut Ctx, m: f32, dt: f32) {
        let up = c.ch.up.normalize_or(Vec3::Z);
        let side = (c.b.part(13).m.d - c.b.part(17).m.d).normalize_or(Vec3::X);
        // left shoulder minus right, crossed this way round, points the way he faces
        let forward = up.cross(side).normalize_or(Vec3::Y);
        let dir = if self.dir == Vec3::ZERO { -forward } else { self.dir.normalize_or(-forward) };
        let (use_dir, random) = (self.use_dir, self.random);
        let r = Vec3::new(c.ch.random(), c.ch.random(), c.ch.random()) * 2.0 - 1.0;
        let axis = *self.axis.get_or_insert_with(|| {
            let a = if use_dir { up.cross(dir).normalize_or(side) } else { side };
            a.lerp(r.normalize_or(a), random).normalize_or(a)
        });
        // his state: nothing touching (airborne), only the feet (moving or standing), more than the feet (as it is)
        let feet = c.b.doll.collided(&[3, 6]);
        let other = c.b.doll.collided(&[0, 1, 2, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]);
        let horizontal = (c.vel - up * c.vel.dot(up)).length();
        let m = if !feet && !other {
            m * self.mult[2]
        } else if !other {
            m * self.mult[if horizontal > self.moving { 0 } else { 1 }]
        } else {
            m
        };
        let (s, n) = (*SPINE(), *NECK());
        if (0..21).contains(&self.part) {
            let k = self.part as usize;
            for j in [k.checked_sub(1), (k < 20).then_some(k)].into_iter().flatten() {
                Self::kick(c, j, axis, m * 15.0, dt);
            }
        }
        if self.limbs[4] {
            for (i, &j) in s.iter().enumerate() {
                Self::kick(c, j, axis, m * if i == 3 { 8.0 } else { 15.0 }, dt);
            }
        }
        if self.limbs[5] {
            for &j in &n {
                Self::kick(c, j, axis, m, dt);
            }
        }
        let mix = (up + dir).normalize_or(up);
        for (on, sign, clavicle, shoulder) in [(self.limbs[2], 1.0, 12, 13), (self.limbs[3], -1.0, 16, 17)] {
            if on {
                Self::kick(c, shoulder, mix, m * 4.0 * sign, dt);
                Self::kick(c, clavicle, up, m * 4.0 * sign, dt);
            }
        }
        if self.hip > 0 {
            let leg_axis = if self.hip >= 2 { dir } else { axis };
            for (on, hip, sign) in [(self.limbs[0], 0, 1.0), (self.limbs[1], 3, if self.phased { -1.0 } else { 1.0 })] {
                if on {
                    Self::kick(c, hip, leg_axis, m * 15.0 * sign, dt);
                }
            }
        }
    }
}

impl Extra for Snap {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        let since = self.t - self.delay;
        self.t += dt;
        if since + dt <= 0.0 {
            return;
        }
        let k = self.share(since, dt, self.kicked);
        if k > 0.0 {
            self.kicked += k;
            self.apply(c, self.mag * k, dt);
        }
        let back = since - self.duration - self.interval;
        if self.ratio != 0.0 && back + dt > 0.0 {
            let k = self.share(back, dt, self.countered);
            if k > 0.0 {
                self.countered += k;
                self.apply(c, -self.ratio * self.mag * k, dt);
            }
        }
    }

    fn done(&self) -> bool {
        self.kicked >= 0.999 && (self.ratio == 0.0 || self.countered >= 0.999)
    }
}

/// RDR2's shot spin and step turn. A hit off the body's centre spins him round: the balancer is turned towards a
/// heading that swings round by the hit's spin (its lever arm about his centre, across the bullet, times
/// shotSpinTwistMult, at most shotSpinTwist), and a damped yaw torque on the pelvis and trunk pulls him onto it and
/// holds him there while it settles; it stops early when he's already turning faster than shotSpinTwistVel. Then, with stepTurnType 1, after stepTurnWait he steps
/// round to face attackerPos.
pub struct Spin {
    spin: Option<f32>, hit: Vec3, dir: Vec3, min: f32, time: f32, settle: f32, mult: f32, max_twist: f32,
    max_vel: f32, torque: f32, torque_max: f32, step_type: i32, step_wait: f32, step_time: f32, attacker: Vec3,
    t: f32, heading: Option<(Vec3, f32)>, turned: f32,
}

impl Spin {
    fn new(p: &mut P) -> Result<Spin, Error> {
        let spin = p.f("spin", f32::NAN)?;
        Ok(Spin {
            spin: (!spin.is_nan()).then_some(spin), hit: p.v("hitPoint", Vec3::ZERO)?, dir: p.v("bulletDir", Vec3::ZERO)?,
            min: p.f("shotSpinMin", 0.01)?.max(0.0), time: p.f("shotSpinTime", 0.4)?.max(0.0), settle: p.f("shotSpinSettle", 0.8)?.max(0.0),
            mult: p.f("shotSpinTwistMult", 10.0)?, max_twist: p.f("shotSpinTwist", 1.5)?.abs(), max_vel: p.f("shotSpinTwistVel", 6.0)?.abs(),
            torque: p.f("shotSpinTorque", 150.0)?, torque_max: p.f("shotSpinTorqueMax", 300.0)?.abs(),
            step_type: p.i("stepTurnType", 0)?, step_wait: p.f("stepTurnWait", 0.3)?.max(0.0), step_time: p.f("stepTurnTime", 2.0)?.max(0.0),
            attacker: p.v("attackerPos", Vec3::ZERO)?, t: 0.0, heading: None, turned: 0.0,
        })
    }

    fn facing(c: &Ctx, up: Vec3) -> Vec3 {
        let side = c.b.part(13).m.d - c.b.part(17).m.d;
        let f = side.cross(up);
        (f - up * f.dot(up)).normalize_or(Vec3::Y)
    }
}

impl Extra for Spin {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let yaw_rate = c.angvel.dot(up);
        self.turned += yaw_rate * dt;
        let (spin, hit, dir, mult, max_twist) = (self.spin, self.hit, self.dir, self.mult, self.max_twist);
        let (start, total) = *self.heading.get_or_insert_with(|| {
            // the spin: the hit's lever arm about his centre, across the bullet, about up
            let s = spin.unwrap_or_else(|| (hit - c.com).cross(dir.normalize_or_zero()).dot(up));
            (Self::facing(c, up), (s * mult).clamp(-max_twist, max_twist))
        });
        let spinning = self.t <= self.time + self.settle;
        if spinning && total.abs() > self.min * mult && self.turned.abs() <= max_twist + 0.5 && yaw_rate.abs() < self.max_vel {
            // the heading swings round to start + total over shotSpinTime, then holds while it settles: a yaw torque
            // on pelvis and trunk towards it (shotSpinTorque per rad off), damped by his turn rate so the planted
            // feet don't spring him back, and the balancer told to face it
            let a = total * (self.t / self.time.max(dt)).min(1.0);
            let heading = bevy_math::Quat::from_axis_angle(up, a) * start;
            let now = Self::facing(c, up);
            let off = now.cross(heading).dot(up).atan2(now.dot(heading));
            let tq = (self.torque * off - self.torque * 0.3 * yaw_rate).clamp(-self.torque_max, self.torque_max);
            for k in [0, 7, 8] {
                c.out.torque(k, up * tq / 3.0);
            }
            c.out.dbm_turn = Some(heading * 0.5);
            return;
        }
        let step_from = self.time + self.settle + self.step_wait;
        if self.step_type > 0 && self.t > step_from && self.t <= step_from + self.step_time {
            let to = self.attacker - c.com;
            let to = (to - up * to.dot(up)).normalize_or_zero();
            if to != Vec3::ZERO && Self::facing(c, up).dot(to) < 0.98 {
                c.out.dbm_turn = Some(to * 0.5);
                return;
            }
        }
        c.out.dbm_turn = Some(Vec3::ZERO);
    }

    fn done(&self) -> bool {
        let end = self.time + self.settle + if self.step_type > 0 { self.step_wait + self.step_time } else { 0.0 };
        self.t > end
    }

    fn stop(&mut self, c: &mut Ctx) {
        c.out.dbm_turn = Some(Vec3::ZERO);
    }
}

/// RDR2's fall to knees (a shot module there): after balancing for ftkBalanceTime the balancer lets go and he sinks:
/// knees bend at ftkBendRate, hips flex, the spine bends forward by ftkSpineBend and a helper push on the pelvis
/// tips him forwards over his feet. When both shins touch the ground he is kneeling: thighs come upright under him and
/// a lean help keeps his centre over his knees for ftkKneelTime; then he topples forwards (or back) and goes limp.
pub struct Knees {
    balance: f32, bend_rate: f32, helper: f32, spine_bend: f32, stiffness: f32, kneel_time: f32, lean_help: f32,
    forwards: bool, pitch: f32, arms_down: bool, kneel_hip: f32,
    t: f32, phase: u8, since: f32, knee: [f32; 2], hip: [f32; 2],
}

impl Knees {
    fn new(p: &mut P) -> Result<Knees, Error> {
        Ok(Knees {
            balance: p.f("ftkBalanceTime", 0.1)?.max(0.0), bend_rate: p.f("ftkBendRate", 3.0)?.abs(),
            helper: p.f("ftkHelperForce", 0.0)?, spine_bend: p.f("ftkSpineBend", 0.1)?, stiffness: p.f("ftkStiffness", 9.0)?,
            kneel_time: p.f("ftkKneelTime", 2.0)?.max(0.0), lean_help: p.f("ftkLeanHelp", 1500.0)?,
            forwards: p.b("ftkPitchForwards", true)?, pitch: p.f("ftkPitchForce", 300.0)?, arms_down: p.i("ftkOnKneesArmType", 1)? == 1, kneel_hip: p.f("ftkKneelHip", 0.3)?,
            t: 0.0, phase: 0, since: 0.0, knee: [0.0; 2], hip: [0.0; 2],
        })
    }

    fn forward(c: &Ctx, up: Vec3) -> Vec3 {
        let f = up.cross(c.b.part(13).m.d - c.b.part(17).m.d);
        (f - up * f.dot(up)).normalize_or(Vec3::Y)
    }

    /// Moves joint j's axis-k target towards `to` by at most `step`, from where it is now the first time.
    fn ease(c: &mut Ctx, j: usize, k: usize, to: f32, step: f32) {
        let now = c.ch.eff[j].desired[k];
        c.ch.eff[j].desired[k] = now + (to - now).clamp(-step, step);
    }
}

impl Extra for Knees {
    fn tick(&mut self, c: &mut Ctx, dt: f32) {
        self.t += dt;
        self.since += dt;
        let up = c.ch.up.normalize_or(Vec3::Z);
        let fwd = Self::forward(c, up);
        let (knees, hips, spine) = (*KNEE(), *HIP(), *SPINE());
        match self.phase {
            0 => {
                if self.t >= self.balance {
                    // let go of the balancer; start from the legs' current angles
                    c.out.dbm = Some(false);
                    for s in 0..2 {
                        self.knee[s] = c.ch.actual(knees[s], c.b.frames)[0];
                        self.hip[s] = c.ch.actual(hips[s], c.b.frames)[0];
                        c.ch.eff[knees[s]].desired[0] = self.knee[s];
                        c.ch.eff[hips[s]].desired[0] = self.hip[s];
                    }
                    (self.phase, self.since) = (1, 0.0);
                }
            }
            1 => {
                // sinking: knees towards their deepest bend, hips flexing, spine forwards, a push over the feet
                let step = self.bend_rate * dt;
                c.ch.set_stiffness(self.stiffness, 1.0, "lb", None);
                c.ch.set_stiffness(self.stiffness, 1.0, "us", None);
                for s in 0..2 {
                    let kl = c.ch.limits(knees[s]);
                    let deep = if kl[0].abs() > kl[1].abs() { kl[0] } else { kl[1] } * 0.9;
                    Self::ease(c, knees[s], 0, deep, step);
                    let hl = c.ch.limits(hips[s]);
                    Self::ease(c, hips[s], 0, (hl[1] * 0.2).min(0.4), step);
                }
                for &j in &spine {
                    Self::ease(c, j, 0, -self.spine_bend / 4.0, step);
                }
                c.out.force(0, fwd * self.helper);
                // on his knees once they are near the ground (measured from his feet; shin contacts are too patchy)
                let ground = c.b.part(3).m.d.dot(up).min(c.b.part(6).m.d.dot(up)) - 0.05;
                let knee = |t: usize, s: usize| (c.b.part(t).m.d + c.b.part(s).m.d).dot(up) * 0.5 - ground;
                let down = knee(1, 2) < 0.2 && knee(4, 5) < 0.2;
                // as the knees come down, start holding his centre over them so he doesn't pitch on
                if knee(1, 2).max(knee(4, 5)) < 0.35 {
                    let knees_at = (c.b.part(1).m.d + c.b.part(2).m.d + c.b.part(4).m.d + c.b.part(5).m.d) * 0.25;
                    let off = knees_at - c.com;
                    let vel = c.vel - up * c.vel.dot(up);
                    let pull = (off - up * off.dot(up)) * self.lean_help - vel * self.lean_help * 0.2;
                    c.out.force(0, pull * 0.5);
                    c.out.force(10, pull * 0.5);
                }
                if down || self.since > 1.5 {
                    (self.phase, self.since) = (2, 0.0);
                }
            }
            2 => {
                // kneeling: thighs upright under him, spine straight, his centre held over the knees
                let step = self.bend_rate * dt;
                c.ch.set_stiffness(self.stiffness, 1.0, "lb", None);
                c.ch.set_stiffness(self.stiffness, 1.0, "us", None);
                // hips held a little flexed and stiffer, so the trunk stays up over the knees
                for s in 0..2 {
                    Self::ease(c, hips[s], 0, self.kneel_hip, step);
                    c.ch.eff[hips[s]].set_stiffness(self.stiffness * 2.5, 1.0, None);
                }
                for &j in &spine {
                    Self::ease(c, j, 0, 0.0, step);
                    c.ch.eff[j].set_stiffness(self.stiffness * 1.5, 1.0, None);
                }
                if self.arms_down {
                    c.ch.set_stiffness(self.stiffness * 0.5, 1.0, "ua", None);
                }
                // the knees: between each thigh and shin; hold his centre over them
                let knees_at = (c.b.part(1).m.d + c.b.part(2).m.d + c.b.part(4).m.d + c.b.part(5).m.d) * 0.25;
                let off = knees_at - c.com;
                let vel = c.vel - up * c.vel.dot(up);
                let pull = (off - up * off.dot(up)) * self.lean_help - vel * self.lean_help * 0.2;
                c.out.force(0, pull * 0.5);
                c.out.force(10, pull * 0.5);
                if self.since > self.kneel_time {
                    (self.phase, self.since) = (3, 0.0);
                }
            }
            _ => {
                // topple: go loose and tip forwards (or back)
                c.ch.set_stiffness(self.stiffness * 0.3, 1.0, "fb", None);
                if self.since < 0.3 {
                    c.out.force(10, if self.forwards { fwd } else { -fwd } * self.pitch);
                }
            }
        }
    }

    fn done(&self) -> bool {
        self.phase == 3 && self.since > 1.0
    }
}
