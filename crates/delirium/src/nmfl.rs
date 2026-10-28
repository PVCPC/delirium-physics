use crate::prelude::*;
use crate::Vx;
use crate::Fma;

use crate::nmdbm::Dbm;
use crate::nmhl::HeadLook;
use crate::nmrs::{Character, CLAVICLE, ELBOW, HIP, KNEE, NECK, SHOULDER, SPINE, WRIST};
use crate::nmshot::{limb, Ctx, Shot};
use crate::nmst::SpineTwist;

#[allow(non_snake_case)]
#[inline]
fn SPINE3() -> usize { delirium_data::kv!(77 int) }

#[derive(Clone, Debug)]
pub struct Flinch {
    pub pos: Vec3,
    pub hand_distance_vertical: f32,
    pub hand_distance_left_right: f32,
    pub hand_distance_front_back: f32,
    pub body_stiffness: f32,
    pub body_damping: f32,
    pub back_bend_amount: f32,
    pub use_right_arm: bool,
    pub use_left_arm: bool,
    pub noise_scale: f32,
    pub new_hit: bool,
    pub turn_towards: i32,
    turn: bool,
    dir: Vec3,
    t_behind: f32,
    t_front: f32,
    leg_offset: f32,
    bearing: f32,
    noise_at: f32,
    t_hands: f32,
    t_ramp: f32,
    ramp: f32,
    ms: f32,
    pub active: bool,
    pub hands_off: bool,
    behind: bool,
    right: bool,
}

impl Default for Flinch {
    fn default() -> Flinch {
        Flinch { pos: Vec3::ZERO, hand_distance_vertical: delirium_data::kv!(73), hand_distance_left_right: delirium_data::kv!(73), hand_distance_front_back: delirium_data::kv!(136),
                 body_stiffness: delirium_data::kv!(107), body_damping: 1.0, back_bend_amount: -delirium_data::kv!(123), use_right_arm: true, use_left_arm: true,
                 noise_scale: delirium_data::kv!(73), new_hit: true, turn_towards: 1, turn: true, dir: Vec3::ZERO, t_behind: 0.0, t_front: 0.0,
                 leg_offset: 0.0, bearing: 0.0, noise_at: 0.0, t_hands: 0.0, t_ramp: 0.0, ramp: delirium_data::kv!(106), ms: delirium_data::kv!(48), active: false,
                 hands_off: false, behind: false, right: false }
    }
}

