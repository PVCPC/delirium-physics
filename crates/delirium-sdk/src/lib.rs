#![allow(clippy::missing_safety_doc)]

pub mod character;
pub mod rig;

use delirium_data::taskparams::TaskParams;
use delirium::collision::Collision;
use delirium::msg::{self, Kind, Value};
use delirium::nm::{self, Agent};
use delirium::prelude::*;
use delirium::ragdoll::{Build, Obstacle, Ragdoll};
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

pub const NM_MODE_LEAD: i32 = 0;
pub const NM_MODE_FOLLOW: i32 = 1;

thread_local! {
    static ERR: RefCell<String> = const { RefCell::new(String::new()) };
}
static TP: RwLock<Option<TaskParams>> = RwLock::new(None);
static HEADSHOT_KILLS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

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
pub unsafe extern "C" fn nm_last_error(buf: *mut c_char, len: i32) -> i32 {
    ERR.with(|e| unsafe { put(&e.borrow(), buf, len) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_init_memory(xrt: *const u8, len: i32) -> i32 {
    if xrt.is_null() || len <= 0 {
        return fail(NM_E_ARG, "nm_init_memory: no data");
    }
    let d = unsafe { std::slice::from_raw_parts(xrt, len as usize) };
    let Ok(asset) = std::panic::catch_unwind(|| delirium_data::nm_asset::read(d)) else {
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

pub struct NmWorld(Collision, Vec<Obstacle>, Units, std::sync::Mutex<Bodies>);

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
    Box::into_raw(Box::new(NmWorld(Collision::default(), vec![], Units(1.0), Default::default())))
}

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
    let w = NmWorld(Collision::default(), vec![], a.u, Default::default());
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
    fall: Option<nm::FallDown>,
    grab: Option<delirium::grabhelper::GrabHelper>,
    car: (Option<(i32, Vec3, Vec3, f32)>, f32),
    door: Option<(i32, f32, f32)>,
    bank: f32,
    stairs: (Option<f32>, u8, bool),
    cuts: Vec<(Build, Ragdoll, Vec<delirium::ragdoll::Rigid>, Vec<usize>)>,
    tackle: Option<Tackle>,
    water: (Option<f32>, bool),
    clock: f64,
    frame: f64,
    still: f32,
    u: Units,
}

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
    let mut a = Box::new(NmAgent { agent, build: b.build.clone(), mode: NM_MODE_LEAD, set: vec![false; n], pending: None, shot: None, blast: None, dead: false, fire: None, fall: None, grab: None, car: (None, f32::MAX), door: None, bank: 0.0, stairs: (None, 0, false), cuts: vec![], tackle: None, water: (None, false), singe: 0.0, clock: 0.0,
                                     frame: 0.0, still: 0.0, u: Units(b.scale) });
    tuned(&mut a, &b.tune);
    Box::into_raw(a)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_destroy(a: *mut NmAgent) {
    if !a.is_null() {
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
    let others: Vec<delirium::ragdoll::Capsule> = {
        let mut b = w.3.lock().unwrap_or_else(|e| e.into_inner());
        b.0 += 1;
        let (now, keep) = (b.0, 2 * b.1.len() as u64 + 2);
        b.1.retain(|e| e.0 == me || now - e.1 <= keep);
        b.1.iter().filter(|e| e.0 != me).flat_map(|e| e.2.iter().copied()).collect()
    };
    catch_fire(a, w, dt);
    tackle(a, dt);
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
    if let Some((mut f, burning, health)) = a.fire.take() {
        if a.agent.feedback.iter().any(|f| f.slot == 8 && f.name == "balance") {
            f.balance_failed();
        }
        let done = f.update(&mut a.agent, &a.build, &w.0, &mut RAND.lock().unwrap_or_else(|e| e.into_inner()), dt, burning, health, health <= 0.0);
        if done.is_none() {
            a.fire = Some((f, burning, health));
        }
    }
    a.clock += dt as f64 * 1000.0;
    if a.mode == NM_MODE_FOLLOW {
        a.agent.step_follow(&a.build, &w.0, dt, &w.1, others);
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
    a.agent.doll.impulse(k, unsafe { v3(at) } / u.len(), unsafe { v3(impulse) } / u.impulse());
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_apply_push(a: *mut NmAgent, at: *const f32, impulse: *const f32) -> i32 {
    let a = agent!(a);
    if at.is_null() || impulse.is_null() {
        return fail(NM_E_ARG, "null vector");
    }
    let u = a.u;
    a.agent.doll.push(&a.build, unsafe { v3(at) } / u.len(), unsafe { v3(impulse) } / u.impulse());
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
            *armature = a.agent.doll.armature.get(k).copied().unwrap_or(0.0) * a.u.mass() * a.u.len() * a.u.len();
        }
    }
    if j.hinge { 2 } else { 1 }
}

fn message(m: i32) -> Option<&'static msg::Message> {
    msg::MESSAGES.get(m as usize).filter(|_| m >= 0)
}

fn param(m: i32, p: i32) -> Option<&'static msg::Param> {
    message(m).and_then(|m| m.params.get(p as usize)).filter(|_| p >= 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn nm_message_count() -> i32 {
    msg::MESSAGES.len() as i32
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
    msg::MESSAGES.iter().position(|m| m.name == n).map_or_else(|| fail(NM_E_MESSAGE, format!("no message {n}")), |i| i as i32)
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
pub unsafe extern "C" fn nm_msg_send(a: *mut NmAgent, w: *const NmWorld) -> i32 {
    let a = agent!(a);
    let Some(w) = (unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null world") };
    let Some((m, ps)) = a.pending.take() else { return fail(NM_E_STATE, "nm_msg_begin first") };
    let ps: Vec<(&str, Value)> = ps.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
    let mut st = std::mem::take(&mut a.agent.msgs);
    let r = msg::send(&mut a.agent, &a.build, &w.0, &mut st, &m, &ps);
    a.agent.msgs = st;
    match r {
        Ok(n) => n as i32,
        Err(msg::Error::Message) => fail(NM_E_MESSAGE, format!("no message {m}")),
        Err(msg::Error::Param) => fail(NM_E_PARAM, format!("{m}: a param it doesn't take")),
        Err(msg::Error::Type) => fail(NM_E_TYPE, format!("{m}: a param of the wrong type")),
    }
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
    (!a.dead && !legless(a) && after > 0.0 && a.still * a.u.time() >= after) as i32
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
    if a.mode == NM_MODE_FOLLOW {
        return fail(NM_E_STATE, "nm_agent_sever: LEAD only (a FOLLOW body is the host's)");
    }
    let Some((b, mut d, parts)) = a.agent.doll.sever(&a.build, k) else { return fail(NM_E_STATE, "nm_agent_sever: the root, or cut off already") };
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_task_running(a: *const NmAgent) -> i32 {
    let Some(a) = (unsafe { a.as_ref() }) else { return fail(NM_E_ARG, "null agent") };
    (a.shot.is_some() || a.blast.is_some() || a.fire.is_some()) as i32
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
    with_tp(|tp| f.start(&mut a.agent, &a.build, &w.0, tp, &mut RAND.lock().unwrap_or_else(|e| e.into_inner())));
    a.fire = Some((f, true, health));
    0
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
    let other = Thing { id: k, kind: Kind::Vehicle { player_driver: player_driver != 0, train: None } };
    let imp = bump::Impact { impulse: sum.length(), dt: 1.0 / 60.0, component: d.nearest(&a.build, at) as i32, other: Some(other) };
    let ped = bump::Ped { id: -1, health, inv_mass: 1.0 / d.mass().max(1.0), woke: false, drop: None, vz: d.vel(0).0.z, tough: false,
                          player_cut: false, player: false, mission: false, quick_relax: false, agility: agility.clamp(0, 255) as u8,
                          gun: false, place: d.frames().next().map_or(at, |f| f.translation), vel: Vec3::ZERO, standing_on: None };
    let mut rand = RAND.lock().unwrap_or_else(|e| e.into_inner());
    let Some((amount, _)) = bump::damage(&ped, &imp, &mut rand).filter(|d| d.0 >= 1.0) else { return 0 };
    let place = w.1.get(k as usize).map_or(at, |o| o.centre);
    let around = bump::Around { touching: vec![other], player: (player_driver != 0).then_some(bump::Player { thing: other, place, in_car: true }),
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

