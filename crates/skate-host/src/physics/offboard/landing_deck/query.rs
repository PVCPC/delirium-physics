use super::{Owner, input};
use crate::physics::{offboard::contact_toolkit::StaticScene, player_input::PlayerInputRuntime};
use skate_core::player::{
    input_phase::ProcessedPhysicsInput,
    offboard::{
        hippy_jump,
        landing_deck::{QueryRequest, QueryResult, SyncInput},
    },
};

pub(super) fn execute(
    scene: &StaticScene<'_>,
    request: QueryRequest,
    processed: &ProcessedPhysicsInput,
) -> Result<QueryResult, String> {
    scene
        .trajectory(request, processed.actor_query_2952 as i32, 0)
        .map_err(str::to_owned)
}

impl Owner {
    pub(crate) fn submit(
        &mut self,
        scene: &StaticScene<'_>,
        request: QueryRequest,
        processed: &ProcessedPhysicsInput,
    ) -> Result<(), String> {
        let completion = execute(scene, request, processed)?;
        self.completion = Some(completion);
        self.manager.query_submitted();
        Ok(())
    }

    pub(crate) fn post_physics(&mut self, input: &PlayerInputRuntime) -> Result<(), String> {
        if !self.manager.pending_262 {
            if self.completion.is_some() {
                return Err("Landing query completion exists without native pending262".into());
            }
            return Ok(());
        }
        let completion = self
            .completion
            .ok_or("Landing pending262 has no submitted world-query completion")?;
        let p = &input.processed;
        self.manager.sync(
            completion.valid().then_some(completion),
            &SyncInput {
                position_592: input::position(input),
                flags_2488: p.flags_2488,
            },
            |_| {
                let processed = input::processed(input)?;
                Ok::<_, String>(hippy_jump::calculate(hippy_jump::Input {
                    desired_height: f32::from_bits(p.vectors_880_896_912_928_944[3][1])
                        + f32::from_bits(0x3e4c_cccd),
                    board_position: processed.board_position_112,
                    centre_of_mass_position: input::position(input),
                    reference_up: processed.up_544,
                    current_velocity: processed.board_velocity_400,
                }))
            },
        )?;
        self.completion = None;
        Ok(())
    }
}