#[allow(non_snake_case)]
#[inline]
fn PERM() -> &'static [u8; 256] { static V: std::sync::OnceLock<[u8; 256]> = std::sync::OnceLock::new(); let f = || [
    delirium_data::kv!(275 int), delirium_data::kv!(84 int), delirium_data::kv!(276 int), delirium_data::kv!(277 int), delirium_data::kv!(278 int), delirium_data::kv!(191 int), delirium_data::kv!(279 int), delirium_data::kv!(103 int), delirium_data::kv!(280 int), delirium_data::kv!(281 int), delirium_data::kv!(282 int), delirium_data::kv!(283 int), delirium_data::kv!(284 int), delirium_data::kv!(285 int), delirium_data::kv!(22 int), delirium_data::kv!(286 int), delirium_data::kv!(287 int), delirium_data::kv!(90 int), delirium_data::kv!(288 int), delirium_data::kv!(274 int), delirium_data::kv!(289 int), delirium_data::kv!(290 int), delirium_data::kv!(23 int), delirium_data::kv!(291 int), delirium_data::kv!(292 int),
    delirium_data::kv!(293 int), delirium_data::kv!(100 int), delirium_data::kv!(77 int), delirium_data::kv!(95 int), delirium_data::kv!(294 int), delirium_data::kv!(6 int), delirium_data::kv!(295 int), delirium_data::kv!(296 int), delirium_data::kv!(297 int), delirium_data::kv!(298 int), delirium_data::kv!(299 int), 0, delirium_data::kv!(98 int), delirium_data::kv!(300 int), delirium_data::kv!(301 int), delirium_data::kv!(302 int), delirium_data::kv!(303 int), delirium_data::kv!(304 int), delirium_data::kv!(305 int), delirium_data::kv!(306 int), delirium_data::kv!(307 int), delirium_data::kv!(102 int), delirium_data::kv!(7 int), delirium_data::kv!(308 int), delirium_data::kv!(309 int),
    delirium_data::kv!(310 int), delirium_data::kv!(311 int), delirium_data::kv!(312 int), delirium_data::kv!(313 int), delirium_data::kv!(314 int), delirium_data::kv!(315 int), delirium_data::kv!(316 int), delirium_data::kv!(118 int), delirium_data::kv!(317 int), delirium_data::kv!(318 int), delirium_data::kv!(319 int), delirium_data::kv!(320 int), delirium_data::kv!(321 int), delirium_data::kv!(322 int), delirium_data::kv!(323 int), delirium_data::kv!(324 int), delirium_data::kv!(325 int), delirium_data::kv!(326 int), delirium_data::kv!(327 int), delirium_data::kv!(89 int), delirium_data::kv!(328 int), delirium_data::kv!(329 int), delirium_data::kv!(330 int), delirium_data::kv!(331 int),
    delirium_data::kv!(332 int), delirium_data::kv!(333 int), delirium_data::kv!(334 int), delirium_data::kv!(335 int), delirium_data::kv!(336 int), delirium_data::kv!(337 int), delirium_data::kv!(338 int), delirium_data::kv!(339 int), delirium_data::kv!(340 int), delirium_data::kv!(341 int), delirium_data::kv!(342 int), delirium_data::kv!(343 int), delirium_data::kv!(344 int), delirium_data::kv!(345 int), delirium_data::kv!(346 int), delirium_data::kv!(347 int), delirium_data::kv!(348 int), delirium_data::kv!(91 int), delirium_data::kv!(349 int), delirium_data::kv!(350 int), delirium_data::kv!(351 int), delirium_data::kv!(34 int), delirium_data::kv!(352 int), delirium_data::kv!(97 int),
    delirium_data::kv!(250 int), delirium_data::kv!(353 int), 1, delirium_data::kv!(354 int), delirium_data::kv!(355 int), delirium_data::kv!(356 int), delirium_data::kv!(357 int), delirium_data::kv!(358 int), delirium_data::kv!(359 int), delirium_data::kv!(360 int), delirium_data::kv!(361 int), delirium_data::kv!(362 int), delirium_data::kv!(206 int), delirium_data::kv!(363 int), delirium_data::kv!(85 int), delirium_data::kv!(364 int), delirium_data::kv!(365 int), delirium_data::kv!(366 int), delirium_data::kv!(367 int), delirium_data::kv!(368 int), delirium_data::kv!(369 int), delirium_data::kv!(370 int), delirium_data::kv!(371 int), delirium_data::kv!(189 int),
    delirium_data::kv!(372 int), delirium_data::kv!(373 int), delirium_data::kv!(374 int), delirium_data::kv!(375 int), delirium_data::kv!(0 int), delirium_data::kv!(251 int), delirium_data::kv!(376 int), delirium_data::kv!(377 int), delirium_data::kv!(378 int), delirium_data::kv!(36 int), delirium_data::kv!(379 int), delirium_data::kv!(380 int), delirium_data::kv!(2 int), delirium_data::kv!(381 int), delirium_data::kv!(382 int), delirium_data::kv!(383 int), delirium_data::kv!(384 int), delirium_data::kv!(385 int), delirium_data::kv!(386 int), delirium_data::kv!(387 int), delirium_data::kv!(388 int), delirium_data::kv!(389 int), delirium_data::kv!(390 int), delirium_data::kv!(391 int),
    delirium_data::kv!(392 int), delirium_data::kv!(393 int), delirium_data::kv!(394 int), delirium_data::kv!(51 int), delirium_data::kv!(395 int), delirium_data::kv!(99 int), delirium_data::kv!(396 int), delirium_data::kv!(397 int), delirium_data::kv!(81 int), delirium_data::kv!(398 int), delirium_data::kv!(399 int), delirium_data::kv!(400 int), delirium_data::kv!(401 int), delirium_data::kv!(402 int), delirium_data::kv!(403 int), delirium_data::kv!(404 int), delirium_data::kv!(405 int), 2, delirium_data::kv!(406 int), delirium_data::kv!(407 int), delirium_data::kv!(408 int), delirium_data::kv!(409 int), delirium_data::kv!(410 int), delirium_data::kv!(411 int),
    delirium_data::kv!(412 int), delirium_data::kv!(413 int), delirium_data::kv!(414 int), delirium_data::kv!(415 int), delirium_data::kv!(416 int), delirium_data::kv!(93 int), delirium_data::kv!(417 int), delirium_data::kv!(94 int), delirium_data::kv!(418 int), delirium_data::kv!(419 int), delirium_data::kv!(192 int), delirium_data::kv!(420 int), delirium_data::kv!(421 int), delirium_data::kv!(422 int), delirium_data::kv!(423 int), delirium_data::kv!(424 int), delirium_data::kv!(425 int), delirium_data::kv!(426 int), delirium_data::kv!(427 int), delirium_data::kv!(428 int), delirium_data::kv!(429 int), delirium_data::kv!(430 int), delirium_data::kv!(431 int), delirium_data::kv!(432 int),
    delirium_data::kv!(433 int), delirium_data::kv!(88 int), delirium_data::kv!(434 int), delirium_data::kv!(435 int), delirium_data::kv!(436 int), delirium_data::kv!(437 int), delirium_data::kv!(438 int), delirium_data::kv!(439 int), delirium_data::kv!(83 int), delirium_data::kv!(52 int), delirium_data::kv!(440 int), delirium_data::kv!(441 int), delirium_data::kv!(442 int), delirium_data::kv!(443 int), delirium_data::kv!(444 int), delirium_data::kv!(445 int), delirium_data::kv!(446 int), delirium_data::kv!(447 int), delirium_data::kv!(448 int), delirium_data::kv!(53 int), delirium_data::kv!(449 int), delirium_data::kv!(450 int), delirium_data::kv!(451 int), delirium_data::kv!(452 int),
    delirium_data::kv!(453 int), delirium_data::kv!(454 int), delirium_data::kv!(455 int), delirium_data::kv!(456 int), delirium_data::kv!(457 int), delirium_data::kv!(458 int), delirium_data::kv!(459 int), delirium_data::kv!(460 int), delirium_data::kv!(461 int), delirium_data::kv!(87 int), delirium_data::kv!(462 int), delirium_data::kv!(463 int), delirium_data::kv!(33 int), delirium_data::kv!(464 int), delirium_data::kv!(465 int), delirium_data::kv!(1 int), delirium_data::kv!(466 int), delirium_data::kv!(467 int), delirium_data::kv!(468 int), delirium_data::kv!(86 int), delirium_data::kv!(469 int), delirium_data::kv!(470 int), delirium_data::kv!(471 int), delirium_data::kv!(472 int),
    delirium_data::kv!(473 int), delirium_data::kv!(474 int), delirium_data::kv!(96 int), delirium_data::kv!(475 int), delirium_data::kv!(476 int), delirium_data::kv!(477 int), delirium_data::kv!(249 int), delirium_data::kv!(478 int), delirium_data::kv!(479 int), delirium_data::kv!(480 int), delirium_data::kv!(481 int), delirium_data::kv!(482 int), delirium_data::kv!(483 int), delirium_data::kv!(484 int),
]; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

