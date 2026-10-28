use crate::weaponinfoload::{self, Element, Row, EXPLOSIONS, FIRE_TYPES, PROJECTILE_TYPES, ROW, WEAPON_TYPES};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weapon {
    pub name: String,
    pub fire: String,
    pub damage: f32,
    pub damage_fps: f32,
    pub range: f32,
    pub target_range: f32,
    pub clip: u32,
    pub ammo_max: u32,
    pub between_shots: f32,
    pub reload: f32,
    pub reload_crouch: f32,
    pub reload_fast: f32,
    pub force: f32,
    pub accuracy: f32,
    pub accuracy_fps: f32,
    pub crouch_drop: f32,
    pub aim_offset: [f32; 3],
    pub fire_rate: f32,
    pub blind_rate: f32,
    pub model: Option<String>,
    pub slot: String,
    pub damage_type: String,
    pub group: String,
    pub flags: Vec<String>,
    pub projectile: Option<Projectile>,
    pub muzzle: Option<String>,
    pub shell: Option<String>,
    pub anims: Option<String>,
    pub rumble: (f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub kind: String,
    pub fuse: f32,
    pub explosion: Option<String>,
    pub force: f32,
    pub explode_impact: f32,
    pub offset: [f32; 3],
    pub rot_offset: [f32; 3],
}

impl Weapon {
    pub fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

pub const SLOTS: [&str; 12] = ["UNARMED", "MELEE", "HANDGUN", "SHOTGUN", "SMG", "RIFLE", "SNIPER", "HEAVY", "THROWN", "SPECIAL",
    "PARACHUTE", "UNKNOWN"];
pub const DAMAGE_TYPES: [&str; 10] = ["NONE", "MELEE", "BULLET", "EXPLOSIVE", "FIRE", "COLLISION", "FALL", "DROWN", "UNKNOWN", "NONE"];
pub const GROUPS: [&str; 18] = ["PUNCH_KICK", "MELEE_WOOD", "MELEE_METAL", "MELEE_SHARP", "MELEE_GENERIC", "PISTOL_SMALL",
    "PISTOL_LARGE", "PISTOL_SILENCED", "SMG", "SHOTGUN", "RIFLE_ASSAULT", "RIFLE_SNIPER", "ROCKET", "GRENADE", "MOLOTOV", "FIRE",
    "EXPLOSION", "UNKNOWN"];
pub const FLAGS: [&str; 25] = ["CAN_AIM", "CAN_AIM_WITH_ARM", "CAN_FREE_AIM", "FIRST_PERSON", "KEEP_CAMERA_BEHIND", "GUN", "THROWN",
    "HEAVY", "SILENCED", "MELEE_CLUB", "MELEE_BLADE", "ARMOUR_PENETRATING", "2HANDED", "TREAT_AS_2HANDED_IN_COVER", "ANIM_RELOAD",
    "ANIM_CROUCH_FIRE", "CREATE_VISIBLE_ORDNANCE", "EXPLOSION_BASED_ON_IMPACT", "ADD_SMOKE_ON_EXPLOSION", "INSTANT_KILL_IN_MP",
    "HIGHER_BREAK_FORCE", "CAN_BE_USED_AS_DRIVEBY", "HEAVY_WEAPON_USES_RIFLE_ANIMS", "MISSION_CAN_FORCE_DRIVEBY_USE",
    "DONT_RUMBLE_WHEN_DOING_DRIVE_BY"];

fn lookup(table: &[&str], name: &str, none: i32) -> i32 {
    if name.is_empty() {
        return none;
    }
    table.iter().position(|t| t.eq_ignore_ascii_case(name)).map_or(none, |i| i as i32)
}

pub fn slot_index(name: &str) -> i32 { lookup(&SLOTS, name, 0) }
pub fn damage_type_index(name: &str) -> i32 { lookup(&DAMAGE_TYPES, name, crate::kv!(23 int)) }
pub fn group_index(name: &str) -> i32 { lookup(&GROUPS, name, 0) }
pub fn flag_bit(name: &str) -> i32 { lookup(&FLAGS, name, -1) }

fn text_attr<'a>(el: &'a Element, key: &str) -> Option<&'a str> {
    el.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn find<'a>(el: &'a Element, name: &str) -> Option<&'a Element> {
    el.children.iter().find_map(|c| if c.name == name { Some(c) } else { find(c, name) })
}

