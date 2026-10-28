pub mod facing;

#[derive(Clone, Copy, Debug)]
pub struct FadeSettings {
    pub response: f32,
    pub input_scale: f32,
    pub acceleration: f32,
    pub maximum_step: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Fade {
    pub just_began: bool,
    pub elapsed: f32,
    pub value: f32,
    pub target: f32,
    pub step: f32,
    pub minimum: f32,
    pub maximum: f32,
}
impl Fade {
    pub fn begin(&mut self, minimum: f32, maximum: f32, twist: f32) -> f32 {
        let value = bound(twist, minimum, maximum);
        *self = Self {
            just_began: true,
            minimum,
            maximum,
            value,
            target: value,
            ..Self::default()
        };
        value
    }
    pub fn update(&mut self, dt: f32, intent: f32, settings: FadeSettings) -> Option<f32> {
        if self.just_began {
            self.just_began = false;
            return None;
        }
        self.elapsed += dt;
        self.target = bound(
            settings.input_scale.mul_add(intent, self.target),
            self.minimum,
            self.maximum,
        );
        let desired =
            (1.0 - settings.response).mul_add(self.value, self.target * settings.response);
        let step = bound(
            desired - self.value,
            self.step - settings.acceleration,
            self.step + settings.acceleration,
        );
        self.step = bound(step, -settings.maximum_step, settings.maximum_step);
        self.value = bound(self.value + self.step, self.minimum, self.maximum);
        Some(self.value)
    }
}

pub fn crouch(
    previous: f32,
    intent: f32,
    grind_crouch: f32,
    minimum: f32,
    maximum: f32,
    rate: f32,
    dt: f32,
) -> f32 {
    let input = bound(1.0 - intent, minimum, maximum);
    let physical = bound(1.0 - grind_crouch, minimum, maximum);
    let desired = if input - physical >= 0.0 {
        physical
    } else {
        input
    };
    let step = dt * rate;
    bound(desired, previous - step, previous + step)
}

pub fn mirrored_twist(twist: f32, mirrored: bool) -> f32 {
    if !mirrored {
        return twist;
    }
    if twist >= 0.0 {
        std::f32::consts::PI - twist
    } else {
        -std::f32::consts::PI - twist
    }
}

fn bound(value: f32, minimum: f32, maximum: f32) -> f32 {
    let value = if minimum - value >= 0.0 {
        minimum
    } else {
        value
    };
    if maximum - value >= 0.0 {
        value
    } else {
        maximum
    }
}
