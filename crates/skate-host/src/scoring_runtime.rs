use skate_core::{
    animation::output::attributes::AttributeName,
    physics::filtered_state::FilteredCategory,
    scoring::{
        carrier::{Carrier, delay_ticks},
        conversions::LINKS,
        session::Session,
    },
};
use skate_data::{collections::Collections, scoring::ScoringData};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Collector {
    None,
    Ground,
    Air,
    Grind,
    Offboard,
    Special,
}
pub(crate) struct Frame {
    pub tick: u32,
    pub dt: f32,
    pub category: FilteredCategory,
    pub state: u32,
    pub descriptor: Option<AttributeName>,
    pub grind_id: i32,
    pub flags: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub forward: [f32; 3],
    pub switch: bool,
    pub fakie: bool,
    pub nollie: bool,
    pub body_flip: bool,
    pub suspend_air: bool,
    pub landing: skate_core::animation::landing_quality::Output,
    pub teleported: bool,
    pub reverting: bool,
}
pub(crate) struct Runtime {
    pub data: ScoringData,
    pub session: Session,
    collector: Collector,
    carriers: [Option<Carrier>; 4],
    held: [f32; 4],
    distance: [f32; 4],
    metric_rewards: [f32; 4],
    metric_started: [bool; 4],
    start: [f32; 3],
    previous: [f32; 3],
    previous_heading: f32,
    spin: f32,
    peak: f32,
    air_factor: f32,
    air_repetition: f32,
    air_repetition_set: bool,
    grab_chain: u32,
    air_metrics: [f32; 5],
    landing_countdown: u32,
    idle_ticks: u32,
    collector_ticks: u32,
    manual_revert_ticks: u32,
    revert_id: Option<usize>,
    sequence_active: bool,
    sequence_score: f32,
    trick_name: String,
    stance: [bool; 4],
    clean: bool,
    sketchy: bool,
    pub new_trick: bool,
    pub modified_trick: bool,
    pub close_tricks: bool,
}
impl Runtime {
    pub fn load(data: &Collections) -> Result<Self, String> {
        Ok(Self {
            data: ScoringData::load(data)?,
            session: Session::default(),
            collector: Collector::None,
            carriers: std::array::from_fn(|_| None),
            held: [0.; 4],
            distance: [0.; 4],
            metric_rewards: [0.; 4],
            metric_started: [false; 4],
            start: [0.; 3],
            previous: [0.; 3],
            previous_heading: 0.,
            spin: 0.,
            peak: 0.,
            air_factor: 1.,
            air_repetition: 1.,
            air_repetition_set: false,
            grab_chain: 0,
            air_metrics: [0.; 5],
            landing_countdown: 0,
            idle_ticks: 0,
            collector_ticks: 0,
            manual_revert_ticks: 0,
            revert_id: None,
            sequence_active: false,
            sequence_score: 0.,
            trick_name: String::new(),
            stance: [false; 4],
            clean: false,
            sketchy: false,
            new_trick: false,
            modified_trick: false,
            close_tricks: false,
        })
    }

