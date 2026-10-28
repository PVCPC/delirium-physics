use super::{
    ScoreHolder,
    timer::{ComboTimer, PointTimer},
};

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub combo_capacity: f32,
    pub combo_levels: [(f32, f32); 3],
    pub combo_refresh_threshold: f32,
    pub line_capacity: f32,
    pub bail_factor: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Session {
    pub holder: ScoreHolder,
    pub combo: ComboTimer,
    pub line: PointTimer,
}

impl Session {
    pub fn publish_sequence(
        &mut self,
        rules: &Rules,
        landing_factor: f32,
        penalized: bool,
        multiplier_enabled: bool,
    ) -> f32 {
        self.holder.reward_sequence(landing_factor);
        let snapshot = &self.holder.snapshot;
        let mut raw =
            (snapshot.fingerflip_pending + snapshot.general_pending) + snapshot.accumulated;
        if penalized && rules.bail_factor < 1.0 {
            raw *= rules.bail_factor;
        }
        let multiplier = if multiplier_enabled {
            self.combo.multiplier
        } else {
            1.0
        };
        if multiplier_enabled {
            self.combo.credit(
                raw,
                rules.combo_capacity,
                rules.combo_levels,
                rules.combo_refresh_threshold,
            );
            if self.combo.multiplier > f32::from_bits(0x3f8147ae) {
                self.line.credit(raw, rules.line_capacity);
            }
        }
        let reward = multiplier * raw;
        self.holder
            .publish(reward, multiplier_enabled && self.line.points > 0.0);
        reward
    }

    pub fn settle_line(&mut self, reset_requested: bool, collector_active: bool) {
        if reset_requested || self.line.expired {
            self.line = PointTimer::default();
            self.combo.multiplier = 1.0;
            self.holder.finish_line();
        } else if self.line.points <= 0.0 {
            self.holder.bank_line(!collector_active);
        }
    }
}

