"""Retarget one private BVH walking cycle to Homer's existing joints. Never copies data into the public repo."""
import argparse
import hashlib
import json
import re
import struct
from pathlib import Path

import numpy as np
from scipy.signal import find_peaks
from scipy.optimize import least_squares
from scipy.ndimage import gaussian_filter1d
from scipy.spatial.transform import Rotation as R, Slerp

JOINTS = ['Hips', 'LeftUpLeg', 'LeftLeg', 'LeftFoot', 'RightUpLeg', 'RightLeg', 'RightFoot',
          'Spine', 'Spine1', 'Spine2', 'Neck', 'Neck', 'Head', 'LeftShoulder', 'LeftArm',
          'LeftForeArm', 'LeftHand', 'RightShoulder', 'RightArm', 'RightForeArm', 'RightHand']
C = np.array([[1., 0, 0], [0, 0, -1.], [0, 1., 0]])  # BVH y-up -> z-up, preserving handedness


def joint_rotation(parameters, limits, hinge):
    """Use the same Z hinge / elliptical swing and Z twist convention as the active physics joints."""
    if hinge:
        return R.from_rotvec([0., 0., parameters[0]])
    lean = np.asarray(parameters[:2]) / max(1., np.linalg.norm(parameters[:2]))
    lean = lean * np.maximum(np.array([limits[1], limits[0]]) - .04, 0.)
    return R.from_rotvec([lean[0], lean[1], 0.]) * R.from_rotvec([0., 0., parameters[2]])


def calibrated_foot_axes(rotations, neutral_rotation, neutral_toe):
    toe = np.array(neutral_toe, dtype=float, copy=True); toe[2] = 0.
    assert np.linalg.norm(toe) > 1e-6, 'Neutral foot needs a horizontal toe direction'
    toe /= np.linalg.norm(toe)
    # An ankle-to-toe bone slopes down even in a flat shoe; calibrate the shoe axes in a standing frame.
    return (rotations.apply(neutral_rotation.inv().apply(toe)),
            rotations.apply(neutral_rotation.inv().apply([0., 0., 1.])))


