use crate::input::graph_intents::apply_filter;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub starting_value: f32,
    pub default_value: f32,
    pub scale: f32,
    pub filters: [u32; 4],
    pub ramp_time: Option<f32>,
    pub blend_rising: f32,
    pub blend_falling: f32,
    pub blend_out: Option<f32>,
    pub clamp_velocity: Option<f32>,
    pub clamp_acceleration: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    pub elapsed: f32,
    pub previous_delta: f32,
    pub value: f32,
}

impl State {
    pub fn begin(&mut self, settings: &Settings) -> f32 {
        self.elapsed = 0.0;
        self.previous_delta = 0.0;
        self.value = settings.starting_value;
        self.value
    }

    pub fn update(
        &mut self,
        settings: &Settings,
        input: Option<f32>,
        dt: f32,
        stance: (bool, bool),
    ) -> f32 {
        self.elapsed += dt;
        let mut target = settings.scale * input.unwrap_or(settings.default_value);
        for (kind, enabled) in settings.filters.into_iter().zip([true, true, stance.0, stance.1]) {
            if enabled && kind <= 7 {
                target = apply_filter(target, kind);
            }
        }
        let mut blend = if target < self.value {
            settings.blend_falling
        } else {
            settings.blend_rising
        };
        if input.is_some() {
            if let Some(time) = settings.ramp_time {
                blend = ramp(self.elapsed, time) * blend;
            }
        } else if let Some(out) = settings.blend_out {
            blend = out;
        }
        let weighted_target = blend * target;
        let candidate = (1.0 - blend).mul_add(self.value, weighted_target);
        let mut delta = candidate - self.value;
        if let Some(limit) = settings.clamp_acceleration {
            delta = limit_delta(delta, self.previous_delta - limit, self.previous_delta + limit);
        }
        if let Some(limit) = settings.clamp_velocity {
            delta = limit_delta(delta, -limit, limit);
        }
        self.previous_delta = delta;
        self.value = delta + self.value;
        self.value
    }
}

fn limit_delta(value: f32, lower: f32, upper: f32) -> f32 {
    let selected = if lower - value >= 0.0 { lower } else { value };
    if upper - selected >= 0.0 { selected } else { upper }
}

fn ramp(elapsed: f32, time: f32) -> f32 {
    let keys = [0.0f32, time];
    let mut begin = 0usize;
    let mut count = 2usize;
    while count != 0 {
        let step = count / 2;
        let middle = begin + step;
        if (keys[middle].to_bits() as i32) < (elapsed.to_bits() as i32) {
            begin = middle + 1;
            count -= step + 1;
        } else {
            count = step;
        }
    }
    match begin {
        0 => 0.0,
        2 => 1.0,
        _ => ((1.0f32 - 0.0) / (time - 0.0)).mul_add(elapsed - 0.0, 0.0),
    }
}

