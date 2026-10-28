"""FOLLOW mode with MuJoCo: MuJoCo owns the ragdoll (21 capsules, NM's joints and limits); NM reads its state each
60 Hz step and returns per-part forces and torques, which MuJoCo applies while it steps. The ped stands under
bodyBalance, then takes a chest hit.

usage: DELIRIUM_PHYSICS_LIB=<dir with delirium_physics.dll> python mujoco_follow.py <fred.xrt> <build.txt> [taskparams.txt] [--view]
"""
import ctypes, math, os, sys
import numpy as np
import mujoco

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'bindings', 'python'))
from delirium_physics import lib, vec, check, NM_MODE_FOLLOW  # noqa: E402

F3, F4 = ctypes.c_float * 3, ctypes.c_float * 4
SUB = 4  # MuJoCo steps per NM step (1/240 s each)

args = [a for a in sys.argv[1:] if not a.startswith('--')]
check(lib.nm_init(args[0].encode()))
if len(args) > 2:
    check(lib.nm_load_task_params(args[2].encode()))
build = lib.nm_build_load(args[1].encode()) or check(-1)
world = lib.nm_world_create()
lib.nm_world_add_floor(world, 0, 0, 0, 50)
agent = lib.nm_agent_create(build, vec(0, 0, 0), 0.0, 0.3)
n = lib.nm_agent_part_count(agent)


def frame(i):
    p, q = F3(), F4()
    lib.nm_agent_get_part_transform(agent, i, p, q)
    return np.array(p[:]), np.array([q[3], q[0], q[1], q[2]])  # MuJoCo's quaternions are (w, x, y, z)


def qmul(a, b):
    r = np.zeros(4)
    mujoco.mju_mulQuat(r, a, b)
    return r


def qinv(a):
    return np.array([a[0], -a[1], -a[2], -a[3]])


def rot(q, v):
    r = np.zeros(3)
    mujoco.mju_rotVecQuat(r, v, q)
    return r


# The ragdoll as MJCF: each part a body at its capsule's frame (capsule along local Y), nested under its parent,
# with NM's joint (pivot, frame, limits). Parts only touch the floor (NM's self-collision pairs are left out).
frames = [frame(i) for i in range(n)]
kids = {i: [] for i in range(-1, n)}
info = {}
for i in range(n):
    m, r, h, par = ctypes.c_float(), ctypes.c_float(), ctypes.c_float(), ctypes.c_int32()
    lib.nm_build_part_info(build, i, ctypes.byref(m), ctypes.byref(r), ctypes.byref(h), ctypes.byref(par))
    piv, jf, lim, jp = F3(), F4(), F3(), ctypes.c_int32()
    kind = lib.nm_agent_get_joint(agent, i, ctypes.byref(jp), piv, jf, lim, None)
    info[i] = (m.value, r.value, h.value, kind, np.array(piv[:]), np.array([jf[3], jf[0], jf[1], jf[2]]), lim[:])
    kids[par.value].append(i)


def body(i, parent):
    m, r, h, kind, piv, jf, lim = info[i]
    x, q = frames[i]
    if parent < 0:
        pos, quat = x, q
    else:
        px, pq = frames[parent]
        pos, quat = rot(qinv(pq), x - px), qmul(qinv(pq), q)
    s = f'<body name="p{i}" pos="{pos[0]:.5f} {pos[1]:.5f} {pos[2]:.5f}" quat="{quat[0]:.6f} {quat[1]:.6f} {quat[2]:.6f} {quat[3]:.6f}">'
    if kind == 0:
        s += '<freejoint/>'
    else:
        jpos = rot(qinv(q), piv - x)
        if kind == 2:
            ax = rot(qinv(q), rot(jf, [0, 0, 1]))
            s += (f'<joint type="hinge" pos="{jpos[0]:.5f} {jpos[1]:.5f} {jpos[2]:.5f}" axis="{ax[0]:.5f} {ax[1]:.5f} {ax[2]:.5f}" '
                  f'range="{math.degrees(lim[0]):.2f} {math.degrees(lim[1]):.2f}" limited="true" damping="0.5" armature="0.01"/>')
        else:
            cone = math.degrees(max(lim[0], lim[1], lim[2], 0.05))
            s += f'<joint type="ball" pos="{jpos[0]:.5f} {jpos[1]:.5f} {jpos[2]:.5f}" range="0 {cone:.2f}" limited="true" damping="0.5" armature="0.01"/>'
    shape = f'type="capsule" fromto="0 {-h:.5f} 0 0 {h:.5f} 0"' if h > 1e-3 else 'type="sphere"'
    s += f'<geom {shape} size="{r:.5f}" mass="{m:.4f}" contype="1" conaffinity="0" friction="1 0.01 0.001"/>'
    s += ''.join(body(c, i) for c in kids[i]) + '</body>'
    return s


