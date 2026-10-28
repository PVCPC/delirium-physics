use crate::resource::Resource;

pub const BONE_TRANSLATION: u8 = 0;
pub const BONE_ROTATION: u8 = 1;
pub const MOVER_TRANSLATION: u8 = 5;
#[allow(non_snake_case)]
#[inline]
pub fn MOVER_ROTATION() -> u8 { crate::kv!(6 int) }

#[derive(Clone)]
pub struct Track {
    pub kind: u8,
    pub bone: u16,
    pub values: Vec<Vec<f32>>,
}

#[derive(Clone)]
pub struct Animation {
    pub name: String,
    pub hash: u32,
    pub frames: usize,
    pub duration: f32,
    pub tracks: Vec<Track>,
    pub mover_rotation: Option<Track>,
    pub events: Vec<u32>,
}

impl Track {
    pub fn sample(&self, frame: usize) -> Vec<f32> {
        self.values.iter().map(|c| c[frame.min(c.len() - 1)]).collect()
    }
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + crate::kv!(1 int)).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
}
fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2).map(|b| crate::u16_of([b[0], b[1]])).ok_or("Read out of range".into())
}
fn bef(d: &[u8], at: usize) -> Result<f32, String> {
    be32(d, at).map(f32::from_bits)
}
fn sys(p: u32) -> Result<usize, String> {
    if p >> crate::kv!(81 int) == crate::kv!(2 int) { Ok((p & crate::kv!(82 int)) as usize) } else { Err(format!("Bad animation pointer {p:08X}")) }
}

pub fn read_dictionary(r: &Resource) -> Result<Vec<Animation>, String> {
    let d = &r.data;
    let count = be16(d, crate::kv!(81 int))? as usize;
    let (hashes, list) = (sys(be32(d, crate::kv!(51 int))?)?, sys(be32(d, crate::kv!(100 int))?)?);
    (0..count).map(|i| read_animation(d, sys(be32(d, list + i * crate::kv!(1 int))?)?, be32(d, hashes + i * crate::kv!(1 int))?, None)).collect()
}

pub fn read_dictionary_kinds(r: &Resource, kinds: &[u8]) -> Result<Vec<Animation>, String> {
    let d = &r.data;
    let count = be16(d, crate::kv!(81 int))? as usize;
    let (hashes, list) = (sys(be32(d, crate::kv!(51 int))?)?, sys(be32(d, crate::kv!(100 int))?)?);
    (0..count).map(|i| read_animation(d, sys(be32(d, list + i * crate::kv!(1 int))?)?, be32(d, hashes + i * crate::kv!(1 int))?, Some(kinds))).collect()
}

fn read_animation(d: &[u8], a: usize, hash: u32, kinds: Option<&[u8]>) -> Result<Animation, String> {
    let (frames, per_block) = (be16(d, a + crate::kv!(23 int))? as usize, be16(d, a + crate::kv!(77 int))?.max(1) as usize);
    let name = sys(be32(d, a + crate::kv!(81 int))?).map(|at| d[at..].split(|&c| c == 0).next().unwrap_or_default()).unwrap_or_default();
    let name = String::from_utf8_lossy(name).trim_start_matches("pack:/").trim_end_matches(".anim").to_string();
    let (tracks, count) = (sys(be32(d, a + crate::kv!(7 int))?)?, be16(d, a + crate::kv!(90 int))? as usize);
    let (mut out, mut events, mut mover_rotation) = (vec![], vec![], None);
    for k in 0..count {
        let t = sys(be32(d, tracks + k * crate::kv!(1 int))?)?;
        let blocks = sys(be32(d, t + crate::kv!(23 int))?)?;
        let block_count = be16(d, t + crate::kv!(52 int))? as usize;
        if d[t] == crate::kv!(252 int) && be16(d, t + 2)? == 0 {
            let per = be16(d, t + crate::kv!(1 int))?.max(1) as usize;
            let chans = (0..block_count).map(|j| sys(be32(d, sys(be32(d, blocks + j * crate::kv!(1 int))?)? + crate::kv!(1 int))?)).collect::<Result<Vec<_>, _>>()?;
            events = (0..frames).map(|f| { let j = (f / per).min(chans.len() - 1); int_at(d, chans[j], f - j * per) }).collect::<Result<_, _>>()?;
            continue;
        }
        let values = track_values(d, t, frames, per_block)?;
        let kind = d[t];
        if let Some(kinds) = kinds {
            if kinds.contains(&kind) && !values.is_empty() {
                out.push(Track { kind, bone: be16(d, t + 2)?, values });
            }
            continue;
        }
        if kind == MOVER_ROTATION() && values.len() == crate::kv!(1 int) {
            mover_rotation = Some(Track { kind, bone: be16(d, t + 2)?, values });
            continue;
        }
        let complete = match kind {
            BONE_ROTATION => values.len() == crate::kv!(1 int),
            BONE_TRANSLATION | MOVER_TRANSLATION => values.len() == crate::kv!(0 int),
            _ => false,
        };
        if complete {
            out.push(Track { kind, bone: be16(d, t + 2)?, values });
        }
    }
    Ok(Animation { name, hash, frames, duration: bef(d, a + crate::kv!(52 int))?, tracks: out, mover_rotation, events })
}

