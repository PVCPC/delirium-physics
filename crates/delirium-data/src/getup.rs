use crate::animblend::{self, Player};
use crate::mathrt;
use crate::rng::Rand;

pub const TASK: u32 = 2117;

pub const RAGDOLL_TRANS: i32 = 3;
pub const RAGDOLL_TRANS_BACK: i32 = 4;
pub const GET_UP: i32 = 5;
pub const GET_UP_BACK: i32 = 6;

pub mod id {
    pub const INJ_FRONT_TO_DEFAULT: i32 = 0x5C;
    pub const INJ_LSIDE_TO_DEFAULT: i32 = 0x5D;
    pub const INJ_RSIDE_TO_DEFAULT: i32 = 0x5E;
    pub const PREACHER_TRANS: i32 = 0x5F;
    pub const SEATED_TRANS: i32 = 0x60;
    pub const RECOVER_ABDOMEN: i32 = 0x61;
    pub const RECOVER_BALANCE: i32 = 0x62;
    pub const INJ_BACK_TO_DEFAULT: i32 = 0x63;
    pub const GET_UP_FAST: i32 = 0x64;
    pub const GET_UP_INJURED: i32 = 0x65;
    pub const GET_UP_NORMAL: i32 = 0x66;
    pub const GET_UP_SLOW: i32 = 0x67;
    pub const MOVE_IDLE: i32 = 0xF;
}

#[derive(Clone, Copy, Debug)]
pub struct ClipDef {
    pub group: i32,
    pub id: i32,
    pub dict: &'static str,
    pub clip: &'static str,
    pub flags: u32,
}

const TRANS: u32 = 0x4100_C600;
const UP: u32 = 0x0100_8600;
pub const IDLE_FLAGS: u32 = 0x4000_0020;

pub const CLIPS: [ClipDef; 19] = [
    ClipDef { group: RAGDOLL_TRANS, id: id::INJ_FRONT_TO_DEFAULT, dict: "ragdoll_trans", clip: "inj_front_to_default", flags: TRANS },
    ClipDef { group: RAGDOLL_TRANS, id: id::INJ_LSIDE_TO_DEFAULT, dict: "ragdoll_trans", clip: "inj_lside_to_default", flags: TRANS },
    ClipDef { group: RAGDOLL_TRANS, id: id::INJ_RSIDE_TO_DEFAULT, dict: "ragdoll_trans", clip: "inj_rside_to_default", flags: TRANS },
    ClipDef { group: RAGDOLL_TRANS, id: id::PREACHER_TRANS, dict: "ragdoll_trans", clip: "preacher_trans", flags: TRANS },
    ClipDef { group: RAGDOLL_TRANS, id: id::SEATED_TRANS, dict: "ragdoll_trans", clip: "seated_trans", flags: TRANS },
    ClipDef { group: RAGDOLL_TRANS, id: id::RECOVER_ABDOMEN, dict: "ragdoll_trans", clip: "recover_abdomen", flags: TRANS },
    ClipDef { group: RAGDOLL_TRANS, id: id::RECOVER_BALANCE, dict: "ragdoll_trans", clip: "recover_balance", flags: TRANS },
    ClipDef { group: RAGDOLL_TRANS_BACK, id: id::INJ_BACK_TO_DEFAULT, dict: "ragdoll_trans_back", clip: "inj_back_to_default", flags: 0x4000_4600 },
    ClipDef { group: GET_UP, id: id::GET_UP_FAST, dict: "get_up", clip: "get_up_fast", flags: UP },
    ClipDef { group: GET_UP, id: id::GET_UP_INJURED, dict: "get_up", clip: "get_up_injured", flags: UP },
    ClipDef { group: GET_UP, id: id::GET_UP_NORMAL, dict: "get_up", clip: "get_up_normal", flags: UP },
    ClipDef { group: GET_UP, id: id::GET_UP_SLOW, dict: "get_up", clip: "get_up_slow", flags: UP },
    ClipDef { group: GET_UP_BACK, id: id::GET_UP_FAST, dict: "get_up_back", clip: "get_up_fast", flags: UP },
    ClipDef { group: GET_UP_BACK, id: id::GET_UP_NORMAL, dict: "get_up_back", clip: "get_up_normal", flags: UP },
    ClipDef { group: GET_UP_BACK, id: id::GET_UP_SLOW, dict: "get_up_back", clip: "get_up_slow", flags: UP },
    ClipDef { group: 43, id: 0xA7, dict: "injured", clip: "inj_default_to_rside", flags: 0x4000_C600 },
    ClipDef { group: 43, id: 0xA8, dict: "injured", clip: "inj_rside_idle", flags: 0x4000_4020 },
    ClipDef { group: 43, id: 0xA9, dict: "injured", clip: "inj_default_to_back", flags: 0x4000_C600 },
    ClipDef { group: 43, id: 0xAA, dict: "injured", clip: "inj_back_idle", flags: 0x4000_4020 },
];

