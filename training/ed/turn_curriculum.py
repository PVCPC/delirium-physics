"""Finite-turn progress is measured in the requested route, not global +X distance."""
import numpy as np

SCHEDULE=dict(walk=[0,8],walk_to_jog=[8,14],jog=[14,18],turn=[18,19.733333333333],jog_back=[19.733333333333,60])

def schedule(duration=52/30):
    if not np.isfinite(duration) or not 52/30<=duration<=3.:raise ValueError('Invalid captured-cut duration')
    return dict(SCHEDULE,turn=[18,18+duration],jog_back=[18+duration,60])

def return_ready(report):
    start=report.get('turn_return_start_seconds',20.)
    if not isinstance(start,(float,int)) or not np.isfinite(start) or not 20.<=start<=21.3:
        raise ValueError('Invalid finite-turn return measurement time')
    return start+1.

def rows(evaluation,report):
    e=np.asarray(evaluation,dtype=float);r=np.asarray(report['turn'],dtype=float)
    if e.shape!=(8,6) or r.shape!=(8,6) or not np.isfinite(e).all() or not np.isfinite(r).all():
        raise ValueError('Finite turn needs eight matching physical evaluations and route rows')
    np.testing.assert_allclose(r[::2,4],e[::2,5],atol=.02)
    return e,r[::2]

def progress(evaluation,report):
    e,r=rows(evaluation,report)
    if np.any(e[1::2,5]<59.99) or np.any(e[:,3]<.7):return None
    # Falls before the cut receive no invented heading or return credit.
    heading=np.where(r[:,4]>=return_ready(report),r[:,2],2.)
    return np.array([e[::2,5].mean(),-heading.mean(),np.maximum(r[:,3],0.).mean()])

def improved(current,previous):
    if current is None:return False
    if previous is None:return True
    return bool(np.any(np.asarray(current)-np.asarray(previous)>=np.array([.25,.03,.5])))

def guarded(evaluation,report,anchor):
    e,r=rows(evaluation,report);a=np.asarray(anchor)
    # Development selection only. Natural gait and contact/flight still require assessment.
    return bool(np.all(e[:,5]>=59.99) and np.all(e[:,3]>=.7)
        and e[:,1].mean()<=.2878 and e[:,0].mean()<=max(.35,a[:,0].mean()*1.1)
        and e[:,2].mean()<=max(.12,a[:,2].mean()*1.15)
        and np.max(r[:,0])<=2. and np.max(r[:,1])<=3.
        and np.max(r[:,2])<=.15 and np.min(r[:,3])>=25.)

def summary(evaluation,report):
    e,r=rows(evaluation,report)
    return dict(columns=report['turn_columns'],trials=report['turn'],
        all_turn_trials_complete=bool(np.all(e[::2,5]>=59.99)),
        mean_return_heading_cost=float(np.where(r[:,4]>=return_ready(report),r[:,2],2.).mean()),
        min_return_progress_m=float(np.maximum(r[:,3],0.).min()),
        max_cross_track_m=float(r[:,0].max()),qualified_natural_gait=False)
