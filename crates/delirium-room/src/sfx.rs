use bevy::audio::{AddAudioSource, AudioSinkPlayback, Decodable, Source};
use bevy::prelude::*;
use std::sync::Arc;
use std::time::Duration;

const RATE: u32 = 44100;
const NOISE_CLOCK: f32 = 2.5;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Snd {
    Shot(usize),
    Hit,
    Kill,
    Boom,
    Punch,
    Throw,
    Crash,
    Step,
    Thud(u8),
    PortalShoot(u8),
    PortalOpen(u8),
}

#[derive(Asset, TypePath, Clone)]
pub struct Sfx(Arc<[f32]>);

pub struct Play(Arc<[f32]>, usize);

impl Iterator for Play {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        self.1 += 1;
        self.0.get(self.1 - 1).copied()
    }
}

impl Source for Play {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> std::num::NonZero<u16> {
        std::num::NonZero::<u16>::MIN
    }
    fn sample_rate(&self) -> std::num::NonZero<u32> {
        std::num::NonZero::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.0.len() as f32 / RATE as f32))
    }
}

impl Decodable for Sfx {
    type Decoder = Play;
    fn decoder(&self) -> Play {
        Play(self.0.clone(), 0)
    }
}

struct Voice {
    noise: bool,
    duty: f32,
    from: f32,
    to: f32,
    len: f32,
    vol: f32,
    fade: f32,
    at: f32,
}

fn sq(from: f32, to: f32, len: f32, vol: f32) -> Voice {
    Voice { noise: false, duty: 0.5, from, to, len, vol, fade: 1.2, at: 0.0 }
}

fn nz(from: f32, to: f32, len: f32, vol: f32) -> Voice {
    Voice { noise: true, duty: 0.5, from: from * NOISE_CLOCK, to: to * NOISE_CLOCK, len, vol, fade: 1.2, at: 0.0 }
}

fn voices(s: Snd, k: usize) -> Vec<Voice> {
    let p = [1.0, 0.92, 1.08][k];
    match s {
        Snd::PortalShoot(_) | Snd::PortalOpen(_) => vec![],
        Snd::Shot(0) => vec![nz(9000.0, 6000.0, 0.03, 0.6), Voice { duty: 0.5, ..sq(760.0 * p, 240.0, 0.15, 0.6) }],
        Snd::Shot(1) => vec![Voice { fade: 1.2, ..nz(6000.0 * p, 1500.0, 0.28, 0.9) }, sq(420.0 * p, 90.0, 0.24, 0.6), nz(10000.0, 8000.0, 0.025, 0.6)],
        Snd::Shot(2) => vec![Voice { fade: 1.6, ..nz(10000.0 * p, 6500.0, 0.08, 0.8) }, Voice { duty: 0.125, ..sq(1500.0 * p, 900.0, 0.03, 0.3) }],
        Snd::Shot(3) => vec![nz(7500.0 * p, 4500.0, 0.15, 0.9), Voice { duty: 0.25, ..sq(300.0 * p, 140.0, 0.09, 0.45) }],
        Snd::Shot(4) => [0.35, 0.75].iter().zip([0.45, 0.22])
            .map(|(&at, v)| Voice { at, fade: 1.6, ..nz(3000.0 * p, 500.0, 0.4, v) })
            .chain([nz(12000.0, 6000.0, 0.05, 1.0), Voice { fade: 1.4, ..nz(4000.0 * p, 400.0, 0.7, 1.0) },
                    Voice { fade: 1.0, ..sq(110.0 * p, 30.0, 0.6, 1.0) }]).collect(),
        Snd::Shot(5) => vec![nz(7000.0, 3000.0, 0.04, 1.0), Voice { fade: 1.3, ..nz(2200.0 * p, 300.0, 0.45, 1.0) },
                             Voice { fade: 1.0, ..sq(80.0 * p, 30.0, 0.35, 1.0) },
                             Voice { at: 0.48, ..nz(9000.0, 7000.0, 0.04, 0.5) }, Voice { at: 0.6, ..nz(8000.0, 6000.0, 0.05, 0.5) }],
        Snd::Shot(11) => vec![Voice { duty: 0.5, fade: 1.0, ..sq(120.0 * p, 110.0, 0.6, 0.35) }, Voice { fade: 1.0, ..nz(7000.0 * p, 7000.0, 0.6, 0.45) }],
        Snd::Shot(_) => vec![Voice { fade: 0.6, ..nz(900.0 * p, 6000.0, 0.6, 0.6) }, Voice { duty: 0.125, fade: 0.8, ..sq(110.0 * p, 520.0, 0.45, 0.3) },
                             nz(4000.0, 800.0, 0.1, 0.9)],
        Snd::Hit => vec![Voice { duty: 0.25, ..sq(330.0 * p, 70.0, 0.09, 0.7) }, nz(5000.0, 1500.0, 0.05, 0.4)],
        Snd::Kill => [440.0, 330.0, 262.0, 196.0, 131.0].iter().enumerate()
            .map(|(i, &f)| Voice { at: i as f32 * 0.07, fade: 1.0, duty: 0.25, ..sq(f * p, f * p, 0.09, 0.45) }).collect(),
        Snd::Boom => vec![Voice { fade: 0.8, ..nz(3000.0 * p, 1000.0, 1.1, 1.0) }, Voice { fade: 1.0, ..nz(700.0 * p, 120.0, 0.8, 0.6) }],
        Snd::Punch => vec![nz(4000.0 * p, 800.0, 0.07, 0.7), sq(160.0 * p, 60.0, 0.07, 0.6)],
        Snd::Throw => vec![Voice { duty: 0.125, fade: 1.0, ..sq(250.0 * p, 800.0, 0.14, 0.3) }],
        Snd::Step | Snd::Thud(_) => vec![],
        Snd::Crash => vec![Voice { fade: 0.9, ..nz(3000.0 * p, 600.0, 0.45, 1.0) }, sq(90.0 * p, 35.0, 0.3, 0.6)],
    }
}

