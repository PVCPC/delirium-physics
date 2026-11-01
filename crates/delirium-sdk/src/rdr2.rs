//! Red Dead Redemption 2 bodies: RDR2 keeps its Euphoria characters as plain XML (common_0.rpf/data/naturalmotion/
//! fred.xml, wilma.xml and size variants), read from the player's own copy. This turns one into an `nm-character-1`
//! body on top of a base body (Homer's, from the player's GTA IV): RDR2's part frames, joint limits, motor flips,
//! muscle stiffness / strength / damping and force caps; the base's joint offsets (re-placed on RDR2's parts), masses
//! and self-collision, which the XML doesn't hold. Nothing here runs unless asked for; existing bodies are unchanged.
use crate::character::{BuildPart, Character, Joint, Part, FORMAT};
use crate::*;

/// Delirium part index -> RDR2 part number (RDR2 numbers its parts breadth-first).
const PART_OF: [usize; 21] = [0, 1, 4, 5, 2, 6, 7, 3, 8, 9, 10, 13, 20, 11, 14, 15, 16, 12, 17, 18, 19];
/// Each Delirium part's parent part (joint j joins part j + 1 to this).
const PARENT: [usize; 21] = [0, 0, 1, 2, 0, 4, 5, 0, 7, 8, 9, 10, 11, 10, 13, 14, 15, 10, 17, 18, 19];

type M3 = [[f32; 3]; 3];

fn tag<'a>(s: &'a str, t: &str) -> Option<&'a str> {
    let a = s.find(&format!("<{t}>"))? + t.len() + 2;
    Some(s[a..].split('<').next()?.trim())
}

fn num(s: &str, t: &str) -> Option<f32> {
    tag(s, t)?.parse().ok()
}

/// The XML's joint blocks: (name, is three-dof, body).
fn joints(xml: &str) -> Vec<(String, bool, &str)> {
    let mut out = vec![];
    for (kind, three) in [("threedof", true), ("onedof", false)] {
        let open = format!("<{kind} name=\"");
        let mut rest = xml;
        while let Some(i) = rest.find(&open) {
            let r = &rest[i + open.len()..];
            let name = r.split('"').next().unwrap_or("").to_string();
            let end = r.find(&format!("</{kind}>")).unwrap_or(r.len());
            out.push((name, three, &r[..end]));
            rest = &r[end..];
        }
    }
    out
}

/// The XML's part frames by part number: rotation rows and translation.
fn parts(xml: &str) -> Vec<Option<(M3, [f32; 3])>> {
    let mut out = vec![None; 21];
    let mut rest = xml;
    while let Some(i) = rest.find("<bodyPart name=\"") {
        let r = &rest[i + 16..];
        let end = r.find("</bodyPart>").unwrap_or(r.len());
        let b = &r[..end];
        let rows: Vec<[f32; 4]> = b.split("<r").skip(1).map(|row| {
            let v = |c: &str| row.split(&format!("{c}=\"")).nth(1).and_then(|x| x.split('"').next()).and_then(|x| x.parse().ok()).unwrap_or(0.0);
            [v("c0"), v("c1"), v("c2"), v("c3")]
        }).collect();
        if let (Some(n), true) = (num(b, "partNumber"), rows.len() == 4) {
            if (n as usize) < 21 {
                out[n as usize] = Some(([[rows[0][0], rows[0][1], rows[0][2]], [rows[1][0], rows[1][1], rows[1][2]],
                    [rows[2][0], rows[2][1], rows[2][2]]], [rows[3][0], rows[3][1], rows[3][2]]));
            }
        }
        rest = &r[end..];
    }
    out
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
/// World -> part local (the rows are the part's axes).
fn to_local(r: &M3, v: [f32; 3]) -> [f32; 3] { [dot(r[0], v), dot(r[1], v), dot(r[2], v)] }
/// Part local -> world.
fn to_world(r: &M3, v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|k| r[0][k] * v[0] + r[1][k] * v[1] + r[2][k] * v[2])
}
fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt().max(1e-9);
    v.map(|x| x / l)
}
/// Angle between two rotations (from the trace of a·bᵀ), radians.
fn angle(a: &M3, b: &M3) -> f32 {
    let t: f32 = (0..3).map(|i| dot(a[i], b[i])).sum();
    ((t - 1.0) * 0.5).clamp(-1.0, 1.0).acos()
}
fn rows(f: &[[f32; 3]; 4]) -> M3 { [f[0], f[1], f[2]] }