const GRAD: [[f32; 3]; 16] = [
    [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, -1.0, 0.0], [1.0, 0.0, 1.0], [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0], [-1.0, 0.0, -1.0], [0.0, 1.0, 1.0], [0.0, 1.0, -1.0], [0.0, -1.0, 1.0], [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0], [0.0, -1.0, 1.0], [0.0, -1.0, -1.0],
];

fn noise3(x: f32, y: f32, z: f32) -> f32 {
    let p = |i: usize| (*PERM())[i & delirium_data::kv!(386 int)] as usize;
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (xi, yi, zi) = (fx as i32 as usize & delirium_data::kv!(386 int), fy as i32 as usize & delirium_data::kv!(386 int), fz as i32 as usize & delirium_data::kv!(386 int));
    let (x, y, z) = (x - fx, y - fy, z - fz);
    let fade = |t: f32| t.mul_add(delirium_data::kv!(32), -delirium_data::kv!(26)).mul_add(t, delirium_data::kv!(27)) * (t * (t * t));
    let (u, v, w) = (fade(x), fade(y), fade(z));
    let g = |h: usize, x: f32, y: f32, z: f32| {
        let k = GRAD[h & delirium_data::kv!(191 int)];
        k[0] * x + k[1] * y + k[2] * z
    };
    let lerp = |t: f32, a: f32, b: f32| crate::Fma::fma(b - a, t, a);
    let (a, b) = (p(xi) + yi, p(xi + 1) + yi);
    let (aa, ab, ba, bb) = (p(a) + zi, p(a + 1) + zi, p(b) + zi, p(b + 1) + zi);
    lerp(w,
         lerp(v, lerp(u, g(p(aa), x, y, z), g(p(ba), x - 1.0, y, z)),
              lerp(u, g(p(ab), x, y - 1.0, z), g(p(bb), x - 1.0, y - 1.0, z))),
         lerp(v, lerp(u, g(p(aa + 1), x, y, z - 1.0), g(p(ba + 1), x - 1.0, y, z - 1.0)),
              lerp(u, g(p(ab + 1), x, y - 1.0, z - 1.0), g(p(bb + 1), x, y, z))))
}

