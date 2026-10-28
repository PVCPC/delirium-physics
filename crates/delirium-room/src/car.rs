use bevy::prelude::*;
use delirium_data::{damage::Health, suspension::{self, Spring}, transmission::{Controls, Transmission}, tyre::{self, Contact, Tyre}, vehicles,
                wheelstep::{self, Out, Strut, Times, Tread, V4}};
use delirium::collision::Collision;

#[allow(non_snake_case)]
#[inline]
fn GRAVITY() -> f32 { delirium_data::kv!(31) }
const CORNERS: [(f32, f32); 4] = [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
const AXLE: V4 = [1.0, 0.0, 0.0, 0.0];
const BUMP: f32 = 1.0;
#[allow(non_snake_case)]
#[inline]
fn BODY_BOUNCE() -> f32 { delirium_data::kv!(73) }
const BODY_FRICTION: f32 = 0.5;

const DEFAULT_HANDLING: &str = "OURS 1500.0 10.0 80 0.0 0.0 -0.1 0.5 5 0.2 1.0 150.0 0.3 0.7 0.7 35.0 1.2 0.9 14.0 0.15 0.5 1.8 1.0 1.0 0.15 -0.12 0.0 0.5 0.7 1.0 0.7 1.5 0.0 30000 440010 0 0";

#[derive(Clone)]
pub struct Tune {
    handling: vehicles::Handling,
    inertia: Vec3,
    surfaces: std::rc::Rc<Vec<[f32; 2]>>,
    ground: std::rc::Rc<Vec<u32>>,
    damping: [[[f32; 3]; 3]; 2],
    rim: Option<[f32; 2]>,
}

impl Tune {
    pub fn new(model: &str, handling: Option<&str>, ide: Option<&str>, materials: Option<&str>, size: Vec3) -> Self {
        let ide = ide.map(vehicles::read_ide_handling).and_then(|m| m.get(model).cloned());
        let h = handling.map(vehicles::read_handling).zip(ide.as_ref()).and_then(|(mut t, (id, _))| t.remove(id))
            .unwrap_or_else(|| vehicles::read_handling(DEFAULT_HANDLING).remove("OURS").unwrap());
        let s = size * size;
        Tune {
            damping: [[[delirium_data::kv!(48); 3], [delirium_data::kv!(48); 3], [h.drag; 3]], [[0.0; 3], [delirium_data::kv!(64); 3], [0.0; 3]]],
            inertia: Vec3::new(s.y + s.z, s.x + s.z, s.x + s.y) / delirium_data::kv!(21),
            surfaces: materials.map(vehicles::read_materials).unwrap_or_default().into(),
            ground: materials.map(|t| vehicles::read_material_names(t).iter().map(|n| suspension::ground_flags(n)).collect::<Vec<u32>>()).unwrap_or_default().into(),
            rim: ide.map(|(_, r)| r),
            handling: h,
        }
    }
}

#[derive(Clone, Copy)]
struct Probe {
    p: Vec3,
    n: Vec3,
    x: f32,
    prev: f32,
}

struct Wheels<'a> {
    h: &'a vehicles::Handling,
    mass: wheelstep::Mass,
    struts: [Strut; 4],
    treads: [Tread; 4],
    axes: [[V4; 2]; 4],
    base: [Tyre; 4],
    tyres: [Tyre; 4],
}

impl wheelstep::Calls for Wheels<'_> {
    fn advance(&mut self, m: &[V4; 4], vel: V4, ang: V4, h: f32) -> [V4; 4] {
        wheelstep::advance(m, vel, ang, h)
    }

    fn suspension(&mut self, i: usize, m: &[V4; 4], vel: V4, _ang: V4, t: &Times) -> Out {
        wheelstep::suspension(self.h, &self.mass, &self.struts[i], m, vel, t)
    }

    fn tyre(&mut self, i: usize, load: f32, slip: f32, timer: f32, m: &[V4; 4], vel: V4, ang: V4, t: &Times) -> Out {
        let (h, b, s, cache) = (self.h, &self.mass, &self.struts[i], &mut self.axes[i]);
        let mut ty = self.base[i].clone();
        let r = Tread { no_brake: timer, ..self.treads[i] };
        let o = wheelstep::tyre(h, b, s, &r, load, slip, m, vel, ang, t,
                                |steer| wheelstep::axes(cache, AXLE, s.normal, s.flags, &b.m, steer),
                                |c| tyre::tyre_force(h, &mut ty, b.mass, b.inv_mass, c));
        self.tyres[i] = ty;
        o
    }
}

