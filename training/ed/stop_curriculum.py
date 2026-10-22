"""A successful braking task moves before the cue and holds still afterward."""
import numpy as np

PROGRESS_COLUMNS=['mean_survival_seconds','negative_stop_latency_seconds','negative_hold_speed_m_s']

def schedule(_duration=None):
    return dict(walk=[0,8],walk_to_jog=[8,14],jog=[14,19],brake=[19,22],hold=[22,60])

def rows(evaluation,report):
    e=np.asarray(evaluation,dtype=float);r=np.asarray(report['stop'],dtype=float)
    if e.shape!=(8,6) or r.shape!=(8,9) or not np.isfinite(e).all() or not np.isfinite(r).all():
        raise ValueError('Stopping requires eight matching physical evaluation rows')
    np.testing.assert_allclose(r[::2,6],e[::2,5],atol=.02)
    return e,r[::2]

def progress(evaluation,report):
    e,r=rows(evaluation,report)
    if np.any(e[1::2,5]<59.99) or np.any(e[1::2,3]<.7) or np.any(r[:,0]<1.2):return None
    latency=np.where(r[:,6]>=23.,r[:,1],41.)
    hold=np.where(r[:,6]>=59.99,r[:,3],10.)
    return np.array([r[:,6].mean(),-latency.mean(),-hold.mean()])

def improved(current,previous):
    return current is not None and (previous is None or bool(np.any(np.asarray(current)-np.asarray(previous)>=np.array([.25,.15,.02]))))

def guarded(evaluation,report,anchor):
    e,r=rows(evaluation,report);a=np.asarray(anchor)
    return bool(np.all(e[:,5]>=59.99) and np.all(e[1::2,3]>=.7) and np.all(r[:,0]>=1.2)
        and np.all(r[:,1]<=2.) and np.all(r[:,2]<=3.) and np.all(r[:,3]<=.15)
        and np.all(r[:,4]<=.5) and np.all(r[:,5]<=.75) and np.all(r[:,7]<=2.) and np.all(r[:,8]<=.15)
        and e[:,1].mean()<=.2878 and e[:,0].mean()<=max(.35,a[:,0].mean()*1.1)
        and e[:,2].mean()<=max(.12,a[:,2].mean()*1.15))

def summary(evaluation,report):
    e,r=rows(evaluation,report)
    return dict(columns=report['stop_columns'],trials=report['stop'],
        all_stop_trials_complete=bool(np.all(r[:,6]>=59.99)),
        mean_stop_latency_s=float(r[:,1].mean()),mean_hold_speed_m_s=float(r[:,3].mean()),
        max_hold_displacement_m=float(r[:,5].max()),qualified_natural_gait=False)
