use godot::classes::{
    AnimatableBody3D, BoxShape3D, CapsuleShape3D, CharacterBody3D, CollisionObject3D, CollisionShape3D, ConcavePolygonShape3D,
    Engine, ISkeletonModifier3D, Node, Node3D, PhysicsRayQueryParameters3D, PhysicsShapeQueryParameters3D, ProjectSettings,
    RigidBody3D, SkeletonModifier3D, SphereShape3D, StaticBody3D,
};
use godot::prelude::*;
use delirium_physics::rig::*;
use delirium_physics::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CString, c_char};
use std::sync::Mutex;

struct Delirium;
#[gdextension]
unsafe impl ExtensionLibrary for Delirium {}

fn nm(v: Vector3) -> [f32; 3] { [v.x, -v.z, v.y] }
fn gd(f: &[f32]) -> Vector3 { Vector3::new(f[0], f[2], -f[1]) }
fn nm_turn(q: Quaternion) -> [f32; 4] { [q.x, -q.z, q.y, q.w] }
fn gd_turn(f: &[f32]) -> Quaternion { Quaternion::new(f[0], f[2], -f[1], f[3]) }
fn turn(t: &Transform3D) -> Quaternion { t.basis.orthonormalized().get_quaternion() }

fn c(s: &str) -> CString { CString::new(s).unwrap_or_default() }

fn bone_map(text: &str) -> std::collections::HashMap<String, i32> {
    let part = |n: &str| (0..21).find(|&i| {
        let mut b = [0u8; 64];
        unsafe { nm_part_name(i, b.as_mut_ptr().cast(), 64) };
        std::ffi::CStr::from_bytes_until_nul(&b).is_ok_and(|s| s.to_bytes() == n.as_bytes())
    });
    text.split(',').filter_map(|p| p.split_once('=')).filter_map(|(b, n)| {
        let n = n.trim();
        Some((b.trim().to_string(), if n.eq_ignore_ascii_case("none") { -1 } else { part(n)? }))
    }).collect()
}

fn last_error() -> String {
    let mut buf = [0 as c_char; 512];
    unsafe { nm_last_error(buf.as_mut_ptr(), buf.len() as i32) };
    unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().into_owned()
}

fn check(r: i32, what: &str) -> bool {
    if r < 0 { godot_warn!("Delirium {what}: {}", last_error()) }
    r >= 0
}

fn setting(name: &str, default: Variant) -> Variant {
    let ps = ProjectSettings::singleton();
    let key = format!("delirium/{name}");
    if ps.has_setting(&key) { ps.get_setting(&key) } else { default }
}
fn setting_f(name: &str, d: f32) -> f32 { setting(name, d.to_variant()).try_to::<f32>().unwrap_or(d) }
fn setting_b(name: &str, d: bool) -> bool { setting(name, d.to_variant()).try_to::<bool>().unwrap_or(d) }
fn setting_s(name: &str, d: &str) -> String { setting(name, d.to_variant()).try_to::<GString>().map(|s| s.to_string()).unwrap_or(d.into()) }
fn path(p: &str) -> String {
    if p.is_empty() || std::path::Path::new(p).is_absolute() { return p.into() }
    ProjectSettings::singleton().globalize_path(&format!("res://{p}")).to_string()
}

struct Shared { world: *mut NmWorld, scale: f32, at: Vector3, refs: i32, space: Rid }

thread_local! {
    static BASE: RefCell<Option<*mut NmBuild>> = const { RefCell::new(None) };
    static SHARED: RefCell<Vec<Shared>> = const { RefCell::new(Vec::new()) };
    static CARS: RefCell<Vec<Car>> = const { RefCell::new(Vec::new()) };
    static LEAD: RefCell<Lead> = RefCell::new(Lead::default());
}

#[derive(Default)]
struct Lead { agents: Vec<(*mut NmWorld, *mut NmAgent)>, frame: u64, clock: f32, heard: HashMap<usize, Vec<(String, i32, f32)>> }

impl Lead {
    fn step(world: *mut NmWorld, agent: *mut NmAgent, seconds: f32) -> Vec<(String, i32, f32)> {
        LEAD.with_borrow_mut(|l| {
            if !l.agents.iter().any(|a| a.1 == agent) { l.agents.push((world, agent)); }
            let frame = godot::classes::Engine::singleton().get_process_frames();
            if l.frame != frame {
                l.frame = frame;
                l.clock = (l.clock + seconds).min(4.0 / 60.0);
                while l.clock >= 1.0 / 60.0 {
                    l.clock -= 1.0 / 60.0;
                    let mut worlds: Vec<*mut NmWorld> = l.agents.iter().map(|a| a.0).collect();
                    worlds.sort();
                    worlds.dedup();
                    for w in worlds {
                        let these: Vec<*mut NmAgent> = l.agents.iter().filter(|a| a.0 == w).map(|a| a.1).collect();
                        check(unsafe { nm_world_step_agents(w, these.as_ptr(), these.len() as i32, 1.0 / 60.0, 0) }, "step");
                        for a in these { l.heard.entry(a as usize).or_default().extend(heard(a)); }
                    }
                }
            }
            l.heard.remove(&(agent as usize)).unwrap_or_default()
        })
    }

    fn leave(agent: *mut NmAgent) {
        LEAD.with_borrow_mut(|l| {
            l.agents.retain(|a| a.1 != agent);
            l.heard.remove(&(agent as usize));
        });
    }
}

fn heard(a: *mut NmAgent) -> Vec<(String, i32, f32)> {
    (0..unsafe { nm_agent_feedback_count(a) }).filter_map(|i| {
        let (mut buf, mut slot, mut value) = ([0u8; 64], 0, 0f32);
        (unsafe { nm_agent_feedback_get(a, i, buf.as_mut_ptr() as *mut c_char, 64, &mut slot, &mut value) } >= 0).then(|| {
            let end = buf.iter().position(|&b| b == 0).unwrap_or(64);
            (String::from_utf8_lossy(&buf[..end]).into_owned(), slot, value)
        })
    }).collect()
}

struct Car { node: Gd<Node3D>, centre: Vector3, half: Vector3, last: Vector3, vel: Vector3, turned: Quaternion, spin: Vector3, frame: u64, player: bool, kind: i32, brace: bool, knock_down: bool }
static TUNED: Mutex<Option<HashMap<String, usize>>> = Mutex::new(None);

fn base() -> Option<*mut NmBuild> {
    BASE.with_borrow_mut(|b| {
        if let Some(p) = *b { return (!p.is_null()).then_some(p) }
        *b = Some(std::ptr::null_mut());
        let exe = path(&setting_s("game_exe", ""));
        if !exe.is_empty() && !check(unsafe { nm_unlock(c(&exe).as_ptr()) }, "game_exe") { return None }
        if !check(unsafe { nm_init(c(&path(&setting_s("character", "character.xrt"))).as_ptr()) }, "character") { return None }
        let tp = path(&setting_s("task_params", ""));
        if !tp.is_empty() && !check(unsafe { nm_load_task_params(c(&tp).as_ptr()) }, "task_params") { return None }
        let tunes = ProjectSettings::singleton().globalize_path("user://delirium-tunes.txt").to_string();
        unsafe { nm_tune_file(c(&tunes).as_ptr()) };
        let build = unsafe { nm_build_load(c(&path(&setting_s("build", "build.txt"))).as_ptr()) };
        if build.is_null() {
            godot_error!("Delirium build: {}", last_error());
            return None;
        }
        nm_set_headshot_kills(setting_b("headshot_kills", true) as i32);
        unsafe { nm_crash_report(c(&ProjectSettings::singleton().globalize_path("user://").to_string()).as_ptr()) };
        nm_set_step_limit(setting_b("step_limit", true) as i32);
        nm_set_fire_reaction(setting_b("fire_reaction", true) as i32);
        let weapons = path(&setting_s("weapons", ""));
        if weapons.ends_with(".xml") {
            check(unsafe { nm_weapons_load_info(c(&weapons).as_ptr()) }, "weapons");
        } else if !weapons.is_empty() {
            check(unsafe { nm_weapons_parse(c(&weapons).as_ptr()) }, "weapons");
        }
        *b = Some(build);
        Some(build)
    })
}

fn surface(friction: f32, bounce: f32) -> u8 {
    thread_local! { static ROWS: RefCell<Vec<(f32, f32)>> = const { RefCell::new(Vec::new()) } }
    if friction == 1.0 && bounce == 0.0 { return 0 }
    ROWS.with_borrow_mut(|r| {
        let k = r.iter().position(|&m| m == (friction, bounce)).unwrap_or_else(|| {
            r.push((friction, bounce));
            r.len() - 1
        });
        let row = 200 + k.min(55) as i32;
        if k == r.len() - 1 && k < 56 { unsafe { nm_set_surface(row, friction, bounce.clamp(0.0, 1.0)) }; }
        row as u8
    })
}