    fn penalty(&self, id: usize) -> f32 {
        let Some(d) = self.data.by_id(id) else {
            return 1.;
        };
        if d.metadata.repetition_applies() {
            self.data.repetition.evaluate(
                self.session
                    .holder
                    .repetition_count(d.metadata)
                    .unwrap_or(0) as f32,
            )
        } else {
            1.
        }
    }
    fn finish(&mut self, slot: usize, complete: bool, keep_metric: bool) {
        if let Some(mut carrier) = self.carriers[slot].take() {
            if complete {
                carrier.complete(self.data.unannounced_factor);
                if self.collector == Collector::Air {
                    self.session
                        .holder
                        .end_trick(carrier.scorable, carrier.reward);
                } else {
                    let metric = self.collector == Collector::Grind
                        || self.collector == Collector::Ground && slot < 3;
                    if !(keep_metric && self.collector == Collector::Ground) {
                        let reward = if keep_metric {
                            0.
                        } else if metric {
                            if carrier.scorable.score_type == 9 {
                                carrier.reward.max(self.metric_rewards[slot])
                            } else {
                                self.metric_rewards[slot].max(0.)
                            }
                        } else {
                            carrier.reward
                        };
                        self.session.holder.credit_trick(carrier.scorable, reward);
                    }
                }
            }
        }
        if !keep_metric {
            self.metric_started[slot] = false;
            self.held[slot] = 0.;
            self.distance[slot] = 0.;
            self.metric_rewards[slot] = 0.;
        }
    }
    fn carrier(&mut self, slot: usize, id: Option<usize>, f: &Frame) -> Result<(), String> {
        if self.carriers[slot].as_ref().map(|c| c.scorable.id) != id {
            let conversion = id.filter(|new| {
                self.collector == Collector::Air
                    && self.carriers[slot].is_some()
                    && LINKS[*new].0 >= 0
            });
            let chained_grab = self.collector == Collector::Air
                && id
                    .and_then(|id| self.data.by_id(id))
                    .is_some_and(|d| d.metadata.class == 2)
                && self.carriers[slot]
                    .as_ref()
                    .is_some_and(|c| c.scorable.class == 2 && !c.announced);
            let previous_tick = self.carriers[slot].as_ref().map(|c| c.start_tick);
            let keep_metric = id.is_some()
                && (self.collector == Collector::Grind
                    || self.collector == Collector::Ground && slot == 0);
            if conversion.is_none() {
                if chained_grab {
                    self.carriers[slot] = None;
                    self.grab_chain += 1;
                } else {
                    self.finish(slot, true, keep_metric);
                    self.grab_chain = 0;
                }
            }
            if let Some(id) = id {
                let d = self
                    .data
                    .by_id(id)
                    .ok_or_else(|| format!("Missing native scorable {id}"))?;
                let factor = self.penalty(id)
                    * if self.collector == Collector::Air {
                        self.air_factor
                    } else {
                        1.
                    };
                let mut carrier = Carrier::new(
                    d.metadata,
                    d.points,
                    factor,
                    self.data.announcement.evaluate(d.points as f32),
                    if chained_grab {
                        previous_tick.unwrap_or(f.tick)
                    } else {
                        f.tick
                    },
                    delay_ticks(
                        d.completion_delay,
                        if self.grab_chain > 1 {
                            self.data.collector.scalar(0x698)
                        } else {
                            0.
                        },
                    ),
                    f.switch,
                    f.fakie,
                );
                if let Some(old) = self.carriers[slot].as_mut() {
                    old.convert_to(&mut carrier, self.data.unannounced_factor);
                    self.modified_trick = true;
                }
                if conversion.is_some() {
                    self.trick_name = d.label.clone();
                }
                self.carriers[slot] = Some(carrier);
                self.sequence_active = true;
                self.idle_ticks = 0;
            }
        }
        let penalty = self.carriers[slot]
            .as_ref()
            .map(|c| self.penalty(c.scorable.id))
            .unwrap_or(1.);
        if let Some(c) = self.carriers[slot].as_mut() {
            if c.announce(f.tick, self.data.unannounced_factor) {
                let d = self
                    .data
                    .by_id(c.scorable.id)
                    .ok_or("Missing announced scorable")?;
                self.trick_name = d.label.clone();
                self.stance = [f.switch, f.fakie, f.nollie, false];
                self.new_trick = true;
                if self.collector == Collector::Air && !self.air_repetition_set {
                    self.air_repetition = penalty;
                    self.air_repetition_set = true;
                }
            }
            let distance_metric = self.collector == Collector::Grind
                || self.collector == Collector::Ground && slot < 3;
            let was_active = self.metric_started[slot];
            self.metric_started[slot] = true;
            if c.announced && !(self.collector == Collector::Air && f.suspend_air) {
                if !distance_metric || was_active {
                    self.held[slot] += f.dt;
                }
                let previous_distance = self.distance[slot];
                let displacement =
                    std::array::from_fn::<_, 3, _>(|i| f.position[i] - self.previous[i]);
                let delta = if self.collector == Collector::Ground && slot == 1 {
                    displacement
                        .iter()
                        .zip(f.forward)
                        .map(|(x, y)| x * y)
                        .sum::<f32>()
                } else {
                    (displacement.iter().map(|x| x * x).sum::<f32>()).sqrt()
                };
                if was_active && delta > f32::from_bits(0x3ba3d70a) {
                    self.distance[slot] += delta;
                }
                let curve = match self.collector {
                    Collector::Air if c.scorable.class == 2 => Some(0x460),
                    Collector::Offboard => Some(0x140),
                    _ => None,
                };
                if let Some(curve) = curve {
                    c.reward += (self.data.collector.curve(curve, self.held[slot]) * f.dt)
                        * c.announcement_threshold;
                }
                let curve = match (self.collector, c.scorable.score_type) {
                    (Collector::Ground, 9) => Some((0x000, 0x050)),
                    (Collector::Ground, 8) => Some((0x0f0, 0x0a0)),
                    (Collector::Ground, _) if slot == 2 => Some((0x230, 0x1e0)),
                    (Collector::Grind, _) => Some((0x2d0, 0x280)),
                    _ => None,
                };
                if let Some((distance, time)) = curve {
                    let delta = (self.data.collector.curve(distance, self.distance[slot])
                        - self.data.collector.curve(distance, previous_distance))
                    .max(0.);
                    self.metric_rewards[slot] = (self
                        .data
                        .collector
                        .curve(time, self.held[slot])
                        .mul_add(f.dt, delta))
                    .mul_add(c.announcement_threshold, self.metric_rewards[slot]);
                }
            }
        }
        Ok(())
    }
    pub fn trick(&self) -> &str {
        &self.trick_name
    }

