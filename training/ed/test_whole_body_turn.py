import unittest
import numpy as np
from gait_refinement import penalties
from train_motion_rust import learner_capability

class WholeBodyTests(unittest.TestCase):
    def test_explicit_ownership_and_packet_shape(self):
        args=['mocap-actuator-v2',True,True,False,False,False,True,True,True,True]
        self.assertEqual(learner_capability(*args,True),'protected-walk-whole-body-turn-v1')
        with self.assertRaises(ValueError):learner_capability('mocap-actuator-v2',whole_body_turn=True)
        profile={'minimum_lateral_foot_separation_m':.08,'walking_target_speed_m_s':1.5,'jog_reference_speed_m_s':2.2}
        old=np.zeros((3,357),dtype='f4');old[:,-5]=.3;old[2,350]=1.
        new=np.concatenate([old[:,:349],np.ones((3,1),dtype='f4'),old[:,349:]],axis=1)
        opts=dict(foot_state_feedback=True,turn_preview_feedback=True)
        np.testing.assert_array_equal(penalties(old,349,'mocap-actuator-v2',profile,**opts),penalties(new,350,'mocap-actuator-v2',profile,whole_body_turn=True,**opts))
        with self.assertRaises(ValueError):penalties(new,350,'mocap-actuator-v2',profile,**opts)

if __name__=='__main__':unittest.main()
