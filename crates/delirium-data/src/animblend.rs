pub const UNSET: f32 = -1.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub duration: Option<f32>,
    pub phase: f32,
    pub prev_phase: f32,
    pub rate: f32,
    pub weight: f32,
    pub goal: f32,
    pub blend_rate: f32,
    pub weight_step: f32,
    pub phase_step: f32,
    pub blend_out: f32,
    pub priority: u8,
    pub mask: u8,
    pub playing: bool,
    pub paused: bool,
    pub finished: bool,
    pub looped: bool,
    pub hold: bool,
    pub blending: bool,
    pub active: bool,
    pub loops: bool,
    pub synced: bool,
    pub remove_blended_out: bool,
    pub blend_out_at_end: bool,
    pub own_channel: bool,
    pub skip_weight: bool,
}

impl Player {
    pub fn new(duration: f32) -> Self {
        Player {
            duration: Some(duration), phase: 0.0, prev_phase: 0.0, rate: 1.0, weight: 1.0, goal: 0.0, blend_rate: 0.0,
            weight_step: UNSET, phase_step: UNSET, blend_out: -4.0, priority: 0, mask: 0,
            playing: false, paused: false, finished: false, looped: false, hold: false, blending: false,
            active: false, loops: false, synced: false, remove_blended_out: false, blend_out_at_end: false,
            own_channel: false, skip_weight: false,
        }
    }

    pub fn blend_speed(&self) -> f32 {
        if self.blending && (self.goal == 1.0 || self.goal == 0.0) { self.blend_rate } else { 0.0 }
    }

    pub fn signed_blend_rate(&self) -> f32 {
        if self.blending && self.goal == 1.0 {
            self.blend_speed()
        } else if self.blending && self.goal == 0.0 {
            -self.blend_speed()
        } else {
            0.0
        }
    }

    pub fn set_blend(&mut self, delta: f32) {
        if delta > 0.0 {
            if self.weight >= 1.0 && delta <= self.signed_blend_rate() {
                return;
            }
            (self.goal, self.blending) = (1.0, true);
            self.blend_rate = if delta - 0.001 >= 0.0 { delta } else { 0.001 };
        } else if delta < 0.0 {
            if self.weight <= 0.0 && delta >= self.signed_blend_rate() {
                return;
            }
            let d = -delta;
            (self.goal, self.blending) = (0.0, true);
            self.blend_rate = if d - 0.001 >= 0.0 { d } else { 0.001 };
        } else {
            self.blending = false;
        }
    }

    pub fn restart(&mut self) {
        self.active = true;
        let at = if self.rate >= 0.0 { 0.0 } else { 1.0 };
        (self.phase, self.prev_phase, self.paused) = (at, at, false);
        if self.duration.is_some() {
            self.playing = true;
        }
    }

    fn weight_step_now(&self, dt: f32) -> f32 {
        if self.weight_step != UNSET { self.weight_step } else { self.blend_rate * dt }
    }

    pub fn phase_step_now(&self, dt: f32) -> f32 {
        if self.phase_step != UNSET {
            return self.phase_step;
        }
        frame_rate(self.duration.unwrap_or(0.0)) * self.rate * dt
    }

    pub fn blend_weight(&mut self, dt: f32) -> bool {
        if self.duration.is_some() && self.blending {
            self.weight_step = self.blend_speed() * dt;
            return self.update_weight(dt);
        }
        false
    }

    pub fn update_weight(&mut self, dt: f32) -> bool {
        if self.skip_weight {
            self.skip_weight = false;
        } else if self.blending {
            let step = self.weight_step_now(dt);
            let d = self.goal - self.weight;
            if !(d.abs() > step) {
                (self.weight, self.blending) = (self.goal, false);
            } else {
                let sign: f32 = if d >= 0.0 { if -d >= 0.0 { 0.0 } else { 1.0 } } else { -1.0 };
                let w = sign.mul_add(step, self.weight);
                self.weight = if w >= 0.0 { w } else { 0.0 };
            }
        }
        if self.weight > 0.0 {
            return false;
        }
        self.weight = 0.0;
        if self.blending || !self.remove_blended_out {
            return false;
        }
        self.duration = None;
        true
    }

