use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Player,
    Skeleton,
}

#[derive(Default)]
struct Services(Vec<Event>);

impl PostOutputServices for Services {
    fn finish_player_output_n15(&mut self, _: &mut PlayerOutputField1392) {
        self.0.push(Event::Player);
    }

    fn finish_skeleton_output_n16(&mut self) {
        self.0.push(Event::Skeleton);
    }
}

#[test]
fn preserves_the_complete_native_call_order() {
    let mut field = PlayerOutputField1392;
    let mut services = Services::default();

    run_post_output(&mut field, &mut services);

    assert_eq!(services.0, [Event::Player, Event::Skeleton]);
}
