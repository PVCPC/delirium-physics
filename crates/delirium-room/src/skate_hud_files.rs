use crate::skate_files::{big, read, safe, Entry};
use delirium_data::disc::Disc;
use skate_data::sha256::digest;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

const MOVIE: &str = "data/fe/source/screens/hud2/homscoring";
const LABELS: &str = "data/fe/languages/labels/language_labels_global_skate3ng.bin";
const ENGLISH: &str = "data/fe/languages/english/language_english_global_skate3ng.bin";
const GLOBAL: &str = "data/content/textures/globaltextures.rx2";
const EXTS: [&str; 8] = [".apt", ".bmpfont", ".const", ".geo", ".rx2", ".rps3", ".jpg", ".png"];
const FONT_CLASS: u64 = 0xFECF_BCAF_3565_18C4;
const CHARACTERS: [&str; 21] = ["", "shape", "text", "font", "button", "sprite", "sound", "bitmap", "morph", "animation",
    "static_text", "none", "video", "command", "command_list", "level", "custom_control", "image", "plugin", "bitmap_slice", "avatar"];
const CONTROLS: [&str; 12] = ["", "do_action", "frame_label", "place_object2", "remove_object2", "background_color", "start_sound",
    "start_sound_stream", "do_init_action", "place_object3", "place_object4", "remove_object3"];
