use bevy::prelude::*;
use bevy_egui::egui;

mod clip;
mod funnel;
mod fx;
mod place;
mod render;

use crate::{OursCam, OursGizmos, Sim};

const HALF: Vec2 = Vec2::new(32.0 * 0.0254, 56.0 * 0.0254);
const COLORS: [Color; 2] = [Color::srgb(0.15, 0.55, 1.0), Color::srgb(1.0, 0.55, 0.1)];
const PATH: &str = "portal2_path.txt";

#[derive(Clone, Copy)]
pub struct Frame {
    at: Vec3,
    rot: Quat,
}

impl Frame {
    fn new(at: Vec3, normal: Vec3, up_hint: Vec3) -> Option<Frame> {
        let n = normal.try_normalize()?;
        let up = (up_hint - n * up_hint.dot(n)).try_normalize()?;
        Some(Frame { at, rot: Quat::from_mat3(&Mat3::from_cols(up.cross(n), up, n)).normalize() })
    }
    fn local(&self, p: Vec3) -> Vec3 {
        self.rot.inverse() * (p - self.at)
    }
    fn normal(&self) -> Vec3 {
        self.rot * Vec3::Z
    }
}

fn through(a: &Frame, b: &Frame) -> (Quat, impl Fn(Vec3) -> Vec3) {
    const FLIP: Quat = Quat::from_xyzw(0.0, 1.0, 0.0, 0.0);
    let (a, b) = (*a, *b);
    ((b.rot * FLIP * a.rot.inverse()).normalize(), move |p| b.at + b.rot * (FLIP * a.local(p)))
}

struct SolidWorld<'a>(&'a delirium::collision::Collision);

impl place::World for SolidWorld<'_> {
    fn ray(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<place::Hit> {
        self.0.contact(origin, dir, max).map(|(t, normal, _)| place::Hit { point: origin + dir * t, normal, distance: t })
    }
}

#[derive(Default)]
pub struct Portals {
    pub gun: bool,
    ends: [Option<Frame>; 2],
    game: Option<String>,
    typed: String,
    asked: bool,
    model_tried: bool,
    cube: Option<Option<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>>,
    sounds_tried: bool,
    homer_last: [f32; 2],
    obj_last: Vec<[f32; 2]>,
    hero_last: [f32; 2],
    placed: [f32; 2],
    stat: f32,
    was_linked: bool,
    tex: Option<[Handle<Image>; 3]>,
    tex_tried: bool,
    fx: Vec<fx::Event>,
    fx_ready: bool,
    pub ots: Option<(Vec3, Vec3)>,
    pub float: Option<(Vec3, Vec3)>,
}

impl Portals {
    pub fn new() -> Portals {
        let typed = std::fs::read_to_string(PATH).unwrap_or_default().trim().to_string();
        let game = Some(typed.clone()).filter(|g| is_portal2(g));
        Portals { game, typed, ..default() }
    }
    pub fn on(&self) -> bool {
        self.game.is_some()
    }

    fn link(&mut self, path: Option<String>) -> bool {
        let Some(path) = path.filter(|p| is_portal2(p)) else { self.asked = true; return false };
        self.typed = path.clone();
        self.game = Some(path);
        self.asked = false;
        self.model_tried = false;
        self.cube = None;
        self.sounds_tried = false;
        self.tex = None;
        self.tex_tried = false;
        true
    }
}

fn is_portal2(dir: &str) -> bool {
    std::path::Path::new(dir).join("portal2/pak01_dir.vpk").is_file()
}

fn find_portal2() -> Option<String> {
    let steam = ["C:/Program Files (x86)/Steam", "C:/Program Files/Steam"];
    let vdf: String = steam.iter().filter_map(|s| std::fs::read_to_string(format!("{s}/steamapps/libraryfolders.vdf")).ok()).collect();
    let libs = vdf.lines().filter_map(|l| l.trim().strip_prefix("\"path\"")).map(|p| p.trim().trim_matches('"').replace("\\\\", "/"));
    steam.iter().map(|s| s.to_string()).chain(libs).map(|l| format!("{l}/steamapps/common/Portal 2")).find(|g| is_portal2(g))
}

