use crate::getup::{self, Blender, ClipDef, ClipInfo, Ped, SimpleGetUp};
use crate::mathrt;
use crate::melee::WEAPON_TYPES;
use crate::rng::{self, Rand};
use crate::tasks::StandStill;
use crate::weapons::Weapon;

pub mod kind {
    pub const GET_UP: u32 = 205;
    pub const GET_UP_AND_STAND_STILL: u32 = 206;
    pub const FALL: u32 = 207;
    pub const FALL_AND_GET_UP: u32 = 208;
}

pub const DAMAGE_KO: i32 = 0x2C;
pub const CLIMB: i32 = 9;
pub const PRONE: i32 = 10;
pub const DAMAGE_REACT: i32 = 0x2D;
pub const DAMAGE_REACT_FLOOR: i32 = 0x2E;
pub mod id {
    pub const KO_FRONT: i32 = 0xBA;
    pub const KO_BACK: i32 = 0xBB;
    pub const KO_LEFT: i32 = 0xBC;
    pub const KO_RIGHT: i32 = 0xBD;
    pub const KO_COLLAPSE: i32 = 0xBE;
    pub const REACT_FRONT: i32 = 0xC1;
    pub const REACT_BACK: i32 = 0xC2;
    pub const REACT_LEFT: i32 = 0xC3;
    pub const REACT_RIGHT: i32 = 0xC4;
    pub const REACT_FLOOR_FRONT: i32 = 0xBF;
    pub const REACT_FLOOR_BACK: i32 = 0xC0;
    pub const FALL_BACK: i32 = 0x80;
    pub const FALL_FRONT: i32 = 0x81;
    pub const FLOOR_FRONT: i32 = 0x8E;
    pub const FLOOR_BACK: i32 = 0x8F;
}

pub const DEFAULT_CLIP: (i32, i32) = (DAMAGE_KO, id::KO_FRONT);

pub const CLIPS: [ClipDef; 11] = [
    ClipDef { group: DAMAGE_KO, id: id::KO_FRONT, dict: "dam_ko", clip: "ko_front", flags: 0x0100_0600 },
    ClipDef { group: DAMAGE_KO, id: id::KO_BACK, dict: "dam_ko", clip: "ko_back", flags: 0x0100_0600 },
    ClipDef { group: DAMAGE_KO, id: id::KO_LEFT, dict: "dam_ko", clip: "ko_left", flags: 0x0100_0600 },
    ClipDef { group: DAMAGE_KO, id: id::KO_RIGHT, dict: "dam_ko", clip: "ko_right", flags: 0x0100_0600 },
    ClipDef { group: CLIMB, id: id::FALL_BACK, dict: "climb_std", clip: "fall_back", flags: 0x4020 },
    ClipDef { group: CLIMB, id: id::FALL_FRONT, dict: "climb_std", clip: "fall_front", flags: 0x4020 },
    ClipDef { group: PRONE, id: id::FLOOR_FRONT, dict: "dam_rec_civi", clip: "floor_front", flags: 0x4000 },
    ClipDef { group: PRONE, id: id::FLOOR_BACK, dict: "dam_rec_civi", clip: "floor_back", flags: 0x4000 },
    ClipDef { group: DAMAGE_KO, id: id::KO_COLLAPSE, dict: "dam_ko", clip: "ko_collapse", flags: 0x4100_0600 },
    ClipDef { group: DAMAGE_REACT_FLOOR, id: id::REACT_FLOOR_FRONT, dict: "dam_rec_civi", clip: "floor_front", flags: 0x4000_9000 },
    ClipDef { group: DAMAGE_REACT_FLOOR, id: id::REACT_FLOOR_BACK, dict: "dam_rec_civi", clip: "floor_back", flags: 0x4000_8000 },
];

pub fn clip_def(group: i32, id: i32) -> Option<&'static ClipDef> {
    CLIPS.iter().find(|c| c.group == group && c.id == id)
}

pub fn ko_id(side: u32) -> i32 {
    match side {
        1 => id::KO_LEFT,
        2 => id::KO_BACK,
        3 => id::KO_RIGHT,
        _ => id::KO_FRONT,
    }
}

