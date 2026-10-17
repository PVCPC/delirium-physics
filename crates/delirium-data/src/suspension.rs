use crate::vehicles::Handling;

pub const GRAVITY: f32 = 9.8;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spring {
    pub front: bool,
    pub share: f32,
    pub travel: f32,
    pub rest: f32,
    pub limit: f32,
}

impl Spring {
    pub fn new(h: &Handling, front: bool, share: f32, radius: f32) -> Self {
        Self::new_at_height(h, front, share, radius, 0.0)
    }

    pub fn new_at_height(h: &Handling, front: bool, share: f32, radius: f32, model_z: f32) -> Self {
        let centre = model_z - h.suspension_raise;
        let top = h.suspension_upper + centre;
        let bottom = ((h.suspension_lower + centre) - h.suspension_raise) - radius;
        let dz = bottom - top;
        let travel = (dz * dz).sqrt();
        let rest = -h.suspension_lower * travel / (h.suspension_upper - h.suspension_lower + radius);
        let span = h.suspension_upper - h.suspension_lower;
        let limit = span * travel / (span + radius);
        Spring { front, share, travel, rest, limit }
    }

    pub fn top(&self, h: &Handling) -> f32 {
        h.suspension_upper - h.suspension_raise
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Load {
    pub force: f32,
    pub tyre: f32,
}

pub fn force(h: &Handling, s: &Spring, x: f32, rate: f32, mass: f32) -> Load {
    let x = x.min(s.travel);
    let k = if s.front { 1.0 } else { 0.75 } * h.suspension_force;
    let spring = k.mul_add(x - s.rest, s.share).clamp(0.0, 1.0) * mass * GRAVITY;
    let c = if rate > 0.0 { h.suspension_comp_damp } else { h.suspension_rebound_damp };
    let bias = h.suspension_bias[if s.front { 0 } else { 1 }];
    let damp = (c * mass * GRAVITY * rate * bias).max(-spring);
    Load { force: spring + damp, tyre: spring + damp.min(0.0) }
}

pub fn sag(s: &Spring, health: f32, x: f32, drop: f32) -> f32 {
    if health <= 0.0 {
        drop
    } else if health < 1000.0 {
        drop * ((-health).mul_add(0.001, 1.0) * (x / s.rest).min(1.0)).max(0.0)
    } else {
        0.0
    }
}

pub fn ground_flags(material: &str) -> u32 {
    match material {
        "SAND" => crate::tyre::flags::SAND,
        "MUD_SOFT" | "GRASS" | "SHORT_GRASS" => crate::tyre::flags::SOFT,
        _ => 0,
    }
}

pub fn sink(flags: u32) -> f32 {
    use crate::tyre::flags::{SAND, SOFT};
    if flags & (SAND | SOFT) == 0 {
        0.0
    } else if flags & SAND != 0 {
        0.05
    } else {
        0.03
    }
}

fn at_least_zero(v: f32) -> f32 {
    if -v >= 0.0 { 0.0 } else { v }
}

pub fn settled(s: &Spring, health: f32, flags: u32, capped: f32, x: f32, drop: f32) -> f32 {
    let mut v = capped;
    if health < 1000.0 {
        v = at_least_zero(v - sag(s, health, x, drop));
    }
    if sink(flags) != 0.0 { at_least_zero(v - sink(flags)) } else { v }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub capped: f32,
    pub grip: f32,
    pub wet: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn contact(s: &Spring, health: f32, flags: u32, x: f32, limit: f32, drop: f32, grip: f32, wet_grip: f32,
               wetness: f32) -> Contact {
    let mut give = if health < 1000.0 { sag(s, health, x, drop) } else { 0.0 };
    if sink(flags) != 0.0 {
        give = sink(flags) + give;
    }
    let cap = limit + give;
    let capped = if x - cap >= 0.0 { cap } else { x };
    let wet = (wet_grip + 1.0).mul_add(wetness, 1.0 - wetness);
    Contact { capped, grip, wet }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelSetup {
    pub bone: i16,
    pub radius: f32,
    pub rim: f32,
    pub width: f32,
    pub share: f32,
    pub inv_share: f32,
    pub sign: f32,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct WheelPart {
    pub bone: i16,
    pub radius: f32,
    pub half_width: f32,
    pub slot99: bool,
    pub compare: bool,
}

pub fn setup(ide_radius: f32, share: f32, flags: u32, part: Option<WheelPart>, first: Option<(f32, f32)>) -> WheelSetup {
    let mut flags = flags;
    let (bone, radius, rim, width) = match part {
        Some(p) if p.bone > -1 => {
            let width = p.half_width * 2.0;
            if p.slot99 {
                flags |= 0x40_0000;
                if flags & 0x1000 != 0 && p.compare && first.is_some_and(|(r, w)| width != w || p.radius != r) {
                    flags |= 0x80_0000;
                }
            }
            (p.bone, p.radius, ide_radius, width)
        }
        _ => (-1, ide_radius, ide_radius - 0.1, 0.25),
    };
    flags |= 2;
    let sign = if flags & 0x1000 != 0 { -1.0 } else { 1.0 };
    WheelSetup { bone, radius, rim, width, share, inv_share: 1.0 / share, sign, flags }
}

