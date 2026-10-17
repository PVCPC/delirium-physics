use bevy::prelude::*;
use delirium_data::actiontable::{none, wrap, Action, ActionTable, ResultDef};
use delirium_data::anim::Animation;
use delirium_data::controls::{index, Controls, RageInput};
use delirium_data::melee::{self, Ask, Fighter, Held, Opponent, Target};
use delirium_data::pedpersonality::Personality;

pub struct Data {
    pub table: ActionTable,
    groups: Vec<(Vec<u8>, Vec<u8>, Vec<(Vec<u8>, u32)>)>,
    player_groups: [i32; 7],
    player: Personality,
    personality: Vec<u8>,
    pub weapons: Vec<delirium_data::weapons::Weapon>,
}

const WEAPON_GROUPS: [&str; 4] = ["club_core", "club_extra", "blade_core", "blade_extra"];

fn h(s: &str) -> u32 {
    delirium_data::map::hash_bytes(s.as_bytes(), 0)
}

pub fn load(meleeanims: &[u8], csv: &[u8], personality: &[u8]) -> Data {
    let mut groups = vec![];
    for g in delirium_data::meleeanims::read(meleeanims) {
        groups.push((g.name.clone(), g.dictionary.clone(), g.anims.iter().map(|a| (a.name.clone(), a.entry[0])).collect()));
        if g.trans {
            groups.push((g.trans_name(), g.dictionary.clone(), vec![]));
        }
    }
    let group = |n: &[u8]| groups.iter().position(|g| g.0.eq_ignore_ascii_case(n)).map_or(-1, |i| i as i32);
    let table = delirium_data::actiontable::read(csv, group, |g, n| {
        usize::try_from(g).ok().and_then(|g| groups.get(g)).and_then(|g| g.2.iter().find(|a| a.0.eq_ignore_ascii_case(n))).map_or(-1, |a| a.1 as i32)
    });
    let mut models = std::collections::HashMap::from([(h("player"), Personality::default())]);
    delirium_data::pedpersonality::read(personality, &mut models, |_, _| false);
    let player = models.remove(&h("player")).unwrap_or_default();
    let player_groups = player.groups.clone().map(|g| group(&g));
    Data { table, groups, player_groups, player, personality: personality.to_vec(), weapons: vec![] }
}

impl Data {
    pub fn clip(&self, r: &ResultDef) -> Option<String> {
        let (_, dict, anims) = self.groups.get(usize::try_from(r.anim_group).ok()?)?;
        let (name, _) = anims.iter().find(|a| a.1 as i32 == r.anim)?;
        Some(format!("{}/{}", String::from_utf8_lossy(dict), String::from_utf8_lossy(name)).to_lowercase())
    }

    pub fn clip_keys(&self) -> Vec<String> {
        let weapon = self.weapon_groups();
        self.table.results.iter().filter(|r| self.player_groups.contains(&r.anim_group) || weapon.contains(&r.anim_group))
            .filter_map(|r| self.clip(r)).chain(self.reaction_keys()).collect()
    }

    fn weapon_groups(&self) -> Vec<i32> {
        WEAPON_GROUPS.iter().filter_map(|n| self.groups.iter().position(|g| g.0.eq_ignore_ascii_case(n.as_bytes()))).map(|i| i as i32).collect()
    }

    pub fn weapon(&self, name: &str) -> Option<&delirium_data::weapons::Weapon> {
        self.weapons.iter().find(|w| w.name.eq_ignore_ascii_case(name))
    }

    pub fn reaction_keys(&self) -> Vec<String> {
        let mut k: Vec<String> = self.table.actions.iter().filter(|a| a.kind == h("HitReaction"))
            .filter_map(|a| self.result_of(a)).filter_map(|r| self.clip(&self.table.results[r])).collect();
        k.sort();
        k.dedup();
        k
    }

