use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Param {
    pub kind: i32,
    pub scale: i32,
    pub default: f32,
}

fn u32_at(d: &[u8], i: usize) -> Option<u32> {
    d.get(i..i + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

#[derive(Clone, Copy)]
struct Case {
    field: u32,
    kind: u8,
    rec: Option<Param>,
    rec_at: usize,
}

fn cases(d: &[u8]) -> Vec<(usize, u32, Case)> {
    let mut out = vec![];
    let mut i = 0;
    while i + 12 < d.len() {
        let op = match (d[i], d[i + 1], d[i + 2]) {
            (0x3d, _, _) => 1,
            (0x81, m, _) if m >= 0xf8 => 2,
            (0x41, 0x81, m) if m >= 0xf8 => 3,
            _ => 0,
        };
        if op == 0 {
            i += 1;
            continue;
        }
        let h = u32_at(d, i + op).unwrap();
        let mut j = i + op + 4;
        match d.get(j) {
            Some(0x77) => j += 2,
            Some(0x0f) if d.get(j + 1) == Some(&0x87) => j += 6,
            _ => {}
        }
        let t = match (d.get(j), d.get(j + 1)) {
            (Some(0x0f), Some(0x84)) => (j + 6) as i64 + u32_at(d, j + 2).unwrap_or(0) as i32 as i64,
            (Some(0x74), Some(&r)) => (j + 2) as i64 + r as i8 as i64,
            (Some(0x0f), Some(0x85)) => (j + 6) as i64,
            (Some(0x75), _) => (j + 2) as i64,
            _ => {
                i += 1;
                continue;
            }
        };
        let end = j + if d[j] == 0x0f { 6 } else { 2 };
        if let Some(c) = usize::try_from(t).ok().and_then(|t| case(d, t)) {
            out.push((i, h, c));
        }
        i = end;
    }
    out
}

fn case(d: &[u8], t: usize) -> Option<Case> {
    let b = d.get(t..t + 40)?;
    let rex = |x: u8| (0x40..=0x4f).contains(&x);
    let (kind, mut i) = if b[0] == 0xf3 {
        let i = if rex(b[1]) { 2 } else { 1 };
        (b[i..i + 2] == [0x0f, 0x10]).then_some((2u8, i + 2))?
    } else {
        let i = if rex(b[0]) { 1 } else { 0 };
        if rex(b[0]) && b[0] & 8 != 0 {
            return None;
        }
        match b[i] {
            0x8a => (3, i + 1),
            0x8b => (1, i + 1),
            _ => return None,
        }
    };
    if b[i] >> 6 != 1 || b[i] & 7 != 4 || b[i + 2] != 8 {
        return None;
    }
    i += 3;
    if kind == 2 {
        if b[i] != 0xf3 {
            return None;
        }
        i += if rex(b[i + 1]) { 2 } else { 1 };
        if b[i..i + 2] != [0x0f, 0x11] {
            return None;
        }
        i += 2;
    } else {
        if rex(b[i]) {
            i += 1;
        }
        if b[i] != if kind == 3 { 0x88 } else { 0x89 } {
            return None;
        }
        i += 1;
    }
    let (m, rm) = (b[i] >> 6, b[i] & 7);
    if !(1..=2).contains(&m) {
        return None;
    }
    if rm == 4 {
        i += 1;
    }
    let field = if m == 1 { b[i + 1] as u32 } else { u32_at(b, i + 1)? };
    i += if m == 1 { 2 } else { 5 };
    let mut c = Case { field, kind, rec: None, rec_at: 0 };
    if kind == 2 {
        if !(matches!(b[i], 0x48 | 0x4c) && b[i + 1] == 0x63 && b[i + 2] & 0xc7 == 5) {
            return None;
        }
        let a = (t + i + 7) as i64 + u32_at(b, i + 3)? as i32 as i64;
        let a = usize::try_from(a).ok().filter(|&a| a > 4 && a + 8 < d.len())?;
        let (kind, scale) = (u32_at(d, a - 4)? as i32, u32_at(d, a)? as i32);
        if kind != 2 || !(0..31).contains(&scale) {
            return None;
        }
        c.rec = Some(Param { kind, scale, default: f32::from_bits(u32_at(d, a + 4)?) });
        c.rec_at = a - 4;
    }
    Some(c)
}

fn data_section(d: &[u8]) -> Option<(usize, usize)> {
    let pe = u32_at(d, 0x3c)? as usize;
    let n = u16::from_le_bytes(d.get(pe + 6..pe + 8)?.try_into().ok()?) as usize;
    let o = pe + 0x18 + u16::from_le_bytes(d.get(pe + 0x14..pe + 0x16)?.try_into().ok()?) as usize;
    let s = (0..n).map(|k| o + k * 40).find(|&s| d.get(s..s + 8) == Some(b".data\0\0\0"))?;
    Some((u32_at(d, s + 12)? as usize, u32_at(d, s + 8)? as usize))
}

fn defaults_copy(d: &[u8], data: (usize, usize), cs: &[Case]) -> Option<(usize, u32, u32)> {
    let mut fl: Vec<(u32, u32)> = cs.iter().filter_map(|c| Some((c.field, c.rec?.default.to_bits()))).collect();
    fl.sort_unstable();
    fl.dedup_by_key(|f| f.0);
    let plain = |v: u32| matches!(f32::from_bits(v), 0.0 | 1.0 | -1.0);
    let area = d.get(data.0..(data.0 + data.1).min(d.len()))?;
    let need = fl.len().min(5);
    let mut best: Option<(usize, usize, u32, u32)> = None;
    for w in fl.windows(2).filter(|w| w[1].0 == w[0].0 + 4 && !plain(w[0].1) && !plain(w[1].1)) {
        let mut pat = w[0].1.to_le_bytes().to_vec();
        pat.extend(w[1].1.to_le_bytes());
        for p in (0..area.len().saturating_sub(8)).step_by(4).filter(|&p| area[p..p + 8] == pat[..]).map(|p| p + data.0) {
            let Some(base) = p.checked_sub(w[0].0 as usize) else { continue };
            let ok: Vec<bool> = fl.iter().map(|f| u32_at(d, base + f.0 as usize) == Some(f.1)).collect();
            if ok.iter().filter(|&&o| o).count() >= fl.len() - fl.len() / 10 {
                if best.is_none_or(|b| b.0 < usize::MAX) {
                    best = Some((usize::MAX, base, 0, u32::MAX));
                }
                continue;
            }
            let mut i = 0;
            while i < ok.len() {
                let j = (i..ok.len()).find(|&j| !ok[j]).unwrap_or(ok.len());
                if j - i >= need && best.is_none_or(|b| j - i > b.0) {
                    let to = if j < fl.len() { fl[j].0 } else { fl[j - 1].0 + 0x40 };
                    best = Some((j - i, base, fl[i].0, to));
                }
                i = j + 1;
            }
        }
    }
    best.map(|b| (b.1, b.2, b.3))
}

fn functions(d: &[u8]) -> Vec<(u32, u32)> {
    let Some(pe) = u32_at(d, 0x3c).map(|p| p as usize) else { return vec![] };
    let dir = pe + 0x18 + 0x70 + 3 * 8;
    let (Some(at), Some(n)) = (u32_at(d, dir), u32_at(d, dir + 4)) else { return vec![] };
    let mut f: Vec<(u32, u32)> = (0..n as usize / 12).filter_map(|i| Some((u32_at(d, at as usize + i * 12)?, u32_at(d, at as usize + i * 12 + 4)?))).collect();
    f.sort_unstable();
    f
}

fn inits(d: &[u8], fns: &[(u32, u32)]) -> Vec<(u32, u32, usize)> {
    let rex = |x: u8| (0x40..=0x4f).contains(&x);
    let mut out = vec![];
    for i in 0..d.len().saturating_sub(24) {
        let (mut j, op) = if d[i] == 0xf3 {
            let j = if rex(d[i + 1]) { i + 2 } else { i + 1 };
            if d[j..j + 2] != [0x0f, 0x10] {
                continue;
            }
            (j + 2, 0x11u8)
        } else {
            let j = if rex(d[i]) && d[i] & 8 == 0 { i + 1 } else { i };
            match d[j] {
                0x8a => (j + 1, 0x88),
                0x8b => (j + 1, 0x89),
                _ => continue,
            }
        };
        if d[j] & 0xc7 != 5 {
            continue;
        }
        let reg = (d[j] >> 3) & 7;
        let Some(rel) = u32_at(d, j + 1) else { continue };
        j += 5;
        let Some(rec) = usize::try_from((j as i64) + rel as i32 as i64 - 8).ok().filter(|&r| r + 12 < d.len()) else { continue };
        if op == 0x11 {
            if d[j] != 0xf3 {
                continue;
            }
            j += if rex(d[j + 1]) { 2 } else { 1 };
            if d[j..j + 2] != [0x0f, 0x11] {
                continue;
            }
            j += 2;
        } else {
            if rex(d[j]) {
                j += 1;
            }
            if d[j] != op {
                continue;
            }
            j += 1;
        }
        let (m, rm) = (d[j] >> 6, d[j] & 7);
        if !(1..=2).contains(&m) || (d[j] >> 3) & 7 != reg || rm == 4 {
            continue;
        }
        let field = if m == 1 { d[j + 1] as u32 } else { u32_at(d, j + 1).unwrap() };
        let k = fns.partition_point(|f| f.0 as usize <= i);
        if let Some(&(s, e)) = k.checked_sub(1).map(|k| &fns[k]) && (s as usize) <= i && i < e as usize {
            out.push((s, field, rec));
        }
    }
    out
}

pub fn scan(image: &[u8]) -> HashMap<String, HashMap<u32, Param>> {
    let fns = functions(image);
    let data = data_section(image);
    let init = inits(image, &fns);
    let mut groups: HashMap<usize, Vec<(u32, Case)>> = HashMap::new();
    for (at, h, c) in cases(image) {
        let i = fns.partition_point(|f| f.0 as usize <= at);
        let key = match i.checked_sub(1).map(|i| fns[i]) {
            Some((s, e)) if (s as usize) <= at && at < e as usize => s as usize,
            _ => at >> 12,
        };
        groups.entry(key).or_default().push((h, c));
    }
    let mut out = HashMap::new();
    for (name, want) in MESSAGES {
        let jac = |g: &Vec<(u32, Case)>| {
            let got: std::collections::HashSet<u32> = g.iter().filter(|c| c.1.rec.is_some()).map(|c| c.0).collect();
            let both = want.iter().filter(|h| got.contains(h)).count();
            both as f32 / (want.len() + got.len() - both).max(1) as f32
        };
        let Some(g) = groups.values().max_by(|a, b| jac(a).total_cmp(&jac(b))).filter(|g| jac(g) >= 0.6) else { continue };
        let mut ps: HashMap<u32, Param> = g.iter().filter_map(|(h, c)| Some((*h, c.rec?))).collect();
        let cs: Vec<Case> = g.iter().map(|c| c.1).collect();
        if let Some((base, from, to)) = data.and_then(|data| defaults_copy(image, data, &cs)) {
            for (h, c) in g.iter().filter(|c| c.1.rec.is_none() && (from..to).contains(&c.1.field)) {
                let at = base + c.field as usize;
                let v = match c.kind {
                    3 => image.get(at).map(|&b| b as f32),
                    _ => u32_at(image, at).map(|v| v as i32 as f32),
                };
                if let Some(v) = v.filter(|v| c.kind != 3 || *v <= 1.0) {
                    ps.insert(*h, Param { kind: c.kind as i32, scale: 0, default: v });
                }
            }
        }
        let recs: std::collections::HashSet<usize> = cs.iter().filter(|c| c.rec.is_some()).map(|c| c.rec_at).collect();
        let mut by: HashMap<u32, usize> = HashMap::new();
        for (f, _, r) in &init {
            if recs.contains(r) {
                *by.entry(*f).or_default() += 1;
            }
        }
        if let Some((f, _)) = by.into_iter().filter(|b| b.1 >= 3).max_by_key(|b| b.1) {
            let todo: Vec<(u32, Case)> = g.iter().filter(|c| c.1.rec.is_none() && !ps.contains_key(&c.0)).copied().collect();
            for (h, c) in &todo {
                let Some(&(_, _, r)) = init.iter().find(|i| i.0 == f && i.1 == c.field) else { continue };
                let (Some(t), Some(v)) = (u32_at(image, r), u32_at(image, r + 8)) else { continue };
                if t as u8 == c.kind && (c.kind != 3 || v <= 1) {
                    ps.insert(*h, Param { kind: c.kind as i32, scale: 0, default: v as i32 as f32 });
                }
            }
        }
        out.insert(name.to_string(), ps);
    }
    out
}

pub fn to_text(p: &HashMap<String, HashMap<u32, Param>>) -> String {
    let mut lines: Vec<String> = p.iter().flat_map(|(m, ps)| ps.iter().map(move |(h, r)| format!("{m} {h:08x} {} {} {}", r.kind, r.scale, r.default))).collect();
    lines.sort();
    lines.join("\n") + "\n"
}

pub fn from_text(t: &str) -> HashMap<String, HashMap<u32, Param>> {
    let mut out: HashMap<String, HashMap<u32, Param>> = HashMap::new();
    for l in t.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        if let [m, h, k, s, d] = w[..] && let (Ok(h), Ok(kind), Ok(scale), Ok(default)) = (u32::from_str_radix(h, 16), k.parse(), s.parse(), d.parse()) {
            out.entry(m.to_string()).or_default().insert(h, Param { kind, scale, default });
        }
    }
    out
}

pub const MESSAGES: &[(&str, &[u32])] = &[
    ("armswindmill", &[0x00867a2c, 0x1dd81ce1, 0x22ece45d, 0x69b305c7, 0x6d6ab92f, 0x867f0456, 0x91bca4d7, 0xa14f6523, 0xa327d908, 0xa9dd0e33, 0xc2965b41, 0xc79e8a5a, 0xc7ec7ab0, 0xce36727d, 0xd568bd8d, 0xf9f0f824, 0xfbbd57bd]),
    ("armswindmilladaptive", &[0x094e1165, 0x0e848f17, 0x20d720a7, 0x2630f099, 0x2aa8d8fb, 0x34461548, 0x39f06f8e, 0x495e4301, 0x4d69b2a2, 0x82f188b3, 0x891102a6, 0x91d56a55, 0x94620601, 0x95d855ef, 0xa27f482b, 0xbe78bf77, 0xce1879aa, 0xda119d26, 0xf27c7742, 0xf314edf8, 0xfb8bac76]),
    ("balancercollisionsreaction", &[0x073076c4, 0x0bf79ee1, 0x0d6ef9c0, 0x1562a77e, 0x1bbf51fb, 0x1f7b5ea2, 0x1feada9b, 0x2124eb17, 0x269cd0cd, 0x2a27258c, 0x33aa08f4, 0x34b5aabb, 0x34fad25f, 0x36d0ca3f, 0x370ac62f, 0x44533871, 0x4c20613b, 0x4c2997ba, 0x50f48616, 0x56bd6f12, 0x5d727c27, 0x697f0bd4, 0x70ae64d6, 0x7a8cb37c, 0x7b57f0f8, 0x8069cd9a, 0x8a2710a0, 0x8cf8b3f3, 0x8d0acbfa, 0x969b0749, 0x9a46b0ca, 0x9f8c1522, 0xa2d272e6, 0xa6adb936, 0xa88467a8, 0xaaa967d5, 0xae425190, 0xb35452b4, 0xb450e844, 0xb5b2e23c, 0xba4dfdec, 0xc00e4234, 0xcb3d1493, 0xcd5904e3, 0xd6356ff0, 0xe29d8468, 0xe7f0b85e, 0xebe653bb, 0xf07832bb, 0xf59ca94e, 0xf8bcb015]),
    ("bodybalance", &[0x03c32f7b, 0x120f19b0, 0x1237fbb9, 0x12fa6d02, 0x2630f099, 0x27a250de, 0x353d24fc, 0x39748794, 0x3ac7dba8, 0x3d1a2220, 0x466388e4, 0x554939f7, 0x55c65aff, 0x571b6cc2, 0x5aabade4, 0x66bfec57, 0x6cdacd33, 0x6ea17fe4, 0x72a875cb, 0x77f6773d, 0x7bf99dc0, 0x819ca464, 0x8c67084c, 0x93d867b4, 0x986d5575, 0x9af62638, 0xa1ddf80c, 0xa5b379cf, 0xa8d82801, 0xaae39d77, 0xac7fe6bb, 0xb70ac36b, 0xb7432786, 0xbbd04ced, 0xc330c9e9, 0xd48ac868, 0xd6356ff0, 0xe0229437, 0xe9d0442c, 0xea6429fc, 0xec1adfb3, 0xee9381b1, 0xf4ee2807, 0xf548f20f, 0xf55a524b, 0xfd73e796]),
    ("bodyfoetal", &[0x1101c617, 0x261ae9be, 0x4de0e8dc, 0x60b1e57d]),
    ("bodyrollup", &[0x00bc3713, 0x1101c617, 0x12eb2963, 0x1d2be53f, 0x285a9092, 0x34777290, 0x6f7dec62, 0x8a6491bf, 0x99dc25e2, 0xa51a46d6, 0xc0dc7b93, 0xf8437237]),
    ("bodywrithe", &[0x0706e9e5, 0x097c51d7, 0x0d0c190a, 0x2630f099, 0x29b14f87, 0x66de5ab4, 0x6f1897e1, 0x74cbbaca, 0x86cc977e, 0xb0f8fdfa, 0xb7432786, 0xdd74aae6, 0xdd9ad541, 0xe078b5ef, 0xe0fd1d95, 0xe2151d92, 0xec9ab3ec, 0xf4d0c579, 0xfb4ba208, 0xfd55abd5, 0xff457384]),
    ("braceforimpact", &[0x05e01936, 0x0a5e12aa, 0x0e848f17, 0x108408d7, 0x120f19b0, 0x18b8c3a5, 0x24f4c879, 0x2dd023b0, 0x2f7d5e74, 0x31c1d953, 0x3a809eca, 0x3d1a2220, 0x4a67fea6, 0x4e622c5a, 0x66bfec57, 0x703e088f, 0x7bf99dc0, 0x9217ff17, 0x9b805906, 0xaaed99b7, 0xab097083, 0xad3b9109, 0xbf86190a, 0xc330c9e9, 0xc6e040cb, 0xd6356ff0, 0xd72385ae, 0xdd9ad541, 0xe9d0442c, 0xf99cefb5]),
    ("buoyancy", &[0x00bc3713, 0x289369d2, 0x9cb77638, 0xadadbd07, 0xbfdd618d, 0xe8da5ba0]),
    ("catchfall", &[0x31d5c010, 0x6ef52647, 0xb2d428b7, 0xcb21a0d8, 0xd16d8ac0, 0xd3fcc1ae, 0xfe88096d]),
    ("configurebalance", &[0x01002860, 0x0326d50b, 0x06cc47db, 0x07e1be1b, 0x0cb3e981, 0x0d428898, 0x0e4f0cbc, 0x0fff4b0d, 0x104fd67c, 0x1c5002e8, 0x2102e795, 0x24ed7334, 0x252fa59c, 0x25d2ad62, 0x29c54855, 0x2a8884d5, 0x2d6487d7, 0x3497c18a, 0x349cdcca, 0x35451ac9, 0x3571bfc5, 0x428c2b6f, 0x4332bda7, 0x47e73885, 0x4b2dadc1, 0x4cec1434, 0x4f7c1606, 0x506f54f7, 0x53de1762, 0x560d483c, 0x5831d084, 0x5895953e, 0x58e02daf, 0x5de470a0, 0x6269e278, 0x686a0249, 0x6b5dd8b7, 0x747a3a5c, 0x76e322dc, 0x76fd3fe9, 0x7cf88532, 0x80a0b83d, 0x80a45b05, 0x834b2856, 0x93ac8d44, 0x95a8830e, 0x96c82edf, 0x971c4a16, 0x9b5069ce, 0x9d0be20d, 0x9f081314, 0xa0648e98, 0xa4405a9e, 0xa4c37abf, 0xa5d68398, 0xa6a94748, 0xa72cf85c, 0xa81569cc, 0xb101473c, 0xb30bef9a, 0xb7419870, 0xbb58b7cf, 0xbcda0e93, 0xbdbc15cc, 0xbf8ee90b, 0xc09e00f1, 0xc2ed0b44, 0xc40e9adc, 0xc5ebfd7b, 0xc7894428, 0xca713c75, 0xcfe51361, 0xd07094d6, 0xd7cec708, 0xd9da2257, 0xdbfe151c, 0xdd9ad541, 0xe11576e1, 0xe72f16d4, 0xe8cc8e65, 0xe960cc1c, 0xec7b9f0b, 0xefa39467, 0xf05fb178, 0xf655b4a1, 0xf71cc916, 0xfe6cc00d, 0xfe9d3036, 0xffcdc6dd]),
    ("falloverwall", &[0x00bc3713, 0x0e848f17, 0x1bc16d21, 0x31426e35, 0x31a7053c, 0x7ef46c13, 0x834e1923, 0x85023f3f, 0x87267b12, 0x977d8a88, 0xa20daf59, 0xade685e2, 0xb5e39282, 0xd3fcc1ae, 0xdddf5299, 0xef4de4b6, 0xf6398f18, 0xfa5bd265, 0xfd132785]),
    ("grab", &[0x06fe7c19, 0x0e848f17, 0x1d047946, 0x2058bb5f, 0x2630f099, 0x31c1d953, 0x4a67fea6, 0x58da6b05, 0x6100f523, 0x7ccea036, 0x81cc7f64, 0xd7cf5d4f, 0xe2ad47ec, 0xe9e06aa2, 0xfe1275d3]),
    ("headlook", &[0x00bc3713, 0x1101c617, 0x2d9ac14e]),
    ("highfall", &[0x0e848f17, 0x0f4ef108, 0x1908c5c5, 0x1ef23371, 0x206a38b4, 0x277a4064, 0x2fd64983, 0x3167c764, 0x34461548, 0x38cb2cb9, 0x399ed1f0, 0x3a257609, 0x426346f3, 0x49b90d61, 0x50e35e88, 0x536ed681, 0x5e04545c, 0x64e8d0f3, 0x707fa891, 0x76cf5594, 0x7868b4f7, 0x7b17e392, 0x7bbd2a27, 0x82a2b300, 0x925c9d95, 0x9f969ed3, 0xa2ec7c8e, 0xacf2ceae, 0xb70cce37, 0xbfbbebc9, 0xd078977b, 0xd385ad6a, 0xe118a548, 0xe1f4a2c9, 0xe2209616, 0xe271ed21, 0xe72f83c2, 0xebb393ff, 0xf4d90712, 0xf904e22f, 0xfb4ba208]),
    ("keyframe", &[0x16094279, 0x3387d79d, 0x3a9e42b6, 0x60ad7718, 0xc75281d8, 0xcae6fd66, 0xe8217170, 0xf3cab384]),
    ("meleereact", &[0x0ce26b0f, 0x2299dd4d, 0x3443d4c6, 0x39043bd8, 0x84d0401b, 0x9a46b0ca, 0xa4c577ba, 0xd1ecfbb2, 0xf2110347, 0xf9363c38]),
    ("onfire", &[0x0786b8f2, 0x1fca4527, 0x609ea74d, 0x644d6370, 0x6ff36381, 0x83f4282a, 0xbef6bf6c, 0xbf93ebe5, 0xc512323c, 0xe078b5ef, 0xe71fc544, 0xec9ab3ec, 0xee8ab42a]),
    ("pedallegs", &[0x00867a2c, 0x102e5284, 0x20d720a7, 0x30c7994a, 0x4fbb9cf3, 0x8560ab87, 0xb2d379b8, 0xcb687505, 0xd9ea316b, 0xdd9ad541, 0xe19063b8, 0xe8d7d864, 0xf7cecc5c]),
    ("pointarm", &[0x0745dc36, 0x76689210, 0x859ead0e, 0x8886a83c, 0xa1d00b6c, 0xb228d1f8, 0xd4eda2b1, 0xdd341d4d, 0xf6e57cc2, 0xfa2777c9]),
    ("pointgun", &[0x014d07ac, 0x0538f55a, 0x06a8c572, 0x08b1dc5b, 0x09ebcff2, 0x0ee442d4, 0x12c292a1, 0x212adfb9, 0x2630f099, 0x2b2eef31, 0x310c1427, 0x39a90aa7, 0x4176ba90, 0x45e1208a, 0x513e6e0f, 0x5274f454, 0x55b2aa55, 0x5d517209, 0x65982a09, 0x6f5df967, 0x7d99d8ad, 0x832142e7, 0x849d3860, 0x85aaaeed, 0x8c126bdd, 0x94aeaba2, 0xaab58a61, 0xad28e0e0, 0xb7432786, 0xbb58b7cf, 0xbdc780d9, 0xc07d239b, 0xc53bbf28, 0xc7eaeba9, 0xcba3c9db, 0xd643bd9b, 0xd70429c9, 0xdd486078, 0xdfa909fb, 0xe743f9bf, 0xee2ce8c2, 0xf8d5c999, 0xf9ecf1e4, 0xfc04dc23]),
    ("quadrupedbase", &[0x0a364789, 0x1179b8be, 0x15ee4d67, 0x1689ff1e, 0x16cde78a, 0x1738b58a, 0x18b9c869, 0x1f3617d5, 0x20aefd3e, 0x2128bd54, 0x2d0db5ae, 0x33a9d629, 0x3436443c, 0x38505b49, 0x389aba64, 0x463334cb, 0x488dae09, 0x491b9b33, 0x4bbace6e, 0x4c13f16d, 0x4ceee7e9, 0x506f54f7, 0x52fdf336, 0x5312f1ef, 0x554939f7, 0x56ae4579, 0x58216f4c, 0x60b32079, 0x6333fb03, 0x65b9a730, 0x66c0962f, 0x6bb74447, 0x6ce93cc5, 0x6ded80a2, 0x6f1897e1, 0x741b990a, 0x75a875d8, 0x75eb5e0a, 0x76e322dc, 0x78748c58, 0x84cf43be, 0x88cc029f, 0x9a46b0ca, 0xa5d68398, 0xb307bf91, 0xbc7d54fd, 0xbda62333, 0xbf41a9ff, 0xc55d4193, 0xc5dfb64f, 0xc7c20eec, 0xcd714cf5, 0xcdd11768, 0xd1692f94, 0xd5842c66, 0xd9383a95, 0xdcaa8fcc, 0xdcdc9dcb, 0xdd9ad541, 0xddbfdea2, 0xe4adac13, 0xe6200ae9, 0xe761efb1, 0xe9b3c2b2, 0xec1e1f8a, 0xf08a586f, 0xfd73e796, 0xfd9e1e32]),
    ("reachforwound", &[0x00ac70eb, 0x0159799c, 0x0aafaa67, 0x0e7f50bb, 0x0ece4bca, 0x1d63ced4, 0x208793f8, 0x2630f099, 0x288b761d, 0x2abeb75e, 0x2d738ee9, 0x36f06853, 0x4a97e211, 0x51e0fde6, 0x53d14f44, 0x554939f7, 0x5dfefe92, 0x5e1747b7, 0x83f62e0d, 0x8d34bd72, 0xa1368c29, 0xa40c2ad1, 0xa80c9c3f, 0xac441de8, 0xc9da11ba, 0xd1046637, 0xd8144859, 0xdd9ad541, 0xddce35a1, 0xdeff495c, 0xef4b9ab5, 0xf06a101e, 0xf29b83b7, 0xfbf2dfc7, 0xfe353d6b]),
    ("rolldownstairs", &[0x00bc3713, 0x09547dc8, 0x0d219e66, 0x1101c617, 0x285a9092, 0x303de682, 0x4c2b9502, 0x5cc163c6, 0x6481e71f, 0x6f7dec62, 0x8dca3ebd, 0x99dc25e2, 0x9bcda788, 0xa506f28d, 0xae8ff89f, 0xb8bebffe, 0xc0dc7b93, 0xd3fcc1ae, 0xd9ec3162, 0xef4de4b6, 0xf8437237]),
    ("shot", &[0x09f6b2e8, 0x0e756b0a, 0x0e848f17, 0x1183ce07, 0x142093c9, 0x16cde78a, 0x1b6528ff, 0x1ffa26f4, 0x2630f099, 0x270a701e, 0x30eaf283, 0x3145d363, 0x35b11ddd, 0x4b62874f, 0x4bbace6e, 0x4c13f16d, 0x544e6915, 0x56dab19f, 0x58216f4c, 0x5abb8d64, 0x6333fb03, 0x727135cd, 0x7c09155a, 0x85375894, 0x930a6492, 0x9abba7c9, 0x9c33dad1, 0xa380437d, 0xab3b75ea, 0xb9a40189, 0xbc97ba13, 0xc1da2f06, 0xc54479a5, 0xd2a70a2c, 0xd6d4fed6, 0xd870df26, 0xddbfdea2, 0xe42740c4, 0xe68d8296, 0xec1e1f8a, 0xfd73e796]),
    ("shotshockspin", &[0x0e27e08b, 0x14fbe73c, 0x278a4173, 0x2bc90f49, 0x32e5d943, 0x35f99874, 0x3737ec9a, 0x3c653478, 0x4937e71a, 0x84939e76, 0x88dd669c, 0x97449cb5, 0x9f692282, 0xa0b96be1, 0xa4a598ad, 0xa705cc65, 0xab677e86, 0xbb58b7cf, 0xc06487e0, 0xd49f995f, 0xd946d876, 0xe657d6e6, 0xef446bdc, 0xfc04dc23]),
    ("shotspin", &[0x0e27e08b, 0x14fbe73c, 0x278a4173, 0x2bc90f49, 0x32e5d943, 0x35f99874, 0x3737ec9a, 0x3c653478, 0x4937e71a, 0x84939e76, 0x88dd669c, 0x97449cb5, 0x9f692282, 0xa0b96be1, 0xa4a598ad, 0xa705cc65, 0xab677e86, 0xbb58b7cf, 0xc06487e0, 0xd49f995f, 0xd946d876, 0xe657d6e6, 0xef446bdc, 0xfc04dc23]),
    ("sit", &[0x002d1ca5, 0x05ed18ec, 0x088376fa, 0x16094279, 0x1d0ffcf0, 0x1d24def6, 0x27609925, 0x3fef8b11, 0x4005b366, 0x4356388b, 0x470e916c, 0x47195a12, 0x5acb727f, 0x60ad7718, 0x66b2fa7e, 0x66c0962f, 0x696858c4, 0x69aa1b80, 0x7798e2c0, 0x7a822fad, 0x7c3ef672, 0x7eb6c0a8, 0x8c1faf84, 0x949a1547, 0x95df888f, 0x9efb836c, 0xa44a3b46, 0xad50f0e6, 0xb6006cdc, 0xb878dcfc, 0xbf94ac99, 0xc47ee7c4, 0xd3dd8841, 0xdface7f3, 0xe1f4a2c9, 0xe31dbe91, 0xe4ff2d88, 0xf3cab384, 0xfa7b24dc, 0xff905c00, 0xffb7714b, 0xffecd2eb]),
    ("smartfall", &[0x0e848f17, 0x0f4ef108, 0x1908c5c5, 0x1ef23371, 0x206a38b4, 0x277a4064, 0x2fd64983, 0x3167c764, 0x34461548, 0x38cb2cb9, 0x399ed1f0, 0x3a257609, 0x426346f3, 0x49b90d61, 0x50e35e88, 0x536ed681, 0x5e04545c, 0x64e8d0f3, 0x707fa891, 0x76cf5594, 0x7868b4f7, 0x7b17e392, 0x7bbd2a27, 0x82a2b300, 0x925c9d95, 0x9f969ed3, 0xa2ec7c8e, 0xacf2ceae, 0xb70cce37, 0xbfbbebc9, 0xd078977b, 0xd385ad6a, 0xe118a548, 0xe1f4a2c9, 0xe2209616, 0xe271ed21, 0xe72f83c2, 0xebb393ff, 0xf4d90712, 0xf904e22f, 0xfb4ba208]),
    ("staggerfall", &[0x026229d1, 0x03b9314c, 0x0850eaa4, 0x108408d7, 0x1e932d3e, 0x2630f099, 0x2939cef8, 0x2b087d19, 0x33d6b0aa, 0x3471694d, 0x34fd6a6a, 0x3eba313b, 0x3ffe8c02, 0x46f2b4db, 0x4f7c1606, 0x547dbc74, 0x554939f7, 0x56480d47, 0x582ed72b, 0x5917ada8, 0x5bfdb57d, 0x6094a2c1, 0x609cae97, 0x64d754d9, 0x668ad6c5, 0x678b7050, 0x693667bc, 0x730e78ad, 0x759dab0b, 0x80ab3af6, 0x80c462fd, 0x86f98004, 0x8d470083, 0x8f79d1b4, 0x92ab406c, 0x98aef890, 0x98ffd702, 0xab6ee20b, 0xae767e25, 0xb1f9806a, 0xb7432786, 0xb9365403, 0xbb5fe1b5, 0xbde9f74f, 0xbdedb718, 0xc5bbca8a, 0xc64dc386, 0xc87e57d0, 0xd42a30a6, 0xd88c9c0e, 0xe0229437, 0xe0235b5b, 0xe65a6ddf, 0xe691742c, 0xea6429fc, 0xec7c285e, 0xee9381b1, 0xef4feb17, 0xf0c7cfa5, 0xf2636537, 0xfd73e796]),
    ("stayupright", &[0x039910cf, 0x03d0cbe2, 0x09cb8e2e, 0x0d55caf9, 0x0f6b5ab0, 0x283ba685, 0x28e9c730, 0x2d2bf883, 0x2e80e537, 0x524d9244, 0x56bf08b0, 0x58f9e07f, 0x5ad9a221, 0x6593693e, 0x7289816b, 0x8511d1f2, 0x8902973d, 0x8acfbdbf, 0x924b2693, 0x96719913, 0xab3a861c, 0xc3c468df, 0xd2c2d874, 0xdd0cc848, 0xe122ac80, 0xf4c6c679, 0xfb26cdfa]),
    ("swim", &[0x2630f099, 0x2b345680, 0x554939f7, 0x573ea0cc, 0x6f1897e1, 0x936f6879, 0xb7432786, 0xb8fed780, 0xdd9ad541, 0xe0b8b831, 0xfd73e796]),
    ("teeter", &[0x6d635275, 0x969b0749, 0xcca48518, 0xdeba3d8e]),
    ("upperbodyflinch", &[0x0e848f17, 0x3e0978b4, 0x898dc490, 0xb4e27211, 0xe2209616, 0xf2c1e28a, 0xf98cbcea]),
];