const RENDER: [&str; 4] = ["line", "solid", "texture_clamped", "texture_wrapped"];
const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub fn convert(disc: &Disc, collections: &[u8], out: &Path, named: &Path) -> Result<(), String> {
    let mut sel: BTreeMap<String, (String, Entry)> = BTreeMap::new();
    for a in ["fedata.big", "fetexture.big", "miscboot.big"] {
        let file = format!("data/big/{a}");
        for e in big(disc, &file)? {
            let key = e.path.to_lowercase();
            if key == LABELS || key == ENGLISH || key == GLOBAL || (key.starts_with("data/fe/fonts/") || key.starts_with(MOVIE)) && EXTS.contains(&suffix(&key)) {
                sel.insert(key, (file.clone(), e));
            }
        }
    }
    let get = |key: &str| sel.get(key).ok_or(format!("no {key} in the archives")).and_then(|(f, e)| read(disc, f, e));
    let mut bundles: BTreeMap<&str, (&str, BTreeMap<&str, &str>)> = BTreeMap::new();
    for (key, (_, e)) in &sel {
        let ext = suffix(key);
        let b = bundles.entry(&key[..key.len() - ext.len()]).or_insert((&e.path[..e.path.len() - ext.len()], BTreeMap::new()));
        b.1.insert(ext, key);
    }
    let mut fonts = vec![];
    for (stem, (display, parts)) in &bundles {
        if let (true, Some(k)) = (stem.starts_with("data/fe/fonts/"), parts.get(".bmpfont")) {
            let source = named.join("raw").join(safe(&sel[*k].1.path)?).display().to_string();
            if let Ok(def) = bitmap_font(&get(k)?, &source) {
                let arena = parts.get(".rx2").or(parts.get(".rps3")).copied();
                fonts.push(Font { display, family: def.get("family").and_then(J::str).unwrap_or("").to_string(), def, arena });
            }
        }
    }
    let mut c = Convert { files: BTreeMap::new() };

    let table: serde_json::Value = serde_json::from_slice(collections).map_err(|e| format!("skater collections: {e}"))?;
    let mut mappings: Vec<(String, J)> = vec![];
    let mut aliases: HashMap<String, (String, J)> = HashMap::new();
    for row in table["collections"].as_array().ok_or("skater collections: no rows")? {
        let class = row["class"].as_str().unwrap_or("");
        let id = match class.strip_prefix("Hash_") {
            Some(h) => u64::from_str_radix(h, 16).map_err(|_| format!("bad class {class}"))?,
            None => skate_data::attrib_hash::hash(class),
        };
        if id != FONT_CLASS {
            continue;
        }
        let field = |n: &str| {
            let f = &row["fields"];
            f.get(n).or_else(|| f.get(format!("Hash_{:016X}", skate_data::attrib_hash::hash(n)))).and_then(|v| v["data"].as_str())
                .ok_or(format!("font record without {n}"))
        };
        let (apt_name, file) = (field("AptName")?, field("FileName")?);
        let found: Vec<&Font> = fonts.iter().filter(|f| f.display.rsplit('/').next() == Some(file)).collect();
        if found.is_empty() {
            continue;
        }
        if found.len() != 1 || mappings.iter().any(|m| m.0 == apt_name) {
            return Err(format!("ambiguous native font record: {apt_name}"));
        }
        let mut layout = vec![];
        for n in ["ScaleX", "ScaleY", "OffsetX", "OffsetY"] {
            let h = field(n)?;
            let b: Vec<u8> = (0..h.len()).step_by(2).map(|i| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok()))
                .collect::<Option<_>>().ok_or(format!("bad {n}"))?;
            let b: [u8; 4] = b.try_into().map_err(|_| format!("bad {n}"))?;
            layout.push((n, J::Float(f32::from_be_bytes(b) as f64)));
        }
        let prov = o(vec![("class", class.into()), ("key", row["key"].as_str().unwrap_or("").into()),
            ("source_sha256", row["sha256"].as_str().unwrap_or("").into()), ("file_name", file.into()), ("native_layout", o(layout))]);
        aliases.insert(apt_name.to_string(), (found[0].family.clone(), prov.clone()));
        mappings.push((apt_name.to_string(), prov));
    }

    let parts = &bundles.get(MOVIE).ok_or("no homscoring movie in the archives")?.1;
    let part = |ext: &str| parts.get(ext).ok_or(format!("homscoring has no {ext}")).and_then(|k| get(k));
    let (apt, cst) = (part(".apt")?, part(".const")?);
    let (a, k) = (R(&apt, "homscoring.apt"), R(&cst, "homscoring.const"));
    k.need(0, 32)?;
    a.need(0, 16)?;
    let root = character(&a, k.u32(20)? as usize, 0)?.ok_or("homscoring has no movie")?;
    let mut characters = vec![];
    if root.get("type").and_then(J::int) == Some(9) {
        let movie = root.get("movie").unwrap();
        let (count, table) = (movie.get("character_count").and_then(J::int).unwrap_or(0) as usize,
            movie.get("characters_offset").and_then(J::int).unwrap_or(0) as usize);
        if count > 100_000 {
            return Err("unreasonable APT character count".into());
        }
        a.need(table, count * 4)?;
        for id in 0..count {
            if let Some(mut ch) = character(&a, a.u32(table + id * 4)? as usize, id)? {
                if let Some(m) = ch.get("movie") {
                    let f = frames(&a, m)?;
                    ch.push("frames", f);
                }
                characters.push(ch);
            }
        }
    }
    let by_id = |id: i64| characters.iter().rev().find(|c| c.get("id").and_then(J::int) == Some(id));

    let actions = Actions::new(&a, &k)?;
    let mut blocks: Vec<(String, J)> = vec![];
    for ch in &characters {
        for f in ch.get("frames").map_or(&[][..], J::arr) {
            for control in f.get("controls").map_or(&[][..], J::arr) {
                let t = control.get("type_name").and_then(J::str);
                let at = control.get("actions_offset").and_then(J::int).unwrap_or(0);
                if matches!(t, Some("do_action" | "do_init_action")) && at != 0 && !blocks.iter().any(|b| b.0 == at.to_string()) {
                    blocks.push((at.to_string(), actions.stream(at as usize, None, 0)?));
                }
            }
        }
    }

    let geo = geo(&part(".geo")?)?;
    let arena = parts.get(".rx2").or(parts.get(".rps3")).map(|k| get(k)).transpose()?;
    let textures = arena.as_deref().map(rw4).transpose()?.unwrap_or_default();
    let mut shapes: Vec<(String, J)> = vec![];
    let mut fonts_out: Vec<(String, J)> = vec![];
    for ch in &characters {
        let id = ch.get("id").and_then(J::int).unwrap_or(0);
        match ch.get("type_name").and_then(J::str) {
            Some("shape") => {
                let units = geo.get(&(id as u32)).ok_or(format!("shape {id} has no GEO record"))?;
                let mut prims = vec![];
                for (n, u) in units.iter().enumerate() {
                    let render = RENDER.get(u.render as usize).map_or(format!("unknown_{}", u.render), |s| s.to_string());
                    let texture = if render.starts_with("texture") {
                        let bitmap = by_id(u.texture as i64).and_then(|b| b.get("bitmap")).and_then(|b| b.get("texture_id")).and_then(J::int)
                            .ok_or(format!("shape {id}: no bitmap {}", u.texture))?;
                        Some(c.texture(&textures, arena.as_deref().unwrap_or(&[]), bundles[MOVIE].0, bitmap)?)
                    } else {
                        None
                    };
                    let tris = u.triangles.iter().map(|t| J::Arr(t.iter().map(|p| {
                        let mut v = vec![("position", floats(&point(&IDENTITY, p)))];
                        if let Some((_, w, h)) = &texture {
                            let uv = point(&u.uv, p);
                            v.push(("uv", floats(&[uv[0] / *w as f64, uv[1] / *h as f64])));
                        }
                        o(v)
                    }).collect())).collect();
                    let color = [u.color[0], u.color[1], u.color[2], u.color[3] * 1.0];
                    prims.push(o(vec![("draw_order", prims.len().into()), ("path", "/".into()), ("bundle", MOVIE.into()),
                        ("character_id", id.into()), ("unit", n.into()), ("render_type", render.into()), ("matrix", floats(&IDENTITY)),
                        ("color", floats(&color)), ("texture", texture.map_or(J::Null, |t| t.0)), ("triangles", J::Arr(tris))]));
                }
                shapes.push((id.to_string(), J::Arr(prims)));
            }
            Some("font") => {
                let family = ch.get("font").and_then(|f| f.get("name")).and_then(J::str).unwrap_or("").to_string();
                let asset = c.font(&fonts, &aliases, &family, &get)?;
                match fonts_out.iter_mut().find(|f| f.0 == family) {
                    Some(f) => f.1 = asset,
                    None => fonts_out.push((family, asset)),
                }
            }
            _ => {}
        }
    }

    let (labels, values) = (language(&get(LABELS)?)?, language(&get(ENGLISH)?)?);
    if labels.len() != values.len() {
        return Err("language label/value pools have different numbers of strings".into());
    }
    let mut lang: Vec<(String, J)> = vec![];
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (l, v) in labels.iter().zip(values) {
        let l = l.trim_matches(py_space).to_string();
        match seen.get(&l) {
            Some(&i) => lang[i].1 = v.into(),
            None => {
                seen.insert(l.clone(), lang.len());
                lang.push((l, v.into()));
            }
        }
    }

    let unresolved = fonts_out.iter().filter(|f| matches!(f.1, J::Null)).map(|f| J::from(f.0.as_str())).collect();
    let result = o(vec![
        ("format", "skate3-scoring-hud".into()),
        ("version", 1usize.into()),
        ("source", o(vec![("bundle", MOVIE.into()), ("apt_sha256", digest(&apt).into()), ("const_sha256", digest(&cst).into()),
            ("collections_sha256", digest(collections).into())])),
        ("characters", J::Arr(characters.clone())),
        ("shapes", J::Obj(shapes)),
        ("fonts", J::Obj(fonts_out)),
        ("actions", J::Obj(blocks)),
        ("font_mappings", J::Obj(mappings)),
        ("language", J::Obj(lang)),
        ("unresolved_fonts", J::Arr(unresolved)),
    ]);
    let mut s = String::new();
    dump(&result, &mut s);
    s.push_str(if cfg!(windows) { "\r\n" } else { "\n" });
    c.files.insert("runtime/homscoring.json".into(), s.into_bytes());
    let global = get(GLOBAL)?;
    let t = rw4(&global)?.into_iter().find(|t| t.name.to_lowercase().contains("xray")).ok_or("no X-ray film in the global textures")?;
    let f = fetch(t.fetch.as_ref().ok_or("X-ray film: not an texture")?);
    if (f.width, f.height) != (512, 512) {
        return Err(format!("X-ray film: {} x {}", f.width, f.height));
    }
    c.files.insert("xray.rgba".into(), decode_rgba(&global[t.data.0..t.data.1], &f)?);
    for (rel, data) in &c.files {
        let p = out.join(safe(rel)?);
        std::fs::create_dir_all(p.parent().unwrap()).and_then(|_| std::fs::write(&p, data)).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(())
}

