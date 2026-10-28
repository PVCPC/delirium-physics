use crate::archive_key::ArchiveKey;

const MAGIC: u32 = 0xA94E_2A52;
const BLOCK: u64 = 2048;

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub size_field: u32,
    pub resource_type: u32,
    pub offset: u64,
    pub used_blocks: u16,
    pub flags: u16,
}

impl Entry {
    pub fn size(&self) -> u32 {
        self.used_blocks as u32 * 2048 - (self.flags & 0x7FF) as u32
    }

    pub fn is_resource(&self) -> bool {
        self.flags & 0x2000 != 0
    }
}

fn le32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(d[at..at + 4].try_into().unwrap())
}

pub fn table_len(head: &[u8], key: Option<&ArchiveKey>) -> Option<usize> {
    let mut header = head.get(..20)?.to_vec();
    if le32(&header, 0) != MAGIC {
        key?.decrypt(&mut header);
    }
    (le32(&header, 0) == MAGIC).then(|| 20 + (le32(&header, 12) as usize).next_multiple_of(16))
}

pub fn read_table(data: &[u8], key: Option<&ArchiveKey>) -> Result<Vec<Entry>, String> {
    if data.len() < 20 {
        return Err("IMG too short".into());
    }
    let mut header = data[..20].to_vec();
    let encrypted = le32(&header, 0) != MAGIC;
    if encrypted {
        key.ok_or("IMG is encrypted and no key was given")?.decrypt(&mut header);
    }
    if le32(&header, 0) != MAGIC || le32(&header, 4) != 3 {
        return Err(format!("Not an IMG v3 archive (magic {:08X})", le32(&header, 0)));
    }
    let count = le32(&header, 8) as usize;
    let table_size = le32(&header, 12) as usize;
    let item_size = u16::from_le_bytes([header[16], header[17]]) as usize;
    let mut table = data.get(20..20 + table_size).ok_or("Truncated IMG table")?.to_vec();
    if encrypted {
        key.unwrap().decrypt(&mut table);
    }
    let names_at = count * item_size;
    let mut names = table.get(names_at..).ok_or("IMG table smaller than entries")?.split(|&b| b == 0);
    (0..count)
        .map(|i| {
            let e = &table[i * item_size..];
            Ok(Entry {
                size_field: le32(e, 0),
                resource_type: le32(e, 4),
                offset: le32(e, 8) as u64 * BLOCK,
                used_blocks: u16::from_le_bytes([e[12], e[13]]),
                flags: u16::from_le_bytes([e[14], e[15]]),
                name: String::from_utf8_lossy(names.next().ok_or("Missing IMG name")?).into_owned(),
            })
        })
        .collect()
}
