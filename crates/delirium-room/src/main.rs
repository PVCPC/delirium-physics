use bevy::camera::visibility::RenderLayers;
use bevy::camera::ClearColorConfig;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
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

mod getup;
mod map;
mod hero;
mod world;

const WHO: [&str; 1] = ["ours"];
const SIDES: usize = WHO.len();

const CHEST: usize = 9;
const HEAD: usize = 12;
const THIGH_L: usize = 1;
const THIGH_R: usize = 4;

const BEHAVIOURS: [&str; 18] = ["balance", "drunk", "windmill", "pedal", "writhe", "rollup", "catchfall", "highfall", "stairs", "shot",
                                "flinch", "brace", "look", "point", "lean", "relax", "stayup", "STOP all"];
const POKE: f32 = 8.0;
const WEAPONS: [(&str, &str, f32); 6] = [("pistol", "PISTOL", 50.0), ("deagle", "DEAGLE", 50.0), ("smg", "MP5", 50.0),
                                         ("ak47", "AK47", 50.0), ("sniper", "SNIPERRIFLE", 50.0), ("shotgun", "SHOTGUN", 50.0)];

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
    cam: Option<[f32; 3]>,
    look_z: f32,
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
    lean_stop: Option<f32>,
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
    eye: Vec3,
    hover: Option<(Vec2, String)>,
    crowd: Vec<(Agent, msg::State)>,
    group: delirium::ragdoll::Group,
    car: Option<(Vec3, Vec3, f32)>,
    ice: Option<(usize, [f32; 2])>,
    click_shoots: bool,
    grab: Option<Grab>,
    auto_balance: bool,
    hero: Option<hero::Hero>,
    agility: u8,
}

