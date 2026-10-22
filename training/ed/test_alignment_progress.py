import unittest
import numpy as np
from gait_curriculum import alignment_progress,alignment_gain
from gait_refinement import penalties
from test_coordination import PROFILE,packet

class AlignmentProgress(unittest.TestCase):
    def test_alignment_gain_requires_material_gain_without_large_tradeoff(self):
        p=np.array([.01,1.,.1,.25])
        self.assertTrue(alignment_gain(p*.8,p))
        self.assertFalse(alignment_gain(p*.99,p))
        self.assertFalse(alignment_gain([.005,2.,.1,.25],p))
    def test_falls_and_stopping_cannot_qualify_alignment_progress(self):
        e=np.tile([.3,.3,.1,1.,.5,60.],(8,1));r={'alignment':np.ones((8,4)).tolist()}
        self.assertIsNotNone(alignment_progress(e,r));e[0,5]=20.
        self.assertIsNone(alignment_progress(e,r));e[0,5]=60.;e[3,3]=.1
        self.assertIsNone(alignment_progress(e,r))
    def test_new_observations_keep_existing_python_penalties_in_place(self):
        old=packet();new=np.zeros((1,337),dtype=np.float32)
        new[:,:313]=old[:,:313];new[:,313:329]=.2;new[:,329:]=old[:,313:]
        np.testing.assert_array_equal(penalties(old,313,'mocap-jog-ballistic-v2',PROFILE),penalties(new,329,'mocap-alignment-v2',PROFILE))

if __name__=='__main__':unittest.main()