fn damping<const N: usize>(v: [f32; N], [c, l, q]: [[f32; N]; 3], dt: f32) -> [f32; N] {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    std::array::from_fn(|i| {
        let x = v[i];
        let d = if x > 0.0 { c[i] } else { -c[i] } + l[i] * x + q[i] * x * n;
        if d.abs() * dt > x.abs() { x / dt } else { d }
    })
}

pub fn player_pedals(accel: f32, brake: f32, speed: f32, last_throttle: f32) -> (f32, f32) {
    let d = accel - brake;
    if accel > delirium_data::kv!(110) && brake > delirium_data::kv!(110) {
        (accel, brake)
    } else if speed.abs() < 0.5 {
        (d, 0.0)
    } else if speed >= 0.0 {
        if d >= 0.0 { (d, 0.0) } else { (0.0, -d) }
    } else if d < 0.0 || (last_throttle > 0.5 && speed > -delirium_data::kv!(27)) {
        (d, 0.0)
    } else {
        (0.0, d)
    }
}

fn merge(hits: &[(Vec3, Vec3)]) -> Option<(Vec3, Vec3)> {
    let w: f32 = hits.iter().map(|(_, j)| j.length()).sum();
    (w > 0.0).then(|| (hits.iter().map(|(q, j)| *q * j.length()).sum::<Vec3>() / w, hits.iter().map(|(_, j)| *j).sum()))
}

pub struct Car {
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    pub speed: f32,
    pub ang: Vec3,
    tyres: [Tyre; 4],
    treads: [Tread; 4],
    springs: [Spring; 4],
    comp: [f32; 4],
    steer_input: f32,
    pub steer: f32,
    throttle: f32,
    hub_z: f32,
    com: Vec3,
    pub radius: f32,
    rim: [f32; 2],
    pub axle: Vec2,
    hull: [Vec3; 2],
    prev: Option<(Vec3, Quat)>,
    random: tyre::Random,
    tune: Tune,
    gearbox: Transmission,
    clock: f64,
    safe: (Vec3, f32),
    pub player: bool,
    pub asleep: bool,
    still: f32,
    pub health: Health,
    hit: (Vec3, Vec3),
    by_ped: bool,
    pub braced: bool,
}

impl Car {
    pub fn new(tune: Tune, ground: Vec3, heading: f32, radius: f32, axle: Vec2, hull: [Vec3; 2], world: &Collision) -> Self {
        let h = &tune.handling;
        let com = Vec3::from(h.centre_of_mass);
        let lean = (com.y / axle.y).clamp(-delirium_data::kv!(108), delirium_data::kv!(108));
        let springs = CORNERS.map(|(_, y)| {
            let front = y > 0.0;
            Spring::new(h, front, delirium_data::kv!(17) * (1.0 + if front { lean } else { -lean }), radius)
        });
        let comp = springs.map(|s| s.travel - s.top(h) - radius);
        let mut c = Car {
            pos: ground + Vec3::Z * radius,
            rot: Quat::from_rotation_z(heading),
            vel: Vec3::ZERO,
            speed: 0.0,
            ang: Vec3::ZERO,
            tyres: springs.map(|s| Tyre::new(s.front, s.share)),
            treads: [Tread::default(); 4],
            springs,
            comp,
            steer_input: 0.0,
            steer: 0.0,
            throttle: 0.0,
            hub_z: 0.0,
            com,
            radius,
            rim: tune.rim.map_or([radius - delirium_data::kv!(73); 2], |r| r.map(|r| r.min(radius))),
            axle,
            hull,
            prev: None,
            random: tyre::Random::new(delirium_data::kv!(973 int) ^ ground.x.to_bits() ^ ground.y.to_bits().rotate_left(delirium_data::kv!(51 int))),
            tune,
            gearbox: Transmission::default(),
            clock: 0.0,
            safe: (ground, heading),
            player: false,
            asleep: false,
            still: 0.0,
            health: Health::default(),
            hit: (Vec3::ZERO, Vec3::ZERO),
            by_ped: false,
            braced: false,
        };
        c.place(world);
        c
    }

