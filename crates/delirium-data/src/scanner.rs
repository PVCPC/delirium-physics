use crate::audioconfig::{name_hash, ScannerCrime};
use std::collections::BTreeMap;

use crate::policescanner::{
    self as exe, ChaseEnd, Description, Name, NoiseRequest, PhraseRequest, PlaceWords, Said, ScannerGame, SlotNo, SoundOut,
    SuspectReport,
};
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

pub trait ScannerWorld {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SoundHandle(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Made {
    pub sound: Option<SoundHandle>,
    pub echo: Option<SoundHandle>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slot {
    pub time: u32,
    pub priority: u32,
    pub kind: u32,
    pub sub: u32,
    pub area: u32,
    pub at: u32,
    pub phrases: [Phrase; PHRASES],
    pub phrase_count: u32,
    pub made: [Made; PHRASES],
    pub stray_phrases: BTreeMap<u32, Phrase>,
    pub stray_made: BTreeMap<u32, Made>,
    pub predelay: u32,
    pub radio: bool,
    pub queued: ByteFlag,
    pub flag: ByteFlag,
    pub vehicle: bool,
    pub place: Option<[f32; 3]>,
    pub group: u32,
    pub category: u32,
    pub noise: u32,
    pub echo: u32,
    pub described: u32,
    pub said: [bool; 4],
    pub on_foot: bool,
    pub follow: Option<u32>,
}

pub const PHRASES: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ByteFlag(pub u8);

impl ByteFlag {
    pub const ON: ByteFlag = ByteFlag(1);
    pub const OFF: ByteFlag = ByteFlag(0);
    pub fn on(self) -> bool {
        self.0 != 0
    }
}

impl From<bool> for ByteFlag {
    fn from(on: bool) -> Self {
        ByteFlag(on as u8)
    }
}

impl Slot {
    pub fn phrase(&self, k: u32) -> Phrase {
        match self.phrases.get(k as usize) {
            Some(p) => *p,
            None => self.stray_phrases.get(&k).copied().unwrap_or_default(),
        }
    }
    pub fn phrase_mut(&mut self, k: u32) -> &mut Phrase {
        match self.phrases.get_mut(k as usize) {
            Some(p) => p,
            None => self.stray_phrases.entry(k).or_default(),
        }
    }
    pub fn made(&self, k: u32) -> Made {
        match self.made.get(k as usize) {
            Some(m) => *m,
            None => self.stray_made.get(&k).copied().unwrap_or_default(),
        }
    }
    pub fn made_mut(&mut self, k: u32) -> &mut Made {
        match self.made.get_mut(k as usize) {
            Some(m) => m,
            None => self.stray_made.entry(k).or_default(),
        }
    }
    pub fn phrase_list(&self) -> &[Phrase] {
        &self.phrases[..(self.phrase_count as usize).min(PHRASES)]
    }
    pub fn set_phrases(&mut self, list: &[Phrase]) {
        let n = list.len().min(PHRASES);
        self.phrases[..n].copy_from_slice(&list[..n]);
        self.phrase_count = n as u32;
    }
    pub fn described(&self) -> Option<u32> {
        (self.described != 0).then_some(self.described)
    }
    pub fn said(&self, part: Said) -> bool {
        self.said[match part {
            Said::Model => 0,
            Said::Manufacturer => 1,
            Said::Colour => 2,
            Said::Category => 3,
        }]
    }
    pub fn placed(&self) -> bool {
        self.place.is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashedNames<const N: usize> {
    pub made: u32,
    pub hashes: [u32; N],
    pub stray: BTreeMap<u32, u32>,
}

impl<const N: usize> Default for HashedNames<N> {
    fn default() -> Self {
        HashedNames { made: 0, hashes: [0; N], stray: BTreeMap::new() }
    }
}

impl<const N: usize> HashedNames<N> {
    pub fn hash(&self, k: u32) -> u32 {
        match self.hashes.get(k as usize) {
            Some(&h) => h,
            None => self.stray.get(&k).copied().unwrap_or(0),
        }
    }
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
    let a = heading_angle(n[0].mul_add(0.0, z0) + n[1]);
    let mut deg = a * 57.295_776;
    if n[1].mul_add(0.0, z0) + n[0] <= 0.0 {
        deg = 360.0 - deg;
    }
    ((deg + 45.0) * 0.011_111_111) as i64 as u32 & 3
}

pub fn heading_angle(c: f32) -> f32 {
    if c <= -1.0 {
        std::f32::consts::PI
    } else if c >= 1.0 {
        0.0
    } else {
        crate::mathrt::acos(c as f64) as f32
    }
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let r = crate::estimate::vrsqrtefp(crate::estimate::vmsum3(v.map(|c| c as f64 * c as f64)));
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

#[derive(Clone, Copy, Debug, Default)]
pub struct Report {
    pub place: Place,
    pub long: bool,
    pub priority: u32,
    pub sound: u32,
    pub kind: u32,
    pub delay: u32,
    pub sub: u32,
    pub flag: bool,
}

impl Report {
    pub fn scripted(sound: u32, kind: u32) -> Report {
        Report { priority: 4, sound, kind, flag: true, ..Report::default() }
    }
}

#[derive(Clone, Debug)]
pub struct PoliceScanner {
    pub sampled_at: [f32; 4],
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
    pub police_car: ByteFlag,
    pub cut: ByteFlag,
    pub slots: Vec<Slot>,
    pub stray_slots: BTreeMap<SlotNo, Slot>,
    pub on: bool,
    pub sent: Vec<Sent>,
    pub noise_sound: Option<SoundHandle>,
    pub described: std::collections::HashMap<u32, [bool; 4]>,
    pub custody_names: HashedNames<2>,
    pub down_names: HashedNames<2>,
    pub vigilante_names: HashedNames<7>,
    pub beaten_up_name: HashedNames<1>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sent {
    pub station: u32,
    pub units: u32,
    pub kind: u32,
    pub time: u32,
    pub used: ByteFlag,
}

impl Default for PoliceScanner {
    fn default() -> Self {
        PoliceScanner {
            sampled_at: [0.0; 4], sampled: 0, not_before: 0, last_pick: 0, next: 0, playing: NONE, last_kind: 0, last_sub: 0,
            last_area: 0, idle_until: 0, chatter_at: 0, allowed: 1, state: 0, heading: 4, last_scripted: 0, police_car: ByteFlag::OFF,
            cut: ByteFlag::OFF, slots: vec![Slot::default(); SLOTS], stray_slots: BTreeMap::new(), on: ON, sent: vec![Sent::default(); 32],
            noise_sound: None, described: Default::default(), custody_names: HashedNames::default(),
            down_names: HashedNames::default(), vigilante_names: HashedNames::default(), beaten_up_name: HashedNames::default(),
        }
    }
}

pub fn platform_gate(in_area_2: bool, inside: bool, open_room: bool, at: [f32; 3]) -> bool {
    if !in_area_2 {
        return !inside;
    }
    if inside {
        return open_room;
    }
    let d = [at[0] - -368.0, at[1] - 1414.0, at[2] - 28.0];
    d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2])) < 4900.0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlatformStation {
    pub at: [f32; 3],
    pub name: Option<u32>,
}

impl PoliceScanner {
    pub fn slot(&self, s: SlotNo) -> &Slot {
        static EMPTY: std::sync::OnceLock<Slot> = std::sync::OnceLock::new();
        match self.slots.get(s as usize) {
            Some(slot) => slot,
            None => self.stray_slots.get(&s).unwrap_or_else(|| EMPTY.get_or_init(Slot::default)),
        }
    }
    pub fn slot_mut(&mut self, s: SlotNo) -> &mut Slot {
        match self.slots.get_mut(s as usize) {
            Some(slot) => slot,
            None => self.stray_slots.entry(s).or_default(),
        }
    }

    pub fn chase_names(&self, end: ChaseEnd) -> &HashedNames<2> {
        match end {
            ChaseEnd::Custody => &self.custody_names,
            ChaseEnd::Down => &self.down_names,
        }
    }
    pub fn chase_names_mut(&mut self, end: ChaseEnd) -> &mut HashedNames<2> {
        match end {
            ChaseEnd::Custody => &mut self.custody_names,
            ChaseEnd::Down => &mut self.down_names,
        }
    }

    pub fn let_go_ended(&mut self, alive: impl Fn(SoundHandle) -> bool) {
        let keep = |h: &mut Option<SoundHandle>| {
            if h.is_some_and(|h| !alive(h)) {
                *h = None;
            }
        };
        keep(&mut self.noise_sound);
        for slot in self.slots.iter_mut().chain(self.stray_slots.values_mut()) {
            for m in slot.made.iter_mut().chain(slot.stray_made.values_mut()) {
                keep(&mut m.sound);
                keep(&mut m.echo);
            }
        }
    }

    pub fn train_slot(&mut self) -> &mut Slot {
        if self.next == self.playing {
            self.next += 1;
        }
        let k = self.next as usize % self.slots.len().max(1);
        self.next = (self.next + 1) & 31;
        let old = std::mem::take(&mut self.slots[k]);
        self.slots[k] = Slot {
            priority: 4, kind: 8, queued: ByteFlag::ON, category: crate::audioconfig::name_hash("GENERAL_TRAIN_ANNOUNCER"),
            noise: crate::audioconfig::name_hash("NULL_SOUND"), vehicle: old.vehicle, described: old.described, said: old.said,
            on_foot: old.on_foot, ..Slot::default()
        };
        &mut self.slots[k]
    }

    pub fn platform(&mut self, gate: bool, tracks: [&[PlatformStation]; 2], at: [f32; 3], now: u32, rng: &mut AudioRand) -> bool {
        if !gate {
            return false;
        }
        let mut best: (u32, Option<&PlatformStation>) = (u32::MAX, None);
        for list in tracks {
            for st in list {
                let d = [at[0] - st.at[0], at[1] - st.at[1], at[2] - st.at[2]];
                let d2 = d[1].mul_add(d[1], d[2].mul_add(d[2], d[0] * d[0]));
                let n = d2 as i64 as u32;
                if n < best.0 {
                    best = (n, Some(st));
                }
            }
        }
        let (Some(st), true) = (best.1, best.0 < 10000) else { return false };
        let st = *st;
        let warning = st.name.is_none() || rng.chance(0.7);
        let phrases = match st.name.filter(|_| !warning) {
            Some(name) => vec![Phrase { sound: crate::audioconfig::name_hash("ANNOUNCE_ARRIVED"), gap: 0, bank: 2 }, Phrase { sound: name, gap: 0, bank: 1 }],
            None => {
                let n = rng.int_in_range(1, 15);
                vec![Phrase { sound: crate::audioconfig::name_hash(&format!("POLICE_SCANNER_WARNINGS_SUBWAY_WARNING_{n:02}")), gap: 0, bank: 0 }]
            }
        };
        let slot = self.train_slot();
        slot.set_phrases(&phrases);
        (slot.place, slot.echo, slot.group, slot.time) = (Some([st.at[0], st.at[1], st.at[2] + 1.5]), 100, 0, now);
        true
    }

    pub fn speaking(&self) -> bool {
        self.state == 2
    }

    fn start_slot(&mut self, n: usize) {
        self.slots[n] = Slot { queued: ByteFlag::ON, noise: name_hash(NOISE_LOOP), predelay: PREDELAY_MS, radio: true, ..Default::default() };
    }

    fn say_place(&mut self, r: &Report, n: usize, street_ok: bool, rng: &mut AudioRand) {
        let mut count = self.slots[n].phrase_count;
        exe::say_place(self, &mut Placer { rng, street_ok }, r, n as u32, &mut count);
        self.slots[n].phrase_count = count;
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
        s.flag = r.flag.into();
        s.set_phrases(&[Phrase { sound: opening, gap: 0, bank: 0 }, Phrase { sound: line, gap: 0, bank: 1 }]);
        self.say_place(r, n, street_ok, rng);
        self.slots[n].queued = ByteFlag::ON;
        self.next = (self.next + 1) & 31;
    }

    pub fn pick_with(&mut self, now: u32, repeats: bool, may: impl Fn(usize, &Slot) -> bool) -> u32 {
        let mut waiting = false;
        for k in 0..32 {
            let n = ((self.next + k) & 31) as usize;
            let s = &self.slots[n];
            if !(s.queued.on() && (s.priority == 3 || s.priority == 4) && s.time <= now) {
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
                s.queued = ByteFlag::OFF;
                s.flag = ByteFlag::OFF;
            }
        }
        if waiting {
            return NONE;
        }
        if self.last_pick.wrapping_add(PICK_GAP_MS) < now {
            for n in 0..32 {
                let s = &self.slots[n];
                if !(s.queued.on() && now.wrapping_sub(s.time) < 7_500) {
                    continue;
                }
                if may(n, s) && (repeats || s.kind != self.last_kind || s.sub != self.last_sub) {
                    self.last_pick = now;
                    return n as u32;
                }
                let s = &mut self.slots[n];
                s.queued = ByteFlag::OFF;
                s.flag = ByteFlag::OFF;
            }
        }
        for k in 0..32 {
            let n = ((self.next + k) & 31) as usize;
            let s = &self.slots[n];
            if s.queued.on() && s.priority == 0 && self.police_car.on() && s.time <= now {
                return n as u32;
            }
        }
        NONE
    }
}

impl PoliceScanner {
    fn take_next(&mut self) -> usize {
        if self.next == self.playing {
            self.next += 1;
        }
        (self.next & 31) as usize
    }

    fn queued(&mut self, n: usize) {
        self.slots[n].queued = ByteFlag::ON;
        self.next = (self.next + 1) & 31;
    }

    pub fn chatter(&mut self, now: u32, level: u32, at: [f32; 3], wagon: Wagon, w: &mut impl ScannerWorld, rng: &mut AudioRand) {
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
        self.slots[n].set_phrases(&[Phrase { sound: name_hash("POLICE_SCANNER_RANDOM_CHAT"), gap, bank: 0 }]);
        self.queued(n);
    }

    pub fn assistance(&mut self, at: [f32; 3], priority: u32, now: u32, w: &mut impl ScannerWorld, rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        let n = self.take_next();
        let mut place = w.place(at);
        place.street = 0;
        self.start_slot(n);
        let s = &mut self.slots[n];
        (s.kind, s.time, s.priority, s.area) = (1, now, priority, place.area);
        s.set_phrases(&[Phrase { sound: name_hash("POLICE_SCANNER_ASSISTANCE_REQUIRED_ASSISTANCE_REQUIRED"), gap: 0, bank: 0 }]);
        let r = Report { place, ..Default::default() };
        self.say_place(&r, n, false, rng);
        self.queued(n);
    }

    pub fn dispatch(&mut self, units: u32, kind: u32, from: [f32; 3], now: u32, w: &mut impl ScannerWorld, rng: &mut AudioRand) {
        let n = self.take_next();
        if self.slots[n].queued.on() && self.slots[n].priority >= 2 {
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
        self.slots[n].set_phrases(&[Phrase { sound: line, gap: 0, bank: 0 }, Phrase { sound: place.area, gap, bank: 3 }]);
        self.queued(n);
    }

    pub fn ambient_crime(&mut self, at: [f32; 3], wagon: Wagon, now: u32, w: &mut impl ScannerWorld, rng: &mut AudioRand) {
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
        self.slots[n].set_phrases(&p);
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
    pub vehicle: Option<Description<u32>>,
    pub priority: u32,
    pub heading: u32,
    pub speed: f32,
    pub extra: bool,
}

impl PoliceScanner {
    pub fn car_sent(&mut self, station: u32, kind: u32, now: u32) {
        if station >= 16 {
            return;
        }
        let mut free = None;
        for (k, r) in self.sent.iter_mut().enumerate() {
            if !r.used.on() {
                free = Some(k);
            } else if r.station == station && r.kind == kind {
                r.units += 1;
                return;
            }
        }
        if let Some(k) = free {
            self.sent[k] = Sent { station, units: 1, kind, time: now, used: ByteFlag::ON };
        }
    }

    pub fn report_dispatch(&mut self, units: u32, kind: u32, at: [f32; 3], now: u32, w: &mut impl ScannerWorld, rng: &mut AudioRand) {
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
        (s.kind, s.sub, s.time, s.priority, s.flag, s.area) = (4, kind, now, 4, ByteFlag::ON, place.area);
        let gap = rng.int_in_range(100, 2000) as u32;
        self.slots[n].set_phrases(&[Phrase { sound: line, gap: 0, bank: 0 }, Phrase { sound: place.area, gap, bank: 3 }]);
        self.queued(n);
    }

    pub fn scripted(&mut self, name: &str, flags: u32, report: &crate::audioconfig::ScannerScriptedReport, player: [f32; 3], now: u32,
                    w: &mut impl ScannerWorld, rng: &mut AudioRand) {
        if !self.on {
            return;
        }
        let n = self.take_next();
        self.start_slot(n);
        let s = &mut self.slots[n];
        (s.kind, s.sub, s.time, s.priority, s.flag) = (7, name_hash(name), now, 4, ByteFlag::ON);
        s.noise = if flags & 3 == 1 { name_hash("NULL_SOUND") } else { name_hash(NOISE_LOOP) };
        s.echo = if flags & 0xC == 4 { 100 } else { 0 };
        s.set_phrases(&report.phrases.iter().map(|p| Phrase { sound: p.sound, gap: p.gap_ms as u32, bank: p.slot as u32 }).collect::<Vec<_>>());
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

    pub fn hospital(&mut self, at: [f32; 3], group: u32, player: [f32; 3], crime: &ScannerCrime, now: u32, w: &mut impl ScannerWorld,
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
        s.set_phrases(&[Phrase { sound: opening, gap: 0, bank: 0 }, Phrase { sound: line, gap: 0, bank: 1 }]);
        let r = Report { place, ..Default::default() };
        self.say_place(&r, n, false, rng);
        self.queued(n);
    }

    pub fn suspect(&mut self, r: &Suspect, now: u32, rng: &mut AudioRand) {
        let n = self.take_next();
        if self.slots[n].queued.on() && self.slots[n].priority > 1 {
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
        self.slots[n].set_phrases(&p);
        let d = r.vehicle.unwrap_or_default();
        let known = |k: usize| d.audio != 0 && self.described.get(&d.audio).is_some_and(|s| s[k]);
        let place_only = !r.extra && (!d.marked || !rng_chance(rng, 0.85) || known(0));
        if place_only {
            self.say_place(&r.report, n, true, rng);
        } else {
            let null = name_hash("NULL_SOUND");
            let mut p = self.slots[n].phrase_list().to_vec();
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
                if d.manufacturer == 0 || d.manufacturer == null || (d.audio != 0 && d.category != 0 && !known(3) && rng_chance(rng, 0.8)) {
                    let gap = rng.int_in_range(400, 2000) as u32;
                    p.push(Phrase { sound: d.category, gap, bank: 2 });
                    no_prefix = false;
                    said[3] = true;
                } else {
                    p.push(Phrase { sound: d.manufacturer, gap: 0, bank: 2 });
                    said[1] = true;
                }
                if no_prefix {
                    let model = d.category == null || rng_chance(rng, 0.5) || (d.model != null && d.audio != 0 && known(3));
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
                self.slots[n].described = d.audio;
            }
            self.slots[n].set_phrases(&p);
            self.slots[n].said = said;
            self.slots[n].sub = 1;
        }
        self.queued(n);
    }
}

pub fn rng_chance(rng: &mut AudioRand, p: f32) -> bool {
    if p >= 1.0 {
        return true;
    }
    let x = (rng.value as u64).wrapping_mul(0x5CDC_FAA7).wrapping_add(rng.carry as u64);
    (rng.value, rng.carry) = (x as u32, (x >> 32) as u32);
    ((rng.value & 0x7F_FFFF) as f32) * (1.0 / 8_388_608.0) < p
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VehicleFacts {
    pub id: u32,
    pub model: i32,
    pub colour: [u32; 2],
    pub manufacturer: u32,
    pub model_word: u32,
    pub category: u32,
    pub beaten_up: bool,
    pub taxi: bool,
    pub emergency: bool,
    pub marked: bool,
    pub kind: i32,
    pub radio_on: bool,
    pub police: bool,
    pub velocity: [f32; 3],
    pub forward: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surround {
    pub now: u32,
    pub player: Option<[f32; 3]>,
    pub vehicle: Option<VehicleFacts>,
    pub level: i32,
    pub multiplier: f32,
    pub fake_level: i32,
    pub out_of_sight_ms: i32,
    pub on_train: bool,
    pub paused: bool,
    pub held: bool,
    pub fade: f32,
    pub repeats: bool,
    pub radio_levels: [f32; 3],
    pub frame_secs: f32,
    pub street_ok: bool,
    pub ambulance_model: i32,
    pub firetruck_model: i32,
}

impl Surround {
    pub fn wagon(&self) -> Wagon {
        match self.vehicle {
            Some(v) if v.model == self.ambulance_model => Wagon::Ambulance,
            Some(v) if v.model == self.firetruck_model => Wagon::FireTruck,
            _ => Wagon::Other,
        }
    }
}

pub trait Sounds {
    fn make(&mut self, name: u32, start_ms: u32) -> Option<SoundHandle>;
    fn alive(&self, sound: SoundHandle) -> bool;
    fn stop(&mut self, sound: SoundHandle);
    fn play(&mut self, sound: SoundHandle);
    fn set_volume(&mut self, sound: SoundHandle, db: f32);
    fn place(&mut self, sound: SoundHandle, at: Option<[f32; 3]>);
    fn load_state(&mut self, sound: Option<SoundHandle>) -> i32;
    fn position(&mut self, sound: SoundHandle) -> u32;
    fn length(&self, sound: SoundHandle) -> i32;
}

pub struct Placer<'a> {
    pub rng: &'a mut AudioRand,
    pub street_ok: bool,
}

macro_rules! place_tunables {
    () => {
        fn null_sound(&self) -> u32 {
            name_hash("NULL_SOUND")
        }
        fn in_gap_ms(&self) -> u32 {
            IN_GAP_MS
        }
        fn err_gap_ms(&self) -> u32 {
            ERR_GAP_MS
        }
        fn on_gap_ms(&self) -> u32 {
            STREET_GAP_MS
        }
        fn in_chance(&self) -> f32 {
            STUMBLE_CHANCE
        }
        fn skip_area_chance(&self) -> f32 {
            SAME_AREA_SKIP
        }
    };
}

impl PlaceWords for Placer<'_> {
    fn hash_name(&mut self, _sc: &mut PoliceScanner, name: Name) -> u32 {
        name_hash(name.text())
    }
    fn uniform(&mut self, _sc: &mut PoliceScanner, lo: u32, hi: u32) -> u32 {
        self.rng.int_in_range(lo as i32, hi as i32) as u32
    }
    fn chance(&mut self, _sc: &mut PoliceScanner, p: f32) -> bool {
        rng_chance(self.rng, p)
    }
    fn street_known(&mut self, _sc: &mut PoliceScanner, _street: u32) -> bool {
        self.street_ok
    }
    place_tunables!();
}

pub struct Game<'a, V: Sounds, W: ScannerWorld> {
    pub f: &'a Surround,
    pub rng: &'a mut AudioRand,
    pub sounds: &'a mut V,
    pub world: &'a mut W,
    pub report: Place,
}

impl<'a, V: Sounds, W: ScannerWorld> Game<'a, V, W> {
    pub fn new(f: &'a Surround, rng: &'a mut AudioRand, sounds: &'a mut V, world: &'a mut W) -> Self {
        Game { f, rng, sounds, world, report: Place::default() }
    }

    fn facts(&self, id: u32) -> Option<VehicleFacts> {
        self.f.vehicle.filter(|v| v.id == id)
    }

    pub fn let_go_ended(&self, sc: &mut PoliceScanner) {
        sc.let_go_ended(|h| self.sounds.alive(h));
    }
}

fn said_index(part: Said) -> usize {
    match part {
        Said::Model => 0,
        Said::Manufacturer => 1,
        Said::Colour => 2,
        Said::Category => 3,
    }
}

impl<V: Sounds, W: ScannerWorld> PlaceWords for Game<'_, V, W> {
    fn hash_name(&mut self, _sc: &mut PoliceScanner, name: Name) -> u32 {
        name_hash(name.text())
    }
    fn uniform(&mut self, _sc: &mut PoliceScanner, lo: u32, hi: u32) -> u32 {
        self.rng.int_in_range(lo as i32, hi as i32) as u32
    }
    fn chance(&mut self, _sc: &mut PoliceScanner, p: f32) -> bool {
        rng_chance(self.rng, p)
    }
    fn street_known(&mut self, _sc: &mut PoliceScanner, _street: u32) -> bool {
        self.f.street_ok
    }
    place_tunables!();
}

impl<V: Sounds, W: ScannerWorld> ScannerGame for Game<'_, V, W> {
    type Ped = ();
    type Vehicle = VehicleFacts;
    type Position = [f32; 3];
    type Bank = u32;
    type CrimeName = u32;

    fn now(&self) -> u32 {
        self.f.now
    }
    fn zero(&self) -> f32 {
        0.0
    }
    fn fade_out_db(&self) -> f32 {
        FADE_OUT_DB
    }
    fn fade_limit(&self) -> f32 {
        FADE_LIMIT
    }
    fn heading_interval(&self) -> u32 {
        HEADING_MS
    }
    fn moving_distance(&self) -> f32 {
        STANDING_M
    }
    fn overlap_ms(&self) -> u32 {
        NEAR_END_MS
    }
    fn report_pause_ms(&self) -> u32 {
        AFTER_REPORT_MS
    }
    fn chatter_db(&self) -> f32 {
        CHATTER_DB
    }
    fn area_again_chance(&self) -> f32 {
        AREA_AGAIN_CHANCE
    }
    fn radio_levels(&self) -> [f32; 3] {
        self.f.radio_levels
    }
    fn heading_constants(&self) -> [f32; 4] {
        HEADING_CONSTANTS
    }
    fn speed_scale(&self) -> [f32; 2] {
        [TO_MPH, MPH_CAP]
    }
    fn lead_factors(&self) -> [f32; 2] {
        [self.f.frame_secs, -1000.0]
    }
    fn allowed_after(&self, kind: u32) -> u32 {
        NEXT_KINDS.get(kind as usize).copied().unwrap_or(u32::MAX)
    }
    fn has_player(&self) -> bool {
        self.f.player.is_some()
    }
    fn paused(&self) -> bool {
        self.f.paused
    }
    fn bank_of(&self, slot: u32) -> u32 {
        slot
    }

    fn player_wanted_level(&mut self, _sc: &mut PoliceScanner) -> i32 {
        self.f.level
    }
    fn player_wanted_multiplier(&mut self, _sc: &mut PoliceScanner) -> f32 {
        self.f.multiplier
    }
    fn player_ped(&mut self, _sc: &mut PoliceScanner) {}
    fn player_position(&mut self, _sc: &mut PoliceScanner) -> [f32; 4] {
        let [x, y, z] = self.f.player.unwrap_or_default();
        [x, y, z, 0.0]
    }
    fn player_vehicle(&mut self, _sc: &mut PoliceScanner) -> Option<VehicleFacts> {
        self.f.vehicle
    }
    fn player_on_train(&mut self, _sc: &mut PoliceScanner) -> bool {
        self.f.on_train
    }
    fn out_of_sight(&mut self, _sc: &mut PoliceScanner) -> i32 {
        self.f.out_of_sight_ms
    }

    fn ped_position(&mut self, _sc: &mut PoliceScanner, _ped: ()) -> [f32; 3] {
        self.f.player.unwrap_or_default()
    }
    fn ped_in_vehicle(&self, _ped: ()) -> bool {
        self.f.vehicle.is_some()
    }
    fn ped_vehicle(&self, _ped: ()) -> Option<VehicleFacts> {
        self.f.vehicle
    }
    fn ped_task_vehicle(&mut self, _sc: &mut PoliceScanner, _ped: ()) -> Option<VehicleFacts> {
        None
    }
    fn vehicle_audio(&self, v: VehicleFacts) -> u32 {
        v.id
    }
    fn vehicle_model(&self, v: VehicleFacts) -> i32 {
        v.model
    }
    fn ambulance_model(&self) -> i32 {
        self.f.ambulance_model
    }
    fn firetruck_model(&self) -> i32 {
        self.f.firetruck_model
    }
    fn vehicle_radio_on(&self, v: VehicleFacts) -> bool {
        v.radio_on
    }
    fn vehicle_marked(&self, v: VehicleFacts) -> bool {
        v.marked
    }
    fn vehicle_emergency(&self, v: VehicleFacts) -> bool {
        v.emergency
    }
    fn vehicle_type(&self, v: VehicleFacts) -> i32 {
        v.kind
    }
    fn colour_words(&self, v: VehicleFacts) -> [u32; 2] {
        v.colour
    }
    fn manufacturer_word(&self, v: VehicleFacts, _boat: bool) -> u32 {
        v.manufacturer
    }
    fn model_word(&self, v: VehicleFacts, _boat: bool) -> u32 {
        v.model_word
    }
    fn category_word(&self, v: VehicleFacts, _boat: bool) -> u32 {
        v.category
    }
    fn vehicle_velocity(&mut self, _sc: &mut PoliceScanner, v: VehicleFacts) -> [f32; 3] {
        v.velocity
    }
    fn vehicle_forward(&self, v: VehicleFacts) -> [f32; 3] {
        v.forward
    }
    fn is_police_audio(&mut self, _sc: &mut PoliceScanner, audio: u32) -> bool {
        self.facts(audio).is_some_and(|v| v.police)
    }
    fn is_beaten_up(&mut self, _sc: &mut PoliceScanner, audio: u32) -> bool {
        self.facts(audio).is_some_and(|v| v.beaten_up)
    }
    fn is_taxi(&mut self, _sc: &mut PoliceScanner, model: i32) -> bool {
        self.f.vehicle.is_some_and(|v| v.model == model && v.taxi)
    }
    fn note_said(&mut self, sc: &mut PoliceScanner, audio: u32, part: Said) {
        sc.described.entry(audio).or_default()[said_index(part)] = true;
    }

    fn start_slot(&mut self, sc: &mut PoliceScanner, s: SlotNo) {
        sc.start_slot((s & 31) as usize);
    }
    fn clear_slot(&mut self, sc: &mut PoliceScanner, s: SlotNo) {
        exe::clear_slot(sc, self, s);
    }
    fn make_slot(&mut self, sc: &mut PoliceScanner, s: SlotNo) -> bool {
        exe::make_slot(sc, self, s)
    }
    fn slot_load_state(&mut self, sc: &mut PoliceScanner, s: SlotNo) -> u32 {
        exe::slot_load_state(sc, self, s)
    }
    fn note_said_of_playing(&mut self, sc: &mut PoliceScanner) {
        exe::note_said(sc, self);
    }
    fn pick_report(&mut self, sc: &mut PoliceScanner) -> bool {
        let may: Vec<bool> = (0..SLOTS as u32).map(|n| exe::may_play(sc, self, n)).collect();
        let (now, repeats) = (self.f.now, self.f.repeats);
        let picked = sc.pick_with(now, repeats, |n, _| may[n]);
        sc.playing = picked;
        picked != NONE
    }
    fn queued_dispatch(&mut self, sc: &mut PoliceScanner) {
        exe::queued_dispatch(sc, self);
    }
    fn chatter(&mut self, sc: &mut PoliceScanner, now: u32) {
        let f = *self.f;
        let at = f.player.unwrap_or_default();
        sc.chatter(now, f.level as u32, at, f.wagon(), self.world, self.rng);
    }
    fn screen_fade(&mut self, _sc: &mut PoliceScanner) -> f32 {
        self.f.fade
    }
    fn held(&mut self, _sc: &mut PoliceScanner) -> bool {
        self.f.held
    }
    fn heading_angle(&mut self, _sc: &mut PoliceScanner, c: f32) -> f32 {
        heading_angle(c)
    }

    fn stop_sound(&mut self, sc: &mut PoliceScanner, sound: SoundHandle) {
        self.sounds.stop(sound);
        self.let_go_ended(sc);
    }
    fn play(&mut self, sc: &mut PoliceScanner, sound: SoundHandle) {
        self.sounds.play(sound);
        self.let_go_ended(sc);
    }
    fn play_when_loaded(&mut self, sc: &mut PoliceScanner, sound: SoundHandle, _bank: Option<u32>) {
        self.sounds.play(sound);
        self.let_go_ended(sc);
    }
    fn load_state(&mut self, _sc: &mut PoliceScanner, sound: Option<SoundHandle>, _bank: u32) -> i32 {
        self.sounds.load_state(sound)
    }
    fn play_position(&mut self, _sc: &mut PoliceScanner, sound: SoundHandle) -> u32 {
        self.sounds.position(sound)
    }
    fn sound_length(&self, sound: SoundHandle) -> i32 {
        self.sounds.length(sound)
    }
    fn set_volume(&mut self, _sc: &mut PoliceScanner, sound: SoundHandle, db: f32) {
        self.sounds.set_volume(sound, db);
    }
    fn set_group(&mut self, _sc: &mut PoliceScanner, _sound: SoundHandle, _group: u32) {}
    fn unplace(&mut self, _sc: &mut PoliceScanner, sound: SoundHandle) {
        self.sounds.place(sound, None);
    }
    fn place_at_slot(&mut self, sc: &mut PoliceScanner, sound: SoundHandle, s: SlotNo) {
        let at = sc.slot(s).place;
        self.sounds.place(sound, at);
    }
    fn make_phrase_sound(&mut self, sc: &mut PoliceScanner, _echo: bool, name: u32, out: SoundOut, req: &PhraseRequest) {
        let h = self.sounds.make(name, req.start);
        match out {
            SoundOut::Phrase(s, k) => sc.slot_mut(s).made_mut(k).sound = h,
            SoundOut::Echo(s, k) => sc.slot_mut(s).made_mut(k).echo = h,
            SoundOut::Noise => sc.noise_sound = h,
        }
    }
    fn make_noise_sound(&mut self, sc: &mut PoliceScanner, name: u32, s: SlotNo, req: &NoiseRequest) {
        let h = self.sounds.make(name, 0);
        if let Some(h) = h {
            let at = req.placed.and(sc.slot(s).place);
            self.sounds.place(h, at);
        }
        sc.noise_sound = h;
    }

    fn crime_sound(&mut self, _sc: &mut PoliceScanner, name: u32) -> u32 {
        if self.world.crime(name).is_some() { name } else { 0 }
    }
    fn crime_sound_of(&mut self, sc: &mut PoliceScanner, hash: u32) -> u32 {
        self.crime_sound(sc, hash)
    }
    fn describe_place(&mut self, _sc: &mut PoliceScanner, at: [f32; 3]) {
        self.report = self.world.place(at);
    }
    fn report_place(&self) -> Place {
        self.report
    }
    fn queue_crime(&mut self, sc: &mut PoliceScanner, at: [f32; 3], r: &Report) {
        let Some(crime) = self.world.crime(r.sound) else { return };
        let r = Report { place: self.world.place(at), ..*r };
        let (now, street_ok) = (self.f.now, self.f.street_ok);
        sc.queue_crime(&r, &crime, now, street_ok, self.rng);
    }
    fn describe_vehicle_into(&mut self, sc: &mut PoliceScanner, v: VehicleFacts) -> Description<u32> {
        exe::describe_vehicle(sc, self, v)
    }
    fn queue_suspect(&mut self, sc: &mut PoliceScanner, r: &SuspectReport<u32>) {
        let s = Suspect {
            report: Report { place: r.place, long: r.say_area, ..Report::default() },
            vehicle: r.vehicle,
            priority: r.priority,
            heading: r.heading,
            speed: r.speed,
            extra: r.last != 0,
        };
        sc.suspect(&s, self.f.now, self.rng);
    }
    fn report_search(&mut self, sc: &mut PoliceScanner, at: [f32; 3]) {
        sc.assistance(at, 2, self.f.now, self.world, self.rng);
    }
    fn report_dispatch(&mut self, sc: &mut PoliceScanner, units: u32, kind: u32, from: [f32; 3]) {
        sc.dispatch(units, kind, from, self.f.now, self.world, self.rng);
    }

    fn crime_multiplier_floor(&self) -> f32 {
        CRIME_MULTIPLIER_FLOOR
    }
    fn fake_level(&self) -> i32 {
        self.f.fake_level
    }
    fn police_crime_hash(&self, crime: u32) -> u32 {
        CRIME_SOUNDS.get(crime as usize).map_or(0, |c| name_hash(c))
    }
    fn spot_chance(&self) -> f32 {
        SPOT_CHANCE
    }
    fn search_share(&self) -> f32 {
        SEARCH_SHARE
    }
    fn station_position(&mut self, _sc: &mut PoliceScanner, station: u32) -> [f32; 3] {
        let (_, i) = place_entry(1, station).unwrap_or((PlaceTable::Dispatch, 0));
        DISPATCH_PLACES[i as usize & 15]
    }
}

pub const FADE_OUT_DB: f32 = -100.0;
pub const FADE_LIMIT: f32 = 0.1;
pub const AREA_AGAIN_CHANCE: f32 = 1.0;
pub const HEADING_CONSTANTS: [f32; 4] = [57.295_776, 360.0, 45.0, 0.011_111_111];
pub const TO_MPH: f32 = 2.236_936_3;
pub const MPH_CAP: f32 = 91.0;
pub const CRIME_MULTIPLIER_FLOOR: f32 = 0.9;
pub const SPOT_CHANCE: f32 = 0.001;
pub const SEARCH_SHARE: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceTable {
    Dispatch,
    Second,
    Third,
}

pub fn place_entry(kind: u32, i: u32) -> Option<(PlaceTable, u32)> {
    let table = match kind {
        1 => PlaceTable::Dispatch,
        2 => PlaceTable::Second,
        3 => PlaceTable::Third,
        _ => return None,
    };
    Some((table, i))
}

pub fn to_upper(c: u32) -> u32 {
    if c.wrapping_sub(u32::from(b'a')) <= 25 { c - 32 } else { c }
}

