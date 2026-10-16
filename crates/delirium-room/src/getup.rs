use bevy::prelude::*;
use delirium_data::{anim, img, model, archive_key::ArchiveKey, resource, rpf, skeleton};
use delirium::ragdoll::Build;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const DOWN_SECS: f32 = 2.0;
const RISE_BLEND: f32 = 0.25;
pub const IDLE: usize = 2;
pub const WALK: usize = 3;
pub const SWINGS: usize = 6;

pub struct Clips {
    parents: Vec<Option<usize>>,
    bind: Vec<(Vec3, Quat)>,
    clips: Vec<(anim::Animation, Vec<Option<usize>>)>,
    pub hero: Option<delirium::ragdoll::Rig>,
    parts: Vec<(usize, Transform)>,
    chest: usize,
}

pub struct Rise {
    clip: usize,
    hold: bool,
    pub t: f32,
    body: Transform,
    lay: Vec<Transform>,
}

const XEX_KEY_SRC: &str = "https://raw.githubusercontent.com/xenia-project/xenia/95a5c3ee250f80c3b9d139658649d9ffb6db3eec/src/xenia/cpu/xex_module.cc";

fn xex_key() -> Result<[u8; 16], String> {
    let dir = std::env::var_os("APPDATA").map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))).unwrap_or_default().join("delirium");
    if let Some(k) = std::fs::read(dir.join("xex.key")).ok().and_then(|b| <[u8; 16]>::try_from(b).ok()) {
        return Ok(k);
    }
    let fail = "couldn't get the key that opens default.xex (it's fetched from the internet the first time: check the connection)";
    let out = std::process::Command::new("curl").args(["-sSfL", XEX_KEY_SRC]).output().map_err(|_| fail)?;
    let src = String::from_utf8_lossy(&out.stdout);
    let list = src.split("xe_xex2_retail_key[16] = {").nth(1).and_then(|s| s.split('}').next()).ok_or(fail)?;
    let k: [u8; 16] = list.split(',').filter_map(|h| u8::from_str_radix(h.trim().trim_start_matches("0x").trim_start_matches("0X"), 16).ok())
        .collect::<Vec<_>>().try_into().map_err(|_| fail)?;
    let _ = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join("xex.key"), k));
    Ok(k)
}

pub fn key(dir: &Path) -> Result<ArchiveKey, String> {
    static KEY: std::sync::OnceLock<Result<[u8; 32], String>> = std::sync::OnceLock::new();
    let k = KEY.get_or_init(|| {
        let xex = std::fs::read(dir.join("default.xex")).map_err(|e| format!("default.xex: {e}"))?;
        let xex = delirium_data::xex::XexImage::parse_with_key(&xex, &xex_key()?).map_err(|e| format!("default.xex: {e}"))?;
        let mut head = vec![0; 0x810];
        std::fs::File::open(dir.join("xbox360.rpf")).and_then(|mut f| f.read_exact(&mut head)).map_err(|e| format!("xbox360.rpf: {e}"))?;
        ArchiveKey::find_by_trial(&xex.image, &head).map(|(_, k)| k).ok_or_else(|| "no archive key in default.xex".into())
    });
    k.clone().map(|k| ArchiveKey::from_bytes(&k))
}

fn img_open(path: &Path, key: &ArchiveKey) -> Result<(Vec<img::Entry>, std::fs::File), String> {
    let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let mut f = std::fs::File::open(path).map_err(|e| format!("{name}: {e}"))?;
    let mut head = vec![];
    (&mut f).take(4 << 20).read_to_end(&mut head).map_err(|e| e.to_string())?;
    Ok((img::read_table(&head, Some(key))?, f))
}

fn img_read(table: &[img::Entry], f: &mut std::fs::File, name: &str) -> Result<resource::Resource, String> {
    let e = table.iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or(format!("{name} not found"))?;
    let mut raw = vec![];
    f.seek(SeekFrom::Start(e.offset)).map_err(|e| e.to_string())?;
    f.take(e.used_blocks as u64 * 2048).read_to_end(&mut raw).map_err(|e| e.to_string())?;
    resource::parse(&raw)
}

pub fn homer(game: &str, model: &str, nm: usize) -> Result<Build, String> {
    let b = homer_build(game, model, nm)?;
    if (nm == delirium::nmrs::HOMER && delirium::nmdbm::logged_links().is_some()) || delirium::nmdbm::DERIVED_LINKS[nm].get().is_some() {
        return Ok(b);
    }
    let clips = Clips::load(game, &b)?;
    let idle: Vec<Quat> = clips.place(&clips.pose(IDLE, 0.0), Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))).iter().map(|f| f.rotation).collect();
    match delirium::nmrs::derive_links(&b, &idle) {
        Some(l) => {
            let _ = delirium::nmdbm::DERIVED_LINKS[nm].set(l);
            homer_build(game, model, nm)
        }
        None => Ok(b),
    }
}

