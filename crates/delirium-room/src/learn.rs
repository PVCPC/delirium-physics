use bevy::prelude::*;
use delirium::collision::Collision;
use delirium::msg::{self, Value};
use delirium::nm::Agent;
use delirium::ragdoll::{Build, Ragdoll};

const HEAD: usize = 12;
const CHEST: usize = 9;
pub const STEPS: usize = 480;
const HIDDEN: usize = 64;
const PAIRS: usize = 64;
const SIGMA: f32 = 0.04;
const LR: f32 = 0.02;
const TRIES: usize = 2;
pub const LIVE: usize = 8;

pub fn live_path(net: &str) -> String {
    format!("{net}.live")
}

fn save_live(path: &str, generation: usize, scores: &[f32], tries: &[Vec<Vec<Transform>>]) {
    let (steps, parts) = (tries.first().map_or(0, |t| t.len()), tries.first().and_then(|t| t.first()).map_or(0, |f| f.len()));
    let mut b: Vec<u8> = [generation, tries.len(), steps, parts].iter().flat_map(|&x| (x as u32).to_le_bytes()).collect();
    b.extend(scores.iter().flat_map(|x| x.to_le_bytes()));
    for f in tries.iter().flatten().flatten() {
        let (p, r) = (f.translation, f.rotation);
        b.extend([p.x, p.y, p.z, r.x, r.y, r.z, r.w].iter().flat_map(|x| x.to_le_bytes()));
    }
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, b).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

pub fn load_live(path: &str) -> Option<(usize, Vec<f32>, Vec<Vec<Vec<Transform>>>)> {
    let b = std::fs::read(path).ok()?;
    let u = |i: usize| -> Option<usize> { Some(u32::from_le_bytes(b.get(i * 4..i * 4 + 4)?.try_into().ok()?) as usize) };
    let (generation, n, steps, parts) = (u(0)?, u(1)?, u(2)?, u(3)?);
    let f: Vec<f32> = b[16..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    (f.len() == n + n * steps * parts * 7).then(|| {
        let tries = (0..n).map(|t| (0..steps).map(|s| (0..parts).map(|k| {
            let o = n + ((t * steps + s) * parts + k) * 7;
            Transform::from_translation(Vec3::new(f[o], f[o + 1], f[o + 2])).with_rotation(Quat::from_xyzw(f[o + 3], f[o + 4], f[o + 5], f[o + 6]))
        }).collect()).collect()).collect();
        (generation, f[..n].to_vec(), tries)
    })
}

pub fn path() -> String {
    let Ok(e) = std::env::current_exe() else { return "getup.net".into() };
    let at = e.ancestors().skip(1).map(|d| d.join("getup.net")).find(|p| p.exists());
    at.or_else(|| Some(e.parent()?.parent()?.parent()?.parent()?.join("getup.net"))).map_or("getup.net".into(), |p| p.to_string_lossy().into_owned())
}

pub struct Net {
    pub sizes: [usize; 4],
    pub w: Vec<f32>,
}

impl Net {
    fn len(s: [usize; 4]) -> usize {
        (0..3).map(|l| s[l] * s[l + 1] + s[l + 1]).sum()
    }

    pub fn forward(&self, w: &[f32], x: &[f32]) -> Vec<f32> {
        let mut a = x.to_vec();
        let mut o = 0;
        for l in 0..3 {
            let (n, m) = (self.sizes[l], self.sizes[l + 1]);
            let (wl, b) = (&w[o..o + n * m], &w[o + n * m..o + n * m + m]);
            a = (0..m).map(|j| (b[j] + (0..n).map(|i| wl[j * n + i] * a[i]).sum::<f32>()).tanh()).collect();
            o += n * m + m;
        }
        a
    }

    pub fn grown(mut self, out: usize) -> Net {
        let m = self.sizes[3];
        if out > m {
            let n = self.sizes[2];
            let b = self.w.split_off(self.w.len() - m);
            self.w.extend(std::iter::repeat_n(0.0, (out - m) * n));
            self.w.extend(b);
            self.w.extend(std::iter::repeat_n(0.0, out - m));
            self.sizes[3] = out;
        }
        self
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let mut b: Vec<u8> = self.sizes.iter().flat_map(|&s| (s as u32).to_le_bytes()).collect();
        b.extend(self.w.iter().flat_map(|x| x.to_le_bytes()));
        std::fs::write(path, b)
    }

    pub fn load(path: &str) -> Option<Net> {
        let b = std::fs::read(path).ok()?;
        let u = |i: usize| u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap()) as usize;
        let sizes = [u(0), u(1), u(2), u(3)];
        let w: Vec<f32> = b[16..].chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        (w.len() == Net::len(sizes)).then_some(Net { sizes, w })
    }
}

