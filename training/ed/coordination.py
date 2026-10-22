"""Human-reference calibration and native-replay coordination checks; CPU only."""
import hashlib
import struct
from pathlib import Path

import numpy as np
from scipy.spatial.transform import Rotation as R
from scipy.signal import find_peaks

TRACKING_MODES = ('tracking-reward-v1', 'mocap-v1', 'tracking-reward-v2', 'mocap-v2', 'mocap-walk20-v2', 'mocap-walk35-v2', 'mocap-walk-settings-v2', 'mocap-walk-combined-v2', 'mocap-walk-lean-v2', 'mocap-jog-contact-v2', 'mocap-jog-phase-v2', 'mocap-jog-drive-v2', 'mocap-jog-ballistic-v2', 'mocap-push-off-v2')
TRACKING_MODES += ('mocap-alignment-v2','mocap-path-v2','mocap-leg-strength-v2','mocap-leg-skill-v2','mocap-actuator-v2')
MODES = ('placement-v2', 'coordination-v2') + TRACKING_MODES


def motion(path):
    data = Path(path).read_bytes()
    if data[:8] != b'DMOTION1':
        raise ValueError('Expected native motion reference')
    rate, count, parts = struct.unpack_from('<3I', data, 8)
    offset = 20
    for _ in range(parts):
        length = struct.unpack_from('<I', data, offset)[0]
        offset += 4 + length
    if parts != 21 or not 2 <= count <= 36000 or len(data) != offset + count * parts * 28:
        raise ValueError('Invalid reference dimensions')
    return rate, np.frombuffer(data, '<f4', offset=offset).reshape(count, parts, 7).copy()


def axes(frames, hip_axis):
    side = R.from_quat(frames[:, 0, 3:]).apply(hip_axis)
    side[:, 2] = 0.
    side /= np.maximum(np.linalg.norm(side, axis=-1, keepdims=True), 1e-6)
    return side, np.cross([0., 0., 1.], side)


def targets(reference, jog_reference, build, brisk_reference=None):
    rows = [line.split() for line in Path(build).read_text().splitlines() if line.startswith('Char_')]
    hip = np.array(rows[4][16:19], float) - np.array(rows[1][16:19], float)
    hip /= np.linalg.norm(hip)
    # The observation exposes root Z. Up cross root Z approximates anatomical
    # -Y for this upright rig; prior replay audit found >99.5% crossing agreement.
    if np.linalg.norm(hip - [0., -1., 0.]) >= .001:
        raise ValueError('Coordination observation projection requires Homers calibrated hip axis')
    result = dict(hip_axis=hip.tolist(), walking_target_speed_m_s=1.5,
                  minimum_lateral_foot_separation_m=.08, minimum_swing_excursion_m=.12)
    sources = [('walk', reference), ('jog', jog_reference)]
    if brisk_reference:
        sources.append(('brisk', brisk_reference))
    for label, path in sources:
        rate, frames = motion(path)
        _, forward = axes(frames, hip)
        p = frames[..., :3]
        velocity = np.gradient(p - p[:, :1], 1 / rate, axis=0)
        gain = []
        for hand, foot in [(16, 6), (20, 3)]:
            arm = np.sum(velocity[:, hand] * forward, axis=-1)
            leg = np.sum(velocity[:, foot] * forward, axis=-1)
            value = float(np.dot(arm, leg) / max(np.dot(leg, leg), 1e-9))
            if not .05 <= value <= 1.:
                raise ValueError('Reference lacks usable contralateral arm/leg timing')
            gain.append(value)
        result[label + '_arm_velocity_gain'] = gain
        result[label + '_sha256'] = hashlib.sha256(Path(path).read_bytes()).hexdigest()
        result[label + '_reference_speed_m_s'] = float(np.linalg.norm(p[-1, 0, :2] - p[0, 0, :2]) / ((len(p) - 1) / rate))
    result['build_sha256'] = hashlib.sha256(Path(build).read_bytes()).hexdigest()
    return result


