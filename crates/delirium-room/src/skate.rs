use bevy::math::Affine3A;
use bevy::prelude::*;
use skate_host::bridge::{Controls, Pose, Session};
use std::sync::mpsc;

pub fn to_skate(p: Vec3) -> Vec3 {
    Vec3::new(p.x, p.z, -p.y)
}
pub fn from_skate(p: Vec3) -> Vec3 {
    Vec3::new(p.x, -p.z, p.y)
}

const REACH: f32 = 40.0;
const INCH: f32 = 0.0254;
const FLICK: (f32, f32) = (0.12, 0.1);

enum Job {
    Tick(f32, Controls),
    Mount(Vec3, f32),
    World(Affine3A, Vec<(Vec3, Quat)>, Vec2),
}

pub struct Loose {
    parts: Vec<Mat4>,
    pub deck: Transform,
    pub v: Vec3,
    pub w: Vec3,
}

pub struct Skate {
    send: mpsc::Sender<Job>,
    recv: mpsc::Receiver<Result<Pose, String>>,
    pub pose: Option<Pose>,
    pub riding: bool,
    pub loose: Option<Loose>,
    last: Vec<KeyCode>,
    flick: Option<(Vec2, f32)>,
    stick: Vec2,
    pub script: Vec<KeyCode>,
    pub script_stick: Vec2,
    pub script_pad: (Vec2, u16),
    pub step_off: f32,
    pub rejoin: bool,
    pub pad: Option<Controls>,
    pub said: String,
    pub was: String,
    hist: Vec<(Vec3, Vec3)>,
    pub cam_turn: Option<(Vec3, f32)>,
    pub out_of: Option<usize>,
    pub center: Vec3,
    holes: Vec<(Vec3, Quat)>,
    half: Vec2,
    seat: Affine3A,
}

pub const KEYS: &str = "W push, S brake, A/D steer, Space ollie, Q kickflip, E heelflip, hold the wheel button and move the mouse: the right stick (flick tricks), Shift/Ctrl crouch and grab, F (X), Z/X bumpers, V/B stick clicks, arrows the d-pad, grind by landing on a rail or ledge; R off the board, then WASD walk, Shift run, Space jump, F throw the board, Ctrl drop it, F or Ctrl again call it back, R back on; J out of Skate 3 mode; a gamepad as Skate 3's";

fn heading(yaw: f32) -> f32 {
    yaw + std::f32::consts::FRAC_PI_2
}

