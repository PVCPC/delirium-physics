"""CPU PPO for Delirium's native human-motion environment. Models and captured motion stay private."""
import argparse
import json
import os
import shutil
import struct
import subprocess
import tempfile
import time
from pathlib import Path

import numpy as np
import torch
from torch import nn
from torch.distributions import Normal
from live_training import LivePreview
import coordination

REWARD_CONTRACT = 'native-assisted-pelvis-heading-v3'


def network(inputs, outputs, actor=False):
    net = nn.Sequential(nn.Linear(inputs, 128), nn.Tanh(), nn.Linear(128, 128), nn.Tanh(), nn.Linear(128, outputs))
    for layer in net:
        if isinstance(layer, nn.Linear):
            nn.init.orthogonal_(layer.weight, 1.0); nn.init.zeros_(layer.bias)
    if actor:
        nn.init.zeros_(net[-1].weight)
        net.append(nn.Tanh())  # Same three tanh layers as Rust learn::Net.
    return net


def replace_saved_file(source, destination):
    # Windows readers can briefly deny replacement; keep the old complete file until they release it.
    for attempt in range(20):
        try:
            os.replace(source, destination)
            return
        except PermissionError:
            if attempt == 19:
                raise
            time.sleep(.05)


def export(net, path):
    layers = [l for l in net if isinstance(l, nn.Linear)]
    sizes = [layers[0].in_features] + [l.out_features for l in layers]
    weights = np.concatenate([a.detach().cpu().numpy().reshape(-1) for l in layers for a in [l.weight, l.bias]]).astype('<f4')
    assert np.isfinite(weights).all()
    tmp = path.with_suffix(path.suffix + '.tmp')
    with tmp.open('wb') as f:
        f.write(struct.pack('<4I', *sizes)); f.write(weights.tobytes())
    replace_saved_file(tmp, path)


def publish_best(out, model):
    for source, destination in [(out / 'best.net', model), (out / 'best.net.live', Path(str(model) + '.live'))]:
        if destination.exists() and source.read_bytes() == destination.read_bytes():
            continue
        temporary = Path(str(destination) + '.publish-tmp')
        shutil.copyfile(source, temporary); replace_saved_file(temporary, destination)


def eligible_walk(evaluation, seconds):
    return bool(np.all(evaluation[:, 5] >= seconds - .01) and np.mean(evaluation[:, 3]) >= .7)


def transition_progress(evaluation, seconds):
    walking, mixed = evaluation[1::2], evaluation[::2]
    if not eligible_walk(walking, seconds) or np.min(mixed[:, 3]) < .7:
        return None
    return np.array([mixed[:, 5].mean(), (mixed[:, 3] * mixed[:, 5]).mean()])


def transition_improved(current, previous):
    return current is not None and (previous is None or bool(np.any(current >= previous + [.25, .5])))


def save_json(path, value):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(value, indent=2))
    replace_saved_file(temporary, path)


class Native:
    def __init__(self, exe, reference, model, count, log, behaviors=False, assisted=False, jog_reference=None, tracking=None, brisk_reference=None, jog_limit=1., jog_stiffness=12.):
        self.log = log.open('ab')
        self.process = subprocess.Popen([str(exe), '--motion-env', str(reference), str(model), str(count)] +
                                        (['--motion-behaviors'] if behaviors else []) + (['--walking-assisted'] if assisted else []) +
                                        (['--jog-reference', str(jog_reference)] if jog_reference else []),
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log,
                                        env=dict(os.environ, DLM_MOCAP_TRACKING=tracking or 'off',
                                                 DLM_BRISK_REFERENCE=str(brisk_reference) if brisk_reference else '', DLM_JOG_LIMIT=str(jog_limit),DLM_JOG_STIFFNESS=str(jog_stiffness)))
        self.brisk_reference = brisk_reference
        header = self.process.stdout.readline().decode().strip().split()
        protocol = ('MOTIONTRACK2' if tracking.endswith('-v2') else 'MOTIONTRACK1') if tracking else 'MOTIONENV2'
        if len(header) != 7 or header[0] != protocol:
            self.close(); raise RuntimeError(f'Invalid native protocol header: {header}; see {log}')
        self.count, self.obs, self.actions, self.metrics, steps, evaluation_steps = map(int, header[1:])
        assert self.count == count and self.metrics == 6
        assert 0 < steps <= evaluation_steps <= 36000
        self.episode_seconds, self.evaluation_seconds = steps / 60, evaluation_steps / 60
        expected_actions = (50 if tracking=='mocap-actuator-v2' else 48 if tracking in ('mocap-leg-strength-v2','mocap-leg-skill-v2') else 46) if jog_reference else 28
        if assisted and jog_reference and tracking=='mocap-actuator-v2' and os.environ.get('DLM_ANKLE_RESIDUAL')=='1':expected_actions+=4
        if assisted and jog_reference and tracking=='mocap-actuator-v2' and os.environ.get('DLM_WHOLE_BODY_TURN')=='1':expected_actions+=36
        if assisted and jog_reference and os.environ.get('DLM_TURN_JOINT_CONTROL')=='1':expected_actions+=14
        assert self.actions == expected_actions if assisted else ((self.actions > 60) if behaviors else (self.actions == 60))
        self.process.stdin.write(b'\x04'); self.process.stdin.flush()
        length = struct.unpack('<I', self.bytes(4))[0]
        assert 0 < length < 1_000_000, 'Invalid behavior catalog length'
        self.catalog = self.bytes(length).decode('utf-8')
        self.process.stdin.write(b'\x05'); self.process.stdin.flush()
        self.collider_rotations = self.read(21 * 4).reshape(21, 4)
        assert np.max(np.abs(np.linalg.norm(self.collider_rotations, axis=1) - 1)) < .01

    def bytes(self, size):
        data = bytearray()
        while len(data) < size:
            block = self.process.stdout.read(size - len(data))
            if not block:
                raise RuntimeError(f'Native environment stopped, exit {self.process.poll()}; inspect native log')
            data.extend(block)
        return data

    def read(self, count):
        result = np.frombuffer(self.bytes(count * 4), dtype='<f4').copy()
        assert np.isfinite(result).all(), 'Nonfinite native observation or metric'
        return result

    def rollout(self, actor, std, noise):
        # The whole horizon in the native workers (their own copy of the actor), no per-step round trips.
        layers = [l for l in actor if isinstance(l, nn.Linear)]
        w = np.concatenate([a.detach().numpy().reshape(-1) for l in layers for a in [l.weight, l.bias]] + [std.detach().numpy().reshape(-1), noise.numpy().reshape(-1)]).astype('<f4')
        self.process.stdin.write(b'\x06' + np.array([noise.shape[0], layers[0].out_features], dtype='<f4').tobytes() + w.tobytes())
        self.process.stdin.flush()
        return self.read(noise.shape[0] * self.count * (self.actions + self.obs + 2 + self.metrics)).reshape(noise.shape[0], self.count, -1)

    def step(self, action=None):
        if action is None:
            self.process.stdin.write(b'\x00')
        else:
            a = np.asarray(action, dtype='<f4')
            assert a.shape == (self.count, self.actions) and np.isfinite(a).all()
            self.process.stdin.write(b'\x01' + a.tobytes())
        self.process.stdin.flush()
        return self.read(self.count * (self.obs + 2 + self.metrics)).reshape(self.count, -1)

    def evaluate(self, generation):
        self.process.stdin.write(b'\x02' + struct.pack('<I', generation)); self.process.stdin.flush()
        return self.read(8 * self.metrics).reshape(8, self.metrics)

    def set_jog_limit(self, limit):
        assert self.brisk_reference and np.isfinite(limit) and 0. <= limit <= 1.
        self.process.stdin.write(b'\x07' + struct.pack('<f', limit)); self.process.stdin.flush()
        assert abs(float(self.read(1)[0])-limit) < 1e-6

    def close(self):
        if hasattr(self, 'process'):
            try:
                self.process.stdin.write(b'\x03'); self.process.stdin.flush(); self.process.stdin.close()
            except (BrokenPipeError, OSError, ValueError):
                pass
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.process.terminate(); self.process.wait(timeout=15)
        self.log.close()


