use crate::map::{hash_bytes, ObjectDef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelStore {
    Normal,
    Other,
    Time,
    Weapon,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Handed {
    Txd(Vec<u8>),
    Flags(u32),
    Lod(Vec<u8>),
    Anim(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct IdeModel {
    pub section: IdeLine,
    pub line: usize,
    pub store: ModelStore,
    pub name: Vec<u8>,
    pub handed: Vec<Handed>,
    pub words: [u32; 13],
    pub hours: Option<u32>,
}

impl IdeModel {
    fn new(store: ModelStore, name: &[u8]) -> Self {
        IdeModel { section: IdeLine::Objs, line: 0, store, name: name.to_vec(), handed: vec![], words: [0; 13], hours: None }
    }
    fn set(&mut self, at: usize, v: u32) {
        self.words[(at - 0x10) / 4] = v;
    }
    fn word(&self, at: usize) -> u32 {
        self.words[(at - 0x10) / 4]
    }
    fn shape(&mut self, f: &ModelFields) {
        for k in 0..3 {
            self.set(0x20 + 4 * k, f.min[k].to_bits());
            self.set(0x30 + 4 * k, f.max[k].to_bits());
            self.set(0x10 + 4 * k, f.centre[k].to_bits());
        }
        self.set(0x1C, f.radius.to_bits());
    }
    fn kind(&mut self, kind: f32) {
        self.set(0x40, (fctiwz(kind) << 24) as u32 | self.word(0x40));
    }
    pub fn txd(&self) -> Option<&[u8]> {
        self.handed.iter().find_map(|h| if let Handed::Txd(t) = h { Some(&t[..]) } else { None })
    }
    pub fn lod(&self) -> Option<&[u8]> {
        self.handed.iter().find_map(|h| if let Handed::Lod(t) = h { Some(&t[..]) } else { None })
    }
    pub fn anim(&self) -> Option<&[u8]> {
        self.handed.iter().find_map(|h| if let Handed::Anim(t) = h { Some(&t[..]) } else { None })
    }
    pub fn flags(&self) -> Option<u32> {
        self.handed.iter().find_map(|h| if let Handed::Flags(f) = h { Some(*f) } else { None })
    }
    pub fn float(&self, at: usize) -> f32 {
        f32::from_bits(self.word(at))
    }
}

fn fctiwz(v: f32) -> i32 {
    if v.is_nan() { i32::MIN } else { v as i32 }
}

#[derive(Default)]
struct ModelFields {
    name: Vec<u8>,
    txd: Vec<u8>,
    anim: Vec<u8>,
    lod: Vec<u8>,
    draw: f32,
    flags: u32,
    kind: f32,
    min: [f32; 3],
    max: [f32; 3],
    centre: [f32; 3],
    radius: f32,
    on: u32,
    off: u32,
    hours: u32,
    number: u32,
}

#[derive(Clone, Copy)]
enum Slot {
    Name,
    Txd,
    Anim,
    Lod,
    Draw,
    Flags,
    Kind,
    Min(usize),
    Max(usize),
    Centre(usize),
    Radius,
    On,
    Off,
    Hours,
    Number,
}

const SHAPE: [Slot; 11] = [Slot::Min(0), Slot::Min(1), Slot::Min(2), Slot::Max(0), Slot::Max(1), Slot::Max(2), Slot::Centre(0), Slot::Centre(1),
    Slot::Centre(2), Slot::Radius, Slot::Lod];

impl ModelFields {
    fn scan(&mut self, line: &[u8], format: &[u8], slots: &[Slot]) -> i32 {
        let s = crate::crt::sscanf(line, format);
        for (slot, v) in slots.iter().zip(&s.values) {
            let text = || match v {
                crate::crt::Value::Bytes(b) => b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec(),
                _ => vec![],
            };
            let float = || if let crate::crt::Value::Float(f) = v { *f } else { 0.0 };
            let int = || if let crate::crt::Value::Int { bits, .. } = v { *bits as u32 } else { 0 };
            match *slot {
                Slot::Name => self.name = text(),
                Slot::Txd => self.txd = text(),
                Slot::Anim => self.anim = text(),
                Slot::Lod => self.lod = text(),
                Slot::Draw => self.draw = float(),
                Slot::Flags => self.flags = int(),
                Slot::Kind => self.kind = float(),
                Slot::Min(k) => self.min[k] = float(),
                Slot::Max(k) => self.max[k] = float(),
                Slot::Centre(k) => self.centre[k] = float(),
                Slot::Radius => self.radius = float(),
                Slot::On => self.on = int(),
                Slot::Off => self.off = int(),
                Slot::Hours => self.hours = int(),
                Slot::Number => self.number = int(),
            }
        }
        s.count
    }
}

pub fn ide_models(file: &[u8]) -> Vec<IdeModel> {
    use Slot::*;
    let mut out = vec![];
    each_ide_line(file, |kind, memory, line| {
        let mut f = ModelFields::default();
        let mut m = match kind {
            IdeLine::Objs => {
                let head = [Name, Txd, Draw, Flags];
                let all: Vec<Slot> = head.iter().copied().chain([Kind]).chain(SHAPE).collect();
                if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s", &all) != 16 {
                    f.kind = 0.0;
                    let no_kind: Vec<Slot> = head.iter().copied().chain(SHAPE).collect();
                    if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %s", &no_kind) < 14 {
                        f.kind = 0.0;
                        f.scan(memory, b"%s %s %f %d", &head);
                    }
                }
                let store = if f.flags & 8 == 0 && fctiwz(f.kind) != 0x1F {
                    ModelStore::Normal
                } else {
                    f.flags |= 8;
                    ModelStore::Other
                };
                let mut m = IdeModel::new(store, &f.name);
                m.handed.push(Handed::Txd(f.txd.clone()));
                m.set(0x2C, f.draw.to_bits());
                m.handed.push(Handed::Flags(f.flags));
                m.kind(f.kind);
                m.shape(&f);
                m.handed.push(Handed::Lod(f.lod.clone()));
                m
            }
            IdeLine::Tobj => {
                let head = [Name, Txd, Draw, Flags];
                let shape: Vec<Slot> = head.iter().copied().chain([Kind]).chain(SHAPE).collect();
                let mut by_mask = false;
                if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s %d %d", &[&shape[..], &[On, Off]].concat()) != 18 {
                    if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s %d", &[&shape[..], &[Hours]].concat()) == 17 {
                        by_mask = true;
                    } else {
                        f.kind = 0.0;
                        let no_kind: Vec<Slot> = head.iter().copied().chain(SHAPE[..10].iter().copied()).chain([On, Off]).collect();
                        if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %d %d", &no_kind) != 16 {
                            f.scan(memory, b"%s %s %f %d %f %d %d", &[Name, Txd, Draw, Flags, Kind, On, Off]);
                        }
                    }
                }
                let mut m = IdeModel::new(ModelStore::Time, &f.name);
                m.handed.push(Handed::Txd(f.txd.clone()));
                let mut hours = 0;
                if by_mask {
                    hours = f.hours;
                } else {
                    let (mut h, off) = (f.on & 0xFF, f.off & 0xFF);
                    while h != off && h < 256 {
                        hours |= 1u32.checked_shl(h & 0x3F).unwrap_or(0);
                        h += 1;
                        if h == 24 {
                            h = 0;
                        }
                    }
                }
                m.hours = Some(hours);
                m.set(0x2C, f.draw.to_bits());
                m.handed.push(Handed::Flags(f.flags));
                m.kind(f.kind);
                m.shape(&f);
                m.handed.push(Handed::Lod(f.lod.clone()));
                m
            }
            IdeLine::Anim | IdeLine::Tanm => {
                let tanm = kind == IdeLine::Tanm;
                if !tanm {
                    f.anim = b"null".to_vec();
                }
                let slots: Vec<Slot> = [Name, Txd, Anim, Draw, Flags, Kind].into_iter().chain(SHAPE).chain(tanm.then_some(Hours)).collect();
                let (format, want): (&[u8], i32) = if tanm {
                    (b"%s %s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s %d", 18)
                } else {
                    (b"%s %s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s", 17)
                };
                if f.scan(memory, format, &slots) != want {
                    return;
                }
                let mut m = IdeModel::new(if tanm { ModelStore::Time } else { ModelStore::Normal }, &f.name);
                m.handed.push(Handed::Txd(f.txd.clone()));
                m.set(0x2C, f.draw.to_bits());
                if tanm {
                    m.hours = Some(f.hours);
                }
                m.handed.push(Handed::Flags(f.flags));
                m.kind(f.kind);
                m.shape(&f);
                m.handed.push(Handed::Anim(f.anim.clone()));
                m.handed.push(Handed::Lod(f.lod.clone()));
                m
            }
            IdeLine::Hier => {
                if f.scan(memory, b"%d %s %s", &[Number, Name, Txd]) != 3 {
                    return;
                }
                let mut m = IdeModel::new(ModelStore::Normal, &f.name);
                m.handed.push(Handed::Txd(f.txd.clone()));
                m
            }
            IdeLine::Weap => {
                f.scan(memory, b"%s %s %s %d %f", &[Name, Txd, Anim, Number, Draw]);
                let mut m = IdeModel::new(ModelStore::Weapon, &f.name);
                m.handed.push(Handed::Txd(f.txd.clone()));
                m.handed.push(Handed::Anim(f.anim.clone()));
                m.set(0x2C, f.draw.to_bits());
                m.set(0x40, m.word(0x40) | 4);
                m
            }
            _ => return,
        };
        m.name.truncate(m.name.iter().position(|&c| c == 0).unwrap_or(m.name.len()));
        (m.section, m.line) = (kind, line);
        out.push(m);
    });
    out
}

