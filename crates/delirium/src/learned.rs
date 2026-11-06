use crate::collision::Collision;
use crate::msg::{self, Value};
use crate::nm::Agent;
use crate::prelude::*;
use crate::ragdoll::{Build, Ragdoll};

#[allow(non_snake_case)]
#[inline]
pub fn HEAD() -> usize { delirium_data::kv!(55 int) }
#[allow(non_snake_case)]
#[inline]
pub fn CHEST() -> usize { delirium_data::kv!(104 int) }
#[allow(non_snake_case)]
#[inline]
pub fn STEPS() -> usize { delirium_data::kv!(187 int) }
#[allow(non_snake_case)]
#[inline]
pub fn HIDDEN() -> usize { delirium_data::kv!(188 int) }

pub struct Net {
    pub sizes: [usize; 4],
    pub w: Vec<f32>,
    pub frozen: std::sync::OnceLock<Vec<f32>>,
}

impl Net {
    pub fn len(s: [usize; 4]) -> usize {
        (0..delirium_data::kv!(0 int)).map(|l| s[l] * s[l + 1] + s[l + 1]).sum()
    }

    pub fn forward(&self, w: &[f32], x: &[f32]) -> Vec<f32> {
        let mut a = x.to_vec();
        let mut o = 0;
        for l in 0..delirium_data::kv!(0 int) {
            let (n, m) = (self.sizes[l], self.sizes[l + 1]);
            let (wl, b) = (&w[o..o + n * m], &w[o + n * m..o + n * m + m]);
            a = (0..m).map(|j| (b[j] + (0..n).map(|i| wl[j * n + i] * a[i]).sum::<f32>()).tanh()).collect();
            o += n * m + m;
        }
        a
    }

    pub fn frozen(&self, x: &[f32]) -> Vec<f32> {
        let t = self.frozen.get_or_init(|| {
            let (mut t, mut o) = (Vec::new(), 0);
            for l in 0..delirium_data::kv!(0 int) {
                let (n, m) = (self.sizes[l], self.sizes[l + 1]);
                t.extend((0..n * m).map(|k| self.w[o + (k % m) * n + k / m]));
                o += n * m + m;
            }
            t
        });
        let mut a = x.to_vec();
        let (mut o, mut q) = (0, 0);
        for l in 0..delirium_data::kv!(0 int) {
            let (n, m) = (self.sizes[l], self.sizes[l + 1]);
            let mut s = vec![-0.0f32; m];
            for (i, &x) in a[..n].iter().enumerate() {
                for (s, &w) in s.iter_mut().zip(&t[q + i * m..q + (i + 1) * m]) { *s += w * x; }
            }
            a = s.iter().zip(&self.w[o + n * m..o + n * m + m]).map(|(&s, &b)| (b + s).tanh()).collect();
            o += n * m + m; q += n * m;
        }
        a
    }