fn suffix(p: &str) -> &str {
    let name = &p[p.rfind('/').map_or(0, |i| i + 1)..];
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => &p[p.len() - (name.len() - i)..],
        _ => "",
    }
}

fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

struct Font<'a> {
    display: &'a str,
    family: String,
    def: J,
    arena: Option<&'a str>,
}

struct Convert {
    files: BTreeMap<String, Vec<u8>>,
}

impl Convert {
    fn texture(&mut self, textures: &[Tex], arena: &[u8], display: &str, id: i64) -> Result<(J, usize, usize), String> {
        let found: Vec<&Tex> = textures.iter().filter(|t| {
            let base = t.name.replace('\\', "/");
            let base = base.rsplit('/').next().unwrap_or("");
            base.rsplit_once('.').map_or(base, |x| x.0) == id.to_string()
        }).collect();
        let [t] = found[..] else { return Err(format!("{display}: expected one texture resource {id}, found {}", found.len())) };
        let (j, w, h) = self.decode(t, arena, display)?;
        Ok((o(vec![("resource_name", t.name.as_str().into()), ("width", w.into()), ("height", h.into()),
            ("preview", j.0.into()), ("rgba", j.1.into())]), w, h))
    }

    fn decode(&mut self, t: &Tex, arena: &[u8], display: &str) -> Result<((String, String), usize, usize), String> {
        let fetch = fetch(t.fetch.as_ref().ok_or(format!("{display}: not an texture"))?);
        let name = format!("assets/{display}/{:04}_{}", t.index, safe_name(&t.name, t.index));
        let rgba = format!("{name}.rgba");
        if !self.files.contains_key(&rgba) {
            let data = decode_rgba(&arena[t.data.0..t.data.1], &fetch).map_err(|e| format!("{display}: texture {}: {e}", t.index))?;
            self.files.insert(rgba.clone(), data);
        }
        Ok(((format!("{name}.png"), rgba), fetch.width, fetch.height))
    }

    fn font(&mut self, fonts: &[Font], aliases: &HashMap<String, (String, J)>, family: &str,
        get: &dyn Fn(&str) -> Result<Vec<u8>, String>) -> Result<J, String> {
        let alias = aliases.get(family);
        let target = alias.map_or(family, |a| a.0.as_str());
        let found: Vec<&Font> = fonts.iter().filter(|f| f.family == target).collect();
        let [f] = found[..] else { return Ok(J::Null) };
        let data = get(f.arena.ok_or(format!("{}: no texture arena", f.display))?)?;
        let tex = rw4(&data)?;
        let t = tex.first().ok_or(format!("{}: no textures", f.display))?;
        let ((preview, rgba), _, _) = self.decode(t, &data, f.display)?;
        let mut v = vec![("bundle", f.display.into()), ("metadata", format!("metadata/{}.font.json", f.display).into()),
            ("texture", rgba.into()), ("preview", preview.into()), ("definition", f.def.clone())];
        if let Some((t, prov)) = alias {
            v.push(("requested_family", family.into()));
            v.push(("alias", o(vec![("target", t.as_str().into()), ("provenance", prov.clone())])));
        }
        Ok(o(v))
    }
}

#[derive(Clone)]
enum J {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(v) => v.iter().find(|x| x.0 == k).map(|x| &x.1),
            _ => None,
        }
    }
    fn push(&mut self, k: &str, v: J) {
        if let J::Obj(o) = self {
            o.push((k.into(), v));
        }
    }
    fn int(&self) -> Option<i64> {
        if let J::Int(i) = self { Some(*i) } else { None }
    }
    fn str(&self) -> Option<&str> {
        if let J::Str(s) = self { Some(s) } else { None }
    }
    fn arr(&self) -> &[J] {
        if let J::Arr(v) = self { v } else { &[] }
    }
}

macro_rules! ints { ($($t:ty),*) => { $(impl From<$t> for J { fn from(v: $t) -> J { J::Int(v as i64) } })* } }
ints!(u8, u16, u32, i32, i64, usize);
impl From<bool> for J {
    fn from(v: bool) -> J { J::Bool(v) }
}
impl From<&str> for J {
    fn from(v: &str) -> J { J::Str(v.into()) }
}
impl From<String> for J {
    fn from(v: String) -> J { J::Str(v) }
}

fn o(v: Vec<(&str, J)>) -> J {
    J::Obj(v.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn floats(v: &[f64]) -> J {
    J::Arr(v.iter().map(|&f| J::Float(f)).collect())
}

fn dump(j: &J, s: &mut String) {
    match j {
        J::Null => s.push_str("null"),
        J::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        J::Int(i) => s.push_str(&i.to_string()),
        J::Float(f) => py_float(*f, s),
        J::Str(t) => py_str(t, s),
        J::Arr(v) => {
            s.push('[');
            for (n, x) in v.iter().enumerate() {
                if n > 0 {
                    s.push(',');
                }
                dump(x, s);
            }
            s.push(']');
        }
        J::Obj(v) => {
            s.push('{');
            for (n, (k, x)) in v.iter().enumerate() {
                if n > 0 {
                    s.push(',');
                }
                py_str(k, s);
                s.push(':');
                dump(x, s);
            }
            s.push('}');
        }
    }
}

fn py_float(x: f64, s: &mut String) {
    if x.is_nan() {
        return s.push_str("NaN");
    }
    if x.is_infinite() {
        return s.push_str(if x > 0.0 { "Infinity" } else { "-Infinity" });
    }
    if x.is_sign_negative() {
        s.push('-');
    }
    if x == 0.0 {
        return s.push_str("0.0");
    }
    let e = format!("{:e}", x.abs());
    let (m, exp) = e.split_once('e').unwrap();
    let digits: String = m.chars().filter(|&c| c != '.').collect();
    let (n, point) = (digits.len() as i32, exp.parse::<i32>().unwrap() + 1);
    if point <= -4 || point > 16 {
        s.push_str(&digits[..1]);
        if n > 1 {
            s.push('.');
            s.push_str(&digits[1..]);
        }
        s.push_str(&format!("e{}{:02}", if point > 0 { '+' } else { '-' }, (point - 1).abs()));
    } else if point <= 0 {
        s.push_str("0.");
        s.push_str(&"0".repeat(-point as usize));
        s.push_str(&digits);
    } else if point >= n {
        s.push_str(&digits);
        s.push_str(&"0".repeat((point - n) as usize));
        s.push_str(".0");
    } else {
        s.push_str(&digits[..point as usize]);
        s.push('.');
        s.push_str(&digits[point as usize..]);
    }
}

fn py_str(t: &str, s: &mut String) {
    s.push('"');
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\x08' => s.push_str("\\b"),
            '\x0c' => s.push_str("\\f"),
            ' '..='~' => s.push(c),
            _ => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    s.push_str(&format!("\\u{u:04x}"));
                }
            }
        }
    }
    s.push('"');
}

