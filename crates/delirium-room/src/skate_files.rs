use delirium_data::disc::Disc;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const NO_HOM: &str = "Skate 3: no Hall of Meat screen";

const PATH: &str = "skate3_path.txt";

pub struct Link {
    pub typed: String,
    pub note: String,
}

impl Link {
    pub fn new() -> Link {
        Link { typed: std::fs::read_to_string(PATH).unwrap_or_default().trim().to_string(), note: String::new() }
    }

    pub fn link(&mut self) {
        let t = std::time::Instant::now();
        let path = self.typed.trim().trim_matches('"').to_string();
        let hom = std::cell::RefCell::new(String::new());
        self.note = match convert(Path::new(&path), &|s| if s.starts_with(NO_HOM) { *hom.borrow_mut() = format!(" {s}.") }) {
            Ok(()) => {
                let _ = std::fs::write(PATH, &path);
                format!("Linked and converted in {:.1} s.{}", t.elapsed().as_secs_f32(), hom.borrow())
            }
            Err(e) => e,
        };
    }
}

pub fn settings(ui: &mut bevy_egui::egui::Ui, l: &mut Link, pressed: &mut Option<String>) {
    use bevy_egui::egui;
    crate::info(ui, "Optional bonus. Link your own Skate 3 (Xbox 360) here; the base room doesn't need it.");
    ui.label("Skate 3 disc image (.iso) or default.xex");
    let r = ui.add(egui::TextEdit::singleline(&mut l.typed).desired_width(f32::INFINITY).hint_text("e.g. F:/Games/Skate 3.iso"));
    if (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("Link").clicked() {
        *pressed = Some("#skate3".into());
    }
    if !l.note.is_empty() {
        ui.add(egui::Label::new(l.note.as_str()).wrap());
    }
    if ready() {
        crate::info(ui, "Skate 3 mode is ready: J puts Homer on a skateboard (the controls: the Hero tab's Skate 3 tip).");
    } else {
        crate::info(ui, "Skate 3 mode is off. Link your disc image once: only what the skate engine needs is converted, into the room's folder.");
    }
}

pub fn dir() -> PathBuf {
    crate::getup::config_dir().join("skate3")
}

pub fn ready() -> bool {
    dir().join("private/game.json").is_file()
}

const KEEP: [&str; 5] = ["data/anim/", "data/state/", "data/script/", "data/joystick/", "data/camera/"];

pub fn open(path: &Path) -> Result<Disc, String> {
    let is_xex = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xex"));
    let disc = Disc::open(if is_xex { path.parent().unwrap_or(path) } else { path })?;
    for f in ["data/big/miscload.big", "data/big/miscboot.big", "data/big/db.big"] {
        if disc.len(f).is_none() {
            return Err(format!("not Skate 3 for Xbox 360: no {f} there (pick the .iso, or default.xex with its data folder)"));
        }
    }
    Ok(disc)
}

pub fn convert(path: &Path, say: &dyn Fn(&str)) -> Result<(), String> {
    let disc = open(path)?;
    let out = dir();
    let stage = out.with_file_name("skate3.partial");
    let _ = std::fs::remove_dir_all(&stage);
    let stock = stage.join("private/stock");
    let put = |rel: &str, data: &[u8]| -> Result<(), String> {
        let p = stock.join(safe(rel)?);
        std::fs::create_dir_all(p.parent().unwrap()).and_then(|_| std::fs::write(&p, data)).map_err(|e| format!("{}: {e}", p.display()))
    };
    say("Skate 3: animation banks, graphs and gameplay inputs");
    let keep = |p: &str| KEEP.iter().any(|k| p.to_lowercase().starts_with(k));
    for e in big(&disc, "data/big/miscload.big")?.iter().filter(|e| keep(&e.path)) {
        put(&e.path, &read(&disc, "data/big/miscload.big", e)?)?;
    }
    for e in big(&disc, "data/big/miscboot.big")?.iter().filter(|e| e.path.eq_ignore_ascii_case("data/config/input.cfg")) {
        put(&e.path, &read(&disc, "data/big/miscboot.big", e)?)?;
    }
    for p in disc.paths().into_iter().filter(|p| p.starts_with("data/anim/")).map(str::to_string).collect::<Vec<_>>() {
        put(&p, &disc.read(&p)?)?;
    }
    say("Skate 3: physics and difficulty settings");
    let db: HashMap<String, Vec<u8>> = big(&disc, "data/big/db.big")?.iter()
        .filter_map(|e| {
            let name = e.path.rsplit('/').next()?.to_lowercase();
            ["skaterschema.bin", "skaterschema.vlt", "skatercollections.bin", "skatercollections.vlt"].contains(&name.as_str())
                .then(|| read(&disc, "data/big/db.big", e).map(|d| (name, d)))
        }).collect::<Result<_, _>>()?;
    let get = |n: &str| db.get(n).cloned().ok_or(format!("db.big has no {n}"));
    let collections = vlt((get("skaterschema.vlt")?, get("skaterschema.bin")?), (get("skatercollections.vlt")?, get("skatercollections.bin")?))?;
    let write = |rel: &str, v: &serde_json::Value| {
        let p = stage.join(rel);
        std::fs::write(&p, serde_json::to_vec(v).unwrap()).map_err(|e| format!("{}: {e}", p.display()))
    };
    write("private/stock/skater-collections.json", &collections)?;
    let bank = std::fs::read(stock.join("data/anim/OnBoard.abin")).map_err(|e| format!("OnBoard.abin: {e}"))?;
    write("private/stock/physics-skeletons.json", &physics_skeletons(&bank)?)?;
    say("Skate 3: the Hall of Meat screen");
    let hom = stage.join("private/hom");
    if let Err(e) = crate::skate_hud_files::convert(&disc, &serde_json::to_vec(&collections).unwrap(), &hom, &out.join("private").join("hom")) {
        let _ = std::fs::remove_dir_all(&hom);
        say(&format!("{NO_HOM} ({e})"));
    }
    write("private/game.json", &json!({"version": 1, "character_scene": "private/skater.glb", "initial_animation": "R_IDLE_HCOM_000",
        "action_graph": "private/stock/data/state/ActionGraph_OnBoard.stategraph", "motion_graph": "private/stock/data/state/MotionGraph_OnBoard.stategraph"}))?;
    let _ = std::fs::remove_dir_all(&out);
    std::fs::rename(&stage, &out).map_err(|e| format!("{}: {e}", out.display()))?;
    say("Skate 3: ready");
    Ok(())
}

pub(crate) fn safe(p: &str) -> Result<PathBuf, String> {
    let parts: Vec<&str> = p.split(['/', '\\']).collect();
    if parts.iter().any(|s| s.is_empty() || *s == "." || *s == ".." || s.contains(':')) {
        return Err(format!("unsafe archive path {p:?}"));
    }
    Ok(parts.iter().collect())
}

pub(crate) struct Entry {
    pub(crate) path: String,
    offset: u64,
    stored: u64,
    unpacked: u64,
    compression: u8,
}

fn be16(d: &[u8], at: usize) -> Result<u16, String> {
    d.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]])).ok_or("truncated record".into())
}