def self_test():
    torch.manual_seed(7)
    a = network(5, 3, True)
    with torch.no_grad():
        a[-2].weight.normal_(0, .03)
    x = torch.randn(4, 5)
    actual = a(x).detach().numpy()
    value = x.numpy()
    for l in a:
        if isinstance(l, nn.Linear):
            value = value @ l.weight.detach().numpy().T + l.bias.detach().numpy()
        else:
            value = np.tanh(value)
    np.testing.assert_allclose(actual, value, atol=1e-6)
    with torch.inference_mode():
        sampled = Normal(a(x), .2).sample()
    stacked = torch.stack([sampled, sampled])
    assert not stacked.is_inference()
    Normal(a(x), .2).log_prob(stacked[0]).mean().backward()
    assert all(p.grad is not None and torch.isfinite(p.grad).all() for p in a.parameters())
    a.zero_grad()
    assert not torch.cuda.is_initialized()
    evaluation = np.zeros((8, 6)); evaluation[:, 3] = .8; evaluation[:, 5] = 60
    assert eligible_walk(evaluation, 60)
    evaluation[0, 5] = 8
    assert not eligible_walk(evaluation, 60)
    directory = Path(tempfile.mkdtemp(prefix='delirium-publish-check-'))
    from unittest.mock import patch
    original_replace = os.replace
    attempts = 0
    def briefly_locked(source, destination):
        nonlocal attempts
        attempts += 1
        if attempts <= 2:
            raise PermissionError('reader still holds the destination')
        original_replace(source, destination)
    saved_json = directory / 'progress.json'
    save_json(saved_json, {'update': 1})
    with patch('os.replace', side_effect=briefly_locked), patch('time.sleep') as sleep:
        save_json(saved_json, {'update': 2})
        assert attempts == 3 and sleep.call_count == 2
    assert json.loads(saved_json.read_text())['update'] == 2
    with patch('os.replace', side_effect=PermissionError('permanent denial')), patch('time.sleep'):
        try:
            save_json(saved_json, {'update': 3})
        except PermissionError:
            pass
        else:
            raise AssertionError('Permanent save failures must remain visible')
    assert json.loads(saved_json.read_text())['update'] == 2
    for name in ['best.net', 'best.net.live']:
        (directory / name).write_bytes(b'first')
    model = directory / 'published.net'
    publish_best(directory, model)
    stamp = 1_600_000_000_000_000_000
    for path in [model, Path(str(model) + '.live')]:
        os.utime(path, ns=(stamp, stamp))
    publish_best(directory, model)
    assert model.stat().st_mtime_ns == stamp
    assert Path(str(model) + '.live').stat().st_mtime_ns == stamp
    (directory / 'best.net.live').write_bytes(b'new replay')
    publish_best(directory, model)
    assert Path(str(model) + '.live').read_bytes() == b'new replay'
    # Keep test artifacts; this workspace never deletes files without permission.
    print('CPU actor, eligibility, locked-file recovery and unchanged-replay timestamp checks passed', flush=True)


def learn_batch(actor, critic, log_std, optimizer, parameters, observations, actions, old_logp, rewards, dones, values, next_value, envs):
    horizon, gamma, lam = 64, .99, .95
    learning_start = time.perf_counter()
    with torch.no_grad():
        advantage = torch.zeros_like(rewards); carry = torch.zeros(envs)
        for t in reversed(range(horizon)):
            following = next_value if t == horizon - 1 else values[t + 1]
            delta = rewards[t] + gamma * following * (1 - dones[t]) - values[t]
            carry = delta + gamma * lam * (1 - dones[t]) * carry; advantage[t] = carry
        returns = advantage + values
        advantage = (advantage - advantage.mean()) / (advantage.std() + 1e-8)
    x = observations.flatten(0, 1); act = actions.flatten(0, 1); old = old_logp.flatten()
    adv, ret = advantage.flatten(), returns.flatten()
    kl = torch.tensor(0.)
    for epoch in range(4):
        for indices in torch.randperm(len(x)).split(256):
            dist = Normal(actor(x[indices]), log_std.clamp(-4, -.5).exp())
            new = dist.log_prob(act[indices]).sum(-1); ratio = (new - old[indices]).exp()
            policy = -torch.minimum(ratio * adv[indices], ratio.clamp(.8, 1.2) * adv[indices]).mean()
            value_loss = .5 * (critic(x[indices]).squeeze(-1) - ret[indices]).square().mean()
            loss = policy + value_loss - .001 * dist.entropy().sum(-1).mean()
            optimizer.zero_grad(); loss.backward(); nn.utils.clip_grad_norm_(parameters, .5); optimizer.step()
            with torch.no_grad():
                kl = ((ratio - 1) - (new - old[indices])).mean()
            if kl > .04:
                break
        if kl > .04:
            break
    return dict(kl=float(kl), learning=time.perf_counter() - learning_start)


