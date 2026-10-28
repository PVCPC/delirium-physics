use super::motion_animation::MotionAnimation;
use skate_data::state_graph::attributes::Attributes;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operation {
    HandBusy(Hand),
    MaintainShove,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hand {
    Backside,
    Frontside,
}

impl Operation {
    pub fn parse(attributes: &Attributes<'_>) -> Option<Self> {
        match attributes.text("name")? {
            "HandBusy" => Some(Self::HandBusy(Hand::from_name(
                attributes.text("hand").unwrap_or("bs"),
            ))),
            "MaintainShove" => Some(Self::MaintainShove),
            _ => None,
        }
    }
}

impl Hand {
    fn from_name(name: &str) -> Self {
        if name == "bs" { Self::Backside } else { Self::Frontside }
    }

    fn index(self) -> usize {
        match self {
            Self::Backside => 0,
            Self::Frontside => 1,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct HandServices {
    pub busy_hands: [u32; 2],
    pub keep_shove_channels: bool,
}

impl HandServices {
    pub fn execute(&mut self, operation: Operation, phase: u8, animation: &mut MotionAnimation) {
        if self.apply(operation, phase) && animation.channels.has("SkitchAntic") {
            animation.channels.end("SkitchAntic");
        }
    }

    fn apply(&mut self, operation: Operation, phase: u8) -> bool {
        match (operation, phase) {
            (Operation::HandBusy(hand), 0) => {
                let count = &mut self.busy_hands[hand.index()];
                *count = count.wrapping_add(1);
            }
            (Operation::HandBusy(_), 1) => {
            }
            (Operation::HandBusy(hand), _) => {
                let count = &mut self.busy_hands[hand.index()];
                *count = count.wrapping_sub(1);
            }
            (Operation::MaintainShove, 0) => {
            }
            (Operation::MaintainShove, 1) => self.keep_shove_channels = true,
            (Operation::MaintainShove, _) => {
                self.keep_shove_channels = false;
                return true;
            }
        }
        false
    }
}