fn desc_attr<'a>(el: &'a Element, tag: &str, key: &str) -> Option<&'a str> {
    find(el, tag).and_then(|t| text_attr(t, key))
}

fn attributes(mut s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    while let Some(eq) = s.find('=') {
        let key = s[..eq].trim();
        let after = s[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else { break };
        let body = &after[1..];
        let Some(end) = body.find(q) else { break };
        out.push((key.to_string(), body[..end].to_string()));
        s = &body[end + 1..];
    }
    out
}

fn tree(xml: &str) -> Element {
    fn close_top(stack: &mut Vec<Element>) {
        let el = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(el);
    }
    let mut stack = vec![Element { kind: 1, ..Default::default() }];
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        let text = rest[..lt].trim();
        if !text.is_empty() {
            let top = stack.last_mut().unwrap();
            top.text = Some(match top.text.take() {
                Some(old) => format!("{old} {text}"),
                None => text.to_string(),
            });
        }
        rest = &rest[lt..];
        if rest.starts_with("<!--") {
            rest = rest.find("-->").map_or("", |i| &rest[i + crate::kv!(0 int)..]);
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        if tag.starts_with(['?', '!']) {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            let name = name.trim();
            if let Some(i) = (1..stack.len()).rev().find(|&i| stack[i].name == name) {
                while stack.len() > i {
                    close_top(&mut stack);
                }
            }
            continue;
        }
        let selfclose = tag.ends_with('/');
        let body = tag.trim_end_matches('/');
        let (name, attrs) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
        let el = Element { name: name.to_string(), kind: 1, text: None, attrs: attributes(attrs), children: Vec::new() };
        if selfclose {
            stack.last_mut().unwrap().children.push(el);
        } else {
            stack.push(el);
        }
    }
    while stack.len() > 1 {
        close_top(&mut stack);
    }
    stack.pop().unwrap()
}

fn weapons_in<'a>(el: &'a Element, out: &mut Vec<&'a Element>) {
    for c in &el.children {
        if c.name == "weapon" {
            out.push(c);
        } else {
            weapons_in(c, out);
        }
    }
}

fn word(row: &Row, at: usize) -> u32 {
    u32::from_be_bytes([row[at], row[at + 1], row[at + 2], row[at + crate::kv!(0 int)]])
}

fn half(row: &Row, at: usize) -> u16 {
    u16::from_be_bytes([row[at], row[at + 1]])
}

fn float(row: &Row, at: usize) -> f32 {
    f32::from_bits(word(row, at))
}

fn name_at(row: &Row, table: &[&str], at: usize) -> String {
    usize::try_from(word(row, at) as i32).ok().and_then(|i| table.get(i)).map_or(String::new(), |s| s.to_string())
}

pub fn read(xml: &str) -> Vec<Weapon> {
    let root = tree(xml);
    let mut ws = Vec::new();
    weapons_in(&root, &mut ws);
    ws.into_iter().filter_map(|w| {
        let name = text_attr(w, "type")?;
        let data = w.children.iter().find(|c| c.name == "data")?;
        let mut row = [0u8; ROW];
        weaponinfoload::load(data, &mut row);
        Some(from_row(w, data, name, &row))
    }).collect()
}