fn be32(d: &[u8], at: usize) -> Result<u32, String> {
    d.get(at..at + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap())).ok_or("truncated record".into())
}

fn be64(d: &[u8], at: usize) -> Result<u64, String> {
    d.get(at..at + 8).map(|b| u64::from_be_bytes(b.try_into().unwrap())).ok_or("truncated record".into())
}

fn align(v: usize, a: usize) -> usize {
    v.div_ceil(a) * a
}

pub(crate) fn big(disc: &Disc, file: &str) -> Result<Vec<Entry>, String> {
    let h = disc.read_at(file, 0, 48)?;
    let (index, names) = (be32(&h, 12)? as usize, be32(&h, 16)? as usize);
    let m = disc.read_at(file, 0, index + names)?;
    if be16(&m, 0)? != 0x4542 || be16(&m, 2)? != 3 {
        return Err(format!("{file}: not an EB BIG v3 archive"));
    }
    let (count, flags, shift) = (be32(&m, 4)? as usize, be16(&m, 8)?, *m.get(10).ok_or("truncated BIG")?);
    let (name_rec, dir_rec) = (*m.get(20).ok_or("truncated BIG")? as usize, *m.get(21).ok_or("truncated BIG")? as usize);
    if count > 1_000_000 || shift > 31 || name_rec < 3 || dir_rec < 2 {
        return Err(format!("{file}: bad BIG header"));
    }
    let entry = if flags & 1 != 0 { 20 } else { 16 };
    let compression = 48 + align(entry * count, 16);
    let mut dirs = vec![];
    let mut at = index + align(name_rec * count, 16);
    while at + dir_rec <= index + names {
        let r = &m[at..at + dir_rec];
        dirs.push(String::from_utf8_lossy(&r[..r.iter().position(|&b| b == 0).unwrap_or(r.len())]).into_owned());
        at += dir_rec;
    }
    let size = disc.len(file).unwrap_or(0);
    (0..count).map(|i| {
        let e = 48 + i * entry;
        let offset = (be32(&m, e)? as u64) << shift;
        let declared = be32(&m, e + 4)? as u64;
        let unpacked = match be32(&m, e + 8)? as u64 { 0 => declared, u => u };
        let stored = if declared == 0 { unpacked } else { declared };
        let n = index + i * name_rec;
        let r = m.get(n..n + name_rec).ok_or("truncated BIG names")?;
        let name = String::from_utf8_lossy(&r[2..2 + r[2..].iter().position(|&b| b == 0).unwrap_or(r.len() - 2)]).into_owned();
        let d = dirs.get(be16(r, 0)? as usize).map_or(".", String::as_str);
        let path = if d.is_empty() || d == "." { name } else { format!("{d}/{name}") }.replace('\\', "/");
        if offset + stored > size {
            return Err(format!("{file}: {path} runs past the archive"));
        }
        Ok(Entry { path, offset, stored, unpacked, compression: *m.get(compression + i).ok_or("truncated BIG")? })
    }).collect()
}

