use crate::resource::Resource;

pub const BONE_TRANSLATION: u8 = 0;
pub const BONE_ROTATION: u8 = 1;
pub const MOVER_TRANSLATION: u8 = 5;
pub const MOVER_ROTATION: u8 = 6;

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
    d.get(at..at + 4).map(|b| crate::u32_of(b.try_into().unwrap())).ok_or("Read out of range".into())
}
fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2).map(|b| crate::u16_of([b[0], b[1]])).ok_or("Read out of range".into())
}
fn bef(d: &[u8], at: usize) -> Result<f32, String> {
    be32(d, at).map(f32::from_bits)
}
fn sys(p: u32) -> Result<usize, String> {
    if p >> 28 == 5 { Ok((p & 0x0FFF_FFFF) as usize) } else { Err(format!("Bad animation pointer {p:08X}")) }
}

pub fn read_dictionary(r: &Resource) -> Result<Vec<Animation>, String> {
    let d = &r.data;
    let count = be16(d, 0x1C)? as usize;
    let (hashes, list) = (sys(be32(d, 0x10)?)?, sys(be32(d, 0x18)?)?);
    (0..count).map(|i| read_animation(d, sys(be32(d, list + i * 4)?)?, be32(d, hashes + i * 4)?)).collect()
}

fn read_animation(d: &[u8], a: usize, hash: u32) -> Result<Animation, String> {
    let (frames, per_block) = (be16(d, a + 8)? as usize, be16(d, a + 0x0A)?.max(1) as usize);
    let name = sys(be32(d, a + 0x1C)?).map(|at| d[at..].split(|&c| c == 0).next().unwrap_or_default()).unwrap_or_default();
    let name = String::from_utf8_lossy(name).trim_start_matches("pack:/").trim_end_matches(".anim").to_string();
    let (tracks, count) = (sys(be32(d, a + 0x20)?)?, be16(d, a + 0x24)? as usize);
    let (mut out, mut events, mut mover_rotation) = (vec![], vec![], None);
    for k in 0..count {
        let t = sys(be32(d, tracks + k * 4)?)?;
        let blocks = sys(be32(d, t + 8)?)?;
        let block_count = be16(d, t + 0x0C)? as usize;
        if d[t] == 0x80 && be16(d, t + 2)? == 0 {
            let per = be16(d, t + 4)?.max(1) as usize;
            let chans = (0..block_count).map(|j| sys(be32(d, sys(be32(d, blocks + j * 4)?)? + 4)?)).collect::<Result<Vec<_>, _>>()?;
            events = (0..frames).map(|f| { let j = (f / per).min(chans.len() - 1); int_at(d, chans[j], f - j * per) }).collect::<Result<_, _>>()?;
            continue;
        }
        let mut parts = vec![];
        for j in 0..block_count {
            let block = sys(be32(d, blocks + j * 4)?)?;
            let mut values = vec![];
            for c in 0..be32(d, block + 0x14)? as usize {
                match channel(d, sys(be32(d, block + 4 + c * 4)?)?)? {
                    Some(v) => values.extend(v),
                    None => break,
                }
            }
            parts.push(values);
        }
        let values = match parts.len() {
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
        };
        let kind = d[t];
        if kind == MOVER_ROTATION && values.len() == 4 {
            mover_rotation = Some(Track { kind, bone: be16(d, t + 2)?, values });
            continue;
        }
        let complete = match kind {
            BONE_ROTATION => values.len() == 4,
            BONE_TRANSLATION | MOVER_TRANSLATION => values.len() == 3,
            _ => false,
        };
        if complete {
            out.push(Track { kind, bone: be16(d, t + 2)?, values });
        }
    }
    Ok(Animation { name, hash, frames, duration: bef(d, a + 0x0C)?, tracks: out, mover_rotation, events })
}

fn int_at(d: &[u8], ch: usize, frame: usize) -> Result<u32, String> {
    match *d.get(ch + 5).ok_or("Read out of range")? {
        7 => be32(d, sys(be32(d, ch + 8)?)? + frame.min((be16(d, ch + 0x0C)? as usize).saturating_sub(1)) * 4),
        11 => be32(d, ch + 8),
        12 => {
            let (values, n) = (sys(be32(d, ch + 8)?)?, be16(d, ch + 0x0C)? as usize);
            let (bits, k, mut pos, mut left) = (sys(be32(d, ch + 0x10)?)?, d[ch + 0x18] as u32, 0usize, frame as i64);
            for i in 0..n {
                left -= rle(d, bits, k, &mut pos)? as i64;
                if left < 0 {
                    return be32(d, values + i * 4);
                }
            }
            be32(d, values + n.saturating_sub(1) * 4)
        }
        t => Err(format!("Event channel type {t}")),
    }
}

