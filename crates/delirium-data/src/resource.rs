use crate::xex::lzx;

const MAGIC: u32 = 0x0543_5352;
const XCOMPRESS: u32 = 0x0FF5_12EF;
const FRAME: usize = 0x8000;
const WINDOW: usize = 0x20000;

pub struct Resource {
    pub kind: u32,
    pub flags: u32,
    pub data: Vec<u8>,
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or("Truncated resource".into())
}

fn segment(flags: u32, count_at: u32, shift_at: u32) -> usize {
    ((flags >> count_at) as usize & 0x7FF) << (((flags >> shift_at) & 0xF) + 8)
}
pub fn system_size(flags: u32) -> usize { segment(flags, 0, 11) }
pub fn graphics_size(flags: u32) -> usize { segment(flags, 15, 26) }

pub fn parse(raw: &[u8]) -> Result<Resource, String> {
    let le = |at| be32(raw, at).map(u32::swap_bytes);
    let (kind, flags) = match (be32(raw, 0)?, le(0)?) {
        (MAGIC, _) => (be32(raw, 4)?, be32(raw, 8)?),
        (_, MAGIC) => (le(4)?, le(8)?),
        (m, _) => return Err(format!("Not an RSC resource (magic {m:08X})")),
    };
    if raw.get(12) == Some(&0x78) {
        let data = miniz_oxide::inflate::decompress_to_vec_zlib(&raw[12..]).map_err(|e| format!("Bad zlib resource: {e:?}"))?;
        return Ok(Resource { kind, flags, data });
    }
    if be32(raw, 12)? != XCOMPRESS {
        return Err(format!("Unknown resource compression {:08X}", be32(raw, 12)?));
    }
    let end = 20 + be32(raw, 16)? as usize;
    let body = raw.get(20..end).ok_or("Truncated resource stream")?;
    let (mut stream, mut out_len, mut p) = (Vec::with_capacity(body.len()), 0, 0);
    while p < body.len() {
        let (frame_out, frame_in);
        if body[p] == 0xFF {
            let h = body.get(p + 1..p + 5).ok_or("Truncated frame header")?;
            (frame_out, frame_in) = (u16::from_be_bytes([h[0], h[1]]) as usize, u16::from_be_bytes([h[2], h[3]]) as usize);
            p += 5;
        } else {
            (frame_out, frame_in) = (FRAME, u16::from_be_bytes([body[p], *body.get(p + 1).ok_or("Truncated frame")?]) as usize);
            p += 2;
        }
        if frame_in == 0 {
            break;
        }
        stream.extend_from_slice(body.get(p..p + frame_in).ok_or("Truncated frame data")?);
        out_len += frame_out;
        p += frame_in;
    }
    let data = lzx::decompress(&stream, WINDOW, out_len, false)?;
    Ok(Resource { kind, flags, data })
}
