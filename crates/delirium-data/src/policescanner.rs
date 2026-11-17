use crate::scanner::{ByteFlag, Made, Place, PoliceScanner, Report, SoundHandle};

pub type SlotNo = u32;

pub const IDLE: u32 = 0;
pub const STARTING: u32 = 1;
pub const SPEAKING: u32 = 2;
pub const IN_GAP: u32 = 3;
pub const SLOTS: u32 = 32;
pub const SENT_RECORDS: u32 = 32;
pub const SENT_DELAY_MS: u32 = 2000;
pub const KIND_SCRIPTED: i32 = 7;
pub const KIND_ALWAYS: i32 = 8;
pub const KIND_CHASE_END: u32 = 5;
pub const PRIORITY_ALWAYS: u32 = 4;
pub const STANDING: u32 = 4;
pub const CHATTER_MIN_MS: u32 = 5000;
pub const CHATTER_MAX_MS: u32 = 15000;
pub const ALLOW_ALL_AFTER_MS: u32 = 60000;
pub const BANK_STREET: u32 = 2;
pub const BANK_AREA: u32 = 3;
pub const BANK_WORD: u32 = 5;
pub const BANK_ERR: u32 = 6;
pub const AREA_GAP_MS: [u32; 2] = [100, 2000];
pub const PLACE_LATE_MS: u32 = 4000;
pub const POLICE_CRIMES: u32 = 31;
pub const KIND_SCRIPT_CRIME: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Said {
    Colour,
    Model,
    Manufacturer,
    Category,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChaseEnd {
    Custody,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Name {
    OnLong,
    OnShort,
    In,
    Err,
    ThisIsControl,
    SuspectInCustody,
    SuspectDown,
    ColourBeatenUp,
    Vigilante(u32),
}

pub const VIGILANTE_CRIMES: [&str; 7] = [
    "CRIME_FLEEING_CRIME_SCENE", "CRIME_STEAL_CAR", "CRIME_GUN_SPREE", "CRIME_STEAL_CAR", "CRIME_DRIVEBY", "CRIME_DRUG_DEAL",
    "CRIME_GANG_ACTIVITY",
];

impl Name {
    pub fn text(self) -> &'static str {
        match self {
            Name::OnLong => "POLICE_SCANNER_ON_LONG",
            Name::OnShort => "POLICE_SCANNER_ON_SHORT",
            Name::In => "POLICE_SCANNER_IN",
            Name::Err => "POLICE_SCANNER_ERR",
            Name::ThisIsControl => "THIS_IS_CONTROL",
            Name::SuspectInCustody => "SUSPECT_IN_CUSTODY",
            Name::SuspectDown => "SUSPECT_DOWN",
            Name::ColourBeatenUp => "POLICE_SCANNER_COLOUR_FUCKED",
            Name::Vigilante(k) => VIGILANTE_CRIMES[k as usize],
        }
    }
}

impl ChaseEnd {
    pub fn names(self) -> [Name; 2] {
        match self {
            ChaseEnd::Custody => [Name::ThisIsControl, Name::SuspectInCustody],
            ChaseEnd::Down => [Name::ThisIsControl, Name::SuspectDown],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundOut {
    Phrase(SlotNo, u32),
    Echo(SlotNo, u32),
    Noise,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhraseRequest {
    pub category: u32,
    pub follow: Option<u32>,
    pub start: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoiseRequest {
    pub category: u32,
    pub placed: Option<NoisePlace>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoisePlace {
    pub follow: Option<u32>,
    pub group: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Description<A> {
    pub prefix: u32,
    pub colour: u32,
    pub manufacturer: u32,
    pub model: u32,
    pub category: u32,
    pub audio: A,
    pub marked: bool,
    pub bike: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SuspectReport<A> {
    pub place: Place,
    pub vehicle: Option<Description<A>>,
    pub speed: f32,
    pub say_area: bool,
    pub priority: u32,
    pub heading: u32,
    pub last: u8,
}

pub trait PlaceWords {
    fn hash_name(&mut self, sc: &mut PoliceScanner, name: Name) -> u32;
    fn uniform(&mut self, sc: &mut PoliceScanner, lo: u32, hi: u32) -> u32;
    fn chance(&mut self, sc: &mut PoliceScanner, p: f32) -> bool;
    fn null_sound(&self) -> u32;
    fn street_known(&mut self, sc: &mut PoliceScanner, street: u32) -> bool;
    fn in_gap_ms(&self) -> u32;
    fn err_gap_ms(&self) -> u32;
    fn on_gap_ms(&self) -> u32;
    fn in_chance(&self) -> f32;
    fn skip_area_chance(&self) -> f32;
}

pub trait ScannerGame: PlaceWords {
    type Ped: Copy;
    type Vehicle: Copy;
    type Position: Copy;
    type Bank: Copy;
    type CrimeName: Copy;
    fn now(&self) -> u32;
    fn zero(&self) -> f32;
    fn fade_out_db(&self) -> f32;
    fn fade_limit(&self) -> f32;
    fn heading_interval(&self) -> u32;
    fn moving_distance(&self) -> f32;
    fn overlap_ms(&self) -> u32;
    fn report_pause_ms(&self) -> u32;
    fn chatter_db(&self) -> f32;
    fn area_again_chance(&self) -> f32;
    fn radio_levels(&self) -> [f32; 3];
    fn heading_constants(&self) -> [f32; 4];
    fn speed_scale(&self) -> [f32; 2];
    fn lead_factors(&self) -> [f32; 2];
    fn allowed_after(&self, kind: u32) -> u32;
    fn has_player(&self) -> bool;
    fn paused(&self) -> bool;
    fn bank_of(&self, slot: u32) -> Self::Bank;
    fn player_wanted_level(&mut self, sc: &mut PoliceScanner) -> i32;
    fn player_wanted_multiplier(&mut self, sc: &mut PoliceScanner) -> f32;
    fn player_ped(&mut self, sc: &mut PoliceScanner) -> Self::Ped;
    fn player_position(&mut self, sc: &mut PoliceScanner) -> [f32; 4];
    fn player_vehicle(&mut self, sc: &mut PoliceScanner) -> Option<Self::Vehicle>;
    fn player_on_train(&mut self, sc: &mut PoliceScanner) -> bool;
    fn out_of_sight(&mut self, sc: &mut PoliceScanner) -> i32;
    fn ped_position(&mut self, sc: &mut PoliceScanner, ped: Self::Ped) -> Self::Position;
    fn ped_in_vehicle(&self, ped: Self::Ped) -> bool;
    fn ped_vehicle(&self, ped: Self::Ped) -> Option<Self::Vehicle>;
    fn ped_task_vehicle(&mut self, sc: &mut PoliceScanner, ped: Self::Ped) -> Option<Self::Vehicle>;
    fn vehicle_audio(&self, v: Self::Vehicle) -> u32;
    fn vehicle_model(&self, v: Self::Vehicle) -> i32;
    fn ambulance_model(&self) -> i32;
    fn firetruck_model(&self) -> i32;
    fn vehicle_radio_on(&self, v: Self::Vehicle) -> bool;
    fn vehicle_marked(&self, v: Self::Vehicle) -> bool;
    fn vehicle_emergency(&self, v: Self::Vehicle) -> bool;
    fn vehicle_type(&self, v: Self::Vehicle) -> i32;
    fn colour_words(&self, v: Self::Vehicle) -> [u32; 2];
    fn manufacturer_word(&self, v: Self::Vehicle, boat: bool) -> u32;
    fn model_word(&self, v: Self::Vehicle, boat: bool) -> u32;
    fn category_word(&self, v: Self::Vehicle, boat: bool) -> u32;
    fn vehicle_velocity(&mut self, sc: &mut PoliceScanner, v: Self::Vehicle) -> [f32; 3];
    fn vehicle_forward(&self, v: Self::Vehicle) -> [f32; 3];
    fn is_police_audio(&mut self, sc: &mut PoliceScanner, audio: u32) -> bool;
    fn is_beaten_up(&mut self, sc: &mut PoliceScanner, audio: u32) -> bool;
    fn is_taxi(&mut self, sc: &mut PoliceScanner, model: i32) -> bool;
    fn note_said(&mut self, sc: &mut PoliceScanner, audio: u32, part: Said);
    fn start_slot(&mut self, sc: &mut PoliceScanner, s: SlotNo);
    fn clear_slot(&mut self, sc: &mut PoliceScanner, s: SlotNo);
    fn make_slot(&mut self, sc: &mut PoliceScanner, s: SlotNo) -> bool;
    fn slot_load_state(&mut self, sc: &mut PoliceScanner, s: SlotNo) -> u32;
    fn note_said_of_playing(&mut self, sc: &mut PoliceScanner);
    fn pick_report(&mut self, sc: &mut PoliceScanner) -> bool;
    fn queued_dispatch(&mut self, sc: &mut PoliceScanner);
    fn chatter(&mut self, sc: &mut PoliceScanner, now: u32);
    fn screen_fade(&mut self, sc: &mut PoliceScanner) -> f32;
    fn held(&mut self, sc: &mut PoliceScanner) -> bool;
    fn heading_angle(&mut self, sc: &mut PoliceScanner, c: f32) -> f32;
    fn stop_sound(&mut self, sc: &mut PoliceScanner, sound: SoundHandle);
    fn play(&mut self, sc: &mut PoliceScanner, sound: SoundHandle);
    fn play_when_loaded(&mut self, sc: &mut PoliceScanner, sound: SoundHandle, bank: Option<Self::Bank>);
    fn load_state(&mut self, sc: &mut PoliceScanner, sound: Option<SoundHandle>, bank: Self::Bank) -> i32;
    fn play_position(&mut self, sc: &mut PoliceScanner, sound: SoundHandle) -> u32;
    fn sound_length(&self, sound: SoundHandle) -> i32;
    fn set_volume(&mut self, sc: &mut PoliceScanner, sound: SoundHandle, db: f32);
    fn set_group(&mut self, sc: &mut PoliceScanner, sound: SoundHandle, group: u32);
    fn unplace(&mut self, sc: &mut PoliceScanner, sound: SoundHandle);
    fn place_at_slot(&mut self, sc: &mut PoliceScanner, sound: SoundHandle, s: SlotNo);
    fn make_phrase_sound(&mut self, sc: &mut PoliceScanner, echo: bool, name: u32, out: SoundOut, req: &PhraseRequest);
    fn make_noise_sound(&mut self, sc: &mut PoliceScanner, name: u32, s: SlotNo, req: &NoiseRequest);
    fn crime_sound(&mut self, sc: &mut PoliceScanner, name: Self::CrimeName) -> u32;
    fn crime_sound_of(&mut self, sc: &mut PoliceScanner, hash: u32) -> u32;
    fn describe_place(&mut self, sc: &mut PoliceScanner, at: Self::Position);
    fn report_place(&self) -> Place;
    fn queue_crime(&mut self, sc: &mut PoliceScanner, at: Self::Position, r: &Report);
    fn describe_vehicle_into(&mut self, sc: &mut PoliceScanner, v: Self::Vehicle) -> Description<u32>;
    fn queue_suspect(&mut self, sc: &mut PoliceScanner, r: &SuspectReport<u32>);
    fn report_search(&mut self, sc: &mut PoliceScanner, at: Self::Position);
    fn report_dispatch(&mut self, sc: &mut PoliceScanner, units: u32, kind: u32, from: Self::Position);
    fn crime_multiplier_floor(&self) -> f32;
    fn fake_level(&self) -> i32;
    fn police_crime_hash(&self, crime: u32) -> u32;
    fn spot_chance(&self) -> f32;
    fn search_share(&self) -> f32;
    fn station_position(&mut self, sc: &mut PoliceScanner, station: u32) -> Self::Position;
}

pub trait WeightLists {
    type WeightList: Copy;
    fn weights(&self, list: Self::WeightList, count: u32) -> Vec<(u32, f32)>;
    fn zero(&self) -> f32;
    fn random_float(&mut self, lo: f32, hi: f32) -> f32;
}

pub fn clear_slot<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, s: SlotNo) {
    let mut k = 0;
    while k < sc.slot(s).phrase_count {
        let made = sc.slot(s).made(k);
        for sound in [made.sound, made.echo].into_iter().flatten() {
            g.stop_sound(sc, sound);
        }
        sc.slot_mut(s).phrase_mut(k).sound = 0;
        k += 1;
    }
    let slot = sc.slot_mut(s);
    slot.queued = ByteFlag::OFF;
    slot.flag = ByteFlag::OFF;
}

pub fn note_said<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G) {
    let s = sc.playing;
    if let Some(audio) = sc.slot(s).described() {
        if sc.slot(s).said(Said::Colour) {
            g.note_said(sc, audio, Said::Colour);
        }
        for part in [Said::Category, Said::Model, Said::Manufacturer] {
            let slot = sc.slot(sc.playing);
            if slot.said(part)
                && let Some(audio) = slot.described()
            {
                g.note_said(sc, audio, part);
            }
        }
    }
    if sc.slot(sc.playing).kind as i32 == KIND_SCRIPTED {
        sc.last_scripted = g.now();
    }
}

pub fn report_done<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G) {
    match sc.state {
        SPEAKING => sc.cut = ByteFlag::ON,
        STARTING => {
            g.clear_slot(sc, sc.playing);
            sc.state = IDLE;
        }
        _ => {}
    }
}

fn stop_noise<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, noise: SoundHandle) {
    let db = g.fade_out_db();
    g.set_volume(sc, noise, db);
    g.stop_sound(sc, noise);
}

pub fn stop_all<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G) {
    for s in 0..SLOTS {
        g.clear_slot(sc, s);
    }
    let noise = sc.noise_sound;
    sc.state = IDLE;
    sc.sampled = 0;
    sc.not_before = 0;
    if let Some(noise) = noise {
        stop_noise(sc, g, noise);
    }
}

pub fn slot_load_state<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, s: SlotNo) -> u32 {
    let mut ready = 1;
    let mut k = 0;
    while k < sc.slot(s).phrase_count {
        let bank = g.bank_of(sc.slot(s).phrase(k).bank);
        let sound = sc.slot(s).made(k).sound;
        match g.load_state(sc, sound, bank) {
            2 => return 2,
            0 => ready = 0,
            _ => {}
        }
        k += 1;
    }
    ready
}

pub fn make_slot<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, s: SlotNo) -> bool {
    let mut k = 0;
    while k < sc.slot(s).phrase_count {
        let slot = sc.slot(s);
        let start = if k == 0 { slot.predelay } else { 0 };
        let req = PhraseRequest { category: slot.category, follow: slot.follow, start };
        let name = slot.phrase(k).sound;
        g.make_phrase_sound(sc, false, name, SoundOut::Phrase(s, k), &req);
        if sc.slot(s).made(k).sound.is_none() {
            return false;
        }
        let slot = sc.slot(s);
        if slot.echo != 0 {
            let start = if k == 0 { slot.predelay.wrapping_add(slot.echo) } else { slot.echo };
            let req = PhraseRequest { category: slot.category, follow: slot.follow, start };
            let name = slot.phrase(k).sound;
            g.make_phrase_sound(sc, true, name, SoundOut::Echo(s, k), &req);
            if sc.slot(s).made(k).echo.is_none() {
                return false;
            }
        }
        k += 1;
    }
    true
}

fn step_past_playing(sc: &mut PoliceScanner) {
    if sc.next == sc.playing {
        sc.next = sc.next.wrapping_add(1);
    }
}

fn lay_phrase(sc: &mut PoliceScanner, s: SlotNo, k: u32, gap: u32, bank: u32, name: u32) {
    let slot = sc.slot_mut(s);
    let p = slot.phrase_mut(k);
    p.gap = gap;
    p.bank = bank;
    slot.made_mut(k).sound = None;
    slot.phrase_mut(k).sound = name;
}

fn chase_end<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, end: ChaseEnd) {
    if !sc.on {
        return;
    }
    step_past_playing(sc);
    g.start_slot(sc, sc.next);
    sc.slot_mut(sc.next).kind = KIND_CHASE_END;
    let now = g.now();
    let delay = g.uniform(sc, 1000, 3000);
    let mut made = sc.chase_names(end).made;
    sc.slot_mut(sc.next).time = delay.wrapping_add(now);
    sc.slot_mut(sc.next).priority = PRIORITY_ALWAYS;
    let [first, second] = end.names();
    if made & 1 == 0 {
        made |= 1;
        sc.chase_names_mut(end).made = made;
        let h = g.hash_name(sc, first);
        made = sc.chase_names(end).made;
        sc.chase_names_mut(end).hashes[0] = h;
    }
    if made & 2 == 0 {
        sc.chase_names_mut(end).made = made | 2;
        let h = g.hash_name(sc, second);
        sc.chase_names_mut(end).hashes[1] = h;
    }
    let first = sc.chase_names(end).hashes[0];
    lay_phrase(sc, sc.next, 0, 0, 0, first);
    let gap = g.uniform(sc, 100, 1000);
    let second = sc.chase_names(end).hashes[1];
    lay_phrase(sc, sc.next, 1, gap, 1, second);
    let n = sc.next;
    sc.slot_mut(n).phrase_count = 2;
    sc.slot_mut(n).queued = ByteFlag::ON;
    sc.next = n.wrapping_add(1) % SLOTS;
}

pub fn report_custody<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G) {
    chase_end(sc, g, ChaseEnd::Custody);
}

