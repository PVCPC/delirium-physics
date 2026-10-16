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
    pub ide_flags: u32,
    pub anim: Option<String>,
    pub kind: u8,
}

impl ObjectDef {
    pub fn model_flags(&self) -> u32 {
        const MOVES: [(u32, u32); 14] = [
            (0x20000, 4), (0x2000, 0x1000), (0x800, 0x10), (0x20, 0x8000), (0x80, 0x20000), (0x100_0000, 0x10000), (0x400_0000, 0x40000),
            (0x800_0000, 0x80000), (0x1000_0000, 0x40_0000), (0x40_0000, 0x4000), (0x80_0000, 0x400), (0x2000_0000, 0x800), (0x200, 0x10_0000),
            (0x400, 0x20_0000),
        ];
        let mut out = (self.kind as u32) << 24;
        for (from, to) in MOVES {
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
            b'A'..=b'Z' => b + 32,
            b'\\' => b'/',
            _ => b,
        };
        h = h.wrapping_add(c as i8 as i32 as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
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

const XPL_SECTIONS: [(usize, usize); 11] =
    [(0x04, 0x30), (0x08, 0x44), (0x0C, 0x30), (0x10, 0x38), (0x14, 0x2C), (0x18, 0x2C), (0x1C, 0x14), (0x40, 0x84), (0x24, 0x40), (0x28, 0x184), (0x2C, 0x18)];

fn be32(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap()))
}

impl<'a> Xpl<'a> {
    pub fn read(d: &'a [u8]) -> Result<Xpl<'a>, String> {
        let version = be32(d, 0).ok_or("Truncated xpl")?;
        if version != 3 {
            return Err(format!("Unknown xpl version {version}"));
        }
        let mut at = 0x44;
        let mut parts: Vec<&[u8]> = vec![];
        for (head, size) in XPL_SECTIONS {
            let len = be32(d, head).ok_or("Truncated xpl")? as usize * size;
            parts.push(d.get(at..at + len).ok_or("Truncated xpl")?);
            at += len;
        }
        Ok(Xpl {
            version, inst: parts[0], zone: parts[1], grge: parts[2], cars: parts[3], tcyc: parts[4], vnod: parts[5], link: parts[6],
            dropped: parts[7], mlo: parts[8], lodm: parts[9], slow: parts[10],
        })
    }
}

fn records(d: &[u8], size: usize) -> impl Iterator<Item = &[u8]> {
    d.chunks_exact(size)
}
fn word(r: &[u8], k: usize) -> u32 {
    be32(r, k * 4).unwrap_or(0)
}
fn float(r: &[u8], k: usize) -> f32 {
    f32::from_bits(word(r, k))
}

pub fn read_xpl(d: &[u8]) -> Result<Vec<Instance>, String> {
    Ok(records(Xpl::read(d)?.inst, 48)
        .map(|r| Instance {
            position: [float(r, 0), float(r, 1), float(r, 2)],
            rotation: [float(r, 3), float(r, 4), float(r, 5), float(r, 6)],
            model_hash: word(r, 7),
            flags: word(r, 8),
            lod: word(r, 9) as i32,
            draw_distance: float(r, 11),
        })
        .collect())
}

#[derive(Clone)]
pub struct CarGen {
    pub position: [f32; 3],
    pub dir: [f32; 2],
    pub model_hash: u32,
    pub colours: [i32; 4],
    pub flags: u32,
}

pub fn read_xpl_cars(d: &[u8]) -> Result<Vec<CarGen>, String> {
    Ok(records(Xpl::read(d)?.cars, 56)
        .map(|r| CarGen {
            position: [float(r, 0), float(r, 1), float(r, 2)],
            dir: [float(r, 3), float(r, 4)],
            model_hash: word(r, 6),
            colours: [word(r, 7) as i32, word(r, 8) as i32, word(r, 9) as i32, word(r, 10) as i32],
            flags: word(r, 11),
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

pub const SLOW_ZONES_MAX: usize = 86;

pub fn slow_zone_turn(vehicle_id: u16, frame: u32) -> bool {
    (vehicle_id as u32).wrapping_add(frame) & 0xF == 0
}

pub fn in_slow_zone(zones: &[([f32; 3], [f32; 3])], p: [f32; 3]) -> bool {
    zones.iter().take(SLOW_ZONES_MAX).any(|(lo, hi)| (0..3).all(|k| !(lo[k] > p[k]) && !(p[k] > hi[k])))
}

pub fn slow_zone_drag(drag: f32, bike: bool, slow: bool, network: bool, player: f32) -> f32 {
    let zone = match (slow, bike, network) {
        (false, ..) => drag,
        (true, false, false) => drag * 1.5,
        (true, false, true) => drag * 1.1,
        (true, true, false) => drag * 1.3,
        (true, true, true) => drag,
    };
    let forced = player * drag;
    if player > 0.0 && forced > zone { forced } else { zone }
}

pub use crate::lodtree::LodGroups;

pub fn read_xpl_extras(d: &[u8]) -> Result<(Vec<Garage>, Vec<TimeCycleBox>, Vec<([f32; 3], [f32; 3])>, Vec<LodGroup>), String> {
    let x = Xpl::read(d)?;
    let text = |b: &[u8]| String::from_utf8_lossy(b.split(|&c| c == 0).next().unwrap_or_default()).into_owned();
    let garages = records(x.grge, 0x30)
        .map(|r| Garage { floats: std::array::from_fn(|k| float(r, k)), flags: word(r, 8), kind: word(r, 9), name: text(&r[0x28..0x30]) })
        .collect();
    let tcyc = records(x.tcyc, 0x2C)
        .map(|r| TimeCycleBox {
            min: [float(r, 0), float(r, 1), float(r, 2)],
            max: [float(r, 3), float(r, 4), float(r, 5)],
            percentage: float(r, 6),
            range: float(r, 7),
            words: [word(r, 8), word(r, 9)],
            modifier_hash: word(r, 10),
        })
        .collect();
    let slow = records(x.slow, 0x18).map(|r| ([float(r, 0), float(r, 1), float(r, 2)], [float(r, 3), float(r, 4), float(r, 5)])).collect();
    let lodm = records(x.lodm, 0x184)
        .map(|r| {
            let head = word(r, 6);
            let n = ((head & 0xF) as usize).min(10);
            LodGroup {
                min: [float(r, 0), float(r, 1), float(r, 2)],
                max: [float(r, 3), float(r, 4), float(r, 5)],
                byte: (head >> 24) as u8,
                models: (0..n).map(|k| (word(r, 7 + k), text(&r[0x44 + k * 32..0x64 + k * 32]))).collect(),
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
            let c = |v: usize| Some([*f.get(v * 7)?, *f.get(v * 7 + 1)?, *f.get(v * 7 + 2)?]);
            Some([c(0)?, c(1)?, c(2)?, c(3)?]).filter(|q| q[0] != q[3])
        })
        .collect()
}