impl Character {
    /// An RDR2 body (`xml`: one of RDR2's naturalmotion XML files) on top of `base` (21 parts, 20 joints).
    pub fn from_rdr2(xml: &str, base: &Character) -> Result<Character, String> {
        if !xml.contains("<bodyType>biped</bodyType>") {
            return Err("not an RDR2 biped body (the horse isn't supported)".into());
        }
        let rp = parts(xml);
        if rp.iter().any(Option::is_none) {
            return Err("needs 21 body parts".into());
        }
        let rj = joints(xml);
        // RDR2's frames are around its pelvis; put its pelvis where the base's is.
        let shift = base.parts[0].frame[3];
        let frame = |d: usize| -> (M3, [f32; 3]) {
            let (r, t) = rp[PART_OF[d]].unwrap();
            (r, add(t, shift))
        };
        let mut joints_out = Vec::with_capacity(20);
        for bj in &base.joints {
            let j = bj.index as usize;
            let (child, parent) = (j + 1, PARENT[j + 1]);
            let alias = match bj.name.as_str() {
                "clavicle_left" => "clavicle_jnt_left",
                "clavicle_right" => "clavicle_jnt_right",
                n => n,
            };
            let Some((_, three, b)) = rj.iter().find(|(n, _, _)| n == alias || (bj.name == "neck_upper" && n == "upper_neck"))
            else { return Err(format!("no RDR2 joint for {}", bj.name)) };
            if *three != bj.three {
                return Err(format!("{}: RDR2 and base disagree on its joint type", bj.name));
            }
            let (bc, bp) = (&base.parts[child].frame, &base.parts[parent].frame);
            let ((rc, tc), (rpar, tpar)) = (frame(child), frame(parent));
            // a mirrored RDR2 frame (e.g. a clavicle) can't carry the base's offsets: use the parent's alone then
            let flipped = angle(&rows(bc), &rc) > std::f32::consts::FRAC_PI_2;
            let via_parent = add(tpar, to_world(&rpar, to_local(&rows(bp), sub(bj.pos, bp[3]))));
            let via_child = add(tc, to_world(&rc, to_local(&rows(bc), sub(bj.pos, bc[3]))));
            let pos = if flipped { via_parent } else { [0, 1, 2].map(|k| 0.5 * (via_parent[k] + via_child[k])) };
            let (ar, br) = if flipped { (&rpar, rows(bp)) } else { (&rc, rows(bc)) };
            let axis = |a: [f32; 3]| unit(to_world(ar, to_local(&br, a)));
            let lim = if *three {
                [("minfirstleanangle", "maxfirstleanangle"), ("minsecondleanangle", "maxsecondleanangle"), ("mintwistangle", "maxtwistangle")]
                    .map(|(lo, hi)| [num(b, lo).unwrap_or(0.0), num(b, hi).unwrap_or(0.0)])
            } else {
                [[num(b, "minangle").unwrap_or(0.0), num(b, "maxangle").unwrap_or(0.0)], [0.0; 2], [0.0; 2]]
            };
            let flag = |t: &str| tag(b, t) == Some("true");
            joints_out.push(Joint {
                name: bj.name.clone(), index: bj.index, three: bj.three, pos,
                twist_axis: axis(bj.twist_axis), lean_axis: axis(bj.lean_axis), limits: lim,
                flip: [flag("reverseFirstLeanMotor"), flag("reverseSecondLeanMotor"), flag("reverseTwistMotor")],
                muscle_stiffness: num(b, "defaultMuscleStiffness").unwrap_or(bj.muscle_stiffness),
                damping: num(b, "defaultMuscleDamping").unwrap_or(bj.damping),
                strength: num(b, "defaultMuscleStrength").unwrap_or(bj.strength),
                max_torque: [num(b, "defaultLeanForceCap").unwrap_or(bj.max_torque[0]),
                             num(b, "defaultTwistForceCap").unwrap_or(bj.max_torque[1])],
            });
        }
        let parts_out = base.parts.iter().enumerate().map(|(d, bp)| {
            let (r, t) = frame(d);
            Part { name: bp.name.clone(), frame: [r[0], r[1], r[2], t], mass: bp.mass }
        }).collect();
        Ok(Character { format: FORMAT.into(), joints: joints_out, parts: parts_out, self_collision: base.self_collision.clone(), build: None })
    }