const QUARTER_PI: f32 = f32::from_bits(0x3F49_0FDB);
const PI: f32 = f32::from_bits(0x4049_0FDB);
const TWO_PI: f32 = f32::from_bits(0x40C9_0FDB);
const TWO_OVER_PI: f32 = f32::from_bits(0x3F22_F983);

#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn wrap_angle(x: f32) -> f32 {
    let mut x = if !(x >= -25.0) { -25.0 } else if x > 25.0 { 25.0 } else { x };
    while x > PI {
        x -= TWO_PI;
    }
    while x < -PI {
        x += TWO_PI;
    }
    x
}

pub fn bearing(d: [f32; 2], heading: f32) -> f32 {
    let toward = mathrt::atan2(-d[0] as f64, d[1] as f64) as f32;
    wrap_angle(toward - heading)
}

pub fn side(angle: f32) -> u32 {
    let mut a = angle + QUARTER_PI;
    while a < 0.0 {
        a += TWO_PI;
    }
    let q = (a * TWO_OVER_PI) as i32;
    (if q > 3 { q - ((((q - 4) as u32) >> 2) as i32 + 1) * 4 } else { q }) as u32
}

pub fn car_side(velocity: [f32; 2], heading: f32) -> u32 {
    let (x, y) = (-velocity[0], -velocity[1]);
    side(mathrt::atan2(-x as f64, y as f64) as f32 - heading)
}

pub fn car_hit_fall(side: u32) -> FallAndGetUp {
    FallAndGetUp::new(DAMAGE_KO, id::KO_FRONT + side as i32, CAR_HIT_MS)
}

pub const CAR_HIT_MS: u32 = 2000;

pub const SLOT_SHOTGUN: u32 = 3;
const SLOTS: [&str; 10] = ["UNARMED", "MELEE", "HANDGUN", "SHOTGUN", "SMG", "RIFLE", "SNIPER", "HEAVY", "THROWN", "SPECIAL"];

pub fn slot(w: &Weapon) -> u32 {
    SLOTS.iter().position(|s| s.eq_ignore_ascii_case(&w.slot)).map_or(0, |i| i as u32)
}

pub fn weapon_type(w: &Weapon) -> u32 {
    WEAPON_TYPES.iter().position(|n| n.eq_ignore_ascii_case(&w.name)).map_or(u32::MAX, |i| i as u32)
}
pub const EXPLOSION: u32 = 51;
pub const FALL_DAMAGE: u32 = 54;
pub const TORSO: [u16; 9] = [0x1A1, 0x4B2, 0x4B3, 0x36A0, 0x36A1, 0x4B4, 0x4B5, 0x4C0, 0x4C7];

#[derive(Clone, Copy, Debug, Default)]
pub struct Hit {
    pub slot: u32,
    pub weapon: u32,
    pub bone: u16,
    pub flag_29: bool,
    pub flag_24: bool,
    pub crouched: bool,
    pub dying: bool,
    pub by_object: bool,
}

pub fn knocks_down(h: &Hit) -> bool {
    let mut flag = if h.slot == SLOT_SHOTGUN && !h.flag_29 { TORSO.contains(&h.bone) } else { h.weapon == EXPLOSION };
    if h.flag_24 || h.crouched || h.dying {
        flag = false;
    }
    flag || (h.weapon == FALL_DAMAGE && h.by_object)
}

pub fn quadrant(angle: f32) -> i32 {
    let mut a = angle + QUARTER_PI;
    if a < 0.0 {
        a += TWO_PI;
    }
    (a * TWO_OVER_PI) as i32
}

pub fn react_id(quarter: i32) -> i32 {
    match quarter {
        1 => id::REACT_LEFT,
        2 => id::REACT_BACK,
        3 => id::REACT_RIGHT,
        _ => id::REACT_FRONT,
    }
}

