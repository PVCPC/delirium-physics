"""CPU transfer trial of the public DeepMimic walk/run policies through Delirium's real muscles."""
import os
os.environ['OPENBLAS_NUM_THREADS'] = '1'
os.environ['OMP_NUM_THREADS'] = '1'
import argparse
import collections
import inspect
import json
import struct
import subprocess
from pathlib import Path

import numpy as np
from scipy.spatial.transform import Rotation as R, Slerp
from prepare_walk import C, feet_bottom, retarget_geometry, validate_native_walk

CONVERT = R.from_matrix(C)
# Native part -> pretrained body; the extra spine/clavicle links retain calibrated offsets.
MAP = np.array([0, 9, 10, 11, 3, 4, 5, 1, 1, 1, 1, 2, 2, 1, 12, 13, 14, 1, 6, 7, 8])
SENSE = np.array([0, 9, 12, 4, 5, 6, 18, 19, 20, 1, 2, 3, 14, 15, 16])


def weights(assets, mode):
    output = assets / f'{mode}-actor.npz'
    if not output.exists():
        # The small legacy reader needs Python 3.12/NumPy 2 aliases and binary positional reads on Windows.
        os.environ['PROTOCOL_BUFFERS_PYTHON_IMPLEMENTATION'] = 'python'
        np.string_ = np.bytes_; np.unicode_ = np.str_
        inspect.ArgSpec = collections.namedtuple('ArgSpec', 'args varargs keywords defaults')
        inspect.getargspec = lambda f: inspect.ArgSpec(*inspect.getfullargspec(f)[:4])
        if os.name == 'nt':
            import msvcrt
            def pread(fd, size, offset):
                msvcrt.setmode(fd, os.O_BINARY); saved = os.lseek(fd, 0, os.SEEK_CUR)
                try:
                    os.lseek(fd, offset, os.SEEK_SET); return os.read(fd, size)
                finally:
                    os.lseek(fd, saved, os.SEEK_SET)
            os.pread = pread
        from tensorflow_checkpoint_reader import py_checkpoint_reader
        reader = py_checkpoint_reader.NewCheckpointReader(str(assets / f'data/policies/humanoid3d/humanoid3d_{mode}.ckpt'))
        paths = {f'{kind}{i}': f'agent/main/actor/{layer}/{name}' for i, layer in enumerate(['0/dense', '1/dense', 'dist_gauss_diag/mean']) for kind, name in [('w', 'kernel'), ('b', 'bias')]}
        paths.update({f'{kind}_{stat}': f'agent/resource/{kind}_norm/{stat}' for kind in ['s', 'a'] for stat in ['mean', 'std']})
        arrays = {name: np.array(reader.get_tensor(path), dtype=np.float32, copy=True) for name, path in paths.items()}
        assert arrays['w0'].shape == (227, 1024) and arrays['w1'].shape == (1024, 512) and arrays['w2'].shape == (512, 28)
        assert all(np.isfinite(x).all() for x in arrays.values())
        assert (arrays['s_std'] > 0).all() and (arrays['a_std'] > 0).all()
        np.savez(output, **arrays)
    return dict(np.load(output, allow_pickle=False))


def actor(weights, observation):
    x = (np.asarray(observation, dtype=np.float32) - weights['s_mean']) / weights['s_std']
    x = np.maximum(x @ weights['w0'] + weights['b0'], 0)
    x = np.maximum(x @ weights['w1'] + weights['b1'], 0)
    return (x @ weights['w2'] + weights['b2']) * weights['a_std'] + weights['a_mean']