    pub fn grown(mut self, out: usize) -> Net {
        let m = self.sizes[delirium_data::kv!(0 int)];
        if out > m {
            let n = self.sizes[2];
            let b = self.w.split_off(self.w.len() - m);
            self.w.extend(std::iter::repeat_n(0.0, (out - m) * n));
            self.w.extend(b);
            self.w.extend(std::iter::repeat_n(0.0, out - m));
            self.sizes[delirium_data::kv!(0 int)] = out;
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
        if b.len() < delirium_data::kv!(54 int) || (b.len() - delirium_data::kv!(54 int)) % delirium_data::kv!(1 int) != 0 { return None; }
        let u = |i: usize| u32::from_le_bytes(b[i * delirium_data::kv!(1 int)..i * delirium_data::kv!(1 int) + delirium_data::kv!(1 int)].try_into().unwrap()) as usize;
        let sizes = [u(0), u(1), u(2), u(delirium_data::kv!(0 int))];
        let count = (0..delirium_data::kv!(0 int)).try_fold(0usize, |n, l| n.checked_add(sizes[l].checked_mul(sizes[l + 1])?.checked_add(sizes[l + 1])?))?;
        if sizes.contains(&0) || count != (b.len() - delirium_data::kv!(54 int)) / delirium_data::kv!(1 int) { return None; }
        let w: Vec<f32> = b[delirium_data::kv!(54 int)..].chunks_exact(delirium_data::kv!(1 int)).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        w.iter().all(|x| x.is_finite()).then_some(Net { sizes, w, frozen: Default::default() })
    }
}

pub fn obs(d: &Ragdoll, on: usize) -> Vec<f32> {
    body_obs(d, on, false)
}

pub fn body_obs(d: &Ragdoll, on: usize, walking: bool) -> Vec<f32> {
    let fr: Vec<Transform> = d.frames().collect();
    let root = fr[0];
    let f = if walking { Vec3::Z.cross(fr[delirium_data::kv!(1 int)].translation - fr[1].translation) } else { root.rotation * Vec3::X };
    let f = f.with_z(0.0).normalize_or(Vec3::X);
    let inv = Quat::from_rotation_z(-f.y.atan2(f.x));
    let mut o = vec![root.translation.z];
    o.extend((inv * (root.rotation * Vec3::Z)).to_array());
    let v0 = d.motion(0).0;
    for (k, t) in fr.iter().enumerate() {
        o.extend((inv * (t.translation - root.translation)).to_array());
        o.extend((inv * (d.motion(k).0 - v0) * delirium_data::kv!(60)).to_array());
        o.extend((inv * (t.rotation * Vec3::Z)).to_array());
    }
    o.extend((inv * v0 * delirium_data::kv!(60)).to_array());
    o.extend((0..MENU.len()).map(|k| (k == on) as u8 as f32));
    o
}

pub fn drive(a: &mut Agent, act: &[f32], zero: &[Quat], build: &Build) {
    let root = a.doll.frames().next().unwrap_or_default();
    let mut rot: Vec<Quat> = Vec::with_capacity(zero.len());
    for (i, p) in build.parts.iter().enumerate() {
        let r = match p.parent {
            None => root.rotation,
            Some(j) => rot[j] * zero[i] * Quat::from_scaled_axis(Vec3::new(act[delirium_data::kv!(0 int) * (i - 1)], act[delirium_data::kv!(0 int) * (i - 1) + 1], act[delirium_data::kv!(0 int) * (i - 1) + 2]) * delirium_data::kv!(136)),
        };
        rot.push(r);
    }
    let n = delirium_data::kv!(0 int) * (build.parts.len() - 1);
    a.set_stiffness(delirium_data::kv!(65) + delirium_data::kv!(84) * act[n], 1.0);
    a.muscle_stiffness_x(act.get(n + 1).map_or(1.0, |&m| delirium_data::kv!(84 f32).powf(m)));
    crate::nm::task_pose_control(a, rot.into_iter().map(|r| Transform::from_rotation(r)).collect());
}

pub fn zero(build: &Build, frames: &[Transform]) -> Vec<Quat> {
    build.parts.iter().enumerate().map(|(i, p)| p.parent.map_or(frames[i].rotation, |j| frames[j].rotation.inverse() * frames[i].rotation)).collect()
}

pub fn agent(build: &Build, start: &[Transform]) -> Agent {
    Agent::new(build, Ragdoll::new(build, start, Vec3::ZERO), Quat::IDENTITY, delirium_data::kv!(24))
}

pub const MENU: [(&str, bool); 40] = [
    ("muscles", true), ("balance", true), ("drunk", true), ("windmill", true), ("pedal", true), ("writhe", true), ("rollup", true),
    ("catchfall", true), ("highfall", true), ("stairs", true), ("flinch", true), ("brace", true), ("look", true), ("point", true),
    ("lean", true), ("relax", true), ("stayup", true), ("foetal", true), ("grab", true),
    ("electrocute", true), ("teeter", true), ("stumble", true), ("staggerFall", true), ("smartFall", true),
    ("balancerCollisionsReaction", true), ("injuredOnGround", true), ("learnedCrawl", true), ("yanked", true), ("dragged", true),
    ("dangle", true), ("carried", true), ("buoyancy", true), ("animPose", true), ("armsWindmill", true), ("spineTwist", true),
    ("wave", true), ("", false), ("", false), ("", false), ("", false)];

#[allow(non_snake_case)]
#[inline]
pub fn PICK_EVERY() -> usize { delirium_data::kv!(189 int) }

#[derive(Clone, Copy, Default)]
pub struct Pick {
    pub on: usize,
    pub n: usize,
}

pub fn menu(slot: usize, a: &Agent, dir: Vec3, amt: f32) -> Vec<(&'static str, Vec<(&'static str, Value)>)> {
    let fr: Vec<Transform> = a.doll.frames().collect();
    let (head, chest) = (fr[HEAD()].translation, fr[CHEST()].translation);
    let on = || ("start", Value::Bool(true));
    let left = Vec3::Z.cross(dir);
    match MENU[slot].0 {
        "balance" => vec![("bodyBalance", vec![on()])],
        "drunk" => vec![("bodyBalance", vec![on(), ("armsOutOnPushTimeout", Value::Float(delirium_data::kv!(190)))]),
                        ("leanInDirection", vec![on(), ("leanAmount", Value::Float(delirium_data::kv!(60))), ("dir", Value::Vec3(dir))])],
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
        "lean" => vec![("bodyBalance", vec![on()]), ("leanInDirection", vec![on(), ("leanAmount", Value::Float(delirium_data::kv!(116) * amt)), ("dir", Value::Vec3(dir))])],
        "relax" => vec![("bodyRelax", vec![on()])],
        "stayup" => vec![("stayUpright", vec![on(), ("stiffness", Value::Float(delirium_data::kv!(5)))])],
        "foetal" => vec![("bodyFoetal", vec![on()])],
        "grab" => {
            let at = chest + dir * delirium_data::kv!(10);
            vec![("grab", vec![on(), ("linegrab", Value::Bool(true)), ("useLeft", Value::Bool(true)), ("useRight", Value::Bool(true)),
                               ("dontLetGo", Value::Bool(true)), ("pos1", Value::Vec3(at + left * 0.5)), ("pos2", Value::Vec3(at - left * 0.5))])]
        }
        x if !x.is_empty() && x != "muscles" => vec![(x, vec![on()])],
        _ => vec![],
    }
}