pub(crate) fn noise01(x: f32, y: f32, z: f32) -> f32 {
    (noise3(x, y, z) + 0.5).clamp(0.0, 1.0)
}

impl Flinch {
    pub fn activate(&mut self, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist) {
        if self.active {
            return;
        }
        (self.active, self.right) = (true, true);
        let ch = &mut *c.ch;
        self.leg_offset = delirium_data::kv!(73) * ch.random() - delirium_data::kv!(73);
        self.noise_at = delirium_data::kv!(236) * ch.random();
        self.turn = ch.random_bits() & 1 != 0;
        for j in Character::mask("ub") {
            ch.eff[j].oppose_gravity = 1.0;
        }
        st.vel = Vec3::ZERO;
        st.activate(ch);
        head.vel = Vec3::ZERO;
        head.activate(ch, c.b);
        self.aim(c, head, st);
        self.stiffen(c.ch);
    }

    pub fn deactivate(&mut self, ch: &mut Character, head: &mut HeadLook, st: &mut SpineTwist) {
        if !self.active {
            return;
        }
        for j in Character::mask("ub") {
            ch.eff[j].reset(&ch.joints[j]);
        }
        *self = Flinch::default();
        st.deactivate();
        head.deactivate();
    }

    pub fn tick(&mut self, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist, dbm: Option<&mut Dbm>, dbm_failed: bool, dt: f32) {
        if !self.active {
            return;
        }
        if self.hands_off {
            self.t_hands -= dt;
            if self.t_hands <= 0.0 {
                self.hands_off = false;
            }
        }
        self.aim(c, head, st);
        let dbm_on = dbm.is_some();
        if let Some(d) = dbm {
            let mut v = self.pos - c.com;
            if self.bearing.abs() > delirium_data::kv!(169) {
                v = c.m.a * if self.bearing > 0.0 { 1.0 } else if self.bearing < 0.0 { -1.0 } else { 0.0 };
            }
            let n = v.normalize_or_zero();
            if self.turn && self.turn_towards != 0 {
                (d.pv.fixed_vel, d.pv.vel) = (true, (n * self.turn_towards as f32).normalize_or_zero());
            } else {
                (d.pv.fixed_vel, d.pv.vel) = (false, n);
            }
            d.leg_offset = if self.behind { self.leg_offset } else { 0.0 };
        }
        if self.new_hit {
            self.stiffen(c.ch);
            self.new_hit = false;
        } else {
            self.soften(c.ch, dt);
        }
        if self.behind {
            (self.t_behind, self.t_front) = (self.t_behind + dt, 0.0);
            self.turned_away(c, !dbm_on && dbm_failed);
        } else {
            (self.t_behind, self.t_front) = (0.0, self.t_front + dt);
            self.front(c);
        }
    }