class Teacher:
    def __init__(self, assets, mode):
        self.weights = weights(assets, mode)
        rig = json.loads((assets / 'data/characters/humanoid3d.txt').read_text())
        self.joints, self.bodies = rig['Skeleton']['Joints'], rig['BodyDefs']
        assert len(self.joints) == len(self.bodies) == 15
        assert all(not j['AttachTheta' + d] for j in self.joints + self.bodies for d in 'XYZ'), 'Trial supports this published rig, not arbitrary joint frames'
        self.attach = np.array([[j['Attach' + d] for d in 'XYZ'] for j in self.joints])
        self.body_attach = np.array([[b['Attach' + d] for d in 'XYZ'] for b in self.bodies])
        self.frames = np.array(json.loads((assets / f'data/motions/humanoid3d_{mode}.txt').read_text())['Frames'])
        self.times = np.r_[0., np.cumsum(self.frames[:-1, 0])]
        self.period = self.times[-1]
        self.local = []; offset = 8
        self.root_pos = self.frames[:, 1:4]
        self.local.append(R.from_quat(self.frames[:, [5, 6, 7, 4]]))
        for joint in self.joints[1:]:
            if joint['Type'] == 'spherical':
                self.local.append(R.from_quat(self.frames[:, [offset + 1, offset + 2, offset + 3, offset]])); offset += 4
            elif joint['Type'] == 'revolute':
                rv = np.zeros((len(self.frames), 3)); rv[:, 2] = self.frames[:, offset]
                self.local.append(R.from_rotvec(rv)); offset += 1
            else:
                self.local.append(R.identity(len(self.frames)))
        assert offset == self.frames.shape[1]

    def fk(self, local, root):
        positions = [np.array(root)]; rotations = [local[0]]
        for k, joint in enumerate(self.joints[1:], 1):
            parent = joint['Parent']
            positions.append(positions[parent] + rotations[parent].apply(self.attach[k]))
            rotations.append(rotations[parent] * local[k])
        body = np.array([p + q.apply(offset) for p, q, offset in zip(positions, rotations, self.body_attach)])
        return np.array(positions), R.from_quat(np.array([q.as_quat() for q in rotations])), body

    def reference(self, time):
        t = np.clip(time, 0, self.period)
        local = [Slerp(self.times, q)([t])[0] for q in self.local]
        root = np.array([np.interp(t, self.times, self.root_pos[:, d]) for d in range(3)])
        return self.fk(local, root)

    def targets(self, action, root_q, root_p):
        local = [root_q]; offset = 0
        for joint in self.joints[1:]:
            if joint['Type'] == 'spherical':
                rv = action[offset:offset + 3]; rv = rv * min(1., 2 * np.pi / max(np.linalg.norm(rv), 1e-9))
                local.append(R.from_rotvec(rv)); offset += 3
            elif joint['Type'] == 'revolute':
                local.append(R.from_rotvec([0., 0., action[offset]])); offset += 1
            else:
                local.append(R.identity())
        assert offset == len(action)
        return self.fk(local, root_p)[1]


def prepare_reference(teacher, parts, active, output, mode):
    if output.exists():
        return read_motion(output)
    names = ['Hips', 'LeftUpLeg', 'LeftLeg', 'LeftFoot', 'LeftToe', 'RightUpLeg', 'RightLeg', 'RightFoot', 'RightToe',
             'Spine', 'Spine1', 'Spine2', 'Neck', 'Head', 'LeftShoulder', 'LeftArm', 'LeftForeArm', 'LeftHand', 'RightShoulder', 'RightArm', 'RightForeArm', 'RightHand']
    samples = np.linspace(0, teacher.period, round(teacher.period * 60) + 1)
    points = []; roots = []
    for t in samples:
        joints, q, _ = teacher.reference(t); roots.append(joints[0])
        values = [joints[0], joints[9], joints[10], joints[11], joints[11] + q[11].apply([.16, 0, 0]),
                  joints[3], joints[4], joints[5], joints[5] + q[5].apply([.16, 0, 0]),
                  joints[0] * .6 + joints[1] * .4, joints[0] * .25 + joints[1] * .75, joints[1], joints[2], joints[2] + q[2].apply([0, .15, 0]),
                  joints[1] + q[1].apply([0, teacher.attach[12, 1], 0]), joints[12], joints[13], joints[14],
                  joints[1] + q[1].apply([0, teacher.attach[6, 1], 0]), joints[6], joints[7], joints[8]]
        points.append(np.array(values) @ C.T)
    q, error = retarget_geometry(np.array(points), names, parts, active)
    ap = np.array([list(map(float, w[16:19])) for w in parts]); ac = np.array([list(map(float, w[19:22])) for w in parts])
    native_leg = np.linalg.norm(ap[2] - ac[1]) + np.linalg.norm(ap[3] - ac[2])
    source_leg = np.linalg.norm(teacher.attach[10]) + np.linalg.norm(teacher.attach[11])
    positions = np.zeros((len(samples), 21, 3)); positions[:, 0] = np.array(roots) @ C.T * native_leg / source_leg
    positions[:, 0, :2] -= positions[0, 0, :2]
    positions[:, 0, 2] -= samples / samples[-1] * (positions[-1, 0, 2] - positions[0, 0, 2])
    for k in range(1, 21):
        parent = int(parts[k][1]); positions[:, k] = positions[:, parent] + q[parent].apply(ap[k]) - q[k].apply(ac[k])
    positions[:, :, 2] -= np.percentile(feet_bottom(positions, q, parts).min(1), 5)
    validation = validate_native_walk(positions, q, active, walking=mode == 'walk')
    frames = np.concatenate([positions, np.stack([rotation.as_quat() for rotation in q], 1)], 2).astype('<f4')
    with output.open('xb') as file:
        file.write(b'DMOTION1' + struct.pack('<3I', 60, len(frames), 21))
        for part in parts:
            name = part[0].encode(); file.write(struct.pack('<I', len(name)) + name)
        file.write(frames.tobytes())
    output.with_suffix('.json').write_text(json.dumps(dict(source=f'DeepMimic humanoid3d_{mode}', direction_error=error, **validation), indent=2))
    return frames


