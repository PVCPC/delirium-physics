use super::{Horse, LEGS, MID};
use bevy::prelude::*;
use delirium::ragdoll::{Limit, GRAVITY};

pub static ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn on() -> bool {
    ON.load(std::sync::atomic::Ordering::Relaxed) || std::env::var("HORSE_BRAIN").is_ok_and(|v| v == "ours")
}

pub static HYBRID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn hybrid() -> bool {
    HYBRID.load(std::sync::atomic::Ordering::Relaxed) || std::env::var("HORSE_BRAIN").is_ok_and(|v| v == "hybrid")
}

const MU: f32 = 0.6;
const K_HEIGHT: f32 = 40.0;
const K_CENTRE: f32 = 12.0;
const ZETA: f32 = 1.0;
const K_FOOT: f32 = 200.0;
const K_LEVEL: f32 = 3.0;
const D_LEVEL: f32 = 0.8;

#[derive(Clone, Default)]
pub struct Ours {
    height: f32,
    pose: Vec<Quat>,
    spot: [Option<Vec3>; 4],
    home: [Vec2; 4],
    step: Option<Step>,
    pair: Option<Step>,
    diag: bool,
    trot: f32,
    last: Option<usize>,
    rest: f32,
    air: [f32; 4],
    pub t: f32,
}

#[derive(Clone, Copy)]
struct Step {
    leg: usize,
    t: f32,
    from: Option<Vec3>,
    to: Vec3,
    dur: f32,
    lift: f32,
}

