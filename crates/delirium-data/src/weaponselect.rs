pub const PHONE: u32 = 46;
pub const NONE: u32 = 11;
pub const SLOTS: usize = 11;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    pub weapon: u32,
    pub ammo: u16,
    pub held: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Weapons {
    pub cur: u32,
    pub pending1: u32,
    pub pending2: u32,
    pub prev: u32,
    pub slots: [Slot; SLOTS],
    pub flag_8d: u8,
    pub flag_8f: u8,
    pub delay: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Info {
    pub fire: u32,
    pub damage: u32,
}

pub trait Env {
    fn info(&self, weapon: u32) -> Info;
    fn episode(&self) -> u32;
    fn arg_class(&mut self, arg: u32) -> u32;
    fn fits(&mut self, weapon: u32, arg: u32) -> bool;
    fn release(&mut self, weapon: u32);
    fn keep_held(&mut self, w: &Weapons) -> bool;
}

fn usable<E: Env>(env: &E, s: &Slot) -> bool {
    s.weapon as i32 > 0 && (s.ammo != 0 || env.info(s.weapon).damage == 1)
}

pub fn next<E: Env>(w: &Weapons, env: &E, from: i32) -> u32 {
    let mut t = from.wrapping_add(1);
    while t < SLOTS as i32 {
        if usable(env, &w.slots[t as usize]) {
            return t as u32;
        }
        t += 1;
    }
    0
}

pub fn prev<E: Env>(w: &Weapons, env: &E, from: u32) -> u32 {
    let mut t = from.wrapping_sub(1) as i32;
    loop {
        if t < 0 {
            t = SLOTS as i32 - 1;
        } else if t == 0 {
            return 0;
        }
        if usable(env, &w.slots[t as usize]) {
            return t as u32;
        }
        t -= 1;
    }
}

fn clear<E: Env>(w: &mut Weapons, env: &mut E, t: usize) {
    let s = w.slots[t];
    if s.held != 0 {
        env.release(s.weapon);
    }
    w.slots[t] = Slot::default();
}

fn validate<E: Env>(w: &mut Weapons, env: &mut E, t: u32) -> u32 {
    let s = w.slots[t as usize];
    if s.weapon == 0 {
        return 0;
    }
    if env.info(s.weapon).damage == 1 || s.ammo != 0 {
        return t;
    }
    clear(w, env, t as usize);
    0
}

fn commit<E: Env>(w: &mut Weapons, env: &mut E, t: u32, arg6: u32) -> bool {
    let old = w.cur;
    w.flag_8f = 0;
    if t == old && !env.keep_held(w) {
        return false;
    }
    w.prev = old;
    w.cur = t;
    w.flag_8d = 0;
    w.delay = if arg6 & 0xFF == 0 { 0xFFFF } else { 0 };
    true
}

fn back<E: Env>(w: &Weapons, env: &mut E, mode: u32, arg7: u32) -> u32 {
    let cur = w.cur;
    let mut t = prev(w, &*env, cur);
    if t == cur {
        return t;
    }
    loop {
        if t == 4 {
            if env.episode() < 2 || w.slots[4].weapon != 0x20 {
                return t;
            }
        } else if t == 2 {
            return t;
        }
        let taken = match mode {
            3 if t == 3 => {
                if arg7 != 0 && env.fits(w.slots[3].weapon, arg7) {
                    Some(3)
                } else {
                    None
                }
            }
            3 => (t == 8).then_some(8),
            4 if t == 5 => Some(5),
            4 | 5 => (t == 8).then_some(8),
            _ => None,
        };
        if let Some(x) = taken {
            return x;
        }
        t = prev(w, &*env, t);
        if t == cur {
            return t;
        }
    }
}

fn walk<E: Env>(w: &Weapons, env: &E, mode: u32) -> u32 {
    let cur = w.cur;
    let mut t = if mode == 8 || mode == 9 {
        NONE
    } else if w.slots[cur as usize].weapon == PHONE && w.flag_8f != 0 {
        w.prev + 1
    } else {
        cur
    };
    loop {
        t = prev(w, env, t);
        if (mode == 8 || mode == 9) && w.slots[t as usize].weapon == PHONE {
            continue;
        }
        let info = env.info(w.slots[t as usize].weapon);
        let projectile_with_damage = info.fire == 3 && info.damage != 0;
        if !(projectile_with_damage && (mode == 7 || mode == 8)) {
            return t;
        }
    }
}

fn best<E: Env>(w: &Weapons, env: &E) -> u32 {
    const ORDER: [u32; 9] = [5, 4, 3, 2, 7, 8, 6, 1, 0];
    let cur = w.cur;
    let mut t = cur;
    for &k in &ORDER {
        if t != cur {
            break;
        }
        if usable(env, &w.slots[k as usize]) {
            t = k;
        }
    }
    t
}

fn choose<E: Env>(w: &mut Weapons, env: &mut E, mode: u32, arg5: u32, arg7: u32) -> u32 {
    let cur = w.cur;
    match mode {
        0 => arg5,
        1 => {
            let from = if w.slots[cur as usize].weapon == PHONE && w.flag_8f != 0 {
                w.prev.wrapping_sub(1)
            } else {
                cur
            };
            next(w, &*env, from as i32)
        }
        2..=5 => back(w, env, mode, arg7),
        6..=9 => walk(w, &*env, mode),
        10 => best(w, &*env),
        11..=15 => {
            if w.pending2 == NONE || arg5 == 4 || (cur != NONE && w.slots[cur as usize].weapon == PHONE) {
                w.pending2 = cur;
            }
            if mode == 15 {
                return cur;
            }
            let thrown_ok = w.slots[8].weapon as i32 > 0 && w.slots[8].ammo != 0;
            if cur == 8 && (12..=14).contains(&mode) && thrown_ok {
                return 8;
            }
            if mode == 12 {
                let s = w.slots[3];
                if s.weapon as i32 > 0 && s.ammo != 0 && arg7 != 0 && env.fits(s.weapon, arg7) {
                    return 3;
                }
            }
            if mode == 13 {
                let s = w.slots[5];
                if s.weapon as i32 > 0 && s.ammo != 0 {
                    return 5;
                }
            }
            let s4 = w.slots[4];
            if s4.weapon as i32 > 0 && (env.episode() < 2 || s4.weapon != 0x20) && s4.ammo != 0 {
                return 4;
            }
            let s2 = w.slots[2];
            if s2.weapon as i32 > 0 && s2.ammo != 0 {
                return 2;
            }
            if (12..=14).contains(&mode) && thrown_ok {
                return 8;
            }
            0
        }
        16 => {
            let t = w.pending2;
            let r = if t != NONE && w.slots[t as usize].weapon as i32 > 0 { t } else { 0 };
            w.pending2 = NONE;
            r
        }
        17 => {
            let t = w.pending1;
            let r = if t != NONE && w.slots[t as usize].weapon as i32 > 0 { t } else { 0 };
            w.pending1 = NONE;
            r
        }
        18 => {
            let t = w.prev;
            if t != NONE && w.slots[t as usize].weapon as i32 > 0 { t } else { 0 }
        }
        19 => {
            if w.pending2 == NONE {
                return arg5;
            }
            w.pending2 = arg5;
            if arg7 == 0 {
                return cur;
            }
            let class = env.arg_class(arg7) as i32;
            if arg5 == 4 || arg5 == 2 {
                return arg5;
            }
            let through = match class {
                13 => arg5 == 5 || arg5 == 8,
                14 => arg5 == 8,
                12 => arg5 == 3 || arg5 == 8,
                _ => false,
            };
            if through { arg5 } else { cur }
        }
        _ => cur,
    }
}

pub fn select<E: Env>(w: &mut Weapons, env: &mut E, mode: u32, arg5: u32, arg6: u32, arg7: u32) -> bool {
    let t = choose(w, env, mode, arg5, arg7);
    let t = if t != 0 { validate(w, env, t) } else { 0 };
    commit(w, env, t, arg6)
}