fn make(s: Snd, k: usize) -> Vec<f32> {
    let p = [1.0, 0.92, 1.08][k];
    match s {
        Snd::Step => thump(75.0 * p, 0.09, 0.45, 0.6),
        Snd::Thud(n) => {
            let n = n as f32;
            thump((60.0 - 5.0 * n) * p, 0.12 + 0.05 * n, 0.5 + 0.2 * n, 0.8 + 0.4 * n)
        }
        _ => render(&voices(s, k)),
    }
}

fn thump(f: f32, len: f32, vol: f32, grit: f32) -> Vec<f32> {
    let n = (len * RATE as f32) as usize;
    let (mut ph, mut lp, mut seed) = (0.0f32, 0.0f32, 0x1234_5678u32);
    (0..n).map(|i| {
        let (t, x) = (i as f32 / RATE as f32, i as f32 / n as f32);
        ph += f * (1.0 - 0.25 * x) / RATE as f32;
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let w = (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
        lp += (w - lp) * 0.08;
        let env = (t / 0.003).min(1.0) * (-x * 6.0).exp();
        ((std::f32::consts::TAU * ph).sin() * vol + lp * grit) * env * 0.55
    }).map(|x| x.clamp(-1.0, 1.0)).collect()
}

fn render(vs: &[Voice]) -> Vec<f32> {
    let len = vs.iter().map(|v| v.at + v.len).fold(0.0, f32::max);
    let mut out = vec![0.0f32; (len * RATE as f32) as usize + 1];
    for v in vs {
        let (mut phase, mut lfsr, mut bit) = (0.0f32, 1u16, 1.0f32);
        let n = (v.len * RATE as f32) as usize;
        for i in 0..n {
            let t = i as f32 / n as f32;
            let f = v.from * (v.to / v.from).powf(t);
            phase += f / RATE as f32;
            let s = if v.noise {
                while phase >= 1.0 {
                    phase -= 1.0;
                    let b = (lfsr ^ (lfsr >> 1)) & 1;
                    lfsr = (lfsr >> 1) | (b << 14);
                    bit = if lfsr & 1 == 1 { 1.0 } else { -1.0 };
                }
                bit
            } else if v.duty == 0.0 {
                phase = phase.fract();
                ((1.0 - 4.0 * ((phase + 0.25).fract() - 0.5).abs()) * 7.5).round() / 7.5
            } else {
                phase = phase.fract();
                if phase < v.duty { 1.0 } else { -1.0 }
            };
            let a = v.vol * (1.0 - t).powf(v.fade) * if v.duty == 0.0 && !v.noise { (i as f32 / (0.004 * RATE as f32)).min(1.0) } else { 1.0 };
            out[(v.at * RATE as f32) as usize + i] += s * (a * 15.0).round() / 15.0;
        }
    }
    for x in &mut out {
        *x = (*x * 0.35).clamp(-1.0, 1.0);
    }
    out
}

#[derive(Resource)]
pub struct Bank(std::collections::HashMap<Snd, [Handle<Sfx>; 3]>, usize);

impl Bank {
    pub fn add(&mut self, s: Snd, h: Handle<Sfx>) {
        self.0.insert(s, [h.clone(), h.clone(), h]);
    }
}

pub fn wav(b: &[u8]) -> Option<Sfx> {
    let u = |o: usize, n: usize| b.get(o..o + n).map(|x| x.iter().rev().fold(0usize, |a, &c| a << 8 | c as usize));
    let (mut o, mut fmt, mut data) = (12, None, None);
    while let (Some(id), Some(len)) = (b.get(o..o + 4), u(o + 4, 4)) {
        match id {
            b"fmt " => fmt = Some((u(o + 8, 2)?, u(o + 10, 2)?, u(o + 12, 4)?, u(o + 22, 2)?)),
            b"data" => data = b.get(o + 8..(o + 8 + len).min(b.len())),
            _ => {}
        }
        o += 8 + len + (len & 1);
    }
    let ((1, ch, rate, 16), Some(d)) = (fmt?, data) else { return None };
    let frames: Vec<f32> = d.chunks_exact(2 * ch).map(|f| f.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as f32).sum::<f32>() / (ch as f32 * 32768.0)).collect();
    let n = frames.len() * RATE as usize / rate.max(1);
    Some(Sfx((0..n).map(|i| frames[(i * rate / RATE as usize).min(frames.len() - 1)]).collect()))
}