    pub fn advance_phase(&mut self, dt: f32) {
        self.prev_phase = self.phase;
        self.finished = false;
        if !self.playing || self.paused {
            return;
        }
        if self.duration.is_some() {
            self.phase = self.phase_step_now(dt) + self.phase;
            if self.phase >= 1.0 {
                self.finished = true;
                if self.looped {
                    self.phase -= self.phase.trunc();
                } else {
                    self.phase = 1.0;
                    if self.hold { self.prev_phase = 1.0 } else { self.playing = false }
                }
            } else if self.phase < 0.0 {
                self.finished = true;
                if self.looped {
                    self.phase = ((self.phase + 100.0) as f64 % 1.0) as f32;
                } else {
                    self.phase = 0.0;
                    if self.hold { self.prev_phase = 0.0 } else { self.playing = false }
                }
            }
        }
        if self.prev_phase > self.phase {
            self.prev_phase = 0.0;
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.loops {
            self.looped = true;
        } else {
            (self.looped, self.hold) = (false, true);
        }
        self.advance_phase(dt);
        if self.looped || !self.finished {
            return;
        }
        self.active = false;
        if self.blend_out_at_end {
            self.remove_blended_out = true;
            self.set_blend(self.blend_out);
        }
    }
}

fn frame_rate(duration: f32) -> f32 {
    let frames = duration.mul_add(30.0, 1.0) - 1.0;
    30.0 / if 1.0 - frames >= 0.0 { 1.0 } else { frames }
}

pub fn sync_phases(players: &mut [Player], dt: f32) {
    let (mut sum_w, mut sum_d) = (0.0f32, 0.0f32);
    for p in players.iter().filter(|p| p.active && p.synced) {
        if let Some(d) = p.duration {
            sum_w = p.weight + sum_w;
            sum_d = d.mul_add(p.weight / p.rate, sum_d);
        }
    }
    for p in players.iter_mut().filter(|p| p.active) {
        let Some(d) = p.duration else { continue };
        p.phase_step = if sum_d == 0.0 || !p.synced {
            frame_rate(d) * p.rate * dt
        } else {
            let average = sum_d / sum_w;
            (d / p.rate / average) * (frame_rate(d) * p.rate * dt)
        };
    }
    for p in players.iter_mut() {
        if p.active && p.duration.is_some() {
            p.update(dt);
        } else {
            p.prev_phase = p.phase;
        }
    }
}

pub fn priority_weights(players: &[Player]) -> Vec<f32> {
    let mut out = vec![0.0; players.len()];
    let (mut count0, mut only0) = (0, 0);
    for layer in (0..=9u8).rev() {
        let mut sum = 0.0f32;
        for (i, p) in players.iter().enumerate() {
            if p.duration.is_some() && !p.own_channel && p.priority == layer {
                sum = p.weight + sum;
                if layer == 0 {
                    (count0, only0) = (count0 + 1, i);
                }
            }
        }
        let scale = if sum > 0.0 { 1.0 / sum } else { 1.0 };
        for (o, p) in out.iter_mut().zip(players) {
            if p.duration.is_some() && p.priority == layer {
                *o = if layer == 0 || sum > 1.0 { p.weight * scale } else { p.weight };
            }
        }
    }
    if count0 == 1 {
        out[only0] = 1.0;
    }
    out
}

pub fn layer_weights(weights: &mut [f32], priority: &[u8], top: u8) {
    let mut left = 1.0f32;
    for layer in (0..=top).rev() {
        let (mut sum, mut count, mut only) = (0.0f32, 0, 0);
        for (i, (&w, _)) in weights.iter().zip(priority).enumerate().filter(|&(_, (_, &p))| p == layer) {
            (sum, count, only) = (w + sum, count + 1, i);
        }
        if left < sum {
            if count > 1 {
                for (w, _) in weights.iter_mut().zip(priority).filter(|&(_, &p)| p == layer) {
                    *w *= left;
                }
            } else {
                weights[only] = left;
            }
            left = 0.0;
        } else {
            left -= sum;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dof {
    pub track: u8,
    pub id: u16,
    pub kind: u8,
    pub invalid: bool,
    pub value: [f32; 4],
}

pub fn blend_n(dest: &mut [Dof], sources: &[&[Dof]], weights: &[f32], priority: Option<&[u8]>) {
    let total = weights.iter().fold(0.0f32, |s, &w| s + w);
    let total = if total - 1.0 >= 0.0 { 1.0 } else { total };
    let rest = if total > 0.999 { 0.0 } else { 1.0 - total };
    let mut w = vec![0.0f32; weights.len()];
    for (d, dof) in dest.iter_mut().enumerate() {
        for (i, s) in sources.iter().enumerate() {
            w[i] = if s[d].invalid { 0.0 } else { weights[i] };
        }
        if let Some(p) = priority.filter(|_| dof.track < 2 || dof.track == 0x15) {
            layer_weights(&mut w, p, 8);
        }
        let mut acc = rest;
        for w in w.iter_mut() {
            acc = *w + acc;
            if acc >= 0.001 {
                let q = *w / acc;
                *w = if !(q >= 0.0) { 0.0 } else if q >= 1.0 { 1.0 } else { q };
            } else {
                (*w, acc) = (0.0, 0.0);
            }
        }
        for (s, &t) in sources.iter().zip(&w).filter(|&(_, &t)| t > 0.0) {
            blend_dof(dof, &s[d], t);
        }
    }
}

fn blend_dof(d: &mut Dof, s: &Dof, t: f32) {
    match d.kind {
        1 if !d.invalid => {
            let [mut dx, mut dy, mut dz, mut dw] = d.value;
            let [sx, sy, sz, sw] = s.value;
            let dot = sz.mul_add(dz, sy.mul_add(dy, dx.mul_add(sx, dw * sw)));
            if dot < 0.0 {
                (dx, dy, dz, dw) = (-dx, -dy, -dz, -dw);
            }
            let u = 1.0 - t;
            let x = dx.mul_add(u, sx * t);
            let y = sy.mul_add(t, dy * u);
            let z = sz.mul_add(t, dz * u);
            let w = dw.mul_add(u, t * sw);
            let len2 = w.mul_add(w, z.mul_add(z, x.mul_add(x, y * y)));
            let r = if -len2 >= 0.0 { 0.0 } else { (1.0 / (len2 as f64).sqrt()) as f32 };
            d.value = [r * x, r * y, r * z, r * w];
        }
        1 => (d.value, d.invalid) = (s.value, false),
        0 if t > 0.001 => {
            if t < 0.999 && !d.invalid {
                for (a, &b) in d.value.iter_mut().zip(&s.value) {
                    *a = (b - *a).mul_add(t, *a);
                }
            } else {
                d.value = s.value;
            }
            d.invalid = false;
        }
        0 => {}
        2 if t < 0.999 && !d.invalid => (d.value[0], d.invalid) = ((s.value[0] - d.value[0]).mul_add(t, d.value[0]), s.invalid),
        2 if t > 0.001 => (d.value[0], d.invalid) = (s.value[0], s.invalid),
        2 => {}
        _ if t >= 0.5 => (d.value[0], d.invalid) = (s.value[0], false),
        _ => {}
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoverPlayer {
    pub clip: bool,
    pub layer: i32,
    pub weight: f32,
    pub flags: u32,
    pub mover_track: bool,
}

pub const FULL_MOVER_WEIGHT: u32 = 0x1_0000;
pub const OWN_CHANNEL: u32 = 0x2_0000;

pub fn mover_weights(players: &[MoverPlayer], frozen: f32) -> Vec<f32> {
    use crate::staticcount::{MOVER_BITS, NO_MOVER_TRACK};
    let (mut w, mut layer, mut full, mut from) = (vec![], vec![], vec![], vec![]);
    let (mut count0, mut lowest) = (0, 0);
    for (k, p) in players.iter().enumerate().filter(|(_, p)| p.clip) {
        let root = |mask: u32| mask == 0 || mask_passes(mask as u8, 0);
        let counts = p.flags & NO_MOVER_TRACK == 0 && p.mover_track && p.flags & OWN_CHANNEL == 0 && root(p.flags & 0xF)
            || p.flags & MOVER_BITS != 0;
        if counts && p.layer == 0 {
            (count0, lowest) = (count0 + 1, w.len());
        }
        w.push(if counts { p.weight } else { 0.0 });
        layer.push(p.layer);
        full.push(p.flags & FULL_MOVER_WEIGHT != 0);
        from.push(k);
    }
    if frozen > 0.0 {
        (w.push(frozen), layer.push(8), full.push(false), from.push(players.len()));
    }
    let mut sums = [0.0f32; 9];
    for (&l, &x) in layer.iter().zip(&w) {
        if let Some(s) = usize::try_from(l).ok().and_then(|l| sums.get_mut(l)) {
            *s = x + *s;
        }
    }
    for (l, &sum) in sums.iter().enumerate() {
        let inv = if sum > 0.0 { 1.0 / sum } else { 1.0 };
        for i in (0..w.len()).filter(|&i| layer[i] == l as i32) {
            if full[i] {
                w[i] = 1.0;
            } else if l == 0 || sum > 1.0 {
                w[i] *= inv;
            }
        }
    }
    if count0 == 1 {
        w[lowest] = 1.0;
    }
    layer_weights(&mut w, &layer.iter().map(|&l| u8::try_from(l).unwrap_or(255)).collect::<Vec<_>>(), 9);
    let mut out = vec![0.0; players.len() + 1];
    for (&k, &x) in from.iter().zip(&w) {
        out[k] = x;
    }
    out
}

pub const MASK_BONES: [u16; 1] = [0];

pub const MASKS: [&[(u8, u8)]; 12] = [
    &[(0, 132)],
    &[(2, 54), (59, 59), (64, 132)],
    &[(2, 7)],
    &[(32, 50)],
    &[(9, 27)],
    &[(0, 64)],
    &[(65, 86)],
    &[(87, 100)],
    &[(6, 7), (9, 27), (32, 50)],
    &[(6, 7), (32, 50)],
    &[(6, 7), (9, 27)],
    &[(0, 7), (9, 12), (32, 35), (55, 58), (60, 63)],
];

pub fn mask_passes(mask: u8, bone: u16) -> bool {
    MASK_BONES.iter().position(|&b| b == bone)
        .is_some_and(|slot| MASKS[mask as usize].iter().any(|&(a, b)| (a as usize..=b as usize).contains(&slot)))
}

pub fn ease_ratio(ratio: f32, wanted: f32, dt: f32, player: bool, top: f32) -> f32 {
    let (up, down) = if player { (4.0, 2.0) } else { (2.0, 8.0) };
    if wanted > ratio {
        let r = dt.mul_add(up, ratio);
        let r = if r > wanted { wanted } else { r };
        if r > top + 1.0 { top + 1.0 } else { r }
    } else if wanted < ratio {
        let r = (-dt).mul_add(down, ratio);
        let r = if r < wanted { wanted } else { r };
        if r < 0.0 { 0.0 } else { r }
    } else {
        wanted
    }
}

pub fn move_ratio(m: f32, s: f32) -> f32 {
    let cap = if s > 1.0 { 3.0 } else if s > 0.0 { 2.0 } else { 1.0 };
    (cap * m).min(cap)
}

pub fn split_ratio(ratio: f32) -> [(i32, f32); 2] {
    let frac = (ratio as f64 % 1.0) as f32;
    let slot = ratio as i32;
    if frac.abs() < 0.01 { [(slot, 1.0), (-1, 0.0)] } else { [(slot, 1.0 - frac), (slot + 1, frac)] }
}

