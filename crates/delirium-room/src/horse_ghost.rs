use super::*;
use std::sync::Mutex;

pub struct Ghost {
    pub name: String,
    frames: Vec<Vec<(Vec3, Quat)>>,
    dt: f32,
    parent: Vec<Option<usize>>,
    caps: Vec<(usize, Vec3, Quat, f32, f32)>,
    turn: Quat,
    from: Vec3,
    to: Vec3,
    t: f32,
}

pub static GHOST: Mutex<Option<Ghost>> = Mutex::new(None);

fn sweep_dir() -> std::path::PathBuf {
    std::path::Path::new(&std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("Delirium").join("rdr2-sweep")
}

fn euler(rx: f32, ry: f32, rz: f32) -> Quat {
    Quat::from_euler(EulerRot::ZYX, rz.to_radians(), ry.to_radians(), rx.to_radians())
}

fn frame2(a: Vec3, b: Vec3) -> Mat3 {
    let e1 = a.normalize();
    let e2 = a.cross(b).normalize();
    Mat3::from_cols(e1, e2, e1.cross(e2))
}

pub fn load(name: &str, at: Vec3, fwd: Vec3) -> Result<String, String> {
    let path = if name.ends_with(".csv") { std::path::PathBuf::from(name) } else { sweep_dir().join(format!("horse-{name}-body.csv")) };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().ok_or("empty recording")?.split(',').collect();
    let ids: Vec<usize> = head[4..].chunks(6).filter_map(|c| c[0].strip_prefix('b')?.strip_suffix(".x")?.parse().ok()).collect();
    let mut frames = vec![];
    let mut vel = vec![];
    for l in lines {
        let n: Vec<f32> = l.split(',').map(|x| x.parse().unwrap_or(0.0)).collect();
        if n.len() < 4 + ids.len() * 6 {
            continue;
        }
        vel.push(Vec3::new(n[1], n[2], n[3]));
        frames.push(ids.iter().enumerate().map(|(i, _)| {
            let c = &n[4 + i * 6..4 + i * 6 + 6];
            (Vec3::new(c[0], c[1], c[2]), euler(c[3], c[4], c[5]))
        }).collect::<Vec<_>>());
    }
    if frames.len() < 2 {
        return Err(format!("{}: no frames", path.display()));
    }
    let slot = |id: usize| ids.iter().position(|&b| b == id);
    let f0 = &frames[0];
    let root = f0[slot(0).unwrap_or(0)].0;
    let real: Vec<bool> = f0.iter().map(|(p, _)| p.distance(root) < 5.0).collect();
    let (mut d, mut v) = (0.0, 0.0);
    for k in 1..frames.len() {
        if vel[k].length() > 0.5 {
            d += frames[k][slot(0).unwrap_or(0)].0.distance(frames[k - 1][slot(0).unwrap_or(0)].0);
            v += vel[k].length();
        }
    }
    let dt = if v > 1.0 && d > 0.0 { (d / v).clamp(1.0 / 240.0, 1.0 / 15.0) } else { 1.0 / 60.0 };
    let sample: Vec<usize> = (0..frames.len()).step_by((frames.len() / 30).max(1)).collect();
    let parent: Vec<Option<usize>> = (0..ids.len()).map(|b| {
        if !real[b] {
            return None;
        }
        (0..ids.len()).filter(|&a| a != b && real[a]).filter_map(|a| {
            let len = f0[b].0.distance(f0[a].0);
            if len < 0.002 {
                return None;
            }
            let loc: Vec<Vec3> = sample.iter().map(|&k| frames[k][a].1.inverse() * (frames[k][b].0 - frames[k][a].0)).collect();
            let m = loc.iter().copied().sum::<Vec3>() / loc.len() as f32;
            let spread = loc.iter().map(|l| l.distance(m)).fold(0.0, f32::max);
            (spread < 0.005).then_some((a, len))
        }).min_by(|x, y| x.1.total_cmp(&y.1)).map(|x| x.0)
    }).collect();
    let fr = rdr2frag::Frag::from_text(rdr2frag::BUILT_IN_FRAG)?;
    let map = std::fs::read_to_string(path.with_file_name("horse-bones.txt")).unwrap_or_default();
    let bone_of = |name: &str| map.lines().find_map(|l| {
        let mut w = l.split_whitespace();
        (w.next() == Some(name)).then(|| w.next()?.parse::<usize>().ok()).flatten()
    }).and_then(slot);
    let rest_joint = |l: usize| fr.joints.iter().find(|j| j.child == l).map_or(fr.links[l].capsules[0].at, |j| {
        let (t, at) = fr.links[l].frame();
        at + t * j.at_child
    });
    let model = f0[slot(0).unwrap_or(0)].1 * Quat::from_rotation_z(std::f32::consts::PI);
    let mut caps = vec![];
    for (l, link) in fr.links.iter().enumerate() {
        let Some(b) = bone_of(&link.name) else { continue };
        let (pb, rb) = f0[b];
        let j = rest_joint(l);
        let q0 = rb.inverse() * model;
        let kids: Vec<(Vec3, Vec3)> = fr.joints.iter().filter(|x| x.parent == l).filter_map(|x| {
            let c = bone_of(&fr.links[x.child].name)?;
            Some((rest_joint(x.child) - j, rb.inverse() * (f0[c].0 - pb)))
        }).filter(|(v, u)| v.length() > 0.02 && u.length() > 0.02).collect();
        let q = match kids.as_slice() {
            [] => q0,
            [(v, u)] => Quat::from_rotation_arc((q0 * *v).normalize(), u.normalize()) * q0,
            [a, b, ..] => Quat::from_mat3(&(frame2(a.1, b.1) * frame2(a.0, b.0).transpose())).normalize(),
        };
        for c in &link.capsules {
            caps.push((b, q * (c.at - j), q * Quat::from_mat3(&c.turn).normalize(), c.radius, (c.half - c.radius).max(0.0)));
        }
    }
    let low = f0.iter().zip(&real).filter(|(_, r)| **r).map(|(p, _)| p.0.z).fold(f32::MAX, f32::min);
    let rec_fwd = (f0[slot(0).unwrap_or(0)].1 * Vec3::Y).with_z(0.0).normalize_or(Vec3::Y);
    let turn = Quat::from_rotation_arc(rec_fwd, fwd.with_z(0.0).normalize_or(Vec3::X));
    let from = root.with_z(low);
    let n = frames.len();
    let found = parent.iter().filter(|p| p.is_some()).count();
    let name = path.file_name().map_or(name.into(), |f| f.to_string_lossy().into_owned());
    *GHOST.lock().unwrap() = Some(Ghost { name: name.clone(), frames, dt, parent, caps, turn, from, to: at, t: 0.0 });
    Ok(format!("ghost  {name}: {n} frames at {:.0} fps, {} bones ({found} joined), {} capsules", 1.0 / dt, real.iter().filter(|r| **r).count(),
               GHOST.lock().unwrap().as_ref().map_or(0, |g| g.caps.len())))
}

fn label(name: &str) -> String {
    let dir = |d: &str| match d { "left" => "left", "right" => "right", "back" => "behind", "fwd" => "the front", d => d }.to_string();
    let parts: Vec<&str> = name.split('-').collect();
    match parts.as_slice() {
        ["walk", n] => format!("Walk {n}"),
        ["trot", n] => format!("Trot {n}"),
        ["turn", n] => format!("Turn on the spot {n}"),
        ["stand", n] => format!("Standing {n}"),
        ["ragdoll", n] => format!("Ragdoll fall, shove {n}"),
        ["strafe", _] | [_, "ride", _] => "Riding: trainer's strafe try".into(),
        ["ride", t] => format!("Riding {t}"),
        ["clip", c] if c.starts_with("shunt_from_") => format!("Bumped from the {} (trot) {}", if c.contains("left") { "left" } else { "right" },
                                                              if c.ends_with("_l2") { "turning left" } else if c.ends_with("_r2") { "turning right" } else { "" }),
        ["clip", c] if c.starts_with("react_") => format!("Kicked from {}", dir(c.trim_start_matches("react_").trim_end_matches("_vic"))),
        ["clip", c] if c.starts_with("attack_") => format!("Kicking toward {} deg", c.trim_start_matches("attack_").trim_end_matches("_att")),
        _ => name.into(),
    }
}

pub fn panel(ui: &mut bevy_egui::egui::Ui, sim: &Sim) {
    let now = GHOST.lock().unwrap().as_ref().map(|g| g.name.trim_start_matches("horse-").trim_end_matches("-body.csv").to_string());
    let mut pick = None;
    bevy_egui::egui::ComboBox::from_id_salt("horse ghost").width(220.0).selected_text(now.as_deref().map_or("Off".into(), label)).show_ui(ui, |ui| {
        if ui.selectable_label(now.is_none(), "Off").clicked() {
            pick = Some(String::new());
        }
        let mut names: Vec<String> = std::fs::read_dir(sweep_dir()).into_iter().flatten().flatten().filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            let ok = e.metadata().map_or(false, |m| m.len() > 200_000) && !n.starts_with("horse-standing-");
            n.strip_prefix("horse-")?.strip_suffix("-body.csv").filter(|_| ok).map(String::from)
        }).collect();
        names.sort_by_key(|n| label(n));
        for n in names {
            if ui.selectable_label(now.as_deref() == Some(&n), label(&n)).clicked() {
                pick = Some(n);
            }
        }
    });
    match pick.as_deref() {
        Some("") => *GHOST.lock().unwrap() = None,
        Some(n) => command(sim, &format!("ghost {n}")),
        None => {}
    }
}