pub fn obs(d: &Ragdoll, on: usize) -> Vec<f32> {
    let fr: Vec<Transform> = d.frames().collect();
    let root = fr[0];
    let f = (root.rotation * Vec3::X).with_z(0.0).normalize_or(Vec3::X);
    let inv = Quat::from_rotation_z(-f.y.atan2(f.x));
    let mut o = vec![root.translation.z];
    o.extend((inv * (root.rotation * Vec3::Z)).to_array());
    let v0 = d.motion(0).0;
    for (k, t) in fr.iter().enumerate() {
        o.extend((inv * (t.translation - root.translation)).to_array());
        o.extend((inv * (d.motion(k).0 - v0) * 0.2).to_array());
        o.extend((inv * (t.rotation * Vec3::Z)).to_array());
    }
    o.extend((inv * v0 * 0.2).to_array());
    o.extend((0..MENU.len()).map(|k| (k == on) as u8 as f32));
    o
}

pub fn drive(a: &mut Agent, act: &[f32], zero: &[Quat], build: &Build) {
    let root = a.doll.frames().next().unwrap_or_default();
    let mut rot: Vec<Quat> = Vec::with_capacity(zero.len());
    for (i, p) in build.parts.iter().enumerate() {
        let r = match p.parent {
            None => root.rotation,
            Some(j) => rot[j] * zero[i] * Quat::from_scaled_axis(Vec3::new(act[3 * (i - 1)], act[3 * (i - 1) + 1], act[3 * (i - 1) + 2]) * 1.2),
        };
        rot.push(r);
    }
    let n = 3 * (build.parts.len() - 1);
    a.set_stiffness(8.0 + 4.0 * act[n], 1.0);
    a.muscle_stiffness_x(act.get(n + 1).map_or(1.0, |&m| 4f32.powf(m)));
    delirium::nm::task_pose_control(a, rot.into_iter().map(|r| Transform::from_rotation(r)).collect());
}

pub fn zero(build: &Build, frames: &[Transform]) -> Vec<Quat> {
    build.parts.iter().enumerate().map(|(i, p)| p.parent.map_or(frames[i].rotation, |j| frames[j].rotation.inverse() * frames[i].rotation)).collect()
}

pub fn agent(build: &Build, start: &[Transform]) -> Agent {
    Agent::new(build, Ragdoll::new(build, start, Vec3::ZERO), Quat::IDENTITY, 0.3)
}

pub const MENU: [(&str, bool); 40] = [
    ("muscles", true), ("balance", true), ("drunk", true), ("windmill", true), ("pedal", true), ("writhe", true), ("rollup", true),
    ("catchfall", true), ("highfall", true), ("stairs", true), ("flinch", true), ("brace", true), ("look", true), ("point", true),
    ("lean", true), ("relax", true), ("stayup", true), ("foetal", true), ("grab", true),
    ("electrocute", true), ("teeter", true), ("stumble", true), ("staggerFall", true), ("smartFall", true),
    ("balancerCollisionsReaction", true), ("injuredOnGround", true), ("learnedCrawl", true), ("yanked", true), ("dragged", true),
    ("dangle", true), ("carried", true), ("buoyancy", true), ("animPose", true), ("armsWindmill", true), ("spineTwist", true),
    ("wave", true), ("", false), ("", false), ("", false), ("", false)];
