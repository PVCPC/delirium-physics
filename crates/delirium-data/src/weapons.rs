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
pub fn damage_type_index(name: &str) -> i32 { lookup(&DAMAGE_TYPES, name, 8) }
pub fn group_index(name: &str) -> i32 { lookup(&GROUPS, name, 0) }
pub fn flag_bit(name: &str) -> i32 { lookup(&FLAGS, name, -1) }

fn attr<'a>(block: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let at = block.find(&key)? + key.len();
    block[at..].split('"').next()
}

pub fn read(xml: &str) -> Vec<Weapon> {
    xml.split("<weapon ").skip(1).filter_map(|b| {
        let b = &b[..b.find("</weapon>").unwrap_or(b.len())];
        let b = &format!(" {b}");
        let f = |n: &str| attr(b, n).and_then(|v| v.parse::<f32>().ok());
        let after = |tag: &str| b.find(tag).map(|i| format!(" {}", &b[i + tag.len()..]));
        let projectile = after("<projectile").map(|p| Projectile {
            kind: after("<typetocreate").as_deref().or(Some(&p)).and_then(|t| attr(t, "type")).unwrap_or("NONE").to_string(),
            fuse: attr(&p, "fusetime").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            explosion: after("<explosion").and_then(|e| attr(&e, "type").map(str::to_string)),
            force: attr(&p, "force").and_then(|v| v.parse().ok()).unwrap_or(0.0),
            explode_impact: attr(&p, "explodeimpactthreshold").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        });
        Some(Weapon {
            name: attr(b, "type")?.to_string(),
            fire: attr(b, "firetype")?.to_string(),
            damage: f("base").unwrap_or(0.0),
            damage_fps: f("fps").or(f("base")).unwrap_or(0.0),
            range: f("weaponrange").unwrap_or(0.0),
            target_range: f("targetrange").unwrap_or(0.0),
            clip: f("clipsize").unwrap_or(0.0) as u32,
            ammo_max: f("ammomax").unwrap_or(0.0) as u32,
            between_shots: f("timebetweenshots").unwrap_or(0.0),
            reload: f("time").unwrap_or(0.0),
            reload_crouch: f("crouchtime").or(f("time")).unwrap_or(0.0),
            reload_fast: f("fasttime").or(f("time")).unwrap_or(0.0),
            force: f("force").unwrap_or(0.0),
            accuracy: f("accuracy").unwrap_or(1.0),
            accuracy_fps: f("accuracyfps").or(f("accuracy")).unwrap_or(1.0),
            crouch_drop: {
                let z = |t: &str| after(t).and_then(|o| attr(&o, "z").and_then(|v| v.parse::<f32>().ok()));
                z("<offset").zip(z("<crouchedoffset")).map_or(0.0, |(a, b)| a - b)
            },
            aim_offset: after("<offset").map_or([0.0; 3], |o| ["x", "y", "z"].map(|k| attr(&o, k).and_then(|v| v.parse().ok()).unwrap_or(0.0))),
            fire_rate: f("firerate").unwrap_or(1.0),
            blind_rate: f("blindfirerate").unwrap_or(1.0),
            model: attr(b, "model").map(str::to_string),
            slot: attr(b, "slot").unwrap_or("").to_string(),
            damage_type: attr(b, "damagetype").unwrap_or("").to_string(),
            group: attr(b, "group").unwrap_or("").to_string(),
            flags: b.split("<flag>").skip(1).filter_map(|f| f.split("</flag>").next()).map(str::to_string).collect(),
            projectile,
            muzzle: after("<muzzle").and_then(|m| attr(&m, "fx").map(str::to_string)),
            shell: after("<shell").and_then(|m| attr(&m, "fx").map(str::to_string)),
            anims: after("<anim ").and_then(|a| attr(&a, "group").map(str::to_string)),
            rumble: after("<rumble").map_or((0.0, 0.0), |r| {
                let g = |n: &str| attr(&r, n).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                (g("duration"), g("intensity"))
            }),
        })
    }).collect()
}

pub fn pad_motors(intensity: f32) -> (u8, u8) {
    let i = intensity.max(0.0).min(1.0);
    if i == 0.0 {
        (0, 0)
    } else if i < 0.1 {
        (0, (i * 10.0).mul_add(70.0, 50.0) as u8)
    } else {
        (i.mul_add(185.0, 15.0) as u8, if i >= 0.9 { 120 } else { 0 })
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