pub fn report_down<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G) {
    chase_end(sc, g, ChaseEnd::Down);
}

pub fn report_crime<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, at: G::Position, name: G::CrimeName) {
    if !sc.on {
        return;
    }
    let sound = g.crime_sound(sc, name);
    if sound == 0 {
        return;
    }
    g.queue_crime(sc, at, &Report::scripted(sound, KIND_SCRIPT_CRIME));
}

pub fn vigilante_crime<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, crime: u32, at: G::Position) {
    if !sc.on {
        return;
    }
    let made = sc.vigilante_names.made;
    if made & 1 == 0 {
        sc.vigilante_names.made = made | 1;
        for k in 0..VIGILANTE_CRIMES.len() {
            let h = g.hash_name(sc, Name::Vigilante(k as u32));
            sc.vigilante_names.hashes[k] = h;
        }
    }
    let hash = sc.vigilante_names.hash(crime);
    let sound = g.crime_sound_of(sc, hash);
    let kind = crime.wrapping_add(KIND_SCRIPT_CRIME);
    g.queue_crime(sc, at, &Report::scripted(sound, kind));
}

pub fn queued_dispatch<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G) {
    for k in 0..SENT_RECORDS as usize {
        let r = sc.sent[k];
        if !(r.used.on() && r.time.wrapping_add(SENT_DELAY_MS) < g.now()) {
            continue;
        }
        if !(0..16).contains(&(r.station as i32)) {
            sc.sent[k].used = ByteFlag::OFF;
            continue;
        }
        let from = g.station_position(sc, r.station);
        sc.sent[k].used = ByteFlag::OFF;
        let r = sc.sent[k];
        let few_police = r.kind as i32 == 0 && r.units <= 2;
        if few_police && g.player_wanted_level(sc) >= 2 {
            continue;
        }
        let r = sc.sent[k];
        g.report_dispatch(sc, r.units, r.kind, from);
    }
}