fn level(node: &Gd<Node3D>, at: Vector3, radius: f32, world: *mut NmWorld) -> usize {
    let Some(mut space) = node.get_world_3d().and_then(|w| w.get_direct_space_state()) else { return 0 };
    let mut sphere = SphereShape3D::new_gd();
    sphere.set_radius(radius);
    let mut q = PhysicsShapeQueryParameters3D::new_gd();
    q.set_shape(&sphere);
    q.set_transform(Transform3D::new(Basis::IDENTITY, at));
    let hits = space.intersect_shape_ex(&q).max_results(512).done();
    let (mut pts, mut tris, mut mats) = (Vec::<f32>::new(), Vec::<u32>::new(), Vec::<u8>::new());
    let mut seen = Vec::new();
    for h in hits.iter_shared() {
        let Some(body) = h.get("collider").and_then(|v| v.try_to::<Gd<Node>>().ok()) else { continue };
        if body.is_class("CharacterBody3D") || body.is_class("RigidBody3D") || seen.contains(&body.instance_id()) { continue }
        seen.push(body.instance_id());
        let mat = body.clone().try_cast::<StaticBody3D>().ok().and_then(|b| b.get_physics_material_override())
            .map_or(0, |m| surface(m.get_friction(), m.get_bounce()));
        for child in body.get_children().iter_shared() {
            let Ok(cs) = child.try_cast::<CollisionShape3D>() else { continue };
            if cs.is_disabled() { continue }
            let Some(shape) = cs.get_shape() else { continue };
            let t = cs.get_global_transform();
            let first = (pts.len() / 3) as u32;
            if let Ok(b) = shape.clone().try_cast::<BoxShape3D>() {
                let e = b.get_size() * 0.5;
                for i in 0..8 {
                    let p = Vector3::new(if i & 1 != 0 { e.x } else { -e.x }, if i & 2 != 0 { e.y } else { -e.y }, if i & 4 != 0 { e.z } else { -e.z });
                    pts.extend(nm(t * p));
                }
                const F: [[u32; 4]; 6] = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
                for q in F { tris.extend([q[0], q[1], q[2], q[0], q[2], q[3]].map(|k| first + k)) }
            } else if let Ok(m) = shape.try_cast::<ConcavePolygonShape3D>() {
                let faces = m.get_faces();
                for p in faces.as_slice() { pts.extend(nm(t * *p)) }
                for k in (0..faces.len() as u32).step_by(3) { tris.extend([first + k, first + k + 2, first + k + 1]) }
            }
            mats.resize(tris.len() / 3, mat);
        }
    }
    if tris.is_empty() { return 0 }
    check(unsafe { nm_world_add_mesh(world, pts.as_ptr(), (pts.len() / 3) as i32, tris.as_ptr(), (tris.len() / 3) as i32, mats.as_ptr()) }, "level");
    tris.len() / 3
}

#[derive(GodotClass)]
#[class(base = SkeletonModifier3D)]
pub struct DeliriumRagdoll {
    base: Base<SkeletonModifier3D>,
    #[export]
    character: Option<Gd<Node3D>>,
    #[export]
    get_up_blend: f32,
    #[export]
    get_up_after: f32,
    #[export]
    body: GString,
    #[export]
    body_seed: i32,
    #[export]
    body_variance: f32,
    #[export]
    body_symmetric: bool,
    #[export]
    custom_body: GString,
    #[export]
    overrides: GString,
    #[export]
    #[var(set = set_strength)]
    strength: f32,
    #[export]
    bone_map: GString,
    #[export]
    hit_health: f32,
    #[export]
    hit_agility: i32,
    set: Vec<(String, String, f32)>,
    build: *mut NmBuild,
    world: *mut NmWorld,
    agent: *mut NmAgent,
    rig: *mut NmRig,
    bones: Vec<i32>,
    out: Vec<f32>,
    scale: f32,
    told: bool,
    blend: f32,
    last: Vec<Quaternion>,
    last_hips: Vector3,
    hips_world: Transform3D,
    follow: bool,
    down: bool,
    bracing: Option<InstanceId>,
    brace_left: f32,
    bodies: Vec<Gd<AnimatableBody3D>>,
    look: f32,
}

#[godot_api]
impl ISkeletonModifier3D for DeliriumRagdoll {
    fn init(base: Base<SkeletonModifier3D>) -> Self {
        Self {
            base, character: None, get_up_blend: 0.4, get_up_after: 0.0, body: "Fitted".into(), body_seed: 0, body_variance: 1.5, strength: 0.0, bone_map: GString::new(),
            body_symmetric: true, custom_body: "1, 1, 1, 1, 1, 1, 77, 0".into(), overrides: GString::new(), hit_health: 200.0, hit_agility: 5, set: Vec::new(), build: std::ptr::null_mut(), world: std::ptr::null_mut(),
            agent: std::ptr::null_mut(), rig: std::ptr::null_mut(), bones: Vec::new(), out: Vec::new(), scale: 1.0,
            told: false, blend: 0.0, last: Vec::new(), last_hips: Vector3::ZERO, hips_world: Transform3D::IDENTITY, follow: false,
            bracing: None, brace_left: 0.0, down: false, bodies: Vec::new(), look: 0.0,
        }
    }

    fn process_modification_with_delta(&mut self, delta: f64) {
        self.step(delta as f32);
    }

    fn physics_process(&mut self, _delta: f64) {
        if self.bodies.is_empty() || self.agent.is_null() { return }
        for (k, b) in self.bodies.iter_mut().enumerate() {
            let (mut p, mut q) = ([0f32; 3], [0f32; 4]);
            unsafe { nm_agent_get_part_transform(self.agent, k as i32, p.as_mut_ptr(), q.as_mut_ptr()) };
            b.set_global_transform(Transform3D::new(Basis::from_quaternion(gd_turn(&q)), gd(&p)));
        }
    }

    fn exit_tree(&mut self) {
        self.release();
    }
}

#[godot_api]
impl DeliriumRagdoll {
    #[signal]
    fn lying_still(face_up: bool, facing: Vector3);

    #[signal]
    fn car_hit(damage: f32, car: Gd<Node3D>);

    #[signal]
    fn feedback(name: GString, slot: i32, value: f32);

    #[signal]
    fn fell();