impl Sim {
    pub fn portal_shot(&mut self, i: usize, from: Vec3, dir: Vec3) {
        self.sounds.push(crate::sfx::Snd::PortalShoot(i as u8));
        let holes = std::mem::take(&mut self.world.holes);
        let ends = self.portal.ends;
        let windows: Vec<place::Window> = match ends {
            [Some(a), Some(b)] => vec![place::Window { frame: a, exit: b }, place::Window { frame: b, exit: a }],
            _ => vec![],
        };
        let others: Vec<Frame> = ends[1 - i].into_iter().collect();
        let w = SolidWorld(&self.world);
        let trace = place::trace_shot(&w, from, dir, 200.0, HALF, &windows);
        let result = match trace.hit {
            Some(h) => place::place(&w, h, trace.dir, HALF, &others),
            None => Err(place::Fail::NoHit),
        };
        self.world.holes = holes;
        let f = match result {
            Ok(f) => Frame { at: f.at + f.normal() * 0.01, ..f },
            Err(why) => {
                if let Some(h) = trace.hit {
                    self.portal.fx.push(fx::Event::Burst { name: "portal_badsurface", pos: h.point, basis: fx::facing(h.normal), kind: i });
                }
                return self.say(format!("portal  {} shot: {}", ["blue", "orange"][i], match why {
                place::Fail::NoHit => "nothing to stick to",
                place::Fail::DoesNotFit => "it doesn't fit there",
                place::Fail::Overlap => "it would overlap the other portal",
                }));
            }
        };
        let n = f.normal();
        if let Some(old) = self.portal.ends[i] {
            self.portal.fx.push(fx::Event::Burst { name: "portal_close", pos: old.at, basis: Mat3::from_quat(old.rot), kind: i });
        }
        self.portal.fx.push(fx::Event::Burst { name: "portal_success", pos: f.at + f.normal() * 0.01, basis: Mat3::from_quat(f.rot), kind: i });
        self.portal.ends[i] = Some(f);
        self.portal.fx.push(fx::Event::Rim { kind: i });
        (self.portal.placed[i], self.portal.stat) = (f32::NAN, 1.0);
        self.sounds.push(crate::sfx::Snd::PortalOpen(i as u8));
        self.say(format!("portal  {} at {:.2},{:.2},{:.2} facing {:.2},{:.2},{:.2}", ["blue", "orange"][i], f.at.x, f.at.y, f.at.z, n.x, n.y, n.z));
    }

    fn portal_funnel(&mut self, dt: f32) {
        let [Some(a), Some(b)] = self.portal.ends else { return };
        if self.paused || self.held || dt <= 0.0 {
            return;
        }
        let dt = (dt * self.speed).min(0.05);
        funnel_agent(&mut self.agent, [a, b], dt);
        if let Some(h) = self.hero_doll.as_mut().filter(|h| h.rise.is_none()) {
            funnel_agent(&mut h.agent, [a, b], dt);
        }
    }

    fn portal_transit(&mut self) {
        let [Some(a), Some(b)] = self.portal.ends else { return };
        if let Some((i, speed)) = transit(&mut self.agent, a, b, &mut self.portal.homer_last) {
            self.say(format!("portal  homer through {} -> {} at {speed:.1} m/s", ["blue", "orange"][i], ["orange", "blue"][i]));
        }
        if let Some(h) = self.hero_doll.as_mut().filter(|h| h.rise.is_none())
            && let Some((i, speed)) = transit(&mut h.agent, a, b, &mut self.portal.hero_last) {
            self.say(format!("portal  hero (ragdoll) through {} -> {} at {speed:.1} m/s", ["blue", "orange"][i], ["orange", "blue"][i]));
        }
    }

    fn portal_hero(&mut self) {
        let [Some(a), Some(b)] = self.portal.ends else { return };
        let Some(n) = self.hero.as_ref().filter(|_| self.hero_doll.is_none() && !self.driving) else { return };
        let (c, yaw, fall) = (n.feet + Vec3::Z * 0.9, n.yaw, n.fall);
        let over = |f: &Frame| { let l = f.local(n.feet); f.normal().z > 0.7 && l.z.abs() < 0.5 && (l.x / HALF.x).powi(2) + (l.y / HALF.y).powi(2) < 1.0 };
        let trip = fall < 0.0 && (over(&a) || over(&b));
        let last = std::mem::replace(&mut self.portal.hero_last, [a.local(c).z, b.local(c).z]);
        let into = crossed(&a, last[0], c) || crossed(&b, last[1], c);
        if self.hero_physics && (trip || into) {
            let n = self.hero.as_mut().unwrap();
            (n.vel, n.fall) = (n.vel + Vec3::Z * fall, 0.0);
            self.portal.hero_last = last;
            if self.hero_fall() {
                return self.say(format!("portal  hero {} it: on his ragdoll", if trip { "trips into" } else { "falls into" }));
            }
        }
        for (i, (from, to)) in [(a, b), (b, a)].into_iter().enumerate() {
            if !crossed(&from, last[i], c) {
                continue;
            }
            let (turn, map) = through(&from, &to);
            let fwd = (turn * Vec3::new(yaw.cos(), yaw.sin(), 0.0)).with_z(0.0).normalize_or(to.normal().with_z(0.0).normalize_or(Vec3::X));
            let out = map(c);
            let (feet, fall) = if to.normal().z > 0.7 { (out.with_z(to.at.z), (-fall).max(3.0)) } else { (out - Vec3::Z * 0.9, (turn * Vec3::Z * fall).z) };
            let n = self.hero.as_mut().unwrap();
            (n.feet, n.yaw, n.fall) = (feet, fwd.y.atan2(fwd.x), fall);
            let c = feet + Vec3::Z * 0.9;
            self.portal.hero_last = [a.local(c).z, b.local(c).z];
            return self.say(format!("portal  hero through {} -> {}", ["blue", "orange"][i], ["orange", "blue"][i]));
        }
    }