struct Grab {
    part: usize,
    local: Vec3,
    plane: (Vec3, Vec3),
    target: Vec3,
    press: (Vec2, Vec3, Vec3),
    pulling: bool,
}

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
        self.crowd.clear();
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
                let down = !self.tasks && self.health >= 50.0 && now[HEAD].translation.z - pelvis.z < 0.3 && pelvis.distance(self.last[i]) < 0.2 * dt.max(1e-3);
                self.still[i] = if down { self.still[i] + dt } else { 0.0 };
                if self.still[i] >= getup::DOWN_SECS {
                    self.still[i] = 0.0;
                    self.rises[i] = Some(clips.start(now));
                    self.say(format!("{}  getting up (the game's get_up_fast; the viewer's, not the game's task)", WHO[i]));
                }
            }
            self.last[i] = pelvis;
        }
        self.getup = Some(clips);
    }

    fn say(&mut self, s: String) {
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
        self.car = Some((s.p(0).with_z(0.75) + left * 4.0, -left * 8.0, 1.5));
        self.held = false;
        self.say("ours  car: 8 m/s from his left (ours only)".into());
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
        let _ = msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, "highFall", &[("start", Value::Bool(true))]);
        self.held = false;
        self.say("ours  fall: from 3 m, highFall".into());
    }

    fn toggle_hero(&mut self) {
        if self.hero.take().is_some() {
            return self.say("hero  out".into());
        }
        match self.getup.as_ref().filter(|c| c.hero.is_some()) {
            Some(c) => {
                let n = hero::Hero::new(c, self.agent.doll.com().0, self.ours().fwd);
                self.say(format!("hero  in at {}: WASD walk, Shift run, Ctrl sprint, left click punch", v3(n.feet)));
                self.hero = Some(n);
            }
            None => self.say("hero  needs the game's files (playerped.rpf's player.xft)".into()),
        }
    }

    fn play_hero(&mut self, dt: f32, mv: Vec3, gait: usize) {
        let (Some(n), Some(clips)) = (self.hero.as_mut(), self.getup.as_ref()) else { return };
        let up = !self.agent.fallen && self.rises[0].is_none();
        let pelvis = self.agent.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
        let range = self.weapons.iter().find(|w| w.name == "UNARMED").map_or(1.0, |w| w.range);
        let hit = n.update(clips, &self.world, dt, mv, gait, up.then_some(pelvis), range, &caps_of(&self.build, &self.agent.doll));
        let Some(hit) = hit else { return };
        let head = n.head();
        let ours = self.ours();
        let now = self.steps as f32 / 60.0;
        let imp = |k: usize, at: Vec3, j: Vec3| format!("applyImpulse start=true partIndex={k} hitPoint={} impulse={}", v3(at), v3(j));
        let (task, mut o) = match hit {
            hero::Hit::Swing(dmg, at, j) => {
                self.health = (self.health - dmg).max(1.0);
                let k = self.agent.doll.nearest(&self.build, at);
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
        let what = if matches!(hit, hero::Hit::Bump(_)) { "runs into Homer" } else { "punch lands" };
        self.say(format!("hero  {what}: {task} (health {:.0})", self.health));
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
            "STOP all" => vec!["stopAllBehaviours".into()],
            _ => vec![],
        }
    }

    fn fire(&mut self, part: usize, aim: Option<(Vec3, Vec3, Vec3)>, fatal: bool) {
        (self.rises, self.still) = ([None, None], [0.0; 2]);
        let (label, name, _) = WEAPONS[self.weapon];
        let hit = self.weapons.iter().find(|w| w.name == name).map(|w| {
            use delirium_data::ped;
            let mut h = ped::Health { health: self.health, ..Default::default() };
            let hit = ped::Hit::new(w.damage, w, delirium_data::nm_asset::PART_BONES[part], ped::Attacker::Player { sure: false });
            let o = ped::damage(&mut h, &hit, &ped::Victim::default(), &mut self.rand);
            self.health = h.health;
            (o.lost, o.flags & ped::INSTANT != 0)
        });
        if fatal {
            self.health = 0.0;
        }
        let state = if self.health < 50.0 { "fatally injured" } else if self.health < 100.0 { "injured (dying)" } else { "standing" };
        self.say(format!("room  shot ({label}) at part {part} {}: -{:.1}{} -> health {:.1} ({state})",
                         delirium_data::nm_asset::PARTS.get(part).unwrap_or(&"?"), hit.map_or(0.0, |h| h.0), if hit.is_some_and(|h| h.1) { " (instant kill)" } else { "" },
                         self.health));
        if self.tasks {
            (self.exit, self.nm_t, self.failed) = (SHOT_EXIT, [Some(0.0); 2], [false; 2]);
        }
        let ours = self.ours();
        let w = self.weapons.iter().find(|w| w.name == name);
        let set = w.map_or("*", |w| delirium::nm::weapon_set(w, if self.weak { 2 } else { 3 }));
        let (at, d, n) = aim.unwrap_or((ours.p(part) + ours.fwd * 0.1, -ours.fwd, ours.fwd));
        let vel = shottask::bullet(at, at - d.normalize() * 3.0, w.map_or(WEAPONS[self.weapon].2, |w| w.force));
        let mut o = vec![format!("applyImpulse partIndex={part} hitPoint={} impulse={}", v3(at), v3(vel))];
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

    fn poke(&mut self, part: usize, hit: Vec3, dir: Vec3) {
        let line = |at: Vec3, d: Vec3| vec![format!("applyImpulse start=true partIndex={part} hitPoint={} impulse={}", v3(at), v3(d.normalize() * POKE))];
        let o = line(hit, dir);
        self.say(format!("room  poke {} ({POKE} N s)", delirium_data::nm_asset::PARTS.get(part).unwrap_or(&"?")));
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

    fn button(&mut self, b: &str) {
        (self.rises, self.still, self.nm_t) = ([None, None], [0.0; 2], [None, None]);
        if b == "RESET pose" {
            (self.health, self.lean_stop, self.shot_task) = (200.0, None, None);
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

    fn throw(&mut self, kind: usize) {
        let (label, _, kg, speed) = world::KINDS[kind];
        let ours = self.ours();
        let (from, vel) = if speed > 0.0 {
            let from = self.eye.lerp(ours.p(CHEST), 0.15);
            let d = ours.p(CHEST) - from;
            let t = d.length() / speed;
            (from, d / t + Vec3::Z * 4.905 * t)
        } else {
            (ours.p(HEAD) + Vec3::Z * 2.0, Vec3::ZERO)
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
        let sides = [Some(ours)];
        for (i, caps) in sides.into_iter().enumerate() {
            let Some(caps) = caps else { continue };
            for (j, k, at, imp) in self.room.step(i, &caps, 1.0 / 60.0) {
                if !self.room.objs[j].hit[i] && imp.length() > 1.0 {
                    self.room.objs[j].hit[i] = true;
                    self.say(format!("{}  {} hits {} ({:.1} N s)", WHO[i], world::KINDS[self.room.objs[j].kind].0,
                                     delirium_data::nm_asset::PARTS.get(k).unwrap_or(&"?"), imp.length()));
                }
                self.held = false;
                let ps = [("start", Value::Bool(true)), ("partIndex", Value::Int(k as i32)), ("hitPoint", Value::Vec3(at)), ("impulse", Value::Vec3(imp))];
                let _ = msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, "applyImpulse", &ps);
            }
        }
    }

    fn send(&mut self, line: &str) {
        let mut w = line.split_whitespace();
        let Some(name) = w.next() else { return };
        if name == "?" {
            self.say(msg::MESSAGES.iter().map(|m| m.name).collect::<Vec<_>>().join(" "));
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
        match msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, name, &ps) {
            Ok(unused) => self.say(format!("ours  MSG {short}{}", if unused > 0 { format!("  ({unused} kept, not used by the port)") } else { String::new() })),
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
        msg::MESSAGES.iter().map(|m| m.name.to_string()).filter(|n| n.starts_with(last)).collect()
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
    let dir = std::path::Path::new(game);
    let (Ok(key), Ok(data)) = (getup::key(dir), std::fs::read(dir.join(rpf))) else { return vec![None; paths.len()] };
    let toc = delirium_data::rpf::read_toc(&data, &key).unwrap_or_default();
    paths.iter().map(|p| toc.iter().find(|e| e.path.eq_ignore_ascii_case(p)).and_then(|e| delirium_data::rpf::read(&data, e).ok())).collect()
}

static NM: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
fn nm() -> usize {
    NM.get().copied().unwrap_or(delirium::nmrs::HOMER)
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
    let [wi, mats, personality]: [Option<String>; 3] = game_dir.as_deref()
        .map(|g| common(g, &["data/weaponinfo.xml", "data/materials/materials.dat", "data/pedpersonality.dat"]).try_into().unwrap()).unwrap_or_default();
    let (model, xrt) = if a.iter().any(|x| x == "--marge") { ("f_y_pmanhat_01", "wilma") } else { ("m_y_pmanhat_01", "fred") };
    let _ = NM.set(if xrt == "wilma" { delirium::nmrs::MARGE } else { delirium::nmrs::HOMER });
    let agility = personality.as_deref().and_then(|t| t.lines().map(|l| l.split(',').map(str::trim).collect::<Vec<_>>())
        .find(|c| c.len() > 7 && c[0].eq_ignore_ascii_case(model)).and_then(|c| c[7].parse::<u8>().ok())).unwrap_or(3);
    let weapons: Vec<Weapon> = wi.map(|t| delirium_data::weapons::read(&t)).unwrap_or_default();
    let surfaces: Vec<(String, f32, f32)> = mats.as_deref().map(|t| {
        let f = delirium_data::vehicles::read_friction(t);
        *delirium::ragdoll::SURFACES.write().unwrap() = f.clone();
        t.lines().filter(|l| l.starts_with(|c: char| c.is_ascii_alphabetic())).zip(f).map(|(l, [fr, el])| (l.split_whitespace().next().unwrap_or("").to_string(), fr, el)).collect()
    }).unwrap_or_default();
    let room = world::Room::new(surfaces);
    let pos: Vec<String> = a.iter().skip(1).take_while(|x| !x.starts_with("--")).cloned().collect();
    let arg = |i: usize, env: &str| pos.get(i - 1).cloned().or_else(|| std::env::var(env).ok());
    let from_game = |rpf: &str, p: &str| game_dir.as_deref().and_then(|g| archive(g, rpf, &[p]).pop().flatten());
    let bpath = arg(2, "NM_BUILD");
    if game_dir.is_none() && bpath.is_none() && arg(1, "XRT").is_none() {
        pick_game(&args);
    }
    let Some(d) = arg(1, "XRT").map(|x| std::fs::read(&x).unwrap_or_else(|e| panic!("{x}: {e}")))
        .or_else(|| from_game("xbox360.rpf", &format!("data/naturalmotion/{xrt}.xrt"))).filter(|_| bpath.is_some() || game_dir.is_some()) else {
        eprintln!("usage: delirium-room <fred.xrt> <build.txt> [taskparams.txt] [--game <game dir>] [--script file]   (or env XRT, NM_BUILD, TP;");
        eprintln!("       with --game and no XRT / NM_BUILD / TP, fred.xrt, Homer's body and taskparams.txt come out of the game's files)");
        std::process::exit(2);
    };
    delirium::nmrs::load(nm(), &d);
    let (build, frames, getup) = match (&bpath, game_dir.as_deref()) {
        (Some(p), _) => {
            let (b, f) = Build::undump(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}")));
            let getup = game_dir.as_deref().and_then(|g| getup::Clips::load(g, &b).map_err(|e| eprintln!("get-up clips: {e}")).ok());
            (b, f, getup)
        }
        (None, Some(g)) => {
            let mut b = getup::homer(g, model, nm()).unwrap_or_else(|e| panic!("Homer from the game's files: {e}"));
            let clips = getup::Clips::load(g, &b).unwrap_or_else(|e| panic!("the game's clips: {e}"));
            let f = clips.stand_up(&mut b);
            (b, f, Some(clips))
        }
        (None, None) => unreachable!(),
    };
    if let Ok(p) = std::env::var("NM_DUMP") {
        let _ = std::fs::write(p, build.dump(&frames));
    }
    let floor = build.parts.iter().zip(&frames).map(|(p, f)| f.translation.z - p.radius - p.half).fold(f32::MAX, f32::min);
    let shift = -frames[0].translation.with_z(floor);
    let frames: Vec<Transform> = frames.iter().map(|f| Transform { translation: f.translation + shift, ..*f }).collect();
    let mut world = Collision::default();
    let concrete = room.surfaces.iter().position(|s| s.0 == "CONCRETE").unwrap_or(0) as u8;
    world.add_floor(0.0, 0.0, 0.0, 50.0, concrete);
    let (p, t, m) = room.triangles();
    world.add(&p, &t, &m);
    let tp = arg(3, "TP").and_then(|p| std::fs::read_to_string(p).ok())
        .or_else(|| from_game("common.rpf", "data/taskparams.txt").map(|d| String::from_utf8_lossy(&d).into_owned())).map_or_else(TaskParams::default, |t| delirium_data::taskparams::read(&t));
    let agent = Agent::new(&build, Ragdoll::new(&build, &frames, Vec3::ZERO), Quat::IDENTITY, 0.3);
    let mut sim = Sim { build, frames, world, tp, agent, msgs: msg::State::default(), paused: false, speed: 1.0, acc: 0.0, steps: 0, arrows: false,
                        shot_task: None, rand: Default::default(),
                        log: VecDeque::new(), console: String::new(), script, typing: false,
                        weapons, weapon: 0, weak: false, sel: 0, over_ui: false, held: true,   show_ui: true, sens: 0.5, rec, health: 200.0, lean_stop: None, cam: None, look_z: 0.8,
                        getup, rises: [None, None], still: [0.0; 2], tasks: false, exit: SHOT_EXIT, nm_t: [None, None], failed: [false; 2], last: [Vec3::ZERO; 2], room, look: true, avoid: false, eye: Vec3::new(0.0, -4.0, 1.5), hover: None,  crowd: vec![], group: Default::default(), car: None, ice: None,   click_shoots: false,
                        grab: None, auto_balance: true, hero: None, agility };
    sim.auto_balance = sim.script.is_empty();
    if let Some(n) = headless_steps {
        return headless(sim, n);
    }
    sim.say(format!("weaponinfo.xml: {} weapons{}", sim.weapons.len(), if sim.weapons.is_empty() { " (--game <dir> reads it; impulses 50 N s)" } else { "" }));

    let systems = (keys, console, mouse, scripted, step, bodies, props, car_mesh, draw).chain();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Delirium debug room - the game".into(), resolution: (1600, 900).into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(bevy::winit::WinitSettings { focused_mode: bevy::winit::UpdateMode::Continuous, unfocused_mode: bevy::winit::UpdateMode::Continuous })
        .insert_resource(ClearColor(Color::srgb(0.78, 0.79, 0.81)))
        .init_gizmo_group::<OursGizmos>()
        .insert_non_send_resource(sim)
        .add_systems(Startup, setup)
        .add_systems(Update, systems)
        .add_systems(EguiPrimaryContextPass, ui);
    app.run();
}

const GAME_PATH: &str = "game_path.txt";

fn is_game(dir: &str) -> bool {
    std::path::Path::new(dir).join("default.xex").is_file()
}

fn is_iso(path: &str) -> bool {
    path.to_lowercase().ends_with(".iso") && std::path::Path::new(path).is_file()
}

fn from_iso(iso: &str, say: &dyn Fn(u64, u64)) -> Result<String, String> {
    let disc = delirium_data::disc::Disc::open(std::path::Path::new(iso))?;
    let out = std::path::absolute("game").map_err(|e| e.to_string())?;
    let mut want: Vec<&str> = disc.paths().into_iter()
        .filter(|p| *p == "default.xex" || (!p.contains('/') && p.ends_with(".rpf")) || (p.starts_with("xbox360/") && p.ends_with(".img")))
        .collect();
    want.sort();
    if !want.contains(&"default.xex") {
        return Err(format!("{iso}: no default.xex on it (not the game's disc?)"));
    }
    let todo: Vec<(&str, u64)> = want.iter().map(|p| (*p, disc.len(p).unwrap_or(0)))
        .filter(|(p, len)| !out.join(p).metadata().is_ok_and(|m| m.len() == *len)).collect();
    let (all, mut done) = (todo.iter().map(|t| t.1).sum::<u64>(), 0);
    for (p, len) in todo {
        let to = out.join(p);
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
                ui.heading(egui::RichText::new("Delirium room").size(30.0));
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
                        let big = if pick.hover { "Let go to use it" } else { "Drag your game .iso file here" };
                        ui.label(egui::RichText::new(big).size(28.0).strong().color(line));
                        ui.label("Your own copy of the game: its disc image (.iso),");
                        ui.label("or the game folder (the one with default.xex in it).");
                    });
                });
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Or paste where it is:").small());
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 640.0).max(0.0) / 2.0);
                    let field = ui.add_sized([500.0, 32.0], egui::TextEdit::singleline(&mut pick.text).hint_text(r"e.g. D:\Games\the game.iso"));
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
            } else if is_game(&g) {
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
            primary_window: Some(Window { title: "Delirium room - drag your game .iso here".into(), resolution: (900, 460).into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .init_resource::<Pick>()
        .add_systems(Startup, |mut commands: Commands| { commands.spawn(Camera2d); })
        .add_systems(EguiPrimaryContextPass, ui)
        .run();
    let Some(g) = PICKED.get() else { std::process::exit(1) };
    let status = std::process::Command::new(&args[0]).args(&args[1..]).arg("--game").arg(g).status();
    std::process::exit(status.ok().and_then(|s| s.code()).unwrap_or(1));
}

impl Sim {
    fn split(&self) -> bool {
        false
    }
}

#[derive(Component)]
struct OursCam;
#[derive(Component)]
struct PartMesh {
    k: usize,
    crowd: Option<usize>,
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
    commands.spawn((Mesh3d(meshes.add(Plane3d::new(Vec3::Z, Vec2::splat(30.0)))),
                    MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.58, 0.59, 0.61), perceptual_roughness: 1.0, ..default() })), RoomScenery));
    for &(c, h, row) in &sim.room.blocks {
        let brick = sim.room.surfaces.get(row as usize).is_some_and(|s| s.0 == "BRICK_WALL");
        let color = if brick { Color::srgb(0.8, 0.42, 0.33) } else { Color::srgb(0.68, 0.68, 0.7) };
        commands.spawn((Mesh3d(meshes.add(Cuboid::from_size(h * 2.0))), MeshMaterial3d(mats.add(StandardMaterial { base_color: color, perceptual_roughness: 0.9, ..default() })),
                        Transform::from_translation(c), RenderLayers::layer(1), RoomScenery));
    }
    let sides = [(false, 1)];
    let homer = sides.into_iter().map(|(g, l)| (g, l, None)).chain((0..4).map(|c| (false, 1, Some(c))));
    for (game, layer, crowd) in homer {
        let _ = game;
        for (k, p) in sim.build.parts.iter().enumerate() {
            let name = delirium_data::nm_asset::PARTS.get(k).copied().unwrap_or("");
            let c = if name.ends_with("Left") { Color::srgba(0.85, 0.3, 0.25, 0.55) } else if name.ends_with("Right") { Color::srgba(0.3, 0.45, 0.9, 0.55) }
                    else { Color::srgba(0.72, 0.72, 0.74, 0.6) };
            commands.spawn((Mesh3d(meshes.add(Capsule3d::new(p.radius, p.half * 2.0))),
                            MeshMaterial3d(mats.add(StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.6, ..default() })),
                            Transform::default(), Visibility::Hidden, RenderLayers::layer(layer), PartMesh { k,  crowd }));
        }
    }
    for (k, p) in sim.getup.as_ref().and_then(|c| c.hero.as_ref()).map_or(&[][..], |r| &r.build.parts[..]).iter().enumerate() {
        commands.spawn((Mesh3d(meshes.add(Capsule3d::new(p.radius, p.half * 2.0))),
                        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgba(0.3, 0.75, 0.4, 0.6), alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.6, ..default() })),
                        Transform::default(), Visibility::Hidden, RenderLayers::layer(1), HeroMesh(k)));
    }
    let (cfg, _) = gizmos.config_mut::<OursGizmos>();
    (cfg.render_layers, cfg.depth_bias) = (RenderLayers::layer(1), -1.0);
}

