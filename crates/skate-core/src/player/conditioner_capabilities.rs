#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionerCapabilityContext {
    pub in_front_end: bool,
    pub hall_of_meat_enabled: bool,
    pub challenge_query_active: bool,
    pub challenge_configuration_enabled: bool,
}

impl ConditionerCapabilityContext {
    pub fn capabilities(self) -> u32 {
        if self.in_front_end {
            0
        } else if self.challenge_query_active && self.challenge_configuration_enabled {
            0xffff_fff7
        } else if self.hall_of_meat_enabled && !self.challenge_query_active {
            0xffff_f7f7
        } else {
            0x0000_01c0
        }
    }
}

