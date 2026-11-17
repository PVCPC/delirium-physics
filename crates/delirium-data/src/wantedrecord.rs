pub const CRIME_SLOTS: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CrimeSlot<S = u32> {
    pub kind: u32,
    pub subject: S,
    pub time: u32,
    pub place: [f32; 4],
    pub seen: u8,
    pub byte_21: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wanted<S = u32> {
    pub points: u32,
    pub fake_level: u32,
    pub raise_points: u32,
    pub level_changed_at: u32,
    pub increment: u32,
    pub raise_at: u32,
    pub outside_ms: u32,
    pub raise_started: u32,
    pub multiplier: f32,
    pub cop_cars_allowed: u8,
    pub circle_centre: [u32; 4],
    pub last_seen: [u32; 4],
    pub unseen_ms: u32,
    pub word_54: u32,
    pub roadblocks: u16,
    pub control: u8,
    pub control_2: u8,
    pub wanted_time: f32,
    pub six_star_time: f32,
    pub byte_64: u8,
    pub byte_65: u8,
    pub level: u32,
    pub pending_level: u32,
    pub crimes: [CrimeSlot<S>; CRIME_SLOTS],
    pub cops_allowed: u8,
    pub held_off_last_frame: u8,
    pub search_time: u32,
}

pub const HELD_FOR_HELICOPTERS: u8 = 0x80;
pub const RAISE_WAITING: u8 = 0x40;

impl<S: Copy + Default> Wanted<S> {
    pub fn clear_search(&mut self) {
        self.search_time = 0;
        self.control_2 &= !HELD_FOR_HELICOPTERS;
        self.last_seen = [0; 4];
    }

    pub fn reset(&mut self, centre_w: u32) {
        let control_2 = self.control_2 & !RAISE_WAITING;
        let crimes = self.crimes.map(|c| CrimeSlot { kind: 0, subject: S::default(), ..c });
        *self = Wanted {
            raise_at: u32::MAX,
            multiplier: 1.0,
            circle_centre: [0, 0, 0, centre_w],
            last_seen: self.last_seen,
            search_time: self.search_time,
            control_2,
            crimes,
            ..Wanted::default()
        };
    }
}

pub fn note_crime<S: Copy + PartialEq>(slots: &mut [CrimeSlot<S>; CRIME_SLOTS], kind: u32, subject: S, place: [f32; 4],
                                       seen: u8, c: u8, now: u32) -> bool {
    if let Some(slot) = slots.iter_mut().find(|s| s.kind == kind && s.subject == subject) {
        if slot.seen != 0 {
            return true;
        }
        if seen != 0 {
            slot.seen = seen;
        }
        return false;
    }
    if let Some(slot) = slots.iter_mut().find(|s| s.kind == 0) {
        *slot = CrimeSlot { kind, subject, time: now, place, seen, byte_21: c };
    }
    false
}

pub const WITNESS_FAR: f32 = 60.0;
pub const WITNESS_MIDDLE: f32 = 30.0;
pub const WITNESS_NEAR: f32 = 18.0;

pub fn witness_radius(kind: u32) -> f32 {
    match kind {
        17 | 21 | 22 => WITNESS_FAR,
        25 => WITNESS_MIDDLE,
        _ => WITNESS_NEAR,
    }
}

pub trait WantedRaise {
    type Place: Copy;
    fn pending_level(&self) -> u32;
    fn level(&self) -> u32;
    fn clear_pending(&mut self);
    fn set_level(&mut self, place: Self::Place, level: u32);
}

pub fn take_pending_level<G: WantedRaise>(game: &mut G, place: G::Place) {
    let pending = game.pending_level();
    if pending == 0 {
        return;
    }
    game.clear_pending();
    if pending as i32 > game.level() as i32 {
        game.set_level(place, pending);
    }
}

pub trait LocalWanted {
    type Player: Copy;
    type Out: Copy;
    fn local_player(&mut self) -> Option<Self::Player>;
    fn level(&self, player: Self::Player) -> u32;
    fn circle_centre(&self, player: Self::Player) -> [u32; 4];
    fn put_place(&mut self, out: Self::Out, place: [u32; 4]);
}

pub fn wanted_state<G: LocalWanted>(g: &mut G, out: Option<G::Out>) -> u32 {
    let Some(player) = g.local_player() else { return 0 };
    let level = g.level(player);
    if let Some(out) = out {
        let centre = g.circle_centre(player);
        g.put_place(out, centre);
    }
    level
}

pub const FLASH_MS: u32 = 500;
pub const FLASH_FAST_MS: u32 = 250;
pub const FLASH_FAST_AFTER_MS: u32 = 1000;
pub const BLIP_COLOUR: u32 = 0x24;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FakeWanted {
    pub level: i32,
    pub circle: bool,
    pub circle_place: [f32; 2],
    pub circle_radius: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flash {
    pub started: bool,
    pub last_flip: u32,
    pub on: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchBlip<B> {
    pub blip: Option<B>,
    pub flash: Flash,
}

pub trait SearchBlipGame {
    type Record: Copy;
    type Blip: Copy;
    fn local_wanted(&mut self) -> Option<Self::Record>;
    fn level(&self, record: Option<Self::Record>) -> i32;
    fn unseen_ms(&self, record: Self::Record) -> u32;
    fn search_radius(&self, level: u32) -> f32;
    fn wanted_state(&mut self) -> (u32, [f32; 4]);
    fn player_place(&mut self) -> [f32; 4];
    fn blip_exists(&mut self, blip: Option<Self::Blip>) -> bool;
    fn make_blip(&mut self, record: Self::Record) -> Option<Self::Blip>;
    fn move_blip(&mut self, blip: Option<Self::Blip>, place: [f32; 4]);
    fn size_blip(&mut self, blip: Option<Self::Blip>, radius: f32);
    fn colour_blip(&mut self, blip: Option<Self::Blip>, colour: u32);
    fn game_time(&self) -> u32;
    fn clock(&self) -> u32;
}

fn really_wanted<G: SearchBlipGame>(g: &mut G) -> bool {
    g.local_wanted().is_some() && {
        let w = g.local_wanted();
        g.level(w) > 0
    }
}

pub fn draw_search_blip<G: SearchBlipGame>(search: &mut SearchBlip<G::Blip>, fake: &FakeWanted, g: &mut G,
                                           record: G::Record) {
    if !really_wanted(g) && fake.level <= 0 {
        return;
    }
    let mut radius = 0.0;
    let place = if really_wanted(g) {
        let (level, centre) = g.wanted_state();
        radius = g.search_radius(level);
        centre
    } else if fake.level <= 0 {
        [0.0; 4]
    } else {
        let at = g.player_place();
        radius = g.search_radius(fake.level as u32);
        if fake.circle {
            radius = fake.circle_radius;
            [fake.circle_place[0], fake.circle_place[1], 0.0, at[3]]
        } else {
            at
        }
    };
    if !g.blip_exists(search.blip) {
        search.blip = g.make_blip(record);
    } else {
        g.move_blip(search.blip, place);
    }
    g.size_blip(search.blip, radius);
    let flash = &mut search.flash;
    if !flash.started {
        flash.started = true;
        flash.last_flip = g.game_time();
    }
    let period = if g.unseen_ms(record) > FLASH_FAST_AFTER_MS { FLASH_FAST_MS } else { FLASH_MS };
    let now = g.clock();
    if now.wrapping_sub(flash.last_flip) > period {
        flash.on = !flash.on;
        flash.last_flip = now;
    }
    let colour = BLIP_COLOUR + !flash.on as u32;
    g.colour_blip(search.blip, colour);
}

pub trait WantedGame {
    type Blip: Copy;
    fn wanted(&mut self) -> Wanted;
    fn set_wanted(&mut self, wanted: &Wanted);
    fn unset_stack_word(&mut self) -> u32;
    fn blip_exists(&mut self, blip: Option<Self::Blip>) -> bool;
    fn remove_blip(&mut self, blip: Option<Self::Blip>);
}

pub fn reset_wanted<G: WantedGame>(game: &mut G, search: &mut SearchBlip<G::Blip>) {
    let mut wanted = game.wanted();
    let centre_w = game.unset_stack_word();
    wanted.reset(centre_w);
    game.set_wanted(&wanted);
    if game.blip_exists(search.blip) {
        game.remove_blip(search.blip);
    }
    search.blip = None;
}

