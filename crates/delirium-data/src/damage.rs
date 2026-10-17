#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub body: f32,
    pub engine: f32,
    pub petrol: f32,
    pub stalled: bool,
    pub wrecked: bool,
}

impl Default for Health {
    fn default() -> Self {
        Health { body: 1000.0, engine: 1000.0, petrol: 1000.0, stalled: false, wrecked: false }
    }
}

pub fn crash_amount(j: f32, mass: f32, mult: f32) -> f32 {
    10.0 * mult * j / mass
}

pub fn crash_shocks(j: f32, mass: f32, player_driving: bool, by_ped: bool) -> bool {
    let scale = if player_driving { 3.0 } else { 1.5 } * if by_ped { 10.0 } else { 1.0 };
    j / mass * scale > 10.0 || j * scale > 30000.0
}

pub fn crash_shake(d: f32) -> Option<(f32, f32)> {
    (d >= 4.0).then(|| ((d * 5.0).clamp(80.0, 200.0), d * 0.005))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Panel {
    Door,
    Bonnet,
    Boot,
}

pub const PANEL_MIN: f32 = 25.0;

pub fn panel_box(panel: Panel, lo: [f32; 3], hi: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let by = match panel {
        Panel::Door => [-0.2, -0.2, 0.1],
        Panel::Bonnet => [-0.2, -0.2, -0.4],
        Panel::Boot => [-0.2, -0.3, -0.4],
    };
    (std::array::from_fn(|k| lo[k] + by[k]), hi.map(|v| v + 0.2))
}

pub fn panel_frees(panel: Panel, amount: f32, normal_x: f32, side_x: f32, random: &mut impl FnMut() -> f32, inside: impl FnOnce() -> bool) -> bool {
    if amount <= PANEL_MIN {
        return false;
    }
    let side = if side_x > 0.5 { 1.0 } else if side_x < -0.5 { -1.0 } else { 0.0 };
    if normal_x * side * amount < -35.0 {
        return random() < 0.5;
    }
    random() < if panel == Panel::Door { 0.5 } else { 0.3 } && inside()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lamps(pub [bool; 14]);

impl Lamps {
    pub fn smash(&mut self, amount: f32, mut dist2: impl FnMut(usize) -> Option<f32>) -> Vec<usize> {
        let mut out = vec![];
        if amount < 0.5 {
            return out;
        }
        for n in 0..16 {
            let slot = n.min(13);
            if !self.0[slot] && dist2(n).is_some_and(|d| d < 0.04) {
                self.0[slot] = true;
                out.push(n);
            }
        }
        out
    }
}

const FIRE_STEPS: [f32; 8] = [-250.0, -500.0, -750.0, -1000.0, -1250.0, -1500.0, -2000.0, -3000.0];

impl Health {
    pub fn collision(&mut self, j: f32, mass: f32, mult: [f32; 4], up: f32, engine_end: bool, random: &mut impl FnMut() -> f32) -> Option<f32> {
        let d = crash_amount(j, mass, mult[0]);
        if up > 0.6 || d < 10.0 || self.wrecked {
            return None;
        }
        let d = d - 10.0;
        self.body = (self.body - d).max(0.0);
        if engine_end {
            self.engine_damage(d * mult[3], true, random);
        }
        Some(d)
    }

    pub fn weapon(&mut self, amount: f32, mult: [f32; 4], engine_end: bool, random: &mut impl FnMut() -> f32) {
        if self.wrecked {
            return;
        }
        let d = amount * mult[1];
        self.body = (self.body - d).max(0.0);
        if engine_end {
            self.engine_damage(d * mult[3], false, random);
        }
    }

    pub fn engine_damage(&mut self, amount: f32, collision: bool, random: &mut impl FnMut() -> f32) {
        if self.engine < 0.0 {
            return;
        }
        self.engine -= amount;
        if self.engine > 0.0 {
            return;
        }
        self.engine = 0.01;
        if random() <= if collision { 0.7 } else { 0.5 } {
            return;
        }
        if collision && random() < 0.5 {
            (self.engine, self.stalled) = (0.0, true);
        } else {
            self.engine = -1.0;
        }
    }

    pub fn explosion(&mut self, amount: f32, mult: [f32; 4], by_player: bool, random: &mut impl FnMut() -> f32) {
        if self.wrecked {
            return;
        }
        let d = amount * 3.0 * mult[1];
        self.body = (self.body - d).max(0.0);
        self.engine_damage(d, false, random);
        if self.petrol > -1000.0 {
            let p = self.petrol - if by_player { d * 2.0 } else { d };
            if p < 0.0 { self.light_petrol() } else { self.petrol = p }
        }
    }

    pub fn light_petrol(&mut self) {
        if self.petrol >= 0.0 {
            self.petrol = -1.0;
        }
    }

    pub fn put_out(&mut self) {
        self.body = self.body.max(300.0);
        if (self.engine < 0.0 && self.engine > -4000.0) || self.petrol_fire() {
            (self.engine, self.petrol) = (-4000.0, 1.0);
        }
    }

    pub fn update(&mut self, dt: f32, burn: f32, forward: f32, random: &mut impl FnMut() -> f32) -> bool {
        if self.wrecked {
            return false;
        }
        if self.engine < 0.0 && self.engine > -4000.0 {
            let rate = 30.0 + 70.0 * random();
            let was = self.engine;
            self.engine = (self.engine - rate * dt).max(-4000.0);
            for (k, &t) in FIRE_STEPS.iter().enumerate() {
                if self.engine < t && was >= t && (k == 7 || rate > 95.0 - 3.0 * forward) {
                    self.light_petrol();
                }
            }
            if self.engine <= -4000.0 {
                self.stalled = true;
            }
        }
        if self.petrol < 0.0 && self.petrol > -1000.0 {
            self.petrol -= (60.0 + 90.0 * burn) * dt;
            if self.petrol <= -1000.0 {
                (self.petrol, self.wrecked, self.stalled, self.body) = (-1000.0, true, true, 0.0);
                return true;
            }
        }
        false
    }

    pub fn smoke(&self) -> f32 {
        if self.engine > 0.0 { (1.0 - self.engine * 0.0025).max(0.0) } else { 1.0 }
    }

    pub fn fire(&self) -> f32 {
        let e = self.engine;
        if e >= 0.0 {
            0.0
        } else if e > -700.0 {
            e / -700.0
        } else if e >= -3200.0 {
            1.0
        } else if e > -3600.0 {
            1.0 - (e + 3600.0) * 0.00125
        } else {
            0.0
        }
    }

    pub fn petrol_fire(&self) -> bool {
        self.petrol < 0.0 && self.petrol > -1000.0
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Explosion {
    pub damage: [f32; 2],
    pub radius: f32,
    pub speed: f32,
    pub decay: f32,
    pub force: f32,
    pub ragdoll: f32,
    pub fires: [f32; 4],
    pub fx: Option<String>,
    pub fx_air: Option<String>,
    pub fx_scale: f32,
    pub light: [f32; 6],
    pub shake: f32,
    pub weapon: String,
}

impl Explosion {
    pub fn push(&self, area: f32, closing: f32, mass: f32) -> f32 {
        if closing <= 0.0 { 0.0 } else { (self.force * area * closing * closing * 0.2).min(mass * 50.0) }
    }

    pub fn ragdoll_push(&self, area: f32, closing: f32, mass: f32) -> f32 {
        if closing <= 0.0 { 0.0 } else { (self.force * area * closing * closing * 0.2 * self.ragdoll).min(mass * 20.0) }
    }

    pub fn in_air(self, air: bool) -> Explosion {
        if air { Explosion { fx: self.fx_air.clone(), ..self } } else { self }
    }

    pub fn damage_at(&self, r: f32) -> f32 {
        if r >= self.radius { 0.0 } else { self.damage[0] + (self.damage[1] - self.damage[0]) * r / self.radius }
    }
}

pub fn read_explosions(text: &str) -> std::collections::HashMap<String, Explosion> {
    let mut on = false;
    let mut out = std::collections::HashMap::new();
    for line in text.lines().map(str::trim) {
        match line {
            "EXPLOSIONFX_TABLE_START" => on = true,
            "EXPLOSIONFX_TABLE_END" => on = false,
            _ if on && !line.is_empty() && !line.starts_with('#') => {
                let f: Vec<&str> = line.split_whitespace().collect();
                let n = |i: usize| f.get(i).and_then(|s| s.parse::<f32>().ok());
                if let (Some(c), Some(e), Some(r), Some(force)) = (n(1), n(2), n(5), n(8)) {
                    let fires = [15, 16, 17, 18].map(|i| n(i).unwrap_or(0.0));
                    let light = [22, 23, 24, 25, 26, 27].map(|i| n(i).unwrap_or(0.0));
                    out.insert(f[0].to_string(), Explosion { damage: [c, e], radius: r, speed: n(6).unwrap_or(50.0), decay: n(7).unwrap_or(-1.0), force, ragdoll: n(9).unwrap_or(0.0), fires, fx: f.get(12).map(|s| s.to_string()), fx_air: f.get(13).map(|s| s.to_string()),
                                                             fx_scale: n(14).unwrap_or(1.0), light, shake: n(19).unwrap_or(0.0), weapon: f.get(20).map(|s| s.to_string()).unwrap_or_default() });
                }
            }
            _ => {}
        }
    }
    out
}