pub fn command(sim: &Sim, d: &str) {
    let name = d["ghost".len()..].trim();
    if name.is_empty() {
        *GHOST.lock().unwrap() = None;
        return;
    }
    let (at, fwd) = spot(sim);
    match load(name, at, fwd) {
        Ok(s) | Err(s) => eprintln!("{s}"),
    }
}

pub fn spot(sim: &Sim) -> (Vec3, Vec3) {
    let Some(h) = sim.horse.as_ref() else { return (Vec3::new(0.0, 2.0, 0.0), Vec3::X) };
    let f: Vec<Transform> = h.doll.frames().collect();
    let caps = h.capsules();
    let floor = caps.iter().map(|c| c.0.z.min(c.1.z) - c.2).fold(f32::MAX, f32::min);
    let pelvis = f[0].translation;
    let fwd = (f.get(15).map_or(pelvis + Vec3::X, |n| n.translation) - pelvis).with_z(0.0).normalize_or(Vec3::X);
    ((pelvis + Vec3::Z.cross(fwd) * 2.0).with_z(floor), fwd)
}

pub fn draw(mut g: Gizmos, time: Res<Time<Real>>, sim: NonSend<Sim>) {
    static TRIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !TRIED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        if let Ok(n) = std::env::var("HORSE_GHOST") {
            let (at, fwd) = spot(&sim);
            match load(&n, at, fwd) {
                Ok(s) | Err(s) => eprintln!("{s}"),
            }
        }
    }
    let Ok(mut lock) = GHOST.try_lock() else { return };
    let Some(gh) = lock.as_mut() else { return };
    gh.t = (gh.t + time.delta_secs()) % (gh.frames.len() as f32 * gh.dt);
    let k = ((gh.t / gh.dt) as usize).min(gh.frames.len() - 1);
    let fr = &gh.frames[k];
    let room = |p: Vec3| gh.to + gh.turn * (p - gh.from);
    let bone = Color::srgba(1.0, 0.85, 0.3, 0.9);
    for (b, p) in gh.parent.iter().enumerate() {
        if let Some(a) = p {
            g.line(room(fr[*a].0), room(fr[b].0), bone);
        }
    }
    let cap = Color::srgba(0.3, 0.9, 1.0, 0.8);
    for &(b, at, q, r, half) in &gh.caps {
        let (pb, rb) = fr[b];
        g.primitive_3d(&Capsule3d::new(r, half * 2.0), Isometry3d::new(room(pb + rb * at), gh.turn * rb * q), cap);
    }
}
