#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PosturePose {
    Stiff,
    Slouch,
    Buff,
}

impl PosturePose {
    pub const fn from_profile(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::Stiff),
            2 => Some(Self::Slouch),
            3 => Some(Self::Buff),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Stiff => "POSTURE_STIFF_POSE",
            Self::Slouch => "POSTURE_SLOUCH_POSE",
            Self::Buff => "POSTURE_BUFF_POSE",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PendingPosture {
    profile: u32,
    pending: bool,
}

impl PendingPosture {
    pub fn set_profile(&mut self, profile: u32) {
        self.profile = profile;
    }

    pub fn profile(&self) -> u32 {
        self.profile
    }

    pub fn set_requested(&mut self, requested: bool) {
        self.pending = requested;
    }

    pub fn is_pending(&self) -> bool {
        self.pending
    }

    pub fn selected_pose(&self) -> Option<PosturePose> {
        if self.pending {
            PosturePose::from_profile(self.profile)
        } else {
            None
        }
    }

    pub fn apply<T, E>(
        &mut self,
        motion: T,
        wrap: impl FnOnce(T, PosturePose) -> Result<T, E>,
    ) -> Result<T, E> {
        let Some(pose) = self.selected_pose() else {
            return Ok(motion);
        };
        let wrapped = wrap(motion, pose)?;
        self.pending = false;
        Ok(wrapped)
    }
}

