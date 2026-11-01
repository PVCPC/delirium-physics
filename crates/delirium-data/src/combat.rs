use crate::aiweapon::{self, Adjust};
use crate::rng::Rand;
use crate::weaponnatives::Weapons;

const MADE: u32 = 0xC2000;
const ARREST: u32 = 1 << 19;
const COIN: u32 = 1 << 20;
const MEMORY: u32 = 3 << 24;
const ROLLED: u32 = 1 << 26;
const FIGHT: u32 = 1 << 17;
const FLEE: u32 = 1 << 28;
const REFIRE: u32 = 1 << 21;
const MAKER_CLEARS: u32 = 1 << 22;
const BRAVE_SCALE: f32 = 0.66;

pub const NONE: i32 = 200;
pub const FIRE: i32 = 1901;
pub const RETREAT: i32 = 1904;
pub const CHARGE: i32 = 1905;
const REPOSITION: i32 = 1931;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sub {
    Fire,
    Retreat,
    Charge,
    Swap,
}

impl Sub {
    pub fn code(self) -> i32 {
        match self {
            Sub::Fire => FIRE,
            Sub::Retreat => RETREAT,
            Sub::Charge => CHARGE,
            Sub::Swap => aiweapon::SWAP,
        }
    }
}

pub type Ask<'a> = dyn FnMut(i32, &mut Rand) -> i32 + 'a;

const ADVANCE: i32 = 1902;
const SEEK_COVER: i32 = 1903;
const INVESTIGATE: i32 = 1906;
const EXECUTE: i32 = 1911;
const SKIP: [i32; 4] = [714, 281, 742, 319];
const HOLD_RANGE: f32 = 30.0;
const ALERT: i8 = 70;

