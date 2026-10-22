import copy
import io
import unittest

import torch
import torch.multiprocessing as mp
from torch.distributions import Normal

from train_motion import learner_main, network, receive_learner


def equal(a, b):
    if isinstance(a, torch.Tensor):
        return isinstance(b, torch.Tensor) and torch.equal(a, b)
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(equal(a[k], b[k]) for k in a)
    if isinstance(a, (list, tuple)):
        return len(a) == len(b) and all(equal(x, y) for x, y in zip(a, b))
    return a == b


class OverlapCheckpointTest(unittest.TestCase):
    def start(self, saved):
        actor, critic = network(8, 4, True), network(8, 1)
        actor.load_state_dict(saved['actor']); critic.load_state_dict(saved['critic'])
        std = torch.nn.Parameter(saved['log_std'].clone())
        actor.share_memory(); critic.share_memory(); std.data.share_memory_()
        ctx = mp.get_context('spawn'); inq, outq = ctx.Queue(), ctx.Queue()
        proc = ctx.Process(target=learner_main,
                           args=(actor, critic, std, copy.deepcopy(saved['optimizer']), saved.get('learner_rng'), inq, outq, 28))
        proc.start()
        self.assertTrue(receive_learner(proc, outq)['ready'])
        self.addCleanup(self.stop, proc, inq)
        return actor, critic, std, proc, inq, outq

    def stop(self, proc, inq):
        if proc.is_alive():
            inq.put(('stop', None))
        proc.join(timeout=5)
        if proc.is_alive():
            proc.terminate(); proc.join(timeout=5)
            self.fail('Learner did not shut down cleanly')

    def test_child_optimizer_roundtrip_and_failures(self):
        torch.set_num_threads(1); torch.manual_seed(17)
        actor, critic = network(8, 4, True), network(8, 1)
        std = torch.nn.Parameter(torch.full((4,), -2.))
        parameters = list(actor.parameters()) + list(critic.parameters()) + [std]
        optimizer = torch.optim.Adam(parameters, lr=1e-4)
        x = torch.randn(28, 8)
        (actor(x).square().sum() + critic(x).square().sum() + std.square().sum()).backward()
        optimizer.step()
        saved = dict(actor=copy.deepcopy(actor.state_dict()), critic=copy.deepcopy(critic.state_dict()),
                     log_std=std.detach().clone(), optimizer=copy.deepcopy(optimizer.state_dict()))
        actor, critic, std, proc, inq, outq = self.start(saved)
        inq.put(('state', None)); state = receive_learner(proc, outq)
        self.assertTrue(equal(saved['optimizer'], state['optimizer']))
        with torch.no_grad():
            obs = torch.randn(64, 28, 8) * .01
            dist = Normal(actor(obs), std.clamp(-4, -.5).exp())
            actions = dist.sample(); logp = dist.log_prob(actions).sum(-1)
            batch = (obs, actions, logp, torch.randn(64, 28) * .01, torch.zeros(64, 28),
                     critic(obs).squeeze(-1), torch.zeros(28))
        inq.put(('learn', batch)); receive_learner(proc, outq)
        inq.put(('state', None)); learned = receive_learner(proc, outq)
        self.assertTrue(all(learned['optimizer']['state'][k]['step'] > v['step']
                            for k, v in saved['optimizer']['state'].items()))
        resumed = dict(actor=copy.deepcopy(actor.state_dict()), critic=copy.deepcopy(critic.state_dict()),
                       log_std=std.detach().clone(), optimizer=copy.deepcopy(learned['optimizer']), learner_rng=learned['rng'].clone())
        buffer = io.BytesIO(); torch.save(resumed, buffer); buffer.seek(0)
        self.stop(proc, inq)
        actor, critic, std, proc, inq, outq = self.start(torch.load(buffer, weights_only=True))
        inq.put(('state', None)); state = receive_learner(proc, outq)
        self.assertTrue(equal(resumed['optimizer'], state['optimizer']))
        self.assertTrue(equal(resumed['learner_rng'], state['rng']))
        self.assertTrue(equal(resumed['actor'], actor.state_dict()))
        self.assertTrue(equal(resumed['critic'], critic.state_dict()))
        inq.put(('invalid-command', None))
        with self.assertRaisesRegex(RuntimeError, 'Unknown learner command'):
            receive_learner(proc, outq)


if __name__ == '__main__':
    unittest.main()