    #[func]
    fn get_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let a = self.agent;
        d.set("ragdolled", !a.is_null());
        if a.is_null() { return d }
        let (mut p, mut v, mut head, mut up) = ([0f32; 3], [0f32; 3], [0f32; 3], 0);
        unsafe {
            nm_agent_get_com(a, p.as_mut_ptr(), v.as_mut_ptr());
            for (k, f) in [("balancing", nm_agent_is_balancing as unsafe extern "C" fn(*const NmAgent) -> i32), ("fallen", nm_agent_has_fallen),
                           ("catching_fall", nm_agent_is_catching_fall), ("rolled_up", nm_agent_is_rolled_up), ("relaxing", nm_agent_is_relaxing),
                           ("dead", nm_agent_is_dead), ("in_water", nm_agent_in_water), ("burning", nm_agent_is_burning), ("tased", nm_agent_is_tased)] {
                d.set(k, f(a) == 1);
            }
            d.set("tackling", nm_agent_tackle_state(a));
            d.set("lying_for", nm_agent_get_lying(a, head.as_mut_ptr(), &mut up));
            d.set("mass", nm_agent_get_mass(a));
            for (k, f) in [("settled", nm_agent_is_settled as unsafe extern "C" fn(*const NmAgent) -> i32), ("awake", nm_agent_is_awake),
                           ("jumping", nm_agent_jumping), ("learned_getup", nm_agent_learned_getup_running), ("task_running", nm_agent_task_running)] {
                d.set(k, f(a) == 1);
            }
            d.set("resting", nm_agent_is_resting(a, self.world) == 1);
            let mut st = [0f32; 3];
            if nm_agent_get_stance(a, st.as_mut_ptr()) == 1 { d.set("stance", gd(&st)); }
            let contacts: PackedInt32Array = (0..nm_agent_part_count(a)).map(|k| nm_agent_get_part_contact(a, k).max(0)).collect();
            d.set("contacts", &contacts);
        }
        d.set("centre", gd(&p));
        d.set("velocity", gd(&v));
        d
    }

    #[func]
    fn register_car(car: Gd<Node3D>, size: Vector3, player_driven: bool) {
        Self::register_mover(car, Self::MOVER_CAR, size, player_driven, true, true, Vector3::ZERO);
    }

    #[constant]
    const MOVER_CAR: i32 = 0;
    #[constant]
    const MOVER_OBJECT: i32 = 1;
    #[constant]
    const MOVER_TRAIN: i32 = 3;

    #[func]
    fn register_mover(car: Gd<Node3D>, kind: i32, size: Vector3, player_driven: bool, brace: bool, knock_down: bool, centre: Vector3) {
        if ![Self::MOVER_CAR, Self::MOVER_OBJECT, Self::MOVER_TRAIN].contains(&kind) {
            godot_warn!("Delirium: register_mover {}: kind {kind} isn't a MOVER_*", car.get_name());
            return;
        }
        Self::unregister_car(car.clone());
        let (mut centre, mut half) = (centre, size / 2.0);
        if size == Vector3::ZERO {
            let shape = car.find_children_ex("*").type_("CollisionShape3D").owned(false).done().iter_shared()
                .filter_map(|n| n.try_cast::<CollisionShape3D>().ok())
                .find_map(|cs| cs.get_shape().and_then(|sh| sh.try_cast::<BoxShape3D>().ok()).map(|b| (cs.get_global_position(), b.get_size())));
            let Some((at, sz)) = shape else {
                godot_warn!("Delirium: register_car {}: no BoxShape3D to size it by: give a size", car.get_name());
                return;
            };
            (centre, half) = (car.get_global_transform().affine_inverse() * at, sz / 2.0);
        }
        let last = car.get_global_position();
        let turned = turn(&car.get_global_transform());
        CARS.with_borrow_mut(|cs| cs.push(Car { node: car, centre, half, last, vel: Vector3::ZERO, turned, spin: Vector3::ZERO, frame: Engine::singleton().get_process_frames(),
                                                player: player_driven, kind, brace, knock_down }));
    }

    #[func]
    fn unregister_car(car: Gd<Node3D>) {
        CARS.with_borrow_mut(|cs| cs.retain(|c| c.node != car));
    }

    #[func]
    fn car_coming(&mut self, car: Gd<Node3D>) -> bool {
        if !self.take() { return false }
        if self.cars(0.0, Some(&car)) < 0 {
            godot_warn!("Delirium: car_coming: {} isn't registered (register_car)", car.get_name());
            return false;
        }
        (self.bracing, self.brace_left) = (Some(car.instance_id()), 3.0);
        check(unsafe { nm_agent_task_brace(self.agent, self.world, std::ptr::null()) }, "car_coming")
    }

    #[func]
    fn car_door(&mut self, car: Gd<Node3D>) -> bool {
        if !self.take() { return false }
        let k = self.cars(0.0, Some(&car));
        if k < 0 {
            godot_warn!("Delirium: car_door: {} isn't registered (register_car)", car.get_name());
            return false;
        }
        check(unsafe { nm_agent_task_car_door(self.agent, self.world, k, std::ptr::null()) }, "car_door")
    }

    #[func]
    fn ragdoll(&mut self) -> bool { self.take() }

    #[func]
    fn shoot(&mut self, point: Vector3, direction: Vector3, force: f32, set: GString, health: f32) -> bool {
        if !self.take() { return false }
        let d = direction.normalized();
        let (at, dir, imp) = (nm(point), nm(d), nm(d * force * setting_f("shot_force", 1.0)));
        let part = unsafe { nm_agent_nearest_part(self.agent, at.as_ptr()) }.max(0);
        let set = c(&set.to_string());
        check(unsafe { nm_agent_apply_push(self.agent, at.as_ptr(), imp.as_ptr()) }, "shoot")
            && check(unsafe { nm_agent_task_shot(self.agent, self.world, set.as_ptr(), part, at.as_ptr(), dir.as_ptr(), health) }, "shoot")
    }

    #[func]
    fn shoot_along(&mut self, from: Vector3, aim: Vector3, force: f32, set: GString, health: f32) -> bool {
        if !self.take() { return false }
        let mut hit = [0f32; 3];
        if unsafe { nm_agent_aim_hit(self.agent, nm(from).as_ptr(), nm(aim.normalized()).as_ptr(), hit.as_mut_ptr()) } < 0 { return false }
        self.shoot(gd(&hit), aim, force, set, health)
    }

    #[func]
    fn push(&mut self, impulse: Vector3) -> bool {
        if !self.take() { return false }
        let at = nm(self.get_hips() + Vector3::UP * 0.35 * self.scale);
        check(unsafe { nm_agent_apply_push(self.agent, at.as_ptr(), nm(impulse).as_ptr()) }, "push")
    }

    #[func]
    fn explode(&mut self, at: Vector3, radius: f32, speed: f32) -> bool {
        self.take() && check(unsafe { nm_agent_task_explosion(self.agent, self.world, nm(at).as_ptr(), radius, speed * setting_f("explosion_force", 1.0)) }, "explode")
    }

    #[func]
    fn melee_hit(&mut self, point: Vector3, impulse: Vector3, attacker_head: Vector3, timid: bool) -> bool {
        let imp = nm(impulse * setting_f("melee_force", 1.0));
        self.take() && check(unsafe { nm_agent_task_melee_hit(self.agent, self.world, nm(point).as_ptr(), imp.as_ptr(), nm(attacker_head).as_ptr(), timid as i32) }, "melee_hit")
    }

    #[func]
    fn on_fire(&mut self, burning: bool, health: f32) -> bool {
        self.take() && check(unsafe {
            if burning { nm_agent_task_on_fire(self.agent, self.world, health) } else { nm_agent_set_on_fire(self.agent, 0, health) }
        }, "on_fire")
    }

    #[func]
    fn tase(&mut self, seconds: f32) -> bool {
        self.take() && check(unsafe { nm_agent_tase(self.agent, self.world, seconds) }, "tase")
    }

    #[func]
    fn react(&mut self, line: GString) -> bool {
        self.take() && check(unsafe { nm_msg_send_text(self.agent, self.world, c(&line.to_string()).as_ptr()) }, "react")
    }

    #[func]
    fn high_fall(&mut self) -> bool {
        self.take() && check(unsafe { nm_agent_task_high_fall(self.agent, self.world, std::ptr::null(), 0) }, "high_fall")
    }

    #[func]
    fn catch_fall(&mut self) -> bool {
        self.take() && check(unsafe { nm_agent_task_catch_fall(self.agent, std::ptr::null()) }, "catch_fall")
    }

    #[func]
    fn fall_down(&mut self, heading: f32) -> bool {
        if !self.take() { return false }
        let r = unsafe { nm_agent_task_fall_down(self.agent, self.world, heading) };
        check(r, "fall_down") && r == 1
    }

    #[func]
    fn grab(&mut self, left: Vector3, left_normal: Vector3, right: Vector3, right_normal: Vector3, hold: f32) -> bool {
        if !self.take() { return false }
        let hand = |p: Vector3, n: Vector3| (n != Vector3::ZERO).then(|| [nm(p), nm(n.normalized())].concat());
        let (l, r) = (hand(left, left_normal), hand(right, right_normal));
        let ptr = |h: &Option<Vec<f32>>| h.as_ref().map_or(std::ptr::null(), |h| h.as_ptr());
        check(unsafe { nm_agent_task_grab(self.agent, self.world, ptr(&l), ptr(&r), hold) }, "grab")
    }

    #[func]
    fn jump_roll(&mut self) -> bool {
        self.take() && check(unsafe { nm_agent_task_jump_roll(self.agent, self.world) }, "jump_roll")
    }

    #[func]
    fn flinch(&mut self, from: Vector3) -> bool {
        self.take() && check(unsafe { nm_agent_task_flinch(self.agent, self.world, std::ptr::null(), nm(from).as_ptr()) }, "flinch")
    }

    #[func]
    fn tackle(&mut self, mut target: Gd<DeliriumRagdoll>, low: bool, keep: bool) -> bool {
        if target.instance_id() == self.base().instance_id() { return false }
        let them = { let mut t = target.bind_mut(); if t.take() { t.agent } else { std::ptr::null_mut() } };
        !them.is_null() && self.take() && check(unsafe { nm_agent_task_tackle(self.agent, self.world, them, low as i32, keep as i32) }, "tackle")
    }

    #[func]
    fn set_water(&mut self, level: f32) -> bool {
        !self.agent.is_null() && check(unsafe { nm_agent_set_water(self.agent, self.world, level) }, "set_water")
    }

    #[func]
    fn kill(&mut self) -> bool {
        self.take() && check(unsafe { nm_agent_set_dead(self.agent, 1) }, "kill") && check(unsafe { nm_agent_task_relax(self.agent, -1.0, -1.0) }, "kill")
    }

    #[func]
    fn leash(&mut self, point: Vector3, anchor: Vector3, length: f32) -> i32 {
        if !self.take() || length <= 0.0 { return -1 }
        let at = nm(point);
        let part = unsafe { nm_agent_nearest_part(self.agent, at.as_ptr()) };
        if part < 0 || !check(unsafe { nm_agent_leash(self.agent, part, at.as_ptr(), nm(anchor).as_ptr(), length) }, "leash") { return -1 }
        part
    }

    #[func]
    fn leash_length(&mut self, part: i32, length: f32) -> bool {
        self.take() && check(unsafe { nm_agent_leash(self.agent, part, std::ptr::null(), std::ptr::null(), length) }, "leash_length")
    }

    #[func]
    fn leash_anchor(&mut self, part: i32, anchor: Vector3, length: f32) -> bool {
        self.take() && check(unsafe { nm_agent_leash(self.agent, part, std::ptr::null(), nm(anchor).as_ptr(), length) }, "leash_anchor")
    }

    #[func]
    fn unleash(&mut self, part: i32) -> bool {
        !self.agent.is_null() && check(unsafe { nm_agent_unleash(self.agent, part) }, "unleash")
    }

    #[func]
    fn sever(&mut self, point: Vector3, impulse: Vector3) -> bool {
        if !self.take() { return false }
        let at = nm(point);
        let part = unsafe { nm_agent_nearest_part(self.agent, at.as_ptr()) };
        if part <= 0 || unsafe { nm_agent_is_severed(self.agent, part) } == 1 { return false }
        let cut = check(unsafe { nm_agent_sever(self.agent, self.world, part, at.as_ptr(), nm(impulse).as_ptr()) }, "sever");
        cut
    }

    #[func]
    fn is_ragdolled(&self) -> bool { !self.agent.is_null() }

    #[func]
    fn set_strength(&mut self, k: f32) {
        self.strength = k;
        let k = if k > 0.0 { k } else { setting_f("strength", 1.0) };
        if !self.agent.is_null() {
            check(unsafe { nm_agent_set_strength(self.agent, k) }, "set_strength");
        }
    }

    #[func]
    fn set_override(&mut self, behaviour: GString, param: GString, value: f32) -> bool {
        let (m, p) = (behaviour.to_string(), param.to_string());
        if !self.agent.is_null() && !check(unsafe { nm_agent_set_override(self.agent, c(&m).as_ptr(), c(&p).as_ptr(), value) }, "set_override") {
            return false;
        }
        self.set.retain(|(om, op, _)| (om, op) != (&m, &p));
        self.set.push((m, p, value));
        true
    }

    #[func]
    fn clear_overrides(&mut self) {
        self.set.clear();
        self.overrides = GString::new();
        if !self.agent.is_null() { unsafe { nm_agent_clear_overrides(self.agent) }; }
    }

    #[func]
    fn get_hips(&self) -> Vector3 {
        if !self.agent.is_null() { return self.hips_world.origin }
        let (Some(s), Some(&h)) = (self.base().get_skeleton(), self.bones.first()) else { return Vector3::ZERO };
        (s.get_global_transform() * s.get_bone_global_pose(h)).origin
    }

    #[func]
    fn get_up(&mut self) {
        if self.agent.is_null() { return }
        let Some(s) = self.base().get_skeleton() else { return };
        let (mut head, mut up) = ([0f32; 3], 0);
        unsafe { nm_agent_get_lying(self.agent, head.as_mut_ptr(), &mut up) };
        let facing = (gd(&head) * Vector3::new(1.0, 0.0, 1.0)).normalized_or_zero() * if up == 1 { -1.0 } else { 1.0 };
        let hips_world = self.hips_world;
        if let Some(mut ch) = self.owner() {
            let floor = self.ground(hips_world.origin);
            ch.set_global_position(Vector3::new(hips_world.origin.x, floor, hips_world.origin.z));
            if facing != Vector3::ZERO {
                let to = ch.get_global_position() - facing;
                ch.look_at(to);
            }
        }
        let parent = s.get_bone_parent(self.bones[0]);
        let pw = s.get_global_transform() * if parent >= 0 { s.get_bone_global_pose(parent) } else { Transform3D::IDENTITY };
        let local = pw.affine_inverse() * hips_world;
        self.last[0] = turn(&local);
        self.last_hips = local.origin;
        self.release();
        self.blend = self.get_up_blend.max(0.0);
    }

    #[func]
    fn stop(&mut self) -> bool {
        !self.agent.is_null() && check(unsafe { nm_agent_task_stop(self.agent) }, "stop")
    }

    #[func]
    fn balance(&mut self, look: Vector3) -> bool {
        let l = nm(look);
        self.take() && check(unsafe { nm_agent_task_balance(self.agent, self.world, std::ptr::null(), if look == Vector3::ZERO { std::ptr::null() } else { l.as_ptr() }) }, "balance")
    }

    #[func]
    fn shoot_weapon(&mut self, weapon: GString, point: Vector3, direction: Vector3, health: f32) -> f32 {
        if !self.take() { return -1.0 }
        let k = unsafe { nm_weapon_find(c(&weapon.to_string()).as_ptr()) };
        if k < 0 {
            godot_warn!("Delirium shoot_weapon: no weapon {weapon}");
            return -1.0;
        }
        let at = nm(point);
        let part = unsafe { nm_agent_nearest_part(self.agent, at.as_ptr()) }.max(0);
        unsafe { nm_agent_shoot_weapon(self.agent, self.world, k, part, at.as_ptr(), nm(direction.normalized()).as_ptr(), health) }
    }

    #[func]
    fn shotgun(&mut self, weapon: GString, from: Vector3, aim: Vector3, health: f32, seed: i32) -> i32 {
        if !self.take() { return -1 }
        let k = unsafe { nm_weapon_find(c(&weapon.to_string()).as_ptr()) };
        let (mut v, mut set) = ([0f32; 7], [0 as c_char; 64]);
        if k < 0 || unsafe { nm_weapon_get(k, std::ptr::null_mut(), 0, set.as_mut_ptr(), 64, v.as_mut_ptr()) } < 0 {
            godot_warn!("Delirium shotgun: no weapon {weapon}");
            return -1;
        }
        let n = v[2].max(1.0) as i32;
        let (mut dirs, mut seed) = (vec![0f32; (n as usize + 1) * 3], seed);
        let made = unsafe { nm_shotgun_fan(nm(aim.normalized()).as_ptr(), v[5], v[6], n, &mut seed, dirs.as_mut_ptr()) };
        let mut hits = vec![];
        for d in dirs.chunks(3).take(made.max(0) as usize).skip(1) {
            let mut at = [0f32; 3];
            let part = unsafe { nm_agent_aim_hit(self.agent, nm(from).as_ptr(), d.as_ptr(), at.as_mut_ptr()) };
            if part >= 0 { hits.extend([part as f32, at[0], at[1], at[2], d[0], d[1], d[2]]) }
        }
        let count = hits.len() as i32 / 7;
        if count == 0 { return 0 }
        if check(unsafe { nm_agent_task_shotgun(self.agent, self.world, set.as_ptr(), hits.as_ptr(), count, health, v[0] * setting_f("shot_force", 1.0)) }, "shotgun") { count } else { -1 }
    }

    #[func]
    fn weapon_names() -> PackedStringArray {
        (0..nm_weapon_count()).map(|i| {
            let mut n = [0 as c_char; 64];
            unsafe { nm_weapon_get(i, n.as_mut_ptr(), 64, std::ptr::null_mut(), 0, std::ptr::null_mut()) };
            GString::from(unsafe { std::ffi::CStr::from_ptr(n.as_ptr()) }.to_string_lossy().as_ref())
        }).collect()
    }

    #[func]
    fn load_weapons(text: GString) -> i32 {
        let t = text.to_string();
        let r = if t.ends_with(".xml") { unsafe { nm_weapons_load_info(c(&path(&t)).as_ptr()) } }
                else { unsafe { nm_weapons_parse(if t.is_empty() { std::ptr::null() } else { c(&t).as_ptr() }) } };
        check(r, "load_weapons");
        r
    }

    #[func]
    fn set_reactions(&mut self, bits: i32) -> bool {
        self.take() && check(unsafe { nm_agent_set_reactions(self.agent, bits) }, "set_reactions")
    }
    #[constant] const REACT_WINDMILL: i32 = NM_XR_WINDMILL;
    #[constant] const REACT_CURL_UP: i32 = NM_XR_CURL_UP;
    #[constant] const REACT_CURL_SHOT: i32 = NM_XR_CURL_SHOT;
    #[constant] const REACT_LOOK_HIT: i32 = NM_XR_LOOK_HIT;
    #[constant] const REACT_LEDGE: i32 = NM_XR_LEDGE;
    #[constant] const REACT_SMART_FALL: i32 = NM_XR_SMART_FALL;
    #[constant] const REACT_DODGE: i32 = NM_XR_DODGE;
    #[constant] const REACT_WALL: i32 = NM_XR_WALL;
    #[constant] const REACT_ARMS_OUT: i32 = NM_XR_ARMS_OUT;

    #[func]
    fn near_miss(&mut self, from: Vector3, direction: Vector3) -> bool {
        self.take() && check(unsafe { nm_agent_near_miss(self.agent, self.world, nm(from).as_ptr(), nm(direction.normalized()).as_ptr()) }, "near_miss")
    }

    #[func]
    fn set_dive(&mut self, gap: f32) -> bool {
        self.take() && check(unsafe { nm_agent_set_dive(self.agent, gap) }, "set_dive")
    }

    #[func]
    fn jump(&mut self, dive: bool, direction: Vector3) -> bool {
        self.take() && check(unsafe { nm_agent_jump(self.agent, dive as i32, nm(direction).as_ptr()) }, "jump")
    }

    #[func]
    fn thrown_at(&mut self, at: Vector3, velocity: Vector3, look: bool, avoid: bool) -> bool {
        self.take() && check(unsafe { nm_agent_thrown_at(self.agent, self.world, nm(at).as_ptr(), nm(velocity).as_ptr(), look as i32, avoid as i32) }, "thrown_at")
    }

    #[func]
    fn sit(&mut self, look: Vector3, seat: Vector3) -> bool {
        let (l, st) = (nm(look), nm(seat));
        let ptr = |v: Vector3, f: &[f32; 3]| if v == Vector3::ZERO { std::ptr::null() } else { f.as_ptr() };
        self.take() && { self.follow = true; check(unsafe { nm_agent_task_sit(self.agent, self.world, ptr(look, &l), ptr(seat, &st)) }, "sit") }
    }

    #[func]
    fn pose(&mut self, low: f32, high: f32) -> bool {
        self.take() && { self.follow = true; check(unsafe { nm_agent_task_pose(self.agent, self.world, low, high) }, "pose") }
    }

    #[func]
    fn muscle_rise(&mut self, on: bool, help: f32) -> bool {
        self.take() && { self.follow |= on; check(unsafe { nm_agent_muscle_rise(self.agent, self.world, on as i32, help) }, "muscle_rise") }
    }

    #[func]
    fn learned_getup(&mut self, net: GString) -> bool {
        let n = if net.is_empty() { setting_s("getup_net", "getup.net") } else { net.to_string() };
        self.take() && check(unsafe { nm_agent_learned_getup(self.agent, c(&path(&n)).as_ptr()) }, "learned_getup")
    }

    #[func]
    fn learned_walk(&mut self, net: GString, cycle: f32) -> bool {
        if !self.take() { return false }
        let n = c(&path(&net.to_string()));
        check(unsafe { nm_agent_learned_walk(self.agent, self.world, if net.is_empty() { std::ptr::null() } else { n.as_ptr() }, cycle) }, "learned_walk")
    }

    #[func]
    fn set_stick(&mut self, on: bool, x: f32, y: f32, camera: f32) -> bool {
        self.take() && check(unsafe { nm_agent_set_stick(self.agent, on as i32, x, y, camera) }, "set_stick")
    }

    #[func]
    fn roll_up_relax(&mut self, stiffness: f32, damping: f32, roll_ms: i32) -> bool {
        self.take() && check(unsafe { nm_agent_task_roll_up_relax(self.agent, self.world, stiffness, damping, roll_ms) }, "roll_up_relax")
    }

    #[func]
    fn script(&mut self, ms: i32) -> bool {
        self.take() && check(unsafe { nm_agent_task_script(self.agent, ms) }, "script")
    }

    #[func]
    fn set_attacker(&mut self, attacker: Option<Gd<DeliriumRagdoll>>, armed: bool) -> bool {
        if !self.take() { return false }
        let Some(at) = attacker else { return check(unsafe { nm_agent_set_attacker(self.agent, std::ptr::null(), 0) }, "set_attacker") };
        let h = at.bind().get_hips();
        let up = Vector3::UP * at.bind().scale;
        let bones: Vec<f32> = [0.0, 0.2, 0.4, 0.65].iter().flat_map(|&k| nm(h + up * k)).collect();
        check(unsafe { nm_agent_set_attacker(self.agent, bones.as_ptr(), armed as i32) }, "set_attacker")
    }

    #[func]
    fn gun_ready(&mut self) -> bool {
        !self.agent.is_null() && unsafe { nm_agent_gun_ready(self.agent) } == 1
    }

    #[func]
    fn get_param(&self, message: GString, param: GString) -> Variant {
        if self.agent.is_null() { return Variant::nil() }
        let (m, p) = (c(&message.to_string()), c(&param.to_string()));
        let mut v = [0f32; 3];
        match unsafe { nm_agent_get_param(self.agent, m.as_ptr(), p.as_ptr(), v.as_mut_ptr()) } {
            1 => v[0].to_variant(),
            3 => Vector3::new(v[0], v[1], v[2]).to_variant(),
            0 => {
                let mut b = [0 as c_char; 256];
                if unsafe { nm_agent_get_param_string(self.agent, m.as_ptr(), p.as_ptr(), b.as_mut_ptr(), 256) } < 0 { return Variant::nil() }
                GString::from(unsafe { std::ffi::CStr::from_ptr(b.as_ptr()) }.to_string_lossy().as_ref()).to_variant()
            }
            _ => Variant::nil(),
        }
    }

    #[func]
    fn raycast(&self, from: Vector3, to: Vector3) -> VarDictionary {
        let mut d = VarDictionary::new();
        if self.agent.is_null() { return d }
        let (mut p, mut n) = ([0f32; 3], [0f32; 3]);
        let k = unsafe { nm_agent_raycast(self.agent, nm(from).as_ptr(), nm(to).as_ptr(), p.as_mut_ptr(), n.as_mut_ptr()) };
        if k >= 0 {
            d.set("part", k);
            d.set("point", gd(&p));
            d.set("normal", gd(&n));
        }
        d
    }

    #[func]
    fn check_setup(&self) -> GString {
        if self.agent.is_null() { return "not ragdolled".into() }
        let mut b = vec![0 as c_char; 4096];
        unsafe { nm_agent_check(self.agent, b.as_mut_ptr(), b.len() as i32) };
        GString::from(unsafe { std::ffi::CStr::from_ptr(b.as_ptr()) }.to_string_lossy().as_ref())
    }

    #[func]
    fn messages() -> VarArray {
        let text = |f: &dyn Fn(*mut c_char, i32) -> i32| {
            let mut b = vec![0 as c_char; 1024];
            f(b.as_mut_ptr(), 1024);
            GString::from(unsafe { std::ffi::CStr::from_ptr(b.as_ptr()) }.to_string_lossy().as_ref())
        };
        (0..nm_message_count()).map(|m| {
            let mut d = VarDictionary::new();
            d.set("name", &text(&|b, n| unsafe { nm_message_name(m, b, n) }));
            let ps: VarArray = (0..nm_message_param_count(m).max(0)).map(|p| {
                let mut e = VarDictionary::new();
                e.set("name", &text(&|b, n| unsafe { nm_message_param_name(m, p, b, n) }));
                e.set("kind", nm_message_param_kind(m, p));
                e.set("used", nm_message_param_used(m, p) == 1);
                e.set("doc", &text(&|b, n| unsafe { nm_message_param_doc(m, p, b, n) }));
                let mut v = [0f32; 3];
                let k = unsafe { nm_message_param_default(m, p, v.as_mut_ptr()) };
                e.set("default", &if k == 3 { Vector3::new(v[0], v[1], v[2]).to_variant() } else if k == 1 { v[0].to_variant() } else { Variant::nil() });
                e.to_variant()
            }).collect();
            d.set("params", &ps);
            d.to_variant()
        }).collect()
    }
}

