// NmCharacter: a humanoid driven by the NM port (delirium_physics.dll).
//   LEAD:   NM's own physics moves the 21 parts; the Animator's bones follow them.
//   FOLLOW: NM builds a 21-rigidbody ragdoll here (capsules, ConfigurableJoints from its joints) and drives it with
//           per-part forces and torques each physics step; Unity's physics moves it and the bones follow it.
// Run physics at 60 Hz (Time.fixedDeltaTime = 1/60, NM's step). NM is Z up, right-handed; Unity Y up, left-handed:
// points swap y and z, rotations (x, y, z, w) -> (-x, -z, -y, w), spins and torques swap and negate.
using System;
using UnityEngine;
using static Delirium.Native;

namespace Delirium
{
    [DefaultExecutionOrder(-50)]
    public class NmCharacter : MonoBehaviour
    {
        public enum Mode { Lead, Follow }

        [Tooltip("the game's naturalmotion/fred.xrt (absolute, or under StreamingAssets).")]
        public string xrtPath = "fred.xrt";
        [Tooltip("A character build dump (the viewer's NM_DUMP).")]
        public string buildPath = "ped_build.txt";
        [Tooltip("the game's taskparams.txt (optional).")]
        public string taskParamsPath = "";
        public Mode mode = Mode.Lead;
        public bool balanceOnStart = true;
        [Tooltip("Per NM part (Buttocks .. Hand_Right), the bone it poses. Empty: the Animator's humanoid bones.")]
        public Transform[] bones = new Transform[21];
        [Tooltip("Fit NM's body to this character's humanoid skeleton (its bone lengths and pose at Start: stand it in its bind pose). Off: Homer's body (1.8 m), the bones posed from it.")]
        public bool fitToSkeleton = true;

        IntPtr world, build, agent;
        int n;
        Rigidbody[] bodies;
        Vector3[] baseInertia;
        Quaternion[] boneTurn;
        Vector3[] boneAt;
        readonly float[] f3 = new float[3], g3 = new float[3], q4 = new float[4], arm = new float[1];
        float[] states, drives;

        // NM's parts in order, as Unity's humanoid bones (NM's 4 spine parts onto Unity's 3).
        static readonly HumanBodyBones[] Humanoid = {
            HumanBodyBones.Hips, HumanBodyBones.LeftUpperLeg, HumanBodyBones.LeftLowerLeg, HumanBodyBones.LeftFoot,
            HumanBodyBones.RightUpperLeg, HumanBodyBones.RightLowerLeg, HumanBodyBones.RightFoot,
            HumanBodyBones.Spine, HumanBodyBones.Chest, HumanBodyBones.UpperChest, HumanBodyBones.LastBone,
            HumanBodyBones.Neck, HumanBodyBones.Head,
            HumanBodyBones.LeftShoulder, HumanBodyBones.LeftUpperArm, HumanBodyBones.LeftLowerArm, HumanBodyBones.LeftHand,
            HumanBodyBones.RightShoulder, HumanBodyBones.RightUpperArm, HumanBodyBones.RightLowerArm, HumanBodyBones.RightHand,
        };

        public static Vector3 ToUnity(float x, float y, float z) => new Vector3(x, z, y);
        public static Vector3 SpinToUnity(float x, float y, float z) => new Vector3(-x, -z, -y);
        public static Quaternion ToUnity(float x, float y, float z, float w) => new Quaternion(-x, -z, -y, w);
        static void ToNm(Vector3 v, float[] o, int at = 0) { o[at] = v.x; o[at + 1] = v.z; o[at + 2] = v.y; }
        static void SpinToNm(Vector3 v, float[] o, int at = 0) { o[at] = -v.x; o[at + 1] = -v.z; o[at + 2] = -v.y; }
        static void ToNm(Quaternion q, float[] o, int at = 0) { o[at] = -q.x; o[at + 1] = -q.z; o[at + 2] = -q.y; o[at + 3] = q.w; }

        static string Find(string p) => System.IO.Path.IsPathRooted(p) ? p : System.IO.Path.Combine(Application.streamingAssetsPath, p);

        static void Check(int r)
        {
            if (r < 0) throw new Exception($"delirium_physics {r}: {LastError()}");
        }

        public IntPtr Agent => agent;
        public IntPtr World => world;

