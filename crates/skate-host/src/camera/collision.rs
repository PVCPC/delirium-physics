use skate_core::{
    camera::{DropCollisionProvider, FatLine, FatLineResult, PositionerCollisionProvider},
    physics::board_world::BoardWorld,
};

pub(super) struct CameraCollision<'a> {
    world: &'a BoardWorld,
    results: [FatLineResult; 6],
    pub error: Option<String>,
}

impl DropCollisionProvider for CameraCollision<'_> {
    fn query(
        &mut self,
        lines: [FatLine; 10],
        _context: u32,
    ) -> Result<[FatLineResult; 10], String> {
        let mut output = [no_hit(); 10];
        for (result, line) in output.iter_mut().zip(lines) {
            *result = super::world_query::line(self.world, line.start, line.end, line.radius)?;
        }
        Ok(output)
    }
}

impl<'a> CameraCollision<'a> {
    pub fn new(world: &'a BoardWorld) -> Self {
        Self {
            world,
            results: [no_hit(); 6],
            error: None,
        }
    }
}

impl PositionerCollisionProvider for CameraCollision<'_> {
    fn submit(&mut self, lines: [FatLine; 6], _context: u32) {
        self.results = [no_hit(); 6];
        for (result, line) in self.results.iter_mut().zip(lines) {
            match super::world_query::line(self.world, line.start, line.end, line.radius) {
                Ok(hit) => *result = hit,
                Err(error) => {
                    self.error = Some(error.into());
                }
            }
        }
    }
    fn results(&mut self) -> [FatLineResult; 6] {
        self.results
    }
}

fn no_hit() -> FatLineResult {
    FatLineResult {
        position: [0.0; 4],
        normal: [0.0; 4],
        fraction: 0.0,
        hit: 0,
        surface: 0,
    }
}
