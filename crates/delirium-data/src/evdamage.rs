use crate::estimate::vrsqrtefp;

pub const APPLIED: u8 = 0x80;
pub const LANDED: u8 = 0x40;
pub const KILLED: u8 = 0x20;
pub const DIE: u32 = 217;
pub const DEAD: u32 = 218;
pub const SEEN_BY: u32 = 1027;
pub const DAMAGE: u32 = 9;
pub const ON_FIRE: u32 = 58;
pub const IN_WATER: u32 = 66;
pub const SWITCH2NM: u32 = 130;
pub const HIT_ALERT: f32 = 100.0;

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub flags: u8,
    pub real: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Victim {
    pub dead: bool,
    pub root: u32,
    pub dying: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Attacker {
    pub ped: bool,
    pub root: u32,
    pub at: [f32; 3],
    pub friend: bool,
    pub victim_at: [f32; 3],
    pub victim_forward: [f32; 3],
}

pub fn concerns(hit: Hit, v: &Victim, a: Option<&Attacker>) -> (bool, bool) {
    if hit.flags & APPLIED == 0 || hit.flags & LANDED == 0 {
        return (false, false);
    }
    if v.dead && (v.root == DEAD || v.dying) {
        return (false, false);
    }
    let a = a.filter(|a| a.ped);
    if let Some(a) = a.filter(|a| !hit.real && a.root == SEEN_BY) {
        let d = [a.at[0] - a.victim_at[0], a.at[1] - a.victim_at[1], a.at[2] - a.victim_at[2]];
        let s = (0.0f64 + d[0] as f64 * d[0] as f64 + d[1] as f64 * d[1] as f64 + d[2] as f64 * d[2] as f64) as f32;
        let e = vrsqrtefp(s);
        let e = if e == f32::INFINITY { 0.0 } else { e };
        let r = (-s.mul_add(e * e, -1.0)).mul_add(0.5 * e, e);
        let u = d.map(|c| c * r);
        let f = a.victim_forward;
        if f[0].mul_add(u[0], f[1].mul_add(u[1], f[2] * u[2])) < 0.0 {
            return (false, false);
        }
    }
    (true, hit.real && a.is_some_and(|a| !a.friend))
}

#[derive(Clone, Copy, Debug)]
pub struct Running {
    pub kind: u32,
    pub same_source: bool,
}

pub fn overrides(hit: Hit, by_player: bool, nm_task: bool, r: Running, by_priority: bool) -> bool {
    let kills = hit.flags & KILLED != 0 && hit.real;
    match r.kind {
        IN_WATER | ON_FIRE if kills => true,
        DAMAGE if by_player || !r.same_source || kills => true,
        SWITCH2NM if nm_task => true,
        _ => by_priority,
    }
}

pub fn alert(level: i8, amount: f32, first: u32, second: u32) -> i8 {
    let (first, second) = (first & 0x18000, second & 0x18000);
    let factor = if first == 0 || second == 0x18000 {
        1.0
    } else if second == 0x8000 {
        0.75
    } else {
        0.0
    };
    let add = (factor * amount) as i32 as i8;
    (add as i32 + level as i32).clamp(0, 100) as i8
}
