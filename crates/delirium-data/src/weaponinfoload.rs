use crate::map::hash_bytes;
use crate::weapons;

pub type Row = [u8; ROW];
pub const ROW: usize = 272;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    pub name: String,
    pub kind: u8,
    pub text: Option<String>,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Element>,
}

fn hash(name: &str) -> u32 {
    hash_bytes(name.as_bytes(), 0)
}

const RELOAD: u32 = 0x5da46b68;
const DAMAGE: u32 = 0x431375e1;
const AIMING: u32 = 0x09a5b381;
const PROJECTILE: u32 = 0x4ace140a;
const FLAGS: u32 = 0x67d26872;
const PICKUP: u32 = 0xbc9403c8;
const PHYSICS: u32 = 0x22b6caa7;
const CONTROLLER: u32 = 0xcebcfd68;
const OFFSET: u32 = 0xa13c79ef;
const CROUCHED_OFFSET: u32 = 0x04bf038a;
const AIM_PITCH: u32 = 0x9446c004;
const AIM_STANCE: u32 = 0xcb57b1ff;
const TYPE_TO_CREATE: u32 = 0x82527040;
const EXPLOSION: u32 = 0xdd91088f;
const PROJECTILE_VECTOR: u32 = 0x36f9e805;

pub const WEAPON_TYPES: [&str; 58] = [
    "UNARMED", "BASEBALLBAT", "POOLCUE", "KNIFE", "GRENADE", "MOLOTOV", "ROCKET", "PISTOL", "UNUSED0", "DEAGLE", "SHOTGUN",
    "BARETTA", "MICRO_UZI", "MP5", "AK47", "M4", "SNIPERRIFLE", "M40A1", "RLAUNCHER", "FTHROWER", "MINIGUN", "EPISODIC_1",
    "EPISODIC_2", "EPISODIC_3", "EPISODIC_4", "EPISODIC_5", "EPISODIC_6", "EPISODIC_7", "EPISODIC_8", "EPISODIC_9",
    "EPISODIC_10", "EPISODIC_11", "EPISODIC_12", "EPISODIC_13", "EPISODIC_14", "EPISODIC_15", "EPISODIC_16", "EPISODIC_17",
    "EPISODIC_18", "EPISODIC_19", "EPISODIC_20", "EPISODIC_21", "EPISODIC_22", "EPISODIC_23", "EPISODIC_24", "CAMERA", "OBJECT",
    "WEAPONTYPE_LAST_WEAPONTYPE", "ARMOUR", "RAMMEDBYCAR", "RUNOVERBYCAR", "EXPLOSION", "UZI_DRIVEBY", "DROWNING", "FALL",
    "UNIDENTIFIED", "ANYMELEE", "ANYWEAPON",
];
pub const FIRE_TYPES: [&str; 8] = ["MELEE", "INSTANT_HIT", "DELAYED_HIT", "PROJECTILE", "AREA_EFFECT", "CAMERA", "USE", "UNKNOWN"];
pub const PROJECTILE_TYPES: [&str; 7] = ["NONE", "FUSE", "IMPACT", "ROCKET", "STICKY", "UNKNOWN", ""];
pub const EXPLOSIONS: [&str; 26] = [
    "GRENADE", "MOLOTOV", "ROCKET", "HI_OCTANE", "CAR", "PLANE", "PETROL_PUMP", "BIKE", "DIR_STEAM", "DIR_FLAME",
    "DIR_WATER_HYDRANT", "DIR_GAS_CANISTER", "BOAT", "SHIP_DESTROY", "TRUCK", "EPISODIC_1", "EPISODIC_2", "EPISODIC_3",
    "EPISODIC_4", "EPISODIC_5", "EPISODIC_6", "EPISODIC_7", "EPISODIC_8", "EPISODIC_9", "EPISODIC_10", "UNKNOWN",
];

fn attr<'a>(el: &'a Element, name: &str) -> Option<&'a str> {
    el.attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

fn attr_or_empty<'a>(el: &'a Element, name: &str) -> &'a str {
    attr(el, name).unwrap_or("")
}

pub fn int_text(s: &str) -> i32 {
    s.trim().parse().unwrap_or(0)
}

pub fn float_text(s: &str) -> f32 {
    s.trim().parse().unwrap_or(0.0)
}

