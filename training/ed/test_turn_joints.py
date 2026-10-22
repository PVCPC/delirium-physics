import unittest
import numpy as np
from train_motion_rust import learner_capability
from gait_refinement import penalties

class CutJointTests(unittest.TestCase):
    def test_control_contract_and_reward_are_separate(self):
        opts=dict(adaptive=True,active_loss=True,ankle_residual=True,leg_ankle_learning=True,foot_state_feedback=True,turn_preview_feedback=True,whole_body_turn=True,turn_joint_control=True)
        self.assertEqual(learner_capability('mocap-actuator-v2',**opts),'protected-walk-cut-joints-v1')
        with self.assertRaises(ValueError):learner_capability('mocap-actuator-v2',turn_joint_control=True)
        profile={'minimum_lateral_foot_separation_m':.08,'walking_target_speed_m_s':1.5,'jog_reference_speed_m_s':2.2}
        old=np.zeros((3,358),dtype='f4');old[:,-5]=.3;old[2,351]=1.
        new=np.concatenate([old[:,:350],np.ones((3,1),dtype='f4'),old[:,350:]],axis=1)
        args=dict(foot_state_feedback=True,turn_preview_feedback=True,whole_body_turn=True)
        np.testing.assert_array_equal(penalties(old,350,'mocap-actuator-v2',profile,**args),penalties(new,351,'mocap-actuator-v2',profile,turn_joint_control=True,**args))
        with self.assertRaises(ValueError):penalties(new,351,'mocap-actuator-v2',profile,**args)

if __name__=='__main__':unittest.main()