    fn swap_side(&mut self) {
        (self.hands_off, self.t_hands) = (true, delirium_data::kv!(12));
    }

    fn aim(&mut self, c: &mut Ctx, head: &mut HeadLook, st: &mut SpineTwist) {
        let up = c.up;
        let r = self.pos - c.com;
        self.dir = crate::rigid::nrm(up * -crate::nmdbm::sdot(up, r, b"yzx") + r);
        let loc = crate::rigid::to_frame([c.m.a, c.m.b, c.m.c, c.m.d], self.pos);
        self.bearing = crate::nmdbm::atan2f(loc.x, -loc.z);
        if self.bearing.abs() < delirium_data::kv!(12) {
            self.behind = false;
        } else if self.bearing.abs() > delirium_data::kv!(485) {
            self.behind = true;
        }
        if self.bearing > delirium_data::kv!(106) {
            if !self.right {
                self.swap_side();
            }
            self.right = true;
        } else if self.bearing < -delirium_data::kv!(106) {
            if self.right {
                self.swap_side();
            }
            self.right = false;
        }
        let wob = noise01(c.ch.agent as f32, (self.t_behind + self.t_front) * delirium_data::kv!(18), self.noise_at * 0.5) - 0.5;
        let side = self.dir.vx(up) * (wob * delirium_data::kv!(18));
        st.pos = if self.behind { c.b.at(SPINE3()) - self.dir } else { self.pos } + side;
        st.stiffness = self.body_stiffness;
        let look = if self.behind { (c.b.at(limb(2).p[delirium_data::kv!(0 int)]) + c.b.at(limb(delirium_data::kv!(0 int)).p[delirium_data::kv!(0 int)])) * 0.5 } else { self.pos };
        head.pos = look - side * delirium_data::kv!(18);
        (head.stiffness, head.damping) = (self.body_stiffness, self.body_damping);
    }

    fn stiffen(&mut self, ch: &mut Character) {
        self.t_ramp = 0.0;
        let s = self.body_stiffness * 0.5;
        ch.set_stiffness(s, 0.5, "us", Some(delirium_data::kv!(48)));
        ch.set_stiffness(s, 0.5, "ua", Some(delirium_data::kv!(48)));
        ch.set_stiffness(s, delirium_data::kv!(24), "uw", Some(delirium_data::kv!(48)));
        ch.set_stiffness(s, delirium_data::kv!(24), "un", Some(delirium_data::kv!(48)));
    }