pub fn plugin(app: &mut App) {
    app.add_audio_source::<Sfx>().add_systems(Startup, |mut commands: Commands, mut assets: ResMut<Assets<Sfx>>| {
        let all = (0..7).chain([11]).map(Snd::Shot).chain([Snd::Hit, Snd::Kill, Snd::Boom, Snd::Punch, Snd::Throw, Snd::Crash, Snd::Step, Snd::Thud(0), Snd::Thud(1), Snd::Thud(2)]);
        let bank = all.map(|s| (s, [0, 1, 2].map(|k| assets.add(Sfx(make(s, k).into()))))).collect();
        commands.insert_resource(Bank(bank, 0));
        commands.spawn((AudioPlayer(assets.add(Sfx(fire_loop().into()))), PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(0.0)), FireLoop));
        for t in 0..AMB_THEMES.len() {
            commands.spawn((AudioPlayer(assets.add(Sfx(ambience(t).into()))), PlaybackSettings::LOOP.with_volume(bevy::audio::Volume::Linear(0.0)), Ambience(t)));
        }
    });
}

#[derive(Component)]
pub struct FireLoop;

pub fn fire_volume(sinks: &mut Query<&mut AudioSink, (With<FireLoop>, Without<Ambience>)>, v: f32) {
    for mut s in sinks.iter_mut() {
        if (s.volume().to_linear() - v).abs() > 1e-3 {
            s.set_volume(bevy::audio::Volume::Linear(v));
        }
    }
}

pub fn fire_loop() -> Vec<f32> {
    let (n, m) = ((4.0 * RATE as f32) as usize, (0.4 * RATE as f32) as usize);
    let tau = std::f32::consts::TAU;
    let mut seed = 0x9e37_79b9u32;
    let mut rnd = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    let (mut lp, mut lp2) = (0.0f32, 0.0f32);
    let mut out: Vec<f32> = (0..n + m).map(|i| {
        let t = i as f32 / RATE as f32;
        let w = rnd() * 2.0 - 1.0;
        lp += (w - lp) * 0.04;
        lp2 += (lp - lp2) * 0.04;
        let flicker = 0.75 + 0.15 * (tau * t * 1.3).sin() + 0.1 * (tau * t * 3.7 + 1.0).sin();
        lp2 * 6.0 * flicker
    }).collect();
    let mut at = 0.0;
    while at < 4.0 {
        at += 0.03 + rnd() * 0.25;
        let (s0, len, vol) = ((at * RATE as f32) as usize, (0.004 + rnd() * 0.01) * RATE as f32, 0.15 + rnd() * 0.35);
        let mut hp = 0.0f32;
        for i in 0..len as usize {
            let w = rnd() * 2.0 - 1.0;
            let b = w - hp;
            hp = w;
            if let Some(o) = out.get_mut(s0 + i) {
                *o += b * vol * (1.0 - i as f32 / len).powi(3);
            }
        }
    }
    for i in 0..m {
        let k = i as f32 / m as f32;
        out[i] = out[i] * k + out[n + i] * (1.0 - k);
    }
    out.truncate(n);
    let peak = out.iter().fold(0.0f32, |a, x| a.max(x.abs())).max(1e-6);
    out.iter_mut().for_each(|x| *x *= 0.7 / peak);
    out
}

