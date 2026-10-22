import unittest

import numpy as np

from scipy.spatial.transform import Rotation as R

from prepare_walk import lateral_lean, calibrated_foot_axes


class ReferencePostureTest(unittest.TestCase):
    def test_foot_calibration_removes_bone_slope_but_preserves_roll_and_toe_off(self):
        neutral = R.from_euler('xyz', [12., -18., 23.], degrees=True)
        motion = R.from_euler('xyz', [[0., 0., 0.], [20., 0., 0.], [0., -30., 0.]], degrees=True)
        toe, sole = calibrated_foot_axes(motion * neutral, neutral, [1., 0., -.5])
        np.testing.assert_allclose(toe, motion.apply([1., 0., 0.]), atol=1e-12)
        np.testing.assert_allclose(sole, motion.apply([0., 0., 1.]), atol=1e-12)
        np.testing.assert_allclose((toe * sole).sum(1), 0., atol=1e-12)

    def test_forward_lean_and_step_sway_do_not_look_like_a_sideways_bias(self):
        frames = np.zeros((61, 21, 3))
        frames[:, 0, 0] = np.linspace(0., 2., 61)
        for index, height in [(9, .3), (12, .7)]:
            frames[:, index] = frames[:, 0] + [height * .2, 0., height]
            frames[:, index, 1] += height * np.tan(np.radians(6. * np.sin(np.linspace(0., 2. * np.pi, 61))))
        self.assertLess(max(abs(x) for x in lateral_lean(frames).values()), .01)
        for index in [9, 12]:
            frames[:, index, 1] += frames[:, index, 2] * np.tan(np.radians(8.))
        self.assertGreater(min(lateral_lean(frames).values()), 7.)


if __name__ == '__main__':
    unittest.main()
