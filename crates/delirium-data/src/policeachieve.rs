use crate::wantedrecord::FakeWanted;

pub const ACHIEVEMENT_LOSE_LEVEL_4: u32 = 11;
pub const ACHIEVEMENT_SIX_STAR_TIME: u32 = 15;
pub const TIMED_STATS: [u32; 7] = [4, 7, 2, 23, 27, 26, 28];
pub const TIMED_STATS_WINDOW_MS: u32 = 600_000;
pub const KIND_TRAIN: u32 = 3;
pub const KIND_HELICOPTER: u32 = 4;
pub const F09_NO_ACHIEVEMENTS: u8 = 0x10;
pub const SIX_STAR_GOAL: f32 = 300_000.0;
pub const WATCH_RANGE_SQUARED: f32 = 1600.0;
pub const FULL_HEALTH: f32 = 100.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoliceAchievements {
    pub lose_level_4_armed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MostWanted<P> {
    pub watched: Option<P>,
    pub level: i32,
}

pub trait FakeLevelGame {
    type Ped: Copy;
    type Slot: Copy;
    type Wanted: Copy;
    fn local_slot(&self) -> Option<Self::Slot>;
    fn slot_ped(&self, slot: Option<Self::Slot>) -> Option<Self::Ped>;
    fn set_record_fake_level(&mut self, ped: Option<Self::Ped>, level: u32);
    fn clock(&self) -> u32;
    fn local_wanted(&mut self) -> Option<Self::Wanted>;
    fn set_level_changed_at(&mut self, w: Option<Self::Wanted>, time: u32);
}

pub trait PoliceAchieveGame: FakeLevelGame {
    type Vehicle: Copy;
    fn network_game(&mut self) -> bool;
    fn player_vehicle(&mut self) -> Option<Self::Vehicle>;
    fn has_achievement(&mut self, id: u32) -> bool;
    fn award_achievement(&mut self, id: u32);
    fn stat_changed_within(&mut self, stat: u32, ms: u32) -> bool;
    fn vehicle_kind(&self, v: Self::Vehicle) -> u32;
    fn vehicle_flags_f09(&self, v: Self::Vehicle) -> u8;
    fn slot_busy(&self, slot: Option<Self::Slot>) -> bool;
    fn ped_in_vehicle(&self, p: Self::Ped) -> bool;
    fn ped_vehicle(&self, p: Self::Ped) -> Option<Self::Vehicle>;
    fn wanted_level(&self, w: Option<Self::Wanted>) -> i32;
    fn ped_wanted(&self, p: Self::Ped) -> Option<Self::Wanted>;
    fn release_ref(&mut self, p: Self::Ped, slot: &mut Option<Self::Ped>);
    fn hold_ref(&mut self, p: Option<Self::Ped>, slot: &mut Option<Self::Ped>);
    fn local_ped(&mut self) -> Option<Self::Ped>;
    fn keep_place(&mut self, p: Option<Self::Ped>) -> [f32; 3];
    fn ped_slots(&self) -> u32;
    fn ped_slot(&self, i: u32) -> Option<Self::Ped>;
    fn is_player(&self, p: Self::Ped) -> bool;
    fn health(&self, p: Self::Ped) -> f32;
    fn place(&self, p: Self::Ped) -> [f32; 3];
}

fn quiet<G: PoliceAchieveGame + ?Sized>(g: &mut G, v: Option<G::Vehicle>) -> bool {
    for stat in TIMED_STATS {
        if g.stat_changed_within(stat, TIMED_STATS_WINDOW_MS) {
            return false;
        }
    }
    !v.is_some_and(|v| g.vehicle_flags_f09(v) & F09_NO_ACHIEVEMENTS != 0 && g.vehicle_kind(v) == KIND_HELICOPTER)
}

fn player_free<G: PoliceAchieveGame + ?Sized>(g: &G) -> bool {
    let slot = g.local_slot();
    let (busy, ped) = (g.slot_busy(slot), g.slot_ped(slot));
    let Some(ped) = ped else { return false };
    if busy {
        return false;
    }
    !(g.ped_in_vehicle(ped) && g.ped_vehicle(ped).is_some_and(|v| g.vehicle_kind(v) == KIND_TRAIN))
}

pub fn arm_lose_level_4<G: PoliceAchieveGame + ?Sized>(g: &mut G, achievements: &mut PoliceAchievements) {
    if g.network_game() {
        return;
    }
    let v = g.player_vehicle();
    if g.has_achievement(ACHIEVEMENT_LOSE_LEVEL_4) {
        return;
    }
    if quiet(g, v) {
        achievements.lose_level_4_armed = true;
    }
}

pub fn award_lose_level_4<G: PoliceAchieveGame + ?Sized>(g: &mut G, achievements: &PoliceAchievements) {
    let v = g.player_vehicle();
    if g.network_game() || g.has_achievement(ACHIEVEMENT_LOSE_LEVEL_4) {
        return;
    }
    if !achievements.lose_level_4_armed || !quiet(g, v) || !player_free(g) {
        return;
    }
    g.award_achievement(ACHIEVEMENT_LOSE_LEVEL_4);
}

pub fn award_six_star_time<G: PoliceAchieveGame + ?Sized>(g: &mut G, time: f32) {
    let v = g.player_vehicle();
    if g.network_game() || g.has_achievement(ACHIEVEMENT_SIX_STAR_TIME) {
        return;
    }
    if !quiet(g, v) || !player_free(g) {
        return;
    }
    if !(time < SIX_STAR_GOAL) {
        g.award_achievement(ACHIEVEMENT_SIX_STAR_TIME);
    }
}

pub fn pick_most_wanted_nearby<G: PoliceAchieveGame + ?Sized>(g: &mut G, most: &mut MostWanted<G::Ped>) {
    if let Some(old) = most.watched {
        g.release_ref(old, &mut most.watched);
    }
    let mine = g.local_wanted();
    most.level = g.wanted_level(mine);
    most.watched = g.local_ped();
    let me = g.local_ped();
    let [x, y, z] = g.keep_place(me);
    let n = g.ped_slots();
    if n != 0 {
        for i in (0..n).rev() {
            let Some(p) = g.ped_slot(i) else { continue };
            if !g.is_player(p) || g.health(p) < FULL_HEALTH {
                continue;
            }
            let at = g.place(p);
            let (dx, dy, dz) = (at[0] - x, at[1] - y, at[2] - z);
            if dx.mul_add(dx, dz.mul_add(dz, dy * dy)) < WATCH_RANGE_SQUARED {
                let level = g.wanted_level(g.ped_wanted(p));
                if level > most.level {
                    most.watched = Some(p);
                    most.level = level;
                }
            }
        }
    }
    let watched = most.watched;
    g.hold_ref(watched, &mut most.watched);
}

pub fn set_fake_wanted_level<G: FakeLevelGame + ?Sized>(g: &mut G, fake: &mut FakeWanted, level: u32) {
    fake.level = level as i32;
    if let Some(slot) = g.local_slot() {
        let ped = g.slot_ped(Some(slot));
        g.set_record_fake_level(ped, level);
    }
    let now = g.clock();
    let w = g.local_wanted();
    g.set_level_changed_at(w, now);
    if level == 0 {
        fake.circle = false;
    }
}
