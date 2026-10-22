//! A read-only view of Portal 2's `pak01_dir.vpk`: the directory is indexed once, files are read on demand.
use std::collections::HashMap;

use bevy::prelude::*;

/// Normalise a Source path: lowercase, forward slashes, no leading slash, no doubled slashes.
pub fn normalize_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut last_slash = true;
    for c in path.chars() {
        let c = if c == '\\' { '/' } else { c.to_ascii_lowercase() };
        if c == '/' {
            if last_slash {
                continue;
            }
            last_slash = true;
        } else {
            last_slash = false;
        }
        out.push(c);
    }
    out
}

struct Entry {
    preload: Vec<u8>,
    archive: u16,
    offset: u64,
    length: usize,
}

#[derive(Resource, Default)]
pub struct Fs {
    game: String,
    data_at: u64,
    index: HashMap<String, Entry>,
}

impl Fs {
    pub fn ready(&self) -> bool {
        !self.index.is_empty()
    }

    /// Index the game's directory file. Returns false if it can't be read.
    pub fn load(&mut self, game: &str) -> bool {
        let Ok(d) = std::fs::read(format!("{game}/portal2/pak01_dir.vpk")) else { return false };
        let u32_at = |o: usize| d.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let (Some(version), Some(tree)) = (u32_at(4), u32_at(8)) else { return false };
        let head = if version == 2 { 28 } else { 12 };
        self.data_at = head as u64 + tree as u64;
        self.game = game.to_string();
        let mut p = head;
        let cs = |p: &mut usize| -> Option<String> {
            let e = *p + d.get(*p..)?.iter().position(|&c| c == 0)?;
            let s = String::from_utf8_lossy(&d[*p..e]).into_owned();
            *p = e + 1;
            Some(s)
        };
        let mut index = HashMap::new();
        let _ = (|| -> Option<()> {
            loop {
                let ext = cs(&mut p)?;
                if ext.is_empty() {
                    return Some(());
                }
                loop {
                    let path = cs(&mut p)?;
                    if path.is_empty() {
                        break;
                    }
                    loop {
                        let name = cs(&mut p)?;
                        if name.is_empty() {
                            break;
                        }
                        let pre = u16::from_le_bytes([*d.get(p + 4)?, *d.get(p + 5)?]) as usize;
                        let archive = u16::from_le_bytes([*d.get(p + 6)?, *d.get(p + 7)?]);
                        let (offset, length) = (u32_at(p + 8)? as u64, u32_at(p + 12)? as usize);
                        let full = normalize_path(&format!("{path}/{name}.{ext}").replace(" /", "/"));
                        index.insert(full, Entry { preload: d.get(p + 18..p + 18 + pre)?.to_vec(), archive, offset, length });
                        p += 18 + pre;
                    }
                }
            }
        })();
        self.index = index;
        self.ready()
    }

    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        use std::io::{Read, Seek};
        let e = self.index.get(&normalize_path(path))?;
        let mut out = e.preload.clone();
        if e.length > 0 {
            let (file, at) = if e.archive == 0x7fff {
                (format!("{}/portal2/pak01_dir.vpk", self.game), e.offset + self.data_at)
            } else {
                (format!("{}/portal2/pak01_{:03}.vpk", self.game, e.archive), e.offset)
            };
            let mut f = std::fs::File::open(file).ok()?;
            f.seek(std::io::SeekFrom::Start(at)).ok()?;
            let mut b = vec![0; e.length];
            f.read_exact(&mut b).ok()?;
            out.extend(b);
        }
        Some(out)
    }
}