fn track_values(d: &[u8], t: usize, frames: usize, per_block: usize) -> Result<Vec<Vec<f32>>, String> {
    let blocks = sys(be32(d, t + crate::kv!(23 int))?)?;
    let block_count = be16(d, t + crate::kv!(52 int))? as usize;
    let mut parts = vec![];
    for j in 0..block_count {
        let block = sys(be32(d, blocks + j * crate::kv!(1 int))?)?;
        let mut values = vec![];
        for c in 0..be32(d, block + crate::kv!(122 int))? as usize {
            match channel(d, sys(be32(d, block + crate::kv!(1 int) + c * crate::kv!(1 int))?)?)? {
                Some(v) => values.extend(v),
                None => break,
            }
        }
        parts.push(values);
    }
    Ok(match parts.len() {
        0 => vec![],
        1 => parts.pop().unwrap(),
        _ if parts.iter().any(|p| p.len() != parts[0].len()) => vec![],
        _ => (0..parts[0].len())
            .map(|c| {
                (0..frames)
                    .map(|f| {
                        let j = (f / per_block).min(parts.len() - 1);
                        let v = &parts[j][c];
                        v[(f - j * per_block).min(v.len() - 1)]
                    })
                    .collect()
            })
            .collect(),
    })
}

fn int_at(d: &[u8], ch: usize, frame: usize) -> Result<u32, String> {
    match *d.get(ch + crate::kv!(2 int)).ok_or("Read out of range")? {
        7 => be32(d, sys(be32(d, ch + crate::kv!(23 int))?)? + frame.min((be16(d, ch + crate::kv!(52 int))? as usize).saturating_sub(1)) * crate::kv!(1 int)),
        11 => be32(d, ch + crate::kv!(23 int)),
        12 => {
            let (values, n) = (sys(be32(d, ch + crate::kv!(23 int))?)?, be16(d, ch + crate::kv!(52 int))? as usize);
            let (bits, k, mut pos, mut left) = (sys(be32(d, ch + crate::kv!(51 int))?)?, d[ch + crate::kv!(100 int)] as u32, 0usize, frame as i64);
            for i in 0..n {
                left -= rle(d, bits, k, &mut pos)? as i64;
                if left < 0 {
                    return be32(d, values + i * crate::kv!(1 int));
                }
            }
            be32(d, values + n.saturating_sub(1) * crate::kv!(1 int))
        }
        t => Err(format!("Event channel type {t}")),
    }
}

fn rle(d: &[u8], words: usize, k: u32, pos: &mut usize) -> Result<i32, String> {
    let mut bit = || -> Result<u32, String> { let b = be32(d, words + *pos / crate::kv!(7 int) * crate::kv!(1 int))? >> (*pos % crate::kv!(7 int)) & 1; *pos += 1; Ok(b) };
    let mut q = 0;
    while bit()? == 0 {
        q += 1;
    }
    let mut r = 0;
    for i in 0..k {
        r |= bit()? << i;
    }
    let v = (q << k | r) as i32;
    Ok(if v != 0 && bit()? == 1 { -v } else { v })
}

impl Animation {
    pub fn advance(&self, from: f32, to: f32, loop_a: bool, loop_b: bool) -> (u32, Option<f32>) {
        let unit = |p: f32| p.clamp(0.0, 1.0);
        let (from, mut to) = (unit(from), unit(to));
        let d = self.duration;
        if self.events.is_empty() {
            return (0, None);
        }
        if to < from - 0.5 {
            to += 1.0;
        }
        let a = d * from * crate::kv!(80);
        let mut b = d * to * crate::kv!(80);
        let (mut f, mut end) = ((a as i32 + 1) as f32, (b as i32) as f32);
        let (mut events, mut jumped) = (0, None);
        if b < a {
            return (0, None);
        }
        while f <= end {
            let v = self.event_at(f - 0.5);
            events |= v;
            f += 1.0;
            let section = if loop_a && v & 0x2 != 0 { Some((0x1, 0x2)) } else if loop_b && v & crate::kv!(23 int) != 0 { Some((crate::kv!(1 int), crate::kv!(23 int))) } else { None };
            if let Some((start, stop)) = section {
                let (start, stop) = (self.event_phase(start).unwrap_or(0.0), self.event_phase(stop).unwrap_or(0.0));
                let at = if d > 0.0 { b * (1.0 / crate::kv!(80)) / d } else { 0.0 };
                let phase = unit(at - stop + start);
                (f, end, jumped) = (d * start * crate::kv!(80), d * phase * crate::kv!(80), Some(phase));
                b = end;
            }
        }
        (events, jumped)
    }