    pub fn ped_clip_keys(&self) -> Vec<String> {
        let mut named = std::collections::HashSet::new();
        for l in String::from_utf8_lossy(&self.personality).lines().filter(|l| !l.starts_with('#')) {
            named.extend(l.split(',').skip(11).take(7).map(|g| g.trim().to_ascii_lowercase()));
        }
        let ids: Vec<i32> = self.groups.iter().enumerate().filter(|g| named.contains(&String::from_utf8_lossy(&g.1 .0).to_ascii_lowercase())).map(|g| g.0 as i32).chain(self.weapon_groups()).collect();
        let mut keys: Vec<String> = self.table.results.iter().filter(|r| ids.contains(&r.anim_group)).filter_map(|r| self.clip(r)).collect();
        keys.sort();
        keys.dedup();
        keys
    }

    pub fn ped(&self, model: &str) -> Me {
        let mut models = std::collections::HashMap::from([(h(model), Personality::default())]);
        delirium_data::pedpersonality::read(&self.personality, &mut models, |_, _| false);
        let p = models.into_values().next().unwrap_or_default();
        let group = |n: &[u8]| self.groups.iter().position(|g| g.0.eq_ignore_ascii_case(n)).map_or(-1, |i| i as i32);
        Me { attack: p.attack, defend: p.defend, martial: p.melee_percent, groups: p.groups.clone().map(|g| group(&g)), go: false, moves: group(&p.move_group) }
    }

    pub fn stance_key(&self, moves: i32, id: u32) -> Option<String> {
        let name = delirium_data::meleeanims::MOVES.iter().find(|m| m.1[0] == id)?.0;
        let own = usize::try_from(moves).ok().and_then(|g| self.groups.get(g))
            .filter(|g| g.2.iter().any(|a| a.0.eq_ignore_ascii_case(name.as_bytes())))
            .map(|g| String::from_utf8_lossy(&g.1).to_string());
        Some(format!("{}/{name}", own.as_deref().unwrap_or("move_melee")).to_lowercase())
    }

    pub fn stance_keys(&self) -> Vec<String> {
        let mut named = std::collections::HashSet::new();
        for l in String::from_utf8_lossy(&self.personality).lines().filter(|l| !l.starts_with('#')) {
            named.extend(l.split(',').nth(18).map(|g| g.trim().to_ascii_lowercase()));
        }
        let groups = named.iter().filter_map(|n| self.groups.iter().position(|g| g.0.eq_ignore_ascii_case(n.as_bytes()))).map(|g| g as i32);
        STANCE_IDS.iter().filter_map(|&id| self.stance_key(-1, id)).chain(groups.filter_map(|g| self.stance_key(g, 0x0F))).collect()
    }

    pub fn result_of(&self, a: &Action) -> Option<usize> {
        (a.move_ != 0).then(|| self.table.results.iter().rposition(|r| r.name == a.move_)).flatten()
    }
}

#[derive(Default)]
pub struct Pad {
    pub controls: Controls,
    input: RageInput,
    set: bool,
}

impl Pad {
    pub fn feed(&mut self, buttons: u32, stick: [u8; 2], triggers: [u8; 2], now: u32) {
        if !self.set {
            self.controls.set_up_for_pad(0, now, true, 0);
            self.set = true;
        }
        let p = &mut self.input.pads[0];
        p.buttons = buttons;
        p.sticks = [stick[0], stick[1], 0x7F, 0x7F];
        (p.pressure[0xA], p.pressure[0xB]) = (triggers[0], triggers[1]);
        let sticks = self.controls.stick_values();
        self.controls.update(sticks, &mut self.input, now, false);
    }

    pub fn value(&self, c: u32) -> u8 {
        self.controls.values[index(c)].get()
    }

    pub fn pressed(&self, c: u32) -> bool {
        let v = &self.controls.values[index(c)];
        v.get() >= 0x80 && v.last ^ v.invert < 0x80
    }
}

const MELEE_BUTTONS: [u32; 4] = [0xBDC, 0xBE8, 0xC0C, 0xC00];
const ATTACK: u32 = 0xBDC;
pub const ATTACK_BIT: u32 = 0x20;
const QUIT: f32 = 1.5;
const TOO_CLOSE: f32 = 0.65;
const PUSH_RATE: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Home {
    pub point: Vec3,
    pub dist: f32,
    pub rate: f32,
    pub push: bool,
    pub snap: bool,
}