    fn place(&mut self, world: &Collision) {
        let heading = self.heading();
        let flat = Quat::from_rotation_z(heading);
        let z = CORNERS.map(|(x, y)| {
            let p = self.pos + flat * Vec3::new(x * self.axle.x, y * self.axle.y, 0.0);
            world.ground(p + Vec3::Z * 2.0, delirium_data::kv!(60)).unwrap_or(self.pos.z - (self.radius - self.hub_z))
        });
        let ([fl, fr], [rl, rr]) = ([z[0], z[1]], [z[2], z[delirium_data::kv!(0 int)]]);
        let pitch = f32::atan2((fl + fr - rl - rr) * 0.5, 2.0 * self.axle.y);
        let roll = f32::atan2((fl + rl - fr - rr) * 0.5, 2.0 * self.axle.x);
        self.rot = flat * Quat::from_rotation_x(pitch) * Quat::from_rotation_y(roll);
        self.pos = self.pos.with_z(z.iter().sum::<f32>() / delirium_data::kv!(104)) + self.rot * Vec3::Z * (self.radius - self.hub_z);
        self.safe = (self.pos, heading);
    }

    pub fn forward(&self) -> Vec3 {
        self.rot * Vec3::Y
    }

    pub fn heading(&self) -> f32 {
        let f = self.forward();
        f32::atan2(-f.x, f.y)
    }