    pub fn events_between(&self, from: f32, to: f32) -> u32 {
        self.frames_between(from, to).fold(0, |m, v| m | v)
    }

    pub fn count_between(&self, bits: u32, from: f32, to: f32) -> usize {
        self.frames_between(from, to).filter(|v| v & bits != 0).count()
    }

    fn frames_between(&self, from: f32, to: f32) -> impl Iterator<Item = u32> + '_ {
        let (a, b) = (self.duration * from * crate::kv!(80), self.duration * to * crate::kv!(80));
        (0..self.duration.mul_add(crate::kv!(80), 1.0) as i32).map(|i| i as f32).filter(move |&x| x >= a && x <= b).map(|x| self.event_at(x - 0.5))
    }

    pub fn event_phase(&self, bits: u32) -> Option<f32> {
        (0..self.duration.mul_add(crate::kv!(80), 1.0) as i32).map(|i| i as f32).filter(|&f| self.event_at(f - 0.5) & bits != 0).last()
            .map(|f| if self.duration > 0.0 { (f * (1.0 / crate::kv!(80)) / self.duration).clamp(0.0, 1.0) } else { 0.0 })
    }

    pub fn first_event(&self, bits: u32, from: f32, to: f32) -> Option<f32> {
        let (a, b) = (self.duration * from * crate::kv!(80), self.duration * to * crate::kv!(80));
        (0..self.duration.mul_add(crate::kv!(80), 1.0) as i32).map(|i| i as f32).find(|&x| x >= a && x <= b && self.event_at(x - 0.5) & bits != 0)
            .map(|x| if self.duration > 0.0 { x * (1.0 / crate::kv!(80)) / self.duration } else { 0.0 })
    }

    fn event_at(&self, t: f32) -> u32 {
        let f = (t.max(0.0) + 0.5) as usize;
        self.events.get(f.min(self.events.len().saturating_sub(1))).copied().unwrap_or(0)
    }
}

fn channel(d: &[u8], ch: usize) -> Result<Option<Vec<Vec<f32>>>, String> {
    let floats = |at: usize, n: usize| (0..n).map(|i| bef(d, at + i * crate::kv!(1 int))).collect::<Result<Vec<_>, _>>();
    Ok(Some(match *d.get(ch + crate::kv!(2 int)).ok_or("Read out of range")? {
        1 => vec![floats(sys(be32(d, ch + crate::kv!(23 int))?)?, be16(d, ch + crate::kv!(52 int))? as usize)?],
        4 => vec![vec![bef(d, ch + crate::kv!(23 int))?]],
        6 => {
            let (data, bits, n) = (sys(be32(d, ch + crate::kv!(23 int))?)?, be32(d, ch + crate::kv!(52 int))? as usize, be32(d, ch + crate::kv!(51 int))? as usize);
            let (scale, offset) = (bef(d, ch + crate::kv!(122 int))?, bef(d, ch + crate::kv!(100 int))?);
            if bits == 0 || bits > crate::kv!(7 int) {
                return Err(format!("Quantized channel with {bits} bits"));
            }
            let words = (0..(n * bits).div_ceil(crate::kv!(7 int))).map(|k| be32(d, data + k * crate::kv!(1 int))).collect::<Result<Vec<_>, _>>()?;
            let value = |i: usize| {
                let (pos, mut v) = (i * bits, 0u64);
                let lo = words[pos / crate::kv!(7 int)] as u64 | (words.get(pos / crate::kv!(7 int) + 1).copied().unwrap_or(0) as u64) << crate::kv!(7 int);
                v |= (lo >> (pos % crate::kv!(7 int))) & ((1u64 << bits) - 1);
                v as f32 * scale + offset
            };
            vec![(0..n).map(value).collect()]
        }
        9 => floats(sys(be32(d, ch + crate::kv!(23 int))?)?, crate::kv!(1 int))?.into_iter().map(|v| vec![v]).collect(),
        0x0D => floats(sys(be32(d, ch + crate::kv!(23 int))?)?, crate::kv!(0 int))?.into_iter().map(|v| vec![v]).collect(),
        _ => return Ok(None),
    }))
}