pub fn read_ide_objects(text: &str) -> Vec<ObjectDef> {
    let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    ide_models(text.as_bytes())
        .into_iter()
        .filter(|m| matches!(m.section, IdeLine::Objs | IdeLine::Tobj | IdeLine::Anim | IdeLine::Tanm))
        .map(|m| ObjectDef {
            name: lossy(&m.name),
            txd: lossy(m.txd().unwrap_or_default()),
            draw_distance: m.float(0x2C),
            lod_dictionary: m.lod().filter(|l| *l != b"null").map(lossy),
            hours: m.hours.unwrap_or(u32::MAX),
            bounds: Some(([m.float(0x20), m.float(0x24), m.float(0x28)], [m.float(0x30), m.float(0x34), m.float(0x38)])),
            ide_flags: m.flags().unwrap_or(0),
            anim: matches!(m.section, IdeLine::Anim | IdeLine::Tanm).then(|| lossy(m.anim().unwrap_or_default())),
            kind: (m.word(0x40) >> 24) as u8,
        })
        .collect()
}

fn each_ide_line(file: &[u8], mut f: impl FnMut(IdeLine, &[u8], usize)) {
    let mut state = IdeState::default();
    let mut handed = 0;
    crate::textlines::each_line(file, true, |memory| {
        if state.at_mlo_end(memory) {
            f(IdeLine::MloEnd, memory, handed);
        }
        if let Some(kind) = state.line(memory) {
            f(kind, memory, handed);
            handed += 1;
        }
    });
}

