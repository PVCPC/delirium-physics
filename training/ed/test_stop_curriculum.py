import unittest
import numpy as np
import stop_curriculum as s

class StopCurriculumTests(unittest.TestCase):
    def fixture(self):
        e=np.tile([.2,.1,.05,.6,.3,60.],(8,1));e[1::2,3]=1.5
        r=np.tile([2.,1.,1.5,.05,.2,.3,60.,.4,.05],(8,1))
        return e,dict(stop=r.tolist(),stop_columns=list(range(9)))
    def test_correct_stop_is_not_rejected_by_whole_trial_minimum_speed(self):
        e,r=self.fixture();self.assertTrue(s.guarded(e,r,e));self.assertIsNotNone(s.progress(e,r))
    def test_no_standing_cheat_walking_regression_or_moving_hold(self):
        for column,value in [(0,0.),(1,41.),(3,2.),(5,30.)]:
            e,r=self.fixture();r['stop'][0][column]=value;self.assertFalse(s.guarded(e,r,e))
        e,r=self.fixture();e[1,5]=12.;self.assertIsNone(s.progress(e,r))
    def test_padding_never_counts_as_hold_success(self):
        e,r=self.fixture();e[0,5]=20.;r['stop'][0][6]=20.
        self.assertFalse(s.guarded(e,r,e));self.assertAlmostEqual(s.progress(e,r)[2],-2.5375)
    def test_mismatched_evidence_rejected(self):
        e,r=self.fixture();r['stop'][0][6]=12.
        with self.assertRaises(AssertionError):s.rows(e,r)

if __name__=='__main__':unittest.main()
