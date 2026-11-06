use crate::{NM_E_ARG, NmAgent, NmBuild, fail};
use delirium::prelude::*;
use std::ffi::c_char;

pub struct NmRig {
    parents: Vec<i32>,
    part: Vec<i32>,
    rest_local: Vec<Quat>,
    off: Vec<Quat>,
    root: (usize, Vec3),
    take: Vec<Option<(usize, Transform)>>,
}

fn make(parents: &[i32], rest: &[Transform], part: &[i32], parts: &[Transform]) -> Result<NmRig, &'static str> {
    let n = parents.len();
    if (0..n).any(|b| parents[b] >= b as i32 || part[b] < -1 || part[b] >= parts.len() as i32) {
        return Err("each bone's parent must come before it (-1 for none), and its part be -1 or a part");
    }
    let rest_local = (0..n).map(|b| match parents[b] {
        -1 => rest[b].rotation,
        p => rest[p as usize].rotation.inverse() * rest[b].rotation,
    }).collect();
    let off = (0..n).map(|b| match part[b] {
        -1 => Quat::IDENTITY,
        k => parts[k as usize].rotation.inverse() * rest[b].rotation,
    }).collect();
    let Some(hips) = (0..n).find(|&b| part[b] == 0) else { return Err("no bone on part 0 (the hips)") };
    let root = (hips, parts[0].rotation.inverse() * (rest[hips].translation - parts[0].translation));
    let take = (0..parts.len()).map(|k| (0..n).find(|&b| part[b] == k as i32).map(|b| {
        let inv = rest[b].rotation.inverse();
        (b, Transform::from_translation(inv * (parts[k].translation - rest[b].translation)).with_rotation(inv * parts[k].rotation))
    })).collect();
    Ok(NmRig { parents: parents.to_vec(), part: part.to_vec(), rest_local, off, root, take })
}

fn pose(r: &NmRig, parts: &[Transform]) -> (Vec<Quat>, Vec3) {
    let n = r.parents.len();
    let (mut world, mut local) = (vec![Quat::IDENTITY; n], vec![Quat::IDENTITY; n]);
    for b in 0..n {
        let parent = (r.parents[b] >= 0).then(|| world[r.parents[b] as usize]);
        world[b] = match r.part[b] {
            -1 => parent.unwrap_or(Quat::IDENTITY) * r.rest_local[b],
            k => (parts[k as usize].rotation * r.off[b]).normalize(),
        };
        local[b] = parent.map_or(world[b], |p| p.inverse() * world[b]);
    }
    (local, parts[0].translation + parts[0].rotation * r.root.1)
}

pub fn part_for_name(name: &str) -> i32 {
    let name = name.split('[').next().unwrap_or("");
    let name = name.rsplit(':').next().unwrap_or(name);
    let mut words: Vec<String> = vec![];
    let mut prev_lower = false;
    for c in name.chars() {
        if !c.is_ascii_alphanumeric() {
            words.push(String::new());
            prev_lower = false;
            continue;
        }
        if words.is_empty() || c.is_ascii_uppercase() && prev_lower {
            words.push(String::new());
        }
        prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        words.last_mut().unwrap().push(c.to_ascii_lowercase());
    }
    words.retain(|w| !w.is_empty() && !["bip01", "bip001", "npc", "def", "mixamorig", "b", "jnt", "bone", "char", "bn"].contains(&w.as_str()));
    let has = |s: &str| words.iter().any(|w| w.starts_with(s));
    if ["finger", "thumb", "index", "middle", "ring", "pinky", "toe", "twist", "roll", "end", "nub", "top", "eye", "jaw", "ik", "pole"].iter().any(|s| has(s)) {
        return -1;
    }
    let left = words.iter().any(|w| w == "l" || w == "left" || w.starts_with("left"));
    let right = words.iter().any(|w| w == "r" || w == "right" || w.starts_with("right"));
    let joined: String = words.iter().filter(|w| !matches!(w.as_str(), "l" | "r" | "left" | "right")).cloned().collect();
    let joined = joined.trim_start_matches("left").trim_start_matches("right");
    let side = |l: i32, r: i32| if left == right { -1 } else if left { l } else { r };
    let digits: String = joined.chars().filter(char::is_ascii_digit).collect();
    let word = joined.trim_end_matches(|c: char| c.is_ascii_digit());
    match word {
        "hips" | "hip" | "pelvis" if !left && !right => 0,
        "upleg" | "upperleg" | "thigh" => side(1, 4),
        "leg" | "lowerleg" | "calf" | "shin" | "knee" => side(2, 5),
        "foot" | "ankle" => side(3, 6),
        "spine" => match (digits.len(), digits.parse::<i32>().unwrap_or(0)) {
            (2.., k) => 7 + (k - 1).clamp(0, 3),
            (_, k) => 7 + k.clamp(0, 3),
        },
        "chest" => 9,
        "upperchest" => 10,
        "neck" => 11,
        "head" => 12,
        "shoulder" | "clavicle" | "collar" | "clav" => side(13, 17),
        "arm" | "upperarm" | "uparm" | "armupper" => side(14, 18),
        "forearm" | "lowerarm" | "elbow" | "armlower" => side(15, 19),
        "hand" | "wrist" => side(16, 20),
        _ => -1,
    }
}

