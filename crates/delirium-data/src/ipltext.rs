enum Field<'a> {
    F(&'a mut f32),
    D(&'a mut i32),
}

fn scan(line: &str, fields: &mut [Field]) -> usize {
    scan_after(line, &[], fields)
}

fn scan_after(line: &str, words: &[&str], fields: &mut [Field]) -> usize {
    let format: Vec<&str> = words.iter().copied().chain(fields.iter().map(|f| if let Field::F(_) = f { "%f" } else { "%d" })).collect();
    let s = crate::crt::sscanf(line.as_bytes(), format.join(" ").as_bytes());
    for (f, v) in fields.iter_mut().zip(s.values.iter().skip(words.len())) {
        match (f, v) {
            (Field::F(f), crate::crt::Value::Float(x)) => **f = *x,
            (Field::D(d), crate::crt::Value::Int { bits, .. }) => **d = *bits as i32,
            _ => {}
        }
    }
    s.count.max(0) as usize
}

fn short(v: f32) -> i16 {
    (v.trunc() as i64) as i16
}

#[derive(Clone, Debug, Default)]
pub struct MapZones {
    pub boxes: [Vec<([f32; 3], [f32; 3])>; 4],
}

impl MapZones {
    pub fn add(&mut self, a: [f32; 3], b: [f32; 3], area: i32) {
        let Some(list) = usize::try_from(area).ok().and_then(|k| self.boxes.get_mut(k)) else { return };
        if list.len() >= 10 {
            return;
        }
        list.push(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]));
    }
    pub fn in_area(&self, area: usize, p: [f32; 2]) -> bool {
        self.boxes.get(area).is_some_and(|l| l.iter().any(|(lo, hi)| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1]))
    }
    pub fn area_of(&self, p: [f32; 2]) -> i32 {
        (0..4).find(|&k| self.in_area(k, p)).map_or(5, |k| k as i32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occluder {
    pub pos: [i16; 3],
    pub radius: i16,
    pub length: i16,
    pub width: i16,
    pub height: i16,
    pub cos: i16,
    pub sin: i16,
    pub flag: bool,
}

#[derive(Clone, Debug)]
pub struct OccluderLists {
    pub next: Vec<i32>,
    pub far: i32,
    pub near: i32,
    cursor: i32,
    cursor_prev: i32,
}

pub const OCCLUDER_NEAR: f32 = 90000.0;

impl OccluderLists {
    pub fn new(count: usize) -> OccluderLists {
        OccluderLists { next: (0..count as i32).map(|i| i - 1).collect(), far: count as i32 - 1, near: -1, cursor: -1, cursor_prev: -1 }
    }

    fn key(o: &Occluder, cam: [f32; 3]) -> f32 {
        let (dx, dy) = (cam[0] - o.pos[0] as f32 / 4.0, cam[1] - o.pos[1] as f32 / 4.0);
        dx * dx + dy * dy - o.radius as f32 / 4.0
    }

    pub fn update(&mut self, occluders: &[Occluder], cam: [f32; 3], frame: u32) {
        let (mut cur, mut prev) = (self.cursor, self.cursor_prev);
        if cur == -1 {
            cur = self.far;
            prev = -1;
        }
        if cur != -1 {
            let mut walked = 0;
            loop {
                let next = self.next[cur as usize];
                if Self::key(&occluders[cur as usize], cam) >= OCCLUDER_NEAR {
                    prev = cur;
                } else {
                    if prev == -1 {
                        self.far = next;
                    } else {
                        self.next[prev as usize] = next;
                    }
                    self.next[cur as usize] = self.near;
                    self.near = cur;
                }
                walked += 1;
                cur = next;
                if walked >= 100 || next == -1 {
                    break;
                }
            }
            self.cursor = cur;
            self.cursor_prev = prev;
        }
        let mut near: Vec<(f32, i32)> = vec![];
        let (mut cur, mut prev) = (self.near, -1);
        while cur != -1 {
            let next = self.next[cur as usize];
            let key = Self::key(&occluders[cur as usize], cam);
            if key > OCCLUDER_NEAR {
                if prev == -1 {
                    self.near = next;
                } else {
                    self.next[prev as usize] = next;
                }
                self.next[cur as usize] = self.far;
                self.far = cur;
            } else {
                near.push((key, cur));
                prev = cur;
            }
            cur = next;
        }
        if frame & 7 == 2 && near.len() > 64 {
            let mid = near.len().min(100);
            partial_sort(&mut near, mid);
            self.near = near[0].1;
            for w in near.windows(2) {
                self.next[w[0].1 as usize] = w[1].1;
            }
            self.next[near[near.len() - 1].1 as usize] = -1;
        }
    }

    pub fn near_list(&self) -> Vec<i32> {
        let mut out = vec![];
        let mut cur = self.near;
        while cur != -1 && out.len() <= self.next.len() {
            out.push(cur);
            cur = self.next[cur as usize];
        }
        out
    }
}

fn push_heap(a: &mut [(f32, i32)], mut hole: usize, top: usize, val: (f32, i32)) {
    while top < hole {
        let parent = (hole - 1) / 2;
        if !(a[parent].0 < val.0) {
            break;
        }
        a[hole] = a[parent];
        hole = parent;
    }
    a[hole] = val;
}

fn adjust_heap(a: &mut [(f32, i32)], top: usize, bottom: usize, val: (f32, i32)) {
    let mut hole = top;
    let mut child = 2 * hole + 2;
    while child < bottom {
        if a[child].0 < a[child - 1].0 {
            child -= 1;
        }
        a[hole] = a[child];
        hole = child;
        child = 2 * child + 2;
    }
    if child == bottom {
        a[hole] = a[bottom - 1];
        hole = bottom - 1;
    }
    push_heap(a, hole, top, val);
}

pub fn partial_sort(a: &mut [(f32, i32)], mid: usize) {
    if mid > 1 {
        let mut hole = (mid - 2) / 2;
        loop {
            let v = a[hole];
            adjust_heap(a, hole, mid, v);
            if hole == 0 {
                break;
            }
            hole -= 1;
        }
    }
    for i in mid..a.len() {
        if a[i].0 < a[0].0 {
            let v = a[i];
            a[i] = a[0];
            adjust_heap(a, 0, mid, v);
        }
    }
    let mut end = mid;
    while end > 1 {
        let v = a[end - 1];
        a[end - 1] = a[0];
        end -= 1;
        adjust_heap(a, 0, end, v);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CullZone {
    pub corner: [i16; 2],
    pub axis_a: [i16; 2],
    pub axis_b: [i16; 2],
    pub z_low: i16,
    pub z_high: i16,
    pub flags: u16,
}

impl CullZone {
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let c = |i: usize| self.corner[i] as i32;
        let (a, b) = (|i: usize| self.axis_a[i] as i32, |i: usize| self.axis_b[i] as i32);
        let lo = |i: usize| (c(i) + b(i).min(0) + a(i).min(0)) as f32;
        let hi = |i: usize| (c(i) + b(i).max(0) + a(i).max(0)) as f32;
        ([lo(0), lo(1), self.z_low as f32], [hi(0), hi(1), self.z_high as f32])
    }
    pub fn contains(&self, p: [f32; 3]) -> bool {
        if !(p[2] > self.z_low as f32 && p[2] < self.z_high as f32) {
            return false;
        }
        let (dx, dy) = (p[0] - self.corner[0] as f32, p[1] - self.corner[1] as f32);
        let [ax, ay] = self.axis_a.map(|v| v as i32);
        let ta = ay as f32 * dy + ax as f32 * dx;
        if ta < 0.0 || ta > (ax * ax + ay * ay) as f32 {
            return false;
        }
        let [bx, by] = self.axis_b.map(|v| v as i32);
        let tb = bx as f32 * dx + by as f32 * dy;
        tb >= 0.0 && tb <= (bx * bx + by * by) as f32
    }
}

#[derive(Clone, Debug, Default)]
pub struct CullState {
    pub near: Vec<usize>,
    pub built_at: Option<[f32; 3]>,
    pub player_flags: u16,
    pub camera_flags: u16,
}

impl CullState {
    pub fn update(&mut self, zones: &[CullZone], player: [f32; 3], camera: [f32; 3], frame: u32) {
        let moved = self.built_at.is_none_or(|b| {
            let d = [player[0] - b[0], player[1] - b[1], player[2] - b[2]];
            d[0] * d[0] + d[2] * d[2] + d[1] * d[1] > 100.0
        });
        if moved {
            self.built_at = Some(player);
            self.near.clear();
            for (i, z) in zones.iter().enumerate() {
                let (lo, hi) = z.bounds();
                if (0..3).all(|k| lo[k] - 30.0 < player[k] && player[k] < hi[k] + 30.0) && !self.near.contains(&i) {
                    self.near.push(i);
                }
            }
        }
        let flags_at = |p: [f32; 3]| self.near.iter().filter(|&&i| zones[i].contains(p)).fold(0u16, |f, &i| f | zones[i].flags);
        match frame & 7 {
            2 => self.camera_flags = flags_at(camera),
            6 => self.player_flags = flags_at(player),
            _ => {}
        }
    }

    pub fn no_heli_searchlight(&self) -> bool {
        self.player_flags & 0x08 != 0
    }

    pub fn cars_stop_following(&self) -> bool {
        self.player_flags & 0x10 != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttributeZone {
    pub corner: [i16; 2],
    pub axis_a: [i16; 2],
    pub axis_b: [i16; 2],
    pub z_low: i16,
    pub z_high: i16,
    pub value: f32,
    pub bytes: [u8; 3],
    pub flags: u8,
}

#[derive(Clone, Debug, Default)]
pub struct TextIpls {
    pub map_zones: MapZones,
    pub occluders: Vec<Occluder>,
    pub flat_occluders: u32,
    pub cull_zones: Vec<CullZone>,
    pub attribute_zones: Vec<AttributeZone>,
    pub path_nodes: u32,
}

pub const MAX_OCCLUDERS: usize = 4000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IplLine {
    Inst,
    Zone,
    Mzon,
    Cull,
    Occl,
    Grge,
    Enex,
    Pick,
    Cars,
    Tcyc,
    Vnod,
    Link,
    MloPlus,
    Effect,
    Lodm,
    Slow,
}

pub fn ipl_lines(file: &[u8]) -> Vec<(IplLine, Vec<u8>)> {
    let mut state = IplState::default();
    let mut out = vec![];
    for l in crate::textlines::lines(file, true) {
        if let Some(kind) = state.line(&l) {
            out.push((kind, l));
        }
    }
    out
}

#[derive(Default)]
struct IplState {
    section: Option<Option<IplLine>>,
}

impl IplState {
    fn line(&mut self, memory: &[u8]) -> Option<IplLine> {
        use IplLine::*;
        let l = &memory[..memory.iter().position(|&c| c == 0).unwrap_or(memory.len())];
        let &c = l.first()?;
        if c == b'#' {
            return None;
        }
        let Some(section) = self.section else {
            let opens: [(&[u8], Option<IplLine>); 20] = [
                (b"inst", Some(Inst)), (b"mult", None), (b"zone", Some(Zone)), (b"mzon", Some(Mzon)), (b"cull", Some(Cull)),
                (b"vnod", Some(Vnod)), (b"link", Some(Link)), (b"occl", Some(Occl)), (b"grge", Some(Grge)),
                (b"enex", Some(Enex)), (b"pick", Some(Pick)), (b"cars", Some(Cars)), (b"jump", None), (b"tcyc", Some(Tcyc)),
                (b"auzo", None), (b"blok", None), (b"mlo+", Some(MloPlus)), (b"2dfx", Some(Effect)), (b"lodm", Some(Lodm)),
                (b"slow", Some(Slow)),
            ];
            self.section = opens.iter().find(|(k, _)| l.starts_with(k)).map(|&(_, s)| s);
            return None;
        };
        if l.starts_with(b"end") {
            self.section = None;
            return None;
        }
        section
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum IplCall {
    Instance { name: Vec<u8>, record: [u32; 12] },
    Zone { name: Vec<u8>, corners: [f32; 6], label: Vec<u8> },
    Garage { floats: [f32; 8], a: u32, name: Vec<u8>, b: u32 },
    CarGenerator([u32; 14]),
    TimeCycleBox { min: [f32; 3], max: [f32; 3], hash: u32, a: f32, b: f32, c: u32, d: u32 },
    LodGroup { min: [f32; 3], max: [f32; 3], instances: Vec<u32>, byte: u32 },
    SlowZone([f32; 6]),
    MloInstance { instance: u32, record: [u32; 12], d: u32 },
}

pub fn ipl_calls(file: &[u8]) -> Vec<IplCall> {
    use crate::crt::sscanf;
    let mut out = vec![];
    let mut state = IplState::default();
    crate::textlines::each_line(file, true, |memory| {
        let Some(kind) = state.line(memory) else { return };
        let text = |s: &crate::crt::Scanned, k: usize| s.bytes(k).map_or(vec![], |b| b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec());
        let int = |s: &crate::crt::Scanned, k: usize| s.int(k).map_or(0, |v| v as u32);
        let f = |s: &crate::crt::Scanned, k: usize| s.float(k).unwrap_or(0.0);
        let b = |s: &crate::crt::Scanned, k: usize| f(s, k).to_bits();
        match kind {
            IplLine::Inst => {
                let s = sscanf(memory, b"%s %d %f %f %f %f %f %f %f %d %d %f");
                if s.count != 12 && s.count != 11 {
                    return;
                }
                let name = text(&s, 0);
                let last = if s.count == 12 { b(&s, 11) } else { (-1.0f32).to_bits() };
                let record = [b(&s, 2), b(&s, 3), b(&s, 4), b(&s, 5), b(&s, 6), b(&s, 7), b(&s, 8), crate::map::hash_bytes(&name, 0),
                              int(&s, 1), int(&s, 9), s.int(10).map_or(u32::MAX, |v| v as u32), last];
                out.push(IplCall::Instance { name, record });
            }
            IplLine::Zone => {
                let s = sscanf(memory, b"%s %d %f %f %f %f %f %f %d %s");
                if s.count == 10 {
                    out.push(IplCall::Zone { name: text(&s, 0), corners: std::array::from_fn(|k| f(&s, 2 + k)), label: text(&s, 9) });
                }
            }
            IplLine::Grge => {
                let s = sscanf(memory, b"%f %f %f %f %f %f %f %f %d %d %s");
                if s.count == 11 {
                    out.push(IplCall::Garage { floats: std::array::from_fn(|k| f(&s, k)), a: int(&s, 9), name: text(&s, 10), b: int(&s, 8) });
                }
            }
            IplLine::Cars => {
                let s = sscanf(memory, b"%f %f %f %f %f %f %d %d %d %d %d %d %d %d");
                if s.count == 14 {
                    out.push(IplCall::CarGenerator(std::array::from_fn(|k| if k < 6 { b(&s, k) } else { int(&s, k) })));
                }
            }
            IplLine::Tcyc => {
                let s = sscanf(memory, b"%f %f %f %f %f %f %f %f %d %d %s");
                out.push(IplCall::TimeCycleBox {
                    min: [f(&s, 0), f(&s, 1), f(&s, 2)],
                    max: [f(&s, 3), f(&s, 4), f(&s, 5)],
                    hash: crate::map::hash_bytes(&text(&s, 10), 0),
                    a: f(&s, 6),
                    b: f(&s, 7),
                    c: int(&s, 8),
                    d: int(&s, 9),
                });
            }
            IplLine::Lodm => {
                let s = sscanf(memory, b"%f %f %f %f %f %f %d %d %d %d %d %d %d %d %d %d %d");
                if s.count < 7 {
                    return;
                }
                let head = int(&s, 6);
                let n = ((head & 0xF) as usize).min(10);
                out.push(IplCall::LodGroup {
                    min: [f(&s, 0), f(&s, 1), f(&s, 2)],
                    max: [f(&s, 3), f(&s, 4), f(&s, 5)],
                    instances: (0..n).map(|k| int(&s, 7 + k)).collect(),
                    byte: head >> 24,
                });
            }
            IplLine::Slow => {
                let s = sscanf(memory, b"%f %f %f %f %f %f");
                out.push(IplCall::SlowZone(std::array::from_fn(|k| f(&s, k))));
            }
            IplLine::MloPlus => {
                let s = sscanf(memory, b"%s %d %d %d %f %f %f %f %f %f %f %d %d");
                let record = [b(&s, 4), b(&s, 5), b(&s, 6), b(&s, 7), b(&s, 8), b(&s, 9), b(&s, 10),
                              crate::map::hash_bytes(&text(&s, 0), 0), int(&s, 1), int(&s, 11),
                              s.int(12).map_or(u32::MAX, |v| v as u32), (-1.0f32).to_bits()];
                out.push(IplCall::MloInstance { instance: int(&s, 2), record, d: int(&s, 3) });
            }
            _ => {}
        }
    });
    out
}

impl TextIpls {
    pub fn read(&mut self, text: &str) {
        for (kind, line) in ipl_lines(text.as_bytes()) {
            let line = String::from_utf8_lossy(&line);
            match kind {
                IplLine::Mzon => self.mzon(&line),
                IplLine::Occl => self.occl(&line),
                IplLine::Cull => self.cull(&line),
                IplLine::Vnod => self.path_nodes += 1,
                _ => {}
            }
        }
    }

    fn mzon(&mut self, l: &str) {
        let (mut kind, mut area) = (0, 0);
        let mut v = [0f32; 6];
        let [a, b, c, d, e, f] = &mut v;
        let n = scan_after(l, &["%s"], &mut [Field::D(&mut kind), Field::F(a), Field::F(b), Field::F(c), Field::F(d), Field::F(e), Field::F(f), Field::D(&mut area)]);
        if n == 9 {
            self.map_zones.add([v[0], v[1], v[2]], [v[3], v[4], v[5]], area);
        }
    }

    fn occl(&mut self, l: &str) {
        let mut v = [0f32; 9];
        let mut flag = 0;
        {
            let [a, b, c, d, e, f, g, h, i] = &mut v;
            scan(l, &mut [Field::F(a), Field::F(b), Field::F(c), Field::F(d), Field::F(e), Field::F(f), Field::F(g), Field::F(h), Field::F(i), Field::D(&mut flag)]);
        }
        let [x, y, z, width, length, height, rot, _, _] = v;
        self.add_occluder([x, y, z + height * 0.5], width, length, height, rot, flag != 0);
    }

    pub fn add_occluder(&mut self, pos: [f32; 3], width: f32, length: f32, height: f32, rot_degrees: f32, flag: bool) {
        let (l, w, h) = (length.abs().trunc(), width.abs().trunc(), height.abs().trunc());
        let zeros = [l, w, h].iter().filter(|v| **v == 0.0).count();
        if zeros > 1 {
            return;
        }
        if zeros == 1 {
            self.flat_occluders += 1;
        }
        if self.occluders.len() >= MAX_OCCLUDERS {
            return;
        }
        let at_least_one = |v: f32| short(v * 4.0).max(1);
        let angle = rot_degrees * 0.017_453_292;
        self.occluders.push(Occluder {
            pos: [short(pos[0] * 4.0), short(pos[1] * 4.0), short(pos[2] * 4.0)],
            radius: at_least_one((w * w + l * l + h * h).sqrt()),
            length: at_least_one(l),
            width: at_least_one(w),
            height: at_least_one(h),
            cos: short((angle as f64).cos() as f32 * 16384.0),
            sin: short((angle as f64).sin() as f32 * 16384.0),
            flag,
        });
    }

    fn cull(&mut self, l: &str) {
        let mut v = [0f32; 13];
        let mut flags = 0;
        let n = {
            let [c0, c1, c2, a0, a1, zl, b0, b1, zh, x0, x1, x2, x3] = &mut v;
            scan(l, &mut [Field::F(c0), Field::F(c1), Field::F(c2), Field::F(a0), Field::F(a1), Field::F(zl), Field::F(b0), Field::F(b1),
                          Field::F(zh), Field::D(&mut flags), Field::F(x0), Field::F(x1), Field::F(x2), Field::F(x3)])
        };
        let [cx, cy, _, ax, ay, zl, bx, by, zh, f10, f11, f12, f13] = v;
        let corner = [short(cx - ax - bx), short(cy - ay - by)];
        let (axis_a, axis_b) = ([short(ax * 2.0), short(ay * 2.0)], [short(bx * 2.0), short(by * 2.0)]);
        if n == 14 {
            let byte = |v: f32| (v * 100.0).trunc() as i64 as u8;
            self.attribute_zones.push(AttributeZone {
                corner, axis_a, axis_b, z_low: short(zl), z_high: short(zh), value: f13,
                bytes: [byte(f10), byte(f11), byte(f12)], flags: flags as u8,
            });
            return;
        }
        let flags = (flags as u32 & 0xFFFF) as u16 & 0xF77F;
        if flags != 0 {
            self.cull_zones.push(CullZone { corner, axis_a, axis_b, z_low: short(zl), z_high: short(zh), flags });
        }
    }
}

