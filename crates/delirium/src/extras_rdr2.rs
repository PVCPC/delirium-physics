//! Behaviours from Red Dead Redemption 2's Euphoria, written from descriptions of how they behave (see the RDR2
//! Euphoria notes); none of RDR2's code or numbers. They are extra messages: nothing here runs unless one is sent with
//! start=true, so every existing behaviour is unchanged. Defaults are Delirium's own round values; a player's own
//! RDR2 values can be sent as parameters.
use crate::extras::{Extra, P};
use crate::msg::{Error, Kind, Message, Param};
use crate::nmrs::{NECK, SPINE};
use crate::nmshot::Ctx;

pub static MESSAGES: [Message; 1] = [
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
];

pub fn build(name: &str, p: &mut P) -> Option<Result<Box<dyn Extra>, Error>> {
    match name {
        "duck" => Some(Duck::new(p).map(|e| Box::new(e) as _)),
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