fn c_text(s: &crate::crt::Scanned, k: usize) -> Vec<u8> {
    s.bytes(k).map(|b| b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec()).unwrap_or_default()
}

fn c_int(s: &crate::crt::Scanned, k: usize) -> u32 {
    s.int(k).map_or(0, |v| v as u32)
}

pub const PED_TYPES: [&str; 23] = [
    "CIVMALE", "CIVFEMALE", "COP", "GANG_ALBANIAN", "GANG_BIKER_1", "GANG_BIKER_2", "GANG_ITALIAN", "GANG_RUSSIAN",
    "GANG_RUSSIAN_2", "GANG_IRISH", "GANG_JAMAICAN", "GANG_AFRICAN_AMERICAN", "GANG_KOREAN", "GANG_CHINESE_JAPANESE",
    "GANG_PUERTO_RICAN", "DEALER", "MEDIC", "FIREMAN", "CRIMINAL", "BUM", "PROSTITUTE", "SPECIAL", "MISSION",
];

pub fn is_player_type(ty: &[u8]) -> bool {
    [&b"PLAYER1"[..], b"PLAYER2", b"PLAYER_NETWORK", b"PLAYER_UNUSED"].contains(&ty)
}

pub fn ped_type_index(ty: &[u8]) -> u32 {
    if is_player_type(ty) {
        return 0;
    }
    PED_TYPES.iter().position(|t| t.as_bytes() == ty).unwrap_or(23) as u32
}

#[derive(Clone, Debug, PartialEq)]
pub struct PedModel {
    pub name: Vec<u8>,
    pub props: Vec<u8>,
    pub ped_type: Vec<u8>,
    pub looked_up: [Vec<u8>; 3],
    pub word_9a: u16,
    pub anims: Vec<u8>,
    pub blend_shapes: Vec<u8>,
    pub bytes_b0: [u8; 2],
    pub hash_bc: u32,
}

pub fn ide_peds(file: &[u8]) -> Vec<PedModel> {
    let mut out = vec![];
    each_ide_line(file, |kind, memory, _| {
        if kind != IdeLine::Peds {
            return;
        }
        let s = crate::crt::sscanf(memory, b"%s %s %s   %s %s %s %s %s   %x %s %s %d %d   %s %s %s");
        out.push(PedModel {
            name: c_text(&s, 0),
            props: c_text(&s, 1),
            ped_type: c_text(&s, 2),
            looked_up: [c_text(&s, 4), c_text(&s, 6), c_text(&s, 7)],
            word_9a: c_int(&s, 8) as u16,
            anims: c_text(&s, 9),
            blend_shapes: c_text(&s, 10),
            bytes_b0: [c_int(&s, 11) as u8, c_int(&s, 12) as u8],
            hash_bc: hash_bytes(&c_text(&s, 14), 0),
        });
    });
    out
}

pub fn ide_txd_parents(file: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
    let mut out = vec![];
    each_ide_line(file, |kind, memory, _| {
        if kind == IdeLine::Txdp {
            let s = crate::crt::sscanf(memory, b"%s %s");
            out.push((c_text(&s, 0), c_text(&s, 1)));
        }
    });
    out
}

pub fn ide_materials(file: &[u8]) -> Vec<(Vec<u8>, u32, u32)> {
    let mut out = vec![];
    each_ide_line(file, |kind, memory, _| {
        if kind != IdeLine::Amat {
            return;
        }
        let s = crate::crt::sscanf(memory, b"%s %u %s");
        let material = c_text(&s, 2);
        let value = match material.first() {
            Some(b'0'..=b'9') => c_int(&crate::crt::sscanf(&material, b"%u"), 0),
            _ => hash_bytes(&material, 0),
        };
        out.push((c_text(&s, 0), c_int(&s, 1), value));
    });
    out
}