fn stand_in(code: i32) -> i32 {
    match code {
        NONE | FIRE | RETREAT | CHARGE | aiweapon::SWAP => code,
        _ => FIRE,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Facts {
    pub dt: f32,
    pub target: bool,
    pub health: f32,
    pub bravery: u8,
    pub target_armed: u8,
    pub target_cop: bool,
    pub wanted_cop_near: bool,
    pub gang: bool,
    pub hates_target: bool,
    pub cover: bool,
    pub target_in_vehicle: bool,
    pub armed: bool,
    pub target_player: bool,
    pub target_friend: bool,
    pub target_mission: bool,
    pub target_arrested: bool,
    pub fights: u8,
    pub group6: bool,
    pub target_hurt: bool,
    pub for_injured: bool,
    pub population: bool,
    pub gun: bool,
    pub injured_reaction: u8,
    pub caution: u8,
    pub navigation_style: u8,
    pub player_inside: bool,
    pub no_cover: bool,
    pub no_duck: bool,
    pub standing_style: u8,
    pub alertness: i8,
    pub target_dist: f32,
    pub line_state: u8,
    pub mission: bool,
    pub cop: bool,
    pub guard: bool,
}

pub fn firing_factor(factor: f32, dist2: f32, hidden: bool) -> f32 {
    if dist2 < 16.0 { factor * 3.0 } else if hidden { 0.2 } else { factor }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunMake {
    pub kind: u8,
    pub aim: f32,
    pub flags: u32,
    pub at_place: bool,
}

pub const FIRE_COVER: u8 = 0x80;
pub const FIRE_FITS: u8 = 0x40;
pub const FIRE_ARREST: u8 = 0x20;
pub const FIRE_AIM: u8 = 0x10;

pub const GUN_FIRES: u32 = 4;

pub fn make_gun(arrest: bool, state: u8, seen: bool, target_player: bool, react: bool, bits: &mut u8, fire_rate: u8, rng: &mut Rand) -> GunMake {
    if *bits & FIRE_COVER == 0 && state == 3 && !target_player {
        *bits |= FIRE_AIM;
    }
    let unseen = state == 0 || state == 2;
    let mut g = if arrest {
        *bits |= FIRE_ARREST;
        GunMake { kind: 2, aim: -1.0, flags: 0x200_0001, at_place: true }
    } else if unseen || *bits & (FIRE_COVER | FIRE_AIM) != 0 {
        let fires = unseen && seen || state == 3;
        GunMake { kind: 2, aim: 2.0, flags: if fires { 0x200_0005 } else { 0x200_0001 }, at_place: true }
    } else {
        let (kind, aim) = gun_kind(fire_rate, rng);
        GunMake { kind, aim, flags: 0x200_0008, at_place: false }
    };
    if react {
        g.flags |= 8;
    }
    g
}

pub fn gun_stale(flags: u32, state: u8, seen: bool, bits: u8) -> bool {
    let should = (state == 2 || state == 0) && seen || state == 3;
    let fires = flags & GUN_FIRES != 0;
    if should { !fires && bits & (FIRE_COVER | FIRE_AIM) == 0 } else { fires }
}

pub const RETARGET_FIRST: f32 = 2.0;
pub fn retarget_wait(rng: &mut Rand) -> f32 {
    rng.float_in_range(2.0, 5.0)
}

pub fn retarget_gun(bits: u8, rng: &mut Rand) -> GunMake {
    let kind = if bits & (FIRE_COVER | FIRE_ARREST) != 0 { 2 } else { 7 };
    GunMake { kind, aim: rng.float_in_range(0.5, 0.75), flags: 0, at_place: false }
}

pub const RETARGET_CONE: f32 = std::f32::consts::FRAC_PI_4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot<T> {
    pub ped: T,
    pub at: [f32; 3],
    pub state: u8,
}

#[derive(Clone, Debug)]
pub struct Targeting<T> {
    pub slots: [Option<Slot<T>>; 8],
}

impl<T> Default for Targeting<T> {
    fn default() -> Self {
        Targeting { slots: std::array::from_fn(|_| None) }
    }
}

impl<T: Copy + PartialEq> Targeting<T> {
    pub fn add(&mut self, ped: T, at: [f32; 3], state: u8) {
        if self.slots.iter().flatten().any(|s| s.ped == ped) {
            return;
        }
        if let Some(free) = self.slots.iter_mut().find(|s| s.is_none()) {
            *free = Some(Slot { ped, at, state });
        }
    }

    pub fn remove(&mut self, ped: T) {
        for s in &mut self.slots {
            if s.is_some_and(|s| s.ped == ped) {
                *s = None;
            }
        }
    }

    pub fn state(&self, ped: T) -> u8 {
        self.slots.iter().flatten().find(|s| s.ped == ped).map_or(0, |s| s.state)
    }

    pub fn best_in_cone(&self, me: [f32; 3], forward: [f32; 2], dir: f32, cone: f32, exclude: Option<(T, [f32; 3])>, skip_blocked: bool, left: bool) -> Option<T> {
        use crate::actiontable::wrap;
        use crate::lift::heading;
        let from = match exclude {
            Some((_, at)) => heading(at[0] - me[0], at[1] - me[1]),
            None => heading(forward[0], forward[1]),
        };
        let mut best = (f32::from_bits(0xC974_23F0), None);
        for s in self.slots.iter().flatten() {
            if exclude.is_some_and(|x| x.0 == s.ped) || skip_blocked && s.state == 2 {
                continue;
            }
            let h = heading(s.at[0] - me[0], s.at[1] - me[1]);
            if wrap(h - dir).abs() > cone {
                continue;
            }
            let d = wrap(h - from);
            let score = if !left {
                if d < 0.0 { d.abs() - 1000.0 } else { -d }
            } else if d > 0.0 {
                d.abs() - 1000.0
            } else {
                -d.abs()
            };
            if best.0 < score {
                best = (score, Some(s.ped));
            }
        }
        best.1
    }
}

#[derive(Clone, Debug)]
pub struct Arms {
    pub w: Weapons,
    pub rows: &'static aiweapon::Rows,
    pub me: aiweapon::Me,
}

#[derive(Clone, Debug)]
pub struct Combat {
    pub arms: Option<Arms>,
    flags: u32,
    life: f32,
    cap: f32,
    rethink: f32,
    firing: f32,
    t70: f32,
    t74: f32,
    pub sub: Option<Sub>,
    then: (u8, i32),
    advance: (bool, bool),
    pub covers: u8,
    fire_time: Option<f32>,
    pub remade: bool,
    pub made: Option<GunMake>,
    pub level: i8,
    talk: f32,
    abort: Option<(u8, Refusal)>,
    pub fight_run: bool,
}

pub fn armed_state(damage_type: &str) -> u8 {
    match damage_type {
        "MELEE" => 1,
        "BULLET" | "EXPLOSIVE" | "FIRE" => 2,
        _ => 0,
    }
}

fn chance(level: i32, armed: u8) -> f32 {
    match (level, armed) {
        (4, _) => 1.0,
        (0, _) | (_, 2) => 0.0,
        (1, _) => 0.5,
        _ => 1.0,
    }
}

fn gun_kind(fire_rate: u8, rng: &mut Rand) -> (u8, f32) {
    let r = rng.next() as i32 as f32 * f32::from_bits(0x3800_0100);
    let drawn = r.mul_add(f32::from_bits(0x3E4C_CCD0), f32::from_bits(0x3F66_6666)) * 2.0;
    match fire_rate {
        0 => (4, 0.5),
        1 => (5, 2.0),
        2 => (7, 1.0),
        _ => (5, drawn),
    }
}

fn tick(t: &mut f32, dt: f32) -> bool {
    *t > 0.0 && {
        *t -= dt;
        *t <= 0.0
    }
}

impl Combat {
    pub fn new(life: f32, rng: &mut Rand) -> Combat {
        let coin = rng.float_in_range(0.0, 1.0) < 0.5;
        Combat { flags: MADE | if coin { COIN } else { 0 }, ..Combat::blank(life) }
    }

    fn blank(life: f32) -> Combat {
        Combat { arms: None, flags: MADE, life, cap: 0.0, rethink: 0.0, firing: 0.0, t70: 0.0, t74: 0.0, sub: None, then: (0, NONE), advance: (false, false),
                 covers: 0, fire_time: None, remade: false, made: None, abort: None, fight_run: false, level: -1, talk: 0.0 }
    }

    pub fn fleeing(life: f32, rng: &mut Rand) -> Combat {
        let mut c = Combat::new(life, rng);
        c.flags |= FLEE;
        c
    }

    pub fn courage_of(flags: &mut u32, f: &Facts, rng: &mut Rand) -> bool {
        let mut c = Combat { flags: *flags, ..Combat::blank(0.0) };
        let r = c.courage(f, rng);
        *flags = c.flags;
        r
    }

    pub fn fighting(&mut self) {
        self.flags |= FIGHT;
    }

    pub fn screams(&self) -> bool {
        self.flags & 1 << 13 != 0
    }

    pub fn retreating(&self) -> bool {
        self.sub == Some(Sub::Retreat)
    }

    fn situation(f: &Facts, first: bool, ask: &mut Ask, rng: &mut Rand) -> i32 {
        let s = if f.target_in_vehicle { 109 } else { 108 };
        ask(if first { 107 } else { s }, rng)
    }

    fn indoors(f: &Facts) -> bool {
        f.population && f.player_inside
    }

    fn adjust(&mut self, f: &Facts, code: i32, rng: &mut Rand) -> i32 {
        self.weapon_check(f);
        if let Some(a) = self.arms.as_mut() {
            match aiweapon::adjust(&mut a.w, a.rows, a.me.in_vehicle) {
                Adjust::Swap => return aiweapon::SWAP,
                Adjust::Discard => {
                    aiweapon::discard(&mut a.w, a.rows);
                }
                Adjust::On => {}
            }
        }
        if self.flags & FLEE != 0 {
            return if code == FIRE && self.sub == Some(Sub::Retreat) && self.then == (4, FIRE) { FIRE } else { RETREAT };
        }
        if f.target_hurt && (f.gang && f.population || f.injured_reaction == 2) && f.gun {
            return EXECUTE;
        }
        if code == FIRE && self.flags & ARREST == 0 && self.armed(f) && self.sub.map(Sub::code) != Some(SEEK_COVER) && !f.no_cover && !f.cover {
            let chance = if Combat::indoors(f) { 0.5 } else { 1.0 };
            let _cover = rng.float_in_range(0.0, 1.0) < chance;
        }
        if code == SEEK_COVER && (f.no_cover || f.navigation_style == 1) {
            return NONE;
        }
        code
    }

    fn books(&mut self, cover: bool, bit3: bool, rng: &mut Rand) {
        if !cover {
            self.covers = 0;
            return;
        }
        self.covers = self.covers.wrapping_add(1);
        if bit3 {
            self.flags |= 1 << 15;
        }
        self.t74 = rng.float_in_range(0.8, 1.2) * 4.0;
    }

    fn hold(&self, f: &Facts) -> f32 {
        if self.flags & ARREST != 0 {
            return 0.0;
        }
        match f.caution {
            1 => 50.0,
            2 => 25.0,
            _ => 999.0,
        }
    }

    fn pick(&mut self, f: &Facts, finished: bool, rethink: bool, ask: &mut Ask, rng: &mut Rand) -> Option<Sub> {
        let running = self.sub.map(Sub::code);
        if finished && running == Some(INVESTIGATE) {
            return None;
        }
        let mut code = NONE;
        if let Some(r) = running.filter(|r| !SKIP.contains(r)) {
            if r == ADVANCE {
                let (cover, bit3) = self.advance;
                self.books(cover, bit3, rng);
            }
            code = match self.then {
                (3, _) => self.adjust(f, r, rng),
                (4, next) => self.adjust(f, next, rng),
                (5, _) => {
                    let s = Combat::situation(f, true, ask, rng);
                    self.adjust(f, s, rng)
                }
                _ => NONE,
            };
        }
        if f.target && (!f.target_hurt || f.for_injured) && code == NONE {
            let s = Combat::situation(f, false, ask, rng);
            code = self.adjust(f, s, rng);
        }
        if rethink && Some(code) == running {
            return None;
        }
        if rethink && code == ADVANCE && running == Some(FIRE) && f.line_state != 2 && f.target_dist < HOLD_RANGE && self.firing < self.hold(f) {
            return None;
        }
        self.make(code, f, ask, rng)
    }

    fn make(&mut self, code: i32, f: &Facts, ask: &mut Ask, rng: &mut Rand) -> Option<Sub> {
        let code = stand_in(code);
        self.rethink = 0.0;
        self.flags &= !MAKER_CLEARS;
        let (sub, time) = match code {
            FIRE if self.sub == Some(Sub::Retreat) => (Sub::Fire, Some(5.0)),
            FIRE => {
                self.rethink = self.fire_rethink(f, ask, rng);
                (Sub::Fire, None)
            }
            RETREAT => {
                self.flags |= FLEE;
                (Sub::Retreat, None)
            }
            CHARGE => {
                self.rethink = 5.0 * rng.float_in_range(0.9, 1.1);
                (Sub::Charge, None)
            }
            aiweapon::SWAP => {
                if let Some(a) = self.arms.as_mut() {
                    aiweapon::make_swap(&mut a.w, a.rows);
                }
                (Sub::Swap, None)
            }
            _ => return None,
        };
        self.then = if time.is_some() { (4, RETREAT) } else { (0, NONE) };
        (self.sub, self.fire_time, self.made, self.remade) = (Some(sub), time, None, true);
        Some(sub)
    }

    fn fire_rethink(&self, f: &Facts, ask: &mut Ask, rng: &mut Rand) -> f32 {
        let base = if ask(100, rng) == REPOSITION { 1.5 } else if f.cover { 5.0 } else if f.target_in_vehicle { 1.0 } else { 2.5 };
        base * rng.float_in_range(0.9, 1.1)
    }

    fn first(&mut self, f: &Facts, ask: &mut Ask, rng: &mut Rand) -> Option<Sub> {
        self.weapon_check(f);
        self.courage(f, rng);
        let mut code = Combat::situation(f, true, ask, rng);
        if code == NONE {
            code = Combat::situation(f, false, ask, rng);
        }
        if code == NONE {
            code = FIRE;
        }
        let code = self.adjust(f, code, rng);
        let made = self.make(code, f, ask, rng);
        self.covers = 0;
        made
    }

    pub fn finished(&mut self, f: &Facts, ask: &mut Ask, rng: &mut Rand) -> bool {
        f.target && self.pick(f, true, false, ask, rng).is_some()
    }

    pub fn fire_tick(&mut self, dt: f32) {
        if let (Some(Sub::Fire), Some(t)) = (self.sub, self.fire_time.as_mut()) {
            if *t > 0.0 {
                *t -= dt;
            }
        }
    }

    pub fn fire_out(&self) -> bool {
        matches!((self.sub, self.fire_time), (Some(Sub::Fire), Some(t)) if t <= 0.0)
    }

    fn courage(&mut self, f: &Facts, rng: &mut Rand) -> bool {
        if self.flags & FLEE != 0 {
            return true;
        }
        let run = |c: &mut Combat, bits| {
            c.flags |= bits;
            true
        };
        let p = f.population && !f.cop;
        let g = p && f.gang;
        if p && (!f.target_player && f.target_mission && !f.target_friend || f.target_arrested) {
            return run(self, FLEE);
        }
        if self.flags & COIN != 0 && p && !f.guard && f.health < 120.0 + 20.0 * (4 - i32::from(f.bravery)).max(0) as f32 {
            return run(self, FLEE);
        }
        let first = self.flags & ROLLED == 0;
        if !first && (self.flags & MEMORY) >> 24 >= u32::from(f.target_armed) {
            return false;
        }
        self.flags = self.flags & !MEMORY | u32::from(f.target_armed) << 24;
        if first && p && (f.wanted_cop_near || f.target_cop) {
            return run(self, FLEE | ROLLED);
        }
        let mut level = (i32::from(f.bravery) - 1).clamp(0, 3);
        if self.level != -1 {
            level = i32::from(self.level);
        }
        if f.mission || f.cop || f.guard {
            level = 4;
        }
        if g && f.target_armed <= 1 && f.fights >= 2 {
            return run(self, FLEE | ROLLED);
        }
        if g {
            level = if f.guard || self.flags & FIGHT != 0 || !first || f.hates_target { 4 } else { 3 };
        }
        let mut chance = chance(level, f.target_armed);
        if chance < 1.0 {
            chance *= BRAVE_SCALE;
        }
        self.flags |= ROLLED;
        self.cap = 0.0;
        if rng.float_in_range(0.0, 1.0) > chance {
            self.fight_run = f.target_armed <= 1;
            return run(self, FLEE);
        }
        if g && f.target_armed <= 1 && f.group6 && f.target_player {
            self.cap = 20.0;
        }
        false
    }

    pub fn step(&mut self, f: &Facts, ask: &mut Ask, rng: &mut Rand) -> bool {
        self.abort = None;
        self.flags &= !ARREST;
        for t in [&mut self.t70, &mut self.t74] {
            if *t > 0.0 {
                *t -= f.dt;
            }
        }
        let Some(sub) = self.sub else {
            if !f.target {
                return false;
            }
            return self.first(f, ask, rng).is_some();
        };
        if !f.target {
            return self.stop(0, Refusal::Keep);
        }
        let (life, cap) = (tick(&mut self.life, f.dt), tick(&mut self.cap, f.dt));
        if life | cap {
            return self.stop(1, Refusal::Retry { life, cap });
        }
        self.after_lifetimes(f, sub, ask, rng)
    }

    fn after_lifetimes(&mut self, f: &Facts, sub: Sub, ask: &mut Ask, rng: &mut Rand) -> bool {
        if f.target_hurt && !f.for_injured {
            return self.stop(0, Refusal::Keep);
        }
        if self.weapon_check(f) {
            let held = self.clone();
            let made = self.pick(f, false, false, ask, rng).is_some();
            self.abort = Some((1, Refusal::Recheck(Box::new(held))));
            return made;
        }
        self.after_check(f, sub, ask, rng)
    }

    fn after_check(&mut self, f: &Facts, sub: Sub, ask: &mut Ask, rng: &mut Rand) -> bool {
        if self.flags & FLEE == 0 && (self.courage(f, rng) || sub == Sub::Retreat) || sub == Sub::Charge && self.armed(f) {
            let held = self.clone();
            let made = self.pick(f, false, false, ask, rng).is_some();
            self.abort = Some((1, Refusal::Repick(Box::new(held))));
            return made;
        }
        self.rest(f, sub, ask, rng);
        true
    }

    fn weapon_check(&mut self, f: &Facts) -> bool {
        let (flags, swapping) = (self.flags, self.sub == Some(Sub::Swap));
        let Some(a) = self.arms.as_mut() else { return false };
        let t = aiweapon::Target { there: f.target || f.target_hurt && f.for_injured, damaging: f.target_armed == 2, player: f.target_player };
        aiweapon::check(flags, swapping, &mut a.me, &t, &mut a.w, a.rows)
    }

    fn armed(&self, f: &Facts) -> bool {
        self.arms.as_ref().map_or(f.armed, |a| aiweapon::damaging(&a.w, a.rows, true))
    }

    fn stop(&mut self, urgency: u8, refusal: Refusal) -> bool {
        self.abort = Some((urgency, refusal));
        false
    }

    fn rest(&mut self, f: &Facts, sub: Sub, ask: &mut Ask, rng: &mut Rand) {
        if sub == Sub::Fire {
            self.firing += f.dt;
        } else if self.flags & ARREST == 0 {
            self.firing = 0.0;
        }
        let stands = !f.no_duck && sub == Sub::Fire && !f.cover && f.standing_style == 0;
        if !stands && f.alertness >= ALERT {
            self.firing = 0.0;
        }
        if (sub == Sub::Fire && self.t74 <= 0.0 || sub == Sub::Charge) && self.rethink > 0.0 {
            self.rethink -= f.dt;
            if self.rethink <= 0.0 {
                self.flags &= !REFIRE;
                let held = self.clone();
                if self.pick(f, false, true, ask, rng).is_some() {
                    self.abort = Some((1, Refusal::Rethink(Box::new(held))));
                } else {
                    self.rethink_again(f, ask, rng);
                }
            }
        }
    }

    fn rethink_again(&mut self, f: &Facts, ask: &mut Ask, rng: &mut Rand) {
        self.rethink = if self.sub == Some(Sub::Charge) { 5.0 * rng.float_in_range(0.9, 1.1) } else { self.fire_rethink(f, ask, rng) };
        if Combat::indoors(f) {
            self.rethink += rng.float_in_range(5.0, 8.0);
        }
    }

    pub fn abort(&self) -> Option<u8> {
        self.abort.as_ref().map(|a| a.0)
    }

    pub fn refused(&mut self, f: &Facts, ask: &mut Ask, rng: &mut Rand) -> bool {
        match self.abort.take().map(|a| a.1) {
            Some(Refusal::Keep) => true,
            Some(Refusal::Retry { life, cap }) => {
                if life {
                    self.life = 0.0001;
                }
                if cap {
                    self.cap = 0.0001;
                }
                self.sub.is_none_or(|sub| self.after_lifetimes(f, sub, ask, rng))
            }
            Some(Refusal::Recheck(held)) => {
                *self = *held;
                self.sub.is_none_or(|sub| self.after_check(f, sub, ask, rng))
            }
            Some(Refusal::Repick(held)) => {
                *self = *held;
                if let Some(sub) = self.sub {
                    self.rest(f, sub, ask, rng);
                }
                true
            }
            Some(Refusal::Rethink(held)) => {
                (self.sub, self.then, self.fire_time, self.advance, self.made, self.remade) =
                    (held.sub, held.then, held.fire_time, held.advance, held.made, false);
                self.rethink_again(f, ask, rng);
                true
            }
            None => false,
        }
    }

    pub fn move_set(&self, f: &Facts) -> bool {
        let fire = self.sub == Some(Sub::Fire);
        (!f.no_duck && fire && !f.cover && f.standing_style == 0)
            || f.alertness < ALERT && !f.no_duck && !f.cover && self.flags & (REFIRE | MAKER_CLEARS) != 0
    }

    pub fn arrest(&self) -> bool {
        self.flags & ARREST != 0
    }
}

#[derive(Clone, Debug)]
enum Refusal {
    Keep,
    Retry { life: bool, cap: bool },
    Recheck(Box<Combat>),
    Repick(Box<Combat>),
    Rethink(Box<Combat>),
}

const SPOTTED: u32 = 1 << 18;

#[derive(Clone, Copy, Debug, Default)]
pub struct FireIn {
    pub cop: bool,
    pub in_car: bool,
    pub firing: bool,
    pub target_in_car: bool,
    pub target_player: bool,
    pub one_star: bool,
    pub friends: u32,
    pub target_weapon: Option<(i32, i32)>,
    pub cops_near: u32,
    pub swat_near: u32,
    pub wanted: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineAsk {
    pub context: &'static str,
    pub chance: Option<f32>,
    pub repeat: bool,
}

pub fn fire_lines(first: &mut bool, f: &FireIn, mut roll: impl FnMut() -> f32) -> Vec<LineAsk> {
    let say = |context, repeat| LineAsk { context, chance: None, repeat };
    let maybe = |context, repeat| LineAsk { context, chance: Some(0.05), repeat };
    if std::mem::take(first) {
        return if f.cop { vec![say("SPOT_SUSPECT", true)] } else { vec![] };
    }
    let mut out = vec![];
    if f.in_car || !f.firing || roll() >= 0.1 {
        backup(&mut out, f, &mut roll);
        return out;
    }
    if f.target_in_car && f.cop {
        out.push(say(if f.target_player && f.one_star { "GET_OUT_OF_CAR" } else { "GET_OUT_OF_CAR_SHOUT" }, false));
    }
    if f.friends >= 2 && f.target_player {
        out.extend([maybe("MOVE_IN", false), maybe("SURROUNDED", false)]);
        if f.cop {
            out.extend([maybe("GET_DOWN", false), maybe("EVACUATE_AREA", false)]);
        }
    }
    if let (true, Some((damage, kind))) = (f.cop, f.target_weapon) {
        if damage == 2 {
            out.push(maybe("SPOT_GUN", true));
        } else if damage != 1 && kind != 0x2E {
            out.push(maybe("SPOT_WEAPON", false));
        }
    }
    backup(&mut out, f, &mut roll);
    out
}

fn backup(out: &mut Vec<LineAsk>, f: &FireIn, roll: &mut impl FnMut() -> f32) {
    if !f.cop || roll() >= 0.1 {
        return;
    }
    let maybe = |context| LineAsk { context, chance: Some(0.01), repeat: false };
    if f.cops_near <= 1 || (f.cops_near == 2 && f.wanted > 1) {
        out.push(maybe("REQUEST_BACKUP"));
    }
    if f.swat_near == 0 && f.wanted >= 4 {
        out.push(maybe("REQUEST_SWAT"));
    }
}

impl Combat {
    pub fn fire_lines(&mut self, f: &FireIn, roll: impl FnMut() -> f32) -> Vec<LineAsk> {
        let mut first = self.flags & SPOTTED != 0;
        let out = fire_lines(&mut first, f, roll);
        if !first {
            self.flags &= !SPOTTED;
        }
        out
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SpeechIn {
    pub teamwork: u8,
    pub dt: f32,
    pub target_melee: bool,
    pub target_gun: bool,
    pub target_player: bool,
    pub target_ok: bool,
    pub firing: bool,
    pub own_melee: bool,
    pub own_gun: bool,
    pub in_car: bool,
    pub busy_2e8: bool,
    pub target_in_car: bool,
    pub cop: bool,
    pub one_star: bool,
    pub gang: bool,
    pub target_cop: bool,
}

impl Combat {
    pub fn speech_step(&mut self, f: &SpeechIn, mut roll: impl FnMut(f32, f32) -> f32) -> Vec<LineAsk> {
        let say = |context| LineAsk { context, chance: None, repeat: false };
        let mut out = vec![];
        if f.teamwork < 2 {
            return out;
        }
        if self.talk > 0.0 {
            self.talk -= f.dt;
        }
        if self.talk > 0.0 {
            return out;
        }
        if self.flags & FLEE != 0 {
            if f.target_melee {
                out.push(say("FIGHT_RUN"));
            } else if f.target_player && f.target_gun {
                out.push(say("GUN_RUN"));
            }
            self.talk = roll(-0.1, 0.1) + 1.5;
            return out;
        }
        if !f.target_ok {
            return out;
        }
        self.talk = roll(-0.1, 0.1) + 3.0;
        if !f.firing {
            return out;
        }
        if f.own_melee {
            out.push(say("FIGHT"));
            return out;
        }
        if !f.own_gun || f.busy_2e8 || f.in_car {
            return out;
        }
        out.push(say("TARGET"));
        if f.target_in_car && f.cop {
            out.push(say(if f.target_player && f.one_star { "GET_OUT_OF_CAR" } else { "GET_OUT_OF_CAR_SHOUT" }));
        }
        if f.gang && f.target_cop {
            out.push(LineAsk { context: "GANG_YOU_DROP_WEAPON", chance: Some(0.05), repeat: false });
        }
        out
    }
}

pub fn damage_type_number(name: &str) -> i32 {
    crate::weapons::DAMAGE_TYPES.iter().position(|&d| d == name).map_or(0, |k| k as i32)
}

