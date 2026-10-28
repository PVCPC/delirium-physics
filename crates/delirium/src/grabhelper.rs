use crate::prelude::*;
use crate::msg::Value;
use crate::shottask::Msg;
use crate::{Fma, Vd};

#[allow(non_snake_case)]
#[inline]
fn BLOCK() -> &'static [f32; 7] { static V: std::sync::OnceLock<[f32; 7]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(167), delirium_data::kv!(27), delirium_data::kv!(168), delirium_data::kv!(169), delirium_data::kv!(170), 0.0, 1.0]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

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
        GrabHelper { strength: (*BLOCK())[0], ..Default::default() }
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
        self.flags & delirium_data::kv!(171 int) != 0
    }

    pub fn start(&mut self) -> Option<Msg> {
        use Value::{Bool as B, Float as F, Int as I, Vec3 as V};
        if self.pos1 == Vec3::ZERO && self.pos2 == Vec3::ZERO {
            return None;
        }
        let mut p = vec![];
        if self.flags & delirium_data::kv!(172 int) == 0 {
            p.push(("start", B(true)));
            self.flags |= delirium_data::kv!(172 int);
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
        if self.flags & delirium_data::kv!(23 int) != 0 {
            p.push(("justbrace", B(true)));
        }
        if self.flags & delirium_data::kv!(51 int) != 0 {
            p.push(("linegrab", B(true)));
        } else if self.flags & delirium_data::kv!(7 int) != 0 {
            p.push(("surfaceGrab", B(true)));
        }
        p.extend([("bodyStiffness", F((*BLOCK())[1])), ("grabStrength", F(self.strength)), ("grabDist", F((*BLOCK())[2])),
                  ("reachAngle", F((*BLOCK())[delirium_data::kv!(0 int)])), ("oneSideReachAngle", F((*BLOCK())[delirium_data::kv!(1 int)])), ("dontLetGo", B(true)),
                  ("pullUpStrengthLeft", F((*BLOCK())[delirium_data::kv!(2 int)])), ("pullUpStrengthRight", F((*BLOCK())[delirium_data::kv!(2 int)])), ("pullUpTime", F((*BLOCK())[delirium_data::kv!(6 int)]))]);
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
            self.flags |= if !success { delirium_data::kv!(173 int) } else if left { delirium_data::kv!(174 int) } else { delirium_data::kv!(175 int) };
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
        #[allow(non_snake_case)]
        #[inline]
        fn SPEED() -> &'static [f32; 2] { static V: std::sync::OnceLock<[f32; 2]> = std::sync::OnceLock::new(); let f = || [delirium_data::kv!(24), 0.5]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
        #[allow(non_snake_case)]
        #[inline]
        fn REACH() -> f32 { delirium_data::kv!(24) }
        const LIFT: f32 = 0.0;
        let sum = i.vel[0] + i.vel[1];
        let fast = |t: f32| sum.x.fma(sum.x, sum.y.fma(sum.y, sum.z * sum.z)) > t * t;
        let mut go = false;
        if self.flags & 2 != 0 && self.flags & delirium_data::kv!(176 int) != 0 {
            go |= fast((*SPEED())[0]);
        }
        if self.flags & delirium_data::kv!(1 int) != 0 && self.flags & delirium_data::kv!(177 int) != 0 {
            go |= fast((*SPEED())[1]);
        }
        if !go || self.flags & 1 != 0 {
            return None;
        }
        let start = i.hand_pos + Vec3::new(0.0, 0.0, LIFT);
        let end = Vec3::new(i.hand_dir.x.fma(REACH(), start.x), i.hand_dir.y.fma(REACH(), start.y), i.hand_dir.z.fma(REACH(), start.z));
        let hit = probe(end)?;
        let (mut point, mut normal, mut held) = (hit.point, hit.normal, None);
        if let Some(e) = hit.entity.filter(|e| (2..delirium_data::kv!(2 int)).contains(&e.kind)) {
            let d = hit.point - e.rows[delirium_data::kv!(0 int)];
            let local = |v: Vec3| Vec3::new(e.rows[0].vd(v), e.rows[1].vd(v), e.rows[2].vd(v));
            (point, normal) = (local(d), local(hit.normal));
            if !(e.kind == delirium_data::kv!(1 int) && e.b555 != 0) {
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