impl Home {
    pub fn speed(&self, pos: Vec3, heading: f32, anim: Vec2, travel: f32, dt: f32) -> Option<f32> {
        let dir = (pos - self.point).truncate().normalize_or_zero();
        let goal = self.point.truncate() + dir * (self.dist - travel).max(TOO_CLOSE);
        let v = (goal - pos.truncate()) / dt;
        let (right, fwd) = (Vec2::new(heading.cos(), heading.sin()), Vec2::new(-heading.sin(), heading.cos()));
        let (dx, mut dy) = (right.dot(v) - anim.x, fwd.dot(v) - anim.y);
        if self.push && dy > 0.0 {
            return None;
        }
        if self.rate >= 0.0 {
            let top = self.rate * dt;
            let l2 = dy.mul_add(dy, dx * dx);
            if l2 > top * top {
                dy = dy / l2.sqrt() * top;
            }
        } else if !self.snap {
            dy = 0.0;
        }
        Some(anim.y + dy)
    }
}

const HAND_SHORT: f32 = 0.06;
const SPACING: f32 = 0.7;
const MOST_POINTS: usize = 16;
const HIT_LIST: usize = 8;
const HANDS: [u32; 2] = [0x4C3, 0x4D0];
const WEAPON_KEY: u32 = 1 << 16;

#[derive(Clone, Debug, PartialEq)]
pub struct MeleeMove {
    pub action: u32,
    pub result: usize,
    pub key: Option<String>,
    pub clip_t: f32,
    pub dur: f32,
    pub rate: f32,
    pub step: f32,
    pub struck: bool,
    pub forced: bool,
    first: bool,
    window: bool,
    live: [bool; 2],
    was: [bool; 2],
    hits: Vec<u64>,
    events: u32,
    special: bool,
}

impl MeleeMove {
    pub fn phase(&self) -> f32 {
        if self.dur > 0.0 { (self.clip_t / self.dur).clamp(0.0, 1.0) } else { 0.0 }
    }
}

pub fn play_result(slot: &mut Option<MeleeMove>, d: &Data, a: &Action, player: bool, dur: &dyn Fn(&str) -> Option<f32>) {
    let Some(ri) = d.result_of(a) else {
        *slot = None;
        return;
    };
    let r = &d.table.results[ri];
    if let Some(m) = slot.as_mut().filter(|m| m.result == ri && m.key.is_some() && m.clip_t < m.dur * 0.999) {
        (m.action, m.struck, m.first, m.window, m.live, m.was) = (a.name, false, true, true, [false; 2], [false; 2]);
        m.forced = r.force_immediate_reaction;
        m.hits.clear();
        return;
    }
    let (pt, plen) = slot.as_ref().filter(|m| m.key.is_some()).map_or((0.0, 0.0), |m| (m.clip_t, m.dur));
    let key = d.clip(r).filter(|k| dur(k).is_some());
    let len = key.as_deref().and_then(dur).unwrap_or(0.0);
    let t = match r.sync_start_time {
        0 => r.start_time,
        1 => r.start_time + pt,
        2 => plen - pt + r.start_time,
        _ => 0.0,
    };
    let rate = r.playback_rate * if player { r.player_playback_mult } else { 1.0 };
    *slot = Some(MeleeMove { action: a.name, result: ri, key, clip_t: t.clamp(0.0, len), dur: len, rate, step: 0.0, struck: false, forced: r.force_immediate_reaction, first: true, window: true,
                          live: [false; 2], was: [false; 2], hits: vec![], events: 0, special: false });
}

pub const POINT_BONES: [u16; 5] = [1202, 1205, 13985, 419, 424];

#[derive(Clone, Copy, Debug, Default)]
pub struct Foe {
    pub feet: Vec3,
    pub heading: f32,
    pub health: f32,
    pub down: bool,
    pub me: Option<Me>,
    pub bones: [Option<Vec3>; 5],
    pub task: Option<(u32, Option<(f32, f32)>)>,
    pub block: bool,
}

impl Foe {
    pub fn with_move(self, d: &Data, mv: Option<&MeleeMove>) -> Foe {
        let r = mv.map(|m| &d.table.results[m.result]);
        Foe { task: mv.zip(r).map(|(m, r)| (r.name, m.key.is_some().then_some((m.clip_t, m.step)))), block: r.is_some_and(|r| r.is_a_block), ..self }
    }

