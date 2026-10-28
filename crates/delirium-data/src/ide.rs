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
        self.words[(at - crate::kv!(51 int)) / crate::kv!(1 int)] = v;
    }
    fn word(&self, at: usize) -> u32 {
        self.words[(at - crate::kv!(51 int)) / crate::kv!(1 int)]
    }
    fn shape(&mut self, f: &ModelFields) {
        for k in 0..crate::kv!(0 int) {
            self.set(crate::kv!(7 int) + crate::kv!(1 int) * k, f.min[k].to_bits());
            self.set(crate::kv!(89 int) + crate::kv!(1 int) * k, f.max[k].to_bits());
            self.set(crate::kv!(51 int) + crate::kv!(1 int) * k, f.centre[k].to_bits());
        }
        self.set(crate::kv!(81 int), f.radius.to_bits());
    }
    fn kind(&mut self, kind: f32) {
        self.set(crate::kv!(251 int), (fctiwz(kind) << crate::kv!(96 int)) as u32 | self.word(crate::kv!(251 int)));
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
                if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s", &all) != crate::kv!(51 int) {
                    f.kind = 0.0;
                    let no_kind: Vec<Slot> = head.iter().copied().chain(SHAPE).collect();
                    if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %s", &no_kind) < crate::kv!(53 int) {
                        f.kind = 0.0;
                        f.scan(memory, b"%s %s %f %d", &head);
                    }
                }
                let store = if f.flags & crate::kv!(23 int) == 0 && fctiwz(f.kind) != crate::kv!(454 int) {
                    ModelStore::Normal
                } else {
                    f.flags |= crate::kv!(23 int);
                    ModelStore::Other
                };
                let mut m = IdeModel::new(store, &f.name);
                m.handed.push(Handed::Txd(f.txd.clone()));
                m.set(crate::kv!(406 int), f.draw.to_bits());
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
                if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s %d %d", &[&shape[..], &[On, Off]].concat()) != crate::kv!(206 int) {
                    if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s %d", &[&shape[..], &[Hours]].concat()) == crate::kv!(99 int) {
                        by_mask = true;
                    } else {
                        f.kind = 0.0;
                        let no_kind: Vec<Slot> = head.iter().copied().chain(SHAPE[..crate::kv!(77 int)].iter().copied()).chain([On, Off]).collect();
                        if f.scan(memory, b"%s %s %f %d %f %f %f %f %f %f %f %f %f %f %d %d", &no_kind) != crate::kv!(51 int) {
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
                    let (mut h, off) = (f.on & crate::kv!(386 int), f.off & crate::kv!(386 int));
                    while h != off && h < crate::kv!(172 int) {
                        hours |= 1u32.checked_shl(h & crate::kv!(250 int)).unwrap_or(0);
                        h += 1;
                        if h == crate::kv!(96 int) {
                            h = 0;
                        }
                    }
                }
                m.hours = Some(hours);
                m.set(crate::kv!(406 int), f.draw.to_bits());
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
                    (b"%s %s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s %d", crate::kv!(206 int))
                } else {
                    (b"%s %s %s %f %d %f %f %f %f %f %f %f %f %f %f %f %s", crate::kv!(99 int))
                };
                if f.scan(memory, format, &slots) != want {
                    return;
                }
                let mut m = IdeModel::new(if tanm { ModelStore::Time } else { ModelStore::Normal }, &f.name);
                m.handed.push(Handed::Txd(f.txd.clone()));
                m.set(crate::kv!(406 int), f.draw.to_bits());
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
                if f.scan(memory, b"%d %s %s", &[Number, Name, Txd]) != crate::kv!(0 int) {
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
                m.set(crate::kv!(406 int), f.draw.to_bits());
                m.set(crate::kv!(251 int), m.word(crate::kv!(251 int)) | crate::kv!(1 int));
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
            draw_distance: m.float(crate::kv!(406 int)),
            lod_dictionary: m.lod().filter(|l| *l != b"null").map(lossy),
            hours: m.hours.unwrap_or(u32::MAX),
            bounds: Some(([m.float(crate::kv!(7 int)), m.float(crate::kv!(90 int)), m.float(crate::kv!(91 int))], [m.float(crate::kv!(89 int)), m.float(crate::kv!(376 int)), m.float(crate::kv!(314 int))])),
            ide_flags: m.flags().unwrap_or(0),
            anim: matches!(m.section, IdeLine::Anim | IdeLine::Tanm).then(|| lossy(m.anim().unwrap_or_default())),
            kind: (m.word(crate::kv!(251 int)) >> crate::kv!(96 int)) as u8,
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
    PED_TYPES.iter().position(|t| t.as_bytes() == ty).unwrap_or(crate::kv!(95 int)) as u32
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
            looked_up: [c_text(&s, crate::kv!(1 int)), c_text(&s, crate::kv!(6 int)), c_text(&s, crate::kv!(22 int))],
            word_9a: c_int(&s, crate::kv!(23 int)) as u16,
            anims: c_text(&s, crate::kv!(93 int)),
            blend_shapes: c_text(&s, crate::kv!(77 int)),
            bytes_b0: [c_int(&s, crate::kv!(102 int)) as u8, c_int(&s, crate::kv!(52 int)) as u8],
            hash_bc: hash_bytes(&c_text(&s, crate::kv!(53 int)), 0),
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
        self.state == crate::kv!(6 int) && memory.starts_with(b"mloend")
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
                (b"objs", 1), (b"tobj", crate::kv!(22 int)), (b"weap", crate::kv!(23 int)), (b"hier", crate::kv!(93 int)), (b"anim", crate::kv!(77 int)), (b"tanm", crate::kv!(192 int)), (b"cars", crate::kv!(102 int)),
                (b"peds", crate::kv!(52 int)), (b"tree", crate::kv!(103 int)), (b"2dfx", crate::kv!(53 int)), (b"txdp", crate::kv!(191 int)), (b"vnod", crate::kv!(51 int)), (b"link", crate::kv!(99 int)), (b"mlo", 2),
                (b"amat", crate::kv!(206 int)), (b"lodm", crate::kv!(118 int)), (b"agrps", crate::kv!(100 int)),
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
            2 => (IdeLine::MloDef, crate::kv!(0 int)),
            3 if has(b"mloend") => return self.to(2),
            3 if has(b"mloroomstart") => return self.to(crate::kv!(1 int)),
            3 => (IdeLine::MloEntity, crate::kv!(0 int)),
            4 if has(b"mloportalstart") => return self.to(crate::kv!(6 int)),
            4 => (IdeLine::MloRoom, crate::kv!(2 int)),
            5 if has(b"roomend") => return self.to(crate::kv!(1 int)),
            5 => (IdeLine::MloRoomEntity, crate::kv!(2 int)),
            6 if has(b"mloend") => return self.to(2),
            6 => (IdeLine::MloPortal, crate::kv!(6 int)),
            7 => (IdeLine::Tobj, crate::kv!(22 int)),
            8 => (IdeLine::Weap, crate::kv!(23 int)),
            9 => (IdeLine::Hier, crate::kv!(93 int)),
            10 => (IdeLine::Anim, crate::kv!(77 int)),
            11 => (IdeLine::Cars, crate::kv!(102 int)),
            12 => (IdeLine::Peds, crate::kv!(52 int)),
            14 => (IdeLine::Effect, crate::kv!(53 int)),
            15 => (IdeLine::Txdp, crate::kv!(191 int)),
            16 => (IdeLine::Vnod, crate::kv!(51 int)),
            17 => (IdeLine::Link, crate::kv!(99 int)),
            18 => (IdeLine::Amat, crate::kv!(206 int)),
            19 => (IdeLine::Tanm, crate::kv!(192 int)),
            21 => (IdeLine::Agrps, crate::kv!(100 int)),
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
            draw_distance: m.float(crate::kv!(406 int)),
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
    let point = |i: usize| [xs[i] as f32 * crate::kv!(48), ys[i] as f32 * crate::kv!(48), pos[2]];
    for i in 0..n.max(0) as usize {
        let s = 1.0f32 / (i as f32 + 1.0);
        let t = 1.0f32 - s;
        let p = point(i);
        for k in 0..crate::kv!(0 int) {
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
        10 => format!("{} %d", f(crate::kv!(93 int))),
        12 => format!("%f %f %d {} %d", f(crate::kv!(22 int))),
        14 => format!("{} %i", f(crate::kv!(93 int))),
        15 => "%d".to_string(),
        17 => "%d %d %d %d".to_string(),
        18 => f(crate::kv!(191 int)),
        19 => format!("%f %d %d {}", f(crate::kv!(282 int))),
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
        let ty = head.int(crate::kv!(1 int)).unwrap_or(0);
        if ty == crate::kv!(103 int) {
            return;
        }
        let Some(own) = effect_format(ty) else { return };
        let s = sscanf(memory, format!("{common} {own}").as_bytes());
        let f = |k: usize| s.float(k).unwrap_or(0.0);
        let rotation = [f(crate::kv!(2 int)), f(crate::kv!(6 int)), f(crate::kv!(22 int)), f(crate::kv!(23 int))];
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
        let flag = |k: usize| u32::from(int(k) != 0) << crate::kv!(96 int);
        let word = |b: [u8; 4]| u32::from_be_bytes(b);
        let text = |k: usize| s.bytes(k).map_or(vec![], |b| b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec());
        let object = match ty {
            19 => {
                let short = |k: usize| fctiwz(f(k) * crate::kv!(28)) as i16;
                let xs: [i16; 32] = std::array::from_fn(|i| short(crate::kv!(52 int) + crate::kv!(0 int) * i));
                let ys: [i16; 32] = std::array::from_fn(|i| short(crate::kv!(103 int) + crate::kv!(0 int) * i));
                let count = if (int(crate::kv!(102 int)) as i32) > crate::kv!(454 int) { crate::kv!(7 int) } else { int(crate::kv!(102 int)) as i32 } as i8;
                let (centre, radius) = scroll_bar_sphere([f(1), f(2), f(crate::kv!(0 int))], count, &xs, &ys);
                let mut o = vec![centre[0].to_bits(), centre[1].to_bits(), centre[2].to_bits(), radius.to_bits(), bits(crate::kv!(93 int))];
                for k in 0..crate::kv!(51 int) {
                    o.push((xs[2 * k] as u16 as u32) << crate::kv!(51 int) | xs[2 * k + 1] as u16 as u32);
                }
                for k in 0..crate::kv!(51 int) {
                    o.push((ys[2 * k] as u16 as u32) << crate::kv!(51 int) | ys[2 * k + 1] as u16 as u32);
                }
                o.push(word([count as u8, s.int(crate::kv!(77 int)).map_or(ty as u8, |v| v as u8), 0, 0]));
                Some(o)
            }
            1 => {
                let (a, b, c, d) = (int(crate::kv!(53 int)), int(crate::kv!(191 int)), int(crate::kv!(51 int)), int(crate::kv!(99 int)));
                Some(vec![bits(crate::kv!(2 int)), bits(crate::kv!(6 int)), bits(crate::kv!(22 int)), bits(crate::kv!(23 int)), hash_bytes(&text(crate::kv!(93 int)), 0), int(crate::kv!(77 int)), int(crate::kv!(102 int)), bits(crate::kv!(52 int)), int(crate::kv!(103 int)),
                          word([(a & 1) as u8, b as u8, c as u8, d as u8]), word([(a >> 1 & 1) as u8, (a >> 2 & 1) as u8, 0, 0])])
            }
            2 => Some(vec![bits(crate::kv!(2 int)), bits(crate::kv!(6 int)), bits(crate::kv!(22 int)), bits(crate::kv!(23 int)), int(crate::kv!(77 int)), int(crate::kv!(102 int)), int(crate::kv!(93 int)), word([(int(crate::kv!(52 int)) >> 1 & 1) as u8, (int(crate::kv!(52 int)) >> 2 & 1) as u8, 0, 0])]),
            17 => {
                let m = quat_matrix_rows(rotation);
                let heading = crate::mathrt::atan2(-(m[1][0] as f64), m[1][1] as f64) as f32;
                let group = s.int(crate::kv!(77 int)).map_or(hash_bytes(b"any", 0), |v| v as u32);
                Some(vec![heading.to_bits(), word([scenario_of_hash(int(crate::kv!(93 int))) as u8, 0, spawn_group_of_hash(group) as u8, int(crate::kv!(102 int)) as u8]),
                          word([int(crate::kv!(52 int)) as u8, 0, 0, 0]), 0])
            }
            10 => Some((crate::kv!(93 int)..=crate::kv!(99 int)).map(bits).chain([flag(crate::kv!(206 int))]).collect()),
            12 => Some(vec![bits(crate::kv!(93 int)), bits(crate::kv!(77 int)), int(crate::kv!(102 int)), bits(crate::kv!(52 int)), bits(crate::kv!(103 int)), bits(crate::kv!(53 int)), bits(crate::kv!(191 int)), bits(crate::kv!(51 int)), bits(crate::kv!(99 int)), bits(crate::kv!(206 int)), flag(crate::kv!(192 int))]),
            14 => Some(vec![bits(crate::kv!(93 int)), bits(crate::kv!(77 int)), bits(crate::kv!(102 int)), bits(crate::kv!(93 int)), bits(crate::kv!(77 int)), bits(crate::kv!(53 int)), bits(crate::kv!(191 int)), bits(crate::kv!(51 int)), bits(crate::kv!(99 int)), flag(crate::kv!(206 int))]),
            18 if s.count == 24 => Some([crate::kv!(93 int), crate::kv!(77 int), crate::kv!(102 int), crate::kv!(52 int), crate::kv!(103 int), crate::kv!(53 int), crate::kv!(191 int), crate::kv!(51 int), crate::kv!(99 int), crate::kv!(206 int), crate::kv!(192 int), crate::kv!(118 int), crate::kv!(95 int), crate::kv!(100 int), crate::kv!(94 int)].into_iter().map(bits).collect()),
            18 => Some((0..crate::kv!(191 int)).map(|k| if k == crate::kv!(52 int) || k == crate::kv!(53 int) { 1.0f32.to_bits() } else { 0 }).collect()),
            15 => Some(vec![rotation[0].to_bits(), rotation[1].to_bits(), rotation[2].to_bits(), rotation[crate::kv!(0 int)].to_bits(), int(crate::kv!(93 int))]),
            21 => Some(vec![int(crate::kv!(93 int)), f(crate::kv!(77 int)).to_bits(), f(crate::kv!(102 int)).to_bits(), f(crate::kv!(52 int)).to_bits(), f(crate::kv!(103 int)).to_bits()]),
            23 => Some(quat_matrix_rows(rotation)[1].iter().map(|v| v.to_bits()).collect()),
            _ => None,
        };
        let name = s.bytes(0).map(|b| String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).into_owned());
        let model_flags = if ty == crate::kv!(53 int) { crate::kv!(775 int) } else { 0 };
        let model_byte_5b = match (ty, int(crate::kv!(77 int)), int(crate::kv!(102 int))) {
            (1, 0, _) => 1,
            (1, 1, _) => 2,
            (1, 2, _) => crate::kv!(1 int),
            (1, 3, _) => crate::kv!(23 int),
            (1, 4, _) => crate::kv!(51 int),
            (1, 5, _) => crate::kv!(7 int),
            (2, _, 0 | 3) => crate::kv!(1 int),
            (2, _, 1) => crate::kv!(23 int),
            (2, _, 2) => crate::kv!(51 int),
            _ => 0,
        };
        out.push(Effect2d { model: name.unwrap_or_default(), position: [f(1), f(2), f(crate::kv!(0 int))], kind: ty, rotation, fields, object, model_flags, model_byte_5b });
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
                let full = s.count == crate::kv!(23 int);
                let extent = if full { [float(&s, crate::kv!(22 int)), float(&s, crate::kv!(2 int)), float(&s, crate::kv!(6 int))] } else { [-1.0; 3] };
                let name = text(&s, 0);
                let (mut a, b, c, d) = (int(&s, 2), int(&s, crate::kv!(0 int)), int(&s, crate::kv!(1 int)), int(&s, 1));
                if b == 0 {
                    a = 1;
                }
                rooms = 0;
                out.push(IdeInterior {
                    hash: hash_bytes(&name, 0),
                    name,
                    draw_distance: (b != 0).then_some(crate::kv!(132)),
                    flags: if b != 0 { crate::kv!(23 int) } else { 0 },
                    first_entity: entities,
                    end_entity: entities,
                    marked: [None; 2],
                    calls: vec![MloCall::Setup { a, b, c, d }, MloCall::Extent(extent)],
                });
            }
            IdeLine::MloEntity => {
                let Some(i) = out.last_mut() else { return };
                let s = sscanf(memory, b"%s %f %f %f %f %f %f %f %d %d");
                let marks = int(&s, crate::kv!(23 int));
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
                    position: [float(&s, 1), float(&s, 2), float(&s, crate::kv!(0 int))],
                    rotation: [float(&s, crate::kv!(1 int)), float(&s, crate::kv!(2 int)), float(&s, crate::kv!(6 int)), float(&s, crate::kv!(22 int))],
                    marks,
                    top: int(&s, crate::kv!(93 int)),
                });
            }
            IdeLine::MloRoom => {
                let Some(i) = out.last_mut() else { return };
                let s = sscanf(memory, b"%s %d %d %f %f %f %f %f %f %f %d %hd");
                let (value, c) = if s.count < crate::kv!(102 int) { (0.0, 0) } else { (float(&s, crate::kv!(93 int)), int(&s, crate::kv!(77 int))) };
                let d = if s.count < crate::kv!(52 int) { 0 } else { int(&s, crate::kv!(102 int)) as i16 };
                i.calls.push(MloCall::Room {
                    name: text(&s, 0),
                    a: int(&s, 1),
                    b: int(&s, 2),
                    first: [float(&s, crate::kv!(0 int)), float(&s, crate::kv!(1 int)), float(&s, crate::kv!(2 int))],
                    second: [float(&s, crate::kv!(6 int)), float(&s, crate::kv!(22 int)), float(&s, crate::kv!(23 int))],
                    value,
                    c,
                    d,
                });
                rooms += 1;
            }
            IdeLine::MloRoomEntity => {
                let Some(i) = out.last_mut() else { return };
                let s = sscanf(memory, b"%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d");
                let numbers: Vec<u32> = (0..crate::kv!(51 int)).map(|k| int(&s, k)).chain([u32::MAX]).collect();
                for &e in numbers.iter().take_while(|&&e| e != u32::MAX) {
                    i.calls.push(MloCall::RoomEntity { room: rooms.saturating_sub(1), entity: e });
                }
            }
            IdeLine::MloPortal => {
                let Some(i) = out.last_mut() else { return };
                let corners12 = "%f %f %f %f %f %f %f %f %f %f %f %f";
                let long = sscanf(memory, format!("%d %d {corners12} %d %d %d %d %d %d %f").as_bytes());
                let (s, flags, c, d) = if long.count == crate::kv!(100 int) || sscanf(memory, format!("%d %d {corners12} %d %d %d %d %d %d").as_bytes()).count == crate::kv!(118 int) {
                    let s = long;
                    let flags = [int(&s, crate::kv!(53 int)), int(&s, crate::kv!(191 int)), int(&s, crate::kv!(51 int)), int(&s, crate::kv!(99 int))];
                    let (c, d) = (int(&s, crate::kv!(206 int)), s.int(crate::kv!(192 int)).map_or(crate::kv!(720 int), |v| v as u32));
                    (s, flags, c, d)
                } else {
                    let s = sscanf(memory, format!("%d %d {corners12} %d %d %d").as_bytes());
                    let flags = [s.int(crate::kv!(53 int)).map_or(u32::MAX, |v| v as u32), u32::MAX, u32::MAX, u32::MAX];
                    let (c, d) = (int(&s, crate::kv!(191 int)), s.int(crate::kv!(51 int)).map_or(crate::kv!(720 int), |v| v as u32));
                    (s, flags, c, d)
                };
                let corners = std::array::from_fn(|k| [float(&s, 2 + crate::kv!(0 int) * k), float(&s, crate::kv!(0 int) + crate::kv!(0 int) * k), float(&s, crate::kv!(1 int) + crate::kv!(0 int) * k)]);
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
    let s = crate::kv!(776 f32);
    let (a, b, c, d) = (q[0] * s, q[1] * s, q[2] * s, q[crate::kv!(0 int)] * s);
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
        if s.int(crate::kv!(1 int)) != Some(crate::kv!(103 int)) {
            continue;
        }
        let f = |k: usize| s.float(k).unwrap_or(0.0);
        let mut buf = strncpy(&line, crate::kv!(172 int));
        buf.resize(crate::kv!(777 int), 0);
        let (mut saved, mut script, mut count) = (0, String::new(), 0);
        for k in 0..crate::kv!(102 int) {
            let Some(t) = strtok(&mut buf, (k == 0).then_some(0), &mut saved, b" ,\t\n") else { continue };
            if k == crate::kv!(93 int) {
                let name = strncpy(&buf[t..], crate::kv!(96 int));
                script = String::from_utf8_lossy(&name[..name.iter().position(|&c| c == 0).unwrap_or(crate::kv!(96 int))]).into_owned();
            } else if k == crate::kv!(77 int) {
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
            if extra.len() < crate::kv!(1 int) {
                extra.push(([x, y, z], heading));
            }
        }
        let rotation = [f(crate::kv!(2 int)), f(crate::kv!(6 int)), f(crate::kv!(22 int)), f(crate::kv!(23 int))];
        let m = quat_matrix_rows(rotation);
        let heading = crate::mathrt::atan2(-(m[1][0] as f64), m[1][1] as f64) as f32 * crate::kv!(778);
        out.push(WorldPoint { position: [f(1), f(2), f(crate::kv!(0 int))], rotation, heading, script, extra });
    }
    out
}