pub const AMB_THEMES: [&str; 3] = ["Calm", "Lab", "Void"];

#[derive(Component)]
pub struct Ambience(pub usize);

pub fn ambience_volume(sinks: &mut Query<(&mut AudioSink, &Ambience)>, theme: usize, v: f32) {
    for (mut s, a) in sinks.iter_mut() {
        let v = if a.0 == theme { v } else { 0.0 };
        if (s.volume().to_linear() - v).abs() > 1e-3 {
            s.set_volume(bevy::audio::Volume::Linear(v));
        }
    }
}

const AMB_SECS: f32 = 64.0;

struct Amb {
    out: Vec<f32>,
    wow: f32,
}

impl Amb {
    fn note(&mut self, f: f32, at: f32, len: f32, vol: f32, wave: u8, env: &dyn Fn(f32) -> f32) {
        let tau = std::f32::consts::TAU;
        let n = self.out.len();
        let (s0, m) = ((at.rem_euclid(AMB_SECS) * RATE as f32) as usize, (len * RATE as f32) as usize);
        let mut ph = 0.0f32;
        for i in 0..m {
            let t = (s0 + i) as f32 / RATE as f32;
            let wow = 1.0 + self.wow * ((tau * t / 4.0).sin() + 0.4 * (tau * t / 0.8).sin());
            ph = (ph + f * wow / RATE as f32).fract();
            let w = match wave {
                0 => ((1.0 - 4.0 * (ph - 0.5).abs()) * 7.5).round() / 7.5,
                1 => if ph < 0.25 { 1.0 } else { -1.0 },
                _ => if ph < 0.125 { 1.0 } else { -1.0 },
            };
            self.out[(s0 + i) % n] += w * vol * env(i as f32 / m as f32);
        }
    }

    fn wind(&mut self, clock: f32, vol: f32) {
        let tau = std::f32::consts::TAU;
        let (mut ph, mut lfsr, mut bit) = (0.0f32, 1u16, 1.0f32);
        for (i, o) in self.out.iter_mut().enumerate() {
            let t = i as f32 / RATE as f32;
            let gust = 0.5 + 0.3 * (tau * t / 16.0).sin() + 0.2 * (tau * t / (AMB_SECS / 7.0) + 1.0).sin();
            ph += clock * NOISE_CLOCK * (0.8 + 0.4 * gust) / RATE as f32;
            while ph >= 1.0 {
                ph -= 1.0;
                let b = (lfsr ^ (lfsr >> 1)) & 1;
                lfsr = (lfsr >> 1) | (b << 14);
                bit = if lfsr & 1 == 1 { 1.0 } else { -1.0 };
            }
            *o += bit * vol * gust * gust;
        }
    }

    fn tick(&mut self, at: f32, len: f32, vol: f32) {
        let n = self.out.len();
        let (s0, m) = ((at.rem_euclid(AMB_SECS) * RATE as f32) as usize, (len * RATE as f32) as usize);
        let mut lfsr = 1u16;
        for i in 0..m {
            let b = (lfsr ^ (lfsr >> 1)) & 1;
            lfsr = (lfsr >> 1) | (b << 14);
            let x = i as f32 / m as f32;
            self.out[(s0 + i) % n] += if lfsr & 1 == 1 { vol } else { -vol } * (1.0 - x).powi(4);
        }
    }
}

