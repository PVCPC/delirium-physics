use crate::{melee::WEAPON_TYPES, rng::Rand, weapons::Weapon};

#[allow(non_snake_case)]
#[inline]
pub fn START() -> f32 { crate::kv!(15) }
#[allow(non_snake_case)]
#[inline]
pub fn INJURED_BELOW() -> f32 { crate::kv!(28) }
#[allow(non_snake_case)]
#[inline]
pub fn ARMOUR_CAP() -> f32 { crate::kv!(28) }

#[allow(non_snake_case)]
#[inline]
pub fn KNOCKED_OUT() -> u8 { crate::kv!(1 int) }
#[allow(non_snake_case)]
#[inline]
pub fn INSTANT() -> u8 { crate::kv!(23 int) }
#[allow(non_snake_case)]
#[inline]
pub fn INJURED() -> u8 { crate::kv!(51 int) }
#[allow(non_snake_case)]
#[inline]
pub fn KILLED() -> u8 { crate::kv!(7 int) }

pub mod flags {
    #[allow(non_snake_case)]
    #[inline]
    pub fn NOT_BY_GROUP() -> u32 { crate::kv!(626 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn ONLY_BY_GROUP() -> u32 { crate::kv!(790 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn EXPLOSION_PROOF() -> u32 { crate::kv!(792 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn ONLY_BY_PLAYER() -> u32 { crate::kv!(797 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn INVINCIBLE() -> u32 { crate::kv!(798 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn MELEE_PROOF() -> u32 { crate::kv!(794 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn COLLISION_PROOF() -> u32 { crate::kv!(795 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn FIRE_PROOF() -> u32 { crate::kv!(788 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn BULLET_PROOF() -> u32 { crate::kv!(849 int) }
}

pub fn set_proofs(bits: u32, [bullet, fire, explosion, collision, melee]: [bool; 5]) -> u32 {
    use flags::*;
    [(BULLET_PROOF(), bullet), (FIRE_PROOF(), fire), (EXPLOSION_PROOF(), explosion), (COLLISION_PROOF(), collision), (MELEE_PROOF(), melee)]
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
        self.sum = step.mul_add(crate::kv!(132), self.sum);
        if self.sum > crate::kv!(136) {
            (self.window, self.sum) = (crate::kv!(27), 0.0);
        }
    }

    pub fn tick(&mut self, step: f32) {
        if self.window > 0.0 {
            self.window -= step;
        }
        if self.sum > 0.0 {
            self.sum = (-step).mul_add(crate::kv!(73), self.sum);
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
        Health { health: START(), max: START(), armour: 0.0 }
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

#[allow(non_snake_case)]
#[inline]
pub fn NO_CUT() -> f32 { crate::kv!(850) }

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
        self.flags & (INJURED() | KILLED()) != 0
    }
}

fn damage_type(w: &Weapon) -> u8 {
    ["NONE", "MELEE", "BULLET", "EXPLOSIVE", "FIRE", "COLLISION", "FALL", "DROWN"].iter().position(|&n| n == w.damage_type).unwrap_or(0) as u8
}

fn bone_share(bone: u16) -> f32 {
    match bone {
        0x4C1 | 0x4C2 | 0x4C8 | 0x4C9 | 0x1A2 | 0x1A3 | 0x1A7 | 0x1A8 => crate::kv!(18),
        0x4C3 | 0x4D0 | 0x1A4 | 0x1A9 => 0.5,
        _ => 1.0,
    }
}

#[allow(non_snake_case)]
#[inline]
fn HEAD() -> u16 { crate::kv!(200 int) }
#[allow(non_snake_case)]
#[inline]
fn NECK() -> u16 { crate::kv!(201 int) }
#[allow(non_snake_case)]
#[inline]
fn SPINE() -> &'static [u16; 4] { static V: std::sync::OnceLock<[u16; 4]> = std::sync::OnceLock::new(); let f = || [crate::kv!(753 int), crate::kv!(203 int), crate::kv!(754 int), crate::kv!(204 int)]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

pub fn scale(hit: &Hit, victim: &Victim) -> f32 {
    let kind = damage_type(hit.weapon);
    let not_area = !(crate::kv!(0 int)..=crate::kv!(2 int)).contains(&kind);
    let mut a = hit.amount;
    if victim.player {
        if !not_area {
            a *= crate::kv!(25);
        } else if kind != 1 {
            a *= crate::kv!(130);
        }
    } else {
        a *= victim.factor;
    }
    let share = bone_share(hit.bone);
    if share != 1.0 {
        a *= share;
    }
    if victim.in_vehicle && victim.population {
        a *= crate::kv!(12);
    }
    let cut = not_area && match hit.attacker {
        Attacker::Player { .. } => hit.same_group && hit.amount != NO_CUT(),
        Attacker::Ped => (victim.player && hit.friend) || (victim.mission && hit.same_group),
        Attacker::None | Attacker::Vehicle { .. } => false,
    };
    if cut {
        let third = a * crate::kv!(130);
        a = if crate::kv!(851) - third >= 0.0 { third } else { crate::kv!(851) };
    }
    a
}

pub fn damage(h: &mut Health, hit: &Hit, victim: &Victim, rand: &mut Rand) -> Outcome {
    let mut out = Outcome::default();
    if h.health >= INJURED_BELOW() && !allowed(hit, victim) {
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
        if hit.bone == HEAD() {
            if victim.hat {
                a *= crate::kv!(18);
            }
            if hit.weapon.has("MELEE_CLUB") {
                a *= crate::kv!(108);
            }
            if !victim.player && a > crate::kv!(32) && rand.float_in_range(0.0, 1.0) < crate::kv!(57) {
                if h.health - a > INJURED_BELOW() {
                    a = h.health - INJURED_BELOW() + 1.0;
                }
                out.flags |= KNOCKED_OUT();
                guard = false;
            }
        }
        if guard && victim.guard {
            a *= 0.5;
        }
    }
    if instant(hit, victim, rand) {
        out.flags |= INSTANT() | KILLED();
        (out.lost, out.absorbed) = (h.health, h.armour);
        (h.health, h.armour) = (0.0, 0.0);
        return out;
    }
    let d = (a - out.absorbed).max(0.0);
    if hit.floor {
        let left = (h.health - d).max(crate::kv!(852));
        out.lost = h.health - left;
        h.health = left;
    } else if h.health - d >= INJURED_BELOW() {
        out.lost = d;
        h.health += -d;
    } else if h.health - d >= 1.0 && !(victim.in_vehicle || victim.player || victim.dies_when_injured || victim.deep_water) {
        out.lost = d;
        out.flags |= INJURED();
        h.health += -d;
    } else {
        out.flags |= KILLED();
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
    if !hurt.contains(&id) && hurt.len() < crate::kv!(7 int) {
        hurt.push(id);
        return true;
    }
    ped_ragdoll_state == Some(crate::kv!(2 int))
}

pub fn allowed(hit: &Hit, victim: &Victim) -> bool {
    let (weapon, kind) = (hit.number.unwrap_or_else(|| weapon_id(hit.weapon)), damage_type(hit.weapon));
    if (weapon == crate::kv!(287 int) && !victim.drowning_ok) || (weapon == crate::kv!(399 int) && !victim.weapon58_ok) {
        return false;
    }
    if victim.vest && hit.weapon.has("GUN") && (*SPINE()).contains(&hit.bone) {
        return false;
    }
    if victim.player && (!victim.hurtable || (victim.fire_proof && kind == crate::kv!(1 int)) || (weapon == crate::kv!(396 int) && !victim.blades_ok)) {
        return false;
    }
    if matches!(hit.attacker, Attacker::Ped) && hit.same_group && weapon != crate::kv!(448 int) {
        return false;
    }
    if kind == 2 && victim.in_vehicle && !victim.shootable_in_vehicle {
        return false;
    }
    may_damage(victim.flags, victim.group, weapon, kind, hit.attacker, hit.group) && !(weapon == crate::kv!(34 int) && victim.ragdolling)
}

fn may_damage(bits: u32, group: u16, weapon: u32, kind: u8, att: Attacker, att_group: Option<u32>) -> bool {
    use flags::*;
    if bits & INVINCIBLE() != 0 {
        return false;
    }
    let exempt = kind == crate::kv!(0 int) || matches!(weapon, 51 | 53 | 58);
    let who = match att {
        Attacker::Ped | Attacker::Player { .. } => Some(att_group.unwrap_or(0)),
        Attacker::Vehicle { .. } => att_group,
        Attacker::None => None,
    };
    if bits & ONLY_BY_PLAYER() != 0 && !exempt && !matches!(att, Attacker::Player { .. } | Attacker::Vehicle { population: false }) {
        return false;
    }
    if bits & ONLY_BY_GROUP() != 0 && !exempt && !who.is_some_and(|g| g & crate::kv!(50 int) == group as u32) {
        return false;
    }
    if bits & NOT_BY_GROUP() != 0 && !exempt && who == Some(group as u32) {
        return false;
    }
    match kind {
        1 if bits & MELEE_PROOF() != 0 => return false,
        2 if bits & BULLET_PROOF() != 0 => return false,
        3 if bits & EXPLOSION_PROOF() != 0 => return false,
        4 if bits & FIRE_PROOF() != 0 => return false,
        _ => {}
    }
    !(matches!(weapon, 46 | 49 | 50 | 54) && bits & COLLISION_PROOF() != 0)
}

pub fn car_hit(speed: f32, vz: f32) -> Option<(&'static str, f32)> {
    let speed = speed.min(crate::kv!(78));
    if speed > crate::kv!(21) {
        Some(("RUNOVERBYCAR", crate::kv!(80)))
    } else if speed > crate::kv!(32) || (vz < -crate::kv!(20) && speed > crate::kv!(5)) {
        Some(("RAMMEDBYCAR", crate::kv!(80)))
    } else {
        None
    }
}

pub fn car_hit_player(speed: f32, vz: f32, train: bool) -> Option<(&'static str, f32)> {
    let speed = speed.min(crate::kv!(78));
    (speed > crate::kv!(27) || (vz < -crate::kv!(20) && speed > crate::kv!(5))).then_some(("RAMMEDBYCAR", if train { crate::kv!(244) } else { crate::kv!(80) }))
}

pub fn fall_damage(fell: f32, player: bool, health: f32) -> f32 {
    let limit = if player { crate::kv!(26) } else { crate::kv!(27) };
    if fell > limit { health * crate::kv!(28) } else { fell * crate::kv!(108) }
}

pub mod control {
    #[allow(non_snake_case)]
    #[inline]
    pub fn SCRIPT() -> u32 { crate::kv!(7 int) }
    #[allow(non_snake_case)]
    #[inline]
    pub fn OTHER() -> u32 { crate::kv!(699 int) }
}

pub fn police_armour(model: &str) -> f32 {
    if ["M_Y_SWAT", "M_M_FBI"].iter().any(|m| m.eq_ignore_ascii_case(model)) { ARMOUR_CAP() } else { 0.0 }
}

fn instant(hit: &Hit, victim: &Victim, rand: &mut Rand) -> bool {
    if victim.player || victim.no_critical || damage_type(hit.weapon) != 2 {
        return false;
    }
    let sniper = hit.weapon.slot == "SNIPER";
    if (hit.bone == HEAD() || hit.bone == NECK()) && hit.weapon.slot != "SHOTGUN" {
        let kill = victim.in_vehicle
            || (victim.swat && rand.next() & crate::kv!(0 int) != 0)
            || sniper
            || matches!(hit.attacker, Attacker::Player { sure: true })
            || rand.next() & crate::kv!(194 int) == 0;
        if kill {
            return true;
        }
    }
    sniper && (*SPINE()).contains(&hit.bone)
}

