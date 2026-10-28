use super::ground_query::{self, IndexedEdgeBody, PrimaryEdges};
use skate_core::{
    physics::board_world::BoardWorld,
    player::offboard::ground_query::{EdgeSearch, GroundQueryPacket, GroundQueryScene, LineHit},
};

pub(crate) struct SceneService<'a> {
    pub world: &'a BoardWorld,
}

impl GroundQueryScene for SceneService<'_> {
    type Error = &'static str;

    fn edge_candidates(
        &mut self,
        search: &EdgeSearch,
    ) -> Result<Vec<skate_core::player::offboard::ground_query::Edge>, Self::Error> {
        ground_query::with_world_scene(
            self.world,
            PrimaryEdges::Normal {
                dynamic: &[],
                vehicles: &[],
            },
            &[] as &[IndexedEdgeBody<'_>],
            |scene| scene.edge_candidates(search),
        )
    }

    fn query_lines(
        &mut self,
        packet: &GroundQueryPacket,
    ) -> Result<[Option<LineHit>; 7], Self::Error> {
        ground_query::with_world_scene(
            self.world,
            PrimaryEdges::Normal {
                dynamic: &[],
                vehicles: &[],
            },
            &[] as &[IndexedEdgeBody<'_>],
            |scene| scene.query_lines(packet),
        )
    }
}
