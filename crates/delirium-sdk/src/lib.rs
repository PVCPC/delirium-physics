#![allow(clippy::missing_safety_doc)]

pub mod character;
pub mod rig;

use delirium_data::taskparams::TaskParams;
use delirium::collision::Collision;
use delirium::msg::{self, Kind, Value};
use delirium::nm::{self, Agent};
use delirium::prelude::*;
use delirium::ragdoll::{Build, Obstacle, Ragdoll, Sizes};
use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::sync::RwLock;

pub const NM_E_ARG: i32 = -1;
pub const NM_E_IO: i32 = -2;
pub const NM_E_PARSE: i32 = -3;
pub const NM_E_MESSAGE: i32 = -4;
pub const NM_E_PARAM: i32 = -5;
pub const NM_E_TYPE: i32 = -6;
pub const NM_E_STATE: i32 = -7;
pub const NM_E_FOLLOW: i32 = -8;

pub const NM_XR_WINDMILL: i32 = 1;
pub const NM_XR_CURL_UP: i32 = 2;
pub const NM_XR_CURL_SHOT: i32 = 4;
pub const NM_XR_LOOK_HIT: i32 = 8;
pub const NM_XR_LEDGE: i32 = 16;
pub const NM_XR_SMART_FALL: i32 = 32;
pub const NM_XR_DODGE: i32 = 64;
pub const NM_XR_WALL: i32 = 128;
pub const NM_XR_ARMS_OUT: i32 = 256;

pub const NM_MODE_LEAD: i32 = 0;
pub const NM_MODE_FOLLOW: i32 = 1;

thread_local! {
    static ERR: RefCell<String> = const { RefCell::new(String::new()) };
}
static TP: RwLock<Option<TaskParams>> = RwLock::new(None);
static HEADSHOT_KILLS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static FIRE_REACTION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

static RAND: std::sync::Mutex<delirium_data::rng::Rand> = std::sync::Mutex::new(delirium_data::rng::Rand { seed: 1 });
const SHOT_FORCE: f32 = 400.0;

fn fail(code: i32, why: impl Into<String>) -> i32 {
    ERR.with(|e| *e.borrow_mut() = why.into());
    code
}

unsafe fn text<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() { None } else { unsafe { CStr::from_ptr(p) }.to_str().ok() }
}

unsafe fn put(s: &str, buf: *mut c_char, len: i32) -> i32 {
    if !buf.is_null() && len > 0 {
        let n = s.len().min(len as usize - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(s.as_ptr(), buf as *mut u8, n);
            *buf.add(n) = 0;
        }
    }
    s.len() as i32 + 1
}

unsafe fn v3(p: *const f32) -> Vec3 {
    unsafe { Vec3::new(*p, *p.add(1), *p.add(2)) }
}

unsafe fn put3(p: *mut f32, v: Vec3) {
    if !p.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(v.to_array().as_ptr(), p, 3) }
    }
}

unsafe fn put4(p: *mut f32, q: Quat) {
    if !p.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(q.to_array().as_ptr(), p, 4) }
    }
}

