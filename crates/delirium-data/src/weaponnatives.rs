pub const SLOTS: usize = 11;
pub const INFINITE: i32 = 25000;
pub const ROWS: usize = 60;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Info {
    pub slot: i32,
    pub fire: i32,
    pub damage: i32,
    pub clip: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Infos {
    pub rows: Vec<Info>,
}

impl Infos {
    pub fn index(&self, weapon_type: i32) -> usize {
        if (0..60).contains(&weapon_type) {
            weapon_type as usize
        } else {
            0
        }
    }

    pub fn of(&self, weapon_type: i32) -> Info {
        self.rows[self.index(weapon_type)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weapon {
    pub kind: i32,
    pub clip: u16,
    pub total: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hand {
    pub weapon: Option<Weapon>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slot {
    pub kind: i32,
    pub ammo: u16,
    pub flag: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Weapons {
    pub current: i32,
    pub previous: i32,
    pub hand: Option<Hand>,
    pub slot_object: Option<Weapon>,
    pub slots: [Slot; SLOTS],
    pub halfword_84: i16,
    pub flag_8d: u8,
    pub flag_8f: u8,
    pub flag_d9: u8,
}

impl Default for Weapons {
    fn default() -> Self {
        Weapons {
            current: 0,
            previous: 0,
            hand: None,
            slot_object: None,
            slots: [Slot::default(); SLOTS],
            halfword_84: 0,
            flag_8d: 0,
            flag_8f: 0,
            flag_d9: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ped {
    pub remote: bool,
    pub flags_23c: u32,
    pub a80: u32,
    pub weapons: Weapons,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    Local,
    Network,
}

pub trait Calls {
    fn model_of(&mut self, row: usize) -> i32;
    fn release_model(&mut self, model: i32);
    fn put_away(&mut self, a: i32, b: i32);
    fn drop_held(&mut self, flag: i32, zero: i32);
    fn select(&mut self, mode: i32, slot: i32, flag: i32);
    fn equip(&mut self, a: i32, b: i32);
}

pub fn add_armour(armour: f32, amount: i32, player_max: Option<u16>) -> f32 {
    let sum = armour + amount as f32;
    let capped = match player_max {
        Some(max) => {
            let max = max as f32;
            if sum < max {
                sum
            } else {
                max
            }
        }
        None => {
            if sum < 100.0 {
                sum
            } else {
                100.0
            }
        }
    };
    if capped > 0.0 {
        capped
    } else {
        0.0
    }
}

pub fn enable_max_ammo_cap(on: i32) -> u8 {
    (on != 0) as u8
}

fn set_total(w: &mut Weapon, ammo: u16) {
    w.total = if ammo as i32 > INFINITE { INFINITE as u16 } else { ammo };
}

fn add_total(w: &mut Weapon, add: i32) {
    let total = w.total as i16 as i32;
    if total == INFINITE || add == INFINITE {
        w.total = INFINITE as u16;
        return;
    }
    let sum = (total + add) as i16 as i32;
    w.total = if sum < 0 {
        0
    } else if sum > INFINITE {
        INFINITE as u16
    } else {
        sum as u16
    };
}

fn set_clip(w: &mut Weapon, v: i32, infos: &Infos) {
    let size = infos.of(w.kind).clip;
    w.clip = if v > size as i16 as i32 { size } else { v as u16 };
}

impl Weapons {
    fn in_hand_mut(&mut self) -> Option<&mut Weapon> {
        match &mut self.hand {
            Some(h) => h.weapon.as_mut(),
            None => {
                if self.current == 0 || self.current == 10 {
                    self.slot_object.as_mut()
                } else {
                    None
                }
            }
        }
    }

    pub fn held(&self) -> Option<&Weapon> {
        match &self.hand {
            Some(h) => {
                let w = h.weapon.as_ref()?;
                if w.kind == self.slots[self.current as usize].kind {
                    Some(w)
                } else {
                    None
                }
            }
            None => {
                if self.current == 0 || self.current == 10 {
                    self.slot_object.as_ref()
                } else {
                    None
                }
            }
        }
    }
}

pub fn previous(w: &Weapons, infos: &Infos, from: i32) -> i32 {
    let mut i = from - 1;
    if i < 0 {
        i = 10;
    } else if i == 0 {
        return 0;
    }
    loop {
        let s = w.slots[i as usize];
        if s.kind > 0 {
            let info = infos.of(s.kind);
            if s.ammo != 0 || info.damage == 1 {
                return i;
            }
        }
        i -= 1;
        if i == 0 {
            return 0;
        }
    }
}

fn empty_slot(calls: &mut dyn Calls, infos: &Infos, s: &mut Slot) {
    if s.flag != 0 {
        let model = calls.model_of(infos.index(s.kind));
        if model != -1 {
            calls.release_model(model);
        }
    }
    *s = Slot::default();
}

pub fn choose_next(w: &mut Weapons, infos: &Infos, calls: &mut dyn Calls, flag: bool) -> bool {
    let cur = w.current;
    let mut from = cur;
    if w.slots[cur as usize].kind == 46 && w.flag_8f != 0 {
        from = w.previous + 1;
    }
    let mut cand = loop {
        let c = previous(w, infos, from);
        let info = infos.of(w.slots[c as usize].kind);
        if !(info.fire == 3 && info.damage != 0) {
            break c;
        }
        from = c;
    };
    if cand != 0 {
        let kind = w.slots[cand as usize].kind;
        if kind == 0 {
            cand = 0;
        } else if infos.of(kind).damage != 1 && w.slots[cand as usize].ammo == 0 {
            empty_slot(calls, infos, &mut w.slots[cand as usize]);
            cand = 0;
        }
    }
    w.flag_8f = 0;
    if cand == cur && w.held().is_some() {
        return false;
    }
    w.previous = cur;
    w.current = cand;
    w.flag_8d = 0;
    w.halfword_84 = if flag { 0 } else { -1 };
    true
}

pub fn add_ammo(w: &mut Weapons, infos: &Infos, t: i32, add: i32) {
    if let Some(wp) = w.in_hand_mut() {
        if wp.kind == t {
            let outside = (wp.total as i16 as i32) - wp.clip as i32 + add;
            if outside < 0 {
                let size = infos.of(t).clip as i16 as i32;
                let clip = wp.clip as i32 + outside;
                let clip = if clip < 0 {
                    0
                } else if clip > size {
                    size
                } else {
                    clip
                };
                set_clip(wp, clip, infos);
            }
            add_total(wp, add);
            return;
        }
    }
    let slot = infos.of(t).slot as usize;
    let s = &mut w.slots[slot];
    if s.kind == t {
        let had = s.ammo as i16 as i32;
        s.ammo = if had == INFINITE || add == INFINITE {
            INFINITE as u16
        } else {
            let sum = (had + add) as i16 as i32;
            if sum < 0 {
                0
            } else if sum > INFINITE {
                INFINITE as u16
            } else {
                sum as u16
            }
        };
    }
}

pub fn set_ammo(w: &mut Weapons, infos: &Infos, t: i32, ammo: u16) {
    if let Some(wp) = w.in_hand_mut() {
        if wp.kind == t {
            set_total(wp, ammo);
            return;
        }
    }
    let slot = infos.of(t).slot as usize;
    let s = &mut w.slots[slot];
    if s.kind == t {
        s.ammo = ammo;
        if ammo as i16 as i32 > INFINITE {
            s.ammo = INFINITE as u16;
        }
    }
}

fn remove_one(w: &mut Weapons, infos: &Infos, calls: &mut dyn Calls, t: i32, flag: bool) {
    let slot = infos.of(t).slot as usize;
    if w.slots[slot].kind == t {
        empty_slot(calls, infos, &mut w.slots[slot]);
    }
    let in_hand = w.hand.as_ref().and_then(|h| h.weapon.as_ref()).map_or(false, |wp| wp.kind == t);
    if flag && in_hand {
        calls.drop_held(flag as i32, 0);
    }
    choose_next(w, infos, calls, false);
}

fn remove_all(w: &mut Weapons, infos: &Infos, calls: &mut dyn Calls, remote: bool) {
    if !remote || w.flag_d9 != 0 {
        calls.put_away(1, 0);
    }
    for s in w.slots.iter_mut() {
        if s.kind > 0 {
            if s.flag != 0 {
                let model = calls.model_of(infos.index(s.kind));
                if model != -1 {
                    calls.release_model(model);
                }
            }
            *s = Slot::default();
        }
    }
    w.current = 0;
}

pub fn add_ammo_to_char(ped: &mut Ped, infos: &Infos, t: i32, add: i16) -> Route {
    add_ammo(&mut ped.weapons, infos, t, add as i32);
    Route::Local
}

pub fn set_char_ammo(ped: &mut Ped, infos: &Infos, t: i32, ammo: i32) -> Route {
    if ped.remote {
        return Route::Network;
    }
    set_ammo(&mut ped.weapons, infos, t, ammo as u16);
    Route::Local
}

pub fn remove_weapon_from_char(ped: &mut Ped, infos: &Infos, calls: &mut dyn Calls, t: i32) -> Route {
    if ped.remote {
        return Route::Network;
    }
    remove_one(&mut ped.weapons, infos, calls, t, true);
    Route::Local
}

pub fn remove_all_char_weapons(ped: &mut Ped, infos: &Infos, calls: &mut dyn Calls) -> Route {
    if ped.remote {
        return Route::Network;
    }
    remove_all(&mut ped.weapons, infos, calls, false);
    Route::Local
}

pub fn set_current_char_weapon(ped: &mut Ped, infos: &Infos, calls: &mut dyn Calls, t: i32, draw: bool) -> Route {
    let slot = infos.of(t).slot as usize;
    if ped.weapons.slots[slot].kind != t {
        return Route::Local;
    }
    let mode = if ped.flags_23c & 0x2000_0000 != 0 && ped.a80 != 0 { 0x13 } else { 0 };
    calls.select(mode, slot as i32, 1);
    if draw {
        calls.equip(-1, 0);
    }
    Route::Local
}

