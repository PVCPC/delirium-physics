use skate_core::physics::grind_contact::{Primitive, families::Contact};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AcquiredContact {
    pub contact: Contact,
    pub primitive: Primitive,
}