pub const COOLDOWN_MS: u32 = 500;
pub const PLAYER_COOLDOWN_BELOW: u32 = 47;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pick {
    pub hit: Hit,
    pub angle: f32,
    pub now: u32,
    pub cooldown: u32,
    pub gun: bool,
    pub player: bool,
    pub in_vehicle: bool,
    pub head_z: f32,
    pub ped_z: f32,
    pub task: Option<u32>,
    pub lies: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Picked {
    pub group: Option<i32>,
    pub id: Option<i32>,
    pub react: Option<bool>,
    pub fall: bool,
    pub cooldown: Option<u32>,
}

pub fn pick(p: &Pick) -> Picked {
    let mut out = Picked::default();
    if p.now < p.cooldown && (p.gun || p.player && p.hit.weapon < PLAYER_COOLDOWN_BELOW) {
        return out;
    }
    let quarter = quadrant(p.angle);
    let flinch = |out: &mut Picked| {
        out.group = Some(DAMAGE_REACT);
        out.id = Some(react_id(quarter));
        out.react = Some(false);
        out.cooldown = p.player.then_some(p.now.wrapping_add(COOLDOWN_MS));
    };
    if p.in_vehicle {
        flinch(&mut out);
        return out;
    }
    if p.head_z < p.ped_z && !p.player && matches!(p.task, Some(kind::FALL) | Some(kind::GET_UP)) {
        out.group = Some(DAMAGE_REACT_FLOOR);
        out.id = Some(if p.lies == 2 { id::REACT_FLOOR_BACK } else { id::REACT_FLOOR_FRONT });
        out.fall = true;
        return out;
    }
    if knocks_down(&p.hit) {
        out.fall = true;
        out.group = Some(DAMAGE_KO);
        out.id = match quarter {
            0 => Some(id::KO_FRONT),
            1 => Some(id::KO_LEFT),
            2 => Some(id::KO_BACK),
            3 => Some(id::KO_RIGHT),
            _ => None,
        };
        return out;
    }
    flinch(&mut out);
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Thrown {
    Nm { fast: bool },
    Fall(FallAndGetUp),
    Nothing,
}

pub const THROWN_FAST: f32 = 56.25;
pub const THROWN_SIDE: u32 = 2;
pub const THROWN_FALL_MS: u32 = 1500;

pub fn thrown_out(gate: bool, speed2: f32, health: f32) -> Thrown {
    if gate {
        Thrown::Nm { fast: speed2 > THROWN_FAST }
    } else if health < 100.0 {
        Thrown::Nothing
    } else {
        Thrown::Fall(FallAndGetUp::from_side(THROWN_SIDE, THROWN_FALL_MS))
    }
}

pub fn lean(x_z: f32, y_z: f32) -> u8 {
    if x_z.abs() > y_z.abs() {
        if x_z < 0.0 { 1 } else { 3 }
    } else if y_z < 0.0 {
        0
    } else {
        2
    }
}

pub fn lies(x_z: f32, y_z: f32, spine_z: f32, ped_z: f32) -> u8 {
    if y_z.abs() > 0.6 || (spine_z - ped_z).abs() >= 0.35 {
        lean(x_z, y_z)
    } else {
        4
    }
}

#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn lies_on_ragdoll(x_z: f32, y_z: f32, spine_z: f32, feet_z: [f32; 2]) -> u8 {
    if y_z.abs() > 0.6 {
        return lean(x_z, y_z);
    }
    let low = spine_z - 0.5;
    if feet_z[0] < low || !(feet_z[1] >= low) {
        4
    } else {
        lean(x_z, y_z)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Event207 {
    pub kind: i32,
    pub priority: i32,
    pub ends_tasks: bool,
}

const DAMAGE: i32 = 9;
const IN_WATER: i32 = 66;
const STUCK_IN_AIR: i32 = 93;

#[derive(Clone, Debug)]
pub struct SimpleFall {
    pub state: u8,
    pub group: i32,
    pub id: i32,
    pub ms: u32,
    pub left: u32,
    pub blender: Blender,
}

impl SimpleFall {
    pub fn new(group: i32, id: i32, ms: u32) -> SimpleFall {
        let (group, id) = if id == -1 || group == -1 { DEFAULT_CLIP } else { (group, id) };
        SimpleFall { state: 0, group, id, ms, left: ms, blender: Blender::new(0.0) }
    }

    pub fn finished(&self) -> bool {
        self.state == 4
    }

    fn release(&mut self, rate: f32) {
        if let Some(p) = self.blender.player_mut() {
            p.player.set_blend(rate);
            p.mine = false;
        }
    }

    fn start(&mut self, ped: &dyn Ped, group: i32, id: i32, speed: f32) -> bool {
        self.release(-speed);
        self.blender.play(ped, speed, group, id)
    }

    pub fn process(&mut self, ped: &mut dyn Ped, rand: &mut Rand) -> bool {
        match self.state {
            0 => {
                if ped.dying() {
                    self.state = 4;
                    return false;
                }
                let t = self.left as f32;
                self.left = rng::float_from_draw(rand.next(), t * f32::from_bits(0x3F66_6666), t * f32::from_bits(0x3F8C_CCCD)) as i64 as u32;
                if self.left > 500 && ped.waits_short() {
                    self.left = 500;
                }
                if !self.start(ped, self.group, self.id, 8.0) {
                    self.state = 4;
                    return false;
                }
                if let Some(p) = self.blender.player_mut().filter(|_| ped.rammed()) {
                    p.player.rate = 2.0;
                }
                self.state = 1;
            }
            1 => {
                let Some(phase) = self.blender.player().map(|p| p.player.phase) else {
                    self.state = 3;
                    return false;
                };
                if phase >= 1.0 {
                    self.state = 3;
                } else if ped.thrown() && !ped.on_ground() && phase > 0.8 {
                    let id = if matches!(ped.lies(), 2 | 4..) { id::FALL_BACK } else { id::FALL_FRONT };
                    self.start(ped, CLIMB, id, 2.0);
                    self.state = 2;
                }
            }
            2 => {
                if ped.on_ground() {
                    ped.land();
                    let id = if matches!(ped.lies(), 2 | 4..) { id::FLOOR_BACK } else { id::FLOOR_FRONT };
                    self.start(ped, PRONE, id, 16.0);
                    self.state = 3;
                }
            }
            3 => {
                if !(ped.on_ground() || ped.in_water()) {
                    return false;
                }
                let dt = (ped.dt() * 1000.0) as i64 as u32;
                if self.left <= dt {
                    self.left = 0;
                    self.state = 4;
                    return false;
                }
                if ped.rammed() {
                    if ped.is_player() {
                        ped.crush(ped.dt());
                    }
                    return false;
                }
                self.left -= dt;
            }
            4 => {
                self.release(-8.0);
                return true;
            }
            _ => {}
        }
        false
    }

    pub fn update(&mut self, dt: f32) {
        self.blender.step(dt);
        self.blender.ended();
        self.blender.prune();
    }

    pub fn make_abortable(&mut self, urgency: u8, event: Option<Event207>, health: f32) -> bool {
        if urgency == 2 {
            if let Some(p) = self.blender.player_mut() {
                p.player.set_blend(-1000.0);
                p.mine = false;
            }
            self.left = 0;
            self.state = 4;
            return true;
        }
        let Some(e) = event.filter(|_| urgency == 1) else { return false };
        if !(e.ends_tasks || e.kind == DAMAGE && health < 1.0 || e.kind == IN_WATER || e.priority == 0x65 || e.kind == STUCK_IN_AIR) {
            return false;
        }
        self.release(if e.kind == IN_WATER || e.kind == STUCK_IN_AIR { -8.0 } else { -4.0 });
        self.state = 4;
        self.left = 0;
        true
    }

    pub fn finish(&mut self) {
        self.release(-8.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FallAndGetUp {
    pub group: i32,
    pub id: i32,
    pub ms: u32,
}

impl FallAndGetUp {
    pub fn new(group: i32, id: i32, ms: u32) -> FallAndGetUp {
        FallAndGetUp { group, id, ms }
    }

    pub fn from_side(side: u32, ms: u32) -> FallAndGetUp {
        FallAndGetUp { group: DAMAGE_KO, id: ko_id(side), ms }
    }

    pub fn first(&self) -> SimpleFall {
        SimpleFall::new(self.group, self.id, self.ms)
    }

    pub fn next(&self, done: u32) -> Option<SimpleGetUp> {
        (done == kind::FALL).then(default_get_up)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GetUpAndStandStill;

impl GetUpAndStandStill {
    pub fn first(&self) -> SimpleGetUp {
        default_get_up()
    }

    pub fn next(&self, done: u32, state: u8) -> Option<StandStill> {
        (done == kind::GET_UP && state == 3).then(|| StandStill::new(0, 8.0, false))
    }
}

pub fn default_get_up() -> SimpleGetUp {
    SimpleGetUp::new(getup::GET_UP, getup::id::GET_UP_NORMAL, true)
}

pub fn info(def: &ClipDef, duration: f32) -> ClipInfo {
    ClipInfo { duration, flags: def.flags }
}