struct R<'a>(&'a [u8], &'a str);

impl R<'_> {
    fn at(&self, o: usize, n: usize) -> Result<&[u8], String> {
        o.checked_add(n).and_then(|e| self.0.get(o..e)).ok_or_else(|| format!("{}: {o}+{n} is outside {} bytes", self.1, self.0.len()))
    }
    fn need(&self, o: usize, n: usize) -> Result<(), String> {
        self.at(o, n).map(|_| ())
    }
    fn u8(&self, o: usize) -> Result<u8, String> {
        Ok(self.at(o, 1)?[0])
    }
    fn u16(&self, o: usize) -> Result<u16, String> {
        Ok(u16::from_be_bytes(self.at(o, 2)?.try_into().unwrap()))
    }
    fn u32(&self, o: usize) -> Result<u32, String> {
        Ok(u32::from_be_bytes(self.at(o, 4)?.try_into().unwrap()))
    }
    fn i32(&self, o: usize) -> Result<i32, String> {
        Ok(self.u32(o)? as i32)
    }
    fn f32(&self, o: usize) -> Result<f64, String> {
        Ok(f32::from_bits(self.u32(o)?) as f64)
    }
    fn floats(&self, o: usize, n: usize) -> Result<J, String> {
        Ok(J::Arr((0..n).map(|i| self.f32(o + i * 4).map(J::Float)).collect::<Result<_, _>>()?))
    }
    fn cstr(&self, o: u32) -> String {
        let o = o as usize;
        if o == 0 || o >= self.0.len() {
            return String::new();
        }
        let end = self.0[o..].iter().position(|&b| b == 0).map_or(self.0.len(), |e| o + e);
        String::from_utf8_lossy(&self.0[o..end]).into_owned()
    }
}

fn type_name(names: &[&str], t: u32) -> String {
    names.get(t as usize).filter(|_| t > 0).map_or(format!("unknown_{t}"), |s| s.to_string())
}

fn character(a: &R, offset: usize, id: usize) -> Result<Option<J>, String> {
    if offset == 0 {
        return Ok(None);
    }
    a.need(offset, 16)?;
    let t = a.u32(offset)?;
    let body = offset + 16;
    let mut c = o(vec![("id", id.into()), ("offset", offset.into()), ("type", t.into()), ("type_name", type_name(&CHARACTERS, t).into())]);
    if matches!(t, 1 | 2 | 10) {
        a.need(body, 16)?;
        c.push("bounds", a.floats(body, 4)?);
    }
    match t {
        2 => {
            a.need(body, 52)?;
            c.push("text", o(vec![("font_id", a.u32(body + 16)?.into()), ("alignment", a.u32(body + 20)?.into()),
                ("color_argb", format!("#{:08x}", a.u32(body + 24)?).into()), ("font_height", J::Float(a.f32(body + 28)?)),
                ("read_only", (a.u32(body + 32)? != 0).into()), ("multiline", (a.u32(body + 36)? != 0).into()),
                ("word_wrap", (a.u32(body + 40)? != 0).into()), ("initial_text", a.cstr(a.u32(body + 44)?).into()),
                ("variable", a.cstr(a.u32(body + 48)?).into())]));
        }
        3 => {
            a.need(body, 12)?;
            let (n, at) = (a.u32(body + 4)? as usize, a.u32(body + 8)? as usize);
            if n > 100_000 {
                return Err("unreasonable APT font glyph count".into());
            }
            if n > 0 {
                a.need(at, n * 4)?;
            }
            let glyphs = (0..n).map(|i| a.u32(at + i * 4).map(J::from)).collect::<Result<_, _>>()?;
            c.push("font", o(vec![("name", a.cstr(a.u32(body)?).into()), ("glyph_character_ids", J::Arr(glyphs))]));
        }
        10 => return Err(format!("character {id}: static text isn't supported")),
        5 | 9 => {
            a.need(body, 12)?;
            let frames = a.u32(body)?;
            if frames > 100_000 {
                return Err("unreasonable APT frame count".into());
            }
            let mut m = o(vec![("frame_count", frames.into()), ("frames_offset", a.u32(body + 4)?.into()), ("labels_offset", a.u32(body + 8)?.into())]);
            if t == 9 {
                a.need(body, 52)?;
                for (i, k) in ["character_count", "characters_offset", "width", "height", "milliseconds_per_frame", "import_count",
                    "imports_offset", "export_count", "exports_offset", "current_constant_index"].into_iter().enumerate() {
                    m.push(k, a.u32(body + 12 + i * 4)?.into());
                }
            }
            c.push("movie", m);
        }
        7 => {
            a.need(body, 4)?;
            c.push("bitmap", o(vec![("texture_id", a.u32(body)?.into())]));
        }
        _ => {}
    }
    Ok(Some(c))
}

fn frames(a: &R, movie: &J) -> Result<J, String> {
    let n = movie.get("frame_count").and_then(J::int).unwrap_or(0) as usize;
    let at = movie.get("frames_offset").and_then(J::int).unwrap_or(0) as usize;
    a.need(at, n * 8)?;
    let mut out = vec![];
    for i in 0..n {
        let (count, list) = (a.u32(at + i * 8)? as usize, a.u32(at + i * 8 + 4)? as usize);
        if count > 100_000 {
            return Err(format!("frame {i}: unreasonable control count"));
        }
        a.need(list, count * 4)?;
        let mut controls = vec![];
        for k in 0..count {
            let p = a.u32(list + k * 4)? as usize;
            if p != 0 {
                controls.push(control(a, p)?);
            }
        }
        out.push(o(vec![("index", i.into()), ("controls", J::Arr(controls))]));
    }
    Ok(J::Arr(out))
}

