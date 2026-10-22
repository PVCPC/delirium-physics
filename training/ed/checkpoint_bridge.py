"""Torch checkpoint mapping; independent of Pelican's Rust wire/state format."""
import copy

import numpy as np
import torch

NAMES = ('0.weight', '0.bias', '2.weight', '2.bias', '4.weight', '4.bias')


def array(tensor):
    if not isinstance(tensor, torch.Tensor) or tensor.device.type != 'cpu' or tensor.dtype != torch.float32:
        raise ValueError('Expected a CPU float32 tensor')
    value = tensor.detach().contiguous().numpy().reshape(-1).copy()
    if not np.isfinite(value).all():
        raise ValueError('Non-finite checkpoint tensor')
    return value


def layout(checkpoint):
    actor, critic = checkpoint['actor'], checkpoint['critic']
    if set(actor) != set(NAMES) or set(critic) != set(NAMES):
        raise ValueError('Unsupported network parameter names')
    hidden, obs = actor['0.weight'].shape
    actions = checkpoint['log_std'].numel()
    for net, outputs in ((actor, actions), (critic, 1)):
        shapes = ((hidden, obs), (hidden,), (hidden, hidden), (hidden,), (outputs, hidden), (outputs,))
        if any(tuple(net[name].shape) != shape for name, shape in zip(NAMES, shapes)):
            raise ValueError('Checkpoint network shape mismatch')
    if checkpoint.get('obs', obs) != obs or checkpoint.get('actions', actions) != actions:
        raise ValueError('Checkpoint dimensions disagree with weights')
    parameters = [actor[name] for name in NAMES] + [critic[name] for name in NAMES] + [checkpoint['log_std']]
    order = list(range(6)) + [12] + list(range(6, 12))  # Rust: actor, log_std, critic.
    return parameters, order, (obs, hidden, actions)


def optimizer_group(checkpoint):
    optimizer = checkpoint['optimizer']
    if not isinstance(optimizer, dict) or len(optimizer['param_groups']) != 1:
        raise ValueError('Expected one Adam parameter group')
    group = optimizer['param_groups'][0]
    if len(group['params']) != 13 or len(set(group['params'])) != 13:
        raise ValueError('Unsupported Adam parameter layout')
    learning_rate = checkpoint.get('learning_rate', 1e-4)
    if not np.isfinite(learning_rate) or not 0 < learning_rate <= 1e-4:
        raise ValueError('Invalid checkpoint learning rate')
    expected = {'lr': learning_rate, 'eps': 1e-8, 'weight_decay': 0, 'amsgrad': False, 'maximize': False}
    if tuple(group['betas']) != (.9, .999) or any(group.get(k, v) != v for k, v in expected.items()):
        raise ValueError('Adam settings differ from the Rust learner')
    if group.get('differentiable', False):
        raise ValueError('Differentiable Adam is unsupported')
    return optimizer, group


def warm_start(checkpoint):
    """Reorder every parameter and moment; reject unequal per-parameter Adam steps."""
    parameters, order, dimensions = layout(checkpoint)
    optimizer, group = optimizer_group(checkpoint)
    moments, variances, steps = [], [], []
    for parameter, key in zip(parameters, group['params']):
        state = optimizer['state'].get(key)
        if state:
            step = float(state['step'])
            if not np.isfinite(step) or step < 0 or not step.is_integer() or step > 2**31 - 1:
                raise ValueError('Invalid Adam step counter')
            if state['exp_avg'].shape != parameter.shape or state['exp_avg_sq'].shape != parameter.shape:
                raise ValueError('Adam buffer shape mismatch')
            m, v = array(state['exp_avg']), array(state['exp_avg_sq'])
            if np.any(v < 0):
                raise ValueError('Negative Adam second moment')
        else:
            step = 0
            m = np.zeros(parameter.numel(), dtype=np.float32); v = m.copy()
        moments.append(m); variances.append(v); steps.append(int(step))
    if len(set(steps)) != 1:
        raise ValueError('Rust has one Adam counter; Torch parameter counters differ')
    return dict(parameters=np.concatenate([array(parameters[i]) for i in order]),
                exp_avg=np.concatenate([moments[i] for i in order]),
                exp_avg_sq=np.concatenate([variances[i] for i in order]),
                adam_step=steps[0], completed_updates=checkpoint['update'] + 1, dimensions=dimensions)


def freeze_checkpoint(template, rust, *, update, state_blob, state_format):
    """Caller must obtain one quiescent Rust snapshot, not a later learner state."""
    parameters, order, dimensions = layout(template)
    optimizer, group = optimizer_group(template)
    if rust['completed_updates'] != update + 1 or tuple(rust['dimensions']) != dimensions:
        raise ValueError('Rust state belongs to a different update or architecture')
    if not isinstance(state_blob, bytes) or not state_blob or not isinstance(state_format, str) or not state_format:
        raise ValueError('Missing complete Rust state blob/version')
    step = rust['adam_step']
    if not isinstance(step, int) or not 0 <= step <= 2**31 - 1:
        raise ValueError('Invalid Rust Adam counter')
    n = sum(p.numel() for p in parameters)
    vectors = []
    for name in ('parameters', 'exp_avg', 'exp_avg_sq'):
        vector = np.asarray(rust[name])
        if vector.dtype != np.float32 or vector.shape != (n,) or not np.isfinite(vector).all():
            raise ValueError('Invalid Rust state vector: ' + name)
        vectors.append(vector)
    if np.any(vectors[2] < 0):
        raise ValueError('Negative Rust Adam second moment')
    expected = np.concatenate([array(parameters[i]) for i in order])
    if not np.array_equal(expected.view(np.uint32), vectors[0].view(np.uint32)):
        raise ValueError('Evaluation weights and Rust optimizer snapshot do not match')
    result = copy.deepcopy(template)
    result['optimizer'] = copy.deepcopy(optimizer)
    offset = 0
    for index in order:
        parameter = parameters[index]; size = parameter.numel(); key = group['params'][index]
        old_step = optimizer['state'].get(key, {}).get('step')
        dtype = old_step.dtype if isinstance(old_step, torch.Tensor) else torch.float32
        counter = torch.tensor(step, dtype=dtype)
        if counter.item() != step:
            raise ValueError('Adam counter cannot be represented exactly in Torch')
        result['optimizer']['state'][key] = dict(step=counter,
            exp_avg=torch.from_numpy(vectors[1][offset:offset + size].copy()).reshape(parameter.shape),
            exp_avg_sq=torch.from_numpy(vectors[2][offset:offset + size].copy()).reshape(parameter.shape))
        offset += size
    result.update(update=update, learner_update=update, training_pipeline='ppo-rust-v1',
                  rust_learner_state=state_blob, rust_learner_format=state_format)
    return result
