use bevy::camera::visibility::RenderLayers;
use bevy::camera::ClearColorConfig;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use delirium_data::taskparams::TaskParams;
use delirium_data::weapons::Weapon;
use delirium::collision::Collision;
use delirium::msg::{self, Kind, Value};
use delirium::nm::{Agent, Hit};
use delirium::shottask::{self, ShotTask};
use delirium::ragdoll::{Build, Capsule, Ragdoll};
use std::collections::VecDeque;
use std::io::Write;

mod car;
mod getup;
mod learn;
mod map;
#[allow(dead_code)]
mod melee;
mod hero;
mod sfx;
mod world;
#[cfg(feature = "portal")]
mod portal;

const WHO: [&str; 1] = ["ours"];
const SIDES: usize = WHO.len();

const CHEST: usize = 9;
const WHIZZ: f32 = 1.5;
const SNAP: f32 = 0.45;
const HEAD: usize = 12;
const THIGH_L: usize = 1;
const THIGH_R: usize = 4;

const BEHAVIOURS: [&str; 21] = ["balance", "drunk", "windmill", "pedal", "writhe", "rollup", "catchfall", "highfall", "stairs", "shot",
                                "flinch", "brace", "look", "point", "lean", "relax", "stayup", "foetal", "grab", "overwall", "STOP all"];
const POKE: f32 = 8.0;
const TOUCH_GLOW: (LinearRgba, f32) = (LinearRgba::rgb(2.0, 1.7, 0.4), 3.0);
const SHOT_GLOW: (LinearRgba, f32) = (LinearRgba::rgb(3.0, 0.35, 0.15), 0.5);
const BRUISE_GLOW: (LinearRgba, f32) = (LinearRgba::rgb(0.45, 0.04, 0.22), 0.25);
const KNOCK_BLOOD: usize = 2;
const RING_FROM: f32 = 0.015;
const RING_TO: f32 = 0.14;
const RING_LIFE: f32 = 1.2;
const RIPPLE_V: f32 = 1.5;
const RIPPLE_R: f32 = 0.5;
const RIPPLE_LIFE: f32 = 0.9;
const RIPPLE_GAP: f32 = 0.4;
const TAPE: usize = 600;

struct Frame {
    caps: Vec<Transform>,
    links: Vec<Transform>,
    props: Vec<Transform>,
    cars: Vec<(Vec3, Vec3, f32)>,
    pusher: Option<(Vec3, Vec3, f32)>,
}
const BLOOD_PIX: usize = 14;
const BLOOD_MAX: usize = 400;
const BLOOD_LIFE: f32 = 6.0;
const BLOOD_SIZE: f32 = 0.035;
const POP_LIFE: f32 = 1.2;
const POP_RISE: f32 = 0.5;
const CAR_HIT_STEPS: u64 = 8;
const HELP_SEEN: &str = "delirium-room.seen";
const SPEEDS: (f32, f32) = (1.0 / 16.0, 4.0);

#[derive(Clone, Copy, PartialEq)]
struct Look {
    sky: [f32; 3],
    floor: [f32; 3],
    grid: bool,
    fog: bool,
    sun: f32,
    glow: bool,
}
const LOOKS: [(&str, Look); 4] = [
    ("studio", Look { sky: [0.78, 0.79, 0.81], floor: [0.58, 0.59, 0.61], grid: true, fog: false, sun: 1.0, glow: true }),
    ("void", Look { sky: [0.01, 0.01, 0.02], floor: [0.06, 0.1, 0.22], grid: true, fog: true, sun: 1.6, glow: true }),
    ("daylight", Look { sky: [0.55, 0.72, 0.92], floor: [0.45, 0.48, 0.4], grid: false, fog: true, sun: 1.3, glow: true }),
    ("blueprint", Look { sky: [0.05, 0.16, 0.35], floor: [0.08, 0.22, 0.45], grid: true, fog: false, sun: 0.7, glow: false }),
];
const LOOK_FILE: &str = "delirium-room.look";

fn look_path() -> Option<std::path::PathBuf> {
    std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(LOOK_FILE)))
}
fn saved_look() -> Look {
    let n: Vec<f32> = look_path().and_then(|p| std::fs::read_to_string(p).ok()).map_or(vec![], |t| t.split_whitespace().filter_map(|x| x.parse().ok()).collect());
    if n.len() != 11 {
        return LOOKS[0].1;
    }
    Look { sky: [n[0], n[1], n[2]], floor: [n[3], n[4], n[5]], grid: n[6] != 0.0, fog: n[7] != 0.0, sun: n[8], glow: n[10] != 0.0 }
}

fn help_seen() -> bool {
    std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(HELP_SEEN))).is_some_and(|p| p.exists())
}
const WEAPONS: [(&str, &str, f32); 11] = [("pistol", "PISTOL", 50.0), ("deagle", "DEAGLE", 50.0), ("smg", "MP5", 50.0),
                                         ("ak47", "AK47", 50.0), ("sniper", "SNIPERRIFLE", 50.0), ("shotgun", "SHOTGUN", 50.0), ("rpg", "RLAUNCHER", 50.0),
                                          ("grenade", "GRENADE", 50.0), ("molotov", "MOLOTOV", 50.0), ("bat", "BASEBALLBAT", 50.0), ("knife", "KNIFE", 50.0)];

const LEAN_SECS: f32 = 1.5;

#[derive(Clone, Copy)]
struct Exit { min: f32, max: f32, speed: Option<f32>, fail: bool }

const SHOT_EXIT: Exit = Exit { min: 1.0, max: 10.0, speed: Some(0.2), fail: false };

fn exit(b: &str) -> Option<Exit> {
    let e = |min, max, speed, fail| Some(Exit { min, max, speed, fail });
    match b {
        "balance" | "drunk" => e(1.0, 10.0, Some(0.2), true),
        "highfall" => e(1.0, 10.0, None, false),
        "relax" => e(1.0, 10.0, Some(0.1), false),
        "rollup" => e(2.0, 3.0, Some(0.1), false),
        "brace" => e(2.0, 30.0, None, false),
        "flinch" => e(1.0, 10.0, Some(0.2), false),
        "catchfall" | "stairs" => e(2.0, 10.0, Some(0.2), false),
        _ => None,
    }
}

#[derive(Clone, Default)]
struct Gun {
    at: Vec3,
    dir: Vec3,
    aim: Vec3,
    placed: bool,
    wait: f32,
    held: f32,
    kick: f32,
    flash: f32,
    seed: u32,
    tracers: Vec<(Vec3, Vec3, f32)>,
    shells: Vec<(Vec3, Vec3, f32)>,
    rockets: Vec<(Vec3, Vec3, f32)>,
    blasts: Vec<(Vec3, f32)>,
    thrown: Vec<(Vec3, Vec3, f32, bool)>,
    fires: Vec<(Vec3, f32)>,
    swing: f32,
}

impl Gun {
    fn rnd(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }
}

const KICK: [f32; 11] = [0.7, 1.1, 0.4, 0.6, 2.2, 2.0, 1.6, 0.3, 0.3, 1.2, 0.8];
const GUN_FALLBACK: [(f32, f32, f32); 11] = [(300.0, 0.5, 0.02), (350.0, 0.5, 0.03), (70.0, 0.5, 0.02), (130.0, 0.5, 0.04), (400.0, 0.0, 0.06), (1200.0, 0.1, 0.06),
                                            (1000.0, 0.0, 0.0), (1000.0, 0.0, 0.0), (1000.0, 0.0, 0.0), (600.0, 0.0, 0.0), (400.0, 0.0, 0.0)];
const TAXI_RADIUS: f32 = 0.303;
const TAXI_AXLE: Vec2 = Vec2::new(0.73, 1.326);
const TAXI_HULL: [Vec3; 2] = [Vec3::new(-0.916, -2.608, -0.143), Vec3::new(0.916, 2.466, 1.05)];
const TAXI_BELT: f32 = 0.62;
const TAXI_ID: i32 = 100;
const PLANK_ID: i32 = 200;
const INJURY_MAX: f32 = 0.8;
const CRUSH_SPEED: f32 = 2.5;
const DODGE_GAP: f32 = 3.0;
const HANG_SECS: f32 = 3.0;
const CAB_GAP: f32 = 1.45;
const CAB_OFF: f32 = 0.12;
const CAB_GO: f32 = 1.5;
const CAB_GAS: f32 = 0.7;
const CAB_LET_GO: f32 = 12.0;
const CAB_HOLD: f32 = 6.0;
const CAB_STOP: f32 = 14.0;
const TAXIS: usize = 4;

const CLIP_FALLBACK: [u32; 11] = [17, 9, 30, 30, 5, 8, 1, 1, 1, 1, 1];

const ROCKET_SPEED: f32 = 30.0;
const ROCKET_LIFE: f32 = 4.0;
const BLAST_R: f32 = 5.0;
const PILE_FUSE: f32 = 2.0;
const BLAST_DV: f32 = 15.0;
const BLAST_DAMAGE: f32 = 250.0;
const PELLETS: usize = 16;
const PELLET_SPREAD: f32 = 0.4;

const GUN_MODELS: [(&[([f32; 3], [f32; 3], f32)], f32); 11] = [
    (&[([0.03, 0.035, 0.19], [0.0, 0.02, -0.015], 0.0), ([0.028, 0.11, 0.045], [0.0, -0.045, 0.045], -0.25), ([0.006, 0.025, 0.04], [0.0, -0.01, -0.01], 0.0)], 0.11),
    (&[([0.035, 0.045, 0.27], [0.0, 0.025, -0.04], 0.0), ([0.032, 0.12, 0.05], [0.0, -0.05, 0.05], -0.2), ([0.006, 0.03, 0.045], [0.0, -0.01, -0.015], 0.0)], 0.175),
    (&[([0.045, 0.06, 0.36], [0.0, 0.02, -0.08], 0.0), ([0.02, 0.02, 0.1], [0.0, 0.02, -0.31], 0.0), ([0.05, 0.05, 0.14], [0.0, 0.005, -0.2], 0.0),
       ([0.025, 0.14, 0.035], [0.0, -0.08, -0.06], 0.25), ([0.03, 0.1, 0.04], [0.0, -0.05, 0.06], -0.25), ([0.02, 0.05, 0.18], [0.0, 0.0, 0.19], 0.0)], 0.36),
    (&[([0.05, 0.07, 0.42], [0.0, 0.02, -0.05], 0.0), ([0.02, 0.02, 0.3], [0.0, 0.035, -0.4], 0.0), ([0.05, 0.05, 0.2], [0.0, 0.01, -0.3], 0.0),
       ([0.03, 0.2, 0.05], [0.0, -0.1, -0.12], 0.4), ([0.03, 0.1, 0.04], [0.0, -0.05, 0.08], -0.3), ([0.04, 0.08, 0.26], [0.0, -0.01, 0.3], 0.1)], 0.55),
    (&[([0.05, 0.07, 0.45], [0.0, 0.0, 0.0], 0.0), ([0.022, 0.022, 0.6], [0.0, 0.01, -0.52], 0.0), ([0.04, 0.04, 0.3], [0.0, 0.08, -0.02], 0.0),
       ([0.03, 0.08, 0.06], [0.0, -0.06, -0.05], 0.0), ([0.045, 0.12, 0.3], [0.0, -0.03, 0.35], 0.15)], 0.82),
    (&[([0.03, 0.03, 0.75], [0.0, 0.03, -0.32], 0.0), ([0.045, 0.04, 0.2], [0.0, -0.005, -0.42], 0.0), ([0.05, 0.07, 0.2], [0.0, 0.01, 0.04], 0.0),
       ([0.045, 0.1, 0.32], [0.0, -0.04, 0.3], 0.2)], 0.7),
    (&[([0.09, 0.09, 1.0], [0.0, 0.03, -0.1], 0.0), ([0.12, 0.12, 0.22], [0.0, 0.03, -0.7], 0.0), ([0.03, 0.1, 0.04], [0.0, -0.05, 0.05], -0.2),
       ([0.03, 0.09, 0.04], [0.0, -0.04, -0.25], 0.0), ([0.02, 0.06, 0.04], [0.0, 0.1, -0.25], 0.0)], 0.82),
    (&[([0.06, 0.07, 0.06], [0.0, 0.0, -0.03], 0.0), ([0.015, 0.03, 0.015], [0.0, 0.045, -0.03], 0.0)], 0.06),
    (&[([0.075, 0.075, 0.17], [0.0, 0.0, -0.1], 0.0), ([0.03, 0.03, 0.09], [0.0, 0.0, -0.22], 0.0), ([0.02, 0.02, 0.05], [0.0, 0.0, -0.28], 0.0)], 0.3),
    (&[([0.035, 0.035, 0.3], [0.0, 0.0, -0.15], 0.0), ([0.06, 0.06, 0.51], [0.0, 0.0, -0.555], 0.0)], 0.81),
    (&[([0.025, 0.03, 0.11], [0.0, 0.0, -0.055], 0.0), ([0.004, 0.03, 0.22], [0.0, 0.0, -0.22], 0.0)], 0.33),
];

fn kind(w: usize) -> u8 {
    match WEAPONS[w].0 {
        "grenade" | "molotov" => 1,
        "bat" | "knife" => 2,
        _ => 0,
    }
}

fn held(w: usize) -> Option<hero::Held> {
    match WEAPONS[w].0 {
        "bat" => Some((0.09, 0.72, 0.4)),
        "knife" => Some((0.08, 0.25, 0.16)),
        _ => None,
    }
}

const HOMER_REST: f32 = 2.5;
const HOMER_GUARD: f32 = 0.5;
const HOMER_FIRE: f32 = 0.8;
const HOMER_SPREAD: f32 = 0.04;
const HOMER_ARMS: [&str; 8] = ["bat", "knife", "pistol", "deagle", "smg", "ak47", "sniper", "shotgun"];
const HERO_DOWN: f32 = 1.0;
const HERO_SLOW: f32 = 0.3;
const HERO_BLEND: f32 = 0.3;
const HERO: usize = usize::MAX;

const SWING: f32 = 0.35;
const BAT_SWING: f32 = 0.7;
const BAT_PUSH: f32 = 2.5;
const BAT_BLOOD: usize = 3;
const KNIFE_BLOOD: usize = 8;

fn swing_len(w: usize) -> f32 {
    if WEAPONS[w].0 == "bat" { BAT_SWING } else { SWING }
}

