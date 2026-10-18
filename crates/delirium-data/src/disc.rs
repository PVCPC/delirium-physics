use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

const SECTOR: u64 = 2048;
const MAGIC: &[u8] = b"MICROSOFT*XBOX*MEDIA";
const BASES: [u64; 4] = [0, 0x0FD9_0000, 0x0208_0000, 0x1830_0000];

enum Source {
    Folder { files: HashMap<String, PathBuf> },
    Image { file: File, files: HashMap<String, (u64, u64)>, ps3: Option<Ps3> },
}

struct Ps3 {
    serial: String,
    regions: Vec<(u64, u64)>,
    key: Option<aes::Aes128>,
}

pub struct Disc {
    source: Source,
    update: Option<Update>,
}

fn update_path(path: &str) -> Option<&str> {
    path.strip_prefix("update:").map(|p| p.trim_start_matches('/'))
}

fn read_exact_at(f: &File, buf: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    let mut done = 0;
    while done < buf.len() {
        #[cfg(unix)]
        let n = std::os::unix::fs::FileExt::read_at(f, &mut buf[done..], offset)?;
        #[cfg(windows)]
        let n = std::os::windows::fs::FileExt::seek_read(f, &mut buf[done..], offset)?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        done += n;
        offset += n as u64;
    }
    Ok(())
}

fn read_ps3(f: &File, key: &aes::Aes128, regions: &[(u64, u64)], buf: &mut [u8], offset: u64) -> std::io::Result<()> {
    use aes::cipher::{BlockDecrypt, generic_array::GenericArray};
    let (first, last) = (offset / SECTOR, (offset + buf.len() as u64).div_ceil(SECTOR));
    let mut d = vec![0u8; ((last - first) * SECTOR) as usize];
    read_exact_at(f, &mut d, first * SECTOR)?;
    for (i, s) in d.chunks_mut(SECTOR as usize).enumerate() {
        let n = first + i as u64;
        if !regions.iter().any(|&(a, b)| a <= n && n <= b) {
            continue;
        }
        let mut prev = [0u8; 16];
        prev[12..].copy_from_slice(&(n as u32).to_be_bytes());
        for b in s.chunks_mut(16) {
            let c: [u8; 16] = b.try_into().unwrap();
            let mut x = GenericArray::from(c);
            key.decrypt_block(&mut x);
            b.iter_mut().zip(x.iter().zip(prev)).for_each(|(o, (x, p))| *o = x ^ p);
            prev = c;
        }
    }
    let at = (offset - first * SECTOR) as usize;
    buf.copy_from_slice(&d[at..at + buf.len()]);
    Ok(())
}