    fn portal_objects(&mut self) {
        let [Some(a), Some(b)] = self.portal.ends else { return };
        self.portal.obj_last.resize(self.room.objs.len(), [f32::NAN; 2]);
        for j in 0..self.room.objs.len() {
            let Some((v, w, _)) = self.room.obj_motion(j) else { continue };
            let (t, _) = self.room.obj(0, j);
            let last = std::mem::replace(&mut self.portal.obj_last[j], [a.local(t.translation).z, b.local(t.translation).z]);
            for (i, (from, to)) in [(a, b), (b, a)].into_iter().enumerate() {
                if !crossed(&from, last[i], t.translation) {
                    continue;
                }
                let (turn, map) = through(&from, &to);
                let at = map(t.translation);
                self.room.obj_set(j, Transform::from_translation(at).with_rotation((turn * t.rotation).normalize()), turn * v, turn * w);
                self.portal.obj_last[j] = [a.local(at).z, b.local(at).z];
                self.say(format!("portal  {} through {} -> {} at {:.1} m/s", crate::world::KINDS[self.room.objs[j].kind].0, ["blue", "orange"][i], ["orange", "blue"][i], v.length()));
                break;
            }
        }
    }

    pub fn portal_script(&mut self, a: &str) {
        if a == "gun" {
            self.portal.gun = false;
            return pick_gun(self);
        }
        if let Some(d) = a.strip_prefix("drop ") {
            if let [k, x, y, z] = d.split(',').filter_map(|x| x.parse().ok()).collect::<Vec<f32>>()[..] {
                let at = Vec3::new(x, y, z);
                self.room.throw(k as usize, [at, at], Vec3::ZERO);
            }
            return;
        }
        let p = |d: &str| d.split(',').filter_map(|x| x.parse().ok()).collect::<Vec<f32>>();
        if let Some(d) = a.strip_prefix("float ") && let [x, y, z, ax, ay, az] = p(d)[..] {
            let (at, aim) = (Vec3::new(x, y, z), Vec3::new(ax, ay, az));
            (self.portal.gun, self.aim_gun, self.portal.float) = (true, true, Some((at, aim)));
            (self.gun.at, self.gun.dir, self.gun.aim, self.gun.placed) = (at, (aim - at).normalize(), aim, true);
            return;
        }
        if let Some(d) = a.strip_prefix("aim ") && let (&[x, y, z], Some(f)) = (&p(d)[..], self.portal.float.as_mut()) {
            f.1 = Vec3::new(x, y, z);
            return;
        }
        if let Some(i) = a.strip_prefix("fire ").and_then(|i| i.parse::<usize>().ok()) && let Some((at, aim)) = self.portal.float {
            return self.portal_shot(i & 1, at, (aim - at).normalize());
        }
        let v = p(a);
        if let [i, fx, fy, fz, dx, dy, dz] = v[..] {
            self.portal_shot(i as usize & 1, Vec3::new(fx, fy, fz), Vec3::new(dx, dy, dz).normalize());
        }
    }
}

pub fn frame(mut sim: NonSendMut<Sim>, buttons: Res<ButtonInput<MouseButton>>, keys: Res<ButtonInput<KeyCode>>, windows: Query<&Window>, time: Res<Time>,
             cam: Query<(&Camera, &GlobalTransform), With<OursCam>>, mut g: Gizmos<OursGizmos>) {
    let hero = sim.hero.is_some();
    if hero && sim.portal.gun && !sim.hero_gun {
        sim.hero_take(crate::WEAPONS.iter().position(|w| w.0 == "ak47"));
        sim.portal.gun = true;
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if sim.portal.gun && (hero || sim.click_shoots || sim.aim_gun || ctrl || crate::shift_held(&keys)) && (!sim.over_ui || sim.portal.ots.is_some()) {
        for (i, b) in [MouseButton::Left, MouseButton::Right].into_iter().enumerate() {
            if !buttons.just_pressed(b) {
                continue;
            }
            let ray = if hero {
                sim.portal.ots
            } else if sim.aim_gun && sim.gun.placed && !ctrl {
                Some((sim.gun.at, sim.gun.dir))
            } else {
                let c = windows.single().ok().and_then(|w| w.cursor_position());
                cam.single().ok().zip(c).and_then(|((c, gt), p)| c.viewport_to_world(gt, p).ok()).map(|r| (r.origin, *r.direction))
            };
            if let Some((from, dir)) = ray {
                sim.portal_shot(i, from, dir);
            }
        }
    }
    if let Some((from, dir)) = sim.portal.ots {
        if let Some((t, n, _)) = sim.world.contact(from, dir, 200.0) {
            let at = from + dir * t + n * 0.01;
            g.circle(Isometry3d::new(at, Quat::from_rotation_arc(Vec3::Z, n)), 0.012 * t.max(1.0), Color::WHITE);
        }
    }
    let chell = sim.portal.gun && sim.hero_gun;
    if let Some(n) = sim.hero.as_mut().filter(|n| n.chell.is_some() != chell) {
        n.chell = chell.then_some(Quat::IDENTITY);
    }
    let holes = match sim.portal.ends {
        [Some(a), Some(b)] => [a, b].map(|f| delirium::collision::Hole { at: f.at, rot: f.rot, half: HALF }).to_vec(),
        _ => vec![],
    };
    sim.world.holes = holes.clone();
    sim.room.holes = holes;
    sim.portal_funnel(time.delta_secs());
    sim.portal_transit();
    sim.portal_objects();
    sim.portal_hero();
}

pub fn settings(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    crate::info(ui, "Optional bonus. Link your own Portal 2 installation here; the base room doesn't need it.");
    ui.label("Portal 2 folder");
    let r = ui.add(egui::TextEdit::singleline(&mut sim.portal.typed).desired_width(f32::INFINITY).hint_text("e.g. F:/SteamLibrary/steamapps/common/Portal 2"));
    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        *pressed = Some("#portal2 typed".into());
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Link folder").clicked() { *pressed = Some("#portal2 typed".into()); }
        if ui.button("Find Steam install").clicked() { *pressed = Some("#portal2 auto".into()); }
    });
    if sim.portal.asked {
        ui.colored_label(crate::HI, "Folder not found. Choose the Portal 2 folder containing portal2/pak01_dir.vpk.");
    }
    if sim.portal.on() {
        ui.add(egui::Label::new(format!("Linked: {}", sim.portal.game.as_deref().unwrap_or_default())).wrap());
        crate::info(ui, "The portal gun is in the weapons. Left click: blue, right click: orange. Homer goes through them.");
    } else {
        crate::info(ui, "Portal 2 mode is off. Link a folder or enable the mode to use the remembered installation.");
    }
}