fn bat_swing(s: f32) -> (f32, f32) {
    let ease = |u: f32| {
        let u = u.clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    let (rest, back, through) = (-0.35, -1.1, 2.5);
    if s < 0.35 {
        let e = ease(s / 0.35);
        (rest + (back - rest) * e, -0.12 * e)
    } else if s < 0.65 {
        let e = ease((s - 0.35) / 0.3);
        (back + (through - back) * e, -0.12 + 0.42 * e)
    } else {
        let e = ease((s - 0.65) / 0.35);
        (through + (rest + std::f32::consts::TAU - through) * e, 0.3 * (1.0 - e))
    }
}

const THROW_SPEED: f32 = 15.0;
const FUSE: f32 = 3.0;
const BOUNCE: f32 = 0.35;
const FIRE_R: f32 = 1.5;
const FIRE_LIFE: f32 = 8.0;
const BURN: f32 = 6.0;
const BURN_DAMAGE: f32 = 25.0;

#[derive(Component)]
struct GunMesh(usize, bool);

struct HeroDoll {
    agent: Agent,
    st: msg::State,
    t: f32,
    rise: Option<getup::Rise>,
}

#[derive(Component)]
struct GunFlash;

#[derive(Component)]
struct BloodPix(usize);

struct Sim {
    build: Build,
    frames: Vec<Transform>,
    world: Collision,
    tp: TaskParams,
    agent: Agent,
    msgs: msg::State,
    paused: bool,
    speed: f32,
    acc: f32,
    steps: u64,
    arrows: bool,
    log: VecDeque<(f32, String, u32)>,
    console: String,
    script: VecDeque<(f32, String)>,
    on_cue: Vec<String>,
    cam: Option<[f32; 3]>,
    film: f32,
    photo: bool,
    soft: bool,
    gun_reach: f32,
    gun_up: f32,
    gun_off: Vec3,
    gun_aim_off: Vec3,
    crane: Option<([f32; 4], [f32; 4], f32, f32, Option<(Vec3, Vec3)>)>,
    crane_to: Option<([f32; 4], f32, Option<Vec3>)>,
    shadow: f32,
    strict: bool,
    film_vel: Vec3,
    photo_pending: bool,
    photo_follow: bool,
    cam_at: Option<Vec3>,
    shot_scene: String,
    photo_note: String,
    look_z: f32,
    script_walk: Vec3,
    throw_speed: Option<f32>,
    typing: bool,
    weapons: Vec<Weapon>,
    weapon: usize,
    weak: bool,
    sel: usize,
    over_ui: bool,
    held: bool,
    show_ui: bool,
    sens: f32,
    rec: Option<std::io::LineWriter<std::fs::File>>,
    health: f32,
    aim_gun: bool,
    gun: Gun,
    hero_gun: bool,
    ammo: u32,
    taxis: Vec<car::Car>,
    driving: bool,
    pedals: (f32, f32, bool),
    car_tune: Option<car::Tune>,
    reloading: Option<f32>,
    blast_task: Option<delirium::explosion::Explosion>,
    burning: f32,
    on_fire: Option<delirium::nm::OnFire>,
    god: bool,
    lean_stop: Option<f32>,
    hurt: [f32; 4],
    near_at: f32,
    no_dodge: bool,
    super_punch: bool,
    crush: Option<(i32, u64)>,
    cab_gas: f32,
    shot_task: Option<(ShotTask, u64)>,
    rand: delirium_data::rng::Rand,
    getup: Option<getup::Clips>,
    rises: [Option<getup::Rise>; 2],
    still: [f32; 2],
    tasks: bool,
    exit: Exit,
    nm_t: [Option<f32>; 2],
    failed: [bool; 2],
    last: [Vec3; 2],
    room: world::Room,
    look: bool,
    avoid: bool,
    map: Option<Vec<delirium_data::bounds::CollisionMesh>>,
    eye: Vec3,
    hover: Option<(Vec2, String)>,
    crowd: Vec<(Agent, msg::State)>,
    group: delirium::ragdoll::Group,
    cars: Vec<(Vec3, Vec3, f32)>,
    lanes: Option<[f32; 2]>,
    pusher: Option<(Vec3, Vec3, f32)>,
    pile_fuse: Option<(f32, Vec3)>,
    ice: Option<(usize, [f32; 2])>,
    click_shoots: bool,
    grab: Option<Grab>,
    glow: Vec<Vec<(f32, (LinearRgba, f32))>>,
    rings: Vec<(usize, usize, Vec3, Vec3, f32)>,
    ripples: Vec<(Vec3, f32, f32)>,
    ripple_wait: Vec<f32>,
    tape: std::collections::VecDeque<Frame>,
    replay: Option<f32>,
    pops: Vec<(Vec3, f32, f32, bool)>,
    pop_screen: Vec<(Vec2, f32, f32, bool)>,
    show_pops: bool,
    tab: u8,
    skin: Look,
    sounds: Vec<sfx::Snd>,
    sound: bool,
    blood: Vec<(Vec3, Vec3, f32, u8)>,
    pixel_blood: bool,
    show_ripples: bool,
    infinite: bool,
    fx_seed: u32,
    pointer: Option<Vec3>,
    crowd_hp: Vec<f32>,
    car_hit: Vec<Option<(i32, Vec3, Vec3, u64)>>,
    car_reacts: Vec<u64>,
    help: bool,
    throw_at: Option<Vec3>,
    auto_balance: bool,
    dead: bool,
    hero: Option<hero::Hero>,
    passive: bool,
    homer: Option<hero::Hero>,
    homer_weapon: Option<usize>,
    homer_guard: Option<bool>,
    homer_shot: f32,
    hero_physics: bool,
    hero_doll: Option<HeroDoll>,
    #[cfg(feature = "portal")]
    portal: portal::Portals,
    homer_wait: Option<f32>,
    homer_rest: f32,
    agility: u8,
    walk: bool,
    walk_dir: Vec3,
    relock: bool,
    cam_locked: bool,
    wander: delirium::playerctl::Wander,
    getup_mode: u8,
    musc: Option<getup::Rise>,
    learned: Option<(learn::Net, Vec<Quat>, usize, learn::Pick)>,
    walk_learned: Option<(learn::Net, Vec<Quat>, f32, f32, bool)>,
    ped_color: usize,
    eyes: bool,
    skel: bool,
    brows: bool,
    scene: Option<String>,
    switch_to: Option<(String, bool)>,
    poke: f32,
    game_dir: Option<String>,
    watch: Option<Watch>,
}

struct Watch {
    path: String,
    stamp: Option<std::time::SystemTime>,
    generation: usize,
    scores: Vec<f32>,
    tries: Vec<Vec<Vec<Transform>>>,
    t: f32,
    check: f32,
}

impl Watch {
    fn at(&self, i: usize, k: usize) -> Option<Transform> {
        let t = self.tries.get(i)?;
        let f = t.get(((self.t * 60.0) as usize).min(t.len().checked_sub(1)?))?.get(k)?;
        let (x, y) = ((i % 4) as f32 - 1.5, (i / 4) as f32 - ((self.tries.len() + 3) / 4) as f32 / 2.0 + 0.5);
        Some(f.with_translation(f.translation + Vec3::new(x * 2.5, y * 2.5, 0.0) - t[0][0].translation.with_z(0.0)))
    }

    fn reload(&mut self) -> bool {
        let path = learn::live_path(&self.path);
        let stamp = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if stamp.is_none() || stamp == self.stamp {
            return false;
        }
        let Some((generation, scores, tries)) = learn::load_live(&path) else { return false };
        (self.stamp, self.generation, self.scores, self.tries, self.t) = (stamp, generation, scores, tries, 0.0);
        true
    }
}

const GETUP_MODES: [&str; 4] = ["animation", "muscles", "off", "learned"];

struct Grab {
    part: usize,
    local: Vec3,
    plane: (Vec3, Vec3),
    target: Vec3,
    press: (Vec2, Vec3, Vec3),
    pulling: bool,
}

const MUSC_STIFF: (f32, f32) = (11.0, 12.0);
static MUSC_HELP: std::sync::LazyLock<f32> = std::sync::LazyLock::new(|| std::env::var("DELIRIUM_HELP").ok().and_then(|v| v.parse().ok()).unwrap_or(0.5));

const PULL_K: f32 = 1500.0;
const PULL_MAX: f32 = 1500.0;

struct Side {
    parts: Vec<Vec3>,
    fwd: Vec3,
}

impl Side {
    fn new(parts: Vec<Vec3>) -> Side {
        let right = parts.get(THIGH_R).copied().unwrap_or(Vec3::X) - parts.get(THIGH_L).copied().unwrap_or(Vec3::ZERO);
        let fwd = Vec3::Z.cross(right).with_z(0.0).normalize_or(Vec3::Y);
        Side { parts, fwd }
    }
    fn p(&self, k: usize) -> Vec3 {
        self.parts.get(k).copied().unwrap_or(Vec3::ZERO)
    }
    fn left(&self) -> Vec3 {
        Vec3::Z.cross(self.fwd)
    }
}

fn hero_task(tp: &TaskParams, task: &str, set: &str, head: Vec3, me: Vec3, fwd: Vec3, rand: &mut delirium_data::rng::Rand) -> (Vec<String>, u32) {
    use delirium::meleetask::MeleeTask;
    let hit = Hit { attacker_head: Some(head), attacker_neck: Some(head), pos: me, fwd, ..Default::default() };
    let mut t = if task == "flinch" { MeleeTask::Flinch { set: set.into(), started: false, lean_until: 0 } } else { MeleeTask::Balance { set: set.into(), started: false } };
    let lines = t.update(rand, tp, &hit, 0, false).iter().map(shottask::text).collect();
    (lines, if let MeleeTask::Flinch { lean_until, .. } = t { lean_until } else { 0 })
}

fn v3(x: Vec3) -> String {
    format!("{:.3},{:.3},{:.3}", x.x, x.y, x.z)
}

impl Sim {
    fn reset(&mut self) {
        let frames = self.frames.clone();
        let a = Agent::new(&self.build, Ragdoll::new(&self.build, &frames, Vec3::ZERO), Quat::IDENTITY, 0.3);
        (self.agent, self.msgs, self.steps, self.held) = (a, msg::State::default(), 0, true);
        self.homer = None;
        self.hero_doll = None;
        self.crowd.clear();
        self.hurt = [0.0; 4];
        self.say("ours  reset: pose held (pick a behaviour)".into());
    }

    fn rise(&mut self, dt: f32) {
        let Some(clips) = self.getup.take() else { return };
        for i in 0..SIDES {
            let now: Vec<Transform> = self.agent.doll.frames().collect();
            if now.len() != self.build.parts.len() {
                continue;
            }
            let pelvis = now[0].translation;
            if i == 0 && (self.learned.is_some() || self.walk_learned.is_some()) {
                self.last[i] = pelvis;
                continue;
            }
            if i == 0 && self.musc.is_some() {
                self.musc_rise(&clips, dt);
                self.last[i] = pelvis;
                continue;
            }
            if let Some(r) = self.rises[i].as_mut() {
                r.t += dt;
                if clips.frames(r).is_none() {
                    let end = clips.end(r);
                    self.rises[i] = None;
                    if self.tasks {
                        self.rises[i] = Some(clips.stand(end));
                        continue;
                    }
                    if i == 0 {
                        self.agent = Agent::new(&self.build, Ragdoll::new(&self.build, &end, Vec3::ZERO), Quat::IDENTITY, 0.3);
                        self.msgs = msg::State::default();
                        self.send("bodyBalance start=true");
                    }
                    self.say(format!("{}  up: balancing", WHO[i]));
                }
            } else {
                if let Some(t) = self.nm_t[i].map(|t| t + dt) {
                    self.nm_t[i] = Some(t);
                    let (e, lying) = (self.exit, now[HEAD].translation.z - pelvis.z < 0.3);
                    self.failed[i] |= i == 1 && lying;
                    let v = self.agent.doll.com().1.length();
                    let slow = e.speed.filter(|_| !e.fail || self.failed[i]).is_some_and(|s| v < s);
                    if (t >= e.min && slow) || (t >= e.max && v < 2.0) {
                        self.say(format!("{}  game task: out of NM after {t:.2} s at {v:.2} m/s: {}", WHO[i], if lying { "getting up" } else { "back to the animation" }));
                        self.nm_t[i] = None;
                        self.rises[i] = Some(if lying { clips.start(now) } else { clips.stand(now) });
                    }
                    self.last[i] = pelvis;
                    continue;
                }
                let down = !(i == 0 && self.getup_mode == 2) && !self.tasks && self.health >= 50.0 && now[HEAD].translation.z - pelvis.z < 0.3 && pelvis.distance(self.last[i]) < 0.2 * dt.max(1e-3);
                self.still[i] = if down { self.still[i] + dt } else { 0.0 };
                if self.still[i] >= getup::DOWN_SECS {
                    self.still[i] = 0.0;
                    if i == 0 && self.getup_mode == 3 {
                        match learn::Net::load(&learn::path()) {
                            Some(net) => {
                                self.agent = learn::agent(&self.build, &now);
                                self.msgs = msg::State::default();
                                self.learned = Some((net, learn::zero(&self.build, &self.frames), 0, Default::default()));
                                self.say("ours  getting up as he learned (getup.net)".into());
                            }
                            None => self.say("ours  no getup.net yet (train: --learn)".into()),
                        }
                        self.last[i] = pelvis;
                        continue;
                    }
                    if i == 0 && self.getup_mode == 1 {
                        self.musc = Some(clips.start(now));
                        delirium::nm::task_pose(&mut self.agent, &self.build, &self.world, &mut self.rand, MUSC_STIFF.0, MUSC_STIFF.1);
                        self.say("ours  getting up with his muscles (the clip's poses as NMPose's; he can fail)".into());
                        self.last[i] = pelvis;
                        continue;
                    }
                    self.rises[i] = Some(clips.start(now));
                    self.say(format!("{}  getting up (the game's get_up_fast; the viewer's, not the game's task)", WHO[i]));
                }
            }
            self.last[i] = pelvis;
        }
        self.getup = Some(clips);
    }

    fn musc_rise(&mut self, clips: &getup::Clips, dt: f32) {
        let Some(r) = self.musc.as_mut() else { return };
        r.t += dt;
        match clips.frames(r) {
            Some(p) => {
                let (at, v) = (self.agent.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation), self.agent.doll.com().1);
                let m = self.agent.doll.mass();
                let f = (m * (40.0 * (p[0].translation - at) - 8.0 * v)).clamp_length_max(*MUSC_HELP * m * 9.81);
                self.agent.push.push((0, at, f));
                delirium::nm::task_pose_control(&mut self.agent, p)
            }
            None => {
                self.musc = None;
                self.send("bodyBalance start=true");
                self.say("ours  muscles' get-up done: balancing".into());
            }
        }
    }

    fn say(&mut self, s: String) {
        if std::env::var_os("ROOM_LOG").is_some() {
            eprintln!("room {:.2} {s}", self.steps as f32 / 60.0);
        }
        if let Some(r) = self.rec.as_mut() {
            let _ = writeln!(r, "# {:.2} {s}", self.steps as f32 / 60.0);
        }
        let (t, s) = (self.steps as f32 / 60.0, pretty(&s));
        if let Some(i) = self.log.iter().rposition(|l| l.1 == s).filter(|_| s.starts_with("FB ")) {
            let n = self.log.remove(i).map_or(0, |l| l.2);
            self.log.push_back((t, s, n + 1));
            return;
        }
        self.log.push_back((t, s, 1));
        while self.log.len() > 300 {
            self.log.pop_front();
        }
    }

    fn crowd(&mut self) {
        self.homers("crowd", &[(0.9, 0.0), (1.7, 0.0), (0.9, -0.8), (0.9, 0.8)]);
    }

    fn grid(&mut self) {
        let at: Vec<(f32, f32)> = (0..25).filter(|&i| i != 12).map(|i| (((i / 5) as f32 - 2.0) * 1.2, ((i % 5) as f32 - 2.0) * 1.2)).collect();
        self.homers("grid", &at);
    }

    fn homers(&mut self, name: &str, at: &[(f32, f32)]) {
        let had = std::mem::take(&mut self.crowd).len();
        (self.crowd_hp, self.car_hit, self.car_reacts) = (vec![200.0; at.len()], vec![], vec![]);
        self.glow.truncate(1);
        self.rings.retain(|r| r.0 == 0);
        if had == at.len() {
            return self.say(format!("room  {name}: off"));
        }
        let s = self.ours();
        for (i, &(f, x)) in at.iter().enumerate() {
            let off = (s.fwd * f + s.left() * x).with_z(0.0);
            let fr: Vec<Transform> = self.agent.doll.frames().map(|f| f.with_translation(f.translation + off)).collect();
            let mut a = Agent::new(&self.build, Ragdoll::new(&self.build, &fr, Vec3::ZERO), Quat::IDENTITY, 0.3 + 0.1 * i as f32);
            let mut st = msg::State::default();
            let _ = msg::send(&mut a, &self.build, &self.world, &mut st, "bodyBalance", &[("start", Value::Bool(true))]);
            self.crowd.push((a, st));
        }
        self.held = false;
        self.say(format!("room  {name}: {} more Homers, balancing", at.len()));
    }

    fn car(&mut self) {
        let s = self.ours();
        let left = s.left().with_z(0.0).normalize_or_zero();
        self.cars.push((s.p(0).with_z(0.75) + left * 4.0, -left * 8.0, 1.5));
        self.held = false;
        self.say("ours  car: 8 m/s from his left (ours only)".into());
    }

    fn push(&mut self, front: bool) {
        let s = self.ours();
        let fwd = s.fwd.with_z(0.0).normalize_or_zero() * if front { 1.0 } else { -1.0 };
        let c = s.p(0).with_z(0.9) + fwd * 1.6;
        self.pusher = Some((c, -fwd * 1.2, 3.5));
        self.held = false;
        if front {
            self.button("brace");
            self.both(|s| vec![format!("braceForImpact start=true pos={} look={}", v3(s.p(0).with_z(1.2) + s.fwd * 1.4), v3(s.p(0).with_z(1.5) + s.fwd * 1.4))]);
        }
        self.say(format!("ours  push {}: a slab at 1.2 m/s{}", if front { "front" } else { "back" }, if front { ", he braces" } else { "" }));
    }

    fn traffic(&mut self) {
        const LANES: [(f32, f32); 2] = [(-2.6, 1.0), (2.6, -1.0)];
        let car = |x: f32, y: f32, dir: f32| (Vec3::new(x, y, 0.75), Vec3::X * dir * 12.0, (32.0 - x * dir) / 12.0);
        if self.lanes.is_none() {
            for (y, dir) in LANES {
                self.cars.extend((0..3).map(|i| car(-24.0 + 22.0 * i as f32 + 6.0 * dir, y, dir)));
            }
            self.say("room  street: traffic both ways".into());
        }
        let wait = self.lanes.get_or_insert([0.5, 1.5]);
        for (k, (y, dir)) in LANES.into_iter().enumerate() {
            wait[k] -= 1.0 / 60.0;
            if wait[k] <= 0.0 {
                let r = ((self.steps as u64 + 7 * k as u64).wrapping_mul(2654435761) >> 8 & 1023) as f32 / 1023.0;
                wait[k] = 1.2 + 1.8 * r;
                self.cars.push(car(-32.0 * dir, y, dir));
            }
        }
    }

    fn spawn_taxi(&mut self) {
        if self.taxis.len() >= TAXIS {
            return self.say("room  taxis: all 4 out already (clear taxis takes them away)".into());
        }
        let s = self.ours();
        let k = self.taxis.len() as f32;
        let (at, f) = match self.hero.as_ref() {
            Some(n) => {
                let f = n.facing().with_z(0.0).normalize_or(Vec3::X);
                (n.feet + f.cross(Vec3::Z) * (2.5 + 3.0 * k) + f, f)
            }
            None => {
                let left = s.left().with_z(0.0).normalize_or(Vec3::X);
                (s.p(0) + left * (12.0 + 6.0 * k), -left)
            }
        };
        self.spawn_taxi_at(at, f);
        self.say(format!("room  taxi {}: Hero gets in and out with F; W / S drive, A / D steer, Space handbrake", self.taxis.len()));
    }

    fn spawn_taxi_at(&mut self, at: Vec3, f: Vec3) {
        let game = self.game_dir.clone();
        let tune = self.car_tune.get_or_insert_with(|| {
            let [h, ide, mats]: [Option<String>; 3] = game.map(|g| common(&g, &["data/handling.dat", "data/vehicles.ide", "data/materials/materials.dat"])
                .try_into().unwrap()).unwrap_or_default();
            car::Tune::new("taxi", h.as_deref(), ide.as_deref(), mats.as_deref(), TAXI_HULL[1] - TAXI_HULL[0])
        }).clone();
        let ground = at.with_z(self.world.ground(at + Vec3::Z * 1.5, 4.0).unwrap_or(at.z));
        self.taxis.push(car::Car::new(tune, ground, f32::atan2(-f.x, f.y), TAXI_RADIUS, TAXI_AXLE, TAXI_HULL, &self.world));
    }

    fn taxi_door(&mut self) {
        let Some(n) = self.hero.as_mut() else { return };
        if let Some(t) = self.taxis.iter_mut().find(|t| t.player) {
            n.feet = (t.pos + t.rot * Vec3::NEG_X * 1.6).with_z(t.pos.z - TAXI_RADIUS);
            (n.yaw, n.vel) = (t.heading() + std::f32::consts::FRAC_PI_2, Vec3::ZERO);
            t.player = false;
            (self.driving, self.pedals) = (false, (0.0, 0.0, true));
            return self.say("hero  out of the taxi".into());
        }
        let feet = n.feet;
        if let Some(t) = self.taxis.iter_mut().filter(|t| t.pos.distance(feet) < 4.0).min_by(|a, b| a.pos.distance(feet).total_cmp(&b.pos.distance(feet))) {
            (t.player, t.asleep) = (true, false);
            (self.driving, self.pedals) = (true, (0.0, 0.0, false));
            self.gun.placed = false;
            self.say("hero  driving".into());
        }
    }

    fn taxi_frame(&mut self, dt: f32) {
        let (pedal, wheel, hb) = self.pedals;
        let gas = self.cab_gas;
        for (k, t) in self.taxis.iter_mut().enumerate().filter(|t| !t.1.asleep || (t.0 == 0 && gas > 0.0)) {
            let gas = if k == 0 { gas } else { 0.0 };
            t.asleep = false;
            match t.player {
                true => t.step(&self.world, pedal.max(0.0), (-pedal).max(0.0), -wheel, hb, dt),
                false => t.step(&self.world, gas, 0.0, 0.0, gas == 0.0, dt),
            }
        }
        for i in 0..self.taxis.len() {
            let (a, b) = self.taxis.split_at_mut(i + 1);
            for c in b {
                car::collide(&mut a[i], c);
            }
        }
        let mut said = vec![];
        for (k, t) in self.taxis.iter_mut().enumerate() {
            let (took, blew) = t.damage(dt);
            if let Some(d) = took.filter(|&d| d >= 5.0) {
                said.push(format!("room  taxi {}: {d:.0} crash damage (body {:.0}, engine {:.0})", k + 1, t.health.body, t.health.engine));
            }
            if blew {
                said.push(format!("room  taxi {}: its engine catches fire", k + 1));
            }
            t.settle(dt);
        }
        for m in said {
            self.say(m);
        }
        if self.agent.fallen {
            return;
        }
        let ours = self.ours();
        let w = delirium_data::carscan::Walker { pos: ours.p(0).to_array(), forward: ours.fwd.to_array(), on_it: false, animated: true, dodging: false,
                                             player_or_mission: false, health: self.health, agility: self.agility };
        for k in 0..self.taxis.len() {
            let t = &self.taxis[k];
            if t.speed.abs() < 1.0 || t.pos.distance(ours.p(0)) > 20.0 {
                self.taxis[k].braced = false;
                continue;
            }
            let rand = &mut self.rand;
            let (c, speed, right, car) = (t.pos + t.rot * Vec3::Z * 0.4, t.speed, t.rot * Vec3::X, t.scan());
            let weak = match (!t.braced).then(|| delirium_data::carscan::scan(&car, &w, || rand.float_in_range(0.0, 1.0), || true)) {
                Some(delirium_data::carscan::Reaction::Brace { weak }) => weak,
                Some(delirium_data::carscan::Reaction::Warn) => {
                    let dive = delirium_data::carscan::dives(&car, self.agility, false, || rand.float_in_range(0.0, 1.0));
                    let away = (right * right.dot(ours.p(0) - t.pos).signum()).with_z(0.0);
                    let (af, al, amount) = (away.dot(ours.fwd), away.dot(ours.left()), if dive { 0.9 } else { 0.5 });
                    self.taxis[k].braced = true;
                    self.both(|s| vec![format!("leanInDirection start=true applyAsForce=true leanAmount={amount} dir={}", v3((s.fwd * af + s.left() * al).with_z(0.0)))]);
                    self.lean_stop = Some(self.steps as f32 / 60.0 + 0.6);
                    self.say(format!("room  taxi {}: {:.0} km/h near Homer, he {} out of its way", k + 1, speed.abs() * 3.6, if dive { "dives" } else { "steps" }));
                    continue;
                }
                _ => continue,
            };
            self.taxis[k].braced = true;
            self.button("brace");
            let rel = c - ours.p(0);
            let (rf, rl) = (rel.dot(ours.fwd), rel.dot(ours.left()));
            self.both(|s| {
                let p = s.p(0) + s.fwd * rf + s.left() * rl + Vec3::Z * rel.z;
                vec![format!("braceForImpact start=true pos={} look={}", v3(p), v3(p))]
            });
            self.say(format!("room  taxi {}: {:.0} km/h at Homer, he braces{}", k + 1, speed.abs() * 3.6, if weak { " (weak)" } else { "" }));
        }
    }

    fn car_hits(&mut self) {
        use delirium::bump::{self, Kind, Thing};
        let n = self.crowd.len() + 1;
        self.car_hit.resize(n, None);
        self.car_reacts.resize(n, 0);
        let mut pressing = None;
        for (i, a) in std::iter::once(&mut self.agent).chain(self.crowd.iter_mut().map(|c| &mut c.0)).enumerate() {
            for (id, j, at) in std::mem::take(&mut a.doll.ob_hits) {
                let Some(t) = self.taxis.get_mut((id - TAXI_ID) as usize) else { continue };
                t.ped_hit(j, at);
                if i == 0 && !self.dead && (0.2..CRUSH_SPEED).contains(&t.speed.abs()) {
                    pressing = Some(id);
                }
                if i == 0 && self.room.stage == "cab" && !self.room.fell {
                    continue;
                }
                if self.steps < self.car_reacts[i] + 60 {
                    continue;
                }
                let h = self.car_hit[i].get_or_insert((id - TAXI_ID, Vec3::ZERO, at, self.steps));
                h.1 += j;
            }
        }
        match (pressing, self.crush) {
            (Some(id), None) if self.room.stage != "cab" => {
                self.say(format!("room  taxi {} pressing on Homer: crushReaction", id - TAXI_ID + 1));
                self.send_ours(vec![format!("crushReaction start=true flinchMode=1 obstacleID={id} useInjuries=true")]);
                self.crush = Some((id, self.steps));
            }
            (Some(id), Some(_)) => self.crush = Some((id, self.steps)),
            (None, Some((_, at))) if self.steps > at + 60 => {
                self.crush = None;
                self.send("crushReaction start=false");
            }
            _ => {}
        }
        for i in 0..n {
            let Some((k, sum, at, from)) = self.car_hit[i].filter(|h| self.steps >= h.3 + CAR_HIT_STEPS) else { continue };
            self.car_hit[i] = None;
            self.car_reacts[i] = self.steps;
            let a = if i == 0 { &self.agent } else { &self.crowd[i - 1].0 };
            let player = self.taxis.get(k as usize).is_some_and(|t| t.player);
            let other = Thing { id: k, kind: Kind::Vehicle { player_driver: player } };
            let imp = bump::Impact { impulse: sum.length(), dt: 1.0 / 60.0, component: a.doll.nearest(&self.build, at) as i32, other: Some(other) };
            let health = if i == 0 { self.health } else { self.crowd_hp[i - 1] };
            let ped = bump::Ped { id: -2 - i as i32, health, inv_mass: 1.0 / a.doll.mass().max(1.0), woke: false, drop: None,
                                  vz: a.doll.vel(0).0.z, tough: false, player_cut: false, player: false, mission: false, quick_relax: false,
                                  agility: self.agility, gun: false, place: a.doll.frames().next().map_or(at, |f| f.translation), vel: Vec3::ZERO, standing_on: None };
            let _ = from;
            let Some((amount, _)) = bump::damage(&ped, &imp, &mut self.rand).filter(|d| d.0 >= 1.0) else { continue };
            let left = if i == 0 && self.god { 200.0 } else { (health - amount).max(1.0) };
            if i == 0 {
                self.health = left;
            } else {
                self.crowd_hp[i - 1] = left;
            }
            self.pop(at, amount);
            if i == 0 {
                let part = self.agent.doll.nearest(&self.build, at);
                self.injure(part, (amount / 100.0).clamp(0.1, 0.5));
            }
            self.sounds.push(sfx::Snd::Crash);
            let place = self.taxis.get(k as usize).map_or(at, |t| t.pos);
            let around = bump::Around { touching: vec![other], player: player.then_some(bump::Player { thing: other, place, in_car: true }),
                                        blend: None, sit: None, die: None, accepts: true };
            let task = match bump::react(&ped, &imp, &around, amount, false, left, &mut self.rand, || None).map(|m| m.task) {
                Some(bump::Task::Brace { .. }) => "brace",
                Some(bump::Task::RollUp { .. }) => "rollup",
                Some(bump::Task::Balance { .. }) => "balance",
                Some(_) => "relax",
                None => "",
            };
            let who = if i == 0 { "Homer".to_string() } else { format!("crowd ped {i}") };
            self.say(format!("room  taxi {} hits {who}: {amount:.0} damage (health {left:.0}){}", k + 1,
                             if task.is_empty() { String::new() } else { format!(", {task}") }));
            if task.is_empty() {
                continue;
            }
            if i == 0 {
                self.button(task);
            } else {
                let side = Side::new(self.crowd[i - 1].0.doll.frames().map(|f| f.translation).collect());
                for l in self.lines(task, &side) {
                    self.send_to(i, &l);
                }
            }
        }
    }

    fn ice(&mut self) {
        let mut rows = delirium::ragdoll::SURFACES.write().unwrap();
        let on = match self.ice.take() {
            Some((r, was)) => {
                rows[r] = was;
                false
            }
            None => {
                let r = self.room.surfaces.iter().position(|s| s.0 == "CONCRETE").unwrap_or(0);
                if rows.len() <= r {
                    rows.resize(r + 1, [1.0, 0.1]);
                }
                self.ice = Some((r, rows[r]));
                rows[r][0] = 0.05;
                true
            }
        };
        drop(rows);
        self.say(format!("ours  ice: {}", if on { "on (floor friction 0.05)" } else { "off" }));
    }

    fn fall(&mut self) {
        let fr: Vec<Transform> = self.agent.doll.frames().map(|f| f.with_translation(f.translation + Vec3::Z * 3.0)).collect();
        (self.agent, self.msgs) = (Agent::new(&self.build, Ragdoll::new(&self.build, &fr, Vec3::ZERO), Quat::IDENTITY, 0.3), msg::State::default());
        let _ = msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, "smartFall", &[("start", Value::Bool(true))]);
        self.held = false;
        self.say("ours  fall: from 3 m, smartFall".into());
    }

    fn toggle_hero(&mut self) {
        if self.hero.take().is_some() {
            self.hero_doll = None;
            return self.say("hero  out".into());
        }
        match self.getup.as_ref().filter(|c| c.hero.is_some()) {
            Some(c) => {
                let n = hero::Hero::new(c, self.agent.doll.com().0, self.ours().fwd);
                self.say(format!("hero  in at {}: WASD walk, Shift run, Ctrl sprint, left click punch, T heavy, G kick, V block, hold Z lock on", v3(n.feet)));
                self.hero = Some(n);
            }
            None => self.say("hero  needs the game's files (playerped.rpf's player.xft)".into()),
        }
    }

    fn play_hero(&mut self, dt: f32, mv: Vec3, gait: usize) {
        if self.hero_doll.is_some() {
            return;
        }
        let fwd = self.ours().fwd;
        let melee = self.hero_gun && kind(self.weapon) == 2;
        let held = if melee { held(self.weapon) } else { None };
        let (Some(n), Some(clips)) = (self.hero.as_mut(), self.getup.as_ref()) else { return };
        let up = !self.agent.fallen && self.rises[0].is_none();
        let pelvis = self.agent.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
        let fists = self.weapons.iter().find(|w| w.name == if melee { WEAPONS[self.weapon].1 } else { "UNARMED" }).cloned().unwrap_or_default();
        let homer_arm = self.homer_weapon.filter(|&w| kind(w) == 2).and_then(|w| self.weapons.iter().find(|x| x.name == WEAPONS[w].1));
        n.foe = self.homer.as_ref().map_or_else(Default::default, |h| h.as_foe(clips, homer_arm));
        let frames: Vec<Transform> = self.agent.doll.frames().collect();
        let homer = up.then(|| (pelvis, (-fwd.x).atan2(fwd.y), self.health, clips.points(self.build.parts.iter().map(|p| p.name.as_str()), &frames)));
        let hit = n.update(clips, &self.world, dt, mv, gait, homer, &fists, held, &caps_of(&self.build, &self.agent.doll));
        if n.disarmed() && self.homer.is_some() {
            if let Some(w) = self.homer_weapon.take() {
                self.say(format!("hero  disarms Homer: takes his {}", WEAPONS[w].0));
                return self.hero_take(Some(w));
            }
        }
        let Some(hit) = hit else { return };
        if let (hero::Hit::Swing(..), Some(me)) = (&hit, self.homer.as_mut()) {
            if me.blocking(clips) {
                let how = me.hit_by(clips, n, &fists).unwrap_or_else(|| "no reaction".into());
                self.sounds.push(sfx::Snd::Punch);
                return self.say(format!("homer  blocks the Hero's blow: {how}"));
            }
        }
        let head = n.head();
        let what = if matches!(hit, hero::Hit::Bump(_)) { "hero  runs into Homer" } else { "hero  punch lands" };
        self.land(hit, head, what);
    }

    fn homer_fight(&mut self, dt: f32) {
        self.homer_rest -= dt;
        let pelvis = self.agent.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
        let near = self.hero.as_ref().is_some_and(|n| n.feet.with_z(0.0).distance(pelvis.with_z(0.0)) < delirium_data::melee_task::SEEK_BEYOND);
        if self.passive || self.driving || self.dead || self.walk_learned.is_some() || !near || dt <= 0.0 {
            return self.homer_out(true);
        }
        let standing = self.rises[0].as_ref().is_none_or(getup::Clips::standing);
        if self.homer.is_none() && (self.homer_rest > 0.0 || self.agent.fallen || !standing || self.nm_t[0].is_some() || self.musc.is_some() || self.learned.is_some()) {
            return;
        }
        let (Some(clips), Some(hero)) = (self.getup.as_ref(), self.hero.as_mut()) else { return };
        let me = self.homer.get_or_insert_with(|| {
            let fwd = (pelvis - hero.feet).with_z(0.0).normalize_or(Vec3::X);
            hero::Hero::new(clips, pelvis - fwd * 1.5, fwd)
        });
        self.rises[0] = None;
        let to = (hero.feet - me.feet).with_z(0.0);
        self.homer_wait = self.homer_wait.map(|w| w - dt);
        let down = self.hero_doll.is_some();
        if let Some(w) = self.homer_weapon.filter(|&w| kind(w) == 0) {
            let chest = hero.feet + Vec3::Z * 1.3;
            me.hold = clips.guns.get(w).copied().flatten().map(|(fire, _)| (fire, 0.5, chest));
            me.keys = 0;
            let mv = if to.length() > 8.0 { to.normalize() } else { Vec3::ZERO };
            let fists = self.weapons.iter().find(|w| w.name == "UNARMED").cloned().unwrap_or_default();
            me.update(clips, &self.world, dt, mv, 0, None, &fists, None, &hero.capsules(clips));
            if me.homer.len() == self.build.parts.len() {
                self.agent.doll = Ragdoll::new(&self.build, &me.homer, Vec3::ZERO);
            }
            self.homer_shot -= dt;
            if self.homer_shot > 0.0 || me.hold.is_none() || down {
                return;
            }
            self.homer_shot = HOMER_FIRE;
            let from = me.hand;
            let off = Vec3::new(self.rand.float_in_range(-1.0, 1.0), self.rand.float_in_range(-1.0, 1.0), self.rand.float_in_range(-1.0, 1.0)) * HOMER_SPREAD;
            let dir = ((chest - from).normalize_or(Vec3::X) + off).normalize();
            return self.homer_fire(w, from, dir);
        }
        me.hold = None;
        let attacking = !down && hero.playing() && !hero.blocking(clips);
        let was = self.homer_guard == Some(true);
        self.homer_guard = attacking.then(|| self.homer_guard.unwrap_or_else(|| self.rand.float_in_range(0.0, 1.0) < HOMER_GUARD));
        if (self.homer_guard == Some(true) && me.playing() && !me.blocking(clips)) || (was && !attacking) {
            me.punch();
        }
        if !down && self.homer_guard != Some(true) && !me.playing() && delirium_data::melee_task::wants_move(self.homer_wait, to.length(), false, self.rand.float_in_range(0.0, 1.0)) {
            me.punch();
            self.homer_wait = Some(delirium_data::melee_task::wait(&mut self.rand));
        }
        me.keys = 0x1 | if self.homer_guard == Some(true) { 0x40 } else { 0 };
        let mv = if to.length() > 1.2 && !me.playing() && !down { to.normalize() } else { Vec3::ZERO };
        let arm = self.homer_weapon.map_or("UNARMED", |w| WEAPONS[w].1);
        let fists = self.weapons.iter().find(|w| w.name == arm).cloned().unwrap_or_default();
        let foe = (hero.feet + Vec3::Z, hero.yaw - std::f32::consts::FRAC_PI_2, 200.0, [None; 5]);
        let body = if down { vec![] } else { hero.capsules(clips) };
        let hero_arm = (self.hero_gun && kind(self.weapon) == 2).then(|| self.weapons.iter().find(|x| x.name == WEAPONS[self.weapon].1)).flatten();
        me.foe = hero.as_foe(clips, hero_arm);
        let hit = me.update(clips, &self.world, dt, mv, 0, (!down).then_some(foe), &fists, self.homer_weapon.and_then(held), &body);
        let took = me.disarmed() && hero_arm.is_some();
        let (frames, blocked) = (me.homer.clone(), hero.blocking(clips));
        if frames.len() == self.build.parts.len() {
            self.agent.doll = Ragdoll::new(&self.build, &frames, Vec3::ZERO);
        }
        let mut blow = None;
        if let Some(hero::Hit::Swing(dmg, at, j)) = hit {
            self.sounds.push(sfx::Snd::Punch);
            let with = self.homer_weapon.map_or("punch", |w| WEAPONS[w].0);
            if self.hero_physics && !blocked {
                blow = Some((at, j, me.head(), format!("homer  {with} lands on the Hero ({dmg:.0} damage)")));
            } else {
                let how = hero.hit_by(clips, me, &fists).unwrap_or_else(|| "no reaction".into());
                self.say(format!("homer  {with} lands on the Hero ({dmg:.0} damage{}): {how}", if blocked { ", blocking" } else { "" }));
            }
        }
        if took {
            self.homer_weapon = Some(self.weapon);
            self.say(format!("homer  disarms the Hero: takes his {}", WEAPONS[self.weapon].0));
            self.hero_take(None);
        }
        if let Some((at, j, head, what)) = blow {
            let task = self.hero_blow(at, j, head).unwrap_or("no reaction");
            self.say(format!("{what}: on his ragdoll ({task})"));
        }
    }

    fn homer_fire(&mut self, w: usize, from: Vec3, dir: Vec3) {
        self.sounds.push(sfx::Snd::Shot(w));
        let caps = self.hero.as_ref().zip(self.getup.as_ref()).map_or(vec![], |(n, c)| n.capsules(c));
        let hit = caps.iter().filter(|c| !c.4).filter_map(|&(a, b, r, ..)| {
            let (p, q) = hero::closest(from, from + dir * 60.0, a, b);
            (p.distance(q) < r).then(|| (p.distance(from), p))
        }).min_by(|x, y| x.0.total_cmp(&y.0)).map(|x| x.1);
        self.gun.tracers.push((from, hit.unwrap_or(from + dir * 30.0), 0.25));
        let Some(at) = hit else { return self.say(format!("homer  {}: missed the Hero", WEAPONS[w].0)) };
        let force = self.weapons.iter().find(|x| x.name == WEAPONS[w].1).map_or(WEAPONS[w].2, |x| x.force);
        let vel = shottask::bullet(at, at - dir * 3.0, force);
        let how = if self.hero_fall() {
            let k = self.hero_doll.as_ref().map_or(0, |h| h.agent.doll.nearest(&self.build, at));
            for l in [format!("applyImpulse partIndex={k} hitPoint={} impulse={}", v3(at), v3(vel)),
                      format!("shot start=true bodyPart={k} hitPoint={} normal={} bulletVel={}", v3(at), v3(-dir), v3(vel))] {
                self.send_to(HERO, &l);
            }
            "on his ragdoll"
        } else {
            "Hero physics off"
        };
        self.say(format!("homer  {} hits the Hero: {how}", WEAPONS[w].0));
    }

    fn hero_fall(&mut self) -> bool {
        if !self.hero_physics {
            return false;
        }
        let frames = match self.hero_doll.as_ref() {
            Some(HeroDoll { rise: None, .. }) => return true,
            Some(HeroDoll { rise: Some(r), .. }) => self.getup.as_ref().and_then(|c| c.frames(r)),
            None => self.hero.as_ref().map(|n| n.homer.clone()),
        };
        let Some(frames) = frames.filter(|f| f.len() == self.build.parts.len()) else { return false };
        let v = self.hero.as_ref().filter(|_| self.hero_doll.is_none()).map_or(Vec3::ZERO, |n| n.vel);
        let mut agent = Agent::new(&self.build, Ragdoll::new(&self.build, &frames, v), Quat::IDENTITY, 0.3);
        let mut st = msg::State::default();
        let _ = msg::send(&mut agent, &self.build, &self.world, &mut st, "bodyBalance", &[("start", Value::Bool(true))]);
        if let Some(n) = self.hero.as_mut() {
            n.hold = None;
        }
        self.hero_doll = Some(HeroDoll { agent, st, t: 0.0, rise: None });
        true
    }

    fn hero_blow(&mut self, at: Vec3, j: Vec3, head: Vec3) -> Option<&'static str> {
        let fwd = self.hero.as_ref()?.facing();
        if !self.hero_fall() {
            return None;
        }
        let doll = &self.hero_doll.as_ref()?.agent.doll;
        let (k, pelvis) = (doll.nearest(&self.build, at), doll.frames().next()?.translation);
        let task = if self.rand.float_in_range(0.0, 1.0) > 0.7 { "balance" } else { "flinch" };
        let (mut o, _) = hero_task(&self.tp, task, if task == "flinch" { "MELEE" } else { "*" }, head, pelvis, fwd, &mut self.rand);
        o.insert(0, format!("applyImpulse start=true partIndex={k} hitPoint={} impulse={}", v3(at), v3(j)));
        for l in o {
            self.send_to(HERO, &l);
        }
        Some(task)
    }

    fn hero_blast(&mut self, at: Vec3) {
        if !self.hero.as_ref().is_some_and(|n| n.feet.distance(at) < BLAST_R + 1.0) || !self.hero_fall() {
            return;
        }
        let Some(h) = self.hero_doll.as_ref() else { return };
        let o: Vec<String> = h.agent.doll.frames().enumerate().filter_map(|(k, f)| {
            let p = f.translation;
            let s = 1.0 - p.distance(at) / BLAST_R;
            let imp = ((p - at).normalize_or(Vec3::Z) + Vec3::Z * 0.5).normalize() * h.agent.doll.part_mass(k) * BLAST_DV * s;
            (s > 0.0).then(|| format!("applyImpulse partIndex={k} hitPoint={} impulse={}", v3(p), v3(imp)))
        }).chain(["catchFall start=true".to_string()]).collect();
        for l in o {
            self.send_to(HERO, &l);
        }
        self.say("hero  blast: thrown".into());
    }

    fn hero_frame(&mut self, dt: f32) {
        let (Some(h), Some(clips)) = (self.hero_doll.as_mut(), self.getup.as_ref()) else { return };
        h.t += dt;
        let Some(r) = h.rise.as_mut() else {
            if h.t >= HERO_DOWN && h.agent.doll.com().1.length() < HERO_SLOW {
                let now: Vec<Transform> = h.agent.doll.frames().collect();
                let lying = now[HEAD].translation.z - now[0].translation.z < 0.3;
                h.rise = Some(if lying { clips.start(now) } else { clips.stand(now) });
            }
            return;
        };
        r.t += dt;
        let end = match clips.frames(r) {
            Some(f) if getup::Clips::standing(r) && r.t >= HERO_BLEND => f,
            Some(_) => return,
            None => clips.end(r),
        };
        let yaw = |q: Quat| {
            let a = q * Vec3::Y;
            a.y.atan2(a.x)
        };
        let first = clips.place(&clips.pose(getup::IDLE, 0.0), Transform::IDENTITY);
        let turn = yaw(end[0].rotation) - yaw(first[0].rotation);
        let root = end[0].translation - Quat::from_rotation_z(turn) * first[0].translation;
        self.hero_doll = None;
        if let Some(n) = self.hero.as_mut() {
            (n.feet, n.yaw, n.vel) = (root.with_z(end[0].translation.z - 1.0), turn + std::f32::consts::FRAC_PI_2, Vec3::ZERO);
        }
        self.say("hero  up".into());
    }

    fn homer_out(&mut self, balance: bool) {
        let Some(me) = self.homer.take() else { return };
        if me.homer.len() != self.build.parts.len() {
            return;
        }
        let doll = std::mem::replace(&mut self.agent.doll, Ragdoll::new(&self.build, &me.homer, Vec3::ZERO));
        self.agent = Agent::new(&self.build, doll, Quat::IDENTITY, 0.3);
        self.msgs = msg::State::default();
        if !balance {
            self.homer_rest = HOMER_REST;
        } else if self.tasks {
            self.rises[0] = self.getup.as_ref().map(|c| c.stand(me.homer.clone()));
        } else {
            self.send("bodyBalance start=true");
        }
    }

    fn land(&mut self, hit: hero::Hit, head: Vec3, what: &str) {
        self.sounds.push(sfx::Snd::Punch);
        let pelvis = self.agent.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
        let ours = self.ours();
        let now = self.steps as f32 / 60.0;
        let imp = |k: usize, at: Vec3, j: Vec3| format!("applyImpulse start=true partIndex={k} hitPoint={} impulse={}", v3(at), v3(j));
        let (task, mut o) = match hit {
            hero::Hit::Swing(dmg, at, j) => {
                self.health = if self.god { 200.0 } else { (self.health - dmg).max(1.0) };
                let k = self.agent.doll.nearest(&self.build, at);
                if kind(self.weapon) == 2 && (self.hero.is_none() || self.hero_gun) {
                    self.cut(k, at, -j.normalize_or(Vec3::Z), dmg);
                }
                (if self.rand.float_in_range(0.0, 1.0) > 0.7 { "balance" } else { "flinch" }, vec![imp(k, at, j)])
            }
            hero::Hit::Bump(_) => ("balance", vec![]),
        };
        let set = match (task, &hit) {
            ("flinch", _) if self.agility < 3 => "MELEE_PASIVE",
            ("flinch", _) => "MELEE",
            (_, hero::Hit::Bump(_)) => delirium::nm::balance_set(self.agility, self.health, false, false),
            _ => "*",
        };
        let (mo, until) = hero_task(&self.tp, task, set, head, pelvis, ours.fwd, &mut self.rand);
        o.extend(mo);
        if let hero::Hit::Bump(from) = hit {
            let lean = |d: Vec3| format!("leanInDirection start=true applyAsForce=true leanAmount=0.4 dir={}", v3(d.with_z(0.0)));
            o.push(lean(pelvis - from));
            self.lean_stop = Some(now + 1.0);
        } else if until != 0 {
            self.lean_stop = Some(now + until as f32 * 1e-3);
        }
        self.send_ours(o);
        if self.tasks {
            (self.exit, self.nm_t, self.failed) = (exit(task).unwrap_or(SHOT_EXIT), [Some(0.0); 2], [false; 2]);
        }
        self.say(format!("{what}: {task} (health {:.0})", self.health));
        if let (hero::Hit::Swing(_, at, _), true) = (hit, self.super_punch && self.hero.is_some()) {
            self.blast(at, false);
        }
    }

    fn ours(&self) -> Side {
        Side::new(self.agent.doll.frames().map(|f| f.translation).collect())
    }

    fn both(&mut self, make: impl Fn(&Side) -> Vec<String>) {
        let o = make(&self.ours());
        self.send_ours(o);
    }

    fn send_ours(&mut self, o: Vec<String>) {
        self.held = false;
        for l in o {
            self.send(&l);
        }
    }

    fn bridge(&mut self) {
        let Some(n) = self.room.bridge.as_ref().map(|b| b.planks.len()) else { return };
        if self.steps == 0 {
            let rotten: Vec<usize> = self.room.bridge.iter().flat_map(|b| b.planks.iter().enumerate()).filter(|p| p.1.rotten).map(|p| p.0 + 1).collect();
            self.say(format!("bridge  rotten planks {rotten:?} (BRIDGE_SEED={})", self.room.bridge.as_ref().unwrap().seed));
        }
        for i in 0..n {
            let (p, t, m) = self.room.plank_triangles(i);
            self.world.remove_chunk(BRIDGE_CHUNK + i as u64);
            self.world.add_chunk(BRIDGE_CHUNK + i as u64, &p, &t, &m);
        }
        let row = self.room.surfaces.iter().position(|s| s.0 == "CONCRETE").unwrap_or(0) as u8;
        for side in 0..2 {
            let (p, t, m) = self.room.bridge.as_ref().unwrap().net_wall(&self.room.w[0], side, row);
            self.world.remove_chunk(BRIDGE_CHUNK + 0x40 + side as u64);
            self.world.add_chunk(BRIDGE_CHUNK + 0x40 + side as u64, &p, &t, &m);
        }
        let v = self.agent.doll.com().1;
        self.room.com_a += ((v - self.room.com_v) * 60.0 - self.room.com_a) * (1.0 / 60.0 / 0.15);
        self.room.com_v = v;
        let f = (Vec3::NEG_Z * 9.81 - self.room.com_a) * world::HOMER_KG;
        let side = f.truncate().clamp_length_max(0.3 * world::HOMER_N);
        self.room.push = side.extend(f.z.clamp(-1.6 * world::HOMER_N, -0.2 * world::HOMER_N));
        let broke = std::mem::take(&mut self.room.broke);
        if let Some(t) = self.room.snapped.as_mut() {
            *t += 1.0 / 60.0;
            let t = *t;
            let s = self.ours();
            let gap = |h: Vec3| (0..2 * n).map(|k| {
                let p = self.room.bridge.as_ref().unwrap().piece(&self.room.w[0], k / 2, k % 2);
                let l = p.rotation.inverse() * (h - p.translation);
                (l - l.clamp(-world::PIECE_HALF, world::PIECE_HALF)).length()
            }).fold(f32::MAX, f32::min);
            let (l, r) = (gap(s.p(16)), gap(s.p(20)));
            if (t * 60.0).round() as u32 % 60 == 0 {
                self.say(format!("bridge  hands from a plank: left {l:.2} m, right {r:.2} m"));
            }
            if !self.room.fell && t > 1.0 && l.min(r) > 0.4 {
                self.room.fell = true;
                self.send_ours(vec!["smartFall start=true".into()]);
                self.say("bridge  he fell: smartFall".into());
            }
            return;
        }
        let Some(&i) = broke.first() else { return };
        let s = self.ours();
        let ahead = s.fwd.y < 0.0;
        let step = |k: usize| if ahead { k + 1 } else { k.wrapping_sub(1) };
        if step(i) < n {
            self.room.bridge.as_mut().unwrap().fall(&mut self.room.w[0], step(i));
        }
        let j = step(step(i));
        if j < n {
            self.room.bridge.as_mut().unwrap().hold(j);
        }
        let (edge, inst) = match j < n {
            true => (Vec3::new(0.0, if ahead { 1.0 } else { -1.0 } * world::PLANK_HALF.y, world::PLANK_HALF.z), PLANK_ID + j as i32),
            false => (Vec3::new(0.0, if ahead { world::BRIDGE_FAR } else { 0.0 }, world::BRIDGE_TOP), -1),
        };
        let x = Vec3::X;
        self.room.snapped = Some(0.0);
        self.cue();
        (self.walk, self.script_walk) = (false, Vec3::ZERO);
        let x = x * (world::PLANK_HALF.x - 0.1);
        self.send_ours(vec![format!("grab start=true linegrab=true useLeft=true useRight=true dontLetGo=true grabStrength=600 bodyStiffness=10                                      grabDist=4.4 reachAngle=2.8 oneSideReachAngle=1.4 pullUpStrengthLeft=0 pullUpStrengthRight=0 pullUpTime=1 pos1={} pos2={} instanceIndex={inst}",
                                    v3(edge + x), v3(edge - x)), "dangle start=true hold=false doGrab=true".into()]);
        self.say(format!("bridge  plank {} broke (and {}): grab {}", i + 1, step(i).wrapping_add(1), j.wrapping_add(1)));
    }

    fn ledge(&mut self) {
        const EDGE: f32 = 0.5;
        if self.room.stage != "rooftop" || self.room.fell {
            return;
        }
        let p = self.ours().p(0);
        match self.room.snapped.as_mut() {
            None if p.y > EDGE - 0.05 && (3.0..3.9).contains(&p.z) => {
                self.room.snapped = Some(0.0);
                self.send_ours(vec![format!("armHang start=true constraintPosition={}", v3(Vec3::new(p.x.clamp(-2.85, 2.85), EDGE, 4.0)))]);
                self.say("rooftop  over the edge: he catches it (armHang)".into());
                self.cue();
            }
            Some(t) => {
                *t += 1.0 / 60.0;
                if *t > HANG_SECS {
                    self.room.fell = true;
                    self.send_ours(vec!["armHang start=false".into(), "highFall start=true".into()]);
                    self.say("rooftop  he lets go: highFall".into());
                }
            }
            None => {}
        }
    }

    fn cab(&mut self) {
        if self.room.stage != "cab" {
            return;
        }
        let Some(t) = self.room.snapped.as_mut() else {
            let s = self.ours();
            let fwd = s.fwd.with_z(0.0).normalize_or(Vec3::NEG_Y);
            self.spawn_taxi_at(s.p(0).with_z(0.0) + fwd * CAB_GAP, Vec3::Z.cross(fwd));
            let o = self.taxis[0].obstacle(TAXI_ID, TAXI_BELT);
            let local = o.rot.inverse() * (s.p(CHEST) - o.centre);
            let door = o.centre + o.rot * local.clamp(-o.half, o.half) - Vec3::Z * 0.2 - fwd * CAB_OFF;
            self.room.snapped = Some(0.0);
            self.send_ours(vec![format!("armHang start=true constraintPosition={} instanceIndex={TAXI_ID}", v3(door))]);
            return self.say("cab  his hands on the cab's door".into());
        };
        *t += 1.0 / 60.0;
        let t = *t;
        let at = |x: f32| t - 1.0 / 60.0 < x && t >= x;
        let speed = self.taxis.first().map_or(0.0, |c| c.speed.abs());
        if at(0.1) {
            self.send_ours(vec!["pedalLegs start=false".into(), "bodyBalance start=true".into()]);
        } else if at(CAB_GO) {
            self.cab_gas = CAB_GAS;
            self.send_ours(vec!["bodyBalance start=false".into(), "pedalLegs start=true".into(), "bodyWrithe start=true mask=uk".into()]);
            self.say("cab  it pulls away with him on the door".into());
        } else if !self.room.fell && t > CAB_GO && (speed > CAB_LET_GO || t > CAB_GO + CAB_HOLD) {
            self.room.fell = true;
            self.send_ours(["armHang", "pedalLegs", "bodyWrithe"].map(|n| format!("{n} start=false")).into_iter().chain(["smartFall start=true".to_string()]).collect());
            self.say(format!("cab  he lets go at {:.0} km/h: smartFall", speed * 3.6));
        } else if self.room.fell && self.cab_gas > 0.0 && self.taxis.first().is_some_and(|c| c.pos.distance(self.ours().p(0)) > CAB_STOP) {
            self.cab_gas = 0.0;
        }
    }

    fn trampoline(&mut self) {
        self.room.bounced += 1.0 / 60.0;
        if self.room.stage != "trampoline" {
            return;
        }
        if self.room.snapped.is_some() && self.room.bounced >= 0.15 {
            let (p, t, m) = self.room.block_triangles(0);
            self.world.add_chunk(SNAP_CHUNK, &p, &t, &m);
            self.room.snapped = None;
        }
        if self.room.bounced < 0.3 {
            return;
        }
        let vz = self.agent.doll.vel(0).0.z;
        let top = 2.0 * world::PAD_HALF.z;
        let on = self.agent.doll.frames().any(|f| f.translation.z < top + 0.12 && f.translation.x.abs() < world::PAD_HALF.x && f.translation.y.abs() < world::PAD_HALF.y);
        if !on || vz > -1.5 {
            return;
        }
        let n = self.agent.doll.frames().count();
        let mean = self.agent.doll.mass() / n as f32;
        let v = self.agent.doll.com().1;
        let dv = Vec3::new(-v.x, -v.y, -v.z.min(0.0) + (-0.8 * v.z).clamp(0.0, 8.0));
        for k in 0..n {
            self.agent.doll.equalized_impulse(k, mean * dv, None, 1.0);
        }
        self.room.bounced = 0.0;
        self.world.remove_chunk(SNAP_CHUNK);
        self.room.snapped = Some(0.0);
        self.say(format!("trampoline  bounce at {:.1} m/s", -vz));
    }

    fn lines(&self, b: &str, s: &Side) -> Vec<String> {
        let (head, chest) = (s.p(HEAD), s.p(CHEST));
        match b {
            "balance" => vec!["bodyBalance start=true".into()],
            "drunk" => vec!["bodyBalance start=true armsOutOnPushTimeout=999".into(), format!("leanInDirection start=true leanAmount=0.2 dir={}", v3(s.fwd))],
            "windmill" => vec!["bodyBalance start=true".into(), "armsWindmillAdaptive start=true".into()],
            "pedal" => vec!["pedalLegs start=true".into()],
            "writhe" => vec!["bodyWrithe start=true".into()],
            "rollup" => vec!["bodyRollUp start=true".into()],
            "catchfall" => vec!["catchFall start=true".into()],
            "highfall" => vec!["highFall start=true".into()],
            "stairs" => vec!["rollDownStairs start=true".into()],
            "flinch" => vec![format!("upperBodyFlinch start=true pos={}", v3(head + s.fwd * 2.0))],
            "brace" => vec![format!("braceForImpact start=true pos={} look={}", v3(chest + s.fwd), v3(chest + s.fwd))],
            "look" => vec![format!("headLook start=true alwaysLook=true pos={}", v3(head + s.fwd * 2.0 + s.left()))],
            "point" => vec![format!("pointArm start=true target={}", v3(chest + s.fwd * 2.0 - s.left()))],
            "lean" => vec!["bodyBalance start=true".into(), format!("leanInDirection start=true leanAmount=0.4 dir={}", v3(s.fwd))],
            "relax" => vec!["bodyRelax start=true".into()],
            "stayup" => vec!["stayUpright start=true stiffness=3".into()],
            "foetal" => vec!["bodyFoetal start=true".into()],
            "grab" => {
                let at = chest + s.fwd * 0.45;
                vec![format!("grab start=true linegrab=true useLeft=true useRight=true dontLetGo=true pos1={} pos2={}",
                             v3(at + s.left() * 0.5), v3(at - s.left() * 0.5))]
            }
            "overwall" => vec![format!("fallOverWall start=true fallOverPoint={} fallOverEdge={}", v3((s.p(0) - s.fwd * 0.3).with_z(0.9)), v3(s.left()))],
            "STOP all" => vec!["stopAllBehaviours".into()],
            x if delirium::extras::messages().any(|m| m.name == x) => vec![format!("{x} start=true")],
            _ => vec![],
        }
    }

    fn fire(&mut self, part: usize, aim: Option<(Vec3, Vec3, Vec3)>, fatal: bool) {
        (self.rises, self.still) = ([None, None], [0.0; 2]);
        let before = self.health;
        let was_dead = self.dead || before <= 0.0;
        let (label, name, _) = WEAPONS[self.weapon];
        let hit = self.weapons.iter().find(|w| w.name == name).map(|w| {
            use delirium_data::ped;
            let mut h = ped::Health { health: self.health, ..Default::default() };
            let hit = ped::Hit::new(w.damage, w, delirium_data::nm_asset::PART_BONES[part], ped::Attacker::Player { sure: true });
            let o = ped::damage(&mut h, &hit, &ped::Victim::default(), &mut self.rand);
            self.health = h.health;
            (o.lost, o.flags & ped::INSTANT != 0)
        });
        if fatal {
            self.health = 0.0;
        }
        if self.god {
            self.health = 200.0;
        }
        if before >= 100.0 && self.health < 100.0 {
            self.sounds.push(sfx::Snd::Kill);
        }
        let state = if self.health < 50.0 { "fatally injured" } else if self.health < 100.0 { "injured (dying)" } else { "standing" };
        self.say(format!("room  shot ({label}) at part {part} {}: -{:.1}{} -> health {:.1} ({state})",
                         delirium_data::nm_asset::PARTS.get(part).unwrap_or(&"?"), hit.map_or(0.0, |h| h.0), if hit.is_some_and(|h| h.1) { " (instant kill)" } else { "" },
                         self.health));
        if self.tasks {
            (self.exit, self.nm_t, self.failed) = (SHOT_EXIT, [Some(0.0); 2], [false; 2]);
        }
        let ours = self.ours();
        let (at, _, n) = aim.unwrap_or((ours.p(part) + ours.fwd * 0.1, -ours.fwd, ours.fwd));
        self.mark(0, part, at, n);
        if let Some((lost, _)) = hit {
            self.pop(at, lost);
            if !was_dead {
                self.injure(part, (lost / 100.0).clamp(0.15, 0.5));
            }
        }
        let w = self.weapons.iter().find(|w| w.name == name);
        let set = w.map_or("*", |w| delirium::nm::weapon_set(w, if self.weak { 2 } else { 3 }));
        let (at, d, n) = aim.unwrap_or((ours.p(part) + ours.fwd * 0.1, -ours.fwd, ours.fwd));
        let vel = shottask::bullet(at, at - d.normalize() * 3.0, w.map_or(WEAPONS[self.weapon].2, |w| w.force));
        let mut o = vec![format!("applyImpulse partIndex={part} hitPoint={} impulse={}", v3(at), v3(vel))];
        if was_dead {
            return self.send_ours(o);
        }
        let (hit, now) = (self.hit(), self.now());
        match self.shot_task.as_mut() {
            Some((t, _)) => o.extend(t.next_hit(&mut self.rand, &self.tp, &hit, now, set, part, at, vel, n).iter().map(shottask::text)),
            None => self.shot_task = Some((ShotTask::new(&hit, set, part, at, vel, n), self.steps)),
        }
        self.send_ours(o);
    }

    fn hit(&self) -> Hit {
        Hit { health: self.health, ..Hit::ragdoll(&self.agent) }
    }

    fn now(&self) -> u32 {
        (self.steps as f64 * 1000.0 / 60.0) as u32
    }

    fn shot_frame(&mut self) {
        if self.health <= 0.0 && !self.dead {
            self.dead = true;
            (self.shot_task, self.blast_task) = (None, None);
            self.button("STOP all");
            self.button("relax");
            self.say("room  dead (health 0): limp; Reset pose brings him back".into());
        }
        let (mut t, made) = match self.shot_task.take() {
            Some((t, made)) if self.steps >= made + 2 => (t, made),
            other => return self.shot_task = other,
        };
        let (hit, now) = (self.hit(), self.now());
        let step = t.update(&mut self.rand, &self.tp, &hit, now);
        for m in &step.msgs {
            self.send(&shottask::text(m));
        }
        if step.fire {
            self.say("ours  blind fire shot (no gun in the room)".into());
        }
        if step.done {
            self.say("room  the shot task finished".into());
        } else {
            self.shot_task = Some((t, made));
        }
    }

    fn light(&mut self, ped: usize, part: usize, kind: (LinearRgba, f32)) {
        if self.glow.len() <= ped {
            self.glow.resize(ped + 1, vec![]);
        }
        let glow = &mut self.glow[ped];
        if glow.len() <= part {
            glow.resize(part + 1, (0.0, TOUCH_GLOW));
        }
        let g = &mut glow[part];
        if g.0 <= 0.0 || g.1 .1 >= kind.1 || g.0 < 0.5 {
            *g = (1.0, kind);
        }
    }

    fn mark(&mut self, ped: usize, part: usize, at: Vec3, n: Vec3) {
        self.light(ped, part, SHOT_GLOW);
        self.sounds.push(sfx::Snd::Hit);
        self.bleed(at, n);
        let doll = if ped == 0 { Some(&self.agent.doll) } else { self.crowd.get(ped - 1).map(|c| &c.0.doll) };
        if let Some(f) = doll.and_then(|d| d.frames().nth(part)) {
            let back = f.rotation.inverse();
            self.rings.push((ped, part, back * (at - f.translation), back * n, 0.0));
        }
    }

    fn cut(&mut self, part: usize, at: Vec3, n: Vec3, dmg: f32) {
        self.light(0, part, SHOT_GLOW);
        self.bleed_some(at, n, if WEAPONS[self.weapon].0 == "bat" { BAT_BLOOD } else { KNIFE_BLOOD });
        self.pop(at, dmg);
        if let Some(f) = self.agent.doll.frames().nth(part) {
            let back = f.rotation.inverse();
            self.rings.push((0, part, back * (at - f.translation), back * n, 0.0));
        }
    }

    fn bleed(&mut self, at: Vec3, n: Vec3) {
        self.bleed_some(at, n, BLOOD_PIX);
    }

    fn bleed_some(&mut self, at: Vec3, n: Vec3, count: usize) {
        if !self.pixel_blood {
            return;
        }
        for _ in 0..count {
            let mut r = || {
                self.fx_seed = self.fx_seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (self.fx_seed >> 8) as f32 / (1u32 << 24) as f32
            };
            let v = n * (0.8 + r() * 1.6) + Vec3::new(r() - 0.5, r() - 0.5, r() * 0.8 + 0.4) * 1.6;
            let shade = (r() * 3.0) as u8;
            self.blood.push((at + n * 0.02, v, 0.0, shade));
        }
        if self.blood.len() > BLOOD_MAX {
            self.blood.drain(..self.blood.len() - BLOOD_MAX);
        }
    }

    fn injure(&mut self, part: usize, by: f32) {
        let Some(l) = [(13..=16, 0), (17..=20, 1), (1..=3, 2), (4..=6, 3)].into_iter().find(|r| r.0.contains(&part)).map(|r| r.1) else { return };
        self.hurt[l] = (self.hurt[l] + by).min(INJURY_MAX);
        let line = format!("injury mask={} amount={:.2}", ["ul", "ur", "ll", "lr"][l], self.hurt[l]);
        self.send(&line);
    }

    fn pop(&mut self, at: Vec3, amount: f32) {
        if amount >= 0.5 {
            self.pops.push((at, amount, 0.0, false));
        }
    }

    fn crowd_shot(&mut self, i: usize, part: usize, at: Vec3, dir: Vec3, n: Vec3, fatal: bool, hurt: bool) {
        self.mark(i, part, at, n);
        if !hurt || i == 0 || i > self.crowd.len() {
            return;
        }
        let name = WEAPONS[self.weapon].1;
        let w = self.weapons.iter().find(|w| w.name == name).cloned();
        let hp = self.crowd_hp.get(i - 1).copied().unwrap_or(200.0);
        let lost = if fatal { hp } else {
            w.as_ref().map_or(25.0, |w| {
                use delirium_data::ped;
                let mut h = ped::Health { health: hp, ..Default::default() };
                let hit = ped::Hit::new(w.damage, w, delirium_data::nm_asset::PART_BONES[part], ped::Attacker::Player { sure: true });
                ped::damage(&mut h, &hit, &ped::Victim::default(), &mut self.rand).lost
            })
        };
        let left = (hp - lost).max(0.0);
        if hp > 0.0 && left <= 0.0 {
            self.sounds.push(sfx::Snd::Kill);
        }
        if let Some(h) = self.crowd_hp.get_mut(i - 1) {
            *h = left;
        }
        self.pop(at, lost);
        let vel = shottask::bullet(at, at - dir.normalize() * 3.0, w.as_ref().map_or(WEAPONS[self.weapon].2, |w| w.force));
        let mut o = vec![format!("applyImpulse partIndex={part} hitPoint={} impulse={}", v3(at), v3(vel)),
                         format!("shot start=true bodyPart={part} hitPoint={} normal={} bulletVel={}", v3(at), v3(n), v3(vel))];
        if left <= 0.0 {
            o.push("bodyRelax start=true".into());
        }
        for l in o {
            self.send_to(i, &l);
        }
        self.say(format!("room  shot crowd ped {i}: -{lost:.0} -> health {left:.0}{}", if left <= 0.0 { " (dead)" } else { "" }));
    }

    fn poke(&mut self, part: usize, hit: Vec3, dir: Vec3) {
        let line = |at: Vec3, d: Vec3| vec![format!("applyImpulse start=true partIndex={part} hitPoint={} impulse={}", v3(at), v3(d.normalize() * self.poke))];
        let o = line(hit, dir);
        self.light(0, part, TOUCH_GLOW);
        self.sounds.push(sfx::Snd::Hit);
        self.say(format!("room  poke {} ({:.1} N s)", delirium_data::nm_asset::PARTS.get(part).unwrap_or(&"?"), self.poke));
        self.send_ours(o);
    }

    fn pull(&mut self) {
        let Some((part, local, target)) = self.grab.as_ref().filter(|g| g.pulling).map(|g| (g.part, g.local, g.target)) else { return };
        let Some(f) = self.agent.doll.frames().nth(part) else { return };
        let at = f.translation + f.rotation * local;
        let (v, w) = self.agent.doll.vel(part);
        let damp = (2.0 * (PULL_K * self.agent.doll.mass()).sqrt()).min(30.0 * self.agent.doll.part_mass(part));
        let imp = (PULL_K * (target - at) - damp * (v + w.cross(at - f.translation))).clamp_length_max(PULL_MAX) / 60.0;
        self.held = false;
        let ps = [("start", Value::Bool(true)), ("partIndex", Value::Int(part as i32)), ("hitPoint", Value::Vec3(at)), ("impulse", Value::Vec3(imp))];
        let _ = msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, "applyImpulse", &ps);
    }

    fn cue(&mut self) {
        for l in std::mem::take(&mut self.on_cue).into_iter().rev() {
            self.script.push_front((0.0, l));
        }
    }

    fn button(&mut self, b: &str) {
        match b {
            "camera" => {
                self.relock = true;
                return;
            }
            "watch training" | "watch walk training" | "watch run training" => {
                let path = if b == "watch training" { learn::path() } else { learn::walk_path(b == "watch run training") };
                if self.watch.as_ref().is_some_and(|w| w.path == path) {
                    self.watch = None;
                    return self.say("training  watch: off".into());
                }
                let mut w = Watch { path, stamp: None, generation: 0, scores: vec![], tries: vec![], t: 0.0, check: 0.0 };
                let got = w.reload();
                self.watch = Some(w);
                return self.say(format!("training  {b}: {}", if got { "on (each generation's 8 tries, side by side)" } else { "on, waiting for the trainer" }));
            }
            "walk learned" | "run learned" => return self.start_walk_learned(b == "run learned"),
            "infinite health" => {
                (self.god, self.health) = (!self.god, 200.0);
                return self.say(format!("room  infinite health: {}", if self.god { "on (hits never hurt Homer, both sides)" } else { "off" }));
            }
            "homer weapon" => {
                let next = match self.homer_weapon {
                    None => Some(0),
                    Some(w) => HOMER_ARMS.iter().position(|&n| n == WEAPONS[w].0).map(|i| i + 1).filter(|&i| i < HOMER_ARMS.len()),
                };
                self.homer_weapon = next.and_then(|i| WEAPONS.iter().position(|w| w.0 == HOMER_ARMS[i]));
                return self.say(format!("room  Homer fights with: {}", self.homer_weapon.map_or("his fists", |w| WEAPONS[w].0)));
            }
            "super punch" => {
                self.super_punch = !self.super_punch;
                return self.say(format!("room  super punch: {}", if self.super_punch { "on (the Hero's punches go off like a grenade)" } else { "off" }));
            }
            "hero physics" => {
                self.hero_physics = !self.hero_physics;
                return self.say(format!("room  Hero physics: {}", if self.hero_physics { "on (hits put him on his ragdoll)" } else { "off (hit animations)" }));
            }
            "passive" => {
                self.passive = !self.passive;
                return self.say(format!("room  Homer passive: {}", if self.passive { "on (he only takes hits)" } else { "off (near the Hero he fights back)" }));
            }
            "walk" => {
                self.walk = !self.walk;
                return self.say(format!("ours  walk: {}", if self.walk { "on (WASD leans his balance; he steps to follow)" } else { "off (C selects free camera)" }));
            }
            "get-up" => {
                self.getup_mode = (self.getup_mode + 1) % 4;
                return self.say(format!("ours  get-up: {}", GETUP_MODES[self.getup_mode as usize]));
            }
            _ => {}
        }
        if self.dead && BEHAVIOURS.contains(&b) && !matches!(b, "STOP all" | "relax") {
            return self.say(format!("room  {b}: he's dead (health 0); Reset pose brings him back"));
        }
        (self.rises, self.still, self.nm_t, self.musc, self.learned) = ([None, None], [0.0; 2], [None, None], None, None);
        self.walk_learned = None;
        if b == "RESET pose" {
            (self.health, self.lean_stop, self.shot_task, self.dead) = (200.0, None, None, false);
            self.auto_balance = self.script.is_empty();
            self.reset();
            return;
        }
        if b == "shot" {
            self.fire(CHEST, None, false);
            return;
        }
        if b == "shot back" {
            let s = self.ours();
            self.fire(CHEST, Some((s.p(CHEST) - s.fwd * 0.1, s.fwd, -s.fwd)), false);
            return;
        }
        if b == "replay" {
            self.replay = if self.replay.is_some() || self.tape.is_empty() { None } else { Some(0.0) };
            let s = if self.replay.is_some() { format!("room  replay: the last {:.1} s (the speed buttons slow it; again: stop)", self.tape.len() as f32 / 60.0) } else { "room  replay: off".into() };
            return self.say(s);
        }
        if b == "explosion pile" {
            return self.pile();
        }
        if b == "wrecking ball" {
            return self.wrecking_ball();
        }
        if let Some(kind) = world::KINDS.iter().position(|k| b.ends_with(k.0)) {
            self.throw(kind);
            return;
        }
        match b {
            "look" | "avoid" | "game tasks" => {
                let on = if b == "look" { &mut self.look } else if b == "avoid" { &mut self.avoid } else { &mut self.tasks };
                *on = !*on;
                let s = format!("room  {b}: {}", if *on { "on" } else { "off" });
                return self.say(s);
            }
            "crowd" => return self.crowd(),
            "grid" => return self.grid(),
            "car" => return self.car(),
            "push front" => return self.push(true),
            "push back" => return self.push(false),
            "taxi" => {
                while self.taxis.len() < TAXIS {
                    self.spawn_taxi();
                }
                return;
            }
            "add taxi" => return self.spawn_taxi(),
            "clear taxis" => {
                (self.taxis, self.driving) = (vec![], false);
                return self.say("room  taxis: gone".into());
            }
            "ice" => return self.ice(),
            "fall" => return self.fall(),
            "clear" => {
                self.room.clear();
                return self.say("room  clear".into());
            }
            _ => {}
        }
        self.say(format!("room  {b}"));
        if b == "lean" || b == "drunk" {
            self.lean_stop = Some(self.steps as f32 / 60.0 + LEAN_SECS);
        }
        let o = self.lines(b, &self.ours());
        self.send_ours(o);
        if let Some(e) = exit(b).filter(|_| self.tasks) {
            (self.exit, self.nm_t, self.failed) = (e, [Some(0.0); 2], [false; 2]);
        }
    }

    fn fire_from(&mut self, from: Vec3, dir: Vec3, fatal: bool) {
        #[cfg(feature = "portal")]
        if self.portal.gun {
            return;
        }
        if WEAPONS[self.weapon].0 == "rpg" {
            self.sounds.push(sfx::Snd::Shot(self.weapon));
            return self.gun.rockets.push((from, dir * ROCKET_SPEED, 0.0));
        }
        let hit = self.bullet_any(from, dir);
        match hit {
            Some((0, (part, at, normal))) => self.fire(part, Some((at, dir, normal)), fatal),
            Some((i, (part, at, normal))) => self.crowd_shot(i, part, at, dir, normal, fatal, true),
            None => self.near_miss(from, dir),
        }
    }

    fn bullet_any(&mut self, from: Vec3, dir: Vec3) -> Option<(usize, (usize, Vec3, Vec3))> {
        self.sounds.push(sfx::Snd::Shot(self.weapon));
        let hit = std::iter::once(&self.agent).chain(self.crowd.iter().map(|c| &c.0)).enumerate()
            .filter_map(|(i, a)| a.doll.probe(&self.build, from, from + dir * 60.0).map(|h| (i, h)))
            .min_by(|a, b| a.1 .1.distance(from).total_cmp(&b.1 .1.distance(from)));
        let end = hit.map(|h| h.1 .1).or_else(|| self.world.contact(from, dir, 60.0).map(|c| from + dir * c.0)).unwrap_or(from + dir * 60.0);
        self.gun.tracers.push((from, end, 0.25));
        hit
    }

    fn gun_stats(&self) -> (f32, f32, usize, f32) {
        let (label, name, _) = WEAPONS[self.weapon];
        let (between, acc, case) = GUN_FALLBACK[self.weapon];
        let w = self.weapons.iter().find(|w| w.name == name);
        let shotgun = w.map_or(label == "shotgun", |w| w.slot == "SHOTGUN");
        (w.map_or(between, |w| w.between_shots), w.map_or(acc, |w| if w.slot == "SNIPER" { w.accuracy_fps } else { w.accuracy }), if shotgun { PELLETS } else { 1 }, case)
    }

    fn gun_spread(&mut self, len: f32, acc: f32) -> (f32, f32) {
        let full = match WEAPONS[self.weapon].0 { "pistol" | "deagle" => 3.3, "smg" => 1.5, "ak47" => 3.0, _ => f32::INFINITY };
        let ped = 2.0 * (self.gun.held / full).min(1.0);
        let p = ped * (0.1 + self.gun.rnd());
        let w = self.gun.rnd() * acc * if p < 0.5 { (2.0 * p).max(0.2) } else { 1.0 };
        let most = ped * 1.1 + acc * if ped * 1.1 < 0.5 { (2.2 * ped).max(0.2) } else { 1.0 };
        (0.02 * len * (p + w), 0.02 * len * most)
    }

    fn gun_fire(&mut self, fatal: bool) {
        #[cfg(feature = "portal")]
        if self.portal.gun {
            return;
        }
        let (_, acc, pellets, case) = self.gun_stats();
        let from = self.gun.at + self.gun.dir * GUN_MODELS[self.weapon].1;
        match kind(self.weapon) {
            1 => return self.throw_weapon(from, self.gun.aim),
            2 => {
                if self.gun.swing <= 0.0 {
                    self.gun.swing = swing_len(self.weapon);
                }
                return;
            }
            _ => {}
        }
        if WEAPONS[self.weapon].0 == "rpg" {
            self.sounds.push(sfx::Snd::Shot(self.weapon));
            self.gun.rockets.push((from, self.gun.dir * ROCKET_SPEED, 0.0));
            (self.gun.kick, self.gun.flash) = (KICK[self.weapon], 0.08);
            return;
        }
        let len = from.distance(self.gun.aim).max(1.0);
        let (r, _) = self.gun_spread(len, acc);
        let (right, up) = self.gun.dir.any_orthonormal_pair();
        let t = self.gun.rnd() * std::f32::consts::TAU;
        let dir = (self.gun.dir * len + (right * t.sin() + up * t.cos()) * r).normalize();
        let (mut hit, mut hurt) = (None, vec![]);
        for _ in 0..pellets {
            let d = if pellets > 1 {
                let half = PELLET_SPREAD * acc;
                let (a, b) = (self.gun.rnd() * 2.0 - 1.0, self.gun.rnd() * 2.0 - 1.0);
                (dir + right * a * half + up * b * half).normalize()
            } else {
                dir
            };
            match self.bullet_any(from, d) {
                Some((0, h)) => match hit {
                    None => hit = Some((h, d)),
                    Some(_) => self.mark(0, h.0, h.1, h.2),
                },
                Some((i, h)) => {
                    let first = !hurt.contains(&i);
                    hurt.push(i);
                    self.crowd_shot(i, h.0, h.1, d, h.2, fatal, first);
                }
                None => {}
            }
        }
        match hit {
            Some(((part, at, normal), d)) => self.fire(part, Some((at, d, normal)), fatal),
            None if hurt.is_empty() => self.near_miss(from, dir),
            None => {}
        }
        self.gun.kick = KICK[self.weapon];
        self.gun.flash = 0.03 + 0.02 * KICK[self.weapon];
        let side = self.gun.dir.cross(Vec3::Z).normalize_or(Vec3::X);
        let v = side * (1.2 + self.gun.rnd()) + Vec3::Z * (1.5 + self.gun.rnd()) - self.gun.dir * 0.3;
        if case > 0.0 {
            self.gun.shells.push((self.gun.at + Vec3::Z * 0.03, v, 2.0));
        }
        if self.gun.shells.len() > 40 {
            self.gun.shells.remove(0);
        }
    }

    fn rocket_frame(&mut self, dt: f32) {
        let mut boom = vec![];
        let mut rockets = std::mem::take(&mut self.gun.rockets);
        rockets.retain_mut(|(p, v, age)| {
            *v += v.normalize_or(Vec3::Z) * ROCKET_SPEED * dt - Vec3::Z * 9.8 * dt;
            let (d, len) = (v.normalize_or(Vec3::Z), v.length() * dt);
            let hit = self.agent.doll.probe(&self.build, *p, *p + d * len).map(|h| h.1)
                .or_else(|| self.world.contact(*p, d, len).map(|c| *p + d * c.0));
            *age += dt;
            *p += d * len;
            match hit.or((*age > ROCKET_LIFE).then_some(*p)) {
                Some(at) => {
                    boom.push(at);
                    false
                }
                None => true,
            }
        });
        self.gun.rockets = rockets;
        for at in boom {
            self.explode(at);
        }
    }

    fn throw_weapon(&mut self, from: Vec3, to: Vec3) {
        let w = self.weapons.iter().find(|w| w.name == WEAPONS[self.weapon].1);
        let speed = w.map_or(THROW_SPEED, |w| if w.target_range > 0.0 { w.target_range } else { THROW_SPEED });
        let fuse = w.and_then(|w| w.projectile.as_ref()).filter(|p| p.kind == "FUSE" && p.fuse > 0.0).map_or(FUSE, |p| p.fuse / 1000.0);
        let molotov = WEAPONS[self.weapon].0 == "molotov";
        let d = to - from;
        let t = (d.length() / speed).max(0.05);
        self.gun.thrown.push((from, d / t + Vec3::Z * 4.905 * t, if molotov { f32::INFINITY } else { fuse }, molotov));
        self.gun.kick = KICK[self.weapon];
        self.sounds.push(sfx::Snd::Throw);
    }

    fn swing_weapon(&mut self) {
        let w = self.weapons.iter().find(|w| w.name == WEAPONS[self.weapon].1).cloned();
        let dir = (self.gun.aim - self.gun.at).normalize_or(Vec3::X);
        let Some((_, at, _)) = self.agent.doll.probe(&self.build, self.gun.at, self.gun.aim + dir * 0.3) else {
            self.sounds.push(sfx::Snd::Throw);
            return self.say(format!("room  {}: missed", WEAPONS[self.weapon].0));
        };
        let (dmg, push) = w.map_or((20.0, hero::MELEE_PUSH), |w| (w.damage, w.force.max(hero::MELEE_PUSH)));
        let j = if WEAPONS[self.weapon].0 == "bat" { (dir * 0.7 - dir.cross(Vec3::Z).normalize_or(Vec3::X)).normalize() * push * BAT_PUSH } else { dir * push };
        let what = format!("room  {} lands", WEAPONS[self.weapon].0);
        self.land(hero::Hit::Swing(dmg, at, j), self.gun.at, &what);
    }

    fn thrown_frame(&mut self, dt: f32) {
        let (mut boom, mut fire) = (vec![], vec![]);
        let mut thrown = std::mem::take(&mut self.gun.thrown);
        thrown.retain_mut(|(p, v, fuse, molotov)| {
            v.z -= 9.8 * dt;
            *fuse -= dt;
            let (d, len) = (v.normalize_or(Vec3::Z), v.length() * dt);
            let hit = self.agent.doll.probe(&self.build, *p, *p + d * len).map(|h| (h.1, -d))
                .or_else(|| self.world.contact(*p, d, len).map(|c| (*p + d * c.0, c.1)));
            match hit {
                Some((at, _)) if *molotov => {
                    fire.push(at);
                    return false;
                }
                Some((at, n)) => {
                    *p = at + n * 0.03;
                    *v = (*v - 2.0 * v.dot(n) * n) * BOUNCE;
                }
                None => *p += d * len,
            }
            if *fuse <= 0.0 {
                boom.push(*p);
                return false;
            }
            true
        });
        self.gun.thrown = thrown;
        for at in boom {
            self.explode(at);
        }
        for at in fire {
            let z = self.world.ground(at + Vec3::Z, 3.0).unwrap_or(at.z);
            self.gun.fires.push((at.with_z(z), 0.0));
            self.sounds.push(sfx::Snd::Crash);
            self.say("room  molotov: fire".into());
        }
    }

    fn fire_frame(&mut self, dt: f32) {
        for f in &mut self.gun.fires {
            f.1 += dt;
        }
        self.gun.fires.retain(|f| f.1 < FIRE_LIFE);
        let ours = self.ours();
        let caught = self.gun.fires.iter().any(|&(at, _)| (0..ours.parts.len()).map(|k| ours.p(k)).any(|p| (p - at).with_z(0.0).length() < FIRE_R && p.z - at.z < 1.0));
        if caught && !self.dead {
            self.burning = BURN;
        }
        if self.dead {
            (self.burning, self.on_fire) = (0.0, None);
        }
        if self.burning <= 0.0 && self.on_fire.is_none() {
            return;
        }
        self.burning -= dt;
        let burning = self.burning > 0.0;
        if burning {
            self.health = if self.god { 200.0 } else { (self.health - BURN_DAMAGE * dt).max(0.0) };
        }
        if burning && self.on_fire.is_none() {
            let mut f = delirium::nm::OnFire::new("*");
            f.start(&mut self.agent, &self.build, &self.world, &self.tp, &mut self.rand);
            self.on_fire = Some(f);
            self.say("room  Homer catches fire".into());
        }
        if let Some(f) = self.on_fire.as_mut() {
            if self.agent.feedback.iter().any(|f| f.slot == 8 && f.name == "balance") {
                f.balance_failed();
            }
            if f.update(&mut self.agent, &self.build, &self.world, &mut self.rand, dt, burning, self.health, self.health <= 0.0).is_some() {
                self.on_fire = None;
            }
        }
    }

    fn explode(&mut self, at: Vec3) {
        self.blast(at, true);
    }

    fn blast(&mut self, at: Vec3, hero: bool) {
        self.gun.blasts.push((at, 0.0));
        self.sounds.push(sfx::Snd::Boom);
        if hero {
            self.hero_blast(at);
        }
        let ours = self.ours();
        let near = (0..ours.parts.len()).map(|k| ours.p(k).distance(at)).fold(f32::MAX, f32::min);
        if near >= BLAST_R {
            return self.say(format!("room  blast: {near:.1} m from him"));
        }
        let mut o = vec![];
        for k in 0..ours.parts.len() {
            let p = ours.p(k);
            let f = 1.0 - p.distance(at) / BLAST_R;
            if f <= 0.0 {
                continue;
            }
            let imp = ((p - at).normalize_or(Vec3::Z) + Vec3::Z * 0.5).normalize() * self.agent.doll.part_mass(k) * BLAST_DV * f;
            o.push(format!("applyImpulse partIndex={k} hitPoint={} impulse={}", v3(p), v3(imp)));
        }
        let lost = BLAST_DAMAGE * (1.0 - near / BLAST_R);
        self.health = if self.god { 200.0 } else { (self.health - lost).max(0.0) };
        self.say(format!("room  blast: {near:.1} m from him: -{lost:.0} -> health {:.0}", self.health));
        self.shot_task = None;
        self.blast_task = Some(Default::default());
        self.send_ours(o);
    }

    fn blast_frame(&mut self) {
        let Some(mut t) = self.blast_task.take() else { return };
        let set = delirium::explosion::Set::read(&self.tp);
        let pel = self.ours().p(0);
        let (vz, near) = (self.agent.doll.vel(0).0.z, self.world.contact(pel, Vec3::NEG_Z, 2.0).is_some());
        let now = self.now();
        let m: Vec<String> = t.update(&set, &mut self.rand, now, vz, near).iter().map(shottask::text).collect();
        if !m.is_empty() {
            self.send_ours(m);
        }
        if now < t.started.unwrap_or(now) + 6000 {
            self.blast_task = Some(t);
        }
    }

    fn near_miss(&mut self, from: Vec3, dir: Vec3) {
        let ours = self.ours();
        let by = |p: Vec3| from + dir * (p - from).dot(dir).max(0.0);
        let (h, c) = (by(ours.p(HEAD)), by(ours.p(CHEST)));
        let (dh, dc) = (h.distance(ours.p(HEAD)), c.distance(ours.p(CHEST)));
        let (p, d) = if dh < dc { (h, dh) } else { (c, dc) };
        if d > WHIZZ {
            return self.say(format!("room  shot: missed by {d:.1} m (too far to notice)"));
        }
        self.say(format!("room  shot: near miss, {d:.2} m from him"));
        let now = self.steps as f32 / 60.0;
        if now - std::mem::replace(&mut self.near_at, now) < DODGE_GAP && !self.dead && !self.no_dodge {
            self.say("room  shot at again: he dives (shootDodge)".into());
            return self.send_ours(vec![format!("shootDodge start=true rightArmTarget={} leftArmTarget={}", v3(from), v3(from))]);
        }
        let msgs = |p: Vec3| vec![format!("upperBodyFlinch start=true pos={} newHit=true", v3(p)), format!("headLook start=true pos={} alwaysLook=true", v3(p))];
        let o = msgs(p);
        self.send_ours(o);
    }

    fn aim_at(&self, from: Vec3, dir: Vec3) -> Vec3 {
        let chest = self.ours().p(CHEST);
        self.agent.doll.probe(&self.build, from, from + dir * 60.0).map(|h| h.1).unwrap_or_else(|| {
            let near = from + dir * (chest - from).dot(dir).max(0.5);
            let (q, d) = self.agent.doll.frames().map(|f| (f.translation, f.translation.distance(near)))
                .fold((near, f32::MAX), |a, b| if b.1 < a.1 { b } else { a });
            near.lerp(q, (1.0 - d / SNAP).clamp(0.0, 1.0).powi(2) * 0.8)
        })
    }

    fn clip_size(&self) -> (u32, f32) {
        let w = self.weapons.iter().find(|w| w.name == WEAPONS[self.weapon].1);
        (w.map_or(CLIP_FALLBACK[self.weapon], |w| w.clip.max(1)), w.map_or(0.0, |w| w.reload / 1000.0))
    }

    fn hero_gun_frame(&mut self, aim: Vec3, dt: f32, pull: bool, fatal: bool, reload: bool) {
        if self.hero_doll.is_some() {
            return;
        }
        let Some(clips) = self.getup.as_ref() else { return };
        let Some((fire, rel)) = clips.guns.get(self.weapon).copied().flatten() else { return };
        let (size, mut secs) = self.clip_size();
        if secs <= 0.0 {
            secs = clips.duration(rel);
        }
        if reload && self.reloading.is_none() && self.ammo < size {
            self.reloading = Some(0.0);
        }
        if let Some(t) = self.reloading.as_mut() {
            *t += dt;
            if *t >= secs {
                (self.reloading, self.ammo) = (None, size);
            }
        }
        let up = aim + Vec3::Z * self.gun.kick * 0.12 * aim.distance(self.gun.at).max(1.0);
        let hold = match self.reloading {
            Some(t) => (rel, t / secs, up),
            None => (fire, 0.5, up),
        };
        let Some(n) = self.hero.as_mut() else { return };
        n.hold = Some(hold);
        let hand = n.hand;
        let want = (aim - hand).normalize_or(self.gun.dir);
        self.gun.dir = if self.gun.placed { self.gun.dir.lerp(want, 1.0 - (-20.0 * dt).exp()).normalize_or(want) } else { want };
        (self.gun.at, self.gun.aim, self.gun.placed) = (hand - self.gun.dir * 0.05, aim, true);
        if (pull || fatal) && self.reloading.is_none() && self.ammo > 0 && !portal_gun(self) {
            self.gun_fire(fatal);
            self.ammo -= 1;
            if self.ammo == 0 {
                self.reloading = Some(0.0);
            }
        }
    }

    fn hero_take(&mut self, w: Option<usize>) {
        #[cfg(feature = "portal")]
        {
            self.portal.gun = false;
        }
        self.hero_gun = w.is_some();
        if let Some(w) = w {
            self.weapon = w;
            self.ammo = self.clip_size().0;
        }
        self.reloading = None;
        if let Some(n) = self.hero.as_mut() {
            n.hold = None;
        }
        self.gun.placed = false;
        let has = kind(self.weapon) > 0 || self.getup.as_ref().is_some_and(|c| c.guns.get(self.weapon).copied().flatten().is_some());
        self.say(format!("hero  {}", if !self.hero_gun { "fists".to_string() } else if has { WEAPONS[self.weapon].0.to_string() } else { format!("{}: no clips for it", WEAPONS[self.weapon].0) }));
    }

    fn gun_frame(&mut self, ray: Option<(Vec3, Vec3)>, hold: bool, dt: f32) {
        let chest = self.ours().p(CHEST);
        if !self.gun.placed {
            let s = (self.eye - chest).with_z(0.0).normalize_or(Vec3::X);
            self.gun.at = chest + (s * 0.6 + Vec3::Z * 0.15 + Vec3::Z.cross(s) * 1.8) * self.gun_reach + Vec3::Z * self.gun_up;
            self.gun.dir = (chest - self.gun.at).normalize();
            self.gun.aim = chest;
            self.gun.placed = true;
            self.gun_off = self.gun.at - chest;
            self.gun_aim_off = Vec3::ZERO;
        }
        if ray.is_none() && self.film > 0.0 {
            (self.gun.at, self.gun.aim) = (chest + self.gun_off, chest + self.gun_aim_off);
        }
        let nearest = |doll: &delirium::ragdoll::Ragdoll, p: Vec3| doll.frames().map(|f| (f.translation, f.translation.distance(p)))
            .fold((p, f32::MAX), |a, b| if b.1 < a.1 { b } else { a });
        if let Some((from, dir)) = ray {
            if !hold {
                self.gun.at = from + dir * ((chest - from).dot(dir) - 0.5).max(0.5);
                self.gun.aim = nearest(&self.agent.doll, self.gun.at).0;
            } else if portal_gun(self) {
                self.gun.aim = from + dir * self.world.contact(from, dir, 200.0).map_or(50.0, |c| c.0);
            } else {
                self.gun.aim = self.aim_at(from, dir);
            }
        }
        let want = (self.gun.aim - self.gun.at).normalize_or(self.gun.dir);
        self.gun.dir = self.gun.dir.lerp(want, 1.0 - (-14.0 * dt).exp()).normalize_or(want);
    }

    fn shown(&self) -> Option<&Frame> {
        self.tape.get(self.replay? as usize)
    }

    fn on_sides(&self, p: Vec3) -> [Vec3; 2] {
        [p, p]
    }

    fn pile(&mut self) {
        let s = self.ours();
        let (f, l) = (s.fwd.with_z(0.0).normalize_or(Vec3::X), s.left().with_z(0.0).normalize_or(Vec3::Y));
        let at = s.p(0).with_z(0.0) + f * 2.5;
        for (x, y, k) in [(-0.35, 0.0, 3), (0.35, 0.0, 3), (-0.35, 0.7, 3), (0.35, 0.7, 3), (0.0, -0.75, 2), (0.0, 1.55, 2)] {
            let z = if k == 3 { world::BARREL_HALF } else { world::KINDS[2].1 } + 0.01;
            self.room.place(k, self.on_sides(at + l * x + f * (y - 0.35) + Vec3::Z * z), None);
        }
        self.pile_fuse = Some((PILE_FUSE, at + Vec3::Z * 0.3));
        self.held = false;
        self.say(format!("room  explosion pile: 4 barrels and 2 crates 2.5 m in front, blowing in {PILE_FUSE} s"));
    }

    fn wrecking_ball(&mut self) {
        let s = self.ours();
        let l = s.left().with_z(0.0).normalize_or(Vec3::Y);
        let top = s.p(0).with_z(6.6);
        self.room.place(4, self.on_sides(top + l * 5.0 - Vec3::Z * 2.0), Some(top));
        self.held = false;
        self.say("room  wrecking ball: 250 kg, from his left".into());
    }

    fn throw(&mut self, kind: usize) {
        self.sounds.push(sfx::Snd::Throw);
        let (label, _, kg, speed) = world::KINDS[kind];
        let speed = self.throw_speed.take().unwrap_or(speed);
        let ours = self.ours();
        let (from, vel) = if speed > 0.0 {
            let to = self.throw_at.unwrap_or(if self.aim_gun { self.gun.aim } else { ours.p(CHEST) });
            let from = if self.aim_gun { self.gun.at } else { self.eye.lerp(to, 0.15) };
            let d = to - from;
            let t = d.length() / speed;
            (from, d / t + Vec3::Z * 4.905 * t)
        } else {
            (self.throw_at.unwrap_or(ours.p(HEAD)) + Vec3::Z * 2.0, Vec3::ZERO)
        };
        let at = [from, from];
        self.room.throw(kind, at, vel);
        self.say(format!("room  {label} ({:.1} kg) {}", kg * self.room.weight, if speed > 0.0 { format!("thrown at {speed} m/s") } else { "dropped".into() }));
        let msgs = |p: Vec3, v: Vec3| {
            let mut m = vec![];
            if self.look {
                m.push(format!("headLook start=true pos={} vel={} instanceIndex=-1 alwaysLook=true", v3(p), v3(v)));
            }
            if self.avoid {
                m.push(format!("upperBodyFlinch start=true pos={} newHit=true", v3(p)));
            }
            m
        };
        let o = msgs(from, vel);
        self.send_ours(o);
    }

    fn objects(&mut self) {
        let caps = |fr: Vec<Transform>| fr.into_iter().zip(&self.build.parts).map(|(t, p)| (t, p.radius, p.half)).collect::<Vec<_>>();
        let ours = caps(self.agent.doll.capsules().collect());
        let pelvis = self.agent.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
        for j in 0..self.room.objs.len() {
            if self.room.objs[j].kind == 4 && !self.room.objs[j].hit[0] && self.room.obj(0, j).0.translation.distance(pelvis) > 1.5 {
                self.room.steer(j, pelvis);
            }
        }
        let sides = [Some(ours)];
        for (i, caps) in sides.into_iter().enumerate() {
            let Some(caps) = caps else { continue };
            for (j, k, at, imp) in self.room.step(i, &caps, 1.0 / 60.0) {
                if !self.room.objs[j].hit[i] && imp.length() > 1.0 {
                    self.room.objs[j].hit[i] = true;
                    self.say(format!("{}  {} hits {} ({:.1} N s)", WHO[i], world::KINDS[self.room.objs[j].kind].0,
                                     delirium_data::nm_asset::PARTS.get(k).unwrap_or(&"?"), imp.length()));
                    if i == 0 {
                        let amount = (imp.length() * 0.6).clamp(1.0, 35.0);
                        if !self.god {
                            self.health = (self.health - amount).max(1.0);
                        }
                        self.pops.push((at, amount, 0.0, true));
                        self.light(0, k, BRUISE_GLOW);
                        self.bleed_some(at, imp.normalize_or_zero(), KNOCK_BLOOD);
                        self.injure(k, (amount / 200.0).clamp(0.05, 0.2));
                    }
                }
                if imp.length() > 1.0 && self.homer.is_some() {
                    self.homer_out(false);
                    self.say("homer  knocked out of his fight".into());
                }
                self.held = false;
                let ps = [("start", Value::Bool(true)), ("partIndex", Value::Int(k as i32)), ("hitPoint", Value::Vec3(at)), ("impulse", Value::Vec3(imp))];
                let _ = msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, "applyImpulse", &ps);
            }
        }
    }

    fn send(&mut self, line: &str) {
        self.send_to(0, line)
    }

    fn send_to(&mut self, who: usize, line: &str) {
        if who == 0 {
            self.homer_out(false);
        }
        let mut w = line.split_whitespace();
        let Some(name) = w.next() else { return };
        if name == "?" {
            self.say(msg::MESSAGES.iter().chain(delirium::extras::messages()).map(|m| m.name).collect::<Vec<_>>().join(" "));
            return;
        }
        let Some(m) = msg::find(name) else {
            self.say(format!("no message {name}"));
            return;
        };
        let mut ps = vec![];
        for kv in w {
            if kv == "?" {
                for chunk in m.params.chunks(4) {
                    let s = chunk.iter().map(|p| format!("{}={}{}", p.name, show(msg::get(&self.msgs, m.name, p.name)), if p.used { "" } else { "*" }))
                        .collect::<Vec<_>>().join("  ");
                    self.say(s);
                }
                return;
            }
            let Some((k, v)) = kv.split_once('=') else {
                self.say(format!("{kv}: want name=value"));
                return;
            };
            let kind = if k == "start" { Some(Kind::Bool) } else { m.params.iter().find(|p| p.name == k).map(|p| p.kind) };
            let Some(kind) = kind else {
                self.say(format!("{name} has no param {k}"));
                return;
            };
            match parse(kind, v) {
                Some(v) => ps.push((k, v)),
                None => {
                    self.say(format!("{k}: not a {kind:?}"));
                    return;
                }
            }
        }
        let short: String = line.chars().take(110).collect();
        let (agent, st, tag) = match who {
            0 => (&mut self.agent, &mut self.msgs, String::new()),
            HERO => match self.hero_doll.as_mut() {
                Some(h) => (&mut h.agent, &mut h.st, "hero ".to_string()),
                None => return,
            },
            i => match self.crowd.get_mut(i - 1) {
                Some(c) => (&mut c.0, &mut c.1, format!("crowd {i} ")),
                None => return,
            },
        };
        match msg::send(agent, &self.build, &self.world, st, name, &ps) {
            Ok(unused) => self.say(format!("ours  {tag}MSG {short}{}", if unused > 0 { format!("  ({unused} kept, not used by the port)") } else { String::new() })),
            Err(e) => self.say(format!("ours  {name}: {e:?}")),
        }
    }
}

