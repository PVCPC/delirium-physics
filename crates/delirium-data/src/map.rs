pub struct Instance {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub model_hash: u32,
    pub flags: u32,
    pub lod: i32,
    pub draw_distance: f32,
}

pub struct ObjectDef {
    pub name: String,
    pub txd: String,
    pub draw_distance: f32,
    pub lod_dictionary: Option<String>,
    pub hours: u32,
    pub bounds: Option<([f32; 3], [f32; 3])>,
    pub sphere: Option<([f32; 3], f32)>,
    pub ide_flags: u32,
    pub anim: Option<String>,
    pub kind: u8,
}

impl ObjectDef {
    pub fn model_flags(&self) -> u32 {
        #[allow(non_snake_case)]
        #[inline]
        fn MOVES() -> &'static [(u32, u32); 14] { static V: std::sync::OnceLock<[(u32, u32); 14]> = std::sync::OnceLock::new(); let f = || [
            (crate::kv!(648 int), crate::kv!(1 int)), (crate::kv!(52 int), crate::kv!(181 int)), (crate::kv!(722 int), crate::kv!(54 int)), (crate::kv!(7 int), crate::kv!(712 int)), (crate::kv!(257 int), crate::kv!(648 int)), (crate::kv!(816 int), crate::kv!(184 int)), (crate::kv!(817 int), crate::kv!(818 int)),
            (crate::kv!(819 int), crate::kv!(820 int)), (crate::kv!(821 int), crate::kv!(822 int)), (crate::kv!(822 int), crate::kv!(676 int)), (crate::kv!(823 int), crate::kv!(183 int)), (crate::kv!(824 int), crate::kv!(722 int)), (crate::kv!(182 int), crate::kv!(825 int)),
            (crate::kv!(183 int), crate::kv!(826 int)),
        ]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
        let mut out = (self.kind as u32) << crate::kv!(107 int);
        for (from, to) in (*MOVES()) {
            if self.ide_flags & from != 0 {
                out |= to;
            }
        }
        out
    }
}

pub fn hash(name: &str) -> u32 {
    hash_bytes(name.as_bytes(), 0)
}

pub fn hash_bytes(name: &[u8], seed: u32) -> u32 {
    let quoted = name.first() == Some(&b'"');
    let mut h = seed;
    for &b in name.iter().skip(quoted as usize) {
        if b == 0 || (quoted && b == b'"') {
            break;
        }
        let c = match b {
            b'A'..=b'Z' => b + crate::kv!(7 int),
            b'\\' => b'/',
            _ => b,
        };
        h = h.wrapping_add(c as i8 as i32 as u32);
        h = h.wrapping_add(h << crate::kv!(81 int));
        h ^= h >> crate::kv!(6 int);
    }
    h = h.wrapping_add(h << crate::kv!(0 int));
    h ^= h >> crate::kv!(113 int);
    h.wrapping_add(h << crate::kv!(189 int))
}

pub struct Xpl<'a> {
    pub version: u32,
    pub inst: &'a [u8],
    pub zone: &'a [u8],
    pub grge: &'a [u8],
    pub cars: &'a [u8],
    pub tcyc: &'a [u8],
    pub vnod: &'a [u8],
    pub link: &'a [u8],
    pub dropped: &'a [u8],
    pub mlo: &'a [u8],
    pub lodm: &'a [u8],
    pub slow: &'a [u8],
}

#[allow(non_snake_case)]
#[inline]
fn XPL_SECTIONS() -> &'static [(usize, usize); 11] { static V: std::sync::OnceLock<[(usize, usize); 11]> = std::sync::OnceLock::new(); let f = || [(crate::kv!(1 int), crate::kv!(93 int)), (crate::kv!(23 int), crate::kv!(330 int)), (crate::kv!(55 int), crate::kv!(93 int)), (crate::kv!(54 int), crate::kv!(323 int)), (crate::kv!(127 int), crate::kv!(415 int)), (crate::kv!(107 int), crate::kv!(415 int)), (crate::kv!(85 int), crate::kv!(127 int)), (crate::kv!(188 int), crate::kv!(368 int)), (crate::kv!(94 int), crate::kv!(188 int)), (crate::kv!(95 int), crate::kv!(780 int)), (crate::kv!(415 int), crate::kv!(107 int))]; if crate::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

fn be32(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + crate::kv!(1 int)).map(|b| u32::from_be_bytes(b.try_into().unwrap()))
}