pub fn ide_anim_groups(file: &[u8]) -> Vec<(Vec<u8>, u32, i32)> {
    let mut out = vec![];
    each_ide_line(file, |kind, memory, _| {
        if kind == IdeLine::Agrps {
            let s = crate::crt::sscanf(memory, b"%s %s %d");
            out.push((c_text(&s, 0), hash_bytes(&c_text(&s, 1), 0), c_int(&s, 2) as i32));
        }
    });
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct PedDef {
    pub name: String,
    pub props: String,
    pub anims: String,
    pub blend_shapes: String,
}

pub fn read_ide_peds(text: &str) -> Vec<PedDef> {
    let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    ide_peds(text.as_bytes())
        .into_iter()
        .map(|p| PedDef { name: lossy(&p.name), props: lossy(&p.props), anims: lossy(&p.anims), blend_shapes: lossy(&p.blend_shapes) })
        .collect()
}

pub fn read_ide_model_names(text: &str) -> Vec<String> {
    use IdeLine::*;
    let mut named: Vec<(usize, String)> =
        ide_models(text.as_bytes()).into_iter().map(|m| (m.line, String::from_utf8_lossy(&m.name).into_owned())).collect();
    let handed = ide_lines(text.as_bytes()).into_iter().filter(|(k, _)| *k != MloEnd);
    for (line, (k, l)) in handed.enumerate() {
        if [Cars, Peds, MloDef].contains(&k) {
            named.push((line, String::from_utf8_lossy(l.split(|&c| c == b' ').next().unwrap_or_default()).into_owned()));
        }
    }
    named.sort_by_key(|(line, _)| *line);
    named.into_iter().map(|(_, n)| n).collect()
}

pub fn read_ide_txd_parents(text: &str) -> Vec<(String, String)> {
    let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    ide_txd_parents(text.as_bytes()).into_iter().map(|(c, p)| (lossy(&c), lossy(&p))).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdeLine {
    Objs,
    MloDef,
    MloEntity,
    MloRoom,
    MloRoomEntity,
    MloPortal,
    MloEnd,
    Tobj,
    Weap,
    Hier,
    Anim,
    Cars,
    Peds,
    Effect,
    Txdp,
    Vnod,
    Link,
    Amat,
    Tanm,
    Agrps,
}

pub fn ide_lines(file: &[u8]) -> Vec<(IdeLine, Vec<u8>)> {
    let mut state = IdeState::default();
    let mut out = vec![];
    for l in crate::textlines::lines(file, true) {
        if state.at_mlo_end(&l) {
            out.push((IdeLine::MloEnd, vec![]));
        }
        if let Some(kind) = state.line(&l) {
            out.push((kind, l));
        }
    }
    out
}

#[derive(Default)]
struct IdeState {
    state: u32,
}

impl IdeState {
    fn at_mlo_end(&self, memory: &[u8]) -> bool {
        self.state == 6 && memory.starts_with(b"mloend")
    }

    fn line(&mut self, memory: &[u8]) -> Option<IdeLine> {
        let l = &memory[..memory.iter().position(|&c| c == 0).unwrap_or(memory.len())];
        let &c = l.first()?;
        if c == b'#' {
            return None;
        }
        let has = |k: &[u8]| l.starts_with(k);
        if self.state == 0 {
            let opens: [(&[u8], u32); 17] = [
                (b"objs", 1), (b"tobj", 7), (b"weap", 8), (b"hier", 9), (b"anim", 10), (b"tanm", 19), (b"cars", 11),
                (b"peds", 12), (b"tree", 13), (b"2dfx", 14), (b"txdp", 15), (b"vnod", 16), (b"link", 17), (b"mlo", 2),
                (b"amat", 18), (b"lodm", 20), (b"agrps", 21),
            ];
            self.state = opens.iter().find(|(k, _)| has(k)).map_or(0, |&(_, s)| s);
            return None;
        }
        if l == b"end" {
            self.state = 0;
            return None;
        }
        let (kind, next) = match self.state {
            1 => (IdeLine::Objs, 1),
            2 => (IdeLine::MloDef, 3),
            3 if has(b"mloend") => return self.to(2),
            3 if has(b"mloroomstart") => return self.to(4),
            3 => (IdeLine::MloEntity, 3),
            4 if has(b"mloportalstart") => return self.to(6),
            4 => (IdeLine::MloRoom, 5),
            5 if has(b"roomend") => return self.to(4),
            5 => (IdeLine::MloRoomEntity, 5),
            6 if has(b"mloend") => return self.to(2),
            6 => (IdeLine::MloPortal, 6),
            7 => (IdeLine::Tobj, 7),
            8 => (IdeLine::Weap, 8),
            9 => (IdeLine::Hier, 9),
            10 => (IdeLine::Anim, 10),
            11 => (IdeLine::Cars, 11),
            12 => (IdeLine::Peds, 12),
            14 => (IdeLine::Effect, 14),
            15 => (IdeLine::Txdp, 15),
            16 => (IdeLine::Vnod, 16),
            17 => (IdeLine::Link, 17),
            18 => (IdeLine::Amat, 18),
            19 => (IdeLine::Tanm, 19),
            21 => (IdeLine::Agrps, 21),
            _ => return None,
        };
        self.state = next;
        Some(kind)
    }

    fn to(&mut self, state: u32) -> Option<IdeLine> {
        self.state = state;
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelMaterial {
    pub model: String,
    pub index: u32,
    pub material: u32,
}

pub fn read_ide_amat(text: &str) -> Vec<ModelMaterial> {
    ide_materials(text.as_bytes())
        .into_iter()
        .map(|(model, index, material)| ModelMaterial { model: String::from_utf8_lossy(&model).into_owned(), index, material })
        .collect()
}

pub fn read_ide_agrps(text: &str) -> Vec<(String, u32, i32)> {
    ide_anim_groups(text.as_bytes()).into_iter().map(|(m, g, n)| (String::from_utf8_lossy(&m).into_owned(), g, n)).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponDef {
    pub name: String,
    pub txd: String,
    pub anim: String,
    pub draw_distance: f32,
}

pub fn read_ide_weapons(text: &str) -> Vec<WeaponDef> {
    let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    ide_models(text.as_bytes())
        .into_iter()
        .filter(|m| m.section == IdeLine::Weap)
        .map(|m| WeaponDef {
            name: lossy(&m.name),
            txd: lossy(m.txd().unwrap_or_default()),
            anim: lossy(m.anim().unwrap_or_default()),
            draw_distance: m.float(0x2C),
        })
        .collect()
}

pub fn read_ide_hier(text: &str) -> Vec<(String, String)> {
    let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    ide_models(text.as_bytes())
        .into_iter()
        .filter(|m| m.section == IdeLine::Hier)
        .map(|m| (lossy(&m.name), lossy(m.txd().unwrap_or_default())))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub enum EffectField {
    Float(f32),
    Int(i32),
    Text(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Effect2d {
    pub model: String,
    pub position: [f32; 3],
    pub kind: i32,
    pub rotation: [f32; 4],
    pub fields: Vec<EffectField>,
    pub object: Option<Vec<u32>>,
    pub model_flags: u32,
    pub model_byte_5b: u8,
}

pub fn effect_class(kind: i32) -> Option<&'static str> {
    Some(match kind {
        0 => "CLightAttr",
        1 => "CParticleAttr",
        2 => "CExplosionAttr",
        10 => "CEscalatorAttr",
        12 => "CProcObjAttr",
        13 => "CWorldPointAttr",
        14 => "CLadderInfo",
        15 => "CAudioAttr",
        17 => "CSpawnPoint",
        18 => "CLightShaftAttr",
        19 => "CScrollBar",
        21 => "CSwayableAttr",
        22 => "CBuoyancyAttr",
        23 => "CWalkDontWalkAttr",
        _ => return None,
    })
}

pub const SCENARIOS: [&str; 95] = [
    "Seat_Bench", "Seat_StdChair", "Seat_StdCouch", "Seat_CafeChair", "Seat_RestaurantChair", "Seat_BarChair",
    "Seat_StripClubChair", "Seat_StripClubStool", "Seat_BoardGamePlayer", "Seat_OnCar", "Seat_Crate",
    "Seat_InternetCafe", "Seat_OnSteps", "Seat_OnStepsB", "Seat_OnWall", "Seat_OnStepsHangOut", "Seat_OnWallHangOut",
    "Seat_HospitalWaiting", "Seat_SlouchedDruggie", "HangOut_Street", "HangOut_AlleyWay",
    "HangOut_OutsideLiquorStore", "HangOut_BaseballDiamonds", "HangOut_BoardGameWatcher", "HangOut_ProjectCourtyard",
    "HangOut_BasketballCourt", "HangOut_OutsideMassageParlours", "HangOut_ClubHouse", "HangOut_WatchBoardGame",
    "Scenario_SecurityGuard", "Scenario_TouristPhoto", "Scenario_Leaning", "Scenario_SellingDrugs",
    "Scenario_RoadWorkers", "Scenario_DiggingRoadWorkers", "Scenario_DrillingRoadWorkers",
    "Scenario_RoadWorkerWithSign", "Scenario_IndustrialWorkers", "Scenario_BuildingWorkers",
    "Scenario_DiggingBuildingWorkers", "Scenario_DrillingBuildingWorkers", "Scenario_HeavilyArmedPolice",
    "Scenario_SmokingOutsideOffice", "Scenario_PayPhone", "Scenario_DancingNightclub", "Scenario_HospitalNurse",
    "Scenario_HospitalDoctor", "Scenario_AirWorkers", "Scenario_Homeless", "Scenario_Brazier", "Scenario_Gardening",
    "Scenario_ParkGardeners", "Scenario_Sweeper", "Scenario_NewspaperStand", "Scenario_StationedCop",
    "Scenario_PostMan", "Scenario_Binoculars", "Scenario_UpTelegraphPoles", "Scenario_ServiceWorker",
    "Scenario_StreetPerformer", "Scenario_Prostitute", "Scenario_StationarySweeper", "Scenario_WindowCleaner",
    "Scenario_LeaningForwards", "Scenario_WatchingPoleDancer", "Scenario_Bouncer", "Scenario_WaitingForTaxi",
    "Scenario_Preacher", "Scenario_Standing", "Scenario_LayingDruggie", "Scenario_HospitalBed",
    "Scenario_InvestigatingCop", "Scenario_TaiChi", "Scenario_JoggerSpawn", "Scenario_StripperDancing",
    "Scenario_StripperLapdance", "Scenario_DrinkingAtBar", "Scenario_PoliceSniper", "Scenario_PoliceSpotter",
    "Location_DropOffPoint", "Location_ShopBrowsing", "Vehicle_InspectingBrokenDownVehicle", "Vehicle_LookingInBoot",
    "Vehicle_Mechanic", "Vehicle_DeliveryDriver", "Vehicle_ParkDeliveryTruck", "Vehicle_DropPassengersOff",
    "Vehicle_LimoDropPassengerOff", "Vehicle_ParkedThenWander", "Vehicle_StealAbandondedVehicle",
    "Vehicle_SmokeThenDriveAway", "Wander_FollowPed", "Wander_MobileConversation", "Wander_2WayConversation",
    "Standing_MobileConversation",
];

pub const SPAWN_GROUPS: [&str; 19] = [
    "any", "gang", "industrial", "construction", "police", "airworker", "hospital", "bum", "Prostitute", "FireMan",
    "BurgerWorker", "CluckWorker", "PizzaWorker", "Waitress", "HotelDoorman", "Receptionist", "Prisoner", "Office",
    "UsePopulation",
];

pub fn scroll_bar_sphere(pos: [f32; 3], count: i8, xs: &[i16; 32], ys: &[i16; 32]) -> ([f32; 3], f32) {
    let n = count as i32 - 1;
    let mut c = pos;
    let point = |i: usize| [xs[i] as f32 * 0.01, ys[i] as f32 * 0.01, pos[2]];
    for i in 0..n.max(0) as usize {
        let s = 1.0f32 / (i as f32 + 1.0);
        let t = 1.0f32 - s;
        let p = point(i);
        for k in 0..3 {
            c[k] = (c[k] as f64 * t as f64 + (p[k] * s) as f64) as f32;
        }
    }
    let length = |d: [f32; 3]| d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2])).sqrt();
    let mut r = length([pos[0] - c[0], pos[1] - c[1], pos[2] - c[2]]);
    for i in 0..n.max(0) as usize {
        let p = point(i);
        let len = length([p[0] - c[0], p[1] - c[1], p[2] - c[2]]);
        r = if r - len >= 0.0 { r } else { len };
    }
    (c, r)
}

pub fn scenario_of_hash(hash: u32) -> i32 {
    SCENARIOS.iter().position(|n| hash_bytes(n.as_bytes(), 0) == hash).map_or(-1, |k| k as i32)
}

pub fn spawn_group_of_hash(hash: u32) -> u32 {
    SPAWN_GROUPS.iter().position(|n| hash_bytes(n.as_bytes(), 0) == hash).unwrap_or(0) as u32
}

fn effect_format(kind: i32) -> Option<String> {
    let f = |n: usize| vec!["%f"; n].join(" ");
    Some(match kind {
        1 => "%s %d %d %f %d %d %d %d %d".to_string(),
        2 => "%d %d %d %d".to_string(),
        10 => format!("{} %d", f(9)),
        12 => format!("%f %f %d {} %d", f(7)),
        14 => format!("{} %i", f(9)),
        15 => "%d".to_string(),
        17 => "%d %d %d %d".to_string(),
        18 => f(15),
        19 => format!("%f %d %d {}", f(96)),
        21 => "%d %f %f %f %f".to_string(),
        22 | 23 => String::new(),
        _ => return None,
    })
}

pub fn read_ide_effects(text: &str) -> Vec<Effect2d> {
    use crate::crt::{sscanf, Value};
    let mut out = vec![];
    each_ide_line(text.as_bytes(), |kind, memory, _| {
        if kind != IdeLine::Effect {
            return;
        }
        let common = "%s %f %f %f %d %f %f %f %f";
        let head = sscanf(memory, common.as_bytes());
        let ty = head.int(4).unwrap_or(0);
        if ty == 13 {
            return;
        }
        let Some(own) = effect_format(ty) else { return };
        let s = sscanf(memory, format!("{common} {own}").as_bytes());
        let f = |k: usize| s.float(k).unwrap_or(0.0);
        let rotation = [f(5), f(6), f(7), f(8)];
        let fields: Vec<EffectField> = s.values[9.min(s.values.len())..]
            .iter()
            .map(|v| match v {
                Value::Float(x) => EffectField::Float(*x),
                Value::Int { bits, .. } => EffectField::Int(*bits as i32),
                Value::Bytes(b) => EffectField::Text(String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).into_owned()),
                _ => EffectField::Int(0),
            })
            .collect();
        let int = |k: usize| s.int(k).map_or(0, |v| v as u32);
        let bits = |k: usize| f(k).to_bits();
        let flag = |k: usize| u32::from(int(k) != 0) << 24;
        let word = |b: [u8; 4]| u32::from_be_bytes(b);
        let text = |k: usize| s.bytes(k).map_or(vec![], |b| b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec());
        let object = match ty {
            19 => {
                let short = |k: usize| fctiwz(f(k) * 100.0) as i16;
                let xs: [i16; 32] = std::array::from_fn(|i| short(12 + 3 * i));
                let ys: [i16; 32] = std::array::from_fn(|i| short(13 + 3 * i));
                let count = if (int(11) as i32) > 31 { 32 } else { int(11) as i32 } as i8;
                let (centre, radius) = scroll_bar_sphere([f(1), f(2), f(3)], count, &xs, &ys);
                let mut o = vec![centre[0].to_bits(), centre[1].to_bits(), centre[2].to_bits(), radius.to_bits(), bits(9)];
                for k in 0..16 {
                    o.push((xs[2 * k] as u16 as u32) << 16 | xs[2 * k + 1] as u16 as u32);
                }
                for k in 0..16 {
                    o.push((ys[2 * k] as u16 as u32) << 16 | ys[2 * k + 1] as u16 as u32);
                }
                o.push(word([count as u8, s.int(10).map_or(ty as u8, |v| v as u8), 0, 0]));
                Some(o)
            }
            1 => {
                let (a, b, c, d) = (int(14), int(15), int(16), int(17));
                Some(vec![bits(5), bits(6), bits(7), bits(8), hash_bytes(&text(9), 0), int(10), int(11), bits(12), int(13),
                          word([(a & 1) as u8, b as u8, c as u8, d as u8]), word([(a >> 1 & 1) as u8, (a >> 2 & 1) as u8, 0, 0])])
            }
            2 => Some(vec![bits(5), bits(6), bits(7), bits(8), int(10), int(11), int(9), word([(int(12) >> 1 & 1) as u8, (int(12) >> 2 & 1) as u8, 0, 0])]),
            17 => {
                let m = quat_matrix_rows(rotation);
                let heading = crate::mathrt::atan2(-(m[1][0] as f64), m[1][1] as f64) as f32;
                let group = s.int(10).map_or(hash_bytes(b"any", 0), |v| v as u32);
                Some(vec![heading.to_bits(), word([scenario_of_hash(int(9)) as u8, 0, spawn_group_of_hash(group) as u8, int(11) as u8]),
                          word([int(12) as u8, 0, 0, 0]), 0])
            }
            10 => Some((9..=17).map(bits).chain([flag(18)]).collect()),
            12 => Some(vec![bits(9), bits(10), int(11), bits(12), bits(13), bits(14), bits(15), bits(16), bits(17), bits(18), flag(19)]),
            14 => Some(vec![bits(9), bits(10), bits(11), bits(9), bits(10), bits(14), bits(15), bits(16), bits(17), flag(18)]),
            18 if s.count == 24 => Some([9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 23, 21, 22].into_iter().map(bits).collect()),
            18 => Some((0..15).map(|k| if k == 12 || k == 14 { 1.0f32.to_bits() } else { 0 }).collect()),
            15 => Some(vec![rotation[0].to_bits(), rotation[1].to_bits(), rotation[2].to_bits(), rotation[3].to_bits(), int(9)]),
            21 => Some(vec![int(9), f(10).to_bits(), f(11).to_bits(), f(12).to_bits(), f(13).to_bits()]),
            23 => Some(quat_matrix_rows(rotation)[1].iter().map(|v| v.to_bits()).collect()),
            _ => None,
        };
        let name = s.bytes(0).map(|b| String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).into_owned());
        let model_flags = if ty == 14 { 0x1_4000 } else { 0 };
        let model_byte_5b = match (ty, int(10), int(11)) {
            (1, 0, _) => 1,
            (1, 1, _) => 2,
            (1, 2, _) => 4,
            (1, 3, _) => 8,
            (1, 4, _) => 0x10,
            (1, 5, _) => 0x20,
            (2, _, 0 | 3) => 4,
            (2, _, 1) => 8,
            (2, _, 2) => 0x10,
            _ => 0,
        };
        out.push(Effect2d { model: name.unwrap_or_default(), position: [f(1), f(2), f(3)], kind: ty, rotation, fields, object, model_flags, model_byte_5b });
    });
    out
}

#[derive(Clone, Debug, PartialEq)]
pub enum MloCall {
    Setup { a: u32, b: u32, c: u32, d: u32 },
    Extent([f32; 3]),
    Entity { model: Vec<u8>, position: [f32; 3], rotation: [f32; 4], marks: u32, top: u32 },
    Room { name: Vec<u8>, a: u32, b: u32, first: [f32; 3], second: [f32; 3], value: f32, c: u32, d: i16 },
    RoomEntity { room: usize, entity: u32 },
    Portal { a: u32, b: u32, c: u32, flags: [u32; 4], corners: [[f32; 3]; 4], d: u32 },
    PortalsEnd,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IdeInterior {
    pub name: Vec<u8>,
    pub hash: u32,
    pub draw_distance: Option<f32>,
    pub flags: u32,
    pub first_entity: u32,
    pub end_entity: u32,
    pub marked: [Option<u32>; 2],
    pub calls: Vec<MloCall>,
}

pub fn ide_interiors(file: &[u8]) -> Vec<IdeInterior> {
    use crate::crt::sscanf;
    let mut out: Vec<IdeInterior> = vec![];
    let mut entities = 0u32;
    let mut rooms = 0usize;
    each_ide_line(file, |kind, memory, _| {
        let text = |s: &crate::crt::Scanned, k: usize| s.bytes(k).map_or(vec![], |b| b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec());
        let int = |s: &crate::crt::Scanned, k: usize| s.int(k).map_or(0, |v| v as u32);
        let float = |s: &crate::crt::Scanned, k: usize| s.float(k).unwrap_or(0.0);
        match kind {
            IdeLine::MloDef => {
                let s = sscanf(memory, b"%s %d %d %d %d %f %f %f");
                let full = s.count == 8;
                let extent = if full { [float(&s, 7), float(&s, 5), float(&s, 6)] } else { [-1.0; 3] };
                let name = text(&s, 0);
                let (mut a, b, c, d) = (int(&s, 2), int(&s, 3), int(&s, 4), int(&s, 1));
                if b == 0 {
                    a = 1;
                }
                rooms = 0;
                out.push(IdeInterior {
                    hash: hash_bytes(&name, 0),
                    name,
                    draw_distance: (b != 0).then_some(20.0),
                    flags: if b != 0 { 8 } else { 0 },
                    first_entity: entities,
                    end_entity: entities,
                    marked: [None; 2],
                    calls: vec![MloCall::Setup { a, b, c, d }, MloCall::Extent(extent)],
                });
            }
            IdeLine::MloEntity => {
                let Some(i) = out.last_mut() else { return };
                let s = sscanf(memory, b"%s %f %f %f %f %f %f %f %d %d");
                let marks = int(&s, 8);
                if marks & 1 != 0 {
                    i.marked[0] = Some(i.end_entity);
                }
                if marks & 2 != 0 {
                    i.marked[1] = Some(i.end_entity);
                }
                i.end_entity += 1;
                entities += 1;
                i.calls.push(MloCall::Entity {
                    model: text(&s, 0),
                    position: [float(&s, 1), float(&s, 2), float(&s, 3)],
                    rotation: [float(&s, 4), float(&s, 5), float(&s, 6), float(&s, 7)],
                    marks,
                    top: int(&s, 9),
                });
            }
            IdeLine::MloRoom => {
                let Some(i) = out.last_mut() else { return };
                let s = sscanf(memory, b"%s %d %d %f %f %f %f %f %f %f %d %hd");
                let (value, c) = if s.count < 11 { (0.0, 0) } else { (float(&s, 9), int(&s, 10)) };
                let d = if s.count < 12 { 0 } else { int(&s, 11) as i16 };
                i.calls.push(MloCall::Room {
                    name: text(&s, 0),
                    a: int(&s, 1),
                    b: int(&s, 2),
                    first: [float(&s, 3), float(&s, 4), float(&s, 5)],
                    second: [float(&s, 6), float(&s, 7), float(&s, 8)],
                    value,
                    c,
                    d,
                });
                rooms += 1;
            }
            IdeLine::MloRoomEntity => {
                let Some(i) = out.last_mut() else { return };
                let s = sscanf(memory, b"%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d");
                let numbers: Vec<u32> = (0..16).map(|k| int(&s, k)).chain([u32::MAX]).collect();
                for &e in numbers.iter().take_while(|&&e| e != u32::MAX) {
                    i.calls.push(MloCall::RoomEntity { room: rooms.saturating_sub(1), entity: e });
                }
            }
            IdeLine::MloPortal => {
                let Some(i) = out.last_mut() else { return };
                let corners12 = "%f %f %f %f %f %f %f %f %f %f %f %f";
                let long = sscanf(memory, format!("%d %d {corners12} %d %d %d %d %d %d %f").as_bytes());
                let (s, flags, c, d) = if long.count == 21 || sscanf(memory, format!("%d %d {corners12} %d %d %d %d %d %d").as_bytes()).count == 20 {
                    let s = long;
                    let flags = [int(&s, 14), int(&s, 15), int(&s, 16), int(&s, 17)];
                    let (c, d) = (int(&s, 18), s.int(19).map_or(0xFF_FFFF, |v| v as u32));
                    (s, flags, c, d)
                } else {
                    let s = sscanf(memory, format!("%d %d {corners12} %d %d %d").as_bytes());
                    let flags = [s.int(14).map_or(u32::MAX, |v| v as u32), u32::MAX, u32::MAX, u32::MAX];
                    let (c, d) = (int(&s, 15), s.int(16).map_or(0xFF_FFFF, |v| v as u32));
                    (s, flags, c, d)
                };
                let corners = std::array::from_fn(|k| [float(&s, 2 + 3 * k), float(&s, 3 + 3 * k), float(&s, 4 + 3 * k)]);
                i.calls.push(MloCall::Portal { a: int(&s, 0), b: int(&s, 1), c, flags, corners, d });
            }
            IdeLine::MloEnd => {
                if let Some(i) = out.last_mut() {
                    i.calls.push(MloCall::PortalsEnd);
                }
            }
            _ => {}
        }
    });
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorldPoint {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub script: String,
    pub heading: f32,
    pub extra: Vec<([f32; 3], f32)>,
}

pub fn quat_matrix_rows(q: [f32; 4]) -> [[f32; 3]; 3] {
    let s = 1.414_213_538_169_860_8f32;
    let (a, b, c, d) = (q[0] * s, q[1] * s, q[2] * s, q[3] * s);
    let (ca, ba, dc, db, cb, da) = (c * a, b * a, d * c, d * b, c * b, d * a);
    let (cc, bb, aa) = (c * c, b * b, a * a);
    [
        [1.0 - (cc + bb), ba + dc, ca - db],
        [ba - dc, 1.0 - (cc + aa), da + cb],
        [db + ca, cb - da, 1.0 - (bb + aa)],
    ]
}

pub fn read_world_points(text: &str) -> Vec<WorldPoint> {
    use crate::crt::{atof, sscanf, strncpy, strtok};
    let mut out = vec![];
    for (kind, line) in crate::ipltext::ipl_lines(text.as_bytes()) {
        if kind != crate::ipltext::IplLine::Effect {
            continue;
        }
        let s = sscanf(&line, b"%s %f %f %f %d %f %f %f %f");
        if s.int(4) != Some(13) {
            continue;
        }
        let f = |k: usize| s.float(k).unwrap_or(0.0);
        let mut buf = strncpy(&line, 256);
        buf.resize(336, 0);
        let (mut saved, mut script, mut count) = (0, String::new(), 0);
        for k in 0..11 {
            let Some(t) = strtok(&mut buf, (k == 0).then_some(0), &mut saved, b" ,\t\n") else { continue };
            if k == 9 {
                let name = strncpy(&buf[t..], 24);
                script = String::from_utf8_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(24)]).into_owned();
            } else if k == 10 {
                count = crate::textfmt::atoi(&buf[t..]);
            }
        }
        let mut extra = vec![];
        for _ in 0..count.max(0) {
            let mut next = || {
                let t = strtok(&mut buf, None, &mut saved, b" ,\t\n");
                t.map_or(0.0, |t| atof(&buf[t..]) as f32)
            };
            let (x, y, z, heading) = (next(), next(), next(), next());
            if extra.len() < 4 {
                extra.push(([x, y, z], heading));
            }
        }
        let rotation = [f(5), f(6), f(7), f(8)];
        let m = quat_matrix_rows(rotation);
        let heading = crate::mathrt::atan2(-(m[1][0] as f64), m[1][1] as f64) as f32 * 57.295_776_367_187_5;
        out.push(WorldPoint { position: [f(1), f(2), f(3)], rotation, heading, script, extra });
    }
    out
}
