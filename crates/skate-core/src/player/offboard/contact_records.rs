use super::contact_queries::{Input, V};
use crate::physics::native_arithmetic::dot3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub position: V,
    pub normal: V,
    pub coordinates: V,
    pub flags: u32,
    pub distance: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source { Probe, Support, Intersection }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction { None, Forward, Reverse }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location { Surface(usize), Obstacle(usize) }

#[derive(Default)]
pub struct Records {
    pub surface: Vec<Record>,
    pub obstacle: Vec<Record>,
    pub inserted_surface_count: usize,
}

impl Records {
    pub fn clear(&mut self) {
        self.surface.clear(); self.obstacle.clear(); self.inserted_surface_count = 0;
    }

    pub fn insert(&mut self, input: Input, position: V, normal: &mut V,
        source: Source, direction: Direction, requested_distance: f32) -> Option<Location>
    {
        let delta = std::array::from_fn(|i| position[i] - input.position[i]);
        let forward = dot3(input.surface_forward, delta);
        let height = dot3(input.surface_up, delta);
        let coordinates = [forward, height, forward, forward];
        let elevated = height >= f32::from_bits(0x3f4a_3d71) && forward >= 0.;
        let lateral = dot3(*normal, input.surface_right);
        *normal = std::array::from_fn(|i| normal[i] - input.surface_right[i] * lateral);
        let square = dot3(*normal, *normal);
        let mut inverse = crate::physics::reciprocal_sqrt::estimate(square);
        for _ in 0..2 {
            inverse = (inverse * 0.5).mul_add((-square).mul_add(inverse * inverse, 1.), inverse);
        }
        let length = if square == 0. { 0. } else { square * inverse };
        if !(length >= f32::from_bits(0x3a83_126f)) {
            if !(0. > height) {
                *normal = input.surface_forward.map(|x| f32::from_bits(x.to_bits() ^ 0x8000_0000));
            } else { return None; }
        } else {
            let mut reciprocal = crate::physics::native_arithmetic::reciprocal_estimate(length);
            for _ in 0..2 {
                reciprocal = reciprocal.mul_add((-reciprocal).mul_add(length, 1.), reciprocal);
            }
            *normal = normal.map(|x| x * reciprocal);
        }
        let surface = source == Source::Support || elevated;
        if surface {
            if self.surface.len() == 64 { return None; }
        } else if forward < 0. { return None; }
        let flags = match direction { Direction::None => 0, Direction::Forward => 1, Direction::Reverse => 4 }
            | if elevated { 8 } else { 0 }
            | if source == Source::Intersection { 16 } else { 0 };
        let record = Record { position, normal: *normal, coordinates, flags,
            distance: if requested_distance >= 0. { requested_distance } else { forward } };
        Some(if surface {
            let index = self.surface.len(); self.surface.push(record);
            self.inserted_surface_count = self.surface.len(); Location::Surface(index)
        } else {
            let index = self.obstacle.len(); self.obstacle.push(record); Location::Obstacle(index)
        })
    }
}