def learner_main(actor, critic, log_std, optimizer_state, learner_rng, inq, outq, envs):
    import traceback
    try:
        torch.set_num_threads(4); torch.set_num_interop_threads(1); torch.manual_seed(18)
        parameters = list(actor.parameters()) + list(critic.parameters()) + [log_std]
        optimizer = torch.optim.Adam(parameters, lr=1e-4)
        optimizer.load_state_dict(optimizer_state)
        if learner_rng is not None:
            torch.set_rng_state(learner_rng)
        outq.put(dict(ready=True))
        while True:
            command, payload = inq.get()
            if command == 'stop':
                return
            if command == 'state':
                outq.put(dict(optimizer=optimizer.state_dict(), rng=torch.get_rng_state()))
            elif command == 'learn':
                outq.put(learn_batch(actor, critic, log_std, optimizer, parameters, *payload, envs))
            else:
                raise ValueError(f'Unknown learner command: {command}')
    except BaseException:
        outq.put(dict(error=traceback.format_exc()))


def receive_learner(proc, outq):
    import queue
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        try:
            result = outq.get(timeout=.2)
        except queue.Empty:
            if not proc.is_alive():
                raise RuntimeError(f'Learner exited unexpectedly: {proc.exitcode}')
            continue
        if 'error' in result:
            raise RuntimeError(result['error'])
        return result
    raise TimeoutError('Learner did not respond within 60 seconds')