impl DeliriumRagdoll {
    fn owner(&self) -> Option<Gd<Node3D>> {
        self.character.clone().or_else(|| self.base().get_skeleton()?.get_owner()?.try_cast::<Node3D>().ok())
    }

    fn ground(&self, at: Vector3) -> f32 {
        self.ground_hit(at).map_or(at.y, |h| h.0)
    }

    fn ground_hit(&self, at: Vector3) -> Option<(f32, u8)> {
        let mut space = self.base().get_world_3d().and_then(|w| w.get_direct_space_state())?;
        let mut q = PhysicsRayQueryParameters3D::create(at, at - Vector3::UP * 5.0)?;
        if let Some(ch) = self.owner().and_then(|o| o.try_cast::<CharacterBody3D>().ok()) {
            let mut ex = Array::new();
            ex.push(ch.get_rid());
            q.set_exclude(&ex);
        }
        let hit = space.intersect_ray(&q);
        let y = hit.get("position").and_then(|v| v.try_to::<Vector3>().ok())?.y;
        let mat = hit.get("collider").and_then(|v| v.try_to::<Gd<StaticBody3D>>().ok()).and_then(|b| b.get_physics_material_override())
            .map_or(0, |m| surface(m.get_friction(), m.get_bounce()));
        Some((y, mat))
    }

