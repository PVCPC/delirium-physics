use super::motion::MotionHost;
use skate_core::graph::conditions::NumericCondition;
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    CanLandOnBoard,
    DistToEdge(NumericCondition),
    IsDeckFree,
    IsBipedCommittedToMotion,
    EnoughDistToObstacle { database: String, animation: String },
    CanBipedLand,
    IsCrouchedEnoughForBlendToGrabCycle,
    TrucksOrDeckInContact,
    PhysicsWantsManualExit,
    ApexReached,
}
impl Condition {
    pub fn recognizes(name: &str) -> bool {
        matches!(name, "CanLandOnBoard" | "DistToEdge" | "IsDeckFree" |
            "IsBipedCommittedToMotion" | "EnoughDistToObstacle" | "CanBipedLand" |
            "IsCrouchedEnoughForBlendToGrabCycle" | "TrucksOrDeckInContact" |
            "PhysicsWantsManualExit" | "ApexReached")
    }
    pub fn parse(a: &Attributes<'_>) -> Self {
        match a.text("name").unwrap_or("") {
            "CanLandOnBoard" => Self::CanLandOnBoard,
            "DistToEdge" => Self::DistToEdge(super::condition_nodes::numeric(a)),
            "IsDeckFree" => Self::IsDeckFree,
            "IsBipedCommittedToMotion" => Self::IsBipedCommittedToMotion,
            "EnoughDistToObstacle" => Self::EnoughDistToObstacle {
                database: a.text("db").unwrap_or("").to_owned(),
                animation: a.text("anim").unwrap_or("").to_owned(),
            },
            "CanBipedLand" => Self::CanBipedLand,
            "IsCrouchedEnoughForBlendToGrabCycle" => Self::IsCrouchedEnoughForBlendToGrabCycle,
            "TrucksOrDeckInContact" => Self::TrucksOrDeckInContact,
            "PhysicsWantsManualExit" => Self::PhysicsWantsManualExit,
            "ApexReached" => Self::ApexReached,
            _ => unreachable!(),
        }
    }
    pub fn evaluate(&self, host: &MotionHost) -> Result<bool, String> {
        let p = host.gameplay_conditions.as_ref().ok_or("stock gameplay condition requires physical publication")?;
        Ok(match self {
            Self::CanLandOnBoard => p.can_land_on_board,
            Self::CanBipedLand => p.offboard_landing_normal[1] > 0.85,
            Self::IsDeckFree => host.toggle_board_physical.ok_or("IsDeckFree requires completed OffBoard312")?.free_board,
            Self::IsBipedCommittedToMotion => p.offboard_committed_to_motion,
            Self::TrucksOrDeckInContact => p.trucks_or_deck_contact,
            Self::ApexReached => p.reached_apex,
            Self::DistToEdge(numeric) => numeric.matches(p.offboard_edge_distance),
            Self::EnoughDistToObstacle { database, animation } => {
                enough_distance(p.offboard_obstacle_distance, host.animation.stock_clip_translation_z(database, animation)?)
            }
            Self::IsCrouchedEnoughForBlendToGrabCycle => host.crouching_physical
                .ok_or("Crouch condition requires physical animation height")?.animation_height_72 < f32::from_bits(0x3f19_999a),
            Self::PhysicsWantsManualExit => host.manual_exit.ok_or("PhysicsWantsManualExit requires completed physical animation output")?,
        })
    }
}

fn enough_distance(distance: f32, translation: f32) -> bool {
    !(distance < translation + f32::from_bits(0x3e99_999a))
}