#[derive(Component)]
struct HeroMesh(usize);

fn bodies(sim: NonSend<Sim>, mut q: Query<(&PartMesh, &mut Transform, &mut Visibility), Without<HeroMesh>>,
          mut hero: Query<(&HeroMesh, &mut Transform, &mut Visibility), Without<PartMesh>>) {
    for (m, mut t, mut v) in &mut hero {
        match sim.hero.as_ref().and_then(|n| n.caps.get(m.0)) {
            Some(c) => (*t, *v) = (*c, Visibility::Visible),
            None => *v = Visibility::Hidden,
        }
    }
    let up = |i: usize| sim.getup.as_ref().zip(sim.rises[i].as_ref()).and_then(|(c, r)| c.frames(r))
        .map(|f| f.into_iter().enumerate().map(|(k, f)| f.with_rotation(f.rotation * delirium::nmdbm::rage_link(nm(), k))).collect::<Vec<_>>());
    let fr = up(0).unwrap_or_else(|| sim.agent.doll.capsules().collect());
    let crowd: Vec<Vec<Transform>> = sim.crowd.iter().map(|(a, _)| a.doll.capsules().collect()).collect();
    for (p, mut t, mut v) in &mut q {
        let at = match p.crowd {
            Some(c) => crowd.get(c).and_then(|f| f.get(p.k).copied()),
            None => fr.get(p.k).copied(),
        };
        match at {
            Some(a) => (*t, *v) = (a, Visibility::Visible),
            None => *v = Visibility::Hidden,
        }
    }
}

