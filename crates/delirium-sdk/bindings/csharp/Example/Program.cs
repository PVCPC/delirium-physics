// A ped stands, then is shot. usage: Example <fred.xrt> <build.txt> [taskparams.txt], delirium_physics.dll beside it or on PATH.
using System;
using Delirium;
using static Delirium.Native;

static void Check(int r) { if (r < 0) throw new Exception($"delirium_physics {r}: {LastError()}"); }

Check(nm_init(args[0]));
if (args.Length > 2) Check(nm_load_task_params(args[2]));
IntPtr build = nm_build_load(args[1]);
if (build == IntPtr.Zero) throw new Exception(LastError());
IntPtr world = nm_world_create();
nm_world_add_floor(world, 0, 0, 0, 50);
IntPtr agent = nm_agent_create(build, new float[] { 0, 0, 0 }, 0f, 0.3f);
Console.WriteLine($"{nm_message_count()} messages, bodyBalance has {nm_message_param_count(nm_message_find("bodyBalance"))} params");

Check(nm_msg_begin(agent, "bodyBalance"));
Check(nm_msg_set_bool(agent, "start", 1));
Check(nm_msg_set_float(agent, "bodyStiffness", 10f));
Check(nm_msg_send(agent, world));
var p = new float[3];
for (int i = 0; i < 60; i++) Check(nm_agent_step(agent, world, 1f / 60));
nm_agent_get_part_transform(agent, 0, p, null);
Console.WriteLine($"standing: pelvis {p[0]:F2} {p[1]:F2} {p[2]:F2}");

var chest = new float[3];
nm_agent_get_part_transform(agent, 10, chest, null);
nm_agent_apply_push(agent, chest, new float[] { 0, 40, 0 });
Check(nm_agent_task_shot(agent, world, "POWERFUL_WEAPON", 9, chest, new float[] { 0, 1, 0 }, 200f));
for (int i = 0; i < 120; i++) nm_agent_step(agent, world, 1f / 60);
nm_agent_get_part_transform(agent, 0, p, null);
Console.WriteLine($"2 s after the shot: pelvis {p[0]:F2} {p[1]:F2} {p[2]:F2}, balancing {nm_agent_is_balancing(agent)}, fallen {nm_agent_has_fallen(agent)}");
nm_agent_destroy(agent); nm_world_destroy(world); nm_build_destroy(build);