    pub fn step(&mut self, world: &Collision, accel: f32, brake: f32, steer: f32, handbrake: bool, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        self.prev = Some((self.pos, self.rot));
        let s = self.speed;
        let k = self.tune.clone();
        let h = &k.handling;
        let (pedal, brake) = player_pedals(accel, brake, s, self.throttle);
        self.throttle = pedal;
        self.clock += dt as f64 * delirium_data::kv!(78);
        let controls = Controls { throttle: pedal, brake, handbrake, bike: false, ..Default::default() };
        let powered = h.drive_bias.map(|b| b > 0.0);
        let drive_tyres = || self.tyres.iter().filter(|t| powered[if t.front { 0 } else { 1 }]);
        let grounded = drive_tyres().filter(|t| t.flags & tyre::flags::GROUNDED != 0).count();
        let wheel_speeds: Vec<f32> = drive_tyres().map(|t| -t.rate * self.radius).collect();
        let force = self.gearbox.process(h, controls, &wheel_speeds, s, grounded, dt, self.clock as u32);
        let g = self.gearbox.gear;
        let limit = -h.max_flat_velocity / h.gear_ratios.get(g.max(0) as usize).copied().unwrap_or(1.0) / self.radius;
        let dir = if g > 0 { 1.0 } else { -1.0 };
        let (fwd, side, up) = (self.forward(), self.rot * Vec3::X, self.rot * Vec3::Z);
        let (v_fwd, v_side) = (self.vel.dot(fwd), self.vel.dot(side));
        let f = v_fwd / delirium_data::kv!(80);
        let rate = if f > 1.0 { delirium_data::kv!(60) } else if f > 0.0 { (1.0 - f) * delirium_data::kv!(27) + f * delirium_data::kv!(60) } else { delirium_data::kv!(27) };
        self.steer_input = (self.steer_input + (steer - self.steer_input) * dt * rate).clamp(-1.0, 1.0);
        let lock = h.steering_lock;
        let mut angle = self.steer_input * lock;
        if v_fwd > delirium_data::kv!(60) && angle * v_side >= -delirium_data::kv!(73) * v_fwd.abs() {
            angle /= (v_fwd - delirium_data::kv!(60)) * delirium_data::kv!(73) + 1.0;
        }
        if v_fwd > 1.0 {
            let slide = f32::atan2(-v_side, v_fwd).clamp(-delirium_data::kv!(27 f32).to_radians(), delirium_data::kv!(27 f32).to_radians());
            angle = (slide + angle).clamp(-lock, lock);
        }
        self.steer = angle;

        let mass = h.mass;
        let c0 = self.pos + self.rot * self.com;
        let (mut planted, mut probes, mut demand) = (true, [None; 4], [(0.0, 0.0); 4]);
        for (i, (x, y)) in CORNERS.into_iter().enumerate() {
            let spring = self.springs[i];
            let top = self.pos + self.rot * Vec3::new(x * self.axle.x, y * self.axle.y, self.hub_z + spring.top(h));
            let hit = world.contact(top + up * BUMP, -up, spring.travel + BUMP);
            let axle = if i < 2 { 0 } else { 1 };
            let mut drive = if powered[axle] { force * h.drive_bias[axle] } else { 0.0 };
            let mut wheel_brake = brake * h.brake_force * h.brake_bias[axle];
            let tyre = &mut self.tyres[i];
            let burnout = powered[axle] && brake * h.brake_force > delirium_data::kv!(73) && pedal > delirium_data::kv!(108) && (axle == 1 || h.drive_bias[1] < delirium_data::kv!(73));
            if burnout {
                wheel_brake = 0.0;
            }
            let held = burnout && (self.vel.length_squared() < delirium_data::kv!(104) || tyre.flags & tyre::flags::BURNOUT() != 0);
            tyre.flags = if held { tyre.flags | tyre::flags::BURNOUT() } else { tyre.flags & !tyre::flags::BURNOUT() };
            if handbrake && axle == 1 {
                (wheel_brake, drive) = (wheel_brake + h.handbrake, 0.0);
            }
            demand[i] = (drive, wheel_brake);
            tyre.flags &= !(tyre::flags::DRIVEN() | tyre::flags::POWERED() | tyre::flags::GROUNDED);
            tyre.flags |= if drive != 0.0 { tyre::flags::DRIVEN() } else { 0 }
                | if powered[axle] { tyre::flags::POWERED() } else { 0 }
                | if hit.is_some() { tyre::flags::GROUNDED } else { 0 };
            let Some((t, n, m)) = hit else {
                (self.comp[i], planted) = (0.0, false);
                continue;
            };
            let x = spring.travel + BUMP - t;
            probes[i] = Some(Probe { p: top + up * BUMP - up * t, n, x, prev: self.comp[i] });
            self.comp[i] = x.clamp(0.0, spring.travel);
            let [grip, wet] = k.surfaces.get(m as usize).or(k.surfaces.first()).copied().unwrap_or([1.0, 0.0]);
            let ground = k.ground.get(m as usize).or(k.ground.first()).copied().unwrap_or(0);
            tyre.flags = tyre.flags & !(tyre::flags::SAND() | tyre::flags::SOFT()) | ground;
            let drop = self.radius - self.rim[i / 2];
            let c = suspension::contact(&spring, tyre.surface, tyre.flags, x, spring.limit, drop, grip, wet, 0.0);
            (tyre.grip, tyre.wet) = (c.grip, c.wet);
        }

        let inv = self.rot.inverse();
        let ext_acc = Vec3::NEG_Z * GRAVITY() - self.rot * Vec3::from(damping((inv * self.vel).to_array(), k.damping[0], dt));
        let ext_ang = self.rot * -Vec3::from(damping((inv * self.ang).to_array(), k.damping[1], dt));
        let v4 = |v: Vec3, w: f32| -> V4 { [v.x, v.y, v.z, w] };
        let frame = [v4(self.rot * Vec3::X, 0.0), v4(self.rot * Vec3::Y, 0.0), v4(self.rot * Vec3::Z, 0.0), v4(c0, 1.0)];
        let inertia = mass * k.inertia;
        let body = wheelstep::Body { inv_inertia: v4(inertia.recip(), 0.0), inv_mass: 1.0 / mass, m: frame, vel: v4(self.vel, 0.0), ang: v4(self.ang, 0.0) };
        let force = v4((ext_acc - Vec3::NEG_Z * GRAVITY()) * mass, 0.0);
        let torque = v4(self.rot * (inertia * (inv * ext_ang)), 0.0);
        let mut calls = Wheels { h, mass: wheelstep::Mass { mass, inv_mass: 1.0 / mass, inv_inertia: body.inv_inertia, m: frame },
                                 struts: [Strut::default(); 4], treads: [Tread::default(); 4], axes: [[[0.0; 4]; 2]; 4],
                                 base: self.tyres.clone(), tyres: self.tyres.clone() };
        let mut wheels = [wheelstep::Wheel::default(); 4];
        for i in 0..delirium_data::kv!(1 int) {
            wheels[i].slip = self.tyres[i].slip;
            let (drive, brake) = demand[i];
            let r = &mut self.treads[i];
            *r = Tread { steer: if i < 2 { self.steer } else { 0.0 }, drive, brake, drive_prev: r.drive, brake_prev: r.brake,
                         handbrake: self.tyres[i].resistance, ..*r };
            (calls.treads[i], wheels[i].timer) = (*r, r.no_brake);
            let Some(pr) = probes[i] else { continue };
            let t = &self.tyres[i];
            wheels[i].flags = 1;
            let p = v4(pr.p, 0.0);
            calls.struts[i] = Strut { x: pr.x, prev: pr.prev, spring: self.springs[i], axis: [0.0, 0.0, 1.0, 0.0], contact: p, start: p,
                                      normal: v4(pr.n, 0.0), on_entity: false, damage: delirium_data::kv!(78), health: t.surface, flags: t.flags,
                                      drop: self.radius - self.rim[i / 2] };
        }
        let end = wheelstep::integrate(&body, force, torque, &mut wheels, dt, &mut calls);
        let row = |r: V4| Vec3::new(r[0], r[1], r[2]);
        let rot = Quat::from_mat3(&Mat3::from_cols(row(end.m[0]), row(end.m[1]), row(end.m[2]))).normalize();
        let centre = row(end.m[delirium_data::kv!(0 int)]);
        let last = calls.tyres.clone();
        (self.vel, self.ang, self.rot) = (row(end.vel), row(end.ang), rot);
        for i in 0..delirium_data::kv!(1 int) {
            let (drive, brake) = demand[i];
            self.treads[i].no_brake = wheels[i].timer;
            let v_long = wheelstep::hold(h, &calls.mass, &calls.struts[i], &mut self.treads[i], &mut calls.axes[i], AXLE, end.vel, end.ang, dt);
            let tyre = &mut self.tyres[i];
            let grounded = probes[i].is_some();
            if grounded {
                (tyre.slip, tyre.skid, tyre.flags) = (end.slip[i], last[i].skid, last[i].flags);
            }
            if tyre::spin(tyre, &Contact { v_long, drive, brake }, grounded, mass, self.radius, limit, dir, dt) {
                self.treads[i].no_brake = delirium_data::kv!(73);
            }
        }
        for tyre in self.tyres.iter_mut() {
            tyre::burst(tyre, self.random.next(), dt);
        }

        let inv = self.rot.inverse();
        let inv_inertia = |v: Vec3| self.rot * ((inv * v) / (mass * k.inertia));
        let (mut push, mut hits, mut dv, mut dw) = (Vec3::ZERO, 0.0, Vec3::ZERO, Vec3::ZERO);
        let mut dents = vec![];
        self.pos = centre - self.rot * self.com;
        let points = self.hull_points();
        let reach = points.iter().map(|q| (self.rot * *q + self.pos - centre).length()).fold(0.0f32, f32::max);
        let near = world.near(centre - Vec3::splat(reach), centre + Vec3::splat(reach));
        for q in points {
            let r = self.rot * q + self.pos - centre;
            let len = r.length();
            let Some((t, n)) = near.cast(centre, r / len, len) else { continue };
            push += n * ((len - t) * (r / len).dot(-n) - push.dot(n)).max(0.0);
            let v = self.vel + self.ang.cross(r);
            let vn = v.dot(n);
            if vn >= 0.0 {
                continue;
            }
            let reach = |d: Vec3| 1.0 / mass + d.dot(inv_inertia(r.cross(d)).cross(r));
            let j = -(1.0 + BODY_BOUNCE()) * vn / reach(n);
            let mut impulse = n * j;
            let slide = v - n * vn;
            if slide.length() > delirium_data::kv!(68) {
                let d = slide.normalize();
                impulse -= d * (slide.length() / reach(d)).min(BODY_FRICTION * j);
            }
            (hits, dv, dw) = (hits + 1.0, dv + impulse / mass, dw + inv_inertia(r.cross(impulse)));
            dents.push((q, inv * impulse));
        }
        if hits > 0.0 {
            (self.vel, self.ang) = (self.vel + dv / hits, self.ang + dw / hits);
        }
        if let Some((q, j)) = merge(&dents) {
            self.crash(j / hits, q);
        }
        self.pos += push;
        self.speed = self.vel.dot(self.forward());
        if planted && up.z > delirium_data::kv!(108) {
            self.safe = (self.pos, self.heading());
        }
        if self.pos.z < self.safe.0.z - delirium_data::kv!(132) {
            (self.pos, self.rot) = (self.safe.0 + Vec3::Z * delirium_data::kv!(24), Quat::from_rotation_z(self.safe.1));
            (self.vel, self.ang, self.speed, self.steer, self.steer_input) = (Vec3::ZERO, Vec3::ZERO, 0.0, 0.0, 0.0);
        }
    }

