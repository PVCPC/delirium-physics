use crate::anim::{self, Animation};
use crate::animblend::{self, MoverPlayer, Player};
use crate::staticcount::Anim;
use crate::tasks::{ABORT_IMMEDIATE, ABORT_LEAVE};

pub const HIT_WALL_KIND: u32 = 411;
pub const COWER_KIND: u32 = 412;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    pub group: i32,
    pub id: i32,
    pub dict: &'static str,
    pub clip: &'static str,
    pub flags: u32,
    pub layer: u8,
}

pub const HIT_WALL: Row = Row { group: 0, id: 1, dict: "ped", clip: "hit_wall", flags: 0x8400, layer: 3 };

pub const DUCK_COWER: Row = Row { group: 0, id: 3, dict: "ped", clip: "duck_cower", flags: 0x8420, layer: 3 };

pub static AVOIDS: [Row; 8] = avoid(95, "avoids", 0xD600);
pub static EV_DIVES: [Row; 8] = avoid(96, "ev_dives", 0xC600);

const fn avoid(group: i32, dict: &'static str, flags: u32) -> [Row; 8] {
    const CLIPS: [&str; 8] = ["avoid_fl", "avoid_fr", "avoid_bl", "avoid_br", "avoid_lf", "avoid_lb", "avoid_rf", "avoid_rb"];
    let mut rows = [Row { group, id: 0, dict, clip: "", flags, layer: 3 }; 8];
    let mut k = 0;
    while k < 8 {
        (rows[k].id, rows[k].clip) = (0xAF + k as i32, CLIPS[k]);
        k += 1;
    }
    rows
}

pub const RUN_ANIM_KIND: u32 = 400;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    pub duration: f32,
    pub mover_track: bool,
}

#[derive(Clone, Debug)]
pub struct Playing {
    pub row: &'static Row,
    pub player: Player,
    pub mover_track: bool,
    pub mine: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Anims {
    pub players: Vec<Playing>,
    pub known: Vec<(&'static Row, Clip)>,
}

impl Anims {
    pub fn clip(&self, row: &Row) -> Option<Clip> {
        self.known.iter().find(|(r, _)| (r.group, r.id) == (row.group, row.id)).map(|&(_, c)| c)
    }

    pub fn play(&mut self, row: &'static Row, clip: Clip, blend: f32) -> usize {
        let (mut others, mut same) = (false, None);
        for (i, o) in self.players.iter_mut().enumerate().filter(|(_, o)| o.player.duration.is_some()) {
            if (o.row.group, o.row.id) == (row.group, row.id) {
                same = Some(i);
                continue;
            }
            if o.player.priority != row.layer {
                continue;
            }
            if blend > 0.0 {
                let out = o.player.signed_blend_rate();
                if o.player.weight <= 0.0 {
                    o.player.set_blend(-1.0);
                } else if out >= 0.0 || blend > o.player.blend_speed() {
                    o.player.set_blend(-(if blend < 0.05 { 0.05 } else { blend }));
                }
                o.player.remove_blended_out = true;
            }
            o.mine = false;
            others = true;
        }
        if let Some(i) = same {
            let p = &mut self.players[i].player;
            p.set_blend(blend);
            set_flags(p, row.flags);
            if p.phase == 1.0 || p.finished {
                p.restart();
            }
            return i;
        }
        let mut p = Player::new(clip.duration);
        p.priority = row.layer;
        p.weight = if others || row.layer != 0 { 0.0 } else { 1.0 };
        p.playing = true;
        set_flags(&mut p, row.flags);
        p.set_blend(blend);
        self.players.push(Playing { row, player: p, mover_track: clip.mover_track, mine: false });
        self.players.len() - 1
    }

    pub fn step(&mut self, dt: f32) {
        for p in &mut self.players {
            p.player.blend_weight(dt);
        }
        let mut ps: Vec<Player> = self.players.iter().map(|p| p.player.clone()).collect();
        animblend::sync_phases(&mut ps, dt);
        for (slot, p) in self.players.iter_mut().zip(ps) {
            slot.player = p;
        }
        self.players.retain(|p| p.player.duration.is_some());
    }

    pub fn mine(&self) -> Option<&Playing> {
        self.players.iter().find(|p| p.mine)
    }

    fn mine_mut(&mut self) -> Option<&mut Playing> {
        self.players.iter_mut().find(|p| p.mine)
    }

    pub fn ended(&mut self) -> bool {
        let ended = self.mine().is_some_and(|p| p.player.finished && !p.player.looped);
        if let Some(p) = self.mine_mut().filter(|_| ended) {
            p.mine = false;
        }
        ended
    }

    pub fn mover_weights(&self) -> Vec<f32> {
        let gait = MoverPlayer { clip: true, layer: 0, weight: 1.0, flags: 0x20, mover_track: true };
        let list: Vec<MoverPlayer> = std::iter::once(gait)
            .chain(self.players.iter().map(|p| MoverPlayer {
                clip: p.player.duration.is_some(),
                layer: p.player.priority as i32,
                weight: p.player.weight,
                flags: p.row.flags & !animblend::OWN_CHANNEL | if p.player.own_channel { animblend::OWN_CHANNEL } else { 0 },
                mover_track: p.mover_track,
            }))
            .collect();
        animblend::mover_weights(&list, 0.0)[1..=self.players.len()].to_vec()
    }

    pub fn mover<'a>(&self, clip: impl Fn(&Row) -> Option<&'a Animation>) -> [f32; 3] {
        let mut sum = [0.0f32; 3];
        for (p, w) in self.players.iter().zip(self.mover_weights()) {
            if let Some(c) = clip(p.row) {
                let d = mover_delta(c, p.row.flags, p.player.prev_phase, p.player.phase);
                for k in 0..3 {
                    sum[k] += d[k] * w;
                }
            }
        }
        sum
    }

    pub fn list(&self) -> Vec<Anim> {
        self.players
            .iter()
            .map(|p| Anim {
                clip: p.player.duration.is_some(),
                layer: p.player.priority as i32,
                weight: p.player.weight,
                flags: p.row.flags,
                mover_track: p.mover_track,
            })
            .collect()
    }
}

fn set_flags(p: &mut Player, flags: u32) {
    p.mask = (flags & 0xF) as u8;
    p.active = true;
    p.loops = flags & 0x20 != 0;
    p.synced = flags & 0x100 != 0;
    p.blend_out_at_end = flags & 0x8000 != 0;
    p.remove_blended_out = flags & 0x4000 != 0;
    p.own_channel = flags & 0x2_0000 != 0;
    p.skip_weight = flags & 0x40_0000 != 0;
}

pub fn mover_delta(clip: &Animation, flags: u32, from: f32, to: f32) -> [f32; 3] {
    let Some(t) = clip.tracks.iter().find(|t| t.kind == anim::MOVER_TRANSLATION) else { return [0.0; 3] };
    let at = |phase: f32| -> [f32; 3] {
        let x = phase.clamp(0.0, 1.0) * clip.frames.saturating_sub(1) as f32;
        let (i, f) = (x.floor() as usize, x.fract());
        let (a, b) = (t.sample(i), t.sample(i + 1));
        [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * f)
    };
    let (a, b) = (at(from), at(to));
    let bit = [0x200, 0x400, 0x800];
    [0, 1, 2].map(|k| if flags & bit[k] != 0 { b[k] - a[k] } else { 0.0 })
}

#[derive(Clone, Debug)]
pub struct RunAnim {
    row: &'static Row,
    blend: f32,
    speed: f32,
    start: f32,
    kind: u32,
    leave_nicely: bool,
    done: bool,
    started: bool,
}

impl RunAnim {
    pub fn hit_wall() -> RunAnim {
        RunAnim { row: &HIT_WALL, blend: 4.0, speed: 1.0, start: 0.0, kind: HIT_WALL_KIND, leave_nicely: false, done: false, started: false }
    }

