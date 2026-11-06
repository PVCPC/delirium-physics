use std::sync::OnceLock;

use bevy::prelude::*;
use delirium_data::actiontable::{wrap, Action, ActionTable, ResultDef};
use delirium_data::anim::Animation;
use delirium_data::controls::{index, Controls, RageInput};
use delirium_data::melee::{self, Ask, Fighter, Held, Opponent, Target};
use delirium_data::pedpersonality::Personality;

pub struct Data {
    pub table: ActionTable,
    groups: Vec<(Vec<u8>, Vec<u8>, Vec<(Vec<u8>, u32, u32)>)>,
    slotted: Vec<bool>,
    player_groups: [i32; 7],
    player: Personality,
    personality: Vec<u8>,
    pub weapons: Vec<delirium_data::weapons::Weapon>,
}

impl Data {
}

#[allow(non_snake_case)]
#[inline]
pub fn UPPER_BODY() -> u32 { delirium_data::kv!(1075 int) }
#[allow(non_snake_case)]
#[inline]
pub fn HEAD_NECK_AND_ARMS() -> u32 { delirium_data::kv!(1076 int) }

pub fn bone_mask(flags: u32) -> u8 {
    (flags & delirium_data::kv!(189 int)) as u8
}

pub fn partial(key: &str) -> bool {
    DATA.get().and_then(|d| d.clip_flags(key)).is_some_and(|f| bone_mask(f) != 0)
}

pub fn clip_name(clip: &str) -> &str {
    clip.split_once('#').map_or(clip, |c| c.0)
}

#[allow(non_snake_case)]
#[inline]
pub fn ROTATES_ROOT() -> u32 { delirium_data::kv!(1077 int) }

pub fn carries_turn(flags: u32) -> bool {
    flags & delirium_data::kv!(1078 int) != 0
}

pub fn turns(key: &str) -> bool {
    DATA.get().and_then(|d| d.clip_flags(key)).is_some_and(carries_turn)
}

const WEAPON_GROUPS: [&str; 5] = ["club_core", "club_extra", "blade_core", "blade_extra", "firearm_core"];

pub static DATA: OnceLock<Data> = OnceLock::new();

fn h(s: &str) -> u32 {
    delirium_data::map::hash_bytes(s.as_bytes(), 0)
}