    pub fn ped_hit(&mut self, j: Vec3, at: Vec3) {
        let m = self.tune.handling.mass;
        let r = at - (self.pos + self.rot * self.com);
        if !self.asleep {
            self.vel += j / m;
            self.ang += self.rot * ((self.rot.inverse() * r.cross(j)) / (m * self.tune.inertia));
        }
        self.crash(self.rot.inverse() * j, self.rot.inverse() * (at - self.pos));
        self.by_ped = true;
    }

    fn crash(&mut self, j: Vec3, q: Vec3) {
        self.hit = (self.hit.0 + j, self.hit.1 + q * j.length());
    }

    pub fn damage(&mut self, dt: f32) -> (Option<f32>, bool) {
        let (j, _) = std::mem::take(&mut self.hit);
        self.by_ped = false;
        let h = self.tune.handling.clone();
        let mut took = None;
        if j.length() > 0.0 {
            let r = &mut self.random;
            took = self.health.collision(j.length(), h.mass, h.damage, j.normalize().z, false, &mut || r.next());
        }
        let r = &mut self.random;
        (took, self.health.update(dt, 0.5, self.speed, &mut |lo, hi| r.next().mul_add(hi - lo, lo)))
    }

    pub fn obstacle(&self, id: i32, top: f32) -> delirium::ragdoll::Obstacle {
        self.obstacle_box(id, self.hull.map(|p| p.with_z(p.z.min(top))))
    }

