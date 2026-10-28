pub type Vector = [f32; 4];

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub desired_height: f32,
    pub board_position: Vector,
    pub centre_of_mass_position: Vector,
    pub reference_up: Vector,
    pub current_velocity: Vector,
}

pub fn calculate(input: Input) -> Vector {
    let up = input.reference_up;
    let displacement = sub(input.centre_of_mass_position, input.board_position);
    let current_height = dot3(up, displacement);
    let vertical_velocity = dot3(up, input.current_velocity);
    let planar_velocity = sub(input.current_velocity, scale(up, vertical_velocity));
    let launch_speed = ((input.desired_height - current_height) * 19.6).sqrt();
    add(planar_velocity, scale(up, launch_speed))
}

fn dot3(a: Vector, b: Vector) -> f32 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}
fn add(a: Vector, b: Vector) -> Vector {
    core::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: Vector, b: Vector) -> Vector {
    core::array::from_fn(|i| a[i] - b[i])
}
fn scale(v: Vector, scalar: f32) -> Vector {
    core::array::from_fn(|i| v[i] * scalar)
}