fn beaten_up_colour<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G) -> u32 {
    let made = sc.beaten_up_name.made;
    if made & 1 != 0 {
        return sc.beaten_up_name.hashes[0];
    }
    sc.beaten_up_name.made = made | 1;
    let h = g.hash_name(sc, Name::ColourBeatenUp);
    sc.beaten_up_name.hashes[0] = h;
    h
}

pub fn describe_vehicle<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, v: G::Vehicle) -> Description<u32> {
    let [prefix, colour] = g.colour_words(v);
    let boat = g.vehicle_type(v) == 2;
    let mut d = Description {
        prefix,
        colour,
        manufacturer: 0,
        model: 0,
        category: 0,
        audio: g.vehicle_audio(v),
        marked: g.vehicle_marked(v),
        bike: false,
    };
    if !boat && g.is_beaten_up(sc, g.vehicle_audio(v)) {
        d.colour = beaten_up_colour(sc, g);
        d.prefix = 0;
    }
    d.model = g.model_word(v, boat);
    d.manufacturer = g.manufacturer_word(v, boat);
    d.category = g.category_word(v, boat);
    if g.is_taxi(sc, g.vehicle_model(v)) {
        d.category = 0;
    }
    let model = g.vehicle_model(v);
    if g.vehicle_emergency(v) || model == g.ambulance_model() || model == g.firetruck_model() {
        (d.colour, d.manufacturer, d.prefix, d.category) = (0, 0, 0, 0);
    }
    d.bike = g.vehicle_type(v) == 1;
    d
}