    pub fn cabin(&self, id: i32, top: f32) -> delirium::ragdoll::Obstacle {
        let [lo, hi] = self.hull;
        self.obstacle_box(id, [Vec3::new(lo.x + delirium_data::kv!(73), -delirium_data::kv!(74), top), Vec3::new(hi.x - delirium_data::kv!(73), delirium_data::kv!(110), hi.z)])
    }

    fn obstacle_box(&self, id: i32, [lo, hi]: [Vec3; 2]) -> delirium::ragdoll::Obstacle {
        let (vel, spin) = if self.asleep { (Vec3::ZERO, Vec3::ZERO) } else { (self.vel, self.ang) };
        delirium::ragdoll::Obstacle { id, at: self.pos, rot: self.rot, centre: self.pos + self.rot * ((lo + hi) / 2.0), half: (hi - lo) / 2.0,
                                     vel, spin, com: self.pos + self.rot * self.com, parts: &[], last: self.prev }
    }

    pub fn scan(&self) -> delirium_data::carscan::Vehicle {
        let [lo, hi] = self.hull;
        delirium_data::carscan::Vehicle { pos: self.pos.to_array(), forward: self.forward().to_array(), right: (self.rot * Vec3::X).to_array(),
                                      velocity: self.vel.to_array(), min: lo.to_array(), max: hi.to_array(), flipped: false, kind: 0, class: 0,
                                      status: if self.player { 0 } else { 2 }, driver_player: self.player, horn: false, driven: self.player }
    }

    fn hull_points(&self) -> Vec<Vec3> {
        let [lo, hi] = self.hull;
        let mid = (lo + hi) * 0.5;
        [lo.z, mid.z, hi.z].into_iter().flat_map(move |z| {
            [(lo.x, lo.y), (hi.x, lo.y), (lo.x, hi.y), (hi.x, hi.y), (lo.x, mid.y), (hi.x, mid.y), (mid.x, lo.y), (mid.x, hi.y)].map(|(x, y)| Vec3::new(x, y, z))
        }).collect()
    }