xml = f'''<mujoco><option timestep="{1 / 60 / SUB}" integrator="implicitfast"/>
<worldbody><light pos="0 -2 4"/><geom type="plane" size="20 20 0.1" contype="0" conaffinity="1" friction="1 0.01 0.001"/>
{''.join(body(r, -1) for r in kids[-1])}</worldbody></mujoco>'''
model = mujoco.MjModel.from_xml_string(xml)
data = mujoco.MjData(model)
mujoco.mj_forward(model, data)
ids = [mujoco.mj_name2id(model, mujoco.mjtObj.mjOBJ_BODY, f'p{i}') for i in range(n)]

check(lib.nm_agent_set_mode(agent, NM_MODE_FOLLOW))
check(lib.nm_msg_begin(agent, b'bodyBalance'))
check(lib.nm_msg_set_bool(agent, b'start', 1))
check(lib.nm_msg_send(agent, world))

states = (ctypes.c_float * (n * 13))()
drives = (ctypes.c_float * (n * 6))()
v6 = np.zeros(6)


def nm_step():
    for k, b in enumerate(ids):
        mujoco.mj_objectVelocity(model, data, mujoco.mjtObj.mjOBJ_BODY, b, v6, 0)  # world: spin, velocity
        q = data.xquat[b]
        states[k * 13:k * 13 + 13] = [*data.xpos[b], q[1], q[2], q[3], q[0], *v6[3:], *v6[:3]]
    check(lib.nm_agent_set_part_states(agent, states, n))
    check(lib.nm_agent_step(agent, world, 1 / 60))
    lib.nm_agent_get_part_drives(agent, drives, n)
    d = np.frombuffer(drives, dtype=np.float32).reshape(n, 6)
    data.xfrc_applied[:] = 0
    for k, b in enumerate(ids):
        data.xfrc_applied[b] = d[k]
        # the original engine's armature on the joint, as NM had it this step.
        j = model.body_jntadr[b]
        if j >= 0 and model.jnt_type[j] != mujoco.mjtJoint.mjJNT_FREE:
            arm = ctypes.c_float()
            lib.nm_agent_get_joint(agent, k, None, None, None, None, ctypes.byref(arm))
            a0, nd = model.jnt_dofadr[j], 3 if model.jnt_type[j] == mujoco.mjtJoint.mjJNT_BALL else 1
            model.dof_armature[a0:a0 + nd] = min(arm.value, 1e3)
    for _ in range(SUB):
        mujoco.mj_step(model, data)


def pelvis():
    return data.xpos[ids[0]].copy()


viewer = None
if '--view' in sys.argv:
    import mujoco.viewer
    viewer = mujoco.viewer.launch_passive(model, data)

z0 = pelvis()[2]
for t in range(60):
    nm_step()
    viewer and viewer.sync()
print(f'1 s standing under bodyBalance: pelvis {pelvis().round(2)} (started at z {z0:.2f}), fallen {lib.nm_agent_has_fallen(agent)}')

# A chest hit along +Y: the impulse into MuJoCo (one substep's force), the shot task to NM.
chest = ids[10]
at = data.xpos[chest].copy()
check(lib.nm_agent_task_shot(agent, world, b'POWERFUL_WEAPON', 10, vec(*at), vec(0, 1, 0), 200.0))
data.xfrc_applied[:] = 0
data.xfrc_applied[chest, :3] = [0, 40 / model.opt.timestep, 0]
mujoco.mj_step(model, data)
data.xfrc_applied[:] = 0
start = pelvis()
for t in range(150):
    nm_step()
    viewer and viewer.sync()
    if t % 30 == 29:
        p = pelvis()
        print(f'{(t + 1) / 60:.1f} s after the hit: pelvis {p.round(2)}  moved {p[1] - start[1]:+.2f} m  balancing {lib.nm_agent_is_balancing(agent)} fallen {lib.nm_agent_has_fallen(agent)}')