pub fn toggle(sim: &mut Sim, action: Option<&str>) {
    let p = &mut sim.portal;
    if action.is_none() && p.game.is_some() {
        p.game = None;
        p.gun = false;
        return;
    }
    let t = p.typed.trim().trim_matches('"').replace('\\', "/");
    let path = match action {
        Some("typed") => Some(t),
        Some("auto") => find_portal2(),
        _ => Some(t).filter(|t| is_portal2(t)).or_else(find_portal2),
    };
    if !p.link(path) {
        return sim.say("portal  folder not found; any existing link kept".into());
    }
    if let Some(g) = &p.game {
        if let Err(e) = std::fs::write(PATH, g) {
            return sim.say(format!("portal  linked for this session; couldn't remember folder: {e}"));
        }
    }
    let say = p.game.as_ref().map_or("portal  Portal 2 not found".into(), |g| format!("portal  Portal 2 at {g}"));
    sim.say(say);
}

fn vpk(game: &str, want: &[&str]) -> Option<Vec<Vec<u8>>> {
    let dir = format!("{game}/portal2/pak01_dir.vpk");
    let d = std::fs::read(&dir).ok()?;
    let u32_at = |o: usize| d.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let head = if u32_at(4)? == 2 { 28 } else { 12 };
    let data_at = head + u32_at(8)? as usize;
    let mut p = head;
    let mut found = std::collections::HashMap::new();
    let cs = |p: &mut usize| -> Option<String> {
        let e = *p + d[*p..].iter().position(|&c| c == 0)?;
        let s = String::from_utf8_lossy(&d[*p..e]).into_owned();
        *p = e + 1;
        Some(s)
    };
    loop {
        let ext = cs(&mut p)?;
        if ext.is_empty() {
            break;
        }
        loop {
            let path = cs(&mut p)?;
            if path.is_empty() {
                break;
            }
            loop {
                let name = cs(&mut p)?;
                if name.is_empty() {
                    break;
                }
                let full = format!("{path}/{name}.{ext}");
                let (pre, arc) = (u16::from_le_bytes([d[p + 4], d[p + 5]]) as usize, u16::from_le_bytes([d[p + 6], d[p + 7]]));
                let (off, len) = (u32_at(p + 8)? as u64, u32_at(p + 12)? as usize);
                if want.contains(&full.as_str()) {
                    found.insert(full, (d[p + 18..p + 18 + pre].to_vec(), arc, off, len));
                }
                p += 18 + pre;
            }
        }
    }
    want.iter().map(|w| {
        let (mut out, arc, off, len) = found.remove(*w)?;
        if len > 0 {
            use std::io::{Read, Seek};
            let (f, at) = if arc == 0x7fff { (dir.clone(), off + data_at as u64) } else { (format!("{game}/portal2/pak01_{arc:03}.vpk"), off) };
            let mut f = std::fs::File::open(f).ok()?;
            f.seek(std::io::SeekFrom::Start(at)).ok()?;
            let mut b = vec![0; len];
            f.read_exact(&mut b).ok()?;
            out.extend(b);
        }
        Some(out)
    }).collect()
}