    pub fn cower() -> RunAnim {
        RunAnim { row: &DUCK_COWER, blend: 4.0, speed: 1.0, start: 0.0, kind: COWER_KIND, leave_nicely: false, done: false, started: false }
    }

    pub fn plain(row: &'static Row, blend: f32, speed: f32) -> RunAnim {
        RunAnim { row, blend, speed, start: 0.0, kind: RUN_ANIM_KIND, leave_nicely: false, done: false, started: false }
    }

    pub fn kind(&self) -> u32 {
        self.kind
    }

    pub fn blend(&self) -> f32 {
        self.blend
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn start(&self) -> f32 {
        self.start
    }

    pub fn row(&self) -> &'static Row {
        self.row
    }

    pub fn process(&mut self, anims: &mut Anims) -> bool {
        if self.done {
            return true;
        }
        if !self.started {
            match anims.clip(self.row) {
                Some(c) => {
                    let i = anims.play(self.row, c, self.blend);
                    let (p, at) = (&mut anims.players[i], self.start.clamp(0.0, 1.0));
                    (p.player.rate, p.player.phase, p.player.prev_phase, p.mine) = (self.speed, at, at, true);
                    self.started = true;
                }
                None => self.done = true,
            }
        }
        if self.started && (anims.ended() || anims.mine().is_none()) {
            (self.started, self.done) = (false, true);
        }
        self.done
    }

    pub fn abortable(&mut self, anims: &mut Anims, urgency: u8) -> bool {
        if urgency == ABORT_LEAVE && !self.leave_nicely {
            return false;
        }
        if self.started {
            if let Some(p) = anims.mine_mut() {
                p.player.own_channel = false;
                p.player.remove_blended_out = true;
                p.player.set_blend(if urgency == ABORT_IMMEDIATE { -1000.0 } else { -4.0 });
                p.mine = false;
            }
            (self.started, self.done) = (false, true);
        }
        true
    }

    pub fn clean_up(&mut self, anims: &mut Anims) {
        if !self.started {
            return;
        }
        if let Some(p) = anims.mine_mut() {
            p.player.remove_blended_out = true;
            if !self.leave_nicely && p.player.weight > 0.0 && p.player.signed_blend_rate() >= 0.0 {
                p.player.set_blend(-4.0);
            }
            p.mine = false;
        }
        self.started = false;
    }
}

