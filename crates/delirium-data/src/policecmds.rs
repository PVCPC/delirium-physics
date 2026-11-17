pub const BULLET_IMPACT_PRIORITY: u32 = 48;
pub const POLICE_AUDIO: u32 = 2;
pub const LOWER_DELAY_MS: u32 = 0;
pub const RAISE_DELAY_MS: u32 = 10_000;
pub const COPY_LAYER: u32 = 3;
pub const COPY_FLAG: u32 = 0x0200_0000;
pub const UNSET_RATE_FROM: f32 = 0.0;
pub const DEFAULT_COPY_RATE: f32 = -8.0;
pub const COPY_BLEND_IN: f32 = 1000.0;
pub const GROUND_LEVEL_UNSET: f32 = -100.0;
pub const SCANNER_SLOTS: u32 = 32;
pub const ZONE_ROWS: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopPool {
    Peds,
    Vehicles,
    Objects,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crime<S> {
    pub kind: u32,
    pub subject: S,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub a: [f32; 3],
    pub b: [f32; 3],
}

impl Area {
    pub fn from_floats(f: [f32; 6]) -> Self {
        Area { a: [f[0], f[1], f[2]], b: [f[3], f[4], f[5]] }
    }

    pub fn floats(&self) -> [f32; 6] {
        [self.a[0], self.a[1], self.a[2], self.b[0], self.b[1], self.b[2]]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RestartPoint {
    pub place: [f32; 4],
    pub heading: f32,
    pub island: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipStart {
    pub dict: u32,
    pub name: u32,
    pub flags: u32,
    pub priority: u32,
    pub blend_in: f32,
}

pub trait PoliceCmds {
    type Audio: Copy;
    fn audio_kind(&self, audio: Self::Audio) -> u32;
}

pub fn bullet_impact_priority() -> u32 {
    BULLET_IMPACT_PRIORITY
}

pub fn is_police_audio<G: PoliceCmds>(g: &G, audio: G::Audio) -> bool {
    g.audio_kind(audio) == POLICE_AUDIO
}

pub trait CopWorld {
    type Ped: Copy;
    type Vehicle: Copy;
    fn ped_type(&self, ped: Self::Ped) -> u32;
    fn is_police_vehicle(&self, v: Self::Vehicle) -> bool;
}

pub trait CopsInArea: CopWorld {
    type Entity: Copy;
    fn position(&self, e: Self::Entity) -> [f32; 3];
    fn ped_by_handle(&self, handle: u32) -> Option<Self::Ped>;
    fn vehicle_sat_in(&self, ped: Self::Ped) -> Option<Self::Vehicle>;
    fn vehicle_slots(&self) -> i32;
    fn vehicle_slot(&self, i: u32) -> Option<Self::Vehicle>;
    fn ped_slots(&self) -> i32;
    fn ped_slot(&self, i: u32) -> Option<Self::Ped>;
    fn vehicle_in_area(&mut self, v: Self::Vehicle, area: Area) -> bool;
    fn ped_in_area(&mut self, ped: Self::Ped, area: Area) -> bool;
}

pub fn in_police_vehicle<G: CopsInArea>(g: &G, handle: u32) -> bool {
    let Some(ped) = g.ped_by_handle(handle) else { return false };
    g.vehicle_sat_in(ped).is_some_and(|v| g.is_police_vehicle(v))
}

pub fn entity_in_box<G: CopsInArea>(g: &G, e: G::Entity, area: Area) -> bool {
    let order = |a: f32, b: f32| if a > b { (b, a) } else { (a, b) };
    let (x0, x1) = order(area.a[0], area.b[0]);
    let (y0, y1) = order(area.a[1], area.b[1]);
    let (z0, z1) = order(area.a[2], area.b[2]);
    let [x, y, z] = g.position(e);
    if x < x0 || x > x1 || y < y0 || y > y1 || z < z0 {
        return false;
    }
    !(z > z1)
}

pub fn cop_vehicle_in_area<G: CopsInArea>(g: &mut G, area: Area) -> bool {
    let mut found = false;
    let n = g.vehicle_slots();
    for i in 0..n.max(0) as u32 {
        let Some(v) = g.vehicle_slot(i) else { continue };
        if !g.is_police_vehicle(v) {
            continue;
        }
        if g.vehicle_in_area(v, area) {
            found = true;
        }
    }
    found
}

pub fn cop_ped_in_area<G: CopsInArea>(g: &mut G, area: Area) -> bool {
    let mut found = false;
    let n = g.ped_slots();
    for i in 0..n.max(0) as u32 {
        let Some(ped) = g.ped_slot(i) else { continue };
        if g.ped_type(ped) != crate::crew::TYPE_COP {
            continue;
        }
        if g.ped_in_area(ped, area) {
            found = true;
        }
    }
    found
}

pub trait WantedCommands {
    type Player: Copy;
    fn player(&mut self, index: u32) -> Self::Player;
    fn wanted_level(&self, player: Self::Player) -> i32;
    fn set_wanted_level(&mut self, player: Self::Player, level: i32, delay_ms: u32);
    fn finish_scanner_report(&mut self);
    fn set_police_ignore(&mut self, player: Self::Player, on: bool);
    fn stop_emergency_vehicles(&mut self, player: Self::Player);
}

pub trait WantedIncrements {
    type Record: Copy;
    fn wanted_record(&mut self, index: u32) -> Option<Self::Record>;
    fn wanted_increment(&self, record: Self::Record) -> i32;
}

pub fn alter_wanted_level<G: WantedCommands>(g: &mut G, achievements: &mut crate::policeachieve::PoliceAchievements,
                                             player: u32, level: i32) {
    let player = g.player(player);
    if level < g.wanted_level(player) {
        g.set_wanted_level(player, level, LOWER_DELAY_MS);
        g.finish_scanner_report();
        achievements.lose_level_4_armed = false;
    } else {
        g.set_wanted_level(player, level, RAISE_DELAY_MS);
    }
}

pub fn set_police_ignore_player<G: WantedCommands>(g: &mut G, player: u32, on: bool) {
    let player = g.player(player);
    g.set_police_ignore(player, on);
    if on {
        g.stop_emergency_vehicles(player);
    }
}

pub fn player_wanted_increment<G: WantedIncrements>(g: &mut G, player: u32) -> Option<i32> {
    let record = g.wanted_record(player)?;
    Some(g.wanted_increment(record))
}

pub trait CrimeReports {
    type Record: Copy;
    type Place: Copy;
    type Answer: Copy;
    type Subject: Copy;
    fn note_crime(&mut self, record: Self::Record, crime: Crime<Self::Subject>, place: Self::Place, seen: bool, tag: u32) -> Self::Answer;
    fn already_seen(&self, answer: Self::Answer) -> bool;
    fn register_crime(&mut self, record: Self::Record, place: Self::Place) -> Self::Answer;
}

pub fn report_crime<G: CrimeReports>(g: &mut G, record: G::Record, crime: Crime<G::Subject>, place: G::Place, tag: u32) -> G::Answer {
    g.note_crime(record, crime, place, false, tag)
}

pub fn report_crime_seen<G: CrimeReports>(g: &mut G, record: G::Record, crime: Crime<G::Subject>, place: G::Place, tag: u32) -> G::Answer {
    let noted = g.note_crime(record, crime, place, true, tag);
    if g.already_seen(noted) {
        return noted;
    }
    g.register_crime(record, place)
}

pub trait ClearCops: CopWorld {
    type Group: Copy;
    type NetObject: Copy;
    fn set_cop_search(&mut self, centre: [f32; 4], radius: f32);
    fn cop_search(&self) -> ([f32; 2], f32);
    fn clear_cops_in(&mut self, pool: CopPool);
    fn is_player_ped(&self, ped: Self::Ped) -> bool;
    fn may_be_cleared(&mut self, ped: Self::Ped) -> bool;
    fn local_player_group(&mut self) -> Option<Self::Group>;
    fn group_has(&mut self, group: Self::Group, ped: Self::Ped) -> bool;
    fn ped_position(&self, ped: Self::Ped) -> [f32; 3];
    fn vehicle_position(&self, v: Self::Vehicle) -> [f32; 3];
    fn mark_cleared(&mut self, ped: Self::Ped);
    fn delete_ped(&mut self, ped: Self::Ped);
    fn network_object(&self, v: Self::Vehicle) -> Option<Self::NetObject>;
    fn is_remote(&self, n: Self::NetObject) -> bool;
    fn sync_running(&mut self, n: Self::NetObject) -> bool;
    fn vehicle_model_type(&mut self, v: Self::Vehicle) -> u32;
    fn delete_vehicle(&mut self, v: Self::Vehicle);
}

pub fn clear_area_of_cops<G: ClearCops>(g: &mut G, centre: [f32; 4], radius: f32) {
    g.set_cop_search(centre, radius);
    for pool in [CopPool::Peds, CopPool::Vehicles, CopPool::Objects] {
        g.clear_cops_in(pool);
    }
}

fn in_cop_search<G: ClearCops>(g: &G, at: [f32; 3]) -> bool {
    let (centre, radius) = g.cop_search();
    let dx = at[0] - centre[0];
    let dy = at[1] - centre[1];
    dx.mul_add(dx, dy * dy) < radius * radius
}

fn in_local_player_group<G: ClearCops>(g: &mut G, ped: G::Ped) -> bool {
    match g.local_player_group() {
        Some(group) => g.group_has(group, ped),
        None => false,
    }
}

pub fn clear_cop<G: ClearCops>(g: &mut G, ped: G::Ped) -> bool {
    if g.is_player_ped(ped) {
        return true;
    }
    if !g.may_be_cleared(ped) || g.ped_type(ped) != crate::crew::TYPE_COP {
        return true;
    }
    if in_local_player_group(g, ped) {
        return true;
    }
    if in_cop_search(g, g.ped_position(ped)) {
        g.mark_cleared(ped);
        g.delete_ped(ped);
    }
    true
}

pub fn clear_cop_vehicle<G: ClearCops>(g: &mut G, v: G::Vehicle) -> bool {
    let remote = g.network_object(v).is_some_and(|n| g.is_remote(n));
    let running = match g.network_object(v) {
        Some(n) => g.sync_running(n),
        None => false,
    };
    if remote || running {
        return true;
    }
    if g.vehicle_model_type(v) != crate::crew::TYPE_COP {
        return true;
    }
    if in_cop_search(g, g.vehicle_position(v)) {
        g.delete_vehicle(v);
    }
    true
}

pub trait RestartTable {
    fn restart_count(&self) -> u32;
    fn put_restart(&mut self, n: u32, point: RestartPoint);
}

pub trait PoliceRestarts {
    fn ground_height(&mut self, x: f32, y: f32) -> f32;
    fn add_restart_at(&mut self, at: [f32; 3], island: u32, heading: f32);
}

pub fn add_restart<G: RestartTable>(g: &mut G, place: [f32; 4], island: u32, heading: f32) {
    let n = g.restart_count();
    g.put_restart(n, RestartPoint { place, heading, island });
}

pub fn add_police_restart<G: PoliceRestarts>(g: &mut G, island: u32, x: f32, y: f32, z: f32, heading: f32) {
    let z = if z > GROUND_LEVEL_UNSET { z } else { g.ground_height(x, y) };
    g.add_restart_at([x, y, z], island, heading);
}

pub trait PoliceHelis {
    fn set_police_helis(&mut self, on: u8);
}

pub trait RandomCops {
    fn set_random_cops(&mut self, on: u8);
}

pub trait ScannerSwitch {
    type Scanner: Copy;
    fn set_scanner_on(&mut self, on: bool);
    fn free_report_slot(&mut self, scanner: Self::Scanner, slot: u32);
}

pub trait ZoneCopShare {
    fn set_zone_cop_share(&mut self, row: u32, percent: u8);
}

pub fn switch_police_helis<G: PoliceHelis>(g: &mut G, on: u8) {
    g.set_police_helis(on);
}

pub fn set_create_random_cops<G: RandomCops>(g: &mut G, on: u8) {
    g.set_random_cops(on);
}

fn set_scanner<G: ScannerSwitch>(g: &mut G, scanner: G::Scanner, on: bool) {
    g.set_scanner_on(on);
    for slot in 0..SCANNER_SLOTS {
        g.free_report_slot(scanner, slot);
    }
}

pub fn disable_police_scanner<G: ScannerSwitch>(g: &mut G, scanner: G::Scanner) {
    set_scanner(g, scanner, false);
}

pub fn enable_police_scanner<G: ScannerSwitch>(g: &mut G, scanner: G::Scanner) {
    set_scanner(g, scanner, true);
}

pub fn set_script_zone_cop_share<G: ZoneCopShare>(g: &mut G, percent: u8) {
    for row in 0..ZONE_ROWS {
        g.set_zone_cop_share(row, percent);
    }
}

pub trait CopyAnimations {
    type Ped: Copy;
    type Clip: Copy;
    fn ped_by_handle(&self, handle: u32) -> Option<Self::Ped>;
    fn first_clip(&mut self, ped: Option<Self::Ped>) -> Option<Self::Clip>;
    fn next_clip(&mut self, ped: Option<Self::Ped>) -> Option<Self::Clip>;
    fn clip_start(&self, clip: Self::Clip) -> ClipStart;
    fn start_clip(&mut self, ped: Option<Self::Ped>, start: ClipStart) -> Option<Self::Clip>;
    fn stop(&mut self, clip: Self::Clip);
    fn play(&mut self, clip: Self::Clip);
    fn phase(&self, clip: Self::Clip) -> f32;
    fn set_phase(&mut self, clip: Self::Clip, phase: f32);
    fn set_blend_rate(&mut self, clip: Self::Clip, rate: f32);
    fn copy_user_words(&mut self, from: Self::Clip, to: Self::Clip);
}

pub fn copy_animations<G: CopyAnimations>(g: &mut G, to: u32, from: u32, rate: f32) {
    let to = g.ped_by_handle(to);
    let from = g.ped_by_handle(from);
    let rate = if rate < UNSET_RATE_FROM { rate } else { DEFAULT_COPY_RATE };
    let Some(mut clip) = g.first_clip(from) else { return };
    loop {
        let source = g.clip_start(clip);
        let start = ClipStart { flags: source.flags | COPY_FLAG, blend_in: COPY_BLEND_IN, ..source };
        if let Some(copy) = g.start_clip(to, start) {
            g.stop(clip);
            g.play(copy);
            let phase = g.phase(clip);
            g.set_phase(copy, phase);
            g.set_blend_rate(copy, rate);
            g.copy_user_words(clip, copy);
        }
        match g.next_clip(from) {
            Some(next) => clip = next,
            None => return,
        }
    }
}

