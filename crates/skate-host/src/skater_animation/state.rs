use skate_core::animation::{
    output::{attributes::AnimationAttribute, physics_packet::SkaterPublicationState},
    skeleton_input::name::encode,
};

pub(super) struct AnimationState {
    pub flags: u32,
    pub publication: SkaterPublicationState,
    pub phase: f32,
}

impl AnimationState {
    pub fn new(local_player: bool) -> Self {
        Self {
            flags: (u32::from(local_player) << 27) | 0x0002_0000,
            phase: 0.0,
            publication: SkaterPublicationState {
                orientation_bit31: false,
                mirrored: false,
                riding_fakie: false,
                weight_on_nose: false,
                natural_stance: 1,
                relative_stance: 0,
                request_bit16: false,
                request_bit15: false,
                air_dismount_revert_requested: false,
                air_dismount_revert_frames: 0,
                signal: None,
            },
        }
    }

    pub fn mirrored(&self) -> bool {
        self.flags & 0x4000_0000 != 0
    }
    pub fn fakie(&self) -> bool {
        self.flags & 0x2000_0000 != 0
    }
    pub fn switch(&self) -> bool {
        self.publication.relative_stance == 1
    }

    pub fn apply_stance_events(&mut self, attributes: &[AnimationAttribute]) {
        let present = |name: &[u8]| attributes.iter().any(|a| a.name == encode(name));
        if present(b"animboardbackward") {
            self.flags ^= 0x8000_0000;
        }
        if present(b"mirrored") {
            self.flags ^= 0x4000_0000;
        }
        if present(b"switch") {
            self.publication.relative_stance = i32::from(self.publication.relative_stance == 0);
        }
    }

    pub fn prepare_publication(&mut self) {
        let p = &mut self.publication;
        p.orientation_bit31 = self.flags & 0x8000_0000 != 0;
        p.mirrored = self.flags & 0x4000_0000 != 0;
        p.riding_fakie = self.flags & 0x2000_0000 != 0;
        p.weight_on_nose = self.flags & 0x1000_0000 != 0;
        p.request_bit16 = self.flags & 0x0001_0000 != 0;
        p.request_bit15 = self.flags & 0x0000_8000 != 0;
        p.air_dismount_revert_requested = self.flags & 0x0010_0000 != 0;
    }

    pub fn finish_publication(&mut self) {
        self.flags &= !(0x0001_0000 | 0x0000_8000 | 0x0010_0000);
    }

    pub fn cull_threshold(&self) -> f32 {
        f32::from_bits(if self.flags & 0x0800_0000 != 0 {
            0x3c23_d70a
        } else {
            0x3dcc_cccd
        })
    }
}