fn pretty(s: &str) -> String {
    let mut w: Vec<&str> = s.split_whitespace().collect();
    if w.len() > 1 && matches!(w[1], "MSG" | "FB") {
        w.swap(0, 1);
    }
    w.iter().map(|x| {
        let Some((k, v)) = x.split_once('=') else { return x.to_string() };
        let k = k.split(':').next().unwrap_or(k);
        let n: Vec<f32> = v.split(',').map_while(|n| n.parse().ok()).collect();
        let v = match n[..] {
            [x, y, z] if v.split(',').count() == 3 => format!("({x:.2},{y:.2},{z:.2})"),
            [x] if v.contains('.') => format!("{x:.2}"),
            _ => v.to_string(),
        };
        format!("{k}={v}")
    }).collect::<Vec<_>>().join(" ")
}

fn parse(kind: Kind, v: &str) -> Option<Value> {
    Some(match kind {
        Kind::Float => Value::Float(v.parse().ok()?),
        Kind::Int => Value::Int(v.parse().ok()?),
        Kind::Bool => Value::Bool(match v {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return None,
        }),
        Kind::Vec3 => {
            let f: Vec<f32> = v.split(',').map(|x| x.parse().ok()).collect::<Option<_>>()?;
            Value::Vec3(Vec3::new(*f.first()?, *f.get(1)?, *f.get(2)?))
        }
        Kind::Str => Value::Str(v.to_string()),
    })
}