def read_motion(path):
    raw = path.read_bytes(); fps, count, parts = struct.unpack_from('<3I', raw, 8); offset = 20
    assert raw[:8] == b'DMOTION1' and fps == 60 and parts == 21
    for _ in range(parts):
        size, = struct.unpack_from('<I', raw, offset); offset += 4 + size
    frames = np.frombuffer(raw, dtype='<f4', offset=offset).reshape(count, parts, 7).copy()
    assert np.isfinite(frames).all()
    return frames


class Adapter:
    def __init__(self, teacher, native, active):
        self.teacher, self.active = teacher, active
        _, source_q, source_p = teacher.reference(0)
        native_q = R.from_quat(native[0, :, 3:])
        self.offset = native_q.inv() * CONVERT * source_q[MAP]
        self.scale = native[0, 0, 2] / source_p[0, 1]
        self.period = (len(native) - 1) / 60

    def proxy(self, state):
        body = state[4:].reshape(21, 13)
        q = CONVERT.inv() * R.from_quat(body[SENSE, 3:7]) * self.offset[SENSE]
        positions = [CONVERT.inv().apply(body[0, :3]) / self.scale - q[0].apply(self.teacher.body_attach[0])]
        for k, joint in enumerate(self.teacher.joints[1:], 1):
            parent = joint['Parent']; positions.append(positions[parent] + q[parent].apply(self.teacher.attach[k]))
        p = np.array(positions) + q.apply(self.teacher.body_attach)
        return p, q

    def observation(self, state, previous=None):
        p, q = self.proxy(state)
        root_joint = p[0] - q[0].apply(self.teacher.body_attach[0])
        direction = q[0].apply([1., 0., 0.]); heading = R.from_euler('y', -np.arctan2(-direction[2], direction[0]))
        rotations = R.from_quat(np.vstack([q[0].as_quat(), (heading * q[1:]).as_quat()]))
        pose = np.c_[heading.apply(p - root_joint), rotations.apply(np.tile([0., 1., 0.], (15, 1))), rotations.apply(np.tile([1., 0., 0.], (15, 1)))].reshape(-1)
        body = state[4:].reshape(21, 13)
        if previous is not None and state[0] > previous[0]:
            old_p, old_q = self.proxy(previous); dt = state[0] - previous[0]
            v = (p - old_p) / dt; w = (q * old_q.inv()).as_rotvec() / dt
        else:
            v = CONVERT.inv().apply(body[SENSE, 7:10]) / self.scale; w = CONVERT.inv().apply(body[SENSE, 10:13])
        v[1:] = heading.apply(v[1:]); w[1:] = heading.apply(w[1:])
        observation = np.r_[(state[0] / self.period) % 1., root_joint[1], pose, np.c_[v, w].reshape(-1)].astype(np.float32)
        assert observation.shape == (227,) and np.isfinite(observation).all()
        return observation

    def target(self, state, previous=None):
        action = actor(self.teacher.weights, self.observation(state, previous))
        assert action.shape == (28,) and np.isfinite(action).all()
        p, q = self.proxy(state)
        desired = self.teacher.targets(action, q[0], p[0] - q[0].apply(self.teacher.body_attach[0]))
        native = list(CONVERT * desired[MAP] * self.offset.inv())
        # Enforce the recipient's real limits rather than asking its muscles for unreachable angles.
        for k, (parent, hinge, values) in self.active.items():
            pf, cf = R.from_quat(values[:4]), R.from_quat(values[4:8])
            rel = (native[parent] * pf).inv() * native[k] * cf
            if hinge:
                quat = rel.as_quat(); angle = (2 * np.arctan2(quat[2], quat[3]) + np.pi) % (2*np.pi) - np.pi
                rel = R.from_rotvec([0, 0, np.clip(angle, values[8], values[9])])
            else:
                z = rel.apply([0, 0, 1]); theta = np.arccos(np.clip(z[2], -1, 1)); axis = np.cross([0, 0, 1], z)
                axis /= max(np.linalg.norm(axis), 1e-10)
                twist = (R.from_rotvec(axis * theta).inv() * rel).as_rotvec()[2]
                ratio = theta * np.sqrt((axis[1] / (values[8] - .04))**2 + (axis[0] / (values[9] - .04))**2)
                rel = R.from_rotvec(axis * theta / max(1., ratio)) * R.from_rotvec([0, 0, np.clip(twist, -values[10]+.04, values[10]-.04)])
            native[k] = native[parent] * pf * rel * cf.inv()
        return np.array([q.as_quat() for q in native], dtype='<f4')


