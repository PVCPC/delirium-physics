use crate::rng::Rand;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Response {
    pub task: i32,
    pub chance: [u8; 4],
    pub on_foot: bool,
    pub in_vehicle: bool,
    pub flags: u16,
}

impl Default for Response {
    fn default() -> Self {
        Response { task: -1, chance: [0; 4], on_foot: false, in_vehicle: false, flags: 0 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot {
    pub event: i32,
    pub responses: [Response; 6],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attributes {
    pub target_loss_response: u8,
    pub can_change_target: bool,
    pub fire_rate: u8,
    pub caution: u8,
    pub low_health: u8,
    pub movement_style: u8,
    pub standing_style: u8,
    pub navigation_style: u8,
    pub retreating_behaviour: u8,
    pub teamwork: u8,
    pub sight_range: u8,
    pub weapon_accuracy: u8,
    pub target_injured_reaction: u8,
    pub fire_ok: u8,
}

impl Default for Attributes {
    fn default() -> Self {
        Attributes::from_values(&[0; 14])
    }
}

impl Attributes {
    fn from_values(v: &[i32; 14]) -> Self {
        let bits = |x: i32, n: u32| (x as u32 & ((1 << n) - 1)) as u8;
        Attributes {
            target_loss_response: bits(v[0], 2),
            can_change_target: v[1] & 1 != 0,
            fire_rate: bits(v[2], 2),
            caution: bits(v[3], 2),
            low_health: v[4] as u8,
            movement_style: bits(v[5], 3),
            standing_style: bits(v[6], 2),
            navigation_style: bits(v[7], 2),
            retreating_behaviour: bits(v[8], 2),
            teamwork: bits(v[9], 2),
            sight_range: v[10] as u8,
            weapon_accuracy: v[11] as u8,
            target_injured_reaction: bits(v[12], 2),
            fire_ok: bits(v[13], 2),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionMaker {
    pub slots: [Slot; 26],
    pub attributes: Attributes,
}

impl Default for DecisionMaker {
    fn default() -> Self {
        DecisionMaker { slots: [Slot { event: -1, responses: [Response::default(); 6] }; 26], attributes: Attributes::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ped = 0,
    Group = 1,
    Combat = 2,
    GroupCombat = 3,
}

impl DecisionMaker {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let head = lines.next().ok_or("empty decision file")?;
        let version: i32 = head.strip_prefix("data values").and_then(|v| v.trim_end_matches(':').trim().parse().ok())
            .ok_or_else(|| format!("bad decision header {head:?}"))?;
        if version != 19 {
            return Err(format!("decision file version {version}, only 19 is read"));
        }
        let ints = |l: &str| -> Result<Vec<f32>, String> {
            l.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| s.parse::<f32>().map_err(|e| format!("{s:?}: {e}"))).collect()
        };
        let a = ints(lines.next().ok_or("no attribute line")?)?;
        let a: [i32; 14] = a.iter().map(|&x| x as i32).collect::<Vec<_>>().try_into().map_err(|_| "attribute line is not 14 values")?;
        let mut dm = DecisionMaker { attributes: Attributes::from_values(&a), ..Default::default() };
        while let Some(l) = lines.next() {
            let event = ints(l)?.first().copied().ok_or("empty event line")? as i32;
            let mut slot = Slot { event, responses: [Response::default(); 6] };
            for r in slot.responses.iter_mut() {
                let v = ints(lines.next().ok_or("block cut short")?)?;
                if v.len() < 26 {
                    return Err(format!("response line of {} values", v.len()));
                }
                let byte = |x: f32| x as i64 as u8;
                *r = Response {
                    task: v[0] as i32,
                    chance: [byte(v[4]), byte(v[3]), byte(v[1]), byte(v[2])],
                    in_vehicle: v[5] as i32 != 0,
                    on_foot: v[6] as i32 != 0,
                    flags: (0..12).filter(|&k| v[14 + k] as i32 != 0).fold(0, |f, k| f | 1 << k),
                };
            }
            let at = dm.slots.iter().position(|s| s.event == event).or_else(|| dm.slots.iter().position(|s| s.event == -1))
                .ok_or("more than 26 events")?;
            dm.slots[at] = slot;
        }
        Ok(dm)
    }

    pub fn pick(&self, event: i32, column: usize, in_vehicle: bool, mask: Option<u16>, weights: &[(i32, f32)], allowed: &[i32],
                rand: &mut Rand) -> i32 {
        let Some(slot) = self.slots.iter().find(|s| s.event == event) else { return 200 };
        let mut taken: Vec<(i32, u8)> = Vec::with_capacity(6);
        for r in &slot.responses {
            let weight = if weights.is_empty() { 1.0 } else { weights.iter().find(|w| w.0 == r.task).map_or(1.0, |w| w.1) };
            let chance = r.chance[column];
            if chance == 0 || mask.is_some_and(|m| m & !r.flags != 0) || r.task == -1 || weight <= 0.0
                || (!allowed.is_empty() && !allowed.contains(&r.task)) || !if in_vehicle { r.in_vehicle } else { r.on_foot } {
                continue;
            }
            taken.push((r.task, if weight < 1.0 { (chance as f32 * weight) as i64 as u8 } else { chance }));
        }
        if taken.is_empty() {
            return 200;
        }
        let total: f32 = taken.iter().map(|t| t.1 as f32).sum();
        let roll = rand.next() as f32 * f32::from_bits(0x3800_0100);
        let mut sum = 0.0f32;
        for (task, chance) in taken {
            sum += chance as f32;
            if roll <= sum * (1.0 / total) {
                return task;
            }
        }
        200
    }
}

pub const PLAYER: &str = "m_plyr.ped";
pub const DEFAULT: &str = "R_Norm.ped";
pub const MISSION_DEFAULT: &str = "m_norm.ped";
pub const INDEXED: [(&str, Kind); 34] = [
    ("GP_Norm.ped", Kind::Group), ("GP_JustFollow.ped", Kind::Group), ("Cop.ped", Kind::Ped), ("R_Norm.ped", Kind::Ped),
    ("Fireman.ped", Kind::Ped), ("m_empty.ped", Kind::Ped), ("Combat/gp_cr.cmb", Kind::GroupCombat),
    ("Combat/gp_no.cmb", Kind::GroupCombat), ("Combat/gp_pr.cmb", Kind::GroupCombat), ("Combat/gp_hide.cmb", Kind::GroupCombat),
    ("Combat/m_no_no.cmb", Kind::Combat), ("Combat/m_Coward.cmb", Kind::Combat), ("Combat/m_no_no.cmb", Kind::Combat),
    ("Combat/m_no_no.cmb", Kind::Combat), ("Combat/cop_nrm.cmb", Kind::Combat), ("Combat/cop_wl1.cmb", Kind::Combat),
    ("Combat/cop_wl2.cmb", Kind::Combat), ("Combat/cop_wl3.cmb", Kind::Combat), ("m_empty.ped", Kind::Ped),
    ("m_norm.ped", Kind::Ped), ("m_tough.ped", Kind::Ped), ("m_weak.ped", Kind::Ped), ("Combat/m_Empty.cmb", Kind::Combat),
    ("Combat/m_Coward.cmb", Kind::Combat), ("Combat/m_no_de.cmb", Kind::Combat), ("Combat/m_no_fi.cmb", Kind::Combat),
    ("Combat/m_hide.cmb", Kind::Combat), ("Combat/m_cr_de.cmb", Kind::Combat), ("Combat/m_cr_no.cmb", Kind::Combat),
    ("Combat/m_cr_fi.cmb", Kind::Combat), ("Combat/m_pr_de.cmb", Kind::Combat), ("Combat/m_pr_no.cmb", Kind::Combat),
    ("Combat/m_pr_fi.cmb", Kind::Combat), ("Combat/m_boss.cmb", Kind::Combat),
];
pub const STORE_SIZE: usize = 0x36;

impl DecisionMaker {
    pub fn clear_event(&mut self, event: i32) {
        if let Some(s) = self.slots.iter_mut().find(|s| s.event == event) {
            *s = Slot { event: -1, responses: [Response::default(); 6] };
        }
    }

    pub fn add_response(&mut self, event: i32, task: i32, chance: [u8; 4], on_foot: bool, in_vehicle: bool) {
        let k = self.slots.iter().position(|s| s.event == event).or_else(|| self.slots.iter().position(|s| s.event == -1));
        let Some(k) = k else { return };
        let s = &mut self.slots[k];
        s.event = event;
        let empty = |r: &Response| r.task == -1 && r.chance == [0; 4] && !r.on_foot && !r.in_vehicle;
        if let Some(r) = s.responses.iter_mut().find(|r| empty(r)) {
            *r = Response { task, chance, on_foot, in_vehicle, flags: 0 };
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ScriptStore {
    slots: Vec<Option<(DecisionMaker, Kind, u32)>>,
    generation: Vec<u16>,
}

impl ScriptStore {
    pub const fn new() -> ScriptStore {
        ScriptStore { slots: Vec::new(), generation: Vec::new() }
    }

    fn ensure(&mut self) {
        if self.slots.is_empty() {
            self.slots = vec![None; STORE_SIZE - INDEXED.len()];
            self.generation = vec![0; STORE_SIZE - INDEXED.len()];
        }
    }

    pub fn make(&mut self, of: DecisionMaker, kind: Kind, owner: u32) -> i32 {
        self.ensure();
        let Some(k) = self.slots.iter().position(Option::is_none) else { return 0 };
        self.generation[k] = self.generation[k].wrapping_add(1).max(1);
        self.slots[k] = Some((of, kind, owner));
        (self.generation[k] as i32) << 16 | (INDEXED.len() + k) as i32
    }

    pub fn number(&self, handle: i32) -> i32 {
        let index = (handle & 0xFFFF) as usize;
        if index < INDEXED.len() {
            return index as i32;
        }
        let k = index - INDEXED.len();
        match (self.slots.get(k), self.generation.get(k)) {
            (Some(Some(_)), Some(&g)) if g as i32 == (handle >> 16) & 0xFFFF => index as i32,
            _ => -1,
        }
    }

    pub fn get(&self, number: i32) -> Option<&(DecisionMaker, Kind, u32)> {
        let k = usize::try_from(number).ok()?.checked_sub(INDEXED.len())?;
        self.slots.get(k)?.as_ref()
    }

    pub fn get_mut(&mut self, number: i32) -> Option<&mut DecisionMaker> {
        let k = usize::try_from(number).ok()?.checked_sub(INDEXED.len())?;
        self.slots.get_mut(k)?.as_mut().map(|s| &mut s.0)
    }

    pub fn remove(&mut self, number: i32) {
        if let Some(k) = usize::try_from(number).ok().and_then(|n| n.checked_sub(INDEXED.len())) {
            if let Some(s) = self.slots.get_mut(k) {
                *s = None;
            }
        }
    }

    pub fn owned(&self, owner: u32) -> Vec<(i32, Kind)> {
        self.slots.iter().enumerate().filter_map(|(k, s)| s.as_ref().filter(|x| x.2 == owner).map(|x| ((INDEXED.len() + k) as i32, x.1))).collect()
    }

    pub fn release(&mut self, owner: u32) {
        for s in &mut self.slots {
            if s.as_ref().is_some_and(|x| x.2 == owner) {
                *s = None;
            }
        }
    }
}

pub const EMPTY: i32 = 5;
pub const COMBAT_DEFAULT: i32 = 12;
pub const GROUP_COMBAT_DEFAULT: i32 = 7;
pub const COMBAT_COP: i32 = 14;
pub const COMBAT_MISSION: i32 = 10;

pub const LAW_MODEL: u64 = 0x7C00_0000;
pub const FIREMAN_MODEL: u64 = 0x1_0000_0000;
pub const PED_COP: i32 = 2;
pub const PED_STREET: i32 = 3;
pub const PED_FIREMAN: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Defaults {
    pub ped: i32,
    pub combat: Option<i32>,
}

pub fn street_ped(model_flags: u64, security: bool) -> i32 {
    if model_flags & LAW_MODEL != 0 || security {
        PED_COP
    } else if model_flags & FIREMAN_MODEL != 0 {
        PED_FIREMAN
    } else {
        PED_STREET
    }
}

pub fn defaults(player: bool, mission: bool, model_flags: u64, security: bool) -> Defaults {
    if player {
        Defaults { ped: -2, combat: None }
    } else if mission && model_flags & LAW_MODEL == 0 {
        Defaults { ped: -1, combat: Some(COMBAT_MISSION) }
    } else {
        let ped = street_ped(model_flags, security);
        let combat = match ped {
            PED_COP => Some(COMBAT_COP),
            PED_STREET => Some(COMBAT_DEFAULT),
            _ => None,
        };
        Defaults { ped, combat }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub ped: i32,
    pub group: i32,
    pub combat: i32,
    pub group_combat: i32,
}

pub fn freed(n: i32, mut h: Held, mission: bool, model_flags: u64, security: bool) -> (Held, bool) {
    let mut called = false;
    if h.ped == n {
        h.ped = street_ped(model_flags, security);
    }
    if h.combat == n {
        (h.combat, called) = (if mission { COMBAT_MISSION } else { COMBAT_DEFAULT }, true);
    }
    if h.group == n {
        h.group = 0;
    }
    if h.group_combat == n {
        h.group_combat = GROUP_COMBAT_DEFAULT;
    }
    (h, called)
}

pub fn cop_combat(level: i32, network: bool) -> i32 {
    match level {
        1 => 15,
        2 => 16,
        l if l > 2 => 17,
        _ if network => 17,
        _ => COMBAT_COP,
    }
}

pub const HELI_ANIM_GROUP: i32 = 0x59;
pub const HELI_SENSE: f32 = 150.0;

#[derive(Clone, Debug)]
pub struct Store {
    pub indexed: Vec<(DecisionMaker, Kind)>,
    pub default: DecisionMaker,
    pub mission_default: DecisionMaker,
    pub player: DecisionMaker,
}

impl Store {
    pub fn load(read: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let one = |name: &str| DecisionMaker::parse(&read(name).ok_or_else(|| format!("no {name}"))?).map_err(|e| format!("{name}: {e}"));
        Ok(Store {
            indexed: INDEXED.iter().map(|&(n, k)| Ok((one(n)?, k))).collect::<Result<_, String>>()?,
            default: one(DEFAULT)?,
            mission_default: one(MISSION_DEFAULT)?,
            player: one(PLAYER)?,
        })
    }

    pub fn get(&self, index: i32, mission: bool) -> Option<&DecisionMaker> {
        match index {
            -2 => Some(&self.player),
            -1 if mission => Some(&self.mission_default),
            -1 => Some(&self.default),
            i => self.indexed.get(usize::try_from(i).ok()?).map(|d| &d.0),
        }
    }

    pub fn kind(&self, index: i32) -> Option<Kind> {
        self.indexed.get(usize::try_from(index).ok()?).map(|d| d.1)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pick(&self, index: i32, mission: bool, health: f32, event: i32, column: usize, in_vehicle: bool, mask: u16,
                weights: &[(i32, f32)], allowed: &[i32], rand: &mut Rand) -> i32 {
        self.pick_with(None, index, mission, health, event, column, in_vehicle, mask, weights, allowed, rand)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pick_with(&self, scripts: Option<&ScriptStore>, index: i32, mission: bool, health: f32, event: i32, column: usize, in_vehicle: bool,
                     mask: u16, weights: &[(i32, f32)], allowed: &[i32], rand: &mut Rand) -> i32 {
        let script = scripts.and_then(|s| s.get(index));
        let kind = script.map(|s| s.1).or_else(|| self.kind(index));
        if let Some((dm, k, _)) = script.filter(|_| !(health < 100.0 && kind != Some(Kind::Combat))) {
            let mask = matches!(k, Kind::Combat | Kind::GroupCombat).then_some(mask);
            return dm.pick(event, column, in_vehicle, mask, weights, allowed, rand);
        }
        let index = if health < 100.0 && kind != Some(Kind::Combat) { EMPTY } else { index };
        let mask = matches!(kind, Some(Kind::Combat | Kind::GroupCombat)).then_some(mask);
        self.get(index, mission).map_or(200, |d| d.pick(event, column, in_vehicle, mask, weights, allowed, rand))
    }
}

pub mod situation {
    pub const LOW_HEALTH: u16 = 1 << 0;
    pub const NOT_LOW_HEALTH: u16 = 1 << 1;
    pub const UNARMED: u16 = 1 << 2;
    pub const ONE_HANDED: u16 = 1 << 3;
    pub const TWO_HANDED: u16 = 1 << 4;
    pub const TARGET_UNARMED: u16 = 1 << 5;
    pub const TARGET_GUN: u16 = 1 << 6;
    pub const TARGET_ARMED: u16 = 1 << 7;
    pub const NOT_GROUP_TARGET: u16 = 1 << 8;
    pub const GROUP_TARGET: u16 = 1 << 9;
    pub const TARGET_IN_VEHICLE: u16 = 1 << 10;
    pub const TARGET_ON_FOOT: u16 = 1 << 11;

    pub fn mask(low: bool, own: u16, target_armed: u8, target_in_vehicle: bool) -> u16 {
        let health = if low { LOW_HEALTH } else { NOT_LOW_HEALTH };
        let target = if target_armed == 2 { TARGET_GUN | TARGET_ARMED } else { TARGET_UNARMED };
        health | own | target | NOT_GROUP_TARGET | if target_in_vehicle { TARGET_IN_VEHICLE } else { TARGET_ON_FOOT }
    }
}

