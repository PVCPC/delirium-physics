"""Three-reference curriculum gates; partial stages never qualify as learned jogging."""
import numpy as np
import coordination

SCHEDULE = dict(walk=[0, 8], walk_to_brisk=[8, 14], brisk=[14, 20],
                brisk_to_jog=[20, 28], jog=[28, 40], jog_to_brisk=[40, 46],
                brisk_again=[46, 48], brisk_to_walk=[48, 54], walk_again=[54, 60])


def ramp(x):
    x = np.clip(x, 0., 1.)
    return x*x*(3.-2.*x)


def weights(t, limit):
    fast = ramp((np.asarray(t)-8.)/6.)-ramp((np.asarray(t)-48.)/6.)
    jog = limit*(ramp((np.asarray(t)-20.)/8.)-ramp((np.asarray(t)-40.)/6.))
    return 1.-fast, fast-jog, jog


def next_limit(limit):
    assert np.isfinite(limit) and 0. <= limit <= 1.
    return min(1., round(limit+.15, 6))


def ready(evaluation, anchor, measures, protected, seconds=60.):
    e = np.asarray(evaluation)
    return (e.shape == (8, 6) and np.isfinite(e).all()
            and bool(np.all(e[:, 5] >= seconds-.01) and np.all(e[:, 3] >= .7))
            and coordination.improved(e, anchor, measures, protected, None))


def contact_summary(report):
    rows=np.asarray(report.get('gait',[]),dtype=float)
    if rows.shape != (8,4) or not np.isfinite(rows).all():
        raise ValueError('Contact evidence must contain all eight native trials')
    mixed=rows[::2];samples=mixed[:,0].sum()
    return dict(actual_flight_fraction=float(mixed[:,1].sum()/max(samples,1.)),
                reference_flight_fraction=float(mixed[:,2].sum()/max(samples,1.)),
                foot_contact_agreement=float(mixed[:,3].sum()/max(samples,1.)))


def flight_ready(report):
    contact_summary(report)
    mixed=np.asarray(report['gait'],dtype=float)[::2]
    return bool(np.all(mixed[:,0]>=60) and np.all(mixed[:,2]>=1)
                and np.all(mixed[:,1]>=np.maximum(.02*mixed[:,0],.5*mixed[:,2]))
                and np.all(mixed[:,3]>=.75*mixed[:,0]))


def contact_progress(evaluation,report):
    e=np.asarray(evaluation)
    if e.shape!=(8,6) or not np.isfinite(e).all() or np.any(e[:,5]<59.99) or np.any(e[:,3]<.7):
        return None
    summary=contact_summary(report)
    return np.array([summary['actual_flight_fraction'],summary['foot_contact_agreement']])


def contact_gain(current,previous):
    return current is not None and (previous is None or bool(np.any(np.asarray(current)>=np.asarray(previous)+[.01,.02])))


def alignment_progress(evaluation,report):
    e=np.asarray(evaluation);a=np.asarray(report.get('alignment',[]),dtype=float)
    if a.shape!=(8,4) or not np.isfinite(a).all() or np.any(a<0):raise ValueError('Eight finite alignment rows required')
    if e.shape!=(8,6) or not np.isfinite(e).all() or np.any(e[:,5]<59.99) or np.any(e[:,3]<.7):return None
    return a.mean(0)


def alignment_gain(current,previous):
    if current is None:return False
    if previous is None:return True
    c,p=np.asarray(current),np.asarray(previous)
    # Count material improvement, while rejecting a large tradeoff in another defect.
    return bool(np.any(c<=p-np.maximum(.10*p,[.0001,.01,.0025,.01])) and np.all(c<=p*1.10+[.0001,.01,.0025,.01]))