class Environment:
    def __init__(self, exe, motion, balance, log):
        self.log = log.open('ab')
        self.process = subprocess.Popen([str(exe), '--pretrained-env', str(motion)] + (['--pretrained-balance'] if balance else []), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log)
        header = self.process.stdout.readline().decode().split()
        assert header == ['PRETRAINENV1', '21', '277'], f'Native bridge unavailable: {header}'

    def read(self, count):
        data = bytearray()
        while len(data) < count * 4:
            block = self.process.stdout.read(count * 4 - len(data))
            if not block: raise RuntimeError(f'Native bridge exited: {self.process.poll()}')
            data.extend(block)
        result = np.frombuffer(data, dtype='<f4').copy(); assert np.isfinite(result).all()
        return result

    def reset(self, phase):
        self.process.stdin.write(b'\x02' + struct.pack('<f', phase)); self.process.stdin.flush()
        return self.read(277)

    def step(self, target):
        assert target.shape == (21, 4) and np.isfinite(target).all()
        self.process.stdin.write(b'\x01' + target.astype('<f4').tobytes()); self.process.stdin.flush()
        return self.read(554).reshape(2, 277)

    def close(self):
        try:
            self.process.stdin.write(b'\x03'); self.process.stdin.flush(); self.process.stdin.close()
        except (OSError, ValueError): pass
        try: self.process.wait(timeout=15)
        except subprocess.TimeoutExpired: self.process.terminate(); self.process.wait(timeout=15)
        self.log.close()


def reference_target(native, time):
    phase = (time * 60) % (len(native) - 1); index = int(phase); alpha = phase - index
    a, b = R.from_quat(native[index, :, 3:]), R.from_quat(native[index+1, :, 3:])
    return (a * R.from_rotvec((a.inv() * b).as_rotvec() * alpha)).as_quat().astype('<f4')


def save_replay(path, recordings, scores):
    data = struct.pack('<4I', 0, len(recordings), 480, 21) + np.asarray(scores, dtype='<f4').tobytes() + np.asarray(recordings, dtype='<f4').tobytes()
    temporary = path.with_suffix(path.suffix + '.tmp'); temporary.write_bytes(data); os.replace(temporary, path)


