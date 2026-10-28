use skate_core::player::state::PhysicalStateId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StateCapability {
    pub id: PhysicalStateId,
    pub supported: bool,
    pub has_enter: bool,
    pub has_exit: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StateRegistry;

impl StateRegistry {
    pub(crate) const fn new() -> Self {
        Self
    }

    pub(crate) fn capability(self, id: PhysicalStateId) -> StateCapability {
        let supported = id.is_grind()
            || id == PhysicalStateId::Nonspecific
            || matches!(
                id,
                PhysicalStateId::Sleeping
                    | PhysicalStateId::PhysicsGround
                    | PhysicalStateId::PhysicsAir
                    | PhysicalStateId::FootPlant | PhysicalStateId::Boneless | PhysicalStateId::HandPlant | PhysicalStateId::RevertGround
                    | PhysicalStateId::KnownAir
                    | PhysicalStateId::BipedAir
                    | PhysicalStateId::BipedGround
                    | PhysicalStateId::OffBoardPushing
                    | PhysicalStateId::GroundAnimation
                    | PhysicalStateId::SlideGround
                    | PhysicalStateId::WipeoutGround
                    | PhysicalStateId::Teleporting
                    | PhysicalStateId::LandingOnDeck
            );
        StateCapability {
            id,
            supported,
            has_enter: supported && !matches!(id, PhysicalStateId::Sleeping),
            has_exit: supported,
        }
    }

    pub(crate) fn can_transition(
        self,
        current: PhysicalStateId,
        requested: PhysicalStateId,
    ) -> bool {
        if !self.capability(current).supported || !self.capability(requested).supported {
            return false;
        }
        if current != PhysicalStateId::Sleeping
            && (current.is_grind()
                || requested.is_grind()
                || current == PhysicalStateId::Nonspecific
                || requested == PhysicalStateId::Nonspecific)
        {
            return true;
        }
        matches!(
            (current, requested),
            (PhysicalStateId::Sleeping, PhysicalStateId::PhysicsGround)
                | (
                    PhysicalStateId::PhysicsGround
                        | PhysicalStateId::PhysicsAir
                        | PhysicalStateId::FootPlant | PhysicalStateId::Boneless | PhysicalStateId::HandPlant | PhysicalStateId::RevertGround
                    | PhysicalStateId::KnownAir
                        | PhysicalStateId::BipedAir
                        | PhysicalStateId::BipedGround
                        | PhysicalStateId::OffBoardPushing
                        | PhysicalStateId::GroundAnimation
                        | PhysicalStateId::SlideGround
                        | PhysicalStateId::WipeoutGround
                        | PhysicalStateId::Teleporting
                        | PhysicalStateId::LandingOnDeck,
                    PhysicalStateId::PhysicsGround
                        | PhysicalStateId::PhysicsAir
                        | PhysicalStateId::FootPlant | PhysicalStateId::Boneless | PhysicalStateId::HandPlant | PhysicalStateId::RevertGround
                    | PhysicalStateId::KnownAir
                        | PhysicalStateId::BipedAir
                        | PhysicalStateId::BipedGround
                        | PhysicalStateId::OffBoardPushing
                        | PhysicalStateId::GroundAnimation
                        | PhysicalStateId::SlideGround
                        | PhysicalStateId::WipeoutGround
                        | PhysicalStateId::Teleporting
                        | PhysicalStateId::LandingOnDeck
                )
        )
    }
}