fn rle(d: &[u8], words: usize, k: u32, pos: &mut usize) -> Result<i32, String> {
    let mut bit = || -> Result<u32, String> { let b = be32(d, words + *pos / 32 * 4)? >> (*pos % 32) & 1; *pos += 1; Ok(b) };
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
        let a = d * from * 30.0;
        let mut b = d * to * 30.0;
        let (mut f, mut end) = ((a as i32 + 1) as f32, (b as i32) as f32);
        let (mut events, mut jumped) = (0, None);
        if b < a {
            return (0, None);
        }
        while f <= end {
            let v = self.event_at(f - 0.5);
            events |= v;
            f += 1.0;
            let section = if loop_a && v & 0x2 != 0 { Some((0x1, 0x2)) } else if loop_b && v & 0x8 != 0 { Some((0x4, 0x8)) } else { None };
            if let Some((start, stop)) = section {
                let (start, stop) = (self.event_phase(start).unwrap_or(0.0), self.event_phase(stop).unwrap_or(0.0));
                let at = if d > 0.0 { b * (1.0 / 30.0) / d } else { 0.0 };
                let phase = unit(at - stop + start);
                (f, end, jumped) = (d * start * 30.0, d * phase * 30.0, Some(phase));
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
        let (a, b) = (self.duration * from * 30.0, self.duration * to * 30.0);
        (0..self.duration.mul_add(30.0, 1.0) as i32).map(|i| i as f32).filter(move |&x| x >= a && x <= b).map(|x| self.event_at(x - 0.5))
    }

    pub fn event_phase(&self, bits: u32) -> Option<f32> {
        (0..self.duration.mul_add(30.0, 1.0) as i32).map(|i| i as f32).filter(|&f| self.event_at(f - 0.5) & bits != 0).last()
            .map(|f| if self.duration > 0.0 { (f * (1.0 / 30.0) / self.duration).clamp(0.0, 1.0) } else { 0.0 })
    }

    pub fn first_event(&self, bits: u32, from: f32, to: f32) -> Option<f32> {
        let (a, b) = (self.duration * from * 30.0, self.duration * to * 30.0);
        (0..self.duration.mul_add(30.0, 1.0) as i32).map(|i| i as f32).find(|&x| x >= a && x <= b && self.event_at(x - 0.5) & bits != 0)
            .map(|x| if self.duration > 0.0 { x * (1.0 / 30.0) / self.duration } else { 0.0 })
    }

    fn event_at(&self, t: f32) -> u32 {
        let f = (t.max(0.0) + 0.5) as usize;
        self.events.get(f.min(self.events.len().saturating_sub(1))).copied().unwrap_or(0)
    }
}

fn channel(d: &[u8], ch: usize) -> Result<Option<Vec<Vec<f32>>>, String> {
    let floats = |at: usize, n: usize| (0..n).map(|i| bef(d, at + i * 4)).collect::<Result<Vec<_>, _>>();
    Ok(Some(match *d.get(ch + 5).ok_or("Read out of range")? {
        1 => vec![floats(sys(be32(d, ch + 8)?)?, be16(d, ch + 0x0C)? as usize)?],
        4 => vec![vec![bef(d, ch + 8)?]],
        6 => {
            let (data, bits, n) = (sys(be32(d, ch + 8)?)?, be32(d, ch + 0x0C)? as usize, be32(d, ch + 0x10)? as usize);
            let (scale, offset) = (bef(d, ch + 0x14)?, bef(d, ch + 0x18)?);
            if bits == 0 || bits > 32 {
                return Err(format!("Quantized channel with {bits} bits"));
            }
            let words = (0..(n * bits).div_ceil(32)).map(|k| be32(d, data + k * 4)).collect::<Result<Vec<_>, _>>()?;
            let value = |i: usize| {
                let (pos, mut v) = (i * bits, 0u64);
                let lo = words[pos / 32] as u64 | (words.get(pos / 32 + 1).copied().unwrap_or(0) as u64) << 32;
                v |= (lo >> (pos % 32)) & ((1u64 << bits) - 1);
                v as f32 * scale + offset
            };
            vec![(0..n).map(value).collect()]
        }
        9 => floats(sys(be32(d, ch + 8)?)?, 4)?.into_iter().map(|v| vec![v]).collect(),
        0x0D => floats(sys(be32(d, ch + 8)?)?, 3)?.into_iter().map(|v| vec![v]).collect(),
        _ => return Ok(None),
    }))
}

