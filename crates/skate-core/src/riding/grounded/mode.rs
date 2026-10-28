#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessedMode<T> {
    pub requested_mode: u32,
    pub selected_attributes: T,
}

impl<T: Copy> ProcessedMode<T> {
    pub fn update(&mut self, requested_mode: u32, attributes: &[T; 5]) {
        if requested_mode <= 4 {
            self.selected_attributes = attributes[requested_mode as usize];
        }
        self.requested_mode = requested_mode;
    }
}
