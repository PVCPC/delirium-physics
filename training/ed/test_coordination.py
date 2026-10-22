import unittest
import numpy as np
from coordination import observation_cost, improved, score
from gait_refinement import penalties, contract


PROFILE = dict(minimum_lateral_foot_separation_m=.08, walking_target_speed_m_s=1.5,
               jog_reference_speed_m_s=2.2, walk_arm_velocity_gain=[.3,.2], jog_arm_velocity_gain=[.6,.3])


def packet():
    p = np.zeros((1,321), dtype=np.float32)
    p[:,0] = 1.; p[:,1:4] = [0.,-1.,0.]; p[:,318] = 1.5
    body = p[:,4:193].reshape(1,21,9)
    body[:,3,:3] = [-.15,0.,.1]; body[:,6,:3] = [.15,0.,.1]
    return p


class CoordinationTest(unittest.TestCase):
    def test_reference_placement_replaces_only_jogging_static_width_rule(self):
        old=packet();p=np.zeros((1,341),dtype=np.float32);p[:,:313]=old[:,:313];p[:,333:]=old[:,313:]
        body=p[:,4:193].reshape(1,21,9);body[:,6,0]=-.11;p[:,311]=1.6
        self.assertGreater(float(penalties(p,333,'mocap-actuator-v2',PROFILE)[0]),0.)
        self.assertEqual(float(penalties(p,333,'mocap-actuator-v2',PROFILE,reference_placement=True)[0]),0.)
        p[:,311]=0.
        np.testing.assert_array_equal(penalties(p,333,'mocap-actuator-v2',PROFILE),penalties(p,333,'mocap-actuator-v2',PROFILE,reference_placement=True))
        p[:,334]=1.;np.testing.assert_array_equal(penalties(p,333,'mocap-actuator-v2',PROFILE,reference_placement=True),0.)

    def test_reference_relative_startup_bias_does_not_require_zero_sway(self):
        e = np.tile([.3,.3,.1,1.,.5,60.],(8,1))
        m = dict(crossing_fraction=0.,near_crossing_cost=0.,arm_velocity_rmse_m_s=.5,
                 walking_speed_m_s=1.,minimum_swing_excursion_m=.44,startup_lateral_mean_deg=[-1.,-2.])
        a = dict(m,startup_lateral_target_deg=[-.2,-1.2],maximum_startup_lateral_error_deg=1.5)
        self.assertTrue(improved(e,e,m,a,None))
        self.assertFalse(improved(e,e,dict(m,startup_lateral_mean_deg=[-4.,-5.]),a,None))

    def test_new_stride_anchor_rejects_return_to_short_quick_steps(self):
        evaluation = np.tile([.3, .3, .1, 1., .5, 60.], (8, 1))
        metrics = dict(crossing_fraction=0., near_crossing_cost=0., arm_velocity_rmse_m_s=.5,
                       walking_speed_m_s=1., minimum_swing_excursion_m=.44, walking_cycle_seconds=1.0)
        anchor = dict(metrics, walking_cycle_bounds_s=[.82, 1.12], minimum_preserved_swing_excursion_m=.39)
        self.assertTrue(improved(evaluation, evaluation, metrics, anchor, None))
        self.assertFalse(improved(evaluation, evaluation, dict(metrics, walking_cycle_seconds=.8), anchor, None))
        self.assertFalse(improved(evaluation, evaluation, dict(metrics, minimum_swing_excursion_m=.37), anchor, None))
        self.assertTrue(improved(evaluation, evaluation, dict(metrics, walking_cycle_seconds=.8), metrics, None))

    def test_crossing_has_cost_without_rewarding_excessive_width(self):
        p = packet(); normal=observation_cost(p,PROFILE,False)
        self.assertEqual(float(normal[0]),0.)
        body=p[:,4:193].reshape(1,21,9)
        body[:,6,0] = -.2
        self.assertGreater(float(observation_cost(p,PROFILE,False)[0]), .05)
        body[:,6,0] = 1.
        self.assertEqual(float(observation_cost(p,PROFILE,False)[0]),0.)

    def test_arm_timing_and_speed_are_real_targets(self):
        p=packet(); body=p[:,4:193].reshape(1,21,9)
        body[:,6,4]=.2; body[:,3,4]=-.2
        body[:,16,4]=.06; body[:,20,4]=-.04
        self.assertLess(float(observation_cost(p,PROFILE,True)[0]),1e-10)
        body[:,16,4]=-.06; body[:,20,4]=.04
        self.assertGreater(float(observation_cost(p,PROFILE,True)[0]),.005)
        body[:,16,4]=.06; body[:,20,4]=-.04; p[:,318]=.8
        self.assertGreater(float(observation_cost(p,PROFILE,True)[0]),.01)

    def test_terminal_reset_does_not_supply_a_coordination_cost(self):
        p=packet(); p[:,4+6*9]=-.2; p[:,314]=1.
        np.testing.assert_array_equal(penalties(p,313,'coordination-v2',PROFILE),0.)
        self.assertNotEqual(contract('coordination-v2'),contract('slip-arm-v1'))

    def test_turning_the_observation_frame_does_not_change_cost(self):
        p=packet(); p[:,4+6*9]=-.2
        original=observation_cost(p,PROFILE,True)
        rotation=np.array([[0.,-1.,0.],[1.,0.,0.],[0.,0.,1.]])
        p[:,1:4]=p[:,1:4] @ rotation.T
        body=p[:,4:193].reshape(1,21,9)
        body[:,:,:3]=body[:,:,:3] @ rotation.T
        body[:,:,3:6]=body[:,:,3:6] @ rotation.T
        np.testing.assert_allclose(observation_cost(p,PROFILE,True),original,atol=1e-7)

    def test_quality_guard_rejects_cheap_improvements(self):
        e=np.tile([.3,.6,.08,1.,.5,60.],(8,1))
        m=dict(near_crossing_cost=.2,crossing_fraction=.2,arm_velocity_rmse_m_s=.8,
               walking_speed_m_s=1.,minimum_swing_excursion_m=.4)
        better=dict(m,near_crossing_cost=.15)
        self.assertTrue(improved(e,e,better,m,m))
        for key,value in [('crossing_fraction',.25),('arm_velocity_rmse_m_s',1.),('minimum_swing_excursion_m',.01)]:
            self.assertFalse(improved(e,e,dict(better,**{key:value}),m,m))
        bad=e.copy(); bad[0,5]=59.
        self.assertFalse(improved(bad,e,better,m,m))
        bad=e.copy(); bad[:,1]=.8
        self.assertFalse(improved(bad,e,better,m,m))


if __name__=='__main__': unittest.main()
