use std::collections::HashMap;

#[allow(non_snake_case)]
#[inline]
pub fn TICKBOXES() -> &'static [(&'static str, u64); 44] { static V: std::sync::OnceLock<[(&'static str, u64); 44]> = std::sync::OnceLock::new(); let f = || [
    ("bowling", crate::kv!(1 int)), ("pool", crate::kv!(23 int)), ("basketball", crate::kv!(51 int)), ("drivespoorcar", crate::kv!(7 int)), ("drivesaveragecar", crate::kv!(251 int)),
    ("drivesrichcar", crate::kv!(249 int)), ("drivesbigcar", crate::kv!(172 int)), ("drivesmotorcycle", crate::kv!(174 int)), ("drivesboat", crate::kv!(175 int)),
    ("onfoot", crate::kv!(693 int)), ("argue", crate::kv!(173 int)), ("smoker", crate::kv!(49 int)), ("phoneuser", crate::kv!(647 int)), ("buydrugs", crate::kv!(683 int)),
    ("givebummoney", crate::kv!(176 int)), ("insultbum", crate::kv!(620 int)), ("replytoho", crate::kv!(784 int)), ("taketaxi", crate::kv!(786 int)),
    ("watchfight", crate::kv!(791 int)), ("late", crate::kv!(792 int)), ("softjack", crate::kv!(855 int)), ("murdershakefist", crate::kv!(788 int)),
    ("murdercallpolice", crate::kv!(789 int)), ("murderrunaway", crate::kv!(782 int)), ("murderattack", crate::kv!(843 int)),
    ("taxidriver", crate::kv!(856 int)), ("policeofficer", crate::kv!(783 int)), ("swat", crate::kv!(785 int)), ("fbi", crate::kv!(787 int)),
    ("dea", crate::kv!(790 int)), ("csi", crate::kv!(848 int)), ("paramedic", crate::kv!(847 int)), ("fireman", crate::kv!(857 int)),
    ("prostitute", crate::kv!(858 int)), ("pimp", crate::kv!(859 int)), ("bum", crate::kv!(860 int)),
    ("cancarryweapons", crate::kv!(861 int)), ("likesart", crate::kv!(862 int)), ("dislikesart", crate::kv!(863 int)),
    ("tourist", crate::kv!(864 int)), ("office", crate::kv!(865 int)), ("gardening", crate::kv!(866 int)),
    ("jewish", crate::kv!(867 int)), ("criminal", crate::kv!(868 int)),
]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
pub const HAS_PERSONALITY: u64 = 0x1;
pub const MALE: u64 = 0x2;
#[allow(non_snake_case)]
#[inline]
pub fn IN_GROUPS_30_41() -> u64 { crate::kv!(869 int) }
#[allow(non_snake_case)]
#[inline]
pub fn PLAYER() -> u64 { crate::kv!(870 int) }

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
            age: crate::kv!(274 int),
            sexiness: 2,
            bravery: 2,
            agility: crate::kv!(0 int),
            languages: [0; 2],
            attack: crate::kv!(20),
            defend: crate::kv!(190),
            melee_percent: crate::kv!(76),
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
        Some(b'C') => crate::kv!(0 int),
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
        for (slot, k) in [(0, 2), (1, crate::kv!(0 int)), (2, crate::kv!(1 int)), (crate::kv!(0 int), crate::kv!(22 int))] {
            if let Some(v) = s.int(k) {
                f.ints[slot] = v;
            }
        }
        for (slot, k) in [(0, crate::kv!(2 int)), (1, crate::kv!(6 int)), (2, crate::kv!(23 int))] {
            if let Some(v) = s.float(k) {
                f.floats[slot] = v;
            }
        }
        text(crate::kv!(93 int), &mut f.lang[0]);
        text(crate::kv!(77 int), &mut f.lang[1]);
        for g in 0..crate::kv!(22 int) {
            text(crate::kv!(102 int) + g, &mut f.groups[g]);
        }
        text(crate::kv!(206 int), &mut f.move_group);
        text(crate::kv!(192 int), &mut f.ticks);
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
            for (name, bit) in (*TICKBOXES()) {
                if word == name.as_bytes() {
                    p.flags |= bit;
                }
            }
        }
        for group in crate::kv!(274 int)..crate::kv!(398 int) {
            if in_group(hash, group) {
                p.flags |= IN_GROUPS_30_41();
            }
        }
    });
}

