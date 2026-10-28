pub(crate) struct GroundTrajectoryState<T> {
    pub pending_request: Option<T>,
    pub primary_valid_288: bool,
    pub secondary_valid_592: bool,
    pub result_valid_9840: bool,
    pub flags_12836: u8,
}
impl<T> GroundTrajectoryState<T> {
    pub fn cancel(&mut self) {
        self.pending_request.take();
        self.result_valid_9840 = false;
        self.primary_valid_288 = false;
        self.secondary_valid_592 = false;
        self.flags_12836 &= 0x3f;
    }
}