        void Start()
        {
            Check(nm_init(Find(xrtPath)));
            if (taskParamsPath != "") Check(nm_load_task_params(Find(taskParamsPath)));
            build = nm_build_load(Find(buildPath));
            if (build == IntPtr.Zero) throw new Exception(LastError());
            n = nm_build_part_count(build);
            var anim = GetComponent<Animator>();
            for (int k = 0; k < n; k++)
                if (bones[k] == null && anim != null && anim.isHuman && Humanoid[k] != HumanBodyBones.LastBone)
                    bones[k] = anim.GetBoneTransform(Humanoid[k]);
            float yaw = -transform.eulerAngles.y * Mathf.Deg2Rad;
            if (fitToSkeleton && anim != null && anim.isHuman)
            {
                var fitted = nm_build_fit(build, Joints(anim));
                if (fitted == IntPtr.Zero) throw new Exception(LastError());
                nm_build_destroy(build);
                // The fitted build stands as the skeleton does, facing as it does.
                (build, yaw) = (fitted, 0f);
            }
            world = nm_world_create();
            // The floor under it; add the level's geometry with nm_world_add_mesh for NM's own probes.
            ToNm(transform.position, f3);
            Check(nm_world_add_floor(world, f3[0], f3[1], f3[2], 200f));
            agent = nm_agent_create(build, f3, yaw, UnityEngine.Random.value);
            states = new float[n * 13];
            drives = new float[n * 6];
            if (mode == Mode.Follow) BuildRagdoll();
            // Each bone keeps its offset from its part as they stand now.
            boneTurn = new Quaternion[n];
            boneAt = new Vector3[n];
            for (int k = 0; k < n; k++)
            {
                if (bones[k] == null) continue;
                Part(k, out var p, out var q);
                boneTurn[k] = Quaternion.Inverse(q) * bones[k].rotation;
                boneAt[k] = Quaternion.Inverse(q) * (bones[k].position - p);
            }
            if (balanceOnStart) Send("bodyBalance", ("start", true));
        }

        /// NM's 21 joints on a humanoid skeleton (nm_build_fit): the hips' centre, then each part's joint with its
        /// parent. NM's 4 spine joints spread evenly from Spine to Neck; a missing Neck or Shoulder is put between
        /// its neighbours.
        static float[] Joints(Animator anim)
        {
            Vector3 B(HumanBodyBones b) => anim.GetBoneTransform(b) is Transform t ? t.position : throw new Exception($"no {b} bone");
            Vector3 Or(HumanBodyBones b, Vector3 fallback) => anim.GetBoneTransform(b) is Transform t ? t.position : fallback;
            Vector3 spine = B(HumanBodyBones.Spine), head = B(HumanBodyBones.Head);
            var neck = Or(HumanBodyBones.Neck, Vector3.Lerp(spine, head, 0.85f));
            Vector3 lArm = B(HumanBodyBones.LeftUpperArm), rArm = B(HumanBodyBones.RightUpperArm);
            var j = new[] {
                B(HumanBodyBones.Hips), B(HumanBodyBones.LeftUpperLeg), B(HumanBodyBones.LeftLowerLeg), B(HumanBodyBones.LeftFoot),
                B(HumanBodyBones.RightUpperLeg), B(HumanBodyBones.RightLowerLeg), B(HumanBodyBones.RightFoot),
                spine, Vector3.Lerp(spine, neck, 0.25f), Vector3.Lerp(spine, neck, 0.5f), Vector3.Lerp(spine, neck, 0.75f), neck, head,
                Or(HumanBodyBones.LeftShoulder, Vector3.Lerp(neck, lArm, 0.3f)), lArm, B(HumanBodyBones.LeftLowerArm), B(HumanBodyBones.LeftHand),
                Or(HumanBodyBones.RightShoulder, Vector3.Lerp(neck, rArm, 0.3f)), rArm, B(HumanBodyBones.RightLowerArm), B(HumanBodyBones.RightHand),
            };
            var o = new float[j.Length * 3];
            for (int k = 0; k < j.Length; k++) ToNm(j[k], o, k * 3);
            return o;
        }

