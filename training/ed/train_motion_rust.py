"""CPU Rust PPO backend; shared exports and controller protocol stay in train_motion."""
import copy
import hashlib
import json
import os
import shutil
import subprocess
import threading
import time
from pathlib import Path

import numpy as np
import torch
from torch import nn
from live_training import LivePreview
from checkpoint_bridge import warm_start, freeze_checkpoint
from rls2 import Client, encode, decode
from gait_refinement import contract as refinement_contract, penalties as gait_penalties, quality_improved
from training_control import PauseRequest, CAPABILITY, LIMIT_CAPABILITY, update_target, update_range
import coordination
import gait_curriculum
import turn_curriculum
import stop_curriculum
from train_motion import (Native as _Native, REWARD_CONTRACT, network, export,
    replace_saved_file, publish_best, eligible_walk, transition_progress,
    transition_improved, save_json)


def patience_age(update, best_update, continuation_start_update=-1):
    return update - max(best_update, continuation_start_update)

def patience_reference(quality_update, transition_update, transition_patience=False):
    # Partial transition gains buy learning time, never a qualified-gait pass.
    return max(quality_update, transition_update) if transition_patience else quality_update

def learner_capability(refinement, adaptive=False, active_loss=False, balance=False, native_support=False, controlled_rows=False, ankle_residual=False, leg_ankle_learning=False, foot_state_feedback=False, turn_preview_feedback=False, whole_body_turn=False, turn_joint_control=False, walking_ankle_expert=False):
    if walking_ankle_expert and not turn_joint_control:raise ValueError('Walking ankle anchor requires cut-joint learner')
    if turn_joint_control and not whole_body_turn:raise ValueError('Cut motors require whole-body control')
    if whole_body_turn:
        if not (refinement=='mocap-actuator-v2' and adaptive and active_loss and ankle_residual and leg_ankle_learning and foot_state_feedback and turn_preview_feedback and not (balance or native_support or controlled_rows)):
            raise ValueError('Whole-body control requires its explicit350/90 protected learner')
        if walking_ankle_expert:return 'protected-walk-ankle-anchor-v1'
        return 'protected-walk-cut-joints-v1' if turn_joint_control else 'protected-walk-whole-body-turn-v1'
    if turn_preview_feedback and not foot_state_feedback:raise ValueError('Turn preview requires retained foot-state observations')
    if foot_state_feedback and not leg_ankle_learning:raise ValueError('Foot-state feedback requires its341-input leg/ankle learner')
    if leg_ankle_learning and not ankle_residual:raise ValueError('Leg/ankle learning requires the explicit54-output ankle controller')
    if ankle_residual:
        if refinement!='mocap-actuator-v2' or not adaptive or not active_loss or balance or native_support or controlled_rows:raise ValueError('Ankle residual requires its frozen333/54 gait learner')
        if turn_preview_feedback:return 'protected-walk-leg-ankle-turn-preview-v1'
        return ('protected-walk-leg-ankle-foot-state-v1' if foot_state_feedback else 'protected-walk-leg-ankle-loss-v1') if leg_ankle_learning else 'frozen-gait-ankle-residual-v1'
    if controlled_rows:
        if refinement!='mocap-actuator-v2' or not adaptive or not active_loss or balance or native_support:raise ValueError('Control-aware loss requires protected333-input active leg learning')
        return 'protected-walk-controlled-leg-loss-v2'
    if native_support:
        if refinement!='mocap-actuator-v2' or not adaptive or not active_loss:raise ValueError('Native support requires the protected337-input active-leg learner')
        return 'protected-arms-native-support-v1' if balance else 'protected-walk-native-support-v1'
    if balance:
        if refinement!='mocap-actuator-v2' or not adaptive or not active_loss:raise ValueError('Balance adaptation requires active actuator learning')
        return 'protected-arms-balance-leg-loss-v1'
    if active_loss and not adaptive:
        raise ValueError('Leg policy loss requires the protected adaptive controller')
    if adaptive:
        if refinement != 'mocap-actuator-v2':
            raise ValueError('Adaptive jogging features require the protected actuator controller')
        return 'protected-walk-active-leg-loss-v1' if active_loss else 'protected-walk-adaptive-features-v1'
    return {'mocap-actuator-v2':'protected-walk-actuator-v1',
            'mocap-leg-skill-v2':'protected-walk-leg-skill-v1',
            'mocap-leg-strength-v2':'frozen-gait-leg-strength-v1'}[refinement]

class Native(_Native):

    def rollout(self, actor, std, noise):
        layers = [l for l in actor if isinstance(l, nn.Linear)]
        w = np.concatenate([a.detach().numpy().reshape(-1) for l in layers for a in [l.weight, l.bias]] + [std.detach().numpy(), noise.numpy().reshape(-1)]).astype('<f4')
        self.process.stdin.write(b'\x06' + np.array([noise.shape[0], layers[0].out_features], dtype='<f4').tobytes() + w.tobytes())
        self.process.stdin.flush()
        return self.read(noise.shape[0] * self.count * (self.actions + self.obs + 2 + self.metrics)).reshape(noise.shape[0], self.count, -1)

