use crate::actiontable::wrap;
use crate::mathrt;

pub const COUNT: usize = 48;
pub const LEADER: usize = 7;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Member {
    pub pos: [f32; 3],
    pub heading: f32,
    pub facing: f32,
    pub vel: [f32; 3],
    pub dead: bool,
    pub player: bool,
    pub stays: bool,
}

#[derive(Clone, Debug)]
pub struct Group<P> {
    pub slots: [Option<P>; 8],
    pub separation: f32,
    pub formation: Formation,
    pub kind: u8,
    pub in_use: bool,
    pub owner: u32,
}

impl<P: Copy + PartialEq> Default for Group<P> {
    fn default() -> Self {
        Group { slots: [None; 8], separation: 30.0, formation: Formation::new(LOOSE), kind: 0, in_use: false, owner: 0 }
    }
}

impl<P: Copy + PartialEq> Group<P> {
    pub fn leader(&self) -> Option<P> {
        self.slots[LEADER]
    }

    pub fn has(&self, p: P) -> bool {
        self.slots.contains(&Some(p))
    }

    pub fn count(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn followers(&self) -> usize {
        self.slots[..LEADER].iter().flatten().count()
    }

    pub fn add(&mut self, p: P) {
        if !self.has(p) {
            if let Some(s) = self.slots[..LEADER].iter_mut().find(|s| s.is_none()) {
                *s = Some(p);
            }
        }
    }

    pub fn remove(&mut self, p: P) {
        if let Some(s) = self.slots.iter_mut().find(|s| **s == Some(p)) {
            *s = None;
        }
    }

    pub fn set_leader(&mut self, p: P) {
        if self.slots[..LEADER].contains(&Some(p)) {
            self.remove(p);
        }
        self.slots[LEADER] = Some(p);
    }

    pub fn promote(&mut self) {
        if self.slots[LEADER].is_none() {
            if let Some(i) = self.slots[..LEADER].iter().position(|s| s.is_some()) {
                self.slots[LEADER] = self.slots[i].take();
            }
        }
    }

    pub fn flush(&mut self) {
        self.slots = [None; 8];
        self.in_use = false;
    }

    pub fn set_formation(&mut self, kind: u8, member: impl Fn(P) -> Member) {
        self.formation = Formation::new(kind);
        self.place(&member);
    }

    fn place(&mut self, member: &impl Fn(P) -> Member) {
        if let Some(l) = self.slots[LEADER] {
            let followers = std::array::from_fn(|i| self.slots[i].map(member));
            self.formation.update(&member(l), &followers);
        }
    }

    pub fn update(&mut self, member: impl Fn(P) -> Member) {
        for s in 0..8 {
            if let Some(p) = self.slots[s] {
                let m = member(p);
                if m.dead && !(s == LEADER && m.player) {
                    self.slots[s] = None;
                }
            }
        }
        self.promote();
        if let Some(l) = self.slots[LEADER] {
            let l = member(l);
            let range = self.separation * self.separation;
            for s in 0..LEADER {
                if let Some(p) = self.slots[s] {
                    let m = member(p);
                    let d = [m.pos[0] - l.pos[0], m.pos[1] - l.pos[1], m.pos[2] - l.pos[2]];
                    if !m.stays && d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2])) > range {
                        self.slots[s] = None;
                    }
                }
            }
        }
        self.place(&member);
        if self.kind == 0 && self.count() == 0 {
            self.flush();
        }
    }
}

#[derive(Clone, Debug)]
pub struct Groups<P>(pub Vec<Group<P>>);

impl<P: Copy + PartialEq> Default for Groups<P> {
    fn default() -> Self {
        Groups((0..COUNT).map(|_| Group::default()).collect())
    }
}

impl<P: Copy + PartialEq> Groups<P> {
    pub fn of(&self, p: P) -> Option<usize> {
        self.0.iter().position(|g| g.has(p))
    }

    pub fn same(&self, a: P, b: P) -> bool {
        self.of(a).is_some_and(|g| self.0[g].has(b))
    }

    pub fn create(&mut self, kind: u8) -> Option<usize> {
        let range = match kind {
            0 => 32..48,
            1 | 2 => 0..32,
            _ => 0..48,
        };
        let i = range.into_iter().find(|&i| !self.0[i].in_use)?;
        let g = &mut self.0[i];
        g.flush();
        (g.in_use, g.kind) = (true, kind);
        Some(i)
    }

    pub fn release(&mut self, owner: u32, member: impl Fn(P) -> Member) {
        for g in self.0.iter_mut().filter(|g| g.owner == owner) {
            g.set_formation(LOOSE, &member);
        }
    }

