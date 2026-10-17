use crate::actiontable::{Action, ActionTable, Impulse, ResultDef};
use crate::controls::{index, pick as pair_pick, Controls, IoValue, AXIS_PAIRS};

fn back(at: u8, i: u8) -> usize {
    (if i <= at { at - i } else { at.wrapping_sub(i.min(0x3F)).wrapping_add(0x40) }) as usize
}

fn since(v: &IoValue, h: &[(u8, u32)], ms: u32) -> u32 {
    h[v.history_at as usize].1.saturating_sub(ms)
}

fn within(v: &IoValue, value: u8, lo: u8, hi: u8) -> bool {
    let x = value ^ v.invert;
    if lo > hi { hi <= x && x <= lo } else { lo <= x && x <= hi }
}

fn hist(v: &IoValue) -> &[(u8, u32)] {
    v.history.as_deref().expect("melee controls keep a history")
}

pub fn held(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    all_since(v, ms, |x| within(v, x, lo, hi))
}

pub fn held_up(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    all_since(v, ms, |x| !within(v, x, lo, hi))
}

fn all_since(v: &IoValue, ms: u32, ok: impl Fn(u8) -> bool) -> bool {
    let h = hist(v);
    let t = since(v, h, ms);
    for i in 0..0x40 {
        let e = h[back(v.history_at, i)];
        if e.1 < t {
            return true;
        }
        if !ok(e.0) {
            return false;
        }
    }
    true
}

pub fn pressed_and_held(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    let h = hist(v);
    within(v, h[v.history_at as usize].0, lo, hi) && first_since(v, ms, |x| !within(v, x, lo, hi))
}

pub fn released_and_held(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    let h = hist(v);
    !within(v, h[v.history_at as usize].0, lo, hi) && first_since(v, ms, |x| within(v, x, lo, hi))
}

fn first_since(v: &IoValue, ms: u32, hit: impl Fn(u8) -> bool) -> bool {
    let h = hist(v);
    let t = since(v, h, ms);
    for i in 1..0x40 {
        let e = h[back(v.history_at, i)];
        if e.1 < t {
            return false;
        }
        if hit(e.0) {
            return true;
        }
    }
    false
}

pub fn pressed(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    edge_since(v, ms, |before, after| !within(v, before, lo, hi) && within(v, after, lo, hi))
}

pub fn released(v: &IoValue, ms: u32, lo: u8, hi: u8) -> bool {
    edge_since(v, ms, |before, after| within(v, before, lo, hi) && !within(v, after, lo, hi))
}

fn edge_since(v: &IoValue, ms: u32, edge: impl Fn(u8, u8) -> bool) -> bool {
    let h = hist(v);
    let t = since(v, h, ms);
    for i in 1..0x40 {
        let e = h[back(v.history_at, i)];
        if e.1 < t {
            return false;
        }
        if edge(e.0, h[back(v.history_at, i - 1)].0) {
            return true;
        }
    }
    false
}

fn byte(x: f32) -> u8 {
    x as i64 as u8
}