fn forward_speed<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, v: G::Vehicle) -> f32 {
    let vel = g.vehicle_velocity(sc, v);
    let fwd = g.vehicle_forward(v);
    let d = vel[1] * fwd[1];
    let d = vel[2].mul_add(fwd[2], d);
    let d = vel[0].mul_add(fwd[0], d);
    let [scale, cap] = g.speed_scale();
    let speed = d * scale;
    if cap - speed >= 0.0 { speed } else { cap }
}

fn vehicle_part<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, ped: G::Ped) -> (Option<Description<u32>>, f32) {
    let mut v = if g.ped_in_vehicle(ped) { g.ped_vehicle(ped) } else { None };
    if v.is_none() {
        v = g.ped_task_vehicle(sc, ped);
    }
    match v {
        Some(v) => {
            let d = g.describe_vehicle_into(sc, v);
            (Some(d), forward_speed(sc, g, v))
        }
        None => (None, g.zero()),
    }
}

fn say_area_again<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, area: u32) -> bool {
    if sc.last_area != area { g.chance(sc, g.area_again_chance()) } else { false }
}

pub fn report_suspect<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, ped: Option<G::Ped>) {
    let Some(ped) = ped.filter(|_| sc.on) else { return };
    let at = g.ped_position(sc, ped);
    g.describe_place(sc, at);
    let (vehicle, speed) = vehicle_part(sc, g, ped);
    let place = g.report_place();
    let say_area = say_area_again(sc, g, place.area);
    g.queue_suspect(sc, &SuspectReport { place, vehicle, speed, say_area, priority: 4, heading: 0, last: 1 });
}

