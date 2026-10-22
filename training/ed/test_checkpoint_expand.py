import unittest
import numpy as np
import torch
from train_motion import network
from checkpoint_bridge import warm_start
from checkpoint_expand import add_observations, add_actions
from rls2 import encode,decode

class ExpandedCheckpoint(unittest.TestCase):
    def test_new_motor_actions_keep_original_policy_adam_rng_and_snapshot(self):
        torch.manual_seed(11); actor=network(329,46,True); critic=network(329,1)
        log=torch.nn.Parameter(torch.full((46,),-2.))
        opt=torch.optim.Adam([*actor.parameters(),*critic.parameters(),log],lr=1e-5)
        (actor(torch.randn(2,329)).sum()+critic(torch.randn(2,329)).sum()+log.sum()).backward();opt.step()
        saved=dict(actor=actor.state_dict(),critic=critic.state_dict(),log_std=log.detach(),
                   optimizer=opt.state_dict(),obs=329,actions=46,update=273074,learning_rate=1e-5)
        old=warm_start(saved); old.update(rng=9876,snapshot=old['parameters'].copy())
        saved['rust_learner_state']=encode(old); fork=add_actions(saved,2)
        new=network(329,48,True);new.load_state_dict(fork['actor']);x=torch.randn(4,329)
        torch.testing.assert_close(actor(x),new(x)[:,:46],rtol=0,atol=0)
        torch.testing.assert_close(new(x)[:,46:],torch.zeros(4,2),rtol=0,atol=0)
        for name in saved['critic']:torch.testing.assert_close(saved['critic'][name],fork['critic'][name],rtol=0,atol=0)
        state=decode(fork['rust_learner_state'])
        for key in ('rng','adam_step','completed_updates'):self.assertEqual(state[key],old[key])
        np.testing.assert_array_equal(state['parameters'],state['snapshot'])
        for index in (4,5,12):
            key=saved['optimizer']['param_groups'][0]['params'][index]
            for name in ('exp_avg','exp_avg_sq'):
                torch.testing.assert_close(saved['optimizer']['state'][key][name],fork['optimizer']['state'][key][name][:46],rtol=0,atol=0)
                self.assertTrue(torch.all(fork['optimizer']['state'][key][name][46:]==0))
        self.assertEqual(saved['actions'],46)

    def test_actor_critic_adam_rng_and_previous_policy_survive_expansion(self):
        torch.manual_seed(8);actor=network(313,46,True);critic=network(313,1)
        log=torch.nn.Parameter(torch.zeros(46));params=[*actor.parameters(),*critic.parameters(),log]
        optimizer=torch.optim.Adam(params,lr=.0001)
        (actor(torch.randn(2,313)).sum()+critic(torch.randn(2,313)).sum()+log.sum()).backward();optimizer.step()
        saved=dict(actor=actor.state_dict(),critic=critic.state_dict(),log_std=log.detach(),optimizer=optimizer.state_dict(),obs=313,actions=46,update=7,learning_rate=.0001)
        state=warm_start(saved);state.update(rng=1234,snapshot=state['parameters'].copy())
        saved['rust_learner_state']=encode(state);fork=add_observations(saved,10)
        new_actor=network(323,46,True);new_actor.load_state_dict(fork['actor'])
        new_critic=network(323,1);new_critic.load_state_dict(fork['critic'])
        x=torch.randn(4,313);expanded=torch.cat([x,torch.randn(4,10)],dim=1)
        torch.testing.assert_close(actor(x),new_actor(expanded),rtol=1e-5,atol=1e-6)
        torch.testing.assert_close(critic(x),new_critic(expanded),rtol=1e-5,atol=1e-6)
        current=decode(fork['rust_learner_state']);self.assertEqual(current['rng'],1234)
        self.assertEqual(current['adam_step'],state['adam_step']);self.assertEqual(current['completed_updates'],8)
        np.testing.assert_array_equal(current['parameters'],current['snapshot'])
        self.assertEqual(saved['actor']['0.weight'].shape[1],313)

if __name__=='__main__':unittest.main()