impl Skate {
    pub fn start(tris: Vec<[Vec3; 3]>, feet: Vec3, yaw: f32) -> Skate {
        let (send, jobs) = mpsc::channel::<Job>();
        let (publish, recv) = mpsc::channel::<Result<Pose, String>>();
        let _ = std::thread::Builder::new().name("skate".into()).stack_size(32 << 20).spawn(move || {
            let run = || -> Result<(), String> {
                let t = std::time::Instant::now();
                let near: Vec<[Vec3; 3]> = tris.into_iter().filter(|t| t.iter().all(|p| (p.truncate() - feet.truncate()).length() < REACH)).collect();
                let inches: Vec<[Vec3; 3]> = near.iter().map(|t| t.map(|p| p / INCH)).collect();
                let (rails, census) = crate::skate_rails::find(&inches);
                let room_rails: Vec<Vec<Vec3>> = rails.into_iter().map(|r| r.into_iter().map(|p| p * INCH).collect()).collect();
                let rails: Vec<Vec<[f32; 3]>> = room_rails.iter().map(|r| r.iter().map(|&p| to_skate(p).to_array()).collect()).collect();
                info!("skate  world: {} triangles, {} rails ({} lips)", near.len(), census.rails, census.lips);
                let tris = near.iter().map(|t| t.map(|p| to_skate(p).to_array())).collect();
                let spawn = to_skate(feet + Vec3::Z * 0.1).to_array();
                let mut s = Session::new(&crate::skate_files::dir(), tris, rails.clone(), spawn, heading(yaw))?;
                let p = s.activate(spawn, heading(yaw))?;
                info!("skate  on in {:.1} s", t.elapsed().as_secs_f32());
                publish.send(Ok(p)).map_err(|e| e.to_string())?;
                let mut acc = 0.0;
                while let Ok(job) = jobs.recv() {
                    let (dt, c) = match job {
                        Job::Tick(dt, c) => (dt, c),
                        Job::Mount(feet, yaw) => {
                            let p = s.activate(to_skate(feet + Vec3::Z * 0.1).to_array(), heading(yaw))?;
                            publish.send(Ok(p)).map_err(|e| e.to_string())?;
                            continue;
                        }
                        Job::World(seat, holes, half) => {
                            let t = std::time::Instant::now();
                            let inv = seat.inverse();
                            let eng = |p: Vec3| to_skate(inv.transform_point3(p)).to_array();
                            let cut: Vec<[[f32; 3]; 3]> = near.iter().flat_map(|&t| cut_holes(t, &holes, half)).map(|t| t.map(eng)).collect();
                            let rails = room_rails.iter().map(|r| r.iter().map(|&p| eng(p)).collect()).collect();
                            let n = cut.len();
                            let built = s.collision_builder().build(cut, rails)?;
                            s.install_collision(built)?;
                            info!("skate  world: {n} triangles, {} portal holes, in {:.0} ms", holes.len(), t.elapsed().as_secs_f32() * 1000.0);
                            continue;
                        }
                    };
                    acc = (acc + dt).min(0.15);
                    let mut stepped = false;
                    while acc >= s.period() {
                        acc -= s.period();
                        s.tick(c)?;
                        stepped = true;
                    }
                    if stepped {
                        publish.send(Ok(s.pose())).map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            };
            if let Err(e) = run() {
                let _ = publish.send(Err(e));
            }
        });
        Skate { send, recv, pose: None, riding: true, loose: None, last: vec![], flick: None, stick: Vec2::ZERO, script: vec![], script_stick: Vec2::ZERO, script_pad: (Vec2::ZERO, 0), step_off: 0.0, rejoin: false,
                pad: None, said: String::new(), was: String::new(), hist: vec![], cam_turn: None, out_of: None, center: feet, holes: vec![], half: Vec2::ZERO,
                seat: Affine3A::IDENTITY }
    }

    pub fn step(&mut self, dt: f32, c: Controls) -> Result<(), String> {
        if self.pose.is_some() {
            let _ = self.send.send(Job::Tick(dt, c));
        }
        loop {
            match self.recv.try_recv() {
                Ok(p) => {
                    let p = p?;
                    if self.pose.as_ref().is_some_and(|o| o.state != p.state) {
                        self.was = self.pose.as_ref().map_or(String::new(), |o| o.state.clone());
                        info!("skate state {} -> {} at {:.2} board {:.2}", self.was, p.state, from_skate(p.root.w_axis.truncate()), from_skate(p.board.last().map_or(Vec3::ZERO, |b| b.w_axis.truncate())));
                    }
                    self.hist.push((self.room_v(p.velocity), self.room_v(p.spin)));
                    if self.hist.len() > 8 {
                        self.hist.remove(0);
                    }
                    if std::env::var_os("SKATE_TRACE").is_some() && p.tick % 30 == 0 {
                        info!("skate t{} {} at {:.2} board {:.2}", p.tick, p.state, from_skate(p.root.w_axis.truncate()), from_skate(p.board.last().map_or(Vec3::ZERO, |b| b.w_axis.truncate())));
                    }
                    self.pose = Some(p);
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(()),
                Err(mpsc::TryRecvError::Disconnected) => return Err("the engine stopped".into()),
            }
        }
    }

    pub fn mount(&mut self, feet: Vec3, yaw: f32) {
        if self.seat.transform_vector3(Vec3::Z).z < 0.99 {
            self.seat = Affine3A::IDENTITY;
            let _ = self.send.send(Job::World(self.seat, self.holes.clone(), self.half));
        }
        let _ = self.send.send(Job::Mount(self.seat.inverse().transform_point3(feet), yaw - self.turn()));
        (self.riding, self.loose, self.hist, self.said, self.was) = (true, None, vec![], String::new(), String::new());
        self.pose = None;
    }

    pub fn near(&self, p: Vec3) -> bool {
        p.truncate().distance(self.center.truncate()) < REACH * 0.5
    }

    pub fn off_edge(&self, p: Vec3) -> bool {
        p.truncate().distance(self.center.truncate()) > REACH - 3.0
    }

    pub fn through(&mut self, m: Affine3A) {
        self.seat = m * self.seat;
        for h in self.hist.iter_mut() {
            *h = (m.transform_vector3(h.0), m.transform_vector3(h.1));
        }
        let _ = self.send.send(Job::World(self.seat, self.holes.clone(), self.half));
    }

    pub fn holes(&mut self, holes: Vec<(Vec3, Quat)>, half: Vec2) {
        if holes != self.holes {
            let _ = self.send.send(Job::World(self.seat, holes.clone(), half));
            (self.holes, self.half) = (holes, half);
        }
    }

    pub fn room(&self, q: Vec3) -> Vec3 {
        self.seat.transform_point3(from_skate(q))
    }
    pub fn room_v(&self, q: Vec3) -> Vec3 {
        self.seat.transform_vector3(from_skate(q))
    }
    fn turn(&self) -> f32 {
        let x = self.seat.transform_vector3(Vec3::X);
        x.y.atan2(x.x)
    }

    pub fn recent_speed(&self) -> f32 {
        self.hist.iter().map(|h| h.0.with_z(0.0).length()).fold(0.0, f32::max)
    }
    pub fn recent_way(&self) -> Vec3 {
        self.hist.iter().map(|h| h.0.with_z(0.0)).max_by(|a, b| a.length().total_cmp(&b.length())).unwrap_or_default().normalize_or_zero()
    }

    pub fn fling(&mut self) -> (Vec3, Vec3) {
        let (v, w) = self.hist.iter().copied().max_by(|a, b| a.0.length().total_cmp(&b.0.length())).unwrap_or_default();
        let b: Vec<Vec3> = self.board().iter().map(|m| m.w_axis.truncate()).collect();
        self.riding = false;
        if let Some(r) = frame(&b) {
            let deck = Transform::from_translation(b[6]).with_rotation(r);
            let inv = deck.to_matrix().inverse();
            self.loose = Some(Loose { parts: b.iter().map(|&p| inv * Mat4::from_translation(p)).collect(), deck, v, w });
        }
        (v, w)
    }

    pub fn on_foot(&self) -> bool {
        self.pose.as_ref().is_some_and(|p| p.state.starts_with("Biped") || p.state == "OffBoardPushing")
    }

    pub fn controls(&mut self, down: &dyn Fn(KeyCode) -> bool, mouse: Option<Vec2>, dt: f32) -> Controls {
        use KeyCode::*;
        let held: Vec<KeyCode> = [KeyW, KeyS, KeyA, KeyD, KeyR, Space, KeyQ, KeyE, ShiftLeft, ShiftRight, ControlLeft, ControlRight,
            KeyF, KeyZ, KeyX, KeyV, KeyB, ArrowUp, ArrowDown, ArrowLeft, ArrowRight]
            .into_iter().filter(|&k| down(k) || self.script.contains(&k)).collect();
        let on = |k| held.contains(&k);
        let foot = self.on_foot();
        for (k, way) in [(Space, Vec2::Y), (KeyQ, Vec2::new(-0.7, 0.7)), (KeyE, Vec2::new(0.7, 0.7))] {
            if on(k) && !self.last.contains(&k) && !foot {
                self.flick = Some((way, 0.0));
            }
        }
        self.stick = match mouse {
            Some(d) => (self.stick * (-dt * 12.0).exp() + d * Vec2::new(1.0, -1.0) * 0.04).clamp_length_max(1.0),
            None => Vec2::ZERO,
        };
        let mut right = if self.script_stick != Vec2::ZERO { self.script_stick } else { self.stick };
        if let Some((way, t)) = self.flick.as_mut() {
            right = if *t < FLICK.0 { -Vec2::Y } else { *way };
            *t += dt;
            if *t >= FLICK.0 + FLICK.1 {
                self.flick = None;
            }
        }
        self.last = held.clone();
        let bit = |k: KeyCode, b: u16| if on(k) { b } else { 0 };
        let (ctrl, shift) = (on(ControlLeft) || on(ControlRight), on(ShiftLeft) || on(ShiftRight));
        let both = bit(KeyR, 0x8000) | bit(KeyZ, 0x100) | bit(KeyX, 0x200) | bit(KeyV, 0x40) | bit(KeyB, 0x80)
            | bit(ArrowUp, 0x1) | bit(ArrowDown, 0x2) | bit(ArrowLeft, 0x4) | bit(ArrowRight, 0x8)
            | if self.step_off > 0.0 { 0x8000 } else { 0 };
        if self.pose.is_some() {
            self.step_off -= dt;
        }
        let mut c = if foot {
            Controls {
                buttons: both | if shift { 0x1000 } else { 0 } | bit(Space, 0x4000),
                triggers: [ctrl as u8 * 255, on(KeyF) as u8 * 255],
                left: [((on(KeyD) as i32 - on(KeyA) as i32) * 32767) as i16, ((on(KeyW) as i32 - on(KeyS) as i32) * 32767) as i16],
                right: [(right.x * 32767.0) as i16, (right.y * 32767.0) as i16],
            }
        } else {
            Controls {
                buttons: both | bit(KeyW, 0x1000) | bit(KeyS, 0x2000) | bit(KeyF, 0x4000),
                triggers: [ctrl as u8 * 255, shift as u8 * 255],
                left: [((on(KeyD) as i32 - on(KeyA) as i32) * 32767) as i16, 0],
                right: [(right.x * 32767.0) as i16, (right.y * 32767.0) as i16],
            }
        };
        c.buttons |= self.script_pad.1;
        if self.script_pad.0 != Vec2::ZERO {
            c.left = [(self.script_pad.0.x * 32767.0) as i16, (self.script_pad.0.y * 32767.0) as i16];
        }
        if let Some(p) = self.pad {
            c.buttons |= p.buttons;
            c.triggers = [c.triggers[0].max(p.triggers[0]), c.triggers[1].max(p.triggers[1])];
            let or = |a: [i16; 2], b: [i16; 2]| if b != [0, 0] { b } else { a };
            (c.left, c.right) = (or(c.left, p.left), or(c.right, p.right));
        }
        c
    }

    pub fn board(&self) -> Vec<Mat4> {
        if let Some(l) = self.loose.as_ref() {
            let d = l.deck.to_matrix();
            return l.parts.iter().map(|m| d * *m).collect();
        }
        self.pose.as_ref().map_or(vec![], |p| p.board.iter().map(|m| Mat4::from_translation(self.room(m.w_axis.truncate()))).collect())
    }

    pub fn feet(&self) -> Option<(Vec3, Vec3)> {
        let p = self.pose.as_ref()?;
        Some((self.room(p.root.w_axis.truncate()), self.room_v(p.velocity)))
    }

    pub fn loose_frame(&mut self, dt: f32, world: &delirium::collision::Collision, water: impl Fn(Vec3) -> Option<f32>) {
        const R: f32 = 0.06;
        let Some(l) = self.loose.as_mut().filter(|_| dt > 0.0) else { return };
        l.v.z -= 9.81 * dt;
        if let Some(h) = water(l.deck.translation).filter(|&h| l.deck.translation.z < h) {
            l.v = l.v * (1.0 - 3.0 * dt).max(0.0) + Vec3::Z * (h - l.deck.translation.z) * 40.0 * dt;
            l.w *= (1.0 - 3.0 * dt).max(0.0);
        }
        let step = l.v * dt;
        let dir = step.normalize_or_zero();
        match world.contact(l.deck.translation, dir, step.length() + R).filter(|_| dir != Vec3::ZERO) {
            Some((t, n, _)) => {
                l.deck.translation += dir * (t - R).max(0.0);
                let into = l.v.dot(n);
                l.v = (l.v - n * into * if into < -1.5 { 1.4 } else { 1.0 }) * if into < -1.5 { 0.7 } else { (1.0 - 4.0 * dt).max(0.0) };
                l.w *= if into < -1.5 { 0.6 } else { (1.0 - 6.0 * dt).max(0.0) };
                if n.z > 0.7 && l.v.length() < 0.5 {
                    let up = l.deck.rotation * Vec3::Z;
                    let want = if up.dot(n) >= 0.0 { n } else { -n };
                    l.deck.rotation = (Quat::from_rotation_arc(up, want).slerp(Quat::IDENTITY, (1.0 - 8.0 * dt).clamp(0.0, 1.0)) * l.deck.rotation).normalize();
                }
            }
            None => l.deck.translation += step,
        }
        l.deck.rotation = (Quat::from_scaled_axis(l.w * dt) * l.deck.rotation).normalize();
    }

    pub fn retarget(&self, clips: &crate::getup::Clips, mut p: Vec<(Vec3, Quat)>) -> Option<(Vec<(Vec3, Quat)>, Transform)> {
        let pose = self.pose.as_ref()?;
    let at = |n: &str| Some(self.room(pose.root.transform_point3(pose.bones.get(pose.names.iter().position(|b| b == n)?)?.w_axis.truncate())));
        let side = at("RIGHTUPLEG")? - at("LEFTUPLEG")?;
        let fwd = Vec3::Z.cross(side).normalize_or(Vec3::Y);
        let mut body = Transform::from_translation(at("HIPS")?).with_rotation(Quat::from_rotation_z(fwd.y.atan2(fwd.x) - std::f32::consts::FRAC_PI_2));
        for (bone, next, from, to) in BONES {
            if let (Some(a), Some(b)) = (at(from), at(to)) {
                clips.point_bone(&mut p, body, bone, next, b - a);
            }
        }
        let w = clips.world(&p, body);
        let foot = |n: &str| clips.names().iter().position(|b| b == n).map(|i| w[i].0);
        if let (Some(l), Some(r), Some(sl), Some(sr)) = (foot("Char_L_Foot"), foot("Char_R_Foot"), at("LEFTFOOT"), at("RIGHTFOOT")) {
            body.translation += (sl + sr - l - r) * 0.5;
        }
        Some((p, body))
    }
}

fn cut_holes(t: [Vec3; 3], holes: &[(Vec3, Quat)], half: Vec2) -> Vec<[Vec3; 3]> {
    const N: usize = 16;
    let corner = |k: usize| {
        let a = (k as f32 + 0.5) * std::f32::consts::TAU / N as f32;
        Vec2::new(a.cos(), a.sin()) * half / (std::f32::consts::PI / N as f32).cos()
    };
    let sides: Vec<(Vec2, f32)> = (0..N).map(|k| {
        let (a, b) = (corner(k), corner((k + 1) % N));
        let n = Vec2::new(b.y - a.y, a.x - b.x).normalize();
        (n, n.dot(a))
    }).collect();
    let (lo, hi) = (-half / (std::f32::consts::PI / N as f32).cos(), half / (std::f32::consts::PI / N as f32).cos());
    let clip = |poly: Vec<Vec3>, n: Vec2, c: f32| -> Vec<Vec3> {
        let d = |p: Vec3| n.dot(p.truncate()) - c;
        let mut out = vec![];
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if d(a) <= 0.0 {
                out.push(a);
            }
            if (d(a) <= 0.0) != (d(b) <= 0.0) {
                out.push(a + (b - a) * (d(a) / (d(a) - d(b))));
            }
        }
        out
    };
    let mut tris = vec![t];
    for &(at, rot) in holes {
        let inv = rot.inverse();
        tris = tris.into_iter().flat_map(|t| {
            let l = t.map(|p| inv * (p - at));
            if l.iter().any(|p| p.z < -1.5 || p.z > 0.1)
                || l.iter().all(|p| p.x <= lo.x) || l.iter().all(|p| p.x >= hi.x) || l.iter().all(|p| p.y <= lo.y) || l.iter().all(|p| p.y >= hi.y) {
                return vec![t];
            }
            let mut out = vec![];
            for k in 0..N {
                let (n, c) = sides[k];
                let poly = sides[..k].iter().fold(clip(l.to_vec(), -n, -c), |p, &(n, c)| clip(p, n, c));
                for i in 1..poly.len().saturating_sub(1) {
                    let tri = [poly[0], poly[i], poly[i + 1]];
                    if (tri[1] - tri[0]).cross(tri[2] - tri[0]).length() > 1e-5 {
                        out.push(tri.map(|p| at + rot * p));
                    }
                }
            }
            out
        }).collect();
    }
    tris
}

pub fn frame(b: &[Vec3]) -> Option<Quat> {
    let [rf, lf, _, _, front, back, _] = b.try_into().ok()?;
    let along = (front - back).try_normalize()?;
    let across = (lf - rf).reject_from(along).try_normalize()?;
    Some(Quat::from_mat3(&Mat3::from_cols(along, across, along.cross(across))))
}

const BONES: [(&str, &str, &str, &str); 14] = [
    ("Char_Spine", "Char_Neck", "SPINE", "NECK"),
    ("Char_Neck", "Char_Head", "NECK", "HEAD"),
    ("Char_L_Clavicle", "Char_L_UpperArm", "LEFTSHOULDER", "LEFTARM"),
    ("Char_L_UpperArm", "Char_L_Forearm", "LEFTARM", "LEFTFOREARM"),
    ("Char_L_Forearm", "Char_L_Hand", "LEFTFOREARM", "LEFTHAND"),
    ("Char_R_Clavicle", "Char_R_UpperArm", "RIGHTSHOULDER", "RIGHTARM"),
    ("Char_R_UpperArm", "Char_R_Forearm", "RIGHTARM", "RIGHTFOREARM"),
    ("Char_R_Forearm", "Char_R_Hand", "RIGHTFOREARM", "RIGHTHAND"),
    ("Char_L_Thigh", "Char_L_Calf", "LEFTUPLEG", "LEFTLEG"),
    ("Char_L_Calf", "Char_L_Foot", "LEFTLEG", "LEFTFOOT"),
    ("Char_L_Foot", "Char_L_Toe0", "LEFTFOOT", "LEFTTOEBASE"),
    ("Char_R_Thigh", "Char_R_Calf", "RIGHTUPLEG", "RIGHTLEG"),
    ("Char_R_Calf", "Char_R_Foot", "RIGHTLEG", "RIGHTFOOT"),
    ("Char_R_Foot", "Char_R_Toe0", "RIGHTFOOT", "RIGHTTOEBASE"),
];