    fn point(&self, kind: u32, from: [f32; 3]) -> Option<[f32; 3]> {
        let root = self.feet + Vec3::Z;
        let bone = |i: usize| self.bones[i].unwrap_or(root);
        let p = match kind {
            0 => root,
            1 | 3 => {
                let f = Vec3::from(from);
                let flat = (f - self.feet).with_z(0.0).normalize_or_zero();
                self.feet.with_z(f.z.clamp(self.feet.z, self.feet.z + 1.8)) + flat * 0.3
            }
            2 => bone(0),
            4..=7 => bone(kind as usize - 3),
            _ => return None,
        };
        Some(p.into())
    }
}

const BARE: Me = Me { attack: 0.8, defend: 1.3, martial: 50.0, groups: [-1; 7], go: false, moves: -1 };

pub const STANCE_IDS: [u32; 9] = [0x0F, 0x1C, 0x3E, 0x3F, 0x3D, 0x28, 0x41, 0x42, 0x40];

#[derive(Clone, Copy, Debug)]
pub struct Me {
    pub attack: f32,
    pub defend: f32,
    pub martial: f32,
    pub groups: [i32; 7],
    pub go: bool,
    pub moves: i32,
}

pub struct Ctx<'a> {
    pub pad: &'a Pad,
    pub pos: Vec3,
    pub health: f32,
    pub weapon: &'a delirium_data::weapons::Weapon,
    pub foe: Option<Foe>,
    pub me: Option<Me>,
    pub lock_on: bool,
}

pub const LOCK_LT: u32 = 0x960;
pub const LOCK_FULL: u8 = 238;

pub fn pick_score(from: Vec3, look: Vec3, at: Vec3, threat: f32) -> f32 {
    let d = from.distance(at);
    let h = if d < 1.0 { 1.0 } else { (((at - from) / d).dot(look) - 1.0) * (d - 1.0).min(1.0) + 1.0 };
    0.8 / d.max(1.0).powf(0.25) * h + 10.0 * threat
}

fn held(w: &delirium_data::weapons::Weapon) -> Held {
    let kind = melee::WEAPON_TYPES.iter().position(|t| *t == w.name).unwrap_or(0) as u32;
    let flags = [(5, "GUN"), (9, "MELEE_CLUB"), (10, "MELEE_BLADE"), (12, "2HANDED")].iter().filter(|f| w.has(f.1)).fold(0, |b, f| b | 1 << f.0);
    Held { kind, flags, between_shots: w.between_shots as i32 }
}

fn no_special(a: &Action) -> bool {
    a.special_test == 0 || a.special_test == none()
}

#[derive(Default)]
pub struct Melee {
    pub on: bool,
    pub mv: Option<MeleeMove>,
    timer: f32,
    pub started: bool,
    last: Vec<(u32, Vec3, Vec3)>,
    pub home: Option<Home>,
    pub stop_foe_homing: bool,
    pub disarm_foe: bool,
}