def train(args):
    args.updates = update_target(args.out, args.updates)
    torch.set_num_threads(4)
    torch.set_num_interop_threads(1)
    torch.manual_seed(17)
    assert torch.version.cuda is None, 'Use the isolated CPU-only torch environment'
    assert args.rust_learner and not args.overlap_learner, 'Select one learner backend'
    os.environ['RUSTLEARN'] = str(args.rust_learner.resolve())
    device = torch.device('cpu')
    anchor = next((p / 'getup.net' for p in args.exe.resolve().parents if (p / 'getup.net').exists()), None)
    if anchor is None or not anchor.with_name('walk.net').exists():
        raise ValueError('Native walking requires getup.net and walk.net beside or above the executable')
    prior_hash = hashlib.sha256(anchor.with_name('walk.net').read_bytes()).hexdigest()
    args.out.mkdir(exist_ok=True, parents=True)
    model = args.model.resolve()
    reference = args.reference.resolve()
    jog_reference = args.jog_reference.resolve() if args.jog_reference else None
    brisk_reference = args.brisk_reference.resolve() if args.brisk_reference else None
    turn_reference=getattr(args,'turn_reference',None)
    turn_reference=turn_reference.resolve() if turn_reference else None
    turn_hash=hashlib.sha256(turn_reference.read_bytes()).hexdigest() if turn_reference else None
    jog_stop=getattr(args,'jog_stop',False)
    assert not jog_stop or (turn_reference and getattr(args,'turn_joint_control',False) and not getattr(args,'turn_tempo_practice',False))
    os.environ['DLM_JOG_STOP']='1' if jog_stop else '0'
    task_curriculum=stop_curriculum if jog_stop else turn_curriculum
    turn_signal_reward=getattr(args,'turn_signal_reward',False)
    assert not turn_signal_reward or (turn_reference and getattr(args,'turn_preview_feedback',False))
    os.environ['DLM_TURN_SIGNAL_REWARD']='1' if turn_signal_reward else '0'
    whole_body_turn=getattr(args,'whole_body_turn',False)
    assert not whole_body_turn or (turn_signal_reward and turn_reference and getattr(args,'turn_preview_feedback',False))
    os.environ['DLM_WHOLE_BODY_TURN']='1' if whole_body_turn else '0'
    torso_reference_gain=getattr(args,'torso_reference_gain',0.)
    assert np.isfinite(torso_reference_gain) and 0.<=torso_reference_gain<=.25 and (torso_reference_gain==0. or whole_body_turn)
    os.environ['DLM_TURN_TORSO_GAIN']=str(torso_reference_gain)
    turn_joint_control=getattr(args,'turn_joint_control',False)
    assert not turn_joint_control or (whole_body_turn and getattr(args,'jog_native_ownership',0.)==0. and getattr(args,'phase_support',0.)==0.)
    os.environ['DLM_TURN_JOINT_CONTROL']='1' if turn_joint_control else '0'
    walking_ankle_expert=getattr(args,'walking_ankle_expert',None)
    walking_ankle_hash=hashlib.sha256(walking_ankle_expert.read_bytes()).hexdigest() if walking_ankle_expert else None
    if walking_ankle_expert:os.environ['DLM_WALKING_ANKLE_EXPERT']=str(walking_ankle_expert.resolve())
    else:os.environ.pop('DLM_WALKING_ANKLE_EXPERT',None)
    turn_tempo_practice=getattr(args,'turn_tempo_practice',False)
    assert not (walking_ankle_expert or turn_tempo_practice) or turn_joint_control
    os.environ['DLM_TURN_TEMPO_PRACTICE']='1' if turn_tempo_practice else '0'
    cut_push_release=getattr(args,'cut_push_release',0.)
    assert np.isfinite(cut_push_release) and 0.<=cut_push_release<=1. and (cut_push_release==0. or turn_joint_control)
    os.environ['DLM_CUT_PUSH_RELEASE']=str(cut_push_release)
    os.environ['DLM_CUT_SUPPORT_GAIN']='0' # support-phase ablations were rejected
    reference_balance_reset=getattr(args,'reference_balance_reset',False)
    assert not reference_balance_reset or whole_body_turn
    os.environ['DLM_REFERENCE_BALANCE_RESET']='1' if reference_balance_reset else '0'
    turn_duration=getattr(args,'turn_duration',52/30)
    assert np.isfinite(turn_duration) and 52/30<=turn_duration<=3. and (turn_reference or turn_duration==52/30)
    os.environ['DLM_TURN_DURATION']=str(turn_duration)
    turn_preview_feedback=getattr(args,'turn_preview_feedback',False)
    assert not turn_preview_feedback or turn_reference
    os.environ['DLM_TURN_PREVIEW_FEEDBACK']='1' if turn_preview_feedback else '0'
    turn_entry_control=getattr(args,'turn_entry_control',False)
    turn_route_anchor=getattr(args,'turn_route_anchor',False)
    assert not (turn_entry_control or turn_route_anchor) or turn_reference
    os.environ['DLM_TURN_ENTRY_CONTROL']='1' if turn_entry_control else '0'
    os.environ['DLM_TURN_ROUTE_ANCHOR']='1' if turn_route_anchor else '0'
    if turn_reference:os.environ['DLM_TURN_REFERENCE']=str(turn_reference)
    else:os.environ.pop('DLM_TURN_REFERENCE',None)
    resume_metadata = torch.load(args.out/'ppo.pt', map_location='cpu', weights_only=True) if brisk_reference and (args.out/'ppo.pt').exists() else {}
    curriculum = copy.deepcopy(resume_metadata.get('gait_curriculum', dict(limit=args.jog_limit, passes=0, last_promotion=resume_metadata.get('update', 0), history=[]))) if brisk_reference else None
    if turn_reference:curriculum=None
    assert curriculum is None or (np.isfinite(curriculum['limit']) and 0. <= curriculum['limit'] <= 1.)
    def replay_profile(limit=None):
        return dict(coordination_targets, jog_limit=curriculum['limit'] if limit is None else limit) if curriculum else coordination_targets
    assert not jog_reference or (args.assisted and (not args.behaviors)), 'Transitions require native-assisted locomotion'
    refinement = getattr(args, 'gait_refinement', None)
    leg_authority=getattr(args,'jog_leg_authority',.25)
    assert np.isfinite(leg_authority) and .25<=leg_authority<=1.
    assert leg_authority==.25 or (refinement=='mocap-actuator-v2' and getattr(args,'jog_adaptive_features',False))
    os.environ['DLM_JOG_LEG_AUTHORITY']=str(leg_authority)
    mixed_practice=getattr(args,'mixed_jog_practice',False)
    assert not mixed_practice or (refinement=='mocap-actuator-v2' and getattr(args,'jog_adaptive_features',False))
    os.environ['DLM_MIXED_JOG_PRACTICE']='1' if mixed_practice else '0'
    reference_placement=getattr(args,'reference_foot_placement',False)
    assert not reference_placement or getattr(args,'jog_leg_policy_loss',False)
    os.environ['DLM_REFERENCE_FOOT_PLACEMENT']='1' if reference_placement else '0'
    os.environ.pop('DLM_EVAL_PHASE_SHIFT',None)  # Additional phase sweeps are offline diagnostics only.
    transition_prior=getattr(args,'transition_prior',None)
    transition_prior_hash=hashlib.sha256(transition_prior.read_bytes()).hexdigest() if transition_prior else None
    if transition_prior:os.environ['DLM_TRANSITION_PRIOR']=str(transition_prior.resolve())
    else:os.environ.pop('DLM_TRANSITION_PRIOR',None)
    walk_prior=getattr(args,'walk_prior',None)
    assert not walk_prior or transition_prior, 'Walking feedback requires its protected transition controller'
    walk_prior_hash=hashlib.sha256(walk_prior.read_bytes()).hexdigest() if walk_prior else None
    if walk_prior:os.environ['DLM_WALK_REFERENCE_PRIOR']=str(walk_prior.resolve())
    else:os.environ.pop('DLM_WALK_REFERENCE_PRIOR',None)
    brisk_support=getattr(args,'brisk_support',0.)
    assert np.isfinite(brisk_support) and 0.<=brisk_support<=1.
    os.environ['DLM_BRISK_SUPPORT']=str(brisk_support)
    foot_knee_tracking=getattr(args,'foot_knee_tracking',False)
    os.environ['DLM_FOOT_KNEE_TRACKING']='1' if foot_knee_tracking else '0'
    balance_prior=getattr(args,'balance_prior',None)
    assert not balance_prior or (walk_prior and (foot_knee_tracking or getattr(args,'native_support_observations',False)))
    balance_prior_hash=hashlib.sha256(balance_prior.read_bytes()).hexdigest() if balance_prior else None
    if balance_prior:os.environ['DLM_BALANCE_PRIOR']=str(balance_prior.resolve())
    else:os.environ.pop('DLM_BALANCE_PRIOR',None)
    ankle_prior=getattr(args,'ankle_prior',None)
    assert not ankle_prior or (walk_prior and getattr(args,'jog_leg_policy_loss',False)), 'Ankle feedback requires active leg policy learning'
    ankle_prior_hash=hashlib.sha256(ankle_prior.read_bytes()).hexdigest() if ankle_prior else None
    if ankle_prior:os.environ['DLM_ANKLE_PRIOR']=str(ankle_prior.resolve())
    else:os.environ.pop('DLM_ANKLE_PRIOR',None)
    straight_route=getattr(args,'straight_route',False)
    assert not straight_route or walk_prior, 'Straight route requires the protected walking controller'
    os.environ['DLM_STRAIGHT_ROUTE']='1' if straight_route else '0'
    walk_reference_authority=getattr(args,'walk_reference_authority',.35)
    assert np.isfinite(walk_reference_authority) and .35<=walk_reference_authority<=.8
    assert walk_reference_authority==.35 or refinement=='mocap-actuator-v2'
    os.environ['DLM_WALK_REFERENCE_AUTHORITY']=str(walk_reference_authority)
    native_support_observations=getattr(args,'native_support_observations',False)
    assert not native_support_observations or (refinement=='mocap-actuator-v2' and getattr(args,'jog_adaptive_features',False))
    os.environ['DLM_NATIVE_SUPPORT_OBSERVATIONS']='1' if native_support_observations else '0'
    phase_support=getattr(args,'phase_support',0.)
    assert np.isfinite(phase_support) and 0.<=phase_support<=.25
    assert not phase_support or (refinement=='mocap-actuator-v2' and walk_prior)
    os.environ['DLM_PHASE_SUPPORT']=str(phase_support)
    stance_reward=getattr(args,'stance_reward',False)
    assert not stance_reward or (refinement=='mocap-actuator-v2' and walk_prior)
    os.environ['DLM_STANCE_REWARD']='1' if stance_reward else '0'
    native_leg_ownership=getattr(args,'native_leg_ownership',0.)
    assert np.isfinite(native_leg_ownership) and 0.<=native_leg_ownership<=1.
    assert not native_leg_ownership or (refinement=='mocap-actuator-v2' and walk_prior and getattr(args,'jog_leg_policy_loss',False)), 'Native leg ownership requires protected actuator learning'
    assert not native_leg_ownership or (phase_support==0. and not stance_reward), 'Native ownership cannot combine with sidecar support or stance reward'
    os.environ['DLM_NATIVE_LEG_OWNERSHIP']=str(native_leg_ownership)
    contact_fidelity=getattr(args,'contact_fidelity',False)
    assert not contact_fidelity or (refinement=='mocap-actuator-v2' and walk_prior and not stance_reward)
    os.environ['DLM_CONTACT_FIDELITY']='1' if contact_fidelity else '0'
    walking_expert=getattr(args,'walking_expert',None)
    walking_expert_hash=hashlib.sha256(walking_expert.read_bytes()).hexdigest() if walking_expert else None
    walking_expert_update=None
    if walking_expert and walking_expert.with_suffix('.json').exists():
        expert_info=json.loads(walking_expert.with_suffix('.json').read_text())
        assert expert_info['sha256']==walking_expert_hash,'Walking expert source metadata does not match model'
        walking_expert_update=expert_info['source_update']
    if walking_expert:os.environ['DLM_WALKING_EXPERT']=str(walking_expert.resolve())
    else:os.environ.pop('DLM_WALKING_EXPERT',None)
    jog_native_ownership=getattr(args,'jog_native_ownership',0.)
    assert np.isfinite(jog_native_ownership) and 0.<=jog_native_ownership<=.25
    assert not jog_native_ownership or (walking_expert and phase_support==0.)
    os.environ['DLM_JOG_NATIVE_OWNERSHIP']=str(jog_native_ownership)
    long_transition_practice=getattr(args,'long_transition_practice',False)
    assert not long_transition_practice or walking_expert
    os.environ['DLM_LONG_TRANSITIONS']='1' if long_transition_practice else '0'
    jog_route_gain=getattr(args,'jog_route_gain',1.)
    assert np.isfinite(jog_route_gain) and 1.<=jog_route_gain<=2.
    assert jog_route_gain==1. or (walking_expert and straight_route)
    os.environ['DLM_JOG_ROUTE_GAIN']=str(jog_route_gain)
    control_aware_policy_loss=getattr(args,'control_aware_policy_loss',False)
    assert not control_aware_policy_loss or (walking_expert and not native_support_observations and getattr(args,'jog_leg_policy_loss',False))
    ankle_residual=getattr(args,'ankle_residual',False)
    assert not ankle_residual or (walking_expert and not control_aware_policy_loss and not native_support_observations and not balance_prior)
    os.environ['DLM_ANKLE_RESIDUAL']='1' if ankle_residual else '0'
    ankle_fidelity=getattr(args,'ankle_fidelity',False)
    assert not ankle_fidelity or (ankle_residual and contact_fidelity)
    os.environ['DLM_ANKLE_FIDELITY']='1' if ankle_fidelity else '0'
    leg_ankle_learning=getattr(args,'leg_ankle_learning',False)
    assert not leg_ankle_learning or (ankle_residual and ankle_fidelity)
    foot_state_feedback=getattr(args,'foot_state_feedback',False)
    assert not foot_state_feedback or (leg_ankle_learning and not native_support_observations)
    os.environ['DLM_FOOT_STATE_FEEDBACK']='1' if foot_state_feedback else '0'
    # The isolated native support ablation was rejected; keep it outside production recovery.
    os.environ['DLM_JOG_SUPPORT_ONLY']='0'
    assert not refinement or jog_reference, 'Gait refinement requires walk/jog observations'
    coordinating = refinement in coordination.MODES and not turn_reference
    tracking = refinement if refinement in coordination.TRACKING_MODES else None
    contact_refinement = refinement in ('mocap-jog-contact-v2','mocap-jog-phase-v2','mocap-jog-drive-v2','mocap-jog-ballistic-v2','mocap-push-off-v2','mocap-alignment-v2','mocap-path-v2','mocap-leg-strength-v2','mocap-leg-skill-v2','mocap-actuator-v2')
    contact_refinement=contact_refinement and not turn_reference
    alignment_refinement=refinement in ('mocap-alignment-v2','mocap-path-v2','mocap-leg-strength-v2','mocap-leg-skill-v2','mocap-actuator-v2')
    alignment_prior_hash=None
    learner_hash=None
    gait_prior_hash=None
    if refinement in ('mocap-leg-skill-v2','mocap-actuator-v2'):
        if not args.gait_prior:raise ValueError('Leg skill requires its protected walking controller')
        os.environ['DLM_GAIT_PRIOR']=str(args.gait_prior.resolve())
        gait_prior_hash=hashlib.sha256(args.gait_prior.read_bytes()).hexdigest()
    if refinement in ('mocap-leg-strength-v2','mocap-leg-skill-v2','mocap-actuator-v2'):
        capability=subprocess.check_output([str(args.rust_learner),'capabilities'],text=True,timeout=5).strip()
        expected=learner_capability(refinement,getattr(args,'jog_adaptive_features',False),getattr(args,'jog_leg_policy_loss',False),bool(balance_prior),native_support_observations,control_aware_policy_loss,ankle_residual,leg_ankle_learning,foot_state_feedback,turn_preview_feedback,whole_body_turn,turn_joint_control,bool(walking_ankle_expert))
        assert capability==expected,'Leg learning requires the validated matching learner mask'
        learner_hash=hashlib.sha256(args.rust_learner.read_bytes()).hexdigest()
    if alignment_refinement:
        if not args.alignment_prior:raise ValueError('Alignment refinement requires its preserved policy')
        os.environ['DLM_ALIGNMENT_PRIOR']=str(args.alignment_prior.resolve())
        alignment_prior_hash=hashlib.sha256(args.alignment_prior.read_bytes()).hexdigest()
        os.environ['DLM_PATH_GAIN']=str(args.path_gain)
    tracking_native_hash = hashlib.sha256(args.exe.read_bytes()).hexdigest() if tracking else None
    coordination_targets = coordination.targets(reference, jog_reference, reference.parent / 'homer-build.txt', brisk_reference) if refinement in coordination.MODES else None
    if coordinating:
        save_json(args.out / 'coordination-targets.json', coordination_targets)
    reward_contract = refinement_contract(refinement) if jog_reference else REWARD_CONTRACT
    if reference_placement:reward_contract+=':reference-foot-placement-v1'
    if transition_prior:reward_contract+=':transition-feedback-v1'
    if walk_prior:reward_contract+=':walk-reference-feedback-v1'
    if brisk_support:reward_contract+=':brisk-support-v1'
    if foot_knee_tracking:reward_contract+=':foot-knee-tracking-v2'
    if balance_prior:reward_contract+=':balance-feedback-v1'
    if ankle_prior:reward_contract+=':ankle-feedback-v1'
    if straight_route:reward_contract+=':straight-route-v1'
    if walk_reference_authority!=.35:reward_contract+=':walk-reference-authority-v1'
    if native_support_observations:reward_contract+=':native-support-observations-v1'
    if phase_support:reward_contract+=':walk-phase-support-v1'
    if stance_reward:reward_contract+=':stance-quality-v1'
    if native_leg_ownership:reward_contract+=':native-walk-leg-ownership-v1'
    if contact_fidelity:reward_contract+=':contact-fidelity-v1'
    if walking_expert:reward_contract+=':walking-expert-v1'
    if jog_native_ownership:reward_contract+=':native-jog-ownership-v1'
    if long_transition_practice:reward_contract+=':long-transition-practice-v1'
    if jog_route_gain!=1.:reward_contract+=':jog-course-feedback-v1'
    if control_aware_policy_loss:reward_contract+=':controlled-policy-rows-v2'
    if ankle_residual:reward_contract+=':ankle-residual-v1'
    if ankle_fidelity:reward_contract+=':ankle-fidelity-v1'
    if leg_ankle_learning:reward_contract+=':leg-ankle-learning-v1'
    if foot_state_feedback:reward_contract+=':foot-state-feedback-v1'
    if turn_reference:reward_contract+=':finite-turn-route-v1'
    if turn_entry_control:reward_contract+=':turn-entry-control-v1'
    if turn_route_anchor:reward_contract+=':turn-reference-entry-placement-v1'
    if turn_preview_feedback:reward_contract+=':turn-preview-feedback-v1'
    if turn_duration!=52/30:reward_contract+=f':turn-tempo-{turn_duration:g}s-v1'
    if turn_signal_reward:reward_contract+=':turn-signal-v1'
    if whole_body_turn:reward_contract+=':whole-body-turn-v1'
    if torso_reference_gain:reward_contract+=f':torso-reference-v1:{torso_reference_gain:g}'
    if reference_balance_reset:reward_contract+=':reference-balance-reset-v1'
    if turn_joint_control:reward_contract+=':cut-joint-control-v1'
    if cut_push_release:reward_contract+=f':cut-push-release-v1:{cut_push_release:g}'
    if walking_ankle_expert:reward_contract+=':walking-ankle-anchor-v1'
    if turn_tempo_practice:reward_contract+=':turn-tempo-practice-v1'
    if jog_stop:reward_contract+=':jog-stop-physical-braking-v1'
    marker = model.with_suffix('.path')
    if marker.exists() and Path(marker.read_text().strip()).resolve() != reference:
        raise ValueError('Existing model uses a different reference; choose a new model name')
    marker.write_text(str(reference))
    assert not (args.assisted and args.behaviors), 'Walking keeps balance active; skill switching is a later stage'
    evaluation_model = (args.out / 'candidate.net').resolve() if args.assisted else model
    env = Native(args.exe.resolve(), reference, evaluation_model, args.envs, args.out / 'native.log', args.behaviors, args.assisted, jog_reference, tracking=tracking,
                 brisk_reference=brisk_reference, jog_limit=curriculum['limit'] if curriculum else (args.jog_limit if turn_reference else 1.),jog_stiffness=args.jog_stiffness)
    preview = None
    rust_client = None
    eval_env = None
    eval_job = None
    try:
        preview = LivePreview(args.preview_port, args.out) if args.preview_port else None
        save_json(args.out / 'collider-frames.json', dict(body_to_collider_quaternions=env.collider_rotations.tolist()))
        catalog_file = model.with_suffix('.tools.tsv')
        if catalog_file.exists() and catalog_file.read_text(encoding='utf-8') != env.catalog:
            raise ValueError('Delirium tool registry changed; use a new model name to avoid reassigning learned actions')
        catalog_file.write_text(env.catalog, encoding='utf-8', newline='')
        actor, critic = (network(env.obs, env.actions, True).to(device), network(env.obs, 1).to(device))
        log_std = nn.Parameter(torch.full((env.actions,), -2.0, device=device))
        parameters = list(actor.parameters()) + list(critic.parameters()) + [log_std]
        learning_rate = getattr(args, 'learning_rate', 1e-4)
        optimizer = torch.optim.Adam(parameters, lr=learning_rate)
        checkpoint = args.out / 'ppo.pt'
        first = 0
        continuation_start_update = -1
        if checkpoint.exists():
            saved = torch.load(checkpoint, map_location='cpu', weights_only=True)
            assert saved.get('learning_rate', 1e-4) == learning_rate, 'Learning rate changed; use an explicit checkpoint fork'
            assert saved.get('motion_tracking_native_sha256', tracking_native_hash) == tracking_native_hash, 'Tracking controller changed; use an explicit checkpoint fork'
            assert saved.get('native_walk_prior_sha256', prior_hash) == prior_hash, 'Native walking prior changed'
            assert saved['reference'] == str(reference) and saved['obs'] == env.obs
            assert saved.get('actions', env.actions) == env.actions
            assert saved.get('rust_learner_sha256')==learner_hash,'Learner training mask changed; use an explicit checkpoint fork'
            assert saved.get('jog_adaptive_features',False)==getattr(args,'jog_adaptive_features',False),'Jog feature mask changed; use the saved launch command'
            assert saved.get('jog_leg_authority',.25)==leg_authority,'Jog leg authority changed; use an explicit checkpoint fork'
            assert saved.get('mixed_jog_practice',False)==mixed_practice,'Jog practice distribution changed; use the saved launch command'
            assert saved.get('jog_leg_policy_loss',False)==getattr(args,'jog_leg_policy_loss',False),'Policy likelihood changed; use the saved launch command'
            assert saved.get('reference_foot_placement',False)==reference_placement,'Reference placement reward changed; use the saved launch command'
            assert saved.get('transition_prior_sha256')==transition_prior_hash,'Transition correction anchor changed; use the saved launch command'
            assert saved.get('walk_prior_sha256')==walk_prior_hash,'Walk reference correction anchor changed; use the saved launch command'
            assert saved.get('brisk_support',0.)==brisk_support,'Brisk support changed; use an explicit checkpoint fork'
            assert saved.get('foot_knee_tracking',False)==foot_knee_tracking,'Foot/knee reward changed; use an explicit checkpoint fork'
            assert saved.get('balance_prior_sha256')==balance_prior_hash,'Balance controller/mask changed; use an explicit checkpoint fork'
            assert saved.get('ankle_prior_sha256')==ankle_prior_hash,'Ankle correction anchor changed; use the saved launch command'
            assert saved.get('straight_route',False)==straight_route,'Reference route changed; use an explicit checkpoint fork'
            assert saved.get('walk_reference_authority',.35)==walk_reference_authority,'Walking reference authority changed; use an explicit checkpoint fork'
            assert saved.get('native_support_observations',False)==native_support_observations,'Native support observations changed; use an explicit checkpoint fork'
            assert saved.get('phase_support',0.)==phase_support,'Phase support changed; use an explicit checkpoint fork'
            assert saved.get('stance_reward',False)==stance_reward,'Stance reward changed; use an explicit checkpoint fork'
            assert saved.get('native_leg_ownership',0.)==native_leg_ownership,'Native leg ownership changed; use an explicit checkpoint fork'
            assert saved.get('contact_fidelity',False)==contact_fidelity,'Contact fidelity changed; use an explicit checkpoint fork'
            assert saved.get('walking_expert_sha256')==walking_expert_hash,'Walking expert changed; use an explicit checkpoint fork'
            assert saved.get('jog_native_ownership',0.)==jog_native_ownership,'Jog native ownership changed; use an explicit checkpoint fork'
            assert saved.get('long_transition_practice',False)==long_transition_practice,'Transition practice distribution changed; use an explicit checkpoint fork'
            assert saved.get('jog_route_gain',1.)==jog_route_gain,'Jog course feedback changed; use an explicit checkpoint fork'
            assert saved.get('control_aware_policy_loss',False)==control_aware_policy_loss,'Policy ownership loss changed; use an explicit checkpoint fork'
            assert saved.get('ankle_residual',False)==ankle_residual,'Ankle ownership changed; use an explicit checkpoint fork'
            assert saved.get('ankle_fidelity',False)==ankle_fidelity,'Ankle reward changed; use an explicit checkpoint fork'
            assert saved.get('leg_ankle_learning',False)==leg_ankle_learning,'Leg/ankle learning ownership changed; use an explicit checkpoint fork'
            assert saved.get('jog_stop',False)==jog_stop,'Stop task changed; use an explicit full-state fork'
            assert saved.get('turn_reference')==(str(turn_reference) if turn_reference else None),'Finite trajectory changed; use an explicit checkpoint fork'
            assert saved.get('turn_signal_reward',False)==turn_signal_reward,'Turn learning signal changed; use an explicit full-state fork'
            assert saved.get('whole_body_turn',False)==whole_body_turn,'Whole-body action ownership changed; use an explicit full-state fork'
            assert saved.get('torso_reference_gain',0.)==torso_reference_gain,'Torso motor baseline changed; use an explicit full-state fork'
            assert saved.get('walking_ankle_expert_sha256')==walking_ankle_hash,'Walking ankle anchor changed; use an explicit full-state fork'
            assert saved.get('turn_tempo_practice',False)==turn_tempo_practice,'Training tempo distribution changed; use an explicit full-state fork'
            assert saved.get('cut_push_release',0.)==cut_push_release,'Cut propulsion release changed; use an explicit full-state fork'
            assert saved.get('turn_joint_control',False)==turn_joint_control,'Cut joint control changed; use an explicit full-state fork'
            assert saved.get('reference_balance_reset',False)==reference_balance_reset,'Native reference reset changed; use an explicit full-state fork'
            assert saved.get('turn_duration',52/30)==turn_duration,'Turn timing changed; use an explicit full-state fork'
            assert saved.get('turn_preview_feedback',False)==turn_preview_feedback,'Turn preview inputs changed; use an explicit checkpoint fork'
            assert saved.get('turn_entry_control',False)==turn_entry_control,'Turn entry controller changed; use an explicit checkpoint fork'
            assert saved.get('turn_route_anchor',False)==turn_route_anchor,'Turn route placement changed; use an explicit checkpoint fork'
            assert saved.get('turn_reference_sha256')==turn_hash,'Finite trajectory bytes changed; use the saved reference'
            assert saved.get('turn_straight_jog_limit')==(args.jog_limit if turn_reference else None),'Straight-gait rehearsal limit changed; use the saved launch command'
            assert saved.get('foot_state_feedback',False)==foot_state_feedback,'Foot-state observations changed; use an explicit checkpoint fork'
            assert saved.get('gait_prior_sha256')==gait_prior_hash,'Protected walking controller changed; use an explicit fork'
            assert saved.get('catalog', env.catalog) == env.catalog
            assert saved.get('assisted', False) == args.assisted
            assert saved.get('episode_seconds', env.episode_seconds) == env.episode_seconds
            assert saved.get('evaluation_seconds', env.evaluation_seconds) == env.evaluation_seconds
            assert saved.get('jog_reference') == (str(jog_reference) if jog_reference else None), 'Jog reference changed; use a new output directory'
            assert not jog_reference or saved.get('reward_contract') == reward_contract, 'Jog controller changed; use a new output directory'
            assert saved.get('gait_refinement') == refinement, 'Gait refinement changed; use an explicit checkpoint fork'
            assert saved.get('transition_patience', False) == args.transition_patience, 'Patience policy changed; use an explicit checkpoint fork'
            assert saved.get('jog_stiffness',12.)==args.jog_stiffness,'Jog motor setting changed; use an explicit checkpoint fork'
            assert saved.get('alignment_prior_sha256')==alignment_prior_hash,'Alignment prior changed; use an explicit checkpoint fork'
            assert saved.get('path_gain',1.)==args.path_gain,'Course-correction gain changed; use an explicit checkpoint fork'
            assert saved.get('brisk_reference') == (str(brisk_reference) if brisk_reference else None), 'Middle reference changed; use an explicit checkpoint fork'
            assert saved.get('coordination_targets') == coordination_targets, 'Coordination targets changed; use an explicit checkpoint fork'
            actor.load_state_dict(saved['actor'])
            critic.load_state_dict(saved['critic'])
            log_std.data.copy_(saved['log_std'])
            optimizer.load_state_dict(saved['optimizer'])
            first = saved['update'] + 1
            continuation_start_update = saved.get('continuation_start_update', -1)
            assert isinstance(continuation_start_update, int) and -1 <= continuation_start_update < first, 'Invalid continuation start'
        elif model.exists() and (not args.seed):
            raise ValueError('Model already exists without this PPO checkpoint; refusing to overwrite it')
        elif args.seed:
            saved = torch.load(args.seed, map_location='cpu', weights_only=True)
            assert saved['reference'] == str(reference), 'Seed uses a different reference motion'
            for net, source in [(actor, saved['actor']), (critic, saved['critic'])]:
                with torch.no_grad():
                    for name, value in net.state_dict().items():
                        old = source[name]
                        assert old.ndim == value.ndim and all((a <= b for a, b in zip(old.shape, value.shape)))
                        value.zero_()
                        value[tuple((slice(0, n) for n in old.shape))].copy_(old)
            print(f'Warm start from {args.seed}; continuing with the current trial durations', flush=True)
            if jog_reference:
                with torch.no_grad():
                    log_std[:len(saved['log_std'])].copy_(saved['log_std'])
        if first == 0 and args.behaviors:
            with torch.no_grad():
                actor[-2].bias[60] = float(np.arctanh(0.25))
        export(actor, evaluation_model)
        controls = '4 native walking settings + 24 arm corrections; native leg targets preserved' if args.assisted else f'60 joint corrections, {env.actions - 60} behavior choices'
        if jog_reference:
            controls = '4 balance settings + 24 arm and 18 leg corrections; 50% walking rehearsal, 50% walk/jog transitions'
        print(f'Human walking: CPU PPO, {args.envs} environments, {env.obs} inputs, {controls}; reference {reference}', flush=True)
        print(f'Full Delirium message/parameter catalog: {catalog_file}', flush=True)
        print(f'Training trials {env.episode_seconds:g}s; saved evaluation trials {env.evaluation_seconds:g}s', flush=True)
        status = dict(status='running', pid=os.getpid(), native_pid=env.process.pid, target_updates=args.updates, episode_seconds=env.episode_seconds, evaluation_seconds=env.evaluation_seconds, reference=str(reference), model=str(model), out=str(args.out.resolve()), cpu_only=True, updated_at_unix=time.time(), evaluation_interval_updates=args.eval_every)
        status['training_pipeline'] = 'ppo-rust-v1'
        status['learning_rate'] = learning_rate
        status['training_control'] = CAPABILITY
        status['update_limit_control'] = LIMIT_CAPABILITY
        status['rust_learner_exe'] = os.environ['RUSTLEARN']
        status['reward_contract'] = reward_contract
        status['gait_refinement'] = refinement
        if refinement in ('mocap-push-off-v2','mocap-alignment-v2','mocap-path-v2','mocap-leg-strength-v2','mocap-leg-skill-v2','mocap-actuator-v2'):
            status['training_jog_skill_seconds']=6
            status['training_walking_rehearsal_seconds']=60
            status['training_reference']='full genuine jogging during six-second skill practice; saved blend limit during transitions'
            status['training_transition_rehearsal_every_resets']=4 if transition_prior else 16
            status['controls']='Contact-aware stance motors; short push-off/flight/landing practice, walking rehearsal and full transition evaluations'
            if refinement=='mocap-alignment-v2':status['training_focus']='Reference knee alignment, task heading/lateral drift and stance-foot slip; preserve contact/flight practice'
        if coordinating:
            status['walking_target_speed_m_s'] = coordination_targets['walking_target_speed_m_s']
            status['quality_focus'] = 'foot placement, arm/leg timing, retained foot contact, walking toward 1.5m/s'
        if tracking:
            status['quality_focus'] = 'dense mocap surface position/velocity and joint tracking with retained balance'
            status['speed_target_source'] = 'motion reference; 1.5m/s remains the walking goal'
            status['walking_reference_speed_m_s'] = coordination_targets['walk_reference_speed_m_s']
            status['motion_tracking_native_sha256'] = tracking_native_hash
        status['evaluation_reward'] = 'unchanged native movement score; refinement penalties are training-only'
        if jog_reference:
            status.update(stage='walk-jog', jog_reference=str(jog_reference), curriculum=dict(walk_seconds=[0, 8], ramp_up=[8, 12], jog=[12, 28], ramp_down=[28, 32], walk_again=[32, 44], second_ramp=[44, 48], jog_again=[48, 60], rehearsal_fraction=0.5, reference_state_initialization=True))
        if curriculum:
            status.update(stage='walk-brisk-jog', brisk_reference=str(brisk_reference),
                          curriculum=dict(**gait_curriculum.SCHEDULE, rehearsal_fraction=.5, reference_state_initialization=True),
                          gait_curriculum=copy.deepcopy(curriculum))
        status['jog_adaptive_features']=getattr(args,'jog_adaptive_features',False)
        status['jog_leg_authority']=leg_authority
        status['mixed_jog_practice']=mixed_practice
        status['jog_leg_policy_loss']=getattr(args,'jog_leg_policy_loss',False)
        status['reference_foot_placement']=reference_placement
        status['transition_prior_sha256']=transition_prior_hash
        status['walk_prior_sha256']=walk_prior_hash
        status['brisk_support']=brisk_support
        status['foot_knee_tracking']=foot_knee_tracking
        status['balance_prior_sha256']=balance_prior_hash
        status['ankle_prior_sha256']=ankle_prior_hash
        status['straight_route']=straight_route
        status['walk_reference_authority']=walk_reference_authority
        status['native_support_observations']=native_support_observations
        status['phase_support']=phase_support
        status['stance_reward']=stance_reward
        status['native_leg_ownership']=native_leg_ownership
        status['contact_fidelity']=contact_fidelity
        status['walking_expert_sha256']=walking_expert_hash
        status['walking_expert_source_update']=walking_expert_update
        status['jog_native_ownership']=jog_native_ownership
        status['long_transition_practice']=long_transition_practice
        status['jog_route_gain']=jog_route_gain
        status['control_aware_policy_loss']=control_aware_policy_loss
        status['ankle_residual']=ankle_residual
        status['ankle_fidelity']=ankle_fidelity
        status['leg_ankle_learning']=leg_ankle_learning
        status['foot_state_feedback']=foot_state_feedback
        if walking_expert:
            status['experimental_stage']=True
            status['qualified_natural_gait']=False
            status['learning_scope']='brisk-walk/jog contact; retained walking expert'
        if transition_prior:
            status['development_run']=True
            status['quality_focus']='Refine walk/brisk/jog transitions; broader starting-stride stability is still under training'
        if status['jog_leg_policy_loss']:
            status['controls'] += '; PPO scores22 policy-controlled outputs;28 teacher-owned outputs excluded'
        if mixed_practice:
            status['training_reference']='six-second skills alternate saved curriculum blend and full genuine jogging; walking and full transitions retained'
        if status['jog_adaptive_features']:
            status['controls'] += '; adaptive jogging features, immutable walking/balance/arm controller'
        if walk_prior:
            status['controls'] += '; bounded walking leg feedback, immutable balance/arm support'
            status['quality_focus']='Adapt to repaired symmetric-step walking reference while retaining brisk/jog transitions'
        if brisk_support:
            status['controls']+='; contact-aware leg strength and response also learn during brisk walking'
        if foot_knee_tracking:status['quality_focus']='Reference-relative sole tracking and actual knee-joint position/velocity; retain motion and contact'
        if balance_prior:status['controls']='26 trained outputs:4 bounded native step/balance settings plus22 leg/strength/response controls;24 arm outputs remain teacher-owned'
        if phase_support:
            status['quality_focus']='Walking step-phase/native support coordination; retain brisk/jog stability and reduce sliding'
            status['controls']+='; walking-only gravity-support retiming, fades out during brisk/jog and yields to native recovery'
        if stance_reward:
            status['quality_focus']='Reduce actual planted-foot sliding while retaining movement, posture and jogging contact sequence'
        if native_leg_ownership:
            status['quality_focus']='Experimental walking-only engine leg-target/contact-gated support ownership; fade before brisk/jog and retain transitions'
        if contact_fidelity:
            status['quality_focus']='Learn contact-phase fidelity: stable planned stance, no swing-foot dragging, correctly timed flight; preserve walking and transitions'
        if walking_expert:
            status['quality_focus']='Retain stable walking expert while contact-aware learner refines brisk/jog; smooth transition between experts'
        if jog_native_ownership:
            status['quality_focus']='Retain walking expert while testing actual-contact-gated jogging-phase native stance/leg ownership'
        if long_transition_practice:
            status['quality_focus']='More complete standing-to-walk/brisk/jog sequences and sustained motion, with retained walking expert'
            status['evaluation_reward']='Native evaluation unchanged; stance-quality credit applies only to training rewards'
        if jog_route_gain!=1.:
            status['quality_focus']='Jog leg correction and native course feedback; retained walking expert, no root repositioning'
        if control_aware_policy_loss:
            status['quality_focus']='Learn only from student-controlled actions; walking expert rehearsals train the value model without changing actor/Adam'
        if ankle_residual:
            status['quality_focus']='Refine ankle control across walking, brisk walking and jogging while preserving the gait policy'
            status['learning_scope']='Four ankle motor corrections across all gaits; previous gait/core/arm policy frozen'
            status['controls']='Bounded ankle joint motor targets; physics owns body movement; existing balance and gait remain active'
        if ankle_fidelity:
            status['quality_focus']='Phase-matched foot yaw/roll fidelity with planned stance emphasis; unchanged survival/contact/route evaluation'
        if leg_ankle_learning:
            status['quality_focus']='Coordinate leg and ankle control during brisk/jog; walking rows credit ankle actions only'
            status['learning_scope']='22 student leg controls plus4 ankle controls and shared features; immutable walking expert/native balance/arms'
            status['controls']='PPO likelihood and entropy follow physical action ownership on each row; teacher-owned controls excluded'
        if foot_state_feedback:
            status['quality_focus']='Use actual foot rotation rates and floor contacts to coordinate ankle damping and leg placement'
        status['turn_reference']=str(turn_reference) if turn_reference else None
        status['turn_reference_sha256']=turn_hash
        status['turn_preview_feedback']=turn_preview_feedback
        if turn_preview_feedback:status['lookahead_seconds']=[.25,.5]
        status['turn_signal_reward']=turn_signal_reward
        status['whole_body_turn']=whole_body_turn
        status['torso_reference_gain']=torso_reference_gain
        status['walking_ankle_expert']=str(walking_ankle_expert) if walking_ankle_expert else None
        status['turn_tempo_practice']=turn_tempo_practice
        if turn_tempo_practice:status['training_cut_durations_seconds']=[3.,2.5,3.,52/30]
        status['cut_push_release']=cut_push_release
        status['turn_joint_control']=turn_joint_control
        status['reference_balance_reset']=reference_balance_reset
        if reference_balance_reset:status['reset_initialization']='Native balance cache activated after reset-only reference placement; calibrated standing zero and10000step budget preserved'
        if turn_joint_control:status['cut_joint_scope']='14 bounded hip/knee/ankle DOFs16..26s; native effector limits, strengths, damping and balance preserved'
        if whole_body_turn:status['learning_scope']='Existing leg/ankle controls plus12 spine and24 shoulder/arm residuals after8s in finite jog/turn trials; pure walking expert retained'
        if turn_signal_reward:status['training_reward']='Bounded per-term turn tracking after16s; native evaluation reward and quality gates unchanged'
        status['turn_entry_control']=turn_entry_control
        status['turn_route_anchor']=turn_route_anchor
        if turn_reference:
            status.update(stage='walk-jog-turn',curriculum=task_curriculum.schedule(turn_duration),turn_duration=turn_duration,qualified_natural_gait=False,
                quality_focus='Captured sharp turn and return with route-aware physical steering; retained walking rehearsal',
                training_reference='Finite60-second target; reference-state practice near turn; no cyclic endpoint wrap',
                training_jog_skill_seconds=None,experimental_stage=True)
        save_json(args.out / 'status.json', status)
        print('Evaluation columns: pose error rad, stance slip m/s, action jitter, forward speed m/s, walking score, survival seconds', flush=True)
        if jog_stop:
            status.update(stage='walk-jog-stop',curriculum=stop_curriculum.schedule(),jog_stop=True,quality_focus='Physical braking and stable standing after jogging; retained walking rehearsal',cut_joint_scope='14 bounded leg DOFs remain active through braking and hold',training_reference='Finite captured jog-stop followed by standing hold; no cyclic wrap')
            save_json(args.out/'status.json',status)
        if preview:
            preview.send('evaluating', first, force=True)
        initial = env.evaluate(max(first - 1, 0) if refinement else first)
        initial_evaluation = initial.copy()
        initial_coordination = coordination.replay_metrics(Path(str(evaluation_model) + '.live'), replay_profile(), max(first - 1, 0)) if coordinating else None
        if contact_refinement and curriculum and 'contact_progress' not in curriculum:
            initial_report=json.loads(Path(str(evaluation_model)+'.tracking.json').read_text())
            initial_contact=gait_curriculum.contact_progress(initial,initial_report)
            curriculum['contact_progress']=dict(update=max(first-1,0),highwater=initial_contact.tolist() if initial_contact is not None else None)
        if alignment_refinement and curriculum and 'alignment_progress' not in curriculum:
            initial_alignment=gait_curriculum.alignment_progress(initial,json.loads(Path(str(evaluation_model)+'.tracking.json').read_text()))
            curriculum['alignment_progress']=dict(update=max(first-1,0),best=initial_alignment.tolist() if initial_alignment is not None else None)
        print('initial evaluation:', np.round(initial.mean(0), 4).tolist(), flush=True)
        obs = torch.from_numpy(env.step()[:, :env.obs].copy())
        horizon, gamma, lam = (64, 0.99, 0.95)
        best_file = args.out / 'best.json'
        if best_file.exists() and json.loads(best_file.read_text()).get('reward_contract') != reward_contract:
            previous = json.loads(best_file.read_text())
            save_json(args.out / 'best-before-contact-calibration.json', previous)
            shutil.copyfile(args.out / 'best.net', evaluation_model)
            rescored = env.evaluate(previous['update'])
            previous.update(metrics=rescored.mean(0).tolist(), evaluation_seconds=env.evaluation_seconds, eligible=eligible_walk(rescored, env.evaluation_seconds), reward_contract=reward_contract)
            save_json(best_file, previous)
            shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best.net.live')
            export(actor, evaluation_model)
            # Rescoring an older best overwrites its tracking/replay files. Restore
            # the current policy evidence before checking the initial turn rows.
            restored = env.evaluate(max(first - 1, 0) if refinement else first)
            np.testing.assert_array_equal(restored, initial_evaluation)
            print('Re-evaluated saved best using calibrated native foot colliders', flush=True)
        best = json.loads(best_file.read_text())['metrics'][4] if best_file.exists() else float(initial[:, 4].mean())
        best_update = json.loads(best_file.read_text())['update'] if best_file.exists() else (max(first - 1, 0) if refinement else first)
        transition_file = args.out / 'transition-best.json'
        transition_best_update, transition_highwater = (best_update, None)
        if jog_reference:
            if transition_file.exists():
                transition_record = json.loads(transition_file.read_text())
                transition_best_update = transition_record['update']
                transition_highwater = np.array(transition_record['highwater']) if transition_record['highwater'] is not None else None
            else:
                transition_best_update = max(first - 1, 0)
                transition_highwater = task_curriculum.progress(initial,json.loads(Path(str(evaluation_model)+'.tracking.json').read_text())) if turn_reference else transition_progress(initial, env.evaluation_seconds)
                save_json(transition_file, dict(update=transition_best_update, highwater=transition_highwater.tolist() if transition_highwater is not None else None))
                export(actor, args.out / 'best-transition.net')
                shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best-transition.net.live')
        if not best_file.exists():
            export(actor, args.out / 'best.net')
            shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best.net.live')
            save_json(best_file, dict(update=best_update, metrics=initial.mean(0).tolist(), reference=str(reference), evaluation_seconds=env.evaluation_seconds, eligible=eligible_walk(initial, env.evaluation_seconds) and not (curriculum and curriculum['limit']<1.) and not turn_reference, reward_contract=reward_contract))
        if args.assisted:
            publish_best(args.out, model)
        choices = ['referenceMotors'] + list(dict.fromkeys((line.split('\t')[0] for line in env.catalog.splitlines()[1:] if line.split('\t')[1].startswith('selectable;'))))
        if args.behaviors:
            assert len(choices) == env.actions - 60, 'Behavior action mapping does not match the registry'
        stop_reason = 'update_limit'
        update = first - 1
        timing_window = []
        roll_actor, roll_critic = (copy.deepcopy(actor), copy.deepcopy(critic))
        roll_std = log_std.detach().clone()
        pending, last = (None, {})
        eval_job = None
        eval_env = Native(args.exe.resolve(), reference, evaluation_model, 1, args.out / 'native-eval.log', args.behaviors, args.assisted, jog_reference, tracking=tracking,
                          brisk_reference=brisk_reference, jog_limit=curriculum['limit'] if curriculum else (args.jog_limit if turn_reference else 1.),jog_stiffness=args.jog_stiffness)
        RL = os.environ.get('RUSTLEARN')
        learn_params = list(actor.parameters()) + [log_std] + list(critic.parameters())

        def checkpoint_template():
            return dict(actor=actor.state_dict(), critic=critic.state_dict(), log_std=log_std.detach(), optimizer=optimizer.state_dict(), update=first - 1, reference=str(reference), obs=env.obs, actions=env.actions, catalog=env.catalog, assisted=args.assisted, episode_seconds=env.episode_seconds, evaluation_seconds=env.evaluation_seconds, reward_contract=reward_contract, jog_reference=str(jog_reference) if jog_reference else None, rollout_rng=torch.get_rng_state(), native_walk_prior_sha256=prior_hash, continuation_start_update=continuation_start_update, gait_refinement=refinement, coordination_targets=coordination_targets, learning_rate=learning_rate, motion_tracking_native_sha256=tracking_native_hash, transition_patience=args.transition_patience,
                        brisk_reference=str(brisk_reference) if brisk_reference else None, gait_curriculum=copy.deepcopy(curriculum),jog_stiffness=args.jog_stiffness,alignment_prior_sha256=alignment_prior_hash,path_gain=args.path_gain,rust_learner_sha256=learner_hash,gait_prior_sha256=gait_prior_hash,jog_adaptive_features=getattr(args,'jog_adaptive_features',False),jog_leg_authority=leg_authority,mixed_jog_practice=mixed_practice,jog_leg_policy_loss=getattr(args,'jog_leg_policy_loss',False),reference_foot_placement=reference_placement,transition_prior_sha256=transition_prior_hash,walk_prior_sha256=walk_prior_hash,brisk_support=brisk_support,foot_knee_tracking=foot_knee_tracking,balance_prior_sha256=balance_prior_hash,ankle_prior_sha256=ankle_prior_hash,straight_route=straight_route,walk_reference_authority=walk_reference_authority,native_support_observations=native_support_observations,phase_support=phase_support,stance_reward=stance_reward,native_leg_ownership=native_leg_ownership,contact_fidelity=contact_fidelity,walking_expert_sha256=walking_expert_hash,jog_native_ownership=jog_native_ownership,long_transition_practice=long_transition_practice,jog_route_gain=jog_route_gain,control_aware_policy_loss=control_aware_policy_loss,ankle_residual=ankle_residual,ankle_fidelity=ankle_fidelity,leg_ankle_learning=leg_ankle_learning,foot_state_feedback=foot_state_feedback,turn_reference=str(turn_reference) if turn_reference else None,turn_reference_sha256=turn_hash,turn_entry_control=turn_entry_control,turn_route_anchor=turn_route_anchor,turn_preview_feedback=turn_preview_feedback,turn_duration=turn_duration,turn_signal_reward=turn_signal_reward,whole_body_turn=whole_body_turn,torso_reference_gain=torso_reference_gain,reference_balance_reset=reference_balance_reset,turn_joint_control=turn_joint_control,cut_push_release=cut_push_release,walking_ankle_expert=str(walking_ankle_expert) if walking_ankle_expert else None,walking_ankle_expert_sha256=walking_ankle_hash,turn_tempo_practice=turn_tempo_practice,jog_stop=jog_stop,turn_straight_jog_limit=args.jog_limit if turn_reference else None)
        initial = warm_start(checkpoint_template())
        blob = saved.get('rust_learner_state') if checkpoint.exists() else None
        if blob is None:
            blob = encode(initial)
        state = decode(blob)
        if checkpoint.exists() and saved.get('rollout_rng') is not None:
            torch.set_rng_state(saved['rollout_rng'])
        assert state['completed_updates'] == first, 'Rust checkpoint update counter differs'
        for key in ('parameters', 'exp_avg', 'exp_avg_sq'):
            assert np.array_equal(state[key].view('u4'), initial[key].view('u4')), 'Torch and Rust checkpoint state differ: ' + key
        rust_client = Client(RL, initial['dimensions'], args.envs, horizon, 4, blob, args.out / 'rust-wire', learning_rate=learning_rate)
        behavior = state['snapshot'] if state['snapshot'] is not None else state['parameters']
        with torch.no_grad():
            nn.utils.vector_to_parameters(torch.from_numpy(behavior.copy()), list(roll_actor.parameters()) + [roll_std] + list(roll_critic.parameters()))
        waiting = False

        def learner_reply():
            kl, seconds, parameters = rust_client.receive()
            with torch.no_grad():
                nn.utils.vector_to_parameters(torch.from_numpy(parameters.copy()), learn_params)
            return (kl, seconds)

        def capture_checkpoint(update):
            blob, state = rust_client.save()
            return freeze_checkpoint(checkpoint_template(), state, update=update, state_blob=blob, state_format='RLS2')

        quality_file = args.out / 'gait-quality-best.json'
        anchor_file = args.out / 'gait-quality-anchor.json'
        quality_record = json.loads(quality_file.read_text()) if refinement and quality_file.exists() else None
        anchor_data = json.loads(anchor_file.read_text()) if refinement and anchor_file.exists() else dict(evaluation=initial_evaluation.tolist(), coordination=initial_coordination)
        quality_anchor = np.array(anchor_data['evaluation'])

        def save_quality(update, evaluation, snapshot, policy, coordination_metrics=None):
            nonlocal quality_record
            quality_record = dict(update=update, evaluation=evaluation.tolist(), gait_refinement=refinement,
                                  criterion='Lower native stance slip with fixed survival/speed/pose/jitter guards; not final gait acceptance')
            if coordinating:
                quality_record.update(coordination=coordination_metrics, coordination_score=coordination.score(coordination_metrics),
                    criterion='Better placement/arm timing/speed score with fixed movement/contact/pose guards; not final gait acceptance')
            if tracking:
                quality_record['criterion'] = 'Better dense mocap tracking with fixed full-trial movement/contact/pose/coordination guards; not final gait acceptance'
            export(policy, args.out / 'best-gait.net')
            shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best-gait.net.live')
            temporary = args.out / 'best-gait.tmp'
            torch.save(snapshot, temporary)
            replace_saved_file(temporary, args.out / 'best-gait.pt')
            save_json(quality_file, quality_record)
            if tracking:
                report = json.loads(Path(str(evaluation_model) + '.tracking.json').read_text())
                assert report['update'] == update, 'Motion-tracking terms belong to another evaluation'
                save_json(args.out / 'best-gait.tracking.json', report)

        def improves_quality(evaluation, measures):
            if turn_reference:
                report=json.loads(Path(str(evaluation_model)+'.tracking.json').read_text())
                return task_curriculum.guarded(evaluation,report,quality_anchor) and (quality_record is None or evaluation[:,4].mean()>np.asarray(quality_record['evaluation'])[:,4].mean()+.005)
            if curriculum and curriculum['limit'] < 1.:
                return False
            if contact_refinement and not gait_curriculum.flight_ready(json.loads(Path(str(evaluation_model)+'.tracking.json').read_text())):
                return False
            if tracking:
                guarded = coordination.improved(evaluation, quality_anchor, measures, anchor_data['coordination'], None)
                previous = np.array(quality_record['evaluation'])[:, 4].mean() if quality_record else None
                return guarded and (previous is None or evaluation[:, 4].mean() > previous + .005)
            if coordinating:
                return coordination.improved(evaluation, quality_anchor, measures, anchor_data['coordination'],
                    quality_record.get('coordination') if quality_record else None)
            return quality_improved(evaluation, quality_anchor, quality_record['evaluation'] if quality_record else None, env.evaluation_seconds)

        if refinement and not anchor_file.exists():
            save_json(anchor_file, dict(**anchor_data, source_update=max(first - 1, 0)))
        if refinement and quality_record is None and improves_quality(initial_evaluation, initial_coordination):
            save_quality(max(first - 1, 0), initial_evaluation, capture_checkpoint(first - 1), actor, initial_coordination)
        pause_request = PauseRequest(args.out)
        promotion_pending = False
        for update, args.updates in update_range(args.out, first, args.updates):
            if status['target_updates'] != args.updates:
                status.update(target_updates=args.updates, updated_at_unix=time.time())
                save_json(args.out / 'status.json', status)
            start = time.monotonic()
            torch.set_num_threads(1)
            timing = dict(native_rollout=0.0, policy=0.0)
            observations, actions, probabilities, rewards, dones, values, metrics = ([], [], [], [], [], [], [])
            means = []
            with torch.inference_mode():
                fast_std = roll_std.clamp(-4, -0.5).exp()
            tick = time.perf_counter()
            with torch.inference_mode():
                block = torch.from_numpy(env.rollout(roll_actor, fast_std, torch.randn(horizon, env.count, env.actions)))
                if refinement:
                    packet = block[:, :, env.actions:]
                    penalty = gait_penalties(packet.numpy(), env.obs, refinement, coordination_targets,reference_placement=reference_placement,native_support_observations=native_support_observations,foot_state_feedback=foot_state_feedback,turn_preview_feedback=turn_preview_feedback,whole_body_turn=whole_body_turn,turn_joint_control=turn_joint_control)
                    packet[:, :, env.obs].sub_(torch.from_numpy(penalty))
            timing['native_rollout'] += time.perf_counter() - tick
            for t in range(horizon):
                packet = block[t, :, env.actions:]
                observations.append(obs)
                actions.append(block[t, :, :env.actions])
                means.append(None)
                rewards.append(packet[:, env.obs].clone())
                dones.append(packet[:, env.obs + 1].clone())
                metrics.append(packet[:, env.obs + 2:].numpy())
                obs = packet[:, :env.obs].clone()
            if preview:
                preview.send('learning', update, force=True)
            rewards_all = torch.stack(rewards)
            if waiting:
                kl, timing['learning'] = learner_reply()
            else:
                timing['learning'] = 0.0
                kl = 0.0
            with torch.no_grad():
                roll_actor.load_state_dict(actor.state_dict())
                roll_critic.load_state_dict(critic.state_dict())
                roll_std.copy_(log_std)
            rust_client.submit(torch.stack(observations).numpy(), torch.stack(actions).numpy(), rewards_all.numpy(), torch.stack(dones).numpy(), obs.numpy())
            waiting = True
            rewards = rewards_all
            timing['update_before_evaluation'] = time.monotonic() - start
            timing_window.append(timing)
            timing_window = timing_window[-20:]
            m = np.mean(metrics, axis=(0, 1))
            print(f'update {update:4} reward {rewards.mean():.4f} pose {m[0]:.3f} slip {m[1]:.3f} jitter {m[2]:.4f} speed {m[3]:.3f} KL {kl:.4f} {time.monotonic() - start:.1f}s', flush=True)
            pausing = pause_request.requested()
            if pausing or update % args.eval_every == args.eval_every - 1 or update + 1 == args.updates:
                evaluation_start = time.perf_counter()
                if waiting:
                    learner_reply()
                    waiting = False

                def finish(update, box, actor, critic, log_std, observations):
                    nonlocal best, best_update, transition_highwater, transition_best_update, promotion_pending
                    if 'error' in box:
                        raise RuntimeError('Background evaluation failed') from box['error']
                    evaluation, evaluation_seconds = (box['e'], box['s'])
                    average = evaluation.mean(0)
                    print(f'  evaluated {update}: {np.round(average, 4).tolist()}', flush=True)
                    eligible = not args.assisted or eligible_walk(evaluation, env.evaluation_seconds)
                    tested_limit = box['checkpoint']['gait_curriculum']['limit'] if curriculum else None
                    coordination_measures = coordination.replay_metrics(Path(str(evaluation_model) + '.live'), replay_profile(tested_limit), update) if coordinating else None
                    contact_report = json.loads(Path(str(evaluation_model)+'.tracking.json').read_text()) if contact_refinement or turn_reference else None
                    if contact_report: assert contact_report['update']==update,'Contact evidence belongs to another evaluation'
                    if alignment_refinement and curriculum:
                        current_alignment=gait_curriculum.alignment_progress(evaluation,contact_report)
                        record=curriculum['alignment_progress']
                        if gait_curriculum.alignment_gain(current_alignment,record['best']):
                            record.update(update=update,best=current_alignment.tolist())
                            box['checkpoint']['gait_curriculum']['alignment_progress']=copy.deepcopy(record)
                            torch.save(box['checkpoint'],args.out/'best-alignment.tmp')
                            replace_saved_file(args.out/'best-alignment.tmp',args.out/'best-alignment.pt')
                            export(actor,args.out/'best-alignment.net')
                            shutil.copyfile(Path(str(evaluation_model)+'.live'),args.out/'best-alignment.net.live')
                            save_json(args.out/'best-alignment.json',dict(update=update,terms=record['best'],contact=gait_curriculum.contact_summary(contact_report),qualified_jog=False))
                        box['checkpoint']['gait_curriculum']['alignment_progress']=copy.deepcopy(record)
                    if contact_refinement and curriculum:
                        current_contact=gait_curriculum.contact_progress(evaluation,contact_report)
                        record=curriculum['contact_progress'];previous_contact=record['highwater']
                        if gait_curriculum.contact_gain(current_contact,previous_contact):
                            highwater=current_contact if previous_contact is None else np.maximum(previous_contact,current_contact)
                            record.update(update=update,highwater=highwater.tolist())
                            box['checkpoint']['gait_curriculum']['contact_progress']=copy.deepcopy(record)
                            torch.save(box['checkpoint'],args.out/'best-contact.tmp')
                            replace_saved_file(args.out/'best-contact.tmp',args.out/'best-contact.pt')
                            export(actor,args.out/'best-contact.net')
                            shutil.copyfile(Path(str(evaluation_model)+'.live'),args.out/'best-contact.net.live')
                            save_json(args.out/'best-contact.json',dict(update=update,limit=tested_limit,highwater=record['highwater'],summary=gait_curriculum.contact_summary(contact_report)))
                        box['checkpoint']['gait_curriculum']['contact_progress']=copy.deepcopy(record)
                    if turn_reference:eligible=task_curriculum.guarded(evaluation,contact_report,quality_anchor)
                    if tracking and not turn_reference:
                        eligible = eligible and coordination.improved(evaluation, quality_anchor, coordination_measures, anchor_data['coordination'], None)
                    if contact_refinement:
                        eligible = eligible and gait_curriculum.flight_ready(contact_report)
                    if curriculum and tested_limit < 1.:
                        passes = gait_curriculum.ready(evaluation, quality_anchor, coordination_measures, anchor_data['coordination'], env.evaluation_seconds)
                        if contact_refinement: passes = passes and gait_curriculum.flight_ready(contact_report)
                        curriculum['passes'] = min(3,curriculum['passes']+1) if passes else 0
                        box['checkpoint']['gait_curriculum']['passes']=curriculum['passes']
                        promotion_pending = curriculum['passes'] >= 3
                        if promotion_pending:
                            stage_file = args.out / f"curriculum-qualified-{tested_limit:.2f}-{update}.pt"
                            torch.save(box['checkpoint'], stage_file)
                            save_json(stage_file.with_suffix('.json'), dict(update=update, limit=tested_limit, evaluation=evaluation.tolist(), coordination=coordination_measures))
                        eligible = False
                    if refinement and improves_quality(evaluation, coordination_measures):
                        save_quality(update, evaluation, box['checkpoint'], actor, coordination_measures)
                    if jog_reference:
                        current_transition = task_curriculum.progress(evaluation,contact_report) if turn_reference else transition_progress(evaluation, env.evaluation_seconds)
                        if (task_curriculum.improved(current_transition,transition_highwater) if turn_reference else transition_improved(current_transition, transition_highwater)):
                            transition_highwater = current_transition if transition_highwater is None else np.maximum(transition_highwater, current_transition)
                            transition_best_update = update
                            save_json(transition_file, dict(update=update, highwater=transition_highwater.tolist()))
                            export(actor, args.out / 'best-transition.net')
                            shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best-transition.net.live')
                    if average[4] > best and (eligible or not json.loads(best_file.read_text()).get('eligible', False)):
                        best = average[4]
                        best_update = update
                        export(actor, args.out / 'best.net')
                        shutil.copyfile(Path(str(evaluation_model) + '.live'), args.out / 'best.net.live')
                        save_json(best_file, dict(update=update, metrics=average.tolist(), reference=str(reference), evaluation_seconds=env.evaluation_seconds, eligible=eligible, reward_contract=reward_contract))
                        if refinement:
                            temporary = args.out / 'best-full.tmp'
                            torch.save(box['checkpoint'], temporary)
                            replace_saved_file(temporary, args.out / 'best-full.pt')
                        if args.assisted:
                            publish_best(args.out, model)
                    checkpoint_start = time.perf_counter()
                    temporary = checkpoint.with_suffix('.tmp')
                    assert box['checkpoint']['update'] == update
                    torch.save(box['checkpoint'], temporary)
                    replace_saved_file(temporary, checkpoint)
                    checkpoint_seconds = time.perf_counter() - checkpoint_start
                    progress = dict(update=update, best_update=best_update, updates_since_best=update - best_update, evaluation=dict(zip(['pose_error_rad', 'stance_slip_m_s', 'action_jitter', 'forward_speed_m_s', 'walking_score', 'survival_seconds'], average.tolist())), forward_distance_m=float(np.mean(evaluation[:, 3] * evaluation[:, 5])), best_walking_score=float(best), saved_at_unix=time.time(), cpu_only=True)
                    progress['timings_ms'] = {name: float(np.mean([row[name] for row in timing_window]) * 1000) for name in timing}
                    progress['training_pipeline'] = 'ppo-rust-v1'
                    progress['learner_update'] = update
                    progress['continuation_start_update'] = continuation_start_update
                    progress['updates_since_patience_gain'] = patience_age(update, transition_best_update if jog_reference else best_update, continuation_start_update)
                    progress['rust_learner_format'] = 'RLS2'
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
                        progress['transition_highwater'] = dict(zip(['mean_survival_seconds', 'forward_distance_m'], transition_highwater.tolist())) if transition_highwater is not None else None
                        progress['plateau_metric'] = 'moving transition survival/distance with full walking rehearsal'
                    progress['status'] = 'running'
                    progress['episode_seconds'] = env.episode_seconds
                    progress['evaluation_seconds'] = env.evaluation_seconds
                    progress['target_updates'] = args.updates
                    progress['reward_contract'] = reward_contract
                    progress['gait_refinement'] = refinement
                    if refinement:
                        progress['gait_quality_best_update'] = quality_record['update'] if quality_record else None
                        progress['gait_quality_best_slip_m_s'] = float(np.array(quality_record['evaluation'])[:, 1].mean()) if quality_record else None
                        progress['plateau_metric'] = 'lower stance slip under full-trial movement/pose/jitter guards; not final gait acceptance'
                        progress['updates_since_gait_quality_best'] = update - (quality_record['update'] if quality_record else first - 1)
                        progress['updates_since_patience_gain'] = patience_age(update, quality_record['update'] if quality_record else first - 1, continuation_start_update)
                        if coordinating:
                            progress['coordination'] = coordination_measures
                            progress['gait_quality_best_coordination'] = quality_record['coordination'] if quality_record else None
                            progress['walking_target_speed_m_s'] = coordination_targets['walking_target_speed_m_s']
                            progress['plateau_metric'] = 'placement/arm timing/speed score under full-trial movement/contact/pose guards'
                    progress['candidate_eligible'] = bool(eligible)
                    if tracking:
                        report = json.loads(Path(str(evaluation_model) + '.tracking.json').read_text())
                        assert report['update'] == update, 'Motion-tracking terms belong to another evaluation'
                        progress['motion_tracking'] = dict(zip(['surface_position', 'surface_velocity', 'local_pose', 'root_velocity'], np.array(report['terms']).mean(0).tolist()))
                        if 'path' in report:progress['requested_route']=dict(columns=report['path_columns'],trials=report['path'])
                        progress['plateau_metric'] = 'dense mocap tracking under unchanged full-trial/contact/pose/coordination guards'
                    patience_best = (quality_record['update'] if quality_record else first - 1) if refinement else (transition_best_update if jog_reference else best_update)
                    patience_best = patience_reference(patience_best, transition_best_update, args.transition_patience)
                    if args.transition_patience:
                        if curriculum:
                            patience_best=max(patience_best,curriculum['last_promotion'])
                            if contact_refinement:patience_best=max(patience_best,curriculum['contact_progress']['update'])
                            if alignment_refinement:patience_best=max(patience_best,curriculum['alignment_progress']['update'])
                        progress['transition_patience'] = True
                        progress['updates_since_patience_gain'] = patience_age(update, patience_best, continuation_start_update)
                        progress['plateau_metric'] = 'qualified gait OR meaningful moving transition gain with full walking rehearsal; acceptance checks unchanged'
                    if curriculum:
                        progress['gait_curriculum'] = dict(limit=tested_limit, passes=curriculum['passes'], required_passes=3,
                            last_promotion=curriculum['last_promotion'], history=copy.deepcopy(curriculum['history']))
                        progress['plateau_metric'] = 'new-gait curriculum gain or guarded transition gain; only full-limit gait can qualify'
                    if contact_refinement:
                        if alignment_refinement:
                            progress['alignment_terms']=np.asarray(contact_report['alignment']).mean(0).tolist()
                            progress['alignment_best_update']=curriculum['alignment_progress']['update']
                            progress['alignment_best_terms']=curriculum['alignment_progress']['best']
                        progress['jog_contact']=gait_curriculum.contact_summary(contact_report)
                        progress['flight_gate_passed']=gait_curriculum.flight_ready(contact_report)
                        if curriculum:
                            progress['contact_best_update']=curriculum['contact_progress']['update']
                            progress['updates_since_contact_best']=update-curriculum['contact_progress']['update']
                            progress['contact_highwater']=copy.deepcopy(curriculum['contact_progress']['highwater'])
                    if turn_reference:
                        progress['stage']='walk-jog-stop' if jog_stop else 'walk-jog-turn'
                        progress['turn']=task_curriculum.summary(evaluation,contact_report)
                        progress['transition_highwater']=dict(zip(stop_curriculum.PROGRESS_COLUMNS if jog_stop else ['mean_survival_seconds','negative_return_heading_cost','mean_return_progress_m'],transition_highwater.tolist())) if transition_highwater is not None else None
                        progress['plateau_metric']='Stop survival, braking latency and hold stability with full walking rehearsal; natural gait unqualified' if jog_stop else 'Turn survival, return heading and actual return distance with full walking rehearsal; natural gait unqualified'
                        if jog_stop:progress['stop']=progress.pop('turn')
                    progress['best_evaluation'] = json.loads(best_file.read_text())['metrics']
                    if args.behaviors:
                        counts = observations[:, :, 325:325 + len(choices)].mean((0, 1)).numpy()
                        progress['behavior_usage'] = dict(zip(choices, counts.tolist()))
                        top = sorted(progress['behavior_usage'].items(), key=lambda pair: -pair[1])[:3]
                        print('  behaviors: ' + ', '.join((f'{name} {fraction:.0%}' for name, fraction in top)), flush=True)
                    save_json(args.out / 'progress.json', progress)
                    if args.assisted and patience_age(update, patience_best, continuation_start_update) >= 10000:
                        return True
                    return False
                if preview:
                    preview.send('evaluating', update, force=True)
                if eval_job:
                    eval_job[0].join()
                    plateau = finish(*eval_job[1:])
                    eval_job = None
                else:
                    plateau = False
                if pausing or plateau:
                    export(actor, evaluation_model)
                    final_box = dict(checkpoint=capture_checkpoint(update), e=eval_env.evaluate(update), s=time.perf_counter() - evaluation_start)
                    plateau = finish(update, final_box, actor, critic, log_std, observations)
                    if pausing or plateau:
                        stop_reason = 'user_pause' if pausing else 'plateau'
                        break
                    continue
                if promotion_pending:
                    old=curriculum['limit']; new=gait_curriculum.next_limit(old)
                    curriculum.update(limit=new, passes=0, last_promotion=update)
                    if contact_refinement:curriculum['contact_progress']=dict(update=update,highwater=None)
                    curriculum['history'].append(dict(update=update, previous_limit=old, limit=new))
                    env.set_jog_limit(new); eval_env.set_jog_limit(new)
                    transition_best_update=update; transition_highwater=None
                    save_json(transition_file, dict(update=update, highwater=None, limit=new))
                    best=float('-inf'); best_update=update
                    status['gait_curriculum']=copy.deepcopy(curriculum)
                    save_json(args.out/'status.json',status)
                    save_json(args.out/'curriculum.json',curriculum)
                    torch.save(capture_checkpoint(update),checkpoint.with_suffix('.tmp'))
                    replace_saved_file(checkpoint.with_suffix('.tmp'),checkpoint)
                    promotion_pending=False
                    print(f'Curriculum {old:.2f} -> {new:.2f}: three guarded full60s evaluations; final jogging not yet accepted',flush=True)
                snap = (copy.deepcopy(actor), copy.deepcopy(critic), log_std.detach().clone())
                export(actor, evaluation_model)
                box = dict(checkpoint=capture_checkpoint(update))

                def job(update=update, box=box, start=evaluation_start):
                    try:
                        box.update(e=eval_env.evaluate(update), s=time.perf_counter() - start)
                    except BaseException as error:
                        box['error'] = error
                thread = threading.Thread(target=job)
                thread.start()
                eval_job = (thread, update, box, *snap, observations)
                if update + 1 == args.updates:
                    thread.join()
                    finish(*eval_job[1:])
                    eval_job = None
        final_status = 'paused' if stop_reason == 'user_pause' else 'completed'
        status.update(status=final_status, stop_reason=stop_reason, updated_at_unix=time.time())
        if preview:
            preview.send('completed', update, force=True)
        save_json(args.out / 'status.json', status)
        if (args.out / 'progress.json').exists():
            progress = json.loads((args.out / 'progress.json').read_text())
            progress.update(status=final_status, stop_reason=stop_reason, saved_at_unix=time.time())
            save_json(args.out / 'progress.json', progress)
    except BaseException as error:
        if preview:
            preview.send('failed', 0, force=True)
        save_json(args.out / 'status.json', dict(status='failed', error=str(error), updated_at_unix=time.time()))
        raise
    finally:
        if eval_job is not None:
            eval_job[0].join(timeout=30)
        if rust_client is not None:
            rust_client.close()
        if eval_env is not None:
            eval_env.close()
        if preview:
            preview.close()
        env.close()