const PICK_EVERY: usize = 15;

#[derive(Clone, Copy, Default)]
pub struct Pick {
    pub on: usize,
    pub n: usize,
}

fn menu(slot: usize, a: &Agent, dir: Vec3, amt: f32) -> Vec<(&'static str, Vec<(&'static str, Value)>)> {
    let fr: Vec<Transform> = a.doll.frames().collect();
    let (head, chest) = (fr[HEAD].translation, fr[CHEST].translation);
    let on = || ("start", Value::Bool(true));
    let left = Vec3::Z.cross(dir);
    match MENU[slot].0 {
        "balance" => vec![("bodyBalance", vec![on()])],
        "drunk" => vec![("bodyBalance", vec![on(), ("armsOutOnPushTimeout", Value::Float(999.0))]),
                        ("leanInDirection", vec![on(), ("leanAmount", Value::Float(0.2)), ("dir", Value::Vec3(dir))])],
        "windmill" => vec![("bodyBalance", vec![on()]), ("armsWindmillAdaptive", vec![on()])],
        "pedal" => vec![("pedalLegs", vec![on()])],
        "writhe" => vec![("bodyWrithe", vec![on()])],
        "rollup" => vec![("bodyRollUp", vec![on()])],
        "catchfall" => vec![("catchFall", vec![on()])],
        "highfall" => vec![("highFall", vec![on()])],
        "stairs" => vec![("rollDownStairs", vec![on()])],
        "flinch" => vec![("upperBodyFlinch", vec![on(), ("pos", Value::Vec3(head + dir * 2.0))])],
        "brace" => vec![("braceForImpact", vec![on(), ("pos", Value::Vec3(chest + dir)), ("look", Value::Vec3(chest + dir))])],
        "look" => vec![("headLook", vec![on(), ("alwaysLook", Value::Bool(true)), ("pos", Value::Vec3(head + dir * 2.0))])],
        "point" => vec![("pointArm", vec![on(), ("target", Value::Vec3(chest + dir * 2.0))])],
        "lean" => vec![("bodyBalance", vec![on()]), ("leanInDirection", vec![on(), ("leanAmount", Value::Float(0.4 * amt)), ("dir", Value::Vec3(dir))])],
        "relax" => vec![("bodyRelax", vec![on()])],
        "stayup" => vec![("stayUpright", vec![on(), ("stiffness", Value::Float(3.0))])],
        "foetal" => vec![("bodyFoetal", vec![on()])],
        "grab" => {
            let at = chest + dir * 0.45;
            vec![("grab", vec![on(), ("linegrab", Value::Bool(true)), ("useLeft", Value::Bool(true)), ("useRight", Value::Bool(true)),
                               ("dontLetGo", Value::Bool(true)), ("pos1", Value::Vec3(at + left * 0.5)), ("pos2", Value::Vec3(at - left * 0.5))])]
        }
        x if !x.is_empty() && x != "muscles" => vec![(x, vec![on()])],
        _ => vec![],
    }
}

pub fn step(net: &Net, w: &[f32], a: &mut Agent, build: &Build, world: &Collision, zero: &[Quat], p: &mut Pick) {
    let act = net.forward(w, &obs(&a.doll, p.on));
    let n = 3 * (build.parts.len() - 1) + 2;
    if p.n % PICK_EVERY == 0 && act.len() >= n + MENU.len() + 3 {
        let pick = (0..MENU.len()).filter(|&k| MENU[k].1).fold(0, |b, k| if act[n + k] > act[n + b] { k } else { b });
        if pick != p.on {
            let root = a.doll.frames().next().unwrap_or_default();
            let f = (root.rotation * Vec3::X).with_z(0.0).normalize_or(Vec3::X);
            let l = Vec2::new(act[n + MENU.len()], act[n + MENU.len() + 1]);
            let dir = Quat::from_rotation_z(f.y.atan2(f.x)) * l.normalize_or(Vec2::X).extend(0.0);
            let amt = 0.5 + 0.5 * act[n + MENU.len() + 2];
            let mut st = std::mem::take(&mut a.msgs);
            for (name, ps) in std::iter::once(("stopAllBehaviours", vec![])).chain(menu(pick, a, dir, amt)) {
                let _ = msg::send(a, build, world, &mut st, name, &ps);
            }
            a.msgs = st;
            p.on = pick;
        }
    }
    p.n += 1;
    for l in std::mem::take(&mut a.extras.outbox).iter().filter(|l| !l.starts_with("#say ")) {
        send_line(a, build, world, l);
    }
    if p.on == 0 {
        drive(a, &act, zero, build);
    }
    a.doll.kick(Vec3::ZERO);
    a.step(build, world, 1.0 / 60.0, &[], &[]);
}