impl Horse {
    pub(super) fn step_ours(&mut self, dt: f32) -> bool {
        let fr: Vec<Transform> = self.doll.frames().collect();
        let up = fr[MID].rotation * self.stand;
        let hooves: Vec<Vec3> = LEGS.iter().map(|l| fr[l[2]].translation).collect();
        let (com, vel) = self.doll.com();
        let floor = hooves.iter().map(|h| h.z).fold(f32::MAX, f32::min);
        let mut o = self.ours.take().unwrap_or_else(|| {
            if self.core.is_some() { self.rest_rdr2(); }
            if self.held { self.held = false; self.doll.unpin(); }
            let pose: Vec<Quat> = (0..fr.len()).map(|k| self.build.parts[k].parent.map_or(Quat::IDENTITY, |p| fr[p].rotation.inverse() * fr[k].rotation)).collect();
            let yaw = heading(&fr);
            let home = std::array::from_fn(|i| Vec2::from_angle(-yaw).rotate((hooves[i] - com).truncate()));
            if std::env::var_os("HORSE_OURSJ").is_some() {
                for l in LEGS { for &k in &l[1..] { if let Limit::BoneHinge { pf, cf, lo, hi, .. } = self.build.parts[k].limit {
                    let q: Quat = pf.inverse() * pose[k] * cf;
                    let (ax, an) = q.to_axis_angle();
                    eprintln!("hinge {k}: now {:.2} (axis z {:.2}) range {lo:.2}..{hi:.2}", an * ax.z.signum(), ax.z);
                } } }
            }
            Ours { height: com.z - floor, pose, spot: [None; 4], home, step: None, pair: None, diag: false, trot: 0.0, last: None, rest: 0.0, air: [0.0; 4], t: 0.0 }
        });
        if up.z < 0.5 || com.z - floor < 0.6 * o.height {
            if std::env::var_os("HORSE_OURSLOG").is_some() { eprintln!("ours: down at {:.2}, RDR2's brain has it", o.t); }
            return false;
        }
        (self.held, self.hold) = (false, false);
        o.t += dt;
        let m = self.doll.mass();
        let n = self.build.parts.len();
        let (mut force, mut torque) = (vec![Vec3::ZERO; n], vec![Vec3::ZERO; n]);

        let planted: Vec<usize> = (0..4).filter(|&i| {
            let k = LEGS[i][2];
            self.doll.touch[k] & 0x20 != 0 && self.doll.hit[k].is_some_and(|h| h.1.z > 0.5)
        }).collect();
        let unit = o.height / 0.905;
        let mut planted = planted;
        o.rest += dt;
        self.plan_step(&mut o, &fr, &hooves, com, vel, unit, &planted, dt);
        for st in [o.step, o.pair].into_iter().flatten().filter(|s| s.from.is_some()) { planted.retain(|&i| i != st.leg); }
        let hover = HOVER * unit * kx("HORSE_OURSHOVER");
        for i in 0..4 {
            if !planted.contains(&i) && o.air[i] < 0.3 && hooves[i].z - floor < hover && o.step.is_none_or(|s| s.leg != i) && o.pair.is_none_or(|s| s.leg != i) { planted.push(i); }
        }
        for i in 0..4 {
            if !planted.contains(&i) { o.spot[i] = None; } else if o.spot[i].is_none() { o.spot[i] = Some(hooves[i]); }
        }
        if !planted.is_empty() {
            let d = |k: f32| 2.0 * ZETA * k.sqrt();
            let kx = |n: &str| std::env::var(n).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
            let (k_level, k_centre) = (K_LEVEL * kx("HORSE_OURSL"), K_CENTRE * kx("HORSE_OURSC"));
            let mut mid = planted.iter().map(|&i| hooves[i]).sum::<Vec3>() / planted.len() as f32;
            if o.step.is_none() && kx("HORSE_OURSALL") != 0.0 { mid = (hooves.iter().sum::<Vec3>() / 4.0).with_z(mid.z); }
            if kx("HORSE_OURSNAT") != 0.0 && o.step.is_none_or(|_| o.pair.is_some()) {
                let turn = Vec2::from_angle(heading(&fr));
                let over: &[usize] = if o.step.is_none() { &[0, 1, 2, 3] } else { &planted };
                let at = over.iter().map(|&i| hooves[i].truncate() - turn.rotate(o.home[i])).sum::<Vec2>() / over.len() as f32;
                mid = at.extend(mid.z);
            }
            if let Some(st) = o.step.filter(|_| o.pair.is_none()) {
                let three: Vec<Vec2> = (0..4).filter(|&j| j != st.leg).map(|j| hooves[j].truncate()).collect();
                mid = over(&three, com.truncate(), MARGIN * unit).extend(mid.z);
            }
            let fz = m * (GRAVITY() + K_HEIGHT * (o.height - (com.z - mid.z)) - d(K_HEIGHT) * vel.z);
            let fz = fz.clamp(0.0, 2.5 * m * GRAVITY());
            let fxy = (m * (k_centre * (mid - com) - d(k_centre) * vel)).with_z(0.0);
            let fxy = fxy.clamp_length_max(MU * fz);
            let (_, w) = self.doll.vel(MID);
            let tau = (up.cross(Vec3::Z) * k_level - w.with_z(0.0) * D_LEVEL * k_level.sqrt() / K_LEVEL.sqrt()) * m * o.height;
            let mut cop = com.truncate() + Vec2::new(-tau.y, tau.x) / fz.max(1.0);
            if let Some(st) = o.step.filter(|s| s.from.is_none()) {
                let three = (0..4).filter(|&j| j != st.leg).map(|j| hooves[j].truncate()).sum::<Vec2>() / 3.0;
                cop = cop.lerp(three, (st.t / SHIFT).min(1.0));
            }
            let share = shares(&planted.iter().map(|&i| hooves[i].truncate()).collect::<Vec<_>>(), cop);
            for (s, &i) in share.iter().zip(&planted) {
                let k_foot = K_FOOT * std::env::var("HORSE_OURSF").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                let (hv, _) = self.doll.vel(LEGS[i][2]);
                let hold = o.spot[i].map_or(Vec3::ZERO, |a| ((a - hooves[i]) * k_foot - hv * 2.0 * k_foot.sqrt()).with_z(0.0) * m * 0.25);
                let side = (fxy * s + hold).clamp_length_max(MU * fz * s);
                let f = Vec3::Z * fz * s + side;
                if !f.is_finite() { continue; }
                let foot = self.doll.hit[LEGS[i][2]].map_or(hooves[i], |h| h.0);
                for &k in LEGS[i].iter().rev() {
                    let Some(p) = self.build.parts[k].parent else { continue };
                    let joint = fr[k].transform_point(self.build.parts[k].anchor);
                    if std::env::var_os("HORSE_OURSJ").is_some() && o.t < 0.05 {
                        eprintln!("joint {k}<-{p}: from child {joint:.3} from parent {:.3} foot {foot:.3}", fr[p].transform_point(self.build.parts[k].anchor_parent));
                    }
                    let t = -(foot - joint).cross(f) * std::env::var("HORSE_OURSK").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
                    torque[k] += t;
                    torque[p] -= t;
                }
            }
            let hip = tau * kx("HORSE_OURSH") / planted.len() as f32;
            for &i in &planted {
                let k = LEGS[i][0];
                if let Some(p) = self.build.parts[k].parent { torque[p] += hip; torque[k] -= hip; }
            }
            if std::env::var_os("HORSE_OURSLOG").is_some() && (o.t * 20.0) as i32 != ((o.t - dt) * 20.0) as i32 {
                eprintln!("ours {:.2}: planted {planted:?} share {:?} h {:.3}/{:.3} off {:.3} v {:.2} vz {:.2} fz/mg {:.2} tilt {:.3}", o.t,
                    share.iter().map(|s| (s * 100.0).round() / 100.0).collect::<Vec<_>>(), com.z - mid.z, o.height,
                    (mid - com).truncate().length(), vel.length(), vel.z, fz / (m * GRAVITY()), up.z.acos());
                eprintln!("ours hooves {:?}", hooves.iter().map(|h| ((h.x * 100.0).round() / 100.0, (h.y * 100.0).round() / 100.0)).collect::<Vec<_>>());
            }
        }
        self.kept.force = force;
        self.doll.sleep_tol = [0.0; 3];
        self.kept.torque = torque;
        let kx = |n: &str| std::env::var(n).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
        let ks = std::env::var("HORSE_OURSMS").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(LEG_STIFF);
        let most = m / 618.0;
        let relax = std::env::var("HORSE_OURSRELAX").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(RELAX);
        if relax > 0.0 {
            for (i, l) in LEGS.iter().enumerate() {
                if o.step.is_some_and(|s| s.leg == i) || o.pair.is_some_and(|s| s.leg == i) { continue; }
                for &k in l { if let Some(p) = self.build.parts[k].parent {
                    o.pose[k] = o.pose[k].slerp(fr[p].rotation.inverse() * fr[k].rotation, (dt / relax).min(1.0));
                } }
            }
        }
        let mut pose = o.pose.clone();
        if trying("HORSE_OURSIK") {
            let level = Quat::from_rotation_arc(up, Vec3::Z).slerp(Quat::IDENTITY, 1.0 - kx("HORSE_OURSIKL").clamp(0.0, 1.0));
            let mid_floor = planted.iter().map(|&i| hooves[i].z).sum::<f32>() / planted.len().max(1) as f32;
            let rise = (o.height - (com.z - mid_floor)).clamp(-0.1, 0.1) * kx("HORSE_OURSIKH");
            for &i in &planted {
                if [o.step, o.pair].iter().flatten().any(|s| s.leg == i) { continue; }
                let p = self.build.parts[LEGS[i][0]].parent.unwrap_or(MID);
                let pq = level * fr[p].rotation;
                let hip = fr[LEGS[i][0]].transform_point(self.build.parts[LEGS[i][0]].anchor);
                let hip = com + level * (hip - com) + Vec3::Z * rise;
                if std::env::var_os("HORSE_OURSIKLOG").is_some() && o.t < 0.3 {
                    let now = |k: usize| self.build.parts[k].parent.map_or(Quat::IDENTITY, |p| fr[p].rotation.inverse() * fr[k].rotation);
                    if let Some(q) = self.reach(i, &pose, pq, hip, hooves[i]) {
                        eprintln!("ik {:.2} leg {i}: off now {:.3} {:.3} {:.3}, pose {:.3} {:.3} {:.3}", o.t, q[0].angle_between(now(LEGS[i][0])), q[1].angle_between(now(LEGS[i][1])), q[2].angle_between(now(LEGS[i][2])),
                            pose[LEGS[i][0]].angle_between(now(LEGS[i][0])), pose[LEGS[i][1]].angle_between(now(LEGS[i][1])), pose[LEGS[i][2]].angle_between(now(LEGS[i][2])));
                    }
                }
                if let Some(q) = self.reach(i, &pose, pq, hip, hooves[i]) { pose[LEGS[i][0]] = q[0]; pose[LEGS[i][1]] = q[1]; pose[LEGS[i][2]] = q[2]; }
            }
        }
        for Step { leg, t, from, to, dur, lift } in [o.step, o.pair].into_iter().flatten() {
            let Some(a) = from else { continue };
            let s = (t / dur).min(1.0);
            let fold = FOLD * (std::f32::consts::PI * s).sin() * (lift / (LIFT * unit)).min(1.0);
            let sign = std::env::var(if leg < 2 { "HORSE_OURSFH" } else { "HORSE_OURSFF" }).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
            let [k0, k1, k2] = LEGS[leg];
            for (k, x) in [(k1, sign), (k2, -sign * kx("HORSE_OURSFK"))] {
                if let Limit::BoneHinge { pf, .. } = self.build.parts[k].limit {
                    pose[k] = pf * Quat::from_rotation_z(fold * x) * pf.inverse() * pose[k];
                }
            }
            if let Some(p) = self.build.parts[k0].parent {
                let e = s * s * (3.0 - 2.0 * s);
                let want = a.lerp(to, e) + Vec3::Z * lift * (std::f32::consts::PI * s).sin();
                let (d0, d1, d2) = (&self.build.parts[k0], &self.build.parts[k1], &self.build.parts[k2]);
                let pq = fr[p].rotation;
                let hip = fr[k0].transform_point(d0.anchor);
                let r0 = pq * pose[k0];
                let j1 = hip + r0 * (d1.anchor_parent - d0.anchor);
                let r1 = r0 * pose[k1];
                let j2 = j1 + r1 * (d2.anchor_parent - d1.anchor);
                let sole = j2 + r1 * pose[k2] * -d2.anchor;
                let arc = Quat::from_rotation_arc((sole - hip).normalize_or(Vec3::NEG_Z), (want - hip).normalize_or(Vec3::NEG_Z));
                pose[k0] = pq.inverse() * arc * r0;
            }
        }
        self.ours_drive(&pose, ks, most, dt, o.step.filter(|s| s.from.is_some()).map(|s| s.leg));
        self.ours = Some(o.clone());
        true
    }
}

