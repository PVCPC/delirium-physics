use godot::classes::{
    BoxShape3D, CharacterBody3D, CollisionShape3D, ConcavePolygonShape3D, ISkeletonModifier3D, Node, Node3D,
    PhysicsRayQueryParameters3D, PhysicsShapeQueryParameters3D, ProjectSettings, RigidBody3D, SkeletonModifier3D,
    SphereShape3D,
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

struct Shared { world: *mut NmWorld, scale: f32, at: Vector3, refs: i32 }

thread_local! {
    static BASE: RefCell<Option<*mut NmBuild>> = const { RefCell::new(None) };
    static SHARED: RefCell<Vec<Shared>> = const { RefCell::new(Vec::new()) };
}
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
        *b = Some(build);
        Some(build)
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
    let (mut pts, mut tris) = (Vec::<f32>::new(), Vec::<u32>::new());
    let mut seen = Vec::new();
    for h in hits.iter_shared() {
        let Some(body) = h.get("collider").and_then(|v| v.try_to::<Gd<Node>>().ok()) else { continue };
        if body.is_class("CharacterBody3D") || body.is_class("RigidBody3D") || seen.contains(&body.instance_id()) { continue }
        seen.push(body.instance_id());
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
        }
    }
    if tris.is_empty() { return 0 }
    check(unsafe { nm_world_add_mesh(world, pts.as_ptr(), (pts.len() / 3) as i32, tris.as_ptr(), (tris.len() / 3) as i32, std::ptr::null()) }, "level");
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
    build: *mut NmBuild,
    world: *mut NmWorld,
    agent: *mut NmAgent,
    rig: *mut NmRig,
    bones: Vec<i32>,
    out: Vec<f32>,
    scale: f32,
    clock: f32,
    told: bool,
    blend: f32,
    last: Vec<Quaternion>,
    last_hips: Vector3,
    hips_world: Transform3D,
}

#[godot_api]
impl ISkeletonModifier3D for DeliriumRagdoll {
    fn init(base: Base<SkeletonModifier3D>) -> Self {
        Self {
            base, character: None, get_up_blend: 0.4, get_up_after: 0.0, build: std::ptr::null_mut(), world: std::ptr::null_mut(),
            agent: std::ptr::null_mut(), rig: std::ptr::null_mut(), bones: Vec::new(), out: Vec::new(), scale: 1.0, clock: 0.0,
            told: false, blend: 0.0, last: Vec::new(), last_hips: Vector3::ZERO, hips_world: Transform3D::IDENTITY,
        }
    }

    fn process_modification_with_delta(&mut self, delta: f64) {
        self.step(delta as f32);
    }

    fn exit_tree(&mut self) {
        self.release();
    }
}

#[godot_api]
impl DeliriumRagdoll {
    #[signal]
    fn lying_still(face_up: bool, facing: Vector3);

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
    fn high_fall(&mut self) -> bool {
        self.take() && check(unsafe { nm_agent_task_high_fall(self.agent, self.world, std::ptr::null(), 0) }, "high_fall")
    }

    #[func]
    fn kill(&mut self) -> bool {
        self.take() && check(unsafe { nm_agent_set_dead(self.agent, 1) }, "kill") && check(unsafe { nm_agent_task_relax(self.agent, -1.0, -1.0) }, "kill")
    }

    #[func]
    fn sever(&mut self, point: Vector3, impulse: Vector3) -> bool {
        if !self.take() { return false }
        let at = nm(point);
        let part = unsafe { nm_agent_nearest_part(self.agent, at.as_ptr()) };
        if part <= 0 || unsafe { nm_agent_is_severed(self.agent, part) } == 1 { return false }
        check(unsafe { nm_agent_sever(self.agent, self.world, part, at.as_ptr(), nm(impulse).as_ptr()) }, "sever")
    }

    #[func]
    fn is_ragdolled(&self) -> bool { !self.agent.is_null() }

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
}

impl DeliriumRagdoll {
    fn owner(&self) -> Option<Gd<Node3D>> {
        self.character.clone().or_else(|| self.base().get_skeleton()?.get_owner()?.try_cast::<Node3D>().ok())
    }