fn vtf(d: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let i32_at = |o: usize| d.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()));
    let (w, h) = (u16::from_le_bytes([d[16], d[17]]) as usize, u16::from_le_bytes([d[18], d[19]]) as usize);
    let (fmt, mips) = (i32_at(52)?, d[56] as usize);
    let block = match fmt { 13 => 8, 15 => 16, _ => return None };
    let size = |m: usize| (w >> m).max(1).div_ceil(4) * (h >> m).max(1).div_ceil(4) * block;
    let start = (0..i32_at(68)? as usize).map(|k| 80 + k * 8).find(|&r| d[r..r + 3] == [0x30, 0, 0]).and_then(|r| i32_at(r + 4))? as usize;
    let at = start + (1..mips).map(size).sum::<usize>();
    let src = d.get(at..at + size(0))?;
    let mut out = vec![0u8; w * h * 4];
    let across = w.div_ceil(4);
    for (n, c) in src.chunks(block).enumerate() {
        let (bx, by) = (n % across * 4, n / across * 4);
        let col = &c[block - 8..];
        let rgb = |v: u16| [((v >> 11) & 31) as f32 * 255.0 / 31.0, ((v >> 5) & 63) as f32 * 255.0 / 63.0, (v & 31) as f32 * 255.0 / 31.0];
        let (c0, c1) = (u16::from_le_bytes([col[0], col[1]]), u16::from_le_bytes([col[2], col[3]]));
        let (a, b) = (rgb(c0), rgb(c1));
        let mix = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
        let three = c0 <= c1 && block == 8;
        let pal = if three { [a, b, mix(0.5), [0.0; 3]] } else { [a, b, mix(1.0 / 3.0), mix(2.0 / 3.0)] };
        let bits = u32::from_le_bytes(col[4..8].try_into().unwrap());
        let alpha = |k: usize| -> u8 {
            if block == 8 {
                return if three && bits >> (k * 2) & 3 == 3 { 0 } else { 255 };
            }
            let (a0, a1) = (c[0] as f32, c[1] as f32);
            let ab = c[2..8].iter().rev().fold(0u64, |s, &x| s << 8 | x as u64);
            match (ab >> (k * 3) & 7) as usize {
                0 => c[0],
                1 => c[1],
                i if a0 > a1 => ((a0 * (8 - i) as f32 + a1 * (i - 1) as f32) / 7.0) as u8,
                6 => 0,
                7 => 255,
                i => ((a0 * (6 - i) as f32 + a1 * (i - 1) as f32) / 5.0) as u8,
            }
        };
        for k in 0..16 {
            let (x, y) = (bx + k % 4, by + k / 4);
            if x < w && y < h {
                let p = pal[(bits >> (k * 2) & 3) as usize];
                out[(y * w + x) * 4..][..4].copy_from_slice(&[p[0] as u8, p[1] as u8, p[2] as u8, alpha(k)]);
            }
        }
    }
    Some((w as u32, h as u32, out))
}

fn model(game: &str, b: &str, t: &str) -> Option<(Mesh, Image)> {
    let f = vpk(game, &[&format!("{b}.mdl"), &format!("{b}.vvd"), &format!("{b}.dx90.vtx"), t])?;
    let (mdl, vvd, vtx) = (&f[0], &f[1], &f[2]);
    let i = |d: &[u8], o: usize| d.get(o..o + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap())) as usize;
    let fl = |o: usize| f32::from_le_bytes(vvd[o..o + 4].try_into().unwrap());
    let (fixups, fix_at, verts_at, tan_at) = (i(vvd, 48), i(vvd, 52), i(vvd, 56), i(vvd, 60));
    let mut ids: Vec<usize> = (0..(tan_at - verts_at) / 48).collect();
    if fixups > 0 {
        ids = (0..fixups).flat_map(|k| {
            let o = fix_at + k * 12;
            let (lod, src, n) = (i(vvd, o) as i32, i(vvd, o + 4), i(vvd, o + 8));
            if lod >= 0 { src..src + n } else { 0..0 }
        }).collect();
    }
    let (mut pos, mut nrm, mut uv) = (vec![], vec![], vec![]);
    for v in &ids {
        let o = verts_at + v * 48 + 16;
        pos.push([fl(o) * 0.0254, fl(o + 4) * 0.0254, fl(o + 8) * 0.0254]);
        nrm.push([fl(o + 12), fl(o + 16), fl(o + 20)]);
        uv.push([fl(o + 24), fl(o + 28)]);
    }
    let mut idx = vec![];
    let (parts, part_at, vparts) = (i(mdl, 232), i(mdl, 236), i(vtx, 32));
    for b in 0..parts {
        let (mb, vb) = (part_at + b * 16, vparts + b * 8);
        if i(mdl, mb + 4) == 0 {
            continue;
        }
        let mm = mb + i(mdl, mb + 12);
        let (meshes, mesh_at, first) = (i(mdl, mm + 72), i(mdl, mm + 76), i(mdl, mm + 84) / 48);
        let vm = vb + i(vtx, vb + 4);
        let lod = vm + i(vtx, vm + 4);
        let vmesh = lod + i(vtx, lod + 4);
        for e in 0..meshes {
            let off = i(mdl, mm + mesh_at + e * 116 + 12);
            let me = vmesh + e * 9;
            for s in 0..i(vtx, me) {
                let sg = me + i(vtx, me + 4) + s * 25;
                let (nv, vo, ni, io) = (i(vtx, sg), i(vtx, sg + 4), i(vtx, sg + 8), i(vtx, sg + 12));
                for k in 0..ni {
                    let x = u16::from_le_bytes([vtx[sg + io + k * 2], vtx[sg + io + k * 2 + 1]]) as usize;
                    if x >= nv {
                        return None;
                    }
                    let orig = u16::from_le_bytes([vtx[sg + vo + x * 9 + 4], vtx[sg + vo + x * 9 + 5]]) as usize;
                    idx.push((first + off + orig) as u32);
                }
            }
        }
    }
    idx.chunks_exact_mut(3).for_each(|t| t.swap(1, 2));
    let mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos).with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv).with_inserted_indices(bevy::mesh::Indices::U32(idx));
    let (w, h, rgba) = vtf(&f[3])?;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let img = Image::new(Extent3d { width: w, height: h, depth_or_array_layers: 1 }, TextureDimension::D2, rgba,
                         TextureFormat::Rgba8UnormSrgb, bevy::asset::RenderAssetUsages::default());
    Some((mesh, img))
}