pub fn load(meleeanims: &[u8], csv: &[u8], personality: &[u8], animgrp: &[u8], ambient: &[u8], exe: (u32, &[u8])) -> Data {
    use delirium_data::animgrp;
    let file = animgrp::read(animgrp, exe).expect("animgrp.dat: the game stops on it (a missing token, or one sprintf reads an argument for)");
    let before: Vec<Vec<u8>> = animgrp::BUILT_IN.iter().map(|n| n.as_bytes().to_vec()).chain(file.iter().map(|g| g.name.clone())).collect();
    let mut slotted: Vec<bool> = (0..animgrp::BUILT_IN.len()).map(|k| animgrp::BUILT_IN_SLOTS.iter().any(|s| s.0 == k))
        .chain(file.iter().map(|_| true)).collect();
    let added = animgrp::ambient_groups(ambient, &before).expect("Ambient.dat: a definition short of its dictionary (the game faults on it)");
    let mut groups: Vec<(Vec<u8>, Vec<u8>, Vec<(Vec<u8>, u32, u32)>)> = before.into_iter().chain(added.into_iter().map(|g| g.name))
        .map(|n| (n, vec![], vec![])).collect();
    slotted.resize(groups.len(), false);
    for g in delirium_data::meleeanims::read(meleeanims).expect("meleeanims.dat: a line short of its tokens (the game faults on it)") {
        let stored = |name: &[u8]| animgrp::Group::stored(name, &g.dictionary).name;
        groups.push((stored(&g.name), g.dictionary.clone(), g.anims.iter().map(|a| (a.name.clone(), a.entry[0], a.entry[1])).collect()));
        if g.trans {
            groups.push((stored(&g.trans_name()), g.dictionary.clone(), vec![]));
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
    slotted.resize(groups.len(), false);
    Data { table, groups, player_groups, player, personality: personality.to_vec(), weapons: vec![], slotted }
}

impl Data {
    pub fn clip(&self, r: &ResultDef) -> Option<String> {
        let (_, dict, anims) = self.groups.get(usize::try_from(r.anim_group).ok()?)?;
        let (name, _, flags) = anims.iter().find(|a| a.1 as i32 == r.anim)?;
        let key = format!("{}/{}", String::from_utf8_lossy(dict), String::from_utf8_lossy(name)).to_lowercase();
        Some(if self.first_flags(&key) == Some(*flags) { key } else { format!("{key}#{flags:x}") })
    }

    pub fn clip_flags(&self, key: &str) -> Option<u32> {
        match key.split_once('#') {
            Some((_, flags)) => u32::from_str_radix(flags, delirium_data::kv!(54 int)).ok(),
            None => self.first_flags(key),
        }
    }

    fn first_flags(&self, key: &str) -> Option<u32> {
        let (dict, name) = key.split_once('/')?;
        self.groups.iter().filter(|g| g.1.eq_ignore_ascii_case(dict.as_bytes()))
            .find_map(|g| g.2.iter().find(|a| a.0.eq_ignore_ascii_case(name.as_bytes())).map(|a| a.2))
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
        self.info(name).filter(|w| w.fire == "MELEE")
    }

    pub fn info(&self, name: &str) -> Option<&delirium_data::weapons::Weapon> {
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
            named.extend(l.split(',').skip(delirium_data::kv!(113 int)).take(delirium_data::kv!(22 int)).map(|g| g.trim().to_ascii_lowercase()));
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

    pub fn bravery(&self, model: &str) -> u8 {
        let mut models = std::collections::HashMap::from([(h(model), Personality::default())]);
        delirium_data::pedpersonality::read(&self.personality, &mut models, |_, _| false);
        models.into_values().next().unwrap_or_default().bravery
    }

    pub fn fight_group(&self, weapon: Option<&delirium_data::weapons::Weapon>, moves: i32) -> i32 {
        let g = match weapon.filter(|w| w.name != "UNARMED") {
            Some(w) => w.melee_groups[0].as_ref().and_then(|n| self.groups.iter().position(|g| g.0.eq_ignore_ascii_case(n.as_bytes())))
                .map_or(-1, |g| g as i32),
            None => moves,
        };
        let slot = usize::try_from(g).ok().and_then(|g| self.slotted.get(g)).is_some_and(|&s| s);
        if g > -1 && IN(g) && slot && IN(g) { g } else { MOVE_MELEE() }
    }

    pub fn stance_key(&self, moves: i32, id: u32) -> Option<String> {
        let name = delirium_data::meleeanims::MOVES.iter().find(|m| m.1[0] == id)?.0;
        let own = usize::try_from(moves).ok().and_then(|g| self.groups.get(g))
            .filter(|g| g.2.iter().any(|a| a.0.eq_ignore_ascii_case(name.as_bytes())))
            .map(|g| String::from_utf8_lossy(&g.1).to_string());
        Some(format!("{}/{name}", own.as_deref().unwrap_or("move_melee")).to_lowercase())
    }

    pub fn player_moves(&self) -> i32 {
        self.groups.iter().position(|g| g.0.eq_ignore_ascii_case(&self.player.move_group)).map_or(-1, |i| i as i32)
    }

    pub fn stance_keys(&self) -> Vec<String> {
        let mut named = std::collections::HashSet::new();
        for l in String::from_utf8_lossy(&self.personality).lines().filter(|l| !l.starts_with('#')) {
            named.extend(l.split(',').nth(delirium_data::kv!(217 int)).map(|g| g.trim().to_ascii_lowercase()));
        }
        let groups = named.iter().filter_map(|n| self.groups.iter().position(|g| g.0.eq_ignore_ascii_case(n.as_bytes()))).map(|g| g as i32);
        let weapons = self.weapons.iter().map(|w| self.fight_group(Some(w), -1)).filter(|&g| g >= 0)
            .flat_map(|g| (*STANCE_IDS()).iter().filter_map(move |&id| self.stance_key(g, id)));
        let mut keys: Vec<String> = (*STANCE_IDS()).iter().filter_map(|&id| self.stance_key(-1, id))
            .chain(groups.filter_map(|g| self.stance_key(g, delirium_data::kv!(189 int)))).chain(weapons).collect();
        keys.sort();
        keys.dedup();
        keys
    }

    pub fn opponent_depth(&self) -> usize {
        fn hops(t: &ActionTable, a: &Action, left: usize) -> usize {
            let find = |n: u32| (n != 0 && left > 0).then(|| t.actions.iter().find(|b| b.name == n)).flatten();
            let o = find(a.opponents_action_to_test).map_or(0, |b| 1 + hops(t, b, left - 1));
            o.max(find(a.additional_action_test).map_or(0, |b| hops(t, b, left - 1)))
        }
        let n = self.table.actions.len();
        1 + self.table.actions.iter().filter(|a| a.opponents_action_to_test != 0).map(|a| hops(&self.table, a, n)).max().unwrap_or(0)
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
        p.sticks = [stick[0], stick[1], delirium_data::kv!(191 int), delirium_data::kv!(191 int)];
        (p.pressure[delirium_data::kv!(81 int)], p.pressure[delirium_data::kv!(113 int)]) = (triggers[0], triggers[1]);
        let sticks = self.controls.stick_values();
        self.controls.update(sticks, &mut self.input, now, false);
    }

    pub fn value(&self, c: u32) -> u8 {
        self.controls.values[index(c)].get()
    }

    pub fn pressed(&self, c: u32) -> bool {
        let v = &self.controls.values[index(c)];
        v.get() >= delirium_data::kv!(257 int) && v.last ^ v.invert < delirium_data::kv!(257 int)
    }

    pub fn held(&self, c: u32) -> bool {
        self.value(c) >= delirium_data::kv!(257 int)
    }

    pub fn released(&self, c: u32) -> bool {
        let v = &self.controls.values[index(c)];
        v.get() < delirium_data::kv!(257 int) && v.last ^ v.invert >= delirium_data::kv!(257 int)
    }
}

pub(crate) fn starts(pad: &Pad, gun: bool) -> bool {
    let edge = |b: u32| pad.pressed(b) || pad.released(b);
    let melee = if pad.held(LOCK_LT()) { (*MELEE_BUTTONS()).iter().any(|&b| edge(b)) } else { edge(ATTACK()) };
    melee || gun && (*GUN_BUTTONS()).iter().any(|&b| edge(b))
}

fn gun_in_hand(c: &Ctx) -> bool {
    c.in_hand && c.weapon.damage_type != "MELEE" && !(EPISODE == 2 && c.weapon.slot == "THROWN")
}

fn fists_or_melee(w: &delirium_data::weapons::Weapon) -> bool {
    w.name == "UNARMED" || w.damage_type == "MELEE"
}

const EPISODE: u32 = 0;

#[allow(non_snake_case)]
#[inline]
fn MELEE_BUTTONS() -> &'static [u32; 4] { static V: std::sync::OnceLock<[u32; 4]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(690 int), delirium_data::kv!(691 int), delirium_data::kv!(695 int), delirium_data::kv!(694 int)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn GUN_BUTTONS() -> &'static [u32; 2] { static V: std::sync::OnceLock<[u32; 2]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(692 int), delirium_data::kv!(693 int)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn ATTACK() -> u32 { delirium_data::kv!(690 int) }
#[allow(non_snake_case)]
#[inline]
pub fn ATTACK_BIT() -> u32 { delirium_data::kv!(7 int) }
#[allow(non_snake_case)]
#[inline]
fn QUIT() -> f32 { delirium_data::kv!(12) }
#[allow(non_snake_case)]
#[inline]
fn TOO_CLOSE() -> f32 { delirium_data::kv!(8) }
#[allow(non_snake_case)]
#[inline]
fn PUSH_RATE() -> f32 { delirium_data::kv!(25) }

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
        let goal = self.point.truncate() + dir * (self.dist - travel).max(TOO_CLOSE());
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

pub fn homing(r: &ResultDef, foe: Option<Foe>, pos: Vec3, heading: &mut f32, window: bool, first: bool, dt: f32) -> Option<Home> {
    let f = foe?;
    let root = f.bones[0].unwrap_or(f.feet + Vec3::Z);
    if (root - pos).truncate().length_squared() < TOO_CLOSE() * TOO_CLOSE() {
        let point = f.point(0, pos.into()).map_or(f.feet + Vec3::Z, Vec3::from);
        return Some(Home { point, dist: TOO_CLOSE(), rate: PUSH_RATE(), push: true, snap: true });
    }
    if !window {
        return None;
    }
    let at = |k: u32| f.point(k, pos.into()).map(Vec3::from);
    let ranges = !r.homing || at(r.homing_target_type).is_some_and(|p| {
        let off = p - pos;
        let out = |x: f32, r: [f32; 2]| x < r[0] || x > r[1];
        !out(off.z, r.height_range) && !out(off.truncate().length(), r.distance_range)
            && melee::angle_in(wrap((-off.x).atan2(off.y)) - wrap(*heading), r.angle_range)
    });
    let [dist, rate] = r.desired_distance;
    let home = at(r.homing_target_type).filter(|_| ranges && rate != 0.0).map(|point| Home { point, dist, rate, push: false, snap: first });
    let [goal, rate] = r.desired_angle;
    if let Some(p) = at(r.homing_target_type).filter(|_| ranges && rate != 0.0) {
        let want = wrap(goal + (-(p.x - pos.x)).atan2(p.y - pos.y));
        let gap = wrap(want - wrap(*heading));
        if rate > 0.0 {
            *heading = wrap(*heading + gap.clamp(-rate * dt, rate * dt));
        } else if first {
            *heading = want;
        }
    }
    home
}

#[allow(non_snake_case)]
#[inline]
fn HAND_SHORT() -> f32 { delirium_data::kv!(146) }
#[allow(non_snake_case)]
#[inline]
fn SPACING() -> f32 { delirium_data::kv!(74) }
#[allow(non_snake_case)]
#[inline]
fn MOST_POINTS() -> usize { delirium_data::kv!(54 int) }
#[allow(non_snake_case)]
#[inline]
fn HIT_LIST() -> usize { delirium_data::kv!(23 int) }
#[allow(non_snake_case)]
#[inline]
fn HANDS() -> &'static [u32; 2] { static V: std::sync::OnceLock<[u32; 2]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(59 int), delirium_data::kv!(58 int)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
#[allow(non_snake_case)]
#[inline]
fn WEAPON_KEY() -> u32 { 1 << delirium_data::kv!(54 int) }

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
    pub swing: bool,
    first: bool,
    window: bool,
    live: [bool; 2],
    was: [bool; 2],
    hits: Vec<u64>,
    events: u32,
    special: bool,
    hurt: bool,
    pub given: bool,
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
    if let Some(m) = slot.as_mut().filter(|m| m.result == ri && m.key.is_some() && m.clip_t < m.dur * delirium_data::kv!(268)) {
        (m.action, m.struck, m.first, m.window, m.live, m.was) = (a.name, false, true, true, [false; 2], [false; 2]);
        m.forced = r.force_immediate_reaction;
        m.swing = !r.is_an_idle && !r.is_a_block;
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
    *slot = Some(MeleeMove { action: a.name, result: ri, key, clip_t: t.clamp(0.0, len), dur: len, rate, step: 0.0, struck: false, forced: r.force_immediate_reaction, swing: !r.is_an_idle && !r.is_a_block, first: true, window: true,
                          live: [false; 2], was: [false; 2], hits: vec![], events: 0, special: false, hurt: false, given: false });
}

#[allow(non_snake_case)]
#[inline]
pub fn POINT_BONES() -> &'static [u16; 5] { static V: std::sync::OnceLock<[u16; 5]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(776 int), delirium_data::kv!(209 int), delirium_data::kv!(213 int), delirium_data::kv!(611 int), delirium_data::kv!(614 int)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

#[derive(Clone, Copy, Debug)]
pub struct Foe {
    pub feet: Vec3,
    pub heading: f32,
    pub health: f32,
    pub lies: u8,
    pub me: Option<Me>,
    pub bones: [Option<Vec3>; 5],
    pub task: Option<(u32, Option<(f32, f32)>)>,
    pub block: bool,
    pub velocity: Vec3,
    pub flags_240: u32,
    pub from_idle: bool,
    pub on_stairs: bool,
    pub weapon: Option<Held>,
    pub struck: bool,
    pub frame_box: Option<([f32; 3], [f32; 3])>,
}

impl Default for Foe {
    fn default() -> Foe {
        Foe { feet: Vec3::ZERO, heading: 0.0, health: 0.0, lies: delirium_data::kv!(1 int), me: None, bones: [None; 5], task: None, block: false, velocity: Vec3::ZERO,
              flags_240: 0, from_idle: false, on_stairs: false, weapon: None, struck: false, frame_box: None }
    }
}

pub fn lies(spine: Option<&GlobalTransform>, ped_z: f32, ragdoll_feet: Option<[&GlobalTransform; 2]>) -> u8 {
    let Some(s) = spine else { return delirium_data::kv!(1 int) };
    let m = s.affine().matrix3;
    let (x_z, y_z, spine_z) = (to_gta(m.x_axis.into()).z, to_gta(m.y_axis.into()).z, to_gta(s.translation()).z);
    match ragdoll_feet {
        Some(f) => delirium_data::fall::lies_on_ragdoll(x_z, y_z, spine_z, f.map(|f| to_gta(f.translation()).z)),
        None => delirium_data::fall::lies(x_z, y_z, spine_z, ped_z),
    }
}

pub fn frag_box(feet: Vec3, heading: f32, parts: impl IntoIterator<Item = (Vec3, Vec3, f32)>) -> Option<([f32; 3], [f32; 3])> {
    let back = Quat::from_rotation_z(-heading);
    let origin = feet + Vec3::Z;
    parts.into_iter().flat_map(|(a, b, r)| [(a, r), (b, r)]).map(|(q, r)| {
        let l = back * (q - origin);
        (l - Vec3::splat(r), l + Vec3::splat(r))
    }).reduce(|x, y| (x.0.min(y.0), x.1.max(y.1))).map(|(lo, hi)| (lo.into(), hi.into()))
}

impl Foe {
    pub fn rows(&self) -> [[f32; 3]; 4] {
        let h = self.heading;
        [[h.cos(), h.sin(), 0.0], [-h.sin(), h.cos(), 0.0], [0.0, 0.0, 1.0], (self.feet + Vec3::Z).into()]
    }

    pub fn special(&self) -> melee::Special {
        let h = self.heading;
        melee::Special { on_stairs: self.on_stairs, velocity: self.velocity.into(), flags_240: self.flags_240, move_from_idle: self.from_idle,
                         point: self.bones[0].unwrap_or(self.feet + Vec3::Z).into(), forward: [-h.sin(), h.cos(), 0.0] }
    }

    pub fn with_move(self, d: &Data, mv: Option<&MeleeMove>) -> Foe {
        let r = mv.map(|m| &d.table.results[m.result]);
        Foe { task: mv.zip(r).map(|(m, r)| (r.name, m.key.is_some().then_some((m.clip_t, m.step)))), block: r.is_some_and(|r| r.is_a_block), ..self }
    }

    pub(crate) fn point(&self, kind: u32, from: [f32; 3]) -> Option<[f32; 3]> {
        let root = self.feet + Vec3::Z;
        let bone = |i: usize| self.bones[i].unwrap_or(root);
        let p = match kind {
            0 => root,
            1 | 3 if self.frame_box.is_some() => {
                let (lo, hi) = self.frame_box.unwrap_or_default();
                Vec3::from(melee::closest_box_point(self.rows(), lo, hi, from).0)
            }
            1 | 3 => {
                let f = Vec3::from(from);
                let flat = (f - self.feet).with_z(0.0).normalize_or_zero();
                self.feet.with_z(f.z.clamp(self.feet.z, self.feet.z + delirium_data::kv!(207))) + flat * delirium_data::kv!(24)
            }
            2 => bone(0),
            4..=7 => bone(kind as usize - delirium_data::kv!(0 int)),
            _ => return None,
        };
        Some(p.into())
    }
}

const IN: fn(i32) -> bool = |_| true;

#[allow(non_snake_case)]
#[inline]
fn BARE() -> &'static Me { static V: std::sync::OnceLock<Me> = std::sync::OnceLock::new(); let f = || Me { attack: delirium_data::kv!(20), defend: delirium_data::kv!(80), martial: delirium_data::kv!(79), groups: [-1; 7], go: false, moves: -1 }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

pub static TIME_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);

pub fn slow_mo(fx: &melee::SpecialEffect) {
    if let Some(s) = fx.slow_mo {
        TIME_SCALE.store(s.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
}

#[allow(non_snake_case)]
#[inline]
pub fn STANCE_IDS() -> &'static [u32; 9] { static V: std::sync::OnceLock<[u32; 9]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(189 int), delirium_data::kv!(85 int), delirium_data::kv!(310 int), delirium_data::kv!(258 int), delirium_data::kv!(489 int), delirium_data::kv!(95 int), delirium_data::kv!(361 int), delirium_data::kv!(487 int), delirium_data::kv!(188 int)]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

#[allow(non_snake_case)]
#[inline]
pub fn MOVE_MELEE() -> i32 { delirium_data::kv!(404 int) }
#[allow(non_snake_case)]
#[inline]
pub fn MOVE_PLAYER() -> i32 { delirium_data::kv!(459 int) }

pub fn stance_out<K>(old: &mut Vec<(K, f32)>, own: i32, dt: f32) -> f32 {
    let rate = delirium_data::animblend::old_set_rate(MOVE_MELEE(), own, own);
    old.retain_mut(|(_, w)| delirium_data::animblend::old_set_weight(*w, rate, dt).map(|x| *w = x).is_some());
    if old.is_empty() {
        return 1.0;
    }
    let mut list: Vec<f32> = std::iter::once(1.0).chain(old.iter().map(|o| o.1)).collect();
    delirium_data::animblend::normalize_list(&mut list);
    for (o, &w) in old.iter_mut().zip(&list[1..]) {
        o.1 = w;
    }
    list[0]
}

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
    pub flags_240: u32,
    pub on_stairs: bool,
    pub bones: [Option<Vec3>; 5],
    pub lies: u8,
    pub foe_pad: Option<&'a Pad>,
    pub foe_task: Option<FoeTask>,
    pub in_hand: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct FoeTask {
    pub target: TaskTarget,
    pub lock: bool,
    pub given: bool,
    pub block: bool,
    pub struck: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum TaskTarget {
    Me,
    Ped(Foe),
    Nobody,
}

#[derive(Clone, Copy)]
struct Task433 {
    target: Option<usize>,
    lock: bool,
    given: bool,
    block: bool,
    struck: bool,
}

#[derive(Clone, Copy)]
struct Side<'a> {
    foe: Foe,
    special: melee::Special,
    fighter: Fighter,
    groups: [i32; 7],
    player: Option<(Option<&'a Controls>, Option<Task433>)>,
}

fn side_of<R>(s: &[Side], e: usize, tester: usize, depth: usize, k: &mut dyn FnMut(&Opponent) -> R) -> R {
    let me = &s[e];
    let (target, receiving_hit, blocking, just_struck, lock_on, controls) = match me.player {
        Some((pad, Some(t))) => (t.target, t.given, t.block, t.struck, t.lock, pad),
        Some((pad, None)) => (Some(tester), false, false, false, false, pad),
        None => (Some(tester), false, me.foe.block, false, false, None),
    };
    let ask = Ask { fighter: me.fighter, groups: me.groups, loaded: &IN, receiving_hit, blocking, just_struck, target: None, lock_on,
                    impulses: me.player.is_some(), controls, setting: 0, kind: None, idle: None, pos: (me.foe.feet + Vec3::Z).into(),
                    heading: me.foe.heading, opponent_res: 0, special: me.special };
    let lies = me.foe.lies;
    let Some(t) = target else { return k(&Opponent { task: me.foe.task, lies, crouched: false, ask: &ask }) };
    let point = |kind: u32, from: [f32; 3]| s[t].foe.point(kind, from);
    let mut with = |o: Option<&Opponent>| {
        let ask = Ask { target: Some(Target { entity_type: delirium_data::kv!(0 int), point: &point, opponent: o, special: s[t].special }), ..ask };
        k(&Opponent { task: me.foe.task, lies, crouched: false, ask: &ask })
    };
    if depth == 0 { with(None) } else { side_of(s, t, e, depth - 1, &mut |o: &Opponent| with(Some(o))) }
}

fn to_gta(v: Vec3) -> Vec3 {
    v
}

pub fn on_stairs(world: &delirium::collision::Collision, feet: Vec3) -> bool {
    world.ground_stairs(feet + Vec3::Z * 0.5, 1.0).is_some_and(|g| g.1)
}

#[allow(non_snake_case)]
#[inline]
pub fn LOCK_LT() -> u32 { delirium_data::kv!(667 int) }
#[allow(non_snake_case)]
#[inline]
pub fn LOCK_FULL() -> u8 { delirium_data::kv!(446 int) }

pub fn pick_score(from: Vec3, look: Vec3, at: Vec3, threat: f32) -> f32 {
    let d = from.distance(at);
    let h = if d < 1.0 { 1.0 } else { (((at - from) / d).dot(look) - 1.0) * (d - 1.0).min(1.0) + 1.0 };
    delirium_data::kv!(20) / d.max(1.0).powf(delirium_data::kv!(17)) * h + delirium_data::kv!(31) * threat
}

pub fn held(w: &delirium_data::weapons::Weapon) -> Held {
    let kind = melee::WEAPON_TYPES.iter().position(|t| *t == w.name).unwrap_or(0) as u32;
    let flags = [(delirium_data::kv!(2 int), "GUN"), (delirium_data::kv!(104 int), "MELEE_CLUB"), (delirium_data::kv!(81 int), "MELEE_BLADE"), (delirium_data::kv!(55 int), "2HANDED")].iter().filter(|f| w.has(f.1)).fold(0, |b, f| b | 1 << f.0);
    Held { kind, flags, between_shots: w.between_shots as i32 }
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
    pub special_fx: melee::SpecialEffect,
    pub disarm_foe: bool,
    pub drop_weapon: bool,
    pub self_damage: Option<f32>,
    pub from_idle: bool,
    pub velocity: Vec3,
    pos_was: Option<Vec3>,
    pub fighting: bool,
    ended: bool,
    pub request: Option<Request>,
    pub took: bool,
    quit: bool,
}

#[derive(Clone, Copy)]
pub struct Request {
    pub blow: Blow,
    pub from: Option<Entity>,
    pub way: [f32; 3],
    pub locked: bool,
}

impl Melee {
    pub fn step<'c>(&mut self, d: &Data, c: &Ctx, heading: &mut f32, dt: f32, clip: &dyn Fn(&str) -> Option<&'c Animation>, rand: &mut dyn FnMut() -> u32) {
        let dur = &|k: &str| clip(k).map(|a| a.duration);
        let request = self.request.take();
        self.took = false;
        self.started = false;
        self.home = None;
        self.stop_foe_homing = false;
        self.special_fx = Default::default();
        self.disarm_foe = false;
        self.drop_weapon = false;
        self.self_damage = None;
        if dt > 0.0 {
            self.velocity = self.pos_was.map_or(Vec3::ZERO, |w| (c.pos - w) / dt);
            self.pos_was = Some(c.pos);
        }
        if self.on && std::mem::take(&mut self.quit) {
            (self.on, self.mv) = (false, None);
            return;
        }
        let p = &d.player;
        if let Some(me) = c.me {
            self.on = true;
            if request.as_ref().is_some_and(|q| self.take_request(d, c, *heading, q, rand, dur)) {
                return self.play(d, c, heading, dt, clip);
            }
            if !me.go {
                self.from_idle = false;
                return self.play(d, c, heading, dt, clip);
            }
        } else if !self.on && !(c.in_hand && starts(&c.pad, gun_in_hand(c))) {
            return;
        }
        let fighter = match c.me {
            Some(me) => Fighter { player: false, health: c.health, attack: me.attack, defend: me.defend, martial: me.martial, weapon: Some(held(c.weapon)) },
            None => Fighter { player: true, health: c.health, attack: p.attack, defend: p.defend, martial: p.melee_percent, weapon: Some(held(c.weapon)) },
        };
        let foe_player = c.me.is_some() && c.foe_pad.is_some();
        let ai = |f: &Foe| { let m = f.me.unwrap_or((*BARE())); Fighter { player: false, health: f.health, attack: m.attack, defend: m.defend, martial: m.martial, weapon: f.weapon } };
        let foe_fighter = |f: &Foe| if foe_player {
            Fighter { player: true, health: f.health, attack: p.attack, defend: p.defend, martial: p.melee_percent, weapon: f.weapon }
        } else {
            ai(f)
        };
        let foe_groups = |f: &Foe| if foe_player { d.player_groups } else { f.me.map_or([-1; 7], |m| m.groups) };
        let yaw = *heading;
        let special = melee::Special { on_stairs: c.on_stairs, velocity: self.velocity.into(), flags_240: c.flags_240,
                                       move_from_idle: self.on && self.from_idle, point: c.pos.into(), forward: [-yaw.sin(), yaw.cos(), 0.0] };
        let base = |fighter: Fighter| Ask {
            fighter, groups: [-1; 7], loaded: &IN, receiving_hit: false, blocking: false, just_struck: false, target: None, lock_on: false, impulses: false,
            controls: None, setting: 0, kind: None, idle: None, pos: c.pos.into(), heading: *heading, opponent_res: 0, special,
        };
        let mv = self.mv.as_ref();
        let me_foe = Foe { feet: c.pos - Vec3::Z, heading: *heading, health: c.health, lies: c.lies, me: c.me, bones: c.bones,
                           weapon: Some(held(c.weapon)), struck: mv.is_some_and(|m| m.struck), ..Default::default() }.with_move(d, mv);
        let me_task = (c.me.is_none() && self.on).then(|| Task433 { target: c.foe.map(|_| 1), lock: c.lock_on, given: mv.is_some_and(|m| m.given),
                                                                   block: me_foe.block, struck: me_foe.struck });
        let me_side = Side { foe: me_foe, special, fighter, groups: c.me.map_or(d.player_groups, |m| m.groups),
                             player: c.me.is_none().then_some((Some(&c.pad.controls), me_task)) };
        let foe_task = c.foe_task.filter(|_| foe_player).map(|t| Task433 {
            target: match t.target { TaskTarget::Me => Some(0), TaskTarget::Ped(_) => Some(2), TaskTarget::Nobody => None },
            lock: t.lock, given: t.given, block: t.block, struck: t.struck,
        });
        let foe_side = c.foe.map_or(me_side, |f| Side { foe: f, special: f.special(), fighter: foe_fighter(&f), groups: foe_groups(&f),
                                                         player: foe_player.then_some((c.foe_pad.map(|p| &p.controls), foe_task)) });
        let third = match c.foe_task.filter(|_| foe_player).map(|t| t.target) {
            Some(TaskTarget::Ped(f)) => Side { foe: f, special: f.special(), fighter: ai(&f), groups: f.me.map_or([-1; 7], |m| m.groups), player: None },
            _ => me_side,
        };
        let main = match c.me {
            Some(me) => Ask { groups: me.groups, lock_on: true, ..base(fighter) },
            None => Ask { groups: d.player_groups, lock_on: c.lock_on, impulses: true, controls: Some(&c.pad.controls), ..base(fighter) },
        };
        let Some(f) = c.foe else { return self.next_action(d, c, heading, dt, clip, rand, request, dur, main) };
        let point = |k: u32, from: [f32; 3]| f.point(k, from);
        let sides = [me_side, foe_side, third];
        side_of(&sides, 1, 0, d.opponent_depth() - 1, &mut |o: &Opponent| {
            let target = Target { entity_type: delirium_data::kv!(0 int), point: &point, opponent: Some(o), special: f.special() };
            self.next_action(d, c, &mut *heading, dt, clip, &mut *rand, request, dur, Ask { target: Some(target), ..main })
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn next_action<'c>(&mut self, d: &Data, c: &Ctx, heading: &mut f32, dt: f32, clip: &dyn Fn(&str) -> Option<&'c Animation>,
                       rand: &mut dyn FnMut() -> u32, request: Option<Request>, dur: &dyn Fn(&str) -> Option<f32>, ask: Ask) {
        if !self.on {
            let Some(a) = melee::choose(&d.table, &Ask { kind: Some(h("FromOnFoot")), idle: Some(true), ..ask }, &mut *rand) else {
                if c.pad.held(LOCK_LT()) && fists_or_melee(c.weapon) {
                    info!("melee: stance from on foot (LT held)");
                    (self.on, self.timer, self.from_idle, self.mv, self.fighting) = (true, 0.0, false, None, true);
                }
                return;
            };
            info!("melee: {} from on foot", d.table.results.get(d.result_of(a).unwrap_or(usize::MAX)).and_then(|r| d.clip(r)).unwrap_or_default());
            (self.on, self.timer, self.from_idle, self.fighting) = (true, 0.0, false, false);
            play_result(&mut self.mv, d, a, true, dur);
            return;
        }
        let r = self.mv.as_ref().map(|m| &d.table.results[m.result]);
        let block = r.is_some_and(|r| r.is_a_block);
        if c.me.is_none() && self.mv.is_none() && !c.in_hand {
            (self.on, self.mv) = (false, None);
            return;
        }
        let reset = c.lock_on && c.foe.is_some() || c.pad.held(ATTACK());
        self.timer = if reset { 0.0 } else { self.timer + dt * if block { 2.0 } else { 1.0 } };
        if c.me.is_none() && (self.mv.is_none() || block) && self.timer > QUIT() {
            (self.on, self.mv) = (false, None);
            return;
        }
        let ask = Ask { blocking: block, just_struck: self.mv.as_ref().is_some_and(|m| m.struck), ..ask };
        if request.as_ref().is_some_and(|q| self.take_request(d, c, *heading, q, rand, dur)) {
            self.play(d, c, heading, dt, clip);
            if self.ended {
                self.next_subtask(c);
            }
            return;
        }
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
            self.from_idle = self.mv.is_none();
            play_result(&mut self.mv, d, a, c.me.is_none(), dur);
            self.started = true;
        } else {
            self.from_idle = false;
        }
        self.play(d, c, heading, dt, clip);
        if c.me.is_none() && self.ended {
            self.next_subtask(c);
        }
    }

    fn take_request(&mut self, d: &Data, c: &Ctx, heading: f32, q: &Request, rand: &mut dyn FnMut() -> u32, dur: &dyn Fn(&str) -> Option<f32>) -> bool {
        let me = Foe { feet: c.pos - Vec3::Z, heading, health: c.health, me: c.me, bones: c.bones, velocity: self.velocity, flags_240: c.flags_240,
                       from_idle: self.on && self.from_idle, on_stairs: c.on_stairs, ..Default::default() };
        let r = react(d, &me, c.me.is_none(), Some(c.weapon), self.mv.as_ref(), &q.blow, false, [false; 2], Pick::Request { locked: q.locked }, rand);
        let Some(a) = r.action else { return false };
        if let Some(m) = self.mv.as_ref().filter(|m| m.given) {
            let playing = d.table.results[m.result].priority;
            if !r.result.is_some_and(|res| delirium_data::melee_task::request_takes(Some(playing), res.priority)) {
                return false;
            }
        }
        debug!("melee: request taken, {:?}", r.result.and_then(|r| d.clip(r)));
        let idle = self.mv.is_none();
        play_result(&mut self.mv, d, a, c.me.is_none(), dur);
        if let Some(m) = self.mv.as_mut() {
            (m.swing, m.given) = (false, true);
        }
        (self.from_idle, self.started, self.took) = (idle, true, true);
        true
    }

    fn next_subtask(&mut self, c: &Ctx) {
        if !self.fighting && c.lock_on && c.foe.is_some() && fists_or_melee(c.weapon) {
            self.fighting = true;
        }
        if !self.fighting {
            debug!("melee: over with its move");
            (self.on, self.mv) = (false, None);
        }
    }

    pub fn stop_homing(&mut self) {
        if let Some(m) = self.mv.as_mut() {
            m.window = false;
        }
    }

    fn play<'c>(&mut self, d: &Data, c: &Ctx, heading: &mut f32, dt: f32, clip: &dyn Fn(&str) -> Option<&'c Animation>) {
        self.ended = false;
        let Some(m) = self.mv.as_mut() else { return };
        let r = &d.table.results[m.result];
        let paired = |bit| m.key.as_deref().and_then(clip).is_some_and(|a| !a.events.is_empty() && a.count_between(bit, 0.0, 1.0) == 2);
        let (a, b) = (paired(delirium_data::kv!(182 int)), paired(delirium_data::kv!(183 int)));
        if m.clip_t > r.stop_time || r.stop_on_crit_frame && m.events & delirium_data::kv!(54 int) != 0
            || (a || b) && (m.struck || a && m.was[0] || b && m.was[1]) {
            m.window = false;
        }
        if let Some(h) = homing(r, c.foe, c.pos, heading, m.window, m.first, dt) {
            self.home = Some(h);
        }
        m.first = false;
        if m.key.is_some() && !m.special {
            m.special = true;
            let fx = melee::special_result(r.special_result);
            slow_mo(&fx);
            if fx.stop_homing {
                m.window = false;
            }
            (self.special_fx, self.stop_foe_homing) = (fx, fx.stop_opponent_homing);
        }
        self.disarm_foe = r.disarm_opponent && c.foe.is_some() && c.lock_on && (m.key.is_none() || m.events & delirium_data::kv!(182 int) != 0);
        self.drop_weapon = r.drop_weapon && (m.key.is_none() || m.events & delirium_data::kv!(52 int) != 0);
        if r.self_damage > 0.0 && !m.hurt && (m.key.is_none() || m.events & delirium_data::kv!(182 int) != 0) {
            m.hurt = true;
            self.self_damage = Some(r.self_damage);
        }
        if m.key.is_none() {
            (self.mv, self.ended) = (None, true);
            return;
        }
        m.step = dt * m.rate;
        let from = m.phase();
        m.clip_t += m.step;
        if m.clip_t >= m.dur {
            if r.is_loop && m.dur > 0.0 {
                m.clip_t %= m.dur;
            } else {
                (self.mv, self.ended) = (None, true);
                return;
            }
        }
        let ev = m.key.as_deref().and_then(clip).map_or(0, |a| a.advance(from, m.phase(), false, false).0);
        m.events = ev;
        if !r.is_an_idle && !r.is_a_block {
            for (n, bit) in [delirium_data::kv!(182 int), delirium_data::kv!(183 int)].into_iter().enumerate() {
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
            let held = weapon.filter(|_| tag == (*HANDS())[1]).map(|(a, b, big, origin)| (WEAPON_KEY(), a, b, (big, origin)));
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
            if m.hits.len() < HIT_LIST() {
                m.hits.push(id);
            }
            m.struck = true;
            out.push((id, tag, at, r.on_hit_damage));
        }
        out
    }
}

fn points(a: Vec3, b: Vec3, own: f32, tag: u32, r: f32) -> Vec<f32> {
    let len = a.distance(b);
    if !(len > 0.0) || r > own {
        return vec![0.5];
    }
    along(if (*HANDS()).contains(&tag) { HAND_SHORT() } else { 0.0 }, len, len, r)
}

pub fn bound_axis(frame: impl Fn(Vec3) -> Vec3, radius: f32, half: f32, boxed: Option<Vec3>, centroid: f32) -> (Vec3, Vec3, f32) {
    let e = boxed.unwrap_or(Vec3::new(radius, half + radius, radius));
    let axis = if e.x > e.y && e.x > e.z { Vec3::X * e.x } else if e.y > e.x && e.y > e.z { Vec3::Y * e.y } else { Vec3::Z * e.z };
    (frame(-axis), frame(axis), centroid)
}

pub fn frag_capsule(frame: impl Fn(Vec3) -> Vec3, radius: f32, half: f32, boxed: Option<Vec3>) -> (Vec3, Vec3, f32) {
    match boxed {
        None => (frame(Vec3::Y * -half), frame(Vec3::Y * half), radius),
        Some(e) => {
            let mut v = [(e.x, Vec3::X), (e.y, Vec3::Y), (e.z, Vec3::Z)];
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            let ((mid, _), (big, axis)) = (v[1], v[2]);
            (frame(axis * -(big - mid)), frame(axis * (big - mid)), mid)
        }
    }
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
    along(HAND_SHORT(), seg, seg, r)
}

fn along(lo: f32, hi: f32, seg: f32, r: f32) -> Vec<f32> {
    let len = hi - lo;
    let at = |x: f32| x / seg;
    if len <= 2.0 * r {
        return vec![at(lo + len * 0.5)];
    }
    let gap = 2.0 * r * (1.0 - SPACING() * SPACING()).sqrt();
    let n = (((len - 2.0 * r) / gap).ceil() as usize + 1).min(MOST_POINTS());
    (0..n).map(|i| at(lo + r + (len - 2.0 * r) * i as f32 / (n - 1) as f32)).collect()
}

fn closest(p: Vec3, q: Vec3, a: Vec3, b: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, w) = (q - p, b - a, p - a);
    let (aa, ee, f) = (d1.length_squared(), d2.length_squared(), d2.dot(w));
    let (s, t) = if aa <= delirium_data::kv!(70) && ee <= delirium_data::kv!(70) {
        (0.0, 0.0)
    } else if aa <= delirium_data::kv!(70) {
        (0.0, (f / ee).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(w);
        if ee <= delirium_data::kv!(70) {
            ((-c / aa).clamp(0.0, 1.0), 0.0)
        } else {
            let bb = d1.dot(d2);
            let den = aa * ee - bb * bb;
            let s = if den > delirium_data::kv!(70) { ((bb * f - c * ee) / den).clamp(0.0, 1.0) } else { 0.0 };
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
    #[allow(non_snake_case)]
    #[inline]
    fn LINES() -> &'static [(u16, u16, bool, f32); 13] { static V: std::sync::OnceLock<[(u16, u16, bool, f32); 13]> = std::sync::OnceLock::new(); let f = || [
        (delirium_data::kv!(608 int), delirium_data::kv!(609 int), false, delirium_data::kv!(112)), (delirium_data::kv!(609 int), delirium_data::kv!(59 int), false, delirium_data::kv!(146)), (delirium_data::kv!(59 int), delirium_data::kv!(609 int), true, delirium_data::kv!(68)),
        (delirium_data::kv!(606 int), delirium_data::kv!(607 int), false, delirium_data::kv!(112)), (delirium_data::kv!(607 int), delirium_data::kv!(58 int), false, delirium_data::kv!(146)), (delirium_data::kv!(58 int), delirium_data::kv!(607 int), true, delirium_data::kv!(68)),
        (delirium_data::kv!(610 int), delirium_data::kv!(611 int), false, delirium_data::kv!(199)), (delirium_data::kv!(611 int), delirium_data::kv!(612 int), false, delirium_data::kv!(112)), (delirium_data::kv!(612 int), delirium_data::kv!(611 int), true, delirium_data::kv!(146)),
        (delirium_data::kv!(613 int), delirium_data::kv!(614 int), false, delirium_data::kv!(199)), (delirium_data::kv!(614 int), delirium_data::kv!(615 int), false, delirium_data::kv!(112)), (delirium_data::kv!(615 int), delirium_data::kv!(614 int), true, delirium_data::kv!(146)),
        (delirium_data::kv!(209 int), delirium_data::kv!(210 int), true, delirium_data::kv!(125)),
    ]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
    let &(bone, other, before, radius) = (*LINES()).iter().find(|l| l.0 as u32 == tag)?;
    let (a, o) = (at(bone)?, at(other)?);
    Some(capsule_axis(a, if before { a + (a - o).normalize_or_zero() * delirium_data::kv!(142) } else { o }, radius))
}

pub fn capsule_axis(a: Vec3, b: Vec3, radius: f32) -> (Vec3, Vec3, f32) {
    let d = (b - a).normalize_or_zero() * radius;
    (a - d, b + d, a.distance(b) * 0.5 + radius)
}

#[derive(Clone, Copy)]
pub struct Blow {
    pub by: Foe,
    pub player: bool,
    pub res: u32,
    pub clip: Option<(f32, f32)>,
}

#[allow(clippy::too_many_arguments)]
pub fn react<'t>(d: &'t Data, me: &Foe, player: bool, weapon: Option<&delirium_data::weapons::Weapon>, mine: Option<&MeleeMove>, blow: &Blow,
                 gate: bool, tasks: [bool; 2], pick: Pick<'_>, rand: &mut dyn FnMut() -> u32) -> melee::Reaction<'t> {
    let p = &d.player;
    let fighter = |f: &Foe, player: bool| if player {
        Fighter { player, health: f.health, attack: p.attack, defend: p.defend, martial: p.melee_percent, weapon: f.weapon }
    } else {
        let m = f.me.unwrap_or((*BARE()));
        Fighter { player, health: f.health, attack: m.attack, defend: m.defend, martial: m.martial, weapon: f.weapon }
    };
    let base = |f: &Foe, player: bool| Ask {
        fighter: fighter(f, player), groups: if player { d.player_groups } else { f.me.map_or([-1; 7], |m| m.groups) }, loaded: &IN, receiving_hit: false, blocking: false, just_struck: false,
        target: None, lock_on: false, impulses: false, controls: None, setting: 0, kind: None, idle: None, pos: (f.feet + Vec3::Z).into(),
        heading: f.heading, opponent_res: 0, special: f.special(),
    };
    let by_ask = base(&blow.by, blow.player);
    let task = match pick {
        Pick::Request { .. } => blow.by.task,
        _ => (blow.res != 0).then_some((blow.res, blow.clip)),
    };
    let opponent = Opponent { task, lies: blow.by.lies, crouched: false, ask: &by_ask };
    let point = |k: u32, from: [f32; 3]| blow.by.point(k, from);
    let mut ask = base(me, player);
    ask.fighter.weapon = weapon.map(held);
    ask.blocking = mine.is_some_and(|m| d.table.results[m.result].is_a_block);
    ask.target = Some(Target { entity_type: delirium_data::kv!(0 int), point: &point, opponent: Some(&opponent), special: blow.by.special() });
    ask.opponent_res = blow.res;
    let ask = match pick {
        Pick::Dispatcher => return melee::hit_reaction(&d.table, &ask, gate, tasks, rand),
        Pick::Forced { pad } => Ask { kind: Some(h("HitReaction")), blocking: false, impulses: player, controls: pad.filter(|_| player), ..ask },
        Pick::Request { locked } => Ask { kind: Some(h("HitReaction")), idle: Some(true), receiving_hit: true,
                                          just_struck: mine.is_some_and(|m| m.struck), lock_on: locked, ..ask },
    };
    let action = melee::choose(&d.table, &ask, rand);
    melee::Reaction { action, result: action.and_then(|a| d.result_of(a)).map(|r| &d.table.results[r]), ragdoll: false }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick<'p> {
    Dispatcher,
    Forced { pad: Option<&'p Controls> },
    Request { locked: bool },
}

