#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vehicle {
    pub pos: [f32; 3],
    pub forward: [f32; 3],
    pub right: [f32; 3],
    pub velocity: [f32; 3],
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub flipped: bool,
    pub kind: i8,
    pub class: i32,
    pub status: u32,
    pub driver_player: bool,
    pub horn: bool,
    pub driven: bool,
}

impl Vehicle {
    pub fn alarming(&self) -> bool {
        self.driver_player || (crate::kv!(0 int)..=crate::kv!(2 int)).contains(&self.class) || !matches!(self.kind, 0 | 1 | 6) || self.status == 1
    }

    pub fn speed(&self) -> f32 {
        let (f, v) = (self.forward, self.velocity);
        f[0].mul_add(v[0], f[2].mul_add(v[2], f[1] * v[1]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    pub pos: [f32; 3],
    pub forward: [f32; 3],
    pub on_it: bool,
    pub animated: bool,
    pub dodging: bool,
    pub player_or_mission: bool,
    pub health: f32,
    pub agility: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reaction {
    None,
    Brace { weak: bool },
    Warn,
}

pub fn weak(w: &Walker) -> bool {
    w.agility < crate::kv!(0 int) || w.health < if w.player_or_mission { crate::kv!(43) } else { crate::kv!(44) }
}

pub fn scan(car: &Vehicle, ped: &Walker, roll: impl FnOnce() -> f32, gate: impl FnOnce() -> bool) -> Reaction {
    if !car.alarming() || ped.on_it {
        return Reaction::None;
    }
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let (front, rear, half) = (car.max[1], car.min[1], car.max[0]);
    let v = car.speed();
    let mut a = v - 2.0;
    if a < 0.0 {
        a = 0.0;
    }
    if v < 0.0 {
        a = -v;
    }
    let ahead = if a > 0.0 { (a * crate::kv!(621 f32)).mul_add(crate::kv!(61), 2.0) } else { 0.0 };
    let (far, near) = if v >= 0.0 {
        let reach = crate::kv!(18) * v;
        (ahead + front, (if reach >= 2.0 { 2.0 } else { reach }) + front)
    } else {
        let reach = crate::kv!(18) * v;
        (rear - ahead, rear - if reach > -2.0 { reach } else { -2.0 })
    };
    let (wide, wider) = (crate::kv!(17) + half, 0.5 + half);
    let axis = if car.flipped { [0.0 - car.forward[0], 0.0 - car.forward[1], 0.0 - car.forward[2]] } else { car.forward };
    let d = sub(ped.pos, car.pos);
    if (ped.pos[2] - car.pos[2]).abs() >= crate::kv!(32) {
        return Reaction::None;
    }
    let along_xy = (ped.pos[0] - car.pos[0]).mul_add(axis[0], (ped.pos[1] - car.pos[1]) * axis[1]);
    if (ped.pos[2] - axis[2].mul_add(along_xy, car.pos[2])).abs() >= crate::kv!(30) {
        return Reaction::None;
    }
    let along = d[0].mul_add(axis[0], d[2].mul_add(axis[2], d[1] * axis[1]));
    let sign = |x: f32| if x < 0.0 { -1 } else { 1 };
    if v == 0.0 || sign(along) != sign(v) || v.abs() <= crate::kv!(622 f32) || along.abs() <= front {
        return Reaction::None;
    }
    let across = car.right[0].mul_add(d[0], car.right[1].mul_add(d[1], car.right[2] * d[2])).abs();
    let facing = {
        let t = ped.forward[1].mul_add(d[1], ped.forward[2] * d[2]);
        d[0].mul_add(ped.forward[0], t) < 0.0
    };
    let roll = roll();
    let mut aware = facing || car.horn_driver() || along.abs() < front;
    let warn_chance = if aware {
        roll < crate::kv!(623 f32)
    } else {
        aware = roll < crate::kv!(624 f32);
        false
    };
    if !aware {
        return Reaction::None;
    }
    let (zone, close) = if v < 0.0 {
        (along > far && along < rear - 2.0, along > near)
    } else {
        (along < far && along > front + 2.0, along < near)
    };
    if !ped.dodging && v.abs() < crate::kv!(26) && close && across <= wide {
        if !ped.animated || !gate() {
            return Reaction::None;
        }
        return Reaction::Brace { weak: weak(ped) };
    }
    if warn_chance && across <= wider && v.abs() > crate::kv!(27) && zone {
        return Reaction::Warn;
    }
    Reaction::None
}

impl Vehicle {
    fn horn_driver(&self) -> bool {
        self.driver_player && self.horn
    }
}

#[allow(non_snake_case)]
#[inline]
pub fn EVASIVE_STEP() -> u32 { crate::kv!(625 int) }

pub fn dives(car: &Vehicle, agility: u8, fighting: bool, roll: impl FnOnce() -> f32) -> bool {
    let v = car.velocity;
    let s = v[2].mul_add(v[2], v[0].mul_add(v[0], v[1] * v[1]));
    let pressed = car.horn_driver() || roll() < 0.5;
    s > crate::kv!(127) && (agility > 1 && (s > crate::kv!(626) || pressed) || fighting)
}

pub fn step_anim(car: &Vehicle, ped: [f32; 3], heading: f32, dive: bool) -> Option<&'static crate::runanim::Row> {
    mover_anim(car.pos, car.velocity, car.forward, ped, heading, dive)
}

pub fn mover_anim(pos: [f32; 3], velocity: [f32; 3], forward: [f32; 3], ped: [f32; 3], heading: f32, dive: bool) -> Option<&'static crate::runanim::Row> {
    use std::f32::consts::{FRAC_2_PI, FRAC_PI_4, TAU};
    let d = [pos[0] - ped[0], pos[1] - ped[1], pos[2] - ped[2]];
    let len = d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2])).sqrt();
    let dir = d.map(|x| x / len);
    let v = velocity;
    let motion = if v[0].mul_add(v[0], v[1].mul_add(v[1], v[2] * v[2])) > 0.5 { v } else { forward };
    let rightward = dir[1].mul_add(motion[0], -dir[0] * motion[1]) > 0.0;
    let mut a = crate::fall::bearing([dir[0], dir[1]], heading) + FRAC_PI_4;
    if a < 0.0 {
        a += TAU;
    }
    let id = match (a * FRAC_2_PI) as i32 {
        0 => crate::kv!(322 int) + rightward as i32,
        1 => crate::kv!(484 int) - rightward as i32,
        2 => crate::kv!(427 int) - rightward as i32,
        3 => crate::kv!(455 int) + rightward as i32,
        _ => return None,
    };
    let rows = if dive { &(*crate::runanim::EV_DIVES()) } else { &(*crate::runanim::AVOIDS()) };
    rows.get((id - crate::kv!(322 int)) as usize)
}

