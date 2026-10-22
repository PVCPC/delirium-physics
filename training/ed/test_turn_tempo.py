import unittest
from train_motion_rust import learner_capability
class TurnTempoTests(unittest.TestCase):
 def test_ankle_teacher_requires_corresponding_loss_mask(self):
  opts=dict(adaptive=True,active_loss=True,ankle_residual=True,leg_ankle_learning=True,foot_state_feedback=True,turn_preview_feedback=True,whole_body_turn=True,turn_joint_control=True)
  self.assertEqual(learner_capability('mocap-actuator-v2',walking_ankle_expert=True,**opts),'protected-walk-ankle-anchor-v1')
  self.assertEqual(learner_capability('mocap-actuator-v2',**opts),'protected-walk-cut-joints-v1')
  with self.assertRaises(ValueError):learner_capability('mocap-actuator-v2',walking_ankle_expert=True)
if __name__=='__main__':unittest.main()