impl Horse {
    pub(super) fn hyb_support(&mut self, fr: &[Transform], planted: [bool; 4], com: Vec3, vel: Vec3, torque: &mut [Vec3]) {
        let legs: Vec<usize> = (0..4).filter(|&i| planted[i]).collect();
        if legs.is_empty() { return; }
        let hooves: Vec<Vec3> = LEGS.iter().map(|l| fr[l[2]].translation).collect();
        let mid = legs.iter().map(|&i| hooves[i]).sum::<Vec3>() / legs.len() as f32;
        let h = *self.hyb_h.get_or_insert(com.z - mid.z);
        let m = self.doll.mass();
        let up = fr[MID].rotation * self.stand;
        let d = |k: f32| 2.0 * ZETA * k.sqrt();
        let (kh, kc, kl) = (K_HEIGHT * kx("HORSE_HYBH"), K_CENTRE * kx("HORSE_HYBC"), K_LEVEL * kx("HORSE_HYBL"));
        let fz = (m * (GRAVITY() * kx("HORSE_HYBG") + kh * (h - (com.z - mid.z)) - d(kh) * vel.z)).clamp(0.0, 2.5 * m * GRAVITY());
        let fxy = (m * (kc * (mid - com) - d(kc) * vel)).with_z(0.0).clamp_length_max(MU * fz);
        let (_, w) = self.doll.vel(MID);
        let tau = (up.cross(Vec3::Z) * kl - w.with_z(0.0) * D_LEVEL * kl.sqrt() / K_LEVEL.sqrt()) * m * h;
        let cop = com.truncate() + Vec2::new(-tau.y, tau.x) / fz.max(1.0);
        let share = shares(&legs.iter().map(|&i| hooves[i].truncate()).collect::<Vec<_>>(), cop);
        if std::env::var_os("HORSE_HYBLOG").is_some() {
            let slide: f32 = legs.iter().map(|&i| self.doll.vel(LEGS[i][2]).0.truncate().length()).sum();
            eprintln!("hyb slide {:.1} down {} h {:.3}/{h:.3} tilt {:.3} fxy/fz {:.2}", slide * 100.0, legs.len(), com.z - mid.z, up.z.acos(), fxy.length() / fz.max(1.0));
        }
        for (s, &i) in share.iter().zip(&legs) {
            let f = Vec3::Z * fz * s + (fxy * s).clamp_length_max(MU * fz * s);
            if !f.is_finite() { continue; }
            let foot = self.doll.hit[LEGS[i][2]].map_or(hooves[i], |h| h.0);
            for &k in LEGS[i].iter().rev() {
                let Some(p) = self.build.parts[k].parent else { continue };
                let t = -(foot - fr[k].transform_point(self.build.parts[k].anchor)).cross(f);
                torque[k] += t;
                torque[p] -= t;
            }
        }
    }