impl Melee {
    pub fn step<'c>(&mut self, d: &Data, c: &Ctx, heading: &mut f32, dt: f32, clip: &dyn Fn(&str) -> Option<&'c Animation>, rand: &mut dyn FnMut() -> u32) {
        let dur = &|k: &str| clip(k).map(|a| a.duration);
        self.started = false;
        self.home = None;
        self.stop_foe_homing = false;
        self.disarm_foe = false;
        let melee_weapon = c.weapon.fire == "MELEE";
        let p = &d.player;
        if let Some(me) = c.me {
            self.on = true;
            if !me.go {
                return self.play(d, c, heading, dt, clip);
            }
        } else if !self.on && !(melee_weapon && MELEE_BUTTONS.iter().any(|&b| c.pad.pressed(b))) {
            return;
        }
        let fighter = match c.me {
            Some(me) => Fighter { player: false, health: c.health, attack: me.attack, defend: me.defend, martial: me.martial, weapon: Some(held(c.weapon)) },
            None => Fighter { player: true, health: c.health, attack: p.attack, defend: p.defend, martial: p.melee_percent, weapon: Some(held(c.weapon)) },
        };
        let foe_fighter = |f: &Foe| match c.me {
            Some(_) => Fighter { player: true, health: f.health, attack: p.attack, defend: p.defend, martial: p.melee_percent, weapon: None },
            None => { let m = f.me.unwrap_or(BARE); Fighter { player: false, health: f.health, attack: m.attack, defend: m.defend, martial: m.martial, weapon: None } }
        };
        let point = |k: u32, from: [f32; 3]| c.foe.and_then(|f| f.point(k, from));
        let base = |fighter: Fighter| Ask {
            fighter, groups: [-1; 7], receiving_hit: false, blocking: false, just_struck: false, target: None, lock_on: false, impulses: false,
            controls: None, setting: 0, kind: None, idle: None, pos: c.pos.into(), heading: *heading, opponent_res: 0, special: &no_special,
        };
        let foe_groups = |f: &Foe| if c.me.is_some() { d.player_groups } else { f.me.map_or([-1; 7], |m| m.groups) };
        let foe_ask = c.foe.map(|f| Ask { pos: (f.feet + Vec3::Z).into(), heading: f.heading, groups: foe_groups(&f), blocking: f.block, ..base(foe_fighter(&f)) });
        let opponent = c.foe.zip(foe_ask.as_ref()).map(|(f, ask)| Opponent { task: f.task, lies: if f.down { 2 } else { 4 }, crouched: false, ask });
        let target = c.foe.map(|_| Target { entity_type: 3, point: &point, opponent: opponent.as_ref() });
        let ask = match c.me {
            Some(me) => Ask { groups: me.groups, lock_on: target.is_some(), target, ..base(fighter) },
            None => Ask { groups: d.player_groups, target, lock_on: c.lock_on, impulses: true, controls: Some(&c.pad.controls), ..base(fighter) },
        };
        if !self.on {
            let Some(a) = melee::choose(&d.table, &Ask { kind: Some(h("FromOnFoot")), idle: Some(true), ..ask }, &mut *rand) else { return };
            info!("melee: {} from on foot", d.table.results.get(d.result_of(a).unwrap_or(usize::MAX)).and_then(|r| d.clip(r)).unwrap_or_default());
            (self.on, self.timer) = (true, 0.0);
            play_result(&mut self.mv, d, a, true, dur);
            return;
        }
        let r = self.mv.as_ref().map(|m| &d.table.results[m.result]);
        let block = r.is_some_and(|r| r.is_a_block);
        self.timer = if c.pad.pressed(ATTACK) { 0.0 } else { self.timer + dt * if block { 2.0 } else { 1.0 } };
        if c.me.is_none() && (self.mv.is_none() || block) && self.timer > QUIT {
            (self.on, self.mv) = (false, None);
            return;
        }
        let ask = Ask { blocking: block, just_struck: self.mv.as_ref().is_some_and(|m| m.struck), ..ask };
        let next = match (&self.mv, r) {
            (Some(m), Some(r)) if !r.is_an_idle => {
                let open = |[s, e]: [f32; 2]| if m.clip_t >= s { m.clip_t < e } else { s < m.clip_t + m.step };
                let hits = r.branches.iter().filter(|b| b.0 && m.key.is_some() && open(b.2))
                    .filter_map(|b| d.table.actions.iter().find(|a| a.name == b.1))
                    .filter(|a| melee::action_test(a, &d.table, &ask)).collect();
                melee::pick(hits, &mut *rand)
            }
            _ => melee::choose(&d.table, &Ask { kind: Some(h("Melee")), idle: Some(true), ..ask }, &mut *rand),
        };
        if let Some(a) = next {
            if self.mv.as_ref().is_none_or(|m| d.result_of(a) != Some(m.result)) {
                info!("melee: next {}", d.result_of(a).and_then(|r| d.clip(&d.table.results[r])).unwrap_or("stance".into()));
            }
            play_result(&mut self.mv, d, a, c.me.is_none(), dur);
            self.started = true;
        }
        self.play(d, c, heading, dt, clip)
    }

    pub fn stop_homing(&mut self) {
        if let Some(m) = self.mv.as_mut() {
            m.window = false;
        }
    }

    fn play<'c>(&mut self, d: &Data, c: &Ctx, heading: &mut f32, dt: f32, clip: &dyn Fn(&str) -> Option<&'c Animation>) {
        let Some(m) = self.mv.as_mut() else { return };
        let r = &d.table.results[m.result];
        let paired = |bit| m.key.as_deref().and_then(clip).is_some_and(|a| !a.events.is_empty() && a.count_between(bit, 0.0, 1.0) == 2);
        let (a, b) = (paired(0x200), paired(0x400));
        if m.clip_t > r.stop_time || r.stop_on_crit_frame && m.events & 0x10 != 0
            || (a || b) && (m.struck || a && m.was[0] || b && m.was[1]) {
            m.window = false;
        }
        let root = c.foe.map(|f| f.bones[0].unwrap_or(f.feet + Vec3::Z));
        let far = root.is_none_or(|r| (r - c.pos).truncate().length_squared() >= TOO_CLOSE * TOO_CLOSE);
        if let (Some(f), false) = (c.foe, far) {
            let point = f.point(0, c.pos.into()).map_or(f.feet + Vec3::Z, Vec3::from);
            self.home = Some(Home { point, dist: TOO_CLOSE, rate: PUSH_RATE, push: true, snap: true });
        }
        if let Some(f) = c.foe.filter(|_| m.window && far) {
            let at = |k: u32| f.point(k, c.pos.into()).map(Vec3::from);
            let ranges = !r.homing || at(r.homing_target_type).is_some_and(|p| {
                let off = p - c.pos;
                let out = |x: f32, r: [f32; 2]| x < r[0] || x > r[1];
                !out(off.z, r.height_range) && !out(off.truncate().length(), r.distance_range)
                    && melee::angle_in(wrap((-off.x).atan2(off.y)) - wrap(*heading), r.angle_range)
            });
            let [dist, rate] = r.desired_distance;
            if let Some(point) = at(r.homing_target_type).filter(|_| ranges && rate != 0.0) {
                self.home = Some(Home { point, dist, rate, push: false, snap: m.first });
            }
            let [goal, rate] = r.desired_angle;
            if let Some(p) = at(r.homing_target_type).filter(|_| ranges && rate != 0.0) {
                let want = wrap(goal + (-(p.x - c.pos.x)).atan2(p.y - c.pos.y));
                let gap = wrap(want - wrap(*heading));
                if rate > 0.0 {
                    *heading = wrap(*heading + gap.clamp(-rate * dt, rate * dt));
                } else if m.first {
                    *heading = want;
                }
            }
        }
        m.first = false;
        if m.key.is_some() && !m.special {
            m.special = true;
            self.stop_foe_homing = r.special_result == h("SetOpponentStopMovesHoming");
        }
        self.disarm_foe = r.disarm_opponent && (m.key.is_none() || m.events & 0x200 != 0);
        if m.key.is_none() {
            self.mv = None;
            return;
        }
        m.step = dt * m.rate;
        let from = m.phase();
        m.clip_t += m.step;
        if m.clip_t >= m.dur {
            if r.is_loop && m.dur > 0.0 {
                m.clip_t %= m.dur;
            } else {
                self.mv = None;
                return;
            }
        }
        let ev = m.key.as_deref().and_then(clip).map_or(0, |a| a.advance(from, m.phase(), false, false).0);
        m.events = ev;
        if !r.is_an_idle && !r.is_a_block {
            for (n, bit) in [0x200, 0x400].into_iter().enumerate() {
                if ev & bit != 0 {
                    m.live[n] = !m.live[n];
                    m.was[n] |= m.live[n];
                }
            }
        }
    }

    pub fn strike(&mut self, d: &Data, feet: Vec3, heading: f32, part: &dyn Fn(u32) -> Option<(Vec3, Vec3, f32)>,
                  weapon: Option<(Vec3, Vec3, f32, f32)>, bodies: &[(u64, &[(Vec3, Vec3, f32, u16)])]) -> Vec<(u64, u16, Vec3, f32)> {
        let (turn, back) = (Quat::from_rotation_z(heading), Quat::from_rotation_z(-heading));
        let last = std::mem::take(&mut self.last);
        let Some(m) = self.mv.as_mut() else { return vec![] };
        let r = &d.table.results[m.result];
        let mut sweeps = vec![];
        for (n, &(some, tag, radius)) in r.strike_bones.iter().enumerate() {
            let Some((a, b, own)) = part(tag).filter(|_| some) else { continue };
            let held = weapon.filter(|_| tag == HANDS[1]).map(|(a, b, big, origin)| (WEAPON_KEY, a, b, (big, origin)));
            for (key, a, b, kind) in [(tag, a, b, (own, f32::NAN))].into_iter().chain(held) {
                let (a, b) = (back * (a - feet), back * (b - feet));
                self.last.push((key, a, b));
                let live = m.live[n] && !r.is_an_idle && !r.is_a_block;
                let Some(&(_, la, lb)) = last.iter().find(|l| l.0 == key).filter(|_| live) else { continue };
                let at = match kind {
                    (own, o) if o.is_nan() => points(a, b, own, tag, radius),
                    (big, origin) => weapon_points(a.distance(b), big, origin, radius),
                };
                for s in at {
                    let now = |p: Vec3, q: Vec3| feet + turn * p.lerp(q, s);
                    sweeps.push((now(la, lb), now(a, b), radius));
                }
            }
        }
        let mut out = vec![];
        for &(id, caps) in bodies {
            let met = sweeps.iter().find_map(|&(p, q, r)| caps.iter().filter_map(|&(a, b, cr, tag)| {
                let (x, y) = closest(p, q, a, b);
                let gap = x.distance(y);
                (gap <= r + cr).then_some((gap, tag, (x + y) * 0.5))
            }).min_by(|x, y| x.0.total_cmp(&y.0)));
            let Some((_, tag, at)) = met else { continue };
            if m.hits.contains(&id) {
                continue;
            }
            if m.hits.len() < HIT_LIST {
                m.hits.push(id);
            }
            m.struck = true;
            out.push((id, tag, at, r.on_hit_damage));
        }
        out
    }
}

