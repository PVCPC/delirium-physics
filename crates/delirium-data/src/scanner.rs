use crate::audioconfig::{name_hash, ScannerCrime};
use crate::rng::AudioRand;

pub const SLOTS: usize = 32;
pub const NONE: u32 = u32::MAX;
pub const PICK_GAP_MS: u32 = 15_000;
pub const AFTER_REPORT_MS: u32 = 5_000;
pub const AMBIENT_LEFTOVER_DELAY_MS: u32 = 0x4472_E471;

pub const STUMBLE_CHANCE: f32 = 0.6;
pub const SAME_AREA_SKIP: f32 = 0.85;
pub const NEAR_CENTRE: f32 = 0.01;
pub const OVER_90M_SQUARED: f32 = 0.000_123_456_79;
pub const STANDING_M: f32 = 12.0;
pub const IN_GAP_MS: u32 = 50;
pub const ERR_GAP_MS: u32 = 400;
pub const STREET_GAP_MS: u32 = 423;
pub const SCRIPTED_GAP_MS: u32 = 7_000;
pub const HEADING_MS: u32 = 5_000;
pub const ON: bool = true;
pub const NEAR_END_MS: u32 = 85;
pub const CHATTER_DB: f32 = -9.0;
pub const NEXT_KINDS: [u32; 9] = [0xFFFF_FFFF, 0xFFFF_FFFD, 0xFFFF_FFFB, 0xFFFF_FFFF, 0xFFFF_FFEF, 0x0000_0081, 0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF];
pub const PREDELAY_MS: u32 = 750;
pub const NOISE_LOOP: &str = "POLICE_SCANNER_NOISE_LOOP";

pub const AMBIENT_CRIMES: [&str; 28] = [
    "CRIME_CAR_SET_ON_FIRE", "CRIME_COP_SET_ON_FIRE", "CRIME_DAMAGE_TO_PROPERTY", "CRIME_DISTURBANCE",
    "CRIME_DRIVE_AGAINST_TRAFFIC", "CRIME_DRIVEBY", "CRIME_DRUG_DEAL", "CRIME_FLEEING_CRIME_SCENE", "CRIME_GANG_ACTIVITY",
    "CRIME_GUN_SPREE", "CRIME_HIT_COP", "CRIME_HIT_PED", "CRIME_PED_SET_ON_FIRE", "CRIME_POSSESSION_GUN",
    "CRIME_RECKLESS_DRIVING", "CRIME_RIDING_BIKE_WITHOUT_HELMET", "CRIME_RUN_REDLIGHT", "CRIME_RUNOVER_COP",
    "CRIME_RUNOVER_PED", "CRIME_SHOOT_COP", "CRIME_SHOOT_GUN_FIRED", "CRIME_SHOOT_PED", "CRIME_SPEEDING", "CRIME_STAB_COP",
    "CRIME_STAB_PED", "CRIME_STEAL_CAR", "CRIME_STEAL_VEHICLE", "CRIME_TARGET_COP",
];

pub const DISPATCH_PLACES: [[f32; 3]; 17] = [
    [-1065.92, 1261.53, 0.0], [-1891.55, 283.57, 0.0], [-1795.53, -373.53, 0.0], [-1174.27, -914.58, 0.0],
    [91.74, 1188.95, 0.0], [-426.31, 1099.28, 0.0], [50.2, 679.66, 0.0], [-407.38, 279.56, 0.0], [221.13, -255.53, 0.0],
    [-351.22, -321.13, 0.0], [974.29, 1869.2, 0.0], [440.68, 1596.93, 0.0], [1290.01, 662.83, 0.0], [2379.73, 552.87, 0.0],
    [1465.62, -289.3, 0.0], [894.41, -356.759, 0.0], [-1572.53, 553.76, 24.44],
];

