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
mod gamenums;
use gamenums::*;
mod fire;
mod flood;
#[allow(dead_code)]
mod fluid;
#[allow(dead_code)]
mod fluid_gpu;
mod foam;
mod getup;
mod learn;
mod locomotion;
mod motion_tools;
mod map;
#[allow(dead_code)]
mod melee;
mod hero;
mod sfx;
mod jump;
mod sever;
mod skate;
mod skate_files;
mod skate_hud_files;
mod hom;
mod skate_rails;
mod tackle;
mod water;
mod wet;
mod world;
#[cfg(feature = "portal")]
mod portal;
#[cfg(feature = "portal")]
mod p2vtf;
#[cfg(windows)]
mod upscale;

const WHO: [&str; 1] = ["ours"];
const SIDES: usize = WHO.len();

const CHEST: usize = 9;
const WHIZZ: f32 = 1.5;
const SNAP: f32 = 0.45;
const HEAD: usize = 12;
const HAND_R: usize = 20;
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
const BLOOD_POOL_LIFE: f32 = 40.0;
const BLOOD_POOL: u8 = 3;
const BLOOD_DROP: u8 = 4;
const WOUNDS: usize = 6;
const WOUND_LIFE: f32 = 8.0;
const WOUND_BEAT: f32 = 0.45;
const BLOOD_SIZE: f32 = 0.035;
const SPLATS: usize = 8;
const SPLAT_PX: usize = 12;
const BLOOD_SPLAT: f32 = 0.3;
const POP_LIFE: f32 = 1.2;
const MEAT_HIT: f32 = 2.0;
const HOM_PART: [usize; 25] = [12, 11, 16, 15, 14, 13, 20, 19, 18, 17, 10, 9, 8, 7, 3, 3, 2, 1, 6, 6, 5, 4, 0, 12, 0];
const MEAT_HOLD: f32 = 4.0;
const MEAT_MAX: f32 = 12.0;
const MEAT_XRAY: f32 = 40.0 / 60.0;
const MEAT_SHELL: Color = Color::srgba(0.05, 0.12, 0.25, 0.35);
const MEAT_SHAKE: f32 = 0.006;
const MEAT_BONES: [&str; 21] = ["Pelvis", "Left Femur", "Left Tibia", "Left Ankle", "Right Femur", "Right Tibia", "Right Ankle",
    "Coccyx", "Lower Spine", "Ribs", "Upper Spine", "Neck", "Skull", "Left Collarbone", "Left Humerus", "Left Forearm",
    "Left Wrist", "Right Collarbone", "Right Humerus", "Right Forearm", "Right Wrist"];

#[derive(Default)]
struct Meat {
    score: f32,
    broke: Vec<(usize, f32)>,
    last: Vec<(Vec3, Vec3)>,
    still: f32,
    age: f32,
    done: Option<f32>,
    speed: f32,
    top: f32,
    low: f32,
    first: Option<f32>,
    turn: f32,
    engine: bool,
    rot: Quat,
    air: f32,
    flip: f32,
    spin: f32,
    outro: bool,
    xray: f32,
    shake: f32,
    sum: [f32; 25],
    peak: [f32; 25],
    shown: bool,
}

struct HomBone {
    links: Vec<(usize, f32)>,
    injury: [(f32, f32); 6],
}

fn hom_bones() -> Option<&'static [HomBone]> {
    static T: std::sync::OnceLock<Option<Vec<HomBone>>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let c = skate_data::collections::Collections::load(&skate_files::dir()).ok()?;
        let hex = &c.field("scoring_wipeouts", "default", "Hash_1F9B5EA147CBEEE9").ok()?.data;
        let b: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect::<Option<_>>()?;
        if b.len() < 25 * 84 {
            return None;
        }
        let w = |i: usize| u32::from_be_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
        Some((0..25).map(|r| HomBone {
            links: (0..4).map(|l| (w(r * 21 + 1 + l * 2) as i32, f32::from_bits(w(r * 21 + 2 + l * 2))))
                .filter(|l| (0..25).contains(&l.0)).map(|l| (l.0 as usize, l.1)).collect(),
            injury: std::array::from_fn(|k| (w(r * 21 + 9 + k * 2) as i32 as f32, f32::from_bits(w(r * 21 + 10 + k * 2)))),
        }).collect())
    }).as_deref()
}

struct HomGraphs {
    air: PointGraph<8>,
    speed: PointGraph<8>,
    drop: PointGraph<8>,
    tumble: PointGraph<8>,
    flips: PointGraph<8>,
    spins: PointGraph<8>,
}
use skate_core::point_graph::PointGraph;

fn hom_graphs() -> Option<&'static HomGraphs> {
    static T: std::sync::OnceLock<Option<HomGraphs>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let c = skate_data::collections::Collections::load(&skate_files::dir()).ok()?;
        let g = |name: &str| c.words::<20>("scoring_wipeouts", "default", name).ok()
            .map(|w| PointGraph { x: std::array::from_fn(|i| f32::from_bits(w[4 + i])), y: std::array::from_fn(|i| f32::from_bits(w[12 + i])) });
        Some(HomGraphs { air: g("Hash_2451424AD330B08F")?, speed: g("Hash_B2A997F8579B1998")?, drop: g("Hash_A9B81D5BF44762E4")?,
                         tumble: g("Hash_6CD9F0CD7675949C")?, flips: g("Hash_1D49983B40C59E5A")?, spins: g("Hash_36274FBAE087E832")? })
    }).as_ref()
}

impl Meat {
    fn hurt(&mut self, t: &[HomBone], r: usize, d: f32, from: Option<usize>) {
        self.sum[r] += d;
        self.peak[r] = self.peak[r].max(self.sum[r]);
        for &(n, f) in t[r].links.iter().filter(|l| Some(l.0) != from) {
            if d * f > 0.1 {
                self.hurt(t, n, d * f, Some(r));
            }
        }
    }

    fn level(&self, t: &[HomBone], r: usize) -> i8 {
        (0..6).rev().find(|&k| t[r].injury[k].1 > 0.0 && self.peak[r] > t[r].injury[k].1).map_or(-1, |k| k as i8)
    }

    fn data(&self) -> skate_hud::hom::Data {
        let metrics = [self.speed * 3.6, (self.top - self.low).max(0.0), self.air, self.done.unwrap_or(self.age), self.turn.to_degrees()];
        let injury: f32 = hom_bones().map_or(0.0, |t| (0..t.len()).filter_map(|r| usize::try_from(self.level(t, r)).ok().map(|l| t[r].injury[l].0)).sum());
        let Some(g) = hom_graphs() else {
            return skate_hud::hom::Data { score: injury, metrics, points: [0.0; 5] };
        };
        let turns = |rad: f32| (rad.to_degrees() + 90.0) / 360.0;
        let points = [g.speed.evaluate(self.speed), g.drop.evaluate(metrics[1]), g.air.evaluate(self.air), 0.0, g.tumble.evaluate(metrics[4])].map(f32::trunc);
        let extra = g.flips.evaluate(turns(self.flip)).trunc() + g.spins.evaluate(turns(self.spin)).trunc();
        skate_hud::hom::Data { score: injury + points.iter().sum::<f32>() + extra, metrics, points }
    }
}
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
    if n.len() != 11 && n.len() != 13 {
        return LOOKS[0].1;
    }
    Look { sky: [n[0], n[1], n[2]], floor: [n[3], n[4], n[5]], grid: n[6] != 0.0, fog: n[7] != 0.0, sun: n[8], glow: n[10] != 0.0 }
}

const SUN_DIR: [f32; 2] = [-33.7, 59.0];
const SUN_SIZE_MAX: f32 = 10.0;
const REAL_SUN: f32 = 0.53;

fn saved_sun() -> [f32; 2] {
    let n: Vec<f32> = look_path().and_then(|p| std::fs::read_to_string(p).ok()).map_or(vec![], |t| t.split_whitespace().filter_map(|x| x.parse().ok()).collect());
    if n.len() == 13 { [n[11], n[12]] } else { SUN_DIR }
}

const SETTINGS: &str = "settings.txt";
const STARTING: &str = "starting";
const STARTED: u32 = 60;
const FALLBACK: &str = "graphics-fallback";

const SAFE: &str = "water_quality 0
shadows 0
shadow_quality 1
soft 0
sun_size 0
shadow_filter 0
render_scale 0.5
aa 0
smaa 0
upscaler 1
dlss_mode 0
fsr_mode 0
xess_mode 0
sharpen 0
ssao 0
bloom 0
dof 0
contact 0
vignette 0
lens 0
chroma 0
ray_tracing 0
sky 0
god_rays 0
restir 0
fire 0
real_fire 0
smoke 0
fire_light 0
voxel_water 0
photon_caustics 0
caustics 0
spray 0
waves 0
pixels 0
haze 0
mirror 0
foam 0
mist 0
wet 0
wet_film 0
wet_drips 0
sky_shade 0
traced_water 0
glass_tint 0
glass_shadows 0
water_ripples 0
water_refraction 0
water_surface 0
fine_ripples 0
clear_spill 0
wet_trail 0
impact_foam 0
finer_caustics 0
depth_colour 0
keep_volume 0
ghost_surface 0
vorticity 0
spray_physics 0
";

fn settings(sim: &Sim) -> String {
    let b = |x: bool| x as u8;
    format!("pedcol {}
eyes {}
brows {}
skel {}
skel_style {}
clean {}
look {}
avoid {}
tasks {}
getup {}
god {}
weak {}
passive {}
walk {}
hero_physics {}
super_punch {}
even_fight {}
dive_shot {}
shadows {}
opacity {}
soft {}
sun_size {}
fill {}
sun_r {}
sun_g {}
sun_b {}
sound {}
vol {}
vol_fx {}
vol_portal {}
dodge_gap {}
extra_reactions {}
extra_pick {}
hero_opacity {}
render_scale {}
msaa {}
shadow_quality {}
gpu {}
aa {}
smaa {}
dlss_mode {}
sharpen {}
fps_limit {}
upscaler {}
fsr_mode {}
xess_mode {}
ssao {}
ao_strength {}
bloom {}
dof {}
sharp_by {}
contact {}
contact_strength {}
vignette {}
lens {}
chroma {}
ray_tracing {}
sky {}
god_rays {}
restir {}
fire {}
real_fire {}
smoke {}
fire_reaction {}
water_quality {}
voxel_water {}
photon_caustics {}
caustics {}
spray {}
waves {}
pixels {}
haze {}
mirror {}
foam {}
wet {}
wet_film {}
wet_drips {}
sky_shade {}
traced_water {}
glass_tint {}
glass_shadows {}
water_ripples {}
water_refraction {}
water_surface {}
fine_ripples {}
clear_spill {}
wet_trail {}
impact_foam {}
faucet {}
fire_light {}
fire_homer {}
shadow_filter {}
mist {}
finer_caustics {}
depth_colour {}
keep_volume {}
ghost_surface {}
vorticity {}
spray_physics {}
skate3_bails {}
",
            sim.ped_color, b(sim.eyes), b(sim.brows), b(sim.skel), sim.skel_style, b(sim.clean), b(sim.look), b(sim.avoid), b(sim.tasks), sim.getup_mode, b(sim.god), b(sim.weak),
            b(sim.passive), b(sim.walk), b(sim.hero_physics), b(sim.super_punch), b(sim.even), b(sim.dive), b(sim.shadows), sim.body_alpha,
            b(sim.soft), sim.shadow, sim.fill_k, sim.sun_color[0], sim.sun_color[1], sim.sun_color[2], b(sim.sound),
            sim.volume[0], sim.volume[1], sim.volume[2], sim.dodge_gap, b(sim.extra), sim.extra_pick, sim.hero_alpha, sim.render_scale, sim.msaa, sim.shadow_q, sim.gpu, sim.aa, sim.smaa_q, sim.dlss_q, sim.sharpen, sim.fps_limit, sim.upscaler, sim.fsr_q, sim.xess_q,
            sim.ssao, sim.ao_strength, sim.bloom, sim.dof, sim.sharp_by.map(|x| x.to_string()).join(","), sim.contact, sim.contact_k, sim.vignette, sim.lens, sim.chroma, sim.rt, sim.sky, b(sim.god_rays), b(sim.restir), sim.fire, b(sim.real_fire), b(sim.smoke), b(sim.fire_react), sim.water_q, b(sim.voxel_water), b(sim.photons), b(sim.water_caustics), b(sim.water_spray), b(sim.water_waves), b(sim.water_pixels), b(sim.water_haze), b(sim.water_mirror), b(sim.water_foam), b(sim.water_wet), b(sim.wet_film), b(sim.wet_drips), b(sim.sky_shade), b(sim.water_traced), b(sim.glass_tint), b(sim.glass_shade), b(sim.water_ripples), b(sim.water_refract), b(sim.real_surface), b(sim.real_ripples), b(sim.water_clear), b(sim.wet_trail), b(sim.water_impact), b(sim.water_faucet), b(sim.fire_light), b(sim.fire_homer), sim.shadow_filter, b(sim.water_mist), b(sim.water_fine), b(sim.water_depth), b(sim.water_relax), b(sim.water_ghost), b(sim.water_vort), b(sim.water_spray_phys), b(sim.skate3_bails))
}

const XR: [(&str, &str); 8] = [
    ("Windmill arms", "Sometimes he windmills his arms as he loses his balance."),
    ("Curl up before getting up", "Sometimes he curls up on the ground a while before he gets up."),
    ("Curl up when shot at lying down", "Down, he curls up with his head at the shot."),
    ("Look at who hit him", "He looks at who hit or pushed him for 2 s."),
    ("Tumble off a ledge", "Off a high ledge with way on, head over heels onto a hand and shoulder; dropping, arms out (instead of the game's highFall task)."),
    ("smartFall when dropped", "Dropped from the panel, a smartFall."),
    ("Dodge from above", "He dodges what's coming down at him."),
    ("Brace on a wall", "Staggering into a wall or rail, a hand flat on it or a hold on its top."),
];

fn coin() -> bool {
    use std::hash::BuildHasher;
    std::collections::hash_map::RandomState::new().hash_one(0u8) & 1 == 0
}

fn load_settings(sim: &mut Sim) {
    if sim.safe {
        return;
    }
    let f = getup::config_dir().join(SETTINGS);
    let Ok(t) = std::fs::read_to_string(&f) else { return };
    sim.say(format!("room  settings from {}", f.display()));
    apply_settings(sim, &t);
}

fn apply_settings(sim: &mut Sim, t: &str) {
    for (k, v) in t.lines().filter_map(|l| l.split_once(' ')) {
        if k == "gpu" {
            sim.gpu = v.trim().into();
            continue;
        }
        if k == "sharp_by" {
            for (to, x) in sim.sharp_by.iter_mut().zip(v.trim().split(',')) {
                *to = x.parse::<f32>().unwrap_or(0.0).clamp(0.0, 1.0);
            }
            continue;
        }
        let Ok(x) = v.trim().parse::<f32>() else { continue };
        let (n, on) = (x.max(0.0) as usize, x != 0.0);
        match k {
            "pedcol" => if let Some(c) = ped_colour(v.trim()) { (sim.ped_color, sim.body_alpha) = c },
            "opacity" => sim.body_alpha = x.clamp(0.1, 1.0),
            "hero_opacity" => sim.hero_alpha = x.clamp(0.1, 1.0),
            "render_scale" => sim.render_scale = x.clamp(RENDER_SCALE_MIN, RENDER_SCALE_MAX),
            "msaa" if x < 1.5 => sim.aa = 0,
            "msaa" => sim.msaa = [2, 4, 8].into_iter().find(|&k| k as f32 >= x).unwrap_or(8),
            "aa" => sim.aa = (x as u8).min(AA_XESS),
            "fsr_mode" => sim.fsr_q = (x as u8).min(4),
            "xess_mode" => sim.xess_q = (x as u8).min(4),
            "smaa" => sim.smaa_q = (x as u8).min(3),
            "dlss_mode" => sim.dlss_q = (x as u8).min(3),
            "shadow_quality" => sim.shadow_q = (x as u8).clamp(1, 3),
            "sharpen" => sim.sharpen = x.clamp(0.0, 1.0),
            "ssao" => sim.ssao = (x as u8).min(4),
            "ao_strength" => sim.ao_strength = x.clamp(0.0, 2.0),
            "bloom" => sim.bloom = x.clamp(0.0, 1.0),
            "dof" => sim.dof = x.clamp(0.0, 1.0),
            "contact" => sim.contact = x.clamp(0.0, 1.0),
            "contact_strength" => sim.contact_k = x.clamp(0.0, 1.0),
            "vignette" => sim.vignette = x.clamp(0.0, 1.0),
            "lens" => sim.lens = x.clamp(0.0, 1.0),
            "chroma" => sim.chroma = x.clamp(0.0, 1.0),
            "ray_tracing" => sim.rt = (x as u8).min(2),
            "sky" => sim.sky = (x as u8).min(2),
            "god_rays" => sim.god_rays = on,
            "restir" => sim.restir = on,
            "fire" => sim.set_fire((x as u8).min(FIRE_HIGH)),
            "fire_light" => sim.fire_light = on,
            "finer_caustics" => sim.water_fine = on,
            "depth_colour" => sim.water_depth = on,
            "keep_volume" => sim.water_relax = on,
            "ghost_surface" => sim.water_ghost = on,
            "vorticity" => sim.water_vort = on,
            "spray_physics" => sim.water_spray_phys = on,
            "fire_homer" => sim.fire_homer = on,
            "shadow_filter" => sim.shadow_filter = (x as u8).min(3),
            "real_fire" => sim.real_fire = on,
            "smoke" => sim.smoke = on,
            "fire_reaction" => sim.fire_react = on,
            "water_quality" => sim.set_water((x as u8).min(4)),
            "voxel_water" => sim.voxel_water = on,
            "photon_caustics" => sim.photons = on,
            "caustics" => sim.water_caustics = on,
            "spray" => sim.water_spray = on,
            "waves" => sim.water_waves = on,
            "pixels" => sim.water_pixels = on,
            "haze" => sim.water_haze = on,
            "mirror" => sim.water_mirror = on,
            "foam" => sim.water_foam = on,
            "mist" => sim.water_mist = on,
            "wet" => sim.water_wet = on,
            "wet_film" => sim.wet_film = on,
            "wet_drips" => sim.wet_drips = on,
            "sky_shade" => sim.sky_shade = on,
            "traced_water" => sim.water_traced = on,
            "glass_tint" => sim.glass_tint = on,
            "glass_shadows" => sim.glass_shade = on,
            "water_ripples" => sim.water_ripples = on,
            "water_refraction" => sim.water_refract = on,
            "water_surface" => sim.real_surface = on,
            "fine_ripples" => sim.real_ripples = on,
            "clear_spill" => sim.water_clear = on,
            "wet_trail" => sim.wet_trail = on,
            "impact_foam" => sim.water_impact = on,
            "faucet" => sim.water_faucet = on,
            "upscaler" => sim.upscaler = (x as u8).min(UP_FSR1),
            "fps_limit" => sim.fps_limit = x.round().clamp(FPS_VSYNC, FPS_FREE),
            "eyes" => sim.eyes = on,
            "brows" => sim.brows = on,
            "skel" => sim.skel = on,
            "skel_style" => sim.skel_style = (x as u8).min(2),
            "clean" => sim.clean = on,
            "look" => sim.look = on,
            "avoid" => sim.avoid = on,
            "tasks" => sim.tasks = on,
            "getup" => sim.getup_mode = (n % 4) as u8,
            "god" => sim.god = on,
            "weak" => sim.weak = on,
            "skate3_bails" => sim.skate3_bails = on,
            "passive" => sim.passive = on,
            "walk" => sim.walk = on,
            "hero_physics" => sim.hero_physics = on,
            "super_punch" => sim.super_punch = on,
            "even_fight" => sim.even = on,
            "dive_shot" => sim.dive = on,
            "shadows" => sim.shadows = on,
            "soft" => sim.soft = on,
            "sun_size" => sim.shadow = x.clamp(0.0, SUN_SIZE_MAX),
            "fill" => sim.fill_k = x.clamp(0.0, 3.0),
            "sun_r" => sim.sun_color[0] = x.clamp(0.0, 1.0),
            "sun_g" => sim.sun_color[1] = x.clamp(0.0, 1.0),
            "sun_b" => sim.sun_color[2] = x.clamp(0.0, 1.0),
            "sound" => sim.sound = on,
            "vol" => sim.volume[0] = x.clamp(0.0, 1.0),
            "vol_fx" => sim.volume[1] = x.clamp(0.0, 1.0),
            "vol_portal" => sim.volume[2] = x.clamp(0.0, 1.0),
            "dodge_gap" => sim.dodge_gap = x.clamp(0.0, 10.0),
            "extra_reactions" => sim.extra = on,
            "extra_pick" => sim.extra_pick = x as u32,
            _ => {}
        }
    }
}

fn reset_settings(sim: &mut Sim, keys: &[&str]) {
    let t: String = sim.defaults.lines().filter(|l| keys.iter().any(|k| l.split(' ').next() == Some(k))).map(|l| format!("{l}\n")).collect();
    apply_settings(sim, &t);
}

fn save_settings(sim: NonSend<Sim>, mut last: Local<Option<Option<String>>>) {
    let now = settings(&sim);
    let last = last.get_or_insert_with(|| sim.script.is_empty().then(|| now.clone()));
    if let Some(l) = last && *l != now {
        let _ = std::fs::create_dir_all(getup::config_dir()).and_then(|_| std::fs::write(getup::config_dir().join(SETTINGS), &now));
        *l = now;
    }
}

fn help_seen() -> bool {
    std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(HELP_SEEN))).is_some_and(|p| p.exists())
}
const WEAPONS: [(&str, &str, f32); 13] = [("pistol", "PISTOL", 50.0), ("deagle", "DEAGLE", 50.0), ("smg", "MP5", 50.0),
                                         ("ak47", "AK47", 50.0), ("sniper", "SNIPERRIFLE", 50.0), ("shotgun", "SHOTGUN", 50.0), ("rpg", "RLAUNCHER", 50.0),
                                          ("grenade", "GRENADE", 50.0), ("molotov", "MOLOTOV", 50.0), ("bat", "BASEBALLBAT", 50.0), ("knife", "KNIFE", 50.0),
                                          ("taser", "TASER", 10.0), ("baton", "BASEBALLBAT", 50.0)];

const LEAN_SECS: f32 = 1.5;

#[derive(Clone, Copy)]
struct Exit { min: f32, max: f32, speed: Option<f32>, fail: bool }

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

const KICK: [f32; 13] = [0.7, 1.1, 0.4, 0.6, 2.2, 2.0, 1.6, 0.3, 0.3, 1.2, 0.8, 0.2, 1.2];
const GUN_FALLBACK: [(f32, f32, f32); 13] = [(300.0, 0.5, 0.02), (350.0, 0.5, 0.03), (70.0, 0.5, 0.02), (130.0, 0.5, 0.04), (400.0, 0.0, 0.06), (1200.0, 0.1, 0.06),
                                            (1000.0, 0.0, 0.0), (1000.0, 0.0, 0.0), (1000.0, 0.0, 0.0), (600.0, 0.0, 0.0), (400.0, 0.0, 0.0),
                                            (1500.0, 0.0, 0.0), (600.0, 0.0, 0.0)];
const TAXI_RADIUS: f32 = 0.303;
const TAXI_AXLE: Vec2 = Vec2::new(0.73, 1.326);
const TAXI_HULL: [Vec3; 2] = [Vec3::new(-0.916, -2.608, -0.143), Vec3::new(0.916, 2.466, 1.05)];
const TAXI_BELT: f32 = 0.62;
const TAXI_ID: i32 = 100;
const PLANK_ID: i32 = 200;
const BODIES: [(&str, &str, f32, f32, f32); 6] = [("S", "Small", 1.60, 55.0, 1.0), ("A", "Average", 1.75, 77.0, 1.0), ("L", "Large", 1.90, 110.0, 1.0),
                                                 ("K", "Lanky", 1.90, 64.0, 1.12), ("F", "Fat", 1.72, 125.0, 0.95), ("T", "Stocky", 1.63, 88.0, 0.9)];
const PLAT_ID: i32 = 300;
const INJURY_MAX: f32 = 0.8;
const CRUSH_SPEED: f32 = 2.5;
const DODGE_GAP: f32 = 3.0;
const NEAR_BURST: f32 = 0.5;
const DIVE: (f32, f32) = (3.5, 3.0);
fn hang_let_go(grab: bool, hand: usize) -> String {
    if grab { format!("grab start=true {}=false", ["useLeft", "useRight"][hand]) } else { format!("armHang letGo={hand}") }
}

const HANG_SECS: f32 = 3.0;
const CAB_GAP: f32 = 1.45;
const CAB_OFF: f32 = 0.12;
const CAB_GO: f32 = 1.5;
const CAB_GAS: f32 = 0.7;
const CAB_LET_GO: f32 = 12.0;
const CAB_HOLD: f32 = 6.0;
const CAB_STOP: f32 = 14.0;
const HIGH_DROP: f32 = 12.0;
const TUMBLE: f32 = 7.0;
const TAXIS: usize = 4;

const CLIP_FALLBACK: [u32; 13] = [17, 9, 30, 30, 5, 8, 1, 1, 1, 1, 1, 1, 1];

const BLAST_R: f32 = 5.0;
const PILE_FUSE: f32 = 2.0;
const BLAST_DV: f32 = 15.0;
const BLAST_DAMAGE: f32 = 250.0;
const PELLETS: usize = 16;

const GUN_MODELS: [(&[([f32; 3], [f32; 3], f32)], f32); 13] = [
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
    (&[([0.035, 0.045, 0.15], [0.0, 0.02, -0.01], 0.0), ([0.028, 0.1, 0.045], [0.0, -0.045, 0.045], -0.25)], 0.1),
    (&[([0.035, 0.035, 0.25], [0.0, 0.0, -0.125], 0.0), ([0.04, 0.04, 0.4], [0.0, 0.0, -0.45], 0.0), ([0.008, 0.008, 0.05], [0.012, 0.0, -0.675], 0.0),
       ([0.008, 0.008, 0.05], [-0.012, 0.0, -0.675], 0.0)], 0.7),
];

fn kind(w: usize) -> u8 {
    match WEAPONS[w].0 {
        "grenade" | "molotov" => 1,
        "bat" | "baton" | "knife" => 2,
        _ => 0,
    }
}

fn bat(w: usize) -> bool {
    matches!(WEAPONS[w].0, "bat" | "baton")
}

fn held(w: usize) -> Option<hero::Held> {
    match WEAPONS[w].0 {
        "bat" => Some((0.09, 0.72, 0.4)),
        "baton" => Some((0.09, 0.6, 0.35)),
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
    if bat(w) { BAT_SWING } else { SWING }
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
const SHOCK: f32 = 2.5;
const SHOCK_HZ: f32 = 1.6;
const SHOCK_XRAY: f32 = 0.75;
const SHOCK_SHELL: Color = Color::BLACK;
const BONE_GLOW: LinearRgba = LinearRgba::rgb(0.3, 0.4, 0.6);
const BONE_BIAS: f32 = 1.0e6;
const SKULL: f32 = 0.85;
const AURA_GLOW: Color = Color::srgb(0.12, 0.4, 0.9);

#[derive(Component)]
struct GunMesh(usize, bool);

struct HeroDoll {
    agent: Agent,
    build: Build,
    st: msg::State,
    t: f32,
    rise: Option<getup::Rise>,
}

const HOMERS: usize = 8;

struct Benched {
    homer_id: usize,
    body: usize,
    build: Build,
    frames: Vec<Transform>,
    scale: (Vec<f32>, Vec<f32>),
    dead: bool,
    hurt: [f32; 4],
    wounds: Vec<(usize, Vec3, Vec3, f32)>,
    burning: f32,
    on_fire: Option<delirium::nm::OnFire>,
    charred: Vec<f32>,
    wet: bool,
    voxels: Vec<Option<Voxels>>,
    cuts: Vec<sever::Cut>,
    hat: Option<sever::Hat>,
    shock: f32,
    musc: Option<getup::Rise>,
    jump: Option<jump::Jump>,
    shot_task: Option<(ShotTask, u64)>,
    blast_task: Option<delirium::explosion::Explosion>,
    homer: Option<hero::Hero>,
    hang: Option<(Vec<String>, [bool; 2])>,
    crush: Option<(i32, u64)>,
    fall: Option<(f32, bool)>,
    hold: Option<f32>,
    look_stop: Option<f32>,
    look_after: Option<f32>,
    lean_stop: Option<f32>,
    airborne: bool,
    dodging: bool,
    stairs_rolled: bool,
    was_fallen: bool,
    curled: bool,
    homer_rest: f32,
    learned: Option<(learn::Net, Vec<Quat>, usize, learn::Pick)>,
    rise: Option<getup::Rise>,
    still: f32,
    last: Vec3,
    nm_t: Option<f32>,
    failed: bool,
    up: Option<Vec<Transform>>,
}

impl Benched {
    fn new(homer_id: usize, body: usize, build: Build, frames: Vec<Transform>, scale: (Vec<f32>, Vec<f32>)) -> Benched {
        Benched { homer_id, body, build, frames, scale, dead: false, hurt: [0.0; 4], wounds: vec![], burning: 0.0, on_fire: None, charred: vec![], wet: false,
                  voxels: vec![], cuts: vec![], hat: None, shock: 0.0, musc: None, jump: None, shot_task: None, blast_task: None, homer: None, hang: None,
                  crush: None, fall: None, hold: None, look_stop: None, look_after: None, lean_stop: None, airborne: false, dodging: false,
                  stairs_rolled: false, was_fallen: false, curled: false, homer_rest: 0.0, learned: None, rise: None, still: 0.0, last: Vec3::ZERO, nm_t: None, failed: false, up: None }
    }
}

#[derive(Component)]
struct GunFlash;

#[derive(Component)]
struct BloodPix(usize);

#[derive(Component)]
struct Glow(usize);
const GLOW_MAX: usize = 20;
pub(crate) const SKY_POOL: Vec3 = Vec3::new(0.0, -13.5, 12.0);
pub(crate) const SKY_HALF: Vec2 = Vec2::new(10.0, 3.5);
pub(crate) const SKY_DEEP: f32 = 1.5;
const FIRE_LIGHT_OUT: f32 = 0.45;
const FIRE_HIGH: u8 = 3;
const CHAR_TIME: f32 = 12.0;
const CHAR_PX: usize = 16;
const SPILL_RATE: f32 = 3.0;
const SPILL_LIFE: f32 = 3.0;
const CHAR_MAX: f32 = 3.0;
const VOX: f32 = 0.015;
const VOX_HOLE: f32 = 0.03;
const BONE_COLOR: Color = Color::srgb(0.9, 0.87, 0.78);
const EMBER_GLOW: LinearRgba = LinearRgba::rgb(3.0, 0.5, 0.05);
const GLOW_SHRINK: f32 = 0.2;
const FIRE_GLOW: LinearRgba = LinearRgba::rgb(1.4, 0.32, 0.04);

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
    shadows: bool,
    sun_color: [f32; 3],
    fill_k: f32,
    gun_reach: f32,
    gun_up: f32,
    gun_off: Vec3,
    gun_aim_off: Vec3,
    crane: Option<([f32; 4], [f32; 4], f32, f32, Option<(Vec3, Vec3)>)>,
    crane_to: Option<([f32; 4], f32, Option<Vec3>)>,
    shadow: f32,
    strict: bool,
    stairs_rolled: bool,
    cam_head: bool,
    cam_rig: Option<[f32; 3]>,
    spawn_at: Option<Vec2>,
    tear_k: f32,
    sky_k: f32,
    kept_gun: Option<(usize, Option<Transform>)>,
    cam_floor: f32,
    pov: bool,
    film_vel: Vec3,
    photo_pending: bool,
    photo_follow: bool,
    detached: bool,
    cam_at: Option<Vec3>,
    cam2: Option<(Vec3, Vec3)>,
    cam2_queue: Vec<String>,
    cam2_b: Option<String>,
    meat: Option<Meat>,
    meat_on: bool,
    meat_wipe: bool,
    skate3_bails: bool,
    meat_screen: Option<Option<hom::Screen>>,
    shot_scene: String,
    photo_note: String,
    look_z: f32,
    script_walk: Vec3,
    click_at: Option<Vec3>,
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
    steam: (u32, Vec<Vec3>),
    shock: f32,
    charred: Vec<f32>,
    on_fire: Option<delirium::nm::OnFire>,
    god: bool,
    lean_stop: Option<f32>,
    look_stop: Option<f32>,
    dodge_at: f32,
    dodging: bool,
    look_after: Option<f32>,
    fall: Option<(f32, bool)>,
    hold: Option<f32>,
    airborne: bool,
    wet: bool,
    hurt: [f32; 4],
    near_at: f32,
    near_last: f32,
    dived: bool,
    no_dodge: bool,
    dodge_gap: f32,
    extra: bool,
    extra_pick: u32,
    fire_react: bool,
    dive: bool,
    hang: Option<(Vec<String>, [bool; 2])>,
    was_fallen: bool,
    curled: bool,
    super_punch: bool,
    even: bool,
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
    stress: bool,
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
    tab: u16,
    skin: Look,
    sounds: Vec<sfx::Snd>,
    sound: bool,
    volume: [f32; 3],
    blood: Vec<(Vec3, Vec3, f32, u8, Vec3)>,
    pixel_blood: bool,
    gore: bool,
    layers: bool,
    voxels: Vec<Option<Voxels>>,
    wounds: Vec<(usize, Vec3, Vec3, f32)>,
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
    skate: Option<skate::Skate>,
    skate_link: skate_files::Link,
    passive: bool,
    homer: Option<hero::Hero>,
    homer_weapon: Option<usize>,
    homer_guard: Option<bool>,
    homer_shot: f32,
    hero_physics: bool,
    hero_doll: Option<HeroDoll>,
    tackle: Option<tackle::Tackle>,
    cuts: Vec<sever::Cut>,
    hat: Option<sever::Hat>,
    dismember: bool,
    jump: Option<jump::Jump>,
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
    walk_learned: Option<(learn::Net, f32, f32, bool)>,
    preview_on: bool,
    preview: Option<(locomotion::preview::Preview, Vec3)>,
    preview_wait: bool,
    preview_keys: (bool, bool),
    script_jog: bool,
    ped_color: usize,
    body_alpha: f32,
    hero_alpha: f32,
    render_scale: f32,
    aa: u8,
    smaa_q: u8,
    dlss_q: u8,
    fsr_q: u8,
    xess_q: u8,
    fsr_ok: bool,
    xess_ok: bool,
    msaa: u8,
    msaa_max: u8,
    shadow_q: u8,
    frame_ms: f32,
    gpu: String,
    gpu_now: String,
    gpus: Option<Vec<String>>,
    sharpen: f32,
    sharp_by: [f32; 9],
    ssao: u8,
    ao_strength: f32,
    bloom: f32,
    dof: f32,
    contact: f32,
    contact_k: f32,
    vignette: f32,
    lens: f32,
    chroma: f32,
    sky: u8,
    god_rays: bool,
    fire: u8,
    fire_light: bool,
    fire_homer: bool,
    shadow_filter: u8,
    real_fire: bool,
    water_q: u8,
    voxel_water: bool,
    photons: bool,
    water_caustics: bool,
    water_spray: bool,
    water_waves: bool,
    water_pixels: bool,
    water_haze: bool,
    water_mirror: bool,
    water_foam: bool,
    water_mist: bool,
    water_wet: bool,
    wet_film: bool,
    wet_drips: bool,
    sky_shade: bool,
    water_traced: bool,
    glass_tint: bool,
    glass_shade: bool,
    water_ripples: bool,
    water_refract: bool,
    water_fine: bool,
    water_depth: bool,
    water_relax: bool,
    water_ghost: bool,
    water_vort: bool,
    water_spray_phys: bool,
    safe: bool,
    real_water: bool,
    real_drain: bool,
    pouring: bool,
    real_surface: bool,
    real_ripples: bool,
    water_clear: bool,
    wet_trail: bool,
    water_impact: bool,
    water_faucet: bool,
    flow: Vec<Option<Vec4>>,
    hero_flow: Vec<Option<Vec4>>,
    real_level: Option<f32>,
    flood: Option<flood::Flood>,
    smoke: bool,
    restir: bool,
    rt: u8,
    rt_ok: bool,
    view_px: UVec2,
    defaults: String,
    upscaler: u8,
    dlss_ok: bool,
    fps_limit: f32,
    eyes: bool,
    skel: bool,
    skel_style: u8,
    clean: bool,
    sun_dir: [f32; 2],
    body: usize,
    brows: bool,
    scene: Option<String>,
    switch_to: Option<(String, bool)>,
    poke: f32,
    game_dir: Option<String>,
    watch: Option<Watch>,
    bench: Vec<Benched>,
    homer_id: usize,
    spawn_body: usize,
    base: (Build, Vec<Transform>),
    crowd_build: Build,
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

fn body_of(build: &Build, frames: &[Transform], body: usize) -> (Build, Vec<Transform>, Option<(Vec<f32>, f32)>) {
    let (_, name, h, m, l) = BODIES[body];
    if body == 1 {
        return (build.clone(), frames.to_vec(), None);
    }
    let s = h / BODIES[1].2 / (0.53 * l + 0.47);
    let len: Vec<f32> = build.parts.iter().map(|p| if p.name.contains("_L_") || p.name.contains("_R_") { s * l } else { s }).collect();
    let (b, f) = build.scaled(frames, &len, m / BODIES[1].3);
    eprintln!("body  {name}: trunk x{s:.3}, limbs x{:.3}, girth x{:.3}", s * l, b.parts[0].radius / build.parts[0].radius / s);
    (b, f, Some((len, s * l)))
}

fn v3(x: Vec3) -> String {
    format!("{:.3},{:.3},{:.3}", x.x, x.y, x.z)
}

impl Sim {
    fn reset(&mut self) {
        self.cuts.clear();
        self.jump = None;
        if self.hat.is_some() {
            self.hat = Some(sever::Hat::On);
        }
        let frames = self.frames.clone();
        let a = Agent::new(&self.build, Ragdoll::new(&self.build, &frames, Vec3::ZERO), Quat::IDENTITY, 0.3);
        (self.agent, self.msgs, self.steps, self.held) = (a, msg::State::default(), 0, true);
        self.homer = None;
        self.hero_doll = None;
        self.crowd.truncate(self.bench.len());
        self.crowd_hp.truncate(self.bench.len());
        self.charred.clear();
        self.voxels.clear();
        self.shock = 0.0;
        self.hurt = [0.0; 4];
        self.wounds.clear();
        if matches!(self.room.stage.as_str(), "rooftop" | "bridge") {
            (self.room.snapped, self.room.fell, self.hang) = (None, false, None);
        }
        if let Some(b) = self.room.bridge.as_mut() {
            b.rehang(&mut self.room.w[0]);
            self.room.broke.clear();
        }
        self.say("ours  reset: pose held (pick a behaviour)".into());
    }

    fn reset_here(&mut self, was: Side) {
        let start = self.ours();
        let hips = |s: &Side| (s.p(THIGH_L) + s.p(THIGH_R)) * 0.5;
        let (from, at) = (hips(&start), hips(&was));
        let turn = Quat::from_rotation_z(start.fwd.cross(was.fwd).z.atan2(start.fwd.dot(was.fwd)));
        let under = |p: Vec3| self.world.ground(p + Vec3::Z * 0.3, 4.0).unwrap_or(0.0);
        let to = at.with_z(from.z + under(at) - under(from));
        let fr: Vec<Transform> = self.agent.doll.frames()
            .map(|f| Transform { translation: to + turn * (f.translation - from), rotation: turn * f.rotation, scale: f.scale }).collect();
        self.agent = Agent::new(&self.build, Ragdoll::new(&self.build, &fr, Vec3::ZERO), turn, 0.3);
        self.say(format!("ours  reset: here, hips at {:.2} {:.2} {:.2} (pick a behaviour)", to.x, to.y, to.z));
    }

    fn rise(&mut self, dt: f32) {
        self.rise_sides(dt, SIDES);
    }

    fn rise_sides(&mut self, dt: f32, sides: usize) {
        let fallen = self.agent.fallen;
        if self.xr(0) && fallen && !std::mem::replace(&mut self.was_fallen, fallen) && coin() {
            self.send("armsWindmillAdaptive start=true disableOnImpact=true");
            self.say("ours  off balance: windmilling (additional reactions)".into());
        }
        self.was_fallen = fallen;
        let Some(clips) = self.getup.take() else { return };
        for i in 0..sides {
            let now: Vec<Transform> = self.agent.doll.frames().collect();
            if now.len() != self.build.parts.len() {
                continue;
            }
            let pelvis = now[0].translation;
            if i == 0 && (self.learned.is_some() || self.walk_learned.is_some() || self.preview.is_some()) {
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
                    if (t >= e.min && slow) || (t >= e.max && v < EXIT_SLOW) {
                        self.say(format!("{}  game task: out of NM after {t:.2} s at {v:.2} m/s: {}", WHO[i], if lying { "getting up" } else { "back to the animation" }));
                        self.nm_t[i] = None;
                        self.rises[i] = Some(if lying { clips.start(now) } else { clips.stand(now) });
                    }
                    self.last[i] = pelvis;
                    continue;
                }
                let down = !(i == 0 && (self.getup_mode == 2 || self.riding())) && !self.tasks && self.health >= FATAL() && now[HEAD].translation.z - pelvis.z < LYING_HEAD() && pelvis.distance(self.last[i]) < 0.2 * dt.max(1e-3);
                self.still[i] = if down { self.still[i] + dt } else { 0.0 };
                if self.still[i] >= getup::DOWN_SECS + if i == 0 && self.curled { 3.0 } else { 0.0 } {
                    self.still[i] = 0.0;
                    if i == 0 && self.xr(1) && !self.curled && coin() {
                        self.curled = true;
                        self.send("bodyFoetal start=true");
                        self.say("ours  curled up (additional reactions)".into());
                        self.last[i] = pelvis;
                        continue;
                    }
                    if i == 0 && std::mem::take(&mut self.curled) {
                        self.send("bodyFoetal start=false");
                    }
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

    fn stress(&mut self) {
        let mut at: Vec<(f32, f32)> = (0..49).filter(|&i| i != 24).map(|i| (((i / 7) as f32 - 3.0) * 1.1, ((i % 7) as f32 - 3.0) * 1.1)).collect();
        at.push((4.4, 0.0));
        self.homers("stress", &at);
        self.stress = !self.crowd.is_empty();
    }

    fn stress_step(&mut self) {
        if !self.stress || self.steps % 20 != 0 {
            return;
        }
        let centre = self.ours().p(0);
        for i in 1..=self.crowd.len() {
            let h = ((self.steps / 20) as u32).wrapping_mul(2654435761) ^ (i as u32).wrapping_mul(40503);
            let h = (h ^ (h >> 15)).wrapping_mul(2246822519);
            let h = h ^ (h >> 13);
            if h % 5 != 0 {
                continue;
            }
            let (p, _) = self.crowd[i - 1].0.doll.com();
            let to = (centre - p).with_z(0.0).normalize_or_zero();
            let shove = |k: f32| format!("applyImpulse partIndex={} impulse={}", SHOVE_PART(), v3(to * k + Vec3::Z * 5.0));
            let lines = match (h >> 8) % 9 {
                0 | 1 => vec![shove(60.0 + (h >> 16) as f32 % 40.0), "bodyBalance start=true".into()],
                2 => {
                    let part = [0, 7, 9, 10, 12, 15, 19][(h >> 16) as usize % 7];
                    let at = self.crowd[i - 1].0.doll.frames().nth(part).map_or(p, |f| f.translation);
                    self.crowd_shot(i, part, at, -to, to, false, true);
                    vec![]
                }
                3 => vec!["bodyWrithe start=true".into()],
                4 => vec!["bodyFoetal start=true".into()],
                5 => vec!["armsWindmill start=true".into(), shove(40.0)],
                6 => vec![shove(50.0), "stumble start=true".into()],
                7 => vec![shove(90.0), "catchFall start=true".into()],
                _ => vec![format!("applyImpulse partIndex=0 impulse=0,0,{}", LIFT_IMPULSE()), "highFall start=true".into()],
            };
            for l in lines {
                self.send_to(i, &l);
            }
        }
    }

    fn homers(&mut self, name: &str, at: &[(f32, f32)]) {
        if !self.bench.is_empty() {
            return self.say(format!("room  {name}: remove the spawned Homers first"));
        }
        self.stress = false;
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

    fn spawn(&mut self, key: &str) {
        let Some(body) = BODIES.iter().position(|b| b.0 == key.trim()) else { return self.say(format!("room  spawn: no body {key} (S A L K F T)")) };
        if self.bench.len() + 1 >= HOMERS {
            return self.say(format!("room  spawn: {HOMERS} Homers at most"));
        }
        if self.crowd.len() > self.bench.len() {
            let n = self.bench.len();
            (self.stress, self.car_hit, self.car_reacts) = (false, vec![], vec![]);
            self.crowd.truncate(n);
            self.glow.truncate(n + 1);
            self.rings.retain(|r| r.0 <= n);
            self.say("room  crowd: off (spawned Homers instead)".into());
        }
        self.crowd_hp.resize(self.crowd.len(), 200.0);
        let s = self.ours();
        let hips = (s.p(THIGH_L) + s.p(THIGH_R)) * 0.5;
        let taken: Vec<Vec3> = std::iter::once(&self.agent).chain(self.crowd.iter().map(|c| &c.0)).map(|a| a.doll.com().0.with_z(0.0)).collect();
        let spots = [(2.0, 0.0), (2.0, 1.2), (2.0, -1.2), (3.2, 0.0), (0.0, 1.4), (0.0, -1.4), (3.2, 1.2), (3.2, -1.2), (-2.0, 0.0)];
        let spot = spots.iter().map(|&(f, x)| hips + (s.fwd * f + s.left() * x).with_z(0.0))
            .find(|p| taken.iter().all(|t| t.distance(p.with_z(0.0)) > 0.9)).unwrap_or(hips + s.fwd * 2.0);
        let spot = self.spawn_at.take().map_or(spot, |p| p.extend(hips.z));
        let (i, to) = self.stand_homer(body, spot, s.fwd);
        if self.held {
            self.auto_balance = false;
            self.button("balance");
        }
        self.held = false;
        self.say(format!("room  spawned Homer {} ({}) at {}: {} Homers", self.bench[i].homer_id + 1, BODIES[body].1, v3(to), self.bench.len() + 1));
    }

    fn stand_homer(&mut self, body: usize, spot: Vec3, fwd: Vec3) -> (usize, Vec3) {
        let (build, frames, grow) = body_of(&self.base.0, &self.base.1, body);
        let scale = grow.zip(self.getup.as_ref()).map_or_else(Default::default, |((len, root), c)| c.grown(&len, root));
        let start = Side::new(frames.iter().map(|f| f.translation).collect());
        let ground = self.world.ground(spot + Vec3::Z * 0.3, 4.0).unwrap_or(0.0);
        let floor = build.parts.iter().zip(&frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
        let turn = Quat::from_rotation_z(start.fwd.cross(fwd).z.atan2(start.fwd.dot(fwd)));
        let (from, to) = (((start.p(THIGH_L) + start.p(THIGH_R)) * 0.5).with_z(floor), spot.with_z(ground));
        let fr: Vec<Transform> = frames.iter().map(|f| Transform { translation: to + turn * (f.translation - from), rotation: turn * f.rotation, scale: f.scale }).collect();
        let mut a = Agent::new(&build, Ragdoll::new(&build, &fr, Vec3::ZERO), turn, 0.3);
        let mut st = msg::State::default();
        let _ = msg::send(&mut a, &build, &self.world, &mut st, "bodyBalance", &[("start", Value::Bool(true))]);
        let id = (0..).find(|n| *n != self.homer_id && self.bench.iter().all(|b| b.homer_id != *n)).unwrap_or(0);
        self.crowd.insert(self.bench.len(), (a, st));
        self.crowd_hp.insert(self.bench.len(), 200.0);
        self.bench.push(Benched::new(id, body, build, fr, scale));
        (self.bench.len() - 1, to)
    }

    fn rebody(&mut self, key: &str) {
        let Some(body) = BODIES.iter().position(|b| b.0 == key.trim()) else { return self.say(format!("room  body: no body {key} (S A L K F T)")) };
        self.spawn_body = body;
        let (s, id) = (self.ours(), self.homer_id);
        let (i, _) = self.stand_homer(body, (s.p(THIGH_L) + s.p(THIGH_R)) * 0.5, s.fwd);
        self.take_control(i);
        self.drop_homer(i);
        self.homer_id = id;
        self.say(format!("room  body: Homer {} now {}", id + 1, BODIES[body].1));
    }

    fn swap_homer(&mut self, i: usize) {
        let (Some(b), Some(c), Some(hp)) = (self.bench.get_mut(i), self.crowd.get_mut(i), self.crowd_hp.get_mut(i)) else { return };
        std::mem::swap(&mut self.agent, &mut c.0);
        std::mem::swap(&mut self.msgs, &mut c.1);
        std::mem::swap(&mut self.health, hp);
        macro_rules! own {
            ($($f:ident),*) => { $(std::mem::swap(&mut self.$f, &mut b.$f);)* };
        }
        own!(homer_id, body, build, frames, dead, hurt, wounds, burning, on_fire, charred, wet, voxels, cuts, hat, shock, musc, jump, shot_task, blast_task,
             homer, hang, crush, fall, hold, look_stop, look_after, lean_stop, airborne, dodging, stairs_rolled, was_fallen, curled, homer_rest, learned);
        std::mem::swap(&mut self.rises[0], &mut b.rise);
        std::mem::swap(&mut self.still[0], &mut b.still);
        std::mem::swap(&mut self.last[0], &mut b.last);
        std::mem::swap(&mut self.nm_t[0], &mut b.nm_t);
        std::mem::swap(&mut self.failed[0], &mut b.failed);
        if let Some(g) = self.getup.as_mut() {
            std::mem::swap(&mut g.scale, &mut b.scale);
        }
        let p = i + 1;
        if self.glow.len() <= p {
            self.glow.resize(p + 1, vec![]);
        }
        self.glow.swap(0, p);
        for r in &mut self.rings {
            r.0 = if r.0 == 0 { p } else if r.0 == p { 0 } else { r.0 };
        }
        self.car_hit.resize(self.car_hit.len().max(p + 1), None);
        self.car_hit.swap(0, p);
        self.car_reacts.resize(self.car_reacts.len().max(p + 1), 0);
        self.car_reacts.swap(0, p);
    }

    fn as_homer<R>(&mut self, i: usize, f: impl FnOnce(&mut Self) -> R) -> R {
        self.swap_homer(i);
        let r = f(self);
        self.swap_homer(i);
        r
    }

    fn control(&mut self, i: usize) {
        if i >= self.bench.len() {
            return;
        }
        self.take_control(i);
        let at = self.agent.doll.com().0;
        self.say(format!("room  control: Homer {} ({}) at {}", self.homer_id + 1, BODIES[self.body].1, v3(at)));
    }

    fn take_control(&mut self, i: usize) {
        self.homer_out(true);
        self.stop_walk_learned();
        (self.preview, self.grab, self.replay) = (None, None, None);
        self.script_walk = Vec3::ZERO;
        self.swap_homer(i);
        (self.group, self.tape) = (Default::default(), Default::default());
        self.held = false;
    }

    fn control_id(&mut self, n: usize) {
        match self.bench.iter().position(|b| b.homer_id + 1 == n) {
            Some(i) => self.control(i),
            None => self.say(format!("room  control: {}", if n == self.homer_id + 1 { "that one already" } else { "no such Homer" })),
        }
    }

    fn next_homer(&mut self) {
        let ids = || self.bench.iter().map(|b| b.homer_id);
        match ids().filter(|&n| n > self.homer_id).min().or_else(|| ids().min()) {
            Some(n) => self.control_id(n + 1),
            None => self.say("room  control: one Homer (Character: Spawn Homer for more)".into()),
        }
    }

    fn remove_homer(&mut self) {
        if !self.can_remove() {
            return self.say("room  remove: control a spawned Homer (not Homer 1, not the last)".into());
        }
        let n = self.homer_id + 1;
        self.next_homer();
        self.unspawn(n);
    }

    fn can_remove(&self) -> bool {
        self.homer_id != 0 && !self.bench.is_empty()
    }

    fn unspawn(&mut self, n: usize) {
        let Some(i) = self.bench.iter().position(|b| b.homer_id + 1 == n) else {
            return self.say(format!("room  remove: {}", if n == self.homer_id + 1 { "control another Homer first" } else { "no such Homer" }));
        };
        self.drop_homer(i);
        self.say(format!("room  removed Homer {n}: {} left", self.bench.len() + 1));
    }

    fn drop_homer(&mut self, i: usize) {
        self.bench.remove(i);
        self.crowd.remove(i);
        self.crowd_hp.remove(i);
        if self.glow.len() > i + 1 {
            self.glow.remove(i + 1);
        }
        self.rings.retain(|r| r.0 != i + 1);
        for r in &mut self.rings {
            r.0 -= (r.0 > i + 1) as usize;
        }
        (self.car_hit, self.car_reacts, self.group) = (vec![], vec![], Default::default());
    }

    fn look(&self, h: usize) -> Option<(&[Option<Voxels>], &[f32], f32, f32, usize)> {
        match h {
            0 => Some((&self.voxels, &self.charred, self.burning, self.shock, self.homer_id)),
            _ => self.bench.get(h - 1).map(|b| (&b.voxels[..], &b.charred[..], b.burning, b.shock, b.homer_id)),
        }
    }

    fn who(&self) -> String {
        if self.bench.is_empty() { "Homer".into() } else { format!("Homer {}", self.homer_id + 1) }
    }

    fn bench_frame(&mut self, even: bool) -> Option<Vec<Transform>> {
        self.extras_frame();
        self.burn(1.0 / 60.0);
        self.wounds_frame(1.0 / 60.0);
        if even {
            self.shot_frame();
            self.blast_frame();
            self.self_preserve();
        }
        self.stairs_frame();
        if let Some((net, zero, n, pick)) = self.learned.as_mut() {
            learn::step(net, &net.w, &mut self.agent, &self.build, &self.world, zero, pick);
            *n += 1;
        }
        self.learned_done();
        let mine = (self.walk_learned.take(), self.preview.take());
        self.rise_sides(1.0 / 60.0, 1);
        (self.walk_learned, self.preview) = mine;
        self.getup.as_ref().zip(self.rises[0].as_ref()).and_then(|(c, r)| c.frames(r))
            .map(|f| f.into_iter().enumerate().map(|(k, f)| f.with_rotation(f.rotation * delirium::nmdbm::rage_link(nm(), k))).collect())
    }

    fn build_of(&self, ped: usize) -> &Build {
        ped_build(&self.build, &self.bench, &self.crowd_build, ped)
    }

    fn nearest_benched(&self, p: Vec3) -> Option<usize> {
        let d = |a: &Agent| a.doll.com().0.with_z(0.0).distance(p.with_z(0.0));
        let mine = d(&self.agent);
        (0..self.bench.len()).map(|i| (i, d(&self.crowd[i].0))).filter(|x| x.1 < mine).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0)
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
        let ground = self.cast(s.p(0), -Vec3::Z, 50.0).map_or(0.0, |(t, _)| s.p(0).z - t);
        let c = s.p(0).with_z(ground + 0.9) + fwd * 1.6;
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
            let Some((k, sum, at, _)) = self.car_hit[i].filter(|h| self.steps >= h.3 + CAR_HIT_STEPS) else { continue };
            self.car_hit[i] = None;
            self.car_reacts[i] = self.steps;
            match i {
                _ if i > 0 && i <= self.bench.len() => self.as_homer(i - 1, |s| s.car_react(0, k, sum, at)),
                _ => self.car_react(i, k, sum, at),
            }
        }
    }

    fn car_react(&mut self, i: usize, k: i32, sum: Vec3, at: Vec3) {
        use delirium::bump::{self, Kind, Thing};
        {
            let a = if i == 0 { &self.agent } else { &self.crowd[i - 1].0 };
            let player = self.taxis.get(k as usize).is_some_and(|t| t.player);
            let other = Thing { id: k, kind: Kind::Vehicle { player_driver: player } };
            let imp = bump::Impact { impulse: sum.length(), dt: 1.0 / 60.0, component: a.doll.nearest(self.build_of(i), at) as i32, other: Some(other) };
            let health = if i == 0 { self.health } else { self.crowd_hp[i - 1] };
            let ped = bump::Ped { id: -2 - i as i32, health, inv_mass: 1.0 / a.doll.mass().max(1.0), woke: false, drop: None,
                                  vz: a.doll.vel(0).0.z, tough: false, player_cut: false, player: false, mission: false, quick_relax: false,
                                  agility: self.agility, gun: false, place: a.doll.frames().next().map_or(at, |f| f.translation), vel: Vec3::ZERO, standing_on: None };
            let Some((amount, _)) = bump::damage(&ped, &imp, &mut self.rand).filter(|d| d.0 >= 1.0) else { return };
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
            let who = if i == 0 { self.who() } else { format!("crowd ped {i}") };
            self.say(format!("room  taxi {} hits {who}: {amount:.0} damage (health {left:.0}){}", k + 1,
                             if task.is_empty() { String::new() } else { format!(", {task}") }));
            if task.is_empty() {
                return;
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

    fn fall(&mut self, h: f32) {
        let fr: Vec<Transform> = self.agent.doll.frames().map(|f| f.with_translation(f.translation + Vec3::Z * h)).collect();
        (self.agent, self.msgs) = (Agent::new(&self.build, Ragdoll::new(&self.build, &fr, Vec3::ZERO), Quat::IDENTITY, 0.3), msg::State::default());
        if self.xr(5) {
            let _ = msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, "smartFall", &[("start", Value::Bool(true))]);
        }
        self.held = false;
        self.say(format!("ours  fall: from {h} m{}", if self.xr(5) { ", smartFall" } else { "" }));
    }

    fn in_air(&mut self) {
        if self.xr(4) || self.dead || self.nm_t[0].is_some() || !self.agent.extras.on.is_empty() || self.agent.high_falling()
            || self.agent.doll.com().1.z > -HIGHFALL_SPEED() {
            return;
        }
        self.send("stopAllBehaviours");
        delirium::nm::task_high_fall(&mut self.agent, &self.build, &self.world, &self.tp, "*", false);
        if let Some(e) = exit("highfall").filter(|_| self.tasks) {
            (self.exit, self.nm_t, self.failed) = (e, [Some(0.0); 2], [false; 2]);
        }
        self.say("ours  in the air: the game's NMHighFall".into());
    }

    fn xr(&self, i: usize) -> bool {
        self.extra && self.extra_pick & (1 << i) != 0
    }

    fn riding(&self) -> bool {
        self.skate.as_ref().is_some_and(|s| s.riding)
    }

    fn meat_levels(&self) -> Option<Vec<i8>> {
        let (m, t) = (self.meat.as_ref()?, hom_bones()?);
        let mut v = vec![-1i8; self.build.parts.len()];
        for (r, &k) in HOM_PART.iter().enumerate() {
            if let Some(x) = v.get_mut(k) {
                *x = (*x).max(m.level(t, r));
            }
        }
        Some(v)
    }

    fn toggle_skate(&mut self) {
        if self.riding() {
            if let Some(s) = self.skate.as_mut() {
                s.fling();
                if let Some(l) = s.loose.as_mut() {
                    (l.v, l.w) = (Vec3::ZERO, Vec3::ZERO);
                }
            }
            self.skate_off(Vec3::ZERO, Vec3::ZERO);
            if self.tasks {
                let fr: Vec<Transform> = self.agent.doll.frames().collect();
                self.rises[0] = self.getup.as_ref().map(|c| c.stand(fr));
            } else {
                self.send("bodyBalance start=true");
            }
            return self.say("skate  off: Homer steps off".into());
        }
        let s = self.ours();
        let (pelvis, head) = (s.p(0), s.p(HEAD));
        if head.z - pelvis.z < 0.4 {
            return self.say("skate  Homer's down: J again once he's up".into());
        }
        let feet = pelvis.with_z(self.world.ground(pelvis, 2.0).unwrap_or(pelvis.z - 1.0));
        let fwd = Vec3::Z.cross(s.p(THIGH_R) - s.p(THIGH_L));
        self.mount_skate(feet, fwd.y.atan2(fwd.x));
    }

    fn mount_skate(&mut self, feet: Vec3, yaw: f32) {
        if !skate_files::ready() {
            return self.say("skate  needs your own Skate 3: link its .iso (or default.xex) in Settings, Skate 3 \u{b7} bonus".into());
        }
        self.homer_out(true);
        self.rises[0] = None;
        match self.skate.as_mut().filter(|s| s.near(feet)) {
            Some(s) => s.mount(feet, yaw),
            None => self.skate = Some(skate::Skate::start(self.world.triangles(), feet, yaw)),
        }
        self.say(format!("skate  on: Homer at {}, {}", v3(feet), skate::KEYS));
    }

    fn skate_off(&mut self, v: Vec3, w: Vec3) {
        let fr: Vec<Transform> = self.agent.doll.frames().collect();
        let mut doll = Ragdoll::new(&self.build, &fr, v);
        let c = doll.com().0;
        for (k, f) in fr.iter().enumerate() {
            doll.set_body(k, f.translation, f.rotation, v + w.cross(f.translation - c), w);
        }
        (self.agent, self.msgs) = (Agent::new(&self.build, doll, Quat::IDENTITY, 0.3), msg::State::default());
        self.held = false;
    }

    fn skate_bail(&mut self, why: &str) {
        let Some(s) = self.skate.as_mut().filter(|s| s.riding) else { return };
        let was = s.was.clone();
        let (v, w) = s.fling();
        s.rejoin = true;
        self.skate_off(v, w);
        let c = self.agent.doll.com().0;
        let ground = self.world.ground(c, 30.0).unwrap_or(c.z - 1.0);
        let what = if self.room.pool && world::over_pool(c) {
            "into the pool"
        } else if was.contains("Air") || c.z - ground > 1.3 {
            self.send("stopAllBehaviours");
            delirium::nm::task_high_fall(&mut self.agent, &self.build, &self.world, &self.tp, "*", false);
            "highFall"
        } else if self.room.stairs_ahead(c, v.with_z(0.0).normalize_or_zero()) {
            self.stairs_rolled = true;
            self.send("rollDownStairs start=true");
            "rollDownStairs"
        } else {
            if !self.god {
                self.health = (self.health - (v.length() * 4.0).clamp(1.0, 40.0)).max(1.0);
            }
            self.send("catchFall start=true");
            "catchFall"
        };
        self.say(format!("skate  bail ({why}): {:.1} m/s, spin {:.1} rad/s, {what}", v.length(), w.length()));
        self.meat_start(v.length(), false);
    }

    fn meat_start(&mut self, speed: f32, engine: bool) {
        if self.meat_on {
            let z = self.agent.doll.com().0.z;
            let s = self.ours();
            let last = (0..self.build.parts.len()).map(|k| (Vec3::ZERO, s.p(k))).collect();
            let rot = self.agent.doll.frames().next().map_or(Quat::IDENTITY, |f| f.rotation);
            self.meat = Some(Meat { speed, top: z, low: z, engine, rot, last, ..Default::default() });
            if hom_bones().is_none() {
                self.say("skate  hall of meat: no injuries (Skate 3's bone table not found; set Skate 3 up again)".into());
            }
            if self.meat_screen.is_none() {
                self.meat_screen = Some(hom::Screen::load().map_err(|e| self.say(format!("skate  hall of meat screen: {e}"))).ok());
            }
            if let Some(Some(screen)) = self.meat_screen.as_mut() {
                if let Err(e) = screen.start() {
                    self.say(format!("skate  hall of meat screen: {e}"));
                }
            }
        }
    }

    fn play_skate(&mut self, dt: f32, c: skate_host::bridge::Controls) {
        let Some(s) = self.skate.as_mut() else { return };
        if let Err(e) = s.step(dt, c) {
            self.skate = None;
            return self.say(format!("skate  stopped: {e}"));
        }
        let (Some(clips), Some((feet, vel))) = (self.getup.as_ref(), s.feet()) else { return };
        let Some((mut p, body)) = s.retarget(clips, clips.play(getup::IDLE, 0.0)) else { return };
        #[cfg(feature = "portal")]
        portal::skate_hold(&mut self.portal, clips, &mut p, body);
        let homer = clips.place(&p, body);
        if homer.len() == self.build.parts.len() {
            self.agent.doll = Ragdoll::new(&self.build, &homer, Vec3::ZERO);
        }
        #[cfg(feature = "portal")]
        if let (Some((at, turn)), Some(h)) = (self.portal.held, self.agent.doll.frames().nth(HAND_R)) {
            self.portal.grip = Some((h.rotation.inverse() * (at - h.translation), h.rotation.inverse() * turn));
        }
        let (state, was) = s.pose.as_ref().map_or((String::new(), String::new()), |p| (p.state.clone(), s.was.clone()));
        let (fast, way, edge, board) = (s.recent_speed(), s.recent_way(), s.off_edge(feet), !s.on_foot());
        let trick = s.pose.as_ref().map_or(String::new(), |p| p.trick.clone());
        if trick != s.said {
            s.said = trick.clone();
            self.say(format!("skate  {trick}"));
        }
        if dt <= 0.0 {
            return;
        }
        let flat = vel.with_z(0.0).normalize_or_zero();
        let own = self.skate3_bails;
        let wiping = own && state.contains("Wipeout");
        if wiping && !std::mem::replace(&mut self.meat_wipe, true) {
            self.say(format!("skate  wipeout (Skate 3 physics, {was}): {fast:.1} m/s"));
            self.meat_start(fast, true);
        }
        self.meat_wipe &= wiping;
        let why = if wiping {
            None
        } else if state.contains("Wipeout") {
            Some(if was.contains("Air") { "missed the landing" } else if was.contains("Grind") { "fell off the rail" }
                 else if self.world.contact(feet + Vec3::Z * 0.4, flat, 1.0).is_some() { "hit a wall" } else { "wiped out" })
        } else if !own && board && fast > 4.0 && vel.with_z(0.0).length() < 1.0 && self.world.contact(feet + Vec3::Z * 0.4, way, 1.0).is_some() {
            Some("hit a wall")
        } else if edge {
            Some("rode off the edge")
        } else if board && self.hero.as_ref().is_some_and(|n| n.feet.distance(feet) < 0.6) && vel.length() > 1.5 {
            Some("ran into the Hero")
        } else if self.room.pool && world::over_pool(feet) && feet.z < self.flood.as_ref().map_or(world::WATER(), |f| f.level) - 0.4 {
            Some("rode into the pool")
        } else {
            None
        };
        if let Some(why) = why {
            self.skate_bail(why);
        }
    }

    fn skate_idle(&mut self, dt: f32) {
        let (pool, level) = (self.room.pool, self.flood.as_ref().map_or(world::WATER(), |f| f.level));
        let Some(s) = self.skate.as_mut().filter(|s| !s.riding) else { return };
        s.loose_frame(dt, &self.world, |p| (pool && world::over_pool(p)).then_some(level));
        let rejoin = s.rejoin;
        let o = self.ours();
        if rejoin && !self.agent.fallen && self.rises[0].is_none() && o.p(HEAD).z - o.p(0).z > 0.4 {
            let (feet, f) = (o.p(0).with_z(self.world.ground(o.p(0), 2.0).unwrap_or(o.p(0).z - 1.0)), o.fwd);
            self.mount_skate(feet, f.y.atan2(f.x));
            if let Some(s) = self.skate.as_mut() {
                (s.rejoin, s.step_off) = (false, 0.3);
            }
            return;
        }
        let Some(at) = self.skate.as_ref().and_then(|s| s.loose.as_ref()).as_ref().filter(|l| l.v.length() < 0.3).map(|l| l.deck.translation) else { return };
        if self.walk_dir != Vec3::ZERO && o.p(HEAD).z - o.p(0).z > 0.4 && o.p(0).with_z(0.0).distance(at.with_z(0.0)) < 0.5 {
            let d = self.walk_dir;
            self.mount_skate(at.with_z(self.world.ground(at + Vec3::Z * 0.5, 2.0).unwrap_or(at.z)), d.y.atan2(d.x));
        }
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
        let hit = n.update(clips, &self.world, dt, mv, gait, homer, &fists, held, &caps_of(&self.build, &self.agent.doll), self.room.pool.then_some(self.flood.as_ref().map_or(world::WATER(), |f| f.level)));
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
        let near = self.hero.as_ref().is_some_and(|n| n.feet.with_z(0.0).distance(pelvis.with_z(0.0)) < delirium_data::melee_task::SEEK_BEYOND());
        if self.passive || self.driving || self.dead || self.walk_learned.is_some() || !near || dt <= 0.0 {
            return self.homer_out(true);
        }
        let upright = self.agent.doll.frames().nth(HEAD).is_some_and(|h| h.translation.z - pelvis.z > 0.4);
        let standing = self.rises[0].as_ref().is_none_or(getup::Clips::standing) && upright;
        let stumbling = self.even && self.homer.is_none() && self.agent.doll.com().1.length() > HERO_SLOW;
        if self.homer.is_none() && (self.homer_rest > 0.0 || self.agent.fallen || !standing || stumbling || self.nm_t[0].is_some() || self.musc.is_some() || self.learned.is_some()) {
            return;
        }
        if self.homer.is_none() {
            (self.shot_task, self.blast_task) = (None, None);
            self.say("homer  back in the fight".into());
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
            me.update(clips, &self.world, dt, mv, 0, None, &fists, None, &hero.capsules(clips), self.room.pool.then_some(self.flood.as_ref().map_or(world::WATER(), |f| f.level)));
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
        if !down && self.homer_guard != Some(true) && !me.playing() && delirium_data::melee_task::wants_move(self.homer_wait, to.length(), false, self.rand.float_in_range(0.0, 1.0), dt) {
            me.punch();
            self.homer_wait = Some(delirium_data::melee_task::wait(&mut self.rand));
        }
        me.keys = 0x1 | if self.homer_guard == Some(true) { 0x40 } else { 0 };
        let mv = if to.length() > 1.2 && !me.playing() && !down { to.normalize() } else { Vec3::ZERO };
        let arm = self.homer_weapon.map_or("UNARMED", |w| WEAPONS[w].1);
        let fists = self.weapons.iter().find(|w| w.name == arm).cloned().unwrap_or_default();
        let foe = (hero.feet + Vec3::Z, hero.yaw - std::f32::consts::FRAC_PI_2, 200.0, [None; 5]);
        let body = match &self.hero_doll { Some(h) => caps_of(&h.build, &h.agent.doll), None => hero.capsules(clips) };
        let hero_arm = (self.hero_gun && kind(self.weapon) == 2).then(|| self.weapons.iter().find(|x| x.name == WEAPONS[self.weapon].1)).flatten();
        me.foe = hero.as_foe(clips, hero_arm);
        let hit = me.update(clips, &self.world, dt, mv, 0, (!down).then_some(foe), &fists, self.homer_weapon.and_then(held), &body, self.room.pool.then_some(self.flood.as_ref().map_or(world::WATER(), |f| f.level)));
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
        let vel = shottask::bullet(at, at - dir * BULLET_BACK(), force);
        let how = if self.hero_fall() {
            let k = self.hero_doll.as_ref().map_or(0, |h| h.agent.doll.nearest(&h.build, at));
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
        let build = self.crowd_build.clone();
        let Some(frames) = frames.filter(|f| f.len() == build.parts.len()) else { return false };
        let v = self.hero.as_ref().filter(|_| self.hero_doll.is_none()).map_or(Vec3::ZERO, |n| n.vel);
        let mut agent = Agent::new(&build, Ragdoll::new(&build, &frames, v), Quat::IDENTITY, 0.3);
        let mut st = msg::State::default();
        let _ = msg::send(&mut agent, &build, &self.world, &mut st, "bodyBalance", &[("start", Value::Bool(true))]);
        if let Some(n) = self.hero.as_mut() {
            n.hold = None;
        }
        self.hero_doll = Some(HeroDoll { agent, build, st, t: 0.0, rise: None });
        true
    }

    fn hero_blow(&mut self, at: Vec3, j: Vec3, head: Vec3) -> Option<&'static str> {
        let fwd = self.hero.as_ref()?.facing();
        if !self.hero_fall() {
            return None;
        }
        let h = self.hero_doll.as_ref()?;
        let (k, pelvis) = (h.agent.doll.nearest(&h.build, at), h.agent.doll.frames().next()?.translation);
        let task = if self.rand.float_in_range(0.0, 1.0) > BALANCE_ROLL() { "balance" } else { "flinch" };
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
            self.homer_rest = if self.even { HERO_DOWN } else { HOMER_REST };
        } else if self.tasks {
            self.rises[0] = self.getup.as_ref().map(|c| c.stand(me.homer.clone()));
        } else {
            self.send("bodyBalance start=true");
        }
    }

    fn knocked_out(&mut self) {
        self.say("homer  knocked out of his fight".into());
        self.send("bodyBalance start=true");
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
                (if self.rand.float_in_range(0.0, 1.0) > BALANCE_ROLL() { "balance" } else { "flinch" }, vec![imp(k, at, j)])
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
            let lean = |d: Vec3| format!("leanInDirection start=true applyAsForce=true leanAmount={} dir={}", BUMP_LEAN(), v3(d.with_z(0.0)));
            o.push(lean(pelvis - from));
            self.lean_stop = Some(now + BUMP_LEAN_SECS);
        } else if until != 0 {
            self.lean_stop = Some(now + until as f32 * 1e-3);
        }
        self.send_ours(o);
        if self.tasks {
            (self.exit, self.nm_t, self.failed) = (exit(task).unwrap_or((*SHOT_EXIT())), [Some(0.0); 2], [false; 2]);
        }
        if task == "flinch" {
            self.extra_look(head);
        }
        self.say(format!("{what}: {task} (health {:.0})", self.health));
        if let (hero::Hit::Swing(_, at, _), true) = (hit, self.super_punch && self.hero.is_some()) {
            self.blast(at, false);
        }
    }

    fn stairs_frame(&mut self) {
        if self.agent.balancing() {
            self.stairs_rolled = false;
            return;
        }
        let (com, v) = self.agent.doll.com();
        let on = v.with_z(0.0);
        if !self.stairs_rolled && on.length() > 0.5 && self.room.stairs_ahead(com, on.normalize()) {
            self.stairs_rolled = true;
            self.send_to(0, "rollDownStairs start=true");
            self.say("room  stairs: rolls down".into());
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
        let f = (Vec3::NEG_Z * 9.81 - self.room.com_a) * world::HOMER_KG();
        let side = f.truncate().clamp_length_max(0.3 * world::HOMER_N());
        self.room.push = side.extend(f.z.clamp(-1.6 * world::HOMER_N(), -0.2 * world::HOMER_N()));
        let broke = std::mem::take(&mut self.room.broke);
        if let Some(t) = self.room.snapped.as_mut() {
            *t += 1.0 / 60.0;
            let t = *t;
            let s = self.ours();
            let gap = |h: Vec3| (0..2 * n).map(|k| {
                let p = self.room.bridge.as_ref().unwrap().piece(&self.room.w[0], k / 2, k % 2);
                let l = p.rotation.inverse() * (h - p.translation);
                (l - l.clamp(-(*world::PIECE_HALF()), (*world::PIECE_HALF()))).length()
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
            true => (Vec3::new(0.0, if ahead { 1.0 } else { -1.0 } * (*world::PLANK_HALF()).y, (*world::PLANK_HALF()).z), PLANK_ID + j as i32),
            false => (Vec3::new(0.0, if ahead { world::BRIDGE_FAR() } else { 0.0 }, world::BRIDGE_TOP()), -1),
        };
        let x = Vec3::X;
        self.room.snapped = Some(0.0);
        self.cue();
        (self.walk, self.script_walk) = (false, Vec3::ZERO);
        let x = x * ((*world::PLANK_HALF()).x - 0.1);
        let hold = vec![format!("grab start=true linegrab=true useLeft=true useRight=true dontLetGo=true grabStrength={} bodyStiffness={}                                      grabDist={} reachAngle={} oneSideReachAngle={} pullUpStrengthLeft=0 pullUpStrengthRight=0 pullUpTime=1 pos1={} pos2={} instanceIndex={inst}",
                                    GRAB_STRENGTH(), GRAB_STIFFNESS(), GRAB_DIST(), REACH_ANGLE(), ONE_SIDE_REACH(), v3(edge + x), v3(edge - x)), "dangle start=true hold=false doGrab=true".into()];
        self.hang = Some((hold.clone(), [false; 2]));
        self.send_ours(hold);
        self.say(format!("bridge  plank {} broke (and {}): grab {}", i + 1, step(i).wrapping_add(1), j.wrapping_add(1)));
    }

    fn shot_hand(&mut self, part: usize) {
        let Some(h) = [(14..=16, 0), (18..=20, 1)].into_iter().find(|(r, _)| r.contains(&part)).map(|x| x.1) else { return };
        if !self.hanging() {
            return;
        }
        let Some((lines, free)) = self.hang.as_mut() else { return };
        free[h] = true;
        let grab = lines[0].starts_with("grab");
        if *free != [true; 2] {
            let l = hang_let_go(grab, h);
            self.send_ours(vec![l]);
            return self.say(format!("room  shot in the {} arm: that hand lets go", ["left", "right"][h]));
        }
        self.hang = None;
        self.say("room  shot in his other holding arm: he drops".into());
        if grab {
            return self.send_ours(vec!["grab start=false".into(), "dangle start=false".into()]);
        }
        self.send_ours(vec!["armHang start=false".into()]);
        if self.room.stage == "cab" {
            self.room.fell = true;
            self.send_ours(["pedalLegs", "bodyWrithe"].map(|n| format!("{n} start=false")).into_iter().chain(["smartFall start=true".to_string()]).collect());
        }
    }

    fn hanging(&self) -> bool {
        self.hang.is_some() && self.room.snapped.is_some() && !self.room.fell && !self.dead
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
                let l = format!("armHang start=true constraintPosition={}", v3(Vec3::new(p.x.clamp(-2.85, 2.85), EDGE, 4.0)));
                self.hang = Some((vec![l.clone()], [false; 2]));
                self.send_ours(vec![l]);
                self.say("rooftop  over the edge: he catches it (armHang)".into());
                self.cue();
            }
            Some(t) => {
                *t += 1.0 / 60.0;
                let held = self.agent.extras.on.iter().any(|(n, _)| *n == "armHang");
                if *t > HANG_SECS || *t > 0.5 && !held {
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
            let l = format!("armHang start=true constraintPosition={} instanceIndex={TAXI_ID}", v3(door));
            self.hang = Some((vec![l.clone()], [false; 2]));
            self.send_ours(vec![l]);
            if let Some(c) = self.getup.as_ref().filter(|c| c.hero.is_some() && self.hero.is_none()) {
                self.hero = Some(hero::Hero::new(c, self.agent.doll.com().0, -Vec3::Z.cross(fwd)));
            }
            return self.say("cab  his hands on the cab's door".into());
        };
        let speed = self.taxis.first().map_or(0.0, |c| c.speed.abs());
        let hero = self.hero.is_some();
        if !(hero && *t >= CAB_GO - 1.0 / 60.0 && *t < CAB_GO && !(self.taxis[0].player && speed > 0.5)) {
            *t += 1.0 / 60.0;
        }
        let t = *t;
        let at = |x: f32| t - 1.0 / 60.0 < x && t >= x;
        if at(0.1) {
            self.send_ours(vec!["pedalLegs start=false".into(), "bodyBalance start=true".into()]);
        } else if at(CAB_GO) {
            self.cab_gas = if hero { 0.0 } else { CAB_GAS };
            self.send_ours(vec!["bodyBalance start=false".into(), "pedalLegs start=true".into(), "bodyWrithe start=true mask=uk".into()]);
            self.say(format!("cab  {} pulls away with him on the door", if hero { "Hero" } else { "it" }));
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
        let top = 2.0 * (*world::PAD_HALF()).z;
        let on = self.agent.doll.frames().any(|f| f.translation.z < top + 0.12 && f.translation.x.abs() < (*world::PAD_HALF()).x && f.translation.y.abs() < (*world::PAD_HALF()).y);
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
            "drunk" => vec![format!("bodyBalance start=true armsOutOnPushTimeout={}", DRUNK_TIMEOUT()), format!("leanInDirection start=true leanAmount={} dir={}", DRUNK_LEAN(), v3(s.fwd))],
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
            "lean" => vec!["bodyBalance start=true".into(), format!("leanInDirection start=true leanAmount={} dir={}", LEAN_BUTTON(), v3(s.fwd))],
            "relax" => vec!["bodyRelax start=true".into()],
            "stayup" => vec![format!("stayUpright start=true stiffness={}", STAYUP_STIFFNESS())],
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

    fn fire(&mut self, part: usize, aim: Option<(Vec3, Vec3, Vec3)>, fatal: bool, more: &[usize]) {
        let (at, dir) = aim.map_or((self.agent.doll.centre(part), Vec3::NEG_Y), |a| (a.0, a.1));
        let part = self.sever_shot(part, at, dir);
        self.shot_hand(part);
        (self.rises, self.still) = ([None, None], [0.0; 2]);
        self.homer_rest = self.homer_rest.max(HOMER_REST);
        let before = self.health;
        let was_dead = self.dead || before <= 0.0;
        let (label, name, _) = WEAPONS[self.weapon];
        let hit = self.weapons.iter().find(|w| w.name == name).map(|w| {
            use delirium_data::ped;
            let mut h = ped::Health { health: self.health, ..Default::default() };
            let (mut lost, mut instant) = (0.0, false);
            for &p in std::iter::once(&part).chain(more) {
                let hit = ped::Hit::new(w.damage, w, delirium_data::nm_asset::PART_BONES[p], ped::Attacker::Player { sure: true });
                let o = ped::damage(&mut h, &hit, &ped::Victim::default(), &mut self.rand);
                (lost, instant) = (lost + o.lost, instant || o.flags & ped::INSTANT() != 0);
            }
            self.health = h.health;
            (lost, instant)
        });
        if fatal {
            self.health = 0.0;
        }
        if self.god {
            self.health = 200.0;
        }
        if before >= DYING() && self.health < DYING() {
            self.sounds.push(sfx::Snd::Kill);
        }
        let state = if self.health < FATAL() { "fatally injured" } else if self.health < DYING() { "injured (dying)" } else { "standing" };
        self.say(format!("room  shot ({label}) at part {part} {}: -{:.1}{} -> health {:.1} ({state})",
                         delirium_data::nm_asset::PARTS.get(part).unwrap_or(&"?"), hit.map_or(0.0, |h| h.0), if hit.is_some_and(|h| h.1) { " (instant kill)" } else { "" },
                         self.health));
        if self.tasks {
            (self.exit, self.nm_t, self.failed) = ((*SHOT_EXIT()), [Some(0.0); 2], [false; 2]);
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
        let vel = shottask::bullet(at, at - d.normalize() * BULLET_BACK(), w.map_or(WEAPONS[self.weapon].2, |w| w.force));
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
            let (on, dir) = (back * (at - f.translation), back * n);
            self.rings.push((ped, part, on, dir, 0.0));
            let hurt = |s: &mut Self| {
                s.wound(part, on, dir);
                s.tear(part, at, n, 1.0);
            };
            match ped {
                0 => hurt(self),
                _ if ped <= self.bench.len() => self.as_homer(ped - 1, hurt),
                _ => {}
            }
        }
    }

    fn cut(&mut self, part: usize, at: Vec3, n: Vec3, dmg: f32) {
        self.light(0, part, SHOT_GLOW);
        self.bleed_some(at, n, if bat(self.weapon) { BAT_BLOOD } else { KNIFE_BLOOD });
        self.pop(at, dmg);
        let frame = self.agent.doll.frames().nth(part).map(|f| (f.rotation.inverse(), f.translation));
        if let Some((back, from)) = frame {
            self.rings.push((0, part, back * (at - from), back * n, 0.0));
            if WEAPONS[self.weapon].0 == "knife" {
                self.wound(part, back * (at - from), back * n);
            }
            self.tear(part, at, n, if WEAPONS[self.weapon].0 == "knife" { 1.0 } else { 0.4 });
        }
    }

    fn zap(&mut self, part: usize) {
        self.light(0, part, SHOT_GLOW);
        if self.shock <= 0.0 {
            self.send(&format!("electrocute start=true stunMag={} largeMult=2", STUN_MAG()));
            self.say(format!("room  {} tased", self.who()));
        }
        self.shock = SHOCK;
    }

    fn tear(&mut self, part: usize, at: Vec3, n: Vec3, by: f32) {
        let (Some(def), Some(f)) = (self.build.parts.get(part), self.agent.doll.capsules().nth(part)) else { return };
        if !self.layers {
            return;
        }
        if self.voxels.len() <= part {
            self.voxels.resize_with(part + 1, || None);
        }
        let name = delirium_data::nm_asset::PARTS.get(part).copied().unwrap_or("");
        let back = f.rotation.inverse();
        self.voxels[part].get_or_insert_with(|| Voxels::new(def, name)).dig(back * (at - f.translation), back * -n, VOX_HOLE * (by * self.tear_k).sqrt());
    }

    fn wound(&mut self, part: usize, at: Vec3, n: Vec3) {
        self.wounds.push((part, at, n, 0.0));
        if self.wounds.len() > WOUNDS {
            self.wounds.remove(0);
        }
    }

    fn bleed(&mut self, at: Vec3, n: Vec3) {
        self.bleed_some(at, n, BLOOD_PIX);
    }

    fn spurt(&mut self, at: Vec3, n: Vec3, left: f32) {
        if !self.pixel_blood {
            return;
        }
        for _ in 0..if self.gore { 3 } else { 1 } {
            self.fx_seed = self.fx_seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let r = (self.fx_seed >> 8) as f32 / (1u32 << 24) as f32;
            let v = (n + Vec3::Z * 0.3) * (0.6 + 2.2 * left) * (0.8 + 0.4 * r) + Vec3::new(r - 0.5, 0.5 - r, 0.0) * 0.3;
            self.blood.push((at + n * 0.02, v, 0.0, BLOOD_DROP, Vec3::Z));
        }
        if self.blood.len() > BLOOD_MAX {
            self.blood.drain(..self.blood.len() - BLOOD_MAX);
        }
    }

    fn drip(&mut self, at: Vec3) {
        self.fx_seed = self.fx_seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = (self.fx_seed >> 8) as f32 / (1u32 << 24) as f32 * std::f32::consts::TAU;
        self.blood.push((at, Vec3::new(a.cos() * 0.3, a.sin() * 0.3, -0.5), 0.0, BLOOD_POOL, Vec3::Z));
    }

    fn bleed_some(&mut self, at: Vec3, n: Vec3, count: usize) {
        if !self.pixel_blood {
            return;
        }
        let count = if self.gore { count } else { count / 2 };
        if count >= BLOOD_PIX && self.gore {
            self.drip(at);
        }
        for _ in 0..count {
            let mut r = || {
                self.fx_seed = self.fx_seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (self.fx_seed >> 8) as f32 / (1u32 << 24) as f32
            };
            let v = n * (0.8 + r() * 1.6) + Vec3::new(r() - 0.5, r() - 0.5, r() * 0.8 + 0.4) * 1.6;
            let shade = (r() * 3.0) as u8;
            self.blood.push((at + n * 0.02, v, 0.0, shade, Vec3::Z));
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

    fn meat_tick(&mut self, dt: f32) {
        if self.riding() && self.meat.as_ref().is_some_and(|m| !m.engine) {
            self.meat = None;
        }
        let wiping = self.meat_wipe;
        if self.meat.is_none() {
            if let Some(Some(screen)) = self.meat_screen.as_mut() {
                screen.run = None;
            }
            return;
        }
        let s = self.ours();
        let rot = self.agent.doll.frames().next().map_or(Quat::IDENTITY, |f| f.rotation);
        let engine = self.meat.as_ref().is_some_and(|m| m.engine);
        let now: Vec<(Vec3, Vec3)> = (0..self.build.parts.len()).map(|k| match self.meat.as_ref().filter(|m| m.engine) {
            Some(m) => (m.last.get(k).map_or(Vec3::ZERO, |l| (s.p(k) - l.1) / dt), s.p(k)),
            None => (self.agent.doll.vel(k).0, s.p(k)),
        }).collect();
        let com = self.agent.doll.com().0;
        let w = match self.meat.as_ref().filter(|m| m.engine) {
            Some(m) => (rot * m.rot.inverse()).to_scaled_axis() / dt,
            None => self.agent.doll.vel(0).1,
        };
        let low = now.iter().map(|n| n.1).min_by(|a, b| a.z.total_cmp(&b.z)).unwrap_or(com);
        let up = self.world.ground(low + Vec3::Z * 0.1, 3.0).is_none_or(|g| low.z - g > 0.15);
        let Some(m) = self.meat.as_mut() else { return };
        m.age += dt;
        m.rot = rot;
        m.xray = (m.xray - dt).max(0.0);
        m.shake *= (-8.0 * dt).exp();
        (m.top, m.low) = (m.top.max(com.z), m.low.min(com.z));
        if m.done.is_none() {
            m.turn += (w.x.abs() + w.y.abs() + w.z.abs()) * dt;
            m.flip += w.truncate().length() * dt;
            m.spin += w.z.abs() * dt;
            m.air += up as u8 as f32 * dt;
        }
        let (mut broke, mut hits) = (vec![], vec![0.0; now.len()]);
        for (k, (&(v, p), &(v0, p0))) in now.iter().zip(&m.last).enumerate() {
            let dv = v - v0 + Vec3::Z * 9.81 * dt;
            let hit = dv.length();
            if m.done.is_some() || p.distance(p0) > 1.0 || hit < MEAT_HIT || !(p.z < 0.35 || self.world.contact(p, -dv / hit, 0.35).is_some()) {
                continue;
            }
            m.score += self.agent.doll.part_mass(k) * hit * hit;
            m.first.get_or_insert(m.age);
            hits[k] = hit;
        }
        let t = hom_bones().unwrap_or(&[]);
        let before: Vec<i8> = (0..t.len()).map(|r| m.level(t, r)).collect();
        for r in (0..t.len()).filter(|r| !matches!(r, 14 | 18 | 23 | 24)) {
            let d = hits.get(HOM_PART[r]).copied().unwrap_or(0.0) * MEAT_DAMAGE();
            if d > 0.1 {
                m.hurt(t, r, d, None);
            }
        }
        m.sum.iter_mut().for_each(|s| *s *= 0.8);
        for r in 0..t.len() {
            let l = m.level(t, r);
            if l >= 2 && !m.shown {
                (m.shown, m.xray) = (true, MEAT_XRAY);
            }
            if l >= 4 && before[r] < 4 {
                m.shake = MEAT_SHAKE;
                if !m.broke.iter().any(|b| b.0 == HOM_PART[r]) {
                    m.broke.push((HOM_PART[r], m.age));
                    broke.push(HOM_PART[r]);
                }
            }
        }
        m.still = if now.iter().all(|n| n.0.length() < 0.3) { m.still + dt } else { 0.0 };
        m.last = now;
        let rest = m.done.is_none() && (m.still > 1.0 || m.age > MEAT_MAX || engine && !wiping);
        if rest {
            m.done = Some(m.age);
        }
        let (score, over) = (m.score, m.done.is_some_and(|d| m.age > d + MEAT_HOLD));
        let outro = !m.outro && m.done.is_some_and(|d| m.age > d + MEAT_HOLD - 0.5);
        m.outro |= outro;
        let data = m.data();
        if let Some(Some(screen)) = self.meat_screen.as_mut() {
            let r = if outro { screen.outro() } else { Ok(()) }.and_then(|_| screen.step(data));
            if let Err(e) = r {
                screen.run = None;
                self.say(format!("skate  hall of meat screen: {e}"));
            }
        }
        for k in broke {
            self.say(format!("skate  hall of meat: broke {} ({score:.0})", MEAT_BONES[k]));
        }
        if rest {
            self.say(format!("skate  hall of meat: {:.0} (hits {score:.0}, metrics {:?}, points {:?})", data.score,
                             data.metrics.map(|v| v.round()), data.points.map(|v| v.round())));
        }
        if over {
            self.meat = None;
        }
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
        let vel = shottask::bullet(at, at - dir.normalize() * BULLET_BACK(), w.as_ref().map_or(WEAPONS[self.weapon].2, |w| w.force));
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
        self.extra_look(hit - dir.normalize_or_zero() * 2.0);
    }

    fn cast(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3)> {
        self.world.cast(from, dir, len).into_iter().chain(self.room.cast(from, dir, len)).chain(world::cast_boxes(self.room.resting(), from, dir, len))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    fn self_preserve(&mut self) {
        const DODGE_DUE: f32 = 1.0;
        const BRACE: f32 = 0.45;
        let arms_out = format!("bodyBalance start=true armsOutOnPush=true armsOutOnPushMultiplier=1 armsOutOnPushTimeout={}", ARMS_OUT_TIMEOUT());
        let now = self.steps as f32 / 60.0;
        if self.dead || !(self.xr(4) || self.xr(6) || self.xr(7)) && self.fall.is_none() && self.look_after.is_none() && self.hold.is_none() && !self.airborne {
            return;
        }
        let (me, mv) = self.agent.doll.com();
        let things: Vec<(Vec3, Vec3, bool)> = (0..self.room.objs.len()).map(|j| { let (t, v) = self.room.obj(0, j); (t.translation, v, true) })
            .chain(self.crowd.iter().map(|c| { let (p, v) = c.0.doll.com(); (p, v, false) }))
            .chain(self.hero_doll.iter().map(|h| { let (p, v) = h.agent.doll.com(); (p, v, false) })).collect();
        let overhead = || things.iter().filter_map(|&(p, v, _)| {
            let (r, down) = (p - me, -v.z);
            let h = r.z - 0.8;
            (h > 0.0 && r.with_z(0.0).length() < 2.5 && down > 1.0).then(|| (p, v, (-down + (down * down + 2.0 * 9.81 * h).sqrt()) / 9.81))
        }).filter(|x| x.2 < 3.0).min_by(|a, b| a.2.total_cmp(&b.2));
        if self.look_after.is_some_and(|t| now >= t) {
            self.look_after = None;
            if std::mem::take(&mut self.dodging) {
                self.send("stopAllBehaviours");
                self.send(&arms_out);
            }
            let down = things.iter().map(|x| x.0).filter(|p| (*p - me).length() < 4.0).min_by(|a, b| (*a - me).length().total_cmp(&(*b - me).length()));
            if let Some(at) = down {
                self.extra_look(at);
                self.look_stop = Some(now + 2.5);
            }
        }
        let land = self.cast(me, -Vec3::Z, 50.0).map_or(f32::MAX, |(t, _)| (t - 0.95) / (-mv.z).max(0.1));
        if self.airborne {
            if land < 0.12 || mv.z > 0.0 {
                self.airborne = false;
                self.send("bodyBalance start=true");
            }
        } else if self.xr(4) && self.homer.is_none() && mv.z < -0.8 && land > 0.4 && self.agent.balancing() {
            if mv.truncate().length() > 0.8 && land > 0.8 {
                self.send("stopAllBehaviours");
                self.send(&format!("smartFall start=true tumble={TUMBLE}"));
                self.say("ours  over the edge: tumbling (additional reactions)".into());
            } else {
                self.airborne = true;
                self.send("highFall start=true");
                self.say("ours  dropping: arms out (additional reactions)".into());
            }
        }
        let wet = self.room.pool && world::over_pool(me) && me.z < if self.wet { 0.8 } else { 0.3 };
        if wet != self.wet {
            self.wet = wet;
            self.say(format!("ours  {} the pool at {}", if wet { "into" } else { "out of" }, v3(me)));
            let real = if self.real_water { " buoyancy=0 damping=0" } else { "" };
            self.send(&if wet { format!("buoyancy start=true surfacePoint={}{real}", v3(Vec3::Z * world::WATER())) } else { "buoyancy start=false".into() });
        }
        if self.hold.is_some_and(|t| now >= t) {
            self.hold = None;
            self.send("grab start=false");
        }
        let (go, chest) = (mv.with_z(0.0), self.ours().p(CHEST));
        if self.xr(7) && self.homer.is_none() && self.hold.is_none() && self.fall.is_none() && !(self.walk && self.walk_dir != Vec3::ZERO) && self.preview.is_none() && go.length() > 0.4 && chest.z - me.z > 0.2 {
            let d = go.normalize();
            let reach = 0.35 + (0.3 * go.length()).min(0.7);
            let hit = [chest.z, me.z, me.z - 0.3].into_iter().find_map(|z| {
                let from = me.with_z(z);
                self.cast(from, d, reach).filter(|h| h.1.z.abs() < 0.5).map(|(t, n)| (from + d * t, n.with_z(0.0).normalize()))
            });
            if let Some((h, n)) = hit {
                let head = chest.z + 0.45;
                let right = (-n).cross(Vec3::Z);
                let top = (h - n * 0.03).with_z(head);
                let top = top.with_z(head - self.cast(top, -Vec3::Z, 1.5).map_or(0.0, |x| x.0));
                let l = if self.cast(me.with_z(head), d, reach + 0.2).is_some() {
                    let at = h.with_z(chest.z);
                    format!("grab start=true justbrace=true useLeft=true useRight=true pos1={} pos2={} normalR={} normalL={}",
                            v3(at + right * 0.25), v3(at - right * 0.25), v3(n), v3(n))
                } else if top.z > me.z - 0.15 {
                    format!("grab start=true linegrab=true useLeft=true useRight=true pos1={} pos2={}", v3(top + right * 0.3), v3(top - right * 0.3))
                } else {
                    String::new()
                };
                if !l.is_empty() {
                    self.hold = Some(now + 2.0);
                    self.send(&l);
                    self.say(format!("ours  into a wall/rail: {} (additional reactions)", if l.contains("justbrace") { "hands on it" } else { "grabs it" }));
                }
            }
        }
        if let Some((land, braced)) = self.fall {
            let seen = overhead();
            if seen.is_none() && now > land + 0.3 || now > land + 1.5 {
                (self.fall, self.look_after) = (None, Some(now + 0.4));
                return;
            }
            let Some((at, v, t)) = seen else { return };
            self.fall = Some((now + t, braced || t < BRACE));
            if !braced && t < BRACE {
                let away = (me - at).with_z(0.0).normalize_or(-self.ours().fwd.with_z(0.0));
                self.dodging = true;
                self.lean_stop = Some(now + 0.8);
                for l in ["pointArm start=false".to_string(), arms_out.clone(), format!("upperBodyFlinch start=true pos={} newHit=true", v3(at)),
                          format!("leanInDirection start=true applyAsForce=true leanAmount={} dir={}", DODGE_LEAN(), v3(away))] {
                    self.send(&l);
                }
            } else if !braced && self.steps % 10 == 0 {
                self.send(&format!("pointArm start=true target={}", v3(at)));
            }
            return;
        }
        if !self.xr(6) || self.homer.is_some() || now < self.dodge_at || !self.agent.balancing() {
            return;
        }
        if let Some((at, v, t)) = overhead() {
            self.dodge_at = now + 3.0;
            if (at - me).with_z(0.0).dot(self.ours().fwd) < -0.2 {
                self.look_after = Some(now + t + 0.7);
                self.send(&arms_out);
                return self.say("ours  something falling on him from behind: arms out (additional reactions)".into());
            }
            self.fall = Some((now + t, false));
            self.look_stop = None;
            for l in [format!("headLook start=true pos={} vel={} alwaysLook=true", v3(at), v3(v)), format!("pointArm start=true target={}", v3(at))] {
                self.send(&l);
            }
            return self.say("ours  something falling on him: watches it, arm up (additional reactions)".into());
        }
        let due = |p: Vec3, v: Vec3| (me - p).length() / (me - p).normalize_or_zero().dot(v - mv).max(1e-3);
        let Some(&(at, v, thrown)) = things.iter().find(|&&(p, v, _)| (me - p).length() < 30.0 && (me - p).normalize_or_zero().dot(v - mv) > 3.0 && due(p, v) < DODGE_DUE)
        else { return };
        let away = (me - at).with_z(0.0).normalize_or(-self.ours().fwd.with_z(0.0));
        self.dodge_at = now + 3.0;
        self.look_stop = Some(now + 2.0);
        self.lean_stop = Some(now + DODGE_DUE);
        let lean = format!("leanInDirection start=true applyAsForce=true leanAmount={} dir={}", BRACE_LEAN(), v3(away));
        let look = format!("headLook start=true pos={} vel={} alwaysLook=true", v3(at), v3(v));
        if thrown {
            self.dodging = true;
            for l in [look, format!("upperBodyFlinch start=true pos={} newHit=true", v3(at)), lean] {
                self.send(&l);
            }
        } else {
            for l in [arms_out, look, lean] {
                self.send(&l);
            }
        }
        self.say("ours  something coming: dodging (additional reactions)".into());
    }

    fn extra_look(&mut self, at: Vec3) {
        if self.xr(3) && !self.dead {
            self.send(&format!("headLook start=true pos={} alwaysLook=true", v3(at)));
            self.look_stop = Some(self.steps as f32 / 60.0 + 2.0);
        }
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
            "tackle high" | "tackle low" | "tackle high keep" | "tackle low keep" => return self.tackle_start(b.contains("low"), b.ends_with("keep")),
            "hat" => return self.hat_button(),
            "next homer" => return self.next_homer(),
            "remove homer" => return self.remove_homer(),
            _ if b.starts_with("body ") => return self.rebody(&b[5..]),
            _ if b.starts_with("spawn ") => return self.spawn(&b[6..]),
            _ if b.starts_with("control ") => return self.control_id(b[8..].trim().parse().unwrap_or(0)),
            _ if b.starts_with("unspawn ") => return self.unspawn(b[8..].trim().parse().unwrap_or(0)),
            "jump" | "jump dive" => return self.jump_start(b == "jump dive", None),
            "dismember" => {
                self.dismember = !self.dismember;
                return self.say(format!("sever  dismember: {}", if self.dismember { "on (shots and blasts cut limbs off)" } else { "off" }));
            }
            _ if b.starts_with("sever ") => {
                if let Some(k) = sever::CUTS.iter().find(|c| c.0 == b).map(|c| c.1).or_else(|| b[6..].parse().ok()).filter(|&k| k < self.build.parts.len()) {
                    let at = self.agent.doll.centre(k);
                    let out = (at - self.agent.doll.centre(0)).with_z(0.0).normalize_or(Vec3::X);
                    if !self.sever(k, at, out * sever::KICK * 0.5) {
                        self.say("sever  already off".into());
                    }
                }
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
            "experimental walk" => {
                self.preview_on = !self.preview_on;
                return self.say(format!("ours  experimental walk: {}", if self.preview_on { "on; W walks, Shift+W jogs (straight only), let go to stop" } else { "off" }));
            }
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
            "even fight" => {
                self.even = !self.even;
                return self.say(format!("room  even fight: {}", if self.even { "on (Homer gets back into the fight as fast as the Hero)" } else { "off" }));
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
            "detached cam" => {
                self.relock = true;
                return;
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
        self.stop_walk_learned();
        if b == "RESET pose" || b == "RESET here" {
            let was = self.ours();
            (self.health, self.lean_stop, self.shot_task, self.dead) = (200.0, None, None, false);
            self.auto_balance = self.script.is_empty();
            self.reset();
            if b == "RESET here" {
                self.reset_here(was);
            }
            return;
        }
        if b == "shot" {
            self.fire(CHEST, None, false, &[]);
            return;
        }
        if b == "shot back" {
            let s = self.ours();
            self.fire(CHEST, Some((s.p(CHEST) - s.fwd * 0.1, s.fwd, -s.fwd)), false, &[]);
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
        if let Some(kind) = (*world::KINDS()).iter().position(|k| b.ends_with(k.0)) {
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
            "stress" => return self.stress(),
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
            "fall" => return self.fall(3.0),
            "high fall" => return self.fall(HIGH_DROP),
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
            return self.gun.rockets.push((from, dir * ROCKET_SPEED(), 0.0));
        }
        let (_, acc, pellets, _) = self.gun_stats();
        self.pellets(from, dir, acc, pellets, fatal);
    }

    fn bullet_any(&mut self, from: Vec3, dir: Vec3) -> Option<(usize, (usize, Vec3, Vec3))> {
        self.sounds.push(sfx::Snd::Shot(self.weapon));
        let hit = std::iter::once(&self.agent).chain(self.crowd.iter().map(|c| &c.0)).enumerate()
            .filter_map(|(i, a)| a.doll.probe(self.build_of(i), from, from + dir * 60.0).map(|h| (i, h)))
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
        let held = self.gun.held;
        gamenums::gun_spread(WEAPONS[self.weapon].0, held, len, acc, || self.gun.rnd())
    }

    fn pellets(&mut self, from: Vec3, dir: Vec3, acc: f32, pellets: usize, fatal: bool) {
        if WEAPONS[self.weapon].0 == "taser" {
            match self.bullet_any(from, dir) {
                Some((0, (part, _, _))) => self.zap(part),
                Some((i, (part, _, _))) if i <= self.bench.len() => self.as_homer(i - 1, |s| s.zap(part)),
                _ => {}
            }
            return;
        }
        let (right, up) = dir.any_orthonormal_pair();
        let (mut hit, mut hurt, mut more) = (None, vec![], vec![]);
        for _ in 0..pellets {
            let d = if pellets > 1 {
                let half = PELLET_SPREAD() * acc;
                let (a, b) = (self.gun.rnd() * 2.0 - 1.0, self.gun.rnd() * 2.0 - 1.0);
                (dir + right * a * half + up * b * half).normalize()
            } else {
                dir
            };
            match self.bullet_any(from, d) {
                Some((0, h)) => match hit {
                    None => hit = Some((h, d)),
                    Some(_) => {
                        self.mark(0, h.0, h.1, h.2);
                        more.push(h.0);
                    }
                },
                Some((i, h)) if i <= self.bench.len() => {
                    let first = !hurt.contains(&i);
                    hurt.push(i);
                    self.as_homer(i - 1, |s| if first { s.fire(h.0, Some((h.1, d, h.2)), fatal, &[]) } else { s.mark(0, h.0, h.1, h.2) });
                }
                Some((i, h)) => {
                    let first = !hurt.contains(&i);
                    hurt.push(i);
                    self.crowd_shot(i, h.0, h.1, d, h.2, fatal, first);
                }
                None => {}
            }
        }
        match hit {
            Some(((part, at, normal), d)) => self.fire(part, Some((at, d, normal)), fatal, &more),
            None if hurt.is_empty() => self.near_miss(from, dir),
            None => {}
        }
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
            self.gun.rockets.push((from, self.gun.dir * ROCKET_SPEED(), 0.0));
            (self.gun.kick, self.gun.flash) = (KICK[self.weapon], 0.08);
            return;
        }
        let len = from.distance(self.gun.aim).max(1.0);
        let (r, _) = self.gun_spread(len, acc);
        let (right, up) = self.gun.dir.any_orthonormal_pair();
        let t = self.gun.rnd() * std::f32::consts::TAU;
        let dir = (self.gun.dir * len + (right * t.sin() + up * t.cos()) * r).normalize();
        self.pellets(from, dir, acc, pellets, fatal);
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
            *v += v.normalize_or(Vec3::Z) * ROCKET_SPEED() * dt - Vec3::Z * 9.8 * dt;
            let (d, len) = (v.normalize_or(Vec3::Z), v.length() * dt);
            let hit = self.agent.doll.probe(&self.build, *p, *p + d * len).map(|h| h.1)
                .or_else(|| self.world.contact(*p, d, len).map(|c| *p + d * c.0));
            *age += dt;
            *p += d * len;
            match hit.or((*age > ROCKET_LIFE()).then_some(*p)) {
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
        let Some((part, at, _)) = self.agent.doll.probe(&self.build, self.gun.at, self.gun.aim + dir * 0.3) else {
            self.sounds.push(sfx::Snd::Throw);
            return self.say(format!("room  {}: missed", WEAPONS[self.weapon].0));
        };
        let (dmg, push) = w.map_or((20.0, hero::MELEE_PUSH), |w| (w.damage, w.force.max(hero::MELEE_PUSH)));
        let j = if bat(self.weapon) { (dir * 0.7 - dir.cross(Vec3::Z).normalize_or(Vec3::X)).normalize() * push * BAT_PUSH } else { dir * push };
        let what = format!("room  {} lands", WEAPONS[self.weapon].0);
        self.land(hero::Hit::Swing(dmg, at, j), self.gun.at, &what);
        if WEAPONS[self.weapon].0 == "baton" {
            self.sounds.push(sfx::Snd::Shot(11));
            self.shock = 0.0;
            self.zap(part);
        }
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

    fn pose(&self) -> Vec<Transform> {
        self.shown().map(|f| f.links.clone()).or_else(|| self.getup.as_ref().zip(self.rises[0].as_ref()).and_then(|(c, r)| c.frames(r)))
            .unwrap_or_else(|| self.agent.doll.frames().collect())
    }

    fn soak(&mut self) {
        let n = self.build.parts.len();
        for i in 0..self.bench.len() {
            let f = self.flow.get((i + 1) * n..(i + 2) * n).map_or(vec![], <[_]>::to_vec);
            self.as_homer(i, |s| s.soak_in(&f));
        }
        let f = self.flow.get(..n).map_or(vec![], <[_]>::to_vec);
        self.soak_in(&f);
    }

    fn soak_in(&mut self, flow: &[Option<Vec4>]) {
        if flow.is_empty() || self.homer.is_some() {
            return;
        }
        for (k, c) in caps_of(&self.build, &self.agent.doll).into_iter().enumerate() {
            let Some(Some(f)) = flow.get(k) else { continue };
            let m = self.agent.doll.part_mass(k);
            let imp = fluid_gpu::push(*f, (c.0, c.1, c.2), m / BODY_RHO, m, c.3, 1.0 / 60.0);
            if imp != Vec3::ZERO {
                self.agent.doll.impulse(k, (c.0 + c.1) * 0.5, imp);
            }
        }
    }

    pub fn hero_caps(&self) -> Vec<Capsule> {
        match (&self.hero_doll, &self.hero, &self.getup) {
            _ if self.driving => vec![],
            (Some(h), ..) => caps_of(&self.build, &h.agent.doll),
            (None, Some(n), Some(c)) => n.capsules(c).into_iter().filter(|c| !c.4).collect(),
            _ => vec![],
        }
    }

    fn soak_hero(&mut self, dt: f32, doll: bool) {
        if self.hero_flow.is_empty() {
            if let Some(n) = self.hero.as_mut() {
                n.drift = Vec3::ZERO;
            }
            return;
        }
        if self.hero_doll.is_some() != doll || dt <= 0.0 {
            return;
        }
        let body = self.hero_caps();
        if let Some(h) = self.hero_doll.as_mut() {
            for (k, c) in body.iter().enumerate() {
                let Some(Some(f)) = self.hero_flow.get(k) else { continue };
                let m = h.agent.doll.part_mass(k);
                let imp = fluid_gpu::push(*f, (c.0, c.1, c.2), m / BODY_RHO, m, c.3, dt);
                if imp != Vec3::ZERO {
                    h.agent.doll.impulse(k, (c.0 + c.1) * 0.5, imp);
                }
            }
            return;
        }
        let Some(n) = self.hero.as_mut() else { return };
        let mut dv = Vec3::ZERO;
        for (k, c) in body.iter().enumerate() {
            let Some(Some(f)) = self.hero_flow.get(k) else { continue };
            let vol = std::f32::consts::PI * c.2 * c.2 * (c.0.distance(c.1) + 4.0 / 3.0 * c.2);
            dv += fluid_gpu::push(*f, (c.0, c.1, c.2), vol, vol * BODY_RHO, n.drift, dt) / HERO_MASS;
        }
        n.drift += dv.with_z(0.0);
        let grip = if n.swimming { 0.0 } else { HERO_GRIP };
        n.drift *= (n.drift.length() - grip * dt).max(0.0) / n.drift.length().max(1e-6);
    }

    fn water_level(&self) -> f32 {
        self.real_level.or(self.flood.as_ref().map(|f| f.level)).unwrap_or(world::WATER())
    }

    fn in_water(&self, p: Vec3) -> bool {
        let d = p - SKY_POOL;
        self.room.pool && (world::over_pool(p) && p.z < self.water_level() || self.flood.as_ref().is_some_and(|f| f.depth_at(p) > 0.01 && p.z < f.depth_at(p) + 0.05))
            || self.room.stage == "pttest" && d.x.abs() < SKY_HALF.x && d.y.abs() < SKY_HALF.y && (0.0..SKY_DEEP).contains(&d.z)
    }

    fn flood_toggle(&mut self) {
        self.pouring = !self.pouring && self.room.pool;
        self.water_sync();
    }

    fn set_real_water(&mut self, on: bool) {
        self.real_water = on;
        self.water_sync();
    }

    fn water_sync(&mut self) {
        if self.real_water || !self.room.pool {
            self.flood = None;
        } else if self.pouring && self.flood.is_none() {
            self.flood = Some(flood::Flood::new(&self.world));
        }
        if let Some(f) = &mut self.flood {
            f.pouring = self.pouring;
        }
    }

    fn drain(&mut self) {
        (self.pouring, self.real_drain, self.flood) = (false, self.real_water, None);
    }

    fn fire_frame(&mut self, dt: f32) {
        for f in &mut self.gun.fires {
            f.1 += dt;
        }
        self.gun.fires.retain(|f| f.1 < FIRE_LIFE);
        if self.room.pool {
            let doused: Vec<Vec3> = self.gun.fires.iter().map(|f| f.0)
                .filter(|&at| (world::over_pool(at) && at.z < self.water_level() + 0.05) || self.flood.as_ref().is_some_and(|f| f.depth_at(at) > 0.005)).collect();
            self.gun.fires.retain(|f| !doused.contains(&f.0));
            if !doused.is_empty() {
                self.steam = (self.steam.0 + 1, doused.iter().map(|p| p.with_z(world::WATER())).collect());
            }
        }
        self.burn(dt);
    }

    fn burn(&mut self, dt: f32) {
        if self.shock > 0.0 {
            self.shock -= dt;
            if self.shock <= 0.0 {
                self.send("electrocute start=false");
            }
        }
        if self.room.pool {
            let ours = self.ours();
            let wet = (0..ours.parts.len()).filter(|&k| world::over_pool(ours.p(k)) && ours.p(k).z < world::WATER()).count();
            if self.burning > 0.0 && wet * 2 > ours.parts.len() {
                self.steam = (self.steam.0 + 1, (0..ours.parts.len()).map(|k| ours.p(k).with_z(world::WATER())).collect());
                self.burning = 0.0;
                self.say("room  the water puts Homer out".into());
            }
        }
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
            self.health = if self.god { 200.0 } else { (self.health - BURN_DAMAGE() * dt).max(0.0) };
            let n = self.ours().parts.len();
            self.charred.resize(n, 0.0);
            for (k, c) in self.charred.iter_mut().enumerate() {
                *c = (*c + dt / CHAR_TIME * (0.6 + 0.8 * ((k as f32 * 12.9898).sin() * 43758.545).rem_euclid(1.0))).min(CHAR_MAX);
            }
        }
        if burning && self.on_fire.is_none() && self.fire_react {
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
        for i in 0..self.bench.len() {
            self.as_homer(i, |s| s.blast_on(at, false));
        }
        self.blast_on(at, true);
    }

    fn blast_on(&mut self, at: Vec3, game: bool) {
        self.sever_blast(at);
        let ours = self.ours();
        let near = (0..ours.parts.len()).map(|k| ours.p(k).distance(at)).fold(f32::MAX, f32::min);
        let who = if self.bench.is_empty() { "him".to_string() } else { self.who() };
        if near >= BLAST_R {
            return if game { self.say(format!("room  blast: {near:.1} m from {who}")) };
        }
        let mut o = vec![];
        let mut hit = vec![];
        for k in 0..ours.parts.len() {
            let p = ours.p(k);
            let f = 1.0 - p.distance(at) / BLAST_R;
            if f <= 0.0 || self.agent.doll.gone >> k & 1 != 0 {
                continue;
            }
            let imp = ((p - at).normalize_or(Vec3::Z) + Vec3::Z * 0.5).normalize() * self.agent.doll.part_mass(k) * BLAST_DV * f;
            hit.push((k, p, (at - p).normalize_or(Vec3::Z), f));
            o.push(format!("applyImpulse partIndex={k} hitPoint={} impulse={}", v3(p), v3(imp)));
        }
        let lost = BLAST_DAMAGE * (1.0 - near / BLAST_R);
        for &(k, ..) in &hit {
            self.light(0, k, SHOT_GLOW);
        }
        if let Some(&(k, p, n, _)) = hit.iter().max_by(|a, b| a.3.total_cmp(&b.3)) {
            self.mark(0, k, p, n);
            self.bleed_some(p, n, BLOOD_PIX * 2);
            self.pop(p, lost);
        }
        self.health = if self.god { 200.0 } else { (self.health - lost).max(0.0) };
        self.say(format!("room  blast: {near:.1} m from {who}: -{lost:.0} -> health {:.0}", self.health));
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
        let now = self.steps as f32 / 60.0;
        if now - std::mem::replace(&mut self.near_last, now) >= self.dodge_gap {
            self.dived = false;
        }
        if self.dead || !self.agent.balancing() && !learn::standing(&self.agent.doll) {
            if !self.dead && self.xr(2) {
                if !self.curled {
                    self.curled = true;
                    self.send("bodyFoetal start=true");
                }
                self.still[0] = 0.0;
                self.send(&format!("headLook start=true pos={} alwaysLook=true", v3(p)));
                return self.say(format!("room  shot: near miss, {d:.2} m from him (down: curls up, additional reactions)"));
            }
            return self.say(format!("room  shot: near miss, {d:.2} m from him (down: no reaction)"));
        }
        self.say(format!("room  shot: near miss, {d:.2} m from him"));
        let since = now - self.near_at;
        if since >= self.dodge_gap || since < 0.0 {
            self.near_at = now;
        }
        if self.dive && (NEAR_BURST..self.dodge_gap).contains(&since) && !self.no_dodge && !self.dived {
            (self.near_at, self.dived) = (now, true);
            self.say("room  shot at again: he dives".into());
            let chest = ours.p(CHEST);
            let off = (chest - by(chest)).with_z(0.0);
            let away = if off.length() > 0.05 { off.normalize() } else { dir.cross(Vec3::Z).normalize_or(Vec3::X) };
            if self.agent.balancing() && self.jump.is_none() {
                return self.jump_start(true, Some(away));
            }
            self.agent.doll.kick(away * DIVE.0 + Vec3::Z * DIVE.1);
            return self.send_ours(vec![format!("shootDodge start=true rightArmTarget={} leftArmTarget={}", v3(from), v3(from))]);
        }
        let msgs = |p: Vec3| vec!["bodyBalance start=true".into(), format!("upperBodyFlinch start=true pos={} newHit=true", v3(p)), format!("headLook start=true pos={} alwaysLook=true", v3(p))];
        let o = msgs(p);
        self.send_ours(o);
        if let Some(e) = exit("flinch").filter(|_| self.tasks) {
            (self.exit, self.nm_t, self.failed) = (e, [Some(0.0); 2], [false; 2]);
        }
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
        #[cfg(feature = "portal")]
        if let Some((at, aim)) = self.portal.float {
            (self.gun.at, self.gun.aim) = (at, aim);
        }
        let nearest = |doll: &delirium::ragdoll::Ragdoll, p: Vec3| doll.frames().map(|f| (f.translation, f.translation.distance(p)))
            .fold((p, f32::MAX), |a, b| if b.1 < a.1 { b } else { a });
        if let Some((from, dir)) = ray {
            if !hold {
                self.gun.at = from + dir * ((chest - from).dot(dir) - 0.5).max(0.5);
            }
            if portal_gun(self) {
                self.gun.aim = from + dir * self.world.contact(from, dir, 200.0).map_or(50.0, |c| c.0);
            } else if !hold {
                self.gun.aim = nearest(&self.agent.doll, self.gun.at).0;
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
            let z = if k == 3 { world::BARREL_HALF() } else { (*world::KINDS())[2].1 } + 0.01;
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
        let (label, _, kg, speed) = (*world::KINDS())[kind];
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
            if self.dead {
                return m;
            }
            if self.look {
                m.push(format!("headLook start=true pos={} vel={} instanceIndex=-1 alwaysLook=true", v3(p), v3(v)));
            }
            if self.avoid {
                m.push(format!("upperBodyFlinch start=true pos={} newHit=true", v3(p)));
            }
            m
        };
        let o = msgs(from, vel);
        if self.avoid && !self.dead {
            (self.dodging, self.look_stop) = (true, Some(self.steps as f32 / 60.0 + 2.0));
        }
        self.send_ours(o);
    }

    fn objects(&mut self) {
        let caps = |fr: Vec<Transform>, b: &Build| fr.into_iter().zip(&b.parts).map(|(t, p)| (t, p.radius, p.half)).collect::<Vec<_>>();
        let mut ours = caps(self.agent.doll.capsules().collect(), &self.build);
        let n = ours.len().max(1);
        for (c, b) in self.crowd.iter().zip(&self.bench) {
            ours.extend(caps(c.0.doll.capsules().collect(), &b.build));
        }
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
                match k / n {
                    0 => self.obj_hit(i, j, k, at, imp),
                    h => self.as_homer(h - 1, |s| s.obj_hit(i, j, k % n, at, imp)),
                }
            }
        }
    }

    fn obj_hit(&mut self, i: usize, j: usize, k: usize, at: Vec3, imp: Vec3) {
        if !self.room.objs[j].hit[i] && imp.length() > 1.0 {
            self.room.objs[j].hit[i] = true;
            self.say(format!("{}  {} hits {} ({:.1} N s)", WHO[i], (*world::KINDS())[self.room.objs[j].kind].0,
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
        if imp.length() > 1.0 && self.riding() {
            self.skate_bail(&format!("hit by a {}", (*world::KINDS())[self.room.objs[j].kind].0));
        }
        if imp.length() > 1.0 && self.homer.is_some() {
            self.knocked_out();
        }
        self.held = false;
        let ps = [("start", Value::Bool(true)), ("partIndex", Value::Int(k as i32)), ("hitPoint", Value::Vec3(at)), ("impulse", Value::Vec3(imp))];
        let _ = msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, "applyImpulse", &ps);
    }

    fn send(&mut self, line: &str) {
        self.send_to(0, line);
        if line.starts_with("stopAllBehaviours") && self.hanging() {
            let (lines, free) = self.hang.clone().unwrap();
            let grab = lines[0].starts_with("grab");
            for l in lines.into_iter().chain((0..2).filter(|&i| free[i]).map(|i| hang_let_go(grab, i))) {
                self.send_to(0, &l);
            }
        }
        if line.starts_with("stopAllBehaviours") && self.wet {
            self.send_to(0, &format!("buoyancy start=true surfacePoint={}", v3(Vec3::Z * world::WATER())));
        }
    }

    fn send_to(&mut self, who: usize, line: &str) {
        if who == 0 {
            self.skate_bail(&format!("hit: {}", line.split_whitespace().next().unwrap_or("")));
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
        let (agent, st, tag, build) = match who {
            0 => (&mut self.agent, &mut self.msgs, String::new(), &self.build),
            HERO => match self.hero_doll.as_mut() {
                Some(h) => (&mut h.agent, &mut h.st, "hero ".to_string(), &h.build),
                None => return,
            },
            i => match self.crowd.get_mut(i - 1) {
                Some(c) => (&mut c.0, &mut c.1, format!("crowd {i} "), ped_build(&self.build, &self.bench, &self.crowd_build, i)),
                None => return,
            },
        };
        match msg::send(agent, build, &self.world, st, name, &ps) {
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
    let body = a.iter().position(|x| x == "--body").and_then(|i| a.get(i + 1)).and_then(|n| BODIES.iter().position(|b| b.0 == n)).unwrap_or(1);
    let base = (build, frames);
    let (build, frames, grow) = body_of(&base.0, &base.1, body);
    let crowd_build = build.clone();
    let getup = getup.map(|mut c| {
        if let Some((len, root)) = &grow {
            c.grow(len, *root);
        }
        c
    });
    let floor = build.parts.iter().zip(&frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
    let shift = room.start - frames[0].translation.with_z(floor);
    let frames: Vec<Transform> = frames.iter().map(|f| Transform { translation: f.translation + shift, ..*f }).collect();
    let mut world = Collision::default();
    let concrete = room.surfaces.iter().position(|s| s.0 == "CONCRETE").unwrap_or(0) as u8;
    let (fp, ft) = world::floor_tris(room.pool, 50.0);
    world.add(&fp, &ft, &vec![concrete; ft.len()]);
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
                        weapons, weapon: 0, weak: false, sel: 0, over_ui: false, held: true,   show_ui: true, sens: 0.5, rec, health: 200.0, god: false, aim_gun: false, gun: Gun::default(), blast_task: None, burning: 0.0, shock: 0.0, charred: vec![], voxels: vec![], on_fire: None, hero_gun: false, passive: true, homer: None, homer_weapon: None, homer_wait: None, homer_rest: 0.0, ammo: 0, reloading: None, taxis: vec![], driving: false, pedals: (0.0, 0.0, false), car_tune: None, lean_stop: None, hurt: [0.0; 4], near_at: -9.0, near_last: -9.0, dived: false, no_dodge: false, dodge_gap: DODGE_GAP, look_stop: None, dodge_at: 0.0, dodging: false, look_after: None, fall: None, hold: None, airborne: false, wet: false, hero_alpha: 0.6, render_scale: 1.0, msaa: 4, msaa_max: 4, shadow_q: 3, frame_ms: 0.0, gpu: String::new(), gpu_now: String::new(), gpus: None, aa: AA_MSAA, smaa_q: 2, dlss_q: 0, fsr_q: 1, fsr_ok: false, xess_q: 1, xess_ok: false, sharpen: 0.0, sharp_by: [0.0; 9], ssao: 0, ao_strength: 1.0, bloom: 0.0, dof: 0.0, contact: 0.0, contact_k: 1.0, vignette: 0.0, lens: 0.0, chroma: 0.0, rt: 0, rt_ok: false, sky: 2, god_rays: false, restir: false, fire: 2, water_q: 1, real_fire: false, voxel_water: false, real_water: false, real_drain: false, pouring: false, real_surface: false, real_ripples: false, water_clear: true, wet_trail: true, water_impact: true, water_faucet: true, real_level: None, flow: vec![], hero_flow: vec![], water_caustics: false, water_spray: false, water_waves: false, water_pixels: false, water_haze: false, water_mirror: false, water_foam: false, water_mist: false, photons: false, water_fine: false, water_depth: true, water_relax: false, water_ghost: false, water_vort: false, water_spray_phys: false, safe: false, fire_light: true, fire_homer: false, shadow_filter: 0, water_wet: false, wet_film: false, wet_drips: false, sky_shade: false, water_traced: false, glass_tint: true, glass_shade: true, water_ripples: true, water_refract: false, flood: None, smoke: true, view_px: UVec2::ONE, defaults: String::new(), upscaler: UP_FSR1, dlss_ok: false, fps_limit: FPS_VSYNC, extra: false, extra_pick: 0xff, fire_react: true, dive: false, hang: None, was_fallen: false, curled: false, super_punch: false, even: false, crush: None, cab_gas: 0.0, cam: None, look_z: 0.8, film: 0.0, photo: false, soft: true, shadows: true, sun_color: [1.0; 3], fill_k: 1.0, gun_reach: 1.0, gun_up: 0.0, gun_off: Vec3::ZERO, gun_aim_off: Vec3::ZERO, crane: None, crane_to: None, shadow: 0.0, strict: false, stairs_rolled: false, cam_head: false, cam_rig: None, spawn_at: None, tear_k: 1.0, sky_k: 1.0, kept_gun: None, cam_floor: f32::MIN, pov: false, film_vel: Vec3::ZERO, photo_pending: false, on_cue: vec![], photo_follow: false, detached: false, cam_at: None, cam2: None, cam2_queue: vec![], cam2_b: None, meat: None, meat_on: true, meat_wipe: false, skate3_bails: false, meat_screen: None, shot_scene: String::new(), photo_note: String::new(), script_walk: Vec3::ZERO, throw_speed: None, homer_guard: None, homer_shot: 0.0, hero_physics: true, hero_doll: None, tackle: None, cuts: vec![], hat: None, dismember: false, jump: None, steam: (0, vec![]), #[cfg(feature = "portal")] portal: portal::Portals::new(), click_at: None,
                        getup, rises: [None, None], still: [0.0; 2], tasks: false, exit: (*SHOT_EXIT()), nm_t: [None, None], failed: [false; 2], last: [Vec3::ZERO; 2], room, look: true, avoid: false, eye: Vec3::new(0.0, -4.0, 1.5), hover: None, map, crowd: vec![], stress: false, group: Default::default(), cars: vec![], lanes: None, pusher: None, pile_fuse: None, ice: None,   click_shoots: false,
                        grab: None, glow: vec![], rings: vec![], ripples: vec![], ripple_wait: vec![], tape: Default::default(), replay: None, throw_at: None, pops: vec![], pop_screen: vec![], show_pops: true, tab: 1, sounds: vec![], sound: true, volume: [1.0, 1.0, 0.35], blood: vec![], pixel_blood: true, gore: false, layers: true, wounds: vec![], show_ripples: false, infinite: false, fx_seed: 1, pointer: None, crowd_hp: vec![], car_hit: vec![], car_reacts: vec![], help: !help_seen(), auto_balance: true, dead: false, hero: None, skate: None, skate_link: skate_files::Link::new(), agility,
                        scene: None, switch_to: None, poke: POKE, game_dir: game_dir.map(str::to_string), walk: true, relock: false, cam_locked: true, walk_dir: Vec3::ZERO, wander: Default::default(), getup_mode: 0, musc: None, learned: None, walk_learned: None, preview_on: false, preview: None, preview_wait: false, preview_keys: (false, false), script_jog: false, watch: None, bench: vec![], homer_id: 0, spawn_body: body, base, crowd_build, body, skin: saved_look(), ped_color: 0, body_alpha: BODY_ALPHA, eyes: true, brows: true, skel: true, skel_style: 0, clean: false, sun_dir: saved_sun() };
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
    let safe = a.iter().any(|x| x == "--safe");
    a.retain(|x| x != "--safe");
    let mut opt = |flag: &str| {
        a.iter().position(|x| x == flag).map(|i| {
            let f = a.get(i + 1).cloned().unwrap_or_default();
            a.drain(i..(i + 2).min(a.len()));
            f
        })
    };
    let headless_steps = opt("--headless").map(|n| n.parse().unwrap_or(300));
    if let Some(p) = opt("--skate-setup") {
        let t = std::time::Instant::now();
        let r = skate_files::convert(std::path::Path::new(&p), &|s| println!("{s}"));
        println!("{r:?} in {:.1} s -> {}", t.elapsed().as_secs_f32(), skate_files::dir().display());
        return;
    }
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
        save_game(&f);
        f
    }).or_else(saved_game);
    if let Some(d) = game_dir.as_deref().and_then(|g| archive(g, "xbox360.rpf", &["data/naturalmotion/fred.xrt"]).pop().flatten()) {
        delirium_data::kv::unlock(&delirium_data::nm_asset::read(&d));
    }
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
    sim.defaults = settings(&sim);
    sim.safe = safe;
    if safe {
        apply_settings(&mut sim, SAFE);
        sim.say(format!("room  Safe Mode: lowest settings, settings.txt left alone ({})", settings(&sim).replace("\n", ", ")));
    } else if sim.script.is_empty() {
        load_settings(&mut sim);
    }
    sim.say(format!("weaponinfo.xml: {} weapons{}", sim.weapons.len(), if sim.weapons.is_empty() { " (--game <dir> reads it; impulses 50 N s)" } else { "" }));

    let systems = (keys, console, viewports, mouse, scripted, step, switch, scene, (apply_look, god_rays, glass_light, fire_view), video, follow_floor, bodies, face_view, props, car_mesh, pusher_mesh, taxi_mesh, gun_view, draw, smooth).chain();
    let mut app = App::new();
    let gpu = std::fs::read_to_string(getup::config_dir().join(SETTINGS)).ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("gpu ").map(|g| g.trim().to_lowercase()))).filter(|g| !g.is_empty());
    #[cfg(feature = "dlss")]
    app.insert_resource(bevy::anti_alias::dlss::DlssProjectId(bevy::asset::uuid::uuid!("5d3f1e0a-7c2b-4e8f-9a61-d311e7a0b4c2")));
    let _ = GPUS.set({
        let mut l: Vec<String> = bevy::tasks::block_on(wgpu::Instance::default().enumerate_adapters(wgpu::Backends::PRIMARY)).into_iter().map(|a| a.get_info())
            .filter(|i| i.device_type != wgpu::DeviceType::Cpu).map(|i| i.name).collect();
        l.sort();
        l.dedup();
        l
    });
    let (plugins, why, full) = renderer(&mut app, gpu, safe);
    if let Some(why) = why {
        sim.say(why);
    }
    app.add_plugins(plugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Delirium Physics Playroom".into(), resolution: (1600, 900).into(), resize_constraints: WindowResizeConstraints { min_width: 960.0, min_height: 640.0, ..default() }, ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(bevy::sprite_render::Material2dPlugin::<ScaledMat>::default())
        .insert_resource(bevy::winit::WinitSettings { focused_mode: bevy::winit::UpdateMode::Continuous, unfocused_mode: bevy::winit::UpdateMode::Continuous })
        .insert_resource(Time::<Virtual>::from_max_delta(std::time::Duration::from_secs_f32(if std::env::var_os("FRAME_LOG").is_some() { 10.0 } else { 0.25 })))
        .insert_resource(ClearColor(Color::srgb(0.78, 0.79, 0.81)))
        .insert_resource(std::env::var("FIXED_FPS").ok().and_then(|f| f.parse::<f64>().ok()).map_or(bevy::time::TimeUpdateStrategy::Automatic,
            |f| bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(1.0 / f))))
        .init_gizmo_group::<OursGizmos>()
        .insert_non_send(sim)
        .add_systems(Startup, setup)
        .add_systems(Update, systems)
        .add_systems(Update, skel_cams.after(smooth))
        .add_systems(Update, started)
        .add_systems(Update, (skate_pad.before(mouse), skate_cam.after(scripted), board_mesh.after(bodies)))
        .add_systems(Update, muscles.after(smooth))
        .add_systems(Update, (sounds, blood_view, glow_view, bulbs, planks, save_settings))
        .add_systems(EguiPrimaryContextPass, ui);
    sfx::plugin(&mut app);
    app.add_plugins((bevy::solari::SolariPlugins, bevy::solari::pathtracer::PathtracingPlugin))
        .insert_resource(bevy::pbr::DefaultOpaqueRendererMethod::forward())
        .add_systems(Update, (ray_tracing.after(scripted).before(video), sky));
    #[cfg(feature = "portal")]
    portal::plugin(&mut app);
    fire::plugin(&mut app);
    water::plugin(&mut app);
    fluid_gpu::plugin(&mut app);
    app.add_systems(Update, (flood::view, fluid_gpu::real_water, sky_pool_shade, fluid_gpu::sky_caustics, fluid_gpu::faucet_shown));
    #[cfg(windows)]
    app.add_systems(Update, window_icon);
    #[cfg(feature = "dlss")]
    app.add_systems(Update, dlss.after(apply_look).before(video));
    #[cfg(windows)]
    app.add_plugins(upscale::UpscalePlugin { on: full }).add_systems(Update, upscale.after(apply_look).before(video));
    #[cfg(all(windows, feature = "dlss"))]
    app.add_systems(Update, upscale.after(dlss));
    app.add_systems(Last, fps_limit);
    app.world().resource::<bevy::asset::io::embedded::EmbeddedAssetRegistry>()
        .insert_asset(std::path::PathBuf::new(), std::path::Path::new("taxi.glb"), include_bytes!("taxi.glb").as_slice());
    app.world().resource::<bevy::asset::io::embedded::EmbeddedAssetRegistry>()
        .insert_asset(std::path::PathBuf::new(), std::path::Path::new("sky.ktx2"), include_bytes!("sky.ktx2").as_slice());
    for (path, bytes) in PBR {
        app.world().resource::<bevy::asset::io::embedded::EmbeddedAssetRegistry>().insert_asset(std::path::PathBuf::new(), std::path::Path::new(path), bytes);
    }
    app.run();
}

const GAME_PATH: &str = "game_path.txt";

fn game_paths() -> Vec<std::path::PathBuf> {
    let user = std::env::var_os("APPDATA").or_else(|| std::env::var_os("HOME")).map(|d| std::path::Path::new(&d).join(".delirium").join(GAME_PATH));
    std::iter::once(GAME_PATH.into()).chain(user).collect()
}

fn save_game(g: &str) {
    for p in game_paths() {
        let _ = std::fs::create_dir_all(p.parent().unwrap_or(std::path::Path::new(".")));
        let _ = std::fs::write(p, g);
    }
}

fn saved_game() -> Option<String> {
    game_paths().iter().filter_map(|p| std::fs::read_to_string(p).ok()).map(|t| t.trim().to_string()).find(|g| is_game(g))
}

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

fn renderer(app: &mut App, gpu: Option<String>, safe: bool) -> (bevy::app::PluginGroupBuilder, Option<String>, bool) {
    let dir = getup::config_dir();
    let was = std::fs::read_to_string(dir.join(FALLBACK)).unwrap_or_default();
    let mut step = match was.lines().next().unwrap_or("").trim() { "plain" => 1, "dx12" => 2, _ => 0 };
    let mut crashes = std::fs::read_to_string(dir.join(STARTING)).map_or(0, |s| s.trim().parse::<u32>().unwrap_or(0) + 1);
    if crashes >= 2 && step < 2 {
        (step, crashes) = (step + 1, 0);
        let _ = std::fs::write(dir.join(FALLBACK), ["", "plain", "dx12"][step]);
    }
    let step = if cfg!(windows) { step.max(safe as usize) } else { 0 };
    let dx12 = step == 2;
    let why = (step > 0).then(|| format!("renderer: {} ({})", ["", "plain Vulkan", "DirectX 12"][step], if safe && step == 1 { "Safe Mode" } else {
        "the driver crashed starting twice; full graphics again with another graphics card or driver"
    }));
    let _ = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(STARTING), crashes.to_string()));
    #[cfg(windows)]
    if step == 0 {
        upscale::vulkan_init(app);
    }
    #[cfg(windows)]
    if dx12 {
        unsafe {
            app.world_mut().get_resource_or_init::<bevy::render::renderer::raw_vulkan_init::RawVulkanInitSettings>()
                .add_create_instance_callback(|args, _| args.extensions.push(c"VK_DELIRIUM_no_vulkan"));
        }
    }
    if dx12 {
        app.add_systems(Update, |q: Query<Entity, Added<Camera3d>>, mut c: Commands| for e in &q {
            c.entity(e).insert(bevy::render::view::NoIndirectDrawing);
        });
    }
    let backends = dx12.then_some(wgpu::Backends::DX12).or(bevy::render::settings::WgpuSettings::default().backends);
    use wgpu::Features as F;
    let disabled_features = match step {
        0 => None,
        1 => Some(F::all() - (F::INDIRECT_FIRST_INSTANCE | F::IMMEDIATES | F::TEXTURE_COMPRESSION_BC | F::FLOAT32_FILTERABLE)),
        _ => Some(F::TEXTURE_BINDING_ARRAY | F::BUFFER_BINDING_ARRAY | F::PARTIALLY_BOUND_BINDING_ARRAY | F::MULTI_DRAW_INDIRECT_COUNT),
    };
    let mut settings = bevy::render::settings::WgpuSettings { adapter_name: gpu, backends, disabled_features, ..default() };
    let dxc = dir.join("dxc").join("bin").join("x64").join("dxcompiler.dll");
    if dx12 && dxc.exists() {
        settings.dx12_shader_compiler = wgpu::Dx12Compiler::DynamicDxc { dxc_path: dxc.display().to_string() };
    }
    let render = bevy::render::RenderPlugin { render_creation: settings.into(), ..default() };
    let plugins = DefaultPlugins.set(render);
    #[cfg(feature = "dlss")]
    let plugins = if step > 0 { plugins.disable::<bevy::anti_alias::dlss::DlssInitPlugin>() } else { plugins };
    (plugins, why, step == 0)
}

fn started(mut n: Local<u32>) {
    *n += 1;
    if *n == STARTED {
        let _ = std::fs::remove_file(getup::config_dir().join(STARTING));
    }
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
        ctx.global_style_mut(|s| {
            s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(18.0));
            s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(18.0));
            s.spacing.item_spacing = egui::vec2(8.0, 12.0);
            s.spacing.button_padding = egui::vec2(14.0, 8.0);
        });
        let copying = pick.copy.as_ref().map(|c| c.lock().unwrap().clone());
        let mut root = egui::Ui::new(ctx.clone(), "root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.content_rect()));
        egui::CentralPanel::default().show(&mut root, |ui| {
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
                save_game(&g);
                let _ = PICKED.set(g);
                exit.write(AppExit::Success);
            } else {
                pick.err = format!("That isn't it: {g}\nDrag your game .iso file (or its game folder).");
            }
        }
        let done = pick.copy.as_ref().and_then(|c| c.lock().unwrap().1.take());
        match done {
            Some(Ok(f)) => {
                save_game(&f);
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
    let mut app = App::new();
    #[cfg(feature = "dlss")]
    app.insert_resource(bevy::anti_alias::dlss::DlssProjectId(bevy::asset::uuid::uuid!("5d3f1e0a-7c2b-4e8f-9a61-d311e7a0b4c2")));
    let plugins = renderer(&mut app, None, false).0;
    app
        .add_plugins(plugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Delirium Physics Playroom".into(), resolution: (900, 460).into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_systems(Update, started)
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
pub struct SkyCam;
#[derive(Component)]
struct ScaledView;
const UP_BASIC: u8 = 0;
const UP_FSR1: u8 = 1;
const UP_NAMES: [&str; 2] = ["basic", "FSR 1"];
const FSR1_SHADER: Handle<Shader> = bevy::asset::uuid_handle!("b6e0c2d4-3f71-4a8e-9c15-7d2a4e9f0b38");
#[derive(Asset, TypePath, bevy::render::render_resource::AsBindGroup, Clone)]
struct ScaledMat {
    #[uniform(0)]
    p: Vec4,
    #[texture(1)]
    #[sampler(2)]
    img: Handle<Image>,
}
impl bevy::sprite_render::Material2d for ScaledMat {
    fn fragment_shader() -> bevy::shader::ShaderRef {
        FSR1_SHADER.into()
    }
}
const RENDER_SCALE_MIN: f32 = 0.25;
const RENDER_SCALE_MAX: f32 = 2.0;
#[derive(Component)]
struct FacePart(usize, usize);

fn face_view(sim: NonSend<Sim>, time: Res<Time>, mut q: Query<(&FacePart, &mut Transform, &mut Visibility)>) {
    let faces: Vec<Option<[Transform; 6]>> = (0..=sim.bench.len()).map(|h| face(&sim, h, time.elapsed_secs() + h as f32 * 1.7)).collect();
    for (f, mut tr, mut vis) in &mut q {
        match faces.get(f.0).copied().flatten() {
            Some(t) => (*tr, *vis) = (t[f.1], Visibility::Visible),
            None => *vis = Visibility::Hidden,
        }
    }
}

fn face(sim: &Sim, ped: usize, t: f32) -> Option<[Transform; 6]> {
    if !sim.eyes || sim.watch.is_some() {
        return None;
    }
    let clip = |r: Option<&getup::Rise>| sim.getup.as_ref().zip(r).and_then(|(c, r)| c.frames(r));
    let (links, caps, frames, doll, dead, rising): (Vec<Transform>, Vec<Transform>, &[Transform], _, _, _) = match ped {
        0 => (sim.shown().map(|f| f.links.clone()).or_else(|| clip(sim.rises[0].as_ref())).unwrap_or_else(|| sim.agent.doll.frames().collect()),
              sim.shown().map(|f| f.caps.clone()).unwrap_or_else(|| sim.agent.doll.capsules().collect()), &sim.frames, &sim.agent.doll, sim.dead, sim.rises[0].is_some()),
        _ => {
            let (b, d) = (sim.bench.get(ped - 1)?, &sim.crowd.get(ped - 1)?.0.doll);
            (clip(b.rise.as_ref()).unwrap_or_else(|| d.frames().collect()), d.capsules().collect(), &b.frames, d, b.dead, b.rise.is_some())
        }
    };
    let (h, rest, part) = (links.get(HEAD)?, frames.get(HEAD)?, sim.build_of(ped).parts.get(HEAD)?);
    let stand = Side::new(frames.iter().map(|f| f.translation).collect());
    let link = delirium::nmdbm::rage_link(nm(), HEAD);
    let (cap, cap0) = (h.rotation * link, rest.rotation * link);
    let axis = if (cap0 * Vec3::Y).z < 0.0 { -Vec3::Y } else { Vec3::Y };
    let ahead = cap0.inverse() * stand.fwd;
    let ahead = (ahead - axis * ahead.dot(axis)).normalize_or(Vec3::Z);
    let (fwd, up) = (cap * ahead, cap * axis);
    let left = up.cross(fwd).normalize_or(Vec3::X);
    let centre = caps.get(HEAD).filter(|_| !rising).map_or(h.translation, |c| c.translation);
    let r = part.radius;
    let v = doll.com().1;
    let speed = v.length();
    let fear = ((speed - 1.0) / 2.0 + (-v.z - 0.5).max(0.0) * 0.6).clamp(0.0, 1.0);
    let hit = sim.rings.iter().filter(|r| r.0 == ped).min_by(|a, b| a.4.total_cmp(&b.4));
    let glow = sim.glow.get(ped).map_or(0.0, |g| g.iter().map(|x| x.0).fold(0.0, f32::max));
    let pain = (hit.map_or(0.0, |r| 1.0 - r.4 / 1.5) .max(glow.min(1.0))).clamp(0.0, 1.0);
    let worry = ((speed - 0.3) / 0.7).clamp(0.0, 1.0) * (1.0 - fear);
    let dazed = if h.translation.z < 0.5 && speed < 0.3 { 1.0 } else { 0.0 };
    let upset = fear.max(worry).max(pain);
    let blink = (t + 0.7 * (t / 4.0).floor().sin()) % 4.0 < 0.12
        || hit.is_some_and(|r| r.4 < 0.3) || glow > 0.7;
    let open = if dead || blink { 0.08 } else { (1.0 + 0.3 * fear) * (1.0 - 0.8 * pain.max(0.6 * dazed)) };
    let look = hit.and_then(|r| links.get(r.1).map(|f| f.transform_point(r.2))).map(|p| (p - centre).normalize_or(fwd))
        .or_else(|| (speed > 1.5).then(|| v / speed)).unwrap_or(fwd);
    let gaze = (look - fwd * look.dot(fwd)).clamp_length_max(1.0) * 0.12;
    Some(std::array::from_fn(|i| {
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let n = (fwd + up * 0.1 + left * side * 0.3 + gaze).normalize();
        let el = up.cross(n).normalize_or(left);
        let basis = Quat::from_mat3(&Mat3::from_cols(el, n.cross(el), n));
        let eye = centre + n * r;
        let (w, h) = (0.12 * r, 0.19 * r * open);
        let brow = sim.brows && upset > 0.2;
        match i {
            0 | 1 => Transform::from_translation(eye + n * 0.05 * r + n.cross(el) * h * 0.4 - el * w * 0.35).with_rotation(basis).with_scale(Vec3::splat(0.035 * r * (open > 0.5) as u8 as f32)),
            2 | 3 => Transform::from_translation(eye).with_rotation(basis).with_scale(Vec3::new(w, h, 0.06 * r)),
            _ => {
                let roll = Quat::from_axis_angle(n, -side * 0.45 * upset);
                let at = centre + (fwd + up * (0.42 + 0.06 * upset) + left * side * 0.3).normalize() * r;
                Transform::from_translation(at).with_rotation(roll * basis).with_scale(Vec3::new(0.24 * r, 0.035 * r, 0.03 * r) * brow as u8 as f32)
            }
        }
    }))
}

#[derive(Component)]
struct BoneMesh(usize, usize);

#[derive(Component)]
struct BrokeBone(usize);

#[derive(Component)]
struct Xray(Handle<Mesh>, Option<Handle<Mesh>>, Handle<Mesh>, Option<Handle<Mesh>>);

#[derive(Component)]
struct VoxelOf(Handle<Mesh>, Handle<Mesh>, Handle<Image>);

#[derive(Component)]
struct AuraMesh;

const BODY_RHO: f32 = 960.0;
const HERO_MASS: f32 = 80.0;
const HERO_GRIP: f32 = 3.0;

#[derive(Component)]
struct PartMesh {
    k: usize,
    crowd: Option<usize>,
    watch: Option<usize>,
}

fn ped_build<'a>(build: &'a Build, bench: &'a [Benched], crowd: &'a Build, ped: usize) -> &'a Build {
    match ped.checked_sub(1) {
        None => build,
        Some(i) => bench.get(i).map_or(crowd, |b| &b.build),
    }
}

impl PartMesh {
    fn homer(&self) -> Option<usize> {
        match (self.watch, self.crowd) {
            (None, None) => Some(0),
            (None, Some(c)) => (c + 1 < HOMERS).then_some(c + 1),
            _ => None,
        }
    }
}
#[derive(Default, Reflect, GizmoConfigGroup)]
struct OursGizmos;
const SKEL_STYLES: [&str; 3] = ["Lines", "Muscles", "Stick"];
const SKEL_LAYER: usize = 29;
#[derive(Component)]
struct SkelCam(Entity);

fn skel_cams(mut skel: Query<(&SkelCam, &mut Camera, &mut Transform, &mut Projection, &mut bevy::camera::RenderTarget)>,
             room: Query<(&Camera, &Transform, &Projection, Ref<bevy::camera::RenderTarget>), Without<SkelCam>>) {
    for (of, mut cam, mut at, mut proj, mut target) in &mut skel {
        let Ok((c, t, p, to)) = room.get(of.0) else { continue };
        (cam.is_active, cam.viewport) = (c.is_active, c.viewport.clone());
        at.set_if_neq(*t);
        *proj = p.clone();
        if to.is_changed() {
            *target = (*to).clone();
        }
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, sim: NonSend<Sim>,
         mut scaled: ResMut<Assets<ScaledMat>>, mut shaders: ResMut<Assets<Shader>>,
         mut gizmos: ResMut<GizmoConfigStore>, mut egui_settings: ResMut<bevy_egui::EguiGlobalSettings>, assets: Res<AssetServer>) {
    egui_settings.auto_create_primary_context = false;
    use bevy::render::render_resource::{BlendState, TextureUsages};
    let ours = commands.spawn((Camera3d::default(), bevy::pbr::ScreenSpaceTransmission::default(), Camera { order: 0, ..default() }, RenderLayers::from_layers(&[0, 1]), Transform::default(),
                    Orbit { yaw: -1.3, pitch: 0.3, dist: 3.5, target: Vec3::new(0.0, 0.0, 0.8), follow: 0 }, OursCam)).id();
    for (order, layer, of) in [(2, SKEL_LAYER, ours), ] {
        commands.spawn((Camera3d::default(), bevy::pbr::ScreenSpaceTransmission::default(), Camera { order, clear_color: ClearColorConfig::Custom(Color::NONE),
                                                      output_mode: bevy::camera::CameraOutputMode::Write { blend_state: Some(BlendState::ALPHA_BLENDING), clear_color: ClearColorConfig::None },
                                                      ..default() }, RenderLayers::layer(layer), Transform::default(), Msaa::Off, SkelCam(of),
                        bevy::camera::CameraMainTextureUsages(bevy::camera::CameraMainTextureUsages::default().0 | TextureUsages::COPY_DST)));
    }
    commands.spawn((Camera2d, Camera { order: 4, clear_color: ClearColorConfig::Custom(Color::NONE),
                                       output_mode: bevy::camera::CameraOutputMode::Write { blend_state: Some(BlendState::ALPHA_BLENDING), clear_color: ClearColorConfig::None },
                                       ..default() }, RenderLayers::layer(31), bevy_egui::PrimaryEguiContext, Msaa::Off,
                    bevy::camera::CameraMainTextureUsages(bevy::camera::CameraMainTextureUsages::default().0 | TextureUsages::COPY_SRC)));
    let _ = shaders.insert(&FSR1_SHADER, Shader::from_wesl(include_str!("fsr1.wesl"), "fsr1.wesl"));
    commands.spawn((Mesh2d(meshes.add(Rectangle::new(1.0, 1.0))), MeshMaterial2d(scaled.add(ScaledMat { p: Vec4::ZERO, img: Handle::default() })),
                    Transform::default(), Visibility::Hidden, RenderLayers::layer(31), ScaledView));
    for layer in [1, ] {
        commands.spawn((DirectionalLight { shadow_maps_enabled: true, ..default() }, Transform::from_xyz(3.0, -2.0, 6.0).looking_at(Vec3::ZERO, Vec3::Z),
                        RenderLayers::layer(layer)));
        commands.spawn((DirectionalLight { shadow_maps_enabled: false, illuminance: 0.0, ..default() }, Transform::from_xyz(-3.0, 4.0, 2.0).looking_at(Vec3::ZERO, Vec3::Z),
                        RenderLayers::layer(layer), Fill));
    }
    commands.spawn((bevy::light::FogVolume { density_factor: GOD_RAYS_FOG, absorption: 0.0, ..default() }, Transform::from_scale(GOD_RAYS_BOX), Visibility::Hidden,
                    RenderLayers::layer(1)));
    spawn_world(&mut commands, &mut meshes, &mut mats, &sim, &assets);
    let (cfg, _) = gizmos.config_mut::<OursGizmos>();
    (cfg.render_layers, cfg.depth_bias) = (RenderLayers::layer(SKEL_LAYER), -1.0);
}

pub(crate) fn pbr_tex(assets: &AssetServer, name: String, sharp: bool) -> Handle<Image> {
    use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
    assets.load_builder().with_settings(move |l: &mut ImageLoaderSettings| {
        let d = ImageSamplerDescriptor { address_mode_u: ImageAddressMode::Repeat, address_mode_v: ImageAddressMode::Repeat, ..ImageSamplerDescriptor::linear() };
        l.sampler = ImageSampler::Descriptor(match sharp {
            true => ImageSamplerDescriptor { mag_filter: bevy::image::ImageFilterMode::Nearest, ..d },
            false => ImageSamplerDescriptor { anisotropy_clamp: 16, ..d },
        });
    }).load(format!("embedded://pbr/{name}.ktx2"))
}

fn spawn_world(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, sim: &Sim, assets: &AssetServer) {
    let floor = if sim.room.stage == "street" || sim.room.stage == "cab" { Color::srgb(0.2, 0.2, 0.22) } else { Color::srgb(0.58, 0.59, 0.61) };
    let mut mesh = Mesh::from(Plane3d::new(Vec3::Z, Vec2::splat(30.0)));
    if sim.room.pool {
        let plane = |(lo, hi): (Vec2, Vec2)| Mesh::from(Plane3d::new(Vec3::Z, (hi - lo) * 0.5)).translated_by(((lo + hi) * 0.5).extend(0.0));
        let r = world::floor_rects(true, 3000.0);
        mesh = plane(r[0]);
        for &x in &r[1..] {
            let _ = mesh.merge(&plane(x));
        }
        let [a, b] = (*world::POOL());
        commands.spawn((Mesh3d(meshes.add(water::surface(b - a))),
                        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgba(0.12, 0.42, 0.62, 0.55), alpha_mode: AlphaMode::Blend, ior: 1.33, thickness: 0.0,
                                                                   attenuation_color: Color::linear_rgb(0.325, 0.741, 0.819), attenuation_distance: 2.5,
                                                                   perceptual_roughness: 0.02, reflectance: 0.5, ..default() })),
                        Transform::from_translation(((a + b) * 0.5).extend(world::WATER())), RenderLayers::layer(1), RoomScenery, Water, bevy::light::NotShadowCaster));
    }
    commands.spawn((Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(mats.add(StandardMaterial { base_color: floor, perceptual_roughness: 1.0, ..default() })), RoomScenery, Floor));
    use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
    let tex = |name: String, _srgb: bool, sharp: bool| pbr_tex(assets, name, sharp);
    let glass: Vec<_> = GLASS.iter().filter(|_| sim.room.stage == "pttest").map(|c| mats.add(StandardMaterial { base_color_texture: Some(tex(format!("{c}_stained_glass"), true, true)), alpha_mode: AlphaMode::Blend,
                                                                           specular_transmission: 1.0, ior: 1.5, perceptual_roughness: 0.05, ..default() })).collect();
    let cube = meshes.add(Cuboid::from_size(Vec3::splat(world::GLASS_HALF() * 2.0)));
    if sim.room.stage == "pttest" {
        for (k, name) in ["cobblestone", "ice", "grass"].into_iter().enumerate() {
            let pbr = name != "grass";
            let mut mesh = Mesh::from(Plane3d::new(Vec3::Z, Vec2::new(1.0, 5.0)));
            if let Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
                uv.iter_mut().for_each(|u| *u = [u[0] * 2.0, u[1] * 10.0]);
            }
            let _ = mesh.generate_tangents();
            let mat = StandardMaterial { base_color_texture: Some(tex(name.into(), true, !pbr)), normal_map_texture: pbr.then(|| tex(format!("{name}_n"), false, false)),
                                         metallic_roughness_texture: pbr.then(|| tex(format!("{name}_mr"), false, false)), perceptual_roughness: 1.0, metallic: 1.0, ..default() };
            commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(mat)), Transform::from_xyz(-2.2 + 2.2 * k as f32, -6.0, 0.002),
                            RenderLayers::layer(1), RoomScenery));
        }
        let n = 32;
        let wire: Vec<u8> = (0..n * n).flat_map(|i: i32| {
            let (x, y) = (i % n, i / n);
            if (x + y).rem_euclid(n / 2) < 2 || (x - y).rem_euclid(n / 2) < 2 { [150, 155, 160, 255] } else { [0; 4] }
        }).collect();
        let mut img = Image::new(bevy::render::render_resource::Extent3d { width: n as u32, height: n as u32, depth_or_array_layers: 1 },
                                 bevy::render::render_resource::TextureDimension::D2, wire, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                                 RenderAssetUsages::default());
        img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: ImageAddressMode::Repeat, address_mode_v: ImageAddressMode::Repeat,
                                                                        ..ImageSamplerDescriptor::nearest() });
        let mut mesh = Mesh::from(Plane3d::new(Vec3::Z, Vec2::new(2.0, 1.0)));
        if let Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
            uv.iter_mut().for_each(|u| *u = [u[0] * 16.0, u[1] * 8.0]);
        }
        let _ = mesh.generate_tangents();
        let fence = StandardMaterial { base_color_texture: Some(assets.add(img)), alpha_mode: AlphaMode::Mask(0.5), double_sided: true, cull_mode: None,
                                       metallic: 0.8, perceptual_roughness: 0.4, ..default() };
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(fence)), Transform::from_xyz(3.0, -1.0, 1.0).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                        RenderLayers::layer(1), RoomScenery));
        commands.spawn((fluid_gpu::faucet(meshes, mats, assets), RenderLayers::layer(1), RoomScenery));
        commands.spawn((PointLight { color: Color::srgb(1.0, 0.7, 0.4), intensity: 150_000.0, range: 10.0, shadow_maps_enabled: true, ..default() },
                        Transform::from_xyz(1.5, 1.5, 0.4), RenderLayers::layer(1), RoomScenery));
        let (at, half, deep) = (SKY_POOL, SKY_HALF, SKY_DEEP);
        let pane = mats.add(StandardMaterial { base_color: Color::srgba(0.9, 0.97, 1.0, 0.2), alpha_mode: AlphaMode::Blend, specular_transmission: 1.0, ior: 1.5,
                                               perceptual_roughness: 0.02, ..default() });
        let water = mats.add(StandardMaterial { base_color: Color::srgba(0.9, 0.97, 1.0, 0.35), specular_transmission: 1.0, ior: 1.33,
                                                attenuation_color: Color::linear_rgb(0.86, 0.96, 0.97), attenuation_distance: 20.0, perceptual_roughness: 0.02,
                                                reflectance: 0.5, ..default() });
        let g = 0.02;
        let n = Vec2::new(((half.x + g) / 0.5).round(), ((half.y + g) * 2.0 / 0.5).round());
        let tile = Vec2::new((half.x + g) / n.x, (half.y + g) * 2.0 / n.y);
        let tile_mesh = meshes.add(Cuboid::new(tile.x, tile.y, g));
        for (i, j) in (0..n.x as usize).flat_map(|i| (0..n.y as usize).map(move |j| (i, j))) {
            let c = Vec3::new((i as f32 + 0.5) * tile.x, -half.y - g + (j as f32 + 0.5) * tile.y, -g * 0.5);
            commands.spawn((Mesh3d(tile_mesh.clone()), MeshMaterial3d(glass[i % 16].clone()), Transform::from_translation(at + c), RenderLayers::layer(1),
                            RoomScenery, Glass, Clear, bevy::light::NotShadowCaster));
        }
        let hx = (half.x + g) * 0.5;
        for (c, h) in [(Vec3::new(-hx, 0.0, -g * 0.5), Vec3::new(hx, half.y + g, g * 0.5)),
                       (Vec3::new(half.x + g * 0.5, 0.0, deep * 0.6), Vec3::new(g * 0.5, half.y + g, deep * 0.6)),
                       (Vec3::new(-half.x - g * 0.5, 0.0, deep * 0.6), Vec3::new(g * 0.5, half.y + g, deep * 0.6)),
                       (Vec3::new(0.0, half.y + g * 0.5, deep * 0.6), Vec3::new(half.x, g * 0.5, deep * 0.6)),
                       (Vec3::new(0.0, -half.y - g * 0.5, deep * 0.6), Vec3::new(half.x, g * 0.5, deep * 0.6))] {
            commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(h * 2.0))), MeshMaterial3d(pane.clone()), Transform::from_translation(at + c), RenderLayers::layer(1),
                            RoomScenery, Clear, bevy::light::NotShadowCaster));
        }
        let inner = half.extend(deep * 0.5) - Vec3::splat(0.003);
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(inner * 2.0))), MeshMaterial3d(water), Transform::from_translation(at + Vec3::Z * deep * 0.5),
                        RenderLayers::layer(1), RoomScenery, Clear, bevy::light::NotShadowCaster, SkyWater));
    }
    if sim.room.stage == "lights" {
        let k = 1.5 / bevy::camera::Exposure::default().exposure();
        let panel = meshes.add(Plane3d::new(Vec3::Z, Vec2::new(2.5, 1.5)));
        for (i, c) in [[1.0, 0.08, 0.05], [1.0, 0.4, 0.03], [1.0, 0.85, 0.05], [0.1, 1.0, 0.15], [0.05, 0.9, 1.0], [0.08, 0.2, 1.0], [0.6, 0.1, 1.0]].into_iter().enumerate() {
            let at = Vec3::new(0.0, (*world::HALL())[1].y - 2.0 - 3.0 * i as f32, 0.004);
            let col = LinearRgba::rgb(c[0], c[1], c[2]);
            commands.spawn((Mesh3d(panel.clone()), MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::BLACK, emissive: col * k, emissive_exposure_weight: 1.0, reflectance: 0.0, ..default() })),
                            Transform::from_translation(at), RenderLayers::layer(1), RoomScenery, bevy::light::NotShadowCaster));
            commands.spawn((PointLight { color: col.into(), intensity: 200_000.0, range: 12.0, ..default() }, Transform::from_translation(at + Vec3::Z * 0.3),
                            RenderLayers::layer(1), RoomScenery, StandIn));
        }
    }
    for &(c, h, row) in &sim.room.blocks {
        let brick = sim.room.surfaces.get(row as usize).is_some_and(|s| s.0 == "BRICK_WALL");
        let post = sim.room.stage == "bridge" && h == (*world::POST_HALF());
        if sim.room.stage == "pttest" && c.z > SKY_POOL.z - 1.0 {
            continue;
        }
        if sim.room.stage == "pttest" && h == Vec3::splat(world::GLASS_HALF()) {
            let i = ((c.y + 4.0) / 0.5) as usize;
            commands.spawn((Mesh3d(cube.clone()), MeshMaterial3d(glass[i % 16].clone()), Transform::from_translation(c), RenderLayers::layer(1), RoomScenery,
                            Glass, Clear, bevy::light::NotShadowCaster));
            continue;
        }
        let color = if sim.room.stage == "trampoline" { Color::srgb(0.2, 0.3, 0.75) } else if sim.room.stage == "lights" { Color::srgb(0.9, 0.9, 0.88) } else if post { Color::srgb(0.24, 0.25, 0.27) } else if h.z < 0.01 { Color::srgb(0.92, 0.92, 0.9) } else if c.z + h.z < 0.0 { Color::srgb(0.9, 0.92, 0.93) } else if brick { Color::srgb(0.8, 0.42, 0.33) } else { Color::srgb(0.68, 0.68, 0.7) };
        let (mesh, rot) = match post {
            true => (meshes.add(Cylinder::new(h.x * 1.5, h.z * 2.0)), Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            false => (meshes.add(Cuboid::from_size(h * 2.0)), Quat::IDENTITY),
        };
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mats.add(StandardMaterial { base_color: color, perceptual_roughness: 0.9, ..default() })),
                        Transform::from_translation(c).with_rotation(rot), RenderLayers::layer(1), RoomScenery));
    }
    let curves = if sim.room.stage == "skatepark" { world::QUARTER_SLABS() * (*world::QUARTERS()).len() } else { 0 };
    for i in (0..(*world::QUARTERS()).len()).filter(|_| curves > 0) {
        commands.spawn((Mesh3d(meshes.add(world::quarter_mesh(i))), MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.72, 0.71, 0.68), perceptual_roughness: 0.85, double_sided: true, cull_mode: None, ..default() })),
                        Transform::default(), RenderLayers::layer(1), RoomScenery));
    }
    for &(t, h, row) in sim.room.tilted.iter().skip(curves) {
        let metal = sim.room.surfaces.get(row as usize).is_some_and(|s| s.0.starts_with("METAL"));
        let (color, rough, metallic) = if metal { (Color::srgb(0.6, 0.62, 0.65), 0.35, 1.0) } else { (Color::srgb(0.72, 0.71, 0.68), 0.85, 0.0) };
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(h * 2.0))), MeshMaterial3d(mats.add(StandardMaterial { base_color: color, perceptual_roughness: rough, metallic, ..default() })),
                        t, RenderLayers::layer(1), RoomScenery));
    }
    if world::plat(&sim.room.stage, 0.0).is_some() {
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size((*world::PLAT_HALF()) * 2.0))), MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.5, 0.33, 0.2), perceptual_roughness: 0.9, ..default() })),
                        Transform::default(), RenderLayers::layer(1), RoomScenery, Plat));
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
        for k in 0..world::ROPES() {
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
    let homer = sides.into_iter().map(|(g, l)| (g, l, None, None)).chain((0..49).map(|c| (false, 1, Some(c), None)))
        .chain((0..learn::LIVE).map(|w| (false, 1, None, Some(w))));
    let bone = mats.add(StandardMaterial { base_color: Color::srgb(0.95, 0.97, 1.0), emissive: BONE_GLOW, unlit: true, depth_bias: BONE_BIAS, ..default() });
    let socket = mats.add(StandardMaterial { base_color: Color::BLACK, unlit: true, depth_bias: BONE_BIAS, ..default() });
    let red = mats.add(StandardMaterial { base_color: Color::srgb(1.0, 0.1, 0.05), emissive: (*MEAT_RED()), unlit: true, depth_bias: BONE_BIAS, ..default() });
    let voxel = StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.7, ..default() };
    let bone_in = mats.add(StandardMaterial { base_color: BONE_COLOR, perceptual_roughness: 0.6, ..default() });
    let aura = mats.add(StandardMaterial { base_color: AURA_GLOW, unlit: true, alpha_mode: AlphaMode::Add,
                                           cull_mode: Some(bevy::render::render_resource::Face::Front), ..default() });
    for (game, layer, crowd, watch) in homer {
        for (k, p) in sim.build.parts.iter().enumerate() {
            let name = delirium_data::nm_asset::PARTS.get(k).copied().unwrap_or("");
            let c = part_color(0, name, BODY_ALPHA);
            let mesh = part_mesh(meshes, p);
            let e = commands.spawn((Mesh3d(mesh.clone()),
                                    MeshMaterial3d(mats.add(StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.6, ..default() })),
                                    Transform::default(), Visibility::Hidden, RenderLayers::layer(layer), PartMesh { k,  crowd, watch })).id();
            if !game && crowd.is_none_or(|c| c + 1 < HOMERS) && watch.is_none() {
                let ped = crowd.map_or(0, |c| c + 1);
                let (bones, sockets) = xray_bones(name, p);
                let bones = meshes.add(bones);
                commands.entity(e).with_child((Mesh3d(bones.clone()), MeshMaterial3d(bone.clone()), Transform::default(), Visibility::Hidden,
                                               RenderLayers::layer(layer), BoneMesh(ped, k)));
                let sockets = sockets.map(|m| {
                    let m = meshes.add(m);
                    commands.entity(e).with_child((Mesh3d(m.clone()), MeshMaterial3d(socket.clone()), Transform::default(), Visibility::Hidden,
                                                   RenderLayers::layer(layer), BoneMesh(ped, k)));
                    m
                });
                let (flesh, inner, palette) = (meshes.add(speck()), meshes.add(speck()), assets.add(voxel_palette([255; 4])));
                let voxel = mats.add(StandardMaterial { base_color_texture: Some(palette.clone()), ..voxel.clone() });
                commands.entity(e).insert(VoxelOf(flesh.clone(), inner.clone(), palette))
                    .with_child((Mesh3d(flesh), MeshMaterial3d(voxel), Transform::default(), RenderLayers::layer(layer),
                                 bevy::light::NotShadowCaster))
                    .with_child((Mesh3d(inner), MeshMaterial3d(bone_in.clone()), Transform::default(), RenderLayers::layer(layer)));
                let glow = part_mesh(meshes, p);
                commands.entity(e).with_child((Mesh3d(glow.clone()), MeshMaterial3d(aura.clone()), Transform::from_scale(Vec3::splat(1.25)), Visibility::Hidden,
                                               RenderLayers::layer(layer), BoneMesh(ped, k), AuraMesh));
                let snapped = (ped == 0).then(|| {
                    let m = meshes.add(broken_bone(name, p));
                    commands.entity(e).with_child((Mesh3d(m.clone()), MeshMaterial3d(red.clone()), Transform::default(), Visibility::Hidden,
                                                   RenderLayers::layer(layer), BrokeBone(k)));
                    m
                });
                commands.entity(e).insert(Xray(bones, sockets, glow, snapped));
            }
        }
    }
    let (white, black) = (mats.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() }),
                          mats.add(StandardMaterial { base_color: Color::srgb(0.05, 0.05, 0.07), unlit: true, ..default() }));
    let (ball, bar) = (meshes.add(Sphere::new(1.0)), meshes.add(Cuboid::from_size(Vec3::ONE)));
    for (h, i) in (0..HOMERS).flat_map(|h| (0..6).map(move |i| (h, i))) {
        let (mesh, mat) = if i < 2 { (ball.clone(), white.clone()) } else if i < 4 { (ball.clone(), black.clone()) } else { (bar.clone(), black.clone()) };
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), Visibility::Hidden, RenderLayers::layer(1), FacePart(h, i)));
    }
    let (hat, felt) = (meshes.add(Cylinder::new(sever::HAT.0, sever::HAT.1)), mats.add(StandardMaterial { base_color: Color::srgb(0.25, 0.17, 0.1), perceptual_roughness: 0.8, ..default() }));
    for h in 0..HOMERS {
        commands.spawn((Mesh3d(hat.clone()), MeshMaterial3d(felt.clone()), Transform::default(), Visibility::Hidden, RenderLayers::layer(1), HatMesh(h)));
    }
    for (k, p) in sim.getup.as_ref().and_then(|c| c.hero.as_ref()).map_or(&[][..], |r| &r.build.parts[..]).iter().enumerate() {
        commands.spawn((Mesh3d(part_mesh(meshes, p)),
                        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgba(0.3, 0.75, 0.4, 0.6), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.6, ..default() })),
                        Transform::default(), Visibility::Hidden, RenderLayers::layer(1), HeroMesh(k)));
    }
    let (wheel, metal, wood) = (meshes.add(Cylinder::new(0.027, 0.034)), mats.add(StandardMaterial { base_color: Color::srgb(0.6, 0.6, 0.62), metallic: 0.8, ..default() }),
                                mats.add(StandardMaterial { base_color: Color::srgb(0.12, 0.1, 0.09), perceptual_roughness: 0.7, ..default() }));
    for k in 0..7 {
        let (mesh, mat) = match k { 0..4 => (wheel.clone(), mats.add(StandardMaterial { base_color: Color::srgb(0.9, 0.88, 0.8), ..default() })),
                                    4 | 5 => (bar.clone(), metal.clone()), _ => (bar.clone(), wood.clone()) };
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), Visibility::Hidden, RenderLayers::layer(1), BoardMesh(k)));
    }
}

#[derive(Component)]
pub struct BoardMesh(usize);

fn board_mesh(mut sim: NonSendMut<Sim>, time: Res<Time>, mut q: Query<(&BoardMesh, &mut Transform, &mut Visibility)>) {
    let dt = if sim.paused { 0.0 } else { time.delta_secs() * sim.speed };
    sim.skate_idle(dt);
    let b: Vec<Vec3> = sim.skate.as_ref().map_or(vec![], |s| s.board().iter().map(|m| m.w_axis.truncate()).collect());
    let turn = skate::frame(&b);
    for (m, mut t, mut v) in &mut q {
        let (Some(&at), Some(r)) = (b.get(m.0), turn) else {
            *v = Visibility::Hidden;
            continue;
        };
        *v = Visibility::Visible;
        let base = b[4].distance(b[5]);
        let scale = match m.0 { 0..4 => Vec3::ONE, 4 | 5 => Vec3::new(0.05, 0.17, 0.035), _ => Vec3::new(base * 2.1, 0.21, 0.014) };
        *t = Transform { translation: at, rotation: r, scale };
    }
}

fn skate_pad(mut sim: NonSendMut<Sim>, pads: Query<&bevy::input::gamepad::Gamepad>, windows: Query<&Window>) {
    use bevy::input::gamepad::GamepadButton as B;
    let Some(s) = sim.skate.as_mut() else { return };
    let focus = windows.iter().any(|w| w.focused);
    s.pad = pads.iter().next().filter(|_| focus).map(|pad| {
        const BITS: [(B, u16); 14] = [(B::DPadUp, 0x1), (B::DPadDown, 0x2), (B::DPadLeft, 0x4), (B::DPadRight, 0x8), (B::Start, 0x10), (B::Select, 0x20),
            (B::LeftThumb, 0x40), (B::RightThumb, 0x80), (B::LeftTrigger, 0x100), (B::RightTrigger, 0x200), (B::South, 0x1000), (B::East, 0x2000),
            (B::West, 0x4000), (B::North, 0x8000)];
        let trigger = |b: B| (pad.get(b).unwrap_or(pad.pressed(b) as u8 as f32).clamp(0.0, 1.0) * 255.0) as u8;
        let axis = |v: f32| (v.clamp(-1.0, 1.0) * 32767.0) as i16;
        let dead = |v: Vec2| if v.length() < 0.2 { Vec2::ZERO } else { v };
        let (l, r) = (dead(pad.left_stick()), dead(pad.right_stick()));
        skate_host::bridge::Controls { buttons: BITS.iter().filter(|b| pad.pressed(b.0)).fold(0, |a, b| a | b.1),
            triggers: [trigger(B::LeftTrigger2), trigger(B::RightTrigger2)], left: [axis(l.x), axis(l.y)], right: [axis(r.x), axis(r.y)] }
    });
}

fn skin(k: usize, c: f32, body: [u8; 4]) -> (Image, Image) {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let hash = |i: usize, salt: f32| (((k * 977 + i) as f32 * 12.9898 + salt).sin() * 43758.545).rem_euclid(1.0);
    let (mut base, mut glow) = (vec![], vec![]);
    for i in 0..CHAR_PX * CHAR_PX {
        let clump = hash(20000 + i / CHAR_PX / 4 * 8 + i % CHAR_PX / 4, 0.0);
        let burnt = c * 1.25 > 0.6 * clump + 0.4 * hash(i, 0.0);
        let r = hash(i, 7.3);
        let under: [f32; 3] = body.map(|x| x as f32)[..3].try_into().unwrap();
        let dark = 1.0 - 0.5 * c.min(1.0);
        let px = match () {
            _ if !burnt => under.map(|x| (x * dark) as u8),
            _ => match () {
                _ if r < 0.22 => [20.0, 12.0, 12.0],
                _ if r < 0.44 => [160.0, 14.0, 20.0],
                _ if r < 0.6 => [105.0, 10.0, 16.0],
                _ if r < 0.76 => [100.0, 32.0, 92.0],
                _ if r < 0.9 => [140.0, 30.0, 75.0],
                _ => [88.0, 76.0, 80.0],
            }.map(|x: f32| (x * (1.0 - 0.85 * ((c - 1.0) / (CHAR_MAX - 1.0)).clamp(0.0, 1.0))) as u8),
        };
        base.extend([px[0], px[1], px[2], 255]);
        glow.extend(if burnt && hash(i, 3.1) < 0.25 { [255; 4] } else { [0, 0, 0, 255] });
    }
    let image = |data| {
        let mut i = Image::new(Extent3d { width: CHAR_PX as u32, height: CHAR_PX as u32, depth_or_array_layers: 1 }, TextureDimension::D2, data,
                               TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
        i.sampler = bevy::image::ImageSampler::nearest();
        i
    };
    (image(base), image(glow))
}

fn xray_bones(name: &str, p: &delirium::ragdoll::PartDef) -> (Mesh, Option<Mesh>) {
    let (r, h) = (p.radius, p.half);
    let up = (p.bind.inverse() * Vec3::Z).normalize();
    let fwd = -(p.bind.inverse() * Vec3::Y).normalize();
    let side = up.cross(fwd).normalize();
    let fwd = side.cross(up);
    let ball = |c: Vec3, rad: f32| Mesh::from(Sphere::new(rad)).transformed_by(Transform::from_translation(c));
    let rod = |a: Vec3, b: Vec3, rad: f32| Mesh::from(Cylinder::new(rad, (b - a).length()))
        .transformed_by(Transform::from_translation((a + b) / 2.0).with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize())));
    let ring = |c: Vec3, a: f32, b: f32| Mesh::from(Torus::new(0.9, 1.1))
        .transformed_by(Transform { translation: c, rotation: Quat::from_mat3(&Mat3::from_cols(side, up, side.cross(up))), scale: Vec3::new(a, a.min(b), b) });
    let long = |rad: f32| {
        let k = rad * 1.7;
        let mut v = vec![rod(-h * Vec3::Y, h * Vec3::Y, rad)];
        for y in [-h, h] {
            v.extend([ball(Vec3::new(k * 0.7, y, 0.0), k), ball(Vec3::new(-k * 0.7, y, 0.0), k)]);
        }
        v
    };
    let reach = |d: Vec3| match p.boxed {
        Some(e) => (d * e).abs().element_sum(),
        None => d.y.abs() * h + r,
    };
    let (lat, deep, tall) = (reach(side), reach(fwd), reach(up));
    let spine = |ribs: &[f32]| {
        let back = -fwd * deep * 0.55;
        let mut v = vec![ball(back, r * 0.3), rod(back - up * tall * 0.6, back + up * tall * 0.6, r * 0.16)];
        v.extend(ribs.iter().map(|&y| ring(up * tall * y - fwd * deep * 0.1, lat * 0.8, deep * 0.75)));
        v
    };
    let mut sockets = None;
    let parts = match name {
        "Head" => {
            let (d, w, t) = (deep * SKULL, lat * SKULL, tall * SKULL);
            sockets = [ball(fwd * d * 0.82 + up * t * 0.15 + side * w * 0.38, w * 0.26),
                       ball(fwd * d * 0.82 + up * t * 0.15 - side * w * 0.38, w * 0.26),
                       ball(fwd * d * 0.92 - up * t * 0.2, w * 0.1)].into_iter().reduce(|mut a, b| {
                a.merge(&b).unwrap();
                a
            });
            vec![Mesh::from(Capsule3d::new(r, h * 2.0)).transformed_by(Transform::from_scale(Vec3::splat(SKULL)))]
        }
        "Neck" => vec![rod(-up * tall, up * tall, r * 0.25), ball(Vec3::ZERO, r * 0.35)],
        "Spine0" => spine(&[]),
        "Spine1" | "Spine2" | "Spine3" => spine(&[0.0]),
        "Buttocks" => vec![ring(Vec3::ZERO, lat * 0.75, deep * 0.6), ball(side * lat * 0.6, r * 0.35), ball(-side * lat * 0.6, r * 0.35),
                           ball(-fwd * deep * 0.5, r * 0.3)],
        _ => match p.boxed {
            Some(e) => {
                let (l, w) = if e.x >= e.y.max(e.z) { (Vec3::X * e.x, if e.y > e.z { Vec3::Y * e.y } else { Vec3::Z * e.z }) }
                             else if e.y >= e.z { (Vec3::Y * e.y, if e.x > e.z { Vec3::X * e.x } else { Vec3::Z * e.z }) }
                             else { (Vec3::Z * e.z, if e.x > e.y { Vec3::X * e.x } else { Vec3::Y * e.y }) };
                let rad = w.length() * 0.18;
                let mut v: Vec<Mesh> = [-0.55, 0.0, 0.55].iter().map(|&o| rod(-l * 0.8 + w * o, l * 0.8 + w * o, rad)).collect();
                v.extend([ball(l * 0.8, rad * 2.2), ball(-l * 0.8, rad * 2.2)]);
                v
            }
            None if name.starts_with("Clavicle") => long(r * 0.12),
            None if name.starts_with("Hand") => long(r * 0.25),
            None => long(r * 0.2),
        },
    };
    let bones = parts.into_iter().reduce(|mut a, b| {
        a.merge(&b).unwrap();
        a
    }).unwrap();
    (bones, sockets)
}

fn broken_bone(name: &str, p: &delirium::ragdoll::PartDef) -> Mesh {
    let mut m = xray_bones(name, p).0;
    m.duplicate_vertices();
    let along = match p.boxed {
        Some(e) => if e.x >= e.y.max(e.z) { Vec3::X } else if e.y >= e.z { Vec3::Y } else { Vec3::Z },
        None if matches!(name, "Head" | "Neck" | "Buttocks") || name.starts_with("Spine") => (p.bind.inverse() * Vec3::Z).normalize(),
        None => Vec3::Y,
    };
    let gap = (p.half + p.radius) * 0.15;
    let bend = Transform::from_translation(along * gap).with_rotation(Quat::from_axis_angle(along.any_orthonormal_vector(), 0.5));
    let get = |m: &Mesh, a| match m.attribute(a) {
        Some(bevy::mesh::VertexAttributeValues::Float32x3(v)) => v.iter().map(|&x| Vec3::from(x)).collect::<Vec<_>>(),
        _ => vec![],
    };
    let (pos, nor) = (get(&m, Mesh::ATTRIBUTE_POSITION), get(&m, Mesh::ATTRIBUTE_NORMAL));
    if nor.len() != pos.len() {
        return m;
    }
    let (mut p2, mut n2) = (Vec::with_capacity(pos.len()), Vec::with_capacity(pos.len()));
    for (t, n) in pos.chunks(3).zip(nor.chunks(3)) {
        let far = (t.iter().sum::<Vec3>()).dot(along) > 0.0;
        for (a, b) in t.iter().zip(n) {
            let (a, b) = if far { (bend.transform_point(*a), bend.rotation * *b) } else { (*a - along * gap, *b) };
            (p2.push(a.to_array()), n2.push(b.to_array()));
        }
    }
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, p2);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n2);
    m
}

fn part_mesh(meshes: &mut Assets<Mesh>, p: &delirium::ragdoll::PartDef) -> Handle<Mesh> {
    meshes.add(part_shape(p))
}

fn part_shape(p: &delirium::ragdoll::PartDef) -> Mesh {
    match p.boxed {
        Some(e) => Cuboid { half_size: e }.into(),
        None => Capsule3d::new(p.radius, p.half * 2.0).into(),
    }
}

fn speck() -> Mesh {
    Sphere::new(1e-5).into()
}

struct Voxels {
    n: [usize; 3],
    lo: Vec3,
    alive: Vec<bool>,
    skin: Vec<bool>,
    dug: Vec<bool>,
    boxed: bool,
    core: f32,
    half: f32,
    ver: u32,
}

impl Voxels {
    fn new(p: &delirium::ragdoll::PartDef, name: &str) -> Self {
        let ext = p.boxed.unwrap_or(Vec3::new(p.radius, p.half + p.radius, p.radius));
        let n = ext.to_array().map(|e| ((e * 2.0 / VOX).ceil() as usize).max(1));
        let lo = -Vec3::new(n[0] as f32, n[1] as f32, n[2] as f32) * VOX / 2.0;
        let core = match name {
            _ if p.boxed.is_some() => 0.0,
            "Head" => p.radius * SKULL,
            n if n.starts_with("Spine") || n == "Buttocks" => 0.0,
            _ => p.radius * 0.25,
        };
        let inside = |x: Vec3| match p.boxed {
            Some(e) => x.abs().cmple(e).all(),
            None => x.distance(Vec3::Y * x.y.clamp(-p.half, p.half)) <= p.radius,
        };
        let mut v = Voxels { n, lo, alive: vec![], skin: vec![], dug: vec![false; n[0] * n[1] * n[2]], boxed: p.boxed.is_some(), core, half: p.half, ver: 1 };
        let cells: Vec<Vec3> = (0..n[0] * n[1] * n[2]).map(|i| v.at(i)).collect();
        v.alive = cells.iter().map(|&x| inside(x) && !v.in_core(x)).collect();
        v.skin = cells.iter().map(|&x| [Vec3::X, Vec3::Y, Vec3::Z].iter().any(|&d| !inside(x + d * VOX) || !inside(x - d * VOX))).collect();
        v
    }

    fn at(&self, i: usize) -> Vec3 {
        let [nx, ny, _] = self.n;
        self.lo + (Vec3::new((i % nx) as f32, (i / nx % ny) as f32, (i / nx / ny) as f32) + 0.5) * VOX
    }

    fn cell(&self, x: Vec3) -> Option<usize> {
        let c = ((x - self.lo) / VOX).floor();
        let [nx, ny, nz] = self.n;
        (c.cmpge(Vec3::ZERO).all() && (c.x as usize) < nx && (c.y as usize) < ny && (c.z as usize) < nz)
            .then(|| c.x as usize + nx * (c.y as usize + ny * c.z as usize))
    }

    fn in_core(&self, x: Vec3) -> bool {
        self.core > 0.0 && x.distance(Vec3::Y * x.y.clamp(-self.half, self.half)) < self.core
    }

    fn dig(&mut self, at: Vec3, dir: Vec3, hole: f32) {
        let reach = self.lo.length() * 2.0;
        let mut t = -VOX;
        while t < reach {
            let x = at + dir * t;
            if self.in_core(x) {
                return;
            }
            if self.cell(x).is_some_and(|i| self.alive[i]) {
                let r = (hole / VOX).ceil() as i32;
                for dz in -r..=r {
                    for dy in -r..=r {
                        for dx in -r..=r {
                            let y = x + Vec3::new(dx as f32, dy as f32, dz as f32) * VOX;
                            if y.distance(x) <= hole {
                                if let Some(i) = self.cell(y) {
                                    self.alive[i] = false;
                                    self.dug[i] = true;
                                }
                            }
                        }
                    }
                }
                self.ver += 1;
                return;
            }
            t += VOX * 0.5;
        }
    }

    fn surface(&self, p: &delirium::ragdoll::PartDef) -> Mesh {
        let mut m = Capsule3d::new(p.radius, p.half * 2.0).mesh().rings(24).longitudes(48).latitudes(24).build();
        let (Some(pos), Some(ix)) = (m.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|a| a.as_float3()), m.indices()) else { return m };
        let ix: Vec<u32> = ix.iter().map(|i| i as u32).collect();
        let pos: Vec<Vec3> = pos.iter().map(|&x| Vec3::from(x)).collect();
        let kept = ix.chunks_exact(3).filter(|t| {
            let c = (pos[t[0] as usize] + pos[t[1] as usize] + pos[t[2] as usize]) / 3.0;
            let inward = (Vec3::Y * c.y.clamp(-self.half, self.half) - c).normalize_or_zero() * VOX;
            ![c, c + inward].iter().any(|&x| self.cell(x).is_some_and(|i| self.dug[i]))
        }).flatten().copied().collect();
        m.insert_indices(bevy::mesh::Indices::U32(kept));
        m
    }

    fn mesh(&self) -> Mesh {
        let (mut pos, mut nor, mut uv, mut tan, mut idx) = (vec![], vec![], vec![], vec![], vec![]);
        let six = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];
        for i in (0..self.alive.len()).filter(|&i| self.alive[i]) {
            let x = self.at(i);
            if !self.boxed && !six.iter().any(|&d| self.cell(x + d * VOX).is_some_and(|j| self.dug[j])) {
                continue;
            }
            let h = ((i as f32 * 12.9898).sin() * 43758.545).rem_euclid(1.0);
            let c = [(match () {
                _ if self.skin[i] => 0.0,
                _ if h < 0.45 => 1.0,
                _ if h < 0.8 => 2.0,
                _ => 3.0,
            } + 0.5) / 4.0, 0.5];
            for d in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
                if self.cell(x + d * VOX).is_some_and(|j| self.alive[j]) {
                    continue;
                }
                let (u, w) = (d.any_orthonormal_vector(), d.cross(d.any_orthonormal_vector()));
                let base = pos.len() as u32;
                for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    pos.push((x + (d + u * a + w * b) * VOX / 2.0).to_array());
                    nor.push(d.to_array());
                    uv.push(c);
                    tan.push(u.extend(1.0).to_array());
                }
                idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        let mut m = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        m.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tan);
        m.insert_indices(bevy::mesh::Indices::U32(idx));
        m
    }
}

fn voxel_palette(skin: [u8; 4]) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let px = [skin, [153, 18, 23, 255], [115, 10, 15, 255], [184, 56, 56, 255]].concat();
    let mut img = Image::new(Extent3d { width: 4, height: 1, depth_or_array_layers: 1 }, TextureDimension::D2, px, TextureFormat::Rgba8UnormSrgb,
                             RenderAssetUsages::default());
    img.sampler = bevy::image::ImageSampler::nearest();
    img
}

fn traceable(mesh: &mut Mesh) {
    use bevy::mesh::Indices;
    let n = mesh.count_vertices();
    if !mesh.contains_attribute(Mesh::ATTRIBUTE_UV_0) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; n]);
    }
    if !mesh.contains_attribute(Mesh::ATTRIBUTE_TANGENT) && mesh.generate_tangents().is_err() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0f32, 0.0, 0.0, 1.0]; n]);
    }
    if mesh.contains_attribute(Mesh::ATTRIBUTE_UV_1) {
        mesh.remove_attribute(Mesh::ATTRIBUTE_UV_1);
    }
    if let Some(Indices::U16(i)) = mesh.indices() {
        let i = i.iter().map(|&x| x as u32).collect();
        mesh.insert_indices(Indices::U32(i));
    }
}

#[derive(Component)]
struct MapScenery;

fn switch(mut sim: NonSendMut<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
          old: Query<Entity, Or<(With<RoomScenery>, With<MapScenery>, With<PartMesh>, With<HeroMesh>, With<HatMesh>)>>, assets: Res<AssetServer>) {
    let Some((scene, marge)) = sim.switch_to.take() else { return };
    let t0 = std::time::Instant::now();
    let mut a: Vec<String> = std::env::args().collect();
    for flag in ["--map", "--ground", "--stage", "--body"] {
        if let Some(i) = a.iter().position(|x| x == flag) {
            a.drain(i..(i + 2).min(a.len()));
        }
    }
    a.retain(|x| x != "--marge");
    a.extend(["--body".to_string(), BODIES[sim.body].0.to_string()]);
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
    new.meat_on = sim.meat_on;
    new.skate3_bails = sim.skate3_bails;
    (new.ped_color, new.body_alpha, new.skel, new.soft, new.eyes, new.film) = (sim.ped_color, sim.body_alpha, sim.skel, sim.soft, sim.eyes, sim.film);
    apply_settings(&mut new, &settings(&sim));
    new.scene = Some(scene.clone());
    new.agent.strength = sim.agent.strength;
    *sim = new;
    let said = format!("scene: {scene}, {} ({} ms)", if marge { "Marge" } else { "Homer" }, t0.elapsed().as_millis());
    eprintln!("{said}");
    sim.say(said);
    for e in &old {
        commands.entity(e).despawn();
    }
    spawn_world(&mut commands, &mut meshes, &mut mats, &sim, &assets);
}

#[derive(Component)]
struct HeroMesh(usize);

#[derive(Component)]
struct HatMesh(usize);

fn bodies(sim: NonSend<Sim>, mut q: Query<(&PartMesh, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>, &Mesh3d, Option<&VoxelOf>, Entity, Option<&Xray>), Without<HeroMesh>>,
          mut hero: Query<(&HeroMesh, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>), Without<PartMesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
          mut hat: Query<(&HatMesh, &mut Transform, &mut Visibility), (Without<PartMesh>, Without<HeroMesh>)>, time: Res<Time>,
          mut bones: Query<(&mut Visibility, Has<AuraMesh>, &BoneMesh, &mut RenderLayers), (Without<PartMesh>, Without<HeroMesh>, Without<HatMesh>, Without<BrokeBone>)>,
          mut snapped: Query<(&mut Visibility, &BrokeBone, &mut RenderLayers), (Without<PartMesh>, Without<HeroMesh>, Without<HatMesh>, Without<BoneMesh>)>,
          mut images: ResMut<Assets<Image>>, mut skins: Local<std::collections::HashMap<(usize, usize), (u8, [u8; 4], Handle<Image>, Handle<Image>)>>,
          mut meshes: ResMut<Assets<Mesh>>, mut carved: Local<std::collections::HashMap<Entity, (usize, u32)>>, mut sized: Local<std::collections::HashMap<Entity, [f32; 5]>>,
          mut wet: ResMut<wet::Wet>) {
    let doll: Option<Vec<Transform>> = sim.hero_doll.as_ref().map(|h| match h.rise.as_ref().zip(sim.getup.as_ref()).and_then(|(r, c)| c.frames(r)) {
        Some(f) => f.into_iter().enumerate().map(|(k, f)| f.with_rotation(f.rotation * delirium::nmdbm::rage_link(nm(), k))).collect(),
        None => h.agent.doll.capsules().collect(),
    });
    let rig = sim.getup.as_ref().and_then(|c| c.hero.as_ref());
    let (dt, on, rich) = (time.delta_secs(), sim.water_wet, sim.wet_film);
    let (res, nearest) = (match sim.water_q { 0..=2 => 8, 3 => 16, _ => 32 }, sim.water_pixels);
    let in_water = |p: Vec3| sim.in_water(p);
    let wet = &mut *wet;
    if !on {
        (wet.homer.clear(), wet.hero.clear(), wet.drips.clear());
    }
    if wet.drips.len() > 200 {
        wet.drips.clear();
    }
    for (m, mut t, mut v, mat) in &mut hero {
        let want = if sim.clean { 1.0 } else { sim.hero_alpha };
        let green = Color::srgb(0.3, 0.75, 0.4).with_alpha(want);
        let def = rig.and_then(|r| r.build.parts.get(m.0)).filter(|_| on);
        let mut sk = def.map(|d| wet::Wet::skin(&mut wet.hero, m.0, || part_shape(d), d.radius + d.half, res));
        let look = sk.as_mut().and_then(|s| s.maps(&mut images, green.to_srgba().to_u8_array(), rich, nearest));
        let (bc, bt, mr, rough, coat) = match look {
            Some((b, r, c)) => (Color::WHITE.with_alpha(want), Some(b), Some(r), 1.0, c),
            None => (green, None, None, wet::ROUGH, 0.0),
        };
        if mats.get(&mat.0).is_some_and(|x| x.base_color != bc || x.base_color_texture != bt || x.metallic_roughness_texture != mr || x.perceptual_roughness != rough || x.clearcoat != coat)
            && let Some(mut x) = mats.get_mut(&mat.0) {
            x.metallic = if mr.is_some() { 1.0 } else { 0.0 };
            (x.base_color, x.base_color_texture, x.metallic_roughness_texture, x.perceptual_roughness) = (bc, bt, mr, rough);
            (x.clearcoat, x.clearcoat_perceptual_roughness) = (coat, 0.04);
            x.alpha_mode = if want >= 1.0 { AlphaMode::Opaque } else { AlphaMode::Blend };
        }
        let at = match &doll {
            Some(f) => rig.and_then(|r| r.build.parts.get(m.0)).and_then(|p| sim.build.parts.iter().position(|q| q.name == p.name)).and_then(|j| f.get(j).copied()),
            None => sim.hero.as_ref().filter(|_| !sim.driving).and_then(|n| n.caps.get(m.0)).copied(),
        };
        match at {
            Some(c) => {
                (*t, *v) = (c, Visibility::Visible);
                if let Some(s) = sk.as_mut() {
                    s.step(&c, dt, rich, wet::drain(def.map_or("", |d| &d.name)), &in_water, &mut wet.drips);
                }
            }
            None => *v = Visibility::Hidden,
        }
    }
    let up = |i: usize| sim.getup.as_ref().zip(sim.rises[i].as_ref()).and_then(|(c, r)| c.frames(r))
        .map(|f| f.into_iter().enumerate().map(|(k, f)| f.with_rotation(f.rotation * delirium::nmdbm::rage_link(nm(), k))).collect::<Vec<_>>());
    let mut fr = sim.shown().map(|f| f.caps.clone()).or_else(|| up(0)).unwrap_or_else(|| sim.agent.doll.capsules().collect());
    for (k, t) in sim.cut_capsules() {
        if let Some(x) = fr.get_mut(k) {
            *x = t;
        }
    }
    let mut crowd: Vec<Vec<Transform>> = sim.crowd.iter().enumerate().map(|(c, (a, _))| sim.bench.get(c).and_then(|b| b.up.clone()).unwrap_or_else(|| a.doll.capsules().collect())).collect();
    for (c, b) in sim.bench.iter().enumerate() {
        for (k, t) in sever::cut_caps(&b.cuts) {
            if let Some(x) = crowd[c].get_mut(k) {
                *x = t;
            }
        }
    }
    for (h, mut t, mut v) in &mut hat {
        let at = match h.0 {
            0 => sim.hat_frame(&fr),
            i => sim.bench.get(i - 1).and_then(|b| sever::hat_at(b.hat.as_ref(), &b.build, &crowd[i - 1])),
        };
        match at {
            Some(h) => (*t, *v) = (h, Visibility::Visible),
            None => *v = Visibility::Hidden,
        }
    }
    let link = |k: usize, f: Transform| f.with_rotation(f.rotation * delirium::nmdbm::rage_link(nm(), k));
    let flick = (time.elapsed_secs() * SHOCK_HZ).fract() < SHOCK_XRAY;
    let shocked = |h: usize| sim.look(h).is_some_and(|l| l.3 > 0.0);
    let traced = sim.rt_now() > 0;
    let levels = sim.meat_levels();
    let level = |k: usize| levels.as_ref().and_then(|l| l.get(k).copied()).unwrap_or(-1);
    for (mut v, b, mut l) in &mut snapped {
        let want = if level(b.0) >= 4 { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        let to = RenderLayers::layer(if traced { SKEL_LAYER } else { 1 });
        if *l != to {
            *l = to;
        }
    }
    for (mut v, aura, b, mut l) in &mut bones {
        let hom = b.0 == 0 && !aura && (2..4).contains(&level(b.1));
        let want = if shocked(b.0) && (flick || aura) && !(aura && traced) || hom { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        let to = RenderLayers::layer(if traced && !aura { SKEL_LAYER } else { 1 });
        if *l != to {
            *l = to;
        }
    }
    for (p, mut t, mut v, m, shape, vox, e, xr) in &mut q {
        let ped = p.crowd.map_or(0, |c| c + 1);
        let lit = sim.glow.get(ped).and_then(|g| g.get(p.k)).filter(|_| p.watch.is_none()).map_or(LinearRgba::BLACK, |&(g, (c, _))| c * g);
        let look = p.homer().and_then(|h| sim.look(h));
        let burning = if ped == 0 { sim.burning } else { look.map_or(0.0, |l| l.2) };
        let lit = if p.watch.is_none() && sim.fire_homer && burning > 0.0 {
            let t = time.elapsed_secs() + p.k as f32 * 0.9;
            lit + FIRE_GLOW * ((burning / 2.0).min(1.0) * (0.6 + 0.4 * (t * 13.0).sin() * (t * 5.1).cos()))
        } else {
            lit
        };
        let col = part_color(sim.ped_color, delirium_data::nm_asset::PARTS.get(p.k).copied().unwrap_or(""), sim.body_alpha);
        let col = if sim.clean { col.with_alpha(1.0) } else { col };
        let mut fresh = false;
        let game_half = false;
        let name = delirium_data::nm_asset::PARTS.get(p.k).copied().unwrap_or("");
        if let Some(d) = sim.build_of(ped).parts.get(p.k).filter(|_| p.watch.is_none() && !game_half) {
            let size = [d.radius, d.half, d.boxed.map_or(0.0, |e| e.x), d.boxed.map_or(0.0, |e| e.y), d.boxed.map_or(0.0, |e| e.z)];
            if sized.insert(e, size).is_some_and(|s| s != size) {
                if !look.is_some_and(|l| l.0.get(p.k).is_some_and(|v| v.is_some())) && let Some(mut m) = meshes.get_mut(&shape.0) {
                    *m = part_shape(d);
                }
                if let Some(x) = xr {
                    let (b, s) = xray_bones(name, d);
                    for (h, m) in [(Some(&x.0), Some(b)), (x.1.as_ref(), s), (Some(&x.2), Some(part_shape(d))), (x.3.as_ref(), x.3.as_ref().map(|_| broken_bone(name, d)))] {
                        if let (Some(h), Some(m)) = (h, m) && let Some(mut x) = meshes.get_mut(h) {
                            *x = m;
                        }
                    }
                }
            }
        }
        let c = look.and_then(|l| l.1.get(p.k).copied()).unwrap_or(0.0);
        if let (Some(vo), Some(l)) = (vox, look) {
            let dug = l.0.get(p.k).and_then(|v| v.as_ref());
            let now = (l.4, dug.map_or(0, |v| v.ver));
            if carved.get(&e).copied() != Some(now) {
                carved.insert(e, now);
                if let Some(def) = sim.build_of(ped).parts.get(p.k) {
                    let (solid, flesh, bones) = match dug {
                        Some(v) if v.boxed => (speck(), v.mesh(), Some(xray_bones(name, def))),
                        Some(v) => (v.surface(def), v.mesh(), Some(xray_bones(name, def))),
                        None => (part_shape(def), speck(), None),
                    };
                    let bones = bones.map_or_else(speck, |(mut b, sockets)| {
                        if let Some(s) = sockets {
                            let _ = b.merge(&s);
                        }
                        b
                    });
                    for (h, mut m) in [(&shape.0, solid), (&vo.0, flesh), (&vo.1, bones)] {
                        if m.count_vertices() == 0 {
                            m = speck();
                        }
                        traceable(&mut m);
                        if let Some(mut x) = meshes.get_mut(h) {
                            *x = m;
                        }
                    }
                    if let Some(mut x) = images.get_mut(&vo.2) {
                        *x = voxel_palette(col.to_srgba().to_u8_array());
                    }
                }
            }
        }
        let (col, skin) = match look.is_some() && c > 0.0 {
            true => {
                let (level, px) = ((c * 16.0).ceil() as u8, col.to_srgba().to_u8_array());
                if skins.get(&(ped, p.k)).is_none_or(|(l, x, _, _)| (*l, *x) != (level, px)) {
                    let (base, glow) = skin(p.k, level as f32 / 16.0, px);
                    match skins.get_mut(&(ped, p.k)) {
                        Some((l, x, b, g)) => {
                            (*l, *x) = (level, px);
                            images.get_mut(&*b).map(|mut i| *i = base);
                            images.get_mut(&*g).map(|mut i| *i = glow);
                        }
                        None => {
                            skins.insert((ped, p.k), (level, px, images.add(base), images.add(glow)));
                        }
                    }
                    fresh = true;
                }
                (Color::WHITE.with_alpha(col.alpha()), skins.get(&(ped, p.k)).map(|(_, _, b, g)| (b.clone(), g.clone())))
            }
            false => (col, None),
        };
        let embers = burning > 0.0 && c > 0.0 && skin.is_some();
        let lit = if embers {
            let t = time.elapsed_secs() * 7.0 + p.k as f32 * 1.3;
            lit + EMBER_GLOW * ((burning / 2.0).min(1.0) * (0.55 + 0.45 * t.sin() * (t * 0.37).cos()))
        } else {
            lit
        };
        let (base_tex, glow_tex) = match skin {
            Some((b, g)) => (Some(b), embers.then_some(g)),
            None => (None, None),
        };
        let flash = flick && look.is_some_and(|l| l.3 > 0.0);
        let meat = ped == 0 && p.watch.is_none() && !game_half && sim.meat.as_ref().is_some_and(|m| m.xray > 0.0);
        let (col, lit, base_tex, glow_tex) = match (flash, meat) {
            (true, _) => (SHOCK_SHELL, LinearRgba::BLACK, None, None),
            (_, true) => (MEAT_SHELL, LinearRgba::BLACK, None, None),
            _ => (col, lit, base_tex, glow_tex),
        };
        let flash = flash || meat;
        let def = look.and_then(|l| sim.build_of(ped).parts.get(p.k).map(|d| (d, l.4))).filter(|_| on && !flash && c == 0.0);
        let mut sk = def.map(|(d, id)| wet::Wet::skin(&mut wet.homer, id * 64 + p.k, || part_shape(d), d.radius + d.half, res));
        let look = sk.as_mut().and_then(|s| s.maps(&mut images, col.to_srgba().to_u8_array(), rich, nearest));
        let (col, base_tex, mr, rough, coat) = match look {
            Some((b, r, cc)) => (Color::WHITE.with_alpha(col.alpha()), Some(b), Some(r), 1.0, cc),
            None => (col, base_tex, None, wet::ROUGH, 0.0),
        };
        if fresh || mats.get(&m.0).is_some_and(|x| x.emissive != lit || x.base_color != col || x.base_color_texture != base_tex || x.emissive_texture != glow_tex || x.unlit != flash
                                               || x.metallic_roughness_texture != mr || x.perceptual_roughness != rough || x.clearcoat != coat) {
            if let Some(mut x) = mats.get_mut(&m.0) {
                x.unlit = flash;
                x.emissive = lit;
                x.metallic = if mr.is_some() { 1.0 } else { 0.0 };
                (x.metallic_roughness_texture, x.perceptual_roughness, x.clearcoat, x.clearcoat_perceptual_roughness) = (mr, rough, coat, 0.04);
                x.base_color = col;
                x.base_color_texture = base_tex;
                x.emissive_texture = glow_tex;
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
            Some(a) => {
                (*t, *v) = (a, Visibility::Visible);
                if let Some(s) = sk.as_mut() {
                    s.step(&a, dt, rich, wet::drain(def.map_or("", |d| &d.0.name)), &in_water, &mut wet.drips);
                }
            }
            None => *v = Visibility::Hidden,
        }
    }
    if sim.rt_now() != 0 || !sim.wet_drips {
        wet.drips.clear();
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
struct Plat;

#[derive(Component)]
struct Rope(usize);

fn plank_half(teeth: &[f32], s: usize) -> Mesh {
    let h = (*world::PLANK_HALF());
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

fn planks(sim: NonSend<Sim>, mut q: Query<(&Plank, &mut Transform), Without<Rope>>, mut r: Query<(&Rope, &mut Transform), Without<Plank>>,
          mut plat: Query<&mut Transform, (With<Plat>, Without<Plank>, Without<Rope>)>) {
    if let Some((at, rot)) = world::plat(&sim.room.stage, sim.room.shook) {
        for mut t in &mut plat {
            *t = Transform::from_translation(at).with_rotation(rot);
        }
    }
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
struct Glass;
#[derive(Component)]
pub struct Clear;
#[derive(Component)]
struct Water;

#[derive(Component)]
pub(crate) struct SkyWater;
#[derive(Component)]
struct GlassLight;

fn video(mut commands: Commands, mut sim: NonSendMut<Sim>, mut last: Local<Option<(u8, u8, u8, u8, u8, bool, (u8, u8), u8, bool, [u8; 5], u8)>>, adapter: Option<Res<bevy::render::renderer::RenderAdapter>>,
         mut map: ResMut<bevy::light::DirectionalLightShadowMap>, mut aa_was: Local<Option<u8>>,
         info: Option<Res<bevy::render::renderer::RenderAdapterInfo>>, mut cam: Query<(Entity, &Orbit, Option<&mut bevy::post_process::dof::DepthOfField>), With<OursCam>>,
         sun: Query<(Entity, Has<Fill>), With<DirectionalLight>>) {
    if let Some(i) = info.filter(|_| sim.gpu_now.is_empty()) {
        sim.gpu_now = i.name.clone();
        let (f, id) = (getup::config_dir().join(FALLBACK), format!("{} {} {}", i.name, i.driver, i.driver_info));
        if let Ok(t) = std::fs::read_to_string(&f) {
            match t.lines().nth(1) {
                None => drop(std::fs::write(&f, format!("{}
{id}", t.trim()))),
                Some(was) if was != id => {
                    let _ = std::fs::remove_file(&f);
                    sim.say(format!("room  new graphics card or driver ({id}): full graphics from the next start"));
                }
                _ => {}
            }
        }
        sim.gpus = GPUS.get().cloned();
        if i.device_type == wgpu::DeviceType::IntegratedGpu && !sim.safe && sim.script.is_empty() && !getup::config_dir().join(SETTINGS).exists() {
            apply_settings(&mut sim, SAFE);
            sim.say(format!("room  integrated graphics ({}): lowest settings to begin with", i.name));
        }
    }
    if last.is_none() {
        if let Some(a) = adapter {
            use bevy::render::render_resource::TextureFormat as F;
            let ok = |n| [F::Rgba8UnormSrgb, F::Rgba16Float, F::Depth32Float].iter().all(|&f| a.get_texture_format_features(f).flags.sample_count_supported(n));
            sim.msaa_max = [8, 4, 2].into_iter().find(|&n| ok(n as u32)).unwrap_or(1);
        }
    }
    if *aa_was != Some(sim.aa) {
        if aa_was.is_some() {
            sim.sharpen = sim.sharp_by[sim.aa as usize];
        }
        *aa_was = Some(sim.aa);
    }
    let a = sim.aa as usize;
    sim.sharp_by[a] = sim.sharpen;
    let f_stops = 0.25 * 16f32.powf(1.0 - sim.dof);
    for (_, orbit, dof) in &mut cam {
        if let Some(mut d) = dof && (d.focal_distance != orbit.dist || d.aperture_f_stops != f_stops) {
            (d.focal_distance, d.aperture_f_stops) = (orbit.dist, f_stops);
        }
    }
    let aa = sim.aa_now();
    let ssao = sim.ssao;
    let now = (aa, sim.msaa, sim.smaa_q, sim.shadow_q, (sim.sharpen * 100.0) as u8, sim.fsr1(), (ssao, (sim.ao_strength * 100.0) as u8), (sim.bloom * 100.0) as u8, sim.dof > 0.0, [sim.contact, sim.contact_k, sim.vignette, sim.lens, sim.chroma].map(|x| (x * 100.0) as u8), sim.rt_now() | (sim.restir as u8) << 4 | sim.shadow_filter << 5 | (sim.water_refract as u8) << 7);
    if *last == Some(now) {
        return;
    }
    *last = Some(now);
    let msaa = match (aa, sim.msaa.min(sim.msaa_max)) { (AA_MSAA, 2) => Msaa::Sample2, (AA_MSAA, 4) => Msaa::Sample4, (AA_MSAA, 8) => Msaa::Sample8, _ => Msaa::Off };
    use bevy::anti_alias::{contrast_adaptive_sharpening::ContrastAdaptiveSharpening as Cas, fxaa::Fxaa, smaa::{Smaa, SmaaPreset}, taa::TemporalAntiAliasing};
    for (e, ..) in &cam {
        let mut c = commands.entity(e);
        c.insert(msaa);
        if aa == AA_FXAA { c.insert(Fxaa::default()); } else { c.remove::<Fxaa>(); }
        if aa == AA_SMAA {
            c.insert(Smaa { preset: [SmaaPreset::Low, SmaaPreset::Medium, SmaaPreset::High, SmaaPreset::Ultra][sim.smaa_q as usize] });
        } else {
            c.remove::<Smaa>();
        }
        if aa == AA_TAA {
            c.insert(TemporalAntiAliasing::default());
        } else {
            c.remove::<TemporalAntiAliasing>();
        }
        if aa < AA_TAA {
            c.remove::<(bevy::render::camera::TemporalJitter, bevy::render::camera::MipBias, bevy::core_pipeline::prepass::MotionVectorPrepass)>();
            if ssao == 0 && sim.contact <= 0.0 && !(sim.water_refract) {
                c.remove::<bevy::core_pipeline::prepass::DepthPrepass>();
            }
        }
        use bevy::pbr::{ScreenSpaceAmbientOcclusion as Ssao, ScreenSpaceAmbientOcclusionQualityLevel as L};
        use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
        if sim.water_refract {
            c.insert(DepthPrepass);
        }
        if ssao > 0 {
            c.insert((Ssao { quality_level: [L::Low, L::Medium, L::High, L::Ultra][ssao as usize - 1], strength: sim.ao_strength, ..default() }, DepthPrepass, NormalPrepass));
        } else {
            c.remove::<(Ssao, NormalPrepass)>();
        }
        if sim.bloom > 0.0 {
            c.insert(bevy::post_process::bloom::Bloom { intensity: 0.3 * sim.bloom, ..bevy::post_process::bloom::Bloom::NATURAL });
        } else {
            c.remove::<bevy::post_process::bloom::Bloom>();
            if !sim.needs_hdr() {
                c.remove::<bevy::camera::Hdr>();
            }
        }
        if sim.dof > 0.0 {
            c.insert_if_new(bevy::post_process::dof::DepthOfField { aperture_f_stops: f_stops, ..default() });
        } else {
            c.remove::<bevy::post_process::dof::DepthOfField>();
        }
        use bevy::solari::{pathtracer::Pathtracer, prelude::SolariLighting};
        match sim.rt_now() {
            0 => {
                c.remove::<(SolariLighting, Pathtracer, bevy::core_pipeline::prepass::DeferredPrepass)>().insert(bevy::camera::CameraMainTextureUsages::default());
            }
            rt => {
                c.insert(bevy::camera::CameraMainTextureUsages::default().with(bevy::render::render_resource::TextureUsages::STORAGE_BINDING));
                if rt == 1 {
                    c.remove::<Pathtracer>().insert(SolariLighting { restir: sim.restir, ..default() });
                } else {
                    c.remove::<(SolariLighting, bevy::core_pipeline::prepass::DeferredPrepass)>().insert(Pathtracer::default());
                }
            }
        }
        if sim.contact > 0.0 {
            c.insert(bevy::pbr::ContactShadows { length: sim.contact, intensity: sim.contact_k, ..default() });
        } else {
            c.remove::<bevy::pbr::ContactShadows>();
        }
        use bevy::post_process::effect_stack::{ChromaticAberration, LensDistortion, Vignette};
        if sim.vignette > 0.0 { c.insert(Vignette { intensity: 0.6 * sim.vignette, ..default() }); } else { c.remove::<Vignette>(); }
        if sim.lens > 0.0 { c.insert(LensDistortion { intensity: 0.5 * sim.lens, ..default() }); } else { c.remove::<LensDistortion>(); }
        if sim.chroma > 0.0 { c.insert(ChromaticAberration { intensity: 0.04 * sim.chroma, ..default() }); } else { c.remove::<ChromaticAberration>(); }
        let fm = match (sim.shadow_filter, sim.shadow_q) {
            (1, _) => bevy::light::ShadowFilteringMethod::Gaussian,
            (2, _) => bevy::light::ShadowFilteringMethod::Temporal,
            (3, _) | (0, 1) => bevy::light::ShadowFilteringMethod::Hardware2x2,
            _ if aa >= AA_TAA => bevy::light::ShadowFilteringMethod::Temporal,
            _ => bevy::light::ShadowFilteringMethod::Gaussian,
        };
        c.insert(fm);
        if sim.sharpen > 0.0 && !sim.fsr1() && aa != AA_FSR3 {
            commands.entity(e).insert(Cas { enabled: true, sharpening_strength: sim.sharpen, denoise: false });
        } else {
            commands.entity(e).remove::<Cas>();
        }
    }
    let b = bevy::light::CascadeShadowConfigBuilder::default();
    map.size = [1024, 2048, 4096][sim.shadow_q.clamp(1, 3) as usize - 1];
    let cascades = match sim.shadow_q {
        1 => bevy::light::CascadeShadowConfigBuilder { num_cascades: 1, maximum_distance: 12.0, ..b },
        2 => bevy::light::CascadeShadowConfigBuilder { num_cascades: 2, first_cascade_far_bound: 8.0, maximum_distance: 30.0, ..b },
        _ => bevy::light::CascadeShadowConfigBuilder { first_cascade_far_bound: 5.0, ..b },
    }.build();
    for (e, fill) in &sun {
        if !fill {
            commands.entity(e).insert(cascades.clone());
        }
    }
}

const AA_NAMES: [&str; 9] = ["off", "FXAA", "SMAA", "MSAA", "TAA", "DLAA", "DLSS", "FSR 3", "XeSS"];
const AA_FXAA: u8 = 1;
const AA_SMAA: u8 = 2;
const AA_MSAA: u8 = 3;
const AA_TAA: u8 = 4;
const AA_DLAA: u8 = 5;
const AA_DLSS: u8 = 6;
const AA_FSR3: u8 = 7;
const AA_XESS: u8 = 8;

impl Sim {
    fn aa_now(&self) -> u8 {
        match self.aa {
            AA_DLAA | AA_DLSS if !self.dlss_ok => AA_MSAA,
            AA_FSR3 if !self.fsr_ok => AA_MSAA,
            AA_XESS if !self.xess_ok => AA_MSAA,
            _ if self.rt_now() == 2 => if self.dlss_ok { AA_DLAA } else { 0 },
            aa if aa < AA_DLAA && self.rt_now() == 1 => if self.dlss_ok { AA_DLAA } else { aa.max(AA_TAA) },
            aa => aa,
        }
    }
    fn photons_now(&self) -> bool {
        self.rt_now() >= 1 && self.photons && self.water_caustics
    }
    fn set_fire(&mut self, q: u8) {
        self.fire = q;
        (self.fire_light, self.fire_homer) = (q != 1, q == FIRE_HIGH);
    }
    fn set_water(&mut self, q: u8) {
        self.water_q = q;
        self.water_traced = q >= 3;
        let x = q >= 3;
        (self.water_caustics, self.water_spray, self.water_waves, self.water_pixels, self.water_haze, self.water_mirror, self.water_foam, self.water_mist, self.water_wet, self.wet_film, self.wet_drips, self.sky_shade) = (x, x, x, x, x, x, x, x, x, x, x, x);
        (self.water_ripples, self.water_refract, self.photons, self.water_fine, self.water_depth) = (q >= 1, q >= 2, q >= 2, q >= 4, q >= 1);
        (self.real_surface, self.real_ripples, self.water_clear, self.wet_trail, self.water_impact) = (q >= 2, x, true, true, true);
        (self.water_relax, self.water_ghost, self.water_vort, self.water_spray_phys) = (q >= 2, q >= 2, q >= 3, q >= 3);
    }

    fn rt_now(&self) -> u8 {
        if self.rt_ok { self.rt } else { 0 }
    }
    fn upscaling(&self) -> bool {
        self.aa_now() >= AA_DLAA
    }
    fn needs_hdr(&self) -> bool {
        self.upscaling() || self.bloom > 0.0 || self.sky == 2 || self.room.stage == "pttest" && self.water_caustics
    }
    fn fsr1(&self) -> bool {
        self.render_scale < 0.999 && !self.upscaling() && self.upscaler == UP_FSR1
    }
}

fn ray_tracing(mut commands: Commands, mut sim: NonSendMut<Sim>, device: Option<Res<bevy::render::renderer::RenderDevice>>, mut last: Local<u8>,
               mut method: ResMut<bevy::pbr::DefaultOpaqueRendererMethod>, mut mats: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>,
               lit: Query<(Entity, &Mesh3d, &InheritedVisibility, Option<&RenderLayers>, Option<&bevy::solari::prelude::RaytracingMesh3d>, Has<Clear>, Has<GlassLight>, Has<Water>), (With<MeshMaterial3d<StandardMaterial>>, Or<(Without<water::Fx>, With<water::Caustic>)>)>,
               ) {
    use bevy::{mesh::Indices, solari::prelude::RaytracingMesh3d};
    sim.rt_ok = device.is_some_and(|d| d.features().contains(bevy::solari::SolariPlugins::required_wgpu_features()));
    let rt = sim.rt_now();
    let ours = RenderLayers::from_layers(&[0, 1]);
    for (e, m, shown, layers, traced, glass, fake, water) in &lit {
        let want = rt > 0 && shown.get() && (layers.is_none_or(|l| l.intersects(&ours)) || water || glass) && !fake
            && !(rt == 1 && water && !sim.water_traced && !sim.photons_now());
        let to = RenderLayers::layer(if rt > 0 { water::TRACED_ONLY } else { 1 });
        if glass && layers != Some(&to) {
            commands.entity(e).insert(to);
        }
        if want == traced.is_some() && traced.is_none_or(|t| t.0 == m.0) {
            continue;
        }
        if !want {
            commands.entity(e).remove::<RaytracingMesh3d>();
            continue;
        }
        {
            if let Some(mut mesh) = meshes.get_mut(&m.0) {
                traceable(&mut mesh);
            }
            commands.entity(e).insert(RaytracingMesh3d(m.0.clone()));
        }
    }
    if *last == rt {
        return;
    }
    *last = rt;
    *method = if rt == 1 { bevy::pbr::DefaultOpaqueRendererMethod::deferred() } else { bevy::pbr::DefaultOpaqueRendererMethod::forward() };
    let ids: Vec<_> = mats.ids().collect();
    for id in ids {
        if let Some(mut m) = mats.get_mut(id) {
            m.opaque_render_method = bevy::material::OpaqueRendererMethod::Auto;
        }
    }
}

fn sky(mut commands: Commands, sim: NonSend<Sim>, mut images: ResMut<Assets<Image>>, assets: Res<AssetServer>,
       mut media: ResMut<Assets<bevy::light::atmosphere::ScatteringMedium>>, mut cam: Query<(Entity, &mut Camera), Or<(With<OursCam>, With<water::Mirror>, With<SkyCam>)>>, new: Query<(), Added<SkyCam>>,
       planet: Query<Entity, With<bevy::light::Atmosphere>>, mut last: Local<Option<(u8, u8, [f32; 3], f32)>>) {
    use bevy::light::{Atmosphere, AtmosphereEnvironmentMapLight, EnvironmentMapLight, GeneratedEnvironmentMapLight, Skybox};
    let now = match sim.sky {
        0 => (0, sim.rt_now().min(1), sim.skin.sky, 0.0),
        1 => (1, 0, [sim.sky_k, 0.0, 0.0], sim.sun_dir[0]),
        k => (k, 0, [0.0; 3], 0.0),
    };
    if *last == Some(now) && new.is_empty() || cam.is_empty() {
        return;
    }
    *last = Some(now);
    let k = 1.0 / bevy::camera::Exposure::default().exposure();
    for e in &planet {
        if sim.sky != 2 {
            commands.entity(e).despawn();
        }
    }
    if sim.sky == 2 && planet.is_empty() {
        commands.spawn((Atmosphere::earth(media.add(bevy::light::atmosphere::ScatteringMedium::default())), Transform::from_xyz(0.0, 0.0, -6_360_000.0)));
    }
    for (e, mut camera) in &mut cam {
        camera.clear_color = if sim.sky == 2 { ClearColorConfig::Custom(Color::BLACK) } else { ClearColorConfig::Default };
        let mut c = commands.entity(e);
        c.remove::<(Skybox, EnvironmentMapLight, GeneratedEnvironmentMapLight, AtmosphereEnvironmentMapLight, bevy::pbr::AtmosphereSettings)>();
        match sim.sky {
            1 => {
                let rotation = Quat::from_rotation_z((sim.sun_dir[0] + 55.8).to_radians()) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
                let image = assets.load("embedded://sky.ktx2");
                c.insert((Skybox { image: Some(image.clone()), brightness: k * sim.sky_k, rotation },
                          GeneratedEnvironmentMapLight { environment_map: image, intensity: k * sim.sky_k, rotation, ..default() }));
            }
            2 => {
                c.insert((bevy::pbr::AtmosphereSettings::default(), AtmosphereEnvironmentMapLight::default(), bevy::camera::Hdr));
            }
            _ if now.1 > 0 => {
                use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
                let [r, g, b] = sim.skin.sky;
                let l = Color::srgb(r, g, b).to_linear();
                let px: Vec<u8> = [l.red, l.green, l.blue, 1.0].map(f32::to_le_bytes).concat();
                let mut img = Image::new_fill(Extent3d { width: 1, height: 1, depth_or_array_layers: 6 }, TextureDimension::D2, &px,
                                              TextureFormat::Rgba32Float, bevy::asset::RenderAssetUsages::RENDER_WORLD);
                img.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
                let h = images.add(img);
                c.insert(EnvironmentMapLight { diffuse_map: h.clone(), specular_map: h, intensity: k, ..default() });
            }
            _ => {}
        }
    }
}

#[cfg(feature = "dlss")]
fn dlss(mut commands: Commands, mut sim: NonSendMut<Sim>, ok: Option<Res<bevy::anti_alias::dlss::DlssSuperResolutionSupported>>,
        rr: Option<Res<bevy::anti_alias::dlss::DlssRayReconstructionSupported>>,
        cam: Query<(Entity, &Camera), With<OursCam>>, mut last: Local<Option<u8>>) {
    use bevy::anti_alias::dlss::{Dlss, DlssPerfQualityMode as M, DlssRayReconstructionFeature as Rr, DlssSuperResolutionFeature as Sr};
    sim.dlss_ok = ok.is_some();
    let want = match sim.aa_now() { AA_DLAA => 1, AA_DLSS => 2 + sim.dlss_q, _ => 0 };
    let want = want + if want > 0 && sim.rt_now() > 0 && rr.is_some() { 8 } else { 0 };
    if *last == Some(want) || cam.iter().any(|(_, c)| c.viewport.is_none()) || cam.is_empty() {
        return;
    }
    *last = Some(want);
    let mode = [M::Dlaa, M::Dlaa, M::Quality, M::Balanced, M::Performance, M::UltraPerformance][want as usize % 8];
    for (e, _) in &cam {
        let mut c = commands.entity(e);
        if want == 0 {
            c.remove::<(Dlss<Sr>, Dlss<Rr>)>();
            if !sim.needs_hdr() {
                c.remove::<bevy::camera::Hdr>();
            }
        } else if want >= 8 {
            c.remove::<Dlss<Sr>>().insert(Dlss::<Rr> { perf_quality_mode: mode, ..default() });
        } else {
            c.remove::<Dlss<Rr>>().insert(Dlss::<Sr> { perf_quality_mode: mode, ..default() });
        }
    }
}

#[cfg(windows)]
fn upscale(mut commands: Commands, mut sim: NonSendMut<Sim>, ok: Option<Res<upscale::Supported>>,
        cam: Query<(Entity, &Camera), With<OursCam>>, mut last: Local<Option<(u8, u8, u8)>>) {
    (sim.fsr_ok, sim.xess_ok) = ok.map_or((false, false), |o| (o.fsr3, o.xess));
    let (by, mode) = match sim.aa_now() {
        AA_FSR3 => (Some(upscale::By::Fsr3), sim.fsr_q),
        AA_XESS => (Some(upscale::By::Xess), sim.xess_q),
        _ => (None, 0),
    };
    let want = if by.is_some() { 1 + mode } else { 0 };
    let now = (want, sim.aa_now(), (sim.sharpen * 100.0) as u8);
    if *last == Some(now) || cam.iter().any(|(_, c)| c.viewport.is_none()) || cam.is_empty() {
        return;
    }
    let was = last.replace(now).is_some_and(|l| l.0 > 0);
    for (e, _) in &cam {
        if let Some(by) = by {
            commands.entity(e).insert((upscale::Upscale { by, mode, sharpness: sim.sharpen }, bevy::camera::Hdr));
        } else if was {
            commands.entity(e).remove::<upscale::Upscale>();
            if !sim.needs_hdr() {
                commands.entity(e).remove::<bevy::camera::Hdr>();
            }
        }
    }
}

const FPS_VSYNC: f32 = 19.0;
const FPS_FREE: f32 = 361.0;
const FPS_BACKGROUND: f32 = 60.0;

#[cfg(windows)]
fn in_front(_: &Query<&mut Window, With<bevy::window::PrimaryWindow>>) -> bool {
    unsafe extern "system" {
        fn GetForegroundWindow() -> *mut std::ffi::c_void;
        fn GetWindowThreadProcessId(w: *mut std::ffi::c_void, pid: *mut u32) -> u32;
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid) };
    pid == std::process::id()
}
#[cfg(not(windows))]
fn in_front(win: &Query<&mut Window, With<bevy::window::PrimaryWindow>>) -> bool {
    win.iter().any(|w| w.focused)
}

fn fps_limit(sim: NonSend<Sim>, mut win: Query<&mut Window, With<bevy::window::PrimaryWindow>>, mut next: Local<Option<std::time::Instant>>) {
    use bevy::window::PresentMode;
    let limit = if in_front(&win) || std::env::var_os("FRAME_LOG").is_some() { sim.fps_limit } else { sim.fps_limit.min(FPS_BACKGROUND) };
    let mode = if limit <= FPS_VSYNC { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
    for mut w in &mut win {
        if w.present_mode != mode {
            w.present_mode = mode;
        }
    }
    if limit <= FPS_VSYNC || limit >= FPS_FREE {
        *next = None;
        return;
    }
    let step = std::time::Duration::from_secs_f32(1.0 / limit);
    let now = std::time::Instant::now();
    let due = next.filter(|&t| t + step > now).unwrap_or(now);
    if let Some(d) = due.checked_duration_since(now).and_then(|d| d.checked_sub(std::time::Duration::from_millis(2))) {
        std::thread::sleep(d);
    }
    while std::time::Instant::now() < due {
        std::hint::spin_loop();
    }
    *next = Some(due + step);
}

static GPUS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

fn preset(sim: &Sim) -> Option<usize> {
    VIDEO_PRESETS.iter().position(|&(m, q, soft, fire, water)| (if m == 1 { sim.aa == 0 } else { sim.aa == AA_MSAA && sim.msaa == m }) && sim.shadows == (q > 0) && (q == 0 || sim.shadow_q == q)
        && (sim.shadow > 0.0) == soft && sim.fire == fire && sim.water_q == water)
}
const VIDEO_PRESETS: [(u8, u8, bool, u8, u8); 3] = [(1, 0, false, 0, 0), (4, 2, false, 2, 1), (4, 3, true, FIRE_HIGH, 2)];

#[derive(Component)]
struct Floor;

fn follow_floor(sim: NonSend<Sim>, mut q: Query<&mut Transform, With<Floor>>) {
    for mut t in &mut q {
        if sim.room.pool {
            (t.translation, t.scale) = (Vec3::ZERO, Vec3::ONE);
            continue;
        }
        (t.translation, t.scale) = (sim.ours().p(0).with_z(0.0), Vec3::new(100.0, 100.0, 1.0));
    }
}

fn god_rays(mut commands: Commands, sim: NonSend<Sim>, under: Res<water::Under>, cam: Query<(Entity, Has<bevy::light::VolumetricFog>), With<OursCam>>,
            mut suns: Query<(Entity, &RenderLayers, &mut DirectionalLight, Has<bevy::light::VolumetricLight>), Without<Fill>>,
            mut fog: Query<(&mut Visibility, &mut Transform), (With<bevy::light::FogVolume>, Without<water::Haze>)>) {
    let on = ((sim.god_rays && !(under.0 && sim.rt_now() > 0)) || (under.0 && sim.rt_now() == 0)) && (sim.shadows || sim.rt_now() > 0);
    let maps = (sim.shadows && sim.rt_now() == 0) || on;
    for (_, _, mut l, _) in &mut suns {
        if l.shadow_maps_enabled != maps {
            l.shadow_maps_enabled = maps;
        }
    }
    for (mut v, mut t) in &mut fog {
        *v = if on && sim.god_rays { Visibility::Inherited } else { Visibility::Hidden };
        t.translation = sim.ours().p(0).with_z(GOD_RAYS_BOX.z * 0.5);
    }
    for (c, has) in &cam {
        if on && !has {
            commands.entity(c).insert(bevy::light::VolumetricFog { ambient_intensity: 0.0, ..default() });
        } else if !on && has {
            commands.entity(c).remove::<bevy::light::VolumetricFog>();
        }
    }
    for (e, l, _, has) in &suns {
        let want = on && l.intersects(&RenderLayers::layer(1));
        if want && !has {
            commands.entity(e).insert(bevy::light::VolumetricLight);
        } else if !want && has {
            commands.entity(e).remove::<bevy::light::VolumetricLight>();
        }
    }
}
fn glass_light(mut commands: Commands, sim: NonSend<Sim>, glass: Query<&Transform, With<Glass>>, made: Query<Entity, With<GlassLight>>,
               mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut last: Local<Option<([f32; 2], bool, bool, f32)>>,
               panes: Query<&MeshMaterial3d<StandardMaterial>, With<Glass>>, mut kept: Local<std::collections::HashMap<AssetId<StandardMaterial>, Handle<Image>>>) {
    for m in &panes {
        if sim.glass_tint {
            if let Some(t) = kept.remove(&m.0.id()) && let Some(mut x) = mats.get_mut(&m.0) {
                (x.base_color_texture, x.base_color) = (Some(t), Color::WHITE);
            }
        } else if mats.get(&m.0).is_some_and(|x| x.base_color_texture.is_some()) && let Some(mut x) = mats.get_mut(&m.0) {
            kept.insert(m.0.id(), x.base_color_texture.take().unwrap());
            x.base_color = Color::srgba(0.9, 0.95, 1.0, 0.2);
        }
    }
    let clear = !sim.glass_shade;
    if bevy::solari::scene::GLASS_CLEAR_LIGHT.swap(clear, std::sync::atomic::Ordering::Relaxed) != clear {
        panes.iter().for_each(|m| if let Some(mut x) = mats.get_mut(&m.0) { let _: &mut StandardMaterial = &mut x; });
    }
    let on = !glass.is_empty() && sim.rt_now() == 0 && sim.glass_tint && sim.glass_shade;
    let now = (sim.sun_dir, sim.god_rays, on, sim.shadow);
    if *last == Some(now) && made.is_empty() != on {
        return;
    }
    *last = Some(now);
    made.iter().for_each(|e| commands.entity(e).despawn());
    if !on {
        return;
    }
    let (az, el) = (sim.sun_dir[0].to_radians(), sim.sun_dir[1].clamp(3.0, 89.0).to_radians());
    let d = -Vec3::new(az.cos() * el.cos(), az.sin() * el.cos(), el.sin());
    let floor = |p: Vec3| p + d * (p.z - 0.004) / -d.z;
    let mut cols: Vec<(f32, Vec3, Vec3)> = vec![];
    for t in glass.iter().filter(|t| t.translation.z < SKY_POOL.z - 1.0) {
        let (c, h) = (t.translation, world::GLASS_HALF());
        match cols.iter_mut().find(|k| (k.0 - c.y).abs() < 0.01) {
            Some(k) => (k.1, k.2) = (k.1.min(c - h), k.2.max(c + h)),
            None => cols.push((c.y, c - h, c + h)),
        }
    }
    cols.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (lo, hi) = (cols.iter().fold(Vec3::MAX, |m, c| m.min(c.1)), cols.iter().fold(Vec3::MIN, |m, c| m.max(c.2)));
    let (x, top) = ((lo.x + hi.x) * 0.5, hi.z);
    let rgb = |y: f32| Vec3::from(GLASS_RGB[((y + 4.0) / 0.5) as usize % 16]);
    let blur = |z: f32| (z / -d.z * (sim.shadow.max(0.1)).to_radians().tan()).min(0.4);
    let run = |v: Vec3| v.with_z(0.0).length() / v.z.max(1e-3);
    let up = |z: f32| blur(z) / run(-d).max(0.2);
    let line = |w: f32| ((GLASS_FRAME.max(w)) * 0.5, 1.0 - (1.0 - GLASS_FRAME_K) * (GLASS_FRAME / GLASS_FRAME.max(w)));
    let (h, panes) = (world::GLASS_HALF() * 2.0, (top / (world::GLASS_HALF() * 2.0)).round() as usize);
    let mut rows: Vec<(f32, f32)> = vec![(0.0, 1.0)];
    for k in 1..panes {
        let z = k as f32 * h;
        let (hw, dk) = line(up(z));
        rows.extend([(z - hw, 1.0), (z, dk), (z + hw, 1.0)]);
    }
    let (hw, _) = line(up(top));
    rows.extend([(top - hw, 1.0), (top + hw, 0.0)]);
    let mut tp: Vec<[f32; 3]> = vec![];
    let mut tc: Vec<[f32; 4]> = vec![];
    let tint = |c: Vec3, k: f32| (Vec3::ONE.lerp(c, GLASS_TINT) * k).extend(1.0).to_array();
    let mut grid: Vec<Vec<(Vec3, [f32; 4])>> = vec![];
    for &(z, rk) in &rows {
        let (hw, dk) = line(blur(z.max(0.0)));
        let mut row = vec![];
        let mut at = |y: f32, c: Vec3, k: f32| row.push((floor(Vec3::new(x, y, z.max(0.004))), if rk == 0.0 { [1.0; 4] } else { tint(c, k * rk) }));
        at(cols[0].1.y - hw, Vec3::ONE, 1.0);
        at(cols[0].1.y, rgb(cols[0].0), dk);
        for (k, c) in cols.iter().enumerate() {
            at(c.1.y + hw, rgb(c.0), 1.0);
            at(c.2.y - hw, rgb(c.0), 1.0);
            match cols.get(k + 1) {
                Some(n) => at(c.2.y, (rgb(c.0) + rgb(n.0)) * 0.5, dk),
                None => {
                    at(c.2.y, rgb(c.0), dk);
                    at(c.2.y + hw, Vec3::ONE, 1.0);
                }
            }
        }
        grid.push(row);
    }
    for (r0, r1) in grid.iter().zip(&grid[1..]) {
        for k in 0..r0.len() - 1 {
            for (p, c) in [r0[k], r0[k + 1], r1[k + 1], r0[k], r1[k + 1], r1[k]] {
                tp.push(p.to_array());
                tc.push(c);
            }
        }
    }
    let (mut sp, mut sc): (Vec<[f32; 3]>, Vec<[f32; 4]>) = default();
    for &(y, lo, hi) in &cols {
        let c = rgb(y) * 0.06;
        let (a, b2, a0, b0) = (Vec3::new(x, lo.y, hi.z), Vec3::new(x, hi.y, hi.z), Vec3::new(x, lo.y, lo.z), Vec3::new(x, hi.y, lo.z));
        let (fa, fb, f0, f1) = (floor(a), floor(b2), floor(a0.with_z(0.004)), floor(b0.with_z(0.004)));
        for p in [a, b2, fb, a, fb, fa, a, fa, f0, a, f0, a0, b2, fb, f1, b2, f1, b0] {
            sp.push(p.to_array());
            let k = if p.z < 0.01 && (p - a0).length() > 0.01 && (p - b0).length() > 0.01 { 0.25 } else { 1.0 };
            sc.push((c * k).extend(1.0).to_array());
        }
    }
    let (mut tint, mut shaft) = (Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default()), Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default()));
    let n = |k: usize| vec![[0.0f32, 0.0, 1.0]; k];
    tint.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n(tp.len()));
    tint.insert_attribute(Mesh::ATTRIBUTE_COLOR, tc);
    tint.insert_attribute(Mesh::ATTRIBUTE_POSITION, tp);
    shaft.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n(sp.len()));
    shaft.insert_attribute(Mesh::ATTRIBUTE_COLOR, sc);
    shaft.insert_attribute(Mesh::ATTRIBUTE_POSITION, sp);
    let mut mat = |alpha_mode| mats.add(StandardMaterial { unlit: true, alpha_mode, double_sided: true, cull_mode: None, ..default() });
    commands.spawn((Mesh3d(meshes.add(tint)), MeshMaterial3d(mat(AlphaMode::Multiply)), Transform::default(), RenderLayers::layer(1), GlassLight,
                    bevy::light::NotShadowCaster));
    if sim.god_rays {
        commands.spawn((Mesh3d(meshes.add(shaft)), MeshMaterial3d(mat(AlphaMode::Add)), Transform::default(), RenderLayers::layer(1), GlassLight,
                        bevy::light::NotShadowCaster));
    }
}
#[derive(Component)]
struct PoolShade;
fn sky_pool_shade(mut commands: Commands, sim: NonSend<Sim>, made: Query<Entity, With<PoolShade>>, mut meshes: ResMut<Assets<Mesh>>,
                  mut mats: ResMut<Assets<StandardMaterial>>, mut last: Local<Option<([f32; 2], bool, f32, bool)>>) {
    let on = sim.room.stage == "pttest" && sim.rt_now() == 0 && sim.sky_shade;
    let now = (sim.sun_dir, on, sim.shadow, sim.glass_tint && sim.glass_shade);
    if *last == Some(now) && made.is_empty() != on {
        return;
    }
    *last = Some(now);
    made.iter().for_each(|e| commands.entity(e).despawn());
    if !on {
        return;
    }
    let (az, el) = (sim.sun_dir[0].to_radians(), sim.sun_dir[1].clamp(3.0, 89.0).to_radians());
    let d = -Vec3::new(az.cos() * el.cos(), az.sin() * el.cos(), el.sin());
    let (run, blur) = (SKY_POOL.z / -d.z, (SKY_POOL.z / -d.z * sim.shadow.max(0.1).to_radians().tan()).min(3.0));
    let c = (SKY_POOL + d * run).truncate();
    let through = [0.86f32, 0.96, 0.97].map(|k| k.powf(SKY_DEEP / -d.z / 20.0));
    let tint = Vec3::ONE.lerp(Vec3::from(through), 0.6);
    let ring = |g: f32, col: Vec3| [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(sx, sy)| (Vec3::new(c.x + sx * (SKY_HALF.x + g), c.y + sy * (SKY_HALF.y + g), 0.004), col.extend(1.0).to_array()));
    let (inner, outer) = (ring(-blur.min(SKY_HALF.y * 0.9), tint), ring(blur, Vec3::ONE));
    let (mut p, mut col) = (vec![], vec![]);
    let mut tri = |a: (Vec3, [f32; 4]), b: (Vec3, [f32; 4]), c: (Vec3, [f32; 4])| for v in [a, b, c] {
        p.push(v.0.to_array());
        col.push(v.1);
    };
    tri(inner[0], inner[1], inner[2]);
    tri(inner[0], inner[2], inner[3]);
    for k in 0..4 {
        let n = (k + 1) % 4;
        tri(inner[k], outer[k], outer[n]);
        tri(inner[k], outer[n], inner[n]);
    }
    if sim.glass_tint && sim.glass_shade {
        let n = (SKY_HALF.x / 0.5).round();
        let (w, y) = (SKY_HALF.x / n, SKY_HALF.y - blur.min(SKY_HALF.y * 0.9));
        for i in 0..n as usize {
            let col = Vec3::ONE.lerp(Vec3::from(GLASS_RGB[i % 16]), GLASS_TINT).extend(1.0).to_array();
            let q = [(0.0, -y), (w, -y), (w, y), (0.0, y)].map(|(u, v)| (Vec3::new(c.x + i as f32 * w + u, c.y + v, 0.005), col));
            tri(q[0], q[1], q[2]);
            tri(q[0], q[2], q[3]);
        }
    }
    let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; p.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
    commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mats.add(StandardMaterial { unlit: true, alpha_mode: AlphaMode::Multiply, double_sided: true, cull_mode: None, ..default() })),
                    Transform::default(), RenderLayers::layer(1), PoolShade, bevy::light::NotShadowCaster));
}
const GLASS_TINT: f32 = 0.4;
const GLASS_FRAME: f32 = 0.06;
const GLASS_FRAME_K: f32 = 0.7;
const GLASS_RGB: [[f32; 3]; 16] = [[0.95, 0.95, 0.95], [0.95, 0.22, 0.01], [0.57, 0.08, 0.51], [0.04, 0.45, 0.70], [0.99, 0.69, 0.05], [0.22, 0.57, 0.01],
    [0.90, 0.26, 0.40], [0.06, 0.08, 0.09], [0.34, 0.34, 0.31], [0.01, 0.33, 0.33], [0.25, 0.03, 0.48], [0.05, 0.06, 0.40], [0.23, 0.09, 0.03],
    [0.11, 0.20, 0.01], [0.43, 0.03, 0.02], [0.01, 0.01, 0.02]];
fn sky_transmittance(el: f32) -> Vec3 {
    let (r0, top) = (6_360_000.0f32, 100_000.0f32);
    let d = Vec2::new(el.to_radians().cos(), el.to_radians().sin());
    let len = -r0 * d.y + ((r0 * d.y).powi(2) + (r0 + top).powi(2) - r0 * r0).sqrt();
    let (n, medium) = (256, bevy::light::atmosphere::ScatteringMedium::default());
    let mut depth = Vec3::ZERO;
    for i in 0..n {
        let h = (Vec2::new(0.0, r0) + d * (i as f32 + 0.5) * len / n as f32).length() - r0;
        for t in &medium.terms {
            depth += (t.absorption + t.scattering) * t.falloff.sample(1.0 - h / top) * len / n as f32;
        }
    }
    (-depth).exp()
}

#[test]
fn sky_reddens_low_sun() {
    let (hi, lo) = (sky_transmittance(89.0), sky_transmittance(3.0));
    assert!(hi.x > 0.8 && hi.z < hi.x && hi.z > 0.5, "{hi}");
    assert!(lo.x < hi.x && lo.z < 0.2 * lo.x, "{lo}");
}

const SKY_FILL: Color = Color::srgb(0.66, 0.78, 1.0);
const GOD_RAYS_BOX: Vec3 = Vec3::new(40.0, 40.0, 12.0);
const GOD_RAYS_FOG: f32 = 0.15;

fn apply_look(sim: NonSend<Sim>, mut last: Local<Option<(Look, Entity, bool, f32, [f32; 2], bool, [f32; 3], f32, u8, u8)>>, mut sky: ResMut<ClearColor>, mut sun: Query<(&mut DirectionalLight, &mut Transform, Has<Fill>)>,
              mut fill: ResMut<GlobalAmbientLight>,
              floor: Query<(Entity, &MeshMaterial3d<StandardMaterial>), With<Floor>>, mut mats: ResMut<Assets<StandardMaterial>>,
              cam: Query<Entity, With<OursCam>>, mut commands: Commands) {
    bevy::solari::pathtracer::CAUSTIC_PHOTONS.store(sim.photons_now(), std::sync::atomic::Ordering::Relaxed);
    let (l, Ok((f, m))) = (sim.skin, floor.single()) else { return };
    if *last == Some((l, f, sim.soft, sim.shadow, sim.sun_dir, sim.shadows, sim.sun_color, sim.fill_k, sim.sky, sim.rt_now())) {
        return;
    }
    if last.is_some_and(|o| o.0 != l || o.4 != sim.sun_dir) {
        if let Some(p) = look_path() {
            let b = |x: bool| x as u8 as f32;
            let n = [l.sky[0], l.sky[1], l.sky[2], l.floor[0], l.floor[1], l.floor[2], b(l.grid), b(l.fog), l.sun, 0.0, b(l.glow), sim.sun_dir[0], sim.sun_dir[1]];
            let _ = std::fs::write(p, n.map(|x| x.to_string()).join(" "));
        }
    }
    *last = Some((l, f, sim.soft, sim.shadow, sim.sun_dir, sim.shadows, sim.sun_color, sim.fill_k, sim.sky, sim.rt_now()));
    fill.brightness = if sim.soft { 400.0 * sim.fill_k } else { GlobalAmbientLight::default().brightness };
    if sim.sky != 0 {
        fill.brightness = 0.0;
    }
    fill.color = if sim.soft { SKY_FILL } else { Color::WHITE };
    let [r, g, b] = l.sky;
    sky.0 = Color::srgb(r, g, b);
    let from = |az: f32, el: f32, r: f32| {
        let (az, el) = (az.to_radians(), el.clamp(3.0, 89.0).to_radians());
        Vec3::new(az.cos() * el.cos(), az.sin() * el.cos(), el.sin()) * r
    };
    *bevy::solari::scene::SUN_TINT.lock().unwrap() = if sim.sky == 2 { sky_transmittance(sim.sun_dir[1].clamp(3.0, 89.0)) } else { Vec3::ONE };
    for (mut s, mut t, fill) in &mut sun {
        *t = Transform::from_translation(if fill { from(sim.sun_dir[0] + 160.6, 22.0, 5.4) } else { from(sim.sun_dir[0], sim.sun_dir[1], 7.0) }).looking_at(Vec3::ZERO, Vec3::Z);
        let k = match (fill, sim.soft) { (false, _) => 1.0, (true, true) if sim.rt_now() == 0 && sim.sky == 0 => 0.3 * sim.fill_k, (true, _) => 0.0 };
        s.illuminance = bevy::light::light_consts::lux::AMBIENT_DAYLIGHT * l.sun * k;
        s.color = if fill { SKY_FILL } else { Color::srgb(sim.sun_color[0], sim.sun_color[1], sim.sun_color[2]) };
        s.soft_shadow_size = (!fill && sim.shadow > 0.0).then_some((sim.shadow * 0.5).to_radians().tan());
        if fill {
            s.shadow_maps_enabled = false;
        }
        s.contact_shadows_enabled = !fill && sim.contact > 0.0;
    }
    if sim.room.stage != "street" {
        if let Some(mut m) = mats.get_mut(&m.0) {
            let [r, g, b] = l.floor;
            m.base_color = Color::srgb(r, g, b);
        }
    }
    for c in &cam {
        if l.fog && sim.sky == 0 {
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
const EXTRAS: [(&str, &str); 9] = [("skatepark", "skate park"), ("highledge", "high ledge"), ("lights", "coloured lights hall"), ("bridge", "rope bridge"), ("trampoline", "trampoline"), ("quake", "earthquake"), ("bull", "mechanical bull"), ("cab", "cab door"), ("pttest", "path tracing test")];
const PBR: [(&str, &[u8]); 26] = [
    ("pbr/steel.ktx2", include_bytes!("pbr/steel.ktx2")),
    ("pbr/steel_n.ktx2", include_bytes!("pbr/steel_n.ktx2")),
    ("pbr/steel_mr.ktx2", include_bytes!("pbr/steel_mr.ktx2")),
    ("pbr/cobblestone.ktx2", include_bytes!("pbr/cobblestone.ktx2")),
    ("pbr/cobblestone_n.ktx2", include_bytes!("pbr/cobblestone_n.ktx2")),
    ("pbr/cobblestone_mr.ktx2", include_bytes!("pbr/cobblestone_mr.ktx2")),
    ("pbr/ice.ktx2", include_bytes!("pbr/ice.ktx2")),
    ("pbr/ice_n.ktx2", include_bytes!("pbr/ice_n.ktx2")),
    ("pbr/ice_mr.ktx2", include_bytes!("pbr/ice_mr.ktx2")),
    ("pbr/grass.ktx2", include_bytes!("pbr/grass.ktx2")),
    ("pbr/white_stained_glass.ktx2", include_bytes!("pbr/white_stained_glass.ktx2")),
    ("pbr/orange_stained_glass.ktx2", include_bytes!("pbr/orange_stained_glass.ktx2")),
    ("pbr/magenta_stained_glass.ktx2", include_bytes!("pbr/magenta_stained_glass.ktx2")),
    ("pbr/light_blue_stained_glass.ktx2", include_bytes!("pbr/light_blue_stained_glass.ktx2")),
    ("pbr/yellow_stained_glass.ktx2", include_bytes!("pbr/yellow_stained_glass.ktx2")),
    ("pbr/lime_stained_glass.ktx2", include_bytes!("pbr/lime_stained_glass.ktx2")),
    ("pbr/pink_stained_glass.ktx2", include_bytes!("pbr/pink_stained_glass.ktx2")),
    ("pbr/gray_stained_glass.ktx2", include_bytes!("pbr/gray_stained_glass.ktx2")),
    ("pbr/light_gray_stained_glass.ktx2", include_bytes!("pbr/light_gray_stained_glass.ktx2")),
    ("pbr/cyan_stained_glass.ktx2", include_bytes!("pbr/cyan_stained_glass.ktx2")),
    ("pbr/purple_stained_glass.ktx2", include_bytes!("pbr/purple_stained_glass.ktx2")),
    ("pbr/blue_stained_glass.ktx2", include_bytes!("pbr/blue_stained_glass.ktx2")),
    ("pbr/brown_stained_glass.ktx2", include_bytes!("pbr/brown_stained_glass.ktx2")),
    ("pbr/green_stained_glass.ktx2", include_bytes!("pbr/green_stained_glass.ktx2")),
    ("pbr/red_stained_glass.ktx2", include_bytes!("pbr/red_stained_glass.ktx2")),
    ("pbr/black_stained_glass.ktx2", include_bytes!("pbr/black_stained_glass.ktx2")),
];
const GLASS: [&str; 16] = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];
const SNAP_CHUNK: u64 = 0xB41D6E;
const PLAT_CHUNK: u64 = 0xB41D70;
const BRIDGE_CHUNK: u64 = 0xB41D00;
const STAGES: [(&str, &str); 6] = [("room", "room"), ("bigstairs", "big stairs"), ("rooftop", "rooftop"), ("lowwall", "low wall"), ("rail", "rail"), ("street", "street")];

#[derive(Component)]
struct PropMesh(usize, usize);

fn sounds(mut sim: NonSendMut<Sim>, mut commands: Commands, mut bank: Option<ResMut<sfx::Bank>>, mut assets: ResMut<Assets<sfx::Sfx>>) {
    #[cfg(feature = "portal")]
    if let Some(b) = bank.as_mut() {
        portal::sounds(&mut sim, &mut assets, b);
    }
    let (mut q, vol) = (std::mem::take(&mut sim.sounds), if sim.sound { sim.volume } else { [0.0; 3] });
    sfx::play(&mut commands, &mut bank, &mut q, vol);
}

fn blood_view(sim: NonSend<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
              mut images: ResMut<Assets<Image>>, mut q: Query<(&BloodPix, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>, &mut Mesh3d)>,
              mut made: Local<Vec<Handle<StandardMaterial>>>, mut shapes: Local<Vec<Handle<Mesh>>>) {
    if made.is_empty() {
        let shades = [(0.75, 0.02, 0.03), (0.55, 0.0, 0.02), (0.95, 0.12, 0.08)];
        *made = shades.map(|(r, g, b)| mats.add(StandardMaterial { base_color: Color::srgb(r, g, b), unlit: true, ..default() })).to_vec();
        shapes.push(meshes.add(Cuboid::from_length(BLOOD_SIZE)));
        for k in 0..SPLATS * 2 {
            let (mesh, image) = splat_mesh(k as u32, k >= SPLATS);
            shapes.push(meshes.add(mesh));
            made.push(mats.add(StandardMaterial { base_color_texture: Some(images.add(image)), perceptual_roughness: 0.06, reflectance: 0.5, ..default() }));
        }
        for k in 0..BLOOD_MAX {
            commands.spawn((Mesh3d(shapes[0].clone()), MeshMaterial3d(made[k % 3].clone()), Transform::default(), Visibility::Hidden, BloodPix(k)));
        }
    }
    for (p, mut t, mut v, mut m, mut shape) in &mut q {
        let Some(&(at, vel, age, kind, n)) = sim.blood.get(p.0) else {
            *v = Visibility::Hidden;
            continue;
        };
        let landed = vel == Vec3::ZERO;
        let pool = kind == BLOOD_POOL;
        let fade = (1.0 - (age - (if landed && sim.gore { BLOOD_POOL_LIFE } else { BLOOD_LIFE } - 1.0)).max(0.0)).clamp(0.05, 1.0);
        let id = if landed { ((at.x * 1000.0) as i32 ^ (at.y * 1000.0) as i32 * 31 ^ (at.z * 1000.0) as i32 * 17) as usize } else { p.0 };
        let g = ((age - 0.3) / if pool { 3.0 } else { 1.2 }).clamp(0.0, 1.0);
        let spread = 0.35 + 0.65 * (1.0 - (1.0 - g) * (1.0 - g));
        *t = if landed {
            Transform::from_translation(at + n * 0.0004 * (id % 8) as f32)
                .with_rotation(Quat::from_rotation_arc(Vec3::Z, n) * Quat::from_rotation_z((id % 4) as f32 * std::f32::consts::FRAC_PI_2))
                .with_scale(Vec3::new(spread, spread, 1.0) * BLOOD_SPLAT * if pool { 1.4 } else if kind == BLOOD_DROP || !sim.gore { 0.35 } else { 1.0 } * fade)
        } else {
            Transform::from_translation(at).with_scale(Vec3::splat(fade))
        };
        let k = if landed { 1 + id % SPLATS + SPLATS * pool as usize } else { 0 };
        if shape.0 != shapes[k] {
            shape.0 = shapes[k].clone();
        }
        let want = &made[if landed { 2 + k } else { p.0 % 3 }];
        if m.0 != *want {
            m.0 = want.clone();
        }
        *v = Visibility::Visible;
    }
}

fn splat_mesh(seed: u32, pool: bool) -> (Mesh, Image) {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let n = SPLAT_PX * (1 + pool as usize);
    let size = BLOOD_SPLAT * (1 + pool as usize) as f32;
    let mut s = seed.wrapping_mul(2654435761).wrapping_add(12345 + pool as u32 * 777);
    let mut r = move || {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        (s >> 8) as f32 / (1u32 << 24) as f32
    };
    let (base, lump, reach) = if pool { (0.34, 0.12, 0.4) } else { (0.24, 0.08, 1.0) };
    let waves: Vec<(f32, f32, f32)> = (0..3).map(|k| ((2 + k * 2) as f32 + (r() * 2.0).floor(), lump / (1.0 + k as f32 * 0.5) * (0.5 + r()), r() * 6.28)).collect();
    let spikes: Vec<(f32, f32, f32)> = (0..3 + (r() * 4.0) as usize).map(|_| (r() * 6.28, (0.08 + r() * 0.15) * reach, 0.12 + r() * 0.12)).collect();
    let blob = |a: f32| {
        let w: f32 = waves.iter().map(|&(f, amp, ph)| amp * (f * a + ph).sin()).sum();
        let sp: f32 = spikes.iter().map(|&(at, len, wd)| {
            let d = ((a - at + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs();
            len * (-(d / wd).powi(2)).exp()
        }).sum();
        (base * (1.0 + w) + sp).min(0.46)
    };
    let mut drops: Vec<(Vec2, f32)> = vec![];
    for &(at, len, _) in &spikes {
        for k in 0..1 + (r() * 2.5) as usize {
            let c = Vec2::from_angle(at + (r() - 0.5) * 0.4) * (base + len + 0.08 + k as f32 * 0.08);
            if c.length() < 0.47 {
                drops.push((c, (0.045 + r() * 0.04) / (1 + pool as usize) as f32));
            }
        }
    }
    let inside: Vec<Vec<bool>> = (0..n).map(|y| (0..n).map(|x| {
        let c = Vec2::new((x as f32 + 0.5) / n as f32 - 0.5, (y as f32 + 0.5) / n as f32 - 0.5);
        c.length() < blob(c.to_angle()) || drops.iter().any(|&(d, rad)| c.distance(d) < rad)
    }).collect()).collect();
    let mut depth: Vec<Vec<u8>> = inside.iter().map(|row| row.iter().map(|&b| b as u8 * 4).collect()).collect();
    for level in 1..4u8 {
        let was = depth.clone();
        let at = |x: i32, y: i32| was.get(y as usize).and_then(|r| r.get(x as usize)).copied().unwrap_or(0);
        for y in 0..n as i32 {
            for x in 0..n as i32 {
                if at(x, y) > level && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| at(x + dx, y + dy) < level) {
                    depth[y as usize][x as usize] = level;
                }
            }
        }
    }
    let height = |x: i32, y: i32| depth.get(y as usize).and_then(|r| r.get(x as usize)).map_or(0.0, |&d| d as f32 * 0.0016 / size);
    let (mut pos, mut nor, mut uv, mut idx, mut data) = (vec![], vec![], vec![], vec![], vec![0u8; n * n * 4]);
    let cell = 1.0 / n as f32;
    let mut quad = |c: [[f32; 3]; 4], nm: [f32; 3], t: [f32; 2]| {
        let base = pos.len() as u32;
        pos.extend(c);
        nor.extend([nm; 4]);
        uv.extend([t; 4]);
        idx.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    };
    for y in 0..n {
        for x in 0..n {
            let d = depth[y][x];
            if d == 0 {
                continue;
            }
            let [cr, cg, cb] = if d > 1 && r() < 0.05 { [165, 16, 18] } else { [[40u8, 0, 2], [125, 3, 9], [104, 0, 6], [86, 0, 5]][d as usize - 1] };
            data[(y * n + x) * 4..][..4].copy_from_slice(&[cr, cg, cb, 255]);
            let (x0, y0, x1, y1) = (x as f32 * cell - 0.5, y as f32 * cell - 0.5, (x + 1) as f32 * cell - 0.5, (y + 1) as f32 * cell - 0.5);
            let (xi, yi) = (x as i32, y as i32);
            let h = height(xi, yi);
            let t = [(x as f32 + 0.5) * cell, (y as f32 + 0.5) * cell];
            quad([[x0, y0, h], [x1, y0, h], [x1, y1, h], [x0, y1, h]], [0.0, 0.0, 1.0], t);
            let lo = |dx, dy| height(xi + dx, yi + dy).min(h);
            if lo(1, 0) < h { let l = lo(1, 0); quad([[x1, y0, l], [x1, y1, l], [x1, y1, h], [x1, y0, h]], [1.0, 0.0, 0.0], t); }
            if lo(-1, 0) < h { let l = lo(-1, 0); quad([[x0, y1, l], [x0, y0, l], [x0, y0, h], [x0, y1, h]], [-1.0, 0.0, 0.0], t); }
            if lo(0, 1) < h { let l = lo(0, 1); quad([[x1, y1, l], [x0, y1, l], [x0, y1, h], [x1, y1, h]], [0.0, 1.0, 0.0], t); }
            if lo(0, -1) < h { let l = lo(0, -1); quad([[x0, y0, l], [x1, y0, l], [x1, y0, h], [x0, y0, h]], [0.0, -1.0, 0.0], t); }
        }
    }
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(bevy::mesh::Indices::U32(idx));
    let mut image = Image::new(Extent3d { width: n as u32, height: n as u32, depth_or_array_layers: 1 }, TextureDimension::D2, data,
                               TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    image.sampler = bevy::image::ImageSampler::nearest();
    (mesh, image)
}

#[derive(Component)]
struct StandIn;

#[derive(Component)]
struct Bulb(Entity);

fn bulbs(mut commands: Commands, sim: NonSend<Sim>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
         lights: Query<(Entity, &PointLight, &InheritedVisibility, Option<&Bulb>), (Without<Mesh3d>, Without<StandIn>)>, mut mesh: Local<Option<Handle<Mesh>>>) {
    const R: f32 = 0.05;
    for (e, light, shown, bulb) in &lights {
        let want = sim.rt_now() > 0 && shown.get();
        match (want, bulb) {
            (true, None) => {
                let m = mesh.get_or_insert_with(|| {
                    let mut m = Sphere::new(R).mesh().ico(2).unwrap();
                    let _ = m.generate_tangents();
                    if let Some(bevy::mesh::Indices::U16(i)) = m.indices() {
                        let i = i.iter().map(|&x| x as u32).collect();
                        m.insert_indices(bevy::mesh::Indices::U32(i));
                    }
                    meshes.add(m)
                }).clone();
                let nits = light.intensity / (std::f32::consts::PI * 4.0 * std::f32::consts::PI * R * R);
                let mat = mats.add(StandardMaterial { base_color: Color::BLACK, emissive: light.color.to_linear() * nits, ..default() });
                let b = commands.spawn((bevy::solari::prelude::RaytracingMesh3d(m), MeshMaterial3d(mat), Transform::default(), ChildOf(e))).id();
                commands.entity(e).insert(Bulb(b));
            }
            (false, Some(b)) => {
                commands.entity(b.0).despawn();
                commands.entity(e).remove::<Bulb>();
            }
            _ => {}
        }
    }
}

fn glow_view(sim: NonSend<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
             mut q: Query<(&Glow, &mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>, &mut PointLight)>, time: Res<Time>, mut made: Local<bool>) {
    if !*made {
        *made = true;
        let mesh = meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap());
        for k in 0..GLOW_MAX {
            commands.spawn((Glow(k), Mesh3d(mesh.clone()), MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::BLACK, ..default() })),
                            PointLight { range: 12.0, ..default() }, bevy::light::NotShadowCaster, Transform::default(), Visibility::Hidden));
        }
        return;
    }
    let t = time.elapsed_secs();
    let flick = |k: f32| 0.8 + 0.2 * (t * 11.0 + k * 1.7).sin() * (t * 7.3 + k).cos();
    let ball = sim.rt_now() > 0 || sim.fire == 0;
    let dark = !sim.fire_light;
    let blasts = sim.gun.blasts.iter().map(|&(at, age)| (at, BLAST_R * 0.25 * (age / 0.3).min(1.0).max(0.1), 2.0 * (1.0 - age / 0.8).max(0.0)));
    let fires = sim.gun.fires.iter().filter(|_| !dark).enumerate().map(|(k, &(at, age))| (at + Vec3::Z * 0.25, if ball { 0.4 } else { 0.001 }, 0.3 * (1.0 - age / FIRE_LIFE).clamp(0.25, 1.0) * flick(k as f32)));
    let chest = sim.ours().p(0) + Vec3::Z * 0.3;
    let round = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y];
    let burning: Vec<_> = match (sim.burning > 0.0 && !dark, sim.fire_homer) {
        (false, _) => vec![],
        (true, false) => vec![(chest, if ball { 0.25 } else { 0.001 }, 0.2 * flick(9.0))],
        (true, true) if ball => [Vec3::X, -Vec3::X].iter().enumerate().map(|(k, d)| (chest + *d * FIRE_LIGHT_OUT, 0.25, 0.2 * flick(9.0 + k as f32))).collect(),
        (true, true) => round.iter().enumerate().map(|(k, d)| (chest + *d * FIRE_LIGHT_OUT, 0.001, 0.08 * flick(9.0 + k as f32))).collect(),
    };
    let crowd: Vec<_> = sim.crowd.iter().zip(&sim.bench).filter(|(_, b)| ball && !dark && b.burning > 0.0).enumerate()
        .flat_map(|(k, (c, _))| [Vec3::X, -Vec3::X].map(|d| (c.0.doll.com().0 + d * FIRE_LIGHT_OUT + Vec3::Z * 0.2, 0.25, 0.2 * flick(5.0 + k as f32)))).collect();
    let lit: Vec<_> = blasts.chain(burning).chain(crowd).chain(fires).take(GLOW_MAX).collect();
    for (g, mut tr, mut v, m, mut light) in &mut q {
        let Some(&(at, r, b)) = lit.get(g.0) else {
            *v = Visibility::Hidden;
            continue;
        };
        let warm = LinearRgba::rgb(1.0, 0.42, 0.1);
        let (r, e) = if sim.rt_now() > 0 && r > 0.01 { (r * GLOW_SHRINK, b / (GLOW_SHRINK * GLOW_SHRINK)) } else { (r, b) };
        *tr = Transform::from_translation(at).with_scale(Vec3::splat(r));
        if let Some(mut m) = mats.get_mut(&m.0) {
            m.emissive = warm * 400_000.0 * e;
        }
        light.color = Color::LinearRgba(warm);
        light.intensity = 600_000.0 * b;
        *v = Visibility::Visible;
    }
}

fn fire_view(sim: NonSend<Sim>, time: Res<Time>, cam: Query<&GlobalTransform, With<OursCam>>,
             mut q: Query<(&mut fire::Fire, &Mesh3d, &MeshMaterial3d<fire::FlameMaterial>, &mut Visibility)>, mut meshes: ResMut<Assets<Mesh>>,
             mut mats: ResMut<Assets<fire::FlameMaterial>>, mut warm: Local<u32>, mut spills: Local<(Vec<(Vec3, f32)>, f32, u32)>, mut steamed: Local<u32>) {
    let (Ok(cam), Ok((mut f, m, mat, mut v))) = (cam.single(), q.single_mut()) else { return };
    if *warm < 30 {
        *warm += 1;
        *v = Visibility::Inherited;
        return;
    }
    if sim.fire == 0 {
        if *v != Visibility::Hidden {
            *v = Visibility::Hidden;
        }
        return;
    }
    let voxel = if sim.real_fire { 0.0 } else { 1.0 };
    if mats.get(&mat.0).is_some_and(|x| x.extension.k.y != voxel) {
        if let Some(mut x) = mats.get_mut(&mat.0) {
            x.extension.k.y = voxel;
        }
    }
    let mut burns: Vec<fire::Source> = sim.gun.fires.iter().map(|&(at, age)| fire::Source { at, r: FIRE_R * 0.8, k: ((FIRE_LIFE - age) / 2.0).clamp(0.0, 1.0) }).collect();
    let dt = time.delta_secs();
    let (spilt, due, n) = &mut *spills;
    spilt.retain_mut(|(_, age)| {
        *age += dt;
        *age < SPILL_LIFE
    });
    let ours = sim.ours();
    let burners: Vec<(f32, Vec<(Vec3, f32)>)> = (0..=sim.bench.len()).filter_map(|h| {
        let (at, build, burning): (Vec<Vec3>, _, _) = match h {
            0 => ((0..ours.parts.len()).map(|k| ours.p(k)).collect(), &sim.build, sim.burning),
            _ => (sim.crowd[h - 1].0.doll.frames().map(|f| f.translation).collect(), &sim.bench[h - 1].build, sim.bench[h - 1].burning),
        };
        let r = |k: usize| 0.12 * build.parts.get(k).zip(sim.base.0.parts.get(k)).map_or(1.0, |(p, q)| p.radius / q.radius);
        (burning > 0.0 && !at.is_empty()).then(|| (burning, at.into_iter().enumerate().map(|(k, p)| (p, r(k))).collect()))
    }).collect();
    if !burners.is_empty() {
        for (burning, parts) in &burners {
            burns.extend(parts.iter().map(|&(at, r)| fire::Source { at, r, k: 0.7 * (burning / 2.0).min(1.0) }));
        }
        *due += dt * SPILL_RATE * burners.len() as f32;
        while *due >= 1.0 {
            *due -= 1.0;
            *n += 1;
            let hash = |salt: f32| ((*n as f32 * 12.9898 + salt).sin() * 43758.545).rem_euclid(1.0);
            let parts = &burners[(hash(2.3) * burners.len() as f32) as usize % burners.len()].1;
            let floor = parts.iter().map(|p| p.0.z).fold(f32::MAX, f32::min) - 0.05;
            let at = parts[(hash(0.0) * parts.len() as f32) as usize % parts.len()].0;
            let a = hash(4.1) * std::f32::consts::TAU;
            spilt.push((Vec3::new(at.x + a.cos() * 0.35 * hash(7.7), at.y + a.sin() * 0.35 * hash(7.7), floor), 0.0));
        }
    } else {
        *due = 0.0;
    }
    burns.extend(spilt.iter().map(|&(at, age)| fire::Source { at, r: 0.1, k: 0.6 * (1.0 - age / SPILL_LIFE) }));
    if *steamed != sim.steam.0 {
        *steamed = sim.steam.0;
        fire::steam(&mut f, &sim.steam.1);
    }
    let Some(mut mesh) = meshes.get_mut(&m.0) else { return };
    let any = fire::step(&mut f, &mut mesh, &burns, time.delta_secs(), cam, !sim.real_fire, sim.smoke);
    let want = if any { Visibility::Inherited } else { Visibility::Hidden };
    if *v != want {
        *v = want;
    }
}

fn props(mut sim: NonSendMut<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
         mut images: ResMut<Assets<Image>>, mut q: Query<(Entity, &PropMesh, &mut Transform)>, mut made: Local<usize>) {
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
    for j in *made..sim.room.objs.len() {
        let kind = sim.room.objs[j].kind;
        let (_, s, _, _) = (*world::KINDS())[kind];
        #[cfg(feature = "portal")]
        if kind == 1 && let Some((mesh, mat, fit)) = portal::cube(&mut sim, &mut meshes, &mut mats, &mut images) {
            for i in 0..SIDES {
                commands.spawn((sim.room.obj(i, j).0, Visibility::default(), PropMesh(j, i))).with_children(|c| {
                    c.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_scale(Vec3::splat(s)) * fit, RenderLayers::layer(1 + i)));
                });
            }
            continue;
        }
        let o = &sim.room.objs[j];
        let mesh = match o.kind {
            0 | 4 => meshes.add(Sphere::new(s)),
            3 => meshes.add(Cylinder::new(s, world::BARREL_HALF() * 2.0)),
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
    *made = sim.room.objs.len();
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
    let bat = sim.hero.is_none() && bat(sim.weapon);
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
        let now = Transform::from_translation(at).looking_to(shown, up);
        if let Some((w, kept)) = sim.kept_gun.as_mut() && *w == m.0 {
            *t = *kept.get_or_insert(now);
            *v = Visibility::Visible;
            continue;
        }
        *v = if on && m.0 == sim.weapon { Visibility::Visible } else { Visibility::Hidden };
        *t = now;
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
            g.rect(iso, Vec2::splat(2.0 * PELLET_SPREAD() * acc * reach), Color::srgba(1.0, 0.6, 0.3, 0.4));
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
    for &(at, age) in sim.gun.fires.iter().filter(|_| sim.fire == 0) {
        let a = (1.0 - age / FIRE_LIFE).clamp(0.25, 1.0);
        for k in 0..6 {
            let s = (flick + k as f32 * 1.7).sin() * 0.5 + 0.5;
            g.circle(Isometry3d::from_translation(at + Vec3::Z * (0.02 + 0.12 * k as f32 * (0.5 + s))), FIRE_R * (1.0 - 0.14 * k as f32), Color::srgba(1.0, 0.35 + 0.1 * k as f32, 0.1, a));
        }
    }
    if sim.burning > 0.0 && sim.fire == 0 {
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
        commands.entity(e).try_insert(Smooth { prev: *t, cur: *t, step: sim.steps });
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
            .with_child((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset("embedded://taxi.glb"))), Transform::from_xyz(0.0, -0.071, -TAXI_RADIUS)));
    }
    let car_of = |e: Entity| up.iter_ancestors(e).find_map(|a| q.get(a).ok().map(|m| m.1.0));
    for e in &meshes {
        if car_of(e).is_some() {
            commands.entity(e).try_insert(RenderLayers::layer(1));
        }
    }
    for (e, name, t) in &named {
        if let (Some(k), Some(i)) = (car_of(e), WHEELS.iter().position(|w| name.as_str() == *w)) {
            commands.entity(e).try_insert(TaxiWheel(k, i, t.translation));
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
            .with_child((WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset("embedded://taxi.glb"))), Transform::from_xyz(0.0, 0.0, -0.757)));
    }
    for e in &meshes {
        if up.iter_ancestors(e).any(|a| q.contains(a)) {
            commands.entity(e).try_insert(RenderLayers::layer(1));
        }
    }
}

fn keys(mut sim: NonSendMut<Sim>, k: Res<ButtonInput<KeyCode>>, mut contexts: EguiContexts) {
    if sim.typing || contexts.ctx_mut().is_ok_and(|ctx| ctx.egui_wants_keyboard_input()) {
        return;
    }
    if k.just_pressed(KeyCode::KeyJ) {
        sim.toggle_skate();
    }
    if k.just_pressed(KeyCode::Space) && !sim.riding() {
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
    if k.just_pressed(KeyCode::KeyR) && !sim.riding() {
        sim.button(if k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight) { "RESET here" } else { "RESET pose" });
    }
    if k.just_pressed(KeyCode::KeyF) && !sim.riding() {
        sim.arrows = !sim.arrows;
    }
    if k.just_pressed(KeyCode::KeyX) && !sim.riding() {
        sim.button("STOP all");
    }
    if k.just_pressed(KeyCode::KeyN) {
        let b = if k.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
            format!("spawn {}", BODIES[sim.spawn_body].0)
        } else if k.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
            "remove homer".into()
        } else {
            "next homer".into()
        };
        sim.button(&b);
    }
    if k.just_pressed(KeyCode::KeyH) {
        sim.show_ui = !sim.show_ui;
    }
}

fn console(mut sim: NonSendMut<Sim>, mut typed: MessageReader<KeyboardInput>, mut contexts: EguiContexts) {
    if !sim.typing && contexts.ctx_mut().is_ok_and(|ctx| ctx.egui_wants_keyboard_input()) {
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

const VIEW_RECT: &str = "view rect";

fn viewports(mut sim: NonSendMut<Sim>, mut contexts: EguiContexts, windows: Query<&Window>, mut cams: Query<(Entity, &mut Camera, Has<OursCam>), RoomCameras>,
             mut commands: Commands, mut images: ResMut<Assets<Image>>, mut shown: Query<(&mut Transform, &mut Visibility, &MeshMaterial2d<ScaledMat>), With<ScaledView>>,
             mut scaled: ResMut<Assets<ScaledMat>>,
             mut target: Local<Option<(Handle<Image>, UVec2)>>) {
    let (Ok(ctx), Ok(window)) = (contexts.ctx_mut(), windows.single()) else { return };
    let rect = match ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(VIEW_RECT))) { Some(r) if sim.show_ui => r, _ => ctx.content_rect() };
    if rect.width() < 2.0 || rect.height() < 1.0 || window.physical_width() == 0 || window.physical_height() == 0 { return; }
    let win = window.physical_size();
    let s = if (sim.render_scale - 1.0).abs() > 0.001 && !sim.upscaling() { sim.render_scale } else { 1.0 };
    let size = (win.as_vec2() * s).as_uvec2().clamp(UVec2::ONE, UVec2::splat(8192));
    let want = (s != 1.0).then_some(size);
    let mut bound = want.unwrap_or(win);
    if target.as_ref().map(|t| t.1) != want {
        bound = bound.min(target.as_ref().map_or(win, |t| t.1));
        let to = match want {
            Some(size) => {
                let h = images.add(Image::new_target_texture(size.x, size.y, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, None));
                *target = Some((h.clone(), size));
                bevy::camera::RenderTarget::Image(bevy::camera::ImageRenderTarget { handle: h, scale_factor: window.scale_factor() * s })
            }
            None => {
                *target = None;
                bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Primary)
            }
        };
        for (e, ..) in &cams {
            commands.entity(e).insert(to.clone());
        }
    }
    if let Ok((mut at, mut vis, mat)) = shown.single_mut() {
        *vis = if target.is_some() { Visibility::Visible } else { Visibility::Hidden };
        if let Some((h, size)) = &*target {
            at.scale = window.size().extend(1.0);
            let fsr = sim.fsr1();
            let sharp = if fsr && sim.sharpen > 0.0 { (-2.0 * (1.0 - sim.sharpen)).exp2() } else { 0.0 };
            let p = Vec4::new(size.x as f32, size.y as f32, sharp, if fsr { 1.0 } else { 0.0 });
            if scaled.get(&mat.0).is_some_and(|m| m.p != p || m.img != *h) {
                if let Some(mut m) = scaled.get_mut(&mat.0) {
                    (m.p, m.img) = (p, h.clone());
                }
            }
        }
    }
    for (_, mut camera, ours) in &mut cams {
        camera.is_active = ours || sim.split();
        let mut v = room_viewport(rect, ctx.pixels_per_point() * s, sim.split(), !ours);
        v.physical_position = v.physical_position.min(bound - 1);
        v.physical_size = v.physical_size.min(bound - v.physical_position).max(UVec2::ONE);
        if ours {
            sim.view_px = v.physical_size;
        }
        camera.viewport = Some(v);
    }
}

fn skate_cam(mut sim: NonSendMut<Sim>, mut commands: Commands, mut cam: Query<(&mut Transform, &Orbit, &mut Projection), With<OursCam>>) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    let Ok((mut t, o, mut proj)) = cam.single_mut() else { return };
    let shot = sim.skate.as_ref().filter(|s| s.riding && o.follow == 0 && !sim.detached && sim.cam_rig.is_none())
        .and_then(|s| s.pose.as_ref()?.camera.map(|(at, b, fov)| (s.room(at), s.room_v(b.z_axis), s.room_v(b.y_axis), fov)));
    let mut fov = shot.map_or(std::f32::consts::FRAC_PI_4, |v| v.3.to_radians());
    if let Some((at, ahead, up, _)) = shot {
        *t = Transform::from_translation(at).looking_to(ahead, up);
    }
    if let Some(b) = sim.cam2_b.take() {
        let (from, at) = sim.cam2.unwrap_or_default();
        *t = Transform::from_translation(from).looking_at(at, Vec3::Z);
        fov = std::f32::consts::FRAC_PI_4;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(b));
    } else if !sim.cam2_queue.is_empty() {
        let a = sim.cam2_queue.remove(0);
        sim.cam2_b = Some(a.rsplit_once('/').map_or(format!("b{a}"), |(d, f)| format!("{d}b/{f}")));
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(a));
    }
    if matches!(&*proj, Projection::Perspective(p) if p.fov != fov) && let Projection::Perspective(p) = proj.as_mut() {
        p.fov = fov;
    }
}

fn mouse(mut sim: NonSendMut<Sim>, mut contexts: EguiContexts, buttons: Res<ButtonInput<MouseButton>>, mut last: Local<Option<Vec2>>, mut notches: Local<f32>, scroll: Res<AccumulatedMouseScroll>,
         keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, windows: Query<&Window>,
         motion: Res<bevy::input::mouse::AccumulatedMouseMotion>, mut grab: Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
         mut cam: Query<(&mut Transform, &mut Orbit, &Camera, &GlobalTransform), With<OursCam>>,
          mut rig: Local<Option<(Vec3, Vec3)>>, mut rig_steps: Local<(u64, [Vec3; 5], [Vec3; 5])>) {
    let editing = sim.typing || contexts.ctx_mut().is_ok_and(|ctx| ctx.egui_wants_keyboard_input());
    let Ok((mut t, mut o, camera, gt)) = cam.single_mut() else { return };
    let filming = sim.film > 0.0 && !sim.photo;
    let ots = portal_gun(&sim) && sim.hero.is_some() && !sim.detached && !sim.driving && sim.hero_doll.is_none() && !filming && !sim.photo;
    let locked = grab.single().is_ok_and(|c| c.grab_mode != bevy::window::CursorGrabMode::None);
    let flying = sim.detached && buttons.pressed(MouseButton::Right) && (locked || !sim.over_ui);
    if let Ok(mut c) = grab.single_mut() {
        let lock = flying || ots && !keys.just_pressed(KeyCode::Escape) && (locked || buttons.just_pressed(MouseButton::Left) && !sim.over_ui);
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
    let lines = match scroll.unit {
        bevy::input::mouse::MouseScrollUnit::Line => scroll.delta.y,
        bevy::input::mouse::MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    }.clamp(-3.0, 3.0);
    let hero_wheel = sim.hero.is_some() && !sim.driving;
    let wheel = lines * sim.sens * !(filming || hero_wheel) as u8 as f32;
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
    let view = |o: &Orbit| Vec3::new(o.yaw.cos() * o.pitch.cos(), o.yaw.sin() * o.pitch.cos(), o.pitch.sin());
    if flying {
        let eye = o.target + view(&o) * o.dist;
        let k = 0.0028;
        o.yaw -= motion.delta.x * k;
        o.pitch = (o.pitch + motion.delta.y * k).clamp(-1.5, 1.5);
        o.target = eye - view(&o) * o.dist;
    }
    let flat = Vec3::new(o.yaw.cos(), o.yaw.sin(), 0.0);
    let side = Vec3::Z.cross(flat);
    if !sim.over_ui {
        if ots_look {
            let k = (0.022 * 3.0f32).to_radians();
            o.yaw -= motion.delta.x * k;
            o.pitch = (o.pitch + motion.delta.y * k).clamp(-0.6, 1.2);
        } else if buttons.pressed(MouseButton::Right) && !sim.detached {
            o.yaw -= delta.x * 0.005;
            o.pitch = (o.pitch + delta.y * 0.005).clamp(if sim.photo { -1.5 } else { -0.2 }, 1.5);
        }
        if buttons.pressed(MouseButton::Middle) && delta != Vec2::ZERO && !sim.riding() {
            let k = o.dist * 0.0015;
            o.target += side * delta.x * k + Vec3::Z * delta.y * k;
            o.follow = 3;
            sim.detached = true;
        }
        if !sim.detached {
            o.dist = (o.dist * (1.0 - wheel * 0.1)).clamp(0.3, 40.0);
        }
    }
    sim.walk_dir = sim.script_walk;
    sim.preview_keys = ((!editing && !sim.detached && keys.pressed(KeyCode::KeyW)) || sim.script_walk != Vec3::ZERO, shift_held(&keys) || sim.script_jog);
    if !editing {
        let mut mv = Vec3::ZERO;
        if keys.just_pressed(KeyCode::Tab) {
            sim.button("detached cam");
        }
        for (k, d) in [(KeyCode::KeyW, -flat), (KeyCode::KeyS, flat), (KeyCode::KeyA, -side), (KeyCode::KeyD, side), (KeyCode::KeyE, Vec3::Z), (KeyCode::KeyQ, -Vec3::Z)] {
            if keys.pressed(k) {
                mv += d;
            }
        }
        let fly = if sim.detached {
            mv = Vec3::ZERO;
            let fwd = -view(&o);
            let go: Vec3 = [(KeyCode::KeyW, fwd), (KeyCode::KeyS, -fwd), (KeyCode::KeyD, side), (KeyCode::KeyA, -side), (KeyCode::KeyE, Vec3::Z), (KeyCode::KeyQ, -Vec3::Z)]
                .into_iter().filter(|k| flying && keys.pressed(k.0)).map(|k| k.1).sum();
            go.normalize_or_zero()
        } else {
            Vec3::ZERO
        };
        if sim.riding() {
            let (mouse, free) = ((buttons.pressed(MouseButton::Middle) && !sim.over_ui).then_some(motion.delta), !sim.detached);
            let s = sim.skate.as_mut().expect("riding");
            let c = s.controls(&|k| keys.pressed(k) && free, mouse, time.delta_secs());
            let dt = if sim.paused { 0.0 } else { time.delta_secs() * sim.speed };
            sim.play_skate(dt, c);
            if sim.held && dt > 0.0 {
                sim.meat_tick(dt);
            }
            if let Some((at, turn)) = sim.skate.as_mut().and_then(|s| s.cam_turn.take()) {
                (o.yaw, o.target) = (o.yaw + turn, at);
            }
            if let Some((_, v)) = sim.skate.as_ref().filter(|s| s.riding).and_then(|s| s.feet()).filter(|(_, v)| v.with_z(0.0).length() > 1.0) {
                let behind = (-v.y).atan2(-v.x);
                let d = (behind - o.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                o.yaw += d * (time.delta_secs() * 2.0).min(1.0);
            }
            mv = Vec3::ZERO;
        } else if sim.hero.is_some() {
            let cam_mv = mv;
            if sim.photo {
                mv = Vec3::ZERO;
            }
            for (k, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8,
                             KeyCode::Digit9, KeyCode::Digit0, KeyCode::Minus, KeyCode::Equal]
                .into_iter().enumerate() {
                if keys.just_pressed(key) {
                    sim.hero_take(k.checked_sub(1));
                }
            }
            *notches += if sim.over_ui || !hero_wheel { 0.0 } else { lines };
            while notches.abs() >= 1.0 {
                let n = WEAPONS.len() + 1;
                let at = if sim.hero_gun { sim.weapon + 1 } else { 0 };
                sim.hero_take(((at + if *notches < 0.0 { 1 } else { n - 1 }) % n).checked_sub(1));
                *notches -= notches.signum();
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
            let drive_keys = [KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD, KeyCode::Space];
            if sim.driving && !(keys.any_pressed(drive_keys) || keys.any_just_released(drive_keys)) {
            } else if sim.driving {
                let k = |c| (keys.pressed(c) && !sim.detached) as i32 as f32;
                let (pedal, wheel, hb) = (k(KeyCode::KeyW) - k(KeyCode::KeyS), k(KeyCode::KeyD) - k(KeyCode::KeyA), keys.pressed(KeyCode::Space) && !sim.detached);
                sim.pedals = (pedal, wheel, hb);
            } else {
                let (go, near) = (mv.with_z(0.0).normalize_or_zero(), sim.hero.as_ref().and_then(|n| sim.nearest_benched(n.feet)));
                match near {
                    Some(i) => sim.as_homer(i, |s| s.play_hero(dt, go, gait)),
                    None => sim.play_hero(dt, go, gait),
                }
                sim.homer_fight(dt);
            }
            mv = if sim.photo { cam_mv } else { Vec3::ZERO };
        }
        if fly != Vec3::ZERO {
            o.target += fly * time.delta_secs() * if shift_held(&keys) { 24.0 } else { 8.0 };
            o.follow = 3;
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
            let follow = o.follow == 0 && !sim.detached;
            sim.detached = follow;
            o.follow = if follow { 3 } else { 0 };
            sim.say(format!("room  camera: {}", if follow { "detached (hold right mouse: look, WASD fly where you look, Q/E down/up, Shift boost; the Hero stands still. Tab or C to follow again)" } else { "follows Homer" }));
        }
        if sim.detached {
            o.follow = 3;
        }
    }
    sim.cam_locked = o.follow == 0;
    let k = (time.delta_secs() * if sim.film > 0.0 { sim.film } else { 4.0 }).min(1.0);
    let mut dist = o.dist;
    if o.follow == 0 {
        let at = sim.skate.as_ref().filter(|s| s.riding).and_then(|s| s.feet()).map(|f| f.0)
            .or(sim.taxis.iter().find(|t| t.player).map(|t| t.pos)).or(sim.hero.as_ref().map(|n| n.feet))
            .unwrap_or_else(|| { let c = sim.shown().map_or(sim.agent.doll.com().0, |f| f.links[0].translation); c.with_z(c.z - STAND) });
        let mut at = if sim.cam_head { sim.agent.doll.centre(HEAD) } else { at.with_z(at.z + sim.look_z) };
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
    let slide = (-off).cross(Vec3::Z).normalize_or_zero() * (dist * 0.12 * (sim.show_ui && !sim.detached) as u8 as f32);
    let from = o.target + off - slide;
    *t = Transform::from_translation(from.with_z(from.z.max(sim.cam_floor))).looking_at(o.target - slide, Vec3::Z);
    if let Some(n) = sim.hero.as_ref().filter(|_| sim.pov) {
        *t = Transform::from_translation(n.head() + n.facing() * 0.25).looking_at(sim.agent.doll.centre(CHEST), Vec3::Z);
    }
    if let Some([ahead, up, side]) = sim.cam_rig {
        let s = sim.ours();
        let now = [0, HEAD, CHEST, THIGH_L, THIGH_R].map(|k| s.p(k));
        if rig_steps.0 != sim.steps {
            *rig_steps = (sim.steps, if rig_steps.0 == 0 { now } else { rig_steps.2 }, now);
        }
        let a = if sim.paused || sim.held { 1.0 } else { (sim.acc * 60.0).clamp(0.0, 1.0) };
        let [hips, head, chest, tl, tr] = std::array::from_fn(|i| rig_steps.1[i].lerp(rig_steps.2[i], a));
        let u = (head - hips).normalize_or(Vec3::Z);
        let r = (tr - tl).reject_from(u).normalize_or(Vec3::X);
        let f = u.cross(r);
        let (at, look) = (chest + f * ahead + u * up + r * side, chest.lerp(head, 0.6));
        let k = (time.delta_secs() * 8.0).min(1.0);
        let (p, l) = rig.map_or((at, look), |(p, l)| (p.lerp(at, k), l.lerp(look, k)));
        let p = p.with_z(p.z.max(0.15));
        *rig = Some((p, l));
        *t = Transform::from_translation(p).looking_at(l, u.lerp(Vec3::Z, 0.5).normalize());
    }
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
    let shake = (sim.gun.kick - 1.0).max(0.0) * 0.012 + sim.meat.as_ref().map_or(0.0, |m| m.shake);
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
        let d = std::iter::once(&sim.agent).chain(sim.crowd.iter().map(|c| &c.0)).enumerate().filter_map(|(i, a)| a.doll.probe(sim.build_of(i), from, from + dir * 60.0))
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
    let shift = shift_held(&keys) || ctrl && portal_gun(&sim);
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
    let scripted = sim.click_at.take();
    if (buttons.just_pressed(MouseButton::Left) || fatal || auto || scripted.is_some()) && !sim.over_ui {
        let Some(cursor) = scripted.and_then(|p| camera.world_to_viewport(gt, p).ok()).or_else(|| windows.single().ok().and_then(|w| w.cursor_position())) else { return };
        if camera.logical_viewport_rect().is_some_and(|r| !r.contains(cursor)) {
            return;
        }
        let Ok(ray) = camera.viewport_to_world(gt, cursor) else { return };
        let (from, dir) = (ray.origin, *ray.direction);
        if shift || sim.click_shoots {
            (sim.gun.wait, sim.gun.kick) = (between / 1000.0, KICK[sim.weapon]);
            return sim.fire_from(from, dir, fatal);
        }
        let near: Vec<f32> = std::iter::once(&sim.agent).chain(sim.crowd.iter().take(sim.bench.len()).map(|c| &c.0)).enumerate()
            .map(|(i, a)| a.doll.probe(sim.build_of(i), from, from + dir * 50.0).map_or(f32::MAX, |h| h.1.distance(from))).collect();
        if let Some(i) = (1..near.len()).filter(|&i| near[i] < near[0]).min_by(|&a, &b| near[a].total_cmp(&near[b])) {
            sim.control(i - 1);
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

fn scripted(mut sim: NonSendMut<Sim>, time: Res<Time>, mut commands: Commands, mut exit: MessageWriter<AppExit>,
            mut key_in: MessageWriter<bevy::input::keyboard::KeyboardInput>, window: Query<Entity, With<bevy::window::PrimaryWindow>>) {
    if sim.photo && sim.paused {
        let d = time.delta_secs();
        sim.script.iter_mut().for_each(|l| l.0 += d);
    }
    while sim.script.front().is_some_and(|l| l.0 <= time.elapsed_secs()) {
        let (_, line) = sim.script.pop_front().unwrap();
        let mut w = line.splitn(2, ' ');
        match (w.next().unwrap_or(""), w.next()) {
            ("!button", Some(b)) => sim.button(b),
            ("!real_water", _) => {
                let on = !sim.real_water;
                sim.set_real_water(on)
            }
            ("!flood", a) => if a.is_some_and(|a| a.trim() == "drain") { sim.drain() } else { sim.flood_toggle() },
            ("!sun", Some(a)) => {
                let v: Vec<f32> = a.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if let [az, el] = v[..] {
                    sim.sun_dir = [az, el];
                }
            }
            ("!look", Some(n)) => sim.skin = LOOKS.iter().find(|l| l.0 == n).map_or(sim.skin, |l| l.1),
            ("!pedcol", Some(n)) => if let Some(c) = ped_colour(n) { (sim.ped_color, sim.body_alpha) = c },
            ("!eyes", _) => sim.eyes = !sim.eyes,
            ("!skel", _) => sim.skel = !sim.skel,
            ("!clean", _) => sim.clean = !sim.clean,
            ("!shadows", _) => sim.shadows = !sim.shadows,
            ("!settings", _) => load_settings(&mut sim),
            ("!set", Some(k)) => apply_settings(&mut sim, &format!("{k} {}", w.next().unwrap_or(""))),
            ("!hero", _) => sim.toggle_hero(),
            ("!skate", Some(a)) => {
                let v: Vec<f32> = a.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if let [x, y, yaw] = v[..] {
                    let at = Vec3::new(x, y, 2.5);
                    let g = sim.world.ground(at, 5.0).unwrap_or(0.0);
                    sim.mount_skate(at.with_z(g), yaw);
                }
            }
            ("!skate", _) => sim.toggle_skate(),
            ("!skate_key", Some(a)) => if let Some(s) = sim.skate.as_mut() {
                let mut a = a.split_whitespace();
                let key = match a.next().unwrap_or("") {
                    "w" => KeyCode::KeyW, "s" => KeyCode::KeyS, "a" => KeyCode::KeyA, "d" => KeyCode::KeyD, "r" => KeyCode::KeyR,
                    "space" => KeyCode::Space, "q" => KeyCode::KeyQ, "e" => KeyCode::KeyE, "shift" => KeyCode::ShiftLeft, "f" => KeyCode::KeyF, _ => KeyCode::ControlLeft,
                };
                s.script.retain(|&k| k != key);
                if a.next() != Some("0") {
                    s.script.push(key);
                }
            },
            ("!skate_pad", Some(a)) => if let Some(s) = sim.skate.as_mut() {
                let mut a = a.split_whitespace();
                let v: Vec<f32> = a.next().unwrap_or("").split(',').filter_map(|x| x.trim().parse().ok()).collect();
                let b = a.next().and_then(|b| u16::from_str_radix(b.trim_start_matches("0x"), 16).ok()).unwrap_or(0);
                s.script_pad = (Vec2::new(v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0)), b);
            },
            ("!skate_stick", Some(a)) => if let Some(s) = sim.skate.as_mut() {
                let v: Vec<f32> = a.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                s.script_stick = Vec2::new(v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0));
            },
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
            ("!blast", Some(a)) => {
                let v: Vec<f32> = a.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                sim.explode(Vec3::new(v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)));
            }
            ("!poke", Some(a)) => {
                let mut w = a.split_whitespace();
                let (k, s) = (w.next().and_then(|k| k.parse().ok()).unwrap_or(CHEST), sim.ours());
                if let Some(f) = w.next().and_then(|f| f.parse().ok()) {
                    sim.poke = f;
                }
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
                sim.fire(part, None, w.next() == Some("fatal"), &[]);
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
            ("!speed", Some(a)) => sim.speed = a.parse().unwrap_or(1.0),
            ("!crane", Some(a)) => {
                let v: Vec<f32> = a.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if let [yaw, pitch, dist, z, secs, ref at @ ..] = v[..] {
                    sim.crane_to = Some(([yaw, pitch, dist, z], secs, (at.len() == 3).then(|| Vec3::from_slice(at))));
                }
            }
            ("!shadow", Some(a)) => sim.shadow = a.parse().unwrap_or(0.0),
            ("!strict", _) => sim.strict = true,
            ("!camhead", _) => sim.cam_head = true,
            ("!gore", _) => sim.gore = !sim.gore,
            ("!dropgrid", Some(a)) => if let [c, r, g] = a.split_whitespace().filter_map(|x| x.parse().ok()).collect::<Vec<f32>>()[..] {
                let at: Vec<(f32, f32)> = (0..(c * r) as usize).map(|i| (((i / c as usize) as f32 - (r - 1.0) / 2.0) * g, ((i % c as usize) as f32 - (c - 1.0) / 2.0) * g))
                    .filter(|&(f, x)| f.abs() + x.abs() > g * 0.6).take(49).collect();
                sim.homers("drop", &at);
                for i in 0..=sim.crowd.len() {
                    let h = |k: u32| ((i as u32 * 7 + k).wrapping_mul(2654435761) >> 8) as f32 / 16777216.0 - 0.5;
                    let part = [0, 7, 9, 10, 12, 15, 19][i * 5 % 7];
                    sim.send_to(i, &format!("applyImpulse partIndex={part} impulse={}", v3(Vec3::new(h(1) * 140.0, h(2) * 140.0, (h(3) + 0.5) * 220.0))));
                    sim.send_to(i, "highFall start=true");
                }
            },
            ("!camfloor", Some(z)) => sim.cam_floor = z.parse().unwrap_or(f32::MIN),
            ("!keepgun", a) => sim.kept_gun = (a != Some("off")).then_some((sim.weapon, None)),
            ("!gunoff", Some(d)) => if let [x, y, z] = d.split(',').filter_map(|x| x.parse().ok()).collect::<Vec<f32>>()[..] {
                let chest = sim.ours().p(CHEST);
                (sim.gun_off, sim.gun_aim_off, sim.gun.placed) = (Vec3::new(x, y, z), Vec3::ZERO, true);
                (sim.gun.at, sim.gun.aim) = (chest + sim.gun_off, chest);
                sim.gun.dir = (chest - sim.gun.at).normalize();
            },
            ("!skyk", Some(k)) => sim.sky_k = k.parse().unwrap_or(1.0),
            ("!tear", Some(k)) => sim.tear_k = k.parse().unwrap_or(1.0),
            ("!spawnat", Some(a)) => if let [b, x, y] = a.split_whitespace().collect::<Vec<_>>()[..] && let (Ok(x), Ok(y)) = (x.parse(), y.parse()) {
                sim.spawn_at = Some(Vec2::new(x, y));
                sim.button(&format!("spawn {b}"));
            },
            ("!camrig", Some(a)) => if let [d, u, x] = a.split_whitespace().filter_map(|x| x.parse().ok()).collect::<Vec<f32>>()[..] { sim.cam_rig = Some([d, u, x]) },
            ("!pov", _) => sim.pov = !sim.pov,
            ("!miss", Some(a)) => {
                let head = sim.agent.doll.centre(HEAD);
                let from = head + Vec3::new(0.0, -6.0, 0.0);
                sim.near_miss(from, (head + Vec3::X * a.parse().unwrap_or(0.5) - from).normalize());
            }
            ("!hatshot", _) => {
                let head = sim.agent.doll.centre(HEAD);
                let from = head + Vec3::new(0.0, -4.0, 0.3);
                let dir = (head + Vec3::Z * 0.15 - from).normalize();
                sim.hat_off(dir * 3.0);
                sim.near_miss(from, dir);
            }
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
                sim.script_jog = v.get(2).is_some_and(|&j| j > 0.0);
                sim.walk = sim.script_walk != Vec3::ZERO;
                sim.held = false;
            }
            ("!infinite", _) => sim.infinite = !sim.infinite,
            ("!hide", _) => sim.show_ui = !sim.show_ui,
            ("!tab", Some(n)) => sim.tab = n.parse().unwrap_or(1),
            ("!cam2", a) => sim.cam2 = match a.unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect::<Vec<f32>>()[..] {
                [x, y, z, a, b, c] => Some((Vec3::new(x, y, z), Vec3::new(a, b, c))),
                _ => None,
            },
            ("!meat", _) => sim.meat_on = !sim.meat_on,
            ("!screenshot", Some(f)) if sim.cam2.is_some() => sim.cam2_queue.push(f.to_string()),
            ("!screenshot", Some(f)) => {
                commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                    .observe(bevy::render::view::screenshot::save_to_disk(f.to_string()));
            }
            ("!tp", Some(a)) => {
                if let [x, y] = a.split_whitespace().filter_map(|x| x.parse().ok()).collect::<Vec<f32>>()[..] {
                    let by = (Vec2::new(x, y) - sim.ours().p(0).truncate()).extend(0.0);
                    let fr: Vec<Transform> = sim.agent.doll.frames().map(|f| f.with_translation(f.translation + by)).collect();
                    sim.agent = Agent::new(&sim.build, Ragdoll::new(&sim.build, &fr, Vec3::ZERO), Quat::IDENTITY, 0.3);
                    sim.msgs = msg::State::default();
                    sim.send("bodyBalance start=true");
                }
            }
            ("!hfall", a) => {
                let up = a.and_then(|a| a.trim().parse().ok()).unwrap_or(15.0);
                let mut l = vec!["stopAllBehaviours".to_string()];
                l.extend(delirium::highfall::start(&delirium::highfall::Set::read(&sim.tp, "*"), false).iter().map(shottask::text));
                sim.send_ours(l);
                sim.agent.doll.kick(Vec3::Z * up);
                sim.say(format!("both  hfall: NMHFall, up {up} m/s"));
            }
            ("!lift", Some(h)) => {
                let up = Vec3::Z * h.parse().unwrap_or(2.0);
                let fr: Vec<Transform> = sim.agent.doll.frames().map(|f| f.with_translation(f.translation + up)).collect();
                sim.agent = Agent::new(&sim.build, Ragdoll::new(&sim.build, &fr, Vec3::ZERO), Quat::IDENTITY, 0.3);
                sim.msgs = msg::State::default();
                sim.send("bodyBalance start=true");
            }
            ("!skydrop", Some(a)) => {
                let v: Vec<f32> = a.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                let s = sim.ours();
                let off = s.fwd * v.get(1).copied().unwrap_or(0.3) + Vec3::Z * v.first().copied().unwrap_or(3.0);
                let (pel, flat) = (sim.agent.doll.frames().next().unwrap().translation, Quat::from_axis_angle(s.fwd, std::f32::consts::FRAC_PI_2));
                let fr: Vec<Transform> = sim.agent.doll.frames().map(|f| Transform::from_translation(pel + flat * (f.translation - pel) + off).with_rotation(flat * f.rotation)).collect();
                let a = Agent::new(&sim.build, Ragdoll::new(&sim.build, &fr, -off.with_z(0.0).normalize_or_zero() * v.get(2).copied().unwrap_or(0.0)), Quat::IDENTITY, 0.3);
                sim.crowd.push((a, msg::State::default()));
                sim.crowd_hp.push(200.0);
            }
            ("!spawn", Some(b)) => sim.button(&format!("spawn {b}")),
            (c @ ("!key" | "!keyup"), Some(a)) => if let Ok(w) = window.single() {
                use bevy::input::{ButtonState, keyboard::{Key, KeyboardInput}};
                let state = if c == "!key" { ButtonState::Pressed } else { ButtonState::Released };
                let a = a.to_lowercase();
                let (mods, key) = a.rsplit_once('+').unwrap_or(("", &a));
                let mut keys: Vec<(KeyCode, Key)> = vec![];
                if mods.contains("shift") {
                    keys.push((KeyCode::ShiftLeft, Key::Shift));
                }
                if mods.contains("ctrl") {
                    keys.push((KeyCode::ControlLeft, Key::Control));
                }
                if let Some(code) = match key { "n" => Some(KeyCode::KeyN), "r" => Some(KeyCode::KeyR), "x" => Some(KeyCode::KeyX), _ => None } {
                    keys.push((code, Key::Character(key.into())));
                }
                if state == ButtonState::Released {
                    keys.reverse();
                }
                for (key_code, logical_key) in keys {
                    key_in.write(KeyboardInput { key_code, logical_key, state, text: None, repeat: false, window: w });
                }
            },
            ("!click", Some(a)) => {
                let v: Vec<f32> = a.split(',').filter_map(|x| x.parse().ok()).collect();
                if let [x, y, z] = v[..] {
                    sim.click_at = Some(Vec3::new(x, y, z));
                }
            }
            ("!control", Some(n)) => sim.button(&format!("control {n}")),
            ("!unspawn", Some(n)) => sim.button(&format!("unspawn {n}")),
            ("!ped", Some(n)) => {
                let scene = sim.scene.clone().unwrap_or_else(|| "room".into());
                sim.switch_to = Some((scene, n == "marge"));
            }
            ("!body", Some(n)) => if let Some(i) = BODIES.iter().position(|b| b.0 == n) {
                sim.body = i;
                sim.switch_to = Some((sim.scene.clone().unwrap_or_else(|| "room".into()), nm() == delirium::nmrs::MARGE));
            },
            ("!strength", Some(n)) => sim.agent.strength = n.parse().unwrap_or(1.0),
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
    fn stop_walk_learned(&mut self) {
        if self.walk_learned.take().is_some() {
            self.agent.configure_balance(Default::default());
            self.agent.muscle_stiffness_x(1.0);
        }
    }

    fn start_walk_learned(&mut self, run: bool) {
        if self.walk_learned.as_ref().is_some_and(|w| w.3 == run) {
            self.stop_walk_learned();
            self.send("bodyBalance start=true");
            return self.say("ours  learned locomotion: off".into());
        }
        let path = learn::walk_path(run);
        let Some(net) = learn::Net::load(&path).filter(|n| n.sizes == learn::walk_sizes(&self.agent.doll)) else {
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
        self.walk_learned = Some((net, 0.0, walk.duration, run));
        self.walk = true;
        self.say(format!("ours  learned WASD {}: {path}; use WASD to move, reset or another reaction stops it", if run { "run" } else { "walk" }));
    }

    fn preview_frame(&mut self) -> bool {
        delirium::nmrs::LOOSE.store(self.preview_on, std::sync::atomic::Ordering::Relaxed);
        let (w, shift) = self.preview_keys;
        self.preview_wait &= w;
        let w = w || (self.preview.is_some() && self.agent.doll.motion(0).0.with_z(0.0).length() > 1.3);
        if !self.preview_on || !w || self.preview_wait || self.hero.is_some() || self.driving || self.dead {
            if self.preview.take().is_some() {
                self.send("bodyBalance start=true");
            }
            return false;
        }
        if self.preview.is_none() {
            if self.agent.doll.frames().nth(HEAD).is_none_or(|f| f.translation.z < 1.3) || self.walk_dir == Vec3::ZERO {
                return false;
            }
            let start: Vec<Transform> = self.agent.doll.frames().collect();
            let moving: Vec<(Vec3, Vec3)> = (0..start.len()).map(|k| self.agent.doll.motion(k)).collect();
            let dir = std::path::Path::new(&learn::path()).with_file_name("learned-preview");
            match locomotion::preview::Preview::new(&self.build, &self.world, &start, &dir) {
                Ok((p, mut a)) => {
                    for (k, (f, &(v, w))) in start.iter().zip(&moving).enumerate() {
                        a.doll.set_body(k, f.translation, f.rotation, v, w);
                    }
                    self.agent = a;
                    self.msgs = Default::default();
                    (self.rises, self.still, self.nm_t, self.musc, self.learned, self.walk_learned) = ([None, None], [0.0; 2], [None, None], None, None, None);
                    (self.held, self.auto_balance, self.homer, self.tasks, self.watch) = (false, false, None, false, None);
                    self.preview = Some((p, locomotion::yaw(&start) * Vec3::X));
                }
                Err(e) => {
                    self.preview_on = false;
                    self.say(format!("ours  experimental walk: {e} ({})", dir.display()));
                    return false;
                }
            }
        }
        let beta = if !self.preview_keys.0 { 0.0 } else if shift { 1.0 } else { 0.25 };
        let (p, ahead) = self.preview.as_mut().unwrap();
        let at = self.agent.doll.frames().next().unwrap().translation.with_z(0.0);
        if [0.4, 1.2].iter().any(|&z| self.world.cast(at + Vec3::Z * z, *ahead, 2.0).is_some()) || (at + *ahead * 4.0).abs().max_element() > 48.0 {
            (self.preview, self.preview_wait) = (None, true);
            self.send("bodyBalance start=true");
            self.say("ours  experimental walk: wall or edge ahead, stopped; turn the camera and press W again".into());
            return false;
        }
        let side = self.walk_dir - *ahead * self.walk_dir.dot(*ahead);
        if side.length() > 0.01 {
            let ped = self.agent.doll.frames().next().unwrap().translation;
            let (f, _) = delirium::playerctl::frame((0, -127), (-side.x).atan2(side.y), ped, self.agent.doll.mass(), &mut self.wander.clone(), &mut self.rand.clone());
            self.agent.push.push((0, ped, f * side.length()));
        }
        if !p.step(&mut self.agent, &self.build, &self.world, beta) {
            (self.preview, self.preview_wait) = (None, true);
            self.say("ours  experimental walk: fell; let go of W and press it again once he's up".into());
        }
        true
    }

    fn walk_frame(&mut self) {
        let d = self.walk_dir;
        if !self.walk || d == Vec3::ZERO || self.musc.is_some() || self.preview.is_some() {
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

    fn extras_frame(&mut self) {
        for l in std::mem::take(&mut self.agent.extras.outbox) {
            match l.strip_prefix("#say ") {
                Some(s) => self.say(s.to_string()),
                None => self.send(&l),
            }
        }
    }

    fn wounds_frame(&mut self, dt: f32) {
        let (beat, life, mut spurts) = (WOUND_BEAT, if self.gore { WOUND_LIFE } else { WOUND_LIFE / 4.0 }, vec![]);
        for w in &mut self.wounds {
            w.3 += dt;
            if w.3 % beat < dt && let Some(f) = self.agent.doll.frames().nth(w.0) {
                spurts.push((f.translation + f.rotation * w.1, f.rotation * w.2, 1.0 - w.3 / life, (w.3 / beat) as u32 % 3 == 0));
            }
        }
        self.wounds.retain(|w| w.3 < life);
        for (at, n, left, pool) in spurts {
            self.spurt(at, n, left);
            if pool && self.gore {
                self.drip(at);
            }
        }
    }

    fn tick(&mut self) {
        self.extras_frame();
        self.bridge();
        self.trampoline();
        self.stress_step();
        self.room.shook += 1.0 / 60.0;
        if let Some((p, t, m)) = self.room.plat_triangles() {
            self.world.remove_chunk(PLAT_CHUNK);
            self.world.add_chunk(PLAT_CHUNK, &p, &t, &m);
        }
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
                    Some((t, n, _)) => (b.0, b.1, b.4) = (b.0 + d * t + n * 0.002, Vec3::ZERO, n),
                    None => b.0 += d * len,
                }
                if b.0.z < -50.0 {
                    b.2 = BLOOD_LIFE;
                }
            }
        }
        let gore = self.gore;
        self.blood.retain(|b| b.2 < if b.1 == Vec3::ZERO && gore { BLOOD_POOL_LIFE } else { BLOOD_LIFE });
        self.wounds_frame(dt);
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
        self.meat_tick(1.0 / 60.0);
        let hero = self.hero.as_ref().zip(self.getup.as_ref()).filter(|_| !self.driving && self.hero_doll.is_none()).map_or(vec![], |(n, c)| n.capsules(c));
        self.taxi_frame(1.0 / 60.0);
        let car: Vec<_> = self.cars.iter().map(|&(c, v, _)| delirium::ragdoll::Obstacle { at: c, rot: Quat::from_rotation_arc(Vec3::Y, v.normalize()), centre: c,
            half: Vec3::new(0.92, 2.54, 0.6), vel: v, com: c, ..Default::default() })
            .chain(self.pusher.iter().map(|&(c, v, _)| delirium::ragdoll::Obstacle { at: c, rot: Quat::from_rotation_arc(Vec3::Y, v.normalize()), centre: c,
                half: Vec3::new(0.7, 0.25, 0.9), vel: v, com: c, ..Default::default() })).chain(self.taxis.iter().enumerate().flat_map(|(k, t)| [t.obstacle(TAXI_ID + k as i32, TAXI_BELT), t.cabin(TAXI_ID + k as i32, TAXI_BELT)]))
            .chain(self.room.bridge.iter().flat_map(|b| (0..b.planks.len()).map(|i| b.plank(&self.room.w[0], i)).enumerate().map(|(i, t)| delirium::ragdoll::Obstacle {
                id: PLANK_ID + i as i32, at: t.translation, rot: t.rotation, centre: t.translation - Vec3::Z * 1000.0, ..Default::default() })))
            .chain(world::plat(&self.room.stage, self.room.shook).zip(world::plat(&self.room.stage, self.room.shook + 1.0 / 60.0)).map(|((at, rot), (next, turn))| {
                let q = turn * rot.inverse();
                let (axis, angle) = if q.w < 0.0 { -q } else { q }.to_axis_angle();
                delirium::ragdoll::Obstacle { id: PLAT_ID, at, rot, centre: at, half: (*world::PLAT_HALF()), vel: (next - at) * 60.0, spin: axis * angle * 60.0, com: at, ..Default::default() }
            })).collect();
        self.pull();
        self.soak();
        self.soak_hero(1.0 / 60.0, true);
        self.fire_frame(1.0 / 60.0);
        if self.steps % 2 == 0 {
            self.shot_frame();
            self.blast_frame();
            self.walk_frame();
            self.in_air();
            self.self_preserve();
        }
        let even = self.steps % 2 == 0;
        for i in 0..self.bench.len() {
            let up = self.as_homer(i, |s| s.bench_frame(even));
            self.bench[i].up = up;
        }
        let t0 = std::time::Instant::now();
        let learning = self.learned.is_some();
        if (self.homer.is_some() || self.riding()) && caps_of(&self.build, &self.agent.doll).iter().any(|c| car.iter().any(|o| {
            let back = o.rot.inverse();
            [c.0, (c.0 + c.1) * 0.5, c.1].iter().any(|&p| ((back * (p - o.centre)).abs() - o.half).max_element() < c.2)
        })) {
            if self.riding() {
                self.skate_bail("hit by a car");
            } else {
                self.knocked_out();
            }
        }
        let learning = learning || self.homer.is_some() || self.riding();
        if let Some((net, zero, n, pick)) = self.learned.as_mut() {
            learn::step(net, &net.w, &mut self.agent, &self.build, &self.world, zero, pick);
            *n += 1;
        }
        if let Some((net, phase, duration, _)) = self.walk_learned.as_mut() {
            if self.walk && self.walk_dir != Vec3::ZERO {
                learn::drive_walk(net, &net.w, &mut self.agent, phase, *duration);
            } else {
                *phase = 0.0;
                self.agent.configure_balance(Default::default());
                self.agent.muscle_stiffness_x(1.0);
            }
        }
        let learning = self.preview_frame() || learning;
        self.tackle_frame(1.0 / 60.0);
        self.jump_frame(1.0 / 60.0);
        self.stairs_frame();
        let homer = if self.homer.is_some() { caps_of(&self.build, &self.agent.doll) } else { vec![] };
        let doll = self.hero_doll.as_mut().filter(|h| h.rise.is_none()).map(|h| (&mut h.agent, &homer[..], &h.build));
        let (build, bench, crowd) = (&self.build, &self.bench, &self.crowd_build);
        let mut all: Vec<(&mut Agent, usize, &Build, &[_], &[Capsule])> = std::iter::once(&mut self.agent).chain(self.crowd.iter_mut().map(|c| &mut c.0)).enumerate()
            .filter(|(p, _)| p.checked_sub(1).map_or(!learning, |i| bench.get(i).is_none_or(|b| b.learned.is_none())))
            .map(|(p, a)| (a, &hero[..], ped_build(build, bench, crowd, p)))
            .chain(doll).enumerate().map(|(i, (a, o, b))| (a, i, b, &car[..], o)).collect();
        Agent::step_group(&mut all, &self.world, 1.0 / 60.0, &mut self.group);
        self.car_hits();
        self.sever_frame(1.0 / 60.0, &car);
        for i in 0..self.bench.len() {
            if !self.bench[i].cuts.is_empty() || matches!(self.bench[i].hat, Some(sever::Hat::Off(..))) {
                self.as_homer(i, |s| s.sever_frame(1.0 / 60.0, &car));
            }
        }
        if (self.crowd.len() > 4 || !self.bench.is_empty()) && self.steps % 300 == 299 {
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            self.say(format!("ours  {}: {} Homers, {ms:.2} ms a step", if self.bench.is_empty() { "grid" } else { "spawned" }, self.crowd.len() + 1));
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
            self.stop_walk_learned();
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
        if self.look_stop.is_some_and(|t| self.steps as f32 / 60.0 >= t) {
            self.look_stop = None;
            if self.homer.is_none() {
                self.send("headLook start=false");
            }
            if std::mem::take(&mut self.dodging) {
                if self.agent.balancing() {
                    self.send("stopAllBehaviours");
                    self.send("bodyBalance start=true");
                }
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

fn step(mut sim: NonSendMut<Sim>, time: Res<Time>, mut timing: Local<(f32, f32, u32)>) {
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
    sim.frame_ms += (time.delta_secs() * 1000.0 - sim.frame_ms) * 0.05;
    if !sim.paused {
        sim.acc += time.delta_secs() * sim.speed;
    }
    sim.acc = sim.acc.min(4.0 / 60.0);
    if sim.held {
        sim.acc = 0.0;
    }
    let started = std::time::Instant::now();
    while sim.acc >= 1.0 / 60.0 {
        sim.acc -= 1.0 / 60.0;
        sim.tick();
    }
    let dt = if sim.paused { 0.0 } else { time.delta_secs() * sim.speed };
    sim.rise(dt);
    sim.hero_frame(dt);
    let held = sim.held;
    sim.soak_hero(if held { 0.0 } else { dt }, false);
    if std::env::var_os("FRAME_LOG").is_some() {
        *timing = (timing.0 + time.delta_secs() * 1000.0, timing.1 + started.elapsed().as_secs_f32() * 1000.0, timing.2 + 1);
        if timing.0 >= 1000.0 {
            let n = timing.2 as f32;
            eprintln!("frame {:.1} FPS, {:.1} ms a frame, physics {:.1} ms", n * 1000.0 / timing.0, timing.0 / n, timing.1 / n);
            *timing = (0.0, 0.0, 0);
        }
    }
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

const PED_COLORS: [(&str, &str); 3] = [("mixed", "Red & blue"), ("blue", "All blue"), ("red", "All red")];
const BODY_ALPHA: f32 = 0.55;
fn ped_colour(k: &str) -> Option<(usize, f32)> {
    match k {
        "solid" | "3" => Some((1, 1.0)),
        "opaque" | "4" => Some((0, 1.0)),
        _ => PED_COLORS.iter().position(|c| c.0 == k).or_else(|| k.parse().ok().filter(|&i: &usize| i < PED_COLORS.len())).map(|i| (i, BODY_ALPHA)),
    }
}
fn part_color(c: usize, name: &str, a: f32) -> Color {
    let (red, blue) = (Color::srgba(0.85, 0.3, 0.25, a), Color::srgba(0.3, 0.45, 0.9, a));
    match c {
        1 => blue,
        2 => red,
        _ if name.ends_with("Left") => red,
        _ if name.ends_with("Right") => blue,
        _ => Color::srgba(0.72, 0.72, 0.74, (a + 0.05).min(1.0)),
    }
}
fn bone_tint(c: usize) -> Option<Color> {
    [None, Some(Color::srgb(0.35, 0.55, 1.0)), Some(Color::srgb(1.0, 0.35, 0.3)), Some(Color::srgb(0.35, 0.55, 1.0)), None][c.min(4)]
}

#[cfg(windows)]
fn window_icon(_main: NonSend<Sim>, mut done: Local<bool>) {
    use winit::platform::windows::{IconExtWindows, WindowExtWindows};
    if *done {
        return;
    }
    let (Ok(icon), Ok(big)) = (winit::window::Icon::from_resource(1, None), winit::window::Icon::from_resource(1, Some(winit::dpi::PhysicalSize::new(256, 256)))) else {
        *done = true;
        return;
    };
    bevy::winit::WINIT_WINDOWS.with_borrow(|w| {
        for win in w.windows.values() {
            win.set_window_icon(Some(icon.clone()));
            win.set_taskbar_icon(Some(big.clone()));
            *done = true;
        }
    });
}

fn skel_tint(sim: &Sim) -> Option<Color> {
    if sim.skel_style == 2 { Some(Color::srgb(1.0, 0.85, 0.1)) } else { bone_tint(sim.ped_color) }
}

fn muscles(mut commands: Commands, sim: NonSend<Sim>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
           mut parts: Local<Vec<(Entity, Entity, Entity)>>, mut at: Query<(&mut Transform, &mut Visibility)>) {
    let on = sim.skel && !sim.clean && sim.skel_style == 1 && sim.watch.is_none();
    if parts.len() != sim.build.parts.len() {
        if !on {
            return;
        }
        let bone = meshes.add(ConicalFrustum { radius_top: 0.5, radius_bottom: 1.0, height: 1.0 });
        let ball = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
        let red = mats.add(StandardMaterial { base_color: Color::srgb(0.7, 0.1, 0.08), emissive: LinearRgba::rgb(0.3, 0.02, 0.015), perceptual_roughness: 0.45, ..default() });
        let yellow = mats.add(StandardMaterial { base_color: Color::srgb(1.0, 0.8, 0.1), emissive: LinearRgba::rgb(0.6, 0.4, 0.02), perceptual_roughness: 0.35, ..default() });
        let layer = RenderLayers::layer(SKEL_LAYER);
        *parts = sim.build.parts.iter().map(|_| {
            let mut spawn = |m: &Handle<Mesh>, c: &Handle<StandardMaterial>| commands.spawn((Mesh3d(m.clone()), MeshMaterial3d(c.clone()), Transform::default(),
                                                                                               Visibility::Hidden, layer.clone(), bevy::light::NotShadowCaster)).id();
            (spawn(&bone, &red), spawn(&bone, &red), spawn(&ball, &yellow))
        }).collect();
        return;
    }
    let fr = sim.pose();
    let joint = |k: usize| fr.get(k).map(|f| if sim.build.parts[k].parent.is_some() { f.translation + f.rotation * sim.build.parts[k].anchor } else { f.translation });
    let mut put = |e: Entity, ab: Option<(Vec3, Vec3)>, w: Option<f32>| if let Ok((mut t, mut v)) = at.get_mut(e) {
        let Some((a, b)) = ab.filter(|_| on) else {
            *v = Visibility::Hidden;
            return;
        };
        let d = b - a;
        let w = w.unwrap_or((d.length() * 0.14).clamp(0.015, 0.05));
        *v = Visibility::Inherited;
        *t = Transform { translation: (a + b) * 0.5, rotation: Quat::from_rotation_arc(Vec3::NEG_Y, d.normalize_or(Vec3::Z)), scale: Vec3::new(w, d.length(), w) };
    };
    for (k, p) in sim.build.parts.iter().enumerate() {
        let (bone, tip, ball) = parts[k];
        let x = joint(k);
        put(bone, x.zip(p.parent.and_then(&joint)).map(|(x, y)| (y, x)), None);
        let end = p.parent.is_some() && !sim.build.parts.iter().any(|c| c.parent == Some(k));
        put(tip, x.zip(fr.get(k)).filter(|_| end).map(|(x, f)| (x, f.translation * 2.0 - x)), None);
        put(ball, x.map(|x| (x - Vec3::Z * 0.015, x + Vec3::Z * 0.015)), Some(0.03));
    }
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

fn grid_line(g: &mut Gizmos, pool: bool, a: Vec3, b: Vec3, c: Color) {
    let [lo, hi] = (*world::POOL());
    let (k, across) = if a.x == b.x { (1, a.x) } else { (0, a.y) };
    let (l0, l1, c0, c1) = if k == 1 { (lo.y, hi.y, lo.x, hi.x) } else { (lo.x, hi.x, lo.y, hi.y) };
    if !pool || across <= c0 || across >= c1 || b[k] <= l0 || a[k] >= l1 {
        g.line(a, b, c);
        return;
    }
    if a[k] < l0 {
        let mut e = b;
        e[k] = l0;
        g.line(a, e, c);
    }
    if b[k] > l1 {
        let mut s = a;
        s[k] = l1;
        g.line(s, b, c);
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
            grid_line(&mut g, sim.room.pool, o + Vec3::new(x, -(n as f32 * 0.5), 0.002), o + Vec3::new(x, (n as f32 * 0.5), 0.002), c);
            grid_line(&mut g, sim.room.pool, o + Vec3::new(-(n as f32 * 0.5), x, 0.002), o + Vec3::new((n as f32 * 0.5), x, 0.002), c);
            continue;
        }
        for j in -n..n {
            let (y0, y1) = (j as f32 * 0.5, j as f32 * 0.5 + 0.5);
            for (a, b) in [(o + Vec3::new(x, y0, 0.002), o + Vec3::new(x, y1, 0.002)), (o + Vec3::new(y0, x, 0.002), o + Vec3::new(y1, x, 0.002))] {
                let f = (1.0 - ((a + b) * 0.5).distance(me) / fade).clamp(0.0, 1.0);
                if f > 0.0 {
                    grid_line(&mut g, sim.room.pool, a, b, c.with_alpha(0.35 * f));
                }
            }
        }
    }
    let doll = &sim.agent.doll;
    let dbg = sim.skel && !sim.clean;
    let up = |i: usize| sim.getup.as_ref().zip(sim.rises[i].as_ref()).and_then(|(c, r)| c.frames(r));
    let ours_up = up(0);
    let rising = ours_up.is_some();
    let fr = sim.pose();
    if let Some(w) = sim.watch.as_ref() {
        for i in 0..w.tries.len() {
            skeleton(&mut ours, &sim.build, bone_tint(sim.ped_color), |k| w.at(i, k), |_| false);
        }
    } else if dbg && sim.skel_style != 1 {
        skeleton(&mut ours, &sim.build, skel_tint(&sim), |k| fr.get(k).copied(), |k| doll.touch[k] & 0x48 != 0);
    }
    if let Some(p) = sim.throw_at.filter(|_| !sim.clean) {
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
    for &(ped, k, at, n, age) in sim.rings.iter().filter(|_| !sim.clean) {
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
    if sim.arrows && !sim.clean {
        for (k, f) in fr.iter().enumerate() {
            let (force, torque) = doll.out[k];
            ours.arrow(f.translation, f.translation + torque * 0.002, Color::srgb(0.9, 0.3, 0.9));
            ours.arrow(f.translation, f.translation + force * 0.0005, Color::srgb(0.3, 0.6, 1.0));
        }
    }
    for (a, _) in sim.crowd.iter().filter(|_| dbg) {
        let f: Vec<Transform> = a.doll.frames().collect();
        skeleton(&mut ours, &sim.build, skel_tint(&sim), |k| f.get(k).copied(), |k| a.doll.touch[k] & 0x48 != 0);
    }
    let (com, v) = doll.com();
    let (com, v) = if rising { (fr[0].translation, Vec3::ZERO) } else { (com, v) };
    if dbg {
        ours.sphere(Isometry3d::from_translation(com), 0.04, Color::srgb(1.0, 0.3, 0.2));
        ours.arrow(com.with_z(0.01), (com + v * 0.3).with_z(0.01), Color::srgb(1.0, 0.5, 0.2));
    }
    for (k, p) in sim.build.parts.iter().enumerate().filter(|_| dbg) {
        let Some(e) = p.boxed else { continue };
        let c = Color::srgb(0.2, 0.9, 0.95);
        if let Some(f) = fr.get(k) {
            ours.primitive_3d(&Cuboid { half_size: e }, Isometry3d::new(f.translation, f.rotation * delirium::nmdbm::rage_link(nm(), k)), c);
        }
    }
    if !sim.clean {
        looks(&sim, &mut ours, fr.get(HEAD).map(|f| f.translation), 0);
    }
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
const TIPS: [(&str, &str); 49] = [
    ("skate", "Skate 3 mode (J): Homer on a skateboard, run by the Skate 3 Rust engine on your own Skate 3 (link it in Settings, Skate 3 · bonus). W push, S brake, A/D steer, Space ollie, Q kickflip, E heelflip, hold the wheel button and move the mouse for the right stick's flick tricks, Shift/Ctrl crouch and grab, F (X), Z/X bumpers, land on a rail or ledge to grind. R off the board: WASD walk, Shift run, Space jump, F throw the board, Ctrl drop it, either again calls it back, R back on. Bails are Skate 3's own or Homer's ragdoll (Settings, Skate 3 · bonus); after a ragdoll he gets up and walks on as in Skate 3. J leaves Skate 3 mode. A gamepad works as in Skate 3."),
    ("throw ball", "Throws a ball at Homer. Hold Ctrl and point first: it goes there instead."), ("throw box", "Throws a box at Homer. Hold Ctrl and point first: it goes there instead."),
    ("drop crate", "Drops a heavy crate on Homer."), ("crowd", "Four more peds beside Homer, balancing. Again: take them away."), ("grid", "24 more peds in a grid round Homer. Again: take them away."),
    ("car", "A taxi comes at Homer from his left at 8 m/s and hits him."),
    ("#ripples", "Cyan rings on the floor where a body part lands hard."),
    ("#photosky", "A photo of a partly cloudy sky (Poly Haven, CC0) instead of the sky colour, turned to follow Sun direction. It lights the room too, and ray tracing sees it."),
    ("#bevysky", "Bevy's sky instead of the sky colour, worked out from the sun: a sunset when it's low. It lights the room too, and ray tracing sees it."),
    ("#shadows", "Shadows on or off. Off is lighter on a slow graphics card."),
    ("mixed", "Left side red, right side blue: easy to tell his left and right apart."),
    ("blue", "All blue."),
    ("red", "All red."),
    ("#skel", "The skeleton's lines and joints drawn over Homer's body."),
    ("#clean", "Hides every debug drawing on the characters (skeleton, boxes, centre of mass, arrows, look and avoid markers) and makes their bodies solid instead of see-through."),
    ("#eyes", "Homer's eyes: they look where he's reacting to, blink, squint when he's hurt and shut when he's out."),
    ("#brows", "Homer's eyebrows: they show when he's scared, hurt or stumbling."),
    ("explosion pile", "Barrels and crates in front of Homer that blow up 2 s later."), ("wrecking ball", "A 250 kg ball on a rope swings through Homer."),
    ("push front", "A slab slides into Homer from the front; he braces against it."), ("push back", "A slab shoves Homer from behind; his balance catches it."), ("taxi", "4 parked taxis to drive (with Hero: F gets in and out, W / S drive, A / D steer)."),
    ("clear taxis", "Takes the parked taxis away."),
    ("fall", "Lifts Homer 3 m and lets him fall."), ("high fall", "Lifts Homer 12 m and lets him fall."), ("clear", "Takes away thrown balls, boxes and crates."), ("ice", "Makes the floor ice. Again: back to concrete."),
    ("replay", "Plays the last 10 s again (the speed buttons slow it down). Press again to stop."),
    ("Hero", "Play as Hero beside Homer: WASD walk, Shift run, Ctrl sprint, click punch; walk into Homer to push him. Again: take Hero out."),
    ("homer weapon", "What Homer fights back with: fists, a bat, a knife or a gun (click to change). With a gun he stands off and shoots the Hero."),
    ("even fight", "Homer gets back into the fight as fast as the Hero does after a hit, so he trades blows instead of going down."), ("super punch", "Bonus: the Hero's punches that land go off like a grenade and throw Homer."),
    ("hero physics", "On: blows, shots and blasts put the Hero on his own ragdoll, and he gets up after. Off: he plays the hit animations."),
    ("passive", "On: Homer only takes hits. Off: near the Hero he fights back (walks up, punches, kicks); a hit puts him on his ragdoll a while."),
    ("walk", "WASD steers Homer through his balance: he leans that way and steps to follow, and can trip."),
    ("game tasks", "NM -> animation (the game's tasks).\nOn: when the game's task would end (a shot, a balance, a fall...), Homer goes back to his animation: standing, or getting up if he's lying.\nOff: he stays in Delirium; he only gets up after lying still a while."),
    ("get-up", "When he lies still: animation = the game's get-up clip; muscles = his muscles try the clip's poses (he can fail); off = he stays down; learned = the trained network."),
    ("#pops", "Numbers for the damage each hit does, rising from where it landed."), ("#help", "The first-start tips again."),
    ("#sound", "8-bit sounds for shots, hits, explosions, punches and crashes."), ("#layers", "Hits dig into him: through his skin to the red flesh, and on to his bones."), ("#blood", "Little square blood pixels from each hit; they land and fade."), ("#gore", "Much more blood: big splats and pools that stay, wounds that keep spurting."),
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

fn gpu(name: &str, cost: usize) -> String {
    format!("{name}  (GPU: {})", ["varies", "low", "medium", "high"][cost])
}

fn section_title(t: &str) -> egui::RichText {
    egui::RichText::new(t).family(egui::FontFamily::Name("big".into())).size(22.0).color(INK)
}

const NAV: [(u8, &str); 8] = [(2, "Scene"), (3, "Character"), (0, "Reactions"), (1, "Interact"),
                                     (4, "Graphics / Camera"), (8, "Audio / Video"), (6, "Inspect"), (5, "Help")];

fn active_tab(bits: u16) -> u16 {
    let tabs = bits & NAV.iter().fold(0, |m, &(b, _)| m | 1 << b);
    if tabs != 0 { 1 << tabs.trailing_zeros() } else if bits & 0x80 != 0 { 1 << 2 } else { 1 }
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
    ctx.global_style_mut(|s| {
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
    let film = sim.meat.as_ref().map_or(0.0, |m| (m.xray / 0.3).min(1.0));
    let view = match ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(VIEW_RECT))) { Some(r) if sim.show_ui => r, _ => ctx.content_rect() };
    if let Some(Some(screen)) = sim.meat_screen.as_mut() {
        if film > 0.0 {
            screen.film(ctx, view, film);
        }
        if let Err(e) = screen.draw(ctx, view) {
            screen.run = None;
            sim.say(format!("skate  hall of meat screen: {e}"));
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
    let mut root = egui::Ui::new(ctx.clone(), "root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.content_rect()));
    egui::Panel::top("playback").exact_size(58.0).show(&mut root, |ui| {
        ui.horizontal_centered(|ui| {
            ui.label(egui::RichText::new("DELIRIUM").family(egui::FontFamily::Name("big".into())).size(20.0).color(HI));
            ui.label(egui::RichText::new("/ Playroom").color(DIM));
            if sim.safe {
                ui.label(egui::RichText::new("Safe Mode").strong().color(HI))
                    .on_hover_text("Started in Safe Mode: every graphics setting at its lowest, and your saved settings untouched until you change one here. Start the regular launcher for your own settings.");
            }
            ui.separator();
            if ui.button(if sim.paused { "Resume" } else { "Pause" }).on_hover_text("Pause or resume (Space)").clicked() {
                sim.paused = !sim.paused;
            }
            if ui.add_enabled(sim.paused, egui::Button::new("Step")).on_hover_text("Advance one simulation step (.)").clicked() {
                (sim.paused, sim.acc) = (true, 1.0 / 60.0);
            }
            if ui.button("Reset pose").on_hover_text(RESET_TIP).clicked() { pressed = Some("RESET pose".into()); }
            if ui.button("Reset here").on_hover_text("Stand Homer back up where he is, facing his way (Shift+R)").clicked() { pressed = Some("RESET here".into()); }
            ui.separator();
            egui::ComboBox::from_id_salt("bar body").width(64.0).selected_text(BODIES[sim.spawn_body].1).show_ui(ui, |ui| {
                for (i, (_, n, h, m, _)) in BODIES.iter().enumerate() {
                    ui.selectable_value(&mut sim.spawn_body, i, *n).on_hover_text(format!("{h:.2} m, {m:.0} kg"));
                }
            }).response.on_hover_text("The body the next spawned Homer has");
            if ui.add_enabled(sim.bench.len() + 1 < HOMERS, egui::Button::new("Spawn Homer"))
                .on_hover_text(format!("Another Homer of this body, 2 m in front of the one you control (Shift+N); up to {HOMERS}")).clicked() {
                pressed = Some(format!("spawn {}", BODIES[sim.spawn_body].0));
            }
            if ui.add_enabled(sim.can_remove(), egui::Button::new("Remove")).on_hover_text("Remove the Homer you control, a spawned one; control passes to the next (Ctrl+N)").clicked() {
                pressed = Some("remove homer".into());
            }
            if ui.add_enabled(!sim.bench.is_empty(), egui::Button::new("Next (N)")).on_hover_text("Control the next Homer: keys, pokes, drags, buttons and camera (N)").clicked() {
                pressed = Some("next homer".into());
            }
            ui.separator();
            if ui.button("Stop reactions").on_hover_text("Stop every active behaviour (X)").clicked() { pressed = Some("STOP all".into()); }
            ui.separator();
            if ui.add_enabled(sim.speed > SPEEDS.0, egui::Button::new("Slower")).on_hover_text("Halve simulation speed ([)").clicked() { pressed = Some("#slower".into()); }
            ui.label(egui::RichText::new(format!("{}× · {}", sim.speed, if sim.speed == 1.0 { "Real time" } else if sim.speed < 1.0 { "Slow motion" } else { "Fast forward" })).strong().color(HI));
            if ui.add_enabled(sim.speed != 1.0, egui::Button::new("1×")).on_hover_text("Return to real-time speed").clicked() { pressed = Some("#realtime".into()); }
            if ui.add_enabled(sim.speed < SPEEDS.1, egui::Button::new("Faster")).on_hover_text("Double simulation speed (])").clicked() { pressed = Some("#faster".into()); }
        });
    });
    egui::Panel::bottom("status").exact_size(38.0).show(&mut root, |ui| {
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
            ui.label(egui::RichText::new(format!("{:.0} FPS", 1000.0 / sim.frame_ms.max(0.1))).monospace().color(DIM))
                .on_hover_text("Frames drawn a second. Audio / Video has the settings that change it.");
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
    egui::Panel::bottom("activity").resizable(true).default_size(180.0).size_range(100.0..=320.0).show(&mut root, |ui| {
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

    egui::Panel::left("control dock").resizable(true).default_size(400.0).size_range(380.0..=480.0).frame(egui::Frame::side_top_panel(&ctx.global_style()).inner_margin(16.0)).show(&mut root, |ui| {
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
                if ui.add_sized([nav_width, 34.0], egui::Button::new(egui::RichText::new(name).size(14.0)).wrap().selected(sim.tab == 1 << bit)).on_hover_text(match bit { 4 => "Camera and appearance", 8 => "Graphics quality, resolution, shadows, frame rate and sound", _ => name }).clicked() {
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
            ui.checkbox(&mut sim.extra, "Additional experimental reactions")
                .on_hover_text("Our own uses of the reaction engine's moves, beyond the game's: pick which below.");
            let on = sim.extra;
            ui.add_enabled_ui(on, |ui| {
                ui.collapsing("Which reactions", |ui| {
                    for (i, (name, tip)) in XR.iter().enumerate() {
                        let mut b = sim.extra_pick & (1 << i) != 0;
                        if ui.checkbox(&mut b, *name).on_hover_text(*tip).changed() {
                            sim.extra_pick ^= 1 << i;
                        }
                    }
                });
            });
            ui.checkbox(&mut sim.fire_react, "Fire reaction")
                .on_hover_text("Set alight, he flails and tries to put himself out. Off: he burns (and chars, and gets hurt) without reacting.");
            ui.checkbox(&mut sim.dive, "Dive when shot at")
                .on_hover_text("Shot at again soon after a near miss, he dives out of the line of fire (once a burst: he won't keep diving under automatic fire).");
            ui.add_enabled(sim.dive, egui::Slider::new(&mut sim.dodge_gap, 0.0..=10.0).text("Dive if shot at again within").custom_formatter(|x, _| if x <= 0.0 { "never".into() } else { format!("{x:.1} s") }))
                .on_hover_text("A first near miss: he flinches. Another within this time: he dives out of the line. Rounds in the first half second (one click of an automatic) count as one.");
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
            sub(ui, "Weapon");
            let per = per_row(ui, 108.0);
            for (k, head) in [(0, "Guns"), (1, "Thrown"), (2, "Melee")] {
                ui.label(egui::RichText::new(head).weak());
                egui::Grid::new(("guns", k)).spacing([10.0, 8.0]).show(ui, |ui| {
                    for (j, (i, w)) in WEAPONS.iter().enumerate().filter(|(i, _)| kind(*i) == k).enumerate() {
                        let on = sim.weapon == i && !picked;
                        let b = ui.add_sized([108.0, 40.0], egui::Button::new(match w.0 { "deagle" => "Desert Eagle", "smg" => "SMG", "ak47" => "AK-47", "rpg" => "RPG", "pistol" => "Pistol", "sniper" => "Sniper", "shotgun" => "Shotgun", "grenade" => "Grenade", "molotov" => "Molotov", "bat" => "Bat", "knife" => "Knife", "taser" => "Taser", "baton" => "Stun baton", _ => w.0 }).selected(on));
                        if b.clicked() { pressed = Some(format!("#weapon {i}")); }
                        if j % per == per - 1 {
                            ui.end_row();
                        }
                    }
                });
            }
            if portal {
                ui.label(egui::RichText::new("Portal 2").weak());
                if ui.add_sized([108.0, 40.0], egui::Button::new("Portal gun").selected(picked)).clicked() {
                    pressed = Some("#portalgun".into());
                }
            }
            if !picked && kind(sim.weapon) == 0 {
                let (between, _, pellets, _) = sim.gun_stats();
                let damage = sim.weapons.iter().find(|w| w.name == WEAPONS[sim.weapon].1).map_or(String::new(), |w| format!("Damage {:.0}{} · ", w.damage, if pellets > 1 { " a pellet" } else { "" }));
                info(ui, &format!("{}{}{:.0} shots a minute · {} a clip", damage, if pellets > 1 { format!("{pellets} pellets · ") } else { String::new() },
                                  60000.0 / between.max(1.0), sim.clip_size().0));
            }
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
                ("car", false, "Car impact".into()), ("fall", false, "Drop Homer".into()), ("high fall", false, "High drop".into()),
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
            ui.separator();
            sub(ui, "Skate 3 · bonus");
            skate_files::settings(ui, &mut sim.skate_link, &mut pressed);
            info(ui, "Bails: Skate 3's own, or Homer onto his ragdoll (Delirium physics). Riding, walking and the board are Skate 3's either way.");
            let s3 = sim.skate3_bails;
            pressed = pressed.take().or(tiles(ui, "skatephys", &[("#skate3_bails 1", s3, "Skate 3 physics".into()), ("#skate3_bails 0", !s3, "Delirium physics".into())]));

        }

        if sim.tab & (1 << 2) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("Scene"));
            info(ui, "Choose a location, then add people or vehicles.");
            sub(ui, "Location");
            let stages: Vec<(&str, bool, String)> = STAGES.iter().chain(EXTRAS.iter()).map(|&(k, l)| (k, sim.room.stage == k, match l { "room" => "Room", "big stairs" => "Big stairs", "rooftop" => "Rooftop", "low wall" => "Low wall", "rail" => "Railing", "street" => "Street", "rope bridge" => "Rope bridge", "trampoline" => "Trampoline", "earthquake" => "Earthquake", "mechanical bull" => "Mechanical bull", "cab door" => "Taxi door test", "high ledge" => "High ledge", "path tracing test" => "Path tracing test", "coloured lights hall" => "Coloured lights hall", "skate park" => "Skate park", _ => l }.to_string())).collect();
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
            ui.horizontal_wrapped(|ui| {
                ui.label("Body");
                for (i, (_, n, h, m, _)) in BODIES.iter().enumerate() {
                    let tip = if sim.bench.is_empty() { format!("{h:.2} m, {m:.0} kg (restarts the scene)") } else { format!("{h:.2} m, {m:.0} kg: the Homer you control, where he stands") };
                    if ui.selectable_label(sim.body == i, *n).on_hover_text(tip).clicked() && sim.body != i {
                        if sim.bench.is_empty() {
                            sim.body = i;
                            sim.switch_to = Some((sim.scene.clone().unwrap_or_else(|| "room".into()), nm() == delirium::nmrs::MARGE));
                        } else {
                            pressed = Some(format!("body {}", BODIES[i].0));
                        }
                    }
                }
            });
            sub(ui, "Homers");
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("spawn body").selected_text(BODIES[sim.spawn_body].1).show_ui(ui, |ui| {
                    for (i, (_, n, h, m, _)) in BODIES.iter().enumerate() {
                        ui.selectable_value(&mut sim.spawn_body, i, *n).on_hover_text(format!("{h:.2} m, {m:.0} kg"));
                    }
                });
                if ui.add_enabled(sim.bench.len() + 1 < HOMERS, egui::Button::new("Spawn Homer")).on_hover_text("Another Homer of this body, 2 m in front of the one you control, balancing (Shift+N)").clicked() {
                    pressed = Some(format!("spawn {}", BODIES[sim.spawn_body].0));
                }
            });
            if !sim.bench.is_empty() {
                let mut all: Vec<(usize, usize, bool)> = std::iter::once((sim.homer_id, sim.body, true)).chain(sim.bench.iter().map(|b| (b.homer_id, b.body, false))).collect();
                all.sort();
                for (id, body, on) in all {
                    ui.horizontal(|ui| {
                        ui.label(format!("Homer {} · {}", id + 1, BODIES[body].1));
                        if on {
                            ui.label(egui::RichText::new("controlled").color(HI));
                        } else {
                            if ui.small_button("Control").clicked() { pressed = Some(format!("control {}", id + 1)); }
                            if ui.small_button("Remove").clicked() { pressed = Some(format!("unspawn {}", id + 1)); }
                        }
                    });
                }
            }
            info(ui, &format!("Up to {HOMERS}. N, or a click on one, controls it: the keys, pokes, drags, buttons and camera follow. Shift+N spawns, Ctrl+N removes the controlled one. Each costs a physics step."));
            slide(ui, "Strength", &mut sim.agent.strength, 0.25, 3.0, "×");
            sub(ui, "Character control");
            pressed = pressed.take().or(tiles(ui, "play", &[("Hero", sim.hero.is_some(), onoff("Play as Hero", sim.hero.is_some())),
                ("walk", sim.walk, onoff("Walk Homer", sim.walk)), ("skate", sim.riding(), onoff("Skate (J)", sim.riding())), ("passive", sim.passive, onoff("Passive Homer", sim.passive)),
                ("homer weapon", sim.homer_weapon.is_some(), format!("Homer weapon: {}", sim.homer_weapon.map_or("fists", |w| WEAPONS[w].0))),
                ("hero physics", sim.hero_physics, onoff("Hero physics", sim.hero_physics)), ("super punch", sim.super_punch, onoff("Super punch", sim.super_punch)),
                ("even fight", sim.even, onoff("Even fight", sim.even)),
                ("tackle high", sim.tackle.is_some(), "Hero: tackle high".into()), ("tackle low", sim.tackle.is_some(), "Hero: tackle low".into())]));
            sub(ui, "Damage");
            pressed = pressed.take().or(tiles(ui, "damage", &[("#blood", sim.pixel_blood, onoff("Pixel blood", sim.pixel_blood)),
                ("#gore", sim.gore, onoff("Extra bloody", sim.gore)), ("#layers", sim.layers, onoff("Layered damage", sim.layers))]));
            sub(ui, "Jump");
            pressed = pressed.take().or(tiles(ui, "jump", &[("jump", false, "Jump".into()), ("jump dive", false, "Dive".into())]));
            sub(ui, "Sever");
            let hat = matches!(sim.hat, Some(sever::Hat::On));
            let mut cut: Vec<(&str, bool, String)> = vec![("dismember", sim.dismember, onoff("Dismember", sim.dismember)), ("hat", hat, if hat { "Knock hat off".into() } else { "Hat on".into() })];
            cut.extend(sever::CUTS.iter().map(|&(n, k)| (n, sim.agent.doll.gone >> k & 1 != 0, format!("Cut {}", &n[6..]))));
            pressed = pressed.take().or(tiles(ui, "sever", &cut));
            sub(ui, "Automatic reactions");
            pressed = pressed.take().or(tiles(ui, "homer", &[("look", sim.look, onoff("Look at objects", sim.look)), ("avoid", sim.avoid, onoff("Avoid objects", sim.avoid)),
                ("game tasks", sim.tasks, onoff("Animation mode", sim.tasks)), ("get-up", sim.getup_mode != 0, format!("get-up: {}", GETUP_MODES[sim.getup_mode as usize])),
                ("infinite health", sim.god, onoff("Unlimited health", sim.god)), ("weak", sim.weak, onoff("Weaken muscles", sim.weak))]));
            egui::CollapsingHeader::new(egui::RichText::new("Character appearance").strong().size(15.0)).default_open(true).show(ui, |ui| {
            ui.label(egui::RichText::new("Body colour").color(DIM));
            let tip = |k: &str| TIPS.iter().find(|t| t.0 == k).map_or("", |t| t.1);
            ui.horizontal_wrapped(|ui| {
                for (i, (k, l)) in PED_COLORS.iter().enumerate() {
                    if ui.selectable_label(sim.ped_color == i, *l).on_hover_text(tip(k)).clicked() {
                        sim.ped_color = i;
                    }
                }
            });
            let mut see = 1.0 - sim.body_alpha;
            if ui.add_enabled(!sim.clean, egui::Slider::new(&mut see, 0.0..=0.9).text("See-through").custom_formatter(|x, _| if x <= 0.0 { "solid".into() } else { format!("{:.0}%", x * 100.0) }))
                .on_hover_text("How much you see through the bodies. Solid hides the skeleton inside; Clean view makes them solid too.").changed() {
                sim.body_alpha = 1.0 - see;
            }
            let mut see = 1.0 - sim.hero_alpha;
            if ui.add_enabled(!sim.clean, egui::Slider::new(&mut see, 0.0..=0.9).text("Hero see-through").custom_formatter(|x, _| if x <= 0.0 { "solid".into() } else { format!("{:.0}%", x * 100.0) }))
                .on_hover_text("How much you see through the Hero.").changed() {
                sim.hero_alpha = 1.0 - see;
            }
            ui.label(egui::RichText::new("Show").color(DIM));
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut sim.eyes, "Eyes").on_hover_text(tip("#eyes"));
                ui.checkbox(&mut sim.brows, "Eyebrows").on_hover_text(tip("#brows"));
                ui.checkbox(&mut sim.skel, "Skeleton").on_hover_text(tip("#skel"));
                ui.add_enabled_ui(sim.skel, |ui| {
                    egui::ComboBox::from_id_salt("skel_style").width(80.0).selected_text(SKEL_STYLES[sim.skel_style as usize]).show_ui(ui, |ui| {
                        for k in 0..3u8 {
                            ui.selectable_value(&mut sim.skel_style, k, SKEL_STYLES[k as usize]);
                        }
                    }).response.on_hover_text("Lines: white bones, coloured joints. Muscles: solid red bones and yellow joints. Stick: all yellow.");
                });
                ui.checkbox(&mut sim.clean, "Clean view").on_hover_text(tip("#clean"));
            });
            });
            sub(ui, "Experimental movement");
            pressed = pressed.take().or(tiles(ui, "preview_move", &[("experimental walk", sim.preview_on, "Experimental walk · W walk, Shift jog".into())]));
            if sim.watch.is_some() || [learn::path(), learn::walk_path(false), learn::walk_path(true)].iter().any(|p|
                std::path::Path::new(p).exists() || std::path::Path::new(&learn::live_path(p)).exists()) {
            sub(ui, "Learned movement · local training");
            pressed = pressed.take().or(tiles(ui, "learned_move", &[
                ("walk learned", sim.walk_learned.as_ref().is_some_and(|w| !w.3), "Learned WASD walk".into()),
                ("run learned", sim.walk_learned.as_ref().is_some_and(|w| w.3), "Learned WASD run".into())]));
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
            ui.label(section_title("Graphics / Camera"));
            info(ui, "The camera, lighting, shadows and effects. (GPU: ...) is what a setting costs a slow graphics card.");
            pressed = pressed.take().or(tiles(ui, "view", &[("camera", sim.cam_locked, onoff("Follow camera", sim.cam_locked)),
                ("#pops", sim.show_pops, onoff("Damage numbers", sim.show_pops)), ("#meat", sim.meat_on, onoff("Hall of Meat", sim.meat_on))]));
            slide(ui, "Mouse sensitivity", &mut sim.sens, 0.05, 2.0, "");
            if ui.button("Enter photo mode  ·  P").on_hover_text("Hide the interface and move the camera freely; P returns to the room.").clicked() {
                sim.photo_pending = true;
            }
            ui.add_space(6.0);
            egui::CollapsingHeader::new(egui::RichText::new("Renderer").strong().size(15.0)).default_open(true).show_unindented(ui, |ui| {
                let mut q = sim.rt as f32;
                if ui.add_enabled(sim.rt_ok, egui::Slider::new(&mut q, 0.0..=2.0).step_by(1.0).text(gpu("Renderer", 3)).custom_formatter(|x, _| ["raster", "ray traced", "path traced"][x as usize].into()))
                    .on_hover_text("Raster: the usual game picture, fast (shadow maps, ambient occlusion). Ray traced: light and shadows traced as rays (Bevy's Solari), bouncing off the walls and floor, in real time; with an RTX card DLSS's Ray Reconstruction cleans up the noise (anti-aliasing turns to DLAA if it was below it), other cards are grainy. Path traced: the true picture, every bounce of light traced; with an RTX card DLSS's Ray Reconstruction cleans it up as it goes (as games' path tracing modes do), so it holds up while the camera moves; other cards build it up over a few seconds while the camera holds still.")
                    .on_disabled_hover_text("Needs a graphics card that can ray trace (RTX, RX 6000 or newer).").changed() {
                    sim.rt = q as u8;
                }
                ui.add_enabled(sim.rt_now() == 1, egui::Checkbox::new(&mut sim.restir, gpu("ReSTIR", 2)))
                    .on_hover_text("Ray-traced lighting reuses light paths between pixels and frames: finer shadows and steadier light, for a good chunk of speed.")
                    .on_disabled_hover_text("With ray-traced lighting (Renderer: ray traced).");
            });
            ui.add_space(6.0);
            egui::CollapsingHeader::new(egui::RichText::new("Room appearance").strong().size(15.0)).default_open(true).show_unindented(ui, |ui| {
                let looks: Vec<(&str, bool, String)> = LOOKS.iter().map(|(n, l)| (*n, sim.skin == *l, n.to_string())).collect();
                if let Some(n) = tiles(ui, "looks", &looks) {
                    pressed = Some(format!("#look {n}"));
                }
                pressed = pressed.take().or(tiles(ui, "look on", &[("#grid", sim.skin.grid, onoff("Grid lines", sim.skin.grid)), ("#fog", sim.skin.fog, onoff("Fog", sim.skin.fog)), ("#infinite", sim.infinite, onoff("Extend floor", sim.infinite)),
                    ("#ripples", sim.show_ripples, onoff("Floor ripples", sim.show_ripples)), ("#photosky", sim.sky == 1, onoff("Photo sky", sim.sky == 1)),
                    ("#bevysky", sim.sky == 2, onoff("Bevy sky", sim.sky == 2)), ("#godrays", sim.god_rays, onoff("God rays", sim.god_rays))]));
            });
            ui.add_space(6.0);
            egui::CollapsingHeader::new(egui::RichText::new("Custom colours & lighting").strong().size(15.0)).default_open(true).show_unindented(ui, |ui| {
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
                ui.add(egui::Slider::new(&mut sim.sun_dir[0], -180.0..=180.0).text("Sun direction").suffix("°"));
                ui.add(egui::Slider::new(&mut sim.sun_dir[1], 3.0..=89.0).text("Sun height").suffix("°"));
            });
            ui.add_space(6.0);
            egui::CollapsingHeader::new(egui::RichText::new("Lights & shadows").strong().size(15.0)).default_open(true).show_unindented(ui, |ui| {
                let mut f = if sim.soft { sim.fill_k } else { 0.0 };
                if ui.add(egui::Slider::new(&mut f, 0.0..=3.0).text("Fill light").custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{x:.2}×") }))
                    .on_hover_text("How bright the shadows and the dark side are: off is near black, high is flat and washed out.").changed() {
                    (sim.soft, sim.fill_k) = (f > 0.0, if f > 0.0 { f } else { sim.fill_k });
                }
                            let mut q = if sim.shadows { sim.shadow_q as f32 } else { 0.0 };
                if ui.add(egui::Slider::new(&mut q, 0.0..=3.0).step_by(1.0).text(gpu("Shadow quality", 3)).custom_formatter(|x, _| ["off", "low", "medium", "high"][x as usize].into()))
                    .on_hover_text("Low: a coarse shadow near the camera, crisp but jagged. Medium: twice the detail, smoothed, further out. High: twice that again, out to the horizon; with TAA, DLAA, DLSS, FSR 3 or XeSS it smooths over frames (no grain on soft shadows). Shadows are the biggest cost after resolution.").changed() {
                    (sim.shadows, sim.shadow_q) = (q > 0.0, (q as u8).max(1));
                }
                ui.add_enabled(sim.shadows, egui::Slider::new(&mut sim.shadow, 0.0..=SUN_SIZE_MAX).logarithmic(true).smallest_positive(0.25).text(gpu("Sun size", 2)).custom_formatter(|x, _| if x <= 0.0 { "point (hard)".into() } else { format!("{x:.1}°") }))
                    .on_hover_text("How big the sun is. Like real shadows, they stay sharp where they touch and soften further away; a bigger sun softens more. The real sun is about 0.5°, a hazy sky 2–5°; a point is sharp everywhere. Costs more on a slow graphics card; grainy without TAA, DLAA, DLSS, FSR 3 or XeSS.");
                if ui.add_enabled(sim.shadows && (sim.shadow - REAL_SUN).abs() > 0.005, egui::Button::new("Real sun")).on_hover_text("Sun size as the real sun's, 0.53°: sharp shadows near, a little soft far off.").clicked() {
                    sim.shadow = REAL_SUN;
                }
                let mut f = sim.shadow_filter as f32;
                if ui.add_enabled(sim.shadows, egui::Slider::new(&mut f, 0.0..=3.0).step_by(1.0).text("Shadow edges").custom_formatter(|x, _| ["match anti-aliasing", "soft", "smoothed over frames", "sharp"][x as usize].into()))
                    .on_hover_text("Raster's shadow edge filter. Match anti-aliasing: sharp at Low shadows, else smoothed over frames with TAA, DLAA, DLSS, FSR 3 or XeSS and soft without. Soft: a blurred edge, steady. Smoothed over frames: soft without the grain, needs one of those methods. Sharp: crisp, jagged.").changed() {
                    sim.shadow_filter = f as u8;
                }
                ui.checkbox(&mut sim.glass_tint, "Stained glass colour").on_hover_text("The path tracing test's stained glass coloured, in raster and the ray tracers (off: plain clear glass, clear light through it).");
                ui.add_enabled(sim.glass_tint, egui::Checkbox::new(&mut sim.glass_shade, "Coloured glass light")).on_hover_text("The sun through the stained glass takes its colours onto the floor and walls, in raster and the ray tracers (off: the light through it clear, the glass still coloured).")
                    .on_disabled_hover_text("With Stained glass colour.");
            });
            ui.add_space(6.0);
            egui::CollapsingHeader::new(egui::RichText::new("Effects").strong().size(15.0)).default_open(true).show_unindented(ui, |ui| {
                let mut q = sim.ssao as f32;
                if ui.add(egui::Slider::new(&mut q, 0.0..=4.0).step_by(1.0).text(gpu("Ambient occlusion", 2)).custom_formatter(|x, _| ["off", "low", "medium", "high", "ultra"][x as usize].into()))
                    .on_hover_text("Darkens creases and corners where light can't reach, like under Homer's arms and where he meets the floor (SSAO). Grainy without TAA, DLAA, DLSS, FSR 3 or XeSS.").changed() {
                    sim.ssao = q as u8;
                }
                ui.add_enabled(sim.ssao > 0, egui::Slider::new(&mut sim.ao_strength, 0.0..=2.0).text("Ambient occlusion strength").custom_formatter(|x, _| format!("{:.0}%", x * 100.0)))
                    .on_hover_text("How dark ambient occlusion makes the creases. It darkens all the light there, the sun's too, as most games do.");
                ui.horizontal(|ui| {
                    let mut q = sim.fire as f32;
                    if ui.add(egui::Slider::new(&mut q, 0.0..=FIRE_HIGH as f32).step_by(1.0).text(gpu("Fire", 1))
                        .custom_formatter(|x, _| ["wireframe", "flames", "flames + light", "flames + light on Homer"][x as usize].into()))
                        .on_hover_text("Wireframe: plain rings, for a slow PC. Flames: flames and embers, no light from them. Flames + light: they light the room. \
                                        Flames + light on Homer: burning Homer glows and his flames light him from every side.").changed() {
                        sim.set_fire(q as u8);
                    }
                    ui.add_enabled(sim.fire > 0, egui::Checkbox::new(&mut sim.real_fire, "Realistic")).on_hover_text("Rolling, glowing flames instead of voxels (cubes).");
                    ui.add_enabled(sim.fire > 0, egui::Checkbox::new(&mut sim.smoke, "Smoke"));
                    ui.checkbox(&mut sim.fire_light, "Fire light").on_hover_text("The flames light the room (the molotov's and burning fires'); off: they give no light.");
                    ui.checkbox(&mut sim.fire_homer, "Light on Homer").on_hover_text("Burning Homer glows and his flames light him from every side (needs Fire light for the light round him).");
                });
            });
            ui.add_space(6.0);
            egui::CollapsingHeader::new(egui::RichText::new("Water").strong().size(15.0)).default_open(true).show_unindented(ui, |ui| {
                let mut q = sim.water_q as f32;
                if ui.add(egui::Slider::new(&mut q, 0.0..=4.0).step_by(1.0).text(gpu("Water", 1)).custom_formatter(|x, _| ["plain", "ripples", "ripples + refraction", "Extra High", "Max"][x as usize].into()))
                    .on_hover_text("Plain: a see-through blue, for a slow PC. Ripples: the breeze's and splashes' ripples catch the light.                                 Ripples + refraction: what's under the water bends and gets bluer the deeper it is. Extra High: the surface itself moves, finer ripples, caustics (sunlight focused by the ripples) on the pool's floor and walls, spray from splashes, a blue haze underwater. Max: finer caustics and three times the spray.").changed() {
                    sim.set_water(q as u8);
                }
                ui.add(egui::Checkbox::new(&mut sim.voxel_water, "Voxel water (experimental)")).on_hover_text("The water's surface in blocks, splashes and all. Raster only: the ray tracers draw the smooth water.");
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut sim.water_ripples, "Ripples").on_hover_text("The pool's own surface: the breeze's and splashes' ripples catch the light, glints on it (off: a flat, smooth surface). Real water's spill and splashes: Fine ripples.");
                    ui.add_enabled(sim.rt > 0, egui::Checkbox::new(&mut sim.water_traced, "Ray-traced water")).on_hover_text("Ray traced and path traced: the pool's water traced as glass, the light bent through it and its haze from in it the ray tracer's (off: raster's water over the traced room; the slider sets it on from Extra High).")
                        .on_disabled_hover_text("With ray tracing (Renderer).");
                    ui.checkbox(&mut sim.water_refract, "Refraction").on_hover_text("What's under the water bent and bluer the deeper it is, in raster and the ray tracers.");
                    ui.checkbox(&mut sim.water_depth, "Depth colour").on_hover_text("The colour the water takes on with depth: red soaked up the deeper it is, the blue-green haze seen from in it (off: clear).");
                    ui.checkbox(&mut sim.water_caustics, "Caustics").on_hover_text("The sun's light through the ripples, focused into bright lines on the pool's floor and walls.");
                    ui.add_enabled(sim.water_caustics && sim.rt > 0, egui::Checkbox::new(&mut sim.photons, "Photon caustics")).on_hover_text("Ray and path traced: the caustics traced by photons on the graphics card (off: worked out on the CPU, as without ray tracing).");
                    ui.add_enabled(sim.water_caustics, egui::Checkbox::new(&mut sim.water_fine, "Finer caustics")).on_hover_text("The caustics on a texture twice as fine (sharper lines, more memory and work), in raster and the CPU's for the ray tracers.");
                    ui.checkbox(&mut sim.water_spray, "Spray & foam").on_hover_text("Drops thrown up where things hit the water (without ray tracing); with Real water, its spray, foam and bubbles (ray traced too).");
                    ui.checkbox(&mut sim.water_waves, "Moving surface").on_hover_text("The surface itself rises and falls with the ripples (without ray tracing).");
                    ui.checkbox(&mut sim.water_pixels, "Pixel look").on_hover_text("The ripples drawn in blocky pixels, a glint a pixel, and the wet look's patches blocky (off: smooth).");
                    ui.checkbox(&mut sim.water_haze, "Underwater haze").on_hover_text("With the camera under the water: a blue haze the sun shines through in shafts (without ray tracing).");
                    ui.checkbox(&mut sim.water_wet, "Wet look").on_hover_text("Homer and Hero wet from the water: darker and glossier where it reached them, drying over ~25 s, dripping. Extra High and up: the wet runs down and pools, beads, a water film.");
                    ui.add_enabled(sim.water_wet, egui::Checkbox::new(&mut sim.wet_film, "Water film")).on_hover_text("Wet look: the wet runs down the skin and pools at the low points, beads, a glossy clear coat and richer colour (off: darkening and shine, drying evenly).");
                    ui.add_enabled(sim.water_wet, egui::Checkbox::new(&mut sim.wet_drips, "Drips")).on_hover_text("Wet look: drops falling off wet bodies (without ray tracing).");
                    ui.checkbox(&mut sim.sky_shade, "Sky pool shadow").on_hover_text("The sky pool's shadow on the floor tinted by its water, softly edged (without ray tracing: they do it themselves). Path tracing test.");
                    ui.checkbox(&mut sim.water_mirror, "Reflections").on_hover_text("The room mirrored in the water (without ray tracing: the ray tracers reflect it themselves).");
                    ui.checkbox(&mut sim.water_foam, "Surface foam").on_hover_text("With Real water and Spray & foam: the foam as a lacy sheet on the water, solid where it's thick, a web of bubbles where it thins, carried along and popping (off: foam as flecks).");
                    ui.checkbox(&mut sim.water_mist, "Splash mist").on_hover_text("With Real water and Spray & foam: hard splashes throw off a fine mist that drifts, spreads and fades in a couple of seconds (raster; the ray tracers leave it out).");
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.checkbox(&mut sim.real_water, gpu("Real water (experimental)", 2)).on_hover_text("The pool's water simulated for real (particles on a grid, on the graphics card; the sky pool's too in the path tracing test): bodies and things push it about and it pushes back (they float, bob and get carried off), it sloshes and rises. A Flood with it: real water poured in, spilling over.").changed() {
                        sim.water_sync();
                    }
                    if ui.add_enabled(sim.room.pool, egui::Button::selectable(sim.pouring, "Flood"))
                        .on_hover_text("Pours water into the pool till it spills over its sides and runs off across the floor (with Real water, real water; without, a cheaper one: a height a square). Again: stops pouring.")
                        .on_disabled_hover_text("In the room.").clicked() {
                        sim.flood_toggle();
                    }
                    if ui.add_enabled(sim.real_water || sim.flood.is_some(), egui::Button::new("Drain")).on_hover_text("The pool back as it was, the floor dry.").clicked() {
                        sim.drain();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.real_surface, "Water surface")).on_hover_text("Real water: the water in the air (the pour, splashes) a smooth surface round it, in raster, ray and path tracing (off: only the pool's surface and the spill on the floor, a little faster).")
                        .on_disabled_hover_text("With Real water.");
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.water_clear, "Clear spill")).on_hover_text("Real water: its spill on the floor a clear thin film, as real: the floor darker where it's wet, the sky and sun glinting in it, faint caustics only where it's deep (off: the pool's blue).")
                        .on_disabled_hover_text("With Real water.");
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.wet_trail, "Wet trail")).on_hover_text("Real water: the floor stays dark where the spill has been after it's gone, drying over half a minute (raster).")
                        .on_disabled_hover_text("With Real water.");
                    ui.add_enabled(sim.real_water && sim.water_clear, egui::Checkbox::new(&mut sim.water_impact, "Impact foam")).on_hover_text("Real water: white where water lands on its clear spill (under the rim's overflow, splashes coming down; raster).")
                        .on_disabled_hover_text("With Real water and Clear spill.");
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.water_faucet, "Faucet")).on_hover_text("Real water: the pour comes out of a fill pipe over the pool's near end (off: out of the air).")
                        .on_disabled_hover_text("With Real water.");
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.real_ripples, "Fine ripples")).on_hover_text("Real water: small ripples running over its spill on the floor and its splashes (in Pixel look, blocky), and the sky pool's breeze in the path tracing test (the pool's own surface: Ripples).")
                        .on_disabled_hover_text("With Real water.");
                });
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Water physics").strong());
                ui.horizontal_wrapped(|ui| {
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.water_relax, "Keep volume")).on_hover_text("Real water: the water's thickness (its particles a cell) kept even: crowded spots spread out, sparse ones pulled in, so the water doesn't swell after splashes and pours (the slider sets it on from High).")
                        .on_disabled_hover_text("With Real water.");
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.water_vort, "Vorticity")).on_hover_text("Real water: swirls and eddies (behind a swimmer, under a pour) kept alive instead of smoothed away (the slider sets it on from Extra High).")
                        .on_disabled_hover_text("With Real water.");
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.water_spray_phys, "Spray physics")).on_hover_text("Real water: spray thrown where the water's churned and cresting, its drops slowed by the air by their size (fine mist stops short), and more simulation steps in fast splashes (the slider sets it on from Extra High).")
                        .on_disabled_hover_text("With Real water.");
                    ui.add_enabled(sim.real_water, egui::Checkbox::new(&mut sim.water_ghost, "Smooth surface pressure")).on_hover_text("Real water: the pressure at the water's surface worked out where the surface really is, not a cell out: no steps in the surface, waves that run smooth (the slider sets it on from High).")
                        .on_disabled_hover_text("With Real water.");
                });
            });
            ui.add_space(6.0);
            egui::CollapsingHeader::new(egui::RichText::new("Lens").strong().size(15.0)).default_open(true).show_unindented(ui, |ui| {
                ui.add(egui::Slider::new(&mut sim.bloom, 0.0..=1.0).text(gpu("Bloom", 1)).custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{:.0}%", x * 100.0) }))
                    .on_hover_text("A soft glow around bright things, like a camera lens.");
                ui.add(egui::Slider::new(&mut sim.dof, 0.0..=1.0).text(gpu("Depth of field", 2)).custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{:.0}%", x * 100.0) }))
                    .on_hover_text("Blurs what's nearer or further than what the camera orbits, like a real lens; more blurs more.");
                ui.add(egui::Slider::new(&mut sim.contact, 0.0..=1.0).text(gpu("Contact shadows", 1)).custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{:.0} cm", x * 100.0) }))
                    .on_hover_text("Fine sun shadows where things touch, which the shadow map is too coarse for: between the fingers, under the feet. The number is how far they reach.");
                ui.add_enabled(sim.contact > 0.0, egui::Slider::new(&mut sim.contact_k, 0.0..=1.0).text("Contact shadow strength").custom_formatter(|x, _| format!("{:.0}%", x * 100.0)))
                    .on_hover_text("How dark the contact shadows are.");
                ui.add(egui::Slider::new(&mut sim.vignette, 0.0..=1.0).text(gpu("Vignette", 1)).custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{:.0}%", x * 100.0) }))
                    .on_hover_text("Darkens the corners, like a camera lens.");
                ui.add(egui::Slider::new(&mut sim.lens, 0.0..=1.0).text(gpu("Lens distortion", 1)).custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{:.0}%", x * 100.0) }))
                    .on_hover_text("Bends straight lines near the edges, like a wide camera lens.");
                ui.add(egui::Slider::new(&mut sim.chroma, 0.0..=1.0).text(gpu("Chromatic aberration", 1)).custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{:.0}%", x * 100.0) }))
                    .on_hover_text("Splits the colours apart toward the edges, like a cheap lens.");
                info(ui, "Screen-space reflections need Bevy's deferred renderer, which the room doesn't use; not offered yet.");
            });
            ui.horizontal(|ui| {
                let mut k = sim.sun_color;
                if ui.color_edit_button_rgb(&mut k).on_hover_text("The sun's colour: warm for sunset, blue for moonlight.").changed() {
                    sim.sun_color = k;
                }
                ui.label("Sun colour");
            });
            ui.horizontal(|ui| {
                if ui.button("Reset sun angle").clicked() {
                    sim.sun_dir = SUN_DIR;
                }
                if ui.button("Reset tab").on_hover_text("Lights, shadows and effects back to how they start.").clicked() {
                    reset_settings(&mut sim, &["shadows", "shadow_quality", "sun_size", "soft", "fill", "sun_r", "sun_g", "sun_b", "ssao", "ao_strength", "bloom", "dof", "contact", "contact_strength", "vignette", "lens", "chroma", "fire", "real_fire", "smoke", "water_quality", "voxel_water", "photon_caustics", "caustics", "spray", "waves", "pixels", "haze", "mirror", "foam", "mist", "wet", "wet_film", "wet_drips", "sky_shade", "traced_water", "glass_tint", "glass_shadows", "water_ripples", "water_refraction", "finer_caustics", "depth_colour", "keep_volume", "ghost_surface", "vorticity", "spray_physics", "water_surface", "fine_ripples", "clear_spill", "wet_trail", "impact_foam", "faucet", "fire_light", "fire_homer", "shadow_filter"]);
                    sim.sun_dir = SUN_DIR;
                }
            });
        }

        if sim.tab & (1 << 8) != 0 {
            ui.add_space(6.0);
            ui.label(section_title("Audio / Video"));
            info(ui, &format!("{:.0} FPS ({:.1} ms a frame). On a slow graphics card, lower Graphics quality first; Resolution is the last resort.", 1000.0 / sim.frame_ms.max(0.1), sim.frame_ms));
            let custom = preset(&sim);
            let mut q = custom.unwrap_or(1) as f32;
            if ui.add(egui::Slider::new(&mut q, 0.0..=2.0).step_by(1.0).text("Graphics quality")
                .custom_formatter(|x, _| if custom.is_none() { "custom".into() } else { ["Low", "Medium", "High"][x as usize].into() }))
                .on_hover_text("Sets edge smoothing, shadow quality, the sun's size, fire and water together. Low: off, no shadows, wireframe fire, plain water. Medium: 4x, medium, a point sun, flames + light, rippling water. High: 4x, high, a 1° sun, flames + light on Homer, ripples and refraction.").changed() {
                let (m, s, soft, fire, water) = VIDEO_PRESETS[q as usize];
                sim.set_fire(fire);
                sim.water_q = water;
                sim.water_traced = water >= 3;
                (sim.water_ripples, sim.water_refract, sim.photons, sim.water_fine, sim.water_depth) = (water >= 1, water >= 2, water >= 2, water >= 4, water >= 1);
                (sim.water_relax, sim.water_ghost, sim.water_vort, sim.water_spray_phys) = (water >= 2, water >= 2, water >= 3, water >= 3);
                (sim.aa, sim.shadows, sim.shadow_q) = (if m == 1 { 0 } else { AA_MSAA }, s > 0, s.max(1));
                sim.msaa = m.max(2);
                sim.shadow = if !soft { 0.0 } else if sim.shadow > 0.0 { sim.shadow } else { REAL_SUN };
            }
            sub(ui, "Graphics card");
            let names: Vec<String> = std::iter::once(String::new()).chain(sim.gpus.clone().unwrap_or_default()).collect();
            ui.horizontal_wrapped(|ui| {
                for n in names {
                    let label = if n.is_empty() { "Automatic (fastest)".to_string() } else { n.clone() };
                    if ui.selectable_label(sim.gpu.eq_ignore_ascii_case(&n), label).clicked() {
                        sim.gpu = n;
                    }
                }
            });
            let wants = !sim.gpu.is_empty() && !sim.gpu_now.to_lowercase().contains(&sim.gpu.to_lowercase());
            info(ui, &format!("In use: {}.{}", sim.gpu_now, if wants { " Restart the room to switch." } else { "" }));
            sub(ui, "Display");
            let up = sim.upscaling();
            ui.add_enabled(!up, egui::Slider::new(&mut sim.render_scale, RENDER_SCALE_MIN..=RENDER_SCALE_MAX).text(gpu("Resolution", 3)).custom_formatter(|x, _| format!("{:.0}%", x * 100.0)))
                .on_hover_text("How sharp the 3D view is drawn, as a share of the window's pixels. Lower is blurrier and much faster on a slow graphics card; above 100% it draws more pixels than the window has and shrinks them down, the smoothest edges there are (200% costs about four times 100%).");
            if sim.render_scale < 0.999 && !up {
                let mut u = sim.upscaler as f32;
                if ui.add(egui::Slider::new(&mut u, 0.0..=UP_FSR1 as f32).step_by(1.0).text("Upscaler").custom_formatter(|x, _| UP_NAMES[x as usize].into()))
                    .on_hover_text("How the lower Resolution is stretched over the window. Basic just smooths it; FSR 1 (AMD FidelityFX Super Resolution 1, any graphics card) rebuilds sharp edges, and Sharpening sharpens after it.").changed() {
                    sim.upscaler = u as u8;
                }
            }
            let v = sim.view_px.as_vec2();
            let r = match sim.aa_now() {
                AA_DLSS => [1.5, 1.72, 2.0, 3.0][sim.dlss_q as usize],
                AA_FSR3 => [1.0, 1.5, 1.7, 2.0, 3.0][sim.fsr_q as usize],
                AA_XESS => [1.0, 1.7, 2.0, 2.3, 3.0][sim.xess_q as usize],
                _ => 1.0,
            };
            let s = if (sim.render_scale - 1.0).abs() > 0.001 && !up { sim.render_scale } else { 1.0 };
            let (drawn, shown) = ((v / r).round(), (v / s).round());
            info(ui, &format!("Drawn at {}×{} pixels, shown at {}×{}.", drawn.x, drawn.y, shown.x, shown.y));
            ui.add(egui::Slider::new(&mut sim.fps_limit, FPS_VSYNC..=FPS_FREE).step_by(1.0).text("Frame rate limit")
                .custom_formatter(|x, _| if x <= FPS_VSYNC as f64 { "monitor (V-Sync)".into() } else if x >= FPS_FREE as f64 { "unlimited".into() } else { format!("{x:.0} FPS") }))
                .on_hover_text("The most frames a second it draws. Monitor matches your screen and never tears; a number caps it there (lower keeps the graphics card cooler and quieter); unlimited is as fast as it can go. With the window in the background it never goes over 60.");
            sub(ui, "Anti-aliasing & upscaling");
            let methods: Vec<u8> = (0..=AA_TAA).chain(sim.fsr_ok.then_some(AA_FSR3)).chain(sim.xess_ok.then_some(AA_XESS)).chain(if sim.dlss_ok { vec![AA_DLAA, AA_DLSS] } else { vec![] }).collect();
            let mut aa = sim.aa_now();
            egui::ComboBox::from_label(gpu("Anti-aliasing", 0)).selected_text(AA_NAMES[aa as usize]).show_ui(ui, |ui| {
                for &k in &methods {
                    ui.selectable_value(&mut aa, k, AA_NAMES[k as usize]);
                }
            }).response.on_hover_text("Smooths jagged edges; one method at a time. FXAA: cheapest, a little blurry. SMAA: sharper, still cheap. MSAA: clean edges, costs more on a slow graphics card. TAA: smoothest, can smear when things move fast. FSR 3: AMD's, on any graphics card; draws the room smaller and rebuilds it sharp, much faster. XeSS: Intel's, like FSR 3, best on Intel Arc cards. DLAA: NVIDIA's, the best looking. DLSS: NVIDIA's, like FSR 3, sharper.");
            if aa != sim.aa_now() {
                sim.aa = aa;
            }
            if !sim.fsr_ok {
                info(ui, if cfg!(windows) { "FSR 3 needs amd_fidelityfx_vk.dll beside the room." } else { "FSR 3 is Windows only for now." });
            }
            if !sim.xess_ok {
                info(ui, if cfg!(windows) { "XeSS needs libxess.dll beside the room." } else { "XeSS is Windows only for now." });
            }
            if !sim.dlss_ok {
                info(ui, if cfg!(feature = "dlss") { "DLAA and DLSS need an NVIDIA RTX graphics card." } else { "DLAA and DLSS aren't in this build." });
            }
            match sim.aa {
                AA_SMAA => {
                    let mut q = sim.smaa_q as f32;
                    if ui.add(egui::Slider::new(&mut q, 0.0..=3.0).step_by(1.0).text(gpu("SMAA quality", 1)).custom_formatter(|x, _| ["low", "medium", "high", "ultra"][x as usize].into()))
                        .on_hover_text("How hard SMAA looks for edges. Higher catches more, costs a little more.").changed() {
                        sim.smaa_q = q as u8;
                    }
                }
                AA_MSAA => {
                    let steps: Vec<u8> = [2, 4, 8].into_iter().filter(|&k| k <= sim.msaa_max).collect();
                    let mut i = steps.iter().position(|&k| k == sim.msaa).unwrap_or(0) as f32;
                    if steps.len() > 1 && ui.add(egui::Slider::new(&mut i, 0.0..=(steps.len() - 1) as f32).step_by(1.0).text(gpu("MSAA samples", 2))
                        .custom_formatter(|x, _| format!("{}x", steps[x as usize])))
                        .on_hover_text("More samples, smoother edges; each step costs more on a slow graphics card.").changed() {
                        sim.msaa = steps[i as usize];
                    }
                }
                AA_FSR3 | AA_XESS => {
                    let name = AA_NAMES[sim.aa as usize];
                    let field = if sim.aa == AA_FSR3 { &mut sim.fsr_q } else { &mut sim.xess_q };
                    let mut q = *field as f32;
                    if ui.add(egui::Slider::new(&mut q, 0.0..=4.0).step_by(1.0).text(format!("{name} mode"))
                        .custom_formatter(|x, _| ["Native AA", "Quality", "Balanced", "Performance", "Ultra Performance"][x as usize].into()))
                        .on_hover_text(format!("How small {name} draws the room before rebuilding it. Native AA keeps full size, only smoothing; Quality is sharpest, Ultra Performance fastest.")).changed() {
                        *field = q as u8;
                    }
                }
                AA_DLSS => {
                    let mut q = sim.dlss_q as f32;
                    if ui.add(egui::Slider::new(&mut q, 0.0..=3.0).step_by(1.0).text("DLSS mode")
                        .custom_formatter(|x, _| ["Quality", "Balanced", "Performance", "Ultra Performance"][x as usize].into()))
                        .on_hover_text("How small DLSS draws the room before rebuilding it. Quality is sharpest, Ultra Performance fastest.").changed() {
                        sim.dlss_q = q as u8;
                    }
                }
                _ => {}
            }
            ui.add(egui::Slider::new(&mut sim.sharpen, 0.0..=1.0).text(gpu("Sharpening", 1)).custom_formatter(|x, _| if x <= 0.0 { "off".into() } else { format!("{:.0}%", x * 100.0) }))
                .on_hover_text("Crisps the picture after upscaling or a lower Resolution. Too much looks grainy. Each anti-aliasing method keeps its own.");
            sub(ui, "Audio");
            pressed = pressed.take().or(tiles(ui, "audio", &[("#sound", sim.sound, onoff("Sound", sim.sound))]));
            if ui.add_enabled(sim.sound, egui::Button::new("Test sound")).on_hover_text("Play an impact sound through the current Windows output device.").clicked() { sim.sounds.push(sfx::Snd::Hit); }
            info(ui, "8-bit effects for pokes, shots, hits and crashes. No background music. If Test sound is silent, check Delirium in the Windows volume mixer.");
            slide(ui, "Master volume", &mut sim.volume[0], 0.0, 1.0, "");
            slide(ui, "Sound effects", &mut sim.volume[1], 0.0, 1.0, "");
            #[cfg(feature = "portal")]
            slide(ui, "Portal 2 sounds", &mut sim.volume[2], 0.0, 1.0, "").on_hover_text("The portal gun's shots and portals opening (with Portal 2 on).");
            ui.add_space(6.0);
            if ui.button("Reset tab").on_hover_text("Graphics card, display, anti-aliasing and sound back to how they start.").clicked() {
                reset_settings(&mut sim, &["render_scale", "msaa", "gpu", "aa", "smaa", "dlss_mode", "sharpen", "sharp_by", "fps_limit", "upscaler", "fsr_mode", "xess_mode", "sound", "vol", "vol_fx", "vol_portal"]);
            }
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
                                  ("right drag", "orbit the camera"), ("wheel", "zoom"), ("WASD", "walk Homer when Walk Homer is on; otherwise move only in free camera"), ("middle drag", "pan (enters the detached camera)"), ("C  or  Tab", "follow Homer / detached camera: hold right mouse to look and fly (WASD, Q/E, Shift boost) while the Hero stays put"),
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
    let view_rect = root.available_rect_before_wrap();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(VIEW_RECT), view_rect));
    sim.over_ui = ctx.is_pointer_over_egui() || ctx.egui_wants_pointer_input() || ctx.pointer_hover_pos().is_some_and(|p| !view_rect.contains(p));
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
            (Some("#skate3"), _) => sim.skate_link.link(),
            (Some("#look"), Some(n)) => sim.skin = LOOKS.iter().find(|l| l.0 == n).map_or(sim.skin, |l| l.1),
            (Some("#grid"), _) => sim.skin.grid = !sim.skin.grid,
            (Some("#photosky"), _) => sim.sky = if sim.sky == 1 { 0 } else { 1 },
            (Some("#godrays"), _) => sim.god_rays = !sim.god_rays,
            (Some("#bevysky"), _) => sim.sky = if sim.sky == 2 { 0 } else { 2 },
            (Some("#infinite"), _) => sim.infinite = !sim.infinite,
            (Some("#pedcol"), Some(n)) => sim.ped_color = PED_COLORS.iter().position(|c| c.0 == n).unwrap_or(sim.ped_color),
            (Some("#eyes"), _) => sim.eyes = !sim.eyes,
            (Some("#skel"), _) => sim.skel = !sim.skel,
            (Some("#clean"), _) => sim.clean = !sim.clean,
            (Some("#brows"), _) => sim.brows = !sim.brows,
            (Some("#fog"), _) => sim.skin.fog = !sim.skin.fog,
            (Some("#shadows"), _) => sim.shadows = !sim.shadows,
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
            (Some("skate"), _) => sim.toggle_skate(),
            (Some("weak"), _) => sim.weak = !sim.weak,
            (Some("#pops"), _) => sim.show_pops = !sim.show_pops,
            (Some("#meat"), _) => sim.meat_on = !sim.meat_on,
            (Some("#skate3_bails"), Some(v)) => sim.skate3_bails = v == "1",
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
            (Some("#gore"), _) => sim.gore = !sim.gore,
            (Some("#layers"), _) => {
                sim.layers = !sim.layers;
                sim.voxels.clear();
                sim.bench.iter_mut().for_each(|b| b.voxels.clear());
            }
            (Some("#slower"), _) => sim.speed = (sim.speed * 0.5).max(SPEEDS.0),
            (Some("#faster"), _) => sim.speed = (sim.speed * 2.0).min(SPEEDS.1),
            _ => sim.button(&p),
        }
    }
    Ok(())
}

#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[test]
#[ignore = "needs the game; timing: cargo test --release stress_50 -- --ignored --nocapture"]
fn stress_50() {
    let mut sim = crate::load(&["sandbox".into()], &[], saved_game().as_deref(), Default::default(), None);
    sim.button("stress");
    let t = std::time::Instant::now();
    let mut worst = 0.0f32;
    for _ in 0..600 {
        let s = std::time::Instant::now();
        sim.tick();
        worst = worst.max(s.elapsed().as_secs_f32() * 1000.0);
    }
    let mut h = 0u64;
    for c in &sim.crowd {
        for f in c.0.doll.frames() {
            for x in f.translation.to_array() {
                h = h.wrapping_mul(1099511628211).wrapping_add(x.to_bits() as u64);
            }
        }
    }
    eprintln!("stress_50: hash {h:x}, {} Homers, {:.1} ms a step, worst {worst:.1}", sim.crowd.len() + 1, t.elapsed().as_secs_f32() * 1000.0 / 600.0);
}
