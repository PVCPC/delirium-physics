use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod params;
pub mod pso;
mod steps;
#[cfg(windows)]
mod win;

type Res<T> = Result<T, String>;

const FIRST: u32 = 0x3758d50;
const LAST_BYTE: u32 = 0x3761150;
const LAST_MASK: u32 = 0x3760d50;
const LAST_OUT: u32 = 0x3761160;
const IV: u32 = 0x3c75f40;
pub const KEY_LEN: usize = 0x120;

pub fn joaat(s: &str) -> u32 {
    let mut h: u32 = 0;
    for b in s.bytes().map(|b| b.to_ascii_lowercase()) {
        h = h.wrapping_add(b as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

struct Cipher {
    rdata: Vec<u8>,
    va: u32,
    first: Vec<[u64; 256]>,
    masks: Vec<[u64; 16]>,
    starts: Vec<u64>,
    last: Vec<(u8, [u64; 8], [u8; 256])>,
    iv: [u8; 16],
}

fn q(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

fn spread(v: u64) -> [u64; 8] {
    std::array::from_fn(|i| ((v >> (8 * (7 - i))) & 0xff).wrapping_mul(0x0101_0101_0101_0101))
}

fn fold(v: u64, by: u32, n: u32) -> u64 {
    (0..n).fold(0, |a, i| a ^ (v >> (by * i)))
}

impl Cipher {
    fn from_exe(exe: &Path) -> Res<Self> {
        let d = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
        let at = |o: usize, n: usize| d.get(o..o + n).ok_or("RDR2.exe: too short");
        let pe = u32::from_le_bytes(at(0x3c, 4)?.try_into().unwrap()) as usize;
        let ns = u16::from_le_bytes(at(pe + 6, 2)?.try_into().unwrap()) as usize;
        let so = pe + 24 + u16::from_le_bytes(at(pe + 20, 2)?.try_into().unwrap()) as usize;
        let secs: Vec<(u32, u32, usize)> = (0..ns)
            .map(|i| {
                let s = so + 40 * i;
                let u = |o| u32::from_le_bytes(d[s + o..s + o + 4].try_into().unwrap());
                (u(12), u(16).max(u(8)), u(20) as usize)
            })
            .collect();
        let from = |rva: u32| -> Res<(u32, &[u8])> {
            let &(va, size, ro) = secs.iter().find(|s| s.0 <= rva && rva < s.0 + s.1).ok_or("RDR2.exe: not a version this reads")?;
            Ok((va, d.get(ro + (rva - va) as usize..(ro + size as usize).min(d.len())).ok_or("RDR2.exe: too short")?))
        };
        let va = from(FIRST)?.0;
        let rdata = from(va)?.1.to_vec();
        let iv: [u8; 16] = from(IV)?.1.get(..16).ok_or("RDR2.exe: too short")?.try_into().unwrap();
        let r = |rva: u32| (rva - va) as usize;
        let first = (0..16).map(|i| std::array::from_fn(|b| q(&rdata, r(FIRST + 2048 * i) + 8 * b))).collect();
        let masks = steps::STEPS.iter().map(|s| std::array::from_fn(|t| q(&rdata, r(s.1) + 8 * t))).collect();
        let starts = steps::STEPS.iter().map(|s| q(&rdata, r(s.2))).collect();
        let last = (0..16u32)
            .map(|i| {
                let o = r(LAST_OUT + 256 * i);
                (rdata[r(LAST_BYTE + i)], std::array::from_fn(|t| q(&rdata, r(LAST_MASK + 64 * i) + 8 * t)), rdata[o..o + 256].try_into().unwrap())
            })
            .collect();
        Ok(Cipher { rdata, va, first, masks, starts, last, iv })
    }

    fn block(&self, key: &[u8], b: &[u8]) -> [u8; 16] {
        let (mut lo, mut hi) = (0u64, 0u64);
        for i in 0..8 {
            lo ^= self.first[i][b[i] as usize];
            hi ^= self.first[8 + i][b[8 + i] as usize];
        }
        let mut k = 0;
        for r in 0..17 {
            let (a, bh) = (spread(lo), spread(hi));
            let mut out = [0u64; 2];
            for (half, acc) in out.iter_mut().enumerate() {
                *acc = q(key, 0x10 + r * 16 + half * 8);
                for _ in 0..8 {
                    let (use_hi, _, _, table) = steps::STEPS[k];
                    let (m, p) = (&self.masks[k], if use_hi { &bh } else { &a });
                    let (mut u1, mut u2) = (0u64, 0u64);
                    for t in 0..8 {
                        u1 ^= p[t] & m[t];
                        u2 ^= p[t] & m[8 + t];
                    }
                    let u1 = (fold(u1, 1, 7) ^ (u1 << 1)) & 0x0202_0202_0202_0202;
                    let u2 = fold(u2, 1, 8) & 0x0101_0101_0101_0101;
                    let idx = self.starts[k] ^ ((u1 >> 14 ^ u1 >> 7 ^ u1 ^ u1 << 7) & 0xf00) ^ (fold(u2, 7, 8) & 0xff);
                    *acc ^= q(&self.rdata, (table - self.va) as usize + 8 * idx as usize);
                    k += 1;
                }
            }
            [lo, hi] = out;
        }
        let (a, bh) = (spread(lo), spread(hi));
        std::array::from_fn(|i| {
            let (byte, m, o) = &self.last[i];
            let p = if i >= 8 { &bh } else { &a };
            let u = (0..8).fold(0, |u, t| u ^ (p[t] & m[t]));
            let u = fold(fold(u, 1, 8) & 0x0101_0101_0101_0101, 7, 8) & 0xff;
            o[(byte ^ u as u8) as usize]
        })
    }

    fn cbc(&self, key: &[u8], iv: &mut [u8; 16], d: &mut [u8]) {
        for c in d.chunks_exact_mut(16) {
            let next: [u8; 16] = c.try_into().unwrap();
            let p = self.block(key, c);
            for i in 0..16 {
                c[i] = p[i] ^ iv[i];
            }
            *iv = next;
        }
    }
}

pub struct Rdr2 {
    dir: PathBuf,
    cipher: Cipher,
    keys: HashMap<u8, Vec<u8>>,
    #[cfg(windows)]
    oodle: win::Oodle,
}

pub fn key_cache() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("HOME")).unwrap_or_default();
    PathBuf::from(base).join("Delirium").join("rdr2-keys.bin")
}

pub fn params_cache() -> PathBuf {
    key_cache().with_file_name("rdr2-params.txt")
}

pub fn params(dir: &Path) -> Option<HashMap<String, HashMap<u32, params::Param>>> {
    let t = std::fs::read_to_string(params_cache()).ok()?;
    let (size, rest) = t.split_once('\n')?;
    (size.trim().parse::<u64>().ok()? == exe_size(dir).ok()?).then(|| params::from_text(rest)).filter(|p| !p.is_empty())
}

fn exe_size(dir: &Path) -> Res<u64> {
    std::fs::metadata(dir.join("RDR2.exe")).map(|m| m.len()).map_err(|_| format!("{}: no RDR2.exe here", dir.display()))
}

fn load_keys(dir: &Path) -> Option<HashMap<u8, Vec<u8>>> {
    let d = std::fs::read(key_cache()).ok()?;
    if d.get(..4)? != b"DRK1" || u64::from_le_bytes(d.get(4..12)?.try_into().ok()?) != exe_size(dir).ok()? {
        return None;
    }
    Some(d[12..].chunks_exact(1 + KEY_LEN).map(|c| (c[0], c[1..].to_vec())).collect())
}

fn save_keys(dir: &Path, keys: &HashMap<u8, Vec<u8>>) -> Res<()> {
    let mut d = b"DRK1".to_vec();
    d.extend(exe_size(dir)?.to_le_bytes());
    let mut ids: Vec<_> = keys.keys().copied().collect();
    ids.sort();
    for id in ids {
        d.push(id);
        d.extend(&keys[&id]);
    }
    let p = key_cache();
    std::fs::create_dir_all(p.parent().unwrap()).and_then(|_| std::fs::write(&p, d)).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn has_keys(dir: &Path) -> bool {
    load_keys(dir).is_some_and(|k| !k.is_empty()) && params(dir).is_some()
}

#[cfg(windows)]
pub fn capture_keys(dir: &Path) -> Res<()> {
    let (keys, image) = win::capture()?;
    save_keys(dir, &keys)?;
    let p = params::scan(&image);
    if p.is_empty() {
        return Err("RDR2's behaviour parameters weren't where they should be (a game update?)".into());
    }
    let f = params_cache();
    std::fs::write(&f, format!("{}\n{}", exe_size(dir)?, params::to_text(&p))).map_err(|e| format!("{}: {e}", f.display()))
}

#[cfg(not(windows))]
pub fn capture_keys(_dir: &Path) -> Res<()> {
    Err("RDR2's keys can only be captured on Windows".into())
}

impl Rdr2 {
    pub fn open(dir: &Path) -> Res<Self> {
        let cipher = Cipher::from_exe(&dir.join("RDR2.exe"))?;
        let keys = load_keys(dir).ok_or("RDR2's keys aren't on this PC yet: run the one-time key step (capture_keys)")?;
        Ok(Rdr2 {
            dir: dir.into(),
            cipher,
            keys,
            #[cfg(windows)]
            oodle: win::Oodle::load(&dir.join("oo2core_5_win64.dll"))?,
        })
    }

    pub fn archive(&self, rel: &str) -> Res<Rpf<'_>> {
        let p = self.dir.join(rel);
        let f = File::open(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        Rpf::new(self, Arc::new(f), 0)
    }

    pub fn read(&self, spec: &str) -> Res<Vec<u8>> {
        let parts: Vec<&str> = spec.split('|').collect();
        let (last, mid) = parts[1..].split_last().ok_or("rdr2: want archive|path")?;
        let mut a = self.archive(parts[0])?;
        for p in mid {
            a = a.sub(p)?;
        }
        a.read(last)
    }

    fn decrypt(&self, key: u8, iv: &mut [u8; 16], d: &mut [u8]) -> Res<()> {
        let k = self.keys.get(&key).ok_or(format!("rdr2: key {key} missing from this PC's keys: run the key step again"))?;
        self.cipher.cbc(k, iv, d);
        Ok(())
    }
}

pub fn spec(name: &str) -> String {
    if name.ends_with(".xml") {
        format!("common_0.rpf|data/naturalmotion/{name}")
    } else if name.contains('|') {
        name.to_string()
    } else {
        format!("packs_0.rpf|packs/base/models/metaped_fragments|{name}")
    }
}

static OPEN: std::sync::Mutex<Option<(PathBuf, Rdr2)>> = std::sync::Mutex::new(None);

pub fn read_named(dir: &Path, name: &str) -> Res<Vec<u8>> {
    let mut open = OPEN.lock().unwrap_or_else(|e| e.into_inner());
    if open.as_ref().is_none_or(|(at, _)| at != dir) {
        *open = Some((dir.to_path_buf(), Rdr2::open(dir)?));
    }
    open.as_ref().unwrap().1.read(&spec(name))
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub meta: u32,
    pub key: u8,
    pub off: u64,
    pub stored: usize,
    pub compressed: bool,
    pub size: u64,
}

impl Entry {
    pub fn is_archive(&self) -> bool {
        self.meta & 0xffffff == 0xff00
    }
    fn is_resource(&self) -> bool {
        self.meta >> 24 & 1 != 0
    }
}

pub struct Rpf<'a> {
    game: &'a Rdr2,
    file: Arc<File>,
    base: u64,
    pub entries: HashMap<u32, Entry>,
}

fn read_at(f: &File, buf: &mut [u8], off: u64) -> Res<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut n = 0;
        while n < buf.len() {
            match f.seek_read(&mut buf[n..], off + n as u64) {
                Ok(0) => return Err("rdr2: archive ends early".into()),
                Ok(k) => n += k,
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        f.read_exact_at(buf, off).map_err(|e| e.to_string())
    }
}

impl<'a> Rpf<'a> {
    fn new(game: &'a Rdr2, file: Arc<File>, base: u64) -> Res<Self> {
        let mut h = [0u8; 16];
        read_at(&file, &mut h, base)?;
        if &h[..4] != b"8FPR" {
            return Err("rdr2: not an RPF8 archive".into());
        }
        let n = u32::from_le_bytes(h[4..8].try_into().unwrap()) as usize;
        let key = h[12];
        let mut toc = vec![0u8; n * 24];
        read_at(&file, &mut toc, base + 0x110)?;
        if key != 0xff {
            let mut iv = game.cipher.iv;
            game.decrypt(key, &mut iv, &mut toc[..n * 24 & !15])?;
        }
        let entries = toc
            .chunks_exact(24)
            .map(|c| {
                let (a, b, s) = (q(c, 0), q(c, 8), q(c, 16));
                let e = Entry {
                    meta: (a >> 32) as u32,
                    key: (a >> 40) as u8,
                    off: (b >> 28 & 0xffff_ffff) * 16,
                    stored: (b & 0xfff_ffff) as usize * 16,
                    compressed: b >> 60 != 0,
                    size: s,
                };
                (a as u32, e)
            })
            .collect();
        Ok(Rpf { game, file, base, entries })
    }

    fn entry(&self, path: &str) -> Res<Entry> {
        self.entries.get(&joaat(path)).copied().ok_or(format!("rdr2: no {path} in the archive"))
    }

    pub fn sub(&self, path: &str) -> Res<Rpf<'a>> {
        let e = self.entry(path)?;
        if !e.is_archive() {
            return Err(format!("rdr2: {path} isn't an archive"));
        }
        Rpf::new(self.game, self.file.clone(), self.base + e.off)
    }

    pub fn sub_hash(&self, hash: u32) -> Res<Rpf<'a>> {
        match self.entries.get(&hash) {
            Some(e) if e.is_archive() => Rpf::new(self.game, self.file.clone(), self.base + e.off),
            _ => Err(format!("rdr2: no archive {hash:08x}")),
        }
    }

    pub fn read(&self, path: &str) -> Res<Vec<u8>> {
        self.read_entry(&self.entry(path)?)
    }

    pub fn read_hash(&self, hash: u32) -> Res<Vec<u8>> {
        self.read_entry(self.entries.get(&hash).ok_or(format!("rdr2: no file {hash:08x}"))?)
    }

    fn read_entry(&self, e: &Entry) -> Res<Vec<u8>> {
        let mut d = vec![0u8; e.stored];
        read_at(&self.file, &mut d, self.base + e.off)?;
        if e.meta >> 25 & 1 != 0 {
            d.truncate(d.len().saturating_sub(0x100));
        }
        if e.is_resource() {
            d.drain(..16.min(d.len()));
        }
        if e.key != 0xff {
            self.decrypt_spans(e, &mut d)?;
        }
        let size = (if e.is_resource() { (e.size & 0xffff_fff0) + (e.size >> 32 & 0xffff_fff0) } else { e.size }) as usize;
        if e.compressed {
            #[cfg(windows)]
            return self.game.oodle.decompress(&d, size);
            #[cfg(not(windows))]
            return Err("rdr2: decompressing needs Windows (the game's Oodle DLL)".into());
        }
        d.truncate(size);
        Ok(d)
    }

    fn decrypt_spans(&self, e: &Entry, d: &mut [u8]) -> Res<()> {
        let c = e.meta & 0xff;
        let n = d.len();
        let head = if c & 3 != 0 { 1024usize << (2 * (c & 3)) } else { 0 };
        let (stripe, stride) = match c >> 2 & 7 {
            0 => (0, 0),
            s => (1024usize << s, ((c as usize >> 5 & 7) + 1) << 16),
        };
        let mut spans = vec![(0, head)];
        if stripe != 0 {
            spans.extend((stride..(n + 1).saturating_sub(0x400 + stripe)).step_by(stride).map(|o| (o, stripe)));
        }
        if n > head + 0x400 {
            spans.push(((n - 0x400) & !15, 0x400));
        }
        let mut iv = self.game.cipher.iv;
        for (o, l) in spans {
            let l = l.min(n - o) & !15;
            if l > 0 {
                self.game.decrypt(e.key, &mut iv, &mut d[o..o + l])?;
            }
        }
        Ok(())
    }
}