pub fn ambience(theme: usize) -> Vec<f32> {
    let mut a = Amb { out: vec![0.0; (AMB_SECS * RATE as f32) as usize], wow: if theme == 2 { 0.007 } else { 0.0035 } };
    let swell = |x: f32| (std::f32::consts::PI * x).sin().powi(2);
    let ring = |x: f32| (1.0 - x).powi(3) * (x * 200.0).min(1.0);
    let pluck = |x: f32| (1.0 - x).powi(2) * (x * 100.0).min(1.0);
    let pads = |a: &mut Amb, chords: &[[f32; 5]; 4], vol: f32| {
        for (c, ch) in chords.iter().enumerate() {
            let at = c as f32 * 16.0 - 3.0;
            for (j, &f) in ch.iter().enumerate() {
                a.note(f, at + j as f32 * 0.4, 22.0, vol, 0, &swell);
                a.note(f * 2.0, at + 1.5 + j as f32 * 0.3, 19.0, vol * 0.17, 1, &swell);
            }
            a.note(ch[0] / 2.0, at, 22.0, vol * 1.3, 0, &swell);
        }
    };
    match theme {
        1 => {
            let chords: [[f32; 5]; 4] = [[110.0, 130.81, 164.81, 196.0, 246.94], [87.31, 110.0, 130.81, 164.81, 220.0],
                [130.81, 164.81, 196.0, 246.94, 329.63], [98.0, 123.47, 146.83, 164.81, 196.0]];
            pads(&mut a, &chords, 0.045);
            for st in 0..256usize {
                let (c, k) = (st / 64, st % 16);
                if (st / 16) % 4 == 3 && k >= 8 {
                    continue;
                }
                let f = chords[c][[0, 1, 2, 3, 4, 3, 2, 1][k % 8]] * if k < 8 { 2.0 } else { 4.0 };
                let at = st as f32 * 0.25;
                a.note(f, at, 0.6, 0.022, 1, &pluck);
                a.note(f, at + 0.75, 0.6, 0.009, 1, &pluck);
                if st % 4 == 2 {
                    a.tick(at, 0.04, 0.003);
                }
            }
            a.wind(250.0, 0.006);
        }
        2 => {
            let chords: [[f32; 4]; 4] = [[293.66, 369.99, 440.0, 659.26], [246.94, 293.66, 369.99, 440.0],
                [196.0, 293.66, 369.99, 440.0], [220.0, 293.66, 329.63, 440.0]];
            for (c, ch) in chords.iter().enumerate() {
                for (j, &f) in ch.iter().enumerate() {
                    a.note(f / 2.0, c as f32 * 16.0 - 4.0 + j as f32 * 1.2, 24.0, 0.05, 0, &swell);
                }
                a.note(ch[0] / 4.0, c as f32 * 16.0 - 4.0, 24.0, 0.07, 0, &swell);
            }
            for &(at, f) in &[(3.0, 587.33), (11.0, 739.99), (19.5, 493.88), (27.0, 659.26), (35.0, 587.33), (43.5, 880.0), (51.0, 493.88), (59.0, 659.26)] {
                for e in 0..6 {
                    a.note(f / 2.0, at + e as f32 * 0.75, 2.5, 0.025 * 0.6f32.powi(e), 0, &ring);
                }
            }
        }
        _ => {
            let chords: [[f32; 5]; 4] = [[146.83, 185.0, 220.0, 277.18, 329.63], [123.47, 146.83, 185.0, 220.0, 277.18],
                [98.0, 123.47, 146.83, 185.0, 220.0], [164.81, 196.0, 246.94, 293.66, 369.99]];
            pads(&mut a, &chords, 0.07);
            for &(at, f) in &[(2.5, 880.0), (9.0, 739.99), (11.2, 587.33), (21.0, 659.26), (30.5, 987.77), (33.0, 880.0),
                              (42.0, 739.99), (49.5, 587.33), (51.0, 659.26), (58.5, 1174.66)] {
                for e in 0..4 {
                    a.note(f, at + e as f32 * 0.45, 3.0, 0.012 * 0.45f32.powi(e), 0, &ring);
                }
            }
            a.wind(300.0, 0.012);
        }
    }
    let peak = a.out.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-6);
    a.out.iter_mut().for_each(|x| *x *= 0.6 / peak);
    a.out
}

pub fn play(commands: &mut Commands, bank: &mut Option<ResMut<Bank>>, queued: &mut Vec<Snd>, vol: [f32; 4]) {
    let Some(bank) = bank.as_mut() else { return queued.clear() };
    let mut seen = vec![];
    for s in queued.drain(..) {
        let v = vol[0] * match s { Snd::PortalShoot(_) | Snd::PortalOpen(_) => vol[2], Snd::Step | Snd::Thud(_) => vol[3], _ => vol[1] };
        if v <= 0.0 || seen.contains(&s) || seen.len() >= 8 {
            continue;
        }
        seen.push(s);
        bank.1 += 1;
        if let Some(h) = bank.0.get(&s) {
            commands.spawn((AudioPlayer(h[bank.1 % 3].clone()), PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(v))));
        }
    }
}