def retarget_geometry(points, names, parts, active, foot_axes, cyclic=True):
    """Match captured limb directions inside native joint limits, independent of the posed snapshot."""
    unit = lambda x: x / max(np.linalg.norm(x), 1e-9)
    count = len(points)
    ap = np.array([list(map(float, w[16:19])) for w in parts])
    ac = np.array([list(map(float, w[19:22])) for w in parts])
    parents = [int(w[1]) for w in parts]
    primary = {1: 2, 2: 3, 4: 5, 5: 6, 7: 8, 8: 9, 9: 10, 10: 11, 11: 12, 13: 14, 14: 15, 15: 16, 17: 18, 18: 19, 19: 20}
    rotations = [[] for _ in parts]
    fitted = [[] for _ in parts]
    previous = [None for _ in parts]
    local_right = unit(ap[4] - ap[1]); local_up = unit(ap[7] - (ap[1] + ap[4]) * .5)
    rest_p = np.array([list(map(float, w[31:34])) for w in parts])
    rest_q = R.from_quat(np.array([list(map(float, w[34:38])) for w in parts]))
    rest_forward = unit(np.cross([0., 0., 1.], rest_p[4] - rest_p[1]))
    errors = []
    for frame, xyz in enumerate(points):
        right = unit(xyz[names.index('RightUpLeg')] - xyz[names.index('LeftUpLeg')])
        up = unit(xyz[names.index('Spine')] - xyz[names.index('Hips')])
        root, _ = R.align_vectors(np.stack([right, up]), np.stack([local_right, local_up]))
        rotations[0].append(root)
        for k in range(1, len(parts)):
            parent, hinge, values = active[k]; limits = values[8:11]
            assert parent == parents[k]
            pf, cf = R.from_quat(values[:4]), R.from_quat(values[4:8])
            joint_parent = rotations[parent][-1] * pf
            source = names.index(JOINTS[k]); child = primary.get(k)
            if child is not None:
                local = unit(ap[child] - ac[k]); desired = xyz[names.index(JOINTS[child])] - xyz[source]
            else:
                local = unit(-ac[k])
                if k in [3, 6]:
                    # The feet's box Z axis runs heel to toe; its ankle-to-center vector does not.
                    local = np.array([0., 0., np.sign(rest_q[k].inv().apply(rest_forward)[2])])
                    desired = foot_axes[k][0][frame]
                elif k in [16, 20]:
                    desired = xyz[source] - xyz[names.index(JOINTS[parents[k]])]
                else:
                    desired = xyz[names.index('Head')] - xyz[names.index('Neck')]
            if np.linalg.norm(desired) < 1e-6:
                desired = up  # The native rig splits one captured neck segment into two parts.
            desired = unit(desired)
            bend_local = bend_goal = None
            if child in active and active[child][1]:
                grandchild = primary[child]
                after = unit(xyz[names.index(JOINTS[grandchild])] - xyz[names.index(JOINTS[child])])
                bend_goal = unit(np.cross(desired, after)) * (1 if active[child][2][9] > 0 else -1)
                bend_local = R.from_quat(active[child][2][:4]).apply([0., 0., 1.])
            if hinge:
                lo, hi = limits[:2]; bounds = ([lo + 1e-4], [hi - 1e-4]); initial = [(lo + hi) * .5]
            else:
                twist = max(limits[2] - .04, 1e-4)
                bounds = ([-1., -1., -twist], [1., 1., twist]); initial = [0., 0., 0.]
            initial = previous[k] if previous[k] is not None else initial
            def residual(parameters):
                world = joint_parent * joint_rotation(parameters, limits, hinge) * cf.inv()
                result = list((world.apply(local) - desired) * 5.)
                if k in [3, 6]:
                    sole = np.array([0., np.sign(rest_q[k].apply([0., 1., 0.])[2]), 0.])
                    result.extend((world.apply(sole) - foot_axes[k][1][frame]) * 5.)
                if bend_local is not None:
                    result.extend((world.apply(bend_local) - bend_goal) * 2.)
                result.extend((np.asarray(parameters) - initial) * .02)
                return result
            solved = least_squares(residual, initial, bounds=bounds, max_nfev=45)
            previous[k] = solved.x
            fitted[k].append(solved.x.copy())
            world = joint_parent * joint_rotation(solved.x, limits, hinge) * cf.inv()
            rotations[k].append(world)
            errors.append(float(np.linalg.norm(world.apply(local) - desired)))
    # Close the cycle in constrained joint coordinates; snapping the final world pose causes an arm/leg jerk.
    phase = np.linspace(0., 1., count)
    root = R.from_quat(np.array([q.as_quat() for q in rotations[0]]))
    result = [root * R.from_rotvec(phase[:, None] * (root[-1].inv() * root[0]).as_rotvec()) if cyclic else root]
    for k in range(1, len(parts)):
        parent, hinge, values = active[k]
        parameters = np.array(fitted[k])
        if cyclic:
            parameters -= phase[:, None] * (parameters[-1] - parameters[0])
            parameters = gaussian_filter1d(parameters[:-1], 1., axis=0, mode='wrap')
            parameters = np.vstack([parameters, parameters[0]])
        else:
            # A captured change of direction is not a loop: keep its final heading and pose.
            parameters = gaussian_filter1d(parameters, 1., axis=0, mode='nearest')
        if hinge:
            parameters[:, 0] = np.clip(parameters[:, 0], values[8] + 1e-4, values[9] - 1e-4)
        else:
            parameters[:, 2] = np.clip(parameters[:, 2], -values[10] + .04, values[10] - .04)
        relative = R.from_quat(np.array([joint_rotation(p, values[8:11], hinge).as_quat() for p in parameters]))
        result.append(result[parent] * R.from_quat(values[:4]) * relative * R.from_quat(values[4:8]).inv())
    return result, float(np.mean(errors))


def reference_forward(p, pelvis_rotation):
    stride = (p[-1, 0] - p[0, 0]).copy(); stride[2] = 0.
    assert np.linalg.norm(stride) > 1e-6, 'Locomotion reference must move forward'
    stride /= np.linalg.norm(stride)
    forward = (pelvis_rotation * pelvis_rotation[0].inv()).apply(np.tile(stride, (len(p), 1)))
    forward[:, 2] = 0.; forward /= np.linalg.norm(forward, axis=1, keepdims=True)
    return forward


def lateral_lean(p):
    stride = p[-1, 0] - p[0, 0]; stride[2] = 0.
    assert np.linalg.norm(stride) > 1e-6, 'Locomotion reference must move forward'
    left = np.cross([0., 0., 1.], stride / np.linalg.norm(stride))
    return {name: float(np.degrees(np.arctan2((p[:-1, k] - p[:-1, 0]) @ left,
                                             p[:-1, k, 2] - p[:-1, 0, 2])).mean())
            for name, k in [('chest', 9), ('head', 12)]}


