#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub name: String,
    pub points: Vec<[f32; 2]>,
    pub tolerance_squared: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub maximum_misses: u8,
    pub difficulty: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Node {
    active: bool,
    complete: bool,
    next: usize,
    elapsed: u16,
    misses: u8,
    distance: f32,
}

impl Node {
    fn tick(&mut self, pattern: &Pattern, sample: [f32; 2], maximum_misses: u8) {
        if !self.active {
            let distance = distance_squared(pattern.points[0], sample);
            if distance <= pattern.tolerance_squared {
                *self = Self {
                    active: true,
                    next: 1,
                    elapsed: 1,
                    distance,
                    ..Self::default()
                };
            }
        } else if !self.complete {
            let distance = distance_squared(pattern.points[self.next], sample);
            if distance <= pattern.tolerance_squared {
                self.distance += distance;
                self.elapsed = (self.elapsed + 1) & 0x3ff;
                if self.next + 1 == pattern.points.len() {
                    self.complete = true;
                } else {
                    self.next += 1;
                    self.misses = 0;
                }
            } else {
                if self.next != 1
                    || distance_squared(pattern.points[0], sample) > pattern.tolerance_squared
                {
                    self.elapsed = (self.elapsed + 1) & 0x3ff;
                    self.misses = (self.misses + 1) & 0x3f;
                }
                if self.misses > maximum_misses {
                    *self = Self::default();
                }
            }
        }
    }

    fn score(self, count: usize) -> f32 {
        let count = count as f32;
        let mean = (self.distance.max(0.15) / count).min(0.15);
        (count * count * count * count) / (mean * self.elapsed as f32)
    }
}

fn distance_squared(a: [f32; 2], b: [f32; 2]) -> f32 {
    let x = a[0] - b[0];
    let y = a[1] - b[1];
    x * x + y * y
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recognition {
    pub pattern: usize,
    pub strength: f32,
    pub distance: f32,
    pub elapsed: f32,
}

#[derive(Clone, Debug)]
pub struct Recognizer {
    patterns: Vec<Pattern>,
    nodes: Vec<Node>,
    has_previous_sample: bool,
    refractory: bool,
    held: Option<usize>,
}

impl Recognizer {
    pub fn new(patterns: Vec<Pattern>) -> Result<Self, String> {
        for pattern in &patterns {
            if !(2..=15).contains(&pattern.points.len())
                || !pattern.tolerance_squared.is_finite()
                || pattern.tolerance_squared < 0.0
                || pattern.points.iter().flatten().any(|v| !v.is_finite())
            {
                return Err(format!("invalid native gesture pattern {}", pattern.name));
            }
        }
        Ok(Self {
            nodes: vec![Node::default(); patterns.len()],
            patterns,
            has_previous_sample: false,
            refractory: false,
            held: None,
        })
    }

    pub fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }

    pub fn held(&mut self, sample: [f32; 2]) -> Option<usize> {
        if let Some(index) = self.held {
            let pattern = &self.patterns[index];
            if distance_squared(*pattern.points.last().unwrap(), sample) > pattern.tolerance_squared
            {
                self.held = None;
            }
        }
        self.held
    }

    pub fn sample(&mut self, sample: [f32; 2], settings: Settings) -> Option<Recognition> {
        if self.refractory {
            self.nodes.fill(Node::default());
            self.refractory = false;
            return None;
        }
        if !self.has_previous_sample {
            self.has_previous_sample = true;
            return None;
        }
        let mut best: Option<usize> = None;
        for (node, pattern) in self.nodes.iter_mut().zip(&self.patterns) {
            node.tick(pattern, sample, settings.maximum_misses);
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if node.complete
                && best.is_none_or(|old| {
                    node.score(self.patterns[index].points.len())
                        > self.nodes[old].score(self.patterns[old].points.len())
                })
            {
                best = Some(index);
            }
        }
        let pattern = best?;
        let node = self.nodes[pattern];
        let ratio = node.elapsed as f32 / self.patterns[pattern].points.len() as f32;
        let (low, high) = if settings.difficulty == 2 {
            (1.5, 3.0)
        } else {
            (1.75, 4.4)
        };
        let strength = if ratio <= low {
            1.0
        } else if ratio >= high {
            0.0
        } else {
            let slope = 1.0 / (low - high);
            slope.mul_add(ratio, -(slope * high))
        };
        self.refractory = true;
        self.held = Some(pattern);
        Some(Recognition {
            pattern,
            strength,
            distance: node.distance,
            elapsed: node.elapsed as f32,
        })
    }
}