    /// The 21 positions `nm_build_fit` wants: each part's joint with its parent, the root's at the pelvis.
    pub fn fit_points(&self) -> Vec<[f32; 3]> {
        let mut p = vec![self.parts[0].frame[3]; 21];
        for j in &self.joints {
            p[j.index as usize + 1] = j.pos;
        }
        p
    }
}

/// An RDR2 body as `nm-character-1` JSON: `xml_path` one of RDR2's naturalmotion XML files (fred.xml, wilma.xml...),
/// `base_json` a base body (from nm_character_from_xrt); with `build_path` (the base's build dump) the result also
/// carries a build fitted to RDR2's proportions. Writes the JSON into `buf` like nm_character_from_xrt.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_character_from_rdr2(xml_path: *const c_char, base_json: *const c_char, build_path: *const c_char,
                                                buf: *mut c_char, len: i32) -> i32 {
    let (Some(xp), Some(bj)) = (unsafe { text(xml_path) }, unsafe { text(base_json) }) else {
        return fail(NM_E_ARG, "nm_character_from_rdr2: no path or no base");
    };
    let xml = match std::fs::read_to_string(xp) {
        Ok(t) => t,
        Err(e) => return fail(NM_E_IO, format!("{xp}: {e}")),
    };
    let base = match Character::parse(bj) {
        Ok(c) => c,
        Err(e) => return fail(NM_E_PARSE, format!("nm_character_from_rdr2: base: {e}")),
    };
    let mut c = match Character::from_rdr2(&xml, &base) {
        Ok(c) => c,
        Err(e) => return fail(NM_E_PARSE, format!("nm_character_from_rdr2: {e}")),
    };
    if let Some(bp) = unsafe { text(build_path) } {
        let Ok(t) = std::fs::read_to_string(bp) else { return fail(NM_E_IO, format!("{bp}: unreadable")) };
        let Ok((b, f)) = std::panic::catch_unwind(move || Build::undump(&t)) else {
            return fail(NM_E_PARSE, "nm_character_from_rdr2: not a build dump");
        };
        let pts: Vec<Vec3> = c.fit_points().iter().map(|p| Vec3::from_array(*p)).collect();
        let (fb, ff) = b.fit(&f, &pts);
        c.build = Some(fb.parts.iter().zip(&ff).map(|(p, fr)| BuildPart::from_def(p, fr)).collect());
    }
    match serde_json::to_string_pretty(&c) {
        Ok(s) => unsafe { put(&s, buf, len) },
        Err(e) => fail(NM_E_PARSE, e.to_string()),
    }
}


/// The RDR2 reactions option every agent shares (nm_rdr2_set); Delirium's own reactions are unchanged unless a caller
/// asks for these (nm_agent_rdr2_shot / nm_agent_rdr2_melee).
static REACTIONS: std::sync::Mutex<Option<delirium::extras_rdr2::Reactions>> = std::sync::Mutex::new(None);

fn reactions() -> delirium::extras_rdr2::Reactions {
    REACTIONS.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or_default()
}