def validate_native_walk(p, q, active, walking=True):
    for k, (parent, hinge, values) in active.items():
        relative = (q[parent] * R.from_quat(values[:4])).inv() * q[k] * R.from_quat(values[4:8])
        if hinge:
            rv = relative.as_rotvec()
            assert abs(rv[:, :2]).max() < 1e-4, f'Joint {k}: off hinge axis'
            assert rv[:, 2].min() >= values[8] - 1e-4 and rv[:, 2].max() <= values[9] + 1e-4
        else:
            z = relative.apply([0., 0., 1.]); theta = np.arccos(np.clip(z[:, 2], -1., 1.))
            axis = np.cross(np.tile([0., 0., 1.], (len(z), 1)), z)
            axis /= np.maximum(np.linalg.norm(axis, axis=1, keepdims=True), 1e-10)
            ellipse = theta * np.sqrt((axis[:, 1] / (values[8] - .03))**2 + (axis[:, 0] / (values[9] - .03))**2)
            assert ellipse.max() <= 1.0001, f'Joint {k}: swing outside native limits'
            swing = R.from_rotvec(axis * theta[:, None]); twist = (swing.inv() * relative).as_rotvec()[:, 2]
            assert abs(twist).max() <= values[10] - .03 + 1e-4, f'Joint {k}: twist outside native limits'
    # Swinging thigh centres cannot define body direction; calibrate the pelvis to the captured stride.
    forward = reference_forward(p, q[0])
    arms = np.stack([((p[:, h] - p[:, s]) * forward).sum(1) for h, s in [(16, 13), (20, 17)]], 1)
    if walking:
        assert (arms.min(0) < -.02).all() and (arms.max(0) > .02).all(), 'Both arms must swing ahead of and behind the shoulders'
    assert np.corrcoef(arms.T)[0, 1] < -.3, 'Arms must swing in opposition'
    peak_speed = max(float((rotation[:-1].inv() * rotation[1:]).magnitude().max()) * 60 for rotation in q)
    assert peak_speed < (15. if walking else 35.), 'Locomotion reference has an abrupt angular jump'
    for hand, shoulder, foot in [(16, 13, 6), (20, 17, 3)]:
        arm = ((p[:, hand] - p[:, shoulder]) * forward).sum(1)
        opposite = ((p[:, foot] - p[:, 0]) * forward).sum(1)
        assert np.corrcoef(arm, opposite)[0, 1] > .3, 'Arm swing must coordinate with the opposite leg'
    lean = lateral_lean(p)
    if not walking:
        assert max(abs(v) for v in lean.values()) < 3., 'Straight jogging reference has a persistent sideways lean'
    return dict(hand_forward_ranges=[arms.min(0).tolist(), arms.max(0).tolist()], arm_opposition=float(np.corrcoef(arms.T)[0, 1]), peak_target_angular_speed=peak_speed, lateral_lean_mean_deg=lean)


def bvh(text):
    header, data = text.split('MOTION', 1)
    names, parents, offsets, channels, stack = [], [], [], [], []
    current = None
    for line in header.splitlines():
        w = line.split()
        if not w:
            continue
        if w[0] in ('ROOT', 'JOINT', 'End'):
            names.append(w[1] if w[0] != 'End' else f'_end{len(names)}')
            parents.append(stack[-1] if stack else -1)
            offsets.append([0., 0., 0.]); channels.append([]); current = len(names) - 1
        elif w[0] == '{':
            stack.append(current)
        elif w[0] == '}':
            stack.pop()
        elif w[0] == 'OFFSET':
            offsets[current] = list(map(float, w[1:4]))
        elif w[0] == 'CHANNELS':
            channels[current] = w[2:]
            assert len(channels[current]) == int(w[1])
    lines = data.strip().splitlines()
    count, dt = int(lines[0].split(':')[1]), float(lines[1].split(':')[1])
    values = np.array([list(map(float, l.split())) for l in lines[2:] if l.strip()])
    assert values.shape == (count, sum(map(len, channels))) and np.isfinite(values).all() and dt > 0
    pos, rotations, col = [], [], 0
    for j, (parent, ch) in enumerate(zip(parents, channels)):
        p = np.tile(offsets[j], (count, 1)); q = R.identity(count)
        for channel in ch:
            v = values[:, col]; col += 1
            axis = 'XYZ'.index(channel[0])
            if channel.endswith('position'):
                p[:, axis] = v
            else:
                rv = np.zeros((count, 3)); rv[:, axis] = np.radians(v)
                q = q * R.from_rotvec(rv)
        if parent >= 0:
            p = pos[parent] + rotations[parent].apply(p)
            q = rotations[parent] * q
        pos.append(p); rotations.append(q)
    return names, np.stack(pos, 1), rotations, dt


