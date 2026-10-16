use crate::actiontable::{Action, Impulse};
use crate::controls::{index, pick, Controls, IoValue, AXIS_PAIRS};

fn back(at: u8, i: u8) -> usize {
    (if i <= at { at - i } else { at.wrapping_sub(i.min(0x3F)).wrapping_add(0x40) }) as usize
}

fn since(v: &IoValue, h: &[(u8, u32)], ms: u32) -> u32 {
    h[v.history_at as usize].1.saturating_sub(ms)
}

fn within(v: &IoValue, value: u8, lo: u8, hi: u8) -> bool {
    let x = value ^ v.invert;
    if lo > hi { hi <= x && x <= lo } else { lo <= x && x <= hi }
}

fn hist(v: &IoValue) -> &[(u8, u32)] {
    v.history.as_deref().expect("melee controls keep a history")
}

pub fn held(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    all_since(v, ms, |x| within(v, x, lo, hi))
}

pub fn held_up(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    all_since(v, ms, |x| !within(v, x, lo, hi))
}

fn all_since(v: &IoValue, ms: u32, ok: impl Fn(u8) -> bool) -> bool {
    let h = hist(v);
    let t = since(v, h, ms);
    for i in 0..0x40 {
        let e = h[back(v.history_at, i)];
        if e.1 < t {
            return true;
        }
        if !ok(e.0) {
            return false;
        }
    }
    true
}

pub fn pressed_and_held(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    let h = hist(v);
    within(v, h[v.history_at as usize].0, lo, hi) && first_since(v, ms, |x| !within(v, x, lo, hi))
}

pub fn released_and_held(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    let h = hist(v);
    !within(v, h[v.history_at as usize].0, lo, hi) && first_since(v, ms, |x| within(v, x, lo, hi))
}

fn first_since(v: &IoValue, ms: u32, hit: impl Fn(u8) -> bool) -> bool {
    let h = hist(v);
    let t = since(v, h, ms);
    for i in 1..0x40 {
        let e = h[back(v.history_at, i)];
        if e.1 < t {
            return false;
        }
        if hit(e.0) {
            return true;
        }
    }
    false
}

pub fn pressed(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    edge_since(v, ms, |before, after| !within(v, before, lo, hi) && within(v, after, lo, hi))
}

pub fn released(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    edge_since(v, ms, |before, after| within(v, before, lo, hi) && !within(v, after, lo, hi))
}

fn edge_since(v: &IoValue, ms: u32, edge: impl Fn(u8, u8) -> bool) -> bool {
    let h = hist(v);
    let t = since(v, h, ms);
    for i in 1..0x40 {
        let e = h[back(v.history_at, i)];
        if e.1 < t {
            return false;
        }
        if edge(e.0, h[back(v.history_at, i - 1)].0) {
            return true;
        }
    }
    false
}

fn byte(x: f32) -> u8 {
    x as i64 as u8
}

impl Impulse {
    pub fn test(&self, c: &Controls, setting: u32) -> bool {
        if !self.check {
            return true;
        }
        let v = |off: u32| &c.values[index(off)];
        let axis = |n: usize| v(pick(AXIS_PAIRS[n], v(AXIS_PAIRS[n].0)));
        let (a, b) = match self.impulse {
            1 => (v(0xBDC), None),
            2 => (v(0xBE8), None),
            3 => (v(0xBF4), None),
            4 => (v(0xD14), None),
            5 => (v(0xC00), None),
            6 => (v(0xC0C), None),
            7 => (axis(0), None),
            8 => (axis(1), None),
            9 => (axis(0), Some(axis(1))),
            _ => return false,
        };
        let ms = (self.interval * 1000.0) as i64 as u32;
        let [r0, r1] = self.range;
        let (lo, hi) = match self.impulse {
            7 | 8 => (byte((r0 + 1.0) * 0.5 * 255.0), byte((r1 + 1.0) * 0.5 * 255.0)),
            9 => (byte(r0.abs() * 127.0), byte(r1.abs() * 127.0)),
            3 if setting == 0 => (byte(r0 * 255.0), byte(r1 * 255.0)),
            _ => (0x80, 0xFF),
        };
        let q: fn(&IoValue, u32, u8, u8) -> bool = match self.state {
            0 => return true,
            1 => held,
            2 => held_up,
            3 => pressed_and_held,
            4 => released_and_held,
            5 => pressed,
            6 => released,
            _ => return false,
        };
        match b {
            None => q(a, ms, lo, hi),
            Some(b) => {
                let (p, n) = ((lo.wrapping_add(0x80), hi.wrapping_add(0x80)), (0x7Fu8.wrapping_sub(hi), 0x7Fu8.wrapping_sub(lo)));
                q(a, ms, p.0, p.1) || q(a, ms, n.0, n.1) || q(b, ms, p.0, p.1) || q(b, ms, n.0, n.1)
            }
        }
    }
}