    pub fn advance(&mut self, f: Frame) -> Result<(), String> {
        self.new_trick = false;
        self.modified_trick = false;
        self.close_tricks = false;
        let descriptor = f
            .descriptor
            .and_then(|name| self.data.by_name(name))
            .filter(|d| !(f.flags & 0x02000000 != 0 && [67, 68, 69].contains(&d.metadata.id)))
            .map(|d| (d.metadata.id, d.metadata.class, d.metadata.score_type));
        let mut next = match f.category {
            FilteredCategory::Ground => Collector::Ground,
            FilteredCategory::Air => Collector::Air,
            FilteredCategory::Grind => Collector::Grind,
            FilteredCategory::Offboard | FilteredCategory::OffboardAir => Collector::Offboard,
            _ => Collector::None,
        };
        if f.state == 600 && matches!(next, Collector::Ground | Collector::Air) {
            next = Collector::Special;
        }
        if next == Collector::Air
            && self.collector == Collector::Ground
            && descriptor.is_some_and(|d| d.1 == 0)
        {
            next = Collector::Ground;
        }
        if next == Collector::Ground
            && self.collector == Collector::Air
            && descriptor.is_some_and(|d| d.1 == 3)
        {
            next = Collector::Air;
        }
        if f.teleported {
            next = Collector::None;
        }
        if next != self.collector {
            let complete = next != Collector::None;
            let previous_type = self
                .carriers
                .iter()
                .flatten()
                .last()
                .map(|c| c.scorable.score_type);
            for slot in 0..4 {
                self.finish(slot, complete, false);
            }
            if self.collector == Collector::Air {
                if complete {
                    for (i, reward) in self.air_metrics.into_iter().enumerate() {
                        if let Some(d) = self.data.by_id(129 + i) {
                            self.session.holder.end_trick(d.metadata, reward);
                        }
                    }
                }
                self.session.holder.finish_collector();
                self.landing_countdown = 2;
            }
            self.collector = next;
            self.start = f.position;
            self.peak = f.position[1];
            self.spin = 0.;
            self.previous_heading = f.forward[0].atan2(f.forward[2]);
            self.air_metrics = [0.; 5];
            self.air_repetition = 1.;
            self.air_repetition_set = false;
            self.air_factor = 1.;
            self.grab_chain = 0;
            self.collector_ticks = 0;
            self.manual_revert_ticks = 0;
            self.revert_id = None;
            if next == Collector::Air {
                self.session.holder.reward_sequence(1.);
                self.landing_countdown = 0;
                if previous_type == Some(8) {
                    self.air_factor *= self.data.collector.scalar(0x68c);
                }
                if previous_type == Some(5) {
                    self.air_factor *= self.data.collector.scalar(0x690);
                }
                if f.velocity[0] * f.velocity[0] + f.velocity[2] * f.velocity[2]
                    < self.data.collector.scalar(0x66c).powi(2)
                {
                    self.air_factor *= self.data.collector.scalar(0x688);
                }
                if f.switch && !f.fakie {
                    self.air_factor *= self.data.collector.scalar(0x684);
                }
                if f.fakie && !f.switch {
                    self.air_factor *= self.data.collector.scalar(0x694);
                }
            }
        }
        let mut ids = [None; 4];
        if !(self.collector == Collector::Air && f.suspend_air) {
            self.collector_ticks = self.collector_ticks.saturating_add(1);
        }
        match self.collector {
            Collector::Ground => {
                ids[0] = if f.flags & 0x80000000 != 0 {
                    Some(0)
                } else if f.flags & 0x40000000 != 0 {
                    Some(1)
                } else {
                    None
                };
                ids[1] = if f.flags & 0x08000000 != 0 {
                    Some(2)
                } else if f.flags & 0x04000000 != 0 {
                    Some(3)
                } else {
                    None
                };
                if ids[1].is_some() && f.reverting {
                    self.manual_revert_ticks += 1;
                    ids[1] = None;
                } else {
                    self.manual_revert_ticks = 0;
                }
                if self.manual_revert_ticks > 6 {
                    ids[1] = None;
                }
                if ids[1].is_some() {
                    if let Some(c) = &self.carriers[1] {
                        ids[1] = Some(c.scorable.id);
                    }
                }
                ids[2] = descriptor
                    .filter(|d| d.0 == 60 && f.flags & 0x02000000 != 0)
                    .map(|d| d.0);
                if f.flags & 0x20000000 != 0 {
                    self.revert_id = Some(4);
                } else if f.flags & 0x10000000 != 0 {
                    self.revert_id = Some(5);
                }
                ids[3] = if f.reverting { self.revert_id } else { None };
            }
            Collector::Air => ids[0] = descriptor.filter(|d| d.2 != 5).map(|d| d.0),
            Collector::Grind => ids[0] = (f.grind_id >= 0).then_some(f.grind_id as usize),
            Collector::Offboard => ids[0] = descriptor.filter(|d| d.2 == 6).map(|d| d.0),
            Collector::Special => ids[0] = descriptor.filter(|d| d.0 == 234).map(|d| d.0),
            Collector::None => {}
        }
        for (slot, id) in ids.into_iter().enumerate() {
            self.carrier(slot, id, &f)?;
        }
        if self.collector == Collector::Air && !f.suspend_air {
            if self.collector_ticks > 5 || f.flags & 0x01000000 != 0 {
                self.sequence_active = true;
            }
            let heading = f.forward[0].atan2(f.forward[2]);
            let delta = (heading - self.previous_heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            self.spin += delta;
            self.previous_heading = heading;
            self.peak = self.peak.max(f.position[1]);
            let dx = f.position[0] - self.start[0];
            let dz = f.position[2] - self.start[2];
            let scale = self.air_factor * self.air_repetition;
            self.air_metrics[0] =
                self.data.collector.curve(0x3c0, (dx * dx + dz * dz).sqrt()) * scale;
            self.air_metrics[1] =
                self.data.collector.curve(0x410, self.peak - self.start[1]) * scale;
            self.air_metrics[2] = self
                .data
                .collector
                .curve(0x320, f.position[1] - self.start[1])
                * scale;
            let turns =
                ((self.spin.to_degrees().abs() + self.data.collector.scalar(0x63c)) / 180.) as i32;
            self.air_metrics[3] = self.data.collector.curve(0x370, (turns * 180) as f32) * scale;
            if f.body_flip
                && self.carriers[0]
                    .as_ref()
                    .is_some_and(|c| c.announced && c.scorable.class == 2)
            {
                self.air_metrics[4] = self.data.collector.scalar(0x640) * scale;
            }
        }
        let active = self.carriers.iter().any(Option::is_some) || self.collector == Collector::Air;
        self.idle_ticks = if active {
            0
        } else {
            self.idle_ticks.saturating_add(1)
        };
        if self.session.holder.has_pending_sequence() {
            if f.landing.landing_data_167 {
                self.clean = f.landing.landing_type_96 == 0;
                self.sketchy = f.landing.landing_type_96 == 1
                    && f.landing.sideways_speed_84 > self.data.sketchy_side_speed
                    && f.landing.spin_92.abs() > 0.5;
            }
            if self.landing_countdown == 0 {
                let mut factor = 1.;
                if f.switch && !f.fakie {
                    factor *= self.data.collector.scalar(0x61c);
                }
                if f.fakie && !f.switch {
                    factor *= self.data.collector.scalar(0x62c);
                }
                if self.collector == Collector::Grind {
                    factor *= self.data.collector.scalar(0x628);
                }
                if self.clean {
                    factor *= self.data.collector.scalar(0x630);
                }
                if self.sketchy {
                    factor *= self.data.collector.scalar(0x620);
                }
                self.session.holder.reward_sequence(factor);
            } else {
                self.landing_countdown -= 1;
            }
        }
        let scales = match self.collector {
            Collector::Air => (0x668, 0x664),
            Collector::Grind => (0x618, 0x614),
            Collector::Offboard => (0x608, 0x604),
            _ => (0x610, 0x60c),
        };
        let line_scale = if active {
            self.data.collector.scalar(scales.0)
        } else {
            1.
        };
        let combo_scale = if active {
            self.data.collector.scalar(scales.1)
        } else {
            1.
        };
        self.session
            .line
            .advance(f.dt, self.data.line_drain, line_scale, active);
        self.session
            .combo
            .timer
            .advance(f.dt, self.data.combo_drain, combo_scale, active);
        let bailout = self.collector == Collector::None && self.sequence_active;
        if self.sequence_active && (bailout || self.idle_ticks >= 3 && self.landing_countdown == 0)
        {
            if bailout {
                self.session.holder.cancel_pending();
            }
            self.sequence_score =
                self.session
                    .publish_sequence(&self.data.session_rules(), 1., bailout, true);
            self.sequence_active = false;
            self.close_tricks = bailout;
        } else if self.sequence_active {
            let s = &self.session.holder.snapshot;
            self.sequence_score = (s.accumulated
                + s.general_pending
                + s.fingerflip_pending
                + self
                    .carriers
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        c.as_ref().map(|c| {
                            if self.collector == Collector::Grind
                                || self.collector == Collector::Ground && i < 3
                            {
                                if c.scorable.score_type == 9 {
                                    c.reward.max(self.metric_rewards[i])
                                } else {
                                    self.metric_rewards[i]
                                }
                            } else {
                                c.reward
                            }
                        })
                    })
                    .sum::<f32>()
                + self.air_metrics.iter().sum::<f32>())
                * self.session.combo.multiplier;
        }
        if self.session.line.expired || f.teleported || bailout {
            self.sequence_score = 0.;
        }
        self.session.settle_line(f.teleported || bailout, active);
        self.previous = f.position;
        Ok(())
    }
}
