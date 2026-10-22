"""Explicit network expansion, preserving learned state and Adam/RNG history."""
import copy
import numpy as np
import torch
from checkpoint_bridge import warm_start
from rls2 import decode,encode

def add_observations(saved,extra):
    if not isinstance(extra,int) or extra<=0:raise ValueError('Positive input expansion required')
    result=copy.deepcopy(saved);old=warm_start(saved);state=decode(saved['rust_learner_state'])
    for key in ('parameters','exp_avg','exp_avg_sq'):
        np.testing.assert_array_equal(state[key].view('u4'),old[key].view('u4'))
    o,h,a=state['dimensions']
    critic_start=h*o+h+h*h+h+a*h+a+a
    def expand(flat):
        if flat is None:return None
        first=flat[:h*o].reshape(h,o)
        second=flat[critic_start:critic_start+h*o].reshape(h,o)
        return np.concatenate([np.pad(first,((0,0),(0,extra))).reshape(-1),flat[h*o:critic_start],
            np.pad(second,((0,0),(0,extra))).reshape(-1),flat[critic_start+h*o:]]).astype(np.float32)
    for net,index in [('actor',0),('critic',6)]:
        result[net]['0.weight']=torch.nn.functional.pad(result[net]['0.weight'],(0,extra))
        key=result['optimizer']['param_groups'][0]['params'][index]
        buffers=result['optimizer']['state'].get(key,{})
        for name in ('exp_avg','exp_avg_sq'):
            if name in buffers:buffers[name]=torch.nn.functional.pad(buffers[name],(0,extra))
    result['obs']=o+extra
    for key in ('parameters','exp_avg','exp_avg_sq','snapshot'):state[key]=expand(state[key])
    state['dimensions']=(o+extra,h,a)
    result['rust_learner_state']=encode(state)
    mirror=warm_start(result)
    for key in ('parameters','exp_avg','exp_avg_sq'):
        np.testing.assert_array_equal(state[key].view('u4'),mirror[key].view('u4'))
    assert result['update']==saved['update'] and state['completed_updates']==saved['update']+1
    return result


def add_actions(saved, extra, log_std=-2.0):
    """Zero new motor outputs; retain every existing moment, RNG and policy snapshot."""
    if not isinstance(extra, int) or extra <= 0 or not np.isfinite(log_std):
        raise ValueError('Positive action expansion and finite exploration required')
    result = copy.deepcopy(saved)
    state = decode(saved['rust_learner_state']); mirror = warm_start(saved)
    for key in ('parameters', 'exp_avg', 'exp_avg_sq'):
        np.testing.assert_array_equal(state[key].view('u4'), mirror[key].view('u4'))
    o, h, a = state['dimensions']; prefix = h*o+h+h*h+h

    def expand(flat, parameter=False):
        if flat is None:
            return None
        w = flat[prefix:prefix+a*h].reshape(a, h)
        b = prefix+a*h
        return np.concatenate([flat[:prefix], np.pad(w, ((0, extra), (0, 0))).reshape(-1),
            np.pad(flat[b:b+a], (0, extra)),
            np.pad(flat[b+a:b+2*a], (0, extra), constant_values=log_std if parameter else 0),
            flat[b+2*a:]]).astype(np.float32)

    result['actor']['4.weight'] = torch.nn.functional.pad(result['actor']['4.weight'], (0, 0, 0, extra))
    result['actor']['4.bias'] = torch.nn.functional.pad(result['actor']['4.bias'], (0, extra))
    result['log_std'] = torch.nn.functional.pad(result['log_std'], (0, extra), value=log_std)
    for index in (4, 5, 12):
        key = result['optimizer']['param_groups'][0]['params'][index]
        buffers = result['optimizer']['state'].get(key, {})
        for name in ('exp_avg', 'exp_avg_sq'):
            if name in buffers:
                padding = (0, 0, 0, extra) if index == 4 else (0, extra)
                buffers[name] = torch.nn.functional.pad(buffers[name], padding)
    result['actions'] = a+extra
    for key in ('parameters', 'exp_avg', 'exp_avg_sq', 'snapshot'):
        state[key] = expand(state[key], key in ('parameters', 'snapshot'))
    state['dimensions'] = (o, h, a+extra)
    result['rust_learner_state'] = encode(state)
    mirror = warm_start(result)
    for key in ('parameters', 'exp_avg', 'exp_avg_sq'):
        np.testing.assert_array_equal(state[key].view('u4'), mirror[key].view('u4'))
    assert state['completed_updates'] == saved['update']+1
    return result