fn homer_build(game: &str, model: &str, nm: usize) -> Result<Build, String> {
    let dir = Path::new(game);
    let (table, mut f) = img_open(&dir.join("xbox360/models/cdimages/componentpeds.img"), &key(dir)?)?;
    let frag = img_read(&table, &mut f, &format!("{model}.xft"))?;
    let bones = skeleton::read(&frag, model::fragment_root(&frag)?)?;
    let parts = delirium_data::ragdoll::read(&frag)?;
    let parents: Vec<Option<usize>> = bones.iter().map(|b| b.parent).collect();
    let bind: Vec<(Vec3, Quat)> = bones.iter().map(|b| (Vec3::from(b.translation), Quat::from_array(b.rotation).normalize())).collect();
    delirium::ragdoll::Rig::new(&parts, &parents, &bind, &bones, nm).map(|r| r.build).ok_or(format!("{model}.xft: no ragdoll"))
}

impl Clips {
    pub fn load(game: &str, build: &Build) -> Result<Clips, String> {
        let dir = Path::new(game);
        let rd = |p: &str| std::fs::read(dir.join(p)).map_err(|e| format!("{p}: {e}"));
        let key = key(dir)?;
        let platform = rd("xbox360.rpf")?;
        let toc = rpf::read_toc(&platform, &key)?;
        let nested = toc.iter().find(|e| e.path.eq_ignore_ascii_case("models/cdimages/playerped.rpf")).ok_or("playerped.rpf not found")?;
        let peds = rpf::read(&platform, nested)?;
        let inner = rpf::read_toc(&peds, &key)?;
        let e = inner.iter().find(|e| e.path.eq_ignore_ascii_case("player.xft")).ok_or("player.xft not in playerped.rpf")?;
        let frag = resource::parse(&rpf::read(&peds, e)?)?;
        let bones = skeleton::read(&frag, model::fragment_root(&frag)?)?;
        let (table, mut f) = img_open(&dir.join("xbox360/anim/anim.img"), &key)?;
        let mut clip = |dict: &str, name: &str| -> Result<(anim::Animation, Vec<Option<usize>>), String> {
            let a = anim::read_dictionary(&img_read(&table, &mut f, dict)?)?.into_iter().find(|a| a.name.eq_ignore_ascii_case(name))
                .ok_or(format!("no {name} in {dict}"))?;
            let map = a.tracks.iter().map(|t| bones.iter().position(|b| b.id == t.bone).filter(|&b| b != 0)).collect();
            Ok((a, map))
        };
        let mut clips = vec![clip("get_up.xad", "get_up_fast")?, clip("get_up_back.xad", "get_up_fast")?];
        for c in ["idle", "walk", "run", "sprint"] {
            clips.push(clip("move_player.xad", c)?);
        }
        for c in ["hook_r", "hook_l", "uppercut_r"] {
            clips.push(clip("melee_player_unarmed.xad", c)?);
        }
        let parts = build.parts.iter().map(|p| bones.iter().position(|b| b.name == p.name).map(|b| (b, p.offset)).ok_or(format!("no bone {}", p.name)))
            .collect::<Result<_, _>>()?;
        let chest = build.parts.iter().position(|d| d.parent == Some(0) && !d.lower).unwrap_or(0);
        Ok(Clips {
            parents: bones.iter().map(|b| b.parent).collect(),
            bind: bones.iter().map(|b| (Vec3::from(b.translation), Quat::from_array(b.rotation).normalize())).collect(),
            hero: delirium_data::ragdoll::read(&frag).ok().and_then(|p| {
                let bind: Vec<(Vec3, Quat)> = bones.iter().map(|b| (Vec3::from(b.translation), Quat::from_array(b.rotation).normalize())).collect();
                delirium::ragdoll::Rig::new(&p, &bones.iter().map(|b| b.parent).collect::<Vec<_>>(), &bind, &bones, delirium::nmrs::HOMER)
            }),
            clips, parts, chest,
        })
    }

    pub fn pose(&self, c: usize, phase: f32) -> Vec<(Vec3, Quat)> {
        let (a, map) = &self.clips[c];
        let x = phase.clamp(0.0, 1.0) * (a.frames - 1) as f32;
        let (i, f) = (x.floor() as usize, x.fract());
        let q = |v: &[f32]| Quat::from_xyzw(v[0], v[1], v[2], v[3]).normalize();
        let mut out = self.bind.clone();
        out[0] = (Vec3::ZERO, Quat::IDENTITY);
        for (t, bone) in a.tracks.iter().zip(map) {
            let (va, vb) = (t.sample(i), t.sample(i + 1));
            match (t.kind, bone, t.bone) {
                (anim::BONE_ROTATION, Some(b), _) => out[*b].1 = q(&va).slerp(q(&vb), f),
                (anim::BONE_TRANSLATION, Some(b), _) => out[*b].0 = Vec3::from_slice(&va).lerp(Vec3::from_slice(&vb), f),
                (anim::BONE_ROTATION, None, 0) => out[0].1 = q(&va).slerp(q(&vb), f),
                (anim::BONE_TRANSLATION | anim::MOVER_TRANSLATION, None, 0) => out[0].0 += Vec3::from_slice(&va).lerp(Vec3::from_slice(&vb), f),
                _ => {}
            }
        }
        out
    }