fn control(a: &R, at: usize) -> Result<J, String> {
    let t = a.u32(at)?;
    let mut c = o(vec![("offset", at.into()), ("type", t.into()), ("type_name", type_name(&CONTROLS, t).into())]);
    let first_op = |c: &mut J, ao: u32| -> Result<(), String> {
        if ao != 0 {
            c.push("first_action_opcode", a.u8(ao as usize)?.into());
        }
        Ok(())
    };
    match t {
        3 | 9 => {
            a.need(at, if t == 9 { 72 } else { 64 })?;
            let ct = a.at(at + 40, 8)?.iter().map(|&b| J::from(b)).collect();
            for (k, v) in [("flags", a.u32(at + 4)?.into()), ("depth", a.i32(at + 8)?.into()), ("character_id", a.i32(at + 12)?.into()),
                ("matrix", a.floats(at + 16, 6)?), ("color_transform", J::Arr(ct)), ("ratio", J::Float(a.f32(at + 48)?)),
                ("name", a.cstr(a.u32(at + 52)?).into()), ("clip_depth", a.i32(at + 56)?.into()), ("actions_offset", a.u32(at + 60)?.into())] {
                c.push(k, v);
            }
            if t == 9 {
                c.push("blend_mode", a.i32(at + 64)?.into());
                c.push("filter_pointer", a.u32(at + 68)?.into());
            }
        }
        2 => c.push("label", a.cstr(a.u32(at + 4)?).into()),
        5 => c.push("color", format!("#{:08x}", a.u32(at + 4)?).into()),
        1 => {
            let ao = a.u32(at + 4)?;
            c.push("actions_offset", ao.into());
            first_op(&mut c, ao)?;
        }
        8 => {
            a.need(at, 12)?;
            c.push("sprite_id", a.u32(at + 4)?.into());
            let ao = a.u32(at + 8)?;
            c.push("actions_offset", ao.into());
            first_op(&mut c, ao)?;
        }
        4 | 11 => c.push("depth", a.i32(at + 4)?.into()),
        6 => c.push("sound_id", a.u32(at + 4)?.into()),
        _ => {}
    }
    Ok(c)
}

struct Actions<'a> {
    apt: &'a R<'a>,
    constants: Vec<J>,
}

impl<'a> Actions<'a> {
    fn new(apt: &'a R<'a>, c: &R) -> Result<Actions<'a>, String> {
        let (count, start) = (c.u32(24)? as usize, c.u32(28)? as usize);
        c.need(start, count * 8)?;
        let mut constants = vec![];
        for i in 0..count {
            let at = start + i * 8;
            let (kind, value) = (c.u32(at)?, c.u32(at + 4)?);
            let v = match kind {
                1 => {
                    c.need(value as usize, 1)?;
                    c.cstr(value).into()
                }
                6 => J::Float(c.f32(at + 4)?),
                7 => c.i32(at + 4)?.into(),
                _ => value.into(),
            };
            constants.push(o(vec![("kind", kind.into()), ("value", v)]));
        }
        Ok(Actions { apt, constants })
    }

    fn stream(&self, mut at: usize, end: Option<usize>, nesting: u32) -> Result<J, String> {
        if nesting > 64 {
            return Err("APT action nesting exceeds 64".into());
        }
        let r = self.apt;
        let end = end.unwrap_or(r.0.len());
        r.need(at, end.checked_sub(at).ok_or("APT action block ends before it starts")?)?;
        let mut out = vec![];
        while at < end {
            let (start, op) = (at, r.u8(at)?);
            at += 1;
            let mut row = o(vec![("offset", start.into()), ("opcode", op.into())]);
            if op == 0 {
                out.push(row);
                break;
            }
            match op {
                0xA2 | 0xAE | 0xAF | 0xB0 | 0xB1 | 0xB2 | 0xB3 | 0xB5 | 0xB9 => {
                    row.push("operand", r.u8(at)?.into());
                    at += 1;
                }
                0xA3 | 0xB6 => {
                    row.push("operand", r.u16(at)?.into());
                    at += 2;
                }
                0x77 | 0xB4 | 0xB7 => {
                    row.push("operand", r.i32(at)?.into());
                    at += 4;
                }
                0x81 | 0x87 | 0x99 | 0x9D | 0x9F | 0xB8 | 0x8B | 0x8C | 0xA1 | 0xA4 | 0xA5 | 0xA6 | 0xA7 | 0x94 => {
                    at = at.next_multiple_of(4);
                    let v = r.u32(at)?;
                    let text = matches!(op, 0x8B | 0x8C | 0xA1 | 0xA4 | 0xA5 | 0xA6 | 0xA7);
                    row.push("operand", if text { r.cstr(v).into() } else { v.into() });
                    if matches!(op, 0x99 | 0x9D | 0x94 | 0xB8) {
                        row.push("target", (at as i64 + 4 + r.i32(at)? as i64).into());
                    }
                    at += 4;
                }
                0x88 | 0x96 => {
                    at = at.next_multiple_of(4);
                    let (count, p) = (r.u32(at)? as usize, r.u32(at + 4)? as usize);
                    r.need(p, count * 4)?;
                    let mut values = vec![];
                    for i in 0..count {
                        let k = r.u32(p + i * 4)? as usize;
                        values.push(self.constants.get(k).cloned().ok_or(format!("APT constant index outside CONST at {start}"))?);
                    }
                    row.push("values", J::Arr(values));
                    at += 8;
                }
                0x8E | 0x9B => {
                    let f2 = op == 0x8E;
                    at = at.next_multiple_of(4);
                    let header = if f2 { 28 } else { 24 };
                    r.need(at, header)?;
                    row.push("name", r.cstr(r.u32(at)?).into());
                    let count = r.u32(at + 4)? as usize;
                    let p = r.u32(at + if f2 { 12 } else { 8 })? as usize;
                    let len = r.u32(at + if f2 { 16 } else { 12 })? as usize;
                    let stride = if f2 { 8 } else { 4 };
                    r.need(p, count * stride)?;
                    let params = (0..count).map(|i| Ok(o(vec![
                        ("register", if f2 { r.u32(p + i * 8)?.into() } else { 0u32.into() }),
                        ("name", r.cstr(r.u32(p + i * stride + if f2 { 4 } else { 0 })?).into())]))).collect::<Result<_, String>>()?;
                    row.push("parameters", J::Arr(params));
                    row.push("flags", if f2 { r.u32(at + 8)?.into() } else { 0u32.into() });
                    at += header;
                    row.push("body", self.stream(at, Some(at + len), nesting + 1)?);
                    at += len;
                }
                0x83 | 0x8F => return Err(format!("APT compound opcode {op} at {start} needs decoding")),
                _ => {}
            }
            row.push("next", at.into());
            if at > end {
                return Err(format!("APT instruction at {start} crosses its function body"));
            }
            out.push(row);
        }
        Ok(J::Arr(out))
    }
}