    pub(super) fn reach(&self, i: usize, pose: &[Quat], pq: Quat, hip: Vec3, to: Vec3) -> Option<[Quat; 3]> {
        let [k0, k1, k2] = LEGS[i];
        let (d0, d1, d2) = (&self.build.parts[k0], &self.build.parts[k1], &self.build.parts[k2]);
        let (Limit::BoneHinge { pf: f1, .. }, Limit::BoneHinge { pf: f2, .. }) = (d1.limit, d2.limit) else { return None };
        let sign = std::env::var(if i < 2 { "HORSE_OURSFH" } else { "HORSE_OURSFF" }).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
        let folded = |f: f32| (f1 * Quat::from_rotation_z(f * sign) * f1.inverse() * pose[k1], f2 * Quat::from_rotation_z(-f * sign) * f2.inverse() * pose[k2]);
        let sole = |q0: Quat, q1: Quat, q2: Quat| {
            let r0 = pq * q0;
            let j1 = hip + r0 * (d1.anchor_parent - d0.anchor);
            let r1 = r0 * q1;
            let j2 = j1 + r1 * (d2.anchor_parent - d1.anchor);
            j2 + r1 * q2 * -d2.anchor
        };
        let want = (to - hip).length();
        let len = |f: f32| { let (q1, q2) = folded(f); (sole(pose[k0], q1, q2) - hip).length() };
        let (mut lo, mut hi) = (-0.6, 1.2);
        if !(len(lo) >= want && len(hi) <= want) { lo = 0.0; hi = 0.0; if len(lo) < want - 0.05 { lo = -0.6; hi = -0.6; } }
        for _ in 0..16 { let m = 0.5 * (lo + hi); if len(m) > want { lo = m } else { hi = m } }
        let (q1, q2) = folded(0.5 * (lo + hi));
        let a = (sole(pose[k0], q1, q2) - hip).normalize_or(Vec3::NEG_Z);
        let q0 = pq.inverse() * Quat::from_rotation_arc(a, (to - hip).normalize_or(Vec3::NEG_Z)) * pq * pose[k0];
        Some([q0, q1, q2])
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_step(&self, o: &mut Ours, fr: &[Transform], hooves: &[Vec3], com: Vec3, vel: Vec3, unit: f32, planted: &[usize], dt: f32) {
        let log = std::env::var_os("HORSE_OURSLOG").is_some();
        for i in 0..4 { o.air[i] = if planted.contains(&i) { 0.0 } else { o.air[i] + dt }; }
        let turn = Vec2::from_angle(heading(fr));
        let floor = hooves.iter().map(|h| h.z).fold(f32::MAX, f32::min);
        let speed = vel.truncate().length();
        if trying("HORSE_OURSTROT") && (speed > V_TROT * kx("HORSE_OURSVT") || o.trot > 0.0) {
            o.trot = if speed > V_STILL { TROT_HOLD } else { o.trot - dt };
            o.step = None.or(o.step.filter(|s| s.from.is_some()));
            let mut busy = false;
            for slot in [&mut o.step, &mut o.pair] {
                if let Some(st) = slot.as_mut() {
                    st.t += dt;
                    if st.t >= st.dur * 1.3 || (st.t >= st.dur * 0.6 && planted.contains(&st.leg)) {
                        if log { eprintln!("ours {:.2}: trot land {} {:.2} m from aim", o.t, st.leg, (hooves[st.leg] - st.to).truncate().length()); }
                        o.spot[st.leg] = None;
                        *slot = None;
                        o.rest = 0.0;
                    } else { busy = true; }
                }
            }
            o.rest += dt;
            if !busy && o.trot > 0.0 && o.rest >= TROT_GAP * kx("HORSE_OURSGAP") {
                let dur = TROT * unit.sqrt() * kx("HORSE_OURSTT");
                let lead = vel.truncate() * (dur * 0.5 + RAIBERT * kx("HORSE_OURSKR") * (o.height / GRAVITY()).sqrt());
                let legs = if o.diag { [0, 3] } else { [1, 2] };
                o.diag = !o.diag;
                let mk = |i: usize| {
                    let to = com.truncate() + turn.rotate(o.home[i]) + lead;
                    let to = hooves[i].truncate() + (to - hooves[i].truncate()).clamp_length_max(REACH * unit);
                    Step { leg: i, t: 0.0, from: Some(hooves[i]), to: to.extend(floor), dur, lift: TROT_LIFT * unit * kx("HORSE_OURSTL") }
                };
                (o.step, o.pair) = (Some(mk(legs[0])), Some(mk(legs[1])));
                if log { eprintln!("ours {:.2}: trot {legs:?} v {speed:.2}", o.t); }
            }
            if o.trot <= 0.0 && !busy { o.trot = 0.0; o.rest = 0.0; }
            return;
        }
        if trying("HORSE_OURSWALK") && (speed > V_WALK * kx("HORSE_OURSVW") || o.trot > 0.0) {
            o.trot = if speed > V_STILL { WALK_HOLD } else { o.trot - dt };
            if let Some(st) = o.step.as_mut() {
                st.t += dt;
                if st.from.is_none() { st.from = Some(hooves[st.leg]); st.t = 0.0; }
                if st.t >= st.dur * 1.4 || (st.t >= st.dur * 0.6 && planted.contains(&st.leg)) {
                    if log { eprintln!("ours {:.2}: walk land {} {:.2} m from aim, moved {:.2} m in {:.2} s", o.t, st.leg, (hooves[st.leg] - st.to).truncate().length(),
                        st.from.map_or(0.0, |a| (hooves[st.leg] - a).truncate().length()), st.t); }
                    o.spot[st.leg] = None;
                    o.last = Some(st.leg);
                    o.step = None;
                    o.rest = 0.0;
                }
                return;
            }
            o.rest += dt;
            if o.trot <= 0.0 { o.trot = 0.0; o.last = None; return; }
            let dur = WALK * unit.sqrt() * kx("HORSE_OURSWT");
            let lead = vel.truncate() * (dur * 1.5 * kx("HORSE_OURSKS") + RAIBERT * kx("HORSE_OURSKR") * (o.height / GRAVITY()).sqrt());
            let dir = vel.truncate().normalize_or_zero();
            let want = |i: usize| com.truncate() + turn.rotate(o.home[i]) + lead;
            let need = |i: usize| (want(i) - hooves[i].truncate()).length() + 0.3 * (hooves[i].truncate() - com.truncate()).dot(dir).max(0.0);
            let Some(leg) = (0..4).filter(|&i| Some(i) != o.last && planted.contains(&i)).max_by(|&a, &b| need(a).total_cmp(&need(b))) else { return };
            if (want(leg) - hooves[leg].truncate()).length() < WALK_MIN * unit || o.rest < WALK_GAP * kx("HORSE_OURSWG") { return; }
            let to = hooves[leg].truncate() + (want(leg) - hooves[leg].truncate()).clamp_length_max(REACH * unit);
            o.step = Some(Step { leg, t: 0.0, from: Some(hooves[leg]), to: to.extend(floor), dur, lift: WALK_LIFT * unit * kx("HORSE_OURSWL") });
            if log { eprintln!("ours {:.2}: walk {leg} v {speed:.2}, {:.2} m", o.t, (to - hooves[leg].truncate()).length()); }
            return;
        }
        if let Some(st) = o.step.as_mut() {
            st.t += dt;
            match st.from {
                None if st.t >= SHIFT_MOST || (st.t >= SHIFT && {
                    let three: Vec<Vec2> = (0..4).filter(|&j| j != st.leg).map(|j| hooves[j].truncate()).collect();
                    (over(&three, com.truncate(), MARGIN * unit) - com.truncate()).length() < 0.02 * unit && vel.truncate().length() < 0.15
                }) => { st.from = Some(hooves[st.leg]); st.t = 0.0; if log { eprintln!("ours {:.2}: lift {}", o.t, st.leg); } }
                Some(_) if st.t >= st.dur * 1.5 || (st.t >= st.dur * 0.7 && planted.contains(&st.leg)) => {
                    if log { eprintln!("ours {:.2}: land {} {:.2} m from aim, moved {:.2} m in {:.2} s", o.t, st.leg, (hooves[st.leg] - st.to).truncate().length(),
                        st.from.map_or(0.0, |a| (hooves[st.leg] - a).truncate().length()), st.t); }
                    o.spot[st.leg] = None;
                    o.step = None;
                    o.rest = 0.0;
                }
                _ => {}
            }
            return;
        }
        let lead = vel.truncate() * (o.height / GRAVITY()).sqrt();
        let cp = com.truncate() + lead;
        let mut quad: Vec<Vec2> = hooves.iter().map(|h| h.truncate()).collect();
        let c4 = quad.iter().sum::<Vec2>() / 4.0;
        quad.sort_by(|a, b| (*a - c4).to_angle().total_cmp(&(*b - c4).to_angle()));
        if kx("HORSE_OURSQUICK") != 0.0 && (over(&quad, cp, EDGE * unit * kx("HORSE_OURSEDGE")) - cp).length() > 0.0 {
            let dir = lead.normalize_or_zero();
            let out = |i: usize| (hooves[i].truncate() - com.truncate()).dot(dir);
            let cross = trying("HORSE_OURSCROSS");
            let (leg, to) = if cross {
                let leg = (0..4).min_by(|&a, &b| out(a).total_cmp(&out(b))).unwrap();
                let h = hooves[leg].truncate() - com.truncate();
                (leg, cp + dir * EDGE * unit + (h - dir * dir.dot(h)))
            } else {
                let leg = (0..4).filter(|&i| out(i) > 0.0).min_by(|&a, &b| out(a).total_cmp(&out(b))).unwrap_or(0);
                (leg, cp + lead * (CATCH - 1.0) + (hooves[leg].truncate() - com.truncate()))
            };
            let to = hooves[leg].truncate() + (to - hooves[leg].truncate()).clamp_length_max(REACH * unit);
            o.step = Some(Step { leg, t: 0.0, from: Some(hooves[leg]), to: to.extend(floor), dur: SWING * unit.sqrt() * QUICK * kx("HORSE_OURSQ"), lift: LIFT * unit });
            if log { eprintln!("ours {:.2}: catch {leg}, capture point {:.2} m out", o.t, (over(&quad, cp, 0.0) - cp).length()); }
            return;
        }
        if let Some(leg) = (0..4).find(|&i| o.air[i] > AIR * kx("HORSE_OURSAIR")).filter(|_| trying("HORSE_OURSREPLANT")) {
            let to = com.truncate() + turn.rotate(o.home[leg]);
            let to = hooves[leg].truncate() + (to - hooves[leg].truncate()).clamp_length_max(REACH * unit);
            o.step = Some(Step { leg, t: 0.0, from: Some(hooves[leg].with_z(floor)), to: to.extend(floor), dur: SWING * unit.sqrt() * QUICK, lift: 0.0 });
            if log { eprintln!("ours {:.2}: replant {leg}", o.t); }
            return;
        }
        if planted.len() < 4 || o.rest < REST || !trying("HORSE_OURSSTEP") { return; }
        let (leg, miss) = (0..4).map(|i| (i, (com.truncate() + turn.rotate(o.home[i]) + lead - hooves[i].truncate()).length()))
            .fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
        if miss > STEP_AT * unit {
            let to = com.truncate() + turn.rotate(o.home[leg]) + lead * CATCH;
            let to = hooves[leg].truncate() + (to - hooves[leg].truncate()).clamp_length_max(REACH * unit);
            o.step = Some(Step { leg, t: 0.0, from: None, to: to.extend(floor), dur: SWING * unit.sqrt(), lift: LIFT * unit });
            if log { eprintln!("ours {:.2}: step {leg}, {miss:.2} m off", o.t); }
        }
    }
}

const STEP_AT: f32 = 0.12;
const SHIFT: f32 = 0.2;
const AIR: f32 = 0.15;
const SHIFT_MOST: f32 = 0.8;
const MARGIN: f32 = 0.05;
const RELAX: f32 = 0.5;
const LEG_STIFF: f32 = 3.0;
const SWING: f32 = 0.46;
const QUICK: f32 = 0.3;
const V_TROT: f32 = 0.25;
const V_STILL: f32 = 0.1;
const TROT_HOLD: f32 = 0.6;
const TROT: f32 = 0.3;
const TROT_LIFT: f32 = 0.08;
const RAIBERT: f32 = 0.5;
const V_WALK: f32 = 0.15;
const WALK_HOLD: f32 = 0.5;
const WALK: f32 = 0.3;
const WALK_LIFT: f32 = 0.1;
const WALK_MIN: f32 = 0.04;
const WALK_GAP: f32 = 0.0;
const TROT_GAP: f32 = 0.1;
const HOVER: f32 = 0.06;
const EDGE: f32 = 0.05;
const LIFT: f32 = 0.18;
const FOLD: f32 = 1.0;
const CATCH: f32 = 1.0;
const REACH: f32 = 0.6;
const REST: f32 = 0.1;

fn over(tri: &[Vec2], p: Vec2, margin: f32) -> Vec2 {
    let c = tri.iter().sum::<Vec2>() / tri.len() as f32;
    let mut q = p;
    for _ in 0..4 {
        for e in 0..tri.len() {
            let (a, b) = (tri[e], tri[(e + 1) % tri.len()]);
            let n = (b - a).perp().normalize_or_zero();
            let n = if n.dot(c - a) < 0.0 { -n } else { n };
            let m = margin.min(n.dot(c - a));
            let d = n.dot(q - a) - m;
            if d < 0.0 { q -= n * d; }
        }
    }
    q
}

fn trying(n: &str) -> bool {
    std::env::var(n).is_ok_and(|v| v == "1")
}

fn kx(n: &str) -> f32 {
    std::env::var(n).ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0)
}

pub(super) fn heading(fr: &[Transform]) -> f32 {
    let f = fr[MID].rotation * Vec3::X;
    f.y.atan2(f.x)
}

fn shares(feet: &[Vec2], cop: Vec2) -> Vec<f32> {
    let n = feet.len();
    let mut s = vec![1.0 / n as f32; n];
    if n >= 3 {
        let mean = feet.iter().sum::<Vec2>() / n as f32;
        let d: Vec<Vec2> = feet.iter().map(|f| *f - mean).collect();
        let m = d.iter().fold(Mat2::ZERO, |a, x| a + Mat2::from_cols(*x * x.x, *x * x.y));
        if m.determinant().abs() > 1e-6 {
            let g = m.inverse() * (cop - mean);
            for (si, di) in s.iter_mut().zip(&d) { *si += di.dot(g); }
        }
    } else if n == 2 {
        let ab = feet[1] - feet[0];
        let t = ((cop - feet[0]).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        s = vec![1.0 - t, t];
    }
    for x in s.iter_mut() { *x = x.max(0.0); }
    let sum: f32 = s.iter().sum();
    s.iter().map(|x| x / sum.max(1e-6)).collect()
}

