pub trait PostOutputServices {
    fn finish_player_output_n15(&mut self, player_field_1392: &mut PlayerOutputField1392);

    fn finish_skeleton_output_n16(&mut self);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerOutputField1392;

pub fn run_post_output(
    player_field_1392: &mut PlayerOutputField1392,
    services: &mut impl PostOutputServices,
) {
    services.finish_player_output_n15(player_field_1392);
    services.finish_skeleton_output_n16();
}