struct Unit {
    render: u32,
    color: [f64; 4],
    texture: u32,
    uv: [f64; 6],
    triangles: Vec<[[f64; 2]; 3]>,
}

fn geo(d: &[u8]) -> Result<HashMap<u32, Vec<Unit>>, String> {
    let r = R(d, "homscoring.geo");
    let mut shapes = HashMap::new();
    if d.len() == 8 && r.u32(4)? == 0 {
        return Ok(shapes);
    }
    r.need(0, 12)?;
    let count = r.u32(4)? as usize;
    if count > 4096 {
        return Err(format!("unreasonable GEO shape count {count}"));
    }
    r.need(8, count * 4)?;
    for i in 0..count {
        let s = r.u32(8 + i * 4)? as usize;
        if s == 0 {
            continue;
        }
        let (id, n) = (r.u32(s)?, r.u32(s + 4)? as usize);
        if n > 65536 {
            return Err(format!("shape {id}: unreasonable unit count"));
        }
        r.need(s + 8, n * 4)?;
        let mut units = vec![];
        for k in 0..n {
            let u = r.u32(s + 8 + k * 4)? as usize;
            r.need(u, 0x34)?;
            let prims = r.u32(u + 0x30)? as usize;
            if prims > 1_000_000 {
                return Err(format!("shape {id}: unreasonable primitive count"));
            }
            r.need(u + 0x34, prims * 0x18)?;
            let f = |at: usize| r.f32(at);
            let mut triangles = vec![];
            for p in 0..prims {
                let b = u + 0x34 + p * 0x18;
                triangles.push([[f(b)?, f(b + 4)?], [f(b + 8)?, f(b + 12)?], [f(b + 16)?, f(b + 20)?]]);
            }
            units.push(Unit {
                render: r.u32(u)?,
                color: [f(u + 4)?, f(u + 8)?, f(u + 12)?, f(u + 16)?],
                texture: r.u32(u + 0x14)?,
                uv: [f(u + 0x18)?, f(u + 0x1C)?, f(u + 0x20)?, f(u + 0x24)?, f(u + 0x28)?, f(u + 0x2C)?],
                triangles,
            });
        }
        shapes.insert(id, units);
    }
    Ok(shapes)
}

fn point(m: &[f64; 6], p: &[f64; 2]) -> [f64; 2] {
    [m[0] * p[0] + m[2] * p[1] + m[4], m[1] * p[0] + m[3] * p[1] + m[5]]
}

fn bitmap_font(raw: &[u8], source: &str) -> Result<J, String> {
    if raw.iter().any(|b| (0x80..0xA0).contains(b)) {
        return Err(format!("{source}: unexpected cp1252 punctuation"));
    }
    let text: String = raw.iter().map(|&b| b as char).collect::<String>().replace("\r\n", "\n").replace('\r', "\n");
    let mut starts: Vec<(&str, usize, usize)> = vec![];
    let mut line = 0;
    while line <= text.len() {
        let l = &text[line..];
        let name_end = l.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(l.len());
        if l.starts_with(|c: char| c.is_ascii_alphabetic()) && l[name_end..].starts_with(':') {
            let v = line + name_end + 1;
            let v = v + text[v..].find(|c| c != ' ' && c != '\t').unwrap_or(text.len() - v);
            starts.push((&l[..name_end], line, v));
        }
        match text[line..].find('\n') {
            Some(i) => line += i + 1,
            None => break,
        }
    }
    let mut fields: Vec<(&str, String)> = vec![];
    for (i, &(k, _, v)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map_or(text.len(), |s| s.1);
        let value = text[v..end].replace('\n', " ").trim_matches(py_space).to_string();
        match fields.iter_mut().find(|f| f.0 == k) {
            Some(f) => f.1 = value,
            None => fields.push((k, value)),
        }
    }
    let field = |k: &str| fields.iter().find(|f| f.0 == k).map(|f| f.1.as_str());
    for k in ["Version", "FullName", "Family", "GlyphMetricsMap", "CharMapSet"] {
        field(k).ok_or(format!("{source}: missing bitmap-font field {k}"))?;
    }
    if field("Version") != Some("3") {
        return Err(format!("{source}: unsupported bitmap-font version"));
    }
    let ints = |chunk: &str| chunk.split(py_space).filter(|s| !s.is_empty()).map(|v| v.parse::<i64>().map_err(|_| format!("{source}: bad number {v:?}")))
        .collect::<Result<Vec<_>, _>>();
    let table = |k: &str, width: usize| -> Result<Vec<Vec<i64>>, String> {
        let mut rows = field(k).unwrap().split(',').map(ints).collect::<Result<Vec<_>, _>>()?;
        if rows[0].len() != width + 1 {
            return Err(format!("{source}: malformed first {k} entry"));
        }
        let declared = rows[0].remove(0);
        if rows.iter().any(|r| r.len() != width) || rows.len() as i64 != declared {
            return Err(format!("{source}: malformed {k}"));
        }
        Ok(rows)
    };
    let glyphs = table("GlyphMetricsMap", 9)?.into_iter().map(|g| {
        let (left, top) = (g[2] + g[6], g[3] - g[7]);
        let mut v: Vec<(&str, J)> = ["glyph_index", "texture_index", "x", "y", "width", "height", "x_offset", "y_offset", "x_advance"]
            .into_iter().zip(g.iter().map(|&x| J::Int(x))).collect();
        v.push(("atlas_bounds", J::Arr([left, top, left + g[4], top + g[5]].map(J::Int).to_vec())));
        o(v)
    }).collect();
    let characters = table("CharMapSet", 2)?.into_iter().map(|c| o(vec![("codepoint", J::Int(c[0])), ("glyph_index", J::Int(c[1]))])).collect();
    let mut metrics = vec![];
    for k in ["Weight", "Stretch", "Size", "HAdvanceXMax", "VAdvanceYMax", "Ascent", "Descent", "Leading", "Baseline", "LineHeight",
        "XHeight", "CapsHeight", "UnderlinePosition", "UnderlineThickness", "LinethroughPosition", "LinethroughThickness"] {
        if let Some(v) = field(k) {
            let bad = || format!("{source}: bad {k}");
            metrics.push((k, if v.contains(['.', 'e', 'E']) { J::Float(v.parse().map_err(|_| bad())?) } else { J::Int(v.parse().map_err(|_| bad())?) }));
        }
    }
    let mut textures = vec![];
    for (k, v) in &fields {
        let Some(index) = k.strip_prefix("Texture") else { continue };
        let p: Vec<&str> = v.split(py_space).filter(|s| !s.is_empty()).collect();
        let n = |s: &str| s.parse::<i64>().map_err(|_| format!("{source}: malformed texture declaration {v:?}"));
        if p.len() < 4 {
            return Err(format!("{source}: malformed texture declaration {v:?}"));
        }
        textures.push((n(index)?, o(vec![("index", J::Int(n(index)?)), ("bytes", J::Int(n(p[0])?)), ("width", J::Int(n(p[1])?)),
            ("height", J::Int(n(p[2])?)), ("name", p[3..].join(" ").into())])));
    }
    textures.sort_by_key(|t| t.0);
    Ok(o(vec![("format", "skate3-bitmap-font".into()), ("source", source.into()), ("full_name", field("FullName").unwrap().into()),
        ("family", field("Family").unwrap().into()), ("style", field("Style").unwrap_or("").into()), ("smooth", (field("Smooth") == Some("Yes")).into()),
        ("fixed_pitch", (field("FixedPitch") == Some("Yes")).into()), ("metrics", o(metrics)), ("glyphs", J::Arr(glyphs)),
        ("characters", J::Arr(characters)), ("textures", J::Arr(textures.into_iter().map(|t| t.1).collect()))]))
}