def feet_bottom(p, q, parts):
    heights = []
    for k in [3, 6]:
        w = parts[k]
        if 'b' in w:
            i = w.index('b'); half = np.array(list(map(float, w[i + 1:i + 4])))
            radius = (np.abs(q[k].as_matrix()[:, 2, :]) * half).sum(1)
        else:
            radius = float(w[3]) + np.abs(q[k].apply([0., 1., 0.])[:, 2]) * float(w[4])
        heights.append(p[:, k, 2] - radius)
    return np.stack(heights, 1)


def prepare(source, build, out, rig=None, gait='walk', cycle=None):
    text = source.read_text(); names, positions, source_rotations, dt = bvh(text)
    assert all(j in names for j in JOINTS), 'BVH is missing a mapped body joint'
    xyz = positions @ C.T * .01
    root = xyz[:, names.index('Hips')]
    speed = np.linalg.norm(np.gradient(root[:, :2], dt, axis=0), axis=1)
    hip_right = xyz[:, names.index('RightUpLeg')] - xyz[:, names.index('LeftUpLeg')]
    fwd = np.cross([0., 0., 1.], hip_right); fwd[:, 2] = 0
    fwd /= np.linalg.norm(fwd, axis=1)[:, None]
    # Calibrate against a high, still, two-feet-down pose; no standing template is invented from walking frame zero.
    floor = np.percentile(xyz[:, [names.index('LeftFoot'), names.index('RightFoot')], 2], 1)
    hip_height = root[:, 2] - floor
    neutral = int(np.argmax(hip_height - 2 * speed))
    left = xyz[:, names.index('LeftFoot')] - root
    forward_foot = (left * fwd).sum(1)
    jogging = gait == 'jog'
    peaks, _ = find_peaks(forward_foot, distance=int((.4 if jogging else .65) / dt), prominence=.12)
    candidates = []
    for a, b in zip(peaks[:-1], peaks[1:]):
        duration = (b - a) * dt
        delta = root[b] - root[a]; delta[2] = 0
        direction = delta / max(np.linalg.norm(delta), 1e-8)
        alignment = np.min(fwd[a:b] @ direction)
        v = np.linalg.norm(delta) / duration
        low_t, high_t, low_v, high_v, preferred = (.5, 1.1, 1.7, 3.0, 2.1) if jogging else (.8, 1.6, .8, 1.9, 1.25)
        if low_t < duration < high_t and low_v < v < high_v and alignment > .97:
            candidates.append((alignment - abs(v - preferred) * .1, int(a), int(b)))
    assert candidates, f'No clean, straight human {gait} cycle found'
    if cycle is not None:
        candidates = [candidate for candidate in candidates if tuple(candidate[1:]) == tuple(cycle)]
        assert candidates, 'Requested cycle failed the existing duration/speed/straightness checks'
    _, a, b = max(candidates)
    parts = [l.split() for l in build.read_text().splitlines() if l.strip() and not l.startswith('nm ')]
    assert len(parts) == len(JOINTS)
    parents = [int(w[1]) for w in parts]
    rest_p = np.array([list(map(float, w[31:34])) for w in parts])
    ap = np.array([list(map(float, w[16:19])) for w in parts])
    ac = np.array([list(map(float, w[19:22])) for w in parts])
    target_yaw = np.arctan2(fwd[a, 1], fwd[a, 0])
    source_align = R.from_euler('z', -target_yaw)
    neutral_floor = min(rest_p[k, 2] - float(parts[k][3]) - float(parts[k][4]) for k in [3, 6])
    height = rest_p[12, 2] - neutral_floor
    source_height = xyz[neutral, names.index('Head'), 2] - floor
    scale = height / source_height
    root_motion = source_align.apply((root[a:b + 1] - root[a]) * scale)
    root_motion[:, 2] += rest_p[0, 2] - neutral_floor + (root[a, 2] - root[neutral, 2]) * scale
    root_motion[:, :2] -= root_motion[0, :2]
    times = np.arange(b - a + 1) * dt
    root_motion[:, 2] -= times / times[-1] * (root_motion[-1, 2] - root_motion[0, 2])
    period_steps = round(times[-1] * 60)
    sampled = np.linspace(0, times[-1], period_steps + 1)
    p = np.zeros((len(sampled), len(parts), 3))
    p[:, 0] = np.stack([np.interp(sampled, times, root_motion[:, d]) for d in range(3)], 1)
    active = {}
    if rig:
        for line in rig.read_text().splitlines():
            w = line.split(); active[int(w[0])] = (int(w[1]), bool(int(w[2])), list(map(float, w[3:])))
    assert len(active) == len(parts) - 1, 'Actual native joint frames are required for every non-root part'
    sampled_points = np.stack([np.interp(sampled, times, xyz[a:b+1, j, d]) for j in range(len(names)) for d in range(3)], -1).reshape(len(sampled), len(names), 3)
    sampled_points = source_align.apply(sampled_points.reshape(-1, 3)).reshape(sampled_points.shape)
    foot_axes = {}
    for k, name in [(3, 'LeftFoot'), (6, 'RightFoot')]:
        index = names.index(name); toe = names.index('LeftToe' if k == 3 else 'RightToe')
        curve = R.from_matrix(C @ source_rotations[index][a:b + 1].as_matrix() @ C.T)
        neutral_q = R.from_matrix(C @ source_rotations[index][neutral].as_matrix() @ C.T)
        axes = calibrated_foot_axes(Slerp(times, curve)(sampled), neutral_q, xyz[neutral, toe] - xyz[neutral, index])
        foot_axes[k] = tuple(source_align.apply(axis) for axis in axes)
    q, direction_error = retarget_geometry(sampled_points, names, parts, active, foot_axes)
    for k, parent in enumerate(parents):
        if parent >= 0:
            p[:, k] = p[:, parent] + q[parent].apply(ap[k]) - q[k].apply(ac[k])
    quats = np.stack([r.as_quat() for r in q], 1)
    # One constant ground alignment; per-frame root lifting would hide foot penetration and create vertical jitter.
    bottoms = feet_bottom(p, q, parts)
    p[:, :, 2] -= np.percentile(bottoms.min(1), 5)
    assert (feet_bottom(p, q, parts).min(0) < .04).all(), 'Each reference foot must reach the ground during its stride'
    validation = validate_native_walk(p, q, active, walking=not jogging)
    frames = np.concatenate([p, quats], 2).astype('<f4')
    assert np.isfinite(frames).all() and np.max(np.abs(np.linalg.norm(quats, axis=2) - 1)) < 1e-5
    assert np.allclose(quats[0], quats[-1], atol=1e-5) or np.allclose(abs(quats[0]), abs(quats[-1]), atol=1e-5)
    for k, parent in enumerate(parents):
        if parent >= 0:
            assert np.max(np.linalg.norm(p[:, parent] + q[parent].apply(ap[k]) - p[:, k] - q[k].apply(ac[k]), axis=1)) < 1e-6
    assert not out.exists(), f'Refusing to overwrite {out}'
    with out.open('wb') as f:
        f.write(b'DMOTION1'); f.write(struct.pack('<3I', 60, len(frames), len(parts)))
        for w in parts:
            name = w[0].encode(); f.write(struct.pack('<I', len(name))); f.write(name)
        f.write(frames.tobytes())
    metadata = dict(source=str(source), source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                    frames=[a, b], seconds=[a * dt, b * dt], neutral_frame=neutral, scale=scale,
                    period=sampled[-1], speed=float(np.linalg.norm(p[-1, 0, :2] - p[0, 0, :2]) / sampled[-1]),
                    foot_bottom_range=feet_bottom(p, q, parts).min(0).tolist(), native_limits_validated=True,
                    mapping=dict(zip([w[0] for w in parts], JOINTS)), limb_direction_error=direction_error,
                    foot_orientation='standing-calibrated-mocap-rotation', gait=gait, **validation)
    out.with_suffix('.json').write_text(json.dumps(metadata, indent=2))
    print(json.dumps({k: metadata[k] for k in ['frames', 'seconds', 'period', 'speed', 'foot_bottom_range']}, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('bvh', type=Path); parser.add_argument('build', type=Path); parser.add_argument('out', type=Path)
    parser.add_argument('--rig', type=Path)
    parser.add_argument('--gait', choices=['walk', 'jog'], default='walk')
    parser.add_argument('--cycle', type=int, nargs=2, metavar=('START', 'END'), help='Select an already-qualified captured cycle explicitly')
    args = parser.parse_args()
    prepare(args.bvh, args.build, args.out, args.rig, args.gait, args.cycle)
