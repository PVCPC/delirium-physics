use skate_core::{
    camera::TrajectoryQuery,
    math::Vector3,
    physics::{
        board_world::BoardWorld,
        skeleton_body::SkeletonBody,
        triangle_query::{TriangleLineHit, triangle_segment},
    },
    player::input_phase::{LineTestFields, PlayerInputState},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SkeletonLineHit {
    pub position: [f32; 4],
    pub normal: [f32; 4],
    pub surface: u32,
    pub hit: bool,
    pub collision_time: f32,
}
impl SkeletonLineHit {
    fn pending_result() -> Self {
        Self {
            position: [0.0; 4],
            normal: [0.0, 1.0, 0.0, 0.0],
            surface: 0,
            hit: false,
            collision_time: -1.0,
        }
    }
}

pub(crate) struct SkeletonLineTests {
    pub hips: SkeletonLineHit,
    pub feet: [SkeletonLineHit; 2],
}
impl SkeletonLineTests {
    pub fn publish(&self, player: &mut PlayerInputState) {
        let fields = |hit: SkeletonLineHit| LineTestFields {
            position: hit.position.map(f32::to_bits),
            normal: hit.normal.map(f32::to_bits),
            surface: hit.surface,
            valid: u8::from(hit.collision_time >= 0.0),
        };
        player.hips_line_test_1488 = fields(self.hips);
        player.left_line_test_1536 = fields(self.feet[0]);
        player.right_line_test_1584 = fields(self.feet[1]);
    }
}

pub(crate) fn query(world: &BoardWorld, body: &SkeletonBody) -> Result<SkeletonLineTests, String> {
    let parts = body.part_transforms();
    let hips = query_trajectory(world, parts[23][3], [0.0, -100.0, 0.0, 0.0])?;
    let raised = [0.0, f32::from_bits(0x3EA8_F5C3), 0.0, 0.0];
    let lowered = [0.0, -1.5, 0.0, 0.0];
    let velocity = std::array::from_fn(|lane| lowered[lane] - raised[lane]);
    let mut feet = [SkeletonLineHit::pending_result(); 2];
    for (foot, part) in [15, 19].into_iter().enumerate() {
        let start = std::array::from_fn(|lane| parts[part][3][lane] + raised[lane]);
        feet[foot] = query_trajectory(world, start, velocity)?;
    }
    Ok(SkeletonLineTests { hips, feet })
}

fn query_trajectory(
    world: &BoardWorld,
    start: [f32; 4],
    velocity: [f32; 4],
) -> Result<SkeletonLineHit, String> {
    let request = TrajectoryQuery {
        position: start,
        velocity,
        gravity: [0.0; 4],
        duration: 1.0,
        radius: f32::from_bits(0x3CF5_C28F),
        start_error: 0.0,
        end_error: 0.0,
    };
    let mut result = SkeletonLineHit::pending_result();
    result.collision_time = request.collision_time(|start, end, radius| {
        let origin = vector(start);
        let direction = Vector3::new(end[0] - start[0], end[1] - start[1], end[2] - start[2]);
        let mut nearest = f32::MAX;
        let mut position = None;
        for (_, triangle) in world.line_candidates(origin, vector(end), radius) {
            let mut geometry = TriangleLineHit {
                position: Vector3::ZERO,
                normal: Vector3::ZERO,
                fraction: 0.0,
                volume_parameter: [0.0; 3],
            };
            if triangle_segment(
                &mut geometry,
                origin,
                direction,
                triangle.triangle.vertices,
                radius,
                0.0,
            ) {
                let lower = if -geometry.fraction >= 0.0 {
                    0.0
                } else {
                    geometry.fraction
                };
                let fraction = if 1.0 - lower >= 0.0 { lower } else { 1.0 };
                if fraction < nearest {
                    nearest = fraction;
                    let point = lanes(geometry.position);
                    result.position = point;
                    result.normal = lanes(triangle.triangle.feature.normal);
                    result.surface = triangle.tag;
                    position = Some(point);
                }
            }
        }
        Ok::<_, String>(position)
    })?;
    result.hit = result.collision_time >= 0.0;
    Ok(result)
}
fn vector(v: [f32; 4]) -> Vector3 {
    Vector3::new(v[0], v[1], v[2])
}
fn lanes(v: Vector3) -> [f32; 4] {
    [v.x, v.y, v.z, 0.0]
}
