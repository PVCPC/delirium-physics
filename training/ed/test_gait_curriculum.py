import unittest
from unittest.mock import patch
import numpy as np
import gait_curriculum as curriculum
import coordination


class ThreeGaitCurriculum(unittest.TestCase):
    def test_contact_patience_tracks_mechanics_only_with_full_moving_trials(self):
        e=np.zeros((8,6));e[:,3]=1.;e[:,5]=60.
        rows=np.zeros((8,4));rows[::2]=[1000,0,200,800]
        value=curriculum.contact_progress(e,dict(gait=rows.tolist()))
        self.assertFalse(curriculum.contact_gain(value,[0,.80]))
        rows[::2,3]=825
        self.assertTrue(curriculum.contact_gain(curriculum.contact_progress(e,dict(gait=rows.tolist())),value))
        e[0,5]=30.
        self.assertIsNone(curriculum.contact_progress(e,dict(gait=rows.tolist())))
        self.assertFalse(curriculum.contact_gain(None,value))

    def test_flight_gate_rejects_grounded_survival_and_requires_each_mixed_trial(self):
        rows=np.zeros((8,4));rows[::2]=[1000,0,200,800]
        self.assertFalse(curriculum.flight_ready(dict(gait=rows.tolist())))
        rows[::2,1]=120
        self.assertTrue(curriculum.flight_ready(dict(gait=rows.tolist())))
        rows[0,1]=0
        self.assertFalse(curriculum.flight_ready(dict(gait=rows.tolist())))
        with self.assertRaises(ValueError):curriculum.flight_ready(dict(gait=[[1,2,3,4]]))

    def test_arm_reference_tracks_all_three_gait_weights(self):
        profile=dict(minimum_lateral_foot_separation_m=.08,walking_target_speed_m_s=1.5,jog_reference_speed_m_s=3.16,
                     walk_arm_velocity_gain=[.2,.3],brisk_arm_velocity_gain=[.6,.7],jog_arm_velocity_gain=[.9,1.])
        packet=np.zeros((4,321));packet[:,0]=1;packet[:,2]=-1;packet[:,311]=[0,1,1.15,2]
        body=packet[:,4:193].reshape(4,21,9);body[:,6,0]=.1;body[:,3,0]=-.1
        body[:,[6,3],4]=.2
        gains=np.array([[.2,.3],[.6,.7],[.645,.745],[.9,1.]])
        body[:,[16,20],4]=gains*.2
        np.testing.assert_allclose(coordination.observation_cost(packet,profile,True,match_speed=False),0.,atol=1e-15)

    def test_schedule_retains_brisk_section_and_returns_to_walk(self):
        for t,expected in [(0,(1,0,0)),(16,(0,1,0)),(30,(0,.85,.15)),(47,(0,1,0)),(58,(1,0,0))]:
            np.testing.assert_allclose(curriculum.weights(t,.15),expected,atol=1e-12)
        w=np.array(curriculum.weights(np.arange(0,60,.01),1.))
        self.assertTrue(np.all(w>=-1e-12))
        np.testing.assert_allclose(w.sum(0),1.)

    def test_cap_is_bounded_and_only_increases_in_small_steps(self):
        value=.15
        history=[]
        while value<1.:
            history.append(value)
            new=curriculum.next_limit(value)
            self.assertLessEqual(new-value,.1500001)
            value=new
        self.assertEqual(value,1.)
        self.assertEqual(len(history),6)

    def test_promotion_rejects_a_fall_or_stopped_trial_even_if_other_guards_pass(self):
        e=np.zeros((8,6));e[:,3]=1.;e[:,5]=60.
        with patch('coordination.improved',return_value=True):
            self.assertTrue(curriculum.ready(e,e,{},{}))
            e[0,5]=59.
            self.assertFalse(curriculum.ready(e,e,{},{}))
            e[0,5]=60.;e[1,3]=.1
            self.assertFalse(curriculum.ready(e,e,{},{}))
        with patch('coordination.improved',return_value=False):
            e[:,3]=1.
            self.assertFalse(curriculum.ready(e,e,{},{}))


if __name__=='__main__':
    unittest.main()
