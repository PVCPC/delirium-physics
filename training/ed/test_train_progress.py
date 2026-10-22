import unittest

import numpy as np

from train_motion import transition_progress, transition_improved


class TransitionProgressTest(unittest.TestCase):
    def test_recognizes_longer_faster_transitions_despite_lower_composite_score(self):
        trials = np.zeros((8, 6)); trials[:, 3] = 1.; trials[:, 5] = 60.
        trials[::2, 5] = 12.
        previous = transition_progress(trials, 60.)
        trials[::2, 5] = 14.; trials[::2, 3] = 1.3; trials[:, 4] = -1.
        self.assertTrue(transition_improved(transition_progress(trials, 60.), previous))
        trials[1, 5] = 30.
        self.assertIsNone(transition_progress(trials, 60.))

    def test_does_not_count_stalling_or_small_measurement_changes_as_progress(self):
        trials = np.zeros((8, 6)); trials[:, 3] = 1.; trials[:, 5] = 60.
        trials[::2, 3] = 0.
        self.assertIsNone(transition_progress(trials, 60.))
        self.assertFalse(transition_improved(np.array([13.1, 14.1]), np.array([13., 14.])))
        self.assertFalse(transition_improved(None, np.array([13., 14.])))


if __name__ == '__main__':
    unittest.main()