pub fn report_spotted<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, at: G::Position, moved: bool, last: u8) {
    if !sc.on {
        return;
    }
    if !moved && !g.chance(sc, g.spot_chance()) {
        let p = g.spot_chance() * g.search_share();
        if g.chance(sc, p) {
            g.report_search(sc, at);
        }
        return;
    }
    let player = g.player_ped(sc);
    g.describe_place(sc, at);
    let (vehicle, speed) = vehicle_part(sc, g, player);
    let priority = if moved { 3 } else { 1 };
    let place = g.report_place();
    let say_area = say_area_again(sc, g, place.area);
    let heading = sc.heading;
    g.queue_suspect(sc, &SuspectReport { place, vehicle, speed, say_area, priority, heading, last });
}

pub fn report_police_crime<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, at: G::Position, crime: u32, delay: u32) {
    if g.player_wanted_multiplier(sc) < g.crime_multiplier_floor() {
        return;
    }
    if g.player_wanted_level(sc) == 0 && g.fake_level() > 0 {
        return;
    }
    if crime >= POLICE_CRIMES {
        return;
    }
    let sound = g.crime_sound_of(sc, g.police_crime_hash(crime));
    if sound == 0 {
        return;
    }
    g.queue_crime(sc, at, &Report { priority: 3, sound, kind: crime, delay, sub: crime, ..Report::default() });
}

pub fn beaten_up(flags: Option<u16>) -> bool {
    flags.is_some_and(|f| f & 3 == 1)
}

pub fn pick_sound<G: WeightLists>(g: &mut G, list: G::WeightList, count: u32) -> u32 {
    let zero = g.zero();
    let mut sum = zero;
    for (_, weight) in g.weights(list, count) {
        sum += weight;
    }
    let x = g.random_float(zero, sum);
    crate::audioconfig::ScannerCrime::pick_below(&g.weights(list, count), x, zero)
}