#[derive(Component)]
struct RoomScenery;

const STAND: f32 = 0.95;

#[derive(Component)]
struct PropMesh(usize, usize);

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
            *t = sim.room.obj(p.1, p.0).0;
        }
    }
    for (j, o) in objs.iter().enumerate().skip(*made) {
        let (_, s, _, _) = world::KINDS[o.kind];
        let mesh = if o.kind == 0 { meshes.add(Sphere::new(s)) } else { meshes.add(Cuboid::from_length(s * 2.0)) };
        let color = if o.kind == 0 { Color::srgb(0.15, 0.15, 0.17) } else { Color::srgb(0.62, 0.42, 0.24) };
        let mat = mats.add(StandardMaterial { base_color: color, perceptual_roughness: 0.8, ..default() });
        for i in 0..SIDES {
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), sim.room.obj(i, j).0, RenderLayers::layer(1 + i), PropMesh(j, i)));
        }
    }
    *made = objs.len();
}

#[derive(Component)]
struct CarMesh;

fn car_mesh(sim: NonSend<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
            mut q: Query<(&mut Transform, &mut Visibility), With<CarMesh>>) {
    let Ok((mut t, mut v)) = q.single_mut() else {
        let mat = mats.add(StandardMaterial { base_color: Color::srgb(0.55, 0.1, 0.1), perceptual_roughness: 0.5, ..default() });
        commands.spawn((Mesh3d(meshes.add(Cuboid::new(1.8, 4.6, 1.4))), MeshMaterial3d(mat), Transform::default(), Visibility::Hidden, RenderLayers::layer(1), CarMesh));
        return;
    };
    *v = match sim.car {
        Some((c, vel, _)) => {
            *t = Transform::from_translation(c).with_rotation(Quat::from_rotation_arc(Vec3::Y, vel.normalize()));
            Visibility::Visible
        }
        None => Visibility::Hidden,
    };
}