fn points(a: Vec3, b: Vec3, own: f32, tag: u32, r: f32) -> Vec<f32> {
    let seg = a.distance(b);
    if seg <= 0.0 || r > seg * 0.5 + own {
        return vec![0.5];
    }
    along(-own + if HANDS.contains(&tag) { HAND_SHORT } else { 0.0 }, seg + own, seg, r)
}

pub fn weapon_axis(min: Vec3, max: Vec3) -> (Vec3, Vec3, f32) {
    let e = (max - min).abs();
    let axis = if e.x > e.y && e.x > e.z { Vec3::X } else if e.y > e.x && e.y > e.z { Vec3::Y } else { Vec3::Z };
    let (lo, hi) = (min.dot(axis), max.dot(axis));
    (axis * lo, axis * hi, if hi > lo { -lo / (hi - lo) } else { 0.5 })
}

fn weapon_points(seg: f32, big: f32, origin: f32, r: f32) -> Vec<f32> {
    if seg <= 0.0 || r > big {
        return vec![origin];
    }
    along(HAND_SHORT, seg, seg, r)
}

fn along(lo: f32, hi: f32, seg: f32, r: f32) -> Vec<f32> {
    let len = hi - lo;
    let at = |x: f32| x / seg;
    if len <= 2.0 * r {
        return vec![at(lo + len * 0.5)];
    }
    let gap = 2.0 * r * (1.0 - SPACING * SPACING).sqrt();
    let n = (((len - 2.0 * r) / gap).ceil() as usize + 1).min(MOST_POINTS);
    (0..n).map(|i| at(lo + r + (len - 2.0 * r) * i as f32 / (n - 1) as f32)).collect()
}