def observation_cost(packet, profile, arm_timing, match_speed=True, reference_placement=False):
    body = packet[..., 4:193].reshape(*packet.shape[:-1], 21, 9)
    axis = packet[..., 1:4]
    side = np.stack([-axis[..., 1], axis[..., 0], np.zeros_like(axis[..., 0])], axis=-1)
    norm = np.linalg.norm(side, axis=-1, keepdims=True)
    side /= np.maximum(norm, 1e-6)
    forward = np.stack([-side[..., 1], side[..., 0], np.zeros_like(side[..., 0])], axis=-1)
    width = np.sum((body[..., 6, :3] - body[..., 3, :3]) * side, axis=-1)
    shortfall = np.maximum(profile['minimum_lateral_foot_separation_m'] - width, 0.)
    cost = .12 * np.minimum(shortfall / .16, 2.) ** 2
    if reference_placement:
        # Native tracking now compares actual/reference foot spacing at the same phase.
        cost = np.where(packet[...,311] > 1.05,0.,cost)
    beta = np.clip(packet[..., 311], 0., 1.)
    speed_target = (1 - beta) * profile['walking_target_speed_m_s'] + beta * profile['jog_reference_speed_m_s']
    if match_speed:
        cost += .04 * np.minimum((packet[..., 316 + 2] - speed_target) ** 2, 4.)
    if arm_timing:
        velocity = body[..., 3:6] * 5.  # learn::body_obs scales relative velocities by .2.
        arm = np.sum(velocity[..., [16, 20], :] * forward[..., None, :], axis=-1)
        leg = np.sum(velocity[..., [6, 3], :] * forward[..., None, :], axis=-1)
        if 'brisk_arm_velocity_gain' in profile:
            jog = np.clip(packet[..., 311]-1., 0., 1.)
            gain = ((1-beta[...,None])*np.array(profile['walk_arm_velocity_gain'])
                    +(beta-jog)[...,None]*np.array(profile['brisk_arm_velocity_gain'])
                    +jog[...,None]*np.array(profile['jog_arm_velocity_gain']))
        else:
            gain = (1 - beta[..., None]) * np.array(profile['walk_arm_velocity_gain']) + beta[..., None] * np.array(profile['jog_arm_velocity_gain'])
        cost += .04 * np.mean(np.minimum((arm - gain * leg) ** 2, 4.), axis=-1)
    return np.where((norm[..., 0] > .5) & (packet[..., 0] > .5), cost, 0.)


