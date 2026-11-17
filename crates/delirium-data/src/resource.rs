use crate::lzx::{LzxStream, STREAM_BLOCK_HEAD};
use crate::xex::lzx;

const MAGIC: u32 = 0x0543_5352;
const XCOMPRESS: u32 = 0x0FF5_12EF;

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
    let data = unpack(&raw[12..], system_size(flags) + graphics_size(flags))?;
    Ok(Resource { kind, flags, data })
}

fn unpack(input: &[u8], size: usize) -> Result<Vec<u8>, String> {
    if input.len() < STREAM_BLOCK_HEAD {
        return Err("Truncated resource stream".into());
    }
    let mut ctx = LzxStream::make_context();
    let mut stream = LzxStream::default();
    stream.reset(&mut ctx, 0, size);
    stream.input_left = input.len();
    stream.output_room = size;
    let mut out = vec![0u8; size];
    while stream.output_made < size {
        let (made, used) = (stream.output_made, stream.input_at);
        stream.step(&mut ctx, input, &mut out, &mut |_| false).map_err(|e| format!("Bad resource stream: {e:?}"))?;
        if stream.output_made == made && stream.input_at == used {
            return Err("Truncated resource stream".into());
        }
    }
    Ok(out)
}