pub const DISPATCH_UNITS: [&str; 7] = [
    "POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_A_UNIT_FROM", "POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_2_UNITS_FROM",
    "POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_3_UNITS_FROM", "POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_4_UNITS_FROM",
    "POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_5_UNITS_FROM", "POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_6_UNITS_FROM",
    "POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_7_UNITS_FROM",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Wagon {
    Ambulance,
    FireTruck,
    #[default]
    Other,
}

pub trait World {
    fn place(&mut self, at: [f32; 3]) -> Place;
    fn crime(&self, hash: u32) -> Option<ScannerCrime>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chatter {
    Ambient,
    Dispatch { units: u32, kind: u32, from: [f32; 3] },
    Assistance([f32; 3]),
    Chat,
}

pub fn ambient_pick(at: [f32; 3], wagon: Wagon, rng: &mut AudioRand) -> ([f32; 3], u32, u32) {
    let to = offset(at, 400.0, 2000.0, rng);
    let hash = match wagon {
        Wagon::Ambulance => name_hash("CRIME_AMBIENT_MEDICAL_EVENT"),
        Wagon::FireTruck => name_hash("CRIME_AMBIENT_FIRE"),
        Wagon::Other => AMBIENT_CRIMES.get(rng.int_in_range(0, 28) as usize).map_or(name_hash(NOISE_LOOP), |c| name_hash(c)),
    };
    (to, hash, rng.int_in_range(0, 31) as u32)
}

fn offset(at: [f32; 3], lo: f32, hi: f32, rng: &mut AudioRand) -> [f32; 3] {
    let (dx, dy) = (rng.float_in_range(lo, hi), rng.float_in_range(lo, hi));
    let sign = |v: f32| if v >= 0.0 { if -v >= 0.0 { 0.0 } else { 1.0 } } else { -1.0f32 };
    let x = sign(rng.float_in_range(-1.0, 1.0)).mul_add(dx, at[0]);
    let y = sign(rng.float_in_range(-1.0, 1.0)).mul_add(dy, at[1]);
    [x, y, at[2]]
}

pub const CRIME_SOUNDS: [&str; 31] = [
    "CRIME_NONE", "CRIME_POSSESSION_GUN", "CRIME_RUN_REDLIGHT", "CRIME_RECKLESS_DRIVING", "CRIME_SPEEDING",
    "CRIME_DRIVE_AGAINST_TRAFFIC", "CRIME_RIDING_BIKE_WITHOUT_HELMET", "CRIME_STEAL_VEHICLE", "CRIME_STEAL_CAR",
    "CRIME_BLOCK_POLICE_CAR", "CRIME_STAND_ON_POLICE_CAR", "CRIME_HIT_PED", "CRIME_HIT_COP", "CRIME_SHOOT_PED",
    "CRIME_SHOOT_COP", "CRIME_RUNOVER_PED", "CRIME_RUNOVER_COP", "CRIME_DESTROY_HELI", "CRIME_PED_SET_ON_FIRE",
    "CRIME_COP_SET_ON_FIRE", "CRIME_CAR_SET_ON_FIRE", "CRIME_DESTROY_PLANE", "CRIME_CAUSE_EXPLOSION", "CRIME_STAB_PED",
    "CRIME_STAB_COP", "CRIME_DESTROY_VEHICLE", "CRIME_DAMAGE_TO_PROPERTY", "CRIME_TARGET_COP", "CRIME_SHOOT_GUN_FIRED",
    "CRIME_RESISTING_ARREST", "CRIME_MOLOTOV",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Phrase {
    pub sound: u32,
    pub gap: u32,
    pub bank: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slot {
    pub time: u32,
    pub priority: u32,
    pub kind: u32,
    pub sub: u32,
    pub area: u32,
    pub at: u32,
    pub phrases: Vec<Phrase>,
    pub queued: bool,
    pub flag: bool,
    pub vehicle: bool,
    pub place: Option<[f32; 3]>,
    pub group: u32,
    pub category: u32,
    pub noise: u32,
    pub echo: u32,
    pub described: u32,
    pub said: [bool; 4],
    pub on_foot: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Place {
    pub area: u32,
    pub street: u32,
    pub dir: u32,
    pub in_dir: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct PlaceZone<'a> {
    pub text: &'a str,
    pub centre: [i16; 2],
    pub big: bool,
}

fn phrase_name(prefix: &str, text: &str) -> u32 {
    name_hash(&format!("{prefix}{}", text.replace(' ', "_")))
}

pub fn quarter(n: [f32; 3]) -> u32 {
    let z0 = n[2] * 0.0;
    let c = n[0].mul_add(0.0, z0) + n[1];
    let a = if c <= -1.0 {
        std::f32::consts::PI
    } else if c >= 1.0 {
        0.0
    } else {
        crate::mathrt::acos(c as f64) as f32
    };
    let mut deg = a * 57.295_776;
    if n[1].mul_add(0.0, z0) + n[0] <= 0.0 {
        deg = 360.0 - deg;
    }
    ((deg + 45.0) * 0.011_111_111) as i64 as u32 & 3
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let r = 1.0 / (v[2].mul_add(v[2], v[0].mul_add(v[0], v[1] * v[1]))).sqrt();
    [v[0] * r, v[1] * r, v[2] * r]
}

pub fn place(street: Option<&str>, at: [f32; 3], zone: &PlaceZone) -> Place {
    let mut p = Place { street: street.map_or(0, |s| phrase_name("POLICE_SCANNER_STREETS_", s)), ..Default::default() };
    let d = [at[0] - zone.centre[0] as f32, at[1] - zone.centre[1] as f32, 0.0];
    if zone.big {
        let far = d[1].mul_add(d[1], d[0] * d[0]) * OVER_90M_SQUARED;
        let far = if 1.0 - far >= 0.0 { far } else { 1.0 };
        let upper = zone.text.as_bytes();
        let said = ["THE", "EAST", "NORTH", "WEST", "SOUTH"].iter().any(|w| upper.len() >= w.len() && upper[..w.len()].to_ascii_uppercase() == w.as_bytes());
        if !said {
            let (dir, in_dir) = if far > NEAR_CENTRE {
                let q = quarter(unit(d)) as usize;
                if zone.text.len() & 1 == 0 {
                    (["NORTH", "EAST", "SOUTH", "WEST"][q], ["IN_NORTH", "IN_EAST", "IN_SOUTH", "IN_WEST"][q])
                } else {
                    (["NORTHERN", "EASTERN", "SOUTHERN", "WESTERN"][q], ["IN_NORTHERN", "IN_EASTERN", "IN_SOUTHERN", "IN_WESTERN"][q])
                }
            } else {
                ("CENTRAL", "IN_CENTRAL")
            };
            p.dir = phrase_name("POLICE_SCANNER_DIRECTION_DIR_", dir);
            p.in_dir = phrase_name("POLICE_SCANNER_IN_DIRECTION_", in_dir);
        }
    }
    p.area = phrase_name("POLICE_SCANNER_AREAS_", zone.text);
    p
}

pub fn crime_gate(crime: u32, multiplier: f32, level: u32, fake: i32) -> bool {
    multiplier >= 0.9 && (level != 0 || fake < 1) && crime < 0x1F
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Report {
    pub place: Place,
    pub long: bool,
    pub priority: u32,
    pub delay: u32,
    pub sub: u32,
    pub flag: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhraseState {
    Gone,
    Ending,
    Playing,
}

pub trait Voice {
    fn make(&mut self, slot: usize, s: &Slot) -> bool;
    fn loaded(&mut self, slot: usize) -> u8;
    fn noise(&mut self, s: &Slot, db: f32);
    fn stop_noise(&mut self);
    fn play(&mut self, slot: usize, i: usize, s: &Slot, db: f32) -> bool;
    fn phrase(&mut self, slot: usize, i: usize) -> PhraseState;
    fn stop(&mut self, slot: usize);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Frame {
    pub now: u32,
    pub at: [f32; 3],
    pub level: u32,
    pub outside_ms: i32,
    pub police_car: bool,
    pub blocked: bool,
    pub faded: bool,
    pub repeats: bool,
    pub db: f32,
    pub wagon: Wagon,
    pub in_train: bool,
}

#[derive(Clone, Debug)]
pub struct Scanner {
    pub sampled_at: [f32; 3],
    pub sampled: u32,
    pub not_before: u32,
    pub last_pick: u32,
    pub next: u32,
    pub playing: u32,
    pub last_kind: u32,
    pub last_sub: u32,
    pub last_area: u32,
    pub idle_until: u32,
    pub chatter_at: u32,
    pub allowed: u32,
    pub state: u32,
    pub heading: u32,
    pub last_scripted: u32,
    pub police_car: bool,
    pub cut: bool,
    pub slots: Vec<Slot>,
    pub on: bool,
    pub sent: Vec<Sent>,
    pub described: std::collections::HashMap<u32, [bool; 4]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sent {
    pub station: u32,
    pub units: u32,
    pub kind: u32,
    pub time: u32,
    pub used: bool,
}

impl Default for Scanner {
    fn default() -> Self {
        Scanner {
            sampled_at: [0.0; 3], sampled: 0, not_before: 0, last_pick: 0, next: 0, playing: NONE, last_kind: 0, last_sub: 0,
            last_area: 0, idle_until: 0, chatter_at: 0, allowed: 1, state: 0, heading: 4, last_scripted: 0, police_car: false,
            cut: false, slots: vec![Slot::default(); SLOTS], on: ON, sent: vec![Sent::default(); 32], described: Default::default(),
        }
    }
}

impl Scanner {
    pub fn speaking(&self) -> bool {
        self.state == 2
    }

    fn start_slot(&mut self, n: usize) {
        self.slots[n] = Slot { queued: true, noise: name_hash(NOISE_LOOP), ..Default::default() };
    }

    fn say_place(&mut self, r: &Report, n: usize, street_ok: bool, rng: &mut AudioRand) {
        let p = &r.place;
        let mut said = false;
        let mut out = vec![];
        if p.street == 0 || !street_ok {
            if r.long && rng_chance(rng, STUMBLE_CHANCE) {
                out.push(Phrase { sound: name_hash("POLICE_SCANNER_IN"), gap: IN_GAP_MS, bank: 5 });
                out.push(Phrase { sound: name_hash("POLICE_SCANNER_ERR"), gap: ERR_GAP_MS, bank: 6 });
                said = true;
            }
            if p.dir != 0 {
                out.push(Phrase { sound: if said { p.dir } else { p.in_dir }, gap: 0, bank: 2 });
                said = true;
            }
            if !said {
                out.push(Phrase { sound: name_hash("POLICE_SCANNER_IN"), gap: 0, bank: 5 });
            }
            said = false;
        } else {
            out.push(Phrase { sound: name_hash(if r.long { "POLICE_SCANNER_ON_LONG" } else { "POLICE_SCANNER_ON_SHORT" }), gap: 0, bank: 5 });
            out.push(Phrase { sound: p.street, gap: STREET_GAP_MS, bank: 2 });
            said = true;
        }
        let s = &mut self.slots[n];
        s.phrases.extend(out);
        if !said || self.last_area != p.area || !rng_chance(rng, SAME_AREA_SKIP) {
            let gap = rng.int_in_range(100, 2000) as u32;
            s.phrases.push(Phrase { sound: p.area, gap, bank: 3 });
            s.area = p.area;
        }
    }

    pub fn queue_crime(&mut self, r: &Report, crime: &ScannerCrime, now: u32, street_ok: bool, rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        let draw = |list: &[(u32, f32)], rng: &mut AudioRand| {
            let sum = list.iter().fold(0.0f32, |s, w| s + w.1);
            ScannerCrime::pick(list, rng.float_in_range(0.0, sum))
        };
        let opening = draw(&crime.opening, rng);
        let line = draw(&crime.crime, rng);
        if self.next == self.playing {
            self.next += 1;
        }
        let n = (self.next & 31) as usize;
        self.start_slot(n);
        let s = &mut self.slots[n];
        s.sub = r.sub;
        s.time = r.delay.wrapping_add(now);
        s.priority = r.priority;
        s.flag = r.flag;
        s.phrases = vec![Phrase { sound: opening, gap: 0, bank: 0 }, Phrase { sound: line, gap: 0, bank: 1 }];
        self.say_place(r, n, street_ok, rng);
        self.slots[n].queued = true;
        self.next = (self.next + 1) & 31;
    }

    pub fn pick(&mut self, f: &Frame) -> u32 {
        self.pick_with(f.now, f.repeats, |_, s| may_play(s, f))
    }

    pub fn pick_with(&mut self, now: u32, repeats: bool, may: impl Fn(usize, &Slot) -> bool) -> u32 {
        let mut waiting = false;
        for k in 0..32 {
            let n = ((self.next + k) & 31) as usize;
            let s = &self.slots[n];
            if !(s.queued && (s.priority == 3 || s.priority == 4) && s.time <= now) {
                continue;
            }
            let fits = s.priority == 4
                || (self.last_scripted.wrapping_add(SCRIPTED_GAP_MS) < now
                    && now.wrapping_sub(s.time) < 15_000
                    && (s.kind == 0 || s.kind != self.last_kind || s.sub != self.last_sub));
            if fits && may(n, s) {
                if self.last_pick.wrapping_add(PICK_GAP_MS) < now {
                    return n as u32;
                }
                waiting = true;
            } else {
                let s = &mut self.slots[n];
                s.queued = false;
                s.flag = false;
            }
        }
        if waiting {
            return NONE;
        }
        if self.last_pick.wrapping_add(PICK_GAP_MS) < now {
            for n in 0..32 {
                let s = &self.slots[n];
                if !(s.queued && now.wrapping_sub(s.time) < 7_500) {
                    continue;
                }
                if may(n, s) && (repeats || s.kind != self.last_kind || s.sub != self.last_sub) {
                    self.last_pick = now;
                    return n as u32;
                }
                let s = &mut self.slots[n];
                s.queued = false;
                s.flag = false;
            }
        }
        for k in 0..32 {
            let n = ((self.next + k) & 31) as usize;
            let s = &self.slots[n];
            if s.queued && s.priority == 0 && self.police_car && s.time <= now {
                return n as u32;
            }
        }
        NONE
    }

    fn drop_slot(&mut self, n: u32, v: &mut impl Voice) {
        if let Some(s) = self.slots.get_mut(n as usize) {
            v.stop(n as usize);
            s.queued = false;
            s.flag = false;
        }
    }

    pub fn stop_all(&mut self, v: &mut impl Voice) {
        for n in 0..32 {
            self.drop_slot(n, v);
        }
        self.state = 0;
        self.sampled = 0;
        self.not_before = 0;
        v.stop_noise();
    }

    pub fn cut_off(&mut self, v: &mut impl Voice) {
        match self.state {
            2 => self.cut = true,
            1 => {
                self.drop_slot(self.playing, v);
                self.state = 0;
            }
            _ => {}
        }
    }

    fn phrase_db(&self, n: usize, f: &Frame) -> f32 {
        if self.slots[n].priority == 0 { CHATTER_DB + f.db } else { f.db }
    }

    pub fn frame(&mut self, f: &Frame, rng: &mut AudioRand, v: &mut impl Voice, w: &mut impl World) {
        let now = f.now;
        if self.cut && f.faded {
            if self.state == 2 {
                self.drop_slot(self.playing, v);
            }
            v.stop_noise();
            self.state = 0;
            self.cut = false;
        }
        if !self.on {
            return;
        }
        if f.police_car {
            if !self.police_car {
                self.chatter_at = (rng.int_in_range(5000, 15000) as u32).wrapping_add(now);
            }
            self.police_car = true;
        } else {
            self.police_car = false;
        }
        if f.level == 0 {
            self.allowed = 1;
            if !self.police_car {
                self.last_sub = 0;
            }
            self.idle_until = (rng.int_in_range(5000, 15000) as u32).wrapping_add(now);
        } else if self.last_pick.wrapping_add(60_000) < now {
            self.allowed = u32::MAX;
        }
        self.queued_dispatch(now, f.level, w, rng);
        if self.police_car {
            self.chatter(now, f.level, f.at, f.wagon, w, rng);
        }
        if self.sampled.wrapping_add(HEADING_MS) <= now {
            let d = [f.at[0] - self.sampled_at[0], f.at[1] - self.sampled_at[1], f.at[2] - self.sampled_at[2]];
            self.heading = if d[2].mul_add(d[2], d[0].mul_add(d[0], d[1] * d[1])) <= STANDING_M * STANDING_M { 4 } else { quarter(unit(d)) };
            self.sampled_at = f.at;
            self.sampled = now;
        }
        match self.state {
            0 => {
                self.cut = false;
                if !(self.not_before < now && !f.blocked) {
                    return;
                }
                self.playing = self.pick(f);
                if self.playing == NONE {
                    return;
                }
                let n = self.playing as usize;
                let s = &self.slots[n];
                if s.priority == 0 || s.priority == 4 || s.kind == 8 || (1u64 << (s.kind & 0x3F)) as u32 & self.allowed != 0 {
                    if !v.make(n, &self.slots[n]) || v.loaded(n) == 2 {
                        self.drop_slot(self.playing, v);
                    } else {
                        let s = &self.slots[n];
                        self.last_kind = s.kind;
                        self.last_sub = s.sub;
                        if s.area != 0 {
                            self.last_area = s.area;
                        }
                        self.state = 1;
                        self.allowed = NEXT_KINDS.get(s.kind as usize).copied().unwrap_or(u32::MAX);
                    }
                } else {
                    self.slots[n].queued = false;
                }
            }
            1 => {
                let n = self.playing as usize;
                if self.slots.get(n).is_some_and(|s| s.queued) {
                    match v.loaded(n) {
                        2 => {
                            self.drop_slot(self.playing, v);
                            self.state = 0;
                            return;
                        }
                        1 => {}
                        _ => return,
                    }
                    let noise_db = if self.slots[n].priority == 0 && f.level == 0 { CHATTER_DB + f.db } else { f.db };
                    v.noise(&self.slots[n], noise_db);
                    self.slots[n].at = 0;
                    let db = self.phrase_db(n, f);
                    if !self.slots[n].phrases.is_empty() && v.play(n, 0, &self.slots[n], db) {
                        self.state = 2;
                        let s = &self.slots[n];
                        if s.described != 0 {
                            let d = self.described.entry(s.described).or_default();
                            for k in 0..4 {
                                d[k] |= s.said[k];
                            }
                        }
                        if s.kind == 7 {
                            self.last_scripted = now;
                        }
                        return;
                    }
                }
                self.drop_slot(self.playing, v);
                self.state = 0;
            }
            2 => {
                let n = self.playing as usize;
                let i = self.slots[n].at as usize;
                if v.phrase(n, i) == PhraseState::Playing {
                    return;
                }
                let gap = self.slots[n].phrases.get(i).map_or(0, |p| p.gap);
                if gap != 0 {
                    self.not_before = gap.wrapping_add(now);
                    self.state = 3;
                    return;
                }
                self.slots[n].at += 1;
                let i = self.slots[n].at as usize;
                let db = self.phrase_db(n, f);
                if i >= self.slots[n].phrases.len() || !v.play(n, i, &self.slots[n], db) {
                    v.stop_noise();
                    self.not_before = AFTER_REPORT_MS.wrapping_add(now);
                    let s = &mut self.slots[n];
                    s.queued = false;
                    s.flag = false;
                    self.playing = NONE;
                    self.state = 0;
                }
            }
            3 => {
                if self.not_before < now {
                    let n = self.playing as usize;
                    let i = self.slots[n].at as usize;
                    if let Some(p) = self.slots[n].phrases.get_mut(i) {
                        p.gap = 0;
                    }
                    self.state = 2;
                }
            }
            _ => {}
        }
    }
}

impl Scanner {
    fn take_next(&mut self) -> usize {
        if self.next == self.playing {
            self.next += 1;
        }
        (self.next & 31) as usize
    }

    fn queued(&mut self, n: usize) {
        self.slots[n].queued = true;
        self.next = (self.next + 1) & 31;
    }

    pub fn chatter(&mut self, now: u32, level: u32, at: [f32; 3], wagon: Wagon, w: &mut impl World, rng: &mut AudioRand) {
        self.chatter_with(now, level, at, rng, |s, c, rng| match c {
            Chatter::Dispatch { units, kind, from } => s.dispatch(units, kind, from, now, w, rng),
            Chatter::Assistance(to) => s.assistance(to, 0, now, w, rng),
            Chatter::Chat => s.random_chat(now, rng),
            Chatter::Ambient => s.ambient_crime(at, wagon, now, w, rng),
        });
    }

    pub fn chatter_with(&mut self, now: u32, level: u32, at: [f32; 3], rng: &mut AudioRand, mut make: impl FnMut(&mut Self, Chatter, &mut AudioRand)) {
        if self.chatter_at >= now {
            return;
        }
        let mut draw = rng.int_in_range(-2, 3);
        if level as i32 > 0 {
            draw = 3;
        }
        let c = match draw {
            1 => {
                let units = rng.int_in_range(0, 7) as u32;
                let kind = rng.int_in_range(0, 2) as u32;
                Chatter::Dispatch { units, kind, from: DISPATCH_PLACES[rng.int_in_range(0, 16) as usize] }
            }
            2 => Chatter::Assistance(offset(at, 400.0, 1000.0, rng)),
            3 => Chatter::Chat,
            _ => Chatter::Ambient,
        };
        make(self, c, rng);
        self.chatter_at = (rng.int_in_range(7500, 15000) as u32).wrapping_add(now);
    }

    pub fn random_chat(&mut self, now: u32, rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        let n = self.take_next();
        self.start_slot(n);
        let s = &mut self.slots[n];
        (s.kind, s.time, s.priority) = (6, now, 0);
        let gap = rng.int_in_range(500, 2000) as u32;
        self.slots[n].phrases = vec![Phrase { sound: name_hash("POLICE_SCANNER_RANDOM_CHAT"), gap, bank: 0 }];
        self.queued(n);
    }

    pub fn assistance(&mut self, at: [f32; 3], priority: u32, now: u32, w: &mut impl World, rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        let n = self.take_next();
        let mut place = w.place(at);
        place.street = 0;
        self.start_slot(n);
        let s = &mut self.slots[n];
        (s.kind, s.time, s.priority, s.area) = (1, now, priority, place.area);
        s.phrases = vec![Phrase { sound: name_hash("POLICE_SCANNER_ASSISTANCE_REQUIRED_ASSISTANCE_REQUIRED"), gap: 0, bank: 0 }];
        let r = Report { place, ..Default::default() };
        self.say_place(&r, n, false, rng);
        self.queued(n);
    }

    pub fn dispatch(&mut self, units: u32, kind: u32, from: [f32; 3], now: u32, w: &mut impl World, rng: &mut AudioRand) {
        let n = self.take_next();
        if self.slots[n].queued && self.slots[n].priority >= 2 {
            return;
        }
        let place = w.place(from);
        self.start_slot(n);
        let line = match kind {
            1 => name_hash("POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_SWAT_TEAM_FROM"),
            3 => name_hash("DISPATCH_FIB_TEAM"),
            2 if units == 1 => name_hash("POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_AIR_UNIT_FROM"),
            2 => name_hash("POLICE_SCANNER_DISPATCH_MULTIPLE_AIR_UNITS_FROM"),
            _ => match units.min(7) {
                0 => 0,
                u => name_hash(DISPATCH_UNITS[u as usize - 1]),
            },
        };
        let s = &mut self.slots[n];
        (s.kind, s.sub, s.time, s.area) = (4, kind, now, place.area);
        s.priority = if kind == 2 || units > 3 { 3 } else { 2 };
        let gap = rng.int_in_range(100, 2000) as u32;
        self.slots[n].phrases = vec![Phrase { sound: line, gap: 0, bank: 0 }, Phrase { sound: place.area, gap, bank: 3 }];
        self.queued(n);
    }

    pub fn ambient_crime(&mut self, at: [f32; 3], wagon: Wagon, now: u32, w: &mut impl World, rng: &mut AudioRand) {
        let (to, hash, sub) = ambient_pick(at, wagon, rng);
        let Some(crime) = w.crime(hash) else { return };
        let mut place = w.place(to);
        place.street = 0;
        let r = Report { place, priority: 0, sub, delay: AMBIENT_LEFTOVER_DELAY_MS, ..Default::default() };
        if rng_chance(rng, 0.5) {
            self.car_call(&r, &crime, now, rng);
        } else {
            self.queue_crime(&r, &crime, now, false, rng);
        }
    }

    fn car_call(&mut self, r: &Report, crime: &ScannerCrime, now: u32, rng: &mut AudioRand) {
        self.car_call_for_check(r, crime, now, rng)
    }

    #[doc(hidden)]
    pub fn car_call_for_check(&mut self, r: &Report, crime: &ScannerCrime, now: u32, rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        let draw = |list: &[(u32, f32)], rng: &mut AudioRand| {
            let sum = list.iter().fold(0.0f32, |s, w| s + w.1);
            ScannerCrime::pick(list, rng.float_in_range(0.0, sum))
        };
        draw(&crime.opening, rng);
        let line = draw(&crime.crime, rng);
        let n = self.take_next();
        self.start_slot(n);
        let delay = rng.int_in_range(2000, 4000) as u32;
        let s = &mut self.slots[n];
        (s.kind, s.sub, s.time, s.priority) = (0, r.sub, delay.wrapping_add(now), r.priority);
        let mut p = vec![
            Phrase { sound: name_hash("POLICE_CAR_NAME"), gap: 0, bank: 0 },
            Phrase { sound: name_hash("POLICE_CAR_NUMBER"), gap: 0, bank: 2 },
            Phrase { sound: name_hash("INSTRUCTIONS_DISPATCH_TO"), gap: 0, bank: 7 },
        ];
        if rng_chance(rng, STUMBLE_CHANCE) {
            p[2].gap = IN_GAP_MS;
            p.push(Phrase { sound: name_hash("POLICE_SCANNER_ERR"), gap: ERR_GAP_MS, bank: 6 });
        }
        p.push(Phrase { sound: r.place.area, gap: 0, bank: 3 });
        p.push(Phrase { sound: name_hash("POLICE_SCANNER_CONJUNCTIVES_FOR"), gap: 0, bank: 5 });
        p.push(Phrase { sound: line, gap: 0, bank: 1 });
        self.slots[n].phrases = p;
        self.queued(n);
    }
}

pub fn station_of(x: f32, y: f32) -> u32 {
    let mut best = (0, 1.0e10f32);
    for (k, p) in DISPATCH_PLACES[..16].iter().enumerate() {
        let (dx, dy) = ((x as f64 - p[0] as f64) as f32, (y as f64 - p[1] as f64) as f32);
        let d = dx.mul_add(dx, dy * dy);
        if d < best.1 {
            best = (k as u32, d);
        }
    }
    best.0
}

pub const DISPATCH_BACKUP: &str = "SCRIPTED_REPORTS_DISPATCH_BACKUP";
pub const TRAIN_TERMINATES_HERE: &str = "SCRIPTED_REPORT_TRAIN_TERMINATES_HERE";

pub const VIGILANTE_CRIMES: [&str; 7] = [
    "CRIME_FLEEING_CRIME_SCENE", "CRIME_STEAL_CAR", "CRIME_GUN_SPREE", "CRIME_STEAL_CAR", "CRIME_DRIVEBY", "CRIME_DRUG_DEAL",
    "CRIME_GANG_ACTIVITY",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Description {
    pub prefix: u32,
    pub colour: u32,
    pub manufacturer: u32,
    pub model: u32,
    pub category: u32,
    pub vehicle: u32,
    pub marked: bool,
    pub bike: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Vehicle {
    pub colour: [u32; 2],
    pub words: [u32; 3],
    pub beat_up: bool,
    pub taxi: bool,
    pub service: bool,
    pub id: u32,
    pub marked: bool,
    pub bike: bool,
}

pub fn describe(v: &Vehicle) -> Description {
    let mut d = Description { prefix: v.colour[0], colour: v.colour[1], manufacturer: v.words[0], model: v.words[1], category: v.words[2],
                              vehicle: v.id, marked: v.marked, bike: v.bike };
    if v.beat_up {
        (d.colour, d.prefix) = (name_hash("POLICE_SCANNER_COLOUR_FUCKED"), 0);
    }
    if v.taxi {
        d.category = 0;
    }
    if v.service {
        (d.colour, d.manufacturer, d.prefix, d.category) = (0, 0, 0, 0);
    }
    d
}

pub fn colour_names(carcols: &str) -> Vec<[u32; 2]> {
    let mut out = vec![];
    let mut inside = false;
    for line in carcols.lines() {
        let l = line.trim_start();
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        if !inside {
            inside = l.starts_with("col");
            continue;
        }
        if l.starts_with("end") {
            break;
        }
        let t: Vec<&str> = l.split([',', ' ', '\t', '\r']).filter(|s| !s.is_empty()).collect();
        if out.len() < 196 {
            let word = |k: usize, f: &str| t.get(k).filter(|w| !w.starts_with('-')).map_or(0, |w| name_hash(&format!("{f}{w}")));
            out.push([word(3, "POLICE_SCANNER_PREFIX_"), word(4, "POLICE_SCANNER_COLOUR_")]);
        }
    }
    out
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Suspect {
    pub report: Report,
    pub vehicle: Option<Description>,
    pub priority: u32,
    pub heading: u32,
    pub speed: f32,
    pub extra: bool,
}

impl Scanner {
    pub fn car_sent(&mut self, station: u32, kind: u32, now: u32) {
        if station >= 16 {
            return;
        }
        let mut free = None;
        for (k, r) in self.sent.iter_mut().enumerate() {
            if !r.used {
                free = Some(k);
            } else if r.station == station && r.kind == kind {
                r.units += 1;
                return;
            }
        }
        if let Some(k) = free {
            self.sent[k] = Sent { station, units: 1, kind, time: now, used: true };
        }
    }

    pub fn queued_dispatch(&mut self, now: u32, level: u32, w: &mut impl World, rng: &mut AudioRand) {
        for k in 0..32 {
            let r = self.sent[k];
            if !(r.used && r.time.wrapping_add(2000) < now) {
                continue;
            }
            self.sent[k].used = false;
            if r.station > 15 {
                continue;
            }
            if r.kind == 0 && r.units < 3 && level as i32 > 1 {
                continue;
            }
            self.dispatch(r.units, r.kind, DISPATCH_PLACES[r.station as usize], now, w, rng);
        }
    }

    pub fn report_dispatch(&mut self, units: u32, kind: u32, at: [f32; 3], now: u32, w: &mut impl World, rng: &mut AudioRand) {
        if self.next == self.playing {
            self.next = (self.next + 1) & 31;
        }
        let n = self.next as usize;
        let place = w.place(at);
        self.start_slot(n);
        let line = match kind {
            1 => name_hash("POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_SWAT_TEAM_FROM"),
            3 => name_hash("DISPATCH_FIB_TEAM"),
            2 if units == 1 => name_hash("POLICE_SCANNER_DISPATCH_UNIT_FROM_DFROM_DISPATCH_AIR_UNIT_FROM"),
            2 => name_hash("POLICE_SCANNER_DISPATCH_MULTIPLE_AIR_UNITS_FROM"),
            _ => match units.min(7) {
                0 => 0,
                u => name_hash(DISPATCH_UNITS[u as usize - 1]),
            },
        };
        let s = &mut self.slots[n];
        (s.kind, s.sub, s.time, s.priority, s.flag, s.area) = (4, kind, now, 4, true, place.area);
        let gap = rng.int_in_range(100, 2000) as u32;
        self.slots[n].phrases = vec![Phrase { sound: line, gap: 0, bank: 0 }, Phrase { sound: place.area, gap, bank: 3 }];
        self.queued(n);
    }

    pub fn report_crime_at(&mut self, at: [f32; 3], crime: &ScannerCrime, now: u32, w: &mut impl World, rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        let r = Report { place: w.place(at), priority: 4, flag: true, ..Default::default() };
        self.queue_crime(&r, crime, now, true, rng);
    }

    pub fn vigilante(&mut self, at: [f32; 3], crime: &ScannerCrime, now: u32, w: &mut impl World, rng: &mut AudioRand) {
        self.report_crime_at(at, crime, now, w, rng);
    }

    pub fn scripted(&mut self, name: &str, flags: u32, report: &crate::audioconfig::ScannerScriptedReport, player: [f32; 3], now: u32,
                    w: &mut impl World, rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        let n = self.take_next();
        self.start_slot(n);
        let s = &mut self.slots[n];
        (s.kind, s.sub, s.time, s.priority, s.flag) = (7, name_hash(name), now, 4, true);
        s.noise = if flags & 3 == 1 { name_hash("NULL_SOUND") } else { name_hash(NOISE_LOOP) };
        s.echo = if flags & 0xC == 4 { 100 } else { 0 };
        s.phrases = report.phrases.iter().map(|p| Phrase { sound: p.sound, gap: p.gap_ms as u32, bank: p.slot as u32 }).collect();
        if name.eq_ignore_ascii_case(DISPATCH_BACKUP) {
            let r = Report { place: w.place(player), ..Default::default() };
            self.say_place(&r, n, true, rng);
        }
        if name.eq_ignore_ascii_case(TRAIN_TERMINATES_HERE) {
            let s = &mut self.slots[n];
            (s.sub, s.priority, s.kind) = (1, 3, 8);
        }
        self.queued(n);
    }

    pub fn hospital(&mut self, at: [f32; 3], group: u32, player: [f32; 3], crime: &ScannerCrime, now: u32, w: &mut impl World,
                    rng: &mut AudioRand) {
        let to = offset(player, 400.0, 900.0, rng);
        let mut place = w.place(to);
        place.street = 0;
        let draw = |list: &[(u32, f32)], rng: &mut AudioRand| {
            let sum = list.iter().fold(0.0f32, |s, w| s + w.1);
            ScannerCrime::pick(list, rng.float_in_range(0.0, sum))
        };
        let opening = draw(&crime.opening, rng);
        let line = draw(&crime.crime, rng);
        let n = self.take_next();
        self.start_slot(n);
        let delay = rng.int_in_range(0, 5000) as u32;
        let s = &mut self.slots[n];
        (s.category, s.place, s.group, s.noise) = (name_hash("AMBIENCE_HOSPITAL_SCANNER"), Some(at), group, name_hash("POSITIONED_NOISE_LOOP"));
        (s.kind, s.sub, s.time, s.priority) = (8, 0, delay.wrapping_add(now), 4);
        s.phrases = vec![Phrase { sound: opening, gap: 0, bank: 0 }, Phrase { sound: line, gap: 0, bank: 1 }];
        let r = Report { place, ..Default::default() };
        self.say_place(&r, n, false, rng);
        self.queued(n);
    }

    pub fn spotted(&mut self, at: [f32; 3], moved: bool, extra: bool, vehicle: Option<(Description, f32)>, now: u32, w: &mut impl World,
                   rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        if !moved && !rng_chance(rng, 0.001) {
            if rng_chance(rng, 0.001f32 * 0.3) {
                self.assistance(at, 2, now, w, rng);
            }
            return;
        }
        let place = w.place(at);
        let long = self.last_area != place.area && rng_chance(rng, 1.0);
        let s = Suspect {
            report: Report { place, long, ..Default::default() },
            vehicle: vehicle.map(|v| v.0),
            priority: if moved { 3 } else { 1 },
            heading: self.heading,
            speed: vehicle.map_or(0.0, |v| v.1),
            extra,
        };
        self.suspect(&s, now, rng);
    }

    pub fn suspect(&mut self, r: &Suspect, now: u32, rng: &mut AudioRand) {
        let n = self.take_next();
        if self.slots[n].queued && self.slots[n].priority > 1 {
            return;
        }
        self.start_slot(n);
        let s = &mut self.slots[n];
        (s.kind, s.time, s.priority) = (2, now, r.priority);
        let mut p = vec![Phrase { sound: name_hash("POLICE_SCANNER_SUSPECT"), gap: 0, bank: 4 }];
        let sub;
        if r.speed > 50.0 && rng_chance(rng, 0.75) {
            let mph = (r.speed as i64 as u32) / 10 * 10 & !1;
            p.push(Phrase { sound: name_hash(&format!("SPEED_{mph}")), gap: 0, bank: 0 });
            sub = 1;
        } else if r.heading != 4 && rng_chance(rng, 0.9) {
            let h = ["POLICE_SCANNER_HEADING_NORTH", "POLICE_SCANNER_HEADING_EAST", "POLICE_SCANNER_HEADING_SOUTH", "POLICE_SCANNER_HEADING_WEST"];
            p.push(Phrase { sound: h.get(r.heading as usize).map_or(0, |h| name_hash(h)), gap: 0, bank: 0 });
            sub = 2;
        } else {
            p.push(Phrase { sound: name_hash("POLICE_SCANNER_LAST_SEEN"), gap: 0, bank: 0 });
            sub = 3;
        }
        self.slots[n].sub = sub;
        self.slots[n].phrases = p;
        let d = r.vehicle.unwrap_or_default();
        let known = |k: usize| d.vehicle != 0 && self.described.get(&d.vehicle).is_some_and(|s| s[k]);
        let place_only = !r.extra && (!d.marked || !rng_chance(rng, 0.85) || known(0));
        if place_only {
            self.say_place(&r.report, n, true, rng);
        } else {
            let null = name_hash("NULL_SOUND");
            let mut p = std::mem::take(&mut self.slots[n].phrases);
            let mut said = [false; 4];
            if r.vehicle.is_none() {
                let gap = rng.int_in_range(100, 2000) as u32;
                p.push(Phrase { sound: name_hash("POLICE_SCANNER_ON_FOOT"), gap, bank: 3 });
                self.slots[n].on_foot = true;
            } else {
                p.push(Phrase { sound: name_hash(if d.bike { "POLICE_SCANNER_ON_SHORT" } else { "POLICE_SCANNER_IN" }), gap: 0, bank: 5 });
                p.push(Phrase { sound: name_hash("POLICE_SCANNER_A"), gap: 0, bank: 6 });
                let mut no_prefix = true;
                if d.prefix != 0 && rng_chance(rng, 0.3) {
                    p.push(Phrase { sound: d.prefix, gap: 0, bank: 3 });
                    no_prefix = false;
                }
                if d.colour != 0 {
                    p.push(Phrase { sound: d.colour, gap: 0, bank: 1 });
                    said[2] = true;
                }
                if d.manufacturer == 0 || d.manufacturer == null || (d.vehicle != 0 && d.category != 0 && !known(3) && rng_chance(rng, 0.8)) {
                    let gap = rng.int_in_range(400, 2000) as u32;
                    p.push(Phrase { sound: d.category, gap, bank: 2 });
                    no_prefix = false;
                    said[3] = true;
                } else {
                    p.push(Phrase { sound: d.manufacturer, gap: 0, bank: 2 });
                    said[1] = true;
                }
                if no_prefix {
                    let model = d.category == null || rng_chance(rng, 0.5) || (d.model != null && d.vehicle != 0 && known(3));
                    let gap = rng.int_in_range(100, 2000) as u32;
                    if model {
                        p.push(Phrase { sound: d.model, gap, bank: 3 });
                        said[0] = true;
                    } else {
                        p.push(Phrase { sound: d.category, gap, bank: 3 });
                    }
                    said[3] = true;
                } else {
                    let gap = rng.int_in_range(100, 2000) as u32;
                    if let Some(last) = p.last_mut() {
                        last.gap = gap;
                    }
                }
                self.slots[n].on_foot = false;
                self.slots[n].described = d.vehicle;
            }
            self.slots[n].phrases = p;
            self.slots[n].said = said;
            self.slots[n].sub = 1;
        }
        self.queued(n);
    }
}

pub fn may_play(s: &Slot, f: &Frame) -> bool {
    if s.kind == 8 && ((s.sub == 1) != f.in_train || f.now > s.time.wrapping_add(4000)) {
        return false;
    }
    if s.priority == 4 {
        return true;
    }
    !s.vehicle && ((f.level != 0 && f.outside_ms < 1) || s.priority == 0 || s.kind == 5)
}

pub fn rng_chance(rng: &mut AudioRand, p: f32) -> bool {
    if p >= 1.0 {
        return true;
    }
    let x = (rng.value as u64).wrapping_mul(0x5CDC_FAA7).wrapping_add(rng.carry as u64);
    (rng.value, rng.carry) = (x as u32, (x >> 32) as u32);
    ((rng.value & 0x7F_FFFF) as f32) * (1.0 / 8_388_608.0) < p
}