fn show(v: Option<Value>) -> String {
    match v {
        Some(Value::Float(x)) => format!("{x:.3}").trim_end_matches('0').trim_end_matches('.').to_string(),
        Some(Value::Int(i)) => i.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Vec3(v)) => format!("{:.2},{:.2},{:.2}", v.x, v.y, v.z),
        Some(Value::Str(s)) => s,
        None => "-".into(),
    }
}

fn complete(line: &str) -> (String, Vec<String>) {
    let words: Vec<&str> = line.split(' ').collect();
    let last = *words.last().unwrap_or(&"");
    let cands: Vec<String> = if words.len() <= 1 {
        msg::MESSAGES.iter().chain(delirium::extras::messages()).map(|m| m.name.to_string()).filter(|n| n.starts_with(last)).collect()
    } else if let Some(m) = msg::find(words[0]) {
        m.params.iter().map(|p| format!("{}=", p.name)).chain(["start=".to_string()]).filter(|n| n.starts_with(last)).collect()
    } else {
        vec![]
    };
    let common = cands.first().map_or(last.to_string(), |f| {
        let mut p = f.clone();
        for c in &cands {
            while !c.starts_with(p.as_str()) {
                p.pop();
            }
        }
        p
    });
    let head = words[..words.len().saturating_sub(1)].join(" ");
    (if head.is_empty() { common } else { format!("{head} {common}") }, cands)
}

#[derive(Component)]
struct Orbit {
    yaw: f32,
    pitch: f32,
    dist: f32,
    target: Vec3,
    follow: u8,
}

fn common(game: &str, paths: &[&str]) -> Vec<Option<String>> {
    archive(game, "common.rpf", paths).into_iter().map(|d| d.map(|d| String::from_utf8_lossy(&d).into_owned())).collect()
}

fn archive(game: &str, rpf: &str, paths: &[&str]) -> Vec<Option<Vec<u8>>> {
    thread_local! {
        static READ: std::cell::RefCell<std::collections::HashMap<String, Vec<Option<Vec<u8>>>>> = Default::default();
    }
    let k = format!("{game}|{rpf}|{}", paths.join("|"));
    if let Some(v) = READ.with_borrow(|r| r.get(&k).cloned()) {
        return v;
    }
    let v = archive_read(game, rpf, paths);
    READ.with_borrow_mut(|r| r.insert(k, v.clone()));
    v
}

fn archive_read(game: &str, rpf: &str, paths: &[&str]) -> Vec<Option<Vec<u8>>> {
    let dir = std::path::Path::new(game);
    let Ok(key) = getup::key(dir) else { return vec![None; paths.len()] };
    if getup::pc() {
        let sub = if rpf == "common.rpf" { "common" } else { "pc" };
        return paths.iter().map(|p| std::fs::read(dir.join(sub).join(getup::plat(p))).ok()).collect();
    }
    let Ok(data) = std::fs::read(dir.join(getup::plat(rpf))) else { return vec![None; paths.len()] };
    let toc = delirium_data::rpf::read_toc(&data, &key).unwrap_or_default();
    paths.iter().map(|p| toc.iter().find(|e| e.path.eq_ignore_ascii_case(&getup::plat(p))).and_then(|e| delirium_data::rpf::read(&data, e).ok())).collect()
}

fn load(a: &[String], args: &[String], game_dir: Option<&str>, script: VecDeque<(f32, String)>, rec: Option<std::io::LineWriter<std::fs::File>>) -> Sim {
    let [wi, mats, personality]: [Option<String>; 3] = game_dir
        .map(|g| common(g, &["data/weaponinfo.xml", "data/materials/materials.dat", "data/pedpersonality.dat"]).try_into().unwrap()).unwrap_or_default();
    let (model, xrt) = if a.iter().any(|x| x == "--marge") { ("f_y_pmanhat_01", "wilma") } else { ("m_y_pmanhat_01", "fred") };
    NM.store(if xrt == "wilma" { delirium::nmrs::MARGE } else { delirium::nmrs::HOMER }, std::sync::atomic::Ordering::Relaxed);
    let agility = personality.as_deref().and_then(|t| t.lines().map(|l| l.split(',').map(str::trim).collect::<Vec<_>>())
        .find(|c| c.len() > 7 && c[0].eq_ignore_ascii_case(model)).and_then(|c| c[7].parse::<u8>().ok())).unwrap_or(3);
    let weapons: Vec<Weapon> = wi.map(|t| delirium_data::weapons::read(&t)).unwrap_or_default();
    let surfaces: Vec<(String, f32, f32)> = mats.as_deref().map(|t| {
        let f = delirium_data::vehicles::read_friction(t);
        *delirium::ragdoll::SURFACES.write().unwrap() = f.clone();
        t.lines().filter(|l| l.starts_with(|c: char| c.is_ascii_alphabetic())).zip(f).map(|(l, [fr, el])| (l.split_whitespace().next().unwrap_or("").to_string(), fr, el)).collect()
    }).unwrap_or_default();
    let room = world::Room::new(surfaces, a.iter().position(|x| x == "--stage").and_then(|i| a.get(i + 1)).map_or("room", |s| s.as_str()));
    let map = game_dir.zip(a.iter().position(|x| x == "--map")).and_then(|(g, i)| {
        let v: Vec<f32> = a[i + 1].split(',').filter_map(|x| x.parse().ok()).collect();
        (v.len() >= 2).then(|| map::load(g, Vec2::new(v[0], v[1]), SCENE_R).unwrap_or_else(|e| panic!("map: {e}")))
    });
    let pos: Vec<String> = a.iter().skip(1).take_while(|x| !x.starts_with("--")).cloned().collect();
    let arg = |i: usize, env: &str| pos.get(i - 1).cloned().or_else(|| std::env::var(env).ok());
    let from_game = |rpf: &str, p: &str| game_dir.and_then(|g| archive(g, rpf, &[p]).pop().flatten());
    let bpath = arg(2, "NM_BUILD");
    if game_dir.is_none() && bpath.is_none() && arg(1, "XRT").is_none() {
        pick_game(args);
    }
    let Some(d) = arg(1, "XRT").map(|x| std::fs::read(&x).unwrap_or_else(|e| panic!("{x}: {e}")))
        .or_else(|| from_game("xbox360.rpf", &format!("data/naturalmotion/{xrt}.xrt"))).filter(|_| bpath.is_some() || game_dir.is_some()) else {
        eprintln!("usage: delirium-room <fred.xrt> <build.txt> [taskparams.txt] [--game <game dir>] [--script file]   (or env XRT, NM_BUILD, TP;");
        eprintln!("       with --game and no XRT / NM_BUILD / TP, fred.xrt, Homer's body and taskparams.txt come out of the game's files)");
        std::process::exit(2);
    };
    delirium::nmrs::load(nm(), &d);
    let (build, frames, getup) = match (&bpath, game_dir) {
        (Some(p), _) => {
            let (b, f) = Build::undump(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}")));
            let getup = game_dir.and_then(|g| getup::Clips::load(g, &b).map_err(|e| eprintln!("get-up clips: {e}")).ok());
            (b, f, getup)
        }
        (None, Some(g)) => PEDS.with_borrow_mut(|peds| peds.entry(model).or_insert_with(|| {
            let mut b = getup::homer(g, model, nm()).unwrap_or_else(|e| panic!("Homer from the game's files: {e}"));
            let clips = getup::Clips::load(g, &b).unwrap_or_else(|e| panic!("the game's clips: {e}"));
            let f = clips.stand_up(&mut b);
            (b, f, Some(clips))
        }).clone()),
        (None, None) => unreachable!(),
    };
    if let Ok(p) = std::env::var("NM_DUMP") {
        let _ = std::fs::write(p, build.dump(&frames));
    }
    let floor = build.parts.iter().zip(&frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
    let shift = room.start - frames[0].translation.with_z(floor);
    let frames: Vec<Transform> = frames.iter().map(|f| Transform { translation: f.translation + shift, ..*f }).collect();
    let mut world = Collision::default();
    let concrete = room.surfaces.iter().position(|s| s.0 == "CONCRETE").unwrap_or(0) as u8;
    world.add_floor(0.0, 0.0, 0.0, 50.0, concrete);
    let (p, t, m) = room.triangles();
    world.add(&p, &t, &m);
    if let Some(i) = room.snap {
        let (p, t, m) = room.block_triangles(i);
        world.add_chunk(SNAP_CHUNK, &p, &t, &m);
    }
    let tp = arg(3, "TP").and_then(|p| std::fs::read_to_string(p).ok())
        .or_else(|| from_game("common.rpf", "data/taskparams.txt").map(|d| String::from_utf8_lossy(&d).into_owned())).map_or_else(TaskParams::default, |t| delirium_data::taskparams::read(&t));
    let agent = Agent::new(&build, Ragdoll::new(&build, &frames, Vec3::ZERO), Quat::IDENTITY, 0.3);
    let mut sim = Sim { build, frames, world, tp, agent, msgs: msg::State::default(), paused: false, speed: 1.0, acc: 0.0, steps: 0, arrows: false,
                        shot_task: None, rand: Default::default(),
                        log: VecDeque::new(), console: String::new(), script, typing: false,
                        weapons, weapon: 0, weak: false, sel: 0, over_ui: false, held: true,   show_ui: true, sens: 0.5, rec, health: 200.0, god: false, aim_gun: false, gun: Gun::default(), blast_task: None, burning: 0.0, on_fire: None, hero_gun: false, passive: true, homer: None, homer_weapon: None, homer_wait: None, homer_rest: 0.0, ammo: 0, reloading: None, taxis: vec![], driving: false, pedals: (0.0, 0.0, false), car_tune: None, lean_stop: None, hurt: [0.0; 4], near_at: -9.0, no_dodge: false, super_punch: false, crush: None, cab_gas: 0.0, cam: None, look_z: 0.8, film: 0.0, photo: false, soft: false, gun_reach: 1.0, gun_up: 0.0, gun_off: Vec3::ZERO, gun_aim_off: Vec3::ZERO, crane: None, crane_to: None, shadow: 0.0, strict: false, film_vel: Vec3::ZERO, photo_pending: false, on_cue: vec![], photo_follow: false, cam_at: None, shot_scene: String::new(), photo_note: String::new(), script_walk: Vec3::ZERO, throw_speed: None, homer_guard: None, homer_shot: 0.0, hero_physics: true, hero_doll: None, #[cfg(feature = "portal")] portal: portal::Portals::new(),
                        getup, rises: [None, None], still: [0.0; 2], tasks: false, exit: SHOT_EXIT, nm_t: [None, None], failed: [false; 2], last: [Vec3::ZERO; 2], room, look: true, avoid: false, eye: Vec3::new(0.0, -4.0, 1.5), hover: None, map, crowd: vec![], group: Default::default(), cars: vec![], lanes: None, pusher: None, pile_fuse: None, ice: None,   click_shoots: false,
                        grab: None, glow: vec![], rings: vec![], ripples: vec![], ripple_wait: vec![], tape: Default::default(), replay: None, throw_at: None, pops: vec![], pop_screen: vec![], show_pops: true, tab: 1, sounds: vec![], sound: true, blood: vec![], pixel_blood: true, show_ripples: false, infinite: false, fx_seed: 1, pointer: None, crowd_hp: vec![], car_hit: vec![], car_reacts: vec![], help: !help_seen(), auto_balance: true, dead: false, hero: None, agility,
                        scene: None, switch_to: None, poke: POKE, game_dir: game_dir.map(str::to_string), walk: true, relock: false, cam_locked: true, walk_dir: Vec3::ZERO, wander: Default::default(), getup_mode: 0, musc: None, learned: None, walk_learned: None, watch: None, skin: saved_look(), ped_color: 0, eyes: true, brows: true, skel: true };
    sim.auto_balance = sim.script.is_empty();
    sim
}

thread_local! {
    static PEDS: std::cell::RefCell<std::collections::HashMap<&'static str, (Build, Vec<Transform>, Option<getup::Clips>)>> = Default::default();
}

static NM: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(delirium::nmrs::HOMER);
fn nm() -> usize {
    NM.load(std::sync::atomic::Ordering::Relaxed)
}

fn main() {
    let mut a: Vec<String> = std::env::args().collect();
    let args = a.clone();
    let mut opt = |flag: &str| {
        a.iter().position(|x| x == flag).map(|i| {
            let f = a.get(i + 1).cloned().unwrap_or_default();
            a.drain(i..(i + 2).min(a.len()));
            f
        })
    };
    let headless_steps = opt("--headless").map(|n| n.parse().unwrap_or(300));
    let script = opt("--script").map(|f| {
        let text = std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("{f}: {e}"));
        let mut lines: Vec<(f32, String)> = text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty())
            .filter_map(|l| { let (t, rest) = l.split_once(' ')?; Some((t.parse().ok()?, rest.trim().to_string())) }).collect();
        lines.sort_by(|x, y| x.0.total_cmp(&y.0));
        VecDeque::from(lines)
    }).unwrap_or_default();
    let rec = opt("--rec").map(|f| {
        let mut w = std::io::LineWriter::new(std::fs::File::create(&f).unwrap_or_else(|e| panic!("{f}: {e}")));
        let _ = w.write_all(b"# game: frame t pelvis xyz |v| touches | ours: t com xyz |v|   (# lines: the log)\n");
        w
    });
    let game_dir = opt("--game").map(|g| if !is_iso(&g) { g } else {
        let f = from_iso(&g, &|done, all| println!("Copying the game's files out of the .iso, once: {}%", done * 100 / all.max(1)))
            .unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) });
        let _ = std::fs::write(GAME_PATH, &f);
        f
    }).or_else(|| std::fs::read_to_string(GAME_PATH).ok().map(|t| t.trim().to_string()).filter(|g| is_game(g)));
    if let (Some(g), true) = (game_dir.as_deref(), a.iter().any(|x| x == "--find-stairs")) {
        let meshes = map::load(g, Vec2::ZERO, 0.0).unwrap_or_else(|e| panic!("map: {e}"));
        for (c, n, lo, hi) in map::stairs(&meshes).iter().take(60) {
            println!("{:8.1} {:8.1} {:6.1}  {n:4} tris  rise {:.1} m", c.x, c.y, c.z, hi - lo);
        }
        return;
    }
    if let (Some(g), Some(i)) = (game_dir.as_deref(), a.iter().position(|x| x == "--find-spots")) {
        let v: Vec<f32> = a[i + 1].split(',').filter_map(|x| x.parse().ok()).collect();
        let meshes = map::load(g, Vec2::new(v[0], v[1]), v[2] + 60.0).unwrap_or_else(|e| panic!("map: {e}"));
        let mut seen: Vec<(&str, Vec3)> = vec![];
        for (what, p, h, high) in map::spots(&meshes, Vec2::new(v[0], v[1]), v[2]) {
            if seen.iter().any(|s| s.0 == what && s.1.distance(p) < 6.0) {
                continue;
            }
            seen.push((what, p));
            println!("{what:6} {:.2},{:.2},{:.2} heading {h:.0}  {high:.2} m", p.x, p.y, p.z);
        }
        return;
    }
    if let (Some(g), Some(i)) = (game_dir.as_deref(), a.iter().position(|x| x == "--dump-map")) {
        let v: Vec<f32> = a[i + 1].split(',').filter_map(|x| x.parse().ok()).collect();
        let (at, r) = (Vec2::new(v[0], v[1]), v[2]);
        let mut out = String::new();
        for m in map::load(g, at, r).unwrap_or_else(|e| panic!("map: {e}")) {
            for (k, t) in m.triangles.iter().enumerate() {
                let p = t.map(|i| m.positions[i as usize]);
                if p.iter().any(|q| Vec2::new(q[0], q[1]).distance(at) < r) {
                    out += &format!("{} {} {}
", p.iter().flatten().map(|x| format!("{x:.3}")).collect::<Vec<_>>().join(" "), m.flags[k], m.materials[k]);
                }
            }
        }
        std::fs::write(&a[i + 2], out).unwrap();
        return;
    }
    let mut sim = load(&a, &args, game_dir.as_deref(), script, rec);
    let scene_var = "DELIRIUM_SCENE";
    sim.scene = std::env::var(scene_var).ok();
    if let Some(i) = a.iter().position(|x| x == "--learn-walk" || x == "--learn-run") {
        let run = a[i] == "--learn-run";
        let gens = a.get(i + 1).and_then(|n| n.parse().ok()).unwrap_or(100);
        let path = a.get(i + 2).cloned().unwrap_or_else(|| learn::walk_path(run));
        let result = sim.getup.as_ref().ok_or_else(|| "movement clips unavailable: use or PC game files".into())
            .and_then(|clips| learn::train_walk(&sim.build, &sim.world, &sim.frames, clips, gens, &path, run));
        if let Err(e) = result { eprintln!("learn: {e}"); std::process::exit(1); }
        return;
    }
    if let Some(i) = a.iter().position(|x| x == "--learn") {
        let gens = a.get(i + 1).and_then(|n| n.parse().ok()).unwrap_or(100);
        return learn::train(&sim.build, &sim.world, &sim.frames, gens, &a.get(i + 2).cloned().unwrap_or_else(learn::path));
    }
    if let Some(n) = headless_steps {
        return headless(sim, n);
    }
    sim.say(format!("weaponinfo.xml: {} weapons{}", sim.weapons.len(), if sim.weapons.is_empty() { " (--game <dir> reads it; impulses 50 N s)" } else { "" }));

    let systems = (keys, console, viewports, mouse, scripted, step, switch, scene, apply_look, follow_floor, bodies, face_view, props, car_mesh, pusher_mesh, taxi_mesh, gun_view, draw, smooth).chain();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Delirium Physics Playroom".into(), resolution: (1600, 900).into(), resize_constraints: WindowResizeConstraints { min_width: 960.0, min_height: 640.0, ..default() }, ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(bevy::winit::WinitSettings { focused_mode: bevy::winit::UpdateMode::Continuous, unfocused_mode: bevy::winit::UpdateMode::Continuous })
        .insert_resource(ClearColor(Color::srgb(0.78, 0.79, 0.81)))
        .init_gizmo_group::<OursGizmos>()
        .insert_non_send_resource(sim)
        .add_systems(Startup, setup)
        .add_systems(Update, systems)
        .add_systems(Update, (sounds, blood_view, planks))
        .add_systems(EguiPrimaryContextPass, ui);
    sfx::plugin(&mut app);
    #[cfg(feature = "portal")]
    portal::plugin(&mut app);
    app.world().resource::<bevy::asset::io::embedded::EmbeddedAssetRegistry>()
        .insert_asset(std::path::PathBuf::new(), std::path::Path::new("taxi.glb"), include_bytes!("taxi.glb").as_slice());
    app.run();
}

const GAME_PATH: &str = "game_path.txt";

fn is_game(dir: &str) -> bool {
    let d = std::path::Path::new(dir);
    d.join("default.xex").is_file() || d.join("ps3.rpf").is_file() && (d.join("eboot.elf").is_file() || d.join("EBOOT.BIN").is_file())
        || d.join("GTAIV.exe").is_file() && d.join("pc").is_dir()
}

fn is_iso(path: &str) -> bool {
    path.to_lowercase().ends_with(".iso") && std::path::Path::new(path).is_file()
}

fn ps3_disc_key(disc: &mut delirium_data::disc::Disc, iso: &str) -> bool {
    let hex = |s: &str| -> Vec<[u8; 16]> {
        let b = s.as_bytes();
        (0..b.len().saturating_sub(31)).filter(|&i| b[i..i + 32].iter().all(u8::is_ascii_hexdigit)
            && (i == 0 || !b[i - 1].is_ascii_hexdigit()) && b.get(i + 32).is_none_or(|c| !c.is_ascii_hexdigit()))
            .map(|i| std::array::from_fn(|j| u8::from_str_radix(&s[i + j * 2..i + j * 2 + 2], 16).unwrap())).collect()
    };
    if let Ok(k) = std::fs::read_to_string(std::path::Path::new(iso).with_extension("dkey")) {
        if hex(&k).into_iter().any(|k| disc.set_disc_key(k)) {
            return true;
        }
    }
    let Some(serial) = disc.ps3_serial().map(str::to_string) else { return false };
    let get = |url: &str| std::process::Command::new("curl").args(["-sSfL", url]).output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let found = get(&format!("http://redump.org/discs/quicksearch/{serial}/"));
    let mut pages: Vec<String> = found.split("/disc/").skip(1).filter_map(|t| t.split('/').next())
        .filter(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit())).map(String::from).collect();
    pages.dedup();
    for page in std::iter::once(found).chain(pages.iter().map(|n| get(&format!("http://redump.org/disc/{n}/")))) {
        if let Some(at) = page.find("Disc Key") {
            if hex(&page[at..(at + 400).min(page.len())]).into_iter().any(|k| disc.set_disc_key(k)) {
                return true;
            }
        }
    }
    false
}

fn from_iso(iso: &str, say: &dyn Fn(u64, u64)) -> Result<String, String> {
    let mut disc = delirium_data::disc::Disc::open(std::path::Path::new(iso))?;
    if disc.needs_disc_key() && !ps3_disc_key(&mut disc, iso) {
        return Err(format!("{iso}: this PS3 disc image is encrypted and its key wasn't found (put its .dkey file beside it, \
            same name, or check the connection)"));
    }
    let out = std::path::absolute("game").map_err(|e| e.to_string())?;
    let (root, prog) = if disc.ps3_serial().is_some() { ("ps3_game/usrdir/", "eboot.bin") } else { ("", "default.xex") };
    let mut want: Vec<&str> = disc.paths().into_iter().filter_map(|p| Some((p, p.strip_prefix(root)?)))
        .filter(|(_, p)| *p == prog || (!p.contains('/') && p.ends_with(".rpf"))
            || ((p.starts_with("xbox360/") || p.starts_with("ps3/")) && p.ends_with(".img")))
        .map(|(p, _)| p).collect();
    want.sort();
    if !want.iter().any(|p| p.strip_prefix(root) == Some(prog)) {
        return Err(format!("{iso}: no {prog} on it (not the game's disc?)"));
    }
    let name = |p: &str| { let p = &p[root.len()..]; if p == "eboot.bin" { "EBOOT.BIN".to_string() } else { p.to_string() } };
    let todo: Vec<(&str, u64)> = want.iter().map(|p| (*p, disc.len(p).unwrap_or(0)))
        .filter(|(p, len)| !out.join(name(p)).metadata().is_ok_and(|m| m.len() == *len)).collect();
    let (all, mut done) = (todo.iter().map(|t| t.1).sum::<u64>(), 0);
    for (p, len) in todo {
        let to = out.join(name(p));
        std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
        let part = to.with_extension("part");
        let mut f = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        for at in (0..len).step_by(64 << 20) {
            say(done, all);
            let b = disc.read_at(p, at, 64 << 20)?;
            f.write_all(&b).map_err(|e| format!("{}: {e}", part.display()))?;
            done += b.len() as u64;
        }
        drop(f);
        std::fs::rename(&part, &to).map_err(|e| e.to_string())?;
    }
    Ok(out.display().to_string())
}

fn pick_game(args: &[String]) -> ! {
    #[derive(Resource, Default)]
    struct Pick {
        text: String,
        err: String,
        hover: bool,
        copy: Option<std::sync::Arc<std::sync::Mutex<((u64, u64), Option<Result<String, String>>)>>>,
    }
    static PICKED: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    fn ui(mut contexts: EguiContexts, mut pick: ResMut<Pick>, mut drops: MessageReader<FileDragAndDrop>, mut exit: MessageWriter<AppExit>) -> Result {
        let mut tried = None;
        for d in drops.read() {
            match d {
                FileDragAndDrop::HoveredFile { .. } => pick.hover = true,
                FileDragAndDrop::HoveredFileCanceled { .. } => pick.hover = false,
                FileDragAndDrop::DroppedFile { path_buf, .. } => {
                    pick.hover = false;
                    let iso = is_iso(&path_buf.display().to_string());
                    let p = if path_buf.is_file() && !iso { path_buf.parent().unwrap_or(path_buf).to_path_buf() } else { path_buf.clone() };
                    pick.text = p.display().to_string();
                    tried = Some(pick.text.clone());
                }
            }
        }
        let ctx = contexts.ctx_mut()?;
        ctx.style_mut(|s| {
            s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(18.0));
            s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(18.0));
            s.spacing.item_spacing = egui::vec2(8.0, 12.0);
            s.spacing.button_padding = egui::vec2(14.0, 8.0);
        });
        let copying = pick.copy.as_ref().map(|c| c.lock().unwrap().clone());
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.heading(egui::RichText::new("Delirium Physics Playroom").size(30.0));
                ui.add_space(8.0);
                if let Some(((done, all), _)) = copying {
                    ui.label(egui::RichText::new("Getting the game's files out of your .iso").size(24.0).strong());
                    ui.label("This happens once. It takes about a minute. Your .iso isn't changed.");
                    ui.add(egui::ProgressBar::new(done as f32 / all.max(1) as f32).show_percentage().desired_width(560.0));
                    return;
                }
                let (fill, line) = if pick.hover { (egui::Color32::from_rgb(40, 90, 60), egui::Color32::from_rgb(120, 230, 150)) }
                                   else { (egui::Color32::from_gray(38), egui::Color32::from_gray(140)) };
                egui::Frame::new().fill(fill).stroke(egui::Stroke::new(2.0, line)).corner_radius(10.0).inner_margin(28.0).show(ui, |ui| {
                    ui.set_width(640.0);
                    ui.vertical_centered(|ui| {
                        let big = if pick.hover { "Let go to use it" } else { "Drag your game here" };
                        ui.label(egui::RichText::new(big).size(28.0).strong().color(line));
                        ui.label(egui::RichText::new("You need your own GTA IV for Xbox 360, PS3 or PC (not included)").strong());
                        ui.label("Use the Xbox 360 or PS3 disc image (.iso), or the PC game folder (the one with GTAIV.exe in it).");
                        ui.label(egui::RichText::new("A PS3 disc image straight off the disc is fine: its key is looked up online.").small());
                        ui.label(egui::RichText::new("On PS3 everything works except the animations for now (get-ups and the Hero's moves don't play yet): or PC is best.").small());
                    });
                });
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Or paste where it is:").small());
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 640.0).max(0.0) / 2.0);
                    let field = ui.add_sized([500.0, 32.0], egui::TextEdit::singleline(&mut pick.text).hint_text(r"e.g. D:\Games\game.iso"));
                    if ui.button("Use it").clicked() || (field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                        tried = Some(pick.text.clone());
                    }
                });
                if !pick.err.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(240, 120, 100), &pick.err);
                }
                ui.label(egui::RichText::new("You only do this once: it's remembered.").small());
            });
        });
        if let Some(t) = tried {
            let g = t.trim().trim_matches('"').trim_matches('\'').to_string();
            if is_iso(&g) {
                let c = std::sync::Arc::new(std::sync::Mutex::new(((0, 1), None)));
                let c2 = c.clone();
                std::thread::spawn(move || {
                    let r = from_iso(&g, &|d, a| c2.lock().unwrap().0 = (d, a));
                    c2.lock().unwrap().1 = Some(r);
                });
                pick.copy = Some(c);
            } else if let Some(g) = [format!("{g}/GTAIV"), g.clone()].into_iter().find(|d| is_game(d)) {
                let _ = std::fs::write(GAME_PATH, &g);
                let _ = PICKED.set(g);
                exit.write(AppExit::Success);
            } else {
                pick.err = format!("That isn't it: {g}\nDrag your game .iso file (or its game folder).");
            }
        }
        let done = pick.copy.as_ref().and_then(|c| c.lock().unwrap().1.take());
        match done {
            Some(Ok(f)) => {
                let _ = std::fs::write(GAME_PATH, &f);
                let _ = PICKED.set(f);
                exit.write(AppExit::Success);
            }
            Some(Err(e)) => {
                pick.copy = None;
                pick.err = format!("Couldn't read that .iso: {e}\nIt needs to be the game's disc image (a PC copy won't work).");
            }
            None => ctx.request_repaint(),
        }
        Ok(())
    }
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Delirium Physics Playroom".into(), resolution: (900, 460).into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .init_resource::<Pick>()
        .add_systems(Startup, |mut commands: Commands| { commands.spawn(Camera2d); })
        .add_systems(EguiPrimaryContextPass, ui)
        .run();
    let Some(g) = PICKED.get() else { std::process::exit(1) };
    let ok = std::process::Command::new(&args[0]).args(&args[1..]).arg("--game").arg(g).spawn().is_ok();
    std::process::exit(if ok { 0 } else { 1 });
}

impl Sim {
    fn split(&self) -> bool {
        false
    }
}

#[derive(Component)]
struct OursCam;
#[derive(Component)]
struct FacePart(usize);