impl Impulse {
    pub fn test(&self, c: &Controls, setting: u32) -> bool {
        if !self.check {
            return true;
        }
        let v = |off: u32| &c.values[index(off)];
        let axis = |n: usize| v(pair_pick(AXIS_PAIRS[n], v(AXIS_PAIRS[n].0)));
        let (a, b) = match self.impulse {
            1 => (v(0xBDC), None),
            2 => (v(0xBE8), None),
            3 => (v(0xBF4), None),
            4 => (v(0xD14), None),
            5 => (v(0xC00), None),
            6 => (v(0xC0C), None),
            7 => (axis(0), None),
            8 => (axis(1), None),
            9 => (axis(0), Some(axis(1))),
            _ => return false,
        };
        let ms = (self.interval * 1000.0) as i64 as u32;
        let [r0, r1] = self.range;
        let (lo, hi) = match self.impulse {
            7 | 8 => (byte((r0 + 1.0) * 0.5 * 255.0), byte((r1 + 1.0) * 0.5 * 255.0)),
            9 => (byte(r0.abs() * 127.0), byte(r1.abs() * 127.0)),
            3 if setting == 0 => (byte(r0 * 255.0), byte(r1 * 255.0)),
            _ => (0x80, 0xFF),
        };
        let q: fn(&IoValue, u32, u8, u8) -> bool = match self.state {
            0 => return true,
            1 => held,
            2 => held_up,
            3 => pressed_and_held,
            4 => released_and_held,
            5 => pressed,
            6 => released,
            _ => return false,
        };
        match b {
            None => q(a, ms, lo, hi),
            Some(b) => {
                let (p, n) = ((lo.wrapping_add(0x80), hi.wrapping_add(0x80)), (0x7Fu8.wrapping_sub(hi), 0x7Fu8.wrapping_sub(lo)));
                q(a, ms, p.0, p.1) || q(a, ms, n.0, n.1) || q(b, ms, p.0, p.1) || q(b, ms, n.0, n.1)
            }
        }
    }
}

pub fn impulses_hold(a: &Action, c: Option<&Controls>, setting: u32) -> bool {
    let Some(c) = c else { return false };
    let [i0, i1] = &a.impulses;
    match a.impulse_op {
        0 => i0.test(c, setting) || i1.test(c, setting),
        1 => i0.test(c, setting) && i1.test(c, setting),
        2 => i1.test(c, setting) != i0.test(c, setting),
        _ => false,
    }
}

pub const WEAPON_TYPES: [&str; 58] = [
    "UNARMED", "BASEBALLBAT", "POOLCUE", "KNIFE", "GRENADE", "MOLOTOV", "ROCKET", "PISTOL", "UNUSED0", "DEAGLE", "SHOTGUN",
    "BARETTA", "MICRO_UZI", "MP5", "AK47", "M4", "SNIPERRIFLE", "M40A1", "RLAUNCHER", "FTHROWER", "MINIGUN", "EPISODIC_1",
    "EPISODIC_2", "EPISODIC_3", "EPISODIC_4", "EPISODIC_5", "EPISODIC_6", "EPISODIC_7", "EPISODIC_8", "EPISODIC_9",
    "EPISODIC_10", "EPISODIC_11", "EPISODIC_12", "EPISODIC_13", "EPISODIC_14", "EPISODIC_15", "EPISODIC_16", "EPISODIC_17",
    "EPISODIC_18", "EPISODIC_19", "EPISODIC_20", "EPISODIC_21", "EPISODIC_22", "EPISODIC_23", "EPISODIC_24", "CAMERA",
    "OBJECT", "WEAPONTYPE_LAST_WEAPONTYPE", "ARMOUR", "RAMMEDBYCAR", "RUNOVERBYCAR", "EXPLOSION", "UZI_DRIVEBY",
    "DROWNING", "FALL", "UNIDENTIFIED", "ANYMELEE", "ANYWEAPON",
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held {
    pub kind: u32,
    pub flags: u32,
    pub between_shots: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fighter {
    pub player: bool,
    pub health: f32,
    pub attack: f32,
    pub defend: f32,
    pub martial: f32,
    pub weapon: Option<Held>,
}

pub fn who_takes(who: u32, player: bool) -> bool {
    match who {
        0 => true,
        1 => player,
        2 => !player,
        _ => false,
    }
}

pub fn ped_takes(a: &Action, f: &Fighter, receiving_hit: bool, blocking: bool, just_struck: bool) -> bool {
    if !who_takes(a.ai_player_filter, f.player)
        || a.flags[1] && !receiving_hit
        || a.flags[2] && !blocking
        || a.flags[3] && !just_struck
    {
        return false;
    }
    let stats = [f.health, f.attack, f.defend, f.martial];
    if stats.iter().zip(&a.ranges).any(|(x, r)| *x < r[0] || *x > r[1]) {
        return false;
    }
    let h = |s: &str| crate::map::hash_bytes(s.as_bytes(), 0);
    let class = a.weapon_class;
    if class == 0 || class == crate::actiontable::none() {
        return false;
    }
    if class == h("any") {
        return true;
    }
    if class == h("unarmed") {
        return f.weapon.map_or(true, |w| w.kind == 0);
    }
    let Some(w) = f.weapon else { return false };
    let bit = |n: u32| w.flags >> n & 1 != 0;
    let one_handed = bit(5) && !bit(12);
    let mg = bit(5) && w.between_shots < 200;
    match class {
        c if c == h("object") => w.kind == 0x2E,
        c if c == h("knife") => bit(10),
        c if c == h("club") => bit(9),
        c if c == h("oneHandedGun") => one_handed,
        c if c == h("oneHandedMG") => one_handed && mg,
        c if c == h("pistol") => one_handed && !mg,
        c if c == h("rifle") => bit(5) && bit(12),
        c => w.kind == WEAPON_TYPES.iter().position(|t| h(t) == c).map_or(0x37, |i| i as u32),
    }
}

#[derive(Clone, Copy)]
pub struct Target<'a> {
    pub entity_type: u32,
    pub point: &'a dyn Fn(u32, [f32; 3]) -> Option<[f32; 3]>,
    pub opponent: Option<&'a Opponent<'a>>,
}

#[derive(Clone, Copy)]
pub struct Opponent<'a> {
    pub task: Option<(u32, Option<(f32, f32)>)>,
    pub lies: u8,
    pub crouched: bool,
    pub ask: &'a Ask<'a>,
}