impl<'a> Xpl<'a> {
    pub fn read(d: &'a [u8]) -> Result<Xpl<'a>, String> {
        let version = be32(d, 0).ok_or("Truncated xpl")?;
        if version != crate::kv!(0 int) {
            return Err(format!("Unknown xpl version {version}"));
        }
        let mut at = crate::kv!(330 int);
        let mut parts: Vec<&[u8]> = vec![];
        for (head, size) in (*XPL_SECTIONS()) {
            let len = be32(d, head).ok_or("Truncated xpl")? as usize * size;
            parts.push(d.get(at..at + len).ok_or("Truncated xpl")?);
            at += len;
        }
        Ok(Xpl {
            version, inst: parts[0], zone: parts[1], grge: parts[2], cars: parts[crate::kv!(0 int)], tcyc: parts[crate::kv!(1 int)], vnod: parts[crate::kv!(2 int)], link: parts[crate::kv!(6 int)],
            dropped: parts[crate::kv!(22 int)], mlo: parts[crate::kv!(23 int)], lodm: parts[crate::kv!(104 int)], slow: parts[crate::kv!(81 int)],
        })
    }
}

fn records(d: &[u8], size: usize) -> impl Iterator<Item = &[u8]> {
    d.chunks_exact(size)
}
fn word(r: &[u8], k: usize) -> u32 {
    be32(r, k * crate::kv!(1 int)).unwrap_or(0)
}
fn float(r: &[u8], k: usize) -> f32 {
    f32::from_bits(word(r, k))
}

pub fn read_xpl(d: &[u8]) -> Result<Vec<Instance>, String> {
    Ok(records(Xpl::read(d)?.inst, crate::kv!(93 int))
        .map(|r| Instance {
            position: [float(r, 0), float(r, 1), float(r, 2)],
            rotation: [float(r, crate::kv!(0 int)), float(r, crate::kv!(1 int)), float(r, crate::kv!(2 int)), float(r, crate::kv!(6 int))],
            model_hash: word(r, crate::kv!(22 int)),
            flags: word(r, crate::kv!(23 int)),
            lod: word(r, crate::kv!(104 int)) as i32,
            draw_distance: float(r, crate::kv!(113 int)),
        })
        .collect())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MloPlus {
    pub instance: u32,
    pub record: [u32; 12],
    pub capacity: u32,
}

pub fn read_xpl_mlo(d: &[u8]) -> Result<Vec<MloPlus>, String> {
    Ok(records(Xpl::read(d)?.mlo, crate::kv!(188 int))
        .map(|r| {
            let mut record = [u32::MAX; 12];
            for (k, w) in record[..crate::kv!(22 int)].iter_mut().enumerate() {
                *w = word(r, crate::kv!(104 int) + k);
            }
            record[crate::kv!(22 int)] = hash_bytes(&r[..crate::kv!(107 int)], 0);
            record[crate::kv!(23 int)] = word(r, crate::kv!(6 int));
            record[crate::kv!(113 int)] = (-1.0f32).to_bits();
            MloPlus { instance: word(r, crate::kv!(22 int)), record, capacity: word(r, crate::kv!(23 int)) }
        })
        .collect())
}

#[derive(Clone)]
pub struct CarGen {
    pub position: [f32; 3],
    pub dir: [f32; 2],
    pub width: f32,
    pub model_hash: u32,
    pub colours: [i32; 4],
    pub flags: u32,
    pub alarm: u8,
    pub lock: u8,
}

pub fn read_xpl_cars(d: &[u8]) -> Result<Vec<CarGen>, String> {
    Ok(records(Xpl::read(d)?.cars, crate::kv!(323 int))
        .map(|r| CarGen {
            position: [float(r, 0), float(r, 1), float(r, 2)],
            dir: [float(r, crate::kv!(0 int)), float(r, crate::kv!(1 int))],
            width: float(r, crate::kv!(2 int)),
            model_hash: word(r, crate::kv!(6 int)),
            colours: [word(r, crate::kv!(22 int)) as i32, word(r, crate::kv!(23 int)) as i32, word(r, crate::kv!(104 int)) as i32, word(r, crate::kv!(81 int)) as i32],
            flags: word(r, crate::kv!(113 int)),
            alarm: word(r, crate::kv!(55 int)) as u8,
            lock: word(r, crate::kv!(114 int)) as u8,
        })
        .collect())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Garage {
    pub floats: [f32; 8],
    pub flags: u32,
    pub kind: u32,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimeCycleBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub percentage: f32,
    pub range: f32,
    pub words: [u32; 2],
    pub modifier_hash: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LodGroup {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub byte: u8,
    pub models: Vec<(u32, String)>,
}

#[allow(non_snake_case)]
#[inline]
pub fn SLOW_ZONES_MAX() -> usize { crate::kv!(379 int) }

pub fn slow_zone_turn(vehicle_id: u16, frame: u32) -> bool {
    (vehicle_id as u32).wrapping_add(frame) & crate::kv!(189 int) == 0
}

pub fn in_slow_zone(zones: &[([f32; 3], [f32; 3])], p: [f32; 3]) -> bool {
    zones.iter().take(SLOW_ZONES_MAX()).any(|(lo, hi)| (0..crate::kv!(0 int)).all(|k| !(lo[k] > p[k]) && !(p[k] > hi[k])))
}

pub fn slow_zone_drag(drag: f32, bike: bool, slow: bool, network: bool, player: f32) -> f32 {
    let zone = match (slow, bike, network) {
        (false, ..) => drag,
        (true, false, false) => drag * crate::kv!(12),
        (true, false, true) => drag * crate::kv!(138),
        (true, true, false) => drag * crate::kv!(80),
        (true, true, true) => drag,
    };
    let forced = player * drag;
    if player > 0.0 && forced > zone { forced } else { zone }
}

pub use crate::lodtree::LodGroups;

pub fn read_xpl_extras(d: &[u8]) -> Result<(Vec<Garage>, Vec<TimeCycleBox>, Vec<([f32; 3], [f32; 3])>, Vec<LodGroup>), String> {
    let x = Xpl::read(d)?;
    let text = |b: &[u8]| String::from_utf8_lossy(b.split(|&c| c == 0).next().unwrap_or_default()).into_owned();
    let garages = records(x.grge, crate::kv!(93 int))
        .map(|r| Garage { floats: std::array::from_fn(|k| float(r, k)), flags: word(r, crate::kv!(23 int)), kind: word(r, crate::kv!(104 int)), name: text(&r[crate::kv!(95 int)..crate::kv!(93 int)]) })
        .collect();
    let tcyc = records(x.tcyc, crate::kv!(415 int))
        .map(|r| TimeCycleBox {
            min: [float(r, 0), float(r, 1), float(r, 2)],
            max: [float(r, crate::kv!(0 int)), float(r, crate::kv!(1 int)), float(r, crate::kv!(2 int))],
            percentage: float(r, crate::kv!(6 int)),
            range: float(r, crate::kv!(22 int)),
            words: [word(r, crate::kv!(23 int)), word(r, crate::kv!(104 int))],
            modifier_hash: word(r, crate::kv!(81 int)),
        })
        .collect();
    let slow = records(x.slow, crate::kv!(107 int)).map(|r| ([float(r, 0), float(r, 1), float(r, 2)], [float(r, crate::kv!(0 int)), float(r, crate::kv!(1 int)), float(r, crate::kv!(2 int))])).collect();
    let lodm = records(x.lodm, crate::kv!(780 int))
        .map(|r| {
            let head = word(r, crate::kv!(6 int));
            let n = ((head & crate::kv!(189 int)) as usize).min(crate::kv!(81 int));
            LodGroup {
                min: [float(r, 0), float(r, 1), float(r, 2)],
                max: [float(r, crate::kv!(0 int)), float(r, crate::kv!(1 int)), float(r, crate::kv!(2 int))],
                byte: (head >> crate::kv!(107 int)) as u8,
                models: (0..n).map(|k| (word(r, crate::kv!(22 int) + k), text(&r[crate::kv!(330 int) + k * crate::kv!(7 int)..crate::kv!(203 int) + k * crate::kv!(7 int)]))).collect(),
            }
        })
        .collect();
    Ok((garages, tcyc, slow, lodm))
}

pub use crate::ide::*;

pub fn read_water(text: &str) -> Vec<[[f32; 3]; 4]> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<f32> = line.split_whitespace().filter_map(|t| t.parse().ok()).collect();
            let c = |v: usize| Some([*f.get(v * crate::kv!(22 int))?, *f.get(v * crate::kv!(22 int) + 1)?, *f.get(v * crate::kv!(22 int) + 2)?]);
            Some([c(0)?, c(1)?, c(2)?, c(crate::kv!(0 int))?]).filter(|q| q[0] != q[crate::kv!(0 int)])
        })
        .collect()
}