pub fn dive_anim(place: [f32; 3], ped: [f32; 3], forward: [f32; 3], heading: f32) -> Option<&'static crate::runanim::Row> {
    let d = [place[0] - ped[0], place[1] - ped[1]];
    let len = d[0].mul_add(d[0], d[1] * d[1]).sqrt();
    let dir = if len > 0.0 { [d[0] / len, d[1] / len] } else { [0.0; 2] };
    let ahead = dir[0].mul_add(forward[0], dir[1] * forward[1]) > 0.0;
    way_anim(dir, heading, ahead, true)
}

pub fn way_anim(way: [f32; 2], heading: f32, ahead: bool, dive: bool) -> Option<&'static crate::runanim::Row> {
    use std::f32::consts::{FRAC_2_PI, FRAC_PI_4, TAU};
    let mut a = crate::fall::bearing(way, heading) + FRAC_PI_4;
    if a < 0.0 {
        a += TAU;
    }
    let id = match (a * FRAC_2_PI) as i32 {
        0 => crate::kv!(322 int) + ahead as i32,
        1 => crate::kv!(484 int) - ahead as i32,
        2 => crate::kv!(427 int) - ahead as i32,
        3 => crate::kv!(455 int) + ahead as i32,
        _ => return None,
    };
    let rows = if dive { &(*crate::runanim::EV_DIVES()) } else { &(*crate::runanim::AVOIDS()) };
    rows.get((id - crate::kv!(322 int)) as usize)
}

pub fn step_rate(draw: u32) -> f32 {
    (draw as f32 * crate::kv!(519 f32)).mul_add(crate::kv!(627 f32), crate::kv!(628 f32))
}