pub fn may_play<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, s: SlotNo) -> bool {
    if sc.slot(s).kind as i32 == KIND_ALWAYS {
        if sc.slot(s).sub == 1 && !g.player_on_train(sc) {
            return false;
        }
        if sc.slot(s).sub != 1 && g.player_on_train(sc) {
            return false;
        }
        if sc.slot(s).time.wrapping_add(PLACE_LATE_MS) < g.now() {
            return false;
        }
    }
    if sc.slot(s).priority as i32 == PRIORITY_ALWAYS as i32 {
        return true;
    }
    let chasing = g.player_wanted_level(sc) != 0 && g.out_of_sight(sc) <= 0;
    if !chasing && sc.slot(s).priority as i32 != 0 && sc.slot(s).kind as i32 != KIND_CHASE_END as i32 {
        return false;
    }
    if sc.slot(s).vehicle {
        let Some(_) = g.player_vehicle(sc) else { return false };
        let Some(v) = g.player_vehicle(sc) else { return false };
        if Some(g.vehicle_audio(v)) != sc.slot(s).described() {
            return false;
        }
        if sc.slot(s).on_foot && g.player_vehicle(sc).is_some() {
            return false;
        }
    }
    true
}

#[derive(Clone, Copy)]
enum Gap {
    Zero,
    On,
    In,
    Err,
    Random,
}

#[derive(Clone, Copy)]
enum Word {
    Hash(Name),
    On,
    Area,
    Street,
    Direction,
    InDirection,
}

fn add_phrase<W: PlaceWords>(sc: &mut PoliceScanner, w: &mut W, r: &Report, s: SlotNo, count: &mut u32, bank: u32, gap: Gap, word: Word) {
    let k = *count;
    sc.slot_mut(s).phrase_mut(k).bank = bank;
    let gap = match gap {
        Gap::Zero => 0,
        Gap::On => w.on_gap_ms(),
        Gap::In => w.in_gap_ms(),
        Gap::Err => w.err_gap_ms(),
        Gap::Random => w.uniform(sc, AREA_GAP_MS[0], AREA_GAP_MS[1]),
    };
    sc.slot_mut(s).phrase_mut(k).gap = gap;
    sc.slot_mut(s).made_mut(k).sound = None;
    let name = match word {
        Word::Hash(name) => w.hash_name(sc, name),
        Word::On => w.hash_name(sc, if r.long { Name::OnLong } else { Name::OnShort }),
        Word::Area => r.place.area,
        Word::Street => r.place.street,
        Word::Direction => r.place.dir,
        Word::InDirection => r.place.in_dir,
    };
    sc.slot_mut(s).phrase_mut(k).sound = name;
    *count = k.wrapping_add(1);
}

pub fn say_place<W: PlaceWords>(sc: &mut PoliceScanner, w: &mut W, r: &Report, s: SlotNo, count: &mut u32) {
    let street = r.place.street;
    let named = street != 0 && street != w.null_sound() && w.street_known(sc, street);
    if named {
        add_phrase(sc, w, r, s, count, BANK_WORD, Gap::Zero, Word::On);
        add_phrase(sc, w, r, s, count, BANK_STREET, Gap::On, Word::Street);
        if sc.last_area == r.place.area && w.chance(sc, w.skip_area_chance()) {
            return;
        }
    } else {
        let mut said_in = false;
        if r.long && w.chance(sc, w.in_chance()) {
            add_phrase(sc, w, r, s, count, BANK_WORD, Gap::In, Word::Hash(Name::In));
            said_in = true;
            add_phrase(sc, w, r, s, count, BANK_ERR, Gap::Err, Word::Hash(Name::Err));
        }
        if r.place.dir != 0 {
            let word = if said_in { Word::Direction } else { Word::InDirection };
            add_phrase(sc, w, r, s, count, BANK_STREET, Gap::Zero, word);
            said_in = true;
        }
        if !said_in {
            add_phrase(sc, w, r, s, count, BANK_WORD, Gap::Zero, Word::Hash(Name::In));
        }
    }
    add_phrase(sc, w, r, s, count, BANK_AREA, Gap::Random, Word::Area);
    sc.slot_mut(s).area = r.place.area;
}

fn fmadds(a: f32, b: f32, c: f32) -> f32 {
    if cfg!(feature = "exe-math") { ((a as f64) * (b as f64) + c as f64) as f32 } else { a.mul_add(b, c) }
}

fn dot3(a: [f32; 4], b: [f32; 4]) -> f32 {
    use crate::estimate::vflush as vx;
    if cfg!(feature = "exe-math") {
        return vx(vx(vx(0.0 + vx(a[2] * b[2])) + vx(a[1] * b[1])) + vx(a[0] * b[0]));
    }
    if let Some(n) = [0, 2, 1].iter().flat_map(|&k| [a[k], b[k]]).find(|x| x.is_nan()) {
        return f32::from_bits(n.to_bits() | 0x40_0000);
    }
    let p = [0, 1, 2].map(|k| a[k] as f64 * b[k] as f64);
    let r = ((p[0] + p[2]) + p[1]) as f32;
    if r.is_nan() { f32::from_bits(0x7FC0_0000) } else { r }
}

fn to_int(f: f32) -> i64 {
    if f.is_nan() { i64::MIN } else { f as f64 as i64 }
}

