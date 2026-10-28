use super::*;

impl MotionAnimation {
    pub(super) fn prepare_selection_spaces(
        &mut self,
        tree: &mut PlaybackTree,
        attributes: &[SettableAttribute],
    ) -> Result<(), String> {
        match tree {
            PlaybackTree::SelectionSpace(space) => {
                space.select(attributes)?;
                self.prepare_selection_spaces(space.current_mut().unwrap(), attributes)?;
            }
            PlaybackTree::BlendSpace(space) => {
                let _ = space;
            }
            PlaybackTree::PhaseBlend(space) => {
                for child in &mut space.children {
                    self.prepare_selection_spaces(child, attributes)?;
                }
            }
            PlaybackTree::Transition(transition) => {
                self.prepare_selection_spaces(&mut transition.to, attributes)?;
                if transition.settings.kind == 4 && !transition.complete() {
                    self.prepare_selection_spaces(&mut transition.from, attributes)?;
                }
            }
            PlaybackTree::BindPose { motion, .. } => {
                self.prepare_selection_spaces(motion, attributes)?
            }
            PlaybackTree::Clip { .. } => {}
        }
        Ok(())
    }
}
