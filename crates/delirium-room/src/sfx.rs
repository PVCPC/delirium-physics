use bevy::audio::{AddAudioSource, Decodable, Source};
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
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.0.len() as f32 / RATE as f32))
    }
}

impl Decodable for Sfx {
    type DecoderItem = f32;
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
        Snd::Shot(_) => vec![Voice { fade: 0.6, ..nz(900.0 * p, 6000.0, 0.6, 0.6) }, Voice { duty: 0.125, fade: 0.8, ..sq(110.0 * p, 520.0, 0.45, 0.3) },
                             nz(4000.0, 800.0, 0.1, 0.9)],
        Snd::Hit => vec![Voice { duty: 0.25, ..sq(330.0 * p, 70.0, 0.09, 0.7) }, nz(5000.0, 1500.0, 0.05, 0.4)],
        Snd::Kill => [440.0, 330.0, 262.0, 196.0, 131.0].iter().enumerate()
            .map(|(i, &f)| Voice { at: i as f32 * 0.07, fade: 1.0, duty: 0.25, ..sq(f * p, f * p, 0.09, 0.45) }).collect(),
        Snd::Boom => vec![Voice { fade: 0.8, ..nz(3000.0 * p, 1000.0, 1.1, 1.0) }, Voice { fade: 1.0, ..nz(700.0 * p, 120.0, 0.8, 0.6) }],
        Snd::Punch => vec![nz(4000.0 * p, 800.0, 0.07, 0.7), sq(160.0 * p, 60.0, 0.07, 0.6)],
        Snd::Throw => vec![Voice { duty: 0.125, fade: 1.0, ..sq(250.0 * p, 800.0, 0.14, 0.3) }],
        Snd::Crash => vec![Voice { fade: 0.9, ..nz(3000.0 * p, 600.0, 0.45, 1.0) }, sq(90.0 * p, 35.0, 0.3, 0.6)],
    }
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
            } else {
                phase = phase.fract();
                if phase < v.duty { 1.0 } else { -1.0 }
            };
            let a = v.vol * (1.0 - t).powf(v.fade);
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

pub fn plugin(app: &mut App) {
    app.add_audio_source::<Sfx>().add_systems(Startup, |mut commands: Commands, mut assets: ResMut<Assets<Sfx>>| {
        let all = (0..7).map(Snd::Shot).chain([Snd::Hit, Snd::Kill, Snd::Boom, Snd::Punch, Snd::Throw, Snd::Crash]);
        let bank = all.map(|s| (s, [0, 1, 2].map(|k| assets.add(Sfx(render(&voices(s, k)).into()))))).collect();
        commands.insert_resource(Bank(bank, 0));
    });
}

pub fn play(commands: &mut Commands, bank: &mut Option<ResMut<Bank>>, queued: &mut Vec<Snd>, on: bool) {
    let Some(bank) = bank.as_mut() else { return queued.clear() };
    let mut seen = vec![];
    for s in queued.drain(..) {
        if !on || seen.contains(&s) || seen.len() >= 8 {
            continue;
        }
        seen.push(s);
        bank.1 += 1;
        if let Some(h) = bank.0.get(&s) {
            commands.spawn((AudioPlayer(h[bank.1 % 3].clone()), PlaybackSettings::DESPAWN));
        }
    }
}

