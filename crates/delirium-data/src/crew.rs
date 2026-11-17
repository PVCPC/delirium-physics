#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Driver,
    RearLeft,
    FrontPassenger,
    RearRight,
}

impl Entry {
    pub fn of(n: u32) -> Option<Entry> {
        [Entry::Driver, Entry::RearLeft, Entry::FrontPassenger, Entry::RearRight].get(n as usize).copied()
    }
}

pub const PED_IN_VEHICLE: u32 = 0x2000_0000;

pub const TYPE_COP: u32 = 2;
pub const GANG_FIRST: u32 = 3;
pub const GANG_LAST: u32 = 0xE;
pub const TYPE_MEDIC: u32 = 0x10;
pub const TYPE_FIREMAN: u32 = 0x11;
pub const TYPE_CRIMINAL: u32 = 0x12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Make {
    Driver(u32),
    Passenger(u32),
}

pub trait CrewGame {
    type Vehicle: Copy;
    type Ped: Copy + PartialEq;
    type Task: Copy;
    type Answer: Copy;
    fn driver_seat(&self, v: Self::Vehicle) -> Option<Self::Ped>;
    fn passenger_seat(&self, v: Self::Vehicle, n: u32) -> Option<Self::Ped>;
    fn make_ped(&mut self, v: Self::Vehicle, make: Make) -> Option<Self::Ped>;
    fn gang_vehicle(&self, v: Self::Vehicle) -> bool;
    fn is_gang(&mut self, ped: Self::Ped) -> bool;
    fn finish_gang_ped(&mut self, ped: Self::Ped);
    fn copy_alpha(&mut self, v: Self::Vehicle, ped: Self::Ped);
    fn model(&self, ped: Self::Ped) -> i16;
    fn ped_type(&self, ped: Self::Ped) -> u32;
    fn turn_away(&mut self, v: Self::Vehicle, ped: Self::Ped);
    fn delete(&mut self, ped: Self::Ped);
    fn crew_driver(&mut self, v: Self::Vehicle) -> Option<Self::Ped>;
    fn crew_passenger(&mut self, v: Self::Vehicle) -> Option<Self::Ped>;
    fn give_default_task(&mut self, ped: Self::Ped, v: Self::Vehicle);
    fn give_factory_task(&mut self, ped: Self::Ped, v: Self::Vehicle, other: Self::Ped, drives: bool);
    fn treat_injured(&mut self, v: Self::Vehicle, other: Self::Ped, drives: u32) -> Option<Self::Task>;
    fn drive_fire_truck(&mut self, v: Self::Vehicle, other: Self::Ped, drives: u32) -> Option<Self::Task>;
    fn give(&mut self, ped: Self::Ped, task: Option<Self::Task>) -> Self::Answer;
}

pub fn ped_in_seat<G: CrewGame>(g: &G, v: G::Vehicle, entry: Option<Entry>) -> Option<G::Ped> {
    match entry? {
        Entry::Driver => g.driver_seat(v),
        Entry::RearLeft => g.passenger_seat(v, 1),
        Entry::FrontPassenger => g.passenger_seat(v, 0),
        Entry::RearRight => g.passenger_seat(v, 2),
    }
}

fn finish_ped<G: CrewGame>(g: &mut G, v: G::Vehicle, ped: G::Ped) {
    if g.gang_vehicle(v) && g.is_gang(ped) {
        g.finish_gang_ped(ped);
    }
    g.copy_alpha(v, ped);
}

pub fn driver<G: CrewGame>(g: &mut G, v: G::Vehicle, given: u32) -> Option<G::Ped> {
    if let Some(ped) = g.driver_seat(v) {
        return Some(ped);
    }
    let ped = g.make_ped(v, Make::Driver(given))?;
    finish_ped(g, v, ped);
    g.driver_seat(v)
}

pub fn passenger<G: CrewGame>(g: &mut G, v: G::Vehicle, n: u32) -> Option<G::Ped> {
    if let Some(ped) = g.passenger_seat(v, n) {
        return Some(ped);
    }
    let made = g.make_ped(v, Make::Passenger(n))?;
    let model = g.model(made);
    finish_ped(g, v, made);
    let kind = g.ped_type(made);
    if kind == TYPE_COP || (GANG_FIRST..=GANG_LAST).contains(&kind) || kind == TYPE_MEDIC || kind == TYPE_FIREMAN || kind == TYPE_CRIMINAL {
        return Some(made);
    }
    if g.driver_seat(v).is_some_and(|d| g.model(d) == model) {
        g.turn_away(v, made);
        return None;
    }
    let mut kept = Some(made);
    for k in 0..(n as i32).max(0) as u32 {
        let other = g.passenger_seat(v, k);
        if let (Some(ped), Some(o)) = (kept, other) {
            if o != ped && g.model(o) == model {
                g.turn_away(v, ped);
                kept = None;
            }
        }
    }
    kept
}

pub fn fire_truck_crew<G: CrewGame>(g: &mut G, truck: G::Vehicle) -> bool {
    let Some(driver) = g.crew_driver(truck) else { return false };
    let Some(passenger) = g.crew_passenger(truck) else {
        g.delete(driver);
        return false;
    };
    g.give_default_task(driver, truck);
    g.give_factory_task(driver, truck, passenger, true);
    g.give_default_task(passenger, truck);
    g.give_factory_task(passenger, truck, driver, false);
    true
}

pub fn factory_task<G: CrewGame>(g: &mut G, ped: G::Ped, v: G::Vehicle, other: G::Ped, drives: u32) -> Option<G::Answer> {
    let task = match g.ped_type(ped) {
        TYPE_MEDIC => g.treat_injured(v, other, drives),
        TYPE_FIREMAN => g.drive_fire_truck(v, other, drives),
        _ => return None,
    };
    Some(g.give(ped, task))
}