pub fn step(net: &Net, w: &[f32], a: &mut Agent, build: &Build, world: &Collision, zero: &[Quat], p: &mut Pick) {
    let act = net.forward(w, &obs(&a.doll, p.on));
    let n = delirium_data::kv!(0 int) * (build.parts.len() - 1) + 2;
    if p.n % PICK_EVERY() == 0 && act.len() >= n + MENU.len() + delirium_data::kv!(0 int) {
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
    a.step(build, world, 1.0 / delirium_data::kv!(103), &[], &[]);
}

pub fn send_line(a: &mut Agent, build: &Build, world: &Collision, line: &str) {
    let mut w = line.split_whitespace();
    let Some(m) = w.next().and_then(msg::find) else { return };
    let ps: Vec<(&str, Value)> = w.filter_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        let kind = if k == "start" { msg::Kind::Bool } else { m.params.iter().find(|p| p.name == k)?.kind };
        Some((k, crate::msg::parse_value(kind, v)?))
    }).collect();
    let mut st = std::mem::take(&mut a.msgs);
    let _ = msg::send(a, build, world, &mut st, m.name, &ps);
    a.msgs = st;
}

pub fn standing(d: &Ragdoll) -> bool {
    let fr: Vec<Transform> = d.frames().collect();
    fr[HEAD()].translation.z > delirium_data::kv!(179) && fr[0].translation.z > delirium_data::kv!(20)
}

pub fn walk_obs(d: &Ragdoll, phase: f32) -> Vec<f32> {
    let mut x = body_obs(d, 1, true);
    let (s, c) = (phase * std::f32::consts::TAU).sin_cos();
    x.extend([s, c]);
    x
}

pub fn walk_sizes(d: &Ragdoll) -> [usize; 4] {
    [walk_obs(d, 0.0).len(), HIDDEN(), HIDDEN(), delirium_data::kv!(1 int)]
}

#[allow(non_snake_case)]
#[inline]
pub fn WALK_STEP_BUDGET() -> u32 { delirium_data::kv!(61 int) }

pub fn walk_agent(build: &Build, world: &Collision, frames: &[Transform]) -> Result<Agent, String> {
    let mut a = agent(build, frames);
    start_walk_balance(&mut a,build,world)?;
    Ok(a)
}

pub fn start_walk_balance(a:&mut Agent,build:&Build,world:&Collision)->Result<(),String>{
    let mut st = std::mem::take(&mut a.msgs);
    msg::send(a, build, world, &mut st, "bodyBalance", &[("maxSteps", Value::Int(WALK_STEP_BUDGET() as i32)), ("start", Value::Bool(true))]).map_err(|e| format!("bodyBalance: {e:?}"))?;
    a.msgs = st;
    Ok(())
}

pub fn walk_input(a: &mut Agent, build: &Build, world: &Collision, st: &mut msg::State, dir: Vec3,
                  wander: &mut crate::playerctl::Wander, rand: &mut delirium_data::rng::Rand) {
    let ped = a.doll.frames().next().map_or(Vec3::ZERO, |f| f.translation);
    let m = crate::playerctl::control(a, (0, -delirium_data::kv!(191 int)), (-dir.x).atan2(dir.y), ped, wander, rand);
    crate::shottask::apply(a, build, world, st, &m);
}

pub fn walk_config(act: &[f32]) -> crate::nm::ConfigureBalance {
    crate::nm::ConfigureBalance { leg_stiffness: delirium_data::kv!(21) + delirium_data::kv!(84) * act[0], step_height: delirium_data::kv!(78) * delirium_data::kv!(84 f32).powf(act[1]),
        step_clamp_scale: 2f32.powf(act[2]), ..Default::default() }
}

pub fn drive_walk(net: &Net, w: &[f32], a: &mut Agent, phase: &mut f32, duration: f32) {
    let act = net.forward(w, &walk_obs(&a.doll, *phase));
    tune_walk(a, &act);
    a.doll.kick(Vec3::ZERO);
    *phase = (*phase + 1.0 / (delirium_data::kv!(103) * duration)).rem_euclid(1.0);
}

pub fn tune_walk(a: &mut Agent, act: &[f32]) {
    a.configure_balance(walk_config(act));
    a.muscle_stiffness_x(2f32.powf(act[delirium_data::kv!(0 int)]));
}