    fn ground(&self, at: Vector3) -> f32 {
        let Some(mut space) = self.base().get_world_3d().and_then(|w| w.get_direct_space_state()) else { return at.y };
        let Some(mut q) = PhysicsRayQueryParameters3D::create(at, at - Vector3::UP * 5.0) else { return at.y };
        if let Some(ch) = self.owner().and_then(|o| o.try_cast::<CharacterBody3D>().ok()) {
            let mut ex = Array::new();
            ex.push(ch.get_rid());
            q.set_exclude(&ex);
        }
        let hit = space.intersect_ray(&q);
        hit.get("position").and_then(|v| v.try_to::<Vector3>().ok()).map_or(at.y, |p| p.y)
    }

    fn take(&mut self) -> bool {
        if !self.agent.is_null() { return true }
        let Some(base) = base() else { return false };
        let Some(s) = self.base().get_skeleton() else {
            godot_warn!("Delirium: put DeliriumRagdoll under a Skeleton3D");
            return false;
        };
        let all = s.get_bone_count();
        let guess: Vec<i32> = (0..all).map(|b| unsafe { nm_rig_part_for_name(c(&s.get_bone_name(b).to_string()).as_ptr()) }).collect();
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
        if setting_b("adaptive", true) {
            let kind = (0..all).map(|b| s.get_bone_name(b).to_string()).collect::<Vec<_>>().join("|") + &format!("@{}", self.scale);
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
                        let t = unsafe { nm_build_fit_sized(b as *const NmBuild, j.as_ptr(), mode) };
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
        let (world, fresh) = SHARED.with_borrow_mut(|sh| {
            if let Some(w) = sh.iter_mut().find(|w| w.scale == scale && (w.at.y - floor).abs() < 0.5 * scale && w.at.distance_to(spot) < 5.0 * scale) {
                w.refs += 1;
                return (w.world, false);
            }
            let world = nm_world_create();
            check(unsafe { nm_world_set_scale(world, scale) }, "world_set_scale");
            sh.push(Shared { world, scale, at: spot, refs: 1 });
            (world, true)
        });
        self.world = world;
        if fresh {
            let me = self.base().clone().upcast::<Node3D>();
            let tris = level(&me, hips_now, 15.0, self.world);
            godot_print!("Delirium: {tris} level triangles");
            let f = nm(spot);
            check(unsafe { nm_world_add_floor(self.world, f[0], f[1], f[2], 100.0 * scale) }, "world_add_floor");
        }
        let r = (g * s.get_bone_global_rest(hips)).origin;
        let start = nm(Vector3::new(r.x, floor, r.z));
        self.agent = unsafe { nm_agent_create(self.build, start.as_ptr(), 0.0, rand_seed()) };
        if self.agent.is_null() {
            godot_warn!("Delirium: {}", last_error());
            self.release();
            return false;
        }
        let strength = setting_f("strength", 1.0);
        if strength != 1.0 { check(unsafe { nm_agent_set_strength(self.agent, strength) }, "strength"); }
        self.rig = unsafe { nm_rig_create(self.agent, n as i32, parents.as_ptr(), rest.as_ptr(), parts.as_ptr()) };
        if self.rig.is_null() {
            godot_warn!("Delirium: {}", last_error());
            self.release();
            return false;
        }
        check(unsafe { nm_rig_take(self.rig, self.agent, now.as_ptr(), std::ptr::null(), 0.0) }, "rig_take");
        let owner = self.owner();
        let vel = owner.clone().and_then(|o| o.try_cast::<CharacterBody3D>().ok()).map(|c| c.get_velocity())
            .or_else(|| owner.and_then(|o| o.try_cast::<RigidBody3D>().ok()).map(|r| r.get_linear_velocity()))
            .unwrap_or(Vector3::ZERO);
        if vel != Vector3::ZERO { check(unsafe { nm_agent_add_velocity(self.agent, nm(vel).as_ptr()) }, "add_velocity"); }
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
        self.clock = 0.0;
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
        if self.agent.is_null() { return }
        self.clock = (self.clock + seconds).min(4.0 / 60.0);
        while self.clock >= 1.0 / 60.0 {
            self.clock -= 1.0 / 60.0;
            check(unsafe { nm_agent_step(self.agent, self.world, 1.0 / 60.0) }, "step");
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

    fn release(&mut self) {
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