def train(args):
    torch.set_num_threads(4); torch.set_num_interop_threads(1); torch.manual_seed(17)
    assert torch.version.cuda is None, 'Use the isolated CPU-only torch environment'
    device = torch.device('cpu')
    args.out.mkdir(exist_ok=True, parents=True)
    model = args.model.resolve(); reference = args.reference.resolve()
    jog_reference = args.jog_reference.resolve() if args.jog_reference else None
    assert not jog_reference or (args.assisted and not args.behaviors), 'Transitions require native-assisted locomotion'
    reward_contract = 'native-assisted-walk-jog-aligned-v3' if jog_reference else REWARD_CONTRACT
    marker = model.with_suffix('.path')
    if marker.exists() and Path(marker.read_text().strip()).resolve() != reference:
        raise ValueError('Existing model uses a different reference; choose a new model name')
    marker.write_text(str(reference))
    assert not (args.assisted and args.behaviors), 'Walking keeps balance active; skill switching is a later stage'
    evaluation_model = (args.out / 'candidate.net').resolve() if args.assisted else model
    env = Native(args.exe.resolve(), reference, evaluation_model, args.envs, args.out / 'native.log', args.behaviors, args.assisted, jog_reference)
    preview = None
    proc = None
    try:
        preview = LivePreview(args.preview_port, args.out) if args.preview_port else None
        save_json(args.out / 'collider-frames.json', dict(body_to_collider_quaternions=env.collider_rotations.tolist()))
        catalog_file = model.with_suffix('.tools.tsv')
        if catalog_file.exists() and catalog_file.read_text(encoding='utf-8') != env.catalog:
            raise ValueError('Delirium tool registry changed; use a new model name to avoid reassigning learned actions')
        catalog_file.write_text(env.catalog, encoding='utf-8', newline='')
        actor, critic = network(env.obs, env.actions, True).to(device), network(env.obs, 1).to(device)
        log_std = nn.Parameter(torch.full((env.actions,), -2.0, device=device))
        parameters = list(actor.parameters()) + list(critic.parameters()) + [log_std]
        optimizer = torch.optim.Adam(parameters, lr=1e-4)
        checkpoint = args.out / 'ppo.pt'
        first = 0
        if checkpoint.exists():
            saved = torch.load(checkpoint, map_location='cpu', weights_only=True)
            if saved.get('training_pipeline') == 'ppo-rust-v1':
                raise ValueError('Resume this checkpoint with --rust-learner to preserve Rust optimizer/RNG state')
            assert saved['reference'] == str(reference) and saved['obs'] == env.obs
            assert saved.get('actions', env.actions) == env.actions
            assert saved.get('catalog', env.catalog) == env.catalog
            assert saved.get('assisted', False) == args.assisted
            assert saved.get('episode_seconds', env.episode_seconds) == env.episode_seconds
            assert saved.get('evaluation_seconds', env.evaluation_seconds) == env.evaluation_seconds
            assert saved.get('jog_reference') == (str(jog_reference) if jog_reference else None), 'Jog reference changed; use a new output directory'
            assert not jog_reference or saved.get('reward_contract') == reward_contract, 'Jog controller changed; use a new output directory'
            if saved.get('training_pipeline') == 'ppo-overlap-v1' and not args.overlap_learner:
                raise ValueError('Resume this checkpoint with --overlap-learner to preserve its training pipeline')
            actor.load_state_dict(saved['actor']); critic.load_state_dict(saved['critic'])
            log_std.data.copy_(saved['log_std']); optimizer.load_state_dict(saved['optimizer'])
            first = saved['update'] + 1
        elif model.exists() and not args.seed:
            raise ValueError('Model already exists without this PPO checkpoint; refusing to overwrite it')
        elif args.seed:
            saved = torch.load(args.seed, map_location='cpu', weights_only=True)
            assert saved['reference'] == str(reference), 'Seed uses a different reference motion'
            for net, source in [(actor, saved['actor']), (critic, saved['critic'])]:
                with torch.no_grad():
                    for name, value in net.state_dict().items():
                        old = source[name]
                        assert old.ndim == value.ndim and all(a <= b for a, b in zip(old.shape, value.shape))
                        value.zero_(); value[tuple(slice(0, n) for n in old.shape)].copy_(old)
            print(f'Warm start from {args.seed}; continuing with the current trial durations', flush=True)
            if jog_reference:
                with torch.no_grad():
                    log_std[:len(saved['log_std'])].copy_(saved['log_std'])
        if first == 0 and args.behaviors:
            with torch.no_grad():
                actor[-2].bias[60] = float(np.arctanh(.25))
        export(actor, evaluation_model)
        controls = '4 native walking settings + 24 arm corrections; native leg targets preserved' if args.assisted else f'60 joint corrections, {env.actions - 60} behavior choices'
        if jog_reference:
            controls = '4 balance settings + 24 arm and 18 leg corrections; 50% walking rehearsal, 50% walk/jog transitions'
        print(f'Human walking: CPU PPO, {args.envs} environments, {env.obs} inputs, {controls}; reference {reference}', flush=True)
        print(f'Full Delirium message/parameter catalog: {catalog_file}', flush=True)
        print(f'Training trials {env.episode_seconds:g}s; saved evaluation trials {env.evaluation_seconds:g}s', flush=True)
        status = dict(status='running', pid=os.getpid(), native_pid=env.process.pid, target_updates=args.updates,
                      episode_seconds=env.episode_seconds, evaluation_seconds=env.evaluation_seconds, reference=str(reference),
                      model=str(model), out=str(args.out.resolve()), cpu_only=True, updated_at_unix=time.time(),
                      evaluation_interval_updates=args.eval_every)
        pipeline = 'ppo-overlap-v1' if args.overlap_learner else 'ppo-sequential-v1'
        status['training_pipeline'] = pipeline
        status['reward_contract'] = reward_contract
        if jog_reference:
            status.update(stage='walk-jog', jog_reference=str(jog_reference), curriculum=dict(walk_seconds=[0,8], ramp_up=[8,12], jog=[12,28], ramp_down=[28,32], walk_again=[32,44], second_ramp=[44,48], jog_again=[48,60], rehearsal_fraction=.5, reference_state_initialization=True))
        save_json(args.out / 'status.json', status)
        print('Evaluation columns: pose error rad, stance slip m/s, action jitter, forward speed m/s, walking score, survival seconds', flush=True)
        if preview:
            preview.send('evaluating', first, force=True)
        initial = env.evaluate(first)
        print('initial evaluation:', np.round(initial.mean(0), 4).tolist(), flush=True)
        obs = torch.from_numpy(env.step()[:, :env.obs].copy())
        horizon, gamma, lam = 64, .99, .95
        best_file = args.out / 'best.json'
        if best_file.exists() and json.loads(best_file.read_text()).get('reward_contract') != reward_contract:
            previous = json.loads(best_file.read_text())
            save_json(args.out / 'best-before-contact-calibration.json', previous)
            shutil.copyfile(args.out / 'best.net', evaluation_model)
            rescored = env.evaluate(previous['update'])
            previous.update(metrics=rescored.mean(0).tolist(), evaluation_seconds=env.evaluation_seconds,
                            eligible=eligible_walk(rescored, env.evaluation_seconds), reward_contract=reward_contract)
            save_json(best_file, previous)
            shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best.net.live')
            export(actor, evaluation_model)
            print('Re-evaluated saved best using calibrated native foot colliders', flush=True)
        best = json.loads(best_file.read_text())['metrics'][4] if best_file.exists() else float(initial[:, 4].mean())
        best_update = json.loads(best_file.read_text())['update'] if best_file.exists() else first
        transition_file = args.out / 'transition-best.json'
        transition_best_update, transition_highwater = best_update, None
        if jog_reference:
            if transition_file.exists():
                transition_record = json.loads(transition_file.read_text())
                transition_best_update = transition_record['update']
                transition_highwater = np.array(transition_record['highwater']) if transition_record['highwater'] is not None else None
            else:
                transition_best_update = max(first - 1, 0)
                transition_highwater = transition_progress(initial, env.evaluation_seconds)
                save_json(transition_file, dict(update=transition_best_update,
                          highwater=transition_highwater.tolist() if transition_highwater is not None else None))
                export(actor, args.out / 'best-transition.net')
                shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best-transition.net.live')
        if not best_file.exists():
            export(actor, args.out / 'best.net')
            shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best.net.live')
            save_json(best_file, dict(update=first, metrics=initial.mean(0).tolist(), reference=str(reference),
                                     evaluation_seconds=env.evaluation_seconds, eligible=eligible_walk(initial, env.evaluation_seconds), reward_contract=reward_contract))
        if args.assisted:
            publish_best(args.out, model)
        choices = ['referenceMotors'] + list(dict.fromkeys(line.split('\t')[0] for line in env.catalog.splitlines()[1:]
                                                        if line.split('\t')[1].startswith('selectable;')))
        if args.behaviors:
            assert len(choices) == env.actions - 60, 'Behavior action mapping does not match the registry'
        stop_reason = 'update_limit'
        update = first - 1
        timing_window = []
        import copy
        import torch.multiprocessing as mp
        waiting = False
        snapshot_update = completed_update = first - 1
        roll_actor, roll_critic, roll_std = actor, critic, log_std
        if args.overlap_learner:
            # One update of policy lag is intentional; keep at most one learner batch outstanding.
            roll_actor, roll_critic = copy.deepcopy(actor), copy.deepcopy(critic)
            roll_std = log_std.detach().clone()
            actor.share_memory(); critic.share_memory(); log_std.data.share_memory_()
            ctx = mp.get_context('spawn'); inq, outq = ctx.Queue(), ctx.Queue()
            proc = ctx.Process(target=learner_main,
                               args=(actor, critic, log_std, optimizer.state_dict(), saved.get('learner_rng') if checkpoint.exists() else None, inq, outq, args.envs), daemon=True)
            proc.start()
            assert receive_learner(proc, outq)['ready']
            torch.set_num_threads(1)
        for update in range(first, args.updates):
            start = time.monotonic()
            timing = dict(native_rollout=0., policy=0.)
            observations, actions, probabilities, rewards, dones, values, metrics = [], [], [], [], [], [], []
            means = []
            with torch.inference_mode():
                rollout_std = roll_std.clamp(-4, -.5).exp()
            if args.native_rollout:
                tick = time.perf_counter()
                block = torch.from_numpy(env.rollout(roll_actor, rollout_std, torch.randn(horizon, args.envs, env.actions)))
                timing['native_rollout'] += time.perf_counter() - tick
                rows = block[..., env.actions:]
                observations = [obs] + list(rows[:-1, :, :env.obs]); actions = list(block[..., :env.actions])
                rewards = list(rows[..., env.obs]); dones = list(rows[..., env.obs + 1]); metrics = list(rows[..., env.obs + 2:].numpy())
                obs = rows[-1, :, :env.obs].contiguous()
                tick = time.perf_counter()
                with torch.inference_mode():
                    means = list(roll_actor(torch.stack(observations)))
                timing['policy'] += time.perf_counter() - tick
                if preview:
                    preview.replay(rows[:, 0].numpy(), env.obs, update)
            for _ in range(0 if args.native_rollout else horizon):
                tick = time.perf_counter()
                with torch.inference_mode():
                    if args.overlap_learner:
                        mean = roll_actor(obs); action = mean + rollout_std * torch.randn(mean.shape)
                        means.append(mean); logp = value = None
                    else:
                        dist = Normal(actor(obs), rollout_std)
                        action = dist.sample(); logp = dist.log_prob(action).sum(-1)
                        value = critic(obs).squeeze(-1)
                timing['policy'] += time.perf_counter() - tick
                tick = time.perf_counter()
                packet = env.step(action.numpy())
                timing['native_rollout'] += time.perf_counter() - tick
                if preview:
                    preview.step(packet[0], env.obs, update)
                observations.append(obs); actions.append(action); probabilities.append(logp); values.append(value)
                rewards.append(torch.from_numpy(packet[:, env.obs].copy()))
                dones.append(torch.from_numpy(packet[:, env.obs + 1].copy()))
                metrics.append(packet[:, env.obs + 2:])
                obs = torch.from_numpy(packet[:, :env.obs].copy())
            if preview:
                preview.send('learning', update, force=True)
            tick = time.perf_counter()
            with torch.inference_mode():
                if args.overlap_learner or args.native_rollout:
                    probabilities = Normal(torch.stack(means), rollout_std).log_prob(torch.stack(actions)).sum(-1)
                    values = roll_critic(torch.stack(observations)).squeeze(-1)
                else:
                    probabilities = torch.stack(probabilities); values = torch.stack(values)
                next_value = roll_critic(obs).squeeze(-1)
            timing['policy'] += time.perf_counter() - tick
            rewards = torch.stack(rewards)
            batch = (torch.stack(observations), torch.stack(actions), probabilities, rewards,
                     torch.stack(dones), values, next_value)
            policy_lag = 0
            if args.overlap_learner:
                timing['learning'] = 0.; kl = 0.
                if waiting:
                    result = receive_learner(proc, outq)
                    timing['learning'] = result['learning']; kl = result['kl']
                    completed_update = update - 1
                policy_lag = completed_update - snapshot_update
                assert 0 <= policy_lag <= 1, policy_lag
                with torch.no_grad():
                    roll_actor.load_state_dict(actor.state_dict()); roll_critic.load_state_dict(critic.state_dict())
                    roll_std.copy_(log_std)
                snapshot_update = completed_update
                inq.put(('learn', batch)); waiting = True
            else:
                result = learn_batch(actor, critic, log_std, optimizer, parameters, *batch, args.envs)
                timing['learning'] = result['learning']; kl = result['kl']
                completed_update = update
            timing['update_before_evaluation'] = time.monotonic() - start
            timing_window.append(timing)
            timing_window = timing_window[-20:]
            m = np.mean(metrics, axis=(0, 1))
            print(f'update {update:4} reward {rewards.mean():.4f} pose {m[0]:.3f} slip {m[1]:.3f} jitter {m[2]:.4f} speed {m[3]:.3f} KL {kl:.4f} {time.monotonic()-start:.1f}s', flush=True)
            if update % args.eval_every == args.eval_every - 1 or update + 1 == args.updates:
                evaluation_start = time.perf_counter()
                if waiting:
                    receive_learner(proc, outq); waiting = False
                    completed_update = update
                if args.overlap_learner:
                    with torch.no_grad():
                        roll_actor.load_state_dict(actor.state_dict()); roll_critic.load_state_dict(critic.state_dict())
                        roll_std.copy_(log_std)
                    snapshot_update = completed_update
                export(actor, evaluation_model)
                if preview:
                    preview.send('evaluating', update, force=True)
                evaluation = env.evaluate(update)
                evaluation_seconds = time.perf_counter() - evaluation_start
                average = evaluation.mean(0)
                print(f'  evaluated {update}: {np.round(average, 4).tolist()}', flush=True)
                eligible = not args.assisted or eligible_walk(evaluation, env.evaluation_seconds)
                if jog_reference:
                    current_transition = transition_progress(evaluation, env.evaluation_seconds)
                    if transition_improved(current_transition, transition_highwater):
                        transition_highwater = current_transition if transition_highwater is None else np.maximum(transition_highwater, current_transition)
                        transition_best_update = update
                        save_json(transition_file, dict(update=update, highwater=transition_highwater.tolist()))
                        export(actor, args.out / 'best-transition.net')
                        shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best-transition.net.live')
                if average[4] > best and (eligible or not json.loads(best_file.read_text()).get('eligible', False)):
                    best = average[4]; best_update = update; export(actor, args.out / 'best.net')
                    shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best.net.live')
                    save_json(best_file, dict(update=update, metrics=average.tolist(), reference=str(reference),
                                             evaluation_seconds=env.evaluation_seconds, eligible=eligible, reward_contract=reward_contract))
                    if args.assisted:
                        publish_best(args.out, model)
                checkpoint_start = time.perf_counter()
                learner_state = dict(rng=None)
                if args.overlap_learner:
                    inq.put(('state', None))
                    learner_state = receive_learner(proc, outq)
                    optimizer.load_state_dict(learner_state['optimizer'])
                temporary = checkpoint.with_suffix('.tmp')
                torch.save(dict(actor=actor.state_dict(), critic=critic.state_dict(), log_std=log_std.detach(),
                                optimizer=optimizer.state_dict(), update=update, learner_rng=learner_state['rng'], learner_update=completed_update, policy_lag=policy_lag, training_pipeline=pipeline, reference=str(reference), obs=env.obs,
                                actions=env.actions, catalog=env.catalog, assisted=args.assisted,
                                episode_seconds=env.episode_seconds, evaluation_seconds=env.evaluation_seconds,
                                reward_contract=reward_contract, jog_reference=str(jog_reference) if jog_reference else None), temporary)
                replace_saved_file(temporary, checkpoint)
                checkpoint_seconds = time.perf_counter() - checkpoint_start
                progress = dict(update=update, best_update=best_update, updates_since_best=update - best_update,
                                evaluation=dict(zip(['pose_error_rad', 'stance_slip_m_s', 'action_jitter', 'forward_speed_m_s',
                                                     'walking_score', 'survival_seconds'], average.tolist())),
                                forward_distance_m=float(np.mean(evaluation[:, 3] * evaluation[:, 5])),
                                best_walking_score=float(best), saved_at_unix=time.time(), cpu_only=True)
                progress['timings_ms'] = {name: float(np.mean([row[name] for row in timing_window]) * 1000)
                                         for name in timing}
                progress['training_pipeline'] = pipeline
                progress['learner_update'] = completed_update
                progress['policy_lag'] = policy_lag
                progress['timing_window_updates'] = len(timing_window)
                progress['evaluation_ms'] = evaluation_seconds * 1000
                progress['checkpoint_write_ms'] = checkpoint_seconds * 1000
                progress['evaluation_interval_updates'] = args.eval_every
                progress['trainer'] = 'native-assisted-long-walk-v3' if args.assisted else 'reference-tracker'
                if jog_reference:
                    progress['trainer'] = 'native-assisted-walk-jog-aligned-v3'
                    progress['evaluation_trials'] = evaluation.tolist()
                    progress['transition_best_update'] = transition_best_update
                    progress['updates_since_transition_best'] = update - transition_best_update
                    progress['transition_highwater'] = (dict(zip(['mean_survival_seconds', 'forward_distance_m'], transition_highwater.tolist()))
                                                       if transition_highwater is not None else None)
                    progress['plateau_metric'] = 'moving transition survival/distance with full walking rehearsal'
                progress['status'] = 'running'
                progress['episode_seconds'] = env.episode_seconds
                progress['evaluation_seconds'] = env.evaluation_seconds
                progress['target_updates'] = args.updates
                progress['reward_contract'] = reward_contract
                progress['candidate_eligible'] = bool(eligible)
                progress['best_evaluation'] = json.loads(best_file.read_text())['metrics']
                if args.behaviors:
                    counts = observations[:, :, 325:325 + len(choices)].mean((0, 1)).numpy()
                    progress['behavior_usage'] = dict(zip(choices, counts.tolist()))
                    top = sorted(progress['behavior_usage'].items(), key=lambda pair: -pair[1])[:3]
                    print('  behaviors: ' + ', '.join(f'{name} {fraction:.0%}' for name, fraction in top), flush=True)
                save_json(args.out / 'progress.json', progress)
                if args.assisted and update - (transition_best_update if jog_reference else best_update) >= 10000:
                    stop_reason = 'plateau'; break
        status.update(status='completed', stop_reason=stop_reason, updated_at_unix=time.time())
        if preview:
            preview.send('completed', update, force=True)
        save_json(args.out / 'status.json', status)
        if (args.out / 'progress.json').exists():
            progress = json.loads((args.out / 'progress.json').read_text())
            progress.update(status='completed', stop_reason=stop_reason, saved_at_unix=time.time())
            save_json(args.out / 'progress.json', progress)
    except BaseException as error:
        if preview:
            preview.send('failed', 0, force=True)
        save_json(args.out / 'status.json', dict(status='failed', error=str(error), updated_at_unix=time.time()))
        raise
    finally:
        if proc is not None:
            if proc.is_alive():
                inq.put(('stop', None))
            proc.join(timeout=5)
            if proc.is_alive():
                proc.terminate(); proc.join(timeout=5)
        if preview:
            preview.close()
        env.close()


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--exe', type=Path); p.add_argument('--reference', type=Path); p.add_argument('--model', type=Path)
    p.add_argument('--out', type=Path); p.add_argument('--updates', type=int, default=1000); p.add_argument('--envs', type=int, default=28)
    p.add_argument('--self-test', action='store_true')
    p.add_argument('--native-rollout', action='store_true', help='Plain mode: run each update rollout inside the native workers')
    p.add_argument('--overlap-learner', action='store_true', help='Experimental CPU learner process; permits one update of policy lag')
    p.add_argument('--rust-learner', type=Path, help='RLS2 CPU learner executable; requires rollout support in --exe')
    p.add_argument('--jog-adaptive-features', action='store_true', help='Protected actuator experiment: let jogging features adapt while native walking teacher retains control')
    p.add_argument('--jog-leg-authority', type=float, default=.25, help='Jog-only leg correction fraction before existing prior bound; explicit checkpoint fork required')
    p.add_argument('--mixed-jog-practice', action='store_true', help='Alternate current curriculum blend and full jogging in short skill practice')
    p.add_argument('--jog-leg-policy-loss', action='store_true', help='PPO likelihood/KL/gradients exclude28 outputs replaced by the preserved native teacher')
    p.add_argument('--reference-foot-placement', action='store_true', help='Replace fixed8cm jogging foot-spacing penalty with phase-matched reference spacing')
    p.add_argument('--transition-prior',type=Path,help='Immutable333/50 source actor for zero-initialized brisk-transition leg corrections and four-reset rehearsal')
    p.add_argument('--walk-prior',type=Path,help='Immutable333/50 source actor for bounded leg adaptation to a repaired walking reference')
    p.add_argument('--brisk-support',type=float,default=0.,help='Extend contact-aware leg support into brisk walking, from0(default) to1')
    p.add_argument('--foot-knee-tracking',action='store_true',help='Reference-relative sole, knee-joint alignment and knee-velocity rewards')
    p.add_argument('--balance-prior',type=Path,help='Immutable333/50 actor centering bounded native step/balance adaptation; arms remain protected')
    p.add_argument('--ankle-prior',type=Path,help='Immutable333/50 actor centering bounded ankle-only motor feedback')
    p.add_argument('--straight-route',action='store_true',help='Align accumulated reference travel with the requested route independently of initial pelvis phase')
    p.add_argument('--walk-reference-authority',type=float,default=.35,help='Walking leg target share from reference,0.35(default) to0.8; native balance support remains active')
    p.add_argument('--native-support-observations',action='store_true',help='Expose actual foot contacts and native support-leg intent to the learned controller')
    p.add_argument('--phase-support',type=float,default=0.,help='Experimental walking-only native gravity-support retiming,0(default) to0.25')
    p.add_argument('--stance-reward',action='store_true',help='Training-only grounded-stance quality credit, gated by forward motion and reference contact')
    p.add_argument('--native-leg-ownership',type=float,default=0.,help='Experimental walking-only engine leg target/support handoff, 0 (legacy) to 1; requires an explicit checkpoint fork')
    p.add_argument('--contact-fidelity',action='store_true',help='Training-only phase/contact quality multiplier; explicit protected-actuator checkpoint fork')
    p.add_argument('--walking-expert',type=Path,help='Retain immutable333/50 walking expert; blend to learner before brisk/jog, explicit checkpoint fork')
    p.add_argument('--jog-native-ownership',type=float,default=0.,help='Experimental genuine-jog-phase native leg/support handoff,0..0.25 scaled by jog reference fraction')
    p.add_argument('--long-transition-practice',action='store_true',help='Training: three full standing-to-walk/brisk/jog sequences per short RSI jog skill; evaluation unchanged')
    p.add_argument('--jog-route-gain',type=float,default=1.,help='Jog-only native directional feedback multiplier,1..2; explicit checkpoint fork')
    p.add_argument('--control-aware-policy-loss',action='store_true',help='Exclude expert-owned pure-walking rows from actor loss; retain critic learning')
    p.add_argument('--ankle-residual',action='store_true',help='Learn four bounded ankle corrections while preserving the complete existing50-output gait policy')
    p.add_argument('--ankle-fidelity',action='store_true',help='Training-only phase-matched foot yaw/roll fidelity for the frozen-gait ankle learner')
    p.add_argument('--leg-ankle-learning',action='store_true',help='Co-adapt student-controlled leg and ankle actions; retain the walking expert and native upper-body controller')
    p.add_argument('--turn-preview-feedback',action='store_true',help='Append planned heading and velocity at0.25/0.5seconds so the policy can anticipate the turn')
    p.add_argument('--turn-entry-control',action='store_true',help='Retain native body-turn messages while tapering push and braking during the cut')
    p.add_argument('--turn-route-anchor',action='store_true',help='Place the task reference once at actual turn entry; never moves the physical body')
    p.add_argument('--turn-signal-reward',action='store_true',help='Training-only bounded turn tracking without collapsing all errors to one reward floor; evaluations unchanged')
    p.add_argument('--whole-body-turn',action='store_true',help='Add36 bounded spine/shoulder/arm residual motors during finite jog/turn trials; retained walking expert')
    p.add_argument('--torso-reference-gain',type=float,default=0.,help='Bounded mocap spine motor baseline beneath learned whole-body residuals; explicit experimental fork')
    p.add_argument('--walking-ankle-expert',type=Path,help='Retain validated four walking ankle outputs; learner retains brisk/jog/turn ankle authority')
    p.add_argument('--jog-stop',action='store_true',help='Finite jog-to-controlled-stop task with physical braking and balanced hold; explicit full-state fork')
    p.add_argument('--turn-tempo-practice',action='store_true',help='Training-only mixture of3s/2.5s/captured cuts; standing evaluations keep the original sharp cut')
    p.add_argument('--cut-push-release',type=float,default=0.,help='Release the independent constant WASD propulsion only16..26s; native body turning and reference-speed control remain active')
    p.add_argument('--turn-joint-control',action='store_true',help='Add14 bounded hip/knee/ankle motor corrections during the finite cut; native muscles and support remain active')
    p.add_argument('--reference-balance-reset',action='store_true',help='Activate native balance after reset-only mocap placement; preserve standing evaluations and calibrated posture')
    p.add_argument('--turn-duration',type=float,default=52/30,help='Captured-cut curriculum duration,52/30..3seconds; explicit full-state fork')
    p.add_argument('--turn-reference',type=Path,help='Finite60-second walk/jog/captured-turn trajectory; experimental heading-aware CPU curriculum')
    p.add_argument('--foot-state-feedback',action='store_true',help='Append actual foot angular velocities and floor contacts while retaining the legacy333 observation prefix')
    p.add_argument('--learning-rate', type=float, default=1e-4, help='Rust CPU learner step size; changing it requires an explicit full-state fork')
    p.add_argument('--gait-refinement', choices=['slip-v1', 'slip-arm-v1', *coordination.MODES], help='Private Rust CPU gait-quality reward experiment; use a separate checkpoint fork')
    p.add_argument('--behaviors', action='store_true', help='Enable native behavior selection alongside reference joint motors')
    p.add_argument('--assisted', action='store_true', help='Improve native WASD walking; preserve leg targets and drive captured arms')
    p.add_argument('--seed', type=Path, help='Warm start a new model from a compatible smaller PPO checkpoint')
    p.add_argument('--preview-port', type=int, default=0, help='Optional loopback UDP port for the read-only browser monitor')
    p.add_argument('--eval-every', type=int, default=25, help='Updates between full evaluations and saved checkpoints')
    p.add_argument('--transition-patience', action='store_true', help='New jog curriculum: meaningful transition gains extend patience; final gait acceptance stays unchanged')
    p.add_argument('--jog-reference', type=Path, help='Captured jogging cycle for walk/jog transitions and walking rehearsal')
    p.add_argument('--brisk-reference', type=Path, help='Middle captured gait for a three-reference curriculum')
    p.add_argument('--jog-limit', type=float, default=.15, help='Initial genuine-jog fraction; checkpoint state takes precedence on resume')
    p.add_argument('--jog-stiffness', type=float, default=12., help='Jog-drive experiment leg motor strength/damping setting')
    p.add_argument('--alignment-prior', type=Path, help='Preserved verified policy for bounded alignment refinement')
    p.add_argument('--gait-prior',type=Path,help='Immutable walking controller protected during jogging leg learning')
    p.add_argument('--path-gain',type=float,default=1.,help='Bounded native course-correction gain for path refinement')
    args = p.parse_args()
    assert not args.jog_adaptive_features or (args.rust_learner and args.gait_refinement == 'mocap-actuator-v2' and args.gait_prior), 'Adaptive jog features require the protected actuator learner and walking teacher'
    assert np.isfinite(args.jog_leg_authority) and .25 <= args.jog_leg_authority <= 1.
    assert args.jog_leg_authority == .25 or args.jog_adaptive_features, 'Wider jog corrections require the protected adaptive actuator experiment'
    assert not args.mixed_jog_practice or args.jog_adaptive_features, 'Mixed jogging practice requires the protected adaptive actuator experiment'
    assert not args.jog_leg_policy_loss or args.jog_adaptive_features, 'Leg-only policy loss requires the protected adaptive actuator experiment'
    assert not args.reference_foot_placement or args.jog_leg_policy_loss, 'Reference placement requires the corrected protected jogging learner'
    assert not args.transition_prior or args.jog_leg_policy_loss, 'Transition feedback requires the corrected protected jogging learner'
    assert not args.walk_prior or args.transition_prior, 'Walking reference adaptation requires transition feedback and its active leg learner'
    assert 0.<=args.brisk_support<=1. and (args.brisk_support==0. or (args.gait_refinement=='mocap-actuator-v2' and args.jog_leg_policy_loss)), 'Brisk support requires the active actuator learner'
    assert not args.foot_knee_tracking or args.walk_prior, 'Foot/knee tracking requires the protected walking leg learner'
    assert not args.balance_prior or (args.walk_prior and (args.foot_knee_tracking or args.native_support_observations)), 'Balance adaptation requires reference tracking or explicit native support observations'
    assert not args.native_support_observations or (args.rust_learner and args.jog_adaptive_features and args.jog_leg_policy_loss), 'Native support inputs require the matching protected Rust learner'
    assert np.isfinite(args.phase_support) and 0.<=args.phase_support<=.25
    assert not args.phase_support or (args.rust_learner and args.gait_refinement=='mocap-actuator-v2' and args.walk_prior), 'Phase support requires the protected actuator controller'
    assert not args.stance_reward or (args.rust_learner and args.gait_refinement=='mocap-actuator-v2' and args.walk_prior), 'Stance quality requires the protected actuator controller'
    assert np.isfinite(args.native_leg_ownership) and 0.<=args.native_leg_ownership<=1.
    assert not args.native_leg_ownership or (args.rust_learner and args.gait_refinement=='mocap-actuator-v2' and args.walk_prior and args.jog_leg_policy_loss), 'Native leg ownership requires protected actuator learning'
    assert not args.native_leg_ownership or (args.phase_support==0. and not args.stance_reward), 'Native ownership cannot combine with sidecar support or stance reward'
    assert not args.contact_fidelity or (args.rust_learner and args.gait_refinement=='mocap-actuator-v2' and args.walk_prior and not args.stance_reward), 'Contact fidelity requires protected actuator learning without stance bonus'
    assert not args.walking_expert or (args.rust_learner and args.gait_refinement=='mocap-actuator-v2' and args.walk_prior and not args.native_support_observations), 'Walking expert requires the matching protected333-input actuator controller'
    assert np.isfinite(args.jog_native_ownership) and 0.<=args.jog_native_ownership<=.25
    assert not args.jog_native_ownership or (args.walking_expert and args.phase_support==0. and args.gait_refinement=='mocap-actuator-v2'), 'Jog ownership requires the retained walking expert and no sidecar support'
    assert not args.long_transition_practice or (args.walking_expert and args.transition_prior and args.gait_refinement=='mocap-actuator-v2'), 'Long transition practice requires protected walking and transition priors'
    assert np.isfinite(args.jog_route_gain) and 1.<=args.jog_route_gain<=2.
    assert args.jog_route_gain==1. or (args.walking_expert and args.straight_route and args.gait_refinement=='mocap-actuator-v2'), 'Jog steering requires protected walking and a straight requested route'
    assert not args.control_aware_policy_loss or (args.walking_expert and args.jog_leg_policy_loss and not args.native_support_observations), 'Control-aware policy loss requires the retained walking expert and333-input active leg learner'
    assert not args.ankle_residual or (args.walking_expert and args.jog_leg_policy_loss and not args.control_aware_policy_loss and not args.native_support_observations and not args.balance_prior), 'Ankle residual requires its54-output frozen-gait learner without other ownership changes'
    assert not args.ankle_fidelity or (args.ankle_residual and args.contact_fidelity and args.rust_learner), 'Ankle fidelity requires the frozen-gait ankle learner and existing contact fidelity'
    assert not args.leg_ankle_learning or (args.ankle_residual and args.ankle_fidelity and args.rust_learner), 'Leg/ankle co-adaptation requires its explicit54-output ankle/contact learner'
    assert np.isfinite(args.turn_duration) and 52/30<=args.turn_duration<=3. and (args.turn_reference or args.turn_duration==52/30), 'Invalid finite-turn duration'
    assert not args.turn_signal_reward or (args.turn_reference and args.turn_preview_feedback), 'Turn signal requires the explicit finite-turn preview controller'
    assert not args.turn_preview_feedback or (args.turn_reference and args.foot_state_feedback), 'Turn preview requires its349-input finite-reference learner'
    assert not args.whole_body_turn or (args.turn_preview_feedback and args.turn_signal_reward and args.rust_learner), 'Whole-body control requires its explicit350/90 finite-turn Rust learner'
    assert np.isfinite(args.torso_reference_gain) and 0.<=args.torso_reference_gain<=.25 and (args.torso_reference_gain==0. or args.whole_body_turn), 'Torso baseline needs bounded explicit whole-body control'
    assert not (args.walking_ankle_expert or args.turn_tempo_practice) or (args.turn_joint_control and args.rust_learner), 'Ankle retention and tempo practice require the explicit finite cut trainer'
    assert np.isfinite(args.cut_push_release) and 0.<=args.cut_push_release<=1. and (args.cut_push_release==0. or args.turn_joint_control), 'Cut push release requires bounded finite joint-control experiment'
    assert not args.turn_joint_control or (args.whole_body_turn and args.jog_native_ownership==0. and args.phase_support==0.), 'Cut motors require whole-body learner and shared lower-body target ownership'
    assert not args.reference_balance_reset or (args.whole_body_turn and args.rust_learner), 'Reference balance reset requires the explicit finite whole-body trainer'
    assert not (args.turn_entry_control or args.turn_route_anchor) or args.turn_reference, 'Turn-entry changes require an explicit finite reference'
    assert not args.turn_reference or (args.rust_learner and args.foot_state_feedback and args.walking_expert and args.straight_route), 'Finite turn requires the explicit foot-state CPU controller and rehearsal'
    assert not args.foot_state_feedback or (args.leg_ankle_learning and not args.native_support_observations), 'Foot-state feedback requires its341-input leg/ankle learner'
    assert np.isfinite(args.walk_reference_authority) and .35<=args.walk_reference_authority<=.8
    assert args.walk_reference_authority==.35 or (args.rust_learner and args.gait_refinement=='mocap-actuator-v2'), 'Walking target handover requires the actuator learner'
    assert np.isfinite(args.learning_rate) and 0 < args.learning_rate <= 1e-4, 'Learning rate must be within (0, 0.0001]'
    assert args.learning_rate == 1e-4 or args.rust_learner, 'Custom learning rate requires the Rust learner'
    assert not args.gait_refinement or (args.rust_learner and args.assisted and args.jog_reference), 'Gait refinement requires Rust-assisted walk/jog training'
    assert not args.transition_patience or (args.gait_refinement and args.rust_learner and args.jog_reference), 'Transition patience requires Rust gait refinement with jogging'
    assert not args.brisk_reference or (args.transition_patience and args.gait_refinement in coordination.TRACKING_MODES), 'Three-gait curriculum requires tracking and transition patience'
    assert np.isfinite(args.jog_limit) and 0. <= args.jog_limit <= 1.
    assert np.isfinite(args.jog_stiffness) and 6. <= args.jog_stiffness <= 16.
    assert np.isfinite(args.path_gain) and 0.<=args.path_gain<=1.
    assert 0 <= args.preview_port <= 65535, 'Preview port must be 0 (disabled) or a valid local UDP port'
    assert 1 <= args.eval_every <= 1000, 'Evaluation interval must be between 1 and 1000 updates'
    if args.self_test:
        self_test()
    else:
        assert all([args.exe, args.reference, args.model, args.out])
        if os.name == 'nt':
            import ctypes
            if not ctypes.windll.kernel32.SetThreadExecutionState(0x80000001):
                raise RuntimeError('Could not keep Windows awake during training')
        try:
            if args.rust_learner:
                assert not args.overlap_learner, 'Select one learner backend'
                assert args.rust_learner.is_file(), 'Rust learner executable not found'
                from train_motion_rust import train as train_rust
                from training_control import TrainerSession
                with TrainerSession(args.out):
                    train_rust(args)
            else:
                train(args)
        finally:
            if os.name == 'nt':
                ctypes.windll.kernel32.SetThreadExecutionState(0x80000000)