    pub fn play(&self, c: usize, phase: f32) -> Vec<(Vec3, Quat)> {
        let mut out = self.pose(c, phase);
        let a = &self.clips[c].0;
        let x = phase.clamp(0.0, 1.0) * (a.frames - 1) as f32;
        let (i, f) = (x.floor() as usize, x.fract());
        let q = |v: &[f32]| Quat::from_xyzw(v[0], v[1], v[2], v[3]).normalize();
        let roots: Vec<&anim::Track> = a.tracks.iter().filter(|t| t.bone == 0).collect();
        let identity = |t: &anim::Track| t.kind == anim::BONE_ROTATION
            && (0..a.frames).all(|k| { let v = t.sample(k); v[0].abs() < 1e-3 && v[1].abs() < 1e-3 && v[2].abs() < 1e-3 });
        out[0] = self.bind[0];
        if !roots.iter().any(|t| identity(t)) {
            for t in roots {
                match t.kind {
                    anim::BONE_ROTATION => out[0].1 = q(&t.sample(i)).slerp(q(&t.sample(i + 1)), f),
                    anim::BONE_TRANSLATION => out[0].0 = Vec3::from_slice(&t.sample(i)).lerp(Vec3::from_slice(&t.sample(i + 1)), f),
                    _ => {}
                }
            }
        }
        out
    }

    pub fn duration(&self, c: usize) -> f32 {
        self.clips[c].0.duration
    }

    fn place(&self, pose: &[(Vec3, Quat)], body: Transform) -> Vec<Transform> {
        let mut bones: Vec<(Vec3, Quat)> = Vec::with_capacity(pose.len());
        for (i, &(t, r)) in pose.iter().enumerate() {
            let (pt, pr) = self.parents[i].map_or((body.translation, body.rotation), |p| bones[p]);
            bones.push((pt + pr * t, pr * r));
        }
        self.parts.iter().map(|&(b, o)| {
            let (bt, br) = bones[b];
            Transform::from_translation(bt + br * o.translation).with_rotation(br * o.rotation)
        }).collect()
    }

    pub fn start(&self, lay: Vec<Transform>) -> Rise {
        let yaw = |q: Quat| { let a = q * Vec3::Y; a.y.atan2(a.x) };
        let ch = self.chest;
        let (_, clip, body) = (0..IDLE).map(|c| {
            let first = self.place(&self.pose(c, 0.0), Transform::IDENTITY);
            let turn = Quat::from_rotation_z(yaw(lay[ch].rotation) - yaw(first[ch].rotation));
            let body = Transform::from_translation(lay[0].translation - turn * first[0].translation).with_rotation(turn);
            ((turn * first[ch].rotation).angle_between(lay[ch].rotation), c, body)
        }).min_by(|a, b| a.0.total_cmp(&b.0)).expect("a get-up clip");
        Rise { clip, hold: false, t: 0.0, body, lay }
    }

    pub fn stand(&self, lay: Vec<Transform>) -> Rise {
        let yaw = |q: Quat| { let a = q * Vec3::Y; a.y.atan2(a.x) };
        let first = self.place(&self.pose(IDLE, 0.0), Transform::IDENTITY);
        let turn = Quat::from_rotation_z(yaw(lay[0].rotation) - yaw(first[0].rotation));
        let body = Transform::from_translation(lay[0].translation - turn * first[0].translation).with_rotation(turn);
        Rise { clip: IDLE, hold: true, t: 0.0, body, lay }
    }

    pub fn stand_up(&self, build: &mut Build) -> Vec<Transform> {
        let mut frames = self.place(&self.pose(IDLE, 0.0), Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)));
        for (p, f) in build.parts.iter_mut().zip(&frames) {
            p.bind = f.rotation;
        }
        delirium::nmrs::asset_layout(build, &mut frames);
        frames
    }

    pub fn frames(&self, r: &Rise) -> Option<Vec<Transform>> {
        let d = self.clips[r.clip].0.duration;
        (r.t <= d || r.hold).then(|| {
            let k = (r.t / RISE_BLEND).min(1.0);
            self.place(&self.pose(r.clip, if r.hold { (r.t / d).fract() } else { r.t / d }), r.body).into_iter().zip(&r.lay)
                .map(|(n, l)| Transform::from_translation(l.translation.lerp(n.translation, k)).with_rotation(l.rotation.slerp(n.rotation, k)))
                .collect()
        })
    }

    pub fn end(&self, r: &Rise) -> Vec<Transform> {
        self.place(&self.pose(r.clip, 1.0), r.body)
    }
}