    fn soften(&mut self, ch: &mut Character, dt: f32) {
        self.t_ramp = (self.t_ramp + dt).min(self.ramp);
        let x = (1.0 - (self.t_ramp + delirium_data::kv!(4)) / (self.ramp + delirium_data::kv!(4))).clamp(0.0, 1.0);
        for j in (*SPINE()) {
            ch.eff[j].oppose_gravity = x + 1.0;
        }
        for j in (*NECK()).into_iter().chain([0, 1].into_iter().flat_map(|i| [(*CLAVICLE())[i], (*SHOULDER())[i], (*ELBOW())[i]])) {
            ch.eff[j].oppose_gravity = 1.0;
        }
        let f29 = (self.ms + 1.0) * 0.5;
        let s = (1.0 - x * 0.5) * self.body_stiffness;
        let f25 = (1.0 - f29) * 0.5 + f29;
        let cap = delirium_data::kv!(27) / (dt * delirium_data::kv!(193)) + delirium_data::kv!(27);
        ch.set_stiffness(s, f25 * 2.0, "us", Some(f29 * delirium_data::kv!(74)));
        ch.set_stiffness(s, f25 * 2.0, "ua", Some(f29 * 0.5));
        ch.set_stiffness(s, f25 * 2.0, "uc", Some(f25 * delirium_data::kv!(74)));
        ch.set_stiffness(s.min(cap), 2.0, "uw", Some(f25 * 0.5));
        ch.set_stiffness(s.min(cap), f29 * 2.0, "un", Some(f29 * 0.5));
        self.ms = dt.fma(delirium_data::kv!(215), self.ms).min(1.0);
    }

    fn front(&mut self, c: &mut Ctx) {
        let up = c.up;
        let t = self.t_front * delirium_data::kv!(18);
        let h = (crate::nmdbm::sdot(self.pos, up, b"yzx") - crate::nmdbm::sdot(c.com, up, b"yzx")).clamp(-0.5, 2.0);
        let agent = c.ch.agent as f32;
        let n1 = (noise01(t, self.noise_at, agent) - 0.5) * self.noise_scale;
        let n2 = (noise01(t, agent, self.noise_at) - 0.5) * self.noise_scale;
        let n3 = (noise01(agent, t, self.noise_at) - 0.5) * self.noise_scale;
        let side = self.dir.vx(up) * (self.hand_distance_left_right + n1);
        let rise = up * h;
        let fwd = crate::rigid::nrm(self.dir + rise) * delirium_data::kv!(486);
        let mut vert = crate::rigid::nrm(rise) * (self.hand_distance_vertical + n2);
        let mut fb = fwd * (n3 - self.hand_distance_front_back);
        let centre = c.b.at(SPINE3()) + fwd;
        if self.right {
            (vert, fb) = (-vert, -fb);
        }
        let (tl, tr) = (centre + side + fb + vert, centre - side - fb - vert);
        let ch = &mut *c.ch;
        let b = self.bearing.clamp(-1.0, 1.0);
        let (l1, l2) = ((1.0 - b.abs()) * self.back_bend_amount, self.back_bend_amount * b * -0.5);
        for (k, j) in (*SPINE()).into_iter().enumerate() {
            ch.eff[j].desired[0] = l1 * [delirium_data::kv!(73), delirium_data::kv!(57), delirium_data::kv!(24), delirium_data::kv!(106)][k];
            ch.eff[j].desired[1] = l2 * [delirium_data::kv!(57), delirium_data::kv!(24), delirium_data::kv!(24), delirium_data::kv!(57)][k];
        }
        if self.t_front < delirium_data::kv!(73) {
            for (i, on, l1, elbow, w1) in [(1, self.use_right_arm, delirium_data::kv!(115), delirium_data::kv!(24), delirium_data::kv!(73)), (0, self.use_left_arm, delirium_data::kv!(487), delirium_data::kv!(110), delirium_data::kv!(57))] {
                if on {
                    let e = &mut ch.eff[(*SHOULDER())[i]].desired;
                    (e[2], e[0], e[1]) = (-delirium_data::kv!(12), l1, -0.5);
                    ch.eff[(*ELBOW())[i]].desired[0] = elbow;
                    (ch.eff[(*WRIST())[i]].desired[1], ch.eff[(*WRIST())[i]].desired[0]) = (0.0, w1);
                    ch.eff[(*WRIST())[i]].blend_to_zero(delirium_data::kv!(63));
                }
            }
            return;
        }
        let n = crate::rigid::nrm_or_y(c.b.at(SPINE3()) - self.pos);
        let mut yaw = delirium_data::kv!(64);
        if !c.ch.busy[0] && self.use_left_arm {
            let l2 = if self.right {
                yaw = -delirium_data::kv!(73);
                -delirium_data::kv!(24)
            } else {
                -0.5
            };
            Self::reach(c, 0, tl, n, delirium_data::kv!(64), l2, 1.0);
        }
        if !c.ch.busy[1] && self.use_right_arm {
            let (yaw, l2) = if self.right { (yaw, -0.5) } else { (-delirium_data::kv!(73), -delirium_data::kv!(24)) };
            Self::reach(c, 1, tr, n, yaw, l2, -1.0);
            c.ch.eff[(*WRIST())[1]].blend_to_zero(delirium_data::kv!(17));
        }
    }

