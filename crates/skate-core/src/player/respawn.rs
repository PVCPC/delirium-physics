use crate::physics::skeleton_animation_record::{AnimationPartTransform as Matrix, IDENTITY};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub transform: Matrix,
    pub stance: u32,
    pub offboard: bool,
    pub score: f32,
}

pub trait Validation {
    type Error;
    fn ground(&mut self, transform: &Matrix) -> Result<Option<Ground>, Self::Error>;
    fn location(&mut self, transform: &Matrix) -> Result<bool, Self::Error>;
    fn occupants(&mut self, transform: &Matrix) -> Result<bool, Self::Error>;
    fn edges(&mut self, transform: &Matrix) -> Result<bool, Self::Error>;
}

#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub position: [f32; 4],
    pub offboard: bool,
}

pub struct Observation {
    pub measurements: i32,
    pub root_position: [f32; 4],
    pub com_position: [f32; 4],
    pub teleport_requested: bool,
    pub physical_state: u32,
    pub state_frames: u32,
    pub ground_suppressed: bool,
    pub offboard_correction: bool,
    pub ground_category: u32,
    pub foot_categories: [u32; 2],
    pub riding_transform: Matrix,
    pub offboard_transform: Matrix,
    pub stance: u32,
    pub alternate_world: bool,
}

pub struct History {
    initial: Candidate,
    entries: Vec<Candidate>,
    cooldown: i32,
    pub current_orientation: Matrix,
    pub current_position: [f32; 4],
}
impl History {
    pub fn new(initial: Candidate) -> Self {
        Self {
            initial,
            entries: vec![initial],
            cooldown: 20,
            current_orientation: IDENTITY,
            current_position: initial.transform[3],
        }
    }

    pub fn observe<V: Validation>(
        &mut self,
        input: &Observation,
        minimum_state_frames: u32,
        scene: &mut V,
    ) -> Result<(), V::Error> {
        self.current_position = input.com_position;
        if !self.recording_due(input.measurements, input.root_position) || input.teleport_requested
        {
            return Ok(());
        }
        let (transform, offboard, score) = match input.physical_state {
            100 | 104 => {
                self.current_orientation = input.riding_transform;
                if input.state_frames <= minimum_state_frames
                    || input.ground_suppressed
                    || !surface_allowed(input.ground_category)
                {
                    return Ok(());
                }
                (
                    input.riding_transform,
                    input.ground_category == 8,
                    surface_score(input.ground_category),
                )
            }
            500 | 502 => {
                if input.state_frames <= minimum_state_frames || input.offboard_correction {
                    return Ok(());
                }
                (
                    input.offboard_transform,
                    true,
                    surface_score(input.foot_categories[0])
                        .max(surface_score(input.foot_categories[1])),
                )
            }
            _ => return Ok(()),
        };
        if !scene.location(&transform)? || !scene.edges(&transform)? {
            return Ok(());
        }
        if !input.alternate_world && scene.ground(&transform)?.is_none() {
            return Ok(());
        }
        self.insert(Candidate {
            transform,
            stance: input.stance,
            offboard,
            score,
        });
        Ok(())
    }

    pub fn recording_due(&mut self, measurements: i32, root_position: [f32; 4]) -> bool {
        let mut eligible = measurements >= 120;
        if !eligible {
            self.cooldown = 0;
        }
        if self.cooldown > 0 {
            self.cooldown -= 1;
            eligible = false;
        }
        eligible
            && !self
                .entries
                .iter()
                .rev()
                .any(|entry| distance_squared(entry.transform[3], root_position) < 2.25)
    }

    pub fn insert(&mut self, candidate: Candidate) {
        if self.entries.len() == 32 {
            self.entries.remove(0);
        }
        self.entries.push(candidate);
        self.cooldown = 20;
    }

    fn pop_best(&mut self) -> Option<Candidate> {
        let mut best = None;
        let mut score = -99990.0;
        for (age, index) in (0..self.entries.len()).rev().enumerate() {
            let mut value = self.entries[index].score;
            if age < 1 {
                value -= 100.0;
            }
            if age > 5 {
                value -= 200.0;
            }
            if value > score {
                best = Some(index);
                score = value;
            }
        }
        best.map(|index| self.entries.remove(index))
    }

    pub fn automatic<V: Validation>(
        &mut self,
        stance: u32,
        scene: &mut V,
    ) -> Result<Candidate, V::Error> {
        let mut transform = self.current_orientation;
        transform[3] = self.current_position;
        if let Some(ground) = scene.ground(&transform)? {
            if scene.location(&transform)?
                && scene.occupants(&transform)?
                && scene.edges(&transform)?
            {
                if ground.offboard {
                    transform[3][1] = ground.position[1];
                }
                return Ok(Candidate {
                    transform,
                    stance,
                    offboard: ground.offboard,
                    score: 0.0,
                });
            }
        }
        while let Some(candidate) = self.pop_best() {
            if scene.location(&candidate.transform)? && scene.occupants(&candidate.transform)? {
                return Ok(candidate);
            }
        }
        Ok(self.entries.last().copied().unwrap_or(Candidate {
            transform: self.initial.transform,
            stance,
            offboard: false,
            score: 0.0,
        }))
    }
}

pub fn surface_allowed(category: u32) -> bool {
    !matches!(category, 5 | 6 | 9 | 12 | 13)
}

pub fn surface_score(category: u32) -> f32 {
    match category {
        1 | 2 => 100.0,
        3 | 4 => 50.0,
        11 => 20.0,
        _ => 0.0,
    }
}

fn distance_squared(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