fn from_row(w: &Element, data: &Element, name: &str, row: &Row) -> Weapon {
    let fire = FIRE_TYPES.get(word(row, crate::kv!(23 int)) as usize).copied().unwrap_or("").to_string();
    let word20 = word(row, crate::kv!(7 int));
    let flags = FLAGS.iter().enumerate().filter(|&(bit, _)| (word20 >> bit) & 1 != 0).map(|(_, f)| f.to_string()).collect();
    let projectile = data.children.iter().find(|c| c.name == "projectile").filter(|_| fire == "PROJECTILE").map(|p| {
        let explosion = word(row, crate::kv!(420 int)) as i32;
        Projectile {
            kind: if p.children.iter().any(|c| c.name == "typetocreate") {
                name_at(row, &WEAPON_TYPES, crate::kv!(324 int))
            } else {
                name_at(row, &PROJECTILE_TYPES, crate::kv!(84 int))
            },
            fuse: word(row, crate::kv!(375 int)) as i32 as f32,
            explosion: (explosion >= 0).then(|| EXPLOSIONS.get(explosion as usize).copied().unwrap_or("").to_string()),
            force: if p.children.iter().any(|c| c.name == "physics") { float(row, crate::kv!(100 int)) } else { 0.0 },
            explode_impact: float(row, crate::kv!(365 int)),
            offset: [float(row, crate::kv!(87 int)), float(row, crate::kv!(93 int)), float(row, crate::kv!(462 int))],
            rot_offset: [float(row, crate::kv!(455 int)), float(row, crate::kv!(368 int)), float(row, crate::kv!(85 int))],
        }
    });
    Weapon {
        name: name.to_string(),
        fire,
        damage: half(row, crate::kv!(370 int)) as f32,
        damage_fps: half(row, crate::kv!(363 int)) as f32,
        range: float(row, crate::kv!(100 int)),
        target_range: float(row, crate::kv!(122 int)),
        clip: half(row, crate::kv!(330 int)) as u32,
        ammo_max: half(row, crate::kv!(322 int)) as u32,
        between_shots: word(row, crate::kv!(291 int)) as i32 as f32,
        reload: word(row, crate::kv!(299 int)) as i32 as f32,
        reload_crouch: word(row, crate::kv!(486 int)) as i32 as f32,
        reload_fast: word(row, crate::kv!(409 int)) as i32 as f32,
        force: float(row, crate::kv!(83 int)),
        accuracy: float(row, crate::kv!(380 int)),
        accuracy_fps: float(row, crate::kv!(318 int)),
        crouch_drop: float(row, crate::kv!(478 int)) - float(row, crate::kv!(315 int)),
        aim_offset: [float(row, crate::kv!(254 int)), float(row, crate::kv!(325 int)), float(row, crate::kv!(478 int))],
        fire_rate: desc_attr(w, "rates", "firerate").and_then(|v| v.parse().ok()).unwrap_or(1.0),
        blind_rate: desc_attr(w, "rates", "blindfirerate").and_then(|v| v.parse().ok()).unwrap_or(1.0),
        model: desc_attr(w, "assets", "model").map(str::to_string),
        slot: name_at(row, &SLOTS, crate::kv!(1 int)),
        damage_type: name_at(row, &DAMAGE_TYPES, crate::kv!(52 int)),
        group: name_at(row, &GROUPS, crate::kv!(51 int)),
        flags,
        projectile,
        muzzle: desc_attr(w, "muzzle", "fx").map(str::to_string),
        shell: desc_attr(w, "shell", "fx").map(str::to_string),
        anims: desc_attr(w, "anim", "group").map(str::to_string),
        rumble: (word(row, crate::kv!(432 int)) as i32 as f32, float(row, crate::kv!(371 int))),
    }
}

pub fn pad_motors(intensity: f32) -> (u8, u8) {
    let i = intensity.max(0.0).min(1.0);
    if i == 0.0 {
        (0, 0)
    } else if i < crate::kv!(73) {
        (0, (i * crate::kv!(27)).mul_add(crate::kv!(141), crate::kv!(76)) as u8)
    } else {
        (i.mul_add(crate::kv!(980), crate::kv!(26)) as u8, if i >= crate::kv!(112) { crate::kv!(301 int) } else { 0 })
    }
}

pub fn water_hit(quads: &[[[f32; 3]; 4]], from: [f32; 3], to: [f32; 3]) -> Option<[f32; 3]> {
    quads.iter().find_map(|q| {
        let h = q[0][2];
        if (from[2] - h) * (to[2] - h) > 0.0 || from[2] == to[2] {
            return None;
        }
        let t = (h - from[2]) / (to[2] - from[2]);
        let (x, y) = (from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t);
        let (lo, hi) = q.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), v| ([lo[0].min(v[0]), lo[1].min(v[1])], [hi[0].max(v[0]), hi[1].max(v[1])]));
        (x >= lo[0] && x <= hi[0] && y >= lo[1] && y <= hi[1]).then_some([x, y, h])
    })
}

