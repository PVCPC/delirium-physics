"""A ped stands, is shot in the chest, staggers. usage:
   DELIRIUM_PHYSICS_LIB=<dir with delirium_physics.dll> python example.py <fred.xrt> <build.txt> [taskparams.txt]"""
import ctypes, sys
from delirium_physics import lib, vec, text, check, NM_MODE_LEAD
from ctypes import c_float

xrt, build_path = sys.argv[1], sys.argv[2]
check(lib.nm_init(xrt.encode()))
if len(sys.argv) > 3:
    check(lib.nm_load_task_params(sys.argv[3].encode()))
build = lib.nm_build_load(build_path.encode()) or check(-1)
world = lib.nm_world_create()
lib.nm_world_add_floor(world, 0, 0, 0, 50)
agent = lib.nm_agent_create(build, vec(0, 0, 0), 0.0, 0.3)

print(lib.nm_message_count(), "messages:", ", ".join(text(lib.nm_message_name, m) for m in range(lib.nm_message_count())))
check(lib.nm_msg_begin(agent, b"bodyBalance"))
check(lib.nm_msg_set_bool(agent, b"start", 1))
check(lib.nm_msg_set_float(agent, b"bodyStiffness", 10.0))
print("bodyBalance sent;", check(lib.nm_msg_send(agent, world)), "params accepted but unused")

pos = (c_float * 3)()
def pelvis():
    lib.nm_agent_get_part_transform(agent, 0, pos, None)
    return tuple(round(p, 2) for p in pos)

for _ in range(60):
    check(lib.nm_agent_step(agent, world, 1 / 60))
print("standing, pelvis", pelvis())
chest = (c_float * 3)()
lib.nm_agent_get_part_transform(agent, 10, chest, None)
lib.nm_agent_apply_push(agent, chest, vec(0, 40, 0))
check(lib.nm_agent_task_shot(agent, world, b"POWERFUL_WEAPON", 9, chest, vec(0, 1, 0), 200.0))
for t in range(120):
    lib.nm_agent_step(agent, world, 1 / 60)
    for i in range(lib.nm_agent_feedback_count(agent)):
        name, value = ctypes.create_string_buffer(64), c_float()
        lib.nm_agent_feedback_get(agent, i, name, 64, None, ctypes.byref(value))
        print(f"  {t / 60:.2f}s feedback {name.value.decode()} {value.value:g}")
    if t % 30 == 29:
        print(f"{(t + 1) / 60:.1f}s pelvis {pelvis()} balancing {lib.nm_agent_is_balancing(agent)} fallen {lib.nm_agent_has_fallen(agent)}")
lib.nm_agent_destroy(agent); lib.nm_world_destroy(world); lib.nm_build_destroy(build)
