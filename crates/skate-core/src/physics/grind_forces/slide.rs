use super::{V, dot3, scale, sub};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slide { Boardslide, Darkslide }

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub position: V,
    pub point: V,
    pub direction: V,
    pub across: V,
    pub velocity: V,
    pub translation_2796: f32,
    pub total_mass_2660: f32,
    pub update_frequency: f32,
    pub preparing_jump: bool,
    pub geometry_kind: u32,
}

pub fn control(slide: Slide, input: Input) -> Vec<V> {
    let Input { position, point, direction, across, velocity,
        translation_2796: translation, total_mass_2660: mass,
        update_frequency, preparing_jump, geometry_kind } = input;
    let offset = dot3(sub(position, point), across);
    let inward = scale(across, if offset > 0.0 { -1.0 } else { 1.0 });
    if preparing_jump {
        return if offset.abs() > 0.12 { vec![scale(inward, 201.0)] } else { Vec::new() };
    }
    let translation_strength = match slide {
        Slide::Boardslide => 25.0,
        Slide::Darkslide => (1.0 - offset.abs() * 6.25) * 40.0,
    };
    let translate = scale(across, translation * translation_strength);
    if geometry_kind == 2 {
        return vec![translate];
    }
    let perpendicular = sub(velocity, scale(direction, dot3(velocity, direction)));
    if offset.abs() > 0.16 {
        let mut forces = vec![scale(inward, 10.0)];
        if dot3(inward, perpendicular) < 0.0 {
            let mut stop = scale(perpendicular, -(mass * update_frequency));
            stop[1] = 0.0;
            forces.push(stop);
        }
        forces
    } else {
        let mut damping = scale(perpendicular, -20.0);
        damping[1] = 0.0;
        vec![translate, damping]
    }
}