        /// Part k's frame in Unity's world (the ragdoll's body in FOLLOW).
        public void Part(int k, out Vector3 pos, out Quaternion rot)
        {
            if (bodies != null) { pos = bodies[k].position; rot = bodies[k].rotation; return; }
            Check(nm_agent_get_part_transform(agent, k, f3, q4));
            pos = ToUnity(f3[0], f3[1], f3[2]);
            rot = ToUnity(q4[0], q4[1], q4[2], q4[3]);
        }

        void BuildRagdoll()
        {
            var root = new GameObject("NM ragdoll").transform;
            bodies = new Rigidbody[n];
            baseInertia = new Vector3[n];
            for (int k = 0; k < n; k++)
            {
                Check(nm_agent_get_part_transform(agent, k, f3, q4));
                var go = new GameObject(PartName(k));
                go.transform.SetPositionAndRotation(ToUnity(f3[0], f3[1], f3[2]), ToUnity(q4[0], q4[1], q4[2], q4[3]));
                go.transform.SetParent(root, true);
                Check(nm_build_part_info(build, k, f3, g3, q4, out int parent));
                float mass = f3[0], radius = g3[0], half = q4[0];
                var c = go.AddComponent<CapsuleCollider>();
                // NM's capsules run along their part's Y, which is the converted frame's Z.
                (c.direction, c.radius, c.height) = (2, radius, 2 * (half + radius));
                var rb = go.AddComponent<Rigidbody>();
                rb.mass = mass;
                rb.interpolation = RigidbodyInterpolation.Interpolate;
                bodies[k] = rb;
                baseInertia[k] = rb.inertiaTensor;
            }
            for (int k = 0; k < n; k++)
            {
                int parent;
                float[] pivot = new float[3], frame = new float[4], lim = new float[3];
                int kind = nm_agent_get_joint(agent, k, out parent, pivot, frame, lim, null);
                if (kind == 0) continue;
                var j = bodies[k].gameObject.AddComponent<ConfigurableJoint>();
                j.connectedBody = bodies[parent];
                var t = bodies[k].transform;
                var jf = ToUnity(frame[0], frame[1], frame[2], frame[3]);
                j.anchor = t.InverseTransformPoint(ToUnity(pivot[0], pivot[1], pivot[2]));
                // The joint frame's Z (NM's twist / hinge axis) is Unity's local Y of the converted frame.
                j.axis = t.InverseTransformDirection(jf * Vector3.up);
                j.secondaryAxis = t.InverseTransformDirection(jf * Vector3.right);
                (j.xMotion, j.yMotion, j.zMotion) = (ConfigurableJointMotion.Locked, ConfigurableJointMotion.Locked, ConfigurableJointMotion.Locked);
                j.angularXMotion = ConfigurableJointMotion.Limited;
                if (kind == 2)
                {
                    j.lowAngularXLimit = new SoftJointLimit { limit = -lim[1] * Mathf.Rad2Deg };
                    j.highAngularXLimit = new SoftJointLimit { limit = -lim[0] * Mathf.Rad2Deg };
                    (j.angularYMotion, j.angularZMotion) = (ConfigurableJointMotion.Locked, ConfigurableJointMotion.Locked);
                }
                else
                {
                    j.lowAngularXLimit = new SoftJointLimit { limit = -lim[2] * Mathf.Rad2Deg };
                    j.highAngularXLimit = new SoftJointLimit { limit = lim[2] * Mathf.Rad2Deg };
                    (j.angularYMotion, j.angularZMotion) = (ConfigurableJointMotion.Limited, ConfigurableJointMotion.Limited);
                    j.angularYLimit = new SoftJointLimit { limit = lim[0] * Mathf.Rad2Deg };
                    j.angularZLimit = new SoftJointLimit { limit = lim[1] * Mathf.Rad2Deg };
                }
            }
            // Parts touch the world, not each other (NM's own self-collision pairs aren't carried over).
            for (int a = 0; a < n; a++)
                for (int b = a + 1; b < n; b++)
                    Physics.IgnoreCollision(bodies[a].GetComponent<Collider>(), bodies[b].GetComponent<Collider>());
            Check(nm_agent_set_mode(agent, NM_MODE_FOLLOW));
        }

        static string PartName(int k) => Str((b, l) => nm_part_name(k, b, l));

