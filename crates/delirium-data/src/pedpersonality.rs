use std::collections::HashMap;

pub const TICKBOXES: [(&str, u64); 44] = [
    ("bowling", 0x4), ("pool", 0x8), ("basketball", 0x10), ("drivespoorcar", 0x20), ("drivesaveragecar", 0x40),
    ("drivesrichcar", 0x80), ("drivesbigcar", 0x100), ("drivesmotorcycle", 0x200), ("drivesboat", 0x400),
    ("onfoot", 0x800), ("argue", 0x1000), ("smoker", 0x2000), ("phoneuser", 0x4000), ("buydrugs", 0x8000),
    ("givebummoney", 0x10000), ("insultbum", 0x20000), ("replytoho", 0x40000), ("taketaxi", 0x80000),
    ("watchfight", 0x100000), ("late", 0x200000), ("softjack", 0x2_0000_0000), ("murdershakefist", 0x400000),
    ("murdercallpolice", 0x800000), ("murderrunaway", 0x1000000), ("murderattack", 0x2000000),
    ("taxidriver", 0x20_0000_0000), ("policeofficer", 0x4000000), ("swat", 0x8000000), ("fbi", 0x10000000),
    ("dea", 0x20000000), ("csi", 0x40000000), ("paramedic", 0x80000000), ("fireman", 0x1_0000_0000),
    ("prostitute", 0x4_0000_0000), ("pimp", 0x8_0000_0000), ("bum", 0x10_0000_0000),
    ("cancarryweapons", 0x40_0000_0000), ("likesart", 0x80_0000_0000), ("dislikesart", 0x100_0000_0000),
    ("tourist", 0x200_0000_0000), ("office", 0x400_0000_0000), ("gardening", 0x800_0000_0000),
    ("jewish", 0x1000_0000_0000), ("criminal", 0x2000_0000_0000),
];
pub const HAS_PERSONALITY: u64 = 0x1;
pub const MALE: u64 = 0x2;
pub const IN_GROUPS_30_41: u64 = 0x8000_0000_0000;
pub const PLAYER: u64 = 0x4000_0000_0000;

#[derive(Clone, Debug, PartialEq)]
pub struct Personality {
    pub flags: u64,
    pub age: u8,
    pub sexiness: u8,
    pub bravery: u8,
    pub agility: u8,
    pub languages: [u8; 2],
    pub attack: f32,
    pub defend: f32,
    pub melee_percent: f32,
    pub groups: [Vec<u8>; 7],
    pub move_group: Vec<u8>,
}

impl Default for Personality {
    fn default() -> Self {
        Personality {
            flags: MALE,
            age: 30,
            sexiness: 2,
            bravery: 2,
            agility: 3,
            languages: [0; 2],
            attack: 0.8,
            defend: 1.3,
            melee_percent: 50.0,
            groups: Default::default(),
            move_group: vec![],
        }
    }
}

#[derive(Default)]
struct Fields {
    name: Vec<u8>,
    sex: Vec<u8>,
    ints: [i32; 4],
    floats: [f32; 3],
    lang: [Vec<u8>; 2],
    groups: [Vec<u8>; 7],
    move_group: Vec<u8>,
    ticks: Vec<u8>,
}

fn language(s: &[u8]) -> u8 {
    match s.first() {
        Some(b'C') => 3,
        Some(b'R') => 2,
        Some(b'S') => 1,
        _ => 0,
    }
}

pub fn read(file: &[u8], models: &mut HashMap<u32, Personality>, in_group: impl Fn(u32, u32) -> bool) {
    let mut f = Fields::default();
    crate::textlines::each_line(file, true, |memory| {
        if memory.first() == Some(&b'#') {
            return;
        }
        f.lang[0].clear();
        f.lang[1].clear();
        f.ticks.clear();
        let s = crate::crt::sscanf(memory, b"%s %s %d %d %d %f %f %d %f %s %s %s %s %s %s %s %s %s %s %s");
        let text = |k: usize, into: &mut Vec<u8>| {
            if let Some(b) = s.bytes(k) {
                *into = b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec();
            }
        };
        text(0, &mut f.name);
        text(1, &mut f.sex);
        for (slot, k) in [(0, 2), (1, 3), (2, 4), (3, 7)] {
            if let Some(v) = s.int(k) {
                f.ints[slot] = v;
            }
        }
        for (slot, k) in [(0, 5), (1, 6), (2, 8)] {
            if let Some(v) = s.float(k) {
                f.floats[slot] = v;
            }
        }
        text(9, &mut f.lang[0]);
        text(10, &mut f.lang[1]);
        for g in 0..7 {
            text(11 + g, &mut f.groups[g]);
        }
        text(18, &mut f.move_group);
        text(19, &mut f.ticks);
        let hash = crate::map::hash_bytes(&f.name, 0);
        let Some(p) = models.get_mut(&hash) else { return };
        p.flags |= HAS_PERSONALITY;
        if matches!(f.sex.first(), Some(b'm' | b'M')) {
            p.flags |= MALE;
        } else {
            p.flags &= !MALE;
        }
        [p.age, p.sexiness, p.bravery, p.agility] = f.ints.map(|v| v as u8);
        [p.attack, p.defend, p.melee_percent] = f.floats;
        p.groups = f.groups.clone();
        p.move_group = f.move_group.clone();
        p.languages = [language(&f.lang[0]), language(&f.lang[1])];
        for word in f.ticks.split(|&c| c == b'+').filter(|w| !w.is_empty()) {
            for (name, bit) in TICKBOXES {
                if word == name.as_bytes() {
                    p.flags |= bit;
                }
            }
        }
        for group in 30..42 {
            if in_group(hash, group) {
                p.flags |= IN_GROUPS_30_41;
            }
        }
    });
}