pub fn token(text: Option<&str>) -> &str {
    text.and_then(|t| t.split_whitespace().next()).unwrap_or("")
}

pub fn hex_text(s: &str) -> u32 {
    let digits: String = s.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    if digits.is_empty() {
        0
    } else {
        u32::from_str_radix(&digits, crate::kv!(51 int)).unwrap_or(u32::MAX)
    }
}

fn int_attr(el: &Element, name: &str, default: i32) -> i32 {
    attr(el, name).map_or(default, int_text)
}

fn float_attr(el: &Element, name: &str, default: f32) -> f32 {
    attr(el, name).map_or(default, float_text)
}

fn put32(row: &mut Row, at: usize, v: u32) {
    row[at..at + crate::kv!(1 int)].copy_from_slice(&v.to_be_bytes());
}

fn put16(row: &mut Row, at: usize, v: u16) {
    row[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn putf(row: &mut Row, at: usize, v: f32) {
    put32(row, at, v.to_bits());
}

fn get32(row: &Row, at: usize) -> u32 {
    u32::from_be_bytes([row[at], row[at + 1], row[at + 2], row[at + crate::kv!(0 int)]])
}

fn vector(el: &Element, row: &mut Row, at: usize) {
    for (i, k) in ["x", "y", "z"].iter().enumerate() {
        putf(row, at + crate::kv!(1 int) * i, float_attr(el, k, 0.0));
    }
}

pub fn fire_type_index(name: &str) -> i32 {
    if name.is_empty() {
        return 1;
    }
    FIRE_TYPES.iter().position(|t| t.eq_ignore_ascii_case(name)).map_or(1, |i| i as i32)
}

pub fn projectile_type_index(name: &str) -> i32 {
    if name.is_empty() {
        return crate::kv!(2 int);
    }
    PROJECTILE_TYPES.iter().position(|t| t.eq_ignore_ascii_case(name)).map_or(crate::kv!(2 int), |i| i as i32)
}

pub fn explosion_index(name: &str) -> i32 {
    if name.is_empty() {
        return -1;
    }
    EXPLOSIONS.iter().position(|t| t.eq_ignore_ascii_case(name)).map_or(-1, |i| i as i32)
}

pub fn weapon_type_index(name: &str, default: i32) -> i32 {
    if name.is_empty() {
        return default;
    }
    let h = hash(name);
    WEAPON_TYPES.iter().position(|t| hash(t) == h).map_or(default, |i| i as i32)
}

pub fn aiming(el: &Element, row: &mut Row) {
    putf(row, crate::kv!(376 int), float_attr(el, "accuracy", 1.0));
    putf(row, crate::kv!(314 int), float_attr(el, "accuracyfps", 1.0));
    putf(row, crate::kv!(404 int), float_attr(el, "accuracyTime", 0.0));
    put32(row, crate::kv!(303 int), int_attr(el, "pellets", 0) as u32);
    for c in &el.children {
        match hash(&c.name) {
            OFFSET => vector(c, row, crate::kv!(251 int)),
            CROUCHED_OFFSET => vector(c, row, crate::kv!(355 int)),
            AIM_PITCH => putf(row, crate::kv!(282 int), float_attr(c, "pitch", 0.0)),
            AIM_STANCE => {
                putf(row, crate::kv!(189 int), float_attr(c, "standing", 0.0));
                putf(row, crate::kv!(430 int), float_attr(c, "ducked", 0.0));
                putf(row, crate::kv!(421 int), float_attr(c, "scale", 0.0));
            }
            _ => {}
        }
    }
}

pub fn projectile(el: &Element, row: &mut Row) {
    if get32(row, crate::kv!(23 int)) != crate::kv!(0 int) {
        return;
    }
    put32(row, crate::kv!(84 int), projectile_type_index(attr_or_empty(el, "type")) as u32);
    put32(row, crate::kv!(371 int), int_attr(el, "fusetime", 0) as u32);
    for c in &el.children {
        match hash(&c.name) {
            TYPE_TO_CREATE => put32(row, crate::kv!(320 int), weapon_type_index(attr_or_empty(c, "type"), 0) as u32),
            OFFSET => vector(c, row, crate::kv!(87 int)),
            EXPLOSION => put32(row, crate::kv!(416 int), explosion_index(attr_or_empty(c, "type")) as u32),
            PHYSICS => {
                putf(row, crate::kv!(96 int), float_attr(c, "force", 0.0));
                putf(row, crate::kv!(361 int), float_attr(c, "explodeimpactthreshold", 0.0));
                putf(row, crate::kv!(389 int), float_attr(c, "explodeimpactwithvehiclethreshold", 0.0));
                putf(row, crate::kv!(354 int), float_attr(c, "vehiclevelocity", 0.0));
            }
            PROJECTILE_VECTOR => vector(c, row, crate::kv!(452 int)),
            _ => {}
        }
    }
}

pub fn flags(el: &Element, row: &mut Row) {
    for c in &el.children {
        if c.kind == 0 {
            continue;
        }
        if c.name.eq_ignore_ascii_case("flag") {
            let bit = weapons::flag_bit(token(c.text.as_deref()));
            let word = get32(row, crate::kv!(7 int));
            let add = if (0..crate::kv!(7 int)).contains(&bit) { 1u32 << bit } else { 0 };
            put32(row, crate::kv!(7 int), word | add);
        } else if c.name.eq_ignore_ascii_case("flags") {
            put32(row, crate::kv!(7 int), hex_text(token(c.text.as_deref())));
        }
    }
}

pub fn controller(el: &Element, row: &mut Row) {
    for c in &el.children {
        if c.name.eq_ignore_ascii_case("rumble") {
            put32(row, crate::kv!(429 int), int_attr(c, "duration", 0) as u32);
            putf(row, crate::kv!(367 int), float_attr(c, "intensity", 0.0));
        }
    }
}

pub fn load(el: &Element, row: &mut Row) {
    put32(row, crate::kv!(1 int), weapons::slot_index(attr_or_empty(el, "slot")) as u32);
    put32(row, crate::kv!(23 int), fire_type_index(attr_or_empty(el, "firetype")) as u32);
    put32(row, crate::kv!(52 int), weapons::damage_type_index(attr_or_empty(el, "damagetype")) as u32);
    put32(row, crate::kv!(51 int), weapons::group_index(attr_or_empty(el, "group")) as u32);
    putf(row, crate::kv!(118 int), float_attr(el, "targetrange", 0.0));
    putf(row, crate::kv!(96 int), float_attr(el, "weaponrange", 0.0));
    put32(row, crate::kv!(81 int), weapon_type_index(attr_or_empty(el, "stattype"), -1) as u32);
    put16(row, crate::kv!(326 int), int_attr(el, "clipsize", 0) as u16);
    let ammo = ((int_attr(el, "ammomax", crate::kv!(970 int)) as i16) as i32).clamp(0, crate::kv!(970 int));
    put16(row, crate::kv!(318 int), ammo as u16);
    put32(row, crate::kv!(287 int), int_attr(el, "timebetweenshots", 0) as u32);
    for c in &el.children {
        match hash(&c.name) {
            RELOAD => {
                put32(row, crate::kv!(295 int), int_attr(c, "time", 0) as u32);
                put32(row, crate::kv!(405 int), int_attr(c, "fasttime", 0) as u32);
                put32(row, crate::kv!(483 int), int_attr(c, "crouchtime", 0) as u32);
            }
            DAMAGE => {
                put16(row, crate::kv!(366 int), int_attr(c, "base", 0) as u16);
                put16(row, crate::kv!(359 int), int_attr(c, "fps", 0) as u16);
                putf(row, crate::kv!(293 int), float_attr(c, "networkplayermod", 0.0));
                putf(row, crate::kv!(349 int), float_attr(c, "networkpedmod", 0.0));
            }
            AIMING => aiming(c, row),
            PROJECTILE => projectile(c, row),
            FLAGS => flags(c, row),
            PICKUP => {
                put32(row, crate::kv!(297 int), int_attr(c, "regentime", 0) as u32);
                put16(row, crate::kv!(249 int), int_attr(c, "ammoonstreet", 0) as u16);
            }
            PHYSICS => putf(row, crate::kv!(83 int), float_attr(c, "force", 0.0)),
            CONTROLLER => controller(c, row),
            _ => {}
        }
    }
}