fn closest(p: Vec3, q: Vec3, a: Vec3, b: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, w) = (q - p, b - a, p - a);
    let (aa, ee, f) = (d1.length_squared(), d2.length_squared(), d2.dot(w));
    let (s, t) = if aa <= 1e-12 && ee <= 1e-12 {
        (0.0, 0.0)
    } else if aa <= 1e-12 {
        (0.0, (f / ee).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(w);
        if ee <= 1e-12 {
            ((-c / aa).clamp(0.0, 1.0), 0.0)
        } else {
            let bb = d1.dot(d2);
            let den = aa * ee - bb * bb;
            let s = if den > 1e-12 { ((bb * f - c * ee) / den).clamp(0.0, 1.0) } else { 0.0 };
            let t = (bb * s + f) / ee;
            if t < 0.0 {
                ((-c / aa).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((bb - c) / aa).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };
    (p + d1 * s, a + d2 * t)
}

pub fn part(tag: u32, at: &dyn Fn(u16) -> Option<Vec3>) -> Option<(Vec3, Vec3, f32)> {
    const LINES: [(u16, u16, bool, f32); 13] = [
        (0x4C1, 0x4C2, false, 0.07), (0x4C2, 0x4C3, false, 0.06), (0x4C3, 0x4C2, true, 0.05),
        (0x4C8, 0x4C9, false, 0.07), (0x4C9, 0x4D0, false, 0.06), (0x4D0, 0x4C9, true, 0.05),
        (0x1A2, 0x1A3, false, 0.09), (0x1A3, 0x1A4, false, 0.07), (0x1A4, 0x1A3, true, 0.06),
        (0x1A7, 0x1A8, false, 0.09), (0x1A8, 0x1A9, false, 0.07), (0x1A9, 0x1A8, true, 0.06),
        (0x4B5, 0x4B4, true, 0.12),
    ];
    let &(bone, other, before, radius) = LINES.iter().find(|l| l.0 as u32 == tag)?;
    let (a, o) = (at(bone)?, at(other)?);
    Some((a, if before { a + (a - o).normalize_or_zero() * 0.18 } else { o }, radius))
}

pub struct Blow {
    pub by: Foe,
    pub player: bool,
    pub res: u32,
    pub clip: Option<(f32, f32)>,
}

#[allow(clippy::too_many_arguments)]
pub fn react<'t>(d: &'t Data, me: &Foe, player: bool, weapon: Option<&delirium_data::weapons::Weapon>, mine: Option<&MeleeMove>, blow: &Blow,
                 gate: bool, tasks: [bool; 2], forced: bool, rand: &mut dyn FnMut() -> u32) -> melee::Reaction<'t> {
    let p = &d.player;
    let fighter = |f: &Foe, player: bool| if player {
        Fighter { player, health: f.health, attack: p.attack, defend: p.defend, martial: p.melee_percent, weapon: None }
    } else {
        let m = f.me.unwrap_or(BARE);
        Fighter { player, health: f.health, attack: m.attack, defend: m.defend, martial: m.martial, weapon: None }
    };
    let base = |f: &Foe, player: bool| Ask {
        fighter: fighter(f, player), groups: if player { d.player_groups } else { f.me.map_or([-1; 7], |m| m.groups) }, receiving_hit: false, blocking: false, just_struck: false,
        target: None, lock_on: false, impulses: false, controls: None, setting: 0, kind: None, idle: None, pos: (f.feet + Vec3::Z).into(),
        heading: f.heading, opponent_res: 0, special: &no_special,
    };
    let by_ask = base(&blow.by, blow.player);
    let opponent = Opponent { task: (blow.res != 0).then_some((blow.res, blow.clip)), lies: if blow.by.down { 2 } else { 4 }, crouched: false, ask: &by_ask };
    let point = |k: u32, from: [f32; 3]| blow.by.point(k, from);
    let mut ask = base(me, player);
    ask.fighter.weapon = weapon.map(held);
    ask.blocking = mine.is_some_and(|m| d.table.results[m.result].is_a_block);
    ask.target = Some(Target { entity_type: 3, point: &point, opponent: Some(&opponent) });
    ask.opponent_res = blow.res;
    if forced {
        let action = melee::choose(&d.table, &Ask { kind: Some(h("HitReaction")), ..ask }, rand);
        return melee::Reaction { action, result: action.and_then(|a| d.result_of(a)).map(|r| &d.table.results[r]), ragdoll: false };
    }
    melee::hit_reaction(&d.table, &ask, gate, tasks, rand)
}

