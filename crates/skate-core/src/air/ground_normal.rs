use crate::physics::reciprocal_sqrt::estimate;

#[derive(Clone, Debug, PartialEq)]
pub struct GroundNormalFilter {
    words: [u32; 24],
}

impl GroundNormalFilter {
    pub fn initialized(control: [f32; 4], initial: [f32; 4]) -> Self {
        let mut words = [0; 24];
        words[..4].copy_from_slice(&control.map(f32::to_bits));
        words[4..8].copy_from_slice(&initial.map(f32::to_bits));
        words[8..12].copy_from_slice(&initial.map(f32::to_bits));
        Self { words }
    }

    pub fn publish_current(&mut self, current: [f32; 4]) {
        self.words[4..8].copy_from_slice(&current.map(f32::to_bits));
    }

    pub fn filter_raw(&mut self, control: [f32; 4], input: [f32; 4]) -> [f32; 4] {
        self.words[..4].copy_from_slice(&control.map(f32::to_bits));
        self.filter(input)
    }

    pub fn from_words(words: [u32; 24]) -> Self {
        Self { words }
    }

    pub fn words(&self) -> &[u32; 24] {
        &self.words
    }

    pub fn update(&mut self, control: [f32; 4], input: [f32; 4]) -> [f32; 4] {
        self.words[..4].copy_from_slice(&control.map(f32::to_bits));
        let filtered = self.filter(input);
        let squared = dot3(filtered);
        let mut inverse = estimate(squared);
        for _ in 0..2 {
            let correction = (-squared).mul_add(inverse * inverse, 1.0);
            inverse = (inverse * 0.5).mul_add(correction, inverse);
        }
        let normal = filtered.map(|v| v * inverse);
        self.words[4..8].copy_from_slice(&normal.map(f32::to_bits));
        normal
    }

    pub(crate) fn filter(&mut self, input: [f32; 4]) -> [f32; 4] {
        let previous = self.words.map(f32::from_bits);
        let blend = previous[3];
        let retained = 1.0 - blend;
        let mut output = [0.0; 4];
        for lane in 0..4 {
            let error = input[lane] - previous[4 + lane];
            let error_blend = previous[12 + lane].mul_add(retained, error * blend);
            let input_delta = input[lane] - previous[8 + lane];
            let input_blend = previous[20 + lane].mul_add(retained, input_delta * blend);
            let predicted = error_blend.mul_add(previous[1], previous[4 + lane]);
            let predicted = error.mul_add(previous[0], predicted);
            output[lane] = (input_blend - previous[16 + lane]).mul_add(previous[2], predicted);
            self.words[12 + lane] = error_blend.to_bits();
            self.words[20 + lane] = input_blend.to_bits();
            self.words[16 + lane] = (output[lane] - previous[4 + lane]).to_bits();
            self.words[4 + lane] = output[lane].to_bits();
            self.words[8 + lane] = input[lane].to_bits();
        }
        output
    }
}

fn dot3(value: [f32; 4]) -> f32 {
    crate::physics::native_arithmetic::dot3(value, value)
}
