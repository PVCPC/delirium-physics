"""FOLLOW mode with Bullet (PyBullet's btMultiBody): Bullet owns the ragdoll (21 capsules on NM's joints); NM reads its
state each 60 Hz step and returns per-part forces and torques, which Bullet applies while it steps. The ped stands
under bodyBalance, then takes a chest hit.

It shows the limit of engines without joint armature: the original engine's muscle stiffness is armature (s / (1 - s) times the
joint's articulated inertia), which resists every load on the joint. Bullet's btMultiBody has none; with it put on the
parts' inertia and NM's torques applied as joint motor torques (16 substeps), the ped holds together but sinks to the
floor in ~1 s (with them as per-part external torques, or 4 substeps, it blows up). See docs/nm/sdk-recipes.md.

usage: DELIRIUM_PHYSICS_LIB=<dir with delirium_physics.dll> python bullet_follow.py <fred.xrt> <build.txt> [taskparams.txt] [--view]
"""
import ctypes, os, sys
import pybullet as pb

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'bindings', 'python'))
from delirium_physics import lib, vec, check, NM_MODE_FOLLOW  # noqa: E402

F3, F4 = ctypes.c_float * 3, ctypes.c_float * 4
SUB = 16  # Bullet steps per NM step (1/960 s each)

args = [a for a in sys.argv[1:] if not a.startswith('--')]
check(lib.nm_init(args[0].encode()))
if len(args) > 2:
    check(lib.nm_load_task_params(args[2].encode()))
build = lib.nm_build_load(args[1].encode()) or check(-1)
world = lib.nm_world_create()
lib.nm_world_add_floor(world, 0, 0, 0, 50)
agent = lib.nm_agent_create(build, vec(0, 0, 0), 0.0, 0.3)
n = lib.nm_agent_part_count(agent)

pb.connect(pb.GUI if '--view' in sys.argv else pb.DIRECT)
pb.setGravity(0, 0, -9.81)
pb.setTimeStep(1 / 60 / SUB)
floor = pb.createMultiBody(0, pb.createCollisionShape(pb.GEOM_PLANE))
pb.changeDynamics(floor, -1, lateralFriction=1.0)


def into(px, pq, x, q):
    """(x, q) in the frame (px, pq)."""
    ip, iq = pb.invertTransform(px, pq)
    return pb.multiplyTransforms(ip, iq, x, q)


# NM's parts: frame (capsule centre, capsule along local Y), mass, size, parent, joint (quaternions (x, y, z, w) in
# both).
part = []
for i in range(n):
    p, q = F3(), F4()
    lib.nm_agent_get_part_transform(agent, i, p, q)
    m, r, h, par = ctypes.c_float(), ctypes.c_float(), ctypes.c_float(), ctypes.c_int32()
    lib.nm_build_part_info(build, i, ctypes.byref(m), ctypes.byref(r), ctypes.byref(h), ctypes.byref(par))
    piv, jf, lim = F3(), F4(), F3()
    kind = lib.nm_agent_get_joint(agent, i, None, piv, jf, lim, None)
    part.append(dict(x=tuple(p), q=tuple(q), m=m.value, r=r.value, h=h.value, par=par.value, kind=kind,
                     piv=tuple(piv), jf=tuple(jf), lim=tuple(lim)))


def inertia(m, r, h):
    # ponytail: a solid capsule's principal moments (axis Y), the hemispheres' offset approximated.
    vc, vs = 3.14159 * r * r * 2 * h, 4 / 3 * 3.14159 * r ** 3
    mc, ms = m * vc / (vc + vs), m * vs / (vc + vs)
    ax = mc * r * r / 2 + ms * 2 * r * r / 5
    tr = mc * ((2 * h) ** 2 / 12 + r * r / 4) + ms * (2 * r * r / 5 + h * h + 3 * h * r / 4)
    return (tr, ax, tr)


def shape(i):
    r, h = part[i]['r'], part[i]['h']
    if h < 1e-3:
        return pb.createCollisionShape(pb.GEOM_SPHERE, radius=r)
    # Bullet's capsules run along Z: turned onto Y.
    return pb.createCollisionShape(pb.GEOM_CAPSULE, radius=r, height=2 * h,
                                   collisionFrameOrientation=pb.getQuaternionFromEuler((1.5707963, 0, 0)))


# Links depth first (Bullet's own order); each link's frame sits at its joint's pivot with the part's orientation, its
# centre of mass at the capsule's centre (the root's frame is its centre).
root = next(i for i in range(n) if part[i]['par'] < 0)
order, todo = [], [c for c in reversed(range(n)) if part[c]['par'] == root]
while todo:
    c = todo.pop()
    order.append(c)
    todo += [d for d in reversed(range(n)) if part[d]['par'] == c]
link = {root: -1, **{c: k for k, c in enumerate(order)}}


L = {k: [] for k in ('linkMasses', 'linkCollisionShapeIndices', 'linkVisualShapeIndices', 'linkPositions',
                     'linkOrientations', 'linkInertialFramePositions', 'linkInertialFrameOrientations',
                     'linkParentIndices', 'linkJointTypes', 'linkJointAxis')}
for c in order:
    P = part[c]
    lp, lq = into(part[P['par']]['x' if P['par'] == root else 'piv'], part[P['par']]['q'], P['piv'], P['q'])
    hinge = P['kind'] == 2
    for k, v in (('linkMasses', P['m']), ('linkCollisionShapeIndices', shape(c)), ('linkVisualShapeIndices', -1),
                 ('linkPositions', lp), ('linkOrientations', lq),
                 ('linkInertialFramePositions', into(P['piv'], P['q'], P['x'], P['q'])[0]),
                 ('linkInertialFrameOrientations', (0, 0, 0, 1)), ('linkParentIndices', link[P['par']] + 1),
                 ('linkJointTypes', pb.JOINT_REVOLUTE if hinge else pb.JOINT_SPHERICAL),
                 ('linkJointAxis', into((0, 0, 0), P['q'], pb.multiplyTransforms((0, 0, 0), P['jf'], (0, 0, 1), (0, 0, 0, 1))[0], (0, 0, 0, 1))[0] if hinge else (0, 0, 1))):
        L[k].append(v)