    pub fn update(&mut self, member: impl Fn(P) -> Member) {
        for g in self.0.iter_mut().filter(|g| g.in_use) {
            g.update(&member);
            if g.kind != 2 && g.count() == 0 {
                g.flush();
            }
        }
    }
}

pub const LOOSE: u8 = 0;
pub const SURROUND_FACING_INWARDS: u8 = 1;
pub const SURROUND_FACING_AHEAD: u8 = 2;
pub const LINE_ABREAST: u8 = 3;
pub const ARROWHEAD: u8 = 4;
pub const V: u8 = 5;
pub const FOLLOW_IN_LINE: u8 = 6;

pub fn script_formation(n: i32) -> Option<u8> {
    match n {
        0..=3 => Some(n as u8),
        4 => Some(FOLLOW_IN_LINE),
        _ => None,
    }
}

const TURNS: u32 = 0x8000_0000;
const FACE_LEADER: u32 = 0x4000_0000;
const FACE_AWAY: u32 = 0x2000_0000;
pub const LEAD: u32 = 0x0400_0000;
pub const CROWD: u32 = 0x0800_0000;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place {
    pub pos: [f32; 3],
    pub heading: f32,
    pub near: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Formation {
    pub kind: u8,
    pub spacing: f32,
    pub catch_up: [f32; 2],
    pub flags: u32,
    pub places: [Place; 7],
    pub leader: Place,
    count: usize,
    last: [f32; 3],
    trail: [[f32; 3]; 8],
    trail_heading: [f32; 8],
    trail_near: [f32; 8],
}

impl Formation {
    pub fn new(kind: u8) -> Formation {
        let (spacing, flags) = match kind {
            LOOSE => (2.5, 0x4E00_0000),
            SURROUND_FACING_INWARDS => (3.0, 0x4400_0000),
            SURROUND_FACING_AHEAD => (3.0, 0x1400_0000),
            FOLLOW_IN_LINE => (2.0, 0x9000_0000),
            _ => (2.0, 0x9400_0000),
        };
        let catch_up = if kind == FOLLOW_IN_LINE { [1.0, 5.0] } else { [3.0, 16.0] };
        Formation { kind, spacing, catch_up, flags, places: Default::default(), leader: Place::default(), count: 0,
            last: [f32::from_bits(0x461C_3F9A); 3], trail: Default::default(), trail_heading: [0.0; 8],
            trail_near: [0.0; 8] }
    }

    pub fn update(&mut self, leader: &Member, followers: &[Option<Member>; 7]) {
        match self.kind {
            LOOSE => self.loose(leader, followers),
            SURROUND_FACING_INWARDS | SURROUND_FACING_AHEAD => self.surround(leader, followers),
            LINE_ABREAST => self.line_abreast(leader, followers),
            ARROWHEAD | V => self.arrowhead(leader, followers),
            _ => self.follow_in_line(leader, followers),
        }
    }

    fn loose(&mut self, l: &Member, followers: &[Option<Member>; 7]) {
        for (place, m) in self.places.iter_mut().zip(followers) {
            if let Some(m) = m {
                *place = Place { pos: l.pos, heading: heading_between(l.pos[0], l.pos[1], m.pos[0], m.pos[1]),
                    near: self.spacing };
            }
        }
    }

    fn surround(&mut self, l: &Member, followers: &[Option<Member>; 7]) {
        let step = f32::from_bits(0x40C9_0FDB) / followers.iter().flatten().count() as f32;
        let mut a = 0.0f32;
        for (place, _) in self.places.iter_mut().zip(followers).filter(|(_, m)| m.is_some()) {
            let at = if self.flags & TURNS != 0 { wrap(l.facing + a) } else { a };
            let (c, s) = (mathrt::cos(at as f64) as f32, mathrt::sin(at as f64) as f32);
            place.pos = [-(self.spacing * s) + l.pos[0], self.spacing * c + l.pos[1], 0.0 + l.pos[2]];
            place.near = 1.0;
            if self.kind == SURROUND_FACING_INWARDS {
                place.heading = if self.flags & FACE_LEADER != 0 {
                    wrap(a + std::f32::consts::PI)
                } else if self.flags & FACE_AWAY != 0 {
                    wrap(a)
                } else {
                    l.facing
                };
            }
            a += step;
        }
        let heading = if self.kind == SURROUND_FACING_INWARDS { l.facing } else { l.heading };
        self.leader = Place { pos: l.pos, heading, near: 1.0 };
    }

    fn axes(&self, l: &Member) -> ([f32; 3], [f32; 3]) {
        if self.flags & TURNS == 0 {
            return ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        }
        let (c, s) = (mathrt::cos(l.heading as f64) as f32, mathrt::sin(l.heading as f64) as f32);
        ([c, s, 0.0], [-s, c, 0.0])
    }

    fn face(&self, l: &Member, p: [f32; 3]) -> f32 {
        wrap(if self.flags & FACE_LEADER != 0 {
            heading_between(l.pos[0], l.pos[1], p[0], p[1])
        } else if self.flags & FACE_AWAY != 0 {
            heading_between(p[0], p[1], l.pos[0], l.pos[1])
        } else {
            l.facing
        })
    }

    fn line_abreast(&mut self, l: &Member, followers: &[Option<Member>; 7]) {
        let (right, _) = self.axes(l);
        self.leader = Place { pos: l.pos, heading: l.heading, near: 1.0 };
        let mut side = -1.0f32;
        for i in 0..7 {
            if followers[i].is_some() {
                let p: [f32; 3] = std::array::from_fn(|k| (right[k] * side).mul_add(self.spacing, l.pos[k]));
                self.places[i] = Place { pos: p, heading: self.face(l, p), near: 1.0 };
                side *= -1.0;
            }
        }
    }

    fn arrowhead(&mut self, l: &Member, followers: &[Option<Member>; 7]) {
        let (right, fwd) = self.axes(l);
        self.leader = Place { pos: l.pos, heading: l.heading, near: 1.0 };
        let sp = self.spacing;
        let half = sp * 0.5;
        let (mut row, mut size, mut at) = (1i32, 3i32, 0i32);
        for i in 0..7 {
            if followers[i].is_some() {
                let (back, mid) = (row as f32 * sp, size as f32 * 0.5);
                let p: [f32; 3] = std::array::from_fn(|k| {
                    let v = l.pos[k] - fwd[k] * back - right[k] * sp * mid + right[k] * half;
                    (right[k] * sp).mul_add(at as f32, v)
                });
                self.places[i] = Place { pos: p, heading: self.face(l, p), near: 1.0 };
                at += 1;
                if at == size {
                    (row, size, at) = (row + 1, size + 2, 0);
                }
            }
        }
    }

    fn follow_in_line(&mut self, l: &Member, followers: &[Option<Member>; 7]) {
        let n = followers.iter().flatten().count();
        let s2 = self.spacing * self.spacing;
        let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let mut d = sub(l.pos, self.last);
        let mut gone = d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2]));
        if gone > 100.0 {
            (self.trail, self.trail_heading, self.trail_near) = ([l.pos; 8], [l.heading; 8], [6.0; 8]);
            self.last = l.pos;
            (d, gone) = ([0.0; 3], 0.001);
        }
        if gone > s2 {
            self.trail.copy_within(0..7, 1);
            self.trail_heading.copy_within(0..7, 1);
            self.trail_near.copy_within(0..7, 1);
            self.trail[0] = l.pos;
            let v = sub(l.pos, self.last);
            self.trail_near[0] = 1.0;
            self.trail_heading[0] =
                if v[0].mul_add(v[0], v[1] * v[1]) > 0.05 { heading_between(v[0], v[1], 0.0, 0.0) } else { l.heading };
            self.last = l.pos;
            (d, gone) = ([0.0; 3], 0.001);
        }
        let speed = l.vel[2].mul_add(l.vel[2], l.vel[0].mul_add(l.vel[0], l.vel[1] * l.vel[1])).sqrt();
        if gone != 0.0 || n != self.count {
            self.leader = Place { pos: l.pos, heading: l.heading, near: 1.0 };
            let frac = d[1].mul_add(d[1], d[0] * d[0]).sqrt().min(self.spacing) / self.spacing;
            let mut m = 0;
            for i in 0..7 {
                if followers[i].is_some() {
                    let (a, b) = (self.trail[m], self.trail[m + 1]);
                    let seg = sub(a, b);
                    let mut p: [f32; 3] = std::array::from_fn(|k| seg[k].mul_add(frac, b[k]));
                    if speed > 0.1 {
                        p = std::array::from_fn(|k| seg[k].mul_add(0.5, p[k]));
                    }
                    self.places[i] = Place { pos: p, heading: heading_between(seg[0], seg[1], 0.0, 0.0),
                        near: self.trail_near[m + 1] };
                    m += 1;
                }
            }
        }
        self.count = n;
    }
}

pub fn heading_between(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    const H: f32 = std::f32::consts::FRAC_PI_2;
    let (dx, mut dy) = (x2 - x1, y2 - y1);
    if dy == 0.0 {
        dy = 0.0001;
    }
    let a = mathrt::atan2((dx / dy) as f64, 1.0) as f32;
    match (dx > 0.0, dy > 0.0) {
        (true, true) => (H - a) + H,
        (true, false) => H - (a + H),
        (false, true) => -H - (a + H),
        (false, false) => (H - a) - H,
    }
}

