use crate::*;
use delirium_data::nm_asset;
use delirium::ragdoll::{Limit, PartDef};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "nm-character-1";

#[derive(Serialize, Deserialize)]
pub struct Character {
    pub format: String,
    pub joints: Vec<Joint>,
    pub parts: Vec<Part>,
    pub self_collision: Vec<[usize; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<Vec<BuildPart>>,
}

#[derive(Serialize, Deserialize)]
pub struct Joint {
    pub name: String,
    pub index: u32,
    pub three: bool,
    pub pos: [f32; 3],
    pub twist_axis: [f32; 3],
    pub lean_axis: [f32; 3],
    pub limits: [[f32; 2]; 3],
    pub flip: [bool; 3],
    pub muscle_stiffness: f32,
    pub damping: f32,
    pub strength: f32,
    pub max_torque: [f32; 2],
}

#[derive(Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub frame: [[f32; 3]; 4],
    pub mass: f32,
}

#[derive(Serialize, Deserialize)]
pub struct Frame {
    pub pos: [f32; 3],
    pub rot: [f32; 4],
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JointLimit {
    Cone { swing: f32, twist: f32 },
    Hinge { axis: [f32; 3], lo: f32, hi: f32, twist: f32 },
    Bone { parent_frame: [f32; 4], child_frame: [f32; 4], lean: [f32; 2], twist: f32 },
    BoneHinge { parent_frame: [f32; 4], child_frame: [f32; 4], lo: f32, hi: f32 },
    Locked,
}

#[derive(Serialize, Deserialize)]
pub struct BuildPart {
    pub name: String,
    pub parent: Option<usize>,
    pub mass: f32,
    pub radius: f32,
    pub half: f32,
    pub offset: Frame,
    pub rest: [f32; 4],
    pub anchor_parent: [f32; 3],
    pub anchor: [f32; 3],
    pub limit: JointLimit,
    pub lower: bool,
    pub reach: bool,
    pub bind: [f32; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boxed: Option<[f32; 3]>,
    pub inertia: [f32; 3],
    pub frame: Frame,
}

fn q(v: [f32; 4]) -> Quat {
    Quat::from_array(v)
}

fn fr(t: &Transform) -> Frame {
    Frame { pos: t.translation.to_array(), rot: t.rotation.to_array() }
}

impl Character {
    pub fn from_xrt(d: &[u8], build: Option<(&Build, &[Transform])>) -> Character {
        let joints = nm_asset::read(d).into_iter().map(|j| Joint {
            name: j.name, index: j.index, three: j.three, pos: j.pos, twist_axis: j.twist_axis, lean_axis: j.lean_axis,
            limits: j.limits.map(|(a, b)| [a, b]), flip: j.flip, muscle_stiffness: j.muscle_stiffness, damping: j.damping,
            strength: j.strength, max_torque: j.max_torque,
        }).collect();
        let parts = nm_asset::parts(d).into_iter().zip(nm_asset::masses(d)).zip(nm_asset::PARTS)
            .map(|((frame, mass), name)| Part { name: name.into(), frame, mass }).collect();
        let build = build.map(|(b, frames)| b.parts.iter().zip(frames).map(|(p, f)| BuildPart {
            name: p.name.clone(), parent: p.parent, mass: p.mass, radius: p.radius, half: p.half, offset: fr(&p.offset),
            rest: p.rest.to_array(), anchor_parent: p.anchor_parent.to_array(), anchor: p.anchor.to_array(),
            limit: match p.limit {
                Limit::Cone { swing, twist } => JointLimit::Cone { swing, twist },
                Limit::Hinge { axis, lo, hi, twist } => JointLimit::Hinge { axis: axis.to_array(), lo, hi, twist },
                Limit::Bone { pf, cf, lean, twist } => JointLimit::Bone { parent_frame: pf.to_array(), child_frame: cf.to_array(), lean, twist },
                Limit::BoneHinge { pf, cf, lo, hi } => JointLimit::BoneHinge { parent_frame: pf.to_array(), child_frame: cf.to_array(), lo, hi },
                Limit::Locked => JointLimit::Locked,
            },
            lower: p.lower, reach: p.reach, bind: p.bind.to_array(), boxed: p.boxed.map(|e| e.to_array()),
            inertia: p.inertia.to_array(), frame: fr(f),
        }).collect());
        Character { format: FORMAT.into(), joints, parts, self_collision: nm_asset::self_collision(d).into_iter().map(|(a, b)| [a, b]).collect(), build }
    }

    pub fn parse(t: &str) -> Result<Character, String> {
        let c: Character = serde_json::from_str(t).map_err(|e| e.to_string())?;
        if c.format != FORMAT {
            return Err(format!("format {:?}, not {FORMAT:?}", c.format));
        }
        if c.joints.len() != 20 || c.parts.len() != 21 || c.build.as_ref().is_some_and(|b| b.len() != 21) {
            return Err("needs 21 parts and 20 joints".into());
        }
        if c.self_collision.iter().flatten().any(|&k| k >= 21) {
            return Err("self_collision: no such part".into());
        }
        Ok(c)
    }