R = part[root]
body = pb.createMultiBody(R['m'], shape(root), -1, R['x'], R['q'], **L)
base_i = {c: inertia(part[c]['m'], part[c]['r'], part[c]['h']) for c in range(n)}
for c in range(n):
    pb.changeDynamics(body, link[c], lateralFriction=1.0, linearDamping=0, angularDamping=0, jointDamping=0.5,
                      localInertiaDiagonal=base_i[c])
for c in order:
    if part[c]['kind'] == 2:
        pb.setJointMotorControl2(body, link[c], pb.VELOCITY_CONTROL, force=0)
        pb.changeDynamics(body, link[c], jointLowerLimit=part[c]['lim'][0], jointUpperLimit=part[c]['lim'][1])
    else:  # ponytail: Bullet's spherical joints take no limits; NM's drives keep them in range
        pb.setJointMotorControlMultiDof(body, link[c], pb.POSITION_CONTROL, [0, 0, 0, 1], targetVelocity=[0, 0, 0], force=[0, 0, 0])

check(lib.nm_agent_set_mode(agent, NM_MODE_FOLLOW))
check(lib.nm_msg_begin(agent, b'bodyBalance'))
check(lib.nm_msg_set_bool(agent, b'start', 1))
check(lib.nm_msg_send(agent, world))

states = (ctypes.c_float * (n * 13))()
drives = (ctypes.c_float * (n * 6))()


def state(i):
    """Part i's centre, rotation, velocity and spin (world)."""
    if i == root:
        return (*pb.getBasePositionAndOrientation(body), *pb.getBaseVelocity(body))
    s = pb.getLinkState(body, link[i], computeLinkVelocity=1)
    return s[0], s[1], s[6], s[7]


def nm_step(push=None):
    for i in range(n):
        x, q, v, w = state(i)
        states[i * 13:i * 13 + 13] = [*x, *q, *v, *w]
    check(lib.nm_agent_set_part_states(agent, states, n))
    check(lib.nm_agent_step(agent, world, 1 / 60))
    lib.nm_agent_get_part_drives(agent, drives, n)
    # NM's armature on each part's inertia (approximate: Bullet's joints take none).
    for c in order:
        arm = ctypes.c_float()
        lib.nm_agent_get_joint(agent, c, None, None, None, None, ctypes.byref(arm))
        pb.changeDynamics(body, link[c], localInertiaDiagonal=[b + min(arm.value, 1e3) for b in base_i[c]])
    # Each joint's torque: its part's drive plus its children's joints' (each joint's reaction on the parent stays
    # inside the joint solve); what's left on the root goes on as an external torque.
    J = {}
    for c in reversed(order):
        J[c] = [drives[c * 6 + 3 + k] + sum(J[g][k] for g in order if part[g]['par'] == c) for k in range(3)]
    rest = [drives[root * 6 + 3 + k] + sum(J[g][k] for g in order if part[g]['par'] == root) for k in range(3)]
    for c in order:
        q = state(c)[1]
        if part[c]['kind'] == 2:
            ax = pb.multiplyTransforms((0, 0, 0), q, pb.getJointInfo(body, link[c])[13], (0, 0, 0, 1))[0]
            pb.setJointMotorControl2(body, link[c], pb.TORQUE_CONTROL, force=sum(a * b for a, b in zip(J[c], ax)))
        else:  # spherical motor torques are in the child's frame
            pb.setJointMotorControlMultiDof(body, link[c], pb.TORQUE_CONTROL,
                                            force=list(into((0, 0, 0), q, J[c], (0, 0, 0, 1))[0]))
    for s in range(SUB):  # Bullet clears external forces every step
        for i in range(n):
            pb.applyExternalForce(body, link[i], drives[i * 6:i * 6 + 3], state(i)[0], pb.WORLD_FRAME)
        pb.applyExternalTorque(body, -1, rest, pb.WORLD_FRAME)
        if push and s == 0:
            pb.applyExternalForce(body, link[push[0]], push[1], state(push[0])[0], pb.WORLD_FRAME)
        pb.stepSimulation()


def pelvis():
    return pb.getBasePositionAndOrientation(body)[0]


z0 = pelvis()[2]
for t in range(60):
    nm_step()
print(f'1 s standing under bodyBalance: pelvis {[round(c, 2) for c in pelvis()]} (started at z {z0:.2f}), fallen {lib.nm_agent_has_fallen(agent)}')

# A chest hit along +Y: the impulse into Bullet (one substep's force), the shot task to NM.
chest = 10
check(lib.nm_agent_task_shot(agent, world, b'POWERFUL_WEAPON', 10, vec(*state(chest)[0]), vec(0, 1, 0), 200.0))
start = pelvis()
nm_step((chest, (0, 40 * 60 * SUB, 0)))
for t in range(1, 150):
    nm_step()
    if t % 30 == 29:
        p = pelvis()
        print(f'{(t + 1) / 60:.1f} s after the hit: pelvis {[round(c, 2) for c in p]}  moved {p[1] - start[1]:+.2f} m  balancing {lib.nm_agent_is_balancing(agent)} fallen {lib.nm_agent_has_fallen(agent)}')