pub(crate) const PART_PARENT: [i32; 21] = [-1, 0, 1, 2, 0, 4, 5, 0, 7, 8, 9, 10, 11, 10, 13, 14, 15, 10, 17, 18, 19];

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rig_check(bones: i32, parents: *const i32, rest: *const f32, map: *const i32, buf: *mut c_char, len: i32) -> i32 {
    if bones <= 0 || parents.is_null() || rest.is_null() || map.is_null() {
        return fail(NM_E_ARG, "nm_rig_check: no bones");
    }
    let n = bones as usize;
    let (par, mut parts) = unsafe { (std::slice::from_raw_parts(parents, n), std::slice::from_raw_parts(map, n).to_vec()) };
    let mut lines = vec![];
    let mut j = [0.0f32; 63];
    if unsafe { nm_rig_joints(bones, parents, rest, parts.as_mut_ptr(), j.as_mut_ptr()) } < 0 {
        lines.push(crate::ERR.with(|e| e.borrow().clone()));
    } else {
        let g = |k: usize| Vec3::from_slice(&j[k * 3..k * 3 + 3]);
        if (g(1) - g(4)).dot(g(14) - g(18)) < 0.0 {
            lines.push("left and right swapped between the legs and the arms: fix the map".into());
        }
        let bend = |a: usize, b: usize, c: usize| (g(b) - g(a)).angle_between(g(c) - g(b)).to_degrees();
        for (what, a, b, c, most) in [("left knee", 1, 2, 3, 30.0), ("right knee", 4, 5, 6, 30.0), ("left elbow", 14, 15, 16, 60.0), ("right elbow", 18, 19, 20, 60.0)] {
            let d = bend(a, b, c);
            if d > most {
                lines.push(format!("{what} bent {d:.0} degrees in the rest pose: pass the bind pose (the mesh's neutral pose), not the animated one"));
            }
        }
    }
    let above = |p: i32, q: i32| {
        let mut k = PART_PARENT[p as usize];
        while k >= 0 && k != q {
            k = PART_PARENT[k as usize];
        }
        k == q
    };
    let name = |p: i32| delirium_data::nm_asset::PARTS.get(p as usize).copied().unwrap_or("?");
    for b in 0..n {
        let p = parts[b];
        if !(1..21).contains(&p) {
            continue;
        }
        let mut a = par[b];
        while a >= 0 && !(0..21).contains(&parts[a as usize]) {
            a = par[a as usize];
        }
        if let Some(q) = (a >= 0).then(|| parts[a as usize]).filter(|&q| q != p && !above(p, q)) {
            lines.push(format!("bone {b} ({}) is under bone {a} ({}): fix the map", name(p), name(q)));
        }
    }
    unsafe { crate::put(&lines.join("\n"), buf, len) };
    lines.len() as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rig_joints(bones: i32, parents: *const i32, rest: *const f32, map: *mut i32, joints: *mut f32) -> i32 {
    if bones <= 0 || parents.is_null() || rest.is_null() || map.is_null() || joints.is_null() {
        return fail(NM_E_ARG, "nm_rig_joints: no bones");
    }
    let n = bones as usize;
    let (parents, parts) = unsafe { (std::slice::from_raw_parts(parents, n), std::slice::from_raw_parts_mut(map, n)) };
    let at: Vec<Vec3> = unsafe { std::slice::from_raw_parts(rest, n * 3) }.chunks(3).map(Vec3::from_slice).collect();
    let Some(hips) = (0..n).find(|&b| parts[b] == 0) else { return fail(NM_E_ARG, "nm_rig_joints: no bone on part 0 (the hips)") };
    let spine = |p: i32| (7..=12).contains(&p);
    let limb = |p: i32| (1..=6).contains(&p) || p >= 13;
    for b in 0..n {
        let mut a = parents[b];
        while a >= 0 && spine(parts[b]) {
            if limb(parts[a as usize]) {
                parts[b] = -1;
            }
            a = parents[a as usize];
        }
    }
    if !parts.contains(&12) {
        let top = (0..n).filter(|&b| (7..=11).contains(&parts[b])).max_by(|&x, &y| {
            at[x].distance_squared(at[hips]).total_cmp(&at[y].distance_squared(at[hips]))
        });
        if let Some(t) = top {
            parts[t] = 12;
        }
    }
    let mut j = [None::<Vec3>; 21];
    for b in 0..n {
        if let Some(p @ 0..=20) = Some(parts[b]) {
            j[p as usize] = j[p as usize].or(Some(at[b]));
        }
    }
    for (hip, knee, foot) in [(1, 2, 3), (4, 5, 6)] {
        if let (Some(h), Some(f)) = (j[hip], j[foot]) {
            j[knee] = j[knee].or(Some(h.lerp(f, 0.5)));
        }
    }
    for k in [0, 1, 2, 3, 4, 5, 6, 12, 14, 15, 16, 18, 19, 20] {
        if j[k].is_none() {
            let name = delirium_data::nm_asset::PARTS.get(k).copied().unwrap_or("?");
            return fail(NM_E_ARG, &format!("nm_rig_joints: no bone for {name} (add it to the map)"));
        }
    }
    let chain = [0, 7, 8, 9, 10, 11, 12];
    for i in 1..chain.len() - 1 {
        if j[chain[i]].is_some() {
            continue;
        }
        let (lo, mut hi) = (i - 1, i + 1);
        while j[chain[hi]].is_none() {
            hi += 1;
        }
        j[chain[i]] = Some(j[chain[lo]].unwrap().lerp(j[chain[hi]].unwrap(), (i - lo) as f32 / (hi - lo) as f32));
    }
    let mut v: [Vec3; 21] = std::array::from_fn(|k| j[k].unwrap_or_default());
    let shoulders = (v[14] + v[18]) * 0.5;
    let up = shoulders - v[0];
    if (v[12] - v[0]).dot(up.normalize_or_zero()) < up.length() {
        v[12] = shoulders + up * 0.25;
        for (i, &c) in chain.iter().enumerate().take(chain.len() - 1).skip(1) {
            if (v[c] - v[0]).dot(up.normalize_or_zero()) > up.length() {
                v[c] = v[0].lerp(v[12], i as f32 / (chain.len() - 1) as f32);
            }
        }
    }
    if j[13].is_none() {
        v[13] = v[11].lerp(v[14], 0.3);
    }
    if j[17].is_none() {
        v[17] = v[11].lerp(v[18], 0.3);
    }
    let out = unsafe { std::slice::from_raw_parts_mut(joints, 63) };
    for k in 0..21 {
        out[k * 3..k * 3 + 3].copy_from_slice(&v[k].to_array());
    }
    0
}

fn legs(j: &[Vec3]) -> f32 {
    (j[1].distance(j[2]) + j[2].distance(j[3]) + j[4].distance(j[5]) + j[5].distance(j[6])) * 0.5
}

pub const NM_SIZE_REAL: i32 = 0;
pub const NM_SIZE_BASE: i32 = 1;
pub const NM_SIZE_AUTO: i32 = 2;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_size_scale(base: *const NmBuild, joints: *const f32, mode: i32) -> f32 {
    if base.is_null() || joints.is_null() {
        fail(NM_E_ARG, "nm_size_scale: null");
        return 0.0;
    }
    let j: Vec<Vec3> = unsafe { std::slice::from_raw_parts(joints, 63) }.chunks(3).map(Vec3::from_slice).collect();
    let a = unsafe { crate::nm_agent_create(base, [0.0f32; 3].as_ptr(), 0.0, 0.0) };
    if a.is_null() {
        return 0.0;
    }
    let mut b = vec![Vec3::ZERO; 7];
    for (i, p) in b.iter_mut().enumerate().skip(1) {
        let mut v = [0.0f32; 3];
        unsafe { crate::nm_agent_get_joint(a, i as i32, std::ptr::null_mut(), v.as_mut_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
        *p = Vec3::from_array(v);
    }
    unsafe { crate::nm_agent_destroy(a) };
    let k = legs(&j) / legs(&b);
    match mode {
        NM_SIZE_BASE => k,
        NM_SIZE_AUTO if !(0.9..=1.1).contains(&k) => k,
        _ => 1.0,
    }
}

fn frames(a: &NmAgent) -> Vec<Transform> {
    a.agent.doll.frames().collect()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rig_part_for_name(name: *const c_char) -> i32 {
    unsafe { crate::text(name) }.map_or(-1, part_for_name)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rig_create(a: *const NmAgent, bones: i32, parents: *const i32, rest: *const f32, map: *const i32) -> *mut NmRig {
    let Some(a) = (unsafe { a.as_ref() }) else {
        fail(NM_E_ARG, "nm_rig_create: null agent");
        return std::ptr::null_mut();
    };
    if bones <= 0 || parents.is_null() || rest.is_null() || map.is_null() {
        fail(NM_E_ARG, "nm_rig_create: no bones");
        return std::ptr::null_mut();
    }
    let n = bones as usize;
    let (parents, part) = unsafe { (std::slice::from_raw_parts(parents, n), std::slice::from_raw_parts(map, n)) };
    let rest: Vec<Transform> = unsafe { std::slice::from_raw_parts(rest, n * 7) }.chunks(7)
        .map(|c| Transform::from_translation(Vec3::new(c[0], c[1], c[2]) / a.u.len()).with_rotation(Quat::from_xyzw(c[3], c[4], c[5], c[6]).normalize())).collect();
    match make(parents, &rest, part, &frames(a)) {
        Ok(r) => Box::into_raw(Box::new(r)),
        Err(why) => {
            fail(NM_E_ARG, why);
            std::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rig_destroy(r: *mut NmRig) {
    if !r.is_null() {
        drop(unsafe { Box::from_raw(r) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rig_pose(r: *const NmRig, a: *const NmAgent, out: *mut f32) -> i32 {
    let Some(a) = (unsafe { a.as_ref() }) else { return fail(NM_E_ARG, "null agent") };
    let Some(r) = (unsafe { r.as_ref() }) else { return fail(NM_E_ARG, "null rig") };
    if out.is_null() {
        return fail(NM_E_ARG, "null out");
    }
    let (local, root) = pose(r, &frames(a));
    for (b, q) in local.iter().enumerate() {
        unsafe { crate::put4(out.add(b * 4), *q) };
    }
    unsafe { crate::put3(out.add(local.len() * 4), root * a.u.len()) };
    r.root.0 as i32
}

fn take(r: &NmRig, bones: &[Transform], parts: &[Transform]) -> Vec<Transform> {
    let on = |k: usize, b: &[Transform]| r.take[k].map(|(i, t)| Transform::from_translation(b[i].translation + b[i].rotation * t.translation)
        .with_rotation((b[i].rotation * t.rotation).normalize()));
    let hips = on(0, bones).unwrap_or(parts[0]);
    let back = hips.compute_affine() * parts[0].compute_affine().inverse();
    (0..parts.len()).map(|k| on(k, bones).unwrap_or_else(|| Transform::from_matrix(Mat4::from(back * parts[k].compute_affine())))).collect()
}

fn read_frames(p: *const f32, n: usize, len: f32) -> Vec<Transform> {
    unsafe { std::slice::from_raw_parts(p, n * 7) }.chunks(7)
        .map(|c| Transform::from_translation(Vec3::new(c[0], c[1], c[2]) / len).with_rotation(Quat::from_xyzw(c[3], c[4], c[5], c[6]).normalize())).collect()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rig_take(r: *const NmRig, a: *mut NmAgent, now: *const f32, before: *const f32, dt: f32) -> i32 {
    let Some(r) = (unsafe { r.as_ref() }) else { return fail(NM_E_ARG, "null rig") };
    let Some(a) = (unsafe { a.as_mut() }) else { return fail(NM_E_ARG, "null agent") };
    if now.is_null() || (!before.is_null() && !(dt > 0.0)) {
        return fail(NM_E_ARG, "nm_rig_take: no frames, or a `before` without dt > 0");
    }
    let (n, len) = (r.parents.len(), a.u.len());
    let parts = frames(a);
    let to = take(r, &read_frames(now, n, len), &parts);
    let from = (!before.is_null()).then(|| take(r, &read_frames(before, n, len), &parts));
    let u = a.u;
    for (k, t) in to.iter().enumerate() {
        let (v, w) = from.as_ref().map_or((Vec3::ZERO, Vec3::ZERO), |f| {
            let (axis, angle) = (t.rotation * f[k].rotation.inverse()).to_axis_angle();
            let angle = if angle > std::f32::consts::PI { angle - std::f32::consts::TAU } else { angle };
            ((t.translation - f[k].translation) * len / dt / u.time(), axis * angle / dt * u.time())
        });
        a.agent.doll.set_body(k, t.translation, t.rotation, v, w);
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nm_rig_incoming(r: *const NmRig, a: *mut NmAgent, now: *const f32) -> i32 {
    let Some(r) = (unsafe { r.as_ref() }) else { return fail(NM_E_ARG, "null rig") };
    let Some(a) = (unsafe { a.as_mut() }) else { return fail(NM_E_ARG, "null agent") };
    a.agent.incoming = if now.is_null() { vec![] } else { take(r, &read_frames(now, r.parents.len(), a.u.len()), &frames(a)) };
    0
}

