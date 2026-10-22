"""Conservative gait checks on private native-physics replays; no renderer or GPU."""
import argparse
import json
import struct
from pathlib import Path

import numpy as np
from scipy.spatial.transform import Rotation


DT = 1 / 60


def foot_motion(frames, body, half, collider_rotation=None):
    position = frames[:, body, :3]
    rotation = Rotation.from_quat(frames[:, body, 3:])
    if collider_rotation is not None:
        rotation = rotation * Rotation.from_quat(collider_rotation)
    up = rotation.inv().apply(np.tile([0., 0., 1.], (len(frames), 1)))
    offset = rotation.apply(-np.sign(up) * half)
    bottom = position[:, 2] + offset[:, 2]
    angular = np.vstack([(rotation[1:] * rotation[:-1].inv()).as_rotvec() / DT, [0., 0., 0.]])
    velocity = np.gradient(position, DT, axis=0) + np.cross(angular, offset)
    # shortcut: geometric floor contact estimates; instrument contact impulses before terrain assessment.
    contact = bottom < .025
    slip = np.linalg.norm(velocity[contact, :2], axis=1)
    return dict(clearance_95_m=float(np.percentile(bottom, 95)),
                lifted_fraction=float(np.mean(bottom > .025)),
                contact_slip_mean_m_s=float(slip.mean()) if len(slip) else 1000.,
                contact_slip_95_m_s=float(np.percentile(slip, 95)) if len(slip) else 1000.)


def correlation(a, b):
    return float(np.corrcoef(a, b)[0, 1]) if min(np.std(a), np.std(b)) > 1e-6 else 0.


def assess_trial(frames, feet, collider_rotations=None):
    position = frames[..., :3]
    facing = np.cross([0., 0., 1.], position[0, 4] - position[0, 1])
    facing[2] = 0.; facing /= max(np.linalg.norm(facing), 1e-6)
    side = np.cross([0., 0., 1.], facing)
    displacement = position[:, 0] - position[0, 0]
    forward = displacement @ facing
    fallen = (position[:, 12, 2] < .8) | (position[:, 0, 2] < .4)
    alive_frames = int(np.flatnonzero(fallen)[0] + 1) if fallen.any() else len(frames)
    seconds = len(frames) * DT
    active = frames[120:alive_frames]
    if len(active) < 60:
        return dict(passed=False, checks=dict(sustained_walking=False), survival_seconds=alive_frames * DT)
    p = active[..., :3]
    left_arm = (p[:, 16] - p[:, 13]) @ facing
    right_arm = (p[:, 20] - p[:, 17]) @ facing
    left_foot = (p[:, 3] - p[:, 0]) @ facing
    right_foot = (p[:, 6] - p[:, 0]) @ facing
    windows = [float((forward[min(i + 600, len(forward) - 1)] - forward[i]) / ((min(i + 600, len(forward) - 1) - i) * DT))
               for i in range(0, len(forward) - 1, 600)]
    foot_metrics = [foot_motion(active, k, half, None if collider_rotations is None else collider_rotations[k]) for k, half in zip([3, 6], feet)]
    result = dict(survival_seconds=alive_frames * DT, distance_m=float(forward[-1]),
                  forward_speed_m_s=float(forward[-1] / seconds), slowest_window_m_s=min(windows),
                  max_lateral_deviation_m=float(np.max(np.abs(displacement @ side))),
                  arm_excursion_m=[float(np.ptp(left_arm)), float(np.ptp(right_arm))],
                  arm_opposition=correlation(left_arm, right_arm),
                  contralateral_coordination=min(correlation(left_arm, right_foot), correlation(right_arm, left_foot)),
                  foot_opposition=correlation(left_foot, right_foot),
                  head_bob_90_range_m=float(np.percentile(p[:, 12, 2], 95) - np.percentile(p[:, 12, 2], 5)),
                  feet=foot_metrics)
    checks = dict(full_minute=seconds >= 60 and not fallen.any(),
                  sustained_walking=result['forward_speed_m_s'] >= .7 and min(windows) >= .6,
                  straight_path=result['max_lateral_deviation_m'] <= 1.5,
                  arms_swing=min(result['arm_excursion_m']) >= .15 and result['arm_opposition'] <= -.5,
                  arms_coordinate_with_legs=result['contralateral_coordination'] >= .2,
                  alternating_feet=result['foot_opposition'] <= -.3,
                  feet_clear_ground=all(f['clearance_95_m'] >= .025 and f['lifted_fraction'] >= .08 for f in foot_metrics),
                  limited_sliding=all(f['contact_slip_mean_m_s'] <= .2 and f['contact_slip_95_m_s'] <= .5 for f in foot_metrics),
                  steady_head=result['head_bob_90_range_m'] <= .25)
    return dict(passed=all(checks.values()), checks=checks, **result)