#[derive(Clone, Copy)]
pub struct Ask<'a> {
    pub fighter: Fighter,
    pub groups: [i32; 7],
    pub receiving_hit: bool,
    pub blocking: bool,
    pub just_struck: bool,
    pub target: Option<Target<'a>>,
    pub lock_on: bool,
    pub impulses: bool,
    pub controls: Option<&'a Controls>,
    pub setting: u32,
    pub kind: Option<u32>,
    pub idle: Option<bool>,
    pub pos: [f32; 3],
    pub heading: f32,
    pub opponent_res: u32,
    pub special: &'a dyn Fn(&Action) -> bool,
}

pub fn anim_in(a: &Action, results: &[ResultDef], groups: &[i32; 7]) -> bool {
    let h = |s: &str| crate::map::hash_bytes(s.as_bytes(), 0);
    if a.move_ == 0 {
        return true;
    }
    let Some(r) = results.iter().rev().find(|r| r.name == a.move_) else { return true };
    if r.anim_group == -1 {
        return true;
    }
    if a.weapon_class != crate::actiontable::none() && a.weapon_class != h("unarmed") || a.kind == h("HitReaction") {
        return true;
    }
    groups.contains(&r.anim_group)
}

pub fn target_fits(kind: u32, t: Option<&Target>) -> bool {
    let Some(t) = t else { return false };
    match kind {
        0 | 1 => true,
        2..=7 => t.entity_type == 3,
        8..=15 => t.entity_type == 2,
        _ => false,
    }
}

pub fn angle_in(x: f32, r: [f32; 2]) -> bool {
    let x = crate::actiontable::wrap(x);
    if r[0] > r[1] {
        x >= r[0] || x <= r[1]
    } else {
        !(x < r[0] || x > r[1])
    }
}