pub fn clip_def(group: i32, id: i32) -> Option<&'static ClipDef> {
    CLIPS.iter().find(|c| c.group == group && c.id == id)
}


pub const SKELETON_BONES: [usize; 2] = [0x50, 0x5A];

const PI: f32 = f32::from_bits(0x4049_0FDB);
const QUARTER_PI: f32 = f32::from_bits(0x3F49_0FDB);
const TWO_PI: f32 = f32::from_bits(0x40C9_0FDB);
const HALF_PI: f32 = f32::from_bits(0x3FC9_0FDB);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointSet {
    pub points: Vec<[f32; 4]>,
    pub weights: Vec<f32>,
    pub total: f32,
}

impl PointSet {
    pub fn push(&mut self, at: [f32; 3], w: f32) {
        if w > 0.0 {
            self.points.push([at[0], at[1], at[2], 0.0]);
            self.weights.push(w);
            self.total += w;
        }
    }

    pub fn centroid(&self) -> [f32; 4] {
        let mut out = [0.0f32; 4];
        if self.points.is_empty() {
            return out;
        }
        let inv = 1.0f32 / self.total;
        for (p, &w) in self.points.iter().zip(&self.weights) {
            let s = w * inv;
            for k in 0..4 {
                out[k] = p[k].mul_add(s, out[k]);
            }
        }
        out
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fit {
    pub at: [f32; 3],
    pub yaw: f32,
}

pub fn wrap(mut x: f32) -> f32 {
    let range = PI - -PI;
    while x < -PI {
        x += range;
    }
    while x >= PI {
        x -= range;
    }
    x
}

pub fn fit(a: &PointSet, b: &PointSet) -> (f32, Fit) {
    let (ca, cb) = (a.centroid(), b.centroid());
    let inv = 1.0f32 / a.total;
    let (mut cross, mut dot) = (0.0f32, 0.0f32);
    for i in 0..a.points.len() {
        let (ax, ay, bx, by) = (a.points[i][0], a.points[i][1], b.points[i][0], b.points[i][1]);
        let bxay = bx * ay;
        let byay = by * ay;
        let c = by.mul_add(ax, -bxay);
        let w = a.weights[i] * inv;
        let d = bx.mul_add(ax, byay);
        cross = c.mul_add(w, cross);
        dot = d.mul_add(w, dot);
    }
    let (cax, cay, cbx, cby) = (ca[0], ca[1], cb[0], cb[1]);
    let t10 = cbx * cay;
    let t11 = cby * cay;
    let c0 = cby.mul_add(cax, -t10);
    let d0 = cbx.mul_add(cax, t11);
    let theta = mathrt::atan2((cross - c0) as f64, (dot - d0) as f64) as f32;
    let yaw = -theta;
    let cos = mathrt::cos(theta as f64) as f32;
    let sin = mathrt::sin(theta as f64) as f32;
    let (f30, f29, f31) = (cos * cbx, sin * cby, sin * cbx);
    let x = (cax - f30) - f29;
    let y = -cos.mul_add(cby, -(f31 + cay));
    let z = ca[2] - cb[2];
    let fit = Fit { at: [x, y, z], yaw };

    let identity = wrap(yaw) == 0.0 && x == 0.0 && z == 0.0;
    let (sin, cos) = {
        let t = wrap(yaw);
        (mathrt::sin(t as f64) as f32, mathrt::cos(t as f64) as f32)
    };
    let mut acc = 0.0f32;
    for i in 0..a.points.len() {
        let mut p = b.points[i];
        if !identity {
            let (px, py) = (p[0], p[1]);
            let sy = sin * py;
            let cy = cos * py;
            p[0] = cos.mul_add(px, -sy);
            p[1] = sin.mul_add(px, cy);
            for (k, add) in [x, y, z, 0.0].into_iter().enumerate() {
                p[k] += add;
            }
        }
        let g: [f32; 4] = std::array::from_fn(|k| a.points[i][k] - p[k]);
        let s = g[2] * g[2];
        let s = g[1].mul_add(g[1], s);
        let s = g[0].mul_add(g[0], s);
        acc = (s * a.weights[i]).mul_add(inv, acc);
    }
    (acc, fit)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct After {
    pub prev: u32,
    pub wish: Option<(i32, i32)>,
    pub player: bool,
    pub health: f32,
    pub severity: f32,
    pub agility: u8,
    pub wanted: bool,
    pub network: bool,
}

pub mod after {
    pub const SHOT: u32 = 0x83B;
    pub const HIGH_FALL: u32 = 0x83C;
    pub const BALANCE: u32 = 0x83D;
    pub const EXPLOSION: u32 = 0x83E;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Made {
    pub state: i32,
    pub group: i32,
    pub id: i32,
    pub want: i32,
    pub delay: i32,
    pub named_clip: Option<(u32, u32)>,
    pub vehicle: bool,
}

impl Made {
    pub fn plain(state: i32, want: i32) -> Made {
        Made { state, group: -1, id: -1, want, delay: 0, named_clip: None, vehicle: false }
    }

    pub fn scripted(dictionary: u32, clip: u32, vehicle: bool) -> Made {
        Made { named_clip: Some((dictionary, clip)), vehicle, ..Made::plain(9, -1) }
    }
}

pub fn made(a: &After, rand: &mut Rand) -> Made {
    let (mut state, mut group, mut id) = (2, -1, -1);
    if let Some((byte, packed)) = a.wish.filter(|w| w.0 != 0) {
        state = byte;
        if byte == 1 {
            id = packed >> 16;
            group = packed as i16 as i32;
        }
    }
    let mut want = id::GET_UP_NORMAL;
    if a.player {
        want = id::GET_UP_FAST;
        if a.wanted && !a.network && a.health < 125.0 && matches!(a.prev, after::EXPLOSION | after::HIGH_FALL) {
            want = id::GET_UP_NORMAL;
        }
    } else if a.prev != 0 {
        if a.health < 100.0 {
            want = id::GET_UP_INJURED;
        } else if matches!(a.prev, after::SHOT | after::EXPLOSION) || a.health < 150.0 || a.severity > 20.0 {
            want = id::GET_UP_SLOW;
        } else if a.agility > 3 && a.prev == after::BALANCE && rand.float_in_range(0.0, 1.0) > 0.5 {
            want = id::GET_UP_FAST;
        }
    }
    let mut delay = 0;
    if a.severity > 20.0 && !a.player {
        delay = if a.agility < 3 { rand.int_in_range(4000, 6000) } else { rand.int_in_range(2000, 4000) };
    }
    Made { state, group, id, want, delay, named_clip: None, vehicle: false }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipInfo {
    pub duration: f32,
    pub flags: u32,
}

pub trait Ped {
    fn dt(&self) -> f32;
    fn ragdolled(&self) -> bool;
    fn is_player(&self) -> bool;
    fn near_player(&self) -> bool {
        self.is_player()
    }
    fn network(&self) -> bool {
        false
    }
    fn ground_under_root(&self) -> bool;
    fn move_group(&self) -> i32;
    fn fit(&self, group: i32, id: i32) -> Option<(f32, Fit)>;
    fn root(&self) -> Fit;
    fn blocked(&self, at: &Fit) -> bool;
    fn cut_off(&self, from: [f32; 3], to: &Fit) -> bool;
    fn ground_fit(&self, at: [f32; 3]) -> Option<[f32; 3]>;
    fn ground_below(&self, at: [f32; 3]) -> Option<f32>;
    fn landed(&mut self) {}
    fn stand_where_he_can(&self, _at: [f32; 3], _flags: u32) -> Option<[f32; 3]> {
        None
    }
    fn node_place(&self, _at: [f32; 3], _radius: f32) -> Option<[f32; 3]> {
        None
    }
    fn held(&self) -> bool {
        false
    }
    fn stand_helper(&self) -> bool {
        false
    }
    fn on_mover(&self) -> bool {
        false
    }
    fn gave_up(&mut self) {}
    fn clip(&self, group: i32, id: i32) -> Option<ClipInfo>;
    fn named_clip(&self, _dictionary: u32, _clip: u32) -> Option<ClipInfo> {
        None
    }
    fn local_player(&self) -> bool {
        self.is_player()
    }
    fn has_skeleton(&self) -> bool {
        true
    }
    fn in_combat(&self) -> bool {
        false
    }
    fn end_tag(&self, _group: i32, _id: i32) -> Option<f32> {
        None
    }
    fn stick(&self) -> f32 {
        0.0
    }
    fn dying(&self) -> bool {
        false
    }
    fn waits_short(&self) -> bool {
        false
    }
    fn rammed(&self) -> bool {
        false
    }
    fn on_ground(&self) -> bool {
        true
    }
    fn in_water(&self) -> bool {
        false
    }
    fn thrown(&self) -> bool {
        false
    }
    fn land(&mut self) {}
    fn lies(&self) -> u8 {
        4
    }
    fn crush(&mut self, _dt: f32) {}
    fn relax(&mut self);
    fn leave_nm(&mut self, at: &Fit);
}

#[derive(Clone, Debug)]
pub struct Playing {
    pub group: i32,
    pub id: i32,
    pub player: Player,
    pub mine: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub weight: f32,
    pub rate: f32,
    pub vel: [f32; 2],
}

impl Frame {
    pub fn slide(&mut self, speed: f32, right: f32, ahead: f32, object: Option<&mut f32>) {
        if let Some(cell) = object {
            *cell = speed;
        }
        self.vel = [right, ahead];
    }

    pub fn move_vel(&self, anim: [f32; 2]) -> [f32; 2] {
        if self.weight <= 0.0 {
            return anim;
        }
        let w = self.weight;
        [(1.0 - w) * anim[0] + w * self.vel[0], (1.0 - w) * anim[1] + w * self.vel[1]]
    }
}

pub const NM_FADE: f32 = -4.0;

#[derive(Clone, Debug)]
pub struct Blender {
    pub players: Vec<Playing>,
    pub frame: Frame,
}

impl Blender {
    pub fn new(frame_weight: f32) -> Blender {
        Blender { players: vec![], frame: Frame { weight: frame_weight, ..Frame::default() } }
    }

    pub fn player(&self) -> Option<&Playing> {
        self.players.iter().find(|p| p.mine)
    }

    pub fn player_mut(&mut self) -> Option<&mut Playing> {
        self.players.iter_mut().find(|p| p.mine)
    }

    pub fn forget(&mut self, rate: f32, flags: bool) {
        if let Some(p) = self.player_mut() {
            p.player.set_blend(rate);
            if flags {
                p.player.remove_blended_out = true;
                p.player.blend_out_at_end = true;
            }
            p.mine = false;
        }
    }

    pub fn play(&mut self, ped: &dyn Ped, speed: f32, group: i32, id: i32) -> bool {
        ped.clip(group, id).is_some_and(|info| self.play_info(info, speed, group, id))
    }

    fn play_info(&mut self, info: ClipInfo, speed: f32, group: i32, id: i32) -> bool {
        let mut others = false;
        for o in self.players.iter_mut().filter(|o| o.player.priority == 0) {
            if speed > 0.0 {
                let out = o.player.signed_blend_rate();
                if o.player.weight <= 0.0 {
                    o.player.set_blend(-1.0);
                } else if out >= 0.0 || speed > o.player.blend_speed() {
                    o.player.set_blend(-(if speed < 0.05 { 0.05 } else { speed }));
                }
                o.player.remove_blended_out = true;
            }
            o.mine = false;
            others = true;
        }
        let mut p = Player::new(info.duration);
        p.weight = if others { 0.0 } else { 1.0 };
        p.active = true;
        p.playing = true;
        p.loops = info.flags & 0x20 != 0;
        p.blend_out_at_end = info.flags & 0x8000 != 0;
        p.remove_blended_out = info.flags & 0x4000 != 0;
        p.set_blend(speed);
        self.players.push(Playing { group, id, player: p, mine: true });
        true
    }

    pub fn step(&mut self, dt: f32) {
        if self.frame.rate != 0.0 {
            self.frame.weight = (self.frame.rate.mul_add(dt, self.frame.weight)).max(0.0);
        }
        for p in &mut self.players {
            p.player.blend_weight(dt);
        }
        let mut ps: Vec<Player> = self.players.iter().map(|p| p.player.clone()).collect();
        animblend::sync_phases(&mut ps, dt);
        for (slot, p) in self.players.iter_mut().zip(ps) {
            slot.player = p;
        }
    }

    pub fn ended(&mut self) -> bool {
        let ended = self.player().is_some_and(|p| p.player.finished && !p.player.looped);
        if let Some(p) = self.player_mut().filter(|_| ended) {
            p.mine = false;
        }
        ended
    }

    pub fn prune(&mut self) {
        self.players.retain(|p| p.player.duration.is_some());
    }

    pub fn follow(&mut self, ped: &dyn Ped, group: i32, id: i32) -> bool {
        self.play(ped, 8.0, group, id)
    }

    pub fn mover_weights(&self, clip: impl Fn(&Playing) -> Option<(u32, bool)>) -> Vec<f32> {
        let list: Vec<animblend::MoverPlayer> = self.players.iter().map(|p| {
            let (flags, mover_track) = clip(p).unwrap_or((0, false));
            animblend::MoverPlayer {
                clip: p.player.duration.is_some() && clip(p).is_some(),
                layer: p.player.priority as i32,
                weight: p.player.weight,
                flags: flags & !animblend::OWN_CHANNEL | if p.player.own_channel { animblend::OWN_CHANNEL } else { 0 },
                mover_track,
            }
        }).collect();
        let mut w = animblend::mover_weights(&list, self.frame.weight);
        w.truncate(self.players.len());
        w
    }

    pub fn weights(&self) -> (f32, Vec<(usize, f32)>) {
        let ps: Vec<Player> = self.players.iter().map(|p| p.player.clone()).collect();
        let w = animblend::priority_weights(&ps);
        let frame = self.frame.weight.clamp(0.0, 1.0);
        (frame, w.into_iter().enumerate().map(|(i, w)| (i, w * (1.0 - frame))).collect())
    }
}

pub fn response_gives_way(kind: u32) -> bool {
    matches!(kind, 2118 | 217 | 218 | 250 | 268)
}

pub fn urgent(event: u32, response8: u32, response12: u32) -> bool {
    event == 120 || response_gives_way(if response8 != 0 { response8 } else { response12 })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NmNext {
    Nothing,
    Task(u32),
    Relax,
}

pub fn complex_next(done: bool, ragdolled: bool, sub_is_simple_nm: bool, sub: u32, sub_next: Option<u32>) -> NmNext {
    if done || !ragdolled {
        return NmNext::Nothing;
    }
    match (sub_is_simple_nm.then_some(sub_next).flatten(), sub) {
        (Some(t), _) => NmNext::Task(t),
        (None, TASK) => NmNext::Relax,
        _ => NmNext::Task(TASK),
    }
}

#[derive(Clone, Debug)]
pub struct BlendFromNm {
    pub ended: bool,
    pub done: bool,
    relaxed: bool,
    timer: i32,
    waited: i32,
    pub delay: i32,
    pub state: i32,
    pub id: i32,
    pub group: i32,
    pub want: i32,
    pub no_clip: bool,
    pub follow_up: bool,
    pub named: bool,
    pub has_end: bool,
    pub end_phase: f32,
    pub blend_out_at: f32,
    pub blender: Blender,
    pub placed: Option<Fit>,
    pub switched: bool,
    pub named_clip: Option<(u32, u32)>,
    pub vehicle: bool,
}

impl BlendFromNm {
    pub fn new(m: &Made) -> Self {
        BlendFromNm {
            ended: false, done: false, relaxed: false, timer: 2000, waited: 0, delay: m.delay, state: m.state,
            id: m.id, group: m.group, want: m.want, no_clip: false, follow_up: false,
            named: m.group != -1 && m.id != -1, has_end: false, end_phase: 1.0, blend_out_at: 0.0,
            blender: Blender::new(1.0), placed: None, switched: false, named_clip: m.named_clip, vehicle: m.vehicle,
        }
    }

    pub fn finished(&self) -> bool {
        self.done || self.ended
    }

    pub fn remade(&self) -> BlendFromNm {
        match self.named_clip.filter(|&(d, c)| d != u32::MAX && c != 0) {
            Some((d, c)) => BlendFromNm::new(&Made::scripted(d, c, self.vehicle)),
            None => BlendFromNm::new(&Made { state: self.state, group: self.group, id: self.id, want: self.want, delay: self.delay, named_clip: None, vehicle: false }),
        }
    }

    fn play_named(&mut self, ped: &dyn Ped) -> bool {
        let flags = if self.vehicle { 0x4000 } else { 0xC000 };
        self.named_clip.and_then(|(d, c)| ped.named_clip(d, c))
            .is_some_and(|info| self.blender.play_info(ClipInfo { flags, ..info }, 1000.0, -1, -1))
    }

    pub fn player(&self) -> Option<&Playing> {
        self.blender.player()
    }

    fn player_mut(&mut self) -> Option<&mut Playing> {
        self.blender.player_mut()
    }

    pub fn process(&mut self, ped: &mut dyn Ped) -> bool {
        if self.done || self.ended {
            return true;
        }
        let player = self.player().is_some();
        if !player && !self.no_clip {
            if self.follow_up {
                self.start(ped);
                return false;
            }
            if !ped.ragdolled() {
                self.ended = true;
                return true;
            }
            self.look(ped);
            return false;
        }
        if ped.ragdolled() {
            self.ended = true;
            return true;
        }
        if self.delay != 0 && player {
            let ms = (ped.dt() * 1000.0) as i32;
            if self.delay > ms {
                self.delay -= ms;
            } else {
                self.delay = 0;
                if let Some(p) = self.player_mut() {
                    p.player.active = true;
                }
                self.blender.frame.rate = NM_FADE;
            }
        }
        if self.no_clip {
            self.done = true;
            return true;
        }
        let blend = {
            let p = self.player().expect("a player").player.clone();
            if p.loops {
                if self.blender.frame.weight < 0.01 {
                    self.blender.forget(-8.0, false);
                    self.done = true;
                    return true;
                }
                None
            } else if self.has_end && matches!(self.group, GET_UP | GET_UP_BACK) && p.phase >= self.end_phase {
                Some(-0.5)
            } else if self.blend_out_at > 0.0 && p.duration.unwrap_or(0.0) * p.phase > self.blend_out_at {
                Some(NM_FADE)
            } else {
                None
            }
        };
        match blend {
            Some(rate) => {
                self.blender.forget(rate, true);
                self.done = true;
                true
            }
            None => false,
        }
    }

    fn start(&mut self, ped: &mut dyn Ped) {
        self.follow_up = false;
        if self.group != RAGDOLL_TRANS && self.group != RAGDOLL_TRANS_BACK {
            return;
        }
        self.group = if self.group == RAGDOLL_TRANS_BACK { GET_UP_BACK } else { GET_UP };
        self.id = if self.want != -1 { self.want } else { id::GET_UP_NORMAL };
        loop {
            if ped.clip(self.group, self.id).is_some() {
                break;
            }
            self.id = match self.id {
                id::GET_UP_INJURED => id::GET_UP_SLOW,
                id::GET_UP_SLOW => id::GET_UP_NORMAL,
                _ => return,
            };
        }
        if !self.blender.play(ped, 8.0, self.group, self.id) {
            return;
        }
        let duration = ped.clip(self.group, self.id).map_or(0.0, |c| c.duration);
        self.blend_out_at = duration - if ped.is_player() { 0.45 } else { 0.3 };
        if ped.is_player() && self.id == id::GET_UP_FAST && ped.network() {
            if let Some(p) = self.player_mut() {
                p.player.rate = 1.1;
            }
        }
    }

    pub fn make_abortable(&mut self, urgency: u8, urgent: bool) -> bool {
        if urgency == 2 {
            self.blender.players.retain(|p| !p.mine);
            self.blender.frame.weight = 0.0;
            self.ended = true;
            return true;
        }
        if !urgent {
            return false;
        }
        if let Some(p) = self.player_mut() {
            p.player.set_blend(-8.0);
            p.player.remove_blended_out = true;
            p.player.blend_out_at_end = true;
            p.mine = false;
        }
        self.blender.frame.rate = -8.0;
        self.ended = true;
        true
    }

    fn look(&mut self, ped: &mut dyn Ped) {
        self.timer += (ped.dt() * 1000.0) as i32;
        let limit = if ped.is_player() { 2000 - 0x6A4 } else { 2000 };
        if self.timer <= limit {
            return;
        }
        self.timer = 0;
        let root = ped.root();
        let mut at = Fit { at: root.at, yaw: 0.0 };
        let picked = self.pick(ped);
        match picked {
            Some(f) => at = f,
            None => at.yaw = root.yaw - HALF_PI,
        }
        if !self.vehicle && let Some(h) = ped.ground_fit(at.at) {
            at.at = [h[0], h[1], h[2] + 1.0];
        }
        if picked.is_some() && (self.id == id::MOVE_IDLE || self.id == id::RECOVER_BALANCE) {
            self.id = -1;
        }
        if !self.vehicle && ped.blocked(&at) {
            let radius = if ped.near_player() {
                if self.waited > 0 { 0.35 } else { 0.25 }
            } else if self.id == id::SEATED_TRANS {
                if self.waited > 0 { 0.8 } else { 0.5 }
            } else {
                0.2
            };
            let base = at;
            let mut found = false;
            let mut a = 0.0f32;
            while a < TWO_PI {
                let (s, c) = (mathrt::sin(a as f64) as f32, mathrt::cos(a as f64) as f32);
                let spot = Fit { at: [(-s).mul_add(radius, base.at[0]), c.mul_add(radius, base.at[1]), base.at[2]], yaw: base.yaw };
                if !ped.blocked(&spot) && !ped.cut_off(root.at, &spot) {
                    at = spot;
                    found = true;
                    break;
                }
                a += QUARTER_PI;
            }
            if !found {
                self.waited += 2000;
                let limit = if ped.network() { 3000 } else { 6000 };
                if !(ped.near_player() && self.waited > limit) {
                    if !self.relaxed {
                        ped.relax();
                        self.relaxed = true;
                    }
                    return;
                }
                let flags = stand_flags(ped);
                if let Some(spot) = ped.stand_where_he_can(at.at, flags).or_else(|| ped.node_place(at.at, 50.0)) {
                    at.at = spot;
                }
                ped.gave_up();
            }
        }
        if !self.vehicle && let Some(z) = ped.ground_below(at.at) {
            at.at[2] = z + 1.0;
            ped.landed();
        }
        self.blender.frame = Frame { weight: 1.0, rate: NM_FADE, vel: [0.0; 2] };
        self.placed = Some(at);
        if self.id != -1 && self.group != -1 {
            if self.blender.play(ped, 1000.0, self.group, self.id) {
                if (self.group == RAGDOLL_TRANS && self.id != id::RECOVER_ABDOMEN && self.id != id::SEATED_TRANS) || self.group == RAGDOLL_TRANS_BACK {
                    self.follow_up = true;
                }
                let want = self.want;
                let network_player = want == id::GET_UP_FAST && ped.network() && ped.is_player();
                if let Some(p) = self.player_mut() {
                    match want {
                        id::GET_UP_FAST => p.player.rate = if network_player { 1.2 } else { 1.1 },
                        id::GET_UP_INJURED => p.player.rate = 0.5,
                        id::GET_UP_SLOW => p.player.rate = 0.75,
                        _ => {}
                    }
                }
            } else {
                self.no_clip = true;
            }
        } else {
            self.no_clip = !self.play_named(ped);
            self.delay = 0;
        }
        if self.delay > 0 {
            if let Some(p) = self.player_mut() {
                p.player.active = false;
            }
            self.blender.frame.rate = 0.0;
        }
        if self.state == 5 {
            self.follow_up = false;
        }
        self.switched = true;
        ped.leave_nm(&at);
    }

    fn pick(&mut self, ped: &mut dyn Ped) -> Option<Fit> {
        if self.named_clip.is_some_and(|(d, c)| d != u32::MAX && c != 0 && ped.named_clip(d, c).is_some()) {
            return None;
        }
        let list: Vec<(i32, i32)> = if self.named {
            vec![(self.group, self.id)]
        } else if self.state == 5 {
            vec![(43, 0xA8), (43, 0xAA), (RAGDOLL_TRANS, id::INJ_LSIDE_TO_DEFAULT)]
        } else {
            let mut l = vec![];
            if !ped.ground_under_root() {
                l.extend([(RAGDOLL_TRANS, id::RECOVER_ABDOMEN), (RAGDOLL_TRANS, id::RECOVER_BALANCE)]);
            }
            l.push((ped.move_group(), id::MOVE_IDLE));
            l.extend([id::INJ_FRONT_TO_DEFAULT, id::INJ_LSIDE_TO_DEFAULT, id::INJ_RSIDE_TO_DEFAULT, id::PREACHER_TRANS, id::SEATED_TRANS]
                .map(|i| (RAGDOLL_TRANS, i)));
            l.push((RAGDOLL_TRANS_BACK, id::INJ_BACK_TO_DEFAULT));
            l
        };
        let (k, f) = best_fit(&list, ped)?;
        (self.group, self.id) = list[k];
        Some(f)
    }

    pub fn update(&mut self, dt: f32) {
        self.blender.step(dt);
        if self.blender.ended() && !self.follow_up {
            self.done = true;
        }
        self.blender.prune();
    }
}

pub fn stand_flags(ped: &dyn Ped) -> u32 {
    let mut f = if !ped.held() && !ped.stand_helper() { 430 } else { 398 };
    if !ped.on_mover() {
        f |= 64;
    }
    f
}

pub fn vehicle_matrix(m: [[f32; 4]; 4], offset: [f32; 3]) -> [[f32; 4]; 4] {
    let v = [offset[0], offset[1], offset[2], 1.0];
    let dot = |c: usize| (0..4).fold(0.0f64, |s, k| s + v[k] as f64 * m[k][c] as f64) as f32;
    [m[0], m[1], m[2], [dot(0), dot(1), dot(2), v[0]]]
}

pub trait NodeSearch {
    fn node(&self, at: [f32; 3], radius: f32, n: i32) -> Option<[i16; 3]>;
    fn free(&self, p: [f32; 3]) -> bool;
}

pub const NODE_STEP: [f32; 3] = [0.125, 0.125, 0.015625];

pub fn node_place(w: &dyn NodeSearch, at: [f32; 3], radius: f32) -> Option<[f32; 3]> {
    (0..50).find_map(|n| {
        let s = w.node(at, radius, n)?;
        let p = [s[0] as f32 * NODE_STEP[0], s[1] as f32 * NODE_STEP[1], s[2] as f32 * NODE_STEP[2]];
        w.free(p).then_some(p)
    })
}

pub fn best_fit(list: &[(i32, i32)], ped: &dyn Ped) -> Option<(usize, Fit)> {
    let mut best: Option<(f32, usize, Fit)> = None;
    for (k, &(g, i)) in list.iter().enumerate() {
        if let Some((r, f)) = ped.fit(g, i).filter(|(r, _)| best.as_ref().is_none_or(|b| *r < b.0)) {
            best = Some((r, k, f));
        }
    }
    best.map(|(_, k, f)| (k, f))
}

pub fn stick_length(up: f32, side: f32) -> f32 {
    let a = -up;
    side.mul_add(side, a * a).sqrt()
}

pub fn get_up_rate(draw: u32) -> f32 {
    (draw as i32 as f32 * f32::from_bits(0x3800_0100)).mul_add(f32::from_bits(0x3ECC_CCCE), f32::from_bits(0x3F4C_CCCD))
}

pub const SIMPLE_TASK: u32 = 205;

pub const SIMPLE_CANDIDATES: [(i32, i32); 5] = [
    (RAGDOLL_TRANS, id::RECOVER_ABDOMEN),
    (RAGDOLL_TRANS, id::INJ_FRONT_TO_DEFAULT),
    (RAGDOLL_TRANS, id::INJ_LSIDE_TO_DEFAULT),
    (RAGDOLL_TRANS, id::INJ_RSIDE_TO_DEFAULT),
    (RAGDOLL_TRANS_BACK, id::INJ_BACK_TO_DEFAULT),
];

#[derive(Clone, Debug)]
pub struct SimpleGetUp {
    pub state: u8,
    pub group: i32,
    pub id: i32,
    pub fit: bool,
    pub from_back: bool,
    pub has_end: bool,
    pub end_phase: f32,
    pub blender: Blender,
    pub placed: Option<Fit>,
}

impl SimpleGetUp {
    pub fn new(group: i32, id: i32, fit: bool) -> SimpleGetUp {
        let (group, id) = if id == -1 || group == -1 { (GET_UP, id::GET_UP_NORMAL) } else { (group, id) };
        SimpleGetUp { state: 0, group, id, fit, from_back: false, has_end: false, end_phase: 1.0, blender: Blender::new(0.0), placed: None }
    }

    pub fn finished(&self) -> bool {
        self.state == 3
    }

    fn wanted(&self, ped: &dyn Ped) -> i32 {
        if ped.local_player() { id::GET_UP_FAST } else { self.id }
    }

    fn release(&mut self, rate: f32) {
        if let Some(p) = self.blender.player_mut() {
            p.player.set_blend(rate);
            p.player.remove_blended_out = true;
            p.mine = false;
        }
    }

    pub fn start_get_up(&mut self, ped: &dyn Ped, rand: &mut Rand, group: i32, id: i32) -> bool {
        self.release(-8.0);
        if !self.blender.play(ped, 8.0, group, id) {
            return false;
        }
        if ped.in_combat() {
            let tag = ped.end_tag(group, id);
            self.has_end = tag.is_some();
            if let Some(phase) = tag {
                self.end_phase = phase;
            }
        }
        let rate = get_up_rate(rand.next());
        if let Some(p) = self.blender.player_mut() {
            p.player.rate = rate;
        }
        true
    }

    pub fn process(&mut self, ped: &dyn Ped, rand: &mut Rand) -> bool {
        match self.state {
            0 => {
                let chosen = (self.fit && ped.has_skeleton()).then(|| best_fit(&SIMPLE_CANDIDATES, ped)).flatten();
                match chosen {
                    None => {
                        let id = self.wanted(ped);
                        self.start_get_up(ped, rand, self.group, id);
                        self.state = 2;
                    }
                    Some((k, _)) if matches!(SIMPLE_CANDIDATES[k].1, id::RECOVER_ABDOMEN | id::RECOVER_BALANCE) => self.state = 3,
                    Some((k, fit)) => {
                        let (group, id) = SIMPLE_CANDIDATES[k];
                        self.release(-8.0);
                        self.placed = Some(fit);
                        self.blender.play(ped, 100.0, group, id);
                        if group == RAGDOLL_TRANS_BACK {
                            self.from_back = true;
                        }
                        self.state = 1;
                    }
                }
                false
            }
            1 => {
                if self.blender.player().is_some() {
                    return false;
                }
                if self.group == GET_UP && self.from_back {
                    self.group = GET_UP_BACK;
                    if ped.clip(GET_UP_BACK, self.id).is_none() {
                        self.id = id::GET_UP_NORMAL;
                    }
                }
                let id = self.wanted(ped);
                self.start_get_up(ped, rand, self.group, id);
                self.state = 2;
                false
            }
            2 => {
                let Some(phase) = self.blender.player().map(|p| p.player.phase) else {
                    self.state = 3;
                    return false;
                };
                if self.has_end && phase >= self.end_phase {
                    if let Some(p) = self.blender.player_mut() {
                        p.player.set_blend(-0.5);
                    }
                    self.state = 3;
                } else if phase >= 0.75 && ped.local_player() && ped.stick() > 0.25 {
                    self.state = 3;
                }
                false
            }
            _ => true,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.blender.step(dt);
        self.blender.ended();
        self.blender.prune();
    }

    pub fn finish(&mut self) {
        self.release(-8.0);
    }

    pub fn make_abortable(&mut self, urgency: u8, event: Option<Event205>) -> bool {
        let rate = match urgency {
            2 => -1000.0,
            1 if event.is_none_or(|e| e.priority > 0x54 || e.kind == 0x78 || e.ends_tasks || (e.kind == 9 && e.age_ms <= 3.0 * e.frame_ms)) => -8.0,
            _ => return false,
        };
        self.release(rate);
        self.state = 3;
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Event205 {
    pub priority: i32,
    pub kind: i32,
    pub ends_tasks: bool,
    pub age_ms: f32,
    pub frame_ms: f32,
}