        void FixedUpdate()
        {
            if (agent == IntPtr.Zero) return;
            float dt = Time.fixedDeltaTime;
            if (bodies == null)
            {
                Check(nm_agent_step(agent, world, dt));
                return;
            }
            for (int k = 0; k < n; k++)
            {
                var rb = bodies[k];
                ToNm(rb.position, states, k * 13);
                ToNm(rb.rotation, states, k * 13 + 3);
#if UNITY_2023_3_OR_NEWER
                ToNm(rb.linearVelocity, states, k * 13 + 7);
#else
                ToNm(rb.velocity, states, k * 13 + 7);
#endif
                SpinToNm(rb.angularVelocity, states, k * 13 + 10);
            }
            Check(nm_agent_set_part_states(agent, states, n));
            Check(nm_agent_step(agent, world, dt));
            nm_agent_get_part_drives(agent, drives, n);
            for (int k = 0; k < n; k++)
            {
                int i = k * 6;
                bodies[k].AddForce(ToUnity(drives[i], drives[i + 1], drives[i + 2]), ForceMode.Force);
                bodies[k].AddTorque(SpinToUnity(drives[i + 3], drives[i + 4], drives[i + 5]), ForceMode.Force);
                // the original engine's joint armature (its effector stiffness), as extra inertia on the part: Unity's joints have
                // none. ponytail: added on the body's principal axes, not the joint's; ArticulationBody would be exact.
                nm_agent_get_joint(agent, k, out int _, null, null, null, arm);
                if (arm[0] > 0 && arm[0] < 1e3f) bodies[k].inertiaTensor = baseInertia[k] + Vector3.one * arm[0];
            }
        }

        void LateUpdate()
        {
            if (agent == IntPtr.Zero) return;
            for (int k = 0; k < n; k++)
            {
                if (bones[k] == null) continue;
                Part(k, out var p, out var q);
                bones[k].SetPositionAndRotation(p + q * boneAt[k], q * boneTurn[k]);
            }
        }

        /// Sends one of NM's 28 messages: Send("bodyBalance", ("start", true), ("bodyStiffness", 10f)); a behaviour starts
        /// with start true, stops with false and only takes the params without it.
        /// Returns how many params the port kept without acting on them.
        public int Send(string message, params (string name, object value)[] ps)
        {
            Check(nm_msg_begin(agent, message));
            foreach (var (k, v) in ps)
            {
                switch (v)
                {
                    case float f: Check(nm_msg_set_float(agent, k, f)); break;
                    case double d: Check(nm_msg_set_float(agent, k, (float)d)); break;
                    case int i: Check(nm_msg_set_int(agent, k, i)); break;
                    case bool b: Check(nm_msg_set_bool(agent, k, b ? 1 : 0)); break;
                    case Vector3 p: ToNm(p, f3); Check(nm_msg_set_vec3(agent, k, f3)); break;
                    case string s: Check(nm_msg_set_string(agent, k, s)); break;
                    default: nm_msg_cancel(agent); throw new ArgumentException($"{k}: {v?.GetType()} isn't a param type");
                }
            }
            int r = nm_msg_send(agent, world);
            Check(r);
            return r;
        }

        /// A bullet: its push on the part nearest `at` (into NM in LEAD, into the rigidbody in FOLLOW) and the game's
        /// shot task (`set` a taskparams set, e.g. POWERFUL_WEAPON).
        public void Shoot(Vector3 at, Vector3 dir, float impulse = 40f, string set = "POWERFUL_WEAPON")
        {
            ToNm(at, f3);
            int part = nm_agent_nearest_part(agent, f3);
            if (bodies != null) bodies[part].AddForceAtPosition(dir.normalized * impulse, at, ForceMode.Impulse);
            else { ToNm(dir.normalized * impulse, g3); Check(nm_agent_apply_push(agent, f3, g3)); }
            ToNm(dir.normalized, g3);
            Check(nm_agent_task_shot(agent, world, set, part, f3, g3, 200f));
        }

        public bool Balancing => nm_agent_is_balancing(agent) == 1;
        public bool Fallen => nm_agent_has_fallen(agent) == 1;

        void OnDestroy()
        {
            if (agent != IntPtr.Zero) nm_agent_destroy(agent);
            if (world != IntPtr.Zero) nm_world_destroy(world);
            if (build != IntPtr.Zero) nm_build_destroy(build);
            agent = world = build = IntPtr.Zero;
        }
    }
}