    fn cars(&mut self, seconds: f32, car: Option<&Gd<Node3D>>) -> i32 {
        let frame = Engine::singleton().get_process_frames();
        let here = self.base().get_skeleton().map_or(Vector3::ZERO, |s| s.get_global_position());
        let mut coming = None;
        let boxes: Vec<f32> = CARS.with_borrow_mut(|cs| {
            cs.retain(|c| c.node.is_instance_valid());
            cs.iter_mut().flat_map(|c| {
                let t = c.node.get_global_transform();
                if c.frame != frame {
                    let q = turn(&t);
                    if seconds > 0.0 {
                        c.vel = (t.origin - c.last) / seconds;
                        let d = q * c.turned.inverse();
                        let d = if d.w < 0.0 { -d } else { d };
                        c.spin = if d.w >= 1.0 - 1e-7 { Vector3::ZERO } else { d.get_axis() * d.get_angle() / seconds };
                    }
                    (c.last, c.turned, c.frame) = (t.origin, q, frame);
                }
                if c.knock_down && c.vel.length() > 2.0 {
                    let l = t.affine_inverse() * (here - c.vel * 0.25) - c.centre;
                    if l.x.abs() < c.half.x + 0.3 && l.y.abs() < c.half.y + 0.3 && l.z.abs() < c.half.z + 0.3 {
                        coming = Some((c.node.instance_id(), c.brace));
                    }
                }
                [nm(t * c.centre).as_slice(), &nm_turn(t.basis.get_quaternion()), &[c.half.x, c.half.z, c.half.y], &nm(c.vel)].concat()
            }).collect()
        });
        let n = (boxes.len() / 13) as i32;
        if let Some((id, brace)) = coming.filter(|_| self.agent.is_null()) {
            if self.take() && brace {
                (self.bracing, self.brace_left) = (Some(id), 3.0);
                check(unsafe { nm_agent_task_brace(self.agent, self.world, std::ptr::null()) }, "brace");
            }
        }
        if !self.agent.is_null() && n > 0 {
            check(unsafe { nm_world_set_boxes(self.world, boxes.as_ptr(), n) }, "set_boxes");
            let kinds: Vec<i32> = CARS.with_borrow(|cs| cs.iter().map(|c| c.kind | if c.player { NM_HIT_PLAYER } else { 0 }).collect());
            check(unsafe { nm_world_set_box_kinds(self.world, kinds.as_ptr(), n) }, "set_box_kinds");
            let spins: Vec<f32> = CARS.with_borrow(|cs| cs.iter().flat_map(|c| nm(c.spin)).collect());
            check(unsafe { nm_world_set_box_spins(self.world, spins.as_ptr(), n) }, "set_box_spins");
            if let Some(id) = self.bracing {
                self.brace_left -= seconds;
                let at = CARS.with_borrow(|cs| cs.iter().find(|c| c.node.instance_id() == id).map(|c| (c.node.get_global_transform(), c.centre)));
                match at {
                    Some((t, centre)) if self.brace_left > 0.0 => {
                        let m = [nm(t.basis.col_a()), nm(-t.basis.col_c()), nm(t.basis.col_b()), nm(t * centre)].concat();
                        check(unsafe { nm_agent_task_brace_control(self.agent, self.world, m.as_ptr(), std::ptr::null(), std::ptr::null()) }, "brace");
                    }
                    _ => self.bracing = None,
                }
            }
        }
        match car {
            Some(car) => CARS.with_borrow(|cs| cs.iter().position(|c| c.node == *car).map_or(-1, |k| k as i32)),
            None => n,
        }
    }

