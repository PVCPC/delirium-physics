use crate::archive_key::ArchiveKey;

pub struct Entry {
    pub path: String,
    pub offset: u64,
    pub stored: u32,
    pub size: u32,
    pub compressed: bool,
    pub resource: Option<u32>,
}

fn word(d: &[u8], at: usize, be: bool) -> u32 {
    let b: [u8; 4] = d[at..at + 4].try_into().unwrap();
    if be { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }
}

pub fn read_toc(data: &[u8], key: &ArchiveKey) -> Result<Vec<Entry>, String> {
    let hashed = data.get(0..4) == Some(b"RPF3");
    if data.get(0..4) != Some(b"RPF2") && !hashed {
        return Err("Not an RPF2 or RPF3 archive".into());
    }
    let (toc_size, count, encrypted) = (word(data, 4, false) as usize, word(data, 8, false) as usize, word(data, 16, false));
    let mut toc = data.get(0x800..0x800 + toc_size).ok_or("Truncated RPF TOC")?.to_vec();
    if encrypted != 0 {
        key.decrypt(&mut toc);
    }
    let be = false;
    let names = toc.get(count * 16..).ok_or("RPF TOC smaller than its entries")?;
    let name = |i: usize| {
        if hashed {
            return format!("{:08X}", word(&toc, i * 16, be));
        }
        let at = word(&toc, i * 16, be) as usize;
        String::from_utf8_lossy(names.get(at..).unwrap_or_default().split(|&b| b == 0).next().unwrap_or_default()).into_owned()
    };
    let mut out = vec![];
    let mut stack = vec![(0usize, String::new())];
    while let Some((i, parent)) = stack.pop() {
        let at = i * 16;
        let (b, c, dword) = (word(&toc, at + 4, be), word(&toc, at + 8, be), word(&toc, at + 12, be));
        let path = if i == 0 { String::new() } else { format!("{parent}{}", name(i)) };
        if c & 0x8000_0000 != 0 {
            let first = (c & 0x7FFF_FFFF) as usize;
            let dword = if hashed { dword & 0x0FFF_FFFF } else { dword };
            if first + dword as usize > count {
                return Err("RPF directory points past the table".into());
            }
            for child in first..first + dword as usize {
                stack.push((child, if i == 0 { String::new() } else { format!("{path}/") }));
            }
        } else if dword & 0xC000_0000 == 0xC000_0000 {
            out.push(Entry { path, offset: (c & 0x7FFF_FF00) as u64, stored: b, size: b, compressed: false, resource: Some(dword) });
        } else {
            out.push(Entry { path, offset: c as u64, stored: dword & 0x3FFF_FFFF, size: b, compressed: dword & 0x4000_0000 != 0, resource: None });
        }
    }
    Ok(out)
}

pub fn read(data: &[u8], e: &Entry) -> Result<Vec<u8>, String> {
    let stored = data.get(e.offset as usize..e.offset as usize + e.stored as usize).ok_or("RPF entry out of range")?;
    if !e.compressed {
        return Ok(stored.to_vec());
    }
    miniz_oxide::inflate::decompress_to_vec(stored).map_err(|err| format!("{}: inflate failed: {err:?}", e.path))
}