fn face_view(sim: NonSend<Sim>, time: Res<Time>, mut q: Query<(&FacePart, &mut Transform, &mut Visibility)>) {
    let show = sim.eyes && sim.watch.is_none();
    let up_clip = sim.getup.as_ref().zip(sim.rises[0].as_ref()).and_then(|(c, r)| c.frames(r));
    let links: Vec<Transform> = sim.shown().map(|f| f.links.clone()).or(up_clip).unwrap_or_else(|| sim.agent.doll.frames().collect());
    let caps: Vec<Transform> = sim.shown().map(|f| f.caps.clone()).unwrap_or_else(|| sim.agent.doll.capsules().collect());
    let (Some(h), Some(rest), Some(part), true) = (links.get(HEAD), sim.frames.get(HEAD), sim.build.parts.get(HEAD), show) else {
        q.iter_mut().for_each(|(_, _, mut v)| *v = Visibility::Hidden);
        return;
    };
    let stand = Side::new(sim.frames.iter().map(|f| f.translation).collect());
    let link = delirium::nmdbm::rage_link(nm(), HEAD);
    let (cap, cap0) = (h.rotation * link, rest.rotation * link);
    let axis = if (cap0 * Vec3::Y).z < 0.0 { -Vec3::Y } else { Vec3::Y };
    let ahead = cap0.inverse() * stand.fwd;
    let ahead = (ahead - axis * ahead.dot(axis)).normalize_or(Vec3::Z);
    let (fwd, up) = (cap * ahead, cap * axis);
    let left = up.cross(fwd).normalize_or(Vec3::X);
    let centre = caps.get(HEAD).filter(|_| sim.rises[0].is_none()).map_or(h.translation, |c| c.translation);
    let r = part.radius;
    let v = sim.agent.doll.com().1;
    let speed = v.length();
    let fear = ((speed - 1.0) / 2.0 + (-v.z - 0.5).max(0.0) * 0.6).clamp(0.0, 1.0);
    let hit = sim.rings.iter().filter(|r| r.0 == 0).min_by(|a, b| a.4.total_cmp(&b.4));
    let glow = sim.glow.first().map_or(0.0, |g| g.iter().map(|x| x.0).fold(0.0, f32::max));
    let pain = (hit.map_or(0.0, |r| 1.0 - r.4 / 1.5) .max(glow.min(1.0))).clamp(0.0, 1.0);
    let worry = ((speed - 0.3) / 0.7).clamp(0.0, 1.0) * (1.0 - fear);
    let dazed = if h.translation.z < 0.5 && speed < 0.3 { 1.0 } else { 0.0 };
    let upset = fear.max(worry).max(pain);
    let t = time.elapsed_secs();
    let blink = (t + 0.7 * (t / 4.0).floor().sin()) % 4.0 < 0.12
        || hit.is_some_and(|r| r.4 < 0.3) || glow > 0.7;
    let open = if sim.dead || blink { 0.08 } else { (1.0 + 0.3 * fear) * (1.0 - 0.8 * pain.max(0.6 * dazed)) };
    let look = hit.and_then(|r| links.get(r.1).map(|f| f.transform_point(r.2))).map(|p| (p - centre).normalize_or(fwd))
        .or_else(|| (speed > 1.5).then(|| v / speed)).unwrap_or(fwd);
    let gaze = (look - fwd * look.dot(fwd)).clamp_length_max(1.0) * 0.12;
    for (f, mut tr, mut vis) in &mut q {
        let side = if f.0 % 2 == 0 { 1.0 } else { -1.0 };
        let n = (fwd + up * 0.1 + left * side * 0.3 + gaze).normalize();
        let el = up.cross(n).normalize_or(left);
        let basis = Quat::from_mat3(&Mat3::from_cols(el, n.cross(el), n));
        let eye = centre + n * r;
        let (w, h) = (0.12 * r, 0.19 * r * open);
        let brow = sim.brows && upset > 0.2;
        *tr = match f.0 {
            0 | 1 => Transform::from_translation(eye + n * 0.05 * r + n.cross(el) * h * 0.4 - el * w * 0.35).with_rotation(basis).with_scale(Vec3::splat(0.035 * r * (open > 0.5) as u8 as f32)),
            2 | 3 => Transform::from_translation(eye).with_rotation(basis).with_scale(Vec3::new(w, h, 0.06 * r)),
            _ => {
                let roll = Quat::from_axis_angle(n, -side * 0.45 * upset);
                let at = centre + (fwd + up * (0.42 + 0.06 * upset) + left * side * 0.3).normalize() * r;
                Transform::from_translation(at).with_rotation(roll * basis).with_scale(Vec3::new(0.24 * r, 0.035 * r, 0.03 * r) * brow as u8 as f32)
            }
        };
        *vis = Visibility::Visible;
    }
}

#[derive(Component)]
struct PartMesh {
    k: usize,
    crowd: Option<usize>,
    watch: Option<usize>,
}
#[derive(Default, Reflect, GizmoConfigGroup)]
struct OursGizmos;

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, sim: NonSend<Sim>,
         mut gizmos: ResMut<GizmoConfigStore>, mut egui_settings: ResMut<bevy_egui::EguiGlobalSettings>) {
    egui_settings.auto_create_primary_context = false;
    commands.spawn((Camera3d::default(), Camera { order: 0, ..default() }, RenderLayers::from_layers(&[0, 1]), Transform::default(),
                    Orbit { yaw: -1.3, pitch: 0.3, dist: 3.5, target: Vec3::new(0.0, 0.0, 0.8), follow: 0 }, OursCam));
    commands.spawn((Camera2d, Camera { order: 2, clear_color: ClearColorConfig::None, ..default() }, RenderLayers::layer(31), bevy_egui::PrimaryEguiContext));
    commands.spawn((DirectionalLight { shadows_enabled: true, ..default() }, Transform::from_xyz(3.0, -2.0, 6.0).looking_at(Vec3::ZERO, Vec3::Z),
                    RenderLayers::from_layers(&[0, 1, 2])));
    commands.spawn((DirectionalLight { shadows_enabled: false, illuminance: 0.0, ..default() }, Transform::from_xyz(-3.0, 4.0, 2.0).looking_at(Vec3::ZERO, Vec3::Z),
                    RenderLayers::from_layers(&[0, 1, 2]), Fill));
    spawn_world(&mut commands, &mut meshes, &mut mats, &sim);
    let (cfg, _) = gizmos.config_mut::<OursGizmos>();
    (cfg.render_layers, cfg.depth_bias) = (RenderLayers::layer(1), -1.0);
}

fn spawn_world(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, sim: &Sim) {
    let floor = if sim.room.stage == "street" || sim.room.stage == "cab" { Color::srgb(0.2, 0.2, 0.22) } else { Color::srgb(0.58, 0.59, 0.61) };
    commands.spawn((Mesh3d(meshes.add(Plane3d::new(Vec3::Z, Vec2::splat(30.0)))),
                    MeshMaterial3d(mats.add(StandardMaterial { base_color: floor, perceptual_roughness: 1.0, ..default() })), RoomScenery, Floor));
    for &(c, h, row) in &sim.room.blocks {
        let brick = sim.room.surfaces.get(row as usize).is_some_and(|s| s.0 == "BRICK_WALL");
        let post = sim.room.stage == "bridge" && h == world::POST_HALF;
        let color = if sim.room.stage == "trampoline" { Color::srgb(0.2, 0.3, 0.75) } else if post { Color::srgb(0.24, 0.25, 0.27) } else if h.z < 0.01 { Color::srgb(0.92, 0.92, 0.9) } else if brick { Color::srgb(0.8, 0.42, 0.33) } else { Color::srgb(0.68, 0.68, 0.7) };
        let (mesh, rot) = match post {
            true => (meshes.add(Cylinder::new(h.x * 1.5, h.z * 2.0)), Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            false => (meshes.add(Cuboid::from_size(h * 2.0)), Quat::IDENTITY),
        };
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mats.add(StandardMaterial { base_color: color, perceptual_roughness: 0.9, ..default() })),
                        Transform::from_translation(c).with_rotation(rot), RenderLayers::layer(1), RoomScenery));
    }
    let wood = mats.add(StandardMaterial { base_color: Color::srgb(0.36, 0.35, 0.33), perceptual_roughness: 0.85, double_sided: true, cull_mode: None, ..default() });
    for (i, p) in sim.room.bridge.iter().flat_map(|b| b.planks.iter().enumerate()) {
        for s in 0..2 {
            commands.spawn((Mesh3d(meshes.add(plank_half(&p.teeth, s))), MeshMaterial3d(wood.clone()), Transform::default(), RenderLayers::layer(1), RoomScenery, Plank(i, s)));
        }
    }
    if sim.room.bridge.is_some() {
        let n = world::PLANKS;
        let mut grey = |c: f32| mats.add(StandardMaterial { base_color: Color::srgb(c, c, c + 0.02), perceptual_roughness: 0.8, ..default() });
        let (rail, hand, rod, side) = ((meshes.add(Cylinder::new(0.02, 1.0)), grey(0.7)), (meshes.add(Cylinder::new(0.035, 1.0)), grey(0.62)),
                                       (meshes.add(Cylinder::new(0.012, 1.0)), grey(0.55)), (meshes.add(Cylinder::new(0.025, 1.0)), grey(0.62)));
        for k in 0..world::ROPES {
            let foot = 2 * (2 * n + 1);
            let (mesh, mat) = match k {
                _ if k < foot => &rail,
                _ if k < foot + 2 * (n + 1) => &hand,
                _ if k >= foot + 2 * (n + 1) + 2 * n => &side,
                _ if (k - foot - 2 * (n + 1)) % n % 3 == 1 => &rod,
                _ => continue,
            };
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::default(), RenderLayers::layer(1), RoomScenery, Rope(k)));
        }
    }
    let sides = [(false, 1)];
    let homer = sides.into_iter().map(|(g, l)| (g, l, None, None)).chain((0..24).map(|c| (false, 1, Some(c), None)))
        .chain((0..learn::LIVE).map(|w| (false, 1, None, Some(w))));
    for (game, layer, crowd, watch) in homer {
        let _ = game;
        for (k, p) in sim.build.parts.iter().enumerate() {
            let name = delirium_data::nm_asset::PARTS.get(k).copied().unwrap_or("");
            let c = part_color(0, name);
            commands.spawn((Mesh3d(part_mesh(meshes, p)),
                            MeshMaterial3d(mats.add(StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.6, ..default() })),
                            Transform::default(), Visibility::Hidden, RenderLayers::layer(layer), PartMesh { k,  crowd, watch }));
        }
    }
    let (white, black) = (mats.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() }),
                          mats.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.07), unlit: true, ..default() }));
    let (ball, bar) = (meshes.add(Sphere::new(1.0)), meshes.add(Cuboid::from_size(Vec3::ONE)));
    for i in 0..6 {
        let (mesh, mat) = if i < 2 { (ball.clone(), white.clone()) } else if i < 4 { (ball.clone(), black.clone()) } else { (bar.clone(), black.clone()) };
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), Visibility::Hidden, RenderLayers::layer(1), FacePart(i)));
    }
    for (k, p) in sim.getup.as_ref().and_then(|c| c.hero.as_ref()).map_or(&[][..], |r| &r.build.parts[..]).iter().enumerate() {
        commands.spawn((Mesh3d(part_mesh(meshes, p)),
                        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgba(0.3, 0.75, 0.4, 0.6), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.6, ..default() })),
                        Transform::default(), Visibility::Hidden, RenderLayers::layer(1), HeroMesh(k)));
    }
}

fn part_mesh(meshes: &mut Assets<Mesh>, p: &delirium::ragdoll::PartDef) -> Handle<Mesh> {
    match p.boxed {
        Some(e) => meshes.add(Cuboid { half_size: e }),
        None => meshes.add(Capsule3d::new(p.radius, p.half * 2.0)),
    }
}

#[derive(Component)]
struct MapScenery;

fn switch(mut sim: NonSendMut<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
          old: Query<Entity, Or<(With<RoomScenery>, With<MapScenery>, With<PartMesh>, With<HeroMesh>)>>) {
    let Some((scene, marge)) = sim.switch_to.take() else { return };
    let t0 = std::time::Instant::now();
    let mut a: Vec<String> = std::env::args().collect();
    for flag in ["--map", "--ground", "--stage"] {
        if let Some(i) = a.iter().position(|x| x == flag) {
            a.drain(i..(i + 2).min(a.len()));
        }
    }
    a.retain(|x| x != "--marge");
    let (map, ground) = match scene.as_str() {
        "stairs" => ("818.94,-348.88", None),
        "wall" => ("738.83,-467", None),
        "ledge" => ("977.5,-544", None),
        _ => ("none", Some("757.41,-221.61")),
    };
    a.extend(["--map".to_string(), map.to_string()]);
    if let Some(g) = ground {
        a.extend(["--ground".to_string(), g.to_string()]);
    }
    if marge {
        a.push("--marge".into());
    }
    let stage = STAGES.iter().chain(&EXTRAS).any(|t| t.0 == scene && t.0 != "room");
    if stage {
        a.extend(["--stage".to_string(), scene.clone()]);
    }
    let mut new = load(&a, &a, sim.game_dir.as_deref(), VecDeque::new(), None);
    (new.log, new.script, new.show_ui, new.sens, new.walk, new.getup_mode, new.poke, new.god, new.aim_gun) = (std::mem::take(&mut sim.log), std::mem::take(&mut sim.script), sim.show_ui, sim.sens, sim.walk, sim.getup_mode, sim.poke, sim.god, sim.aim_gun);
    (new.show_pops, new.help, new.tab, new.skin) = (sim.show_pops, sim.help, sim.tab, sim.skin);
    new.scene = Some(scene.clone());
    *sim = new;
    let said = format!("scene: {scene}, {} ({} ms)", if marge { "Marge" } else { "Homer" }, t0.elapsed().as_millis());
    eprintln!("{said}");
    sim.say(said);
    for e in &old {
        commands.entity(e).despawn();
    }
    spawn_world(&mut commands, &mut meshes, &mut mats, &sim);
}

#[derive(Component)]
struct HeroMesh(usize);

fn bodies(sim: NonSend<Sim>, mut q: Query<(&PartMesh, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>), Without<HeroMesh>>,
          mut hero: Query<(&HeroMesh, &mut Transform, &mut Visibility), Without<PartMesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let doll: Option<Vec<Transform>> = sim.hero_doll.as_ref().map(|h| match h.rise.as_ref().zip(sim.getup.as_ref()).and_then(|(r, c)| c.frames(r)) {
        Some(f) => f.into_iter().enumerate().map(|(k, f)| f.with_rotation(f.rotation * delirium::nmdbm::rage_link(nm(), k))).collect(),
        None => h.agent.doll.capsules().collect(),
    });
    let rig = sim.getup.as_ref().and_then(|c| c.hero.as_ref());
    for (m, mut t, mut v) in &mut hero {
        let at = match &doll {
            Some(f) => rig.and_then(|r| r.build.parts.get(m.0)).and_then(|p| sim.build.parts.iter().position(|q| q.name == p.name)).and_then(|j| f.get(j).copied()),
            None => sim.hero.as_ref().filter(|_| !sim.driving).and_then(|n| n.caps.get(m.0)).copied(),
        };
        match at {
            Some(c) => (*t, *v) = (c, Visibility::Visible),
            None => *v = Visibility::Hidden,
        }
    }
    let up = |i: usize| sim.getup.as_ref().zip(sim.rises[i].as_ref()).and_then(|(c, r)| c.frames(r))
        .map(|f| f.into_iter().enumerate().map(|(k, f)| f.with_rotation(f.rotation * delirium::nmdbm::rage_link(nm(), k))).collect::<Vec<_>>());
    let fr = sim.shown().map(|f| f.caps.clone()).or_else(|| up(0)).unwrap_or_else(|| sim.agent.doll.capsules().collect());
    let crowd: Vec<Vec<Transform>> = sim.crowd.iter().map(|(a, _)| a.doll.capsules().collect()).collect();
    let link = |k: usize, f: Transform| f.with_rotation(f.rotation * delirium::nmdbm::rage_link(nm(), k));
    for (p, mut t, mut v, m) in &mut q {
        let ped = p.crowd.map_or(0, |c| c + 1);
        let lit = sim.glow.get(ped).and_then(|g| g.get(p.k)).filter(|_| p.watch.is_none()).map_or(LinearRgba::BLACK, |&(g, (c, _))| c * g);
        let col = part_color(sim.ped_color, delirium_data::nm_asset::PARTS.get(p.k).copied().unwrap_or(""));
        if mats.get(&m.0).is_some_and(|x| x.emissive != lit || x.base_color != col) {
            if let Some(x) = mats.get_mut(&m.0) {
                x.emissive = lit;
                x.base_color = col;
                x.alpha_mode = if col.alpha() < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque };
            }
        }
        let watching = sim.watch.as_ref();
        let at = match p.crowd {
            _ if p.watch.is_some() => watching.and_then(|w| w.at(p.watch.unwrap(), p.k)).map(|f| link(p.k, f)),
            _ if watching.is_some() => None,
            Some(c) => crowd.get(c).and_then(|f| f.get(p.k).copied()),
            None => fr.get(p.k).copied(),
        };
        match at {
            Some(a) => (*t, *v) = (a, Visibility::Visible),
            None => *v = Visibility::Hidden,
        }
    }
}

fn scene(mut sim: NonSendMut<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
         room: Query<Entity, With<RoomScenery>>) {
    if sim.map.is_none() {
        return;
    }
    let key = format!("{}-{}", sim.scene.as_deref().unwrap_or("map"), nm());
    let live: Option<(Quat, Vec3, Vec3)> = None;
    let Some(view @ (rot, at, to)) = live.or_else(|| views().remove(&key)) else { return };
    if live.is_some() {
        save_view(&key, view);
    }
    let to_view = |p: Vec3| rot * (p - at) + to;
    let Some(map) = sim.map.take() else { return };
    let (mut game, mut tris, mut mat, mut fl, mut faces) = (vec![], vec![], vec![], vec![], vec![]);
    for m in &map {
        let face = delirium::collision::Face::of(m);
        for (k, t) in m.triangles.iter().enumerate() {
            let v = t.map(|i| Vec3::from(m.positions[i as usize]));
            if v.iter().all(|p| p.truncate().distance(at.truncate()) > SCENE_R) {
                continue;
            }
            tris.push([0, 1, 2].map(|i| game.len() as u32 + i));
            game.extend(v.map(to_view));
            mat.push(m.materials.get(k).copied().unwrap_or(0));
            fl.push(m.flags.get(k).copied().unwrap_or(0));
            faces.push(face[k].clone().map(|f| f.turned(rot, |p| to_view(p) - Vec3::X * 1.5)));
        }
    }
    let ours: Vec<Vec3> = game.iter().map(|p| *p - Vec3::X * 1.5).collect();
    let arr = |v: &[Vec3]| v.iter().map(|p| p.to_array()).collect::<Vec<_>>();
    sim.world = Collision::default();
    sim.world.add_chunk_faces(0, &arr(&ours), &tris, &mat, &fl, &faces);
    sim.room.scene([&ours, &game], &tris);
    for e in &room {
        commands.entity(e).despawn();
    }
    let m = mats.add(StandardMaterial { base_color: Color::srgb(0.62, 0.63, 0.65), perceptual_roughness: 1.0, double_sided: true, cull_mode: None, ..default() });
    for (layer, v) in [(1, &ours), (2, &game)] {
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, arr(v))
            .with_computed_flat_normals();
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(m.clone()), Transform::default(), RenderLayers::layer(layer), MapScenery));
    }
    let under = |x: f32, y: f32| sim.world.ground(Vec3::new(x, y, 2.0), 4.0).map_or("none".into(), |z| format!("{z:.3}"));
    let feet: Vec<String> = sim.agent.doll.frames().map(|f| f.translation).filter(|p| p.z < 0.25).map(|p| format!("{:.2},{:.2},{:.2}: {}", p.x, p.y, p.z, under(p.x, p.y))).collect();
    sim.say(format!("map: {} triangles round {:.1},{:.1},{:.1}{}; ground under ours' low parts {}", tris.len(), at.x, at.y, at.z,
                    if live.is_some() { "" } else { " (as last seen)" }, feet.join("  ")));
}

const VIEWS: &str = "ledge-0	0 0 0.46081 0.8875 977.4801 -544.00006 23.3883 1.5 0 0
ledge-1	0 0 0.96184 -0.27363 977.4165 -544 23.38464 1.5 0 0
stairs-0	0 0 0.68463 0.72889 818.9479 -348.87863 14.68695 1.5 0 0
stairs-1	0 0 -0.59279 -0.80536 818.9429 -348.87146 14.7149 1.5 0 0
wall-0	0 0 0.26058 0.96545 738.8504 -466.99994 4.97328 1.5 0 0
wall-1	0 0 0.27894 0.96031 738.8266 -467.00827 5.04545 1.5 0 0
";

fn views() -> std::collections::HashMap<String, (Quat, Vec3, Vec3)> {
    let f = VIEWS.to_string() + &std::fs::read_to_string(getup::config_dir().join("views.txt")).unwrap_or_default();
    f.lines().filter_map(|l| {
        let (k, v) = l.split_once('\t')?;
        let n: Vec<f32> = v.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        (n.len() == 10).then(|| (k.to_string(), (Quat::from_xyzw(n[0], n[1], n[2], n[3]).normalize(), Vec3::new(n[4], n[5], n[6]), Vec3::new(n[7], n[8], n[9]))))
    }).collect()
}

fn save_view(key: &str, (r, a, t): (Quat, Vec3, Vec3)) {
    let mut all = views();
    if all.get(key) == Some(&(r, a, t)) {
        return;
    }
    all.insert(key.to_string(), (r, a, t));
    let mut ks: Vec<_> = all.iter().collect();
    ks.sort_by(|x, y| x.0.cmp(y.0));
    let s: String = ks.iter().map(|(k, (r, a, t))| format!("{k}\t{} {} {} {} {} {} {} {} {} {}\n", r.x, r.y, r.z, r.w, a.x, a.y, a.z, t.x, t.y, t.z)).collect();
    let _ = std::fs::create_dir_all(getup::config_dir());
    let _ = std::fs::write(getup::config_dir().join("views.txt"), s);
}

#[derive(Component)]
struct RoomScenery;
#[derive(Component)]
struct Plank(usize, usize);

#[derive(Component)]
struct Rope(usize);

fn plank_half(teeth: &[f32], s: usize) -> Mesh {
    let h = world::PLANK_HALF;
    let n = teeth.len() - 1;
    let out = [-h.x, h.x][s];
    let mut o = vec![Vec2::new(out, -h.y), Vec2::new(out, h.y)];
    o.extend((0..=n).rev().map(|k| Vec2::new(teeth[k], -h.y + 2.0 * h.y * k as f32 / n as f32)));
    let mut v: Vec<[f32; 3]> = vec![];
    let mut tri = |a: Vec2, b: Vec2, c: Vec2, z: f32| v.extend([a, b, c].map(|p| [p.x, p.y, z]));
    for k in 0..n {
        let (a, b) = (o[o.len() - 1 - k], o[o.len() - 2 - k]);
        let (oa, ob) = (Vec2::new(out, a.y), Vec2::new(out, b.y));
        for z in [-h.z, h.z] {
            tri(oa, a, b, z);
            tri(oa, b, ob, z);
        }
    }
    for k in 0..o.len() {
        let (a, b) = (o[k], o[(k + 1) % o.len()]);
        v.extend([[a.x, a.y, -h.z], [b.x, b.y, -h.z], [b.x, b.y, h.z], [a.x, a.y, -h.z], [b.x, b.y, h.z], [a.x, a.y, h.z]]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()).with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, v).with_computed_flat_normals()
}

fn planks(sim: NonSend<Sim>, mut q: Query<(&Plank, &mut Transform), Without<Rope>>, mut r: Query<(&Rope, &mut Transform), Without<Plank>>) {
    let Some(b) = sim.room.bridge.as_ref() else { return };
    for (p, mut t) in &mut q {
        *t = b.half(&sim.room.w[0], p.0, p.1);
    }
    let ropes = b.ropes(&sim.room.w[0]);
    for (k, mut t) in &mut r {
        let (a, e) = ropes[k.0];
        *t = Transform::from_translation((a + e) / 2.0).with_rotation(Quat::from_rotation_arc(Vec3::Y, (e - a).normalize_or(Vec3::Y)))
            .with_scale(Vec3::new(1.0, a.distance(e), 1.0));
    }
}
#[derive(Component)]
struct Fill;

#[derive(Component)]
struct Floor;

fn follow_floor(sim: NonSend<Sim>, mut q: Query<&mut Transform, With<Floor>>) {
    for mut t in &mut q {
        (t.translation, t.scale) = (sim.ours().p(0).with_z(0.0), Vec3::new(100.0, 100.0, 1.0));
    }
}

fn apply_look(sim: NonSend<Sim>, mut last: Local<Option<(Look, Entity, bool, f32)>>, mut sky: ResMut<ClearColor>, mut sun: Query<(&mut DirectionalLight, Has<Fill>)>,
              mut fill: ResMut<GlobalAmbientLight>,
              floor: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<Floor>>, mut mats: ResMut<Assets<StandardMaterial>>,
              cam: Query<Entity, With<OursCam>>, mut commands: Commands) {
    let (l, Ok((f, m))) = (sim.skin, floor.single()) else { return };
    if *last == Some((l, f, sim.soft, sim.shadow)) {
        return;
    }
    if last.is_some_and(|o| o.0 != l) {
        if let Some(p) = look_path() {
            let b = |x: bool| x as u8 as f32;
            let n = [l.sky[0], l.sky[1], l.sky[2], l.floor[0], l.floor[1], l.floor[2], b(l.grid), b(l.fog), l.sun, 0.0, b(l.glow)];
            let _ = std::fs::write(p, n.map(|x| x.to_string()).join(" "));
        }
    }
    *last = Some((l, f, sim.soft, sim.shadow));
    fill.brightness = if sim.soft { 400.0 } else { GlobalAmbientLight::default().brightness };
    let [r, g, b] = l.sky;
    sky.0 = Color::srgb(r, g, b);
    for (mut s, fill) in &mut sun {
        let k = match (fill, sim.soft) { (false, true) => 0.75, (false, false) => 1.0, (true, true) => 0.3, (true, false) => 0.0 };
        s.illuminance = bevy::light::light_consts::lux::AMBIENT_DAYLIGHT * l.sun * k;
        s.soft_shadow_size = (!fill && sim.shadow > 0.0).then_some(sim.shadow);
    }
    if sim.room.stage != "street" {
        if let Some(m) = mats.get_mut(&m.0) {
            let [r, g, b] = l.floor;
            m.base_color = Color::srgb(r, g, b);
        }
    }
    for c in &cam {
        if l.fog {
            let (start, end) = if r + g + b < 0.3 { (4.0, 12.0) } else { (6.0, 28.0) };
            commands.entity(c).insert(bevy::pbr::DistanceFog { color: Color::srgb(r, g, b), falloff: bevy::pbr::FogFalloff::Linear { start, end }, ..default() });
        } else {
            commands.entity(c).remove::<bevy::pbr::DistanceFog>();
        }
    }
}

const SCENE_R: f32 = 40.0;
const STAND: f32 = 0.95;
const SCENES: [&str; 4] = ["room", "stairs", "wall", "ledge"];
const EXTRAS: [(&str, &str); 3] = [("bridge", "rope bridge"), ("trampoline", "trampoline"), ("cab", "cab door")];
const SNAP_CHUNK: u64 = 0xB41D6E;
const BRIDGE_CHUNK: u64 = 0xB41D00;
const STAGES: [(&str, &str); 6] = [("room", "room"), ("bigstairs", "big stairs"), ("rooftop", "rooftop"), ("lowwall", "low wall"), ("rail", "rail"), ("street", "street")];

#[derive(Component)]
struct PropMesh(usize, usize);

fn sounds(mut sim: NonSendMut<Sim>, mut commands: Commands, mut bank: Option<ResMut<sfx::Bank>>) {
    let (mut q, on) = (std::mem::take(&mut sim.sounds), sim.sound);
    sfx::play(&mut commands, &mut bank, &mut q, on);
}

fn blood_view(sim: NonSend<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
              mut q: Query<(&BloodPix, &mut Transform, &mut Visibility)>, mut made: Local<bool>) {
    if !*made {
        *made = true;
        let mesh = meshes.add(Cuboid::from_length(BLOOD_SIZE));
        let shades = [(0.75, 0.02, 0.03), (0.55, 0.0, 0.02), (0.95, 0.12, 0.08)]
            .map(|(r, g, b)| mats.add(StandardMaterial { base_color: Color::srgb(r, g, b), unlit: true, ..default() }));
        for k in 0..BLOOD_MAX {
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(shades[k % 3].clone()), Transform::default(), Visibility::Hidden, BloodPix(k)));
        }
    }
    for (p, mut t, mut v) in &mut q {
        let Some(&(at, vel, age, _)) = sim.blood.get(p.0) else {
            *v = Visibility::Hidden;
            continue;
        };
        let fade = (1.0 - (age - (BLOOD_LIFE - 1.0)).max(0.0)).clamp(0.05, 1.0);
        *t = Transform::from_translation(at).with_scale(if vel == Vec3::ZERO { Vec3::new(1.2, 1.2, 0.12) * fade } else { Vec3::splat(fade) });
        *v = Visibility::Visible;
    }
}

fn props(sim: NonSend<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
         mut q: Query<(Entity, &PropMesh, &mut Transform)>, mut made: Local<usize>) {
    let objs = &sim.room.objs;
    if objs.len() < *made {
        for (e, _, _) in &q {
            commands.entity(e).despawn();
        }
        *made = 0;
    }
    for (_, p, mut t) in &mut q {
        if p.0 < objs.len() {
            *t = sim.shown().filter(|_| p.1 == 0).and_then(|f| f.props.get(p.0).copied()).unwrap_or_else(|| sim.room.obj(p.1, p.0).0);
        }
    }
    for (j, o) in objs.iter().enumerate().skip(*made) {
        let (_, s, _, _) = world::KINDS[o.kind];
        let mesh = match o.kind {
            0 | 4 => meshes.add(Sphere::new(s)),
            3 => meshes.add(Cylinder::new(s, world::BARREL_HALF * 2.0)),
            _ => meshes.add(Cuboid::from_length(s * 2.0)),
        };
        let color = match o.kind {
            0 | 4 => Color::srgb(0.15, 0.15, 0.17),
            3 => Color::srgb(0.75, 0.12, 0.08),
            _ => Color::srgb(0.62, 0.42, 0.24),
        };
        let mat = mats.add(StandardMaterial { base_color: color, perceptual_roughness: 0.8, ..default() });
        for i in 0..SIDES {
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), sim.room.obj(i, j).0, RenderLayers::layer(1 + i), PropMesh(j, i)));
        }
    }
    *made = objs.len();
}

fn gun_view(mut sim: NonSendMut<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
            mut q: Query<(&mut Transform, &mut Visibility, &GunMesh), Without<GunFlash>>, mut flash: Query<(&mut Transform, &mut Visibility), With<GunFlash>>,
            mut g: Gizmos<OursGizmos>, time: Res<Time>) {
    if q.is_empty() {
        let m = mats.add(StandardMaterial { base_color: Color::srgba(0.35, 0.9, 1.0, 0.3), emissive: LinearRgba::rgb(0.1, 0.5, 0.6),
                                            alpha_mode: AlphaMode::Blend, unlit: true, double_sided: true, cull_mode: None, ..default() });
        for (i, homer) in (0..GUN_MODELS.len()).map(|i| (i, false)).chain((0..WEAPONS.len()).filter(|&i| held(i).is_some() || kind(i) == 0).map(|i| (i, true))) {
            let parts = &GUN_MODELS[i].0;
            commands.spawn((GunMesh(i, homer), Transform::default(), Visibility::Hidden)).with_children(|c| {
                for &(size, at, tilt) in *parts {
                    c.spawn((Mesh3d(meshes.add(Cuboid::new(size[0], size[1], size[2]))), MeshMaterial3d(m.clone()),
                             Transform::from_translation(Vec3::from(at)).with_rotation(Quat::from_rotation_x(tilt)), RenderLayers::layer(1)));
                }
            });
        }
        let f = mats.add(StandardMaterial { base_color: Color::srgba(1.0, 0.75, 0.3, 0.85), alpha_mode: AlphaMode::Add, unlit: true, ..default() });
        commands.spawn((GunFlash, Mesh3d(meshes.add(Sphere::new(0.5))), MeshMaterial3d(f), Transform::default(), Visibility::Hidden, RenderLayers::layer(1)));
        return;
    }
    let dt = time.delta_secs();
    let on = sim.gun.placed && if sim.hero.is_some() { sim.hero_gun && !sim.driving && sim.hero_doll.is_none() } else { sim.aim_gun };
    let (k, d) = (sim.gun.kick, sim.gun.dir);
    let right = d.cross(Vec3::Z).normalize_or(Vec3::X);
    let at = sim.gun.at - d * 0.05 * k;
    let mut shown = Quat::from_axis_angle(right, 0.35 * k) * d;
    let mut at = at;
    let bat = sim.hero.is_none() && WEAPONS[sim.weapon].0 == "bat";
    if bat {
        let (th, lunge) = bat_swing(if sim.gun.swing > 0.0 { 1.0 - sim.gun.swing / BAT_SWING } else { 0.0 });
        shown = (Vec3::Z * 0.8 + right * 0.6).normalize() * th.cos() + d * th.sin();
        at += d * lunge;
    }
    if sim.gun.swing > 0.0 {
        let len = swing_len(sim.weapon);
        let was = 1.0 - sim.gun.swing / len;
        sim.gun.swing -= dt;
        let s = 1.0 - sim.gun.swing.max(0.0) / len;
        if !bat {
            at += d * 0.3 * (s * std::f32::consts::PI).sin();
        }
        let lands = if bat { bat_swing(was).0 < std::f32::consts::FRAC_PI_2 && bat_swing(s).0 >= std::f32::consts::FRAC_PI_2 } else { was < 0.5 && s >= 0.5 };
        if lands && sim.hero.is_none() {
            sim.swing_weapon();
        }
    }
    let up = if WEAPONS[sim.weapon].0 == "molotov" { Vec3::Y } else { Vec3::Z };
    if up == Vec3::Y {
        shown = Vec3::Z;
    }
    for (mut t, mut v, m) in &mut q {
        if m.1 {
            let me = sim.homer.as_ref().filter(|_| sim.homer_weapon == Some(m.0));
            let h = me.and_then(|me| match held(m.0) {
                Some(x) => {
                    let (lo, hi, ..) = me.held_axis(x);
                    Some(Transform::from_translation(lo).looking_to((hi - lo).normalize_or(Vec3::Z), Vec3::Z))
                }
                None => me.hold.map(|(_, _, to)| {
                    let d = (to - me.hand).normalize_or(Vec3::X);
                    Transform::from_translation(me.hand - d * 0.05).looking_to(d, Vec3::Z)
                }),
            });
            *v = if h.is_some() { Visibility::Visible } else { Visibility::Hidden };
            if let Some(h) = h {
                *t = h;
            }
            continue;
        }
        *v = if on && m.0 == sim.weapon { Visibility::Visible } else { Visibility::Hidden };
        *t = Transform::from_translation(at).looking_to(shown, up);
    }
    let muzzle = at + shown * GUN_MODELS[sim.weapon].1;
    for (mut t, mut v) in &mut flash {
        *v = if on && sim.gun.flash > 0.0 { Visibility::Visible } else { Visibility::Hidden };
        let r = 0.05 + 0.04 * k;
        *t = Transform::from_translation(muzzle + shown * r).looking_to(shown, Vec3::Z).with_scale(Vec3::new(r, r, r * 2.2));
    }
    sim.gun.kick *= (-14.0 * dt).exp();
    sim.gun.flash -= dt;
    if on && kind(sim.weapon) == 0 {
        let reach = muzzle.distance(sim.gun.aim).max(0.5);
        let c = Color::srgba(0.4, 0.95, 1.0, 0.35);
        g.line(muzzle, muzzle + d * reach, c);
        g.sphere(Isometry3d::from_translation(muzzle + d * reach), 0.02, Color::srgba(0.4, 0.95, 1.0, 0.8));
        let (_, acc, pellets, _) = sim.gun_stats();
        let held = sim.gun.held;
        let (_, most) = sim.gun_spread(reach, acc);
        sim.gun.held = held;
        let iso = Isometry3d::new(muzzle + d * reach, Quat::from_rotation_arc(Vec3::Z, d));
        if most > 0.005 {
            g.circle(iso, most, c);
        }
        if pellets > 1 {
            g.rect(iso, Vec2::splat(2.0 * PELLET_SPREAD * acc * reach), Color::srgba(1.0, 0.6, 0.3, 0.4));
        }
    }
    for (from, to, left) in &mut sim.gun.tracers {
        g.line(*from, *to, Color::srgba(1.0, 0.85, 0.4, (*left / 0.25).clamp(0.0, 1.0)));
        *left -= dt;
    }
    sim.gun.tracers.retain(|t| t.2 > 0.0);
    sim.rocket_frame(dt);
    for &(p, v, _) in &sim.gun.rockets {
        let d = v.normalize_or(Vec3::Z);
        g.cube(Transform::from_translation(p).looking_to(d, Vec3::Z).with_scale(Vec3::new(0.08, 0.08, 0.5)), Color::srgba(0.4, 0.95, 1.0, 0.9));
        g.line(p - d * 0.25, p - d * 1.2, Color::srgb(1.0, 0.6, 0.2));
    }
    for (at, age) in &mut sim.gun.blasts {
        let a = (1.0 - *age / 0.8).clamp(0.0, 1.0);
        for (k, c) in [(1.0, Color::srgba(1.0, 0.5, 0.15, a)), (0.6, Color::srgba(1.0, 0.8, 0.3, a)), (0.3, Color::srgba(1.0, 1.0, 0.7, a))] {
            g.sphere(Isometry3d::from_translation(*at), BLAST_R * k * (*age / 0.3).min(1.0).max(0.05), c);
        }
        *age += dt;
    }
    sim.gun.blasts.retain(|b| b.1 < 0.8);
    sim.thrown_frame(dt);
    for &(p, _, _, molotov) in &sim.gun.thrown {
        g.sphere(Isometry3d::from_translation(p), 0.05, if molotov { Color::srgb(1.0, 0.6, 0.2) } else { Color::srgb(0.4, 0.95, 1.0) });
    }
    let flick = time.elapsed_secs() * 9.0;
    for &(at, age) in &sim.gun.fires {
        let a = (1.0 - age / FIRE_LIFE).clamp(0.25, 1.0);
        for k in 0..6 {
            let s = (flick + k as f32 * 1.7).sin() * 0.5 + 0.5;
            g.circle(Isometry3d::from_translation(at + Vec3::Z * (0.02 + 0.12 * k as f32 * (0.5 + s))), FIRE_R * (1.0 - 0.14 * k as f32), Color::srgba(1.0, 0.35 + 0.1 * k as f32, 0.1, a));
        }
    }
    if sim.burning > 0.0 {
        let ours = sim.ours();
        for k in 0..ours.parts.len() {
            let s = (flick + k as f32 * 2.3).sin() * 0.5 + 0.5;
            g.sphere(Isometry3d::from_translation(ours.p(k) + Vec3::Z * 0.08 * s), 0.07 + 0.05 * s, Color::srgba(1.0, 0.45 + 0.3 * s, 0.1, 0.9));
        }
    }
    let case = sim.gun_stats().3.max(0.02);
    let mut shells = std::mem::take(&mut sim.gun.shells);
    for (p, v, left) in &mut shells {
        v.z -= 9.8 * dt;
        if v.z < 0.0 && sim.world.contact(*p, Vec3::NEG_Z, -v.z * dt + 0.005).is_some() {
            *v = Vec3::new(v.x * 0.5, v.y * 0.5, -v.z * 0.35);
        }
        *p += *v * dt;
        *left -= dt;
        let spin = Quat::from_axis_angle(v.normalize_or(Vec3::X).cross(Vec3::Z).normalize_or(Vec3::X), *left * 12.0);
        g.cube(Transform::from_translation(*p).with_rotation(spin).with_scale(Vec3::new(case * 0.4, case * 0.4, case)),
                 Color::srgba(0.4, 0.95, 1.0, (*left).clamp(0.0, 1.0) * 0.8));
    }
    shells.retain(|s| s.2 > 0.0);
    sim.gun.shells = shells;
}