pub fn sounds(sim: &mut Sim, assets: &mut Assets<crate::sfx::Sfx>, bank: &mut crate::sfx::Bank) {
    use crate::sfx::Snd;
    let Some(game) = sim.portal.game.clone().filter(|_| !sim.portal.sounds_tried) else { return };
    sim.portal.sounds_tried = true;
    const W: &str = "sound/weapons/portalgun/";
    let mut n = 0;
    for (s, f) in [(Snd::PortalShoot(0), "portalgun_shoot_blue1"), (Snd::PortalShoot(1), "portalgun_shoot_red1"),
                   (Snd::PortalOpen(0), "portal_open_blue_01"), (Snd::PortalOpen(1), "portal_open_red_01")] {
        if let Some(w) = vpk(&game, &[&format!("{W}{f}.wav")]).and_then(|f| crate::sfx::wav(&f[0])) {
            bank.add(s, assets.add(w));
            n += 1;
        }
    }
    sim.say(format!("portal  {n} of 4 sounds read"));
}

pub fn cube(sim: &mut Sim, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, images: &mut Assets<Image>)
            -> Option<(Handle<Mesh>, Handle<StandardMaterial>, Transform)> {
    let game = sim.portal.game.clone()?;
    if sim.portal.cube.is_none() {
        let got = model(&game, "models/props/metal_box", "materials/models/props/metal_box_skin001.vtf").and_then(|(mesh, img)| {
            use bevy::camera::primitives::MeshAabb;
            let b = mesh.get_aabb()?;
            let k = 1.0 / b.half_extents.max_element();
            let mat = mats.add(StandardMaterial { base_color_texture: Some(images.add(img)), perceptual_roughness: 0.5, ..default() });
            Some((meshes.add(mesh), mat, Transform::from_scale(Vec3::splat(k)).with_translation(-Vec3::from(b.center) * k)))
        });
        sim.say(format!("portal  companion cube {}", if got.is_some() { "read" } else { "not read" }));
        sim.portal.cube = Some(got);
    }
    sim.portal.cube.clone().flatten()
}

#[derive(Component)]
pub struct GunLook(bool);

const GUN_TURN: Quat = Quat::from_xyzw(0.0, 1.0, 0.0, 0.0);
const GUN_GRIP: Vec3 = Vec3::new(0.0, 0.0, 0.02);

pub fn gun_view(mut sim: NonSendMut<Sim>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>,
                mut images: ResMut<Assets<Image>>, mut looks: Query<(&mut Transform, &mut Visibility, &GunLook)>,
                mut others: Query<(&mut Visibility, &crate::GunMesh), Without<GunLook>>) {
    let Some(game) = sim.portal.game.clone() else { return };
    if looks.is_empty() {
        if sim.portal.model_tried {
            return;
        }
        sim.portal.model_tried = true;
        let Some((mesh, img)) = model(&game, "models/weapons/w_portalgun", "materials/models/weapons/w_models/portalgun/w_portalgun.vtf") else { return sim.say("portal  w_portalgun: not read".into()) };
        let mesh = meshes.add(mesh);
        let holo = mats.add(StandardMaterial { base_color: Color::srgba(0.35, 0.9, 1.0, 0.3), emissive: LinearRgba::rgb(0.1, 0.5, 0.6),
                                               alpha_mode: AlphaMode::Blend, unlit: true, double_sided: true, cull_mode: None, ..default() });
        let real = mats.add(StandardMaterial { base_color_texture: Some(images.add(img)), perceptual_roughness: 0.4, ..default() });
        for (real, m) in [(false, holo), (true, real)] {
            commands.spawn((GunLook(real), Mesh3d(mesh.clone()), MeshMaterial3d(m), Transform::default(), Visibility::Hidden,
                            bevy::camera::visibility::RenderLayers::layer(1)));
        }
        return sim.say("portal  w_portalgun read".into());
    }
    let hero = sim.hero.is_some();
    let on = sim.portal.gun && sim.gun.placed && if hero { sim.hero_gun && !sim.driving && sim.hero_doll.is_none() } else { sim.aim_gun };
    if sim.portal.gun {
        others.iter_mut().filter(|(_, g)| !g.1).for_each(|(mut v, _)| *v = Visibility::Hidden);
    }
    let mut t = Transform::from_translation(sim.gun.at).looking_to(sim.gun.dir, Vec3::Z) * Transform::from_rotation(GUN_TURN)
        * Transform::from_translation(-GUN_GRIP);
    if let Some((n, turn)) = sim.hero.as_ref().filter(|_| hero).and_then(|n| Some((n, n.chell?))) {
        t = Transform::from_translation(n.hand + turn * CHELL_GUN.0).with_rotation(turn * Quat::from_mat3(&Mat3::from_cols(CHELL_GUN.1, CHELL_GUN.2, CHELL_GUN.3)));
    }
    for (mut tr, mut v, l) in &mut looks {
        *v = if on && l.0 == hero { Visibility::Visible } else { Visibility::Hidden };
        *tr = t;
    }
}