pub(crate) fn read(disc: &Disc, file: &str, e: &Entry) -> Result<Vec<u8>, String> {
    let packed = disc.read_at(file, e.offset, e.stored as usize)?;
    let out = match e.compression {
        0 => packed,
        1 => refpack(&packed, e.unpacked as usize)?,
        2..=4 => chunkref(&packed, e.unpacked as usize)?,
        c => return Err(format!("{}: unknown compression {c}", e.path)),
    };
    if out.len() as u64 != e.unpacked {
        return Err(format!("{}: unpacked to {} bytes, not {}", e.path, out.len(), e.unpacked));
    }
    Ok(out)
}

fn refpack(src: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let bad = || "bad RefPack stream".to_string();
    let mut pos = match (src.first(), src.get(1)) {
        (Some(0x90), Some(0xFB)) => 6,
        (Some(0x10), Some(0xFB)) => 5,
        _ => 0,
    };
    let mut out = Vec::with_capacity(size);
    while pos < src.len() {
        let c = src[pos] as usize;
        pos += 1;
        let b = |k: usize| src.get(pos + k).map(|&x| x as usize).ok_or_else(bad);
        let (lit, dist, count, skip) = if c < 0x80 {
            (c & 3, ((c & 0x60) << 3) + b(0)? + 1, ((c >> 2) & 7) + 3, 1)
        } else if c < 0xC0 {
            (b(0)? >> 6, ((b(0)? & 0x3F) << 8) + b(1)? + 1, (c & 0x3F) + 4, 2)
        } else if c < 0xE0 {
            (c & 3, ((c & 0x10) << 12) + (b(0)? << 8) + b(1)? + 1, ((c & 0x0C) << 6) + b(2)? + 5, 3)
        } else if c < 0xFC {
            (((c & 0x1F) << 2) + 4, 0, 0, 0)
        } else {
            (c & 3, 0, 0, 0)
        };
        pos += skip;
        out.extend_from_slice(src.get(pos..pos + lit).ok_or_else(bad)?);
        pos += lit;
        if count > 0 {
            if dist > out.len() {
                return Err(bad());
            }
            for _ in 0..count {
                out.push(out[out.len() - dist]);
            }
        }
        if out.len() > size {
            return Err(bad());
        }
        if c >= 0xFC {
            break;
        }
    }
    if out.len() != size {
        return Err(bad());
    }
    Ok(out)
}