impl Disc {
    pub fn open(path: &Path) -> Result<Disc, String> {
        if path.is_dir() {
            let mut files = HashMap::new();
            let mut dirs = vec![path.to_path_buf()];
            while let Some(dir) = dirs.pop() {
                for e in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        dirs.push(p);
                    } else if let Ok(rel) = p.strip_prefix(path) {
                        let key = rel.to_string_lossy().replace('\\', "/").to_lowercase();
                        files.insert(key, p);
                    }
                }
            }
            return Ok(Disc { source: Source::Folder { files }, update: None });
        }
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        for base in BASES {
            let mut vd = vec![0u8; SECTOR as usize];
            if base + 0x10000 + SECTOR > len || read_exact_at(&file, &mut vd, base + 0x10000).is_err() {
                continue;
            }
            if &vd[..20] != MAGIC || &vd[0x7EC..0x800] != MAGIC {
                continue;
            }
            let le32 = |d: &[u8], at: usize| u32::from_le_bytes(d[at..at + 4].try_into().unwrap());
            let mut files = HashMap::new();
            let mut dirs = vec![(le32(&vd, 20), le32(&vd, 24), String::new())];
            while let Some((sector, size, prefix)) = dirs.pop() {
                let mut d = vec![0u8; size as usize];
                read_exact_at(&file, &mut d, base + sector as u64 * SECTOR).map_err(|e| e.to_string())?;
                let (mut stack, mut seen) = (vec![0usize], std::collections::HashSet::new());
                while let Some(off) = stack.pop() {
                    let p = off * 4;
                    if !seen.insert(off) || p + 14 > d.len() {
                        continue;
                    }
                    let (left, right) = (u16::from_le_bytes([d[p], d[p + 1]]), u16::from_le_bytes([d[p + 2], d[p + 3]]));
                    let (at, bytes, attr, n) = (le32(&d, p + 4), le32(&d, p + 8), d[p + 12], d[p + 13] as usize);
                    if n == 0 || p + 14 + n > d.len() {
                        continue;
                    }
                    let name = String::from_utf8_lossy(&d[p + 14..p + 14 + n]).to_lowercase();
                    let path = format!("{prefix}{name}");
                    if attr & 0x10 != 0 {
                        if bytes > 0 {
                            dirs.push((at, bytes, format!("{path}/")));
                        }
                    } else {
                        files.insert(path, (base + at as u64 * SECTOR, bytes as u64));
                    }
                    stack.extend([left, right].into_iter().filter(|&c| c != 0 && c != 0xFFFF).map(usize::from));
                }
            }
            return Ok(Disc { source: Source::Image { file, files, ps3: None }, update: None });
        }
        Self::open_iso9660(file, len).ok_or_else(|| format!("{}: neither a folder nor a disc image of the game", path.display()))
    }

    fn open_iso9660(file: File, len: u64) -> Option<Disc> {
        let mut head = vec![0u8; 17 * SECTOR as usize];
        if len < head.len() as u64 || read_exact_at(&file, &mut head, 0).is_err() {
            return None;
        }
        let vd = &head[16 * SECTOR as usize..];
        if &vd[1..6] != b"CD001" || &head[0x800..0x80C] != b"PlayStation3" {
            return None;
        }
        let be32 = |d: &[u8], at: usize| u32::from_be_bytes(d[at..at + 4].try_into().unwrap()) as u64;
        let le32 = |d: &[u8], at: usize| u32::from_le_bytes(d[at..at + 4].try_into().unwrap()) as u64;
        let n = (be32(&head, 0) as usize).min(250);
        let starts: Vec<u64> = (0..n * 2).map(|i| be32(&head, 8 + i * 4)).collect();
        let regions = (1..(n * 2).saturating_sub(1)).step_by(2).map(|i| (starts[i], starts[i + 1])).collect();
        let serial = String::from_utf8_lossy(&head[0x810..0x820]).trim_matches(|c: char| c == '\0' || c == ' ').to_string();
        let mut files = HashMap::new();
        let mut dirs = vec![(le32(vd, 156 + 2), le32(vd, 156 + 10), String::new())];
        while let Some((sector, size, prefix)) = dirs.pop() {
            let mut d = vec![0u8; size as usize];
            read_exact_at(&file, &mut d, sector * SECTOR).ok()?;
            let mut p = 0;
            while p < d.len() {
                let n = d[p] as usize;
                if n == 0 {
                    p = (p / SECTOR as usize + 1) * SECTOR as usize;
                    continue;
                }
                let r = d.get(p..p + n)?;
                if !(r[32] == 1 && r[33] <= 1) {
                    let name = String::from_utf8_lossy(&r[33..33 + r[32] as usize]).to_lowercase();
                    let name = name.split(';').next().unwrap_or_default();
                    let (at, bytes) = (le32(r, 2), le32(r, 10));
                    if r[25] & 2 != 0 {
                        dirs.push((at, bytes, format!("{prefix}{name}/")));
                    } else {
                        files.insert(format!("{prefix}{name}"), (at * SECTOR, bytes));
                    }
                }
                p += n;
            }
        }
        Some(Disc { source: Source::Image { file, files, ps3: Some(Ps3 { serial, regions, key: None }) }, update: None })
    }

    pub fn ps3_serial(&self) -> Option<&str> {
        match &self.source {
            Source::Image { ps3: Some(p), .. } => Some(&p.serial),
            _ => None,
        }
    }

    pub fn needs_disc_key(&self) -> bool {
        self.ps3_serial().is_some() && self.read_at("ps3_game/usrdir/eboot.bin", 0, 4).ok().as_deref() != Some(b"SCE\0")
    }

    pub fn set_disc_key(&mut self, key: [u8; 16]) -> bool {
        use aes::cipher::KeyInit;
        if let Source::Image { ps3: Some(p), .. } = &mut self.source {
            p.key = Some(aes::Aes128::new(&key.into()));
        }
        if self.needs_disc_key() {
            if let Source::Image { ps3: Some(p), .. } = &mut self.source {
                p.key = None;
            }
            return false;
        }
        true
    }

    pub fn set_update(&mut self, update: Update) {
        self.update = Some(update);
    }

    pub fn update(&self) -> Option<&Update> {
        self.update.as_ref()
    }

    pub fn paths(&self) -> Vec<&str> {
        let mut v: Vec<&str> = match &self.source {
            Source::Folder { files } => files.keys().map(String::as_str).collect(),
            Source::Image { files, .. } => files.keys().map(String::as_str).collect(),
        };
        if let Some(u) = &self.update {
            v.extend(u.files.keys().map(|k| u.prefixed(k)));
        }
        v
    }

    pub fn len(&self, path: &str) -> Option<u64> {
        if let Some(p) = update_path(path) {
            return self.update.as_ref()?.read(p).map(|d| d.len() as u64);
        }
        let key = path.replace('\\', "/").to_lowercase();
        match &self.source {
            Source::Folder { files } => files.get(&key).and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len()),
            Source::Image { files, .. } => files.get(&key).map(|f| f.1),
        }
    }

    pub fn sector(&self, path: &str) -> Option<u32> {
        match &self.source {
            Source::Image { files, .. } if update_path(path).is_none() => {
                files.get(&path.replace('\\', "/").to_lowercase()).map(|f| (f.0 / SECTOR) as u32)
            }
            _ => None,
        }
    }

    pub fn read_at(&self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>, String> {
        if let Some(p) = update_path(path) {
            let d = self.update.as_ref().and_then(|u| u.read(p)).ok_or(format!("{path} not in the update"))?;
            let start = (offset as usize).min(d.len());
            return Ok(d[start..start + len.min(d.len() - start)].to_vec());
        }
        let key = path.replace('\\', "/").to_lowercase();
        let size = self.len(&key).ok_or(format!("{path} not on the disc"))?;
        let len = len.min(size.saturating_sub(offset) as usize);
        let mut buf = vec![0u8; len];
        match &self.source {
            Source::Folder { files } => {
                let f = File::open(&files[&key]).map_err(|e| format!("{path}: {e}"))?;
                read_exact_at(&f, &mut buf, offset)
            }
            Source::Image { file, files, ps3 } => {
                let at = files[&key].0 + offset;
                match ps3 {
                    Some(Ps3 { key: Some(k), regions, .. }) => read_ps3(file, k, regions, &mut buf, at),
                    _ => read_exact_at(file, &mut buf, at),
                }
            }
        }
        .map_err(|e| format!("{path}: {e}"))?;
        Ok(buf)
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        self.read_at(path, 0, usize::MAX)
    }
}