pub fn pick_gun(sim: &mut Sim) {
    if sim.hero.is_some() {
        sim.hero_take(crate::WEAPONS.iter().position(|w| w.0 == "ak47"));
        sim.say("portal  over his shoulder: click the view to aim with the mouse, Esc frees it; left blue, right orange".into());
    }
    sim.portal.gun = true;
}

const CHELL_ARMS: [(&str, &str, Vec3); 8] = [
    ("Char_R_Clavicle", "Char_R_UpperArm", Vec3::new(0.4486, -0.8651, -0.2243)),
    ("Char_R_UpperArm", "Char_R_Forearm", Vec3::new(0.2536, -0.2826, -0.9251)),
    ("Char_R_Forearm", "Char_R_Hand", Vec3::new(0.0081, 0.9996, 0.0282)),
    ("Char_R_Hand", "Char_R_Finger2", Vec3::new(0.2575, 0.9465, -0.1945)),
    ("Char_L_Clavicle", "Char_L_UpperArm", Vec3::new(-0.9413, 0.2880, -0.1764)),
    ("Char_L_UpperArm", "Char_L_Forearm", Vec3::new(0.0784, 0.4190, -0.9046)),
    ("Char_L_Forearm", "Char_L_Hand", Vec3::new(0.7828, 0.5634, -0.2642)),
    ("Char_L_Hand", "Char_L_Finger2", Vec3::new(0.8401, 0.3494, -0.4148)),
];
const CHELL_GUN: (Vec3, Vec3, Vec3, Vec3) = (Vec3::new(0.0024, 0.0043, -0.0385), Vec3::new(-0.9498, -0.0024, 0.3129),
                                             Vec3::new(0.3120, -0.0834, 0.9464), Vec3::new(0.0238, 0.9965, 0.0800));

pub fn chell_arms(clips: &crate::getup::Clips, pose: &mut [(Vec3, Quat)], body: Transform, pitch: f32) -> Quat {
    let turn = body.rotation * Quat::from_rotation_x(pitch);
    for (bone, next, dir) in CHELL_ARMS {
        if !clips.point_bone(pose, body, bone, next, turn * dir) {
            static ONCE: std::sync::Once = std::sync::Once::new();
            ONCE.call_once(|| eprintln!("portal  no {bone}/{next} in {:?}", clips.names()));
        }
    }
    turn
}

pub fn plugin(app: &mut App) {
    render::plugin(app);
    clip::plugin(app);
    fx::plugin(app);
    app.add_systems(Update, (frame.after(crate::step), gun_view.after(crate::gun_view)))
        .add_systems(PostUpdate, ghosts.before(bevy::transform::TransformSystems::Propagate).before(bevy::pbr::check_entities_needing_specialization::<clip::GhostMaterial>));
}

fn funnel_agent(agent: &mut delirium::nm::Agent, portals: [Frame; 2], dt: f32) {
    let (centre, v) = agent.doll.com();
    let steered = funnel::steer(&portals, 9.81, centre, v, dt);
    let delta = steered - v;
    if delta.length_squared() < 1e-10 {
        return;
    }
    let bodies: Vec<(usize, Transform, Vec3, Vec3)> = agent.doll.frames().enumerate().map(|(k, f)| (k, f, agent.doll.vel(k).0, agent.doll.vel(k).1)).collect();
    for (k, f, lin, ang) in bodies {
        agent.doll.set_body(k, f.translation, f.rotation, lin + delta, ang);
    }
}

fn transit(agent: &mut delirium::nm::Agent, a: Frame, b: Frame, last: &mut [f32; 2]) -> Option<(usize, f32)> {
    let c = centre(agent.doll.frames().map(|f| f.translation));
    let was = std::mem::replace(last, [a.local(c).z, b.local(c).z]);
    let (i, (from, to)) = [(a, b), (b, a)].into_iter().enumerate().find(|(i, (from, _))| crossed(from, was[*i], c))?;
    let (turn, map) = through(&from, &to);
    let doll = &agent.doll;
    let parts: Vec<(Vec3, Quat, Vec3, Vec3)> = doll.frames().enumerate()
        .map(|(k, f)| (map(f.translation), turn * f.rotation, turn * doll.vel(k).0, turn * doll.vel(k).1)).collect();
    let speed = doll.vel(0).0.length();
    for (k, (x, q, v, w)) in parts.into_iter().enumerate() {
        agent.doll.set_body(k, x, q.normalize(), v, w);
    }
    let drop = funnel::exit_gimme(&to, agent.doll.com().1);
    if drop != Vec3::ZERO {
        let bodies: Vec<(usize, Transform, Vec3, Vec3)> = agent.doll.frames().enumerate().map(|(k, f)| (k, f, agent.doll.vel(k).0, agent.doll.vel(k).1)).collect();
        for (k, f, lin, ang) in bodies {
            agent.doll.set_body(k, f.translation, f.rotation, lin - drop, ang);
        }
    }
    let c = centre(agent.doll.frames().map(|f| f.translation));
    *last = [a.local(c).z, b.local(c).z];
    Some((i, speed))
}