def main(args):
    parts = [line.split() for line in args.build.read_text().splitlines() if line.strip() and not line.startswith('nm ')]
    active = {}
    for line in args.rig.read_text().splitlines():
        w = line.split(); active[int(w[0])] = (int(w[1]), bool(int(w[2])), np.array(list(map(float, w[3:]))))
    results = []
    for mode in ['walk', 'run']:
        teacher = Teacher(args.assets, mode)
        motion = args.assets / f'retargeted-{mode}.motion'
        native = prepare_reference(teacher, parts, active, motion, mode)
        adapter = Adapter(teacher, native, active)
        synthetic = np.zeros(277, dtype=np.float32); synthetic[4:] = np.c_[native[0], np.zeros((21, 6))].reshape(-1)
        proxy_p, proxy_q = adapter.proxy(synthetic); source_j, source_q, source_p = teacher.reference(0.)
        np.testing.assert_allclose((proxy_q.inv() * source_q).magnitude(), 0., atol=1e-6)
        proxy_root = proxy_p[0] - proxy_q[0].apply(teacher.body_attach[0])
        np.testing.assert_allclose(proxy_p - proxy_root, source_p - source_j[0], atol=1e-6)
        observation = adapter.observation(synthetic)
        assert adapter.target(synthetic).shape == (21, 4)
        # Independent Torch CPU forward verifies the converted normalization and layer orientation.
        import torch
        torch.set_num_threads(1)
        x = (torch.from_numpy(observation) - torch.from_numpy(teacher.weights['s_mean'])) / torch.from_numpy(teacher.weights['s_std'])
        for i in range(3):
            x = x @ torch.from_numpy(teacher.weights[f'w{i}']) + torch.from_numpy(teacher.weights[f'b{i}'])
            if i < 2: x = torch.relu(x)
        x = x * torch.from_numpy(teacher.weights['a_std']) + torch.from_numpy(teacher.weights['a_mean'])
        np.testing.assert_allclose(x.numpy(), actor(teacher.weights, observation), atol=1e-4, rtol=1e-4)
        print(f'{mode}: real actor loaded; CPU forward agrees; native reference fitted', flush=True)
        if args.prepare_only: continue
        best = -float('inf')
        for balance in [False, True]:
            env = Environment(args.exe.resolve(), motion.resolve(), balance, args.assets / 'native-trial.log')
            try:
                for policy in ['reference', 'pretrained']:
                    recordings, scores, trials = [], [], []
                    for phase in np.arange(args.phases) / args.phases * adapter.period:
                        state = env.reset(float(phase)); start = state[4:7].copy(); previous = None; recording = []; reward = 0.
                        for _ in range(240):
                            target = adapter.target(state, previous) if policy == 'pretrained' else reference_target(native, state[0] + 1/60)
                            packet = env.step(target); previous = state.copy(); state = packet[-1]
                            for row in packet:
                                if len(recording) < int(row[1]):
                                    recording.append(row[4:].reshape(21, 13)[:, :7].copy()); reward += float(row[2])
                            if state[3]: break
                        survival = float(state[1]) / 60; distance = float(state[4] - start[0]); score = reward / 480
                        last = recording[-1]; recording.extend([last] * (480 - len(recording)))
                        recordings.append(recording); scores.append(score); trials.append(dict(survival=survival, distance=distance, speed=distance/max(survival,1/60), score=score))
                    result = dict(mode=mode, policy=policy, balance=balance, trials=trials, mean_survival=float(np.mean([t['survival'] for t in trials])), mean_distance=float(np.mean([t['distance'] for t in trials])), mean_score=float(np.mean(scores)))
                    results.append(result); print(json.dumps(result), flush=True)
                    if policy == 'pretrained' and result['mean_score'] > best:
                        best = result['mean_score']; save_replay(args.output / f'pretrained-{mode}.net.live', recordings, scores)
            finally: env.close()
    if results: (args.assets / 'transfer-results.json').write_text(json.dumps(results, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--assets', type=Path, required=True); parser.add_argument('--build', type=Path, required=True); parser.add_argument('--rig', type=Path, required=True)
    parser.add_argument('--exe', type=Path); parser.add_argument('--output', type=Path, default=Path.cwd()); parser.add_argument('--phases', type=int, default=4)
    parser.add_argument('--prepare-only', action='store_true')
    args = parser.parse_args(); assert 1 <= args.phases <= 8 and (args.prepare_only or args.exe)
    main(args)