fn language(d: &[u8]) -> Result<Vec<String>, String> {
    let le = |at: usize| d.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize).ok_or("language table is shorter than its header");
    let (declared, header, pool) = (le(8)?, le(12)?, le(16)?);
    if header != 28 {
        return Err(format!("unsupported language header size {header}"));
    }
    let strings: Vec<String> = d.get(pool + 8..).ok_or("string pool lies outside the file")?.split(|&b| b == 0)
        .map(|s| s.iter().map(|&b| b as char).collect()).collect();
    if strings.len() != declared + 2 {
        return Err(format!("expected {} pool strings, found {}", declared + 2, strings.len()));
    }
    Ok(strings)
}

struct Tex {
    index: usize,
    name: String,
    data: (usize, usize),
    fetch: Option<[u8; 24]>,
}

fn rw4(raw: &[u8]) -> Result<Vec<Tex>, String> {
    let r = R(raw, "RW4 arena");
    r.need(0, 68)?;
    if &raw[..4] != b"\x89RW4" {
        return Err("not a RenderWare 4 arena".into());
    }
    let gamepad = match &raw[4..8] {
        b"xb2\0" => true,
        b"ps3\0" => false,
        m => return Err(format!("unknown RW4 platform {m:?}")),
    };
    let (n, dict, header) = (r.u32(32)? as usize, r.u32(48)? as usize, r.u32(68)? as usize);
    if n > 1_000_000 {
        return Err("unreasonable RW4 dictionary count".into());
    }
    r.need(dict, n * 24)?;
    let entries = (0..n).map(|i| Ok((r.u32(dict + i * 24)? as usize, r.u32(dict + i * 24 + 8)? as usize, r.u32(dict + i * 24 + 20)?)))
        .collect::<Result<Vec<_>, String>>()?;
    let toc = entries.iter().find(|e| e.2 == 0x00EB_000B).ok_or("RW4 arena has no table of contents")?.0;
    r.need(toc, 20)?;
    let (count, items) = (r.u32(toc)? as usize, r.u32(toc + 4)? as usize);
    if count > 1_000_000 {
        return Err("unreasonable RW4 TOC count".into());
    }
    r.need(toc + items, count * 24)?;
    let mut out = vec![];
    for i in 0..count {
        let item = toc + items + i * 24;
        let name = r.cstr((toc + r.u32(item)? as usize) as u32);
        let k = r.u32(item + 20)? as usize;
        if k == 0 || k >= entries.len() {
            continue;
        }
        let (payload, info) = (entries[k - 1], entries[k]);
        let at = payload.0 + header;
        r.need(at, payload.1)?;
        let fetch = if gamepad { Some(r.at(info.0 + 28, 24)?.try_into().unwrap()) } else { r.need(info.0, 40).map(|_| None)? };
        out.push(Tex { index: i, name, data: (at, at + payload.1), fetch });
    }
    Ok(out)
}