fn chunkref(d: &[u8], size: usize) -> Result<Vec<u8>, String> {
    if d.get(..8) != Some(b"chunkref") || be32(d, 8)? != 2 {
        return Err("bad chunkref header".into());
    }
    let (total, chunk, n, a) = (be32(d, 12)? as usize, be32(d, 16)? as usize, be32(d, 20)?, be32(d, 24)? as usize);
    if total != size || chunk == 0 || n == 0 || a == 0 || a > 4096 || !a.is_power_of_two() {
        return Err("bad chunkref sizes".into());
    }
    let mut out = Vec::with_capacity(total);
    let mut at = 28;
    for _ in 0..n {
        let payload = align(at + 8, a);
        let (len, method) = (be32(d, payload - 8)? as usize, be32(d, payload - 4)?);
        let packed = d.get(payload..payload + len).ok_or("truncated chunkref")?;
        let want = chunk.min(total - out.len());
        let got = match method {
            0 | 4 => packed.to_vec(),
            2 => refpack(packed, want)?,
            3 => miniz_oxide::inflate::decompress_to_vec_zlib(packed).map_err(|_| "bad chunkref zlib chunk")?,
            m => return Err(format!("chunkref method {m}")),
        };
        if got.len() != want {
            return Err("chunkref chunk size".into());
        }
        out.extend(got);
        at = payload + len;
    }
    Ok(out)
}

fn chunks(v: &[u8], at: usize) -> Result<Option<(u32, usize, usize)>, String> {
    if at >= v.len() {
        return Ok(None);
    }
    let (tag, size) = (be32(v, at)?, be32(v, at + 4)? as usize);
    if size < 8 || at + size > v.len() {
        return Err("bad AttribSys chunk".into());
    }
    Ok(Some((tag, at + 8, size - 8)))
}

fn vault(mut v: Vec<u8>, mut b: Vec<u8>) -> Result<(Vec<u8>, Vec<u8>, Vec<(u64, u64, u32, u32)>), String> {
    let mut exports = vec![];
    let mut at = 0;
    while let Some((tag, start, size)) = chunks(&v, at)? {
        if tag == 0x5074_724e {
            let mut to_vlt = false;
            for p in (start..start + size).step_by(16) {
                let (offset, kind, index, dest) = (be32(&v, p)? as usize, be16(&v, p + 4)?, be16(&v, p + 6)?, be64(&v, p + 8)?);
                match kind {
                    0 => break,
                    2 => to_vlt = index == 0,
                    1 | 3 => {
                        let t = if to_vlt { &mut v } else { &mut b };
                        let word = if kind == 1 { 0 } else { u32::try_from(dest).map_err(|_| "VLT pointer out of range")? };
                        t.get_mut(offset..offset + 4).ok_or("truncated VLT pointer")?.copy_from_slice(&word.to_be_bytes());
                    }
                    4 => {}
                    k => return Err(format!("unknown VLT pointer kind {k}")),
                }
            }
        } else if tag == 0x4578_704e {
            let n = be64(&v, start)? as usize;
            if n > (size - 8) / 24 {
                return Err("bad VLT export count".into());
            }
            exports = (0..n).map(|i| {
                let e = start + 8 + i * 24;
                Ok((be64(&v, e)?, be64(&v, e + 8)?, be32(&v, e + 16)?, be32(&v, e + 20)?))
            }).collect::<Result<_, String>>()?;
        }
        at = start + size;
        if tag == 0x456e_6443 {
            break;
        }
    }
    Ok((v, b, exports))
}

