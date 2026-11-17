use crate::resource::{system_size, Resource};

#[derive(Clone)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub levels: u32,
    pub data: Vec<u8>,
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or("Pointer out of range".into())
}

fn ptr(r: &Resource, p: u32) -> Result<usize, String> {
    let off = (p & 0x0FFF_FFFF) as usize;
    match p >> 28 {
        5 => Ok(off),
        6 => Ok(system_size(r.flags) + off),
        _ => Err(format!("Bad resource pointer {p:08X}")),
    }
}

fn block_info(format: u32) -> Option<(u32, usize)> {
    Some(match format {
        0x12 | 0x3B => (4, 8),
        0x13 | 0x14 | 0x31 => (4, 16),
        0x02 => (1, 1),
        0x04 | 0x05 | 0x0F => (1, 2),
        0x06 => (1, 4),
        _ => return None,
    })
}

fn tiled_offset(x: u32, y: u32, pitch: u32, bpp: usize) -> usize {
    let log_bpp = bpp.trailing_zeros();
    let aligned = (pitch + 31) & !31;
    let macro_ = ((x >> 5) + (y >> 5) * (aligned >> 5)) << (log_bpp + 7);
    let micro = ((x & 7) + ((y & 6) << 2)) << log_bpp;
    let offset = macro_ + ((micro & !15) << 1) + (micro & 15) + ((y & 8) << (3 + log_bpp)) + ((y & 1) << 4);
    ((((offset & !511) << 3) + ((offset & 448) << 2) + (offset & 63) + ((y & 16) << 7)
        + (((((y & 8) >> 2) + (x >> 3)) & 3) << 6)) >> log_bpp) as usize
}

#[allow(clippy::too_many_arguments)]
fn read_level(d: &[u8], at: usize, endian: u32, tiled: bool, pitch: u32, rows: u32, (ox, oy): (u32, u32),
              (bw, bh): (u32, u32), bpp: usize) -> Option<Vec<u8>> {
    let len = (pitch.next_multiple_of(32) * rows.next_multiple_of(32)) as usize * bpp;
    let mut src = d.get(at..(at + len).min(d.len()))?.to_vec();
    swap(&mut src, endian);
    let mut out = vec![0; (bw * bh) as usize * bpp];
    for y in 0..bh {
        for x in 0..bw {
            let (sx, sy) = (x + ox, y + oy);
            let s = if tiled { tiled_offset(sx, sy, pitch, bpp) } else { (sy * pitch + sx) as usize } * bpp;
            let o = (y * bw + x) as usize * bpp;
            out[o..o + bpp].copy_from_slice(src.get(s..s + bpp)?);
        }
    }
    Some(out)
}

fn log2_ceil(x: u32) -> u32 {
    x.next_power_of_two().trailing_zeros()
}

fn packed_offset(width: u32, height: u32, level: u32, block: u32) -> (u32, u32) {
    let (lw, lh) = (log2_ceil(width), log2_ceil(height));
    let first = lw.min(lh).saturating_sub(4);
    let p = level - first;
    let (x, y) = match (p < 3, lw > lh) {
        (true, true) => (0, 16 >> p),
        (true, false) => (16 >> p, 0),
        (false, true) => ((1 << (lw - first)) >> (p - 2), 0),
        (false, false) => (0, (1 << (lh - first)) >> (p - 2)),
    };
    (x / block, y / block)
}

fn swap(data: &mut [u8], endian: u32) {
    match endian {
        1 => data.chunks_exact_mut(2).for_each(|c| c.swap(0, 1)),
        2 => data.chunks_exact_mut(4).for_each(|c| c.reverse()),
        3 => data.chunks_exact_mut(4).for_each(|c| { c.swap(0, 2); c.swap(1, 3) }),
        _ => {}
    }
}

pub fn read_dictionary(r: &Resource) -> Result<Vec<Texture>, String> {
    read_dictionary_at(r, 0)
}

pub fn read_dictionary_at(r: &Resource, at: usize) -> Result<Vec<Texture>, String> {
    let d = &r.data;
    let count = be32(d, at + 0x1C)? >> 16;
    if count == 0 {
        return Ok(vec![]);
    }
    let list = ptr(r, be32(d, at + 0x18)?)?;
    (0..count as usize)
        .map(|i| {
            let t = ptr(r, be32(d, list + i * 4)?)?;
            let name_at = ptr(r, be32(d, t + 0x14)?)?;
            let name = d.get(name_at..).ok_or("texture name past end")?.split(|&b| b == 0).next().unwrap_or_default();
            let name = String::from_utf8_lossy(name).trim_start_matches("pack:/").to_string();
            let fetch = ptr(r, be32(d, t + 0x18)?)? + 0x1C;
            let [w0, w1, w2, _, w4, w5] = std::array::from_fn(|k| be32(d, fetch + k * 4).unwrap_or(0));
            let (format, endian, tiled) = (w1 & 0x3F, (w1 >> 6) & 3, w0 >> 31 == 1);
            let (width, height) = ((w2 & 0x1FFF) + 1, ((w2 >> 13) & 0x1FFF) + 1);
            let mut data = vec![];
            let mut levels = 0;
            if let Some((block, bpp)) = block_info(format) {
                let surface = |at, pitch, rows, origin, level: u32| {
                    let size = ((width >> level).max(1).div_ceil(block), (height >> level).max(1).div_ceil(block));
                    read_level(d, at, endian, tiled, pitch, rows, origin, size, bpp)
                };
                let base_pitch = (((w0 >> 22) & 0x1FF) * 32).div_ceil(block).next_multiple_of(32);
                let packed_level = if (w5 >> 11) & 1 == 1 { log2_ceil(width.min(height)).saturating_sub(4) } else { u32::MAX };
                let origin = |level: u32| if level >= packed_level { packed_offset(width, height, level, block) } else { (0, 0) };
                let base = ptr(r, w1 & 0xFFFF_F000)?;
                data = surface(base, base_pitch, height.div_ceil(block), origin(0), 0).ok_or_else(|| format!(
                    "{name} {width}x{height} fmt {format:#X}: base level past end of resource (base {base:#X}, total {:#X})", d.len()))?;
                levels = 1;
                let mip_base = if w5 >> 12 != 0 { ptr(r, w5 & 0xFFFF_F000).ok() } else { None };
                let max_level = if mip_base.is_some() { ((w4 >> 6) & 0xF).min(31 - width.max(height).leading_zeros()) } else { 0 };
                let dims = |storage: u32| {
                    let pitch = (width.next_power_of_two() >> storage).max(1).div_ceil(block).next_multiple_of(32);
                    let pitch = if tiled { pitch } else { (pitch * bpp as u32).next_multiple_of(256) / bpp as u32 };
                    (pitch, (height.next_power_of_two() >> storage).max(1).div_ceil(block).next_multiple_of(32))
                };
                for level in 1..=max_level {
                    let storage = level.min(packed_level);
                    let offset: usize = (1..storage).map(|l| { let (p, r) = dims(l); ((p * r) as usize * bpp).next_multiple_of(4096) }).sum();
                    let (pitch, rows) = dims(storage);
                    let Some(mip) = surface(mip_base.unwrap() + offset, pitch, rows, origin(level), level) else { break };
                    data.extend(mip);
                    levels += 1;
                }
            }
            Ok(Texture { name, width, height, format, levels, data })
        })
        .collect()
}