    fn take(&mut self) -> bool {
        if !self.agent.is_null() { return true }
        let Some(base) = base() else { return false };
        let Some(s) = self.base().get_skeleton() else {
            godot_warn!("Delirium: put DeliriumRagdoll under a Skeleton3D");
            return false;
        };
        let all = s.get_bone_count();
        let map = bone_map(&self.bone_map.to_string());
        let guess: Vec<i32> = (0..all).map(|b| {
            let name = s.get_bone_name(b).to_string();
            map.get(&name).copied().unwrap_or_else(|| unsafe { nm_rig_part_for_name(c(&name).as_ptr()) })
        }).collect();
        let hips = guess.iter().position(|&g| g == 0).map(|h| h as i32).or_else(|| {
            let (l, r) = (guess.iter().position(|&g| g == 1)? as i32, guess.iter().position(|&g| g == 4)? as i32);
            (s.get_bone_parent(l) == s.get_bone_parent(r)).then(|| s.get_bone_parent(l)).filter(|&p| p >= 0)
        });
        let Some(hips) = hips else {
            godot_warn!("Delirium: no hips among the skeleton's bones");
            return false;
        };
        self.bones = vec![hips];
        let mut i = 0;
        while i < self.bones.len() {
            let kids = s.get_bone_children(self.bones[i]);
            self.bones.extend(kids.as_slice());
            i += 1;
        }
        let n = self.bones.len();
        let at = |b: i32| self.bones.iter().position(|&x| x == b).map_or(-1, |k| k as i32);
        let parents: Vec<i32> = self.bones.iter().enumerate().map(|(k, &b)| if k == 0 { -1 } else { at(s.get_bone_parent(b)) }).collect();
        let mut parts: Vec<i32> = self.bones.iter().enumerate().map(|(k, &b)| if k == 0 { 0 } else { guess[b as usize] }).collect();
        let g = s.get_global_transform();
        let (mut places, mut rest, mut now) = (Vec::new(), Vec::new(), Vec::new());
        for &b in &self.bones {
            let w = g * s.get_bone_global_rest(b);
            places.extend(nm(w.origin));
            rest.extend(nm(w.origin));
            rest.extend(nm_turn(turn(&w)));
            let p = g * s.get_bone_global_pose(b);
            now.extend(nm(p.origin));
            now.extend(nm_turn(turn(&p)));
        }
        let mut report = [0 as c_char; 2048];
        if unsafe { nm_rig_check(n as i32, parents.as_ptr(), places.as_ptr(), parts.as_ptr(), report.as_mut_ptr(), report.len() as i32) } > 0 {
            godot_warn!("Delirium skeleton: {}", unsafe { std::ffi::CStr::from_ptr(report.as_ptr()) }.to_string_lossy());
        }
        let mut joints = [0f32; 63];
        if !check(unsafe { nm_rig_joints(n as i32, parents.as_ptr(), places.as_ptr(), parts.as_mut_ptr(), joints.as_mut_ptr()) }, "rig_joints") { return false }
        let mode = match setting_s("size", "Auto").as_str() { "Homer" => NM_SIZE_BASE, "Real" => NM_SIZE_REAL, _ => NM_SIZE_AUTO };
        self.build = unsafe { nm_build_fit_sized(base, joints.as_ptr(), mode) };
        if self.build.is_null() {
            godot_warn!("Delirium: {}", last_error());
            return false;
        }
        self.scale = unsafe { nm_build_get_scale(self.build) };
        let body = Body {
            name: self.body.to_string(),
            seed: if self.body_seed != 0 { self.body_seed } else { clock_nanos() },
            variance: self.body_variance,
            symmetric: self.body_symmetric,
            custom: self.custom_body.to_string(),
        };
        match body.make(self.build) {
            Ok(Some(b)) => {
                unsafe { nm_build_destroy(self.build) };
                self.build = b;
            }
            Ok(None) => {}
            Err(e) => godot_warn!("Delirium body {}: {e}", body.name),
        }
        if setting_b("adaptive", true) {
            let kind = (0..all).map(|b| s.get_bone_name(b).to_string()).collect::<Vec<_>>().join("|") + &format!("@{}{}", self.scale, body.key());
            let mut t = TUNED.lock().unwrap();
            let map = t.get_or_insert_with(HashMap::new);
            match map.get(&kind) {
                Some(&p) => {
                    if p != 0 { unsafe { nm_build_copy_tune(self.build, p as *const NmBuild) }; }
                }
                None => {
                    map.insert(kind.clone(), 0);
                    let (b, j) = (base as usize, joints);
                    std::thread::spawn(move || {
                        let mut t = unsafe { nm_build_fit_sized(b as *const NmBuild, j.as_ptr(), mode) };
                        if let (false, Ok(Some(r))) = (t.is_null(), body.make(t)) {
                            unsafe { nm_build_destroy(t) };
                            t = r;
                        }
                        let got = if t.is_null() { -1 } else { unsafe { nm_build_tune(t, 200) } };
                        TUNED.lock().unwrap().get_or_insert_with(HashMap::new).insert(kind, t as usize);
                        godot_print!("Delirium: balance tuned, stands through {got} of 3 pushes");
                    });
                }
            }
        }
        self.hips_world = g * s.get_bone_global_pose(hips);
        self.last = vec![Quaternion::IDENTITY; n];
        let hips_now = self.hips_world.origin;
        let floor = self.ground(hips_now);
        let spot = Vector3::new(hips_now.x, floor, hips_now.z);
        let scale = self.scale;
        let space = self.base().get_world_3d().map_or(Rid::Invalid, |w| w.get_space());
        let (world, fresh) = SHARED.with_borrow_mut(|sh| {
            if let Some(w) = sh.iter_mut().find(|w| w.space == space && w.scale == scale && (w.at.y - floor).abs() < 0.5 * scale && w.at.distance_to(spot) < 5.0 * scale) {
                w.refs += 1;
                return (w.world, false);
            }
            let world = nm_world_create();
            check(unsafe { nm_world_set_scale(world, scale) }, "world_set_scale");
            sh.push(Shared { world, scale, at: spot, refs: 1, space });
            (world, true)
        });
        self.world = world;
        if fresh {
            let me = self.base().clone().upcast::<Node3D>();
            let tris = level(&me, hips_now, 15.0, self.world);
            godot_print!("Delirium: {tris} level triangles");
            let f = nm(spot);
            match self.ground_hit(spot + Vector3::UP * 0.5 * scale) {
                Some((_, mat)) if mat != 0 => {
                    let h = 100.0 * scale;
                    let v = [f[0] - h, f[1] - h, f[2], f[0] + h, f[1] - h, f[2], f[0] + h, f[1] + h, f[2], f[0] - h, f[1] + h, f[2]];
                    check(unsafe { nm_world_add_mesh(self.world, v.as_ptr(), 4, [0u32, 1, 2, 0, 2, 3].as_ptr(), 2, [mat, mat].as_ptr()) }, "world_add_floor");
                }
                _ => { check(unsafe { nm_world_add_floor(self.world, f[0], f[1], f[2], 100.0 * scale) }, "world_add_floor"); }
            }
        }
        let r = (g * s.get_bone_global_rest(hips)).origin;
        let start = nm(Vector3::new(r.x, floor, r.z));
        self.agent = unsafe { nm_agent_create(self.build, start.as_ptr(), 0.0, rand_seed()) };
        if self.agent.is_null() {
            godot_warn!("Delirium: {}", last_error());
            self.release();
            return false;
        }
        let strength = if self.strength > 0.0 { self.strength } else { setting_f("strength", 1.0) };
        if strength != 1.0 { check(unsafe { nm_agent_set_strength(self.agent, strength) }, "strength"); }
        if setting_b("arms_out_early", false) {
            let m = c("braceForImpact");
            check(unsafe { nm_agent_set_override(self.agent, m.as_ptr(), c("braceDistance").as_ptr(), 1.3) }, "arms_out_early");
            check(unsafe { nm_agent_set_override(self.agent, m.as_ptr(), c("targetPredictionTime").as_ptr(), 0.45) }, "arms_out_early");
        }
        for (m, p, v) in parse_overrides(&self.overrides.to_string()).into_iter().chain(self.set.iter().cloned()) {
            check(unsafe { nm_agent_set_override(self.agent, c(&m).as_ptr(), c(&p).as_ptr(), v) }, "overrides");
        }
        self.rig = unsafe { nm_rig_create(self.agent, n as i32, parents.as_ptr(), rest.as_ptr(), parts.as_ptr()) };
        if self.rig.is_null() {
            godot_warn!("Delirium: {}", last_error());
            self.release();
            return false;
        }
        check(unsafe { nm_rig_take(self.rig, self.agent, now.as_ptr(), std::ptr::null(), 0.0) }, "rig_take");
        if setting_b("recentre_joints", false) { unsafe { nm_agent_recentre_joint(self.agent, -1) }; }
        for h in setting_s("hold", "").split(',').filter(|h| !h.trim().is_empty()) {
            let v: Vec<f32> = h.split_once('=').map(|(_, v)| v.split(':').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
            match (h.split_once('='), &v[..]) {
                (Some((m, _)), &[k, d, g]) => { check(unsafe { nm_agent_hold(self.agent, c(m.trim()).as_ptr(), k, d, g) }, "hold"); }
                _ => godot_warn!("Delirium: hold: \"{}\" isn't mask=stiffness:damping:gravity", h.trim()),
            }
        }
        self.follow = false;
        for f in setting_s("follow", "").split(',').filter(|f| !f.trim().is_empty()) {
            match f.split_once('=').map(|(m, w)| (m.trim(), w.trim().parse::<f32>())) {
                Some((m, Ok(w))) => self.follow |= check(unsafe { nm_agent_follow(self.agent, c(m).as_ptr(), w) }, "follow"),
                _ => godot_warn!("Delirium: follow: \"{}\" isn't mask=weight", f.trim()),
            }
        }
        let owner = self.owner();
        let vel = owner.clone().and_then(|o| o.try_cast::<CharacterBody3D>().ok()).map(|c| c.get_velocity())
            .or_else(|| owner.and_then(|o| o.try_cast::<RigidBody3D>().ok()).map(|r| r.get_linear_velocity()))
            .unwrap_or(Vector3::ZERO);
        if vel != Vector3::ZERO { check(unsafe { nm_agent_add_velocity(self.agent, nm(vel).as_ptr()) }, "add_velocity"); }
        if setting_b("engine_physics", false) { self.build_bodies(); }
        self.out = vec![0.0; n * 4 + 3];
        if setting_b("balance", true) {
            let (name, start) = (c("bodyBalance"), c("start"));
            unsafe {
                nm_msg_begin(self.agent, name.as_ptr());
                nm_msg_set_bool(self.agent, start.as_ptr(), 1);
            }
            check(unsafe { nm_msg_send(self.agent, self.world) }, "bodyBalance");
        }
        self.told = false;
        self.blend = 0.0;
        self.base_mut().set_influence(1.0);
        godot_print!("Delirium: ragdolled ({n} bones, scale {:.2})", self.scale);
        true
    }

    fn step(&mut self, seconds: f32) {
        let Some(mut s) = self.base().get_skeleton() else { return };
        if self.blend > 0.0 {
            self.blend = (self.blend - seconds).max(0.0);
            let k = self.blend / self.get_up_blend.max(1e-3);
            self.base_mut().set_influence(k * k * (3.0 - 2.0 * k));
            for (i, &b) in self.bones.iter().enumerate() { s.set_bone_pose_rotation(b, self.last[i]) }
            s.set_bone_pose_position(self.bones[0], self.last_hips);
            return;
        }
        let ncars = self.cars(seconds, None);
        if self.agent.is_null() { return }
        if self.follow {
            let g = s.get_global_transform();
            let now: Vec<f32> = self.bones.iter().flat_map(|&b| { let p = g * s.get_bone_global_pose(b); nm(p.origin).into_iter().chain(nm_turn(turn(&p))) }).collect();
            check(unsafe { nm_rig_incoming(self.rig, self.agent, now.as_ptr()) }, "rig_incoming");
        }
        if !self.bodies.is_empty() {
            self.look -= seconds;
            if self.look <= 0.0 {
                self.look = 0.25;
                self.movers_near();
            }
        }
        let said = Lead::step(self.world, self.agent, seconds);
        self.say(said);
        if self.agent.is_null() { return }
        let down = unsafe { nm_agent_has_fallen(self.agent) } == 1;
        if down && !self.down { self.signals().fell().emit(); }
        self.down = down;
        if ncars > 0 {
            let here = s.get_global_position();
            let player = CARS.with_borrow(|cs| cs.iter().min_by(|a, b| a.last.distance_squared_to(here).total_cmp(&b.last.distance_squared_to(here))).is_some_and(|c| c.player));
            let (mut damage, mut k) = (0f32, -1);
            if unsafe { nm_agent_car_hit(self.agent, self.world, self.hit_health, self.hit_agility, player as i32, &mut damage, &mut k) } == 1 {
                self.bracing = None;
                if let Some(car) = CARS.with_borrow(|cs| cs.get(k as usize).map(|c| c.node.clone())) {
                    self.signals().car_hit().emit(damage, &car);
                }
            }
        }
        let n = self.bones.len();
        let placed = unsafe { nm_rig_pose(self.rig, self.agent, self.out.as_mut_ptr()) } == 0;
        for i in 1..n {
            self.last[i] = gd_turn(&self.out[i * 4..]);
            s.set_bone_pose_rotation(self.bones[i], self.last[i]);
        }
        let g = s.get_global_transform();
        let hips = Transform3D::new(Basis::from_quaternion(gd_turn(&self.out[0..4])), if placed { gd(&self.out[n * 4..]) } else { self.hips_world.origin });
        self.hips_world = hips;
        let parent = s.get_bone_parent(self.bones[0]);
        let pw = g * if parent >= 0 { s.get_bone_global_pose(parent) } else { Transform3D::IDENTITY };
        let local = pw.affine_inverse() * hips;
        self.last[0] = turn(&local);
        self.last_hips = local.origin;
        s.set_bone_pose_rotation(self.bones[0], self.last[0]);
        s.set_bone_pose_position(self.bones[0], local.origin);
        let (mut head, mut up) = ([0f32; 3], 0);
        let still = unsafe { nm_agent_get_lying(self.agent, head.as_mut_ptr(), &mut up) };
        if still < 0.5 {
            self.told = false;
            return;
        }
        if !self.told {
            self.told = true;
            let facing = (gd(&head) * Vector3::new(1.0, 0.0, 1.0)).normalized_or_zero() * if up == 1 { -1.0 } else { 1.0 };
            self.signals().lying_still().emit(up == 1, facing);
        }
        if self.get_up_after > 0.0 && !self.agent.is_null() && unsafe { nm_agent_should_get_up(self.agent, self.get_up_after) } == 1 {
            self.get_up();
        }
    }

    fn say(&mut self, said: Vec<(String, i32, f32)>) {
        for (name, slot, value) in said {
            self.signals().feedback().emit(&GString::from(name.as_str()), slot, value);
        }
    }

    fn build_bodies(&mut self) {
        let n = unsafe { nm_agent_part_count(self.agent) } as usize;
        let Some(owner) = self.owner() else { return };
        let Some(mut parent) = owner.get_parent() else { return };
        let mut root = Node3D::new_alloc();
        root.set_name(&format!("{} ragdoll (Delirium)", owner.get_name()));
        parent.add_child(&root);
        for k in 0..n {
            let (mut radius, mut half, mut mass, mut inertia, mut half_box) = (0f32, 0f32, 0f32, [0f32; 3], [0f32; 3]);
            unsafe { nm_build_part_info(self.build, k as i32, std::ptr::null_mut(), &mut radius, &mut half, std::ptr::null_mut()) };
            let kind = unsafe { nm_agent_get_part_body(self.agent, k as i32, &mut mass, inertia.as_mut_ptr(), half_box.as_mut_ptr()) };
            let mut b = AnimatableBody3D::new_alloc();
            b.set_name(&format!("part {k}"));
            b.set_sync_to_physics(false);
            let mut cs = CollisionShape3D::new_alloc();
            if kind == 2 {
                let mut shape = BoxShape3D::new_gd();
                shape.set_size(Vector3::new(half_box[0], half_box[2], half_box[1]) * 2.0);
                cs.set_shape(&shape);
            } else {
                let mut shape = CapsuleShape3D::new_gd();
                shape.set_radius(radius * self.scale);
                shape.set_height(2.0 * (half + radius) * self.scale);
                cs.set_shape(&shape);
                cs.set_basis(Basis::from_axis_angle(Vector3::RIGHT, std::f32::consts::FRAC_PI_2));
            }
            b.add_child(&cs);
            root.add_child(&b);
            let (mut p, mut q) = ([0f32; 3], [0f32; 4]);
            unsafe { nm_agent_get_part_transform(self.agent, k as i32, p.as_mut_ptr(), q.as_mut_ptr()) };
            b.set_global_transform(Transform3D::new(Basis::from_quaternion(gd_turn(&q)), gd(&p)));
            self.bodies.push(b);
        }
        let own: Vec<Gd<CollisionObject3D>> = owner.clone().upcast::<Node>().find_children_ex("*").type_("CollisionObject3D").owned(false).done()
            .iter_shared().filter_map(|c| c.try_cast::<CollisionObject3D>().ok()).chain(owner.try_cast::<CollisionObject3D>().ok()).collect();
        for a in 0..n {
            for b in 0..n { if a != b { let o = self.bodies[b].clone(); self.bodies[a].add_collision_exception_with(&o); } }
            for o in &own { self.bodies[a].add_collision_exception_with(o); }
        }
        self.look = 0.0;
    }

    fn movers_near(&self) {
        let Some(s) = self.base().get_skeleton() else { return };
        let Some(mut space) = s.get_world_3d().and_then(|w| w.get_direct_space_state()) else { return };
        let mut sphere = SphereShape3D::new_gd();
        sphere.set_radius(5.0 * self.scale);
        let mut q = PhysicsShapeQueryParameters3D::new_gd();
        q.set_shape(&sphere);
        q.set_transform(Transform3D::new(Basis::IDENTITY, s.get_global_position()));
        for h in space.intersect_shape_ex(&q).max_results(64).done().iter_shared() {
            let Some(body) = h.get("collider").and_then(|v| v.try_to::<Gd<Node>>().ok()).and_then(|n| n.try_cast::<RigidBody3D>().ok()) else { continue };
            if CARS.with_borrow(|cs| cs.iter().any(|c| c.node.instance_id() == body.instance_id())) { continue }
            let mut bounds: Option<Aabb> = None;
            for cs in body.get_children().iter_shared().filter_map(|c| c.try_cast::<CollisionShape3D>().ok()) {
                let (Some(b), t) = (cs.get_shape().and_then(|s| s.get_debug_mesh()).map(|m| m.get_aabb()), cs.get_transform()) else { continue };
                for k in 0..8 {
                    let p = t * (b.position + b.size * Vector3::new((k & 1) as f32, (k >> 1 & 1) as f32, (k >> 2 & 1) as f32));
                    bounds = Some(bounds.map_or(Aabb::new(p, Vector3::ZERO), |a| a.expand(p)));
                }
            }
            let Some(bounds) = bounds else { continue };
            Self::register_mover(body.upcast(), Self::MOVER_OBJECT, bounds.size, false, false, false, bounds.center());
        }
    }

    fn release(&mut self) {
        Lead::leave(self.agent);
        if let Some(mut b) = self.bodies.first().and_then(|b| b.get_parent()) { b.queue_free(); }
        self.bodies.clear();
        unsafe {
            if !self.rig.is_null() { nm_rig_destroy(self.rig) }
            if !self.agent.is_null() { nm_agent_destroy(self.agent) }
            if !self.build.is_null() { nm_build_destroy(self.build) }
        }
        if !self.world.is_null() {
            let w = self.world;
            SHARED.with_borrow_mut(|sh| {
                let i = sh.iter().position(|x| x.world == w);
                match i {
                    Some(i) if sh[i].refs > 1 => sh[i].refs -= 1,
                    _ => {
                        if let Some(i) = i { sh.remove(i); }
                        unsafe { nm_world_destroy(w) }
                    }
                }
            });
        }
        self.rig = std::ptr::null_mut();
        self.agent = std::ptr::null_mut();
        self.build = std::ptr::null_mut();
        self.world = std::ptr::null_mut();
    }
}

fn rand_seed() -> f32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    (t % 1000) as f32 / 1000.0
}

