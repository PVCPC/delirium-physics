import unittest
import numpy as np
import turn_curriculum as t

class TurnChecks(unittest.TestCase):
    def fixture(self):
        e=np.tile([.25,.25,.09,1.6,.5,60.],(8,1));r=np.zeros((8,6));r[::2]=[.5,.8,.01,60.,60.,120.]
        return e,dict(turn=r.tolist(),turn_columns=['cross','rmse','heading','return','seconds','arc'])
    def test_straight_or_wrong_way_is_not_a_turn_success(self):
        e,r=self.fixture();self.assertTrue(t.guarded(e,r,e))
        r['turn'][0][2]=2.;r['turn'][0][3]=-60.
        self.assertFalse(t.guarded(e,r,e))
    def test_pre_turn_fall_has_no_heading_credit(self):
        e,r=self.fixture();e[::2,5]=12.
        for i in [0,2,4,6]:r['turn'][i][4]=12.
        p=t.progress(e,r);self.assertEqual(p[1],-2.)
        self.assertFalse(t.guarded(e,r,e))
    def test_rehearsal_fall_blocks_progress(self):
        e,r=self.fixture();e[1,5]=30.;self.assertIsNone(t.progress(e,r))
    def test_slower_cut_schedule_never_credits_heading_before_return(self):
        self.assertEqual(t.schedule(3.)['turn'],[18,21.])
        self.assertEqual(t.schedule()['jog_back'],[18+52/30,60])
        e,r=self.fixture();e[::2,5]=21.5
        for i in [0,2,4,6]:r['turn'][i][4]=21.5
        r['turn_return_start_seconds']=21+16/60
        self.assertEqual(t.progress(e,r)[1],-2.)
        r['turn_return_start_seconds']=float('nan')
        with self.assertRaises(ValueError):t.progress(e,r)
    def test_progress_and_matching_evidence(self):
        e,r=self.fixture();p=t.progress(e,r);self.assertTrue(t.improved(p,None));self.assertFalse(t.improved(p,p))
        r['turn'][0][4]=30.
        with self.assertRaises(AssertionError):t.progress(e,r)

if __name__=='__main__':unittest.main()