#[derive(Component)]
struct Smooth {
    prev: Transform,
    cur: Transform,
    step: u64,
}

fn smooth(sim: NonSend<Sim>, mut commands: Commands,
          new: Query<(Entity, &Transform), (Or<(With<PartMesh>, With<FacePart>, With<HeroMesh>, With<TaxiMesh>, With<TaxiWheel>, With<CarMesh>)>, Without<Smooth>)>,
          mut q: Query<(&mut Transform, &mut Smooth)>) {
    for (e, t) in &new {
        commands.entity(e).insert(Smooth { prev: *t, cur: *t, step: sim.steps });
    }
    let a = if sim.paused || sim.held || sim.replay.is_some() { 1.0 } else { (sim.acc * 60.0).clamp(0.0, 1.0) };
    for (mut t, mut s) in &mut q {
        if s.step != sim.steps {
            (s.prev, s.cur, s.step) = (s.cur, *t, sim.steps);
            #[cfg(feature = "portal")]
            if s.prev.translation.distance(s.cur.translation) > 0.5 {
                s.prev = s.cur;
            }
        } else if *t != s.cur {
            (s.prev, s.cur) = (*t, *t);
        }
        *t = Transform { translation: s.prev.translation.lerp(s.cur.translation, a), rotation: s.prev.rotation.slerp(s.cur.rotation, a),
                         scale: s.prev.scale.lerp(s.cur.scale, a) };
    }
}

#[derive(Component)]
struct TaxiMesh(usize);
#[derive(Component)]
struct TaxiWheel(usize, usize, Vec3);
const WHEELS: [&str; 4] = ["wheel_lf", "wheel_rf", "wheel_lr", "wheel_rr"];

fn taxi_mesh(sim: NonSend<Sim>, mut commands: Commands, assets: Res<AssetServer>, mut q: Query<(Entity, &TaxiMesh, &mut Transform), Without<TaxiWheel>>,
             mut wheels: Query<(&TaxiWheel, &mut Transform), Without<TaxiMesh>>, named: Query<(Entity, &Name, &Transform), (Added<Name>, Without<TaxiWheel>, Without<TaxiMesh>)>,
             meshes: Query<Entity, Added<Mesh3d>>, up: Query<&ChildOf>) {
    let mut have = vec![false; sim.taxis.len()];
    for (e, m, mut t) in &mut q {
        match sim.taxis.get(m.0) {
            Some(c) => (*t, have[m.0]) = (Transform::from_translation(c.pos).with_rotation(c.rot), true),
            None => commands.entity(e).despawn(),
        }
    }
    for k in (0..have.len()).filter(|&k| !have[k]) {
        commands.spawn((Transform::default(), Visibility::Visible, TaxiMesh(k)))
            .with_child((SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset("embedded://taxi.glb"))), Transform::from_xyz(0.0, -0.071, -TAXI_RADIUS)));
    }
    let car_of = |e: Entity| up.iter_ancestors(e).find_map(|a| q.get(a).ok().map(|m| m.1.0));
    for e in &meshes {
        if car_of(e).is_some() {
            commands.entity(e).insert(RenderLayers::layer(1));
        }
    }
    for (e, name, t) in &named {
        if let (Some(k), Some(i)) = (car_of(e), WHEELS.iter().position(|w| name.as_str() == *w)) {
            commands.entity(e).insert(TaxiWheel(k, i, t.translation));
        }
    }
    for (w, mut t) in &mut wheels {
        let Some(c) = sim.taxis.get(w.0) else { continue };
        let (lift, roll) = c.wheel(w.1);
        let steer = if w.1 < 2 { c.steer } else { 0.0 };
        *t = Transform::from_translation(w.2 + Vec3::Z * lift).with_rotation(Quat::from_rotation_z(steer) * Quat::from_rotation_x(roll));
    }
}

#[derive(Component)]
struct CarMesh(usize);

fn car_at(c: Vec3, v: Vec3) -> Transform {
    Transform::from_translation(c).with_rotation(Quat::from_rotation_arc(Vec3::Y, v.normalize_or(Vec3::Y)))
}

#[derive(Component)]
struct PusherMesh;

fn pusher_mesh(sim: NonSend<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
               mut q: Query<(&mut Transform, &mut Visibility), With<PusherMesh>>) {
    let Ok((mut t, mut v)) = q.single_mut() else {
        let mat = mats.add(StandardMaterial { base_color: Color::srgb(0.6, 0.62, 0.66), perceptual_roughness: 0.6, ..default() });
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(1.4, 0.5, 1.8))), MeshMaterial3d(mat), Transform::default(), Visibility::Hidden, RenderLayers::layer(1), PusherMesh));
        return;
    };
    *v = match sim.shown().map_or(sim.pusher, |f| f.pusher) {
        Some((c, vel, _)) => {
            *t = Transform::from_translation(c).with_rotation(Quat::from_rotation_arc(Vec3::Y, vel.normalize()));
            Visibility::Visible
        }
        None => Visibility::Hidden,
    };
}

fn car_mesh(sim: NonSend<Sim>, mut commands: Commands, assets: Res<AssetServer>, mut q: Query<(Entity, &CarMesh, &mut Transform)>,
            meshes: Query<Entity, Added<Mesh3d>>, up: Query<&ChildOf>) {
    let cars = sim.shown().map_or(&sim.cars, |f| &f.cars);
    let mut have = vec![false; cars.len()];
    for (e, m, mut t) in &mut q {
        match cars.get(m.0) {
            Some(&(c, v, _)) => (*t, have[m.0]) = (car_at(c, v), true),
            None => commands.entity(e).despawn(),
        }
    }
    for k in (0..have.len()).filter(|&k| !have[k]) {
        commands.spawn((car_at(cars[k].0, cars[k].1), Visibility::Visible, CarMesh(k)))
            .with_child((SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset("embedded://taxi.glb"))), Transform::from_xyz(0.0, 0.0, -0.757)));
    }
    for e in &meshes {
        if up.iter_ancestors(e).any(|a| q.contains(a)) {
            commands.entity(e).insert(RenderLayers::layer(1));
        }
    }
}

fn keys(mut sim: NonSendMut<Sim>, k: Res<ButtonInput<KeyCode>>, mut contexts: EguiContexts) {
    if sim.typing || contexts.ctx_mut().is_ok_and(|ctx| ctx.wants_keyboard_input()) {
        return;
    }
    if k.just_pressed(KeyCode::Space) {
        sim.paused = !sim.paused;
    }
    if k.just_pressed(KeyCode::Period) {
        (sim.paused, sim.acc) = (true, 1.0 / 60.0);
    }
    if k.just_pressed(KeyCode::BracketLeft) {
        sim.speed = (sim.speed * 0.5).max(SPEEDS.0);
    }
    if k.just_pressed(KeyCode::BracketRight) {
        sim.speed = (sim.speed * 2.0).min(SPEEDS.1);
    }
    if k.just_pressed(KeyCode::KeyR) {
        sim.button("RESET pose");
    }
    if k.just_pressed(KeyCode::KeyF) {
        sim.arrows = !sim.arrows;
    }
    if k.just_pressed(KeyCode::KeyX) {
        sim.button("STOP all");
    }
    if k.just_pressed(KeyCode::KeyH) {
        sim.show_ui = !sim.show_ui;
    }
}

fn console(mut sim: NonSendMut<Sim>, mut typed: MessageReader<KeyboardInput>, mut contexts: EguiContexts) {
    if !sim.typing && contexts.ctx_mut().is_ok_and(|ctx| ctx.wants_keyboard_input()) {
        typed.clear();
        return;
    }
    for e in typed.read().filter(|e| e.state.is_pressed()) {
        match &e.logical_key {
            Key::Enter if !sim.typing => sim.typing = true,
            _ if !sim.typing => {}
            Key::Enter => {
                let line = std::mem::take(&mut sim.console);
                sim.typing = false;
                let line = line.trim().to_string();
                sim.both(|_| vec![line.clone()]);
            }
            Key::Backspace => {
                sim.console.pop();
            }
            Key::Escape => {
                sim.console.clear();
                sim.typing = false;
            }
            Key::Tab => {
                let (done, cands) = complete(&sim.console);
                if cands.len() > 1 {
                    let list = cands.join(" ");
                    sim.say(list);
                }
                sim.console = done;
            }
            Key::Character(c) => sim.console.push_str(c),
            Key::Space => sim.console.push(' '),
            _ => {}
        }
    }
}

type RoomCameras = With<OursCam>;

fn room_viewport(rect: egui::Rect, scale: f32, split: bool, right: bool) -> bevy::camera::Viewport {
    let origin = (Vec2::new(rect.min.x, rect.min.y) * scale).as_uvec2();
    let size = (Vec2::new(rect.width(), rect.height()) * scale).as_uvec2().max(UVec2::ONE);
    let left = if split { (size.x / 2).max(1) } else { size.x };
    let offset = if right && split { left } else { 0 };
    bevy::camera::Viewport {
        physical_position: origin + UVec2::new(offset, 0),
        physical_size: UVec2::new(if right && split { size.x.saturating_sub(left).max(1) } else { left }, size.y),
        ..default()
    }
}

fn viewports(sim: NonSend<Sim>, mut contexts: EguiContexts, windows: Query<&Window>, mut cams: Query<(&mut Camera, Has<OursCam>), RoomCameras>) {
    let (Ok(ctx), Ok(window)) = (contexts.ctx_mut(), windows.single()) else { return };
    let rect = if sim.show_ui { ctx.available_rect() } else { ctx.content_rect() };
    if rect.width() < 2.0 || rect.height() < 1.0 || window.physical_width() == 0 || window.physical_height() == 0 { return; }
    for (mut camera, ours) in &mut cams {
        camera.is_active = ours || sim.split();
        camera.viewport = Some(room_viewport(rect, ctx.pixels_per_point(), sim.split(), !ours));
    }
}

fn mouse(mut sim: NonSendMut<Sim>, mut contexts: EguiContexts, buttons: Res<ButtonInput<MouseButton>>, mut last: Local<Option<Vec2>>, scroll: Res<AccumulatedMouseScroll>,
         keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, windows: Query<&Window>,
         motion: Res<bevy::input::mouse::AccumulatedMouseMotion>, mut grab: Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
         mut cam: Query<(&mut Transform, &mut Orbit, &Camera, &GlobalTransform), With<OursCam>>,
         ) {
    let editing = sim.typing || contexts.ctx_mut().is_ok_and(|ctx| ctx.wants_keyboard_input());
    let Ok((mut t, mut o, camera, gt)) = cam.single_mut() else { return };
    let filming = sim.film > 0.0 && !sim.photo;
    let ots = portal_gun(&sim) && sim.hero.is_some() && !sim.driving && sim.hero_doll.is_none() && !filming && !sim.photo;
    let locked = grab.single().is_ok_and(|c| c.grab_mode != bevy::window::CursorGrabMode::None);
    if let Ok(mut c) = grab.single_mut() {
        let lock = ots && !keys.just_pressed(KeyCode::Escape) && (locked || buttons.just_pressed(MouseButton::Left) && !sim.over_ui);
        if lock != locked {
            c.grab_mode = if lock { bevy::window::CursorGrabMode::Locked } else { bevy::window::CursorGrabMode::None };
            c.visible = !lock;
        }
    }
    let ots_look = ots && locked;
    let cur = windows.single().ok().and_then(|w| w.cursor_position()).filter(|_| !filming);
    let delta = match (cur, *last) {
        (Some(c), Some(l)) => (c - l) * sim.sens,
        _ => Vec2::ZERO,
    };
    *last = cur;
    let wheel = match scroll.unit {
        bevy::input::mouse::MouseScrollUnit::Line => scroll.delta.y,
        bevy::input::mouse::MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    }.clamp(-3.0, 3.0) * sim.sens * !filming as u8 as f32;
    if let Some([yaw, pitch, dist]) = sim.cam.take() {
        (o.yaw, o.pitch, o.dist) = (yaw, pitch, dist);
    }
    if let Some((to, secs, at)) = sim.crane_to.take() {
        sim.crane = Some(([o.yaw, o.pitch, o.dist, sim.look_z], to, 0.0, secs, at.map(|a| (o.target, a))));
    }
    if let Some((from, to, t, secs, at)) = sim.crane {
        let t = (t + time.delta_secs() * !sim.photo as u8 as f32).min(secs);
        let k = (t / secs.max(1e-3)).clamp(0.0, 1.0);
        let k = k * k * (3.0 - 2.0 * k);
        let v: [f32; 4] = std::array::from_fn(|i| from[i] + (to[i] - from[i]) * k);
        (o.yaw, o.pitch, o.dist, sim.look_z) = (v[0], v[1], v[2], v[3]);
        if let Some((a, b)) = at {
            (o.target, o.follow) = (a.lerp(b, k), 3);
        }
        sim.crane = (t < secs).then_some((from, to, t, secs, at));
    }
    if let Some(at) = sim.cam_at.take() {
        (o.target, o.follow) = (at, 3);
    }
    if (keys.just_pressed(KeyCode::KeyP) && !editing) || std::mem::take(&mut sim.photo_pending) {
        sim.photo = !sim.photo;
        (sim.show_ui, sim.paused) = (!sim.photo, sim.photo);
        o.follow = if sim.photo && !sim.photo_follow { 3 } else { 0 };
        let on = sim.photo;
        sim.say(format!("room  photo mode {}", if on { "on: WASD/QE move, right-drag turn, wheel zoom, Space play/freeze, K keep the shot, P out" } else { "off" }));
    }
    if sim.photo && keys.just_pressed(KeyCode::KeyL) {
        sim.photo_follow = !sim.photo_follow;
        o.follow = if sim.photo_follow { 0 } else { 3 };
    }
    if sim.photo && keys.just_pressed(KeyCode::KeyK) {
        let c = sim.shown().map_or(sim.agent.doll.com().0, |f| f.links[0].translation);
        let name = if sim.shot_scene.is_empty() { sim.room.stage.clone() } else { sim.shot_scene.clone() };
        let path = format!("cam_shots/{name}.txt");
        let n = std::fs::read_to_string(&path).map_or(0, |t| t.lines().count()) + 1;
        let cam = format!("!cam {:.2} {:.2} {:.2} {:.2}", o.yaw.rem_euclid(std::f32::consts::TAU), o.pitch, o.dist, o.target.z - (c.z - STAND));
        let line = if sim.photo_follow { format!("cam {n}  t {:.2}  follow  {cam}", sim.steps as f32 / 60.0) }
                   else { format!("cam {n}  t {:.2}  locked  {cam}  !camat {:.2} {:.2} {:.2}", sim.steps as f32 / 60.0, o.target.x, o.target.y, o.target.z) };
        use std::io::Write;
        let _ = std::fs::create_dir_all("cam_shots");
        let _ = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| writeln!(f, "{line}"));
        sim.photo_note = format!("{name}: cam {n} kept");
        sim.say(format!("room  {name}: {line}"));
    }
    let flat = Vec3::new(o.yaw.cos(), o.yaw.sin(), 0.0);
    let side = Vec3::Z.cross(flat);
    if !sim.over_ui {
        if ots_look {
            let k = (0.022 * 3.0f32).to_radians();
            o.yaw -= motion.delta.x * k;
            o.pitch = (o.pitch + motion.delta.y * k).clamp(-0.6, 1.2);
        } else if buttons.pressed(MouseButton::Right) {
            o.yaw -= delta.x * 0.005;
            o.pitch = (o.pitch + delta.y * 0.005).clamp(if sim.photo { -1.5 } else { -0.2 }, 1.5);
        }
        if buttons.pressed(MouseButton::Middle) && delta != Vec2::ZERO {
            let k = o.dist * 0.0015;
            o.target += side * delta.x * k + Vec3::Z * delta.y * k;
            o.follow = 3;
        }
        o.dist = (o.dist * (1.0 - wheel * 0.1)).clamp(0.3, 40.0);
    }
    sim.walk_dir = sim.script_walk;
    if !editing {
        let mut mv = Vec3::ZERO;
        for (k, d) in [(KeyCode::KeyW, -flat), (KeyCode::KeyS, flat), (KeyCode::KeyA, -side), (KeyCode::KeyD, side), (KeyCode::KeyE, Vec3::Z), (KeyCode::KeyQ, -Vec3::Z)] {
            if keys.pressed(k) {
                mv += d;
            }
        }
        if sim.hero.is_some() {
            for (k, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8,
                             KeyCode::Digit9, KeyCode::Digit0, KeyCode::Minus, KeyCode::Equal]
                .into_iter().enumerate() {
                if keys.just_pressed(key) {
                    sim.hero_take(k.checked_sub(1));
                }
            }
            let mut gait = if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) { 2 } else { shift_held(&keys) as usize };
            if mv == Vec3::ZERO {
                (mv, gait) = sim.hero.as_ref().map_or((mv, gait), |n| n.go);
            }
            if keys.just_pressed(KeyCode::KeyF) {
                sim.taxi_door();
            }
            if let Some(n) = sim.hero.as_mut() {
                n.keys = [(KeyCode::KeyT, 0x10), (KeyCode::KeyG, 0x80), (KeyCode::KeyV, 0x40), (KeyCode::KeyZ, 0x1)]
                    .into_iter().filter(|k| keys.pressed(k.0)).fold(0, |b, k| b | k.1);
            }
            let dt = if sim.paused || sim.held { 0.0 } else { time.delta_secs() * sim.speed };
            if sim.driving {
                let k = |c| keys.pressed(c) as i32 as f32;
                let (pedal, wheel, hb) = (k(KeyCode::KeyW) - k(KeyCode::KeyS), k(KeyCode::KeyD) - k(KeyCode::KeyA), keys.pressed(KeyCode::Space));
                sim.pedals = (pedal, wheel, hb);
            } else {
                sim.play_hero(dt, mv.with_z(0.0).normalize_or_zero(), gait);
                sim.homer_fight(dt);
            }
            mv = Vec3::ZERO;
        }
        if sim.photo {
            if sim.photo_follow {
                sim.look_z += mv.z * time.delta_secs() * 1.5;
                mv = Vec3::ZERO;
            }
        } else if walks_homer(sim.walk, sim.hero.is_some(), sim.photo, o.follow) {
            sim.walk_dir = if mv == Vec3::ZERO { sim.script_walk } else { mv.with_z(0.0).normalize_or_zero() };
            if mv != Vec3::ZERO {
                o.follow = 0;
            }
            mv = Vec3::ZERO;
        }
        if mv != Vec3::ZERO && moves_camera(sim.photo, o.follow) {
            o.target += mv * time.delta_secs() * if sim.photo && shift_held(&keys) { 7.0 } else { 2.5 };
            o.follow = 3;
        }
        if keys.just_pressed(KeyCode::KeyC) || std::mem::take(&mut sim.relock) {
            o.follow = if o.follow == 0 { 3 } else { 0 };
            sim.say(format!("room  camera: {}", if o.follow == 0 { "each half follows its Homer" } else { "free (ours' half)" }));
        }
    }
    sim.cam_locked = o.follow == 0;
    let k = (time.delta_secs() * if sim.film > 0.0 { sim.film } else { 4.0 }).min(1.0);
    let mut dist = o.dist;
    if o.follow == 0 {
        let at = sim.taxis.iter().find(|t| t.player).map(|t| t.pos).or(sim.hero.as_ref().map(|n| n.feet))
            .unwrap_or_else(|| { let c = sim.shown().map_or(sim.agent.doll.com().0, |f| f.links[0].translation); c.with_z(c.z - STAND) });
        let mut at = at.with_z(at.z + sim.look_z);
        if sim.film > 0.0 && !sim.strict {
            let v = sim.agent.doll.vel(0).0.with_z(0.0);
            sim.film_vel = sim.film_vel.lerp(v, (time.delta_secs() * 1.5).min(1.0));
            at += (sim.film_vel * 0.3).clamp_length_max(1.2);
            if let Some(c) = sim.taxis.iter().map(|t| t.pos.with_z(at.z)).filter(|c| c.distance(at) < 20.0).min_by(|a, b| a.distance(at).total_cmp(&b.distance(at))) {
                dist = dist.max(c.distance(at) * 1.4);
                at = at.lerp(c, 0.4);
            }
        }
        let z = o.target.z + (at.z - o.target.z) * if sim.film > 0.0 && !sim.strict { (time.delta_secs() * 1.5).min(1.0) } else { k };
        o.target = o.target.lerp(at, k).with_z(z);
    }
    let off = Vec3::new(o.yaw.cos() * o.pitch.cos(), o.yaw.sin() * o.pitch.cos(), o.pitch.sin()) * dist;
    let slide = (-off).cross(Vec3::Z).normalize_or_zero() * (dist * 0.12 * sim.show_ui as u8 as f32);
    *t = Transform::from_translation(o.target + off - slide).looking_at(o.target - slide, Vec3::Z);
    #[cfg(feature = "portal")]
    {
        sim.portal.ots = None;
        if let Some(n) = sim.hero.as_ref().filter(|_| ots) {
            let back = off.normalize_or(Vec3::X);
            let right = Vec3::Z.cross(back).normalize_or_zero();
            o.target = n.feet + Vec3::Z * 1.6;
            *t = Transform::from_translation(o.target + back * 2.0 + right * 0.55).looking_to(-back, Vec3::Z);
            sim.portal.ots = ots_look.then_some((t.translation, -back));
        }
    }
    let shake = (sim.gun.kick - 1.0).max(0.0) * 0.012;
    let e = time.elapsed_secs() * 55.0;
    t.rotate_local_x(shake * e.sin());
    t.rotate_local_y(shake * (e * 1.3).cos());
    sim.eye = t.translation;
    let half = camera.logical_viewport_rect().map_or(0.0, |r| r.max.x);
    let hover = cur.filter(|c| !sim.over_ui && c.x < half).and_then(|c| {
        let ray = camera.viewport_to_world(gt, c).ok()?;
        let (from, dir) = (ray.origin, *ray.direction);
        let (d, _, row) = sim.world.contact(from, dir, 60.0)?;
        let homer = sim.agent.doll.probe(&sim.build, from, from + dir * 60.0).map(|p| p.1.distance(from));
        if homer.is_some_and(|f| f < d) {
            return None;
        }
        let (name, fr, el) = sim.room.surfaces.get(row as usize).cloned().unwrap_or(("DEFAULT".into(), 1.0, 0.1));
        Some((c, format!("surface: {name} (friction {fr:.2}, elasticity {el:.2})")))
    });
    sim.hover = hover;
    if let Some((from, dir)) = cur.filter(|c| !sim.over_ui && c.x < half).and_then(|c| camera.viewport_to_world(gt, c).ok()).map(|r| (r.origin, *r.direction)) {
        let d = std::iter::once(&sim.agent).chain(sim.crowd.iter().map(|c| &c.0)).filter_map(|a| a.doll.probe(&sim.build, from, from + dir * 60.0))
            .map(|h| h.1.distance(from)).reduce(f32::min).or_else(|| sim.world.contact(from, dir, 60.0).map(|c| c.0)).unwrap_or(10.0);
        sim.pointer = Some(from + dir * d);
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    sim.throw_at = if ctrl && sim.hero.is_none() { sim.pointer.or(sim.throw_at) } else { None };
    if sim.hero.is_none() && !editing {
        for (key, b) in [(KeyCode::Digit1, "throw ball"), (KeyCode::Digit2, "throw box"), (KeyCode::Digit3, "drop crate")] {
            if keys.just_pressed(key) {
                let keep = sim.throw_at;
                sim.throw_at = sim.pointer.or(keep);
                sim.button(b);
                sim.throw_at = keep;
            }
        }
    }
    sim.pop_screen = sim.pops.iter().filter_map(|&(p, n, age, small)| camera.world_to_viewport(gt, p + Vec3::Z * POP_RISE * age / POP_LIFE).ok().map(|c| (c, n, age, small))).collect();
    if sim.aim_gun && sim.hero.is_none() {
        let ray = cur.filter(|c| !sim.over_ui && c.x < half).and_then(|c| camera.viewport_to_world(gt, c).ok()).map(|r| (r.origin, *r.direction));
        let hold = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
        sim.gun_frame(ray, hold, time.delta_secs());
        let (between, ..) = sim.gun_stats();
        let ok = !sim.over_ui && ray.is_some();
        let down = buttons.pressed(MouseButton::Left) && ok;
        let dt = time.delta_secs();
        sim.gun.held = if down { sim.gun.held + dt } else { 0.0 };
        sim.gun.wait -= dt;
        let pull = if between > 0.0 && between < 200.0 { down } else { buttons.just_pressed(MouseButton::Left) && ok };
        let fatal = shift_held(&keys) && buttons.just_pressed(MouseButton::Right) && ok;
        if pull && sim.gun.wait <= 0.0 || fatal {
            sim.gun.wait = between / 1000.0;
            sim.gun_fire(fatal);
        }
        return;
    }
    let shift = shift_held(&keys);
    let fatal = shift && buttons.just_pressed(MouseButton::Right);
    if let Some(c) = cur {
        let ray = camera.viewport_to_world(gt, c).ok();
        if let (Some(g), Some(ray)) = (sim.grab.as_mut(), ray) {
            let (p, n) = g.plane;
            let along = ray.direction.dot(n);
            if along.abs() > 1e-4 {
                g.target = ray.origin + *ray.direction * ((p - ray.origin).dot(n) / along);
            }
            g.pulling |= c.distance(g.press.0) > 5.0;
        }
    }
    if !buttons.pressed(MouseButton::Left) {
        if let Some(g) = sim.grab.take().filter(|g| !g.pulling) {
            sim.poke(g.part, g.press.2, g.press.1);
        }
    }
    if sim.hero.is_some() && sim.hero_gun && !sim.driving && kind(sim.weapon) > 0 {
        if let Some(n) = sim.hero.as_ref() {
            let (at, tip) = held(sim.weapon).map(|h| n.held_axis(h)).map_or((n.hand, n.hand + n.facing() * 0.1), |(lo, hi, ..)| (lo, hi));
            (sim.gun.at, sim.gun.dir, sim.gun.aim, sim.gun.placed, sim.gun.kick) = (at, (tip - at).normalize_or(Vec3::Z), tip, true, 0.0);
        }
        let ray = cur.filter(|c| !sim.over_ui && c.x < half).and_then(|c| camera.viewport_to_world(gt, c).ok()).map(|r| (r.origin, *r.direction));
        let click = buttons.just_pressed(MouseButton::Left) && ray.is_some() && !sim.over_ui;
        sim.gun.wait -= time.delta_secs();
        if click && kind(sim.weapon) == 2 {
            sim.hero.as_mut().unwrap().punch();
        } else if click && sim.gun.wait <= 0.0 && let (Some((f, d)), Some(n)) = (ray, sim.hero.as_ref()) {
            let (from, aim) = (n.head() + n.facing() * 0.3 + Vec3::Z * 0.2, sim.aim_at(f, d));
            sim.gun.wait = sim.gun_stats().0 / 1000.0;
            sim.throw_weapon(from, aim);
        }
        return;
    }
    if sim.hero.is_some() && sim.hero_gun && !sim.driving {
        let ray = cur.filter(|c| !sim.over_ui && c.x < half).and_then(|c| camera.viewport_to_world(gt, c).ok()).map(|r| (r.origin, *r.direction));
        let aim = match ots_ray(&sim) {
            Some((f, d)) => f + d * sim.world.contact(f, d, 200.0).map_or(50.0, |c| c.0),
            None => ray.map_or(sim.gun.aim, |(f, d)| sim.aim_at(f, d)),
        };
        let (between, ..) = sim.gun_stats();
        let ok = !sim.over_ui && ray.is_some();
        let down = buttons.pressed(MouseButton::Left) && ok;
        let dt = if sim.paused || sim.held { 0.0 } else { time.delta_secs() * sim.speed };
        sim.gun.held = if down { sim.gun.held + dt } else { 0.0 };
        sim.gun.wait -= dt;
        let pull = sim.gun.wait <= 0.0 && if between > 0.0 && between < 200.0 { down } else { buttons.just_pressed(MouseButton::Left) && ok };
        if pull {
            sim.gun.wait = between / 1000.0;
        }
        let fatal = shift_held(&keys) && buttons.just_pressed(MouseButton::Right) && ok;
        let reload = keys.just_pressed(KeyCode::KeyR) && !editing;
        sim.hero_gun_frame(aim, dt, pull, fatal, reload);
        return;
    }
    let click = buttons.just_pressed(MouseButton::Left) && !sim.over_ui && !sim.driving;
    if let Some(n) = sim.hero.as_mut().filter(|_| click) {
        return n.punch();
    }
    let (between, ..) = sim.gun_stats();
    sim.gun.wait -= time.delta_secs();
    let auto = (shift || sim.click_shoots) && between > 0.0 && between < 200.0 && buttons.pressed(MouseButton::Left) && sim.gun.wait <= 0.0;
    if (buttons.just_pressed(MouseButton::Left) || fatal || auto) && !sim.over_ui {
        let Some(cursor) = windows.single().ok().and_then(|w| w.cursor_position()) else { return };
        if camera.logical_viewport_rect().is_some_and(|r| !r.contains(cursor)) {
            return;
        }
        let Ok(ray) = camera.viewport_to_world(gt, cursor) else { return };
        let (from, dir) = (ray.origin, *ray.direction);
        if shift || sim.click_shoots {
            (sim.gun.wait, sim.gun.kick) = (between / 1000.0, KICK[sim.weapon]);
            return sim.fire_from(from, dir, fatal);
        }
        if let Some((part, hit, _)) = sim.agent.doll.probe(&sim.build, from, from + dir * 50.0) {
            if let Some(f) = { let f = sim.agent.doll.frames().nth(part); f } {
                let local = f.rotation.inverse() * (hit - f.translation);
                sim.grab = Some(Grab { part, local, plane: (hit, -dir), target: hit, press: (cursor, dir, hit), pulling: false });
            }
        }
    }
}

fn portal_gun(sim: &Sim) -> bool {
    #[cfg(feature = "portal")]
    return sim.portal.gun;
    #[cfg(not(feature = "portal"))]
    false
}

fn ots_ray(sim: &Sim) -> Option<(Vec3, Vec3)> {
    #[cfg(feature = "portal")]
    return sim.portal.ots;
    #[cfg(not(feature = "portal"))]
    None
}

fn shift_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
}