fn send_line(a: &mut Agent, build: &Build, world: &Collision, line: &str) {
    let mut w = line.split_whitespace();
    let Some(m) = w.next().and_then(msg::find) else { return };
    let ps: Vec<(&str, Value)> = w.filter_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        let kind = if k == "start" { msg::Kind::Bool } else { m.params.iter().find(|p| p.name == k)?.kind };
        Some((k, crate::parse(kind, v)?))
    }).collect();
    let mut st = std::mem::take(&mut a.msgs);
    let _ = msg::send(a, build, world, &mut st, m.name, &ps);
    a.msgs = st;
}

pub fn standing(d: &Ragdoll) -> bool {
    let fr: Vec<Transform> = d.frames().collect();
    fr[HEAD].translation.z > 1.4 && fr[0].translation.z > 0.8
}

fn stage(h: f32, p: f32) -> usize {
    [(1.4, 0.8), (1.1, 0.6), (0.8, 0.35), (0.5, 0.0)].iter().position(|&(hh, pp)| h > hh && p > pp).map_or(0, |i| 4 - i)
}
const STAGE: [f32; 5] = [0.0, 0.3, 1.0, 2.0, 4.0];

const KEYS: [usize; 11] = [1, 4, 14, 18, 12, 16, 20, 2, 5, 3, 6];

fn mimic(frames: &[Transform]) -> Option<Vec<[Vec3; 11]>> {
    let t = std::fs::read_to_string(std::env::var("MIMIC").ok()?).ok()?;
    let mut r: Vec<[Vec3; 11]> = t.lines().filter_map(|l| {
        let v: Vec<f32> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        (v.len() == 33).then(|| std::array::from_fn(|i| Vec3::new(v[3 * i], v[3 * i + 1], v[3 * i + 2])))
    }).collect();
    let top = r.iter().map(|k| k[4].z).fold(0.0, f32::max);
    let s = frames[HEAD].translation.z / top.max(0.1);
    r.iter_mut().flatten().for_each(|p| *p *= s);
    (!r.is_empty()).then_some(r)
}

fn local(k: [Vec3; 11]) -> [Vec3; 11] {
    let mid = ((k[0] + k[1]) * 0.5).with_z(0.0);
    let right = (k[1] - k[0]).with_z(0.0).normalize_or(Vec3::X);
    let inv = Quat::from_rotation_z(-right.y.atan2(right.x));
    k.map(|p| inv * (p - mid))
}