fn centre(p: impl Iterator<Item = Vec3>) -> Vec3 {
    let (sum, n) = p.fold((Vec3::ZERO, 0), |(s, n), p| (s + p, n + 1));
    sum / n.max(1) as f32
}

fn crossed(f: &Frame, last: f32, p: Vec3) -> bool {
    let l = f.local(p);
    last >= 0.0 && l.z < 0.0 && l.z > -1.0 && (l.x / HALF.x).powi(2) + (l.y / HALF.y).powi(2) < 1.0
}

pub fn hero_fall(n: &mut crate::hero::Hero, ground: Option<f32>, dt: f32) -> Option<f32> {
    match ground.filter(|&z| if n.fall == 0.0 { z > n.feet.z - 0.45 } else { n.fall < 0.0 && z >= n.feet.z }) {
        Some(z) => {
            n.fall = 0.0;
            Some(z)
        }
        None => {
            n.fall -= 9.81 * dt;
            n.feet.z += n.fall * dt;
            None
        }
    }
}

#[derive(Component)]
pub struct Ghost(Entity, usize);

#[allow(clippy::type_complexity)]
fn ghosts(sim: NonSend<Sim>, mut commands: Commands, standard: Res<Assets<StandardMaterial>>, mut clipped: ResMut<Assets<clip::GhostMaterial>>,
          src: Query<(Entity, &Transform, &Visibility, &Mesh3d, &MeshMaterial3d<StandardMaterial>, Option<&bevy::camera::visibility::RenderLayers>),
                     (Or<(With<crate::PartMesh>, With<crate::HeroMesh>, With<crate::PropMesh>, With<crate::FacePart>)>, Without<Ghost>)>,
          mut ghosts: Query<(Entity, &Ghost, &mut Transform, &mut Visibility, &MeshMaterial3d<clip::GhostMaterial>)>) {
    let mut have: std::collections::HashMap<(Entity, usize), Entity> = ghosts.iter().map(|(g, o, ..)| ((o.0, o.1), g)).collect();
    let mut shown = std::collections::HashSet::new();
    if let [Some(a), Some(b)] = sim.portal.ends {
        for (e, t, v, mesh, mat, layers) in &src {
            if *v == Visibility::Hidden {
                continue;
            }
            for (i, (from, to)) in [(a, b), (b, a)].into_iter().enumerate() {
                let l = from.local(t.translation);
                if !(l.z > -1.0 && l.z < 0.6 && l.x.abs() < HALF.x + 0.6 && l.y.abs() < HALF.y + 0.6) {
                    continue;
                }
                let (turn, map) = through(&from, &to);
                let at = Transform { translation: map(t.translation), rotation: (turn * t.rotation).normalize(), scale: t.scale };
                let planes = clip::exit_planes(&to);
                match have.remove(&(e, i)) {
                    Some(g) => {
                        if let Ok((_, _, mut gt, mut gv, gm)) = ghosts.get_mut(g) {
                            (*gt, *gv) = (at, Visibility::Visible);
                            let want = standard.get(&mat.0);
                            let stale = clipped.get(&gm.0).is_some_and(|c| c.extension.clip != planes
                                || want.is_some_and(|w| c.base.base_color != w.base_color || c.base.emissive != w.emissive || c.base.alpha_mode != w.alpha_mode));
                            if stale {
                                if let Some(mut c) = clipped.get_mut(&gm.0) {
                                    c.extension.clip = planes;
                                    if let Some(w) = want {
                                        c.base = w.clone();
                                    }
                                }
                            }
                        }
                        shown.insert(g);
                    }
                    None => {
                        let Some(base) = standard.get(&mat.0).cloned() else { continue };
                        let material = clipped.add(clip::GhostMaterial { base, extension: clip::ClipExt { clip: planes } });
                        let g = commands.spawn((mesh.clone(), MeshMaterial3d(material), at, Visibility::Visible, Ghost(e, i))).id();
                        if let Some(l) = layers {
                            commands.entity(g).insert(l.clone());
                        }
                    }
                }
            }
        }
    }
    for (g, o, _, mut v, _) in &mut ghosts {
        if shown.contains(&g) {
            continue;
        }
        if src.get(o.0).is_err() {
            commands.entity(g).despawn();
        } else {
            *v = Visibility::Hidden;
        }
    }
}
