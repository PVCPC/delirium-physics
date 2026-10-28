use skate_core::{math::Vector3, player::input_phase::ProcessedPhysicsInput};

pub(super) struct GroundPacketInputs {
    pub wheel_normal: Vector3,
    pub dynamic_up: Vector3,
    pub speed: f32,
    pub absolute_speed: f32,
    pub wheel_count: i32,
}

impl GroundPacketInputs {
    pub fn from_processed(input: &ProcessedPhysicsInput) -> Self {
        let vector = |bits: [u32; 4]| {
            let value = bits.map(f32::from_bits);
            Vector3::new(value[0], value[1], value[2])
        };
        Self {
            wheel_normal: vector(input.vectors_464_480_496_512_528[0]),
            dynamic_up: vector(input.vectors_464_480_496_512_528[4]),
            speed: input.scalar_2652,
            absolute_speed: input.scalar_2616,
            wheel_count: input.wheel_count_2556 as i32,
        }
    }
}