pub fn opponent_holds(a: &Action, t: &ActionTable, ask: &Ask) -> bool {
    let o = ask.target.and_then(|t| t.opponent).filter(|_| ask.target.is_some_and(|t| t.entity_type == 3));
    if a.opponent_res != 0 {
        let Some(o) = o else { return false };
        let cur = if ask.opponent_res != 0 {
            ask.opponent_res
        } else {
            match o.task {
                Some((r, _)) => r,
                None if a.opponent_res_not => 0,
                None => return false,
            }
        };
        if ask.opponent_res != 0 || o.task.is_some() {
            if (cur == a.opponent_res) == a.opponent_res_not {
                return false;
            }
        }
    }
    if a.check_opponent_anim_time {
        let Some((t, step)) = o.and_then(|o| o.task).and_then(|t| t.1) else { return false };
        let [lo, hi] = a.opponent_anim_time;
        let d = if t >= lo { t - hi } else { lo - (step + t) };
        if d >= 0.0 {
            return false;
        }
    }
    if a.opponent_pose != 1 {
        let Some(o) = o else { return false };
        let ok = match a.opponent_pose {
            2 => o.lies == 4,
            3 => o.crouched,
            4 => o.lies <= 3,
            5..=8 => o.lies as u32 == a.opponent_pose - 5,
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    if a.opponents_action_to_test == 0 {
        return true;
    }
    let Some(o) = o else { return false };
    let Some(b) = t.actions.iter().find(|b| b.name == a.opponents_action_to_test) else { return false };
    action_test(b, t, &Ask { kind: None, idle: None, opponent_res: 0, ..*o.ask })
}

pub fn rel_pos_holds(a: &Action, ask: &Ask) -> bool {
    if !a.check_rel_pos {
        return true;
    }
    let Some(t) = &ask.target else { return false };
    let Some(p) = (t.point)(a.rel_point, ask.pos) else { return false };
    let d = [p[0] - ask.pos[0], p[1] - ask.pos[1], p[2] - ask.pos[2]];
    let out = |x: f32, r: [f32; 2]| x < r[0] || x > r[1];
    if out(d[2], a.height_range) || out(d[1].mul_add(d[1], d[0] * d[0]).sqrt(), a.distance_range) {
        return false;
    }
    let w = crate::actiontable::wrap;
    let dir = w((-d[0] as f64).atan2(d[1] as f64) as f32);
    angle_in(dir - w(ask.heading), a.angle_range)
}

pub fn action_test(a: &Action, t: &ActionTable, ask: &Ask) -> bool {
    if ask.kind.is_some_and(|k| a.kind != k) || ask.idle.is_some_and(|i| a.available_from_idle != i) {
        return false;
    }
    if !anim_in(a, &t.results, &ask.groups)
        || !ped_takes(a, &ask.fighter, ask.receiving_hit, ask.blocking, ask.just_struck)
        || ask.impulses && !impulses_hold(a, ask.controls, ask.setting)
    {
        return false;
    }
    let player = ask.fighter.player;
    let needs = who_takes(a.require_target, player);
    if needs && ask.target.is_none() || who_takes(a.require_lock_on, player) && !ask.lock_on {
        return false;
    }
    if (needs || ask.target.is_some())
        && !(target_fits(a.required_target_type, ask.target.as_ref()) && opponent_holds(a, t, ask) && rel_pos_holds(a, ask))
    {
        return false;
    }
    if !(ask.special)(a) {
        return false;
    }
    if a.additional_action_test == 0 {
        return true;
    }
    let Some(b) = t.actions.iter().find(|b| b.name == a.additional_action_test) else { return false };
    action_test(b, t, &Ask { kind: None, idle: None, impulses: ask.controls.is_some(), ..*ask })
}

pub fn crt_qsort<T>(v: &mut [T], cmp: impl Fn(&T, &T) -> i32) {
    if v.len() < 2 {
        return;
    }
    let c = |v: &[T], a: isize, b: isize| cmp(&v[a as usize], &v[b as usize]);
    let mut stack = vec![];
    let (mut lo, mut hi) = (0isize, v.len() as isize - 1);
    loop {
        let size = hi - lo + 1;
        if size <= 8 {
            let mut h = hi;
            while h > lo {
                let mut max = lo;
                for p in lo + 1..=h {
                    if c(v, p, max) > 0 {
                        max = p;
                    }
                }
                v.swap(max as usize, h as usize);
                h -= 1;
            }
        } else {
            let mut mid = lo + size / 2;
            for (a, b) in [(lo, mid), (lo, hi), (mid, hi)] {
                if c(v, a, b) > 0 {
                    v.swap(a as usize, b as usize);
                }
            }
            let (mut lg, mut hg) = (lo, hi);
            loop {
                if mid > lg {
                    loop {
                        lg += 1;
                        if !(lg < mid && c(v, lg, mid) <= 0) {
                            break;
                        }
                    }
                }
                if mid <= lg {
                    loop {
                        lg += 1;
                        if !(lg <= hi && c(v, lg, mid) <= 0) {
                            break;
                        }
                    }
                }
                loop {
                    hg -= 1;
                    if !(hg > mid && c(v, hg, mid) > 0) {
                        break;
                    }
                }
                if hg < lg {
                    break;
                }
                v.swap(lg as usize, hg as usize);
                if mid == hg {
                    mid = lg;
                }
            }
            hg += 1;
            if mid < hg {
                loop {
                    hg -= 1;
                    if !(hg > mid && c(v, hg, mid) == 0) {
                        break;
                    }
                }
            }
            if mid >= hg {
                loop {
                    hg -= 1;
                    if !(hg > lo && c(v, hg, mid) == 0) {
                        break;
                    }
                }
            }
            if hg - lo >= hi - lg {
                if lo < hg {
                    stack.push((lo, hg));
                }
                if lg < hi {
                    lo = lg;
                    continue;
                }
            } else {
                if lg < hi {
                    stack.push((lg, hi));
                }
                if lo < hg {
                    hi = hg;
                    continue;
                }
            }
        }
        match stack.pop() {
            Some(p) => (lo, hi) = p,
            None => return,
        }
    }
}

pub fn pick<'t>(mut hits: Vec<&'t Action>, rand: impl FnOnce() -> u32) -> Option<&'t Action> {
    if hits.len() < 2 {
        return hits.first().copied();
    }
    crt_qsort(&mut hits, |a, b| b.priority.wrapping_sub(a.priority));
    let top = hits[0].priority as u32;
    hits.retain(|a| a.priority as u32 >= top);
    if hits.len() == 1 {
        return Some(hits[0]);
    }
    let sum = hits.iter().fold(0.0f32, |s, a| a.bias + s);
    let r = rand() as f32 * f32::from_bits(0x3800_0100) * sum;
    let mut i = 0;
    if r > 0.0 {
        let mut acc = 0.0f32;
        loop {
            acc = hits[i].bias + acc;
            if !(acc < r) || i == hits.len() - 1 {
                break;
            }
            i += 1;
        }
    }
    Some(hits[i])
}