fn heading_quarter<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, from: [f32; 4], now_at: [f32; 4], zero: f32) -> u32 {
    use crate::estimate::vflush as vx;
    let near = g.moving_distance();
    let limit = near * near;
    let a = from.map(vx);
    let p = now_at.map(vx);
    let d: [f32; 4] = std::array::from_fn(|k| vx(p[k] - a[k]));
    let length2 = fmadds(d[1], d[1], fmadds(d[0], d[0], d[2] * d[2]));
    if !(length2 > limit) {
        return STANDING;
    }
    let n = dot3(d, d);
    let e = if cfg!(feature = "exe-math") { vx(1.0 / vx(n).sqrt()) } else { vx(crate::estimate::vrsqrtefp(vx(n))) };
    let u: [f32; 4] = std::array::from_fn(|k| vx(d[k] * e));
    let z_part = u[2] * zero;
    let angle = g.heading_angle(sc, fmadds(u[0], zero, u[1]) + z_part);
    let side = fmadds(u[1], zero, u[0]) + z_part;
    let [to_degrees, full, half_quarter, per_quarter] = g.heading_constants();
    let mut degrees = angle * to_degrees;
    if !(side > zero) {
        degrees = full - degrees;
    }
    (to_int((degrees + half_quarter) * per_quarter) as u32) & 3
}

fn phrase_db<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, s: SlotNo, zero: f32, noise: bool) -> f32 {
    let mut db = if sc.slot(s).radio {
        let [a, b, c] = g.radio_levels();
        a * b * c
    } else {
        zero
    };
    if sc.slot(s).priority as i32 == 0 && (!noise || g.player_wanted_level(sc) == 0) {
        db += g.chatter_db();
    }
    db
}

fn current(sc: &PoliceScanner) -> (SlotNo, u32) {
    let s = sc.playing;
    (s, sc.slot(s).at)
}

fn current_made(sc: &PoliceScanner) -> Made {
    let (s, k) = current(sc);
    sc.slot(s).made(k)
}

fn start_phrase<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, sound: SoundHandle, db: f32) {
    g.set_volume(sc, sound, db);
    if let Some(echo) = current_made(sc).echo {
        g.set_volume(sc, echo, db);
    }
    if sc.slot(sc.playing).placed() {
        if sc.slot(sc.playing).follow.is_none() {
            g.place_at_slot(sc, sound, sc.playing);
            if let Some(echo) = current_made(sc).echo {
                g.place_at_slot(sc, echo, sc.playing);
            }
        }
        let group = sc.slot(sc.playing).group;
        g.set_group(sc, sound, group);
        let group = sc.slot(sc.playing).group;
        if let Some(echo) = current_made(sc).echo {
            g.set_group(sc, echo, group);
        }
    } else {
        g.unplace(sc, sound);
        if let Some(echo) = current_made(sc).echo {
            g.unplace(sc, echo);
        }
    }
    g.play(sc, sound);
    let (s, k) = current(sc);
    if let Some(echo) = sc.slot(s).made(k).echo {
        let bank = g.bank_of(sc.slot(s).phrase(k).bank);
        g.play_when_loaded(sc, echo, Some(bank));
    }
}

fn kind_bit(kind: u32) -> u32 {
    if kind & 63 >= 32 { 0 } else { 1 << (kind & 31) }
}

fn pick<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, now: u32) {
    sc.cut = ByteFlag::OFF;
    if now <= sc.not_before || g.paused() || g.held(sc) || !g.pick_report(sc) {
        return;
    }
    let slot = sc.slot(sc.playing);
    let (priority, kind) = (slot.priority as i32, slot.kind);
    let held_back = priority != 0 && priority != PRIORITY_ALWAYS as i32 && kind as i32 != KIND_ALWAYS;
    if held_back && sc.allowed & kind_bit(kind) == 0 {
        sc.slot_mut(sc.playing).queued = ByteFlag::OFF;
        return;
    }
    if g.make_slot(sc, sc.playing) && g.slot_load_state(sc, sc.playing) != 2 {
        let slot = sc.slot(sc.playing);
        let (kind, sub, area) = (slot.kind, slot.sub, slot.area);
        sc.last_kind = kind;
        sc.last_sub = sub;
        if area != 0 {
            sc.last_area = area;
        }
        sc.state = STARTING;
        sc.allowed = g.allowed_after(kind);
        return;
    }
    g.clear_slot(sc, sc.playing);
}

fn drop_playing<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, s: SlotNo) {
    g.clear_slot(sc, s);
    sc.state = IDLE;
}

fn start<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, zero: f32) {
    let s = sc.playing;
    if !sc.slot(s).queued.on() {
        return drop_playing(sc, g, s);
    }
    match g.slot_load_state(sc, s) {
        2 => return drop_playing(sc, g, sc.playing),
        1 => {}
        _ => return,
    }
    let s = sc.playing;
    let slot = sc.slot(s);
    let placed = slot.placed().then(|| NoisePlace { follow: slot.follow, group: slot.group });
    let req = NoiseRequest { category: slot.category, placed };
    let noise = slot.noise;
    g.make_noise_sound(sc, noise, s, &req);
    if let Some(noise) = sc.noise_sound {
        let db = phrase_db(sc, g, sc.playing, zero, true);
        g.set_volume(sc, noise, db);
        g.play_when_loaded(sc, noise, None);
    }
    let s = sc.playing;
    sc.slot_mut(s).at = 0;
    let s = sc.playing;
    let Some(sound) = current_made(sc).sound else { return drop_playing(sc, g, s) };
    let db = phrase_db(sc, g, sc.playing, zero, false);
    start_phrase(sc, g, sound, db);
    sc.state = SPEAKING;
    g.note_said_of_playing(sc);
}