fn strings(d: &[u8], out: &mut Vec<String>) {
    let head = |c: u8| c.is_ascii_alphabetic() || c == b'_';
    let body = |c: u8| c.is_ascii_alphanumeric() || b"_:./ -".contains(&c);
    let mut run = vec![0usize; d.len() + 1];
    for i in (0..d.len()).rev() {
        run[i] = if body(d[i]) { run[i + 1] + 1 } else { 0 };
    }
    let mut i = 0;
    while i + 1 < d.len() {
        let r = run[i + 1];
        if head(d[i]) && (1..=180).contains(&r) && d.get(i + 1 + r) == Some(&0) {
            out.push(String::from_utf8_lossy(&d[i..i + 1 + r]).into_owned());
            i += r + 2;
        } else {
            i += 1;
        }
    }
}

fn vlt(schema: (Vec<u8>, Vec<u8>), collections: (Vec<u8>, Vec<u8>)) -> Result<serde_json::Value, String> {
    use skate_data::attrib_hash::hash;
    let source = skate_data::sha256::digest(&collections.0);
    let (sv, sb, se) = vault(schema.0, schema.1)?;
    let (cv, cb, ce) = vault(collections.0, collections.1)?;
    let mut found: Vec<String> = include_str!("skate_names.txt").lines().map(str::to_string).collect();
    strings(&sb, &mut found);
    strings(&cb, &mut found);
    let mut lookup: HashMap<u64, String> = HashMap::new();
    for s in found {
        lookup.insert(hash(&s), s);
    }
    let name = |lookup: &HashMap<u64, String>, k: u64| if k == 0 { String::new() } else { lookup.get(&k).cloned().unwrap_or_else(|| format!("Hash_{k:016X}")) };
    let hex = |d: &[u8]| d.iter().map(|b| format!("{b:02X}")).collect::<String>();
    let take = |d: &[u8], at: usize, n: usize| d.get(at..at + n).map(<[u8]>::to_vec).ok_or(format!("truncated AttribSys record at {at:#x}"));
    let mut classes: HashMap<u64, Vec<(u64, (u64, usize, usize, u8))>> = HashMap::new();
    for &(_, kind, _, at) in &se {
        if kind != 0x2A78_95AC_4A87_6152 {
            continue;
        }
        let at = at as usize;
        let (key, count, defs) = (be64(&sv, at)?, be32(&sv, at + 12)? as usize, be32(&sv, at + 16)? as usize);
        let mut fields: Vec<(u64, (u64, usize, usize, u8))> = vec![];
        for i in 0..count {
            let d = defs + i * 24;
            let f = (be64(&sb, d + 8)?, be16(&sb, d + 16)? as usize, be16(&sb, d + 18)? as usize, *sb.get(d + 22).ok_or("truncated schema")?);
            let k = be64(&sb, d)?;
            match fields.iter_mut().find(|x| x.0 == k) {
                Some(x) => x.1 = f,
                None => fields.push((k, f)),
            }
        }
        classes.insert(key, fields);
    }
    let mut rows = vec![];
    for &(_, kind, _, at) in &ce {
        if kind != 0xAD30_3B8F_42B3_307E {
            continue;
        }
        let at = at as usize;
        let (key, cls, parent) = (be64(&cv, at)?, be64(&cv, at + 8)?, be64(&cv, at + 16)?);
        let (count, typeslen, layout) = (be32(&cv, at + 32)? as usize, be16(&cv, at + 38)? as usize, be32(&cv, at + 40)? as usize);
        let fields = classes.get(&cls).ok_or(format!("unknown class {cls:x}"))?;
        let mut out = serde_json::Map::new();
        let mut value = |lookup: &mut HashMap<u64, String>, fk: u64, d: &[u8], pos: usize, inline: bool| -> Result<(), String> {
            let (typ, _, n, flags) = fields.iter().find(|f| f.0 == fk).ok_or(format!("unknown schema field {fk:x}"))?.1;
            let t = name(lookup, typ);
            let v = if t == "EA::Reflection::Text" && flags & 1 == 0 {
                let ptr = be32(d, pos)? as usize;
                let raw = if ptr == 0 { String::new() } else {
                    let end = cb.get(ptr..).and_then(|s| s.iter().position(|&c| c == 0)).ok_or("unterminated VLT text")?;
                    String::from_utf8_lossy(&cb[ptr..ptr + end]).into_owned()
                };
                if raw.is_ascii() {
                    lookup.insert(hash(&raw), raw.clone());
                }
                json!({"type": t, "data": raw})
            } else {
                json!({"type": t, "data": hex(&take(d, pos, if inline { 4 } else { n })?)})
            };
            out.insert(name(lookup, fk), v);
            Ok(())
        };
        if layout != 0 {
            for &(fk, (_, offset, _, flags)) in fields {
                if flags & 2 != 0 {
                    value(&mut lookup, fk, &cb, layout + offset, false)?;
                }
            }
        }
        let entries = at + 48 + typeslen * 8;
        for i in 0..count {
            let pos = entries + i * 16;
            let (fk, ptr) = (be64(&cv, pos)?, be32(&cv, pos + 8)? as usize);
            let (_, _, size, flags) = fields.iter().find(|f| f.0 == fk).ok_or(format!("unknown schema field {fk:x}"))?.1;
            if size <= 4 && flags & 1 == 0 {
                value(&mut lookup, fk, &cv, pos + 8, true)?;
            } else {
                value(&mut lookup, fk, &cb, ptr, false)?;
            }
        }
        rows.push((cls, key, parent, out));
    }
    let rows: Vec<serde_json::Value> = rows.into_iter().map(|(cls, key, parent, fields)| json!({
        "class": name(&lookup, cls), "key": name(&lookup, key), "parent": name(&lookup, parent), "fields": fields,
        "source": "skatercollections.vlt", "sha256": source,
    })).collect();
    Ok(json!({"version": 1, "collections": rows}))
}