def assess(out, build):
    data = (out / 'best.net.live').read_bytes()
    if len(data) < 16:
        raise ValueError('Truncated replay')
    update, count, steps, parts = struct.unpack_from('<4I', data)
    if not (1 <= count <= 64 and 2 <= steps <= 36000 and parts == 21):
        raise ValueError('Unexpected replay dimensions')
    if len(data) != 16 + 4 * count + count * steps * parts * 28:
        raise ValueError('Incomplete replay')
    frames = np.frombuffer(data, dtype='<f4', offset=16 + 4 * count).reshape(count, steps, parts, 7)
    if not np.isfinite(frames).all() or np.max(np.abs(np.linalg.norm(frames[..., 3:], axis=-1) - 1)) > .01:
        raise ValueError('Invalid native transforms')
    rows = [line.split() for line in build.read_text().splitlines() if line.startswith('Char_')]
    if len(rows) != parts:
        raise ValueError('Build does not match Homers native skeleton')
    feet = []
    for k in [3, 6]:
        i = rows[k].index('b'); half = np.array(list(map(float, rows[k][i + 1:i + 4])))
        if half.shape != (3,) or not np.isfinite(half).all() or min(half) <= 0:
            raise ValueError('Invalid native foot box')
        feet.append(half)
    calibration = json.loads((out / 'collider-frames.json').read_text())
    collider_rotations = np.array(calibration['body_to_collider_quaternions'])
    if collider_rotations.shape != (21, 4) or not np.isfinite(collider_rotations).all() or np.max(np.abs(np.linalg.norm(collider_rotations, axis=1) - 1)) > .01:
        raise ValueError('Invalid native collider calibration')
    trials = [assess_trial(trial, feet, collider_rotations) for trial in frames]
    return dict(update=update, trial_seconds=steps * DT, trials=trials,
                passes_gait_checks=count >= 8 and all(t['passed'] for t in trials),
                failures=sorted({name for trial in trials for name, passed in trial['checks'].items() if not passed}),
                limitation='Heuristic gait checks; geometric contact estimates are not force measurements. Review quality before promotion.')


def self_test():
    frames = np.zeros((120, 21, 7)); frames[..., 6] = 1.
    half = np.array([.05, .04, .13]); frames[:, 3, 2] = half[2]
    assert foot_motion(frames, 3, half)['contact_slip_mean_m_s'] < 1e-6
    frames[:, 3, 0] = np.arange(len(frames)) * DT
    assert foot_motion(frames, 3, half)['contact_slip_mean_m_s'] > .99
    frames[:, 3, 0] = 0.; frames[:, 3, 2] = half[1]
    turned = Rotation.from_euler('x', 90, degrees=True).as_quat()
    assert abs(foot_motion(frames, 3, half, turned)['clearance_95_m']) < 1e-6
    frames[:, 0, 2] = .9; frames[:, 12, 2] = 1.6; frames[:, 4, 1] = -.2
    assert not assess_trial(frames, [half, half])['passed']
    print('Gait assessment rejects sliding feet and short or stationary trials')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path); parser.add_argument('--build', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
    else:
        assert args.out and args.build
        report = assess(args.out, args.build)
        path = args.out / 'gait-assessment.json'
        path.write_text(json.dumps(report, indent=2))
        print(json.dumps(dict(update=report['update'], trial_seconds=report['trial_seconds'],
                              passes_gait_checks=report['passes_gait_checks'], failures=report['failures'], report=str(path)), indent=2))