    pub fn asset(&self) -> (Vec<nm_asset::Joint>, Vec<[[f32; 3]; 4]>, Vec<f32>, Vec<(usize, usize)>) {
        let mut joints: Vec<nm_asset::Joint> = self.joints.iter().map(|j| nm_asset::Joint {
            name: j.name.clone(), index: j.index, three: j.three, pos: j.pos, twist_axis: j.twist_axis, lean_axis: j.lean_axis,
            limits: j.limits.map(|[a, b]| (a, b)), flip: j.flip, muscle_stiffness: j.muscle_stiffness, damping: j.damping,
            strength: j.strength, max_torque: j.max_torque,
        }).collect();
        joints.sort_by_key(|j| j.index);
        (joints, self.parts.iter().map(|p| p.frame).collect(), self.parts.iter().map(|p| p.mass).collect(),
         self.self_collision.iter().map(|&[a, b]| (a, b)).collect())
    }

    pub fn build(&self) -> Option<(Build, Vec<Transform>)> {
        let parts = self.build.as_ref()?;
        let tf = |f: &Frame| Transform::from_translation(Vec3::from_array(f.pos)).with_rotation(q(f.rot));
        let mut frames: Vec<Transform> = parts.iter().map(|p| tf(&p.frame)).collect();
        let parts = parts.iter().enumerate().map(|(i, p)| PartDef {
            bone: i, parent: p.parent, mass: p.mass, radius: p.radius, half: p.half, offset: tf(&p.offset), rest: q(p.rest),
            anchor_parent: Vec3::from_array(p.anchor_parent), anchor: Vec3::from_array(p.anchor),
            limit: match p.limit {
                JointLimit::Cone { swing, twist } => Limit::Cone { swing, twist },
                JointLimit::Hinge { axis, lo, hi, twist } => Limit::Hinge { axis: Vec3::from_array(axis), lo, hi, twist },
                JointLimit::Bone { parent_frame, child_frame, lean, twist } => Limit::Bone { pf: q(parent_frame), cf: q(child_frame), lean, twist },
                JointLimit::BoneHinge { parent_frame, child_frame, lo, hi } => Limit::BoneHinge { pf: q(parent_frame), cf: q(child_frame), lo, hi },
                JointLimit::Locked => Limit::Locked,
            },
            lower: p.lower, reach: p.reach, name: p.name.clone(), bind: q(p.bind), boxed: p.boxed.map(Vec3::from_array),
            inertia: Vec3::from_array(p.inertia), size: 1.0, heft: 1.0, grown: 1.0,
        }).collect();
        let mut build = Build { parts, nm: delirium::nmrs::HOMER };
        delirium::nmrs::asset_layout(&mut build, &mut frames);
        Some((build, frames))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_init_character(json: *const c_char) -> i32 {
    let Some(t) = (unsafe { text(json) }) else { return fail(NM_E_ARG, "nm_init_character: no text") };
    let c = match Character::parse(t) {
        Ok(c) => c,
        Err(e) => return fail(NM_E_PARSE, format!("nm_init_character: {e}")),
    };
    let fresh = delirium::nmrs::ASSET[delirium::nmrs::HOMER].get().is_none();
    let (j, p, m, s) = c.asset();
    delirium::nmrs::set(delirium::nmrs::HOMER, j, p, m, s);
    if fresh { 0 } else { 1 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_init_character_file(path: *const c_char) -> i32 {
    let Some(p) = (unsafe { text(path) }) else { return fail(NM_E_ARG, "nm_init_character_file: no path") };
    match std::fs::read_to_string(p).map(std::ffi::CString::new) {
        Ok(Ok(c)) => unsafe { nm_init_character(c.as_ptr()) },
        Ok(Err(_)) => fail(NM_E_PARSE, "nm_init_character_file: NUL in file"),
        Err(e) => fail(NM_E_IO, format!("{p}: {e}")),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_build_from_character(json: *const c_char) -> *mut NmBuild {
    let Some(t) = (unsafe { text(json) }) else {
        fail(NM_E_ARG, "nm_build_from_character: no text");
        return std::ptr::null_mut();
    };
    match Character::parse(t).map(|c| c.build()) {
        Ok(Some((build, frames))) => Box::into_raw(Box::new(NmBuild { build, frames })),
        Ok(None) => {
            fail(NM_E_PARSE, "nm_build_from_character: the file has no build");
            std::ptr::null_mut()
        }
        Err(e) => {
            fail(NM_E_PARSE, format!("nm_build_from_character: {e}"));
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_character_from_xrt(xrt_path: *const c_char, build_path: *const c_char, buf: *mut c_char, len: i32) -> i32 {
    let Some(p) = (unsafe { text(xrt_path) }) else { return fail(NM_E_ARG, "nm_character_from_xrt: no path") };
    let d = match std::fs::read(p) {
        Ok(d) => d,
        Err(e) => return fail(NM_E_IO, format!("{p}: {e}")),
    };
    let build = match unsafe { text(build_path) } {
        None => None,
        Some(bp) => match std::fs::read_to_string(bp) {
            Ok(t) => match std::panic::catch_unwind(move || Build::undump(&t)) {
                Ok(b) => Some(b),
                Err(_) => return fail(NM_E_PARSE, "nm_character_from_xrt: not a build dump"),
            },
            Err(e) => return fail(NM_E_IO, format!("{bp}: {e}")),
        },
    };
    let Ok(c) = std::panic::catch_unwind(|| Character::from_xrt(&d, build.as_ref().map(|(b, f)| (b, &f[..])))) else {
        return fail(NM_E_PARSE, "nm_character_from_xrt: not an NM character file");
    };
    match serde_json::to_string_pretty(&c) {
        Ok(s) => unsafe { put(&s, buf, len) },
        Err(e) => fail(NM_E_PARSE, e.to_string()),
    }
}

