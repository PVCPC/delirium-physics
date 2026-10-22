import json
from pathlib import Path
import tempfile
import unittest
from live_training import Monitor

class RouteMonitorTest(unittest.TestCase):
    def test_route_evidence_is_matched_to_the_evaluation_and_rejects_bad_data(self):
        with tempfile.TemporaryDirectory() as folder:
            out=Path(folder)
            (out/'progress.json').write_text(json.dumps(dict(update=10)))
            path=out/'candidate.net.tracking.json'
            path.write_text(json.dumps(dict(update=10,path=[[-1.,2.,100.]]*8)))
            monitor=Monitor(out)
            self.assertEqual(monitor.snapshot()['evaluation_path']['trials'][0],[-1.,2.,100.])
            path.write_text(json.dumps(dict(update=9,path=[[-1.,2.,100.]]*8)))
            self.assertNotIn('evaluation_path',monitor.snapshot())
            path.write_text(json.dumps(dict(update=10,path=[[float('nan'),2.,100.]]*8)))
            self.assertNotIn('evaluation_path',monitor.snapshot())

if __name__=='__main__':unittest.main()
