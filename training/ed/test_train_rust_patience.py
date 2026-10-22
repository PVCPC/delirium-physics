import unittest

from train_motion_rust import patience_age, patience_reference, learner_capability


class ContinuationPatience(unittest.TestCase):
    def test_foot_state_requires_its341_input_learner(self):
        self.assertEqual(learner_capability('mocap-actuator-v2',True,True,False,False,False,True,True,True),'protected-walk-leg-ankle-foot-state-v1')
        with self.assertRaises(ValueError):learner_capability('mocap-actuator-v2',True,True,False,False,False,True,False,True)
    def test_leg_ankle_learning_requires_its_own54_output_capability(self):
        self.assertEqual(learner_capability('mocap-actuator-v2',True,True,False,False,False,True,True),'protected-walk-leg-ankle-loss-v1')
        with self.assertRaises(ValueError):learner_capability('mocap-actuator-v2',True,True,False,False,False,False,True)
    def test_ankle_residual_requires_its_frozen54_output_learner(self):
        self.assertEqual(learner_capability('mocap-actuator-v2',True,True,False,False,False,True),'frozen-gait-ankle-residual-v1')
        for args in [('mocap-actuator-v2',True,True,False,False,True,True),('mocap-actuator-v2',True,True,True,False,False,True)]:
            with self.assertRaises(ValueError):learner_capability(*args)
    def test_control_aware_loss_requires_its_specific_learner(self):
        self.assertEqual(learner_capability('mocap-actuator-v2',True,True,False,False,True),'protected-walk-controlled-leg-loss-v2')
        for args in [('mocap-actuator-v2',True,True,True,False,True),('mocap-actuator-v2',True,True,False,True,True),('mocap-actuator-v2',True,False,False,False,True)]:
            with self.assertRaises(ValueError):learner_capability(*args)
    def test_native_support_requires_its_337_input_mask(self):
        self.assertEqual(learner_capability('mocap-actuator-v2',True,True,False,True),'protected-walk-native-support-v1')
        self.assertEqual(learner_capability('mocap-actuator-v2',True,True,True,True),'protected-arms-native-support-v1')
        for args in [('mocap-actuator-v2',True,False,False,True)]:
            with self.assertRaises(ValueError):learner_capability(*args)
    def test_adaptive_mask_requires_explicit_matching_controller(self):
        self.assertEqual(learner_capability('mocap-actuator-v2',True,True,True),'protected-arms-balance-leg-loss-v1')
        with self.assertRaises(ValueError):learner_capability('mocap-actuator-v2',True,False,True)
        self.assertEqual(learner_capability('mocap-actuator-v2'),'protected-walk-actuator-v1')
        self.assertEqual(learner_capability('mocap-actuator-v2',True),'protected-walk-adaptive-features-v1')
        with self.assertRaises(ValueError):
            learner_capability('mocap-leg-skill-v2',True)
        self.assertEqual(learner_capability('mocap-actuator-v2',True,True),'protected-walk-active-leg-loss-v1')
        with self.assertRaises(ValueError):
            learner_capability('mocap-actuator-v2',False,True)

    def test_default_retains_original_plateau(self):
        self.assertEqual(patience_age(39024, 29024), 10000)

    def test_explicit_continuation_keeps_full_patience_window(self):
        self.assertEqual(patience_age(39049, 29024, 39024), 25)
        self.assertEqual(patience_age(49024, 29024, 39024), 10000)

    def test_actual_progress_supersedes_continuation(self):
        self.assertEqual(patience_age(45000, 44000, 39024), 1000)

    def test_partial_transition_gains_only_extend_opted_in_curriculum(self):
        self.assertEqual(patience_reference(138299, 147000), 138299)
        self.assertEqual(patience_reference(138299, 147000, True), 147000)
        self.assertEqual(patience_age(148299, patience_reference(138299, 147000, True)), 1299)

    def test_qualified_progress_can_supersede_transition_gain(self):
        self.assertEqual(patience_reference(148000, 147000, True), 148000)


if __name__ == '__main__':
    unittest.main()
