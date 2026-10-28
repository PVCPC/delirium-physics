#[derive(Clone, Debug)]
pub struct Entry {
    pub path: String,
    pub dir: bool,
    pub size: u32,
    pub start: u32,
    pub blocks: u32,
    pub consecutive: bool,
}

pub struct Package<'a> {
    data: &'a [u8],
    pub title_id: u32,
    pub entries: Vec<Entry>,
    base: u64,
    shift: u32,
}

impl<'a> Package<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Package<'a>, String> {
        if data.len() < 0xA000 || !matches!(&data[..4], b"LIVE" | b"PIRS" | b"CON ") {
            return Err("not an STFS package".into());
        }
        let be32 = |o: usize| u32::from_be_bytes(data[o..o + 4].try_into().unwrap());
        let le24 = |o: usize| data[o] as u32 | (data[o + 1] as u32) << 8 | (data[o + 2] as u32) << 16;
        let header = be32(0x340) as u64;
        let base = (header + 0xFFF) & 0xF000;
        let shift = if base == 0xB000 { 0 } else { 1 };
        let mut p = Package { data, title_id: be32(0x360), entries: vec![], base, shift };
        let vd = 0x379;
        let table_blocks = u16::from_le_bytes([data[vd + 3], data[vd + 4]]) as u32;
        let table_start = le24(vd + 5);
        let mut raw = vec![];
        for i in 0..table_blocks * 0x40 {
            let e = p.offset(table_start + i / 0x40) as usize + (i % 0x40) as usize * 0x40;
            let rec = data.get(e..e + 0x40).ok_or("file table past the end")?;
            let len = (rec[0x28] & 0x3F) as usize;
            if len == 0 {
                continue;
            }
            let name: String = rec[..len.min(0x28)].iter().map(|&c| c as char).collect();
            let parent = i16::from_be_bytes([rec[0x32], rec[0x33]]);
            raw.push((name, rec[0x28], parent, le24(e + 0x29), le24(e + 0x2F), be32(e + 0x34)));
        }
        for i in 0..raw.len() {
            let mut path = raw[i].0.clone();
            let (mut parent, mut depth) = (raw[i].2, 0);
            while parent >= 0 && depth < 64 {
                let r = raw.get(parent as usize).ok_or("bad parent")?;
                path = format!("{}/{path}", r.0);
                parent = r.2;
                depth += 1;
            }
            let (_, flags, _, blocks, start, size) = raw[i];
            p.entries.push(Entry { path, dir: flags & 0x80 != 0, size, start, blocks, consecutive: flags & 0x40 != 0 });
        }
        Ok(p)
    }

    fn offset(&self, b: u32) -> u64 {
        let b = b as u64;
        let mut n = b + ((b / 0xAA + 1) << self.shift);
        if b >= 0xAA {
            n += (b / 0x70E4 + 1) << self.shift;
        }
        if b >= 0x70E4 {
            n += 1 << self.shift;
        }
        self.base + n * 0x1000
    }

    pub fn read(&self, e: &Entry) -> Result<Vec<u8>, String> {
        if e.dir {
            return Err(format!("{} is a directory", e.path));
        }
        if !e.consecutive {
            return Err(format!("{}: fragmented files are not read", e.path));
        }
        let mut out = Vec::with_capacity(e.size as usize);
        for k in 0..e.blocks {
            let o = self.offset(e.start + k) as usize;
            out.extend_from_slice(self.data.get(o..o + 0x1000).ok_or("block past the end")?);
        }
        out.truncate(e.size as usize);
        Ok(out)
    }

    pub fn files(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        self.entries.iter().filter(|e| !e.dir).map(|e| Ok((e.path.clone(), self.read(e)?))).collect()
    }
}
