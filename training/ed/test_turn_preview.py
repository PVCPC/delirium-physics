import unittest
import numpy as np
from gait_refinement import penalties
from train_motion_rust import learner_capability
from coordination import targets

class TurnPreviewTests(unittest.TestCase):
    def test_requires_matching_ownership_capability(self):
        args=['mocap-actuator-v2',True,True,False,False,False,True,True,True]
        self.assertEqual(learner_capability(*args,True),'protected-walk-leg-ankle-turn-preview-v1')
        self.assertEqual(learner_capability(*args),'protected-walk-leg-ankle-foot-state-v1')
        with self.assertRaises(ValueError):learner_capability('mocap-actuator-v2',turn_preview_feedback=True)
    def test_new_inputs_do_not_change_existing_penalties_or_terminal_handling(self):
        profile={'minimum_lateral_foot_separation_m':.08,'walking_target_speed_m_s':1.5,'jog_reference_speed_m_s':2.2}
        old=np.zeros((3,341+8),dtype=np.float32);old[:,-5]=.3;old[2,342]=1.
        new=np.concatenate([old[:,:341],np.ones((3,8),np.float32),old[:,341:]],axis=1)
        a=penalties(old,341,'mocap-actuator-v2',profile,foot_state_feedback=True)
        b=penalties(new,349,'mocap-actuator-v2',profile,foot_state_feedback=True,turn_preview_feedback=True)
        np.testing.assert_array_equal(a,b);self.assertEqual(float(b[-1]),0.)
        with self.assertRaises(ValueError):penalties(new,349,'mocap-actuator-v2',profile,foot_state_feedback=True)
        with self.assertRaises(ValueError):penalties(new,349,'mocap-actuator-v2',profile,turn_preview_feedback=True)

if __name__=='__main__':unittest.main()
