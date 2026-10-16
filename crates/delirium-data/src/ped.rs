use crate::{melee::WEAPON_TYPES, rng::Rand, weapons::Weapon};

pub const START: f32 = 200.0;
pub const INJURED_BELOW: f32 = 100.0;
pub const ARMOUR_CAP: f32 = 100.0;

pub const KNOCKED_OUT: u8 = 0x4;
pub const INSTANT: u8 = 0x8;
pub const INJURED: u8 = 0x10;
pub const KILLED: u8 = 0x20;

pub mod flags {
    pub const NOT_BY_GROUP: u32 = 0x0002_0000;
    pub const ONLY_BY_GROUP: u32 = 0x0004_0000;
    pub const EXPLOSION_PROOF: u32 = 0x0008_0000;
    pub const ONLY_BY_PLAYER: u32 = 0x0010_0000;
    pub const INVINCIBLE: u32 = 0x0020_0000;
    pub const MELEE_PROOF: u32 = 0x0040_0000;
    pub const COLLISION_PROOF: u32 = 0x0080_0000;
    pub const FIRE_PROOF: u32 = 0x0100_0000;
    pub const BULLET_PROOF: u32 = 0x0200_0000;
}

pub fn set_proofs(bits: u32, [bullet, fire, explosion, collision, melee]: [bool; 5]) -> u32 {
    use flags::*;
    [(BULLET_PROOF, bullet), (FIRE_PROOF, fire), (EXPLOSION_PROOF, explosion), (COLLISION_PROOF, collision), (MELEE_PROOF, melee)]
        .iter().fold(bits, |b, &(bit, on)| if on { b | bit } else { b & !bit })
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Blades {
    pub sum: f32,
    pub window: f32,
}

impl Blades {
    pub fn may_hurt(&self) -> bool {
        !(self.window > 0.0)
    }

    pub fn hit(&mut self, step: f32) {
        self.sum = step.mul_add(1.1, self.sum);
        if self.sum > 20.0 {
            (self.window, self.sum) = (10.0, 0.0);
        }
    }

    pub fn tick(&mut self, step: f32) {
        if self.window > 0.0 {
            self.window -= step;
        }
        if self.sum > 0.0 {
            self.sum = (-step).mul_add(0.1, self.sum);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub health: f32,
    pub max: f32,
    pub armour: f32,
}

impl Default for Health {
    fn default() -> Self {
        Health { health: START, max: START, armour: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Attacker {
    None,
    Ped,
    Player { sure: bool },
    Vehicle { population: bool },
}

pub struct Hit<'a> {
    pub amount: f32,
    pub weapon: &'a Weapon,
    pub bone: u16,
    pub attacker: Attacker,
    pub friend: bool,
    pub floor: bool,
    pub group: Option<u32>,
    pub same_group: bool,
    pub number: Option<u32>,
}

impl<'a> Hit<'a> {
    pub fn new(amount: f32, weapon: &'a Weapon, bone: u16, attacker: Attacker) -> Self {
        Hit { amount, weapon, bone, attacker, friend: false, floor: false, group: None, same_group: false, number: None }
    }
}

pub const NO_CUT: f32 = 5555.55;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Victim {
    pub player: bool,
    pub factor: f32,
    pub in_vehicle: bool,
    pub population: bool,
    pub hat: bool,
    pub guard: bool,
    pub swat: bool,
    pub flags: u32,
    pub group: u16,
    pub vest: bool,
    pub ragdolling: bool,
    pub shootable_in_vehicle: bool,
    pub hurtable: bool,
    pub fire_proof: bool,
    pub mission: bool,
    pub dies_when_injured: bool,
    pub deep_water: bool,
    pub no_critical: bool,
    pub drowning_ok: bool,
    pub weapon58_ok: bool,
    pub blades_ok: bool,
}

impl Default for Victim {
    fn default() -> Self {
        Victim { player: false, factor: 1.0, in_vehicle: false, population: true, hat: false, guard: false, swat: false, flags: 0, group: 0,
                 vest: false, ragdolling: false, shootable_in_vehicle: true, hurtable: true, fire_proof: false, mission: false,
                 dies_when_injured: false, deep_water: false, no_critical: false, drowning_ok: true, weapon58_ok: true, blades_ok: true }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    pub flags: u8,
    pub lost: f32,
    pub absorbed: f32,
}

impl Outcome {
    pub fn dies(&self) -> bool {
        self.flags & (INJURED | KILLED) != 0
    }
}

fn damage_type(w: &Weapon) -> u8 {
    ["NONE", "MELEE", "BULLET", "EXPLOSIVE", "FIRE", "COLLISION", "FALL", "DROWN"].iter().position(|&n| n == w.damage_type).unwrap_or(0) as u8
}

fn bone_share(bone: u16) -> f32 {
    match bone {
        0x4C1 | 0x4C2 | 0x4C8 | 0x4C9 | 0x1A2 | 0x1A3 | 0x1A7 | 0x1A8 => 0.75,
        0x4C3 | 0x4D0 | 0x1A4 | 0x1A9 => 0.5,
        _ => 1.0,
    }
}

const HEAD: u16 = 0x4B5;
const NECK: u16 = 0x4B4;
const SPINE: [u16; 4] = [0x4B2, 0x4B3, 0x36A0, 0x36A1];

pub fn scale(hit: &Hit, victim: &Victim) -> f32 {
    let kind = damage_type(hit.weapon);
    let not_area = !(3..=5).contains(&kind);
    let mut a = hit.amount;
    if victim.player {
        if !not_area {
            a *= 0.66;
        } else if kind != 1 {
            a *= 0.33;
        }
    } else {
        a *= victim.factor;
    }
    let share = bone_share(hit.bone);
    if share != 1.0 {
        a *= share;
    }
    if victim.in_vehicle && victim.population {
        a *= 1.5;
    }
    let cut = not_area && match hit.attacker {
        Attacker::Player { .. } => hit.same_group && hit.amount != NO_CUT,
        Attacker::Ped => (victim.player && hit.friend) || (victim.mission && hit.same_group),
        Attacker::None | Attacker::Vehicle { .. } => false,
    };
    if cut {
        let third = a * 0.33;
        a = if 12.5 - third >= 0.0 { third } else { 12.5 };
    }
    a
}

pub fn damage(h: &mut Health, hit: &Hit, victim: &Victim, rand: &mut Rand) -> Outcome {
    let mut out = Outcome::default();
    if h.health >= INJURED_BELOW && !allowed(hit, victim) {
        return out;
    }
    let kind = damage_type(hit.weapon);
    let mut a = scale(hit, victim);
    if h.armour > 0.0 && !hit.weapon.has("ARMOUR_PENETRATING") {
        if a > h.armour {
            (out.absorbed, h.armour) = (h.armour, 0.0);
        } else {
            out.absorbed = a;
            h.armour -= a;
        }
    }
    if kind == 1 {
        let mut guard = true;
        if hit.bone == HEAD {
            if victim.hat {
                a *= 0.75;
            }
            if hit.weapon.has("MELEE_CLUB") {
                a *= 4.0;
            }
            if !victim.player && a > 6.0 && rand.float_in_range(0.0, 1.0) < 0.2 {
                if h.health - a > INJURED_BELOW {
                    a = h.health - INJURED_BELOW + 1.0;
                }
                out.flags |= KNOCKED_OUT;
                guard = false;
            }
        }
        if guard && victim.guard {
            a *= 0.5;
        }
    }
    if instant(hit, victim, rand) {
        out.flags |= INSTANT | KILLED;
        (out.lost, out.absorbed) = (h.health, h.armour);
        (h.health, h.armour) = (0.0, 0.0);
        return out;
    }
    let d = (a - out.absorbed).max(0.0);
    if hit.floor {
        let left = (h.health - d).max(105.0);
        out.lost = h.health - left;
        h.health = left;
    } else if h.health - d >= INJURED_BELOW {
        out.lost = d;
        h.health += -d;
    } else if h.health - d >= 1.0 && !(victim.in_vehicle || victim.player || victim.dies_when_injured || victim.deep_water) {
        out.lost = d;
        out.flags |= INJURED;
        h.health += -d;
    } else {
        out.flags |= KILLED;
        out.lost = h.health;
        h.health = 0.0;
    }
    out
}

fn weapon_id(w: &Weapon) -> u32 {
    WEAPON_TYPES.iter().position(|n| n.eq_ignore_ascii_case(&w.name)).map_or(u32::MAX, |i| i as u32)
}

pub fn weapon_row(name: &str, rows: &[Weapon]) -> Weapon {
    let name = if name.is_empty() { "EXPLOSION" } else { name };
    rows.iter().find(|w| w.name.eq_ignore_ascii_case(name)).cloned().unwrap_or_else(|| Weapon { name: name.to_string(), ..Default::default() })
}

pub fn blast_hurts(hurt: &mut Vec<u64>, id: u64, ped_ragdoll_state: Option<i32>) -> bool {
    if !hurt.contains(&id) && hurt.len() < 32 {
        hurt.push(id);
        return true;
    }
    ped_ragdoll_state == Some(5)
}

pub fn allowed(hit: &Hit, victim: &Victim) -> bool {
    let (weapon, kind) = (hit.number.unwrap_or_else(|| weapon_id(hit.weapon)), damage_type(hit.weapon));
    if (weapon == 53 && !victim.drowning_ok) || (weapon == 58 && !victim.weapon58_ok) {
        return false;
    }
    if victim.vest && hit.weapon.has("GUN") && SPINE.contains(&hit.bone) {
        return false;
    }
    if victim.player && (!victim.hurtable || (victim.fire_proof && kind == 4) || (weapon == 59 && !victim.blades_ok)) {
        return false;
    }
    if matches!(hit.attacker, Attacker::Ped) && hit.same_group && weapon != 51 {
        return false;
    }
    if kind == 2 && victim.in_vehicle && !victim.shootable_in_vehicle {
        return false;
    }
    may_damage(victim.flags, victim.group, weapon, kind, hit.attacker, hit.group) && !(weapon == 54 && victim.ragdolling)
}

fn may_damage(bits: u32, group: u16, weapon: u32, kind: u8, att: Attacker, att_group: Option<u32>) -> bool {
    use flags::*;
    if bits & INVINCIBLE != 0 {
        return false;
    }
    let exempt = kind == 3 || matches!(weapon, 51 | 53 | 58);
    let who = match att {
        Attacker::Ped | Attacker::Player { .. } => Some(att_group.unwrap_or(0)),
        Attacker::Vehicle { .. } => att_group,
        Attacker::None => None,
    };
    if bits & ONLY_BY_PLAYER != 0 && !exempt && !matches!(att, Attacker::Player { .. } | Attacker::Vehicle { population: false }) {
        return false;
    }
    if bits & ONLY_BY_GROUP != 0 && !exempt && !who.is_some_and(|g| g & 0xFFFF == group as u32) {
        return false;
    }
    if bits & NOT_BY_GROUP != 0 && !exempt && who == Some(group as u32) {
        return false;
    }
    match kind {
        1 if bits & MELEE_PROOF != 0 => return false,
        2 if bits & BULLET_PROOF != 0 => return false,
        3 if bits & EXPLOSION_PROOF != 0 => return false,
        4 if bits & FIRE_PROOF != 0 => return false,
        _ => {}
    }
    !(matches!(weapon, 46 | 49 | 50 | 54) && bits & COLLISION_PROOF != 0)
}

pub fn car_hit(speed: f32, vz: f32) -> Option<(&'static str, f32)> {
    let speed = speed.min(1000.0);
    if speed > 12.0 {
        Some(("RUNOVERBYCAR", 30.0))
    } else if speed > 6.0 || (vz < -0.8 && speed > 3.0) {
        Some(("RAMMEDBYCAR", 30.0))
    } else {
        None
    }
}

pub fn car_hit_player(speed: f32, vz: f32, train: bool) -> Option<(&'static str, f32)> {
    let speed = speed.min(1000.0);
    (speed > 10.0 || (vz < -0.8 && speed > 3.0)).then_some(("RAMMEDBYCAR", if train { 150.0 } else { 30.0 }))
}

pub fn fall_damage(fell: f32, player: bool, health: f32) -> f32 {
    let limit = if player { 15.0 } else { 10.0 };
    if fell > limit { health * 100.0 } else { fell * 4.0 }
}

pub mod control {
    pub const SCRIPT: u32 = 0x20;
    pub const OTHER: u32 = 0x800;
}

pub fn police_armour(model: &str) -> f32 {
    if ["M_Y_SWAT", "M_M_FBI"].iter().any(|m| m.eq_ignore_ascii_case(model)) { ARMOUR_CAP } else { 0.0 }
}

fn instant(hit: &Hit, victim: &Victim, rand: &mut Rand) -> bool {
    if victim.player || victim.no_critical || damage_type(hit.weapon) != 2 {
        return false;
    }
    let sniper = hit.weapon.slot == "SNIPER";
    if (hit.bone == HEAD || hit.bone == NECK) && hit.weapon.slot != "SHOTGUN" {
        let kill = victim.in_vehicle
            || (victim.swat && rand.next() & 3 != 0)
            || sniper
            || matches!(hit.attacker, Attacker::Player { sure: true })
            || rand.next() & 15 == 0;
        if kill {
            return true;
        }
    }
    sniper && SPINE.contains(&hit.bone)
}