pub const TITLE_ID: u32 = 0x545407F2;

pub struct Update {
    pub source: PathBuf,
    files: HashMap<String, Vec<u8>>,
    named: HashMap<String, String>,
}

impl Update {
    pub fn from_package(path: &Path, data: &[u8]) -> Result<Update, String> {
        let p = crate::stfs::Package::parse(data)?;
        if p.title_id != TITLE_ID {
            return Err(format!("{}: title {:08X}, not the game", path.display(), p.title_id));
        }
        let files = p.files()?.into_iter().map(|(n, d)| (n.to_lowercase(), d)).collect();
        Ok(Update::new(path, files))
    }

    pub fn from_folder(path: &Path) -> Result<Update, String> {
        let Disc { source: Source::Folder { files }, .. } = Disc::open(path)? else {
            return Err(format!("{}: not a folder", path.display()));
        };
        let files = files.into_iter().map(|(k, p)| Ok((k, std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?)))
            .collect::<Result<_, String>>()?;
        Ok(Update::new(path, files))
    }

    fn new(path: &Path, files: HashMap<String, Vec<u8>>) -> Update {
        let named = files.keys().map(|k| (k.clone(), format!("update:/{k}"))).collect();
        Update { source: path.to_path_buf(), files, named }
    }

    fn prefixed(&self, key: &str) -> &str {
        &self.named[key]
    }

    pub fn find(places: &[PathBuf]) -> Option<Update> {
        let mut dirs: Vec<(PathBuf, u32)> = places.iter().map(|p| (p.clone(), 0)).collect();
        while let Some((dir, depth)) = dirs.pop() {
            if dir.join("default.xexp").is_file() && dir.join("update.img").is_file() {
                if let Ok(u) = Update::from_folder(&dir) {
                    return Some(u);
                }
            }
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            entries.sort();
            for p in entries {
                if p.is_dir() {
                    if depth < 5 {
                        dirs.push((p, depth + 1));
                    }
                    continue;
                }
                let Ok(m) = p.metadata() else { continue };
                if !(0xA000..64 << 20).contains(&m.len()) {
                    continue;
                }
                let mut head = [0u8; 0x364];
                let ok = File::open(&p).and_then(|f| read_exact_at(&f, &mut head, 0)).is_ok();
                if ok && matches!(&head[..4], b"LIVE" | b"PIRS" | b"CON ")
                    && u32::from_be_bytes(head[0x360..0x364].try_into().unwrap()) == TITLE_ID
                {
                    if let Ok(u) = std::fs::read(&p).map_err(|e| e.to_string()).and_then(|d| Update::from_package(&p, &d)) {
                        if u.read("default.xexp").is_some() {
                            return Some(u);
                        }
                    }
                }
            }
        }
        None
    }

    pub fn read(&self, path: &str) -> Option<&[u8]> {
        let key = path.trim_start_matches("update:").trim_start_matches('/').replace('\\', "/").to_lowercase();
        self.files.get(&key).map(Vec::as_slice)
    }

    pub fn paths(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.files.keys().map(String::as_str).collect();
        v.sort();
        v
    }
}
