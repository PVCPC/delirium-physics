use super::data::GroundContactHistory;

pub type Vector4 = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundContactStateInput {
    pub flags_2476: u32,
    pub flags_2480: u32,
    pub velocity_400: Vector4,
    pub axis_464: Vector4,
    pub contact_position_560: Vector4,
    pub trajectory_position_592: Vector4,
    pub reckoning_vector_1200: Vector4,
    pub differing_contact_frame_limit: i32,
}

pub trait GroundContactServices {
    type Error;
    type Handle: Eq;

    fn stop_contact_tracking_n45(&mut self) -> Result<(), Self::Error>;
    fn clear_inactive_contact_output_3024(&mut self) -> Result<(), Self::Error>;
    fn initial_contact_handle_1920(&mut self) -> Result<Option<Self::Handle>, Self::Error>;
    fn identify_contact_n46(
        &mut self,
        frame: &GroundContactStateInput,
    ) -> Result<Option<Self::Handle>, Self::Error>;
    fn publish_contact_n47(
        &mut self,
        frame: &GroundContactStateInput,
        retained: Option<&Self::Handle>,
    ) -> Result<(), Self::Error>;
}

pub fn update<S: GroundContactServices>(
    history: &mut GroundContactHistory<S::Handle>,
    input: GroundContactStateInput,
    services: &mut S,
) -> Result<(), S::Error> {
    if input.flags_2476 & 0x0040_0000 == 0 {
        services.stop_contact_tracking_n45()?;
        services.clear_inactive_contact_output_3024()?;
        return Ok(());
    }

    if input.flags_2480 & 0x2000_0000 != 0 || input.flags_2480 & 0x1000_0000 == 0 {
        history.tracked_contact_2760 = None;
        history.differing_contact_frames_2756 = 0;
    }

    if input.flags_2480 & 0x1000_0000 != 0 {
        if input.axis_464[1] < f32::from_bits(0x3f73_3333) {
            if history.tracked_contact_2760.is_none() {
                history.tracked_contact_2760 = services.initial_contact_handle_1920()?;
            }
            let current = services.identify_contact_n46(&input)?;
            history.differing_contact_frames_2756 = if current == history.tracked_contact_2760 {
                0
            } else {
                history.differing_contact_frames_2756.wrapping_add(1)
            };
            if history.differing_contact_frames_2756 > input.differing_contact_frame_limit {
                history.tracked_contact_2760 = None;
            }
        } else {
            history.tracked_contact_2760 = None;
        }
    }

    services.publish_contact_n47(&input, history.tracked_contact_2760.as_ref())
}

