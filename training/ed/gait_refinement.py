"""Optional bounded gait penalties; native movement reward and physics stay intact."""
import numpy as np
from coordination import MODES as COORDINATION_MODES, TRACKING_MODES, observation_cost

MODES = ('slip-v1', 'slip-arm-v1') + COORDINATION_MODES
BASE_CONTRACT = 'native-assisted-walk-jog-aligned-v3'


def contract(mode):
    if mode is None:
        return BASE_CONTRACT
    if mode not in MODES:
        raise ValueError('Unknown gait refinement')
    return BASE_CONTRACT + ':' + mode


def penalties(packet, obs, mode, targets=None, reference_placement=False, native_support_observations=False, foot_state_feedback=False, turn_preview_feedback=False, whole_body_turn=False, turn_joint_control=False):
    """Packet contains post-step observations, reward/done, then six native metrics.

    Locomotion observation: 196 body values, 3 phase values, 20 local-joint
    rotation-error vectors. Terminal observations belong to the next reset, so
    they must not supply an imitation penalty for the transition that just ended.
    """
    expected=333 if mode=='mocap-actuator-v2' else 331 if mode in ('mocap-leg-strength-v2','mocap-leg-skill-v2') else 329 if mode in ('mocap-alignment-v2','mocap-path-v2') else 323 if mode=='mocap-push-off-v2' else 313
    if native_support_observations:
        if mode!='mocap-actuator-v2':raise ValueError('Native support inputs require the actuator packet')
        expected+=4
    if foot_state_feedback:
        if mode!='mocap-actuator-v2' or native_support_observations:raise ValueError('Foot-state inputs require the explicit341 actuator packet')
        expected+=8
    if whole_body_turn:
        if not turn_preview_feedback:raise ValueError('Whole-body motors require the finite-turn planned-goal packet')
        expected+=1
    if turn_joint_control:
        if not whole_body_turn:raise ValueError('Cut joint packet requires whole-body motors')
        expected+=1
    if turn_preview_feedback:
        if not foot_state_feedback:raise ValueError('Turn preview inputs require the retained foot-state packet')
        expected+=8
    if mode not in MODES or obs != expected or packet.shape[-1] != obs + 8:
        raise ValueError('Gait refinement requires the matching verified walk/jog packet')
    if not np.isfinite(packet).all():
        raise ValueError('Nonfinite gait packet')
    slip = packet[..., obs + 3]
    penalty = .08 * np.minimum(np.maximum(slip, 0.), 2.) ** 2
    if mode in TRACKING_MODES:
        if targets is None:
            raise ValueError('Motion tracking requires calibrated targets')
        # Native dense tracking already measures limb velocities and reference speed.
        penalty += observation_cost(packet, targets, False, match_speed=False,reference_placement=reference_placement)
        return np.where(packet[..., obs + 1] != 0, 0., penalty).astype(np.float32)
    if mode == 'slip-arm-v1' or mode in COORDINATION_MODES:
        errors = packet[..., 199:259].reshape(*packet.shape[:-1], 20, 3)
        # Upper arms, forearms and hands; preserve their native mapped reference,
        # rather than invent an arm/leg amplitude or rewarding stationary arms.
        arm_error = np.mean(np.sum(errors[..., [13, 14, 15, 17, 18, 19], :] ** 2, axis=-1), axis=-1)
        penalty += .15 * (1. - np.exp(-2. * arm_error))
    if mode in COORDINATION_MODES:
        if targets is None:
            raise ValueError('Coordination reward requires calibrated targets')
        penalty += observation_cost(packet, targets, mode == 'coordination-v2')
    return np.where(packet[..., obs + 1] != 0, 0., penalty).astype(np.float32)


def quality_improved(evaluation, anchor, previous, seconds):
    """Select lower slip without buying it through falls, slowing, or worse pose/jitter.

    This is model selection and patience, not final human-gait acceptance. Keep
    the anchor fixed across restarts; arm coordination needs replay assessment.
    """
    e, a = np.asarray(evaluation), np.asarray(anchor)
    if e.shape != (8, 6) or a.shape != e.shape or not np.isfinite(e).all() or not np.isfinite(a).all():
        raise ValueError('Quality selection requires eight native metric rows')
    if np.any(e[:, 5] < seconds - 1 / 60) or np.any(e[:, 3] < np.maximum(.7, .90 * a[:, 3])):
        return False
    if e[:, 0].mean() > 1.05 * a[:, 0].mean() or e[:, 2].mean() > 1.20 * a[:, 2].mean() + .002:
        return False
    return previous is None or e[:, 1].mean() <= np.asarray(previous)[:, 1].mean() - .01