fn safe_name(name: &str, index: usize) -> String {
    let leaf = name.split(['/', '\\']).filter(|s| !s.is_empty() && *s != ".").next_back().unwrap_or("");
    let s: String = leaf.chars().map(|c| if c.is_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
    if s.is_empty() { format!("texture_{index:04}") } else { s }
}

struct Fetch {
    width: usize,
    height: usize,
    format: u32,
    tiled: bool,
    pitch: usize,
}

fn fetch(d: &[u8; 24]) -> Fetch {
    let w = |i: usize| u32::from_be_bytes(d[i * 4..i * 4 + 4].try_into().unwrap());
    let (w0, w1, w2) = (w(0).reverse_bits(), w(1).reverse_bits(), w(2));
    Fetch {
        width: (w2 & 0x1FFF) as usize + 1,
        height: ((w2 >> 13) & 0x1FFF) as usize + 1,
        format: ((w1 >> 26) & 0x3F).reverse_bits() >> 26,
        tiled: w0 & 1 != 0,
        pitch: (((w(0) >> 22) & 0x1FF) << 5) as usize,
    }
}

fn tiled(x: usize, y: usize, width: usize, log_bpb: u32) -> usize {
    let aligned = width.next_multiple_of(32);
    let macro_ = ((x >> 5) + (y >> 5) * (aligned >> 5)) << (log_bpb + 7);
    let micro = ((x & 7) + ((y & 0xE) << 2)) << log_bpb;
    let offset = macro_ + ((micro & !0xF) << 1) + (micro & 0xF) + ((y & 1) << 4);
    let address = ((offset & !0x1FF) << 3) + ((y & 16) << 7) + ((offset & 0x1C0) << 2) + (((((y & 8) >> 2) + (x >> 3)) & 3) << 6) + (offset & 0x3F);
    address >> log_bpb
}

fn decode_rgba(data: &[u8], f: &Fetch) -> Result<Vec<u8>, String> {
    let (bs, bpb) = match f.format {
        18 => (4, 8),
        19 | 20 => (4, 16),
        6 => (1, 4),
        x => return Err(format!("unsupported texture format {x}")),
    };
    let (bw, bh) = (f.width.div_ceil(bs).max(1), f.height.div_ceil(bs).max(1));
    let packed = bs == 4;
    let mut lin = vec![0u8; bw * bh * bpb];
    let mut put = |dst: usize, src: &[u8]| {
        let d = &mut lin[dst..dst + bpb];
        if packed {
            for i in (0..bpb).step_by(2) {
                (d[i], d[i + 1]) = (src[i + 1], src[i]);
            }
        } else {
            d.iter_mut().zip(src.iter().rev()).for_each(|(d, s)| *d = *s);
        }
    };
    if !f.tiled {
        let pitch = if f.pitch > 0 { f.pitch } else { f.width };
        let row = pitch.div_ceil(bs).max(bw) * bpb;
        let needed = (bh - 1) * row + bw * bpb;
        if data.len() < needed {
            return Err(format!("texture payload is {} bytes, expected at least {needed}", data.len()));
        }
        for y in 0..bh {
            for x in 0..bw {
                let s = y * row + x * bpb;
                put((y * bw + x) * bpb, &data[s..s + bpb]);
            }
        }
    } else {
        let width = f.width.next_multiple_of(if packed { 128 } else { 32 }) / bs;
        for y in 0..bh {
            for x in 0..bw {
                let s = tiled(x, y, width, bpb.trailing_zeros()) * bpb;
                let src = data.get(s..s + bpb).ok_or(format!("tiled texture address {s} exceeds {}-byte payload", data.len()))?;
                put((y * bw + x) * bpb, src);
            }
        }
    }
    let (w, h) = (f.width, f.height);
    let mut out = vec![0u8; w * h * 4];
    if f.format == 6 {
        for (o, p) in out.chunks_exact_mut(4).zip(lin.chunks_exact(4)) {
            o.copy_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
        return Ok(out);
    }
    let rgb = |c: u16| {
        let (r, g, b) = ((c >> 11) & 0x1F, (c >> 5) & 0x3F, c & 0x1F);
        [((r << 3) | (r >> 2)) as u32, ((g << 2) | (g >> 4)) as u32, ((b << 3) | (b >> 2)) as u32]
    };
    for (n, blk) in lin.chunks_exact(bpb).enumerate() {
        let (bx, by) = (n % bw, n / bw);
        let mut alphas: Option<[u8; 16]> = None;
        let color = if f.format == 18 { blk } else { &blk[8..] };
        if f.format == 19 {
            let mut a = [0u8; 16];
            for (i, v) in blk[..8].iter().enumerate() {
                (a[i * 2], a[i * 2 + 1]) = ((v & 0xF) * 17, (v >> 4) * 17);
            }
            alphas = Some(a);
        } else if f.format == 20 {
            let (a0, a1) = (blk[0] as u32, blk[1] as u32);
            let mut pal = [a0, a1, 0, 0, 0, 0, 0, 255];
            if a0 > a1 {
                (1..7).for_each(|i| pal[i + 1] = ((7 - i as u32) * a0 + i as u32 * a1) / 7);
            } else {
                (1..5).for_each(|i| pal[i + 1] = ((5 - i as u32) * a0 + i as u32 * a1) / 5);
            }
            let bits = (0..6).fold(0u64, |acc, i| acc | (blk[2 + i] as u64) << (i * 8));
            alphas = Some(std::array::from_fn(|p| pal[((bits >> (p * 3)) & 7) as usize] as u8));
        }
        let (c0, c1) = (u16::from_le_bytes([color[0], color[1]]), u16::from_le_bytes([color[2], color[3]]));
        let bits = u32::from_le_bytes(color[4..8].try_into().unwrap());
        let (p0, p1) = (rgb(c0), rgb(c1));
        let colors: [[u32; 4]; 4] = if f.format != 18 || c0 > c1 {
            [[p0[0], p0[1], p0[2], 255], [p1[0], p1[1], p1[2], 255],
                std::array::from_fn(|i| if i < 3 { (2 * p0[i] + p1[i]) / 3 } else { 255 }),
                std::array::from_fn(|i| if i < 3 { (p0[i] + 2 * p1[i]) / 3 } else { 255 })]
        } else {
            [[p0[0], p0[1], p0[2], 255], [p1[0], p1[1], p1[2], 255], std::array::from_fn(|i| if i < 3 { (p0[i] + p1[i]) / 2 } else { 255 }), [0; 4]]
        };
        for p in 0..16 {
            let (x, y) = (bx * 4 + p % 4, by * 4 + p / 4);
            if x >= w || y >= h {
                continue;
            }
            let c = colors[((bits >> (p * 2)) & 3) as usize];
            let a = alphas.map_or(c[3] as u8, |a| a[p]);
            out[(y * w + x) * 4..][..4].copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, a]);
        }
    }
    Ok(out)
}