    pub fn wheel(&self, i: usize) -> (f32, f32) {
        let s = &self.springs[i];
        (self.comp[i] - (s.travel - s.top(&self.tune.handling) - self.radius), self.tyres[i].angle)
    }

    pub fn settle(&mut self, dt: f32) {
        if !self.player && !self.asleep {
            self.still = if self.vel.length() < delirium_data::kv!(73) && self.ang.length() < delirium_data::kv!(73) { self.still + dt } else { 0.0 };
            self.asleep = self.still > 1.0;
        }
    }
}

pub fn collide(a: &mut Car, b: &mut Car) {
    if a.asleep && b.asleep || a.pos.distance(b.pos) > (a.hull[1] - a.hull[0]).length() / 2.0 + (b.hull[1] - b.hull[0]).length() / 2.0 {
        return;
    }
    if contact(a, b) | contact(b, a) {
        for c in [a, b] {
            (c.asleep, c.still) = (false, 0.0);
        }
    }
}

fn contact(a: &mut Car, b: &mut Car) -> bool {
    let (ma, mb) = (a.tune.handling.mass, b.tune.handling.mass);
    let (ca, cb) = (a.pos + a.rot * a.com, b.pos + b.rot * b.com);
    let inv_i = |c: &Car, m: f32, v: Vec3| c.rot * ((c.rot.inverse() * v) / (m * c.tune.inertia));
    let ([lo, hi], binv) = (b.hull, b.rot.inverse());
    let (mut push, mut hits, mut dva, mut dwa, mut dvb, mut dwb) = (Vec3::ZERO, 0.0, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
    let mut any = false;
    let mut dents = vec![];
    for q in a.hull_points() {
        let p = a.rot * q + a.pos;
        let l = binv * (p - b.pos);
        if l.cmple(lo).any() || l.cmpge(hi).any() {
            continue;
        }
        any = true;
        let depths = [l.x - lo.x, hi.x - l.x, l.y - lo.y, hi.y - l.y, l.z - lo.z, hi.z - l.z];
        let k = (0..delirium_data::kv!(6 int)).min_by(|&i, &j| depths[i].total_cmp(&depths[j])).unwrap();
        let axis = [Vec3::X, Vec3::Y, Vec3::Z][k / 2];
        let n = b.rot * if k % 2 == 0 { -axis } else { axis };
        push += n * (depths[k] - push.dot(n)).max(0.0);
        let (ra, rb) = (p - ca, p - cb);
        let v = a.vel + a.ang.cross(ra) - b.vel - b.ang.cross(rb);
        let vn = v.dot(n);
        if vn >= 0.0 {
            continue;
        }
        let reach = |d: Vec3| 1.0 / ma + 1.0 / mb + d.dot(inv_i(a, ma, ra.cross(d)).cross(ra)) + d.dot(inv_i(b, mb, rb.cross(d)).cross(rb));
        let j = -(1.0 + BODY_BOUNCE()) * vn / reach(n);
        let mut impulse = n * j;
        let slide = v - n * vn;
        if slide.length() > delirium_data::kv!(68) {
            let d = slide.normalize();
            impulse -= d * (slide.length() / reach(d)).min(BODY_FRICTION * j);
        }
        hits += 1.0;
        (dva, dwa) = (dva + impulse / ma, dwa + inv_i(a, ma, ra.cross(impulse)));
        (dvb, dwb) = (dvb - impulse / mb, dwb - inv_i(b, mb, rb.cross(impulse)));
        dents.push((q, l, impulse));
    }
    if let (Some((q, j)), Some((l, _))) = (merge(&dents.iter().map(|&(q, _, j)| (q, j)).collect::<Vec<_>>()), merge(&dents.iter().map(|&(_, l, j)| (l, j)).collect::<Vec<_>>())) {
        a.crash(a.rot.inverse() * j / hits, q);
        b.crash(binv * -j / hits, l);
    }
    if hits > 0.0 {
        (a.vel, a.ang) = (a.vel + dva / hits, a.ang + dwa / hits);
        (b.vel, b.ang) = (b.vel + dvb / hits, b.ang + dwb / hits);
    }
    a.pos += push * (mb / (ma + mb));
    b.pos -= push * (ma / (ma + mb));
    any
}