fn near_end<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, sound: SoundHandle) -> bool {
    let length = g.sound_length(sound);
    let lead = if length >= 0 {
        let [frame, k] = g.lead_factors();
        (length.wrapping_neg() as u32).wrapping_sub(to_int(frame * k) as u32)
    } else {
        0
    };
    let pos = g.play_position(sc, sound);
    let overlap = g.overlap_ms();
    pos as i32 != -1 && lead > overlap && pos.wrapping_sub(lead) < overlap
}

fn speak<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G, now: u32, zero: f32) {
    let noise = sc.noise_sound;
    let s = sc.playing;
    if let Some(noise) = noise
        && sc.slot(s).placed()
        && sc.slot(s).follow.is_none()
    {
        g.place_at_slot(sc, noise, s);
    }
    if let Some(sound) = current_made(sc).sound
        && !near_end(sc, g, sound)
    {
        return;
    }
    let (s, k) = current(sc);
    let gap = sc.slot(s).phrase(k).gap;
    if gap != 0 {
        sc.not_before = gap.wrapping_add(now);
        sc.state = IN_GAP;
        return;
    }
    let s = sc.playing;
    let slot = sc.slot_mut(s);
    slot.at = slot.at.wrapping_add(1);
    let (k, count) = (slot.at, slot.phrase_count);
    if let Some(sound) = current_made(sc).sound
        && k < count
    {
        let db = phrase_db(sc, g, s, zero, false);
        start_phrase(sc, g, sound, db);
        return;
    }
    if let Some(noise) = sc.noise_sound {
        g.stop_sound(sc, noise);
    }
    sc.not_before = g.report_pause_ms().wrapping_add(now);
    let s = sc.playing;
    sc.slot_mut(s).queued = ByteFlag::OFF;
    sc.slot_mut(s).flag = ByteFlag::OFF;
    sc.playing = u32::MAX;
    sc.state = IDLE;
}

pub fn frame<G: ScannerGame>(sc: &mut PoliceScanner, g: &mut G) {
    if !g.has_player() {
        return;
    }
    if sc.cut.on() && g.screen_fade(sc) < g.fade_limit() {
        if sc.state as i32 == SPEAKING as i32 {
            g.clear_slot(sc, sc.playing);
        }
        if let Some(noise) = sc.noise_sound {
            stop_noise(sc, g, noise);
        }
        sc.state = IDLE;
        sc.cut = ByteFlag::OFF;
    }
    if !sc.on {
        return;
    }
    let now = g.now();
    let v = g.player_vehicle(sc);
    let radio = match v {
        Some(v) => {
            let audio = g.vehicle_audio(v);
            (g.is_police_audio(sc, audio) || g.vehicle_model(v) == g.ambulance_model()) && g.vehicle_radio_on(v)
        }
        None => false,
    };
    if radio {
        if !sc.police_car.on() {
            let t = g.uniform(sc, CHATTER_MIN_MS, CHATTER_MAX_MS);
            sc.chatter_at = t.wrapping_add(now);
        }
        sc.police_car = ByteFlag::ON;
    } else {
        sc.police_car = ByteFlag::OFF;
    }
    if g.player_wanted_level(sc) == 0 {
        sc.allowed = 1;
        if !sc.police_car.on() {
            sc.last_sub = 0;
        }
        let t = g.uniform(sc, CHATTER_MIN_MS, CHATTER_MAX_MS);
        sc.idle_until = t.wrapping_add(now);
    } else if now > sc.last_pick.wrapping_add(ALLOW_ALL_AFTER_MS) {
        sc.allowed = u32::MAX;
    }
    g.queued_dispatch(sc);
    if sc.police_car.on() {
        g.chatter(sc, now);
    }
    let zero = g.zero();
    if sc.sampled.wrapping_add(g.heading_interval()) <= now {
        let at = g.player_position(sc);
        let from = sc.sampled_at;
        let q = heading_quarter(sc, g, from, at, zero);
        sc.heading = q;
        sc.sampled_at = at;
        sc.sampled = now;
    }
    match sc.state {
        IDLE => pick(sc, g, now),
        STARTING => start(sc, g, zero),
        SPEAKING => speak(sc, g, now, zero),
        IN_GAP => {
            if now > sc.not_before {
                let (s, k) = current(sc);
                sc.slot_mut(s).phrase_mut(k).gap = 0;
                sc.state = SPEAKING;
            }
        }
        _ => {}
    }
}