fn scripted(mut sim: NonSendMut<Sim>, time: Res<Time>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    if sim.photo && sim.paused {
        let d = time.delta_secs();
        sim.script.iter_mut().for_each(|l| l.0 += d);
    }
    while sim.script.front().is_some_and(|l| l.0 <= time.elapsed_secs()) {
        let (_, line) = sim.script.pop_front().unwrap();
        let mut w = line.splitn(2, ' ');
        match (w.next().unwrap_or(""), w.next()) {
            ("!button", Some(b)) => sim.button(b),
            ("!look", Some(n)) => sim.skin = LOOKS.iter().find(|l| l.0 == n).map_or(sim.skin, |l| l.1),
            ("!pedcol", Some(n)) => sim.ped_color = PED_COLORS.iter().position(|c| c.0 == n).unwrap_or(sim.ped_color),
            ("!eyes", _) => sim.eyes = !sim.eyes,
            ("!skel", _) => sim.skel = !sim.skel,
            ("!hero", _) => sim.toggle_hero(),
            ("!punch", _) => if let Some(n) = sim.hero.as_mut() { n.punch() },
            ("!pad", Some(a)) => if let (Some(n), Ok(b)) = (sim.hero.as_mut(), u32::from_str_radix(a.trim_start_matches("0x"), 16)) { n.scripted = b },
            ("!drive", Some(a)) => {
                let v: Vec<f32> = a.split(',').filter_map(|x| x.parse().ok()).collect();
                sim.driving = !sim.taxis.is_empty();
                let which = v.get(2).map_or(0, |&k| k as usize);
                for (k, t) in sim.taxis.iter_mut().enumerate() {
                    (t.player, t.asleep) = (k == which, t.asleep && k != which);
                }
                sim.pedals = (v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), false);
            }
            ("!hero_weapon", Some(i)) => sim.hero_take(i.parse::<usize>().ok().filter(|&i| i < WEAPONS.len())),
            ("!hero_fire", Some(d)) => {
                let v: Vec<f32> = d.split(',').filter_map(|x| x.parse().ok()).collect();
                let aim = sim.ours().p(CHEST) + Vec3::new(v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0));
                match (kind(sim.weapon), sim.hero.as_mut()) {
                    (1, Some(n)) => {
                        let from = n.head() + n.facing() * 0.3 + Vec3::Z * 0.2;
                        sim.throw_weapon(from, aim);
                    }
                    (2, Some(n)) => n.punch(),
                    _ => sim.hero_gun_frame(aim, 0.0, true, false, false),
                }
            }
            ("!hero_go", Some(a)) => if let Some(n) = sim.hero.as_mut() {
                let mut a = a.split_whitespace();
                let v: Vec<f32> = a.next().unwrap_or("").split(',').filter_map(|x| x.parse().ok()).collect();
                n.go = (Vec3::new(v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), 0.0), a.next().and_then(|g| g.parse().ok()).unwrap_or(0));
            },
            ("!poke", Some(k)) => {
                let (k, s) = (k.parse().unwrap_or(CHEST), sim.ours());
                let eye = sim.eye;
                sim.poke(k, s.p(k), s.p(k) - eye);
            }
            ("!grab", Some(a)) => {
                let mut w = a.split_whitespace();
                let part = w.next().and_then(|k| k.parse().ok()).unwrap_or(0);
                let d: Vec<f32> = w.next().unwrap_or("").split(',').filter_map(|x| x.parse().ok()).collect();
                let at = sim.ours().p(part);
                let target = at + Vec3::new(d.first().copied().unwrap_or(0.0), d.get(1).copied().unwrap_or(0.0), d.get(2).copied().unwrap_or(0.0));
                sim.grab = Some(Grab { part, local: Vec3::ZERO, plane: (at, Vec3::Z), target, press: (Vec2::ZERO, Vec3::Z, at), pulling: true });
                let com = sim.agent.doll.com().0;
                sim.say(format!("ours  grab {part} to {} (com {})", v3(target), v3(com)));
            }
            ("!release", _) => {
                sim.grab = None;
                let com = sim.agent.doll.com().0;
                sim.say(format!("ours  release (com {})", v3(com)));
            }
            ("!pause", _) => sim.paused = !sim.paused,
            ("!shoot", Some(a)) => {
                let mut w = a.split_whitespace();
                let part = w.next().and_then(|k| k.parse().ok()).unwrap_or(0);
                sim.fire(part, None, w.next() == Some("fatal"));
            }
            ("!nododge", _) => sim.no_dodge = !sim.no_dodge,
            ("!superpunch", _) => sim.button("super punch"),
            #[cfg(feature = "portal")]
            ("!portal", Some(a)) => sim.portal_script(a),
            ("!gunat", Some(d)) => {
                let v: Vec<f32> = d.split(',').filter_map(|x| x.parse().ok()).collect();
                if let [x, y, z] = v[..] {
                    let chest = sim.ours().p(CHEST);
                    sim.gun_off = Vec3::new(x, y, z);
                    (sim.gun.at, sim.gun.aim, sim.gun_aim_off, sim.gun.placed) = (chest + sim.gun_off, chest, Vec3::ZERO, true);
                }
            }
            ("!gunreach", Some(a)) => {
                let v: Vec<f32> = a.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                (sim.gun_reach, sim.gun_up) = (v.first().copied().unwrap_or(1.0), v.get(1).copied().unwrap_or(0.0));
            }
            ("!soft", _) => sim.soft = !sim.soft,
            ("!oncue", Some(l)) => sim.on_cue.push(l.to_string()),
            ("!camat", Some(a)) => {
                if let [x, y, z] = a.split_whitespace().filter_map(|x| x.parse().ok()).collect::<Vec<f32>>()[..] {
                    sim.cam_at = Some(Vec3::new(x, y, z));
                }
            }
            ("!shot", Some(n)) => sim.shot_scene = n.to_string(),
            ("!photo", _) => sim.photo_pending = true,
            ("!film", Some(a)) => sim.film = a.parse().unwrap_or(0.0),
            ("!crane", Some(a)) => {
                let v: Vec<f32> = a.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if let [yaw, pitch, dist, z, secs, ref at @ ..] = v[..] {
                    sim.crane_to = Some(([yaw, pitch, dist, z], secs, (at.len() == 3).then(|| Vec3::from_slice(at))));
                }
            }
            ("!shadow", Some(a)) => sim.shadow = a.parse().unwrap_or(0.0),
            ("!strict", _) => sim.strict = true,
            ("!cam", Some(a)) => {
                let v: Vec<f32> = a.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if let [yaw, pitch, dist, z] = v[..] {
                    (sim.cam, sim.look_z) = (Some([yaw, pitch, dist]), z);
                }
            }
            ("!throw", Some(a)) => {
                let w: Vec<&str> = a.split(' ').collect();
                let n = w.iter().rposition(|x| x.parse::<f32>().is_err()).map_or(0, |i| i + 1);
                let nums: Vec<f32> = w[n..].iter().filter_map(|x| x.parse().ok()).collect();
                sim.throw_speed = nums.first().copied();
                let kg = sim.room.weight;
                sim.room.weight = kg * nums.get(1).copied().unwrap_or(1.0);
                sim.button(&w[..n].join(" "));
                sim.room.weight = kg;
            }
            ("!taxi_run", Some(a)) => {
                let v: Vec<f32> = a.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let (d, gas) = (v.first().copied().unwrap_or(12.0), v.get(1).copied().unwrap_or(1.0));
                if d > 0.0 {
                    let s = sim.ours();
                    let left = s.left().with_z(0.0).normalize_or(Vec3::X);
                    let back = -s.fwd.with_z(0.0).normalize_or_zero() * v.get(2).copied().unwrap_or(0.0);
                    sim.spawn_taxi_at(s.p(0).with_z(0.0) + left * d + back, -left);
                }
                sim.cab_gas = gas;
            }
            ("!walk", Some(a)) => {
                let v: Vec<f32> = a.split(',').filter_map(|x| x.parse().ok()).collect();
                sim.script_walk = Vec3::new(v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), 0.0).normalize_or_zero();
                sim.walk = sim.script_walk != Vec3::ZERO;
                sim.held = false;
            }
            ("!infinite", _) => sim.infinite = !sim.infinite,
            ("!hide", _) => sim.show_ui = !sim.show_ui,
            ("!tab", Some(n)) => sim.tab = n.parse().unwrap_or(1),
            ("!screenshot", Some(f)) => {
                commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                    .observe(bevy::render::view::screenshot::save_to_disk(f.to_string()));
            }
            ("!scene", Some(n)) => {
                if SCENES.contains(&n) || STAGES.iter().chain(&EXTRAS).any(|t| t.0 == n) {
                    sim.switch_to = Some((n.to_string(), nm() == delirium::nmrs::MARGE));
                }
            }
            ("!gunfire", _) => sim.gun_fire(false),
            ("!weapon", Some(i)) => sim.weapon = i.parse::<usize>().unwrap_or(0).min(WEAPONS.len() - 1),
            ("!aim", Some(m)) => (sim.aim_gun, sim.gun.placed) = (m == "gun", false),
            ("!gun", Some(d)) => {
                let v: Vec<f32> = d.split(',').filter_map(|x| x.parse().ok()).collect();
                let aim = sim.ours().p(CHEST) + Vec3::new(v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0));
                sim.gun_frame(None, false, 0.0);
                sim.gun.aim = aim;
                sim.gun_aim_off = aim - sim.ours().p(CHEST);
                sim.gun.dir = (aim - sim.gun.at).normalize();
                sim.gun_fire(false);
            }
            ("!quit", _) => {
                exit.write(AppExit::Success);
            }
            _ => {
                let l = line.clone();
                sim.both(|_| vec![l.clone()]);
            }
        }
    }
}

impl Sim {
    fn start_walk_learned(&mut self, run: bool) {
        if self.walk_learned.as_ref().is_some_and(|w| w.4 == run) {
            self.walk_learned = None;
            self.send("bodyBalance start=true");
            return self.say("ours  learned locomotion: off".into());
        }
        let path = learn::walk_path(run);
        let Some(net) = learn::Net::load(&path).filter(|n| n.sizes == learn::walk_sizes(&self.build, &self.agent.doll)) else {
            return self.say(format!("training  no compatible {path}; train with --learn-{}", if run { "run" } else { "walk" }));
        };
        let Some(clips) = self.getup.as_ref() else { return self.say("training  the game's movement clips are unavailable".into()) };
        let walk = match learn::Walk::new(clips, run) {
            Ok(w) => w,
            Err(e) => return self.say(format!("training  {e}")),
        };
        let a = match learn::walk_agent(&self.build, &self.world, &self.frames) {
            Ok(a) => a,
            Err(e) => return self.say(format!("training  {e}")),
        };
        self.agent = a;
        self.msgs = Default::default();
        (self.rises, self.still, self.nm_t, self.musc, self.learned) = ([None, None], [0.0; 2], [None, None], None, None);
        (self.held, self.auto_balance, self.dead, self.health, self.homer, self.tasks, self.watch) = (false, false, false, 200.0, None, false, None);
        self.walk_learned = Some((net, learn::zero(&self.build, &self.frames), 0.0, walk.duration, run));
        self.walk = true;
        self.say(format!("ours  learned WASD {}: {path}; use WASD to move, reset or another reaction stops it", if run { "run" } else { "walk" }));
    }

    fn walk_frame(&mut self) {
        let d = self.walk_dir;
        if !self.walk || d == Vec3::ZERO || self.musc.is_some() {
            return;
        }
        let mut st = std::mem::take(&mut self.msgs);
        learn::walk_input(&mut self.agent, &self.build, &self.world, &mut st, d, &mut self.wander, &mut self.rand);
        self.msgs = st;
    }

    fn learned_done(&mut self) {
        if self.learned.as_ref().is_none_or(|l| l.2 < learn::STEPS) {
            return;
        }
        self.learned = None;
        let up = learn::standing(&self.agent.doll);
        let fr: Vec<Transform> = self.agent.doll.frames().collect();
        self.agent = Agent::new(&self.build, Ragdoll::new(&self.build, &fr, Vec3::ZERO), Quat::IDENTITY, 0.3);
        self.msgs = msg::State::default();
        if up {
            self.send("bodyBalance start=true");
        }
        self.say(format!("ours  learned get-up: {}", if up { "standing, balancing" } else { "didn't make it" }));
    }

    fn tick(&mut self) {
        for l in std::mem::take(&mut self.agent.extras.outbox) {
            match l.strip_prefix("#say ") {
                Some(s) => self.say(s.to_string()),
                None => self.send(&l),
            }
        }
        self.bridge();
        self.trampoline();
        self.ledge();
        self.cab();
        if self.sounds.len() > 64 {
            self.sounds.clear();
        }
        let dt = 1.0 / 60.0;
        for b in &mut self.blood {
            b.2 += dt;
            if b.1 != Vec3::ZERO {
                b.1.z -= 9.8 * dt;
                let (d, len) = (b.1.normalize_or(Vec3::NEG_Z), b.1.length() * dt);
                match self.world.contact(b.0, d, len) {
                    Some((t, n, _)) => (b.0, b.1) = (b.0 + d * t + n * 0.002, Vec3::ZERO),
                    None => b.0 += d * len,
                }
                if b.0.z < -50.0 {
                    b.2 = BLOOD_LIFE;
                }
            }
        }
        self.blood.retain(|b| b.2 < BLOOD_LIFE);
        if let Some(part) = self.grab.as_ref().map(|g| g.part) {
            self.light(0, part, TOUCH_GLOW);
        }
        for g in self.glow.iter_mut().flatten() {
            g.0 = (g.0 - g.1 .1 / 60.0).max(0.0);
        }
        self.rings.retain_mut(|r| { r.4 += 1.0 / 60.0; r.4 < RING_LIFE });
        self.ripples.retain_mut(|r| { r.2 += 1.0 / 60.0; r.2 < RIPPLE_LIFE });
        let ours = self.ours();
        self.ripple_wait.resize(ours.parts.len(), 0.0);
        for k in 0..ours.parts.len() {
            self.ripple_wait[k] -= 1.0 / 60.0;
            let (p, vz) = (ours.p(k), self.agent.doll.vel(k).0.z);
            if !self.show_ripples || vz > -RIPPLE_V || self.ripple_wait[k] > 0.0 {
                continue;
            }
            let floor = self.world.contact(p, Vec3::NEG_Z, 0.3).map(|c| p - Vec3::Z * c.0).or((p.z < 0.3).then(|| p.with_z(0.0)));
            if let Some(at) = floor {
                self.ripple_wait[k] = RIPPLE_GAP;
                self.ripples.push((at + Vec3::Z * 0.01, RIPPLE_R * (-vz / 3.0).clamp(0.6, 2.0), 0.0));
            }
        }
        self.pops.retain_mut(|p| { p.2 += 1.0 / 60.0; p.2 < POP_LIFE });
        let hero = self.hero.as_ref().zip(self.getup.as_ref()).filter(|_| !self.driving && self.hero_doll.is_none()).map_or(vec![], |(n, c)| n.capsules(c));
        self.taxi_frame(1.0 / 60.0);
        let car: Vec<_> = self.cars.iter().map(|&(c, v, _)| delirium::ragdoll::Obstacle { at: c, rot: Quat::from_rotation_arc(Vec3::Y, v.normalize()), centre: c,
            half: Vec3::new(0.92, 2.54, 0.6), vel: v, com: c, ..Default::default() })
            .chain(self.pusher.iter().map(|&(c, v, _)| delirium::ragdoll::Obstacle { at: c, rot: Quat::from_rotation_arc(Vec3::Y, v.normalize()), centre: c,
                half: Vec3::new(0.7, 0.25, 0.9), vel: v, com: c, ..Default::default() })).chain(self.taxis.iter().enumerate().flat_map(|(k, t)| [t.obstacle(TAXI_ID + k as i32, TAXI_BELT), t.cabin(TAXI_ID + k as i32, TAXI_BELT)]))
            .chain(self.room.bridge.iter().flat_map(|b| (0..b.planks.len()).map(|i| b.plank(&self.room.w[0], i)).enumerate().map(|(i, t)| delirium::ragdoll::Obstacle {
                id: PLANK_ID + i as i32, at: t.translation, rot: t.rotation, centre: t.translation - Vec3::Z * 1000.0, ..Default::default() }))).collect();
        self.pull();
        self.fire_frame(1.0 / 60.0);
        if self.steps % 2 == 0 {
            self.shot_frame();
            self.blast_frame();
            self.walk_frame();
        }
        let t0 = std::time::Instant::now();
        let learning = self.learned.is_some();
        if self.homer.is_some() && caps_of(&self.build, &self.agent.doll).iter().any(|c| car.iter().any(|o| {
            let back = o.rot.inverse();
            [c.0, (c.0 + c.1) * 0.5, c.1].iter().any(|&p| ((back * (p - o.centre)).abs() - o.half).max_element() < c.2)
        })) {
            self.homer_out(false);
            self.say("homer  knocked out of his fight".into());
        }
        let learning = learning || self.homer.is_some();
        if let Some((net, zero, n, pick)) = self.learned.as_mut() {
            learn::step(net, &net.w, &mut self.agent, &self.build, &self.world, zero, pick);
            *n += 1;
        }
        if let Some((net, zero, phase, duration, _)) = self.walk_learned.as_mut() {
            if self.walk && self.walk_dir != Vec3::ZERO {
                learn::drive_walk(net, &net.w, &mut self.agent, &self.build, zero, phase, *duration);
            } else {
                *phase = 0.0;
                learn::drive(&mut self.agent, &vec![0.0; 3 * (self.build.parts.len() - 1) + 2], zero, &self.build);
            }
        }
        let doll = self.hero_doll.as_mut().filter(|h| h.rise.is_none()).map(|h| &mut h.agent);
        let mut all: Vec<(&mut Agent, usize, &Build, &[_], &[Capsule])> = std::iter::once(&mut self.agent).skip(learning as usize).chain(self.crowd.iter_mut().map(|c| &mut c.0)).chain(doll).enumerate()
            .map(|(i, a)| (a, i, &self.build, &car[..], &hero[..])).collect();
        Agent::step_group(&mut all, &self.world, 1.0 / 60.0, &mut self.group);
        self.car_hits();
        if self.crowd.len() > 4 && self.steps % 300 == 299 {
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            self.say(format!("ours  grid: {} Homers, {ms:.2} ms a step", self.crowd.len() + 1));
        }
        for (c, v, t) in self.cars.iter_mut() {
            (*c, *t) = (*c + *v / 60.0, *t - 1.0 / 60.0);
        }
        self.cars.retain(|c| c.2 > 0.0);
        if let Some((t, at)) = self.pile_fuse.as_mut() {
            *t -= 1.0 / 60.0;
            if *t <= 0.0 {
                let at = *at;
                self.pile_fuse = None;
                self.room.blast(self.on_sides(at), 14.0, 6.0);
                self.explode(at);
            }
        }
        if let Some((c, v, t)) = self.pusher.as_mut() {
            (*c, *t) = (*c + *v / 60.0, *t - 1.0 / 60.0);
            if *t <= 0.0 {
                self.pusher = None;
            }
        }
        if self.room.stage == "street" {
            self.traffic();
        }
        self.steps += 1;
        self.learned_done();
        if self.walk_learned.is_some() && self.agent.doll.frames().nth(HEAD).is_none_or(|f| !f.translation.is_finite() || f.translation.z < 0.8) {
            self.walk_learned = None;
            self.say("ours  learned locomotion: fell; select it again to retry".into());
        }
        self.objects();
        if self.tape.len() == TAPE {
            self.tape.pop_front();
        }
        let props = (0..self.room.objs.len()).map(|j| self.room.obj(0, j).0).collect();
        self.tape.push_back(Frame { caps: self.agent.doll.capsules().collect(), links: self.agent.doll.frames().collect(), props,
                                    cars: self.cars.clone(), pusher: self.pusher });
        {
            let (com, v) = self.agent.doll.com();
            if let Some(r) = self.rec.as_mut() {
                let d = &self.agent.doll;
                let mut low: Vec<(usize, Vec3)> = d.frames().map(|f| f.translation).enumerate().collect();
                low.sort_by(|a, b| a.1.z.total_cmp(&b.1.z));
                let feet: Vec<String> = low.iter().take(2).map(|&(k, p)| {
                    let h = d.held_world(k);
                    format!("p{k} {:.3} {:.3} {:.3} n{} g{:.3} [{}]", p.x, p.y, p.z, h.len(), h.iter().map(|x| x.2).fold(9.0, f32::min),
                            h.iter().map(|x| format!("{:.3},{:.3},{:.3}", x.1.x + 1.5, x.1.y, x.1.z)).collect::<Vec<_>>().join(" "))
                }).collect();
                let crowd: String = self.crowd.iter().map(|(a, _)| { let p = a.doll.com().0; format!(" | crowd {:.3} {:.3} {:.3}", p.x, p.y, p.z) }).collect();
                let _ = writeln!(r, "- - - - - - - | {:.3} {:.3} {:.3} {:.3} {:.3} | {}{crowd}", self.steps as f32 / 60.0, com.x, com.y, com.z, v.length(), feet.join(" | "));
            }
        }
        if self.lean_stop.is_some_and(|t| self.steps as f32 / 60.0 >= t) {
            self.lean_stop = None;
            self.say("room  lean stops".into());
            self.both(|_| vec!["leanInDirection start=false".into()]);
        }
        self.failed[0] |= self.agent.feedback.iter().any(|f| f.slot == 8 && f.name == "balance");
        let fb: Vec<String> = self.agent.feedback.iter().map(|f| format!("ours  FB {} slot {}", f.name, f.slot)).collect();
        for f in fb {
            self.say(f);
        }
    }
}

fn headless(mut sim: Sim, n: usize) {
    sim.button("balance");
    let press = std::env::args().skip_while(|a| a != "--press").nth(1).unwrap_or_default();
    for b in press.split(',').filter(|b| !b.is_empty() && *b != "walk") {
        sim.button(b);
    }
    if press.split(',').any(|b| b == "walk") {
        sim.walk_dir = Vec3::X;
    }
    for i in 0..n {
        if i == 60 {
            sim.button("shot");
        }
        sim.tick();
        sim.rise(1.0 / 60.0);
        let line: Vec<String> = sim.agent.doll.frames().map(|f| {
            let (p, r) = (f.translation, f.rotation);
            [p.x, p.y, p.z, r.x, r.y, r.z, r.w].map(|x| format!("{:08x}", x.to_bits())).join(" ")
        }).collect();
        println!("{}", line.join(" | "));
    }
    if !press.is_empty() {
        for (t, l, _) in &sim.log {
            eprintln!("{t:.2} {l}");
        }
    }
}

fn step(mut sim: NonSendMut<Sim>, time: Res<Time>) {
    let ready = time.elapsed_secs() > 2.0;
    if sim.auto_balance && sim.held && ready {
        sim.auto_balance = false;
        sim.button("balance");
    }
    if let Some(c) = sim.replay {
        let c = c + if sim.paused { 0.0 } else { time.delta_secs() * sim.speed * 60.0 };
        sim.replay = (c < sim.tape.len() as f32).then_some(c);
        if sim.replay.is_none() {
            sim.say("room  replay: done, live again".into());
        }
        return;
    }
    if !sim.paused {
        sim.acc += time.delta_secs() * sim.speed;
    }
    sim.acc = sim.acc.min(0.25);
    if sim.held {
        sim.acc = 0.0;
    }
    while sim.acc >= 1.0 / 60.0 {
        sim.acc -= 1.0 / 60.0;
        sim.tick();
    }
    let dt = if sim.paused { 0.0 } else { time.delta_secs() * sim.speed };
    sim.rise(dt);
    sim.hero_frame(dt);
    let mut news = None;
    if let Some(w) = sim.watch.as_mut() {
        w.t += dt;
        w.check += time.delta_secs();
        if w.t > learn::STEPS as f32 / 60.0 + 1.0 {
            w.t = 0.0;
        }
        if w.check > 1.0 {
            w.check = 0.0;
            if w.reload() {
                let best = w.scores.iter().copied().fold(f32::MIN, f32::max);
                news = Some(format!("training  generation {}: best of these 8 scored {best:.3}", w.generation));
            }
        }
    }
    if let Some(n) = news {
        sim.say(n);
    }
}

fn caps_of(build: &Build, d: &Ragdoll) -> Vec<Capsule> {
    d.capsules().zip(&build.parts).enumerate().map(|(k, (t, p))| {
        let a = t.rotation * Vec3::Y * p.half;
        (t.translation - a, t.translation + a, p.radius, d.motion(k).0, true)
    }).collect()
}

const PED_COLORS: [(&str, &str); 5] = [("mixed", "mixed"), ("blue", "all blue"), ("red", "all red"), ("solid", "solid blue"), ("opaque", "mixed, opaque")];
fn part_color(c: usize, name: &str) -> Color {
    let (red, blue) = (Color::srgba(0.85, 0.3, 0.25, 0.55), Color::srgba(0.3, 0.45, 0.9, 0.55));
    match c {
        4 => part_color(0, name).with_alpha(1.0),
        1 => blue,
        2 => red,
        3 => Color::srgb(0.3, 0.45, 0.9),
        _ if name.ends_with("Left") => red,
        _ if name.ends_with("Right") => blue,
        _ => Color::srgba(0.72, 0.72, 0.74, 0.6),
    }
}
fn bone_tint(c: usize) -> Option<Color> {
    [None, Some(Color::srgb(0.35, 0.55, 1.0)), Some(Color::srgb(1.0, 0.35, 0.3)), Some(Color::srgb(0.35, 0.55, 1.0)), None][c.min(4)]
}

fn side_color(name: &str) -> Color {
    if name.ends_with("Left") {
        Color::srgb(0.9, 0.25, 0.2)
    } else if name.ends_with("Right") {
        Color::srgb(0.25, 0.45, 0.95)
    } else {
        Color::srgb(0.95, 0.8, 0.15)
    }
}

fn draw(sim: NonSend<Sim>, mut g: Gizmos, mut ours: Gizmos<OursGizmos>, ) {
    let dark = sim.skin.sky.iter().sum::<f32>() < 1.0;
    for (j, o) in sim.room.objs.iter().enumerate() {
        if let Some(a) = o.anchor {
            let at = sim.shown().and_then(|f| f.props.get(j)).map_or_else(|| sim.room.obj(0, j).0.translation, |t| t.translation);
            ours.line(a, at, Color::srgb(0.2, 0.2, 0.22));
        }
    }
    let void = dark && sim.skin.fog;
    let me = sim.ours().p(0).with_z(0.0);
    let (o, n, fade) = if sim.infinite { ((me / 0.5).round() * 0.5, 30, 12.0) } else { (Vec3::ZERO, 20, 6.0) };
    for i in (-n..=n).filter(|_| sim.skin.grid) {
        let x = i as f32 * 0.5;
        let c = if dark { Color::srgba(0.4, 0.65, 1.0, 0.2) } else { Color::srgba(0.35, 0.36, 0.4, 0.35) };
        if !void {
            g.line(o + Vec3::new(x, -(n as f32 * 0.5), 0.002), o + Vec3::new(x, (n as f32 * 0.5), 0.002), c);
            g.line(o + Vec3::new(-(n as f32 * 0.5), x, 0.002), o + Vec3::new((n as f32 * 0.5), x, 0.002), c);
            continue;
        }
        for j in -n..n {
            let (y0, y1) = (j as f32 * 0.5, j as f32 * 0.5 + 0.5);
            for (a, b) in [(o + Vec3::new(x, y0, 0.002), o + Vec3::new(x, y1, 0.002)), (o + Vec3::new(y0, x, 0.002), o + Vec3::new(y1, x, 0.002))] {
                let f = (1.0 - ((a + b) * 0.5).distance(me) / fade).clamp(0.0, 1.0);
                if f > 0.0 {
                    g.line(a, b, c.with_alpha(0.35 * f));
                }
            }
        }
    }
    let doll = &sim.agent.doll;
    let up = |i: usize| sim.getup.as_ref().zip(sim.rises[i].as_ref()).and_then(|(c, r)| c.frames(r));
    let ours_up = up(0);
    let rising = ours_up.is_some();
    let fr: Vec<Transform> = sim.shown().map(|f| f.links.clone()).or(ours_up).unwrap_or_else(|| doll.frames().collect());
    if let Some(w) = sim.watch.as_ref() {
        for i in 0..w.tries.len() {
            skeleton(&mut ours, &sim.build, bone_tint(sim.ped_color), |k| w.at(i, k), |_| false);
        }
    } else if sim.skel {
        skeleton(&mut ours, &sim.build, bone_tint(sim.ped_color), |k| fr.get(k).copied(), |k| doll.touch[k] & 0x48 != 0);
    }
    if let Some(p) = sim.throw_at {
        ours.sphere(Isometry3d::from_translation(p), 0.08, Color::srgb(1.0, 0.85, 0.2)).resolution(16);
    }
    for &(at, r, age) in &sim.ripples {
        let t = age / RIPPLE_LIFE;
        let iso = Isometry3d::from_translation(at);
        for (k, d) in [0.0, 0.25, 0.5].into_iter().enumerate() {
            let rk = r * (1.0 - (1.0 - (t - d * 0.4).max(0.0)).powi(2)) * (1.0 - d * 0.3);
            if rk > 0.0 {
                ours.circle(iso, rk, Color::srgba(0.25, 0.85, 1.0, (1.0 - t) * [1.0, 0.6, 0.35][k])).resolution(40);
            }
        }
    }
    for &(ped, k, at, n, age) in &sim.rings {
        let f = if ped == 0 { fr.get(k).copied() } else { sim.crowd.get(ped - 1).and_then(|c| c.0.doll.frames().nth(k)) };
        let Some(f) = f else { continue };
        let (t, n) = (age / RING_LIFE, (f.rotation * n).normalize_or(Vec3::Z));
        let iso = Isometry3d::new(f.translation + f.rotation * at + n * 0.004, Quat::from_rotation_arc(Vec3::Z, n));
        let r = RING_FROM + (RING_TO - RING_FROM) * (1.0 - (1.0 - t) * (1.0 - t));
        for d in [0.0, 0.004] {
            ours.circle(iso, r + d, Color::srgba(1.0, 0.05, 0.05, 1.0 - t)).resolution(32);
        }
    }
    if let Some((g, f)) = sim.grab.as_ref().filter(|g| g.pulling).and_then(|g| Some((g, fr.get(g.part)?))) {
        let at = f.translation + f.rotation * g.local;
        ours.line(at, g.target, Color::srgb(0.3, 0.9, 1.0));
        ours.sphere(Isometry3d::from_translation(g.target), 0.03, Color::srgb(0.3, 0.9, 1.0));
    }
    if sim.arrows {
        for (k, f) in fr.iter().enumerate() {
            let (force, torque) = doll.out[k];
            ours.arrow(f.translation, f.translation + torque * 0.002, Color::srgb(0.9, 0.3, 0.9));
            ours.arrow(f.translation, f.translation + force * 0.0005, Color::srgb(0.3, 0.6, 1.0));
        }
    }
    for (a, _) in sim.crowd.iter().filter(|_| sim.skel) {
        let f: Vec<Transform> = a.doll.frames().collect();
        skeleton(&mut ours, &sim.build, bone_tint(sim.ped_color), |k| f.get(k).copied(), |k| a.doll.touch[k] & 0x48 != 0);
    }
    let (com, v) = doll.com();
    let (com, v) = if rising { (fr[0].translation, Vec3::ZERO) } else { (com, v) };
    if sim.skel {
        ours.sphere(Isometry3d::from_translation(com), 0.04, Color::srgb(1.0, 0.3, 0.2));
        ours.arrow(com.with_z(0.01), (com + v * 0.3).with_z(0.01), Color::srgb(1.0, 0.5, 0.2));
    }
    for (k, p) in sim.build.parts.iter().enumerate().filter(|_| sim.skel) {
        let Some(e) = p.boxed else { continue };
        let c = Color::srgb(0.2, 0.9, 0.95);
        if let Some(f) = fr.get(k) {
            ours.primitive_3d(&Cuboid { half_size: e }, Isometry3d::new(f.translation, f.rotation * delirium::nmdbm::rage_link(nm(), k)), c);
        }
    }
    looks(&sim, &mut ours, fr.get(HEAD).map(|f| f.translation), 0);
}

fn looks<T: GizmoConfigGroup>(sim: &Sim, g: &mut Gizmos<T>, head: Option<Vec3>, side: usize) {
    if let (true, Some(j), Some(h)) = (sim.look, sim.room.objs.len().checked_sub(1), head) {
        let o = sim.room.obj(side, j).0.translation;
        let c = Color::srgb(1.0, 0.85, 0.2);
        for n in (0..20).step_by(2) {
            g.line(h.lerp(o, n as f32 / 20.0), h.lerp(o, (n + 1) as f32 / 20.0), c);
        }
        g.circle(Isometry3d::new(o, Quat::IDENTITY), 0.35, c);
    }
    if sim.avoid {
        for j in 0..sim.room.objs.len() {
            let o = sim.room.obj(side, j).0.translation;
            g.circle(Isometry3d::new(o.with_z(0.01), Quat::IDENTITY), 0.45, Color::srgb(1.0, 0.5, 0.15));
        }
    }
}

fn skeleton<T: GizmoConfigGroup>(g: &mut Gizmos<T>, build: &Build, tint: Option<Color>, frame: impl Fn(usize) -> Option<Transform>, touching: impl Fn(usize) -> bool) {
    let line = tint.unwrap_or(Color::WHITE);
    let joint = |k: usize| frame(k).map(|f| if build.parts[k].parent.is_some() { f.translation + f.rotation * build.parts[k].anchor } else { f.translation });
    for (k, p) in build.parts.iter().enumerate() {
        let (Some(f), Some(x)) = (frame(k), joint(k)) else { continue };
        let c = tint.unwrap_or_else(|| side_color(delirium_data::nm_asset::PARTS.get(k).unwrap_or(&"")));
        if let Some(y) = p.parent.and_then(&joint) {
            g.line(x, y, line);
        }
        if p.parent.is_some() && !build.parts.iter().any(|c| c.parent == Some(k)) {
            let tip = f.translation * 2.0 - x;
            g.line(x, tip, line);
            g.sphere(Isometry3d::from_translation(tip), 0.015, c);
        }
        g.sphere(Isometry3d::from_translation(x), 0.022, c);
        if touching(k) {
            g.sphere(Isometry3d::from_translation(f.translation), 0.045, Color::srgb(1.0, 0.85, 0.1));
        }
    }
}

const RESET_TIP: &str = "Stand Homer back up (R)";
const TIPS: [(&str, &str); 37] = [
    ("throw ball", "Throws a ball at Homer. Hold Ctrl and point first: it goes there instead."), ("throw box", "Throws a box at Homer. Hold Ctrl and point first: it goes there instead."),
    ("drop crate", "Drops a heavy crate on Homer."), ("crowd", "Four more peds beside Homer, balancing. Again: take them away."), ("grid", "24 more peds in a grid round Homer. Again: take them away."),
    ("car", "A taxi comes at Homer from his left at 8 m/s and hits him."),
    ("#ripples", "Cyan rings on the floor where a body part lands hard."),
    ("#skel", "The skeleton's lines and joints drawn over Homer's body."),
    ("#eyes", "Homer's eyes: they look where he's reacting to, blink, squint when he's hurt and shut when he's out."),
    ("#brows", "Homer's eyebrows: they show when he's scared, hurt or stumbling."),
    ("explosion pile", "Barrels and crates in front of Homer that blow up 2 s later."), ("wrecking ball", "A 250 kg ball on a rope swings through Homer."),
    ("push front", "A slab slides into Homer from the front; he braces against it."), ("push back", "A slab shoves Homer from behind; his balance catches it."), ("taxi", "4 parked taxis to drive (with Hero: F gets in and out, W / S drive, A / D steer)."),
    ("clear taxis", "Takes the parked taxis away."),
    ("fall", "Lifts Homer 3 m and lets him fall."), ("clear", "Takes away thrown balls, boxes and crates."), ("ice", "Makes the floor ice. Again: back to concrete."),
    ("replay", "Plays the last 10 s again (the speed buttons slow it down). Press again to stop."),
    ("Hero", "Play as Hero beside Homer: WASD walk, Shift run, Ctrl sprint, click punch; walk into Homer to push him. Again: take Hero out."),
    ("homer weapon", "What Homer fights back with: fists, a bat, a knife or a gun (click to change). With a gun he stands off and shoots the Hero."),
    ("super punch", "Bonus: the Hero's punches that land go off like a grenade and throw Homer."),
    ("hero physics", "On: blows, shots and blasts put the Hero on his own ragdoll, and he gets up after. Off: he plays the hit animations."),
    ("passive", "On: Homer only takes hits. Off: near the Hero he fights back (walks up, punches, kicks); a hit puts him on his ragdoll a while."),
    ("walk", "WASD steers Homer through his balance: he leans that way and steps to follow, and can trip."),
    ("game tasks", "NM -> animation (the game's tasks).\nOn: when the game's task would end (a shot, a balance, a fall...), Homer goes back to his animation: standing, or getting up if he's lying.\nOff: he stays in Delirium; he only gets up after lying still a while."),
    ("get-up", "When he lies still: animation = the game's get-up clip; muscles = his muscles try the clip's poses (he can fail); off = he stays down; learned = the trained network."),
    ("#pops", "Numbers for the damage each hit does, rising from where it landed."), ("#help", "The first-start tips again."),
    ("#sound", "8-bit sounds for shots, hits, explosions, punches and crashes."), ("#blood", "Little square blood pixels from each hit; they land and fade."),
    ("watch training", "Shows the get-up training live: each generation's 8 tries side by side. Again: back to Homer."),
    ("watch walk training", "Shows eight live walk training attempts. The trainer holds WASD forward and learns muscle control over the existing balancer."),
    ("watch run training", "Shows eight live run training attempts, continuing from the walk network."),
    ("walk learned", "Use walk.net to improve Homer's WASD movement. Follow Homer with the camera and use WASD. Again: ordinary movement."),
    ("run learned", "Use run.net to improve Homer's WASD running. Again: ordinary movement."),
];
const DIM: egui::Color32 = egui::Color32::from_rgb(170, 190, 215);
const HI: egui::Color32 = egui::Color32::from_rgb(130, 210, 255);
const INK: egui::Color32 = egui::Color32::from_rgb(245, 245, 248);
const ON: egui::Color32 = egui::Color32::from_rgb(53, 107, 183);

fn per_row(ui: &egui::Ui, w: f32) -> usize {
    ((ui.available_width() + 10.0) / (w + 10.0)).floor().max(1.0) as usize
}

fn slide(ui: &mut egui::Ui, name: &str, x: &mut f32, lo: f32, hi: f32, unit: &str) -> egui::Response {
    ui.add(egui::Slider::new(x, lo..=hi).logarithmic(true).text(name).suffix(unit))
}

fn info(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(egui::RichText::new(text).color(DIM)).wrap());
}

fn walks_homer(walk: bool, hero: bool, photo: bool, follow: u8) -> bool {
    walk && !hero && !photo && follow != 3
}

fn moves_camera(photo: bool, follow: u8) -> bool {
    photo || follow == 3
}

fn section_title(t: &str) -> egui::RichText {
    egui::RichText::new(t).family(egui::FontFamily::Name("big".into())).size(22.0).color(INK)
}

const NAV: [(u8, &str); 7] = [(2, "Scene"), (3, "Character"), (0, "Reactions"), (1, "Interact"),
                                     (4, "View"), (6, "Inspect"), (5, "Help")];

fn active_tab(bits: u8) -> u8 {
    if bits & 0x7f != 0 { 1 << bits.trailing_zeros() } else if bits & 0x80 != 0 { 1 << 2 } else { 1 }
}

fn bar(ui: &mut egui::Ui, act: &[f32], des: &[f32], strength: f32) {
    let err: f32 = act.iter().zip(des).map(|(a, d)| (a - d).abs()).sum();
    let x = (err * strength / 120.0).clamp(0.0, 1.0);
    let (r, _) = ui.allocate_exact_size(egui::vec2(40.0, 8.0), egui::Sense::hover());
    let c = if x < 0.33 { egui::Color32::from_rgb(80, 200, 80) } else if x < 0.66 { egui::Color32::from_rgb(230, 190, 50) } else { egui::Color32::from_rgb(220, 60, 50) };
    ui.painter().rect_filled(egui::Rect::from_min_size(r.min, egui::vec2(40.0 * x.max(0.03), 8.0)), 0.0, c);
}

fn deg(v: &[f32]) -> String {
    v.iter().map(|x| format!("{:>5.0}", x.to_degrees())).collect::<String>()
}