fn physics_skeletons(d: &[u8]) -> Result<serde_json::Value, String> {
    let fast = |at: usize, words: usize| -> Result<String, String> {
        const ABC: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_";
        let mut s = String::new();
        for w in 0..words {
            let mut word = be32(d, at + w * 4)? as u64;
            let mut div = 38u64.pow(5);
            for _ in 0..6 {
                let digit = word / div;
                word %= div;
                if digit == 0 {
                    if word != 0 {
                        return Err("bad FastString padding".into());
                    }
                    break;
                }
                s.push(*ABC.get(digit as usize - 1).ok_or("bad FastString digit")? as char);
                div /= 38;
            }
        }
        Ok(s)
    };
    let (end, version) = (be32(d, 0)? as usize, be32(d, 4)?);
    if version != 1 || end > d.len() {
        return Err("bad animation bank".into());
    }
    let mut skeletons = vec![];
    let mut at = 48;
    while at < end {
        let (size, kind) = (be32(d, at)? as usize, be32(d, at + 4)?);
        if size < 48 || size % 16 != 0 || at + size > end {
            return Err("bad ABIN record".into());
        }
        if kind == 5 {
            let payload = (at + 55) & !15;
            let count = be32(d, payload)? as usize;
            let start = payload + 16;
            if start + count * 112 > d.len() {
                return Err("truncated ABIN skeleton".into());
            }
            let bones = (0..count).map(|i| {
                let pos = start + i * 112;
                let words = (0..28).map(|k| be32(d, pos + k * 4)).collect::<Result<Vec<_>, _>>()?;
                Ok(json!({"name": fast(pos + 92, 5)?, "source_offset": pos, "words": words}))
            }).collect::<Result<Vec<_>, String>>()?;
            skeletons.push(json!({"name": fast(at + 16, 6)?, "source_offset": at, "bones": bones}));
        }
        at += size;
    }
    if skeletons.is_empty() {
        return Err("no physics skeletons in the animation bank".into());
    }
    Ok(json!({"version": 1, "source_sha256": skate_data::sha256::digest(d), "skeletons": skeletons}))
}