pub fn choose<'t>(t: &'t ActionTable, ask: &Ask, rand: impl FnOnce() -> u32) -> Option<&'t Action> {
    let mut hits: Vec<&Action> = t.actions.iter().filter(|a| action_test(a, t, ask)).collect();
    if hits.is_empty() {
        let bare = Ask { target: None, lock_on: false, ..*ask };
        hits = t.actions.iter().filter(|a| action_test(a, t, &bare)).collect();
    }
    pick(hits, rand)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Reaction<'t> {
    pub action: Option<&'t Action>,
    pub result: Option<&'t ResultDef>,
    pub ragdoll: bool,
}

pub fn hit_reaction<'t>(t: &'t ActionTable, ask: &Ask, gate: bool, tasks: [bool; 2], rand: impl FnOnce() -> u32) -> Reaction<'t> {
    let ask = Ask { kind: Some(crate::map::hash_bytes(b"HitReaction", 0)), idle: Some(true), receiving_hit: true, just_struck: false, ..*ask };
    let action = choose(t, &ask, rand);
    let result = action.filter(|a| a.move_ != 0).and_then(|a| t.results.iter().rfind(|r| r.name == a.move_));
    let special = result.is_some_and(|r| r.special_result != crate::actiontable::none());
    let low = ask.fighter.health < 125.0;
    let ragdoll = gate
        && (action.is_some_and(|a| a.prefer_ragdoll) || if tasks[0] && !tasks[1] { low } else { !special || low });
    Reaction { action, result, ragdoll }
}

