import unittest
import numpy as np
from gait_refinement import penalties, contract, quality_improved


class RefinementTest(unittest.TestCase):
    def test_foot_state_inputs_preserve_penalties_and_require_explicit_shape(self):
        profile=dict(minimum_lateral_foot_separation_m=.08,walking_target_speed_m_s=1.5,jog_reference_speed_m_s=2.2)
        old=np.zeros((2,341),dtype='f4');old[:,0]=1.;old[:,1:4]=[1.,0.,0.];old[:,336]=[.3,.7]
        extra=np.array([[.4,-.3,.2,-.1,0.,.9,1.,0.],[-.2,.3,0.,.1,-.2,.4,0.,1.]],dtype='f4')
        new=np.concatenate([old[:,:333],extra,old[:,333:]],axis=1)
        np.testing.assert_array_equal(penalties(old,333,'mocap-actuator-v2',profile),penalties(new,341,'mocap-actuator-v2',profile,foot_state_feedback=True))
        new[:,342]=1.;np.testing.assert_array_equal(penalties(new,341,'mocap-actuator-v2',profile,foot_state_feedback=True),0.)
        with self.assertRaises(ValueError):penalties(new,341,'mocap-actuator-v2',profile)
        with self.assertRaises(ValueError):penalties(new,341,'mocap-actuator-v2',profile,foot_state_feedback=True,native_support_observations=True)
    def test_native_support_inputs_keep_reward_fields_and_existing_penalties_exact(self):
        profile=dict(minimum_lateral_foot_separation_m=.08,walking_target_speed_m_s=1.5,jog_reference_speed_m_s=2.2)
        old=np.zeros((2,341),dtype='f4');old[:,0]=1.;old[:,1:4]=[1.,0.,0.];old[:,336]=[.3,.7]
        new=np.concatenate([old[:,:333],np.array([[1.,0.,-1.,1.],[0.,1.,0.,-1.]],dtype='f4'),old[:,333:]],axis=1)
        np.testing.assert_array_equal(penalties(old,333,'mocap-actuator-v2',profile),penalties(new,337,'mocap-actuator-v2',profile,native_support_observations=True))
        with self.assertRaises(ValueError):penalties(new,337,'mocap-actuator-v2',profile)
        with self.assertRaises(ValueError):penalties(new,337,'mocap-v1',profile,native_support_observations=True)
    def test_dense_tracking_does_not_add_competing_speed_or_arm_targets(self):
        profile = dict(minimum_lateral_foot_separation_m=.08, walking_target_speed_m_s=1.5,
            jog_reference_speed_m_s=2.2, walk_arm_velocity_gain=[.3,.2], jog_arm_velocity_gain=[.6,.3])
        p = np.zeros((2,321),dtype=np.float32)
        p[:,0]=1.;p[:,1:4]=[1.,0.,0.]
        p[:,4+6*9+1]=-.2
        original=penalties(p,313,'mocap-v1',profile)
        p[:,318]=10.;p[:,199:259]=5.
        np.testing.assert_array_equal(penalties(p,313,'mocap-v1',profile),original)
        p[:,314]=1.
        np.testing.assert_array_equal(penalties(p,313,'mocap-v1',profile),0.)

    def test_sliding_cost_uses_native_metric_and_is_bounded(self):
        p = np.zeros((3, 321), dtype=np.float32)
        p[:, 316] = [0., 1., 100.]
        np.testing.assert_allclose(penalties(p, 313, 'slip-v1'), [0., .08, .32])
        # Speed and reward fields must not be mistaken for slip.
        p[:, 313] = 123.; p[:, 318] = 999.
        np.testing.assert_allclose(penalties(p, 313, 'slip-v1'), [0., .08, .32])

    def test_arm_error_penalty_ignores_resets_and_leg_errors(self):
        p = np.zeros((2, 3, 321), dtype=np.float32)
        p[..., 199:202] = 2.
        np.testing.assert_array_equal(penalties(p, 313, 'slip-arm-v1'), 0.)
        p[..., 199 + 13 * 3] = 1.
        self.assertTrue(np.all(penalties(p, 313, 'slip-arm-v1') > 0.))
        p[..., 314] = 1.
        np.testing.assert_array_equal(penalties(p, 313, 'slip-arm-v1'), 0.)

    def test_quality_gain_cannot_come_from_falling_or_slowing(self):
        anchor = np.tile([.3, 1., .08, 1., .5, 60.], (8, 1))
        improved = anchor.copy(); improved[:, 1] = .9
        self.assertTrue(quality_improved(improved, anchor, anchor, 60.))
        improved[0, 5] = 59.
        self.assertFalse(quality_improved(improved, anchor, anchor, 60.))
        improved[0, 5] = 60.; improved[0, 3] = .89
        self.assertFalse(quality_improved(improved, anchor, anchor, 60.))

    def test_quality_gain_preserves_pose_and_jitter(self):
        anchor = np.tile([.3, 1., .08, 1., .5, 60.], (8, 1))
        improved = anchor.copy(); improved[:, 1] = .9; improved[:, 0] = .32
        self.assertFalse(quality_improved(improved, anchor, anchor, 60.))
        improved[:, 0] = .3; improved[:, 2] = .1
        self.assertFalse(quality_improved(improved, anchor, anchor, 60.))
        self.assertFalse(quality_improved(anchor, anchor, anchor, 60.))

    def test_contract_and_shape_guard(self):
        self.assertEqual(contract(None), 'native-assisted-walk-jog-aligned-v3')
        self.assertNotEqual(contract('slip-v1'), contract('slip-arm-v1'))
        with self.assertRaises(ValueError):
            penalties(np.zeros((1, 321)), 312, 'slip-v1')
        with self.assertRaises(ValueError):
            contract('unknown')


if __name__ == '__main__':
    unittest.main()