fn episode(net: &Net, w: &[f32], start: &[Transform], build: &Build, world: &Collision, zero: &[Quat], mut rec: Option<&mut Vec<Vec<Transform>>>,
           refm: Option<&[[Vec3; 11]]>) -> f32 {
    let mut a = agent(build, start);
    let mut r = 0.0;
    let (mut best, mut at, mut pick) = (0usize, 0usize, Pick::default());
    for _ in 0..STEPS {
        step(net, w, &mut a, build, world, zero, &mut pick);
        let fr: Vec<Transform> = a.doll.frames().collect();
        if let Some(rec) = rec.as_mut() {
            rec.push(fr.clone());
        }
        let (h, p) = (fr[HEAD].translation.z, fr[0].translation.z);
        if !h.is_finite() {
            return -100.0;
        }
        let now = stage(h, p);
        let own = STAGE[now] + 0.2 * h.min(1.6) / 1.6 + if standing(&a.doll) && a.doll.com().1.length() < 0.5 { 2.0 } else { 0.0 };
        if now > best {
            r += 30.0 * (STAGE[now] - STAGE[best]);
            best = now;
        }
        r += match refm {
            Some(m) => {
                let me = local(KEYS.map(|k| fr[k].translation));
                let err = |j: usize| me.iter().zip(&local(m[j])).map(|(a, b)| a.distance_squared(*b)).sum::<f32>() / 11.0;
                let (j, e) = (at..(at + 30).min(m.len())).map(|j| (j, err(j))).fold((at, f32::MAX), |b, c| if c.1 < b.1 { c } else { b });
                let moved = (j - at) as f32 / m.len() as f32;
                at = j;
                3.0 * (-4.0 * e).exp() + 30.0 * moved + own
            }
            None => own,
        };
        if p < 0.5 {
            r -= 3.0 * a.doll.motion(0).0.truncate().length().min(1.0);
        }
    }
    r / STEPS as f32
}

fn starts(build: &Build, world: &Collision, frames: &[Transform], n: usize, rng: &mut Rng) -> Vec<Vec<Transform>> {
    (0..n).map(|_| {
        let mut a = agent(build, frames);
        let dir = Quat::from_rotation_z(rng.f() * std::f32::consts::TAU) * Vec3::X;
        a.doll.kick(dir * (1.5 + 2.5 * rng.f()));
        a.set_stiffness(2.0 + 3.0 * rng.f(), 1.0);
        for _ in 0..240 {
            a.step(build, world, 1.0 / 60.0, &[], &[]);
        }
        a.doll.frames().collect()
    }).collect()
}

pub struct Rng(u64);

