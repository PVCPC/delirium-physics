use crate::weaponnatives::{Hand, Slot, Weapon, Weapons, INFINITE, SLOTS};
use crate::weaponselect;

pub const SWAP: i32 = 319;
pub const SWAP_FLAGS: u32 = 0x10600;
pub const OBJECT: i32 = 0x2E;
pub const GUN: u32 = 0x20;
pub const TWO_HANDED: u32 = 0x1000;

pub fn can_threaten(flags: u32, both_hands: bool) -> bool {
    flags & GUN != 0 && (flags & TWO_HANDED == 0 || both_hands)
}
const FLEE: u32 = 1 << 28;
const MEMORY: u32 = 3 << 24;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Info {
    pub slot: i32,
    pub fire: u32,
    pub damage: u32,
    pub flags: u32,
    pub clip: u16,
    pub model: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rows(pub Vec<Info>);

impl Rows {
    pub fn of(&self, t: i32) -> Info {
        let i = if (0..60).contains(&t) { t as usize } else { 0 };
        self.0.get(i).copied().unwrap_or_default()
    }
}

pub fn holder() -> Weapons {
    Weapons { slot_object: Some(Weapon { kind: 0, clip: 0, total: 0 }), halfword_84: -1, ..Default::default() }
}

pub fn held_type(w: &Weapons) -> i32 {
    w.held().map_or(0, |x| x.kind)
}

pub fn damaging(w: &Weapons, rows: &Rows, held: bool) -> bool {
    let t = if held { held_type(w) } else { w.slots[w.current as usize].kind };
    matches!(rows.of(t).damage, 2..=4)
}

pub fn object_weapon(w: &Weapons) -> Option<Weapon> {
    match w.hand {
        Some(h) => h.weapon,
        None if w.current == 0 || w.current == 10 => w.slot_object,
        None => None,
    }
}

pub fn ammo_back(w: &mut Weapons, rows: &Rows) {
    if let Some(Hand { weapon: Some(x) }) = w.hand {
        let s = &mut w.slots[rows.of(x.kind).slot as usize];
        s.ammo = if x.total as i16 as i32 > INFINITE { INFINITE as u16 } else { x.total };
    }
}

struct Env<'a> {
    rows: &'a Rows,
    hand: Option<Hand>,
    slot_object: Option<Weapon>,
}

impl weaponselect::Env for Env<'_> {
    fn info(&self, weapon: u32) -> weaponselect::Info {
        let i = self.rows.of(weapon as i32);
        weaponselect::Info { fire: i.fire, damage: i.damage }
    }
    fn episode(&self) -> u32 {
        0
    }
    fn arg_class(&mut self, _: u32) -> u32 {
        0
    }
    fn fits(&mut self, _: u32, _: u32) -> bool {
        false
    }
    fn release(&mut self, _: u32) {}
    fn keep_held(&mut self, w: &weaponselect::Weapons) -> bool {
        let kind = w.slots[w.cur as usize].weapon as i32;
        let held = match self.hand {
            Some(h) => h.weapon.is_some_and(|x| x.kind == kind),
            None => (w.cur == 0 || w.cur == 10) && self.slot_object.is_some(),
        };
        !held
    }
}

pub fn select(w: &mut Weapons, rows: &Rows, mode: u32, slot: u32, delay: u32) -> bool {
    let mut s = weaponselect::Weapons {
        cur: w.current as u32,
        pending1: weaponselect::NONE,
        pending2: weaponselect::NONE,
        prev: w.previous as u32,
        slots: [weaponselect::Slot::default(); SLOTS],
        flag_8d: w.flag_8d,
        flag_8f: w.flag_8f,
        delay: w.halfword_84 as u16,
    };
    for (to, from) in s.slots.iter_mut().zip(&w.slots) {
        *to = weaponselect::Slot { weapon: from.kind as u32, ammo: from.ammo, held: from.flag };
    }
    let mut env = Env { rows, hand: w.hand, slot_object: w.slot_object };
    let changed = weaponselect::select(&mut s, &mut env, mode, slot, delay, 0);
    (w.current, w.previous, w.flag_8d, w.flag_8f, w.halfword_84) = (s.cur as i32, s.prev as i32, s.flag_8d, s.flag_8f, s.delay as i16);
    for (to, from) in w.slots.iter_mut().zip(&s.slots) {
        *to = Slot { kind: from.weapon as i32, ammo: from.ammo, flag: from.held };
    }
    changed
}