pub fn dxt5_masked(t: &Texture, keep: impl Fn(u32, u32) -> bool) -> Option<Texture> {
    if t.format != 0x14 || t.data.is_empty() {
        return None;
    }
    let bw = t.width.div_ceil(4);
    let mut data = t.data[..(bw * t.height.div_ceil(4)) as usize * 16].to_vec();
    for (b, block) in data.chunks_exact_mut(16).enumerate() {
        let (a0, a1) = (block[0] as u32, block[1] as u32);
        let palette: [u32; 8] = std::array::from_fn(|i| match (i, a0 > a1) {
            (0, _) => a0,
            (1, _) => a1,
            (i, true) => ((8 - i as u32) * a0 + (i as u32 - 1) * a1) / 7,
            (6, false) => 0,
            (7, false) => 255,
            (i, false) => ((6 - i as u32) * a0 + (i as u32 - 1) * a1) / 5,
        });
        let bits = block[2..8].iter().rev().fold(0u64, |v, &x| v << 8 | x as u64);
        let (bx, by) = (b as u32 % bw * 4, b as u32 / bw * 4);
        let alpha: [u32; 16] = std::array::from_fn(|p| if keep(bx + p as u32 % 4, by + p as u32 / 4) { palette[(bits >> (3 * p) & 7) as usize] } else { 0 });
        let top = alpha.iter().copied().max().unwrap_or(0).max(1);
        let ramp = |i: u64| match i { 0 => top, 1 => 0, i => (8 - i as u32) * top / 7 };
        let bits = alpha.iter().enumerate().fold(0u64, |v, (p, &a)| {
            let i = (0..8).min_by_key(|&i| ramp(i).abs_diff(a)).unwrap();
            v | i << (3 * p)
        });
        block[0] = top as u8;
        block[1] = 0;
        block[2..8].copy_from_slice(&bits.to_le_bytes()[..6]);
    }
    Some(Texture { levels: 1, data, ..t.clone() })
}

pub fn to_dds(t: &Texture) -> Option<Vec<u8>> {
    let (fourcc, rgb): (&[u8; 4], Option<[u32; 5]>) = match t.format {
        0x12 => (b"DXT1", None),
        0x13 => (b"DXT3", None),
        0x14 => (b"DXT5", None),
        0x31 => (b"ATI2", None),
        0x3B => (b"ATI1", None),
        0x06 => (&[0; 4], Some([32, 0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000])),
        0x04 => (&[0; 4], Some([16, 0xF800, 0x07E0, 0x001F, 0])),
        0x05 => (&[0; 4], Some([16, 0x7C00, 0x03E0, 0x001F, 0x8000])),
        0x0F => (&[0; 4], Some([16, 0x0F00, 0x00F0, 0x000F, 0xF000])),
        0x02 => (&[0; 4], Some([8, 0xFF, 0, 0, 0])),
        _ => return None,
    };
    if t.data.is_empty() {
        return None;
    }
    let mut h = vec![0u32; 32];
    h[0] = u32::from_le_bytes(*b"DDS ");
    h[1] = 124;
    h[2] = 0x1 | 0x2 | 0x4 | 0x1000 | 0x20000;
    h[3] = t.height;
    h[4] = t.width;
    h[7] = t.levels;
    h[19] = 32;
    match rgb {
        None => {
            h[20] = 0x4;
            h[21] = u32::from_le_bytes(*fourcc);
        }
        Some([bits, r, g, b, a]) => {
            h[20] = if bits == 8 { 0x20000 } else { 0x40 | if a != 0 { 0x1 } else { 0 } };
            h[22..27].copy_from_slice(&[bits, r, g, b, a]);
        }
    }
    h[27] = 0x1000 | if t.levels > 1 { 0x8 | 0x40_0000 } else { 0 };
    let mut out: Vec<u8> = h.iter().flat_map(|v| v.to_le_bytes()).collect();
    out.extend_from_slice(&t.data);
    Some(out)
}