def replay_metrics(path, profile, expected_update):
    data = Path(path).read_bytes()
    update, count, steps, parts = struct.unpack_from('<4I', data)
    if update != expected_update or count != 8 or steps != 3600 or parts != 21 or len(data) != 16 + 4 * count + count * steps * parts * 28:
        raise ValueError('Coordination metrics must match eight full native evaluation recordings')
    frames = np.frombuffer(data, '<f4', offset=16 + 4 * count).reshape(count, steps, parts, 7)
    if not np.isfinite(frames).all():
        raise ValueError('Nonfinite replay')
    widths, errors, excursions, walking_speeds, walking_cycles, startup_tilts = [], [], [], [], [], []
    for i, trial in enumerate(frames):
        side, forward = axes(trial, profile['hip_axis'])
        p = trial[..., :3]
        left = -side[0]
        startup_tilts.append([float(np.degrees(np.arctan2((p[60:420,k]-p[60:420,0])@left,
            p[60:420,k,2]-p[60:420,0,2])).mean()) for k in [9,12]])
        width = np.sum((p[:, 6] - p[:, 3]) * side, axis=-1)
        relative = p - p[:, :1]
        velocity = np.gradient(relative, 1 / 60, axis=0)
        arm = np.sum(velocity[:, [16, 20]] * forward[:, None], axis=-1)
        leg = np.sum(velocity[:, [6, 3]] * forward[:, None], axis=-1)
        bridge = 'brisk_arm_velocity_gain' in profile
        windows = [(2, 58)] if i % 2 else ([(14,20),(28,40)] if bridge else [(14, 26), (50, 58)])
        gain = np.array(profile['walk_arm_velocity_gain'] if i % 2 else profile['jog_arm_velocity_gain'])
        if bridge and not i % 2:
            from gait_curriculum import weights
            w,b,j = weights(np.arange(steps)/60., profile['jog_limit'])
            gain = (w[:,None]*np.array(profile['walk_arm_velocity_gain'])
                    +b[:,None]*np.array(profile['brisk_arm_velocity_gain'])
                    +j[:,None]*np.array(profile['jog_arm_velocity_gain']))
        for start, end in windows:
            window = slice(start * 60, end * 60)
            widths.append(width[window])
            errors.append((arm[window] - (gain[window] if gain.ndim == 2 else gain) * leg[window]) ** 2)
            for k in [3, 6]:
                foot = np.sum(relative[window, k] * forward[window], axis=-1)
                excursions.append(float(np.percentile(foot, 95) - np.percentile(foot, 5)))
            if i % 2:
                distance = np.sum((p[end * 60 - 1, 0] - p[start * 60, 0]) * forward[start * 60])
                walking_speeds.append(float(distance / (end - start)))
                separation = np.sum((p[window, 6] - p[window, 3]) * forward[window], axis=-1)
                peaks, _ = find_peaks(separation, distance=21, prominence=.15)
                periods = np.diff(peaks) / 60
                walking_cycles.extend(periods[(periods > .4) & (periods < 2.5)].tolist())
    width = np.concatenate(widths)
    return dict(crossing_fraction=float(np.mean(width < -.01)),
                near_crossing_cost=float(np.mean(np.minimum(np.maximum(.08 - width, 0.) / .16, 2.) ** 2)),
                arm_velocity_rmse_m_s=float(np.sqrt(np.mean(np.concatenate(errors)))),
                walking_speed_m_s=float(np.mean(walking_speeds)), minimum_swing_excursion_m=min(excursions),
                walking_cycle_seconds=float(np.median(walking_cycles)) if walking_cycles else None,
                startup_lateral_mean_deg=np.mean(startup_tilts,axis=0).tolist())


def score(metrics):
    return (metrics['near_crossing_cost'] + .15 * metrics['arm_velocity_rmse_m_s']
            + .10 * abs(metrics['walking_speed_m_s'] - 1.5))


def improved(evaluation, anchor, metrics, anchor_metrics, previous):
    e, a = np.asarray(evaluation), np.asarray(anchor)
    if e.shape != (8, 6) or a.shape != e.shape or not np.isfinite(e).all():
        raise ValueError('Invalid coordination evaluation')
    if np.any(e[:, 5] < 60 - 1/60) or np.any(e[:, 3] < np.maximum(.7, .90 * a[:, 3])):
        return False
    if (e[:, 0].mean() > 1.05 * a[:, 0].mean() or e[:, 2].mean() > 1.20 * a[:, 2].mean() + .002
            or e[:, 1].mean() > 1.10 * a[:, 1].mean() or metrics['minimum_swing_excursion_m'] < .12):
        return False
    if (metrics['crossing_fraction'] > anchor_metrics['crossing_fraction'] + .01
            or metrics['arm_velocity_rmse_m_s'] > 1.10 * anchor_metrics['arm_velocity_rmse_m_s'] + .02):
        return False
    # Explicit new-stage anchors may protect measured stride gains; old checkpoints keep their checks.
    if 'walking_cycle_bounds_s' in anchor_metrics:
        cycle = metrics.get('walking_cycle_seconds')
        low, high = anchor_metrics['walking_cycle_bounds_s']
        if cycle is None or not low <= cycle <= high:
            return False
    if metrics['minimum_swing_excursion_m'] < anchor_metrics.get('minimum_preserved_swing_excursion_m', .12):
        return False
    if 'maximum_startup_lateral_error_deg' in anchor_metrics:
        actual = np.asarray(metrics.get('startup_lateral_mean_deg', [np.nan, np.nan]))
        target = np.asarray(anchor_metrics['startup_lateral_target_deg'])
        if not np.isfinite(actual).all() or np.sqrt(np.mean((actual-target)**2)) > anchor_metrics['maximum_startup_lateral_error_deg']:
            return False
    return previous is None or score(metrics) < score(previous) - .005