fn with_tp<R>(f: impl FnOnce(&TaskParams) -> R) -> R {
    let g = TP.read().unwrap_or_else(|e| e.into_inner());
    match g.as_ref() {
        Some(tp) => f(tp),
        None => f(&TaskParams::default()),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_version() -> i32 {
    100
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_crash_report(dir: *const c_char) -> i32 {
    static DIR: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);
    let d = unsafe { text(dir) }.map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    if DIR.lock().unwrap_or_else(|e| e.into_inner()).replace(d).is_some() {
        return 0;
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.location().is_some_and(|l| ["delirium", "delirium-data"].iter().any(|c| l.file().contains(c))) {
            let p = DIR.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default().join("delirium_physics-crash.txt");
            let text = format!("Delirium Physics SDK {} crashed: {info}\n\nSystem: {} {}\n\n{}", env!("CARGO_PKG_VERSION"),
                               std::env::consts::OS, std::env::consts::ARCH, std::backtrace::Backtrace::force_capture());
            if std::fs::write(&p, text).is_ok() {
                eprintln!("\nDelirium Physics crashed. Please send this file with your report: {}", p.display());
            }
        }
        default(info);
    }));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_last_error(buf: *mut c_char, len: i32) -> i32 {
    ERR.with(|e| unsafe { put(&e.borrow(), buf, len) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_init_memory(xrt: *const u8, len: i32) -> i32 {
    if xrt.is_null() || len <= 0 {
        return fail(NM_E_ARG, "nm_init_memory: no data");
    }
    let d = unsafe { std::slice::from_raw_parts(xrt, len as usize) };
    let Ok(asset) = std::panic::catch_unwind(|| delirium_data::nm_asset::in_order(d, || delirium_data::nm_asset::read(d))) else {
        return fail(NM_E_PARSE, "nm_init: not an NM character file");
    };
    if !delirium_data::kv::open() {
        return fail(NM_E_STATE, "nm_init: call nm_unlock with your own GTAIV.exe first (the PC game's)");
    }
    let fresh = delirium::nmrs::ASSET[delirium::nmrs::HOMER].get().is_none();
    drop(asset);
    delirium::nmrs::load(delirium::nmrs::HOMER, d);
    if fresh { 0 } else { 1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_init(xrt_path: *const c_char) -> i32 {
    let Some(p) = (unsafe { text(xrt_path) }) else { return fail(NM_E_ARG, "nm_init: no path") };
    match std::fs::read(p) {
        Ok(d) => unsafe { nm_init_memory(d.as_ptr(), d.len() as i32) },
        Err(e) => fail(NM_E_IO, format!("nm_init: {p}: {e}")),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_unlock(exe_path: *const c_char) -> i32 {
    let Some(p) = (unsafe { text(exe_path) }) else { return fail(NM_E_ARG, "nm_unlock: no path") };
    let path = std::path::Path::new(p);
    let file = if !path.is_dir() { path.into() } else {
        ["GTAIV.exe", "default.xex"].iter().map(|f| path.join(f)).find(|f| f.is_file()).unwrap_or_else(|| path.join("GTAIV.exe"))
    };
    let d = match delirium_data::xex::program(&file) {
        Ok(d) => d,
        Err(e) => return fail(NM_E_IO, format!("nm_unlock: {e}")),
    };
    if delirium_data::kv::unlock(&d) { 0 } else { fail(NM_E_STATE, format!("nm_unlock: {p} isn't a version the numbers are read from (the Steam GTAIV.exe or the disc's default.xex)")) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_load_task_params(path: *const c_char) -> i32 {
    let Some(p) = (unsafe { text(path) }) else { return fail(NM_E_ARG, "nm_load_task_params: no path") };
    if !delirium_data::kv::open() {
        return fail(NM_E_STATE, "nm_load_task_params: call nm_unlock with your own GTAIV.exe first (the PC game's)");
    }
    match std::fs::read_to_string(p) {
        Ok(t) => {
            let tp = delirium_data::taskparams::read(&t);
            *TP.write().unwrap_or_else(|e| e.into_inner()) = Some(tp);
            0
        }
        Err(e) => fail(NM_E_IO, format!("{p}: {e}")),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_part_name(i: i32, buf: *mut c_char, len: i32) -> i32 {
    match delirium_data::nm_asset::PARTS.get(i as usize) {
        Some(n) if i >= 0 => unsafe { put(n, buf, len) },
        _ => fail(NM_E_ARG, "nm_part_name: no such part"),
    }
}

pub struct NmWorld(Collision, Vec<Obstacle>, Units, std::sync::Mutex<Bodies>, Vec<i32>);

#[derive(Default)]
struct Bodies(u64, Vec<(usize, u64, Vec<delirium::ragdoll::Capsule>, bool)>);

#[derive(Clone, Copy, Debug)]
struct Units(f32);

impl Units {
    fn len(self) -> f32 {
        self.0
    }
    fn time(self) -> f32 {
        self.0.sqrt()
    }
    fn impulse(self) -> f32 {
        self.0.powf(3.5)
    }
    fn mass(self) -> f32 {
        self.0.powi(3)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_world_create() -> *mut NmWorld {
    let w = Box::into_raw(Box::new(NmWorld(Collision::default(), vec![], Units(1.0), Default::default(), vec![])));
    WORLDS.lock().unwrap_or_else(|e| e.into_inner()).push(w as usize);
    w
}

static WORLDS: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_set_scale(w: *mut NmWorld, scale: f32) -> i32 {
    let Some(w) = (unsafe { w.as_mut() }) else { return fail(NM_E_ARG, "null world") };
    if !(scale > 0.0 && scale.is_finite()) {
        return fail(NM_E_ARG, "nm_world_set_scale: scale over 0");
    }
    w.2 = Units(scale);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_destroy(w: *mut NmWorld) {
    if !w.is_null() {
        WORLDS.lock().unwrap_or_else(|e| e.into_inner()).retain(|&x| x != w as usize);
        drop(unsafe { Box::from_raw(w) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_add_mesh(w: *mut NmWorld, positions: *const f32, vertices: i32, triangles: *const u32, count: i32,
                                           materials: *const u8) -> i32 {
    let Some(w) = (unsafe { w.as_mut() }) else { return fail(NM_E_ARG, "null world") };
    if positions.is_null() || triangles.is_null() || vertices < 0 || count < 0 {
        return fail(NM_E_ARG, "nm_world_add_mesh: bad arrays");
    }
    let p = unsafe { std::slice::from_raw_parts(positions as *const [f32; 3], vertices as usize) };
    let t = unsafe { std::slice::from_raw_parts(triangles as *const [u32; 3], count as usize) };
    if t.iter().flatten().any(|&i| i as usize >= p.len()) {
        return fail(NM_E_ARG, "nm_world_add_mesh: index past the vertices");
    }
    let m = if materials.is_null() { vec![0; t.len()] } else { unsafe { std::slice::from_raw_parts(materials, t.len()) }.to_vec() };
    let s = w.2.len();
    let mut p: Vec<[f32; 3]> = p.iter().map(|v| v.map(|x| x / s)).collect();
    let (t, m) = split_big(&mut p, t, &m);
    w.0.add(&p, &t, &m);
    0
}

const MAX_SIDE: f32 = 32.0;

fn split_big(p: &mut Vec<[f32; 3]>, t: &[[u32; 3]], m: &[u8]) -> (Vec<[u32; 3]>, Vec<u8>) {
    let (mut tt, mut mm) = (Vec::with_capacity(t.len()), Vec::with_capacity(t.len()));
    for (tri, &mat) in t.iter().zip(m) {
        let c = tri.map(|i| Vec3::from(p[i as usize]));
        let side = (c[1] - c[0]).length().max((c[2] - c[1]).length()).max((c[0] - c[2]).length());
        if !(side > MAX_SIDE) || side > 20000.0 {
            tt.push(*tri);
            mm.push(mat);
            continue;
        }
        let n = (side / MAX_SIDE).ceil() as u32;
        let mut idx = vec![vec![0u32; n as usize + 1]; n as usize + 1];
        for i in 0..=n {
            for j in 0..=n - i {
                idx[i as usize][j as usize] = p.len() as u32;
                let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
                let x = match (i, j) {
                    (0, _) => c[0].lerp(c[2], v),
                    (_, 0) => c[0].lerp(c[1], u),
                    _ if i + j == n => c[1].lerp(c[2], v),
                    _ => c[0] + (c[1] - c[0]) * u + (c[2] - c[0]) * v,
                };
                p.push(x.into());
            }
        }
        for i in 0..n as usize {
            for j in 0..n as usize - i {
                tt.push([idx[i][j], idx[i + 1][j], idx[i][j + 1]]);
                mm.push(mat);
                if i + j + 1 < n as usize {
                    tt.push([idx[i + 1][j], idx[i + 1][j + 1], idx[i][j + 1]]);
                    mm.push(mat);
                }
            }
        }
    }
    (tt, mm)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_add_floor(w: *mut NmWorld, x: f32, y: f32, z: f32, half: f32) -> i32 {
    let Some(w) = (unsafe { w.as_mut() }) else { return fail(NM_E_ARG, "null world") };
    let s = w.2.len();
    if !(half / s <= 100.001) {
        return fail(NM_E_ARG, "floor half over 100 m (at the world's scale)");
    }
    w.0.add_floor(x / s, y / s, z / s, (half / s).min(100.0), 0);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_set_boxes(w: *mut NmWorld, boxes: *const f32, count: i32) -> i32 {
    let Some(w) = (unsafe { w.as_mut() }) else { return fail(NM_E_ARG, "null world") };
    if count < 0 || (count > 0 && boxes.is_null()) {
        return fail(NM_E_ARG, "nm_world_set_boxes: bad array");
    }
    let f = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(boxes, count as usize * 13) } };
    let u = w.2;
    w.1 = f.chunks(13).enumerate().map(|(k, b)| {
        let c = Vec3::new(b[0], b[1], b[2]) / u.len();
        let (rot, half, vel) = (Quat::from_xyzw(b[3], b[4], b[5], b[6]).normalize(), Vec3::new(b[7], b[8], b[9]) / u.len(), Vec3::new(b[10], b[11], b[12]) / u.time());
        Obstacle { frame_end: false, id: k as i32, at: c, rot, centre: c, half, vel, spin: Vec3::ZERO, com: c, parts: &[], last: None, entity: 0, first: false, inv_mass: 0.0, inv_inertia: None, force: Vec3::ZERO, turn: None }
    }).collect();
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_set_box_kinds(w: *mut NmWorld, kinds: *const i32, count: i32) -> i32 {
    let Some(w) = (unsafe { w.as_mut() }) else { return fail(NM_E_ARG, "null world") };
    if count < 0 || count as usize != w.1.len() || (count > 0 && kinds.is_null()) {
        return fail(NM_E_ARG, "nm_world_set_box_kinds: count is not the boxes' number");
    }
    let k = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(kinds, count as usize) } };
    if k.iter().any(|&k| !(0..=3).contains(&(k & !NM_HIT_PLAYER)) || k & !NM_HIT_PLAYER == 2) {
        return fail(NM_E_ARG, "nm_world_set_box_kinds: a kind isn't NM_HIT_CAR, _OBJECT or _TRAIN (+ NM_HIT_PLAYER)");
    }
    w.4 = k.to_vec();
    0
}

pub const NM_HIT_CAR: i32 = 0;
pub const NM_HIT_OBJECT: i32 = 1;
pub const NM_HIT_TRAIN: i32 = 3;
pub const NM_HIT_PLAYER: i32 = 16;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_set_box_spins(w: *mut NmWorld, spins: *const f32, count: i32) -> i32 {
    let Some(w) = (unsafe { w.as_mut() }) else { return fail(NM_E_ARG, "null world") };
    if count < 0 || count as usize != w.1.len() || (count > 0 && spins.is_null()) {
        return fail(NM_E_ARG, "nm_world_set_box_spins: count is not the boxes' number");
    }
    let f = if count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(spins, count as usize * 3) } };
    for (o, s) in w.1.iter_mut().zip(f.chunks(3)) {
        o.spin = Vec3::new(s[0], s[1], s[2]) * w.2.time();
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_set_surface(material: i32, friction: f32, elasticity: f32) -> i32 {
    if !(0..256).contains(&material) || !(friction >= 0.0) || !(0.0..=1.0).contains(&elasticity) {
        return fail(NM_E_ARG, "nm_set_surface: material 0-255, friction >= 0, elasticity 0-1");
    }
    let mut s = delirium::ragdoll::SURFACES.write().unwrap();
    if s.len() <= material as usize {
        s.resize(material as usize + 1, [1.0, 0.1]);
    }
    s[material as usize] = [friction, elasticity];
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_triangle_count(w: *const NmWorld) -> i32 {
    unsafe { w.as_ref() }.map_or_else(|| fail(NM_E_ARG, "null world"), |w| w.0.len() as i32)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_raycast(w: *const NmWorld, from: *const f32, dir: *const f32, len: f32, dist: *mut f32, normal: *mut f32) -> i32 {
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if from.is_null() || dir.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    let s = w.2.len();
    match w.0.cast(unsafe { v3(from) } / s, unsafe { v3(dir) }.normalize_or_zero(), len / s) {
        Some((t, n)) => {
            if !dist.is_null() {
                unsafe { *dist = t * s };
            }
            unsafe { put3(normal, n) };
            1
        }
        None => 0,
    }
}

pub struct NmBuild {
    build: Build,
    frames: Vec<Transform>,
    scale: f32,
    tune: Vec<(&'static str, f32)>,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_from_text(t: *const c_char) -> *mut NmBuild {
    let Some(t) = (unsafe { text(t) }) else {
        fail(NM_E_ARG, "nm_build_from_text: no text");
        return std::ptr::null_mut();
    };
    let t = t.to_string();
    let r = std::panic::catch_unwind(move || Build::undump(&t));
    match r {
        Ok((build, frames)) if !frames.is_empty() => Box::into_raw(Box::new(NmBuild { build, frames, scale: 1.0, tune: vec![] })),
        _ => {
            fail(NM_E_PARSE, "nm_build: not a build dump");
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_load(path: *const c_char) -> *mut NmBuild {
    let Some(p) = (unsafe { text(path) }) else {
        fail(NM_E_ARG, "nm_build_load: no path");
        return std::ptr::null_mut();
    };
    match std::fs::read_to_string(p) {
        Ok(t) => match std::ffi::CString::new(t) {
            Ok(c) => unsafe { nm_build_from_text(c.as_ptr()) },
            Err(_) => {
                fail(NM_E_PARSE, "nm_build_load: NUL in file");
                std::ptr::null_mut()
            }
        },
        Err(e) => {
            fail(NM_E_IO, format!("{p}: {e}"));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_destroy(b: *mut NmBuild) {
    if !b.is_null() {
        drop(unsafe { Box::from_raw(b) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_part_count(b: *const NmBuild) -> i32 {
    unsafe { b.as_ref() }.map_or_else(|| fail(NM_E_ARG, "null build"), |b| b.build.parts.len() as i32)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_part_info(b: *const NmBuild, i: i32, mass: *mut f32, radius: *mut f32, half: *mut f32, parent: *mut i32) -> i32 {
    let Some(p) = (unsafe { b.as_ref() }).and_then(|b| b.build.parts.get(i as usize)) else { return fail(NM_E_ARG, "no such part") };
    let m = delirium::nmrs::MASSES[unsafe { &*b }.build.nm].get().and_then(|m| m.get(i as usize)).copied().unwrap_or(p.mass) * p.size.powi(3) * p.heft;
    unsafe {
        for (o, v) in [(mass, m), (radius, p.radius), (half, p.half)] {
            if !o.is_null() {
                *o = v;
            }
        }
        if !parent.is_null() {
            *parent = p.parent.map_or(-1, |j| j as i32);
        }
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_fit(build: *const NmBuild, joints: *const f32) -> *mut NmBuild {
    let Some(b) = (unsafe { build.as_ref() }) else {
        fail(NM_E_ARG, "nm_build_fit: null build");
        return std::ptr::null_mut();
    };
    let n = b.build.parts.len();
    if joints.is_null() || n != 21 {
        fail(NM_E_ARG, "nm_build_fit: no joints, or not a 21-part build");
        return std::ptr::null_mut();
    }
    let j: Vec<Vec3> = unsafe { std::slice::from_raw_parts(joints, n * 3) }.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    let bad = j.iter().any(|v| !v.is_finite())
        || (1..n).any(|c| b.build.parts[c].parent.is_some_and(|p| j[c].distance(j[p]) < 1e-3));
    if bad {
        fail(NM_E_ARG, "nm_build_fit: joints not finite, or a joint on its parent's");
        return std::ptr::null_mut();
    }
    let (build, frames) = b.build.fit(&b.frames, &j);
    Box::into_raw(Box::new(NmBuild { build, frames, scale: 1.0, tune: vec![] }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_fit_neck(build: *const NmBuild, joints: *const f32, neck: f32) -> *mut NmBuild {
    if !(neck > 0.0 && neck.is_finite()) {
        fail(NM_E_ARG, "nm_build_fit_neck: neck over 0");
        return std::ptr::null_mut();
    }
    let b = unsafe { nm_build_fit(build, joints) };
    let (Some(out), Some(src)) = (unsafe { b.as_mut() }, unsafe { build.as_ref() }) else { return b };
    let j: Vec<Vec3> = unsafe { std::slice::from_raw_parts(joints, 63) }.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    (out.build, out.frames) = src.build.fit_neck(&src.frames, &j, neck);
    b
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_set_head(b: *mut NmBuild, raise: f32, grow: f32) -> i32 {
    let Some(b) = (unsafe { b.as_mut() }) else { return fail(NM_E_ARG, "null build") };
    if !(raise.is_finite() && grow > 0.0 && grow.is_finite()) {
        return fail(NM_E_ARG, "nm_build_set_head: raise finite, grow over 0");
    }
    let Some(k) = b.build.parts.iter().position(|p| p.name.ends_with("Head")) else { return fail(NM_E_ARG, "nm_build_set_head: no head") };
    b.build.shift_part(&mut b.frames, k, raise / b.scale, grow);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_sized(build: *const NmBuild, legs: f32, arms: f32, trunk: f32, width: f32, head: f32, neck: f32, kg: f32, height: f32) -> *mut NmBuild {
    let Some(b) = (unsafe { build.as_ref() }) else {
        fail(NM_E_ARG, "nm_build_sized: null build");
        return std::ptr::null_mut();
    };
    if b.build.parts.len() != 21 || ![legs, arms, trunk, width, head, neck, kg].iter().all(|v| *v > 0.0 && v.is_finite()) || !(height >= 0.0 && height.is_finite()) {
        fail(NM_E_ARG, "nm_build_sized: not a 21-part build, or a size or the weight not over 0");
        return std::ptr::null_mut();
    }
    let (build, frames, _, _) = b.build.sized(&b.frames, Sizes { legs, arms, trunk, width, head, neck, kg, height });
    Box::into_raw(Box::new(NmBuild { build, frames, scale: b.scale, tune: vec![] }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_named(build: *const NmBuild, name: *const c_char) -> *mut NmBuild {
    let (Some(b), Some(n)) = (unsafe { build.as_ref() }, unsafe { text(name) }) else {
        fail(NM_E_ARG, "nm_build_named: null build or name");
        return std::ptr::null_mut();
    };
    match (b.build.parts.len() == 21).then(|| b.build.named(&b.frames, n)).flatten() {
        Some((build, frames)) => Box::into_raw(Box::new(NmBuild { build, frames, scale: b.scale, tune: vec![] })),
        None => {
            fail(NM_E_ARG, format!("nm_build_named: no body {n:?}, or not a 21-part build"));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_random(build: *const NmBuild, seed: i32, variance: f32, symmetric: i32) -> *mut NmBuild {
    let Some(b) = (unsafe { build.as_ref() }) else {
        fail(NM_E_ARG, "nm_build_random: null build");
        return std::ptr::null_mut();
    };
    if b.build.parts.len() != 21 || !variance.is_finite() {
        fail(NM_E_ARG, "nm_build_random: not a 21-part build, or variance not a number");
        return std::ptr::null_mut();
    }
    let (len, girth) = b.build.roll(seed as u32 as u64, variance, symmetric != 0);
    let (build, frames) = b.build.rolled(&b.frames, &len, &girth);
    Box::into_raw(Box::new(NmBuild { build, frames, scale: b.scale, tune: vec![] }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_fit_sized(build: *const NmBuild, joints: *const f32, mode: i32) -> *mut NmBuild {
    let s = unsafe { rig::nm_size_scale(build, joints, mode) };
    if !(s > 0.0) {
        return std::ptr::null_mut();
    }
    let j: Vec<f32> = unsafe { std::slice::from_raw_parts(joints, 63) }.iter().map(|x| x / s).collect();
    let b = unsafe { nm_build_fit(build, j.as_ptr()) };
    if let Some(b) = unsafe { b.as_mut() } {
        b.scale = s;
    }
    b
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_get_weight(b: *const NmBuild) -> f32 {
    unsafe { b.as_ref() }.map_or(0.0, |b| Ragdoll::new(&b.build, &b.frames, Vec3::ZERO).mass() * b.scale.powi(3))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_set_weight(b: *mut NmBuild, kg: f32) -> i32 {
    let now = unsafe { nm_build_get_weight(b) };
    let Some(b) = (unsafe { b.as_mut() }) else { return fail(NM_E_ARG, "null build") };
    if !(kg > 0.0 && kg.is_finite()) {
        return fail(NM_E_ARG, "nm_build_set_weight: kg over 0");
    }
    let g = (kg / now).clamp(1.0 / 3.0, 3.0).sqrt();
    for p in &mut b.build.parts {
        (p.heft, p.radius) = (p.heft * g * g, p.radius * g.sqrt());
    }
    0
}

const SKIN: f32 = 1.49;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_weigh_skin(b: *mut NmBuild, joints: *const f32, verts: *const f32, parts: *const i32, nverts: i32) -> f32 {
    let Some(bb) = (unsafe { b.as_mut() }) else {
        fail(NM_E_ARG, "null build");
        return 0.0;
    };
    if joints.is_null() || verts.is_null() || nverts < 1 || bb.build.parts.len() != 21 {
        fail(NM_E_ARG, "nm_build_weigh_skin: no skin, or not a 21-part build");
        return 0.0;
    }
    let s = bb.scale;
    let j: Vec<Vec3> = unsafe { std::slice::from_raw_parts(joints, 63) }.chunks(3).map(|c| Vec3::from_slice(c) / s).collect();
    let v = unsafe { std::slice::from_raw_parts(verts, nverts as usize * 3) };
    let vp = (!parts.is_null()).then(|| unsafe { std::slice::from_raw_parts(parts, nverts as usize) });
    let bone: Vec<Option<usize>> = (0..21).map(|k| {
        let kids: Vec<usize> = (0..21).filter(|&c| rig::PART_PARENT[c] == k as i32).collect();
        kids.iter().find(|c| [7, 11].contains(*c)).or(kids.first()).copied()
    }).collect();
    let dist = |k: usize, x: Vec3| bone[k].map(|c| {
        let (a, d) = (j[k], j[c] - j[k]);
        let t = ((x - a).dot(d) / d.length_squared().max(1e-8)).clamp(0.0, 1.0);
        x.distance(a + d * t)
    });
    let mut near: Vec<Vec<f32>> = vec![vec![]; 21];
    for (i, p) in v.chunks(3).enumerate() {
        let x = Vec3::from_slice(p) / s;
        let k = match vp {
            Some(vp) => usize::try_from(vp[i]).ok().filter(|&k| k < 21),
            None => (0..21).filter_map(|k| dist(k, x).map(|d| (k, d))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(k, _)| k),
        };
        if let Some(d) = k.and_then(|k| dist(k, x).map(|d| (k, d))) {
            near[d.0].push(d.1);
        }
    }
    let mut ratio: Vec<f32> = near.iter_mut().enumerate().filter(|(_, n)| n.len() >= 8).map(|(k, n)| {
        n.sort_by(f32::total_cmp);
        n[n.len() / 2] / bb.build.parts[k].radius.max(1e-4) / SKIN
    }).collect();
    if ratio.is_empty() {
        fail(NM_E_ARG, "nm_build_weigh_skin: no part has 8 vertices");
        return 0.0;
    }
    ratio.sort_by(f32::total_cmp);
    let g = ratio[ratio.len() / 2];
    let kg = unsafe { nm_build_get_weight(b) } * g * g;
    unsafe { nm_build_set_weight(b, kg) };
    unsafe { nm_build_get_weight(b) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_mesh_weight(verts: *const f32, nverts: i32, tris: *const u32, ntris: i32) -> f32 {
    if verts.is_null() || tris.is_null() || nverts < 3 || ntris < 1 {
        fail(NM_E_ARG, "nm_mesh_weight: no mesh");
        return 0.0;
    }
    let (v, t) = unsafe { (std::slice::from_raw_parts(verts, nverts as usize * 3), std::slice::from_raw_parts(tris, ntris as usize * 3)) };
    let p = |i: u32| v.get(i as usize * 3..i as usize * 3 + 3).map_or(Vec3::ZERO, Vec3::from_slice);
    let vol: f64 = t.chunks(3).map(|c| p(c[0]).dot(p(c[1]).cross(p(c[2]))) as f64 / 6.0).sum();
    (vol.abs() * 985.0) as f32
}

const TUNE: [(&str, f32, f32); 6] = [("stepHeight", 0.05, 0.3), ("legStiffness", 6.0, 16.0), ("balanceAbortThreshold", 0.3, 1.5),
                                     ("stepClampScale", 0.3, 2.0), ("bodyStiffness", 6.0, 14.0), ("damping", 0.5, 2.0)];

fn send(a: &mut NmAgent, w: &NmWorld, m: &str, ps: &[(&str, Value)]) {
    let mut st = std::mem::take(&mut a.agent.msgs);
    let _ = msg::send(&mut a.agent, &a.build, &w.0, &mut st, m, ps);
    a.agent.msgs = st;
}

pub fn tune_msgs(tune: &[(&'static str, f32)]) -> [(&'static str, Vec<(&'static str, Value)>); 2] {
    let ps = |r: std::ops::Range<usize>| tune.iter().filter(|t| TUNE[r.clone()].iter().any(|u| u.0 == t.0)).map(|t| (t.0, Value::Float(t.1))).collect::<Vec<_>>();
    [("configureBalance", ps(0..4)), ("bodyBalance", ps(4..6))]
}

pub fn tune(build: Build, frames: Vec<Transform>, tries: i32) -> (Vec<(&'static str, f32)>, i32) {
    let mut b = NmBuild { build, frames, scale: 1.0, tune: vec![] };
    let got = unsafe { nm_build_tune(&mut b, tries) };
    (b.tune, got)
}

fn tuned(a: &mut NmAgent, tune: &[(&'static str, f32)]) {
    if tune.is_empty() {
        return;
    }
    let w = NmWorld(Collision::default(), vec![], a.u, Default::default(), vec![]);
    for (m, ps) in tune_msgs(tune) {
        send(a, &w, m, &ps);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_tune(b: *mut NmBuild, tries: i32) -> i32 {
    let Some(bb) = (unsafe { b.as_mut() }) else { return fail(NM_E_ARG, "null build") };
    let key = tune_key(bb, tries);
    if let Some((tune, got)) = tune_saved(&key) {
        bb.tune = tune;
        return got;
    }
    let s = bb.scale;
    let one = |bb: &NmBuild, push: f32| -> i32 {
        unsafe {
            let w = nm_world_create();
            nm_world_set_scale(w, s);
            nm_world_add_floor(w, 0.0, 0.0, 0.0, 100.0 * s);
            let a = nm_agent_create(bb, std::ptr::null(), 0.0, 0.3);
            if a.is_null() {
                nm_world_destroy(w);
                return 0;
            }
            send(&mut *a, &*w, "bodyBalance", &[("start", Value::Bool(true))]);
            let mut c = [0.0f32; 3];
            nm_agent_get_part_transform(a, 10, c.as_mut_ptr(), std::ptr::null_mut());
            let dv = push / 77.69 * nm_agent_get_mass(a) * s.sqrt();
            nm_agent_apply_push(a, c.as_ptr(), [0.0, dv, 0.0].as_ptr());
            for _ in 0..240 {
                nm_agent_step(a, w, 1.0 / 60.0 * s.sqrt());
            }
            let ok = (nm_agent_is_balancing(a) == 1 && nm_agent_has_fallen(a) == 0) as i32;
            nm_agent_destroy(a);
            nm_world_destroy(w);
            ok
        }
    };
    const PUSHES: [f32; 3] = [0.0, 20.0, 40.0];
    let score = |bb: &NmBuild, best: i32| -> i32 {
        if (0..3).contains(&best) && one(bb, PUSHES[best as usize]) == 0 {
            return 0;
        }
        PUSHES.iter().map(|&p| one(bb, p)).sum()
    };
    bb.tune.clear();
    let mut best = score(bb, -1);
    let (mut rng, mut keep) = (0x9E37_79B9u32, vec![]);
    for _ in 0..tries.max(0) {
        if best == 3 {
            break;
        }
        bb.tune = TUNE.iter().map(|&(n, lo, hi)| {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            (n, lo + (hi - lo) * (rng >> 8) as f32 / (1u32 << 24) as f32)
        }).collect();
        let k = score(bb, best);
        if k > best {
            (best, keep) = (k, bb.tune.clone());
        }
    }
    bb.tune = keep;
    tune_save(&key, &bb.tune, best);
    best
}

static TUNE_FILE: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_tune_file(path: *const c_char) -> i32 {
    let p = (!path.is_null()).then(|| unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned());
    *TUNE_FILE.lock().unwrap_or_else(|e| e.into_inner()) = p.map(Into::into);
    0
}

fn tune_key(bb: &NmBuild, tries: i32) -> Vec<f32> {
    bb.build.parts.iter().flat_map(|p| [p.size, p.radius, p.half, p.heft]).chain([bb.scale, tries as f32]).collect()
}

fn tune_saved(key: &[f32]) -> Option<(Vec<(&'static str, f32)>, i32)> {
    let f = TUNE_FILE.lock().unwrap_or_else(|e| e.into_inner());
    let text = std::fs::read_to_string(f.as_ref()?).ok()?;
    let mut w = text.lines().map(|l| l.split_whitespace()).find(|w| {
        let m: Vec<f32> = w.clone().next().unwrap_or("").split(',').filter_map(|x| x.parse().ok()).collect();
        m.len() == key.len() && m.iter().zip(key).all(|(a, b)| (a - b).abs() <= 0.01 * b.abs().max(0.01))
    })?.skip(1);
    let got = w.next()?.parse().ok()?;
    let tune = w.filter_map(|kv| {
        let (n, v) = kv.split_once('=')?;
        Some((TUNE.iter().find(|t| t.0 == n)?.0, v.parse().ok()?))
    }).collect();
    Some((tune, got))
}

fn tune_save(key: &[f32], tune: &[(&'static str, f32)], got: i32) {
    let f = TUNE_FILE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(p) = f.as_ref() else { return };
    let key = key.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    let line = format!("{key} {got}{}\n", tune.iter().map(|(n, v)| format!(" {n}={v}")).collect::<String>());
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(p).and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_copy_tune(dst: *mut NmBuild, src: *const NmBuild) -> i32 {
    let (Some(d), Some(s)) = (unsafe { dst.as_mut() }, unsafe { src.as_ref() }) else { return fail(NM_E_ARG, "null build") };
    d.tune = s.tune.clone();
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_get_scale(b: *const NmBuild) -> f32 {
    unsafe { b.as_ref() }.map_or(0.0, |b| b.scale)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_part_frame(b: *const NmBuild, i: i32, pos: *mut f32, rot: *mut f32) -> i32 {
    let Some(f) = (unsafe { b.as_ref() }).and_then(|b| b.frames.get(i as usize)) else { return fail(NM_E_ARG, "no such part") };
    unsafe {
        put3(pos, f.translation);
        put4(rot, f.rotation);
    }
    0
}

pub struct NmAgent {
    agent: Agent,
    build: Build,
    mode: i32,
    set: Vec<bool>,
    pending: Option<(String, Vec<(String, Value)>)>,
    shot: Option<(delirium::shottask::ShotTask, f32)>,
    blast: Option<(delirium::explosion::Explosion, f64)>,
    dead: bool,
    fire: Option<(nm::OnFire, bool, f32)>,
    singe: f32,
    shock: f32,
    fall: Option<nm::FallDown>,
    grab: Option<delirium::grabhelper::GrabHelper>,
    car: (Option<(i32, Vec3, Vec3, f32)>, f32),
    door: Option<(i32, f32, f32)>,
    bank: f32,
    world: usize,
    stairs: (Option<f32>, u8, bool),
    cuts: Vec<(Build, Ragdoll, Vec<delirium::ragdoll::Rigid>, Vec<usize>)>,
    tackle: Option<Tackle>,
    water: (Option<f32>, bool),
    clock: f64,
    frame: f64,
    still: f32,
    u: Units,
    more: More,
}

#[derive(Default)]
struct More {
    partial: Option<delirium::partial::Partial>,
    partial_event: i32,
    sit: Option<delirium::nmsit::Sit>,
    pose: Option<delirium::nmpose::Pose>,
    roll: Option<nm::RollUpAndRelax>,
    script: Option<(delirium::nmscript::Script, u32)>,
    attacker: Option<([Vec3; 4], Option<nm::PointGun>)>,
    stick: Option<((i32, i32), f32, delirium::playerctl::Wander)>,
    jroll: Option<(f64, Vec3)>,
    look: Option<Vec3>,
    zero: Vec<Quat>,
    tune: Vec<(&'static str, f32)>,
    learned: Option<(std::sync::Arc<delirium::learned::Net>, delirium::learned::Pick, usize)>,
    walk: Option<(std::sync::Arc<delirium::learned::Net>, f32, f32)>,
    musc: Option<f32>,
    jump: Option<delirium::jump::Jump>,
    xr: i32,
    xs: Xs,
    dive_gap: f32,
    leash: Vec<(usize, Vec3, Vec3, f32, f32)>,
}

#[derive(Default)]
struct Xs {
    was_fallen: bool,
    curl: Option<f64>,
    curled: bool,
    near_at: Option<f64>,
    dived: bool,
    airborne: bool,
    hold: Option<f64>,
    dodge_at: f64,
    fall: Option<(f64, bool)>,
    look_after: Option<f64>,
    dodging: bool,
    look_stop: Option<f64>,
    lean_stop: Option<f64>,
}

fn coin() -> bool {
    use std::hash::BuildHasher;
    std::collections::hash_map::RandomState::new().hash_one(0u8) & 1 == 0
}

const DODGE_LEAN: f32 = 0.7;
const BRACE_LEAN: f32 = 0.6;
const ARMS_OUT_TIMEOUT: f32 = 1.1;
const TUMBLE: f32 = 7.0;
const WHIZZ: f32 = 1.5;
const NEAR_BURST: f32 = 0.5;
const DIVE: (f32, f32) = (3.5, 3.0);
const CHEST: usize = 9;
const HEAD: usize = 12;

fn placed(b: &NmBuild, pos: Vec3, yaw: f32) -> Vec<Transform> {
    let floor = b.build.parts.iter().zip(&b.frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
    let r = b.frames[0].translation.with_z(floor);
    let turn = Quat::from_rotation_z(yaw);
    b.frames.iter().map(|f| Transform::from_translation(pos + turn * (f.translation - r)).with_rotation(turn * f.rotation)).collect()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_create(b: *const NmBuild, pos: *const f32, yaw: f32, seed: f32) -> *mut NmAgent {
    let Some(b) = (unsafe { b.as_ref() }) else {
        fail(NM_E_ARG, "null build");
        return std::ptr::null_mut();
    };
    if !delirium_data::kv::open() {
        fail(NM_E_STATE, "nm_agent_create: call nm_unlock with your own GTAIV.exe first (the PC game's)");
        return std::ptr::null_mut();
    }
    let pos = if pos.is_null() { Vec3::ZERO } else { unsafe { v3(pos) / b.scale } };
    let frames = placed(b, pos, yaw);
    let agent = Agent::new(&b.build, Ragdoll::new(&b.build, &frames, Vec3::ZERO), Quat::from_rotation_z(yaw), seed);
    let n = frames.len();
    let mut a = Box::new(NmAgent { agent, build: b.build.clone(), mode: NM_MODE_LEAD, set: vec![false; n], pending: None, shot: None, blast: None, dead: false, fire: None, fall: None, grab: None, car: (None, f32::MAX), door: None, bank: 0.0, world: 0, stairs: (None, 0, false), cuts: vec![], tackle: None, water: (None, false), singe: 0.0, shock: 0.0, more: More { zero: delirium::learned::zero(&b.build, &b.frames), tune: b.tune.clone(), ..Default::default() }, clock: 0.0,
                                     frame: 0.0, still: 0.0, u: Units(b.scale) });
    tuned(&mut a, &b.tune);
    Box::into_raw(a)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_destroy(a: *mut NmAgent) {
    if !a.is_null() {
        let worlds = WORLDS.lock().unwrap_or_else(|e| e.into_inner());
        let w = unsafe { (*a).world };
        if worlds.contains(&w) {
            unsafe { &*(w as *const NmWorld) }.3.lock().unwrap_or_else(|e| e.into_inner()).1.retain(|e| e.0 != a as usize);
        }
        drop(worlds);
        drop(unsafe { Box::from_raw(a) });
    }
}

macro_rules! agent {
    ($a:expr) => {
        match unsafe { $a.as_mut() } {
            Some(a) => {
                a.still = 0.0;
                a
            }
            None => return fail(NM_E_ARG, "null agent"),
        }
    };
}
macro_rules! agent_ref {
    ($a:expr) => {
        match unsafe { $a.as_ref() } {
            Some(a) => a,
            None => return fail(NM_E_ARG, "null agent"),
        }
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_step(a: *mut NmAgent, w: *const NmWorld, dt: f32) -> i32 {
    let Some(a) = (unsafe { a.as_mut() }) else { return fail(NM_E_ARG, "null agent") };
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    agent_step(a, w, dt, None)
}

type Stepped = Vec<(usize, u64, Vec<delirium::ragdoll::Capsule>, bool)>;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_world_step_agents(w: *const NmWorld, agents: *const *mut NmAgent, count: i32, dt: f32, threads: i32) -> i32 {
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if count <= 0 {
        return 0;
    }
    if agents.is_null() {
        return fail(NM_E_ARG, "null agents");
    }
    let ptrs = unsafe { std::slice::from_raw_parts(agents, count as usize) };
    let mut seen: Vec<usize> = ptrs.iter().map(|&p| p as usize).collect();
    seen.sort_unstable();
    if seen.contains(&0) || seen.windows(2).any(|p| p[0] == p[1]) {
        return fail(NM_E_ARG, "null or repeated agent");
    }
    let mut list: Vec<&mut NmAgent> = ptrs.iter().map(|&p| unsafe { &mut *p }).collect();
    for a in list.iter_mut().filter(|a| a.mode != NM_MODE_FOLLOW) {
        let dt = a.bank + dt / a.u.time();
        if dt >= 1.0 / 60.0 - 1e-5 {
            tackle(a, dt);
        }
    }
    let before: Stepped = w.3.lock().unwrap_or_else(|e| e.into_inner()).1.clone();
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let n = (if threads > 0 { threads as usize } else { cores }).clamp(1, list.len());
    let per = list.len().div_ceil(n);
    let step = |chunk: &mut [&mut NmAgent]| chunk.iter_mut().map(|a| agent_step(a, w, dt, Some(&before))).find(|&r| r < 0)
        .map(|r| (r, ERR.with(|e| e.borrow().clone())));
    let errs: Vec<(i32, String)> = if n == 1 {
        step(&mut list).into_iter().collect()
    } else {
        use rayon::prelude::*;
        list.par_chunks_mut(per).with_max_len(1).filter_map(step).collect()
    };
    match errs.into_iter().next() {
        Some((r, why)) => fail(r, why),
        None => 0,
    }
}

fn agent_step(a: &mut NmAgent, w: &NmWorld, dt: f32, before: Option<&Stepped>) -> i32 {
    let dt = dt / a.u.time();
    if a.mode == NM_MODE_FOLLOW {
        return step_in(a, w, dt, &[]);
    }
    a.bank += dt;
    if a.bank < 1.0 / 60.0 - 1e-5 {
        a.agent.feedback.clear();
        return 0;
    }
    let dt = std::mem::take(&mut a.bank);
    let me = a as *const NmAgent as usize;
    a.world = w as *const NmWorld as usize;
    let others: Vec<delirium::ragdoll::Capsule> = {
        let mut b = w.3.lock().unwrap_or_else(|e| e.into_inner());
        b.0 += 1;
        let (now, keep) = (b.0, 2 * b.1.len() as u64 + 2);
        b.1.retain(|e| e.0 == me || now - e.1 <= keep);
        let at = a.agent.doll.com().0;
        let mut near: Vec<delirium::ragdoll::Capsule> = before.unwrap_or(&b.1).iter().filter(|e| e.0 != me).flat_map(|e| e.2.iter().copied())
            .filter(|c| ((c.0 + c.1) / 2.0).distance(at) < 3.0).collect();
        near.sort_by(|x, y| x.0.to_array().iter().chain(&x.1.to_array()).map(|v| v.to_bits()).cmp(y.0.to_array().iter().chain(&y.1.to_array()).map(|v| v.to_bits())));
        near
    };
    catch_fire(a, w, dt);
    if before.is_none() {
        tackle(a, dt);
    }
    partial_step(a, w, dt);
    let r = step_in(a, w, dt, &others);
    car_door(a, w, dt);
    stairs(a, w);
    water(a, w);
    let n = (dt * 60.0).round().clamp(1.0, 8.0) as usize;
    for (b, d, rigid, parts) in &mut a.cuts {
        a.agent.doll.ghost(parts);
        for _ in 0..n {
            d.step(b, &w.0, dt / n as f32, delirium::ragdoll::Drive { rigid, ..Default::default() }, &w.1, &[]);
        }
    }
    a.car.1 += dt;
    if let Some(h) = &mut a.car.0 {
        h.3 += dt;
    }
    for &(id, j, at) in &a.agent.doll.ob_hits {
        if id >= 0 && a.car.1 >= 1.0 {
            a.car.0.get_or_insert((id, Vec3::ZERO, at, 0.0)).1 += j;
        }
    }
    let caps = a.agent.doll.capsules().zip(&a.build.parts).enumerate().map(|(k, (t, p))| {
        let d = t.rotation * Vec3::Y * p.half;
        (t.translation - d, t.translation + d, p.radius, a.agent.doll.motion(k).0, true)
    }).collect();
    let burning = a.fire.as_ref().is_some_and(|f| f.1);
    let mut b = w.3.lock().unwrap_or_else(|e| e.into_inner());
    let now = b.0;
    match b.1.iter_mut().find(|e| e.0 == me) {
        Some(e) => (e.1, e.2, e.3) = (now, caps, burning),
        None => b.1.push((me, now, caps, burning)),
    }
    r
}

fn more_step(a: &mut NmAgent, w: &NmWorld, dt: f32) {
    let now = a.clock as u32;
    if let Some(s) = a.more.sit.as_mut() {
        nm::task_sit(&mut a.agent, &a.build, &w.0, s, now, None, None);
    }
    if let Some(p) = a.more.pose.as_mut() {
        nm::task_pose_start(&mut a.agent, &a.build, &w.0, p, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), now);
        if !a.agent.incoming.is_empty() {
            a.agent.active_pose(None);
        }
    }
    if let Some(r) = a.more.roll.as_mut() {
        r.control(&mut a.agent, &a.build, &w.0, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), dt);
    }
    if let Some((s, flags)) = a.more.script.as_mut() {
        if s.update(now, flags) {
            a.more.script = None;
        }
    }
    if let Some((bones, gun)) = a.more.attacker.as_mut() {
        let task = if a.shot.is_some() { nm::Task::Shot } else { nm::Task::Balance };
        let b = *bones;
        nm::task_control(&mut a.agent, task, |id| match id { 417 => Some(b[0]), 1203 => Some(b[1]), 13985 => Some(b[2]), 1204 | 1205 => Some(b[3]), _ => None });
        if let Some(g) = gun.as_mut() {
            let at = match g.bone() { 417 => b[0], 1203 => b[1], 13985 => b[2], _ => b[3] };
            g.control(&mut a.agent, dt, at);
        }
    }
    if a.frame == a.clock {
        if let Some((stick, camera, wander)) = a.more.stick.as_mut() {
            let ped = a.agent.doll.frames().next().unwrap_or_default().translation;
            let m = delirium::playerctl::control(&mut a.agent, *stick, *camera, ped, wander, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()));
            nm::send_task_msgs(&mut a.agent, &a.build, &w.0, &m);
        }
    }
    if let Some(mut j) = a.more.jump.take() {
        let mut said = delirium::jump::Said::default();
        if j.frame(&mut a.agent, dt, &mut said) {
            a.more.jump = Some(j);
        }
        if said.catch_fall {
            send(a, w, "catchFall", &[("start", Value::Bool(true))]);
        }
    }
    if let Some((net, phase, len)) = a.more.walk.as_mut() {
        delirium::learned::drive_walk(net, &net.w, &mut a.agent, phase, *len);
    }
    if let (Some(help), false) = (a.more.musc, a.agent.incoming.is_empty()) {
        let (at, v) = (a.agent.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation), a.agent.doll.com().1);
        let m = a.agent.doll.mass();
        let f = (m * (40.0 * (a.agent.incoming[0].translation - at) - 8.0 * v)).clamp_length_max(help * m * 9.81);
        a.agent.push.push((0, at, f));
        a.agent.active_pose(None);
    }
    if let Some(at) = a.more.look.take() {
        look_hit(a, w, at);
    }
    reactions(a, w);
    if let Some((_, vel)) = a.more.jroll.filter(|j| a.clock >= j.0) {
        with_tp(|tp| nm::task_jump_roll_up(&mut a.agent, &a.build, &w.0, tp, vel));
        a.more.jroll = None;
    }
}

fn reactions(a: &mut NmAgent, w: &NmWorld) {
    let on = |a: &NmAgent, b: i32| a.more.xr & b != 0;
    let now = a.clock;
    let b = |v: bool| Value::Bool(v);
    let fallen = a.agent.fallen;
    if on(a, NM_XR_WINDMILL) && fallen && !a.more.xs.was_fallen && coin() {
        send(a, w, "armsWindmillAdaptive", &[("start", b(true)), ("disableOnImpact", b(true))]);
    }
    a.more.xs.was_fallen = fallen;
    if a.more.xs.curl.is_some_and(|t| now >= t) {
        a.more.xs.curl = None;
        a.still = 0.0;
        send(a, w, "bodyFoetal", &[("start", b(false))]);
    }
    if a.agent.balancing() {
        a.more.xs.curled = false;
    } else if on(a, NM_XR_CURL_UP) && !a.dead && !a.more.xs.curled && a.still >= 1.0 {
        a.more.xs.curled = true;
        if coin() {
            a.more.xs.curl = Some(now + 3000.0);
            send(a, w, "bodyFoetal", &[("start", b(true))]);
        }
    }
    if a.more.xs.lean_stop.is_some_and(|t| now >= t) {
        a.more.xs.lean_stop = None;
        send(a, w, "leanInDirection", &[("start", b(false))]);
    }
    if a.more.xs.look_stop.is_some_and(|t| now >= t) {
        a.more.xs.look_stop = None;
        send(a, w, "headLook", &[("start", b(false))]);
        if std::mem::take(&mut a.more.xs.dodging) && a.agent.balancing() {
            send(a, w, "stopAllBehaviours", &[]);
            send(a, w, "bodyBalance", &[("start", b(true))]);
        }
    }
    if a.more.xs.hold.is_some_and(|t| now >= t) {
        a.more.xs.hold = None;
        send(a, w, "grab", &[("start", b(false))]);
    }
    let xs = &a.more.xs;
    if a.dead || !(on(a, NM_XR_LEDGE) || on(a, NM_XR_DODGE) || on(a, NM_XR_WALL)) && xs.fall.is_none() && xs.look_after.is_none() && !xs.airborne {
        return;
    }
    let arms_out = |a: &mut NmAgent| send(a, w, "bodyBalance", &[("start", b(true)), ("armsOutOnPush", b(true)), ("armsOutOnPushMultiplier", Value::Float(1.0)), ("armsOutOnPushTimeout", Value::Float(ARMS_OUT_TIMEOUT))]);
    let lean = |a: &mut NmAgent, amount: f32, dir: Vec3| send(a, w, "leanInDirection", &[("start", b(true)), ("applyAsForce", b(true)), ("leanAmount", Value::Float(amount)), ("dir", Value::Vec3(dir))]);
    let (me, mv) = a.agent.doll.com();
    let f: Vec<Transform> = a.agent.doll.frames().collect();
    let fwd = Vec3::Z.cross(f[4].translation - f[1].translation).with_z(0.0).normalize_or(Vec3::X);
    let mine = a as *const NmAgent as usize;
    let things: Vec<(Vec3, Vec3, bool)> = w.1.iter().map(|o| (o.centre, o.vel, true))
        .chain(w.3.lock().unwrap_or_else(|e| e.into_inner()).1.iter().filter(|e| e.0 != mine).filter_map(|e| e.2.first().map(|c| ((c.0 + c.1) * 0.5, c.3, false))))
        .collect();
    let overhead = || things.iter().filter_map(|&(p, v, _)| {
        let (r, down) = (p - me, -v.z);
        let h = r.z - 0.8;
        (h > 0.0 && r.with_z(0.0).length() < 2.5 && down > 1.0).then(|| (p, v, (-down + (down * down + 2.0 * 9.81 * h).sqrt()) / 9.81))
    }).filter(|x| x.2 < 3.0).min_by(|a, b| a.2.total_cmp(&b.2));
    if a.more.xs.look_after.is_some_and(|t| now >= t) {
        a.more.xs.look_after = None;
        if std::mem::take(&mut a.more.xs.dodging) {
            send(a, w, "stopAllBehaviours", &[]);
            arms_out(a);
        }
        let down = things.iter().map(|x| x.0).filter(|p| (*p - me).length() < 4.0).min_by(|x, y| (*x - me).length().total_cmp(&(*y - me).length()));
        if let Some(at) = down {
            look_hit(a, w, at);
            a.more.xs.look_stop = Some(now + 2500.0);
        }
    }
    let land = w.0.cast(me, -Vec3::Z, 50.0).map_or(f32::MAX, |(t, _)| (t - 0.95) / (-mv.z).max(0.1));
    if a.more.xs.airborne {
        if land < 0.12 || mv.z > 0.0 {
            a.more.xs.airborne = false;
            send(a, w, "bodyBalance", &[("start", b(true))]);
        }
    } else if on(a, NM_XR_LEDGE) && mv.z < -0.8 && land > 0.4 && a.agent.balancing() {
        if mv.truncate().length() > 0.8 && land > 0.8 {
            send(a, w, "stopAllBehaviours", &[]);
            send(a, w, "smartFall", &[("start", b(true)), ("tumble", Value::Float(TUMBLE))]);
        } else {
            a.more.xs.airborne = true;
            send(a, w, "highFall", &[("start", b(true))]);
        }
    }
    let (go, chest) = (mv.with_z(0.0), f[CHEST].translation);
    if on(a, NM_XR_WALL) && a.more.xs.hold.is_none() && a.more.xs.fall.is_none() && go.length() > 0.4 && chest.z - me.z > 0.2 {
        let d = go.normalize();
        let reach = 0.35 + (0.3 * go.length()).min(0.7);
        let hit = [chest.z, me.z, me.z - 0.3].into_iter().find_map(|z| {
            let from = me.with_z(z);
            w.0.cast(from, d, reach).filter(|h| h.1.z.abs() < 0.5).map(|(t, n)| (from + d * t, n.with_z(0.0).normalize()))
        });
        if let Some((h, n)) = hit {
            let head = chest.z + 0.45;
            let right = (-n).cross(Vec3::Z);
            let top = (h - n * 0.03).with_z(head);
            let top = top.with_z(head - w.0.cast(top, -Vec3::Z, 1.5).map_or(0.0, |x| x.0));
            let v = Value::Vec3;
            let held = if w.0.cast(me.with_z(head), d, reach + 0.2).is_some() {
                let at = h.with_z(chest.z);
                send(a, w, "grab", &[("start", b(true)), ("justbrace", b(true)), ("useLeft", b(true)), ("useRight", b(true)), ("pos1", v(at + right * 0.25)), ("pos2", v(at - right * 0.25)), ("normalR", v(n)), ("normalL", v(n))]);
                true
            } else if top.z > me.z - 0.15 {
                send(a, w, "grab", &[("start", b(true)), ("linegrab", b(true)), ("useLeft", b(true)), ("useRight", b(true)), ("pos1", v(top + right * 0.3)), ("pos2", v(top - right * 0.3))]);
                true
            } else {
                false
            };
            if held {
                a.more.xs.hold = Some(now + 2000.0);
            }
        }
    }
    if let Some((land, braced)) = a.more.xs.fall {
        let seen = overhead();
        if seen.is_none() && now > land + 300.0 || now > land + 1500.0 {
            (a.more.xs.fall, a.more.xs.look_after) = (None, Some(now + 400.0));
            return;
        }
        let Some((at, _, t)) = seen else { return };
        const BRACE: f32 = 0.45;
        a.more.xs.fall = Some((now + t as f64 * 1000.0, braced || t < BRACE));
        if !braced && t < BRACE {
            let away = (me - at).with_z(0.0).normalize_or(-fwd);
            a.more.xs.dodging = true;
            a.more.xs.lean_stop = Some(now + 800.0);
            send(a, w, "pointArm", &[("start", b(false))]);
            arms_out(a);
            send(a, w, "upperBodyFlinch", &[("start", b(true)), ("pos", Value::Vec3(at)), ("newHit", b(true))]);
            lean(a, DODGE_LEAN, away);
        } else if !braced && (now / (1000.0 / 6.0)) as i64 != ((now - 1000.0 / 60.0) / (1000.0 / 6.0)) as i64 {
            send(a, w, "pointArm", &[("start", b(true)), ("target", Value::Vec3(at))]);
        }
        return;
    }
    if !on(a, NM_XR_DODGE) || now < a.more.xs.dodge_at || !a.agent.balancing() {
        return;
    }
    if let Some((at, v, t)) = overhead() {
        a.more.xs.dodge_at = now + 3000.0;
        if (at - me).with_z(0.0).dot(fwd) < -0.2 {
            a.more.xs.look_after = Some(now + (t as f64 + 0.7) * 1000.0);
            return arms_out(a);
        }
        a.more.xs.fall = Some((now + t as f64 * 1000.0, false));
        a.more.xs.look_stop = None;
        send(a, w, "headLook", &[("start", b(true)), ("pos", Value::Vec3(at)), ("vel", Value::Vec3(v)), ("alwaysLook", b(true))]);
        send(a, w, "pointArm", &[("start", b(true)), ("target", Value::Vec3(at))]);
        return;
    }
    const DODGE_DUE: f32 = 1.0;
    let due = |p: Vec3, v: Vec3| (me - p).length() / (me - p).normalize_or_zero().dot(v - mv).max(1e-3);
    let Some(&(at, v, thrown)) = things.iter().find(|&&(p, v, _)| (me - p).length() < 30.0 && (me - p).normalize_or_zero().dot(v - mv) > 3.0 && due(p, v) < DODGE_DUE)
    else { return };
    let away = (me - at).with_z(0.0).normalize_or(-fwd);
    a.more.xs.dodge_at = now + 3000.0;
    a.more.xs.look_stop = Some(now + 2000.0);
    a.more.xs.lean_stop = Some(now + DODGE_DUE as f64 * 1000.0);
    let look = [("start", b(true)), ("pos", Value::Vec3(at)), ("vel", Value::Vec3(v)), ("alwaysLook", b(true))];
    if thrown {
        a.more.xs.dodging = true;
        send(a, w, "headLook", &look);
        send(a, w, "upperBodyFlinch", &[("start", b(true)), ("pos", Value::Vec3(at)), ("newHit", b(true))]);
    } else {
        arms_out(a);
        send(a, w, "headLook", &look);
    }
    lean(a, BRACE_LEAN, away);
}

fn look_hit(a: &mut NmAgent, w: &NmWorld, at: Vec3) {
    if a.more.xr & NM_XR_LOOK_HIT != 0 && !a.dead {
        send(a, w, "headLook", &[("start", Value::Bool(true)), ("pos", Value::Vec3(at)), ("alwaysLook", Value::Bool(true))]);
        a.more.xs.look_stop = Some(a.clock + 2000.0);
    }
}

fn step_in(a: &mut NmAgent, w: &NmWorld, dt: f32, others: &[delirium::ragdoll::Capsule]) -> i32 {
    if resting(a, w) {
        a.clock += dt as f64 * 1000.0;
        return 0;
    }
    if !(dt > 0.0 && dt <= 0.25) {
        return fail(NM_E_ARG, "dt out of (0, 0.25] (on NM's clock: dt / sqrt(scale))");
    }
    if a.mode == NM_MODE_FOLLOW && !a.set.iter().all(|&s| s) {
        return fail(NM_E_FOLLOW, "FOLLOW: set every part's state (nm_agent_set_part_state[s]) before each step");
    }
    if a.clock - a.frame >= 1000.0 / 30.0 - 1e-6 || a.clock == 0.0 {
        a.frame = a.clock;
        if let Some((mut t, health)) = a.shot.take() {
            let hit = nm::Hit { health, ..nm::Hit::ragdoll(&a.agent) };
            let step = with_tp(|tp| t.update(&mut RAND.lock().unwrap_or_else(|e| e.into_inner()), tp, &hit, a.clock as u32));
            with_tp(|tp| nm::task_shot_msgs(&mut a.agent, &a.build, &w.0, tp, &hit, &t, &step.msgs, false));
            if !step.done {
                a.shot = Some((t, health));
            }
        }
        if let Some((mut t, end)) = a.blast.take() {
            let pelvis = a.agent.doll.frames().next().unwrap_or_default().translation;
            let (vz, near) = (a.agent.doll.vel(0).0.z, w.0.contact(pelvis, Vec3::NEG_Z, 2.0).is_some());
            let set = with_tp(delirium::explosion::Set::read);
            let m = t.update(&set, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), a.clock as u32, vz, near);
            nm::send_task_msgs(&mut a.agent, &a.build, &w.0, &m);
            if a.clock < end {
                a.blast = Some((t, end));
            }
        }
    }
    if a.shock > 0.0 {
        a.shock -= dt;
        if a.shock <= 0.0 {
            let mut st = std::mem::take(&mut a.agent.msgs);
            let _ = msg::send(&mut a.agent, &a.build, &w.0, &mut st, "electrocute", &[("start", Value::Bool(false))]);
            a.agent.msgs = st;
        }
    }
    if let Some(mut g) = a.grab.take() {
        for f in &a.agent.feedback {
            if f.slot == 12 || f.slot == 8 {
                g.feedback(f.slot == 12, &f.name);
            }
        }
        match g.update(dt, false) {
            Some(m) => nm::send_task_msgs(&mut a.agent, &a.build, &w.0, &[m]),
            None => a.grab = Some(g),
        }
    }
    if let Some(mut f) = a.fall.take() {
        if a.agent.feedback.iter().any(|f| f.slot == 8 && f.name == "balance") {
            f.balance_failed();
        }
        f.control(&mut a.agent, &a.build, &w.0, dt);
        if !f.relaxed() {
            a.fall = Some(f);
        }
    }
    if !FIRE_REACTION.load(std::sync::atomic::Ordering::Relaxed) {
        if a.fire.as_ref().is_some_and(|f| !f.1) {
            a.fire = None;
        }
    } else if let Some((mut f, burning, health)) = a.fire.take() {
        if a.agent.feedback.iter().any(|f| f.slot == 8 && f.name == "balance") {
            f.balance_failed();
        }
        let done = f.update(&mut a.agent, &a.build, &w.0, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), dt, burning, health, health <= 0.0);
        if done.is_none() {
            a.fire = Some((f, burning, health));
        }
    }
    more_step(a, w, dt);
    a.clock += dt as f64 * 1000.0;
    for (k, local, anchor, len, now) in a.more.leash.iter_mut() {
        let Some(f) = a.agent.doll.frames().nth(*k) else { continue };
        let pin = *anchor - f.rotation * *local;
        *now = (*now - REEL * dt).max(*len).min(f.translation.distance(pin).max(*len));
        a.agent.doll.tether(*k, Some((pin, *now)));
    }
    if a.mode == NM_MODE_FOLLOW {
        a.agent.step_follow(&a.build, &w.0, dt, &w.1, others);
        let d = &mut a.agent.doll;
        for k in (0..d.out.len()).filter(|k| d.gone >> k & 1 != 0) {
            d.out[k] = (Vec3::ZERO, Vec3::ZERO);
        }
        for (k, j, l) in d.kicks.as_mut().map(std::mem::take).unwrap_or_default() {
            d.out[k].0 += j / dt;
            d.out[k].1 += l / dt;
        }
    } else if let Some((net, pick, n)) = a.more.learned.as_mut() {
        delirium::learned::step(net, &net.w, &mut a.agent, &a.build, &w.0, &a.more.zero, pick);
        *n += 1;
        if *n >= delirium::learned::STEPS() {
            learned_done(a, w);
        }
    } else {
        a.agent.step(&a.build, &w.0, dt, &w.1, others);
    }
    a.set.fill(false);
    a.still = if !a.agent.balancing() && a.agent.doll.com().1.length() < 0.2 { a.still + dt } else { 0.0 };
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_mode(a: *mut NmAgent, mode: i32) -> i32 {
    let a = agent!(a);
    if mode != NM_MODE_LEAD && mode != NM_MODE_FOLLOW {
        return fail(NM_E_ARG, "mode is NM_MODE_LEAD or NM_MODE_FOLLOW");
    }
    a.mode = mode;
    a.agent.doll.kicks = (mode == NM_MODE_FOLLOW).then(Vec::new);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_mode(a: *const NmAgent) -> i32 {
    agent_ref!(a).mode
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_part_count(a: *const NmAgent) -> i32 {
    agent_ref!(a).build.parts.len() as i32
}

fn part(a: &NmAgent, i: i32) -> Option<usize> {
    (0..a.build.parts.len() as i32).contains(&i).then_some(i as usize)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_part_transform(a: *const NmAgent, i: i32, pos: *mut f32, rot: *mut f32) -> i32 {
    let a = agent_ref!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    let f = frames(a)[k];
    unsafe {
        put3(pos, f.translation * a.u.len());
        put4(rot, f.rotation);
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_part_transforms(a: *const NmAgent, out: *mut f32, parts: i32) -> i32 {
    let a = agent_ref!(a);
    if out.is_null() {
        return fail(NM_E_ARG, "null out");
    }
    let mut n = 0;
    for (k, f) in frames(a).into_iter().enumerate().take(parts.max(0) as usize) {
        unsafe {
            put3(out.add(k * 7), f.translation * a.u.len());
            put4(out.add(k * 7 + 3), f.rotation);
        }
        n += 1;
    }
    n
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_part_velocity(a: *const NmAgent, i: i32, vel: *mut f32, angvel: *mut f32) -> i32 {
    let a = agent_ref!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    let (v, w) = a.cuts.iter().find_map(|c| c.3.iter().position(|&p| p == k).map(|i| c.1.motion(i))).unwrap_or(a.agent.doll.motion(k));
    unsafe {
        put3(vel, v * a.u.time());
        put3(angvel, w / a.u.time());
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_part_state(a: *mut NmAgent, i: i32, pos: *const f32, rot: *const f32, vel: *const f32, angvel: *const f32) -> i32 {
    let a = agent!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    if [pos, rot, vel, angvel].iter().any(|p| p.is_null()) {
        return fail(NM_E_ARG, "null vector");
    }
    let q = unsafe { Quat::from_xyzw(*rot, *rot.add(1), *rot.add(2), *rot.add(3)) };
    let u = a.u;
    a.agent.doll.set_body(k, unsafe { v3(pos) } / u.len(), q, unsafe { v3(vel) } / u.time(), unsafe { v3(angvel) } * u.time());
    a.set[k] = true;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_part_states(a: *mut NmAgent, states: *const f32, parts: i32) -> i32 {
    if states.is_null() {
        return fail(NM_E_ARG, "null states");
    }
    for k in 0..parts.max(0) as usize {
        let s = unsafe { states.add(k * 13) };
        let r = unsafe { nm_agent_set_part_state(a, k as i32, s, s.add(3), s.add(7), s.add(10)) };
        if r < 0 {
            return r;
        }
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_incoming_transforms(a: *mut NmAgent, transforms: *const f32, parts: i32) -> i32 {
    let a = agent!(a);
    let n = parts.max(0) as usize;
    if n > 0 && (transforms.is_null() || n != a.agent.doll.frames().count()) {
        return fail(NM_E_ARG, "need every part's transform");
    }
    let u = a.u;
    a.agent.incoming = (0..n).map(|k| unsafe {
        let s = transforms.add(k * 7);
        Transform { translation: v3(s) / u.len(), rotation: Quat::from_xyzw(*s.add(3), *s.add(4), *s.add(5), *s.add(6)), ..Default::default() }
    }).collect();
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_part_drive(a: *const NmAgent, i: i32, force: *mut f32, torque: *mut f32) -> i32 {
    let a = agent_ref!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    let (f, t) = a.agent.doll.out[k];
    unsafe {
        put3(force, f * a.u.mass());
        put3(torque, t * a.u.mass() * a.u.len());
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_part_drives(a: *const NmAgent, out: *mut f32, parts: i32) -> i32 {
    let a = agent_ref!(a);
    if out.is_null() {
        return fail(NM_E_ARG, "null out");
    }
    let n = (parts.max(0) as usize).min(a.agent.doll.out.len());
    for (k, (f, t)) in a.agent.doll.out.iter().take(n).enumerate() {
        unsafe {
            put3(out.add(k * 6), *f * a.u.mass());
            put3(out.add(k * 6 + 3), *t * a.u.mass() * a.u.len());
        }
    }
    n as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_part_contact(a: *const NmAgent, i: i32) -> i32 {
    let a = agent_ref!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    a.agent.doll.touch[k] as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_com(a: *const NmAgent, pos: *mut f32, vel: *mut f32) -> i32 {
    let a = agent_ref!(a);
    let (x, v) = a.agent.doll.com();
    unsafe {
        put3(pos, x * a.u.len());
        put3(vel, v * a.u.time());
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_mass(a: *const NmAgent) -> f32 {
    unsafe { a.as_ref() }.map_or(0.0, |a| a.agent.doll.mass() * a.u.mass())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_kinetic_energy(a: *const NmAgent) -> f32 {
    unsafe { a.as_ref() }.map_or(0.0, |a| a.agent.doll.kinetic() * a.u.mass() * a.u.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_apply_impulse(a: *mut NmAgent, i: i32, at: *const f32, impulse: *const f32) -> i32 {
    let a = agent!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    if at.is_null() || impulse.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    let u = a.u;
    let (at, imp) = (unsafe { v3(at) } / u.len(), unsafe { v3(impulse) } / u.impulse());
    a.agent.doll.impulse(k, at, imp);
    a.more.look = Some(at - imp.normalize_or_zero() * 2.0);
    partial_hit(a, imp.length());
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_apply_push(a: *mut NmAgent, at: *const f32, impulse: *const f32) -> i32 {
    let a = agent!(a);
    if at.is_null() || impulse.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    let u = a.u;
    let (at, imp) = (unsafe { v3(at) } / u.len(), unsafe { v3(impulse) } / u.impulse());
    a.agent.doll.push(&a.build, at, imp);
    a.more.look = Some(at - imp.normalize_or_zero() * 2.0);
    partial_hit(a, imp.length());
    0
}

const REEL: f32 = 2.0;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_leash(a: *mut NmAgent, i: i32, at: *const f32, anchor: *const f32, length: f32) -> i32 {
    let a = agent!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    if !(length > 0.0) {
        return fail(NM_E_ARG, "length must be over 0");
    }
    let u = a.u;
    let was = a.more.leash.iter().position(|l| l.0 == k);
    let Some(f) = a.agent.doll.frames().nth(k) else { return fail(NM_E_ARG, "no such part") };
    let local = if at.is_null() { was.map(|w| a.more.leash[w].1) } else { Some(f.rotation.inverse() * (unsafe { v3(at) } / u.len() - f.translation)) };
    let anchor = if anchor.is_null() { was.map(|w| a.more.leash[w].2) } else { Some(unsafe { v3(anchor) } / u.len()) };
    let (Some(local), Some(anchor)) = (local, anchor) else { return fail(NM_E_ARG, "null vector (and no leash on the part to keep it from)") };
    let pin = anchor - f.rotation * local;
    let len = length / u.len();
    let now = f.translation.distance(pin).max(len);
    match was {
        Some(w) => a.more.leash[w] = (k, local, anchor, len, now.min(a.more.leash[w].4.max(len))),
        None => a.more.leash.push((k, local, anchor, len, now)),
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_unleash(a: *mut NmAgent, i: i32) -> i32 {
    let a = agent!(a);
    if i != -1 && part(a, i).is_none() {
        return fail(NM_E_ARG, "no such part");
    }
    let doll = &mut a.agent.doll;
    a.more.leash.retain(|l| {
        let keep = i != -1 && l.0 != i as usize;
        if !keep {
            doll.tether(l.0, None);
        }
        keep
    });
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_nearest_part(a: *const NmAgent, at: *const f32) -> i32 {
    let a = agent_ref!(a);
    if at.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    a.agent.doll.nearest(&a.build, unsafe { v3(at) } / a.u.len()) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_raycast(a: *const NmAgent, from: *const f32, to: *const f32, point: *mut f32, normal: *mut f32) -> i32 {
    let a = agent_ref!(a);
    if from.is_null() || to.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    match a.agent.doll.probe(&a.build, unsafe { v3(from) } / a.u.len(), unsafe { v3(to) } / a.u.len()) {
        Some((k, p, n)) => {
            unsafe {
                put3(point, p * a.u.len());
                put3(normal, n);
            }
            k as i32
        }
        None => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_add_velocity(a: *mut NmAgent, dv: *const f32) -> i32 {
    let a = agent!(a);
    if dv.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    a.agent.doll.kick(unsafe { v3(dv) } / a.u.time());
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_joint(a: *const NmAgent, i: i32, parent: *mut i32, pivot: *mut f32, frame: *mut f32, limits: *mut f32,
                                            armature: *mut f32) -> i32 {
    let a = agent_ref!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    let Some(j) = a.agent.joints(&a.build)[k] else { return 0 };
    let pf = a.agent.doll.frames().nth(j.parent).unwrap_or_default();
    unsafe {
        if !parent.is_null() {
            *parent = j.parent as i32;
        }
        put3(pivot, (pf.translation + pf.rotation * j.anchor_parent) * a.u.len());
        put4(frame, pf.rotation * j.in_parent);
        put3(limits, Vec3::from_array(j.limits));
        if !armature.is_null() {
            *armature = a.agent.doll.armature.get(k).copied().unwrap_or(0.0) * a.u.mass() * a.u.len() * a.u.len() * a.agent.short;
        }
    }
    if j.hinge { 2 } else { 1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_part_body(a: *const NmAgent, i: i32, mass: *mut f32, inertia: *mut f32, r#box: *mut f32) -> i32 {
    let a = agent_ref!(a);
    let Some(k) = part(a, i) else { return fail(NM_E_ARG, "no such part") };
    let (m, j) = a.agent.doll.mass_inertia(k);
    let b = a.build.parts[k].boxed;
    unsafe {
        if !mass.is_null() { *mass = m * a.u.mass() }
        put3(inertia, j * a.u.mass() * a.u.len() * a.u.len());
        put3(r#box, b.unwrap_or(Vec3::ZERO) * a.u.len());
    }
    if b.is_some() { 2 } else { 1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_zero_pose(a: *const NmAgent, pos: *mut f32, rot: *mut f32, parts: i32) -> i32 {
    let a = agent_ref!(a);
    let joints = a.agent.joints(&a.build);
    if parts != joints.len() as i32 { return fail(NM_E_ARG, "parts isn't the part count") }
    let mut at: Vec<Transform> = a.agent.doll.frames().take(1).collect();
    for j in joints.iter().skip(1) {
        let Some(j) = j else { return fail(NM_E_STATE, "a part without a joint") };
        let Some(&p) = at.get(j.parent) else { return fail(NM_E_STATE, "a part before its parent") };
        let r = (p.rotation * j.in_parent * j.in_child.inverse()).normalize();
        at.push(Transform::from_translation(p.translation + p.rotation * j.anchor_parent - r * j.anchor_child).with_rotation(r));
    }
    for (k, t) in at.iter().enumerate() {
        unsafe {
            if !pos.is_null() { put3(pos.add(k * 3), t.translation * a.u.len()) }
            if !rot.is_null() { put4(rot.add(k * 4), t.rotation) }
        }
    }
    0
}

fn messages() -> impl Iterator<Item = &'static msg::Message> {
    msg::MESSAGES.iter().chain(delirium::extras::messages())
}

fn message(m: i32) -> Option<&'static msg::Message> {
    usize::try_from(m).ok().and_then(|m| messages().nth(m))
}

fn param(m: i32, p: i32) -> Option<&'static msg::Param> {
    message(m).and_then(|m| m.params.get(p as usize)).filter(|_| p >= 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_message_count() -> i32 {
    messages().count() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_message_name(m: i32, buf: *mut c_char, len: i32) -> i32 {
    match message(m) {
        Some(m) => unsafe { put(m.name, buf, len) },
        None => fail(NM_E_MESSAGE, "no such message"),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_message_find(name: *const c_char) -> i32 {
    let n = unsafe { text(name) }.unwrap_or("");
    messages().position(|m| m.name == n).map_or_else(|| fail(NM_E_MESSAGE, format!("no message {n}")), |i| i as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_message_param_count(m: i32) -> i32 {
    message(m).map_or_else(|| fail(NM_E_MESSAGE, "no such message"), |m| m.params.len() as i32)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_message_param_name(m: i32, p: i32, buf: *mut c_char, len: i32) -> i32 {
    match param(m, p) {
        Some(p) => unsafe { put(p.name, buf, len) },
        None => fail(NM_E_PARAM, "no such param"),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_message_param_find(m: i32, name: *const c_char) -> i32 {
    let n = unsafe { text(name) }.unwrap_or("");
    message(m).and_then(|m| m.params.iter().position(|p| p.name == n)).map_or_else(|| fail(NM_E_PARAM, format!("no param {n}")), |i| i as i32)
}

pub const NM_KIND_FLOAT: i32 = 0;
pub const NM_KIND_INT: i32 = 1;
pub const NM_KIND_BOOL: i32 = 2;
pub const NM_KIND_VEC3: i32 = 3;
pub const NM_KIND_STRING: i32 = 4;

#[unsafe(no_mangle)]
pub extern "C" fn nm_message_param_kind(m: i32, p: i32) -> i32 {
    param(m, p).map_or_else(|| fail(NM_E_PARAM, "no such param"), |p| match p.kind {
        Kind::Float => NM_KIND_FLOAT,
        Kind::Int => NM_KIND_INT,
        Kind::Bool => NM_KIND_BOOL,
        Kind::Vec3 => NM_KIND_VEC3,
        Kind::Str => NM_KIND_STRING,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_message_param_used(m: i32, p: i32) -> i32 {
    param(m, p).map_or_else(|| fail(NM_E_PARAM, "no such param"), |p| p.used as i32)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_message_param_doc(m: i32, p: i32, buf: *mut c_char, len: i32) -> i32 {
    match param(m, p) {
        Some(p) => unsafe { put(p.doc, buf, len) },
        None => fail(NM_E_PARAM, "no such param"),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_message_param_default(m: i32, p: i32, out: *mut f32) -> i32 {
    let (Some(mm), Some(pp)) = (message(m), param(m, p)) else { return fail(NM_E_PARAM, "no such param") };
    unsafe { floats(msg::get(&msg::State::default(), mm.name, pp.name), out) }
}

unsafe fn floats(v: Option<Value>, out: *mut f32) -> i32 {
    let Some(v) = v else { return fail(NM_E_STATE, "the port doesn't keep this param") };
    let f: &[f32] = match &v {
        Value::Float(x) => &[*x],
        Value::Int(i) => &[*i as f32],
        Value::Bool(b) => &[*b as i32 as f32],
        Value::Vec3(v) => &v.to_array(),
        Value::Str(_) => &[],
    };
    if !out.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(f.as_ptr(), out, f.len()) };
    }
    f.len() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_begin(a: *mut NmAgent, name: *const c_char) -> i32 {
    let a = agent!(a);
    let Some(n) = (unsafe { text(name) }) else { return fail(NM_E_ARG, "no name") };
    if msg::find(n).is_none() {
        return fail(NM_E_MESSAGE, format!("no message {n}"));
    }
    a.pending = Some((n.to_string(), vec![]));
    0
}

unsafe fn add(a: *mut NmAgent, name: *const c_char, v: Value) -> i32 {
    let a = agent!(a);
    let Some(n) = (unsafe { text(name) }) else { return fail(NM_E_ARG, "no name") };
    let Some((m, ps)) = a.pending.as_mut() else { return fail(NM_E_STATE, "nm_msg_begin first") };
    if n != "start" && !msg::find(m).is_some_and(|m| m.params.iter().any(|p| p.name == n)) {
        return fail(NM_E_PARAM, format!("{m} has no param {n}"));
    }
    ps.push((n.to_string(), v));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_set_float(a: *mut NmAgent, name: *const c_char, v: f32) -> i32 {
    unsafe { add(a, name, Value::Float(v)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_set_int(a: *mut NmAgent, name: *const c_char, v: i32) -> i32 {
    unsafe { add(a, name, Value::Int(v)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_set_bool(a: *mut NmAgent, name: *const c_char, v: i32) -> i32 {
    unsafe { add(a, name, Value::Bool(v != 0)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_set_vec3(a: *mut NmAgent, name: *const c_char, v: *const f32) -> i32 {
    if v.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    unsafe { add(a, name, Value::Vec3(v3(v))) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_set_string(a: *mut NmAgent, name: *const c_char, v: *const c_char) -> i32 {
    let Some(s) = (unsafe { text(v) }) else { return fail(NM_E_ARG, "null string") };
    unsafe { add(a, name, Value::Str(s.to_string())) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_override(a: *mut NmAgent, behaviour: *const c_char, param: *const c_char, value: f32) -> i32 {
    let a = agent!(a);
    let (Some(m), Some(p)) = (unsafe { text(behaviour) }, unsafe { text(param) }) else { return fail(NM_E_ARG, "nm_agent_set_override: no behaviour or param") };
    let Some(msg) = msg::MESSAGES.iter().chain(delirium::extras::messages()).find(|x| x.name == m) else { return fail(NM_E_MESSAGE, format!("no message {m}")) };
    let Some(par) = msg.params.iter().find(|x| x.name == p) else { return fail(NM_E_PARAM, format!("{m}: no param {p}")) };
    let v = match par.kind {
        Kind::Float => Value::Float(value),
        Kind::Int => Value::Int(value.round() as i32),
        Kind::Bool => Value::Bool(value != 0.0),
        _ => return fail(NM_E_TYPE, format!("{m}.{p}: not a number, whole number or on/off")),
    };
    if !par.used || ["bodyPart", "partIndex", "hitPart"].contains(&p) || !value.is_finite() {
        return fail(NM_E_PARAM, format!("{m}.{p}: the port doesn't act on it, each hit sets it, or not a number"));
    }
    a.agent.overrides.retain(|(om, op, _)| (*om, *op) != (msg.name, par.name));
    a.agent.overrides.push((msg.name, par.name, v));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_recentre_joint(a: *mut NmAgent, i: i32) -> i32 {
    let a = agent!(a);
    let parts: Vec<usize> = if i == -1 {
        (1..a.build.parts.len()).collect()
    } else {
        match part(a, i) { Some(k) => vec![k], None => return fail(NM_E_ARG, "no such part") }
    };
    parts.into_iter().filter(|&k| a.agent.recentre_joint(k)).count() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_hold(a: *mut NmAgent, mask: *const c_char, stiffness: f32, damping: f32, oppose_gravity: f32) -> i32 {
    let a = agent!(a);
    let Some(mask) = (unsafe { text(mask) }).map(|m| m.to_string()) else { return fail(NM_E_ARG, "nm_agent_hold: no mask") };
    if delirium::nmrs::Character::mask(&mask).is_empty() {
        return fail(NM_E_ARG, format!("nm_agent_hold: no such mask {mask:?}"));
    }
    a.agent.hold.retain(|h| h.0 != mask);
    if stiffness > 0.0 {
        a.agent.hold.push((mask, stiffness, damping, oppose_gravity));
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_follow(a: *mut NmAgent, mask: *const c_char, weight: f32) -> i32 {
    let a = agent!(a);
    let Some(mask) = (unsafe { text(mask) }).map(|m| m.to_string()) else { return fail(NM_E_ARG, "nm_agent_follow: no mask") };
    if delirium::nmrs::Character::mask(&mask).is_empty() || !weight.is_finite() {
        return fail(NM_E_ARG, format!("nm_agent_follow: no such mask {mask:?} or weight {weight}"));
    }
    a.agent.follow.retain(|f| f.0 != mask);
    if weight > 0.0 {
        a.agent.follow.push((mask, weight.min(1.0)));
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_keyframe(a: *mut NmAgent, mask: *const c_char, on: i32) -> i32 {
    let a = agent!(a);
    let Some(mask) = (unsafe { text(mask) }) else { return fail(NM_E_ARG, "nm_agent_keyframe: no mask") };
    let eff = delirium::nmrs::Character::mask(mask);
    if eff.is_empty() {
        return fail(NM_E_ARG, format!("nm_agent_keyframe: no such mask {mask:?}"));
    }
    let hips = (*delirium::nmrs::HIP()).iter().any(|h| eff.contains(h));
    let n = a.build.parts.len();
    let parts = eff.into_iter().map(|i| i + 1).chain(hips.then_some(0)).filter(|&k| k < n);
    for k in parts {
        a.agent.keyframe.retain(|&x| x != k);
        if on != 0 {
            a.agent.keyframe.push(k);
        } else {
            a.agent.doll.unpin_part(k);
        }
    }
    0
}

pub const NM_PARTIAL_OFF: i32 = 0;
pub const NM_PARTIAL_LEGS: i32 = 1;
pub const NM_PARTIAL_UPPER: i32 = 2;
pub const NM_PARTIAL_NONE: i32 = 0;
pub const NM_PARTIAL_KNOCKED: i32 = 1;
pub const NM_PARTIAL_STUMBLED: i32 = 2;
pub const NM_PARTIAL_FELL: i32 = 3;
pub const NM_PARTIAL_RECOVERED: i32 = 4;
pub const NM_PARTIAL_RESUMED: i32 = 5;
pub const NM_PARTIAL_DROPPED: i32 = 6;

fn partial_event(a: &mut NmAgent, w: Option<&NmWorld>, e: Option<delirium::partial::Event>) {
    use delirium::partial::Event::*;
    let Some(e) = e else { return };
    if let (Fell, Some(w)) = (e, w) {
        send(a, w, "catchFall", &[("start", Value::Bool(true))]);
    }
    a.more.partial_event = match e {
        Knocked => NM_PARTIAL_KNOCKED, Stumbled => NM_PARTIAL_STUMBLED, Fell => NM_PARTIAL_FELL, Recovered => NM_PARTIAL_RECOVERED,
        Resumed => NM_PARTIAL_RESUMED, Dropped => NM_PARTIAL_DROPPED,
    };
}

fn partial_step(a: &mut NmAgent, w: &NmWorld, dt: f32) {
    let Some(mut p) = a.more.partial.take() else { return };
    let e = if a.dead { p.die(&mut a.agent) } else { None };
    let e = e.or_else(|| p.step(&mut a.agent, dt, a.dead));
    a.more.partial = Some(p);
    partial_event(a, Some(w), e);
}

fn partial_hit(a: &mut NmAgent, ns: f32) {
    let Some(mut p) = a.more.partial.take() else { return };
    let e = p.hit(&mut a.agent, ns);
    a.more.partial = Some(p);
    partial_event(a, None, e);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_partial(a: *mut NmAgent, mode: i32) -> i32 {
    let a = agent!(a);
    let mode = match mode {
        NM_PARTIAL_OFF => None,
        NM_PARTIAL_LEGS => Some(delirium::partial::Mode::Legs),
        NM_PARTIAL_UPPER => Some(delirium::partial::Mode::Upper),
        _ => return fail(NM_E_ARG, format!("nm_agent_partial: no mode {mode}")),
    };
    let mut p = a.more.partial.take().unwrap_or_else(|| delirium::partial::Partial::new(delirium::partial::Mode::Legs));
    p.stop(&mut a.agent);
    if let Some(m) = mode {
        p.mode = m;
        p.start(&mut a.agent);
    }
    a.more.partial = Some(p);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_partial_options(a: *mut NmAgent, keep_running: i32, breaks: i32) -> i32 {
    let a = agent!(a);
    let p = a.more.partial.get_or_insert_with(|| delirium::partial::Partial::new(delirium::partial::Mode::Legs));
    (p.keep_running, p.breaks) = (keep_running != 0, breaks != 0);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_partial_hit(a: *mut NmAgent, ns: f32) -> i32 {
    let a = agent!(a);
    partial_hit(a, ns);
    std::mem::take(&mut a.more.partial_event)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_partial_event(a: *mut NmAgent) -> i32 {
    std::mem::take(&mut agent!(a).more.partial_event)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_partial_state(a: *const NmAgent) -> i32 {
    agent_ref!(a).more.partial.as_ref().map_or(0, |p| if p.animating() { 1 } else if p.active() { 2 } else { 0 })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_clear_overrides(a: *mut NmAgent) -> i32 {
    agent!(a).agent.overrides.clear();
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_send(a: *mut NmAgent, w: *const NmWorld) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let Some((m, ps)) = a.pending.take() else { return fail(NM_E_STATE, "nm_msg_begin first") };
    let ps: Vec<(&str, Value)> = ps.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
    let mut st = std::mem::take(&mut a.agent.msgs);
    let r = msg::send(&mut a.agent, &a.build, &w.0, &mut st, &m, &ps);
    a.agent.msgs = st;
    if let (true, Some(Value::Vec3(i))) = (r.is_ok() && m == "applyImpulse", ps.iter().find(|p| p.0 == "impulse").map(|p| &p.1)) {
        let ns = i.length();
        partial_hit(a, ns);
    }
    match r {
        Ok(n) => n as i32,
        Err(msg::Error::Message) => fail(NM_E_MESSAGE, format!("no message {m}")),
        Err(msg::Error::Param) => fail(NM_E_PARAM, format!("{m}: a param it doesn't take")),
        Err(msg::Error::Type) => fail(NM_E_TYPE, format!("{m}: a param of the wrong type")),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_send_text(a: *mut NmAgent, w: *const NmWorld, line: *const c_char) -> i32 {
    let Some(line) = (unsafe { text(line) }) else { return fail(NM_E_ARG, "no text") };
    let mut words = line.split_whitespace();
    let Some(name) = words.next().and_then(|n| std::ffi::CString::new(n).ok()) else { return fail(NM_E_ARG, "no message name") };
    let r = unsafe { nm_msg_begin(a, name.as_ptr()) };
    if r < 0 {
        return r;
    }
    let m = msg::find(name.to_str().unwrap_or("")).expect("found by nm_msg_begin");
    for kv in words {
        let Some((k, v)) = kv.split_once('=') else { unsafe { nm_msg_cancel(a) }; return fail(NM_E_ARG, format!("{kv}: want name=value")) };
        let kind = if k == "start" { Some(msg::Kind::Bool) } else { m.params.iter().find(|p| p.name == k).map(|p| p.kind) };
        let value = kind.and_then(|kind| Some(match kind {
            msg::Kind::Float => Value::Float(v.parse().ok()?),
            msg::Kind::Int => Value::Int(v.parse().ok()?),
            msg::Kind::Bool => Value::Bool(match v { "true" | "1" => true, "false" | "0" => false, _ => return None }),
            msg::Kind::Vec3 => {
                let f: Vec<f32> = v.split(',').map(|x| x.parse().ok()).collect::<Option<_>>()?;
                Value::Vec3(Vec3::new(*f.first()?, *f.get(1)?, *f.get(2)?))
            }
            msg::Kind::Str => Value::Str(v.to_string()),
        }));
        let Some(value) = value else {
            unsafe { nm_msg_cancel(a) };
            return fail(if kind.is_none() { NM_E_PARAM } else { NM_E_TYPE }, format!("{}: {kv}", m.name));
        };
        let k = std::ffi::CString::new(k).unwrap_or_default();
        let r = unsafe { add(a, k.as_ptr(), value) };
        if r < 0 {
            unsafe { nm_msg_cancel(a) };
            return r;
        }
    }
    unsafe { nm_msg_send(a, w) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_msg_cancel(a: *mut NmAgent) -> i32 {
    agent!(a).pending = None;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_param(a: *const NmAgent, message: *const c_char, param: *const c_char, out: *mut f32) -> i32 {
    let a = agent_ref!(a);
    let (Some(m), Some(p)) = (unsafe { text(message) }, unsafe { text(param) }) else { return fail(NM_E_ARG, "no name") };
    unsafe { floats(msg::get(&a.agent.msgs, m, p), out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_param_string(a: *const NmAgent, message: *const c_char, param: *const c_char, buf: *mut c_char, len: i32) -> i32 {
    let a = agent_ref!(a);
    let (Some(m), Some(p)) = (unsafe { text(message) }, unsafe { text(param) }) else { return fail(NM_E_ARG, "no name") };
    match msg::get(&a.agent.msgs, m, p) {
        Some(Value::Str(s)) => unsafe { put(&s, buf, len) },
        Some(_) => fail(NM_E_TYPE, "not a string param"),
        None => fail(NM_E_STATE, "the port doesn't keep this param"),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_balancing(a: *const NmAgent) -> i32 {
    agent_ref!(a).agent.balancing() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_has_fallen(a: *const NmAgent) -> i32 {
    agent_ref!(a).agent.fallen as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_lying(a: *const NmAgent, head: *mut f32, face_up: *mut i32) -> f32 {
    let Some(a) = (unsafe { a.as_ref() }) else { return 0.0 };
    let f: Vec<Transform> = a.agent.doll.frames().collect();
    let pelvis = f.first().copied().unwrap_or_default();
    let top = f.get(12).or(f.last()).copied().unwrap_or_default();
    let dir = (top.translation - pelvis.translation).with_z(0.0).normalize_or(pelvis.rotation * Vec3::Y);
    unsafe {
        put3(head, dir);
        if !face_up.is_null() {
            let side = f.get(1).zip(f.get(4)).map_or(Vec3::X, |(l, r)| l.translation - r.translation);
            *face_up = (side.cross(top.translation - pelvis.translation).z > 0.0) as i32;
        }
    }
    a.still * a.u.time()
}

fn resting(a: &NmAgent, w: &NmWorld) -> bool {
    if !a.dead || a.still < 2.0 || a.mode == NM_MODE_FOLLOW || a.shot.is_some() || a.blast.is_some() || a.fire.is_some() {
        return false;
    }
    let pelvis = a.agent.doll.frames().next().unwrap_or_default().translation;
    !w.1.iter().any(|o| (o.vel.length_squared() > 1e-4 || o.spin.length_squared() > 1e-4) && o.at.distance(pelvis) < 3.0 + o.half.length())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_resting(a: *const NmAgent, w: *const NmWorld) -> i32 {
    let a = agent_ref!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    resting(a, w) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_set_step_limit(on: i32) {
    delirium::nmdbm::STEP_LIMIT.store(on != 0, std::sync::atomic::Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_set_fire_reaction(on: i32) {
    FIRE_REACTION.store(on != 0, std::sync::atomic::Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_set_headshot_kills(on: i32) {
    HEADSHOT_KILLS.store(on != 0, std::sync::atomic::Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_dead(a: *const NmAgent) -> i32 {
    agent_ref!(a).dead as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_dead(a: *mut NmAgent, dead: i32) -> i32 {
    agent!(a).dead = dead != 0;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_should_get_up(a: *const NmAgent, after: f32) -> i32 {
    let a = agent_ref!(a);
    (!a.dead && !legless(a) && a.more.xs.curl.is_none() && after > 0.0 && a.still * a.u.time() >= after) as i32
}

fn legless(a: &NmAgent) -> bool {
    a.cuts.iter().any(|c| c.3.iter().any(|p| (1..=6).contains(p)))
}

fn frames(a: &NmAgent) -> Vec<Transform> {
    let mut f: Vec<Transform> = a.agent.doll.frames().collect();
    for (_, d, _, parts) in &a.cuts {
        for (t, &p) in d.frames().zip(parts) {
            f[p] = t;
        }
    }
    f
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_sever(a: *mut NmAgent, w: *const NmWorld, part_index: i32, at: *const f32, impulse: *const f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let Some(k) = part(a, part_index) else { return fail(NM_E_ARG, "no such part") };
    let Some((b, mut d, parts)) = a.agent.doll.sever(&a.build, k) else { return fail(NM_E_STATE, "nm_agent_sever: the root, or cut off already") };
    if a.mode == NM_MODE_FOLLOW {
        if !impulse.is_null() {
            let c = a.agent.doll.centre(k);
            let (x, j) = (if at.is_null() { c } else { unsafe { v3(at) / a.u.len() } }, unsafe { v3(impulse) } / a.u.impulse());
            if let Some(kicks) = a.agent.doll.kicks.as_mut() { kicks.push((k, j, (x - c).cross(j))); }
        }
        if legless(a) {
            send(a, w, "catchFall", &[("start", Value::Bool(true))]);
        }
        return 0;
    }
    if !impulse.is_null() {
        let x = if at.is_null() { d.centre(0) } else { unsafe { v3(at) / a.u.len() } };
        d.impulse(0, x, unsafe { v3(impulse) } / a.u.impulse());
    }
    let all = a.agent.limp_rage();
    let rigid = parts.iter().map(|&p| all.get(p).copied().unwrap_or_default()).collect();
    a.cuts.push((b, d, rigid, parts));
    if legless(a) {
        send(a, w, "catchFall", &[("start", Value::Bool(true))]);
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_severed(a: *const NmAgent, part_index: i32) -> i32 {
    let a = agent_ref!(a);
    part(a, part_index).is_some_and(|k| a.agent.doll.gone >> k & 1 != 0) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_aim_hit(a: *const NmAgent, from: *const f32, dir: *const f32, point: *mut f32) -> i32 {
    let a = agent_ref!(a);
    if from.is_null() || dir.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    let (from, dir) = unsafe { (v3(from) / a.u.len(), v3(dir).normalize_or_zero()) };
    if let Some((k, p, _)) = a.agent.doll.probe(&a.build, from, from + dir * 1000.0) {
        unsafe { put3(point, p * a.u.len()) };
        return k as i32;
    }
    let on = |p: Vec3| from + dir * (p - from).dot(dir).max(0.0);
    let Some((k, p)) = a.agent.doll.frames().map(|f| f.translation).enumerate()
        .min_by(|x, y| on(x.1).distance_squared(x.1).total_cmp(&on(y.1).distance_squared(y.1))) else { return -1 };
    unsafe { put3(point, on(p) * a.u.len()) };
    k as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_settled(a: *const NmAgent) -> i32 {
    agent_ref!(a).agent.settled() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_relaxing(a: *const NmAgent) -> i32 {
    agent_ref!(a).agent.relaxing() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_catching_fall(a: *const NmAgent) -> i32 {
    agent_ref!(a).agent.catching() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_rolled_up(a: *const NmAgent) -> i32 {
    agent_ref!(a).agent.rolled_up() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_awake(a: *const NmAgent) -> i32 {
    agent_ref!(a).agent.doll.awake() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_get_stance(a: *const NmAgent, pos: *mut f32) -> i32 {
    let a = agent_ref!(a);
    match a.agent.stance() {
        Some(p) => {
            unsafe { put3(pos, p * a.u.len()) };
            1
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_feedback_count(a: *const NmAgent) -> i32 {
    agent_ref!(a).agent.feedback.len() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_feedback_get(a: *const NmAgent, i: i32, buf: *mut c_char, len: i32, slot: *mut i32, value: *mut f32) -> i32 {
    let a = agent_ref!(a);
    let Some(f) = a.agent.feedback.get(i as usize).filter(|_| i >= 0) else { return fail(NM_E_ARG, "no such feedback") };
    unsafe {
        if !slot.is_null() {
            *slot = f.slot as i32;
        }
        if !value.is_null() {
            *value = f.value;
        }
        put(f.name, buf, len)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_shot(a: *mut NmAgent, w: *const NmWorld, set: *const c_char, part: i32, at: *const f32, dir: *const f32, health: f32) -> i32 {
    unsafe { nm_agent_task_shot_force(a, w, set, part, at, dir, health, SHOT_FORCE) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_shot_force(a: *mut NmAgent, w: *const NmWorld, set: *const c_char, part: i32, at: *const f32, dir: *const f32, health: f32, force: f32) -> i32 {
    if !(force > 0.0 && force.is_finite()) {
        return fail(NM_E_ARG, "nm_agent_task_shot_force: force over 0");
    }
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if at.is_null() || dir.is_null() || !(0..a.build.parts.len() as i32).contains(&part) {
        return fail(NM_E_ARG, "bad hit");
    }
    let set = unsafe { text(set) }.unwrap_or("*");
    let health = if part == 12 && HEADSHOT_KILLS.load(std::sync::atomic::Ordering::Relaxed) { 0.0 } else { health };
    a.dead |= health < 50.0;
    let hit = nm::Hit { health, ..nm::Hit::ragdoll(&a.agent) };
    let dir = unsafe { v3(dir) }.normalize_or_zero();
    let (at, vel, now) = (unsafe { v3(at) } / a.u.len(), dir * force / a.u.time(), a.clock as u32);
    let mut rand = RAND.lock().unwrap_or_else(|e| e.into_inner());
    let (t, msgs, next) = match a.shot.take() {
        Some((mut t, _)) => {
            let msgs = with_tp(|tp| t.next_hit(&mut rand, tp, &hit, now, set, part as usize, at, vel, -dir));
            (t, msgs, true)
        }
        None => {
            let mut t = delirium::shottask::ShotTask::new(&hit, set, part as usize, at, vel, -dir);
            let msgs = with_tp(|tp| t.update(&mut rand, tp, &hit, now).msgs);
            (t, msgs, false)
        }
    };
    drop(rand);
    with_tp(|tp| nm::task_shot_msgs(&mut a.agent, &a.build, &w.0, tp, &hit, &t, &msgs, next));
    a.shot = Some((t, health));
    a.frame = a.clock;
    0
}

#[derive(Clone)]
struct Weapon {
    name: String,
    set: String,
    v: [f32; 7],
}

static WEAPONS: RwLock<Vec<Weapon>> = RwLock::new(Vec::new());
const WEAPONS_DEFAULT: &str = "pistol=PISTOL_WEAPON:30:25:1:300:17:0.5:50, smg=SMG_WEAPON:25:15:1:70:30:0.5:50, rifle=POWERFUL_WEAPON:45:35:1:130:30:0.5:70, shotgun=POWERFUL_WEAPON:90:80:16:1200:8:0.1:35";

fn weapons() -> std::sync::RwLockReadGuard<'static, Vec<Weapon>> {
    if WEAPONS.read().unwrap_or_else(|e| e.into_inner()).is_empty() {
        let mut t = WEAPONS.write().unwrap_or_else(|e| e.into_inner());
        if t.is_empty() {
            *t = parse_weapons(WEAPONS_DEFAULT).unwrap_or_default();
        }
    }
    WEAPONS.read().unwrap_or_else(|e| e.into_inner())
}

fn parse_weapons(t: &str) -> Result<Vec<Weapon>, String> {
    let mut out = vec![];
    for row in t.split(',').map(str::trim).filter(|r| !r.is_empty()) {
        let bad = || format!("{row}: want name=set:push:damage[:pellets:between:clip:accuracy:range]");
        let (name, rest) = row.split_once('=').ok_or_else(bad)?;
        let mut f = rest.split(':').map(str::trim);
        let set = f.next().filter(|s| !s.is_empty()).ok_or_else(bad)?.to_string();
        let mut v = [0.0, 0.0, 1.0, 300.0, 30.0, 0.5, 50.0];
        let nums: Vec<f32> = f.map(|x| x.parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
        if !(2..=7).contains(&nums.len()) {
            return Err(bad());
        }
        v[..nums.len()].copy_from_slice(&nums);
        out.push(Weapon { name: name.trim().to_string(), set, v });
    }
    Ok(out)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_weapons_parse(text_: *const c_char) -> i32 {
    match parse_weapons(unsafe { text(text_) }.unwrap_or(WEAPONS_DEFAULT)) {
        Ok(t) => {
            let n = t.len() as i32;
            *WEAPONS.write().unwrap_or_else(|e| e.into_inner()) = t;
            n
        }
        Err(e) => fail(NM_E_ARG, e),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_weapons_load_info(path: *const c_char) -> i32 {
    let Some(p) = (unsafe { text(path) }) else { return fail(NM_E_ARG, "nm_weapons_load_info: no path") };
    let xml = match std::fs::read_to_string(p) {
        Ok(t) => t,
        Err(e) => return fail(NM_E_IO, format!("{p}: {e}")),
    };
    let read = delirium_data::weapons::read(&xml);
    let mut t = weapons().clone();
    for w in &read {
        let row = Weapon { name: w.name.to_lowercase(), set: nm::weapon_set(w, 3).to_string(),
            v: [w.force, w.damage, if w.slot == "SHOTGUN" { 16.0 } else { 1.0 }, w.between_shots, w.clip as f32, w.accuracy, w.range] };
        match t.iter_mut().find(|x| x.name == row.name) {
            Some(x) => *x = row,
            None => t.push(row),
        }
    }
    *WEAPONS.write().unwrap_or_else(|e| e.into_inner()) = t;
    read.len() as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_weapon_count() -> i32 {
    weapons().len() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_weapon_find(name: *const c_char) -> i32 {
    let n = unsafe { text(name) }.unwrap_or("");
    weapons().iter().position(|w| w.name.eq_ignore_ascii_case(n)).map_or_else(|| fail(NM_E_ARG, format!("no weapon {n}")), |i| i as i32)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_weapon_get(i: i32, name: *mut c_char, name_len: i32, set: *mut c_char, set_len: i32, v: *mut f32) -> i32 {
    let t = weapons();
    let Some(w) = usize::try_from(i).ok().and_then(|i| t.get(i)) else { return fail(NM_E_ARG, "no such weapon") };
    if !name.is_null() {
        unsafe { put(&w.name, name, name_len) };
    }
    if !set.is_null() {
        unsafe { put(&w.set, set, set_len) };
    }
    if !v.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(w.v.as_ptr(), v, 7) };
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_shoot_weapon(a: *mut NmAgent, w: *const NmWorld, weapon: i32, part: i32, at: *const f32, dir: *const f32, health: f32) -> f32 {
    let Some(wp) = usize::try_from(weapon).ok().and_then(|i| weapons().get(i).cloned()) else { return fail(NM_E_ARG, "no such weapon") as f32 };
    if at.is_null() || dir.is_null() {
        return fail(NM_E_ARG, "null vector") as f32;
    }
    let d = unsafe { v3(dir) }.normalize_or_zero() * wp.v[0];
    let r = unsafe { nm_agent_apply_push(a, at, d.to_array().as_ptr()) };
    if r < 0 {
        return r as f32;
    }
    let left = (health - wp.v[1]).max(0.0);
    let set = std::ffi::CString::new(wp.set).unwrap_or_default();
    let r = unsafe { nm_agent_task_shot(a, w, set.as_ptr(), part, at, dir, left) };
    if r < 0 { r as f32 } else { left }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_shotgun_fan(dir: *const f32, accuracy: f32, range: f32, pellets: i32, seed: *mut i32, out: *mut f32) -> i32 {
    if dir.is_null() || seed.is_null() || out.is_null() || pellets < 1 {
        return fail(NM_E_ARG, "nm_shotgun_fan: null dir, seed or out, or no pellets");
    }
    const PELLET_REACH: f32 = 0.4;
    let dir = unsafe { v3(dir) }.normalize_or_zero();
    let reach = PELLET_REACH * range;
    let side = dir.cross(Vec3::Z).normalize_or(Vec3::X);
    let (up, half) = (side.cross(dir), reach * accuracy);
    let mut r = delirium_data::rng::Rand::new(unsafe { *seed } as u32);
    (0..3).for_each(|_| _ = r.next());
    for k in 0..=pellets as usize {
        let end = if k == 0 {
            dir * range
        } else {
            let x = r.float_in_range(-half, half);
            let y = r.float_in_range(-half, half);
            dir * reach + side * x + up * y
        };
        unsafe { put3(out.add(k * 3), end.normalize_or_zero()) };
    }
    unsafe { *seed = r.seed as i32 };
    pellets + 1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_shotgun(a: *mut NmAgent, w: *const NmWorld, set: *const c_char, hits: *const f32, count: i32, health: f32, force: f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if hits.is_null() || count < 1 || !(force > 0.0 && force.is_finite()) {
        return fail(NM_E_ARG, "nm_agent_task_shotgun: no hits, or force not over 0");
    }
    let n = a.build.parts.len();
    let (l, t) = (a.u.len(), a.u.time());
    let mut all = vec![];
    for k in 0..count as usize {
        let h = unsafe { std::slice::from_raw_parts(hits.add(k * 7), 7) };
        let part = h[0] as i32;
        if !(0..n as i32).contains(&part) {
            return fail(NM_E_ARG, "nm_agent_task_shotgun: no such part");
        }
        let (at, d) = (Vec3::new(h[1], h[2], h[3]) / l, Vec3::new(h[4], h[5], h[6]).normalize_or_zero());
        all.push((part as usize, at, delirium::shottask::bullet(at, at - d * 3.0, force / t, false), -d));
    }
    for &(p, at, vel, _) in &all {
        let f = vel * a.agent.doll.share(p) * 60.0;
        a.agent.push.push((p, at, f));
    }
    let was_dead = a.dead;
    let health = if all.iter().any(|h| h.0 == 12) && HEADSHOT_KILLS.load(std::sync::atomic::Ordering::Relaxed) { 0.0 } else { health };
    a.dead |= health < 50.0;
    if was_dead {
        return 0;
    }
    let set = unsafe { text(set) }.unwrap_or("*");
    let hit = nm::Hit { health, ..nm::Hit::ragdoll(&a.agent) };
    let now = a.clock as u32;
    let mut rand = RAND.lock().unwrap_or_else(|e| e.into_inner());
    let (tk, msgs, next) = match a.shot.take() {
        Some((mut tk, _)) => {
            let msgs = with_tp(|tp| all.iter().flat_map(|&(p, at, vel, nn)| tk.next_hit(&mut rand, tp, &hit, now, set, p, at, vel, nn)).collect());
            (tk, msgs, true)
        }
        None => {
            let &(p, at, vel, nn) = all.last().unwrap();
            let mut tk = delirium::shottask::ShotTask::new(&hit, set, p, at, vel, nn);
            let msgs = with_tp(|tp| tk.update(&mut rand, tp, &hit, now).msgs);
            (tk, msgs, false)
        }
    };
    drop(rand);
    with_tp(|tp| nm::task_shot_msgs(&mut a.agent, &a.build, &w.0, tp, &hit, &tk, &msgs, next));
    a.shot = Some((tk, health));
    a.frame = a.clock;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_running(a: *const NmAgent) -> i32 {
    let Some(a) = (unsafe { a.as_ref() }) else { return fail(NM_E_ARG, "null agent") };
    let m = &a.more;
    (a.shot.is_some() || a.blast.is_some() || a.fire.is_some() || m.sit.is_some() || m.pose.is_some() || m.roll.is_some() || m.script.is_some()) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_check(a: *const NmAgent, buf: *mut c_char, len: i32) -> i32 {
    let a = agent_ref!(a);
    let ag = &a.agent;
    let tasks = a.shot.is_some() || a.blast.is_some() || a.fire.is_some();
    let lines: Vec<&str> = [
        (!ag.full(), "no behaviours: the agent was made before nm_unlock + nm_init (it runs plain stand-ins); make agents after them"),
        (TP.read().map_or(true, |t| t.is_none()), "no task params: call nm_load_task_params (taskparams.txt) or shots and tasks use rough defaults"),
        (a.mode == NM_MODE_FOLLOW, "FOLLOW mode: your engine's ragdoll only reacts if you apply nm_agent_get_part_drives' forces and torques to it every step"),
        (!(ag.balancing() || ag.catching() || ag.relaxing() || ag.rolled_up() || ag.high_falling() || tasks),
         "nothing running: start one (a task like nm_agent_task_shot, or bodyBalance start=true) when it's hit"),
        (!ag.doll.awake(), "asleep: a push, task or message wakes it"),
    ].into_iter().filter(|x| x.0).map(|x| x.1).collect();
    unsafe { put(&lines.join("\n"), buf, len) };
    lines.len() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_stop(a: *mut NmAgent) -> i32 {
    let a = agent!(a);
    (a.shot, a.blast, a.fire) = (None, None, None);
    let m = &mut a.more;
    (m.sit, m.pose, m.roll, m.script, m.jroll) = (None, None, None, None, None);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_balance(a: *mut NmAgent, w: *const NmWorld, set: *const c_char, look: *const f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let set = unsafe { text(set) }.unwrap_or("*");
    let look = (!look.is_null()).then(|| unsafe { v3(look) } / a.u.len());
    with_tp(|tp| nm::task_balance(&mut a.agent, &a.build, &w.0, tp, set, look, false));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_catch_fall(a: *mut NmAgent, set: *const c_char) -> i32 {
    let a = agent!(a);
    let set = unsafe { text(set) }.unwrap_or("*");
    with_tp(|tp| nm::task_catch_fall(&mut a.agent, tp, set));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_explosion(a: *mut NmAgent, w: *const NmWorld, at: *const f32, radius: f32, speed: f32) -> i32 {
    let a = agent!(a);
    if (unsafe { w.as_ref() }).is_none() || at.is_null() || !(radius > 0.0 && speed >= 0.0) {
        return fail(NM_E_ARG, "nm_agent_task_explosion: world, point, radius over 0");
    }
    let (at, radius, speed) = (unsafe { v3(at) } / a.u.len(), radius / a.u.len(), speed / a.u.time());
    let parts: Vec<Vec3> = a.agent.doll.frames().map(|f| f.translation).collect();
    for (k, p) in parts.into_iter().enumerate() {
        let f = 1.0 - p.distance(at) / radius;
        if f <= 0.0 || a.agent.doll.gone >> k & 1 != 0 {
            continue;
        }
        let imp = ((p - at).normalize_or(Vec3::Z) + Vec3::Z * 0.5).normalize() * a.agent.doll.part_mass(k) * speed * f;
        a.agent.doll.push(&a.build, p, imp);
    }
    a.shot = None;
    a.blast = Some((Default::default(), a.clock + 6000.0));
    a.frame = a.clock - 1000.0;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_on_fire(a: *mut NmAgent, w: *const NmWorld, health: f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if let Some(f) = a.fire.as_mut() {
        (f.1, f.2) = (true, health);
        return 0;
    }
    let mut f = nm::OnFire::new("*");
    if FIRE_REACTION.load(std::sync::atomic::Ordering::Relaxed) {
        with_tp(|tp| f.start(&mut a.agent, &a.build, &w.0, tp, &mut RAND.lock().unwrap_or_else(|e| e.into_inner())));
    }
    a.fire = Some((f, true, health));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_tase(a: *mut NmAgent, w: *const NmWorld, seconds: f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if !seconds.is_finite() || seconds < 0.0 {
        return fail(NM_E_ARG, "nm_agent_tase: seconds 0 or more");
    }
    let on = seconds > 0.0;
    if on == (a.shock > 0.0) {
        a.shock = seconds;
        return 0;
    }
    a.shock = seconds;
    let ps: &[(&str, Value)] = if on { &[("start", Value::Bool(true)), ("stunMag", Value::Float(0.4)), ("largeMult", Value::Float(2.0))] } else { &[("start", Value::Bool(false))] };
    let mut st = std::mem::take(&mut a.agent.msgs);
    let r = msg::send(&mut a.agent, &a.build, &w.0, &mut st, "electrocute", ps);
    a.agent.msgs = st;
    if r.is_err() { fail(NM_E_MESSAGE, "electrocute") } else { 0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_tased(a: *const NmAgent) -> i32 {
    (agent_ref!(a).shock > 0.0) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_on_fire(a: *mut NmAgent, burning: i32, health: f32) -> i32 {
    let a = agent!(a);
    let Some(f) = a.fire.as_mut() else { return fail(NM_E_STATE, "no on-fire task") };
    (f.1, f.2) = (burning != 0, health);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_melee_hit(a: *mut NmAgent, w: *const NmWorld, at: *const f32, impulse: *const f32, head: *const f32, passive: i32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if at.is_null() || impulse.is_null() || head.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    let u = a.u;
    let (at, imp, head) = unsafe { (v3(at) / u.len(), v3(impulse) / u.impulse(), v3(head) / u.len()) };
    (a.shot, a.blast) = (None, None);
    a.agent.doll.push(&a.build, at, imp);
    let mut rand = RAND.lock().unwrap_or_else(|e| e.into_inner());
    with_tp(|tp| if rand.float_in_range(0.0, 1.0) > 0.7 {
        nm::task_balance(&mut a.agent, &a.build, &w.0, tp, "*", Some(head), false)
    } else {
        nm::task_flinch(&mut a.agent, &a.build, &w.0, tp, &mut rand, if passive != 0 { "MELEE_PASIVE" } else { "MELEE" }, head)
    });
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_high_fall(a: *mut NmAgent, w: *const NmWorld, set: *const c_char, player: i32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let set = unsafe { text(set) }.unwrap_or("*");
    if a.more.xr & NM_XR_SMART_FALL != 0 {
        send(a, w, "smartFall", &[("start", Value::Bool(true))]);
        return 0;
    }
    with_tp(|tp| nm::task_high_fall(&mut a.agent, &a.build, &w.0, tp, set, player != 0));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_fall_down(a: *mut NmAgent, w: *const NmWorld, heading: f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let r = if heading < 0.0 { RAND.lock().unwrap_or_else(|e| e.into_inner()).float_in_range(0.0, 1.0) } else { heading.fract() };
    let Some((state, dir, height)) = nm::fall_down_try(&w.0, a.agent.doll.root(), r) else { return 0 };
    (a.shot, a.blast) = (None, None);
    a.fall = Some(nm::FallDown::start(&mut a.agent, &a.build, &w.0, state, dir, height));
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_grab(a: *mut NmAgent, w: *const NmWorld, left: *const f32, right: *const f32, hold: f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if left.is_null() && right.is_null() {
        return fail(NM_E_ARG, "nm_agent_task_grab: no point");
    }
    let l = a.u.len();
    let at = |p: *const f32| (!p.is_null()).then(|| unsafe { v3(p) } / l);
    let normal = |p: *const f32| (!p.is_null()).then(|| unsafe { v3(p.add(3)) }.normalize_or_zero());
    let mut g = delirium::grabhelper::GrabHelper::new();
    g.set_points(None, [at(right), normal(left), at(left), normal(right), None, None]);
    g.block = with_tp(delirium::grabhelper::read_block);
    (g.strength, g.find_left, g.hold_left) = (g.block[0], 1.0, hold.max(0.0));
    let look = at(left).or(at(right));
    (a.shot, a.blast, a.fall) = (None, None, None);
    with_tp(|tp| nm::task_balance(&mut a.agent, &a.build, &w.0, tp, "*", look, false));
    if let Some((n, mut p)) = g.start() {
        for (k, v) in p.iter_mut() {
            match *k {
                "useLeft" => *v = delirium::msg::Value::Bool(!left.is_null()),
                "useRight" => *v = delirium::msg::Value::Bool(!right.is_null()),
                _ => {}
            }
        }
        nm::send_task_msgs(&mut a.agent, &a.build, &w.0, &[(n, p)]);
    }
    a.grab = Some(g);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_jump_roll(a: *mut NmAgent, w: *const NmWorld) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    with_tp(|tp| nm::task_jump_roll(&mut a.agent, &a.build, &w.0, tp));
    let vel = a.agent.doll.vel(0).0;
    a.more.jroll = Some((a.clock + with_tp(nm::jump_roll_up_time) as f64 * 1000.0, vel));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_reactions(a: *mut NmAgent, bits: i32) -> i32 {
    agent!(a).more.xr = bits;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_dive(a: *mut NmAgent, gap: f32) -> i32 {
    agent!(a).more.dive_gap = gap.max(0.0);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_jump(a: *mut NmAgent, dive: i32, dir: *const f32) -> i32 {
    let a = agent!(a);
    let toward = (!dir.is_null()).then(|| unsafe { v3(dir) }.with_z(0.0).normalize_or_zero()).filter(|d| *d != Vec3::ZERO);
    a.more.jump = delirium::jump::Jump::start(&mut a.agent, dive != 0, toward);
    a.more.jump.is_some() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_jumping(a: *const NmAgent) -> i32 {
    agent_ref!(a).more.jump.is_some() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_near_miss(a: *mut NmAgent, w: *const NmWorld, from: *const f32, dir: *const f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if from.is_null() || dir.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    let (from, dir) = (unsafe { v3(from) } / a.u.len(), unsafe { v3(dir) }.normalize_or_zero());
    let f: Vec<Transform> = a.agent.doll.frames().collect();
    let by = |p: Vec3| from + dir * (p - from).dot(dir).max(0.0);
    let (h, c) = (by(f[HEAD].translation), by(f[CHEST].translation));
    let (dh, dc) = (h.distance(f[HEAD].translation), c.distance(f[CHEST].translation));
    let (p, d) = if dh < dc { (h, dh) } else { (c, dc) };
    if d > WHIZZ {
        return 0;
    }
    let (now, gap) = (a.clock, a.more.dive_gap as f64 * 1000.0);
    let b = |v: bool| Value::Bool(v);
    let look = [("start", b(true)), ("pos", Value::Vec3(p)), ("alwaysLook", b(true))];
    let since = a.more.xs.near_at.map_or(f64::INFINITY, |t| now - t);
    if since >= gap {
        a.more.xs.dived = false;
    }
    if a.dead || !a.agent.balancing() {
        if !a.dead && a.more.xr & NM_XR_CURL_SHOT != 0 {
            send(a, w, "bodyFoetal", &[("start", b(true))]);
            a.still = 0.0;
            send(a, w, "headLook", &look);
        }
        return 1;
    }
    if since >= gap || since < 0.0 {
        a.more.xs.near_at = Some(now);
    }
    if gap > 0.0 && (NEAR_BURST as f64 * 1000.0..gap).contains(&since) && !a.more.xs.dived {
        (a.more.xs.near_at, a.more.xs.dived) = (Some(now), true);
        let chest = f[CHEST].translation;
        let off = (chest - by(chest)).with_z(0.0);
        let away = if off.length() > 0.05 { off.normalize() } else { dir.cross(Vec3::Z).normalize_or(Vec3::X) };
        if a.more.jump.is_none() {
            a.more.jump = delirium::jump::Jump::start(&mut a.agent, true, Some(away));
        }
        if a.more.jump.is_none() {
            a.agent.doll.kick(away * DIVE.0 + Vec3::Z * DIVE.1);
            send(a, w, "shootDodge", &[("start", b(true)), ("rightArmTarget", Value::Vec3(from)), ("leftArmTarget", Value::Vec3(from))]);
        }
        return 1;
    }
    send(a, w, "bodyBalance", &[("start", b(true))]);
    send(a, w, "upperBodyFlinch", &[("start", b(true)), ("pos", Value::Vec3(p)), ("newHit", b(true))]);
    send(a, w, "headLook", &look);
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_thrown_at(a: *mut NmAgent, w: *const NmWorld, at: *const f32, vel: *const f32, look: i32, avoid: i32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if at.is_null() || vel.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    if a.dead {
        return 0;
    }
    let (p, v) = (unsafe { v3(at) } / a.u.len(), unsafe { v3(vel) } / a.u.len() * a.u.time());
    if look != 0 {
        send(a, w, "headLook", &[("start", Value::Bool(true)), ("pos", Value::Vec3(p)), ("vel", Value::Vec3(v)), ("instanceIndex", Value::Int(-1)), ("alwaysLook", Value::Bool(true))]);
    }
    if avoid != 0 {
        send(a, w, "upperBodyFlinch", &[("start", Value::Bool(true)), ("pos", Value::Vec3(p)), ("newHit", Value::Bool(true))]);
        (a.more.xs.dodging, a.more.xs.look_stop) = (true, Some(a.clock + 2000.0));
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_muscle_rise(a: *mut NmAgent, w: *const NmWorld, on: i32, help: f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if on == 0 {
        if a.more.musc.take().is_some() {
            send(a, w, "bodyBalance", &[("start", Value::Bool(true))]);
        }
        return 0;
    }
    nm::task_pose(&mut a.agent, &a.build, &w.0, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), 11.0, 12.0);
    a.more.musc = Some(if help < 0.0 { 0.5 } else { help });
    a.still = 0.0;
    0
}

fn net(path: *const c_char) -> Result<std::sync::Arc<delirium::learned::Net>, i32> {
    static NETS: std::sync::Mutex<Vec<(String, std::sync::Arc<delirium::learned::Net>)>> = std::sync::Mutex::new(Vec::new());
    let Some(p) = (unsafe { text(path) }) else { return Err(fail(NM_E_ARG, "no network path")) };
    let mut nets = NETS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(n) = nets.iter().find(|n| n.0 == p) {
        return Ok(n.1.clone());
    }
    let n = std::sync::Arc::new(delirium::learned::Net::load(p).ok_or_else(|| fail(NM_E_IO, format!("{p}: not a network")))?);
    nets.push((p.to_string(), n.clone()));
    Ok(n)
}

fn fresh_agent(a: &mut NmAgent) {
    let fr = frames(a);
    a.agent = delirium::learned::agent(&a.build, &fr);
    let tune = std::mem::take(&mut a.more.tune);
    tuned(a, &tune);
    a.more.tune = tune;
}

fn learned_done(a: &mut NmAgent, w: &NmWorld) {
    a.more.learned = None;
    let up = delirium::learned::standing(&a.agent.doll);
    fresh_agent(a);
    if up {
        send(a, w, "bodyBalance", &[("start", Value::Bool(true))]);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_learned_getup(a: *mut NmAgent, net_path: *const c_char) -> i32 {
    let a = agent!(a);
    let n = match net(net_path) {
        Ok(n) => n,
        Err(e) => return e,
    };
    if a.mode == NM_MODE_FOLLOW {
        return fail(NM_E_STATE, "nm_agent_learned_getup: NM_MODE_LEAD only (the network steps the body itself)");
    }
    (a.shot, a.blast, a.fire, a.fall) = (None, None, None, None);
    fresh_agent(a);
    a.more.learned = Some((n, Default::default(), 0));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_learned_getup_running(a: *const NmAgent) -> i32 {
    agent_ref!(a).more.learned.is_some() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_learned_walk(a: *mut NmAgent, w: *const NmWorld, net_path: *const c_char, cycle: f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if net_path.is_null() {
        if a.more.walk.take().is_some() {
            a.agent.configure_balance(Default::default());
            a.agent.muscle_stiffness_x(1.0);
            send(a, w, "bodyBalance", &[("start", Value::Bool(true))]);
        }
        return 0;
    }
    if !(cycle > 0.0) {
        return fail(NM_E_ARG, "nm_agent_learned_walk: cycle over 0");
    }
    let n = match net(net_path) {
        Ok(n) => n,
        Err(e) => return e,
    };
    if n.sizes != delirium::learned::walk_sizes(&a.agent.doll) {
        return fail(NM_E_ARG, "nm_agent_learned_walk: not a walk network for this body");
    }
    fresh_agent(a);
    if let Err(e) = delirium::learned::start_walk_balance(&mut a.agent, &a.build, &w.0) {
        return fail(NM_E_STATE, e);
    }
    a.more.walk = Some((n, 0.0, cycle / a.u.time()));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_sit(a: *mut NmAgent, w: *const NmWorld, look: *const f32, seat: *const f32) -> i32 {
    let a = agent!(a);
    if w.is_null() {
        return fail(NM_E_ARG, "null world");
    }
    let l = a.u.len();
    let at = |p: *const f32| (!p.is_null()).then(|| unsafe { v3(p) } / l);
    let here = a.agent.doll.frames().next().unwrap_or_default().translation;
    a.more.sit = Some(delirium::nmsit::Sit::new(at(look).unwrap_or(Vec3::ZERO), at(seat).unwrap_or(here), !a.agent.incoming.is_empty()));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_pose(a: *mut NmAgent, w: *const NmWorld, low: f32, high: f32) -> i32 {
    let a = agent!(a);
    if w.is_null() {
        return fail(NM_E_ARG, "null world");
    }
    a.more.pose = Some(delirium::nmpose::Pose::new(low, high, 0, 0, false));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_roll_up_relax(a: *mut NmAgent, w: *const NmWorld, stiffness: f32, damping: f32, roll_ms: i32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    (a.shot, a.blast, a.fall) = (None, None, None);
    let r = with_tp(|tp| nm::RollUpAndRelax::start(&mut a.agent, &a.build, &w.0, tp, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), stiffness, damping, roll_ms.max(0) as u32));
    a.more.roll = Some(r);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_script(a: *mut NmAgent, ms: i32) -> i32 {
    let a = agent!(a);
    a.more.script = Some((delirium::nmscript::Script::new(ms.max(0) as u32, false), 0));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_attacker(a: *mut NmAgent, bones: *const f32, armed: i32) -> i32 {
    let a = agent!(a);
    let old = a.more.attacker.take().and_then(|x| x.1);
    if bones.is_null() || armed == 0 {
        if let Some(g) = old {
            g.stop(&mut a.agent);
        }
        if bones.is_null() {
            return 0;
        }
    }
    let l = a.u.len();
    let b: [Vec3; 4] = std::array::from_fn(|k| unsafe { v3(bones.add(k * 3)) } / l);
    let gun = if armed == 0 {
        None
    } else {
        old.or_else(|| {
            let t = with_tp(|tp| delirium::brace::Set::read(tp, "*")).gun;
            Some(nm::PointGun::draw(&mut RAND.lock().unwrap_or_else(|e| e.into_inner()), true, t, false))
        })
    };
    a.more.attacker = Some((b, gun));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_gun_ready(a: *mut NmAgent) -> i32 {
    let a = agent!(a);
    let Some((_, Some(g))) = a.more.attacker.as_mut() else { return 0 };
    if !g.ready(200.0) {
        return 0;
    }
    let t = with_tp(|tp| delirium::brace::Set::read(tp, "*")).gun;
    g.next = RAND.lock().unwrap_or_else(|e| e.into_inner()).int_in_range(t[2], t[3]) as f32 * 1e-3;
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_stick(a: *mut NmAgent, on: i32, x: f32, y: f32, camera: f32) -> i32 {
    let a = agent!(a);
    let k = |v: f32| (v.clamp(-1.0, 1.0) * 127.0) as i32;
    let wander = a.more.stick.take().map(|s| s.2).unwrap_or_default();
    a.more.stick = (on != 0).then(|| ((k(x), -k(y)), camera, wander));
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_relax(a: *mut NmAgent, stiffness: f32, damping: f32) -> i32 {
    let a = agent!(a);
    (a.shot, a.blast, a.fall, a.grab, a.door, a.tackle) = (None, None, None, None, None, None);
    nm::task_relax(&mut a.agent, stiffness, damping);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_brace(a: *mut NmAgent, w: *const NmWorld, set: *const c_char) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let set = unsafe { text(set) }.unwrap_or("*");
    with_tp(|tp| nm::task_brace(&mut a.agent, &a.build, &w.0, tp, set, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), false));
    if a.more.xr & NM_XR_ARMS_OUT != 0 {
        send(a, w, "braceForImpact", &[("braceDistance", Value::Float(1.3)), ("targetPredictionTime", Value::Float(0.45))]);
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_brace_control(a: *mut NmAgent, w: *const NmWorld, car: *const f32, hit: *const f32, look: *const f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if car.is_null() {
        return fail(NM_E_ARG, "null car matrix");
    }
    let s = a.u.len();
    let mut m: [Vec3; 4] = std::array::from_fn(|i| unsafe { v3(car.add(i * 3)) });
    m[3] /= s;
    let hit = (!hit.is_null()).then(|| unsafe { v3(hit) } / s);
    let look = if look.is_null() { m[3] } else { unsafe { v3(look) / s } };
    nm::task_brace_control(&mut a.agent, &a.build, &w.0, m, hit, m[3], look, 0);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_car_hit(a: *mut NmAgent, w: *const NmWorld, health: f32, agility: i32, player_driver: i32, damage: *mut f32, hit_box: *mut i32) -> i32 {
    use delirium::bump::{self, Kind, Task, Thing};
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let Some((k, sum, at, _)) = a.car.0.filter(|h| h.3 >= 8.0 / 60.0 - 1e-4) else { return 0 };
    a.car = (None, 0.0);
    let d = &a.agent.doll;
    let kind = if w.4.len() == w.1.len() { w.4.get(k as usize).copied() } else { None };
    let player_driver = kind.map_or(player_driver != 0, |k| k & NM_HIT_PLAYER != 0);
    let train = |o: &Obstacle| bump::Train { axes: [o.rot * Vec3::X, o.rot * Vec3::Y, o.rot * Vec3::Z], place: o.centre, vel: o.vel, size: Some(o.half * 2.0) };
    let other = Thing { id: k, kind: match kind.map(|k| k & !NM_HIT_PLAYER) {
        Some(NM_HIT_OBJECT) => Kind::Object,
        Some(NM_HIT_TRAIN) => Kind::Vehicle { player_driver, train: w.1.get(k as usize).map(train) },
        _ => Kind::Vehicle { player_driver, train: None },
    } };
    let imp = bump::Impact { impulse: sum.length(), dt: 1.0 / 60.0, component: d.nearest(&a.build, at) as i32, other: Some(other) };
    let ped = bump::Ped { id: -1, health, inv_mass: 1.0 / d.mass().max(1.0), woke: false, drop: None, vz: d.vel(0).0.z, tough: false,
                          player_cut: false, player: false, mission: false, quick_relax: false, agility: agility.clamp(0, 255) as u8,
                          gun: false, place: d.frames().next().map_or(at, |f| f.translation), vel: Vec3::ZERO, standing_on: None };
    let mut rand = RAND.lock().unwrap_or_else(|e| e.into_inner());
    let Some((amount, _)) = bump::damage(&ped, &imp, &mut rand).filter(|d| d.0 >= 1.0) else { return 0 };
    let place = w.1.get(k as usize).map_or(at, |o| o.centre);
    let around = bump::Around { touching: vec![other], player: player_driver.then_some(bump::Player { thing: other, place, in_car: true }),
                                blend: None, sit: None, die: None, accepts: true };
    let task = bump::react(&ped, &imp, &around, amount, false, (health - amount).max(1.0), &mut rand, || None).map(|m| m.task);
    (a.shot, a.blast, a.fall, a.grab, a.door, a.tackle) = (None, None, None, None, None, None);
    match task {
        Some(Task::Brace { .. }) => {
            with_tp(|tp| nm::task_brace(&mut a.agent, &a.build, &w.0, tp, "*", &mut rand, false));
            if let Some(o) = w.1.get(k as usize) {
                let m = [o.rot * Vec3::X, o.rot * Vec3::Y, o.rot * Vec3::Z, o.centre];
                nm::task_brace_control(&mut a.agent, &a.build, &w.0, m, Some(at), o.centre, o.centre, k);
            }
        }
        Some(Task::RollUp { .. }) => send(a, w, "bodyRollUp", &[("start", Value::Bool(true))]),
        Some(Task::Balance { set, .. }) => with_tp(|tp| nm::task_balance(&mut a.agent, &a.build, &w.0, tp, set, Some(place), false)),
        Some(Task::Relax { a: st, b: dm }) => nm::task_relax(&mut a.agent, st, dm),
        Some(_) => nm::task_relax(&mut a.agent, -1.0, -1.0),
        None => {}
    }
    unsafe {
        if !damage.is_null() {
            *damage = amount;
        }
        if !hit_box.is_null() {
            *hit_box = k;
        }
    }
    1
}

const DOOR_HOLD: f32 = 6.0;
const DOOR_LET_GO: f32 = 12.0;
const DOOR_MOVING: f32 = 0.5;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_car_door(a: *mut NmAgent, w: *const NmWorld, hit_box: i32, at: *const f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let Some(o) = w.1.get(hit_box.max(0) as usize).filter(|_| hit_box >= 0) else { return fail(NM_E_ARG, "nm_agent_task_car_door: no such box") };
    let door = if at.is_null() {
        let f: Vec<Vec3> = a.agent.doll.frames().map(|f| f.translation).collect();
        let grip = f[10].with_z(f[0].z - 0.15);
        let local = (o.rot.inverse() * (grip - o.centre)).clamp(-o.half, o.half);
        let k = (local.abs() / o.half.max(Vec3::splat(1e-3))).with_z(0.0).max_position();
        let mut n = Vec3::ZERO;
        n[k] = local[k].signum();
        o.centre + o.rot * local + o.rot * n * 0.12
    } else {
        unsafe { v3(at) / a.u.len() }
    };
    (a.shot, a.blast, a.fall, a.grab) = (None, None, None, None);
    send(a, w, "armHang", &[("start", Value::Bool(true)), ("constraintPosition", Value::Vec3(door)), ("instanceIndex", Value::Int(hit_box))]);
    a.door = Some((hit_box, 0.0, 0.0));
    0
}

const STAIRS_STEPS: u8 = 2;

fn stairs(a: &mut NmAgent, w: &NmWorld) {
    let (com, v) = a.agent.doll.com();
    let h = v.with_z(0.0);
    let under = w.0.ground(com, 3.0);
    let down = h.length() > 0.5 && matches!((under, w.0.ground(com + h.normalize() * 0.5, 3.0)), (Some(g0), Some(g1)) if g0 - g1 > 0.1);
    let balancing = a.agent.balancing();
    let (last, steps, rolled) = a.stairs;
    let steps = match (down, last, under) {
        (true, Some(l), Some(g)) if l - g > 0.1 => steps + 1,
        (true, _, _) => steps,
        _ => 0,
    };
    let last = if down { last.filter(|&l| under.is_some_and(|g| l - g <= 0.1)).or(under) } else { None };
    let go = !rolled && !a.dead && down && (!balancing || steps >= STAIRS_STEPS);
    if go {
        send(a, w, "rollDownStairs", &[("start", Value::Bool(true))]);
    }
    a.stairs = (last, steps, (rolled || go) && !(balancing && !go));
}

const TK_COMMIT: f32 = 4.0;
const TK_LAUNCH: f32 = 5.5;
const TK_LIFT: f32 = 1.6;
const TK_TIP: f32 = 2.0;
const TK_REACH: f32 = 0.3;
const TK_SEEK: f32 = 0.06;
const TK_GRIP: f32 = 0.3;
const TK_ARM_MASS: f32 = 8.0;
const TK_GRIP_MAX: f32 = 2500.0;
const TK_IMPACT: f32 = 0.1;
const TK_DRIVE: f32 = 500.0;
const TK_PULL: f32 = 900.0;
const TK_PULL_TIME: f32 = 1.0;
const TK_WHIFF: f32 = 1.0;
const TK_HOLD: f32 = 2.0;
const TK_HANDS: [usize; 2] = [16, 20];
const TK_CHEST: usize = 9;
const TK_THIGHS: [usize; 2] = [1, 4];

unsafe impl Send for NmAgent {}

pub struct Tackle {
    target: *mut NmAgent,
    keep: bool,
    t: f32,
    dir: Vec3,
    grips: [(usize, Vec3, Option<f32>); 2],
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_tackle(a: *mut NmAgent, w: *const NmWorld, target: *mut NmAgent, low: i32, keep: i32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if std::ptr::eq(a, target) {
        return fail(NM_E_ARG, "nm_agent_task_tackle: a body can't tackle itself");
    }
    let Some(t) = (unsafe { target.as_ref() }) else { return fail(NM_E_ARG, "null target") };
    if a.mode == NM_MODE_FOLLOW || t.mode == NM_MODE_FOLLOW {
        return fail(NM_E_STATE, "nm_agent_task_tackle: LEAD bodies only");
    }
    let them: Vec<Transform> = t.agent.doll.frames().collect();
    let me: Vec<Transform> = a.agent.doll.frames().collect();
    let c = me[0].translation;
    let aim = them[if low != 0 { TK_THIGHS[0] } else { TK_CHEST }].translation;
    let gap = (aim - c).with_z(0.0).length();
    if gap > TK_COMMIT {
        return fail(NM_E_STATE, "nm_agent_task_tackle: the target's over 4 m off");
    }
    let dir = (aim + t.agent.doll.com().1 * gap / TK_LAUNCH - c).with_z(0.0).normalize_or(Vec3::X);
    let spin = Vec3::Z.cross(dir) * TK_TIP;
    let was = a.agent.doll.com().1.with_z(0.0) * 0.5;
    for (k, f) in me.iter().enumerate() {
        a.agent.doll.set_body(k, f.translation, f.rotation, was + dir * TK_LAUNCH + Vec3::Z * TK_LIFT + spin.cross(f.translation - c), spin);
    }
    let side = dir.cross(Vec3::Z);
    let on = |k: usize, at: Vec3| (k, them[k].rotation.inverse() * (at - them[k].translation), None);
    let grips = if low != 0 {
        let near = |p: Vec3| if them[TK_THIGHS[0]].translation.distance(p) < them[TK_THIGHS[1]].translation.distance(p) { TK_THIGHS[0] } else { TK_THIGHS[1] };
        [-1.0, 1.0].map(|s: f32| { let k = near(c - side * s); on(k, them[k].translation) })
    } else {
        [-1.0, 1.0].map(|s: f32| on(TK_CHEST, them[TK_CHEST].translation - side * s * 0.15))
    };
    (a.shot, a.blast, a.fall, a.grab, a.door) = (None, None, None, None, None);
    send(a, w, "catchFall", &[("start", Value::Bool(true))]);
    a.tackle = Some(Tackle { target, keep: keep != 0, t: 0.0, dir, grips });
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_tackle_state(a: *const NmAgent) -> i32 {
    let a = agent_ref!(a);
    a.tackle.as_ref().map_or(0, |t| if t.grips.iter().any(|g| g.2.is_some()) { 1 } else { 2 })
}

fn tackle(a: &mut NmAgent, dt: f32) {
    let Some(mut tk) = a.tackle.take() else { return };
    let Some(t) = (unsafe { tk.target.as_mut() }) else { return };
    tk.t += dt;
    a.still = 0.0;
    let held = tk.grips.iter().filter_map(|g| g.2).reduce(f32::min);
    if held.is_none() && tk.t > TK_WHIFF || held.is_some_and(|s| !tk.keep && tk.t - s > TK_HOLD) {
        return;
    }
    let (hero, them) = (&mut a.agent.doll, &mut t.agent.doll);
    for (i, g) in tk.grips.iter_mut().enumerate() {
        let (k, local, _) = *g;
        let f = them.frames().nth(k).unwrap_or_default();
        let at = f.translation + f.rotation * local;
        let hand = hero.frames().nth(TK_HANDS[i]).unwrap_or_default().translation;
        let gap = at - hand;
        if g.2.is_none() && gap.length() < TK_REACH {
            g.2 = Some(tk.t);
        }
        let rel = speed_at(them, k, at) - speed_at(hero, TK_HANDS[i], hand);
        let m = 1.0 / (1.0 / TK_ARM_MASS + 1.0 / them.part_mass(k));
        let j = if g.2.is_some() { (gap * TK_GRIP / dt + rel) * m } else { gap.normalize_or_zero() * TK_SEEK / dt * m };
        if g.2.is_some_and(|s| tk.t - s > TK_IMPACT) && j.length() > TK_GRIP_MAX * dt {
            g.2 = None;
            continue;
        }
        hero.impulse(TK_HANDS[i], hand, j);
        them.impulse(k, at, -j);
    }
    if held.is_some_and(|s| tk.t - s < TK_PULL_TIME) {
        let chest = hero.frames().nth(TK_CHEST).unwrap_or_default().translation;
        hero.impulse(TK_CHEST, chest, tk.dir * TK_DRIVE * dt);
        for &(k, local, since) in &tk.grips {
            if since.is_some() {
                let f = them.frames().nth(k).unwrap_or_default();
                them.impulse(k, f.translation + f.rotation * local, (tk.dir * 0.6 - Vec3::Z) * TK_PULL * 0.5 * dt);
            }
        }
    }
    a.tackle = Some(tk);
}

fn speed_at(doll: &Ragdoll, k: usize, at: Vec3) -> Vec3 {
    let (v, w) = doll.vel(k);
    v + w.cross(at - doll.centre(k))
}

const WET_IN: f32 = 0.4;
const WET_OUT: f32 = 0.9;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_water(a: *mut NmAgent, w: *const NmWorld, level: f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    a.water.0 = level.is_finite().then(|| level / a.u.len());
    if a.water.0.is_none() && std::mem::take(&mut a.water.1) {
        send(a, w, "buoyancy", &[("start", Value::Bool(false))]);
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_set_strength(a: *mut NmAgent, k: f32) -> i32 {
    let a = agent!(a);
    if !(0.1..=10.0).contains(&k) {
        return fail(NM_E_ARG, "nm_agent_set_strength: k within 0.1-10");
    }
    a.agent.strength = k;
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_in_water(a: *const NmAgent) -> i32 {
    agent_ref!(a).water.1 as i32
}

fn water(a: &mut NmAgent, w: &NmWorld) {
    let Some(level) = a.water.0 else { return };
    let z = a.agent.doll.frames().next().unwrap_or_default().translation.z - level;
    let wet = z < if a.water.1 { WET_OUT } else { WET_IN };
    if wet != a.water.1 {
        a.water.1 = wet;
        if wet {
            send(a, w, "buoyancy", &[("start", Value::Bool(true)), ("surfacePoint", Value::Vec3(Vec3::new(1.0, 0.0, level)))]);
        } else {
            send(a, w, "buoyancy", &[("start", Value::Bool(false))]);
        }
    }
}

const CATCH_FIRE: f32 = 0.5;

fn catch_fire(a: &mut NmAgent, w: &NmWorld, dt: f32) {
    if a.fire.as_ref().is_some_and(|f| f.1) || a.mode == NM_MODE_FOLLOW {
        a.singe = 0.0;
        return;
    }
    let me = a as *const NmAgent as usize;
    let mine: Vec<(Vec3, f32)> = a.agent.doll.capsules().zip(&a.build.parts).flat_map(|(t, p)| {
        let d = t.rotation * Vec3::Y * p.half;
        [t.translation - d, t.translation, t.translation + d].map(|x| (x, p.radius))
    }).collect();
    let touching = w.3.lock().unwrap_or_else(|e| e.into_inner()).1.iter().filter(|e| e.0 != me && e.3).any(|e| {
        e.2.iter().any(|c| [c.0, (c.0 + c.1) * 0.5, c.1].iter().any(|x| mine.iter().any(|(y, r)| x.distance(*y) < c.2 + r + 0.05)))
    });
    a.singe = if touching { a.singe + dt } else { 0.0 };
    if a.singe >= CATCH_FIRE {
        a.singe = 0.0;
        unsafe { nm_agent_task_on_fire(a, w, 200.0) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_is_burning(a: *const NmAgent) -> i32 {
    agent_ref!(a).fire.as_ref().is_some_and(|f| f.1) as i32
}

fn car_door(a: &mut NmAgent, w: &NmWorld, dt: f32) {
    let Some((k, t, moving)) = a.door else { return };
    let speed = w.1.get(k as usize).map_or(0.0, |o| o.vel.length());
    let (t2, m2) = (t + dt, if speed > DOOR_MOVING || moving > 0.0 { moving + dt } else { 0.0 });
    let stop = |a: &mut NmAgent, w: &NmWorld, names: &[&str]| names.iter().for_each(|n| send(a, w, n, &[("start", Value::Bool(false))]));
    if t < 0.1 && t2 >= 0.1 {
        stop(a, w, &["pedalLegs"]);
        send(a, w, "bodyBalance", &[("start", Value::Bool(true))]);
    }
    if moving == 0.0 && m2 > 0.0 {
        stop(a, w, &["bodyBalance"]);
        send(a, w, "pedalLegs", &[("start", Value::Bool(true))]);
        send(a, w, "bodyWrithe", &[("start", Value::Bool(true)), ("mask", Value::Str("uk".into()))]);
    }
    a.door = Some((k, t2, m2));
    if m2 > 0.0 && (speed > DOOR_LET_GO || m2 > DOOR_HOLD) || w.1.get(k as usize).is_none() {
        stop(a, w, &["armHang", "pedalLegs", "bodyWrithe"]);
        send(a, w, "smartFall", &[("start", Value::Bool(true))]);
        a.door = None;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_flinch(a: *mut NmAgent, w: *const NmWorld, set: *const c_char, from: *const f32) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    if from.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    let set = unsafe { text(set) }.unwrap_or("*");
    with_tp(|tp| nm::task_flinch(&mut a.agent, &a.build, &w.0, tp, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), set, unsafe { v3(from) } / a.u.len()));
    0
}

