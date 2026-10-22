import unittest
import numpy as np
from gait_refinement import penalties
from test_coordination import PROFILE,packet

class SupportPacket(unittest.TestCase):
    def test_actuator_feedback_appends_without_shifting_old_gait_features(self):
        old=packet();new=np.zeros((1,341),dtype=np.float32)
        new[:,:313]=old[:,:313];new[:,313:333]=.7;new[:,333:]=old[:,313:]
        np.testing.assert_array_equal(penalties(old,313,'mocap-jog-ballistic-v2',PROFILE),
            penalties(new,333,'mocap-actuator-v2',PROFILE))
        with self.assertRaises(ValueError):penalties(new,331,'mocap-actuator-v2',PROFILE)

    def test_leg_strength_feedback_keeps_body_gait_and_native_metric_indices(self):
        old=packet();new=np.zeros((1,339),dtype=np.float32)
        new[:,:313]=old[:,:313];new[:,313:331]=.7;new[:,331:]=old[:,313:]
        for mode in ('mocap-leg-strength-v2','mocap-leg-skill-v2'):
            np.testing.assert_array_equal(penalties(old,313,'mocap-jog-ballistic-v2',PROFILE),
                penalties(new,331,mode,PROFILE))
            with self.assertRaises(ValueError):penalties(new,329,mode,PROFILE)

    def test_new_mode_keeps_existing_packet_layout_and_checks_dimension(self):
        # Invalid dimensions must fail before any calibrated profile is consumed.
        with self.assertRaises(ValueError):penalties(np.zeros((2,321),dtype=np.float32),313,'mocap-push-off-v2',{})
        with self.assertRaises(ValueError):penalties(np.zeros((2,331),dtype=np.float32),323,'mocap-push-off-v2',None)

    def test_added_contact_inputs_do_not_move_legacy_body_or_metric_fields(self):
        old=packet();new=np.zeros((1,331),dtype=np.float32)
        new[:,:313]=old[:,:313];new[:,313:323]=.7;new[:,323:]=old[:,313:]
        np.testing.assert_array_equal(penalties(old,313,'mocap-jog-ballistic-v2',PROFILE),
            penalties(new,323,'mocap-push-off-v2',PROFILE))

if __name__=='__main__':unittest.main()