fn keys(mut sim: NonSendMut<Sim>, k: Res<ButtonInput<KeyCode>>) {
    if sim.typing {
        return;
    }
    if k.just_pressed(KeyCode::Space) {
        sim.paused = !sim.paused;
    }
    if k.just_pressed(KeyCode::Period) {
        (sim.paused, sim.acc) = (true, 1.0 / 60.0);
    }
    if k.just_pressed(KeyCode::BracketLeft) {
        sim.speed = (sim.speed * 0.5).max(1.0 / 32.0);
    }
    if k.just_pressed(KeyCode::BracketRight) {
        sim.speed = (sim.speed * 2.0).min(4.0);
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

fn console(mut sim: NonSendMut<Sim>, mut typed: MessageReader<KeyboardInput>) {
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

fn mouse(mut sim: NonSendMut<Sim>, buttons: Res<ButtonInput<MouseButton>>, mut last: Local<Option<Vec2>>, scroll: Res<AccumulatedMouseScroll>,
         keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, windows: Query<&Window>,
         mut cam: Query<(&mut Transform, &mut Orbit, &Camera, &GlobalTransform), With<OursCam>>,
         ) {
    let Ok((mut t, mut o, camera, gt)) = cam.single_mut() else { return };
    let cur = windows.single().ok().and_then(|w| w.cursor_position());
    let delta = match (cur, *last) {
        (Some(c), Some(l)) => (c - l) * sim.sens,
        _ => Vec2::ZERO,
    };
    *last = cur;
    let wheel = match scroll.unit {
        bevy::input::mouse::MouseScrollUnit::Line => scroll.delta.y,
        bevy::input::mouse::MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    }.clamp(-3.0, 3.0) * sim.sens;
    if let Some([yaw, pitch, dist]) = sim.cam.take() {
        (o.yaw, o.pitch, o.dist) = (yaw, pitch, dist);
    }
    let flat = Vec3::new(o.yaw.cos(), o.yaw.sin(), 0.0);
    let side = Vec3::Z.cross(flat);
    if !sim.over_ui {
        if buttons.pressed(MouseButton::Right) {
            o.yaw -= delta.x * 0.005;
            o.pitch = (o.pitch + delta.y * 0.005).clamp(-0.2, 1.5);
        }
        if buttons.pressed(MouseButton::Middle) && delta != Vec2::ZERO {
            let k = o.dist * 0.0015;
            o.target += side * delta.x * k + Vec3::Z * delta.y * k;
            o.follow = 3;
        }
        o.dist = (o.dist * (1.0 - wheel * 0.1)).clamp(0.3, 40.0);
    }
    if !sim.typing {
        let mut mv = Vec3::ZERO;
        for (k, d) in [(KeyCode::KeyW, -flat), (KeyCode::KeyS, flat), (KeyCode::KeyA, -side), (KeyCode::KeyD, side), (KeyCode::KeyE, Vec3::Z), (KeyCode::KeyQ, -Vec3::Z)] {
            if keys.pressed(k) {
                mv += d;
            }
        }
        if sim.hero.is_some() {
            let mut gait = if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) { 2 } else { shift_held(&keys) as usize };
            if mv == Vec3::ZERO {
                (mv, gait) = sim.hero.as_ref().map_or((mv, gait), |n| n.go);
            }
            let dt = if sim.paused || sim.held { 0.0 } else { time.delta_secs() * sim.speed };
            sim.play_hero(dt, mv.with_z(0.0).normalize_or_zero(), gait);
            mv = Vec3::ZERO;
        }
        if mv != Vec3::ZERO {
            o.target += mv * time.delta_secs() * 2.5;
            o.follow = 3;
        }
        if keys.just_pressed(KeyCode::KeyC) {
            o.follow = if o.follow == 0 { 3 } else { 0 };
            sim.say(format!("room  camera: {}", if o.follow == 0 { "each half follows its Homer" } else { "free (ours' half)" }));
        }
    }
    let k = (time.delta_secs() * 4.0).min(1.0);
    if o.follow == 0 {
        let at = sim.hero.as_ref().map_or_else(|| { let c = sim.agent.doll.com().0; c.with_z(c.z - STAND) }, |n| n.feet);
        o.target = o.target.lerp(at.with_z(at.z + sim.look_z), k);
    }
    let off = Vec3::new(o.yaw.cos() * o.pitch.cos(), o.yaw.sin() * o.pitch.cos(), o.pitch.sin()) * o.dist;
    let slide = (-off).cross(Vec3::Z).normalize_or_zero() * (o.dist * 0.12);
    *t = Transform::from_translation(o.target + off - slide).looking_at(o.target - slide, Vec3::Z);
    sim.eye = t.translation;
    let half = windows.single().map_or(0.0, |w| w.width() * if sim.split() { 0.5 } else { 1.0 });
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
    let click = buttons.just_pressed(MouseButton::Left) && !sim.over_ui;
    if let Some(n) = sim.hero.as_mut().filter(|_| click) {
        return n.punch();
    }
    if (buttons.just_pressed(MouseButton::Left) || fatal) && !sim.over_ui {
        let Some(cursor) = windows.single().ok().and_then(|w| w.cursor_position()) else { return };
        if sim.split() && windows.single().is_ok_and(|w| cursor.x > w.width() * 0.5) {
            return;
        }
        let Ok(ray) = camera.viewport_to_world(gt, cursor) else { return };
        let (from, dir) = (ray.origin, *ray.direction);
        if let Some((part, hit, normal)) = sim.agent.doll.probe(&sim.build, from, from + dir * 50.0) {
            if shift || sim.click_shoots {
                sim.fire(part, Some((hit, dir, normal)), fatal);
            } else if let Some(f) = { let f = sim.agent.doll.frames().nth(part); f } {
                let local = f.rotation.inverse() * (hit - f.translation);
                sim.grab = Some(Grab { part, local, plane: (hit, -dir), target: hit, press: (cursor, dir, hit), pulling: false });
            }
        }
    }
}

fn shift_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
}

fn scripted(mut sim: NonSendMut<Sim>, time: Res<Time>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    while sim.script.front().is_some_and(|l| l.0 <= time.elapsed_secs()) {
        let (_, line) = sim.script.pop_front().unwrap();
        let mut w = line.splitn(2, ' ');
        match (w.next().unwrap_or(""), w.next()) {
            ("!button", Some(b)) => sim.button(b),
            ("!hero", _) => sim.toggle_hero(),
            ("!punch", _) => if let Some(n) = sim.hero.as_mut() { n.punch() },
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
            ("!cam", Some(a)) => {
                let v: Vec<f32> = a.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                if let [yaw, pitch, dist, z] = v[..] {
                    (sim.cam, sim.look_z) = (Some([yaw, pitch, dist]), z);
                }
            }
            ("!hide", _) => sim.show_ui = !sim.show_ui,
            ("!screenshot", Some(f)) => {
                commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                    .observe(bevy::render::view::screenshot::save_to_disk(f.to_string()));
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
    fn tick(&mut self) {
        let hero = self.hero.as_ref().zip(self.getup.as_ref()).map_or(vec![], |(n, c)| n.capsules(c));
        let car: Vec<_> = self.car.iter().map(|&(c, v, _)| delirium::ragdoll::Obstacle { at: c, rot: Quat::from_rotation_arc(Vec3::Y, v.normalize()), centre: c,
            half: Vec3::new(0.9, 2.3, 0.7), vel: v, com: c, ..Default::default() }).collect();
        self.pull();
        if self.steps % 2 == 0 {
            self.shot_frame();
        }
        let t0 = std::time::Instant::now();
        let mut all: Vec<(&mut Agent, usize, &Build, &[_], &[Capsule])> = std::iter::once(&mut self.agent).chain(self.crowd.iter_mut().map(|c| &mut c.0)).enumerate()
            .map(|(i, a)| (a, i, &self.build, &car[..], &hero[..])).collect();
        Agent::step_group(&mut all, &self.world, 1.0 / 60.0, &mut self.group);
        if self.crowd.len() > 4 && self.steps % 300 == 299 {
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            self.say(format!("ours  grid: {} Homers, {ms:.2} ms a step", self.crowd.len() + 1));
        }
        if let Some((c, v, t)) = self.car.as_mut() {
            (*c, *t) = (*c + *v / 60.0, *t - 1.0 / 60.0);
            if *t <= 0.0 {
                self.car = None;
            }
        }
        self.steps += 1;
        self.objects();
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
}

fn step(mut sim: NonSendMut<Sim>, time: Res<Time>) {
    let ready = time.elapsed_secs() > 2.0;
    if sim.auto_balance && sim.held && ready {
        sim.auto_balance = false;
        sim.button("balance");
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
}

fn caps_of(build: &Build, d: &Ragdoll) -> Vec<Capsule> {
    d.capsules().zip(&build.parts).enumerate().map(|(k, (t, p))| {
        let a = t.rotation * Vec3::Y * p.half;
        (t.translation - a, t.translation + a, p.radius, d.motion(k).0, true)
    }).collect()
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
    for i in -20..=20 {
        let x = i as f32 * 0.5;
        let c = Color::srgba(0.35, 0.36, 0.4, 0.35);
        g.line(Vec3::new(x, -10.0, 0.002), Vec3::new(x, 10.0, 0.002), c);
        g.line(Vec3::new(-10.0, x, 0.002), Vec3::new(10.0, x, 0.002), c);
    }
    let doll = &sim.agent.doll;
    let up = |i: usize| sim.getup.as_ref().zip(sim.rises[i].as_ref()).and_then(|(c, r)| c.frames(r));
    let ours_up = up(0);
    let rising = ours_up.is_some();
    let fr: Vec<Transform> = ours_up.unwrap_or_else(|| doll.frames().collect());
    skeleton(&mut ours, &sim.build, |k| fr.get(k).copied(), |k| doll.touch[k] & 0x48 != 0);
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
    for (a, _) in &sim.crowd {
        let f: Vec<Transform> = a.doll.frames().collect();
        skeleton(&mut ours, &sim.build, |k| f.get(k).copied(), |k| a.doll.touch[k] & 0x48 != 0);
    }
    let (com, v) = doll.com();
    let (com, v) = if rising { (fr[0].translation, Vec3::ZERO) } else { (com, v) };
    ours.sphere(Isometry3d::from_translation(com), 0.04, Color::srgb(1.0, 0.3, 0.2));
    ours.arrow(com.with_z(0.01), (com + v * 0.3).with_z(0.01), Color::srgb(1.0, 0.5, 0.2));
    for (k, p) in sim.build.parts.iter().enumerate() {
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

fn skeleton<T: GizmoConfigGroup>(g: &mut Gizmos<T>, build: &Build, frame: impl Fn(usize) -> Option<Transform>, touching: impl Fn(usize) -> bool) {
    let joint = |k: usize| frame(k).map(|f| if build.parts[k].parent.is_some() { f.translation + f.rotation * build.parts[k].anchor } else { f.translation });
    for (k, p) in build.parts.iter().enumerate() {
        let (Some(f), Some(x)) = (frame(k), joint(k)) else { continue };
        let c = side_color(delirium_data::nm_asset::PARTS.get(k).unwrap_or(&""));
        if let Some(y) = p.parent.and_then(&joint) {
            g.line(x, y, Color::WHITE);
        }
        if p.parent.is_some() && !build.parts.iter().any(|c| c.parent == Some(k)) {
            let tip = f.translation * 2.0 - x;
            g.line(x, tip, Color::WHITE);
            g.sphere(Isometry3d::from_translation(tip), 0.015, c);
        }
        g.sphere(Isometry3d::from_translation(x), 0.022, c);
        if touching(k) {
            g.sphere(Isometry3d::from_translation(f.translation), 0.045, Color::srgb(1.0, 0.85, 0.1));
        }
    }
}

const RESET_TIP: &str = "Stand Homer back up (R)";
const DIM: egui::Color32 = egui::Color32::from_rgb(150, 150, 160);
const HI: egui::Color32 = egui::Color32::from_rgb(240, 200, 80);

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

fn ui(mut contexts: EguiContexts, mut sim: NonSendMut<Sim>) -> Result {
    let ctx = contexts.ctx_mut()?;
    ctx.style_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(17.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.0));
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.visuals.window_fill = egui::Color32::from_rgba_unmultiplied(20, 22, 26, 225);
    });
    if !sim.show_ui {
        egui::Area::new("hint".into()).anchor(egui::Align2::LEFT_TOP, [6.0, 6.0]).show(ctx, |ui| {
            ui.label(egui::RichText::new("H: show panels").monospace().color(egui::Color32::BLACK));
        });
        sim.over_ui = false;
        return Ok(());
    }
    let mono = |s: String| egui::RichText::new(s).monospace();
    let mut pressed: Option<String> = None;

    let w = ctx.content_rect().width();
    let bg = ctx.layer_painter(egui::LayerId::background());
    let font = egui::FontId::monospace(16.0);
    let alone = true;
    if alone {
        bg.text(egui::pos2(w * 0.5, 8.0), egui::Align2::CENTER_TOP, "OURS  (Delirium Physics)", font, egui::Color32::from_rgb(20, 40, 90));
    }

    egui::Window::new("Messages").anchor(egui::Align2::LEFT_BOTTOM, [8.0, -8.0]).default_size([640.0, 150.0]).resizable(true).collapsible(true).show(ctx, |ui| {
        ui.label(egui::RichText::new("MSG: a command sent to Delirium    FB: Delirium's feedback").small().color(DIM));
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).max_height(120.0).show(ui, |ui| {
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
            ui.label(egui::RichText::new("Enter: type a command (? lists them)    H: hide panels    every control: \"Controls\" in the room panel").small().color(DIM));
        }
    });

    egui::Window::new("Joints (muscle detail)").anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0]).default_open(false).resizable(false).collapsible(true).show(ctx, |ui| {
        let names: Vec<String> = delirium::nmrs::ASSET[nm()].get().map(|a| {
            let mut v: Vec<(u32, String)> = a.iter().map(|j| (j.index, j.name.clone())).collect();
            v.sort();
            v.into_iter().map(|j| j.1).collect()
        }).unwrap_or_default();
        let a = &sim.agent;
        let fr: Vec<Transform> = a.doll.frames().collect();
        let ch = a.character();
        ui.label(mono("joint        actual -> desired (deg)  drive str/damp".into()).color(DIM));
        egui::Grid::new("jt").striped(true).spacing([6.0, 2.0]).show(ui, |ui| {
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

    egui::Window::new("Delirium room").default_pos([8.0, 8.0]).default_width(340.0).resizable(false).collapsible(true).show(ctx, |ui| {
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
        let red = egui::Color32::from_rgb(150, 50, 40);
        let wide = |ui: &mut egui::Ui, text: String, fill: egui::Color32| ui.add_sized([104.0, 24.0], egui::Button::new(text).fill(fill));

        ui.label(egui::RichText::new(format!("Homer: {}", if state.is_empty() { "idle" } else { &state })).heading());
        ui.label(format!("height {:.2} m    speed {:.2} m/s    health {:.0} / 200", com.z, v.length(), sim.health));
        if sim.paused {
            ui.label(egui::RichText::new("PAUSED (Space to go on)").color(HI));
        }
        let held = sim.held;
        if held {
            ui.label(egui::RichText::new("Pose held: pick a behaviour, or STOP").color(HI));
        }
        if sim.health < 100.0 {
            ui.label(egui::RichText::new("Dying: shots now relax him").color(HI));
        }
        ui.separator();

        egui::CollapsingHeader::new(egui::RichText::new("Behaviours").strong()).default_open(true).show(ui, |ui| {
            egui::Grid::new("beh").spacing([4.0, 4.0]).show(ui, |ui| {
                for (i, b) in BEHAVIOURS.iter().filter(|b| !b.starts_with("STOP")).enumerate() {
                    if wide(ui, b.to_string(), egui::Color32::from_rgb(45, 60, 90)).clicked() {
                        pressed = Some(b.to_string());
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            ui.horizontal(|ui| {
                if wide(ui, "STOP all".into(), red).on_hover_text("Stop every behaviour (X)").clicked() {
                    pressed = Some("STOP all".into());
                }
                if wide(ui, "Reset pose".into(), red).on_hover_text(RESET_TIP).clicked() {
                    pressed = Some("RESET pose".into());
                }
            });
        });

        egui::CollapsingHeader::new(egui::RichText::new("Shoot").strong()).default_open(true).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Left click");
                for (on, name, key) in [(false, "pokes / drags", "poke"), (true, "shoots", "shoot")] {
                    if ui.selectable_label(sim.click_shoots == on, name).clicked() {
                        pressed = Some(format!("#click {key}"));
                    }
                }
            });
            ui.label(egui::RichText::new(if sim.click_shoots { "Click a body part to shoot it.  Shift + right click: a fatal shot." }
                                         else { "Shift + click a body part to shoot it.  Shift + right click: a fatal shot." }).small().color(DIM));
            ui.horizontal_wrapped(|ui| {
                for (i, w) in WEAPONS.iter().enumerate() {
                    if ui.selectable_label(sim.weapon == i, w.0).clicked() {
                        pressed = Some(format!("#weapon {i}"));
                    }
                }
                if ui.selectable_label(sim.weak, "weak").on_hover_text("A weakened Homer").clicked() {
                    pressed = Some("#weak".into());
                }
            });
        });

        egui::CollapsingHeader::new(egui::RichText::new("World").strong()).default_open(true).show(ui, |ui| {
            let (teal, lit) = (egui::Color32::from_rgb(30, 90, 85), egui::Color32::from_rgb(40, 170, 150));
            egui::Grid::new("world").spacing([4.0, 4.0]).show(ui, |ui| {
                for (i, (b, on)) in [("throw ball", false), ("throw box", false), ("drop crate", false), ("look", sim.look), ("avoid", sim.avoid), ("game tasks", sim.tasks),
                                     ("crowd", sim.crowd.len() == 4), ("grid", sim.crowd.len() == 24), ("car", sim.car.is_some()), ("ice", sim.ice.is_some()), ("fall", false), ("clear", false), ("Hero", sim.hero.is_some())]
                    .into_iter().enumerate() {
                    let label = match b {
                        "look" | "avoid" => format!("{b}: {}", if on { "on" } else { "off" }),
                        "game tasks" => format!("NM->anim: {}", if on { "on" } else { "off" }),
                        _ => b.into(),
                    };
                    let button = wide(ui, label, if on { lit } else { teal });
                    let button = if b == "game tasks" {
                        button.on_hover_text("NM -> animation (the game's tasks).\nOn: when the game's task would end (a shot, a balance, a fall...), Homer leaves Delirium \
                                              and goes back to his animation: standing, or getting up if he's lying.\nOff: he stays in Delirium; he only gets up after \
                                              lying still a while.")
                    } else if b == "Hero" {
                        button.on_hover_text("Play as Hero here, beside Homer (the game rewrite's player): WASD walk, Shift run, Ctrl sprint, \
                                              left click punch; walk into Homer to push him. Click again to take Hero out.")
                    } else { button };
                    if button.clicked() {
                        pressed = Some(if b == "Hero" { "#hero".into() } else { b.into() });
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            ui.add(egui::Slider::new(&mut sim.room.weight, 0.25..=4.0).logarithmic(true).text("object weight x"));
        });

        egui::CollapsingHeader::new(egui::RichText::new("Settings").strong()).default_open(false).show(ui, |ui| {
            ui.add(egui::Slider::new(&mut sim.sens, 0.05..=2.0).logarithmic(true).text("mouse sensitivity"));
        });

        egui::CollapsingHeader::new(egui::RichText::new("Controls").strong()).default_open(false).show(ui, |ui| {
            egui::Grid::new("keys").spacing([12.0, 2.0]).show(ui, |ui| {
                for (k, what) in [("click", "poke a body part"), ("drag", "pull a body part"), ("Shift + click", "shoot (a plain click too: Shoot's \"shoots\")"), ("Shift + right click", "fatal shot"),
                                  ("right drag", "orbit the camera"), ("wheel", "zoom"), ("middle drag, WASD, Q E", "move the camera"), ("C", "camera mode"),
                                  ("Space", "pause"), (".", "one step"), ("[  ]", "slower / faster"), ("R", "reset pose"), ("X", "stop all"),
                                  ("F", "drive arrows"), ("Enter", "type a command"), ("H", "hide panels"),
                                  ("Hero (World)", "WASD walk, Shift run, Ctrl sprint, click punch")] {
                    ui.label(egui::RichText::new(k).strong());
                    ui.label(what);
                    ui.end_row();
                }
            });
        });

        egui::CollapsingHeader::new(egui::RichText::new("Details").strong()).default_open(false).show(ui, |ui| {
            ui.label(mono(checks).color(DIM));
            ui.label(mono(ours));
            ui.label(mono(format!("world: {} triangles, {} objects{}", sim.world.len(), sim.room.objs.len(),
                                  if sim.look && !sim.room.objs.is_empty() { format!(", looking at {}", sim.room.objs.len() - 1) } else { String::new() })).color(DIM));
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
            (Some("#weapon"), Some(i)) => sim.weapon = i.parse().unwrap_or(0),
            (Some("#weak"), _) => sim.weak = !sim.weak,
            (Some("#click"), Some(m)) => sim.click_shoots = m == "shoot",
            (Some("#hero"), _) => sim.toggle_hero(),
            _ => sim.button(&p),
        }
    }
    Ok(())
}