/// Sends what the RDR2 option picked, in order; the count started.
unsafe fn start_all(a: *mut NmAgent, w: *const NmWorld, o: Vec<delirium::extras_rdr2::Start>) -> i32 {
    let (Some(a), Some(w)) = (unsafe { a.as_mut() }, unsafe { w.as_ref() }) else { return fail(NM_E_ARG, "null agent or world") };
    let mut st = std::mem::take(&mut a.agent.msgs);
    let n = o.iter().filter(|(m, ps)| msg::send(&mut a.agent, &a.build, &w.0, &mut st, m, ps).is_ok()).count();
    a.agent.msgs = st;
    n as i32
}

/// The RDR2 reactions option, for nm_agent_rdr2_shot / nm_agent_rdr2_melee (every agent): `name` one of "snap",
/// "hitLocation", "melee", "balance" (0 off, 1 on), "spin", "knees", "reach" (chances, 0..1). Defaults: all on, spin
/// 0.25, knees 0.15, reach 0.7.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rdr2_set(name: *const c_char, value: f32) -> i32 {
    let mut r = reactions();
    let on = value != 0.0;
    match unsafe { text(name) } {
        Some("snap") => r.snap = on,
        Some("hitLocation") => r.hit_location = on,
        Some("melee") => r.melee = on,
        Some("balance") => r.balance = on,
        Some("spin") => r.spin = value.clamp(0.0, 1.0),
        Some("knees") => r.knees = value.clamp(0.0, 1.0),
        Some("reach") => r.reach = value.clamp(0.0, 1.0),
        _ => return fail(NM_E_PARAM, "nm_rdr2_set: no such option"),
    }
    *REACTIONS.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
    0
}

/// RDR2's reactions to a bullet, started after the shot itself (nm_agent_task_shot): part `part` hit at `at`
/// (world), the bullet going along `dir`. By where it hit: shotFromBehind, shotInGuts, shotInjuredLeg or
/// shotInjuredArm; shotSnap; by chance shotSpin or shotFallToKnees, and reachForWound; rdr2Balance stays on (see
/// nm_rdr2_set). Returns how many messages started.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_rdr2_shot(a: *mut NmAgent, w: *const NmWorld, part: i32, at: *const f32, dir: *const f32) -> i32 {
    let Some(g) = (unsafe { a.as_ref() }) else { return fail(NM_E_ARG, "null agent") };
    if at.is_null() || dir.is_null() || !(0..g.build.parts.len().min(21) as i32).contains(&part) {
        return fail(NM_E_ARG, "bad hit");
    }
    // facing: (left clavicle - right clavicle) x up
    let fwd = (g.agent.doll.centre(13) - g.agent.doll.centre(17)).cross(Vec3::Z);
    let o = reactions().on_shot(&mut RAND.lock().unwrap_or_else(|e| e.into_inner()), part as usize, unsafe { v3(at) }, unsafe { v3(dir) }, fwd);
    unsafe { start_all(a, w, o) }
}

/// RDR2's melee reaction (meleeReact, softened so a punch staggers and rarely drops) to a blow on part `part` at
/// `at` (world) along `dir`, instead of a flinch; rdr2Balance stays on (see nm_rdr2_set). Returns how many messages
/// started (0 with "melee" off).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_agent_rdr2_melee(a: *mut NmAgent, w: *const NmWorld, part: i32, at: *const f32, dir: *const f32) -> i32 {
    let Some(g) = (unsafe { a.as_ref() }) else { return fail(NM_E_ARG, "null agent") };
    if at.is_null() || dir.is_null() || !(0..g.build.parts.len().min(21) as i32).contains(&part) {
        return fail(NM_E_ARG, "bad hit");
    }
    let o = reactions().on_melee(&mut RAND.lock().unwrap_or_else(|e| e.into_inner()), part as usize, unsafe { v3(at) }, unsafe { v3(dir) });
    unsafe { start_all(a, w, o) }
}
