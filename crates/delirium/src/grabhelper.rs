use crate::prelude::*;
use crate::msg::Value;
use crate::shottask::Msg;
use crate::{Fma, Vd};

const BLOCK: [f32; 7] = [600.0, 10.0, 4.4, 2.8, 1.4, 0.0, 1.0];

#[derive(Clone, Debug, Default)]
pub struct GrabHelper {
    pub pos1: Vec3,
    pub normal_l: Vec3,
    pub pos2: Vec3,
    pub normal_r: Vec3,
    pub pos3: Vec3,
    pub pos4: Vec3,
    pub flags: u32,
    pub instance: Option<(i32, i32)>,
    pub strength: f32,
    pub find_left: f32,
    pub hold_left: f32,
}

impl GrabHelper {
    pub fn new() -> GrabHelper {
        GrabHelper { strength: BLOCK[0], ..Default::default() }
    }

    pub fn set_points(&mut self, instance: Option<(i32, i32)>, p: [Option<Vec3>; 6]) {
        self.instance = instance;
        for (to, v) in [&mut self.pos1, &mut self.normal_l, &mut self.pos2, &mut self.normal_r, &mut self.pos3, &mut self.pos4].into_iter().zip(p) {
            if let Some(v) = v {
                *to = v;
            }
        }
        self.flags |= 1;
    }

    pub fn grabbing(&self) -> bool {
        self.flags & 0x600 != 0
    }

    pub fn start(&mut self) -> Option<Msg> {
        use Value::{Bool as B, Float as F, Int as I, Vec3 as V};
        if self.pos1 == Vec3::ZERO && self.pos2 == Vec3::ZERO {
            return None;
        }
        let mut p = vec![];
        if self.flags & 0x100 == 0 {
            p.push(("start", B(true)));
            self.flags |= 0x100;
        }
        for (n, v) in [("pos1", self.pos1), ("normalL", self.normal_l), ("pos2", self.pos2), ("normalR", self.normal_r), ("pos3", self.pos3), ("pos4", self.pos4)] {
            if v != Vec3::ZERO {
                p.push((n, V(v)));
            }
        }
        if let Some((i, part)) = self.instance {
            p.extend([("instanceIndex", I(i)), ("instancePartIndex", I(part))]);
        }
        p.extend([("useLeft", B(self.pos1 != Vec3::ZERO)), ("useRight", B(self.pos2 != Vec3::ZERO))]);
        if self.flags & 8 != 0 {
            p.push(("justbrace", B(true)));
        }
        if self.flags & 0x10 != 0 {
            p.push(("linegrab", B(true)));
        } else if self.flags & 0x20 != 0 {
            p.push(("surfaceGrab", B(true)));
        }
        p.extend([("bodyStiffness", F(BLOCK[1])), ("grabStrength", F(self.strength)), ("grabDist", F(BLOCK[2])),
                  ("reachAngle", F(BLOCK[3])), ("oneSideReachAngle", F(BLOCK[4])), ("dontLetGo", B(true)),
                  ("pullUpStrengthLeft", F(BLOCK[5])), ("pullUpStrengthRight", F(BLOCK[5])), ("pullUpTime", F(BLOCK[6]))]);
        Some(("grab", p))
    }

    pub fn update(&mut self, dt: f32, let_go: bool) -> Option<Msg> {
        let holding = self.grabbing();
        let mut stop = false;
        if holding && self.hold_left > 0.0 {
            self.hold_left -= dt;
            stop = self.hold_left <= 0.0;
        }
        stop |= let_go;
        let mut out = None;
        if stop && holding {
            self.flags = 0;
            out = Some(("grab", vec![("start", Value::Bool(false))]));
        }
        if !holding && self.find_left > 0.0 {
            self.find_left -= dt;
            if self.find_left <= 0.0 {
                self.flags = 0;
                out = Some(("grab", vec![("start", Value::Bool(false))]));
            }
        }
        out
    }

    pub fn feedback(&mut self, success: bool, name: &str) {
        let (left, right) = (name.starts_with("LeftArmGrab"), name.starts_with("RightArmGrab"));
        if left || right {
            self.flags |= if !success { 0x1000 } else if left { 0x200 } else { 0x400 };
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub point: Vec3,
    pub normal: Vec3,
    pub entity: Option<Entity>,
}

#[derive(Clone, Copy, Debug)]
pub struct Entity {
    pub kind: u32,
    pub grabbable: bool,
    pub n482: u32,
    pub b555: u8,
    pub rows: [Vec3; 4],
    pub instance: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct FindIn {
    pub vel: [Vec3; 2],
    pub hand_dir: Vec3,
    pub hand_pos: Vec3,
    pub ped_row0: Vec3,
    pub ped_pos: Vec3,
}

impl GrabHelper {
    pub fn find_surface(&mut self, i: &FindIn, probe: impl FnOnce(Vec3) -> Option<Hit>) -> Option<Msg> {
        const SPEED: [f32; 2] = [0.3, 0.5];
        const REACH: f32 = 0.3;
        const LIFT: f32 = 0.0;
        let sum = i.vel[0] + i.vel[1];
        let fast = |t: f32| sum.x.fma(sum.x, sum.y.fma(sum.y, sum.z * sum.z)) > t * t;
        let mut go = false;
        if self.flags & 2 != 0 && self.flags & 0x10000 != 0 {
            go |= fast(SPEED[0]);
        }
        if self.flags & 4 != 0 && self.flags & 0x70000 != 0 {
            go |= fast(SPEED[1]);
        }
        if !go || self.flags & 1 != 0 {
            return None;
        }
        let start = i.hand_pos + Vec3::new(0.0, 0.0, LIFT);
        let end = Vec3::new(i.hand_dir.x.fma(REACH, start.x), i.hand_dir.y.fma(REACH, start.y), i.hand_dir.z.fma(REACH, start.z));
        let hit = probe(end)?;
        let (mut point, mut normal, mut held) = (hit.point, hit.normal, None);
        if let Some(e) = hit.entity.filter(|e| (2..5).contains(&e.kind)) {
            let d = hit.point - e.rows[3];
            let local = |v: Vec3| Vec3::new(e.rows[0].vd(v), e.rows[1].vd(v), e.rows[2].vd(v));
            (point, normal) = (local(d), local(hit.normal));
            if !(e.kind == 4 && e.b555 != 0) {
                if !e.grabbable || e.n482 >= 2 {
                    return None;
                }
                held = Some(e);
            }
        }
        let d = hit.point - i.ped_pos;
        let left = i.ped_row0.x.fma(d.x, i.ped_row0.y.fma(d.y, i.ped_row0.z * d.z)) > 0.0;
        self.instance = held.map(|e| (e.instance, self.instance.map_or(0, |x| x.1)));
        let (zero, a, b) = (Vec3::ZERO, point, normal);
        (self.pos1, self.normal_l, self.pos2, self.normal_r) = if left { (a, b, zero, zero) } else { (zero, zero, a, a) };
        self.start()
    }
}

