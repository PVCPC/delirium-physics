import copy
import unittest

import numpy as np
import torch

from checkpoint_bridge import NAMES, freeze_checkpoint, warm_start


def fixture():
    shapes = ((4, 8), (4,), (4, 4), (4,), (2, 4), (2,))
    actor = {name: torch.full(shape, i + .25) for i, (name, shape) in enumerate(zip(NAMES, shapes))}
    critic_shapes = shapes[:4] + ((1, 4), (1,))
    critic = {name: torch.full(shape, i + 6.25) for i, (name, shape) in enumerate(zip(NAMES, critic_shapes))}
    parameters = list(actor.values()) + list(critic.values()) + [torch.full((2,), -2.)]
    group = dict(params=list(range(13)), lr=1e-4, betas=(.9, .999), eps=1e-8,
                 weight_decay=0, amsgrad=False, maximize=False)
    states = {i: dict(step=torch.tensor(123.), exp_avg=torch.full(p.shape, i + .5),
                      exp_avg_sq=torch.full(p.shape, i + 1.)) for i, p in enumerate(parameters)}
    return dict(actor=actor, critic=critic, log_std=parameters[-1], obs=8, actions=2,
                optimizer=dict(param_groups=[group], state=states), update=34074)


class BridgeTest(unittest.TestCase):
    def test_learning_rate_must_match_explicit_checkpoint_metadata(self):
        checkpoint = fixture()
        checkpoint['optimizer']['param_groups'][0]['lr'] = 2.5e-5
        with self.assertRaisesRegex(ValueError, 'Adam settings'):
            warm_start(checkpoint)
        checkpoint['learning_rate'] = 2.5e-5
        state = warm_start(checkpoint)
        result = freeze_checkpoint(checkpoint, state, update=34074, state_blob=b'state', state_format='v1')
        self.assertEqual(result['learning_rate'], 2.5e-5)
        self.assertEqual(result['optimizer']['param_groups'][0]['lr'], 2.5e-5)
        checkpoint['learning_rate'] = float('nan')
        with self.assertRaisesRegex(ValueError, 'Invalid checkpoint learning rate'):
            warm_start(checkpoint)

    def test_parameter_and_optimizer_order_roundtrip(self):
        checkpoint = fixture(); state = warm_start(checkpoint)
        actor_length = sum(t.numel() for t in checkpoint['actor'].values())
        np.testing.assert_array_equal(state['parameters'][actor_length:actor_length + 2], [-2., -2.])
        np.testing.assert_array_equal(state['exp_avg'][actor_length:actor_length + 2], [12.5, 12.5])
        self.assertEqual(state['adam_step'], 123)
        saved = freeze_checkpoint(checkpoint, state, update=34074, state_blob=b'opaque-rust-state', state_format='test-v1')
        for key, value in checkpoint['optimizer']['state'].items():
            for name, tensor in value.items():
                self.assertTrue(torch.equal(tensor, saved['optimizer']['state'][key][name]))
        self.assertEqual(saved['rust_learner_state'], b'opaque-rust-state')
        checkpoint['optimizer']['state'][0]['exp_avg'].zero_()
        self.assertNotEqual(saved['optimizer']['state'][0]['exp_avg'][0, 0].item(), 0.)

    def test_rejects_mixed_generation_weights_and_unsupported_adam(self):
        checkpoint = fixture(); state = warm_start(checkpoint)
        with self.assertRaisesRegex(ValueError, 'different update'):
            freeze_checkpoint(checkpoint, state, update=34075, state_blob=b'x', state_format='v1')
        changed = copy.deepcopy(state); changed['parameters'][0] += 1.
        with self.assertRaisesRegex(ValueError, 'do not match'):
            freeze_checkpoint(checkpoint, changed, update=34074, state_blob=b'x', state_format='v1')
        checkpoint['optimizer']['state'][0]['step'] += 1
        with self.assertRaisesRegex(ValueError, 'counters differ'):
            warm_start(checkpoint)
        checkpoint = fixture(); checkpoint['optimizer']['param_groups'][0]['weight_decay'] = .1
        with self.assertRaisesRegex(ValueError, 'settings differ'):
            warm_start(checkpoint)

    def test_uninitialized_adam_and_invalid_buffers(self):
        checkpoint = fixture(); checkpoint['optimizer']['state'] = {}
        state = warm_start(checkpoint)
        self.assertEqual(state['adam_step'], 0)
        self.assertFalse(state['exp_avg'].any())
        checkpoint = fixture(); checkpoint['optimizer']['state'][0]['exp_avg_sq'][0, 0] = -1
        with self.assertRaisesRegex(ValueError, 'Negative'):
            warm_start(checkpoint)


if __name__ == '__main__':
    torch.set_num_threads(1)
    unittest.main()