impl Rng {
    fn u(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn f(&mut self) -> f32 {
        (self.u() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn n(&mut self) -> f32 {
        let (a, b) = (self.f().max(1e-7), self.f());
        (-2.0 * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
    }
}

pub fn train(build: &Build, world: &Collision, frames: &[Transform], gens: usize, path: &str) {
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let pool = starts(build, world, frames, 32, &mut rng);
    let mut a = agent(build, &pool[0]);
    for k in (0..MENU.len()).filter(|&k| MENU[k].1) {
        for (name, ps) in menu(k, &a, Vec3::X, 1.0) {
            let mut st = std::mem::take(&mut a.msgs);
            if let Err(e) = msg::send(&mut a, build, world, &mut st, name, &ps) {
                println!("learn: {}'s {name} not taken: {e:?}", MENU[k].0);
            }
            a.msgs = st;
        }
    }
    let z = zero(build, frames);
    let refm = mimic(frames);
    let refm = refm.as_deref();
    let d = Ragdoll::new(build, frames, Vec3::ZERO);
    let sizes = [obs(&d, 0).len(), HIDDEN, HIDDEN, 3 * (build.parts.len() - 1) + 2 + MENU.len() + 3];
    let mut net = Net::load(path).map(|n| n.grown(sizes[3])).filter(|n| n.sizes == sizes).unwrap_or_else(|| {
        let w = (0..Net::len(sizes)).map(|_| rng.n() * 0.05).collect();
        Net { sizes, w }
    });
    let np = net.w.len();
    let (mut m1, mut m2) = (vec![0.0f32; np], vec![0.0f32; np]);
    let threads = std::thread::available_parallelism().map_or(8, |n| n.get()).saturating_sub(4).max(1);
    let fallen = pool.iter().filter(|s| s[HEAD].translation.z < 0.5).count();
    if let Some(m) = refm {
        println!("learn: copying MIMIC's {} steps", m.len());
    }
    println!("learn: {np} weights, {threads} threads, {} lying starts ({fallen} flat), {PAIRS} pairs x {TRIES} tries a generation", pool.len());
    for g in 0..gens {
        let t0 = std::time::Instant::now();
        let seeds: Vec<u64> = (0..PAIRS).map(|_| rng.u() | 1).collect();
        let tries: Vec<Vec<usize>> = (0..PAIRS).map(|_| (0..TRIES).map(|_| (rng.u() % pool.len() as u64) as usize).collect()).collect();
        let noise = |s: u64| { let mut r = Rng(s); (0..np).map(|_| r.n()).collect::<Vec<f32>>() };
        let mut ret = vec![[0.0f32; 2]; PAIRS];
        let live = std::sync::Mutex::new(vec![Vec::new(); LIVE]);
        std::thread::scope(|sc| {
            for (c, out) in ret.chunks_mut(PAIRS.div_ceil(threads)).enumerate() {
                let (net, pool, z, seeds, tries, live) = (&net, &pool, &z, &seeds, &tries, &live);
                sc.spawn(move || for (i, o) in out.iter_mut().enumerate() {
                    let k = c * PAIRS.div_ceil(threads) + i;
                    let e = noise(seeds[k]);
                    for (s, sign) in [1.0f32, -1.0].iter().enumerate() {
                        let w: Vec<f32> = net.w.iter().zip(&e).map(|(a, b)| a + sign * SIGMA * b).collect();
                        let mut rec = (k < LIVE / 2).then(Vec::new);
                        o[s] = tries[k].iter().enumerate().map(|(n, &t)| episode(net, &w, &pool[t], build, world, z, if n == 0 { rec.as_mut() } else { None }, refm)).sum::<f32>() / TRIES as f32;
                        if let Some(r) = rec {
                            live.lock().unwrap()[2 * k + s] = r;
                        }
                    }
                });
            }
        });
        let mut all: Vec<(f32, usize)> = ret.iter().flatten().copied().zip(0..).collect();
        all.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut rank = vec![0.0f32; all.len()];
        for (r, &(_, i)) in all.iter().enumerate() {
            rank[i] = r as f32 / (all.len() - 1) as f32 - 0.5;
        }
        let mut grad = vec![0.0f32; np];
        for k in 0..PAIRS {
            let (d, e) = (rank[2 * k] - rank[2 * k + 1], noise(seeds[k]));
            for (gi, ei) in grad.iter_mut().zip(e) {
                *gi += d * ei / (2.0 * PAIRS as f32 * SIGMA);
            }
        }
        let t = (g + 1) as i32;
        for i in 0..np {
            let gi = grad[i] - 0.005 * net.w[i];
            m1[i] = 0.9 * m1[i] + 0.1 * gi;
            m2[i] = 0.999 * m2[i] + 0.001 * gi * gi;
            net.w[i] += LR * (m1[i] / (1.0 - 0.9f32.powi(t))) / ((m2[i] / (1.0 - 0.999f32.powi(t))).sqrt() + 1e-8);
        }
        let mean = ret.iter().flatten().sum::<f32>() / (2 * PAIRS) as f32;
        let best = ret.iter().flatten().copied().fold(f32::MIN, f32::max);
        println!("generation {g:4}  mean {mean:.3}  best {best:.3}  {:.1} s", t0.elapsed().as_secs_f32());
        let scores: Vec<f32> = ret.iter().take(LIVE / 2).flatten().copied().collect();
        save_live(&live_path(path), g, &scores, &live.into_inner().unwrap());
        if g % 10 == 9 || g + 1 == gens {
            let mut on = [0usize; MENU.len()];
            let up = pool.iter().take(8).filter(|s| {
                let mut a = agent(build, s);
                let mut p = Pick::default();
                (0..STEPS).for_each(|i| {
                    step(&net, &net.w, &mut a, build, world, &z, &mut p);
                    on[p.on] += (i % PICK_EVERY == 0) as usize;
                });
                standing(&a.doll)
            }).count();
            let picks: Vec<String> = (0..MENU.len()).filter(|&k| on[k] > 0).map(|k| format!("{} {}", MENU[k].0, on[k])).collect();
            println!("        standing at the end: {up}/8; picks: {}; saved {path}", picks.join(", "));
            let _ = net.save(path);
        }
    }
}

