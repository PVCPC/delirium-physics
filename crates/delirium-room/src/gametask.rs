use delirium::msg::{Kind, Message};

pub fn joaat(s: &str) -> u32 {
    let mut h = 0u32;
    for c in s.bytes() {
        h = h.wrapping_add(c.to_ascii_lowercase() as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

#[derive(Default, Debug)]
pub struct Node {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
    pub kids: Vec<Node>,
}

impl Node {
    fn kid(&self, tag: &str) -> Option<&Node> {
        self.kids.iter().find(|k| k.tag == tag)
    }
    fn attr(&self, a: &str) -> Option<&str> {
        self.attrs.iter().find(|x| x.0 == a).map(|x| x.1.as_str())
    }
    fn value(&self) -> Option<&str> {
        self.attr("value")
    }
}

pub fn parse_xml(s: &str) -> Option<Node> {
    let mut stack = vec![Node::default()];
    let mut rest = s;
    while let Some(lt) = rest.find('<') {
        let text = rest[..lt].trim();
        if !text.is_empty() {
            stack.last_mut()?.text.push_str(text);
        }
        rest = &rest[lt + 1..];
        if rest.starts_with("!--") {
            rest = &rest[rest.find("-->")? + 3..];
            continue;
        }
        let gt = rest.find('>')?;
        let tag = &rest[..gt];
        rest = &rest[gt + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(_close) = tag.strip_prefix('/') {
            let n = stack.pop()?;
            stack.last_mut()?.kids.push(n);
            continue;
        }
        let (body, empty) = match tag.strip_suffix('/') {
            Some(b) => (b, true),
            None => (tag, false),
        };
        let (name, mut a) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
        let mut n = Node { tag: name.to_string(), ..Default::default() };
        while let Some((k, r)) = a.split_once("=\"") {
            let Some((v, r2)) = r.split_once('"') else { break };
            n.attrs.push((k.trim().to_string(), v.to_string()));
            a = r2;
        }
        if empty {
            stack.last_mut()?.kids.push(n);
        } else {
            stack.push(n);
        }
    }
    let mut top = stack.pop()?;
    top.kids.pop()
}

#[derive(Clone)]
pub enum Val {
    Bool(bool),
    Float(f32),
    Int(i32),
    Vec3([f32; 3]),
    Name(String),
}

pub struct SentParam {
    pub hash: u32,
    pub label: String,
    pub val: Val,
}

pub struct SentMessage {
    pub hash: u32,
    pub label: String,
    pub params: Vec<SentParam>,
    pub task_side: bool,
    pub force: bool,
    pub reset: bool,
    pub other: Vec<String>,
    pub items: usize,
}

pub struct Set {
    pub path: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub messages: Vec<SentMessage>,
}

fn hash_of(s: &str) -> Option<u32> {
    let s = s.strip_prefix("UNK_MEMBER_").unwrap_or(s);
    u32::from_str_radix(s.strip_prefix("0x")?, 16).ok()
}

fn name_hash(s: &str) -> u32 {
    hash_of(s).unwrap_or_else(|| joaat(s))
}

fn message(m: &Node) -> SentMessage {
    let raw = m.kid("Name").map_or("", |n| n.text.as_str());
    let mut out = SentMessage { hash: name_hash(raw), label: raw.to_string(), params: vec![], other: vec![], reset: false,
                                items: m.kid("Params").map_or(0, |p| p.kids.len()),
                                force: m.kid("ForceNewMessage").and_then(Node::value) == Some("true"),
                                task_side: m.kid("TaskMessage").and_then(Node::value) == Some("true") };
    for p in m.kid("Params").map_or(&[][..], |p| &p.kids) {
        let ty = p.attr("type").unwrap_or("");
        let name = p.kid("Name").map(|n| n.text.clone());
        let v = p.kid("Value");
        let num = |n: &Node, k: &str| n.attr(k).and_then(|x| x.parse::<f32>().ok()).unwrap_or(0.0);
        let val = match ty {
            "CNmParameterBool" => v.and_then(Node::value).map(|x| Val::Bool(x == "true")),
            "CNmParameterFloat" => v.and_then(Node::value).and_then(|x| x.parse().ok()).map(Val::Float),
            "CNmParameterInt" => v.and_then(Node::value).and_then(|x| x.parse().ok()).map(Val::Int),
            "CNmParameterVector" => v.map(|n| Val::Vec3([num(n, "x"), num(n, "y"), num(n, "z")])),
            "CNmParameterRandomFloat" => match (p.kid("Min").and_then(Node::value), p.kid("Max").and_then(Node::value)) {
                (Some(a), Some(b)) => Some(Val::Float(0.5 * (a.parse::<f32>().unwrap_or(0.0) + b.parse::<f32>().unwrap_or(0.0)))),
                _ => None,
            },
            "CNmParameterRandomInt" => match (p.kid("Min").and_then(Node::value), p.kid("Max").and_then(Node::value)) {
                (Some(a), Some(b)) => Some(Val::Int((a.parse::<i32>().unwrap_or(0) + b.parse::<i32>().unwrap_or(0)) / 2)),
                _ => None,
            },
            _ => None,
        };
        if let Some(s) = p.kid("Start").and_then(Node::value) {
            out.params.push(SentParam { hash: joaat("start"), label: "start".into(), val: Val::Bool(s == "true") });
        } else if let Some(pr) = p.kid("Priority").and_then(Node::value) {
            out.other.push(format!("Priority={pr}"));
        } else if let Some(pr) = p.kid("Blend").and_then(Node::value) {
            out.other.push(format!("Blend={pr}"));
        } else if ty == "CNmParameterResetMessage" {
            out.reset = true;
        } else if let Some(n) = p.kids.iter().find(|k| k.tag == "UNK_MEMBER_0xFE9CFFBA" || k.tag == "Mask") {
            out.params.push(SentParam { hash: joaat("mask"), label: "mask".into(), val: Val::Name(n.text.clone()) });
        } else if let Some(name) = name {
            let val = val.unwrap_or_else(|| Val::Name(v.map_or(String::new(), |n| n.text.clone())));
            out.params.push(SentParam { hash: name_hash(&name), label: name, val });
        } else {
            out.other.push(format!("{ty}?"));
        }
    }
    out
}

fn walk(n: &Node, trail: &mut Vec<String>, out: &mut Vec<Set>) {
    if let Some(ms) = n.kid("Messages") {
        let mut path = trail.clone();
        if let Some(id) = n.kid("Id") {
            path.push(id.text.clone());
        }
        out.push(Set { path, enabled: n.kid("Enabled").and_then(Node::value) != Some("false"),
                       priority: n.kid("Priority").and_then(Node::value).and_then(|x| x.parse().ok()).unwrap_or(0),
                       messages: ms.kids.iter().filter(|m| m.attr("type") == Some("CNmMessage")).map(message).collect() });
    }
    for k in n.kids.iter().filter(|k| k.tag != "Messages" && !k.kids.is_empty()) {
        let label = if k.tag == "Item" { k.kid("Name").or(k.kid("Id")).map_or("Item".to_string(), |x| x.text.clone()) } else { k.tag.clone() };
        trail.push(label);
        walk(k, trail, out);
        trail.pop();
    }
}

pub fn sets(file: &str) -> Option<Vec<Set>> {
    let root = parse_xml(file)?;
    let mut out = vec![];
    for it in &root.kid("Tunables")?.kids {
        let mut trail = vec![it.kid("Name").map_or(String::new(), |n| n.text.clone())];
        walk(it, &mut trail, &mut out);
    }
    Some(out)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TunableForce {
    pub enable: bool,
    pub mag: f32,
    pub scale_with_velocity: bool,
    pub velocity_min: f32,
    pub velocity_max: f32,
    pub force_at_min_velocity: f32,
    pub force_at_max_velocity: f32,
    pub clamp_impulse: bool,
    pub min_impulse: f32,
    pub max_impulse: f32,
    pub delay_ms: f32,
    pub duration_ms: f32,
    pub scale_with_upright: bool,
}

pub fn tunable_force(file: &str, tunables: &str, block: &str) -> Option<TunableForce> {
    let root = parse_xml(file)?;
    let it = root.kid("Tunables")?.kids.iter().find(|k| k.attr("type") == Some(tunables))?;
    let b = it.kid(block)?;
    let f = |n: &str| b.kid(n).and_then(Node::value).and_then(|v| v.parse::<f32>().ok());
    let t = |n: &str| b.kid(n).and_then(Node::value).map(|v| v == "true");
    Some(TunableForce {
        enable: t("Enable")?, mag: f("Mag")?, scale_with_velocity: t("ScaleWithVelocity")?, velocity_min: f("VelocityMin")?,
        velocity_max: f("VelocityMax")?, force_at_min_velocity: f("ForceAtMinVelocity")?, force_at_max_velocity: f("ForceAtMaxVelocity")?,
        clamp_impulse: t("ClampImpulse")?, min_impulse: f("MinImpulse")?, max_impulse: f("MaxImpulse")?, delay_ms: f("Delay")?,
        duration_ms: f("Duration")?, scale_with_upright: t("ScaleWithUpright")?,
    })
}

#[derive(Clone, Debug, Default)]
pub struct Tunables {
    pub nums: std::collections::HashMap<String, f32>,
    pub vecs: std::collections::HashMap<String, [f32; 3]>,
    pub texts: std::collections::HashMap<String, String>,
    pub blocks: std::collections::HashMap<String, std::collections::HashMap<String, f32>>,
}

impl Tunables {
    pub fn f(&self, n: &str, d: f32) -> f32 {
        self.nums.get(n).copied().unwrap_or(d)
    }
}

pub fn tunables(file: &str, tunables: &str) -> Option<Tunables> {
    let root = parse_xml(file)?;
    let it = root.kid("Tunables")?.kids.iter().find(|k| k.attr("type") == Some(tunables))?;
    let mut out = Tunables::default();
    for k in &it.kids {
        if let Some(v) = k.value() {
            let x = match v { "true" => Some(1.0), "false" => Some(0.0), v => v.parse().ok() };
            if let Some(x) = x {
                out.nums.insert(k.tag.clone(), x);
            }
        } else if let (Some(x), Some(y), Some(z)) = (k.attr("x"), k.attr("y"), k.attr("z")) {
            if let (Ok(x), Ok(y), Ok(z)) = (x.parse(), y.parse(), z.parse()) {
                out.vecs.insert(k.tag.clone(), [x, y, z]);
            }
        } else if k.kids.is_empty() && !k.text.is_empty() {
            out.texts.insert(k.tag.clone(), k.text.clone());
        } else if !k.kids.is_empty() && k.kids.iter().all(|c| c.value().is_some()) {
            let vals = k.kids.iter().filter_map(|c| {
                let v = c.value()?;
                let x = match v { "true" => Some(1.0), "false" => Some(0.0), v => v.parse().ok() }?;
                Some((c.tag.clone(), x))
            });
            out.blocks.insert(k.tag.clone(), vals.collect());
        }
    }
    Some(out)
}

impl TunableForce {
    pub fn impulse(&self, t_ms: f32, speed: f32, trunk_up_z: f32, dt: f32) -> f32 {
        if !self.enable || t_ms < self.delay_ms || t_ms > self.delay_ms + self.duration_ms {
            return 0.0;
        }
        let mut m = self.mag;
        if self.scale_with_velocity {
            let span = self.velocity_max - self.velocity_min;
            let x = if span > 0.0 { ((speed - self.velocity_min) / span).clamp(0.0, 1.0) } else { 1.0 };
            m *= self.force_at_min_velocity + (self.force_at_max_velocity - self.force_at_min_velocity) * x;
        }
        if self.scale_with_upright {
            m *= trunk_up_z.max(0.0);
        }
        m *= dt;
        if self.clamp_impulse { m.max(self.min_impulse).min(self.max_impulse) } else { m }
    }
}

pub fn read_file(dir: &str) -> Result<String, String> {
    let d = std::fs::read_dir(dir).map_err(|e| format!("{dir}: {e}"))?;
    for e in d.flatten() {
        let n = e.file_name().to_string_lossy().to_lowercase();
        if n.starts_with("physicstasks") && n.ends_with(".xml") {
            return std::fs::read_to_string(e.path()).map_err(|x| x.to_string());
        }
    }
    Err(format!("no physicstasks*.xml in {dir}"))
}

pub fn selects(sel: &str, path: &[String]) -> bool {
    let mut at = path.iter();
    sel.split('/').filter(|s| !s.is_empty()).all(|want| {
        let h = want.strip_prefix('?').map_or_else(|| hash_of(want).or(u32::from_str_radix(want, 16).ok().filter(|_| want.len() == 8)), |x| u32::from_str_radix(x, 16).ok());
        at.any(|p| p.eq_ignore_ascii_case(want) || hash_of(p).is_some_and(|ph| ph == joaat(want) || Some(ph) == h))
    })
}

const MESSAGE_SLOTS: usize = 64;

pub fn merge<'a>(sets: impl IntoIterator<Item = &'a Set>) -> Vec<SentMessage> {
    let mut on: Vec<&Set> = sets.into_iter().filter(|s| s.enabled && !s.messages.is_empty()).collect();
    on.sort_by_key(|s| s.priority);
    let mut out: Vec<SentMessage> = vec![];
    for m in on.iter().flat_map(|s| &s.messages) {
        let need = if m.force { MESSAGE_SLOTS } else { m.items };
        let here = out.iter_mut().rev().find(|x| x.hash == m.hash && x.items + need < MESSAGE_SLOTS);
        let x = match here {
            Some(x) => x,
            None => {
                out.push(SentMessage { hash: m.hash, label: m.label.clone(), params: vec![], task_side: m.task_side, force: m.force, reset: false, other: vec![], items: 0 });
                out.last_mut().unwrap()
            }
        };
        if m.reset {
            x.params.clear();
            x.items = 0;
        }
        for p in &m.params {
            let new = SentParam { hash: p.hash, label: p.label.clone(), val: p.val.clone() };
            let header = p.hash == joaat("start") || p.hash == joaat("mask");
            if !header {
                if x.items == MESSAGE_SLOTS {
                    x.other.push(format!("{} (past the message's 64 slots)", p.label));
                    continue;
                }
                x.items += 1;
            }
            match x.params.iter_mut().find(|q| q.hash == p.hash) {
                Some(q) => *q = new,
                None => x.params.push(new),
            }
        }
        x.other.extend(m.other.iter().cloned());
    }
    out
}

pub fn shot_sets(weapon: &str, hit: ShotHit, kill: bool) -> Vec<String> {
    let mut v = vec!["Base".to_string(), weapon.to_string()];
    match hit {
        ShotHit::Front => {}
        ShotHit::Back => v.push("3a2fb9af".into()),
        ShotHit::Leg => v.extend(["cd34be4d".to_string(), "staggerfall".to_string()]),
        ShotHit::Head => v.push("HeadShot".into()),
        ShotHit::Neck => v.push("0ea0c262".into()),
        ShotHit::Arm => v.push("ArmShot".into()),
        ShotHit::Hand => v.push("3eb0c6ba".into()),
    }
    if kill {
        v.push("FatallyInjured".into());
    }
    v
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ShotHit {
    Front,
    Back,
    Leg,
    Head,
    Neck,
    Arm,
    Hand,
}

pub fn gate_modules(merged: Vec<SentMessage>, hit: ShotHit, part: usize) -> Vec<SentMessage> {
    let flag = |m: &SentMessage, n: &str| m.params.iter().any(|p| p.hash == joaat(n) && matches!(p.val, Val::Bool(true)));
    let shot = merged.iter().find(|m| m.hash == joaat("shot"));
    let (allow_leg, allow_arm) = (shot.is_some_and(|s| flag(s, "allowInjuredLeg")), shot.is_some_and(|s| flag(s, "allowInjuredArm")));
    let (leg, arm) = (matches!(part, 1..=6), matches!(part, 13..=20));
    merged.into_iter().filter(|m| {
        let h = m.hash;
        let is = |n: &str| h == joaat(n);
        if is("shotsnap") {
            flag(m, "snap")
        } else if is("shotinguts") {
            flag(m, "shotInGuts") && std::env::var_os("RDR2_SHOT_GUTS").is_some()
        } else if is("shotfrombehind") {
            flag(m, "shotFromBehind") && hit == ShotHit::Back
        } else if is("shotspin") {
            flag(m, "shotspin") || m.params.iter().any(|p| spine_limit(p.hash))
        } else if is("shotfalltoknees") {
            flag(m, "FallToKnees")
        } else if is("configureshotinjuredleg") {
            allow_leg && leg
        } else if is("configureshotinjuredarm") {
            allow_arm && arm
        } else {
            true
        }
    }).map(|mut m| {
        if m.hash == joaat("shotspin") && !flag(&m, "shotspin") {
            m.params.retain(|p| spine_limit(p.hash));
            m.params.push(SentParam { hash: joaat("start"), label: "start".into(), val: Val::Bool(true) });
            m.params.push(SentParam { hash: joaat("shotSpin"), label: "shotSpin".into(), val: Val::Bool(false) });
        }
        m
    }).collect()
}

fn spine_limit(hash: u32) -> bool {
    ["spinelean1limit", "spinelean2limit", "spinetwistlimit", "torqueexaggeratejumplimit"].iter().any(|n| joaat(n) == hash)
}

pub fn shot_set<'a>(all: &'a [Set], id: &str) -> Option<&'a Set> {
    let want = u32::from_str_radix(id, 16).ok().filter(|_| id.len() == 8).unwrap_or_else(|| joaat(id));
    all.iter().find(|s| {
        s.path.first().is_some_and(|o| hash_of(o) == Some(joaat("CTaskNMShot")) || o.eq_ignore_ascii_case("CTaskNMShot") || hash_of(o) == Some(0xC9CD39F1))
            && s.path.last().is_some_and(|l| name_hash(l) == want)
    })
}

fn shot_block(hash: u32) -> bool {
    ["shotconfigurearms", "shotshockspin", "shotheadlook"].iter().any(|n| joaat(n) == hash)
}

fn core_message(hash: u32) -> bool {
    ["configuresoftlimit", "setfrictionscale", "setmusclestiffness", "setcharacterstrength", "configurebullets", "configurebulletsextra", "setopposegravity", "setcharacterunderwater",
     "configurelimits", "configureconstraints", "softkey", "configureselfavoidance", "shotnewbullet", "setcharactercollisions", "setcharacterdamping"].iter().any(|n| joaat(n) == hash) || hash == 0xb7960dec || hash == 0xe35a5d1a
}

pub fn port_message(hash: u32) -> Option<&'static Message> {
    let alias = [("sit", "rdr2Sit"), ("configurebolas", "rdr2Bolas"), ("configureshotinjuredleg", "shotInjuredLeg"), ("configureshotinjuredarm", "shotInjuredArm"),
                 ("shotconfigurearms", "shot"), ("shotshockspin", "shot"), ("shotheadlook", "shot"), ("configurebulletsextra", "configureBullets"),];
    let name = alias.iter().find(|a| joaat(a.0) == hash).map(|a| a.1).or(match hash { 0xb7960dec => Some("spinLimit"), 0xe35a5d1a => Some("quadLimbLimit"), _ => None });
    let all = || delirium::msg::MESSAGES.iter().chain(delirium::extras::messages());
    match name {
        Some(n) => all().find(|m| m.name == n),
        None => all().find(|m| joaat(m.name) == hash),
    }
}

pub fn line(m: &SentMessage, port: &Message) -> (String, Vec<String>) {
    let mut s = port.name.to_string();
    let mut left = m.other.clone();
    let (block, core) = (shot_block(m.hash), core_message(m.hash));
    if core {
        s += " start=true";
        let start = m.params.iter().find(|p| p.hash == joaat("start")).and_then(|p| if let Val::Bool(b) = p.val { Some(b) } else { None });
        if start == Some(false) && port.params.iter().any(|x| x.name == "on") {
            s += " on=false";
        }
        if start == Some(true) && port.params.iter().any(|x| x.name == "fresh") {
            s += " fresh=true";
        }
        if m.hash == joaat("configurebulletsextra") {
            s += " extra=true";
        }
    }
    for p in &m.params {
        if p.hash == joaat("start") {
            if let (Val::Bool(b), false, false) = (&p.val, block, core) {
                s += &format!(" start={b}");
            }
            continue;
        }
        let own = [(0xa7180f98u32, "clamp"), (0x2bc0e7e9, "flatterSwingFeetRun"), (0xc5ebfd7b, "avoidFootWidthB"), (0x428c2b6f, "turnLiftFeet"), (0xcfe51361, "turnLiftFeetVariance"),
                   (0x96c82edf, "minLegTwistVariance"), (0x5831d084, "maxLegTwistVariance"), (0x747a3a5c, "slopePelvisBlend"), (0xa81569cc, "slopeDrop2"), (0xbcda0e93, "slopeDrop1"),
                   (0x47e73885, "maxStillTime"), (0x9de396e2, "spineBlendPose"), (0xae9bb2a3, "kTime"), (0x9be8054b, "spineLeanGain"), (0xcb0eb739, "armFriction")];
        let wanted = own.iter().find(|o| o.0 == p.hash).map(|o| o.1).or_else(|| crate::own_names::OWN.iter().find(|o| o.0 == p.hash && port.params.iter().any(|x| x.name == o.1)).map(|o| o.1)).map_or(p.hash, joaat);
        if ["snap", "shotinguts", "shotfrombehind", "shotspin", "falltoknees"].iter().any(|n| joaat(n) == p.hash) && m.hash != joaat("shot") {
            if m.hash == joaat("shotspin") && p.hash == joaat("shotspin") && matches!(p.val, Val::Bool(false)) {
                s += " shotSpin=false";
            }
            continue;
        }
        let by_hash = format!("?{:08x}", p.hash);
        let Some(prm) = port.params.iter().find(|x| joaat(x.name) == wanted || x.name == by_hash) else {
            left.push(format!("{} (no such parameter)", p.label));
            continue;
        };
        let txt = match (&p.val, prm.kind) {
            (Val::Bool(b), Kind::Bool) => b.to_string(),
            (Val::Bool(b), Kind::Int) => (*b as i32).to_string(),
            (Val::Bool(b), Kind::Float) => (*b as i32).to_string(),
            (Val::Float(x), Kind::Float) => x.to_string(),
            (Val::Float(x), Kind::Int) => (*x as i32).to_string(),
            (Val::Int(i), Kind::Int) => i.to_string(),
            (Val::Int(i), Kind::Float) => (*i as f32).to_string(),
            (Val::Int(i), Kind::Bool) => (*i != 0).to_string(),
            (Val::Vec3(v), Kind::Vec3) => format!("{},{},{}", v[0], v[1], v[2]),
            (Val::Name(n), Kind::Str) => match hash_of(n) {
                Some(h) => match ["fb", "ub", "lb", "us", "un", "uk", "ua", "ul", "ur", "uc", "uw", "ll", "lr", "la", "ra", "ls", "rs"].iter().find(|w| joaat(w) == h) {
                    Some(w) => w.to_string(),
                    None => { left.push(format!("{} = {n} (a name not known here)", p.label)); continue; }
                },
                None => n.clone(),
            },
            (Val::Name(n), _) => { left.push(format!("{} = {n} (held by hash)", p.label)); continue; }
            _ => { left.push(format!("{} (another type here)", p.label)); continue; }
        };
        s += &format!(" {}={txt}", prm.name);
    }
    (s, left)
}

pub const FILES: [&str; 2] = ["update_4.rpf|x64/data/tune/physicstasks", "data_0.rpf|data/tune/physicstasks"];

const NAMES: &[&str] = &[
    "Tunables", "Name", "Id", "Messages", "Params", "Value", "Min", "Max", "Start", "Priority", "Blend", "Enabled", "TaskMessage",
    "ForceNewMessage", "Mask", "CNmMessage", "CNmParameterBool", "CNmParameterFloat", "CNmParameterInt", "CNmParameterVector",
    "CNmParameterRandomFloat", "CNmParameterRandomInt", "CNmParameterResetMessage", "CNmParameterString", "CNmParameterStart",
    "CNmParameterMask", "CNmParameterPriority", "CNmParameterBlend",
];

pub fn from_pso(d: &[u8]) -> Result<String, String> {
    rdr2_data::pso::to_xml(d, NAMES)
}