    fn reach(c: &mut Ctx, i: usize, t: Vec3, n: Vec3, yaw: f32, clav_lean2: f32, side: f32) {
        let l = limb(i);
        crate::nmdbm::limb_ik(c.b, c.ch, l.e[1], l.e[2], l.p[0], l.p[delirium_data::kv!(0 int)], t, yaw, 0.0, side, 1.0, Vec3::ZERO, None);
        crate::nmcf::clav_follow(c.ch, l.e[0], l.e[1]);
        c.ch.eff[l.e[0]].desired[1] = clav_lean2;
        Shot::wrist(c, l, t, n, side);
        let tw = c.ch.actual(l.e[delirium_data::kv!(0 int)], c.b.frames)[2];
        (c.ch.eff[l.e[delirium_data::kv!(0 int)]].desired[0], c.ch.eff[l.e[delirium_data::kv!(0 int)]].desired[1]) = ((tw as f64).sin() as f32, -((tw as f64).cos() as f32));
    }

    fn turned_away(&mut self, c: &mut Ctx, fallen: bool) {
        let ch = &mut *c.ch;
        for (i, on) in [(1, self.use_right_arm), (0, self.use_left_arm)] {
            if on {
                (ch.eff[(*CLAVICLE())[i]].desired[0], ch.eff[(*CLAVICLE())[i]].desired[1]) = (-delirium_data::kv!(106), -0.5);
            }
        }
        let s = (crate::nmdbm::sdot(c.m.a, crate::rigid::nrm(self.pos - c.com), b"xyz") as f64).sin() as f32;
        for (k, j) in (*SPINE()).into_iter().enumerate() {
            ch.eff[j].desired[0] = [delirium_data::kv!(488 f32), delirium_data::kv!(489 f32), delirium_data::kv!(490), delirium_data::kv!(491 f32)][k];
            ch.eff[j].desired[1] = s * [delirium_data::kv!(57), delirium_data::kv!(24), delirium_data::kv!(24), delirium_data::kv!(57)][k];
        }
        let late = self.t_behind > delirium_data::kv!(492);
        let arms = [(1, self.use_right_arm, if late { (delirium_data::kv!(493), delirium_data::kv!(494)) } else { (1.0, delirium_data::kv!(24)) }, (delirium_data::kv!(70), 0.5)),
                    (0, self.use_left_arm, if late { (delirium_data::kv!(495), delirium_data::kv!(496)) } else { (1.0, delirium_data::kv!(110)) }, (delirium_data::kv!(110), delirium_data::kv!(106)))];
        for (i, on, (l1, elbow), (w2, w1)) in arms {
            if on {
                ch.eff[(*SHOULDER())[i]].desired = [l1, -0.5, -delirium_data::kv!(12)];
                ch.eff[(*ELBOW())[i]].desired[0] = elbow;
                (ch.eff[(*WRIST())[i]].desired[1], ch.eff[(*WRIST())[i]].desired[0]) = (w2, w1);
            }
        }
        if fallen {
            for i in 0..2 {
                ch.eff[(*HIP())[i]].desired[0] = delirium_data::kv!(12);
                ch.eff[(*KNEE())[i]].desired[0] = -2.0;
            }
        }
    }
}

