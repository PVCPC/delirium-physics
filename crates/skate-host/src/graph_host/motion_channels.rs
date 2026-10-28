use skate_core::animation::{
    channel_playback::{ChannelPlayback, ChannelSettings},
    clip_clock::AdvanceResult,
    output::attributes::{AnimationAttribute, AttributeName},
    playback_parameters::{SettableAttribute, intent_key},
    playback_tree::{Evaluation, PlaybackTree, PoseCommand},
};

struct Channel {
    name: String,
    tree: PlaybackTree,
    playback: ChannelPlayback,
}
#[derive(Default)]
pub struct MotionChannels {
    channels: Vec<Channel>,
}
impl MotionChannels {
    pub fn reset_from_stock(&mut self) {
        self.channels.clear();
    }
    pub(super) fn prepare_trees(
        &mut self,
        mut prepare: impl FnMut(&mut PlaybackTree) -> Result<(), String>,
    ) -> Result<(), String> {
        for channel in &mut self.channels {
            prepare(&mut channel.tree)?;
        }
        Ok(())
    }
    pub fn has(&self, name: &str) -> bool {
        self.channels
            .iter()
            .any(|c| intent_key(&c.name) == intent_key(name))
    }
    pub fn remaining(&self, name: &str) -> f32 {
        self.channels
            .iter()
            .find(|c| intent_key(&c.name) == intent_key(name))
            .map_or(0.0, |c| {
                let value = c.tree.length() - c.tree.time();
                if value >= 0.0 { value } else { 0.0 }
            })
    }
    pub fn elapsed(&self, name: &str) -> f32 {
        self.channels.iter()
            .find(|c| intent_key(&c.name) == intent_key(name))
            .map_or(0.0, |c| {
                let time = c.tree.time();
                let lower = if -time >= 0.0 { 0.0 } else { time };
                let length = c.tree.length();
                if length - lower >= 0.0 { lower } else { length }
            })
    }
    pub fn in_transition(&self, name: &str) -> bool {
        self.channels
            .iter()
            .find(|c| intent_key(&c.name) == intent_key(name))
            .is_some_and(|c| matches!(c.tree, PlaybackTree::Transition(_)))
    }
    pub fn insert(&mut self, name: String, tree: PlaybackTree, settings: ChannelSettings) {
        let position = self
            .channels
            .partition_point(|c| c.playback.settings.priority <= settings.priority);
        self.channels.insert(
            position,
            Channel {
                name,
                tree,
                playback: ChannelPlayback::new(settings),
            },
        );
    }
    pub fn end(&mut self, name: &str) {
        if let Some(c) = self.find_mut(name) {
            c.playback.end();
        }
    }
    pub fn end_with(&mut self, name: &str, seconds: f32, from_last_frame: bool) {
        if let Some(c) = self.find_mut(name) {
            c.playback.end_with(seconds, from_last_frame);
        }
    }
    pub fn influence(&mut self, name: &str, value: f32) -> bool {
        match self.find_mut(name) {
            Some(c) => {
                c.playback.influence = value;
                true
            }
            None => false,
        }
    }
    pub fn can_transition(&mut self, name: &str, resurrect: bool) -> bool {
        self.find_mut(name)
            .is_some_and(|c| c.playback.can_transition(resurrect))
    }
    pub fn transition(
        &mut self,
        name: &str,
        tree: PlaybackTree,
        settings: ChannelSettings,
        transition: skate_core::animation::playback::TransitionSettings,
        resurrect: bool,
    ) {
        if let Some(index) = self
            .channels
            .iter()
            .position(|c| intent_key(&c.name) == intent_key(name))
        {
            let mut c = self.channels.remove(index);
            let old = match c.tree {
                PlaybackTree::Transition(t) => *t.to,
                other => other,
            };
            c.tree = PlaybackTree::Transition(
                skate_core::animation::playback_transition::PlaybackTransition::new(
                    old, tree, transition,
                ),
            );
            c.playback.transition(settings, resurrect);
            self.channels.insert(index, c);
        }
    }
    fn find_mut(&mut self, name: &str) -> Option<&mut Channel> {
        self.channels
            .iter_mut()
            .find(|c| intent_key(&c.name) == intent_key(name))
    }
    pub fn retire(&mut self) {
        for c in &mut self.channels {
            if let PlaybackTree::Transition(t) = &c.tree {
                if t.complete() {
                    c.tree = *t.to.clone();
                }
            }
        }
        self.channels.retain(|c| !c.playback.expired());
    }
    pub fn advance(&mut self, dt: f32, phase: f32) {
        for c in &mut self.channels {
            if c.playback.advance(dt, c.tree.length(), c.tree.time()) {
                let mut property = AdvanceResult {
                    crossed_end: false,
                    overshoot: -1.0,
                    remaining_before_wrap: -1.0,
                };
                c.tree.advance(dt, phase, &mut property);
                c.playback.did_advance(property);
            }
        }
    }
    pub fn set_attributes(&mut self, attributes: &[SettableAttribute]) -> Result<(), String> {
        for c in &mut self.channels {
            c.tree.set_attributes(attributes)?;
        }
        Ok(())
    }
    pub fn attributes(
        &self,
        mut base: Vec<AnimationAttribute>,
        mask: u32,
    ) -> Result<Vec<AnimationAttribute>, String> {
        for c in &self.channels {
            if c.playback.settings.use_attributes {
                base = c.playback.merge_attributes(base, &c.tree.attributes(mask)?);
            }
        }
        Ok(base)
    }
    pub fn query_attribute(
        &self,
        name: AttributeName,
        mask: u32,
        output: &mut AnimationAttribute,
    ) -> Result<bool, String> {
        for c in self.channels.iter().rev() {
            if c.tree.query_attribute(name, mask, output)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn evaluate(
        &mut self,
        parameters: Evaluation,
        output: &mut Vec<PoseCommand>,
    ) -> Result<(), String> {
        for c in &mut self.channels {
            let enabled = c.playback.weight > 0.0;
            if c.tree.evaluate(parameters, enabled, output)? && enabled {
                output.push(PoseCommand::ChannelBlend {
                    weight: c.playback.weight,
                    use_channels_from_weights: false,
                });
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct FakieHead {
    value: f32,
    was_fakie: bool,
}
impl FakieHead {
    pub fn update(
        &mut self,
        animation: &mut super::motion_animation::MotionAnimation,
        manualing: bool,
        power_sliding: bool,
        fakie: bool,
    ) -> Result<(), String> {
        use skate_core::animation::{
            playback::TransitionSettings, playback_parameters::AttributeSink,
            skeleton_input::name::encode,
        };
        let target = if manualing {
            1.0
        } else if power_sliding {
            0.0
        } else {
            0.5
        };
        if !self.was_fakie && fakie {
            let settings = ChannelSettings {
                priority: 0,
                keep_alive: true,
                mirrored: false,
                speed: 1.0,
                blend_in: f32::from_bits(0x3e99999a),
                hold_during_blend_in: false,
                blend_out: f32::from_bits(0x3e99999a),
                hold_during_blend_out: false,
                use_attributes: false,
            };
            let transition = TransitionSettings {
                kind: 2,
                seconds: f32::from_bits(0x3dcccccd),
                under: 0,
                matching: 0,
                use_channels_from_weights: false,
            };
            animation.transition_channel(
                "fakie",
                "B_FAKIE_CHANNEL",
                settings,
                transition,
                true,
                true,
            )?;
            self.value = target;
        } else if self.was_fakie && !fakie {
            animation.channels.end("fakie");
        }
        self.was_fakie = fakie;
        if fakie {
            let delta = target - self.value;
            let limit = f32::from_bits(0x3c23d70a);
            let lower = if -limit - delta >= 0.0 { -limit } else { delta };
            let delta = if limit - lower >= 0.0 { lower } else { limit };
            self.value += delta;
            animation.set_attribute(SettableAttribute {
                name: encode(b"torso"),
                value: self.value,
                normalized: false,
                sequence_id: -1,
            });
        }
        Ok(())
    }
}