pub fn impulses_hold(a: &Action, c: Option<&Controls>, setting: u32) -> bool {
    let Some(c) = c else { return false };
    let [i0, i1] = &a.impulses;
    match a.impulse_op {
        0 => i0.test(c, setting) || i1.test(c, setting),
        1 => i0.test(c, setting) && i1.test(c, setting),
        2 => i1.test(c, setting) != i0.test(c, setting),
        _ => false,
    }
}

pub const WEAPON_TYPES: [&str; 58] = [
    "UNARMED", "BASEBALLBAT", "POOLCUE", "KNIFE", "GRENADE", "MOLOTOV", "ROCKET", "PISTOL", "UNUSED0", "DEAGLE", "SHOTGUN",
    "BARETTA", "MICRO_UZI", "MP5", "AK47", "M4", "SNIPERRIFLE", "M40A1", "RLAUNCHER", "FTHROWER", "MINIGUN", "EPISODIC_1",
    "EPISODIC_2", "EPISODIC_3", "EPISODIC_4", "EPISODIC_5", "EPISODIC_6", "EPISODIC_7", "EPISODIC_8", "EPISODIC_9",
    "EPISODIC_10", "EPISODIC_11", "EPISODIC_12", "EPISODIC_13", "EPISODIC_14", "EPISODIC_15", "EPISODIC_16", "EPISODIC_17",
    "EPISODIC_18", "EPISODIC_19", "EPISODIC_20", "EPISODIC_21", "EPISODIC_22", "EPISODIC_23", "EPISODIC_24", "CAMERA",
    "OBJECT", "WEAPONTYPE_LAST_WEAPONTYPE", "ARMOUR", "RAMMEDBYCAR", "RUNOVERBYCAR", "EXPLOSION", "UZI_DRIVEBY",
    "DROWNING", "FALL", "UNIDENTIFIED", "ANYMELEE", "ANYWEAPON",
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held {
    pub kind: u32,
    pub flags: u32,
    pub between_shots: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fighter {
    pub player: bool,
    pub health: f32,
    pub attack: f32,
    pub defend: f32,
    pub martial: f32,
    pub weapon: Option<Held>,
}

pub fn who_takes(who: u32, player: bool) -> bool {
    match who {
        0 => true,
        1 => player,
        2 => !player,
        _ => false,
    }
}

pub fn ped_takes(a: &Action, f: &Fighter, receiving_hit: bool, blocking: bool, just_struck: bool) -> bool {
    if !who_takes(a.ai_player_filter, f.player)
        || a.flags[1] && !receiving_hit
        || a.flags[2] && !blocking
        || a.flags[3] && !just_struck
    {
        return false;
    }
    let stats = [f.health, f.attack, f.defend, f.martial];
    if stats.iter().zip(&a.ranges).any(|(x, r)| *x < r[0] || *x > r[1]) {
        return false;
    }
    let h = |s: &str| crate::map::hash_bytes(s.as_bytes(), 0);
    let class = a.weapon_class;
    if class == 0 || class == crate::actiontable::none() {
        return false;
    }
    if class == h("any") {
        return true;
    }
    if class == h("unarmed") {
        return f.weapon.map_or(true, |w| w.kind == 0);
    }
    let Some(w) = f.weapon else { return false };
    let bit = |n: u32| w.flags >> n & 1 != 0;
    let one_handed = bit(5) && !bit(12);
    let mg = bit(5) && w.between_shots < 200;
    match class {
        c if c == h("object") => w.kind == 0x2E,
        c if c == h("knife") => bit(10),
        c if c == h("club") => bit(9),
        c if c == h("oneHandedGun") => one_handed,
        c if c == h("oneHandedMG") => one_handed && mg,
        c if c == h("pistol") => one_handed && !mg,
        c if c == h("rifle") => bit(5) && bit(12),
        c => w.kind == WEAPON_TYPES.iter().position(|t| h(t) == c).map_or(0x37, |i| i as u32),
    }
}