fn ui(mut contexts: EguiContexts, mut sim: NonSendMut<Sim>, mut log_open: Local<bool>, mut joints_open: Local<bool>, mut fonts: Local<bool>, mut motion: Local<(u64, Vec3, f32)>) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*fonts {
        *fonts = true;
        let mut f = egui::FontDefinitions::default();
        f.font_data.insert("archivo".into(), std::sync::Arc::new(egui::FontData::from_static(include_bytes!("archivo-black.ttf"))));
        let rest = f.families[&egui::FontFamily::Proportional].clone();
        f.families.insert(egui::FontFamily::Name("big".into()), std::iter::once("archivo".to_string()).chain(rest).collect());
        ctx.set_fonts(f);
        return Ok(());
    }
    ctx.style_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(21.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        s.spacing.item_spacing = egui::vec2(10.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 8.0);
        s.spacing.interact_size.y = 34.0;
        s.spacing.slider_width = 140.0;
        s.spacing.scroll = egui::style::ScrollStyle::solid();
        let mut v = egui::Visuals::dark();
        v.override_text_color = Some(INK);
        v.window_fill = egui::Color32::from_rgb(24, 29, 38);
        v.panel_fill = egui::Color32::from_rgb(20, 25, 33);
        v.window_corner_radius = egui::CornerRadius::same(8);
        v.window_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(58, 68, 83));
        v.extreme_bg_color = egui::Color32::from_rgb(15, 19, 26);
        v.selection.bg_fill = ON;
        v.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);
        v.hyperlink_color = HI;
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = egui::CornerRadius::same(5);
            w.fg_stroke = egui::Stroke::new(1.0_f32, INK);
        }
        v.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(34, 42, 55);
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(65, 77, 95));
        v.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(49, 65, 88);
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, INK);
        s.visuals = v;
    });
    if sim.show_pops {
        let paint = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, "pops".into()));
        for &(c, n, age, small) in &sim.pop_screen {
            let a = (1.0 - age / POP_LIFE).clamp(0.0, 1.0);
            let big = n >= 50.0;
            let font = egui::FontId::proportional(if small { 15.0 } else if big { 26.0 } else { 20.0 });
            let text = format!("-{n:.0}");
            paint.text(egui::pos2(c.x + 1.5, c.y + 1.5), egui::Align2::CENTER_CENTER, &text, font.clone(), egui::Color32::from_black_alpha((a * 200.0) as u8));
            paint.text(egui::pos2(c.x, c.y), egui::Align2::CENTER_CENTER, &text, font,
                       egui::Color32::from_rgba_unmultiplied(255, if big { 60 } else { 210 }, 60, (a * 255.0) as u8));
        }
    }
    if !sim.show_ui {
        if sim.film > 0.0 && !sim.photo {
            sim.over_ui = false;
            return Ok(());
        }
        egui::Area::new("hint".into()).anchor(egui::Align2::LEFT_TOP, [6.0, 6.0]).show(ctx, |ui| {
            let t = if sim.photo { format!("PHOTO  {}
{}  (L switch)
WASD/QE move (Shift fast)  right-drag turn  wheel zoom  Space play/freeze  K keep camera  P exit
{}",
                                              sim.shot_scene, if sim.photo_follow { "FOLLOWING HIM (Q/E: look higher/lower)" } else { "LOCKED CAMERA" }, sim.photo_note) } else { "H: show panels".into() };
            ui.label(egui::RichText::new(t).monospace().color(if sim.photo { egui::Color32::WHITE } else { egui::Color32::BLACK }));
        });
        sim.over_ui = false;
        return Ok(());
    }
    let mono = |s: String| egui::RichText::new(s).monospace();
    let mut pressed: Option<String> = None;

    sim.tab = active_tab(sim.tab);
    egui::TopBottomPanel::top("playback").exact_height(58.0).show(ctx, |ui| {
        ui.horizontal_centered(|ui| {
            ui.label(egui::RichText::new("DELIRIUM").family(egui::FontFamily::Name("big".into())).size(20.0).color(HI));
            ui.label(egui::RichText::new("/ Playroom").color(DIM));
            ui.separator();
            if ui.button(if sim.paused { "Resume" } else { "Pause" }).on_hover_text("Pause or resume (Space)").clicked() {
                sim.paused = !sim.paused;
            }
            if ui.add_enabled(sim.paused, egui::Button::new("Step")).on_hover_text("Advance one simulation step (.)").clicked() {
                (sim.paused, sim.acc) = (true, 1.0 / 60.0);
            }
            if ui.button("Reset pose").on_hover_text(RESET_TIP).clicked() { pressed = Some("RESET pose".into()); }
            if ui.button("Stop reactions").on_hover_text("Stop every active behaviour (X)").clicked() { pressed = Some("STOP all".into()); }
            ui.separator();
            if ui.add_enabled(sim.speed > SPEEDS.0, egui::Button::new("Slower")).on_hover_text("Halve simulation speed ([)").clicked() { pressed = Some("#slower".into()); }
            ui.label(egui::RichText::new(format!("{}× · {}", sim.speed, if sim.speed == 1.0 { "Real time" } else if sim.speed < 1.0 { "Slow motion" } else { "Fast forward" })).strong().color(HI));
            if ui.add_enabled(sim.speed != 1.0, egui::Button::new("1×")).on_hover_text("Return to real-time speed").clicked() { pressed = Some("#realtime".into()); }
            if ui.add_enabled(sim.speed < SPEEDS.1, egui::Button::new("Faster")).on_hover_text("Double simulation speed (])").clicked() { pressed = Some("#faster".into()); }
        });
    });
    egui::TopBottomPanel::bottom("status").exact_height(38.0).show(ctx, |ui| {
        let velocity = sim.agent.doll.com().1;
        if sim.steps > motion.0 {
            motion.2 = (velocity - motion.1).length() / ((sim.steps - motion.0) as f32 / 60.0);
            (motion.0, motion.1) = (sim.steps, velocity);
        } else if sim.steps < motion.0 {
            *motion = (sim.steps, velocity, 0.0);
        }
        ui.horizontal(|ui| {
            let compact = ui.available_width() < 1250.0;
            ui.colored_label(if sim.paused { HI } else { egui::Color32::from_rgb(128, 218, 177) }, if sim.paused { "Paused" } else { "Live simulation" });
            ui.separator();
            ui.label(egui::RichText::new(format!("{:.1} s", sim.steps as f32 / 60.0)).monospace().color(DIM));
            ui.separator();
            ui.label(egui::RichText::new(format!("Speed: {:.2} m/s   Acceleration: {:.2} m/s²", velocity.length(), motion.2)).monospace().color(HI))
                .on_hover_text("Homer's centre-of-mass speed and change in velocity per simulation second, sampled between displayed frames. Pause holds the last reading; playback speed does not change the units.");
            ui.separator();
            if ui.selectable_label(*log_open || sim.typing, if compact { "Console" } else { "Activity & console" }).clicked() { *log_open = !*log_open; }
            if ui.selectable_label(sim.replay.is_some(), if compact { "Replay" } else { "Replay last action" }).on_hover_text(TIPS.iter().find(|t| t.0 == "replay").map_or("", |t| t.1)).clicked() { pressed = Some("replay".into()); }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(if compact { "Hide · H" } else { "Hide interface  ·  H" }).clicked() { sim.show_ui = false; }
                if ui.available_width() > 260.0 {
                    ui.label(egui::RichText::new("Right-drag: orbit   ·   Wheel: zoom").small().color(DIM));
                }
            });
        });
    });
    if *log_open || sim.typing {
    egui::TopBottomPanel::bottom("activity").resizable(true).default_height(180.0).height_range(100.0..=320.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.strong("Activity & console");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Close").clicked() { *log_open = false; }
            });
        });
        ui.label(egui::RichText::new("MSG = command sent   ·   FB = engine feedback").small().color(DIM));
        egui::ScrollArea::both().id_salt("activity log").stick_to_bottom(true).auto_shrink([false, false]).max_height((ui.available_height() - 36.0).max(30.0)).show(ui, |ui| {
            for (t, l, n) in &sim.log {
                let c = if l.starts_with("FB ") { egui::Color32::from_rgb(240, 170, 90) } else if l.starts_with("MSG ") { egui::Color32::from_rgb(150, 190, 240) }
                        else if l.starts_with("room ") { egui::Color32::from_rgb(140, 220, 140) } else { egui::Color32::from_rgb(200, 200, 205) };
                let count = if *n > 1 { format!("  x{n}") } else { String::new() };
                ui.label(mono(format!("{:02}:{:05.2} {l}{count}", (*t / 60.0) as u32, t % 60.0)).color(c));
            }
        });
        let hint = if sim.typing { complete(&sim.console).1.iter().take(8).cloned().collect::<Vec<_>>().join(" ") } else { String::new() };
        if sim.typing {
            ui.label(mono(format!("> {}_   {hint}", sim.console)));
        } else {
            ui.label(egui::RichText::new("Press Enter to type a command. Type ? to list commands; Tab completes a name.").small().color(DIM));
        }
    });

    }

    egui::Window::new("Joint inspector").open(&mut *joints_open).default_pos([430.0, 80.0]).default_size([740.0, 500.0]).resizable(true).scroll([true, true]).show(ctx, |ui| {
        let names: Vec<String> = delirium::nmrs::ASSET[nm()].get().map(|a| {
            let mut v: Vec<(u32, String)> = a.iter().map(|j| (j.index, j.name.clone())).collect();
            v.sort();
            v.into_iter().map(|j| j.1).collect()
        }).unwrap_or_default();
        let a = &sim.agent;
        let fr: Vec<Transform> = a.doll.frames().collect();
        let ch = a.character();
        info(ui, "Select a joint for its muscle settings and limits. Angles are in degrees: actual to target.");
        ui.spacing_mut().interact_size.y = 24.0;
        egui::Grid::new("jt").striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
            ui.strong("Joint");
            for label in ["Delirium angles", "Drive", "Strength / damping"] { ui.strong(label); }
            ui.end_row();
            for (j, n) in names.iter().enumerate() {
                if ui.selectable_label(sim.sel == j, mono(format!("{n:<17}"))).clicked() {
                    pressed = Some(format!("#sel {j}"));
                }
                match ch.and_then(|c| c.eff.get(j).map(|e| (c, e))) {
                    Some((c, e)) => {
                        let n = if c.joints[j].three { 3 } else { 1 };
                        let act = c.actual(j, &fr);
                        ui.label(mono(format!("{} ->{}", deg(&act[..n]), deg(&e.desired[..n]))));
                        bar(ui, &act[..n], &e.desired[..n], e.strength);
                        ui.label(mono(format!("{:>3.0}/{:<3.0}", e.strength, e.damping)).color(DIM));
                    }
                    None => {
                        ui.label("-");
                        ui.label("");
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
        if let Some(c) = ch {
            let j = sim.sel.min(c.joints.len().saturating_sub(1));
            let jd = &c.joints[j];
            let pn = |k: usize| delirium_data::nm_asset::PARTS.get(k).copied().unwrap_or("?");
            let e = &c.eff[j];
            ui.separator();
            ui.label(mono(format!("{}: parent {} -> child {}", names.get(j).map_or("?", |s| s.as_str()), pn(jd.parent), pn(jd.child))).color(HI));
            ui.label(mono(format!("ours: muscleStiffness {:.1}  strength {:.1}  damping {:.1}  opposeGravity {:.2}", e.muscle_stiffness, e.strength, e.damping, e.oppose_gravity)));
            let act = c.actual(j, &fr);
            for (k, axis) in ["lean1", "lean2", "twist"].iter().enumerate().take(if jd.three { 3 } else { 1 }) {
                let (lo, hi) = jd.limits[k];
                ui.horizontal(|ui| {
                    ui.label(mono(format!("{axis:<6}[{:>6.1} ..{:>6.1}]", lo.to_degrees(), hi.to_degrees())));
                    let (r, _) = ui.allocate_exact_size(egui::vec2(260.0, 10.0), egui::Sense::hover());
                    let p = ui.painter();
                    p.rect_filled(r, 2.0, egui::Color32::from_gray(70));
                    let at = |x: f32| {
                        let lo2 = lo.min(hi) - 0.3;
                        let hi2 = hi.max(lo) + 0.3;
                        r.min.x + (x - lo2) / (hi2 - lo2) * r.width()
                    };
                    p.rect_filled(egui::Rect::from_x_y_ranges(at(lo.min(hi))..=at(hi.max(lo)), r.y_range()), 2.0, egui::Color32::from_gray(110));
                    let tick = |x: f32, c: egui::Color32, w: f32| p.rect_filled(egui::Rect::from_center_size(egui::pos2(at(x), r.center().y), egui::vec2(w, 12.0)), 0.0, c);
                    tick(act[k], egui::Color32::WHITE, 3.0);
                    tick(e.desired[k], egui::Color32::YELLOW, 3.0);
                });
            }
            ui.label(mono("white actual, yellow desired;  bar: limits".into()).color(DIM));
        }
    });

    egui::SidePanel::left("control dock").resizable(true).default_width(400.0).width_range(380.0..=480.0).frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(16.0)).show(ctx, |ui| {
        let a = &sim.agent;
        let state = [(a.balancing(), "balancing"), (a.fallen, "fallen"), (a.settled(), "settled"), (a.relaxing(), "relaxing"), (a.catching(), "catching his fall"),
                     (a.rolled_up(), "rolled up"), (!a.doll.awake(), "asleep")].iter().filter(|s| s.0).map(|s| s.1).collect::<Vec<_>>().join(", ");
        let (com, v) = a.doll.com();
        let joints = delirium::nmrs::ASSET[nm()].get().map_or(0, |j| j.len());
        let parts = &sim.build.parts;
        let table = joints + 1 == parts.len() && parts.iter().skip(1).all(|p| p.parent.is_some());
        let masses = delirium::nmrs::MASSES[nm()].get().is_some_and(|m| m.len() == parts.len() && m.iter().enumerate().all(|(k, a)| (a - sim.agent.doll.part_mass(k)).abs() < 1e-3));
        let yes = |b: bool| if b { "yes" } else { "NO" };
        let checks = format!("joint table matches Fred.xrt: {} ({joints} joints)\npart masses match: {}", yes(table), yes(masses));
        let ours = format!("ours  t {:.2}s  {}x{}\nCOM ({:.2}, {:.2}, {:.2})  |v| {:.2} m/s\nmass {:.1} kg  KE {:.0} J", sim.steps as f32 / 60.0, sim.speed,
                           if sim.paused { " paused" } else { "" }, com.x, com.y, com.z, v.length(), a.doll.mass(), a.doll.kinetic());
        let red = egui::Color32::from_rgb(255, 110, 160);
        let wide = |ui: &mut egui::Ui, text: String, fill: egui::Color32| {
            let text = egui::RichText::new(text).color(if fill == red { red } else { INK });
            ui.add_sized([170.0, 42.0], egui::Button::new(text).selected(fill == ON).wrap())
        };
        let (stripe, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::hover());
        for (i, color) in [egui::Color32::from_rgb(255, 140, 100), egui::Color32::from_rgb(250, 220, 110), egui::Color32::from_rgb(120, 225, 175), HI, egui::Color32::from_rgb(185, 155, 250)].into_iter().enumerate() {
            let start = stripe.left() + stripe.width() * i as f32 / 5.0;
            ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(start, stripe.top()), egui::vec2(stripe.width() / 5.0, 4.0)), 0.0, color);
        }
        ui.label(section_title(if nm() == delirium::nmrs::MARGE { "Marge" } else { "Homer" }));
        ui.label(egui::RichText::new(if state.is_empty() { "Ready" } else { &state }).color(DIM));
        let hp = if sim.god { 1.0 } else { (sim.health / 200.0).clamp(0.0, 1.0) };
        ui.add(egui::ProgressBar::new(hp).desired_height(8.0).fill(egui::Color32::from_rgb(93, 194, 159)));
        ui.label(egui::RichText::new(if sim.god { "Health: unlimited".to_string() } else { format!("Health: {:.0} / 200", sim.health) }).small().color(DIM));
        let held = sim.held;
        if held {
            ui.label(egui::RichText::new("Pose held. Choose a reaction to begin.").color(HI));
        }
        if sim.dead {
            ui.label(egui::RichText::new("Dead. Use Reset pose in the toolbar to revive.").color(HI));
        } else if sim.health < 100.0 {
            ui.label(egui::RichText::new("Low health. Further shots can make him collapse.").color(HI));
        }
        ui.add_space(4.0);
        ui.separator();
        let nav_width = (ui.available_width() - 18.0) / 4.0;
        egui::Grid::new("section navigation").num_columns(4).spacing([6.0, 6.0]).show(ui, |ui| {
            ui.spacing_mut().button_padding.x = 6.0;
            for (i, &(bit, name)) in NAV.iter().enumerate() {
                if ui.add_sized([nav_width, 34.0], egui::Button::new(egui::RichText::new(name).size(14.0)).wrap().selected(sim.tab == 1 << bit)).on_hover_text(if bit == 4 { "Camera, appearance and audio" } else { name }).clicked() {
                    sim.tab = 1 << bit;
                }
                if i % 4 == 3 { ui.end_row(); }
            }
        });
        ui.separator();
        ui.add_space(4.0);
        egui::ScrollArea::vertical().id_salt(("section", sim.tab)).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible).auto_shrink([false, false]).show(ui, |ui| {
        if sim.tab & (1 << 0) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("Reactions"));
            info(ui, "Start a reaction, then poke, shoot or pull Homer to see how he responds.");
            let per = per_row(ui, 108.0);
            egui::Grid::new("beh").spacing([10.0, 8.0]).show(ui, |ui| {
                for (i, b) in BEHAVIOURS.iter().filter(|b| !b.starts_with("STOP")).enumerate() {
                    let label = match *b {
                        "balance" => "Balance", "drunk" => "Drunk", "windmill" => "Windmill arms", "pedal" => "Pedal legs",
                        "writhe" => "Writhe", "rollup" => "Roll up", "catchfall" => "Catch a fall", "highfall" => "High fall",
                        "stairs" => "Tumble on stairs", "shot" => "Shot reaction", "flinch" => "Flinch", "brace" => "Brace for impact",
                        "look" => "Look around", "point" => "Point", "lean" => "Lean", "relax" => "Relax",
                        "stayup" => "Stay upright", "foetal" => "Foetal curl", "grab" => "Grab", "overwall" => "Climb over wall", _ => b,
                    };
                    if ui.add_sized([108.0, 44.0], egui::Button::new(label).wrap()).on_hover_text(TIPS.iter().find(|t| t.0 == *b).map_or(*b, |t| t.1)).clicked() {
                        pressed = Some(b.to_string());
                    }
                    if i % per == per - 1 {
                        ui.end_row();
                    }
                }
            });
            ui.collapsing("Additional reactions", |ui| {
                info(ui, "Delirium's experimental reactions.");
                let per = per_row(ui, 145.0);
                egui::Grid::new("additional reactions").spacing([10.0, 8.0]).show(ui, |ui| {
                    for (i, m) in delirium::extras::messages().enumerate() {
                        let mut label = String::new();
                        for (k, c) in m.name.chars().enumerate() {
                            if k == 0 { label.extend(c.to_uppercase()); }
                            else {
                                if c.is_uppercase() { label.push(' '); }
                                label.push(c.to_ascii_lowercase());
                            }
                        }
                        if ui.add_sized([145.0, 52.0], egui::Button::new(label).wrap()).on_hover_text(m.name).clicked() { pressed = Some(m.name.into()); }
                        if i % per == per - 1 { ui.end_row(); }
                    }
                });
            });
        }

        let tiles = |ui: &mut egui::Ui, id: &str, items: &[(&str, bool, String)]| -> Option<String> {
            let (teal, lit) = (egui::Color32::WHITE, ON);
            let mut hit = None;
            let per = per_row(ui, 170.0);
            egui::Grid::new(id).spacing([10.0, 8.0]).show(ui, |ui| {
                for (i, (b, on, label)) in items.iter().enumerate() {
                    let button = wide(ui, label.clone(), if *on { lit } else { teal });
                    let button = match TIPS.iter().find(|t| t.0 == *b) {
                        Some(t) => button.on_hover_text(t.1),
                        None => button,
                    };
                    if button.clicked() {
                        hit = Some(b.to_string());
                    }
                    if i % per == per - 1 {
                        ui.end_row();
                    }
                }
            });
            hit
        };
        let onoff = |b: &str, on: bool| format!("{b}: {}", if on { "On" } else { "Off" });
        let sub = |ui: &mut egui::Ui, t: &str| {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(t).strong().size(15.0));
        };

        if sim.tab & (1 << 1) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("Interact"));
            info(ui, "Choose an interaction. Blue marks the current selection.");
            sub(ui, "Aim from");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (on, name, key) in [(true, "Floating weapon", "#aim gun"), (false, "Camera", "#aim camera")] {
                    if wide(ui, name.into(), if sim.aim_gun == on { ON } else { egui::Color32::WHITE }).clicked() {
                        pressed = Some(key.into());
                    }
                }
            });
            sub(ui, "Left click");
            let shoots = sim.click_shoots || sim.aim_gun;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                for (on, name, key) in [(false, "Poke", "#click poke"), (true, "Shoot", "#click shoot")] {
                    if wide(ui, name.into(), if shoots == on { ON } else { egui::Color32::WHITE }).clicked() {
                        pressed = Some(key.into());
                    }
                }
            });
            if !shoots {
                slide(ui, "Poke strength", &mut sim.poke, 1.0, 100.0, " N·s")
                    .on_hover_text("How hard a left click (poke) pushes the body part (N s).");
            }
            info(ui, if sim.aim_gun { "Floating: the weapon floats by the pointer; click to fire or swing." } else { "Camera: you shoot from the camera at what you click." });
            ui.add_space(4.0);
            #[cfg(feature = "portal")]
            let (portal, picked) = (sim.portal.on(), sim.portal.gun);
            #[cfg(not(feature = "portal"))]
            let (portal, picked) = (false, false);
            let per = per_row(ui, 108.0);
            egui::Grid::new("guns").spacing([10.0, 8.0]).show(ui, |ui| {
                for (i, w) in WEAPONS.iter().enumerate() {
                    let on = sim.weapon == i && !picked;
                    let b = ui.add_sized([108.0, 40.0], egui::Button::new(match w.0 { "deagle" => "Desert Eagle", "smg" => "SMG", "ak47" => "AK-47", "rpg" => "RPG", "pistol" => "Pistol", "sniper" => "Sniper", "shotgun" => "Shotgun", "grenade" => "Grenade", "molotov" => "Molotov", "bat" => "Bat", "knife" => "Knife", _ => w.0 }).selected(on));
                    if b.clicked() { pressed = Some(format!("#weapon {i}")); }
                    if i % per == per - 1 {
                        ui.end_row();
                    }
                }
                if portal && ui.add_sized([108.0, 40.0], egui::Button::new("Portal gun").selected(picked)).clicked() {
                    pressed = Some("#portalgun".into());
                }
            });
            if picked {
                info(ui, if sim.hero.is_some() { "Hero: click the view to aim with the mouse over his shoulder (Esc frees it). Left click blue, right click orange." }
                         else if shoots { "Left click: blue portal, right click: orange.  Hold Ctrl: at the pointer." }
                         else { "The portal gun fires with Left click on Shoot." });
            }
            info(ui, if sim.aim_gun { "Click: fire.  Hold Ctrl: the gun stays put and aims at the pointer.  Shift + right click: a fatal shot." }
                    else if sim.click_shoots { "Click a body part to shoot it.  Shift + right click: a fatal shot." }
                    else { "Shift + click a body part to shoot it.  Shift + right click: a fatal shot." });
            ui.collapsing("Throw objects", |ui| {
            info(ui, "At Homer; hold Ctrl and point to aim it.");
            pressed = pressed.take().or(tiles(ui, "throw", &[("throw ball", false, "ball".into()), ("throw box", false, "box".into()), ("drop crate", false, "crate".into()), ("clear", false, "Clear objects".into())]));
            slide(ui, "Object weight", &mut sim.room.weight, 0.25, 4.0, "×");
            });
            ui.collapsing("Impact tests", |ui| {
            pressed = pressed.take().or(tiles(ui, "impact tests", &[("push front", false, "Push from front".into()), ("push back", false, "Push from behind".into()),
                ("car", false, "Car impact".into()), ("fall", false, "Drop Homer".into()),
                ("explosion pile", false, "Explosion pile".into()), ("wrecking ball", false, "Wrecking ball".into())]));
            });
            #[cfg(feature = "portal")]
            {
                ui.separator();
                sub(ui, "Portal 2 · bonus");
                let on = sim.portal.on();
                pressed = pressed.take().or(tiles(ui, "portal2", &[("#portal2", on, onoff("Portal 2", on))]));
                portal::settings(ui, &mut sim, &mut pressed);
            }

        }

        if sim.tab & (1 << 2) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("Scene"));
            info(ui, "Choose a location, then add people or vehicles.");
            sub(ui, "Location");
            let stages: Vec<(&str, bool, String)> = STAGES.iter().chain(EXTRAS.iter()).map(|&(k, l)| (k, sim.room.stage == k, match l { "room" => "Room", "big stairs" => "Big stairs", "rooftop" => "Rooftop", "low wall" => "Low wall", "rail" => "Railing", "street" => "Street", "rope bridge" => "Rope bridge", "trampoline" => "Trampoline", "cab door" => "Taxi door test", _ => l }.to_string())).collect();
            if let Some(k) = tiles(ui, "stage", &stages) {
                pressed = Some(format!("#stage {k}"));
            }
            sub(ui, "Surface");
            pressed = pressed.take().or(tiles(ui, "surface", &[("ice", sim.ice.is_some(), onoff("Ice floor", sim.ice.is_some()))]));
            sub(ui, "People & vehicles");
            pressed = pressed.take().or(tiles(ui, "add", &[("crowd", sim.crowd.len() == 4, "Add 4 people".into()), ("grid", sim.crowd.len() == 24, "Add 24 people".into())]));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                if ui.add_enabled(sim.taxis.len() < TAXIS, egui::Button::new("Add one taxi").min_size(egui::vec2(170.0, 42.0))).on_hover_text("Spawn one drivable taxi. Play as Hero and press F beside it to get in.").clicked() {
                    pressed = Some("add taxi".into());
                }
                if wide(ui, "Remove taxis".into(), red).on_hover_text("Takes the parked taxis away.").clicked() {
                    pressed = Some("clear taxis".into());
                }
            });
            info(ui, &format!("Taxis: {} / {TAXIS} · One vehicle per click.", sim.taxis.len()));

        }

        if sim.tab & (1 << 3) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("Character"));
            info(ui, "Control Homer, change his reactions, or play as the Hero.");
            sub(ui, "Character control");
            pressed = pressed.take().or(tiles(ui, "play", &[("Hero", sim.hero.is_some(), onoff("Play as Hero", sim.hero.is_some())),
                ("walk", sim.walk, onoff("Walk Homer", sim.walk)), ("passive", sim.passive, onoff("Passive Homer", sim.passive)),
                ("homer weapon", sim.homer_weapon.is_some(), format!("Homer weapon: {}", sim.homer_weapon.map_or("fists", |w| WEAPONS[w].0))),
                ("hero physics", sim.hero_physics, onoff("Hero physics", sim.hero_physics)), ("super punch", sim.super_punch, onoff("Super punch", sim.super_punch))]));
            sub(ui, "Automatic reactions");
            pressed = pressed.take().or(tiles(ui, "homer", &[("look", sim.look, onoff("Look at objects", sim.look)), ("avoid", sim.avoid, onoff("Avoid objects", sim.avoid)),
                ("game tasks", sim.tasks, onoff("Animation mode", sim.tasks)), ("get-up", sim.getup_mode != 0, format!("get-up: {}", GETUP_MODES[sim.getup_mode as usize])),
                ("infinite health", sim.god, onoff("Unlimited health", sim.god)), ("weak", sim.weak, onoff("Weaken muscles", sim.weak))]));
            ui.collapsing("Character appearance", |ui| {
            let cols: Vec<(&str, bool, String)> = PED_COLORS.iter().enumerate().map(|(i, (k, l))| (*k, sim.ped_color == i, l.to_string())).collect();
            if let Some(n) = tiles(ui, "pedcol", &cols) {
                pressed = Some(format!("#pedcol {n}"));
            }
            pressed = pressed.take().or(tiles(ui, "face", &[("#eyes", sim.eyes, onoff("Eyes", sim.eyes)), ("#brows", sim.brows, onoff("Eyebrows", sim.brows)),
                ("#skel", sim.skel, onoff("Skeleton", sim.skel))]));
            });
            if sim.watch.is_some() || [learn::path(), learn::walk_path(false), learn::walk_path(true)].iter().any(|p|
                std::path::Path::new(p).exists() || std::path::Path::new(&learn::live_path(p)).exists()) {
            sub(ui, "Learned movement · local training");
            pressed = pressed.take().or(tiles(ui, "learned_move", &[
                ("walk learned", sim.walk_learned.as_ref().is_some_and(|w| !w.4), "Learned WASD walk".into()),
                ("run learned", sim.walk_learned.as_ref().is_some_and(|w| w.4), "Learned WASD run".into())]));
            let watches: Vec<_> = [("watch training", "Watch get-up", learn::path()),
                ("watch walk training", "Watch walk", learn::walk_path(false)), ("watch run training", "Watch run", learn::walk_path(true))]
                .into_iter().map(|(key, label, path)| (key, sim.watch.as_ref().is_some_and(|w| w.path == path), label.into())).collect();
            pressed = pressed.take().or(tiles(ui, "train", &watches));
            info(ui, &match sim.watch.as_ref() {
                Some(w) if w.stamp.is_some() => format!("{} · generation {}: best try {:.2}. The 8 tries of the latest generation, side by side.", std::path::Path::new(&w.path).file_stem().unwrap_or_default().to_string_lossy(), w.generation,
                                                        w.scores.iter().copied().fold(f32::MIN, f32::max)),
                Some(_) => "Waiting for the trainer...".into(),
                None => "Watch the trainer's latest generation live.".into(),
            });
            }
        }

        if sim.tab & (1 << 4) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("View & audio"));
            info(ui, "Adjust the camera, lighting and sound.");
            pressed = pressed.take().or(tiles(ui, "view", &[("camera", sim.cam_locked, onoff("Follow camera", sim.cam_locked)),
                ("#pops", sim.show_pops, onoff("Damage numbers", sim.show_pops)),
                ("#sound", sim.sound, onoff("Sound", sim.sound)), ("#blood", sim.pixel_blood, onoff("Pixel blood", sim.pixel_blood))]));
            if ui.add_enabled(sim.sound, egui::Button::new("Test sound")).on_hover_text("Play an impact sound through the current Windows output device.").clicked() { sim.sounds.push(sfx::Snd::Hit); }
            info(ui, "8-bit effects for pokes, shots, hits and crashes. No background music. If Test sound is silent, check Delirium in the Windows volume mixer.");
            slide(ui, "Mouse sensitivity", &mut sim.sens, 0.05, 2.0, "");
            if ui.button("Enter photo mode  ·  P").on_hover_text("Hide the interface and move the camera freely; P returns to the room.").clicked() {
                sim.photo_pending = true;
            }
            sub(ui, "Room appearance");
            let looks: Vec<(&str, bool, String)> = LOOKS.iter().map(|(n, l)| (*n, sim.skin == *l, n.to_string())).collect();
            if let Some(n) = tiles(ui, "looks", &looks) {
                pressed = Some(format!("#look {n}"));
            }
            pressed = pressed.take().or(tiles(ui, "look on", &[("#grid", sim.skin.grid, onoff("Grid lines", sim.skin.grid)), ("#fog", sim.skin.fog, onoff("Fog", sim.skin.fog)), ("#infinite", sim.infinite, onoff("Extend floor", sim.infinite)),
                ("#ripples", sim.show_ripples, onoff("Floor ripples", sim.show_ripples))]));
            sub(ui, "Custom colours & lighting");
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let skin = &mut sim.skin;
                for (name, c) in [("SKY", &mut skin.sky), ("FLOOR", &mut skin.floor)] {
                    ui.label(egui::RichText::new(name).monospace().size(11.0).color(DIM));
                    let mut k = *c;
                    if ui.color_edit_button_rgb(&mut k).changed() {
                        *c = k;
                    }
                    ui.add_space(14.0);
                }
            });
            slide(ui, "Light intensity", &mut sim.skin.sun, 0.1, 4.0, "×");
        }

        if sim.tab & (1 << 5) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("Help & shortcuts"));
            info(ui, "Choose a scene, start a reaction, then interact with Homer. Pause or reset at any time using the toolbar.");
            if sim.help && ui.button("Got it — start exploring").clicked() {
                sim.help = false;
                if let Some(p) = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(HELP_SEEN))) {
                    let _ = std::fs::write(p, "");
                }
                sim.tab = 1;
            }
            sub(ui, "Mouse & keyboard");
            egui::Grid::new("keys").max_col_width(170.0).spacing([12.0, 8.0]).striped(true).show(ui, |ui| {
                for (k, what) in [("click", if sim.aim_gun { "fire or swing the floating weapon" } else if sim.click_shoots { "shoot a body part" } else { "poke a body part" }), ("drag", "pull a body part (camera aim)"), ("Shift + click", "shoot a body part (camera aim)"), ("Shift + right click", "fatal shot"),
                                  ("right drag", "orbit the camera"), ("wheel", "zoom"), ("WASD", "walk Homer when Walk Homer is on; otherwise move only in free camera"), ("middle drag", "pan and enter free camera"), ("Q E", "move vertically in free camera"), ("C", "follow / free camera"),
                                  ("Space", "pause"), (".", "one step"), ("[  ]", "slower / faster (1/16 to 4x)"), ("1  2  3", "throw a ball / a box / drop a crate at the pointer"), ("R", "reset pose"), ("X", "stop all"),
                                  ("Ctrl + point", "aim throws there (and the floating gun)"), ("F", "get in / out of a taxi"), ("Enter", "type a command"), ("H", "hide panels"),
                                  ("P", "photo mode"), ("Hero (Character)", "WASD walk, Shift run, Ctrl sprint, click punch")] {
                    ui.label(egui::RichText::new(k).strong());
                    ui.label(what);
                    ui.end_row();
                }
            });
        }

        if sim.tab & (1 << 6) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("Inspect"));
            info(ui, "Technical information for comparing and diagnosing the simulation.");
            if ui.button("Open joint inspector").clicked() { *joints_open = true; }
            ui.separator();
            ui.label(mono(checks).color(DIM));
            ui.label(mono(ours));
            ui.label(mono(format!("world: {} triangles, {} objects{}", sim.world.len(), sim.room.objs.len(),
                                  if sim.look && !sim.room.objs.is_empty() { format!(", looking at {}", sim.room.objs.len() - 1) } else { String::new() })).color(DIM));
        }

        });
    });
    sim.over_ui = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
    if let Some((c, s)) = sim.hover.clone() {
        egui::Area::new("hover".into()).fixed_pos([c.x + 14.0, c.y + 4.0]).interactable(false).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(mono(s)));
        });
    }

    if let Some(p) = pressed {
        let mut w = p.split_whitespace();
        match (w.next(), w.next()) {
            (Some("#sel"), Some(j)) => sim.sel = j.parse().unwrap_or(0),
            (Some("#weapon"), Some(i)) => {
                let w = i.parse().unwrap_or(0);
                #[cfg(feature = "portal")]
                {
                    sim.portal.gun = false;
                }
                if sim.hero.is_some() {
                    sim.hero_take(Some(w));
                } else {
                    sim.weapon = w;
                }
            }
            (Some("#weak"), _) => sim.weak = !sim.weak,
            #[cfg(feature = "portal")]
            (Some("#portalgun"), _) => portal::pick_gun(&mut sim),
            #[cfg(feature = "portal")]
            (Some("#portal2"), action) => portal::toggle(&mut sim, action),
            (Some("#look"), Some(n)) => sim.skin = LOOKS.iter().find(|l| l.0 == n).map_or(sim.skin, |l| l.1),
            (Some("#grid"), _) => sim.skin.grid = !sim.skin.grid,
            (Some("#infinite"), _) => sim.infinite = !sim.infinite,
            (Some("#pedcol"), Some(n)) => sim.ped_color = PED_COLORS.iter().position(|c| c.0 == n).unwrap_or(sim.ped_color),
            (Some("#eyes"), _) => sim.eyes = !sim.eyes,
            (Some("#skel"), _) => sim.skel = !sim.skel,
            (Some("#brows"), _) => sim.brows = !sim.brows,
            (Some("#fog"), _) => sim.skin.fog = !sim.skin.fog,
            (Some("#glow"), _) => sim.skin.glow = !sim.skin.glow,
            (Some("#stage"), Some(n)) => sim.switch_to = Some((n.to_string(), nm() == delirium::nmrs::MARGE)),
            (Some("#click"), Some(m)) => {
                sim.click_shoots = m == "shoot";
                if !sim.click_shoots { sim.aim_gun = false; }
            }
            (Some("#aim"), Some(m)) => {
                sim.aim_gun = m == "gun";
                sim.gun.placed = false;
            }
            (Some("#hero"), _) => sim.toggle_hero(),
            (Some("Hero"), _) => sim.toggle_hero(),
            (Some("weak"), _) => sim.weak = !sim.weak,
            (Some("#pops"), _) => sim.show_pops = !sim.show_pops,
            (Some("#help"), _) => { sim.help = true; sim.tab = 1 << 5; },
            (Some("#sound"), _) => sim.sound = !sim.sound,
            (Some("#realtime"), _) => sim.speed = 1.0,

            (Some("#ripples"), _) => {
                sim.show_ripples = !sim.show_ripples;
                sim.ripples.clear();
            }
            (Some("#blood"), _) => {
                sim.pixel_blood = !sim.pixel_blood;
                sim.blood.clear();
            }
            (Some("#slower"), _) => sim.speed = (sim.speed * 0.5).max(SPEEDS.0),
            (Some("#faster"), _) => sim.speed = (sim.speed * 2.0).min(SPEEDS.1),
            _ => sim.button(&p),
        }
    }
    Ok(())
}