fn clock_nanos() -> i32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as i32)
}

fn parse_overrides(s: &str) -> Vec<(String, String, f32)> {
    s.split(',').filter(|x| !x.trim().is_empty()).filter_map(|x| {
        let got = x.split_once('=').and_then(|(k, v)| Some((k.trim().split_once('.')?, v.trim().parse::<f32>().ok()?)));
        if got.is_none() { godot_warn!("Delirium overrides: {x:?} isn't behaviour.param=value") }
        got.map(|((m, p), v)| (m.into(), p.into(), v))
    }).collect()
}

#[derive(Clone)]
struct Body { name: String, seed: i32, variance: f32, symmetric: bool, custom: String }

impl Body {
    fn make(&self, b: *mut NmBuild) -> Result<Option<*mut NmBuild>, String> {
        let r = match self.name.as_str() {
            "" | "Fitted" => return Ok(None),
            "Random" => unsafe { nm_build_random(b, self.seed, self.variance, self.symmetric as i32) },
            "Custom" => {
                let v: Vec<f32> = self.custom.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                let [legs, arms, trunk, width, head, neck, kg, height] = v[..] else { return Err("custom_body needs 8 numbers".into()) };
                unsafe { nm_build_sized(b, legs, arms, trunk, width, head, neck, kg, height) }
            }
            n => unsafe { nm_build_named(b, c(n).as_ptr()) },
        };
        if r.is_null() { Err(last_error()) } else { Ok(Some(r)) }
    }

    fn key(&self) -> String {
        match self.name.as_str() {
            "" | "Fitted" => String::new(),
            "Random" => format!(" Random {} {} {}", self.seed, self.variance, self.symmetric),
            "Custom" => format!(" Custom {}", self.custom),
            n => format!(" {n}"),
        }
    }
}