pub fn discard(w: &mut Weapons, rows: &Rows) -> Option<Weapon> {
    let h = w.hand?;
    if let Some(x) = h.weapon {
        let s = rows.of(x.kind).slot as usize;
        if w.slots[s].kind == x.kind {
            if s as i32 == w.current {
                select(w, rows, if x.kind == OBJECT { 18 } else { 7 }, 0, 1);
            }
            w.slots[s] = Slot::default();
        }
    }
    w.hand = None;
    h.weapon
}

pub fn holster(w: &mut Weapons, rows: &Rows) {
    let Some(h) = w.hand else { return };
    if let Some(x) = h.weapon {
        let s = &mut w.slots[rows.of(x.kind).slot as usize];
        if s.kind == x.kind {
            s.ammo = if x.total as i16 as i32 > INFINITE { INFINITE as u16 } else { x.total };
        }
    }
    w.hand = None;
}

pub fn equip(w: &mut Weapons, rows: &Rows) -> bool {
    let s = w.slots[w.current as usize];
    if w.hand.is_some_and(|h| h.weapon.is_some_and(|x| x.kind == s.kind)) {
        return true;
    }
    holster(w, rows);
    let row = rows.of(s.kind);
    if row.model {
        if s.flag == 0 {
            w.flag_8d = 1;
            return false;
        }
        let total = (s.ammo).min(0x7FFF);
        let cap = row.clip as i16;
        let clip = if total == 0 { 0 } else if total as i16 >= cap { row.clip } else { total };
        w.hand = Some(Hand { weapon: Some(Weapon { kind: s.kind, clip, total }) });
    }
    w.flag_8d = 0;
    w.halfword_84 = -1;
    w.hand.is_some()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Me {
    pub made_by: u8,
    pub cop: bool,
    pub armed_on: bool,
    pub in_vehicle: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Target {
    pub there: bool,
    pub damaging: bool,
    pub player: bool,
}

pub fn suits(flags: u32, me: &Me, t: &Target, w: &Weapons, rows: &Rows) -> (bool, bool) {
    let target_damaging = t.there && t.damaging;
    let player_or_mine = t.there && (t.player || me.made_by == 2);
    let mine_or_cop = me.made_by == 2 || me.cop;
    let want = flags & FLEE == 0
        && (flags & MEMORY >= 2 << 24 || target_damaging || me.armed_on || me.cop || mine_or_cop && player_or_mine);
    let slot_damaging = damaging(w, rows, false);
    let ready = slot_damaging && w.held().is_some_and(|x| x.kind == w.slots[w.current as usize].kind && x.total != 0);
    if !ready && want {
        return (true, true);
    }
    (!want && slot_damaging, want)
}

pub fn check(flags: u32, swapping: bool, me: &mut Me, t: &Target, w: &mut Weapons, rows: &Rows) -> bool {
    let (go, want) = suits(flags, me, t, w, rows);
    if !go || swapping {
        return false;
    }
    if !want {
        return select(w, rows, 0, 0, 1) && !me.in_vehicle;
    }
    if me.in_vehicle || !select(w, rows, 9, 0, 1) {
        return false;
    }
    if me.made_by == 1 {
        me.armed_on = true;
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adjust {
    On,
    Discard,
    Swap,
}

pub fn adjust(w: &mut Weapons, rows: &Rows, in_vehicle: bool) -> Adjust {
    let obj = object_weapon(w).filter(|x| x.kind != 0);
    let usable = obj.is_some_and(|x| rows.of(x.kind).flags & GUN == 0 || x.total as i16 > 0);
    let wrong = obj.is_some_and(|x| rows.of(x.kind).slot != w.current);
    if in_vehicle {
        return Adjust::On;
    }
    if w.current != 0 {
        if !usable || wrong {
            ammo_back(w, rows);
            select(w, rows, 9, 0xB, 0);
            return Adjust::Swap;
        }
        return Adjust::On;
    }
    match obj {
        Some(x) if x.kind == OBJECT => Adjust::Discard,
        Some(_) => {
            ammo_back(w, rows);
            Adjust::Swap
        }
        None => Adjust::On,
    }
}

pub fn make_swap(w: &mut Weapons, rows: &Rows) {
    if w.held().is_some_and(|x| x.kind == OBJECT) {
        discard(w, rows);
    }
}

pub fn fire_swap(w: &mut Weapons, rows: &Rows, swapping: bool) -> bool {
    let held = w.held().copied();
    let spent = held.is_some_and(|x| rows.of(x.kind).damage != 1 && x.clip == 0 && x.total == 0);
    let empty = w.current != 0 && held.is_none();
    if !(spent || empty) || swapping {
        return false;
    }
    ammo_back(w, rows);
    select(w, rows, 9, 0xB, 0);
    true
}

