// Delirium ragdolls for Unity: built two ways from this one file. In the Unity package (com.delirium.ragdoll) it's the
// DeliriumRagdolls component: add it to one object, set its data files (in StreamingAssets) and call its methods from
// your game. Built with BEPINEX defined (../../bepinex) it's DeliriumRagdoll, a BepInEx 5 plugin that turns any skinned
// character in a Mono Unity game into an NM ragdoll.
// Aim at a character and press F9: it's pushed from the camera and its body takes over (balancing, staggering,
// falling); F9 again pushes again, F10 hands every character back to the game. Settings (key, push, data files)
// are in BepInEx/config/delirium_physics.ragdoll.cfg after the first run.
//
// The character's skeleton is found by its bone names (nm_rig_part_for_name), NM's body is fitted to its bone
// lengths (nm_build_fit) and its bones are turned to the body each frame (nm_rig_pose): rotations only, so no bone
// stretches, whatever the skeleton's bone count or bone roll. Names it doesn't know go in BoneMap.
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
#if BEPINEX
using BepInEx;
using BepInEx.Configuration;
#endif
using UnityEngine;
using static Delirium.Native;

namespace Delirium.Ragdoll
{
#if !BEPINEX
    using Plugin = DeliriumRagdolls;
#endif
#if BEPINEX
    [BepInPlugin("delirium_physics.ragdoll", "NM Ragdoll", "0.7.0")]
    public class Plugin : BaseUnityPlugin
    {
        string gameExe, xrt, buildFile, taskParams, boneMap, getUpFront, getUpBack, sizeMode;
        KeyCode ragdollKey, releaseKey;
        float push, getUpAfter, getUpBlend;
        bool balance, bodySymmetric, armsOutEarly;
        string body, customBody, overrides, hold, follow;
        bool recentreJoints, enginePhysics, stepLimit, fireReaction;
        string weaponTable, getUpNet, walkNet;
        int bodySeed;
        float bodyVariance;
#else
    /// Add to one object in your first scene (it lives on across scenes). Ragdoll, Shoot, MeleeHit, ... take the
    /// character's root; getting up blends back into its Animator. The data files come from the player's own game.
    public class DeliriumRagdolls : MonoBehaviour
    {
        [Header("Data: the player's own game (never ship these)")]
        [Tooltip("The player's game: its exe, the disc's default.xex, or the folder either is in (absolute).")]
        public string gameExe = "";
        [Tooltip("Absolute, or in StreamingAssets.")]
        public string xrt = "character.xrt", buildFile = "build.txt", taskParams = "";
        [Header("Skeleton")]
        [Tooltip("Bones the name guess misses: BoneName=Part pairs, e.g. Pelvis_jnt=Buttocks, Torso=Spine2, Tail=None.")]
        public string boneMap = "";
        [Header("Test keys (None: off)")]
        public KeyCode ragdollKey = KeyCode.None, releaseKey = KeyCode.None;
        [Tooltip("The test key's push from the camera, N s.")]
        public float push = 60f;
        [Header("Get up")]
        [Tooltip("Seconds lying still before it gets up by itself (0: never; call GetUp).")]
        public float getUpAfter = 0f;
        [Tooltip("Seconds to blend from the ragdoll's pose into the animation.")]
        public float getUpBlend = 0.4f;
        [Tooltip("Animator triggers set on getting up (skipped if the Animator has none).")]
        public string getUpFront = "GetUpFront", getUpBack = "GetUpBack";
        [Header("Feel")]
        [Tooltip("Ragdolls try to keep their feet (off: they go limp).")]
        public bool balance = true;
        [Tooltip("Bracing for a car (or anything coming at it), both arms thrown out straight well before it hits, as the original demo (the Playroom's 'Arms out early when bracing'). Off: the game's, a late bend.")]
        public bool armsOutEarly = false;
        [Tooltip("A shot whose nearest body part is the head kills.")]
        public bool headshotKills = true;
        [Tooltip("Auto, Real (the character's own size) or Homer (a person-sized body, the world scaled to it).")]
        public string sizeMode = "Auto";
        [Header("Force")]
        public float shotForce = 1f, meleeForce = 1f, explosionForce = 1f;
        [Tooltip("ShootWeapon's and Shotgun's table: name=set:push:damage[:pellets:between:clip:accuracy:range], comma separated, or a weaponinfo.xml (in StreamingAssets). Empty: pistol, smg, rifle, shotgun.")]
        public string weaponTable = "";
        [Tooltip("The game's balance runs out of steps (a push he can't take up in so many drops him). Off: he keeps stepping.")]
        public bool stepLimit = true;
        [Tooltip("A burning character reacts (flails, staggers, writhes). Off: he burns but carries on.")]
        public bool fireReaction = true;
        [Tooltip("The learned nets (in StreamingAssets): LearnedGetUp's and LearnedWalk's.")]
        public string getUpNet = "getup.net", walkNet = "walk.net";
        [Header("Body")]
        [Tooltip("Fit each kind of character to its own shape (weight and balance).")]
        public bool adaptive = true;
        [Tooltip("Every body's muscles times this.")]
        public float strength = 1f;
        [Tooltip("The body (the Playroom's): Fitted (the skeleton's own); Small, Average, Large, Lanky, Fat, Stocky, Waddler, Guard, Brute, Brawler or Bruiser (that shape, of each character); Random; Custom (customBody). The mesh keeps its look: pick one near it. SetBody sets one character's.")]
        public string body = "Fitted";
        [Tooltip("Random's seed: the same seed, the same body (0: each character its own).")]
        public int bodySeed = 0;
        [Tooltip("Random's spread, 1 to 4 (1.5 the Playroom's default).")]
        public float bodyVariance = 1.5f;
        [Tooltip("Random: the right side the left's.")]
        public bool bodySymmetric = true;
        [Tooltip("Custom's sizes against the fitted body: legs, arms, trunk, width, head, neck, weight (kg), height (m; 0 keeps it).")]
        public string customBody = "1, 1, 1, 1, 1, 1, 77, 0";
        [Tooltip("Behaviour parameters changed for every character (the Playroom's Parameters tab): shot.bodyStiffness=7, bodyBalance.useHeadLook=0. SetOverride does one character's.")]
        public string overrides = "";
        [Tooltip("Re-centre each joint's range on the pose it is handed over in (for rigs whose animation bends joints past NM's ranges: they would snap on the first step). Not the game's.")]
        public bool recentreJoints = false;
        [Tooltip("His parts as Unity bodies: NM's own physics still moves him (the game's exactly), his parts follow it as kinematic rigidbodies pushing your rigidbodies and meeting triggers, and loose rigidbodies within 5 m hit him (as Object movers). Off: NM's own physics alone, with the level read in. Run physics at 60 Hz (Fixed Timestep 0.01667) for the game's feel.")]
        public bool enginePhysics = false;
        [Tooltip("Joints held at least this stiff, damped and gravity-opposing (0-1), whatever the reactions set: mask=stiffness:damping:gravity, comma separated (masks as setStiffness's: fb whole body, ua upper arms, ub upper body, lb lower body). fb=6:1:0.5 keeps a limp body from folding; empty: off.")]
        public string hold = "";
        [Tooltip("Joints that keep playing the animation while the rest ragdolls (a boxer's guard, a held gun): mask=weight, comma separated (1 all the way, 0.5 halfway between NM's and the animation's; masks as Hold's, ua upper arms). The Animator keeps running under the ragdoll. Give those joints strength with Hold (ua=10:1:1). Empty: off.")]
        public string follow = "";

        static class Logger
        {
            public static void LogInfo(object m) => Debug.Log("[Delirium] " + m);
            public static void LogWarning(object m) => Debug.LogWarning("[Delirium] " + m);
            public static void LogError(object m) => Debug.LogError("[Delirium] " + m);
        }
#endif
        internal static bool HeadshotKills;
        /// The Force settings: every shot's, blow's and explosion's strength times these.
        internal static float ShotForce = 1f, MeleeForce = 1f, ExplosionForce = 1f;
        /// The Body settings: fit each character's weight and balance to its own shape (off: the game man's on its bones),
        /// and every body's strength.
        internal static bool Adaptive = true;
        internal static float Strength = 1f;
        /// The Body settings' RecentreJoints, Hold (mask, stiffness, damping, gravity) and Follow (mask, weight).
        internal static bool Recentre, ArmsOutEarly, EnginePhysics;
        internal static List<(string mask, float[] v)> Holds = new List<(string, float[])>(), Follows = new List<(string, float[])>();
        /// The Body settings' body (Body.Make), and SetBody's for one character.
        internal static Body Shape = new Body("Fitted", 0, 1.5f, true, "");
        internal static readonly Dictionary<Transform, Body> Shapes = new Dictionary<Transform, Body>();
        internal static readonly Dictionary<Transform, float> Strong = new Dictionary<Transform, float>();
        /// The Overrides setting's (behaviour, param, value) for every character, and SetOverride's for one.
        internal static List<(string, string, float)> AllOverrides = new List<(string, string, float)>();
        internal static readonly Dictionary<Transform, List<(string, string, float)>> Overridden = new Dictionary<Transform, List<(string, string, float)>>();
        /// Where the data files are (StreamingAssets, or next to the BepInEx plugin): relative paths are under it.
        static string DataDir = "";
        static string Find(string p) => p == "" || Path.IsPathRooted(p) ? p : Path.Combine(DataDir, p);
        IntPtr build;
        readonly List<Doll> dolls = new List<Doll>();
        readonly List<Blend> blends = new List<Blend>();

        /// A ragdoll has lain still for half a second: (owner, lying face up, the way it should face once up: its
        /// feet's side when on its back, its head's when on its front; flat). Start your own get-up animation and call
        /// GetUp(owner), or leave it lying.
        public event Action<Transform, bool, Vector3> LyingStill;

        /// NM's own messages from a ragdoll's behaviours as they happen: (owner, name, behaviour slot, value), e.g. the
        /// balancer giving up or a catch fall landing. The Playroom's Feedback list shows what each behaviour sends.
        public event Action<Transform, string, int, float> Feedback;

        /// A ragdoll has lost its feet (the balancer gave up): (owner). Once per fall.
        public event Action<Transform> Fell;

        /// A ragdoll's state now (State(owner)).
        public struct DollState
        {
            /// Under NM at all (the rest is false/zero when not).
            public bool Ragdolled;
            public bool Balancing, Fallen, CatchingFall, RolledUp, Relaxing, Dead, InWater, Burning, Tased;
            /// 2 diving at its tackle target, 1 holding on, 0 neither.
            public int Tackling;
            /// Seconds lain still (0 while moving).
            public float LyingFor;
            /// Centre of mass and its speed (world); its whole mass (kg).
            public Vector3 Centre, Velocity;
            public float Mass;
            /// Settled (still and balanced), awake (its physics running), jumping, a task running, the learned get-up
            /// running, resting (lying still, at rest on the ground).
            public bool Settled, Awake, Jumping, TaskRunning, LearnedGetUp, Resting;
            /// Where the balancer stands (between its feet), null when it isn't balancing.
            public Vector3? Stance;
            /// Each of the 21 parts' contact flags (0: touching nothing).
            public int[] Contacts;
        }

        /// What the ragdoll under `owner` is doing now.
        public DollState State(Transform owner)
        {
            var d = dolls.FirstOrDefault(x => x.Owner == owner);
            if (d == null) return default;
            float[] pos = new float[3], vel = new float[3];
            nm_agent_get_com(d.agent, pos, vel);
            return new DollState
            {
                Ragdolled = true, Balancing = nm_agent_is_balancing(d.agent) == 1, Fallen = nm_agent_has_fallen(d.agent) == 1,
                CatchingFall = nm_agent_is_catching_fall(d.agent) == 1, RolledUp = nm_agent_is_rolled_up(d.agent) == 1,
                Relaxing = nm_agent_is_relaxing(d.agent) == 1, Dead = nm_agent_is_dead(d.agent) == 1, InWater = nm_agent_in_water(d.agent) == 1,
                Burning = nm_agent_is_burning(d.agent) == 1, Tased = nm_agent_is_tased(d.agent) == 1, Tackling = nm_agent_tackle_state(d.agent), LyingFor = d.Lying(out _, out _),
                Centre = new Vector3(pos[0], pos[2], pos[1]), Velocity = new Vector3(vel[0], vel[2], vel[1]), Mass = nm_agent_get_mass(d.agent),
                Settled = nm_agent_is_settled(d.agent) == 1, Awake = nm_agent_is_awake(d.agent) == 1, Jumping = nm_agent_jumping(d.agent) == 1,
                TaskRunning = nm_agent_task_running(d.agent) == 1, LearnedGetUp = nm_agent_learned_getup_running(d.agent) == 1,
                Resting = nm_agent_is_resting(d.agent, d.world) == 1,
                Stance = nm_agent_get_stance(d.agent, pos) == 1 ? new Vector3(pos[0], pos[2], pos[1]) : (Vector3?)null,
                Contacts = Enumerable.Range(0, 21).Select(k => nm_agent_get_part_contact(d.agent, k)).ToArray(),
            };
        }
        float clock;
        /// The loaded one, for other code: Instance.Ragdoll(enemy.transform, push).
        public static Plugin Instance { get; private set; }

#if BEPINEX
        [DllImport("kernel32", CharSet = CharSet.Unicode)]
        static extern IntPtr LoadLibrary(string path);
#endif

        void Awake()
        {
            Instance = this;
#if BEPINEX
            var dir = Path.GetDirectoryName(Info.Location);
            // The SDK library sits next to this plugin, where the game won't look for it on its own.
            LoadLibrary(Path.Combine(dir, "delirium_physics.dll"));
            gameExe = Config.Bind("Data", "GameExe", "", "The player's own game: its exe, the disc's default.xex, or the folder either is in (absolute).").Value;
            xrt = Config.Bind("Data", "Character", "character.xrt", "The character file (absolute, or next to this plugin).").Value;
            buildFile = Config.Bind("Data", "Build", "build.txt", "The build dump (absolute, or next to this plugin).").Value;
            taskParams = Config.Bind("Data", "TaskParams", "", "The tuning file (optional).").Value;
            boneMap = Config.Bind("Skeleton", "BoneMap", "", "Bones the name guess misses or gets wrong, as BoneName=Part pairs: Pelvis_jnt=Buttocks, Torso=Spine2, Tail=None. Parts: Buttocks, Thigh_Left, Shin_Left, Foot_Left, Thigh_Right, Shin_Right, Foot_Right, Spine0-3, Neck, Head, Clavicle_Left, Upper_Arm_Left, Lower_Arm_Left, Hand_Left and the same _Right.").Value;
            ragdollKey = Config.Bind("Keys", "Ragdoll", KeyCode.F9, "Ragdoll (or push again) the character you aim at.").Value;
            releaseKey = Config.Bind("Keys", "Release", KeyCode.F10, "Hand every ragdoll back to the game.").Value;
            getUpAfter = Config.Bind("Get up", "After", 0f, "Seconds lying still before a ragdoll goes back to its own animation by itself (0: never; mods call GetUp).").Value;
            getUpBlend = Config.Bind("Get up", "Blend", 0.4f, "Seconds to blend from the ragdoll's pose into the animation.").Value;
            getUpFront = Config.Bind("Get up", "FrontTrigger", "GetUpFront", "Animator trigger set when it gets up from lying face down (skipped if the Animator has none).").Value;
            getUpBack = Config.Bind("Get up", "BackTrigger", "GetUpBack", "Animator trigger set when it gets up from lying on its back.").Value;
            push = Config.Bind("Feel", "Push", 60f, "The push from the camera, N s.").Value;
            armsOutEarly = Config.Bind("Feel", "ArmsOutEarly", false, "Bracing for a car (or anything coming at it), both arms thrown out straight well before it hits, as the original demo (the Playroom's 'Arms out early when bracing'). Off: the game's, a late bend.").Value;
            balance = Config.Bind("Feel", "Balance", true, "Ragdolls try to keep their feet (off: they go limp).").Value;
            ShotForce = Config.Bind("Force", "Shot", 1f, "Every bullet's push times this (1: as the mod or Weapons table gives it).").Value;
            MeleeForce = Config.Bind("Force", "Melee", 1f, "Every punch's, kick's and melee weapon's push times this.").Value;
            ExplosionForce = Config.Bind("Force", "Explosion", 1f, "Every explosion's throw speed times this.").Value;
            weaponTable = Config.Bind("Force", "Weapons", "",
                "Weapons for ShootWeapon and Shotgun, name=set:push:damage[:pellets:between:clip:accuracy:range], comma separated, or the game's weaponinfo.xml (absolute, or next to this plugin). " +
                "set: the reaction (PISTOL_WEAPON, SMG_WEAPON, POWERFUL_WEAPON, * the defaults); push: the bullet's, N s (40 is a solid hit); damage: the health it takes " +
                "(a ped has 100-200; under 100 left goes down, under 50 is a kill); pellets: a shotgun's. Empty: pistol, smg, rifle, shotgun.").Value;
            stepLimit = Config.Bind("Feel", "StepLimit", true, "The game's balance runs out of steps (a push he can't take up in so many drops him). Off: he keeps stepping (the Playroom's).").Value;
            fireReaction = Config.Bind("Feel", "FireReaction", true, "A burning character reacts (flails, staggers, writhes). Off: he burns but carries on.").Value;
            getUpNet = Config.Bind("Data", "GetUpNet", "getup.net", "LearnedGetUp's network (absolute, or next to this plugin).").Value;
            walkNet = Config.Bind("Data", "WalkNet", "walk.net", "LearnedWalk's network (absolute, or next to this plugin).").Value;
            Adaptive = Config.Bind("Body", "Adaptive", true, "Fit each kind of character to its own shape: its weight from its skin and its " +
                "balance settings searched once in the background (a few seconds). Off: the game man's weight share and balance on its bones.").Value;
            Strength = Config.Bind("Body", "Strength", 1f, new ConfigDescription("Every body's muscles times this (1: as made). 1.5-2 for tough " +
                "enemies or a movie giant: at 2 a person stands a push of 80 N s instead of 60.", new AcceptableValueRange<float>(0.1f, 10f))).Value;
            body = Config.Bind("Body", "Body", "Fitted", new ConfigDescription("The body (the Playroom's): Fitted (the skeleton's own); that shape of " +
                "each character; Random; Custom (CustomBody). The mesh keeps its look: pick one near it.", new AcceptableValueList<string>(Body.Names))).Value;
            bodySeed = Config.Bind("Body", "Seed", 0, "Random's seed: the same seed, the same body (0: each character its own).").Value;
            bodyVariance = Config.Bind("Body", "Variance", 1.5f, new ConfigDescription("Random's spread (1.5 the Playroom's default).", new AcceptableValueRange<float>(1f, 4f))).Value;
            bodySymmetric = Config.Bind("Body", "Symmetric", true, "Random: the right side the left's.").Value;
            customBody = Config.Bind("Body", "CustomBody", "1, 1, 1, 1, 1, 1, 77, 0", "Custom's sizes against the fitted body: legs, arms, trunk, width, head, neck, weight (kg), height (m; 0 keeps it).").Value;
            enginePhysics = Config.Bind("Feel", "EnginePhysics", false, "His parts as Unity bodies: NM's own physics still moves him (the game's exactly), his parts follow it as kinematic rigidbodies pushing your rigidbodies and meeting triggers, and loose rigidbodies within 5 m hit him (as Object movers). Off: NM's own physics alone, with the level read in. Run physics at 60 Hz (Fixed Timestep 0.01667) for the game's feel.").Value;
            recentreJoints = Config.Bind("Body", "RecentreJoints", false, "Re-centre each joint's range on the pose it is handed over in (for rigs whose animation bends joints past NM's ranges: they would snap on the first step). Not the game's.").Value;
            follow = Config.Bind("Body", "Follow", "", "Joints that keep playing the animation while the rest ragdolls (a boxer's guard, a held gun): mask=weight, comma separated (1 all the way, 0.5 halfway between NM's and the animation's; masks as Hold's, ua upper arms). The Animator keeps running under the ragdoll. Give those joints strength with Hold (ua=10:1:1). Empty: off.").Value;
            hold = Config.Bind("Body", "Hold", "", "Joints held at least this stiff, damped and gravity-opposing (0-1), whatever the reactions set: mask=stiffness:damping:gravity, comma separated (masks as setStiffness's: fb whole body, ua upper arms, ub upper body, lb lower body). fb=6:1:0.5 keeps a limp body from folding; empty: off.").Value;
            overrides = Config.Bind("Body", "Overrides", "", "Behaviour parameters changed for every character (the Playroom's Parameters tab): shot.bodyStiffness=7, bodyBalance.useHeadLook=0.").Value;
            HeadshotKills = Config.Bind("Feel", "HeadshotKills", true, "A shot whose nearest body part is the head kills: health 0, the body goes limp and never gets up by itself.").Value;
            sizeMode = Config.Bind("Feel", "Size", "Auto", new ConfigDescription(
                "Real: NM's body is the character's own size (true physics; it keeps its balance only within about 0.9-1.1 of a person). " +
                "Homer: NM runs a person-sized body and the world is scaled to it, its clock slowed for big characters and sped up for small " +
                "ones, so a giant balances and falls at a giant's pace. Auto: Real when its legs are within 0.9-1.1 of a person's, Homer otherwise.",
                new AcceptableValueList<string>("Auto", "Real", "Homer"))).Value;
            var tunes = Path.Combine(BepInEx.Paths.ConfigPath, "delirium-tunes.txt");
#else
            DontDestroyOnLoad(gameObject);
            var dir = Application.streamingAssetsPath;
            var tunes = Path.Combine(Application.persistentDataPath, "delirium-tunes.txt");
            (ShotForce, MeleeForce, ExplosionForce, Adaptive, Strength, HeadshotKills) = (shotForce, meleeForce, explosionForce, adaptive, strength, headshotKills);
#endif
            Shape = new Body(body, bodySeed, bodyVariance, bodySymmetric, customBody);
            AllOverrides = Body.ParseOverrides(overrides);
            Recentre = recentreJoints;
            EnginePhysics = enginePhysics;
            ArmsOutEarly = armsOutEarly;
            Holds = MaskList(hold, 3, "Hold", "mask=stiffness:damping:gravity");
            Follows = MaskList(follow, 1, "Follow", "mask=weight");
            DataDir = dir;
            try
            {
                // Native crashes land in the game's own dump; a panic in the SDK writes delirium_physics-crash.txt here.
                nm_crash_report(Path.GetDirectoryName(tunes));
                nm_set_step_limit(stepLimit ? 1 : 0);
                nm_set_fire_reaction(fireReaction ? 1 : 0);
                if (LoadWeapons(weaponTable.EndsWith(".xml") ? Find(weaponTable) : weaponTable) < 0) Logger.LogWarning($"Weapons: {LastError()}");
                if (gameExe != "") Check(nm_unlock(gameExe));
                Check(nm_init(Find(xrt)));
                if (taskParams != "") Check(nm_load_task_params(Find(taskParams)));
                // Each kind's balance settings, once found, kept for the next time the game runs.
                nm_tune_file(tunes);
                build = nm_build_load(Find(buildFile));
                if (build == IntPtr.Zero) throw new Exception(LastError());
                nm_set_headshot_kills(HeadshotKills ? 1 : 0);
                Logger.LogInfo($"ready: {ragdollKey} ragdolls the character you aim at, {releaseKey} lets go");
            }
            catch (Exception e)
            {
                Logger.LogError($"not started: {e.Message} (check the Data settings)");
                enabled = false;
            }
        }

        /// "mask=a:b:c, ..." as (mask, its `count` numbers); unreadable items warned of and skipped.
        List<(string, float[])> MaskList(string text, int count, string name, string form)
        {
            var list = new List<(string, float[])>();
            foreach (var h in text.Split(','))
            {
                var kv = h.Split('=');
                var v = kv.Length == 2 ? kv[1].Split(':').Select(x => float.TryParse(x, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var f) ? f : float.NaN).ToArray() : new float[0];
                if (v.Length == count && !v.Any(float.IsNaN)) list.Add((kv[0].Trim(), v));
                else if (h.Trim() != "") Logger.LogWarning($"{name}: can't read \"{h.Trim()}\" ({form})");
            }
            return list;
        }

        /// Feedback gathered over the steps, raised once the frame's are done.
        readonly List<(Transform, string, int, float)> said = new List<(Transform, string, int, float)>();
        readonly byte[] said_buf = new byte[64];
        readonly float[] said_value = new float[1];

        void Heard(Doll d)
        {
            if (Feedback == null) return;
            for (int i = 0, n = nm_agent_feedback_count(d.agent); i < n; i++)
                if (nm_agent_feedback_get(d.agent, i, said_buf, said_buf.Length, out int slot, said_value) >= 0)
                    said.Add((d.Owner, System.Text.Encoding.ASCII.GetString(said_buf, 0, Math.Max(0, Array.IndexOf(said_buf, (byte)0))), slot, said_value[0]));
        }

        /// EnginePhysics: the parts' bodies to where NM has them before Unity's physics step (they push what's in the
        /// way); every quarter second, loose rigidbodies near them to NM as movers.
        void FixedUpdate()
        {
            Prune();
            look -= Time.fixedDeltaTime;
            bool near = look <= 0;
            if (near) look = 0.25f;
            foreach (var d in dolls.Where(d => d.Engine))
            {
                d.Follow();
                if (near) MoversNear(d);
            }
        }
        float look;

        /// EnginePhysics: loose rigidbodies within 5 m of `d` not yet registered, to NM as Object movers (sized as
        /// RegisterMover does), so they hit him and he them.
        void MoversNear(Doll d)
        {
            foreach (var c in Physics.OverlapSphere(d.Hips.position, 5 * d.scale, ~0, QueryTriggerInteraction.Ignore))
            {
                var rb = c.attachedRigidbody;
                if (rb == null || rb.isKinematic || cars.Any(x => x.T == rb.transform)) continue;
                RegisterMover(rb.transform, MoverKind.Object);
            }
        }

        /// Characters (or EnginePhysics bodies) the game destroyed while ragdolled: their NM bodies go too.
        void Prune()
        {
            foreach (var d in dolls.Where(d => d.Gone).ToList())
            {
                d.Dispose();
                dolls.Remove(d);
            }
        }

        void Update()
        {
            Prune();
            if (Pressed(releaseKey))
            {
                foreach (var d in dolls) d.Dispose();
                dolls.Clear();
            }
            if (Pressed(ragdollKey)) Grab();
            // NM steps at 60 Hz whatever the game's own rate; at most 4 a frame so a hitch can't snowball.
            clock = Mathf.Min(clock + Time.deltaTime, 4 / 60f);
            Cars();
            for (; clock >= 1 / 60f; clock -= 1 / 60f)
                // Each world's ragdolls at once, spread over the machine's cores.
                foreach (var g in dolls.GroupBy(d => d.world))
                {
                    var agents = g.Select(d => d.agent).ToArray();
                    Check(nm_world_step_agents(g.Key, agents, agents.Length, 1 / 60f, 0));
                    foreach (var d in g) Heard(d);
                }
            var now = said.ToList();
            said.Clear();
            foreach (var (who, name, slot, v) in now) Feedback?.Invoke(who, name, slot, v);
            foreach (var d in dolls.ToList())
            {
                bool down = nm_agent_has_fallen(d.agent) == 1;
                if (down && !d.Down) Fell?.Invoke(d.Owner);
                d.Down = down;
            }
            // Car hits gathered over the steps: each reacts as the game picks (brace, roll up, stagger, limp).
            foreach (var d in dolls.ToList())
            {
                var pos = d.Owner.position;
                bool player = cars.Count > 0 && cars.OrderBy(c => (c.T.position - pos).sqrMagnitude).First().Player;
                var dmg = new float[1];
                var (health, agility) = Toughness.TryGetValue(d.Owner, out var t) ? t : (200f, 5);
                if (nm_agent_car_hit(d.agent, d.world, health, agility, player ? 1 : 0, dmg, out int box) != 1) continue;
                d.Bracing = -1;
                CarHit?.Invoke(d.Owner, dmg[0], box >= 0 && box < cars.Count ? cars[box].T : null);
            }
            foreach (var d in dolls.ToList())
            {
                float still = d.Lying(out bool up, out Vector3 facing);
                if (still < 0.5f) { d.Told = false; continue; }
                if (!d.Told) { d.Told = true; LyingStill?.Invoke(d.Owner, up, facing); }
                if (nm_agent_should_get_up(d.agent, getUpAfter) == 1 && dolls.Contains(d)) GetUp(d.Owner);
            }
        }

        /// Hands the ragdoll under `owner` back to its animation: stands its root where the body lies, turned the way it
        /// should face, sets the Animator's get-up trigger (FrontTrigger / BackTrigger) when it has one, and blends from
        /// the ragdoll's pose into whatever the Animator plays over `blend` seconds (the Blend setting when negative).
        /// For your own animation: start it, then call this. False when it isn't a ragdoll.
        public bool GetUp(Transform owner, float blend = -1)
        {
            var d = dolls.FirstOrDefault(x => x.Owner == owner);
            if (d == null) return false;
            d.Lying(out bool up, out Vector3 facing);
            var b = new Blend(d.Bones, blend < 0 ? getUpBlend : blend);
            var root = d.Hips.position;
            root.y = d.Floor();
            dolls.Remove(d);
            // The bones are kept where they were; only the root moves under them.
            owner.SetPositionAndRotation(root, Quaternion.LookRotation(facing, Vector3.up));
            var animators = d.Release();
            var trigger = up ? getUpBack : getUpFront;
            foreach (var a in animators)
                if (a.parameters.Any(x => x.type == AnimatorControllerParameterType.Trigger && x.name == trigger)) a.SetTrigger(trigger);
            blends.Add(b);
            return true;
        }

        /// The ragdoll's last pose, fading into the animation's.
        class Blend
        {
            readonly Transform[] bones;
            readonly Quaternion[] from;
            readonly Vector3 hips;
            readonly float length;
            float t;

            public Blend(Transform[] bones, float length)
            {
                this.bones = bones;
                from = bones.Select(b => b.localRotation).ToArray();
                from[0] = bones[0].rotation;
                hips = bones[0].position;
                this.length = Mathf.Max(length, 1e-3f);
            }

            /// After the animation has posed the bones; false when done.
            public bool Apply()
            {
                t += Time.deltaTime;
                float k = Mathf.SmoothStep(0, 1, t / length);
                if (bones[0] == null) return false;
                bones[0].SetPositionAndRotation(Vector3.Lerp(hips, bones[0].position, k), Quaternion.Slerp(from[0], bones[0].rotation, k));
                for (int b = 1; b < bones.Length; b++)
                    if (bones[b] != null) bones[b].localRotation = Quaternion.Slerp(from[b], bones[b].localRotation, k);
                return t < length;
            }
        }

        bool newInput;

        /// The key, from the old input system, or the new one (Unity.InputSystem) in games that turned the old one off.
        bool Pressed(KeyCode key)
        {
            if (!newInput)
                try { return Input.GetKeyDown(key); }
                catch (InvalidOperationException) { newInput = true; }
            // Keyboard.current.f9Key.wasPressedThisFrame, by name: the plugin doesn't need the package to build.
            var kb = Type.GetType("UnityEngine.InputSystem.Keyboard, Unity.InputSystem")?.GetProperty("current")?.GetValue(null);
            var k = kb?.GetType().GetProperty(char.ToLower(key.ToString()[0]) + key.ToString().Substring(1) + "Key")?.GetValue(kb);
            return k != null && (bool)k.GetType().GetProperty("wasPressedThisFrame").GetValue(k);
        }

        void LateUpdate()
        {
            // After the game's animation, so the ragdoll's pose is the one drawn.
            foreach (var d in dolls) { d.Feed(); d.Pose(); }
            blends.RemoveAll(b => !b.Apply());
        }

        void OnDestroy()
        {
            foreach (var d in dolls) d.Dispose();
            if (build != IntPtr.Zero) nm_build_destroy(build);
        }

        /// The character nearest the camera's aim: each skinned mesh's hips bone, the one closest to the aim line.
        void Grab()
        {
            var cam = Camera.main;
            if (cam == null) { Logger.LogWarning("no main camera"); return; }
            var ray = new Ray(cam.transform.position, cam.transform.forward);
            var map = Overrides();
            Transform best = null;
            float bestOff = 2f;
            foreach (var r in FindObjectsOfType<SkinnedMeshRenderer>())
            {
                var hips = HipsOf(r.bones, map);
                if (hips == null) continue;
                var to = hips.position - ray.origin;
                float along = Vector3.Dot(to, ray.direction), off = Vector3.Cross(ray.direction, to).magnitude;
                if (along > 0 && off < bestOff) (best, bestOff) = (hips, off);
            }
            if (best == null) { Logger.LogWarning("no character in your aim (or none whose hips bone it knows: see BoneMap)"); return; }
            // The character: the nearest object above the hips with an Animator, else the scene root.
            Take(best, best.GetComponentInParent<Animator>()?.transform ?? best.root, ray.direction * push, map);
        }

        /// Ragdolls the character under `owner` (its skinned meshes' skeleton; everything under it is paused and kept
        /// out of the level), pushed by `impulse` (N s) through the hips, or pushes it again. False when it has no
        /// skeleton whose hips it knows; the log then lists its bones.
        public bool Ragdoll(Transform owner, Vector3 impulse) => Doll(owner, impulse, true) != null;

        /// Shoots the character under `owner` (ragdolling it first): hit at `point`, the bullet going along `dir`. It
        /// reacts as the game's shot peds do (staggers, leans, clutches, falls when it can't keep up), and a hit while
        /// it reacts carries the reaction on. `force` the bullet's push (N s), `set` the weapon's taskparams set
        /// ("POWERFUL_WEAPON", "SMG_WEAPON", "PISTOL_WEAPON", ..., "*" the defaults), `health` the ped's (100-200).
        /// A shot along the shooter's aim (`from` the gun or camera, `aim` its direction) instead of a hit point: it strikes
        /// where the aim passes closest to one of the character's bones, so a game whose hitbox is one big capsule still
        /// gets head and limb hits (and HeadshotKills).
        public bool ShootAlong(Transform owner, Vector3 from, Vector3 aim, float force = 40f, string set = "POWERFUL_WEAPON", float health = 200f)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null) return false;
            d.Shoot(d.AimHit(from, aim), aim, force, set, health);
            return true;

        }

        /// A shot from the Weapons table along the shooter's aim (as ShootAlong): its reaction, push and damage. Returns the
        /// health left (`health` less the weapon's damage; take it as yours), or -1 when `owner` has no character.
        public float ShootWeapon(Transform owner, Vector3 from, Vector3 aim, string weapon, float health = 200f)
        {
            var w = Weapon(weapon);
            if (w.set == null)
            {
                Logger.LogWarning($"ShootWeapon: no weapon \"{weapon}\" in the Weapons setting; shot with the defaults, no damage");
                w = ("*", new[] { 40f, 0f, 1f, 0f, 0f, 0f, 0f });
            }
            float left = Math.Max(health - w.v[1], 0f);
            return ShootAlong(owner, from, aim, w.v[0], w.set, left) ? left : -1f;
        }

        /// A weapon of the table: its reaction set and push, damage, pellets, ms between shots, clip, accuracy, range (m);
        /// set null when there's no such weapon.
        static (string set, float[] v) Weapon(string name)
        {
            int k = nm_weapon_find(name);
            var (set, v) = (new byte[64], new float[7]);
            if (k < 0 || nm_weapon_get(k, null, 0, set, 64, v) < 0) return (null, v);
            return (System.Text.Encoding.UTF8.GetString(set, 0, Array.IndexOf(set, (byte)0)), v);
        }

        /// The weapons table's names (the default pistol, smg, rifle, shotgun, or what the Weapons setting loaded).
        public static string[] WeaponNames() => Enumerable.Range(0, nm_weapon_count()).Select(i => Str((b, n) => nm_weapon_get(i, b, n, null, 0, null))).ToArray();

        /// The weapons table from text ("name=SET:push:damage[:pellets:between:clip:accuracy:range], ..."; empty: the
        /// default four) or a weaponinfo.xml path. Returns how many (negative: unreadable, LastError says why).
        public static int LoadWeapons(string text) =>
            text.EndsWith(".xml") ? nm_weapons_load_info(text) : nm_weapons_parse(text == "" ? null : text);

        int shotgunSeed = 1;
        /// A shotgun blast from the table's `weapon` along the aim (`from` the gun, `aim` its direction): its pellets fanned
        /// as the game's (its accuracy and range), those striking the character pushing as one shot. Returns how many
        /// struck (-1: no character or no such weapon). `health` the ped's after the blast's damage.
        public int Shotgun(Transform owner, Vector3 from, Vector3 aim, string weapon = "shotgun", float health = 150f)
        {
            var w = Weapon(weapon);
            if (w.set == null) { Logger.LogWarning($"Shotgun: no weapon \"{weapon}\""); return -1; }
            var d = Doll(owner, Vector3.zero, false);
            if (d == null) return -1;
            int n = Math.Max((int)w.v[2], 1);
            var dirs = new float[(n + 1) * 3];
            int made = nm_shotgun_fan(global::Delirium.Ragdoll.Doll.Nm(aim.normalized), w.v[5], w.v[6], n, ref shotgunSeed, dirs);
            var hits = new List<float>();
            var at = new float[3];
            for (int i = 1; i < made; i++)
            {
                var dir = new[] { dirs[i * 3], dirs[i * 3 + 1], dirs[i * 3 + 2] };
                int part = nm_agent_aim_hit(d.agent, global::Delirium.Ragdoll.Doll.P(from), dir, at);
                if (part >= 0) hits.AddRange(new[] { part, at[0], at[1], at[2], dir[0], dir[1], dir[2] });
            }
            if (hits.Count == 0) return 0;
            d.Run(x => nm_agent_task_shotgun(x.agent, x.world, w.set, hits.ToArray(), hits.Count / 7, health, w.v[0] * ShotForce));
            return hits.Count / 7;
        }

        /// Where the ragdoll under `owner` has its head (NM's head part, in the game's world); null when it isn't one.
        public Vector3? Head(Transform owner) => dolls.FirstOrDefault(x => x.Owner == owner)?.Part(12);

        /// An explosion at `at` reaching `owner` (call it for each character in range): thrown out and up at up to `speed`
        /// m/s, less with distance, then the game's explosion reaction for 6 s. Take its health yourself.
        public bool Explode(Transform owner, Vector3 at, float radius = 5f, float speed = 15f)
        {
            var d = Doll(owner, Vector3.zero, false);
            d?.Run(x => nm_agent_task_explosion(x.agent, x.world, global::Delirium.Ragdoll.Doll.Nm(at), radius, speed * ExplosionForce));
            return d != null;
        }

        /// A punch, kick or melee weapon blow that doesn't kill: `impulse` at `point`, then a flinch or a stagger toward
        /// keeping its feet, looking at the attacker's `attackerHead` (the game's 7-in-10 pick). `timid` flinches more.
        public bool MeleeHit(Transform owner, Vector3 point, Vector3 impulse, Vector3 attackerHead, bool timid = false)
        {
            var d = Doll(owner, Vector3.zero, false);
            d?.Run(x => nm_agent_task_melee_hit(x.agent, x.world, global::Delirium.Ragdoll.Doll.Nm(point), global::Delirium.Ragdoll.Doll.Nm(impulse * MeleeForce), global::Delirium.Ragdoll.Doll.Nm(attackerHead), timid ? 1 : 0));
            return d != null;
        }

        /// Catches fire: flails, staggers, drops and writhes. Call again with `burning` false once the flames are out (it
        /// calms down), and with its health as it burns (0: dies burning; then call Kill).
        public bool OnFire(Transform owner, bool burning = true, float health = 200f)
        {
            var d = Doll(owner, Vector3.zero, false);
            d?.Run(x => burning ? nm_agent_task_on_fire(x.agent, x.world, health) : nm_agent_set_on_fire(x.agent, 0, health));
            return d != null;
        }

        /// A taser's hit: a current through it for `seconds` (it jolts and locks, the Playroom's), ending by itself; 0
        /// ends it now. No damage.
        public bool Tase(Transform owner, float seconds = 2.5f)
        {
            var d = Doll(owner, Vector3.zero, false);
            d?.Run(x => nm_agent_tase(x.agent, x.world, seconds));
            return d != null;
        }

        /// Cuts off the body part nearest `point` (with what's below it: a hand, an arm, a leg, the head), `impulse` on the
        /// piece. The piece goes limp on its own; a leg gone, it stays down. Show the cut yourself (hide the limb's mesh,
        /// spawn your gib); a head cut doesn't kill: call Kill. False when the point is nearest the hips or cut already.
        public bool Sever(Transform owner, Vector3 point, Vector3 impulse = default)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null) return false;
            var at = global::Delirium.Ragdoll.Doll.Nm(point);
            // Nearest the hips (the root) or cut already: nothing to cut.
            int part = nm_agent_nearest_part(d.agent, at);
            if (part <= 0 || nm_agent_is_severed(d.agent, part) == 1) return false;
            d.Run(x => nm_agent_sever(x.agent, x.world, part, at, global::Delirium.Ragdoll.Doll.Nm(impulse)));
            return true;
        }

        /// A leash: the body part nearest `point` tied to `anchor` (world), at most `length` metres away. No stretch:
        /// slack inside it, he hangs and swings on it past that. Returns the part (for LeashLength / LeashAnchor /
        /// Unleash), -1 if none. To wrap it round a corner, move the anchor there and take what's used up off the length.
        public int Leash(Transform owner, Vector3 point, Vector3 anchor, float length)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null || length <= 0) return -1;
            var at = global::Delirium.Ragdoll.Doll.Nm(point);
            int part = nm_agent_nearest_part(d.agent, at);
            if (part < 0) return -1;
            d.Run(x => nm_agent_leash(x.agent, part, at, global::Delirium.Ragdoll.Doll.Nm(anchor), length));
            return part;
        }

        /// Leash `part`'s new length (shorter reels him in at 2 m/s).
        public bool LeashLength(Transform owner, int part, float length)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null || length <= 0) return false;
            d.Run(x => nm_agent_leash(x.agent, part, null, null, length));
            return true;
        }

        /// Leash `part`'s anchor moved (a moving holder, or the corner it wraps round).
        public bool LeashAnchor(Transform owner, int part, Vector3 anchor, float length)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null || length <= 0) return false;
            d.Run(x => nm_agent_leash(x.agent, part, null, global::Delirium.Ragdoll.Doll.Nm(anchor), length));
            return true;
        }

        /// Lets leash `part` go (-1: every one).
        public bool Unleash(Transform owner, int part = -1)
        {
            var d = Doll(owner, Vector3.zero, false);
            d?.Run(x => nm_agent_unleash(x.agent, part));
            return d != null;
        }

        /// Goes limp and stays down (a kill or a knock-out): it never gets up by itself.
        public bool Kill(Transform owner)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null) return false;
            d.Run(x => nm_agent_set_dead(x.agent, 1));
            d.Run(x => nm_agent_task_relax(x.agent, -1f, -1f));
            return true;
        }

        /// Any reaction, written as the Playroom console's line: its NM message name, then name=value params ("rollUp start=true", "bodyWrithe start=true armStiffness=12", "stopAllBehaviours"): ragdolls it first
        /// if it isn't one. Vectors are x,y,z in NM's space (Z up, metres from the world's origin). False when NM refuses the line.
        public bool React(Transform owner, string line)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null) return false;
            if (nm_msg_send_text(d.agent, d.world, line) >= 0) return true;
            Debug.LogWarning($"[NM] React \"{line}\": {LastError()}");
            return false;
        }

        /// A registered car (RegisterCar): its object, its box (centre in its own frame, half size), its place last frame.
        class Car { public Transform T; public Vector3 Centre, Half, Last, Vel, Spin; public Quaternion Turned; public bool Player, Brace, KnockDown; public MoverKind Kind; }
        readonly List<Car> cars = new List<Car>();

        /// What a registered mover is, for the game's damage and reaction when it hits: a car (a truck, a platform), an
        /// object (a crate, a door, debris: half a car's damage, a hard hit unless the character stands on it), a train
        /// (its own hurt, ahead of its front).
        public enum MoverKind { Car = NM_HIT_CAR, Object = NM_HIT_OBJECT, Train = NM_HIT_TRAIN }

        /// Each character's health and agility for hits (SetToughness); 200 and 5 without.
        readonly Dictionary<Transform, (float, int)> Toughness = new Dictionary<Transform, (float, int)>();

        /// `owner`'s health now and agility (0-255, the game's; under 3 is weak: it goes down sooner) for the game's
        /// reaction to a mover's hit. Set it as its health changes.
        public void SetToughness(Transform owner, float health, int agility = 5) => Toughness[owner] = (health, agility);

        /// A car hit a ragdoll: (who, the health it takes, the car). Take it off your character's health yourself.
        public event Action<Transform, float, Transform> CarHit;

        /// Anything you call a car (a car, a truck, a platform, a wrecking ball): ragdolls can't go into its box and meet it
        /// at its speed, a standing character it's about to hit is ragdolled, and a hit reacts as the game's (brace, roll
        /// up, stagger, limp) with CarHit. `size`: its box (zero: its first BoxCollider, else its renderers' bounds as it
        /// stands now); `playerDriven`: the player drives it (the game's reactions differ). Unregister it when it's gone.
        public void RegisterCar(Transform car, Vector3 size = default, bool playerDriven = false) =>
            RegisterMover(car, MoverKind.Car, size, playerDriven);

        /// Anything that moves and hits characters, as `kind`: as RegisterCar, and `brace`: a standing character in its way
        /// braces against it as it's ragdolled (cars' default; off for objects); `knockDown`: a standing character in its
        /// way is ragdolled at all (off: only ragdolls meet it). `centre`: the box's centre in its frame, with `size`.
        public void RegisterMover(Transform car, MoverKind kind, Vector3 size = default, bool playerDriven = false, bool? brace = null,
            bool knockDown = true, Vector3 centre = default)
        {
            if (car == null) return;
            UnregisterCar(car);
            var half = size / 2;
            if (size == Vector3.zero)
            {
                var b = car.GetComponentInChildren<BoxCollider>();
                // ponytail: a box collider turned off the car's axes, or bounds taken while it's turned, give a loose box.
                if (b != null) (centre, half) = (car.InverseTransformPoint(b.transform.TransformPoint(b.center)), Vector3.Scale(b.size, b.transform.lossyScale) / 2);
                else
                {
                    var rs = car.GetComponentsInChildren<Renderer>();
                    if (rs.Length == 0) { Logger.LogWarning($"RegisterCar {car.name}: no BoxCollider or renderer to size it by: give a size"); return; }
                    var bounds = rs[0].bounds;
                    foreach (var r in rs) bounds.Encapsulate(r.bounds);
                    (centre, half) = (car.InverseTransformPoint(bounds.center), bounds.extents);
                }
            }
            cars.Add(new Car { T = car, Centre = centre, Half = half, Last = car.position, Turned = car.rotation, Player = playerDriven, Kind = kind,
                Brace = brace ?? kind != MoverKind.Object, KnockDown = knockDown });
        }

        public void UnregisterCar(Transform car) => cars.RemoveAll(c => c.T == car);

        /// A registered car is about to hit `owner` (your AI saw it coming): it's ragdolled and braces against it, hands
        /// out (ArmsOutEarly: well before), maybe a jump, until the hit or 3 s.
        public bool CarComing(Transform owner, Transform car)
        {
            int k = cars.FindIndex(c => c.T == car);
            if (k < 0) { Logger.LogWarning($"CarComing: {car?.name} isn't registered (RegisterCar)"); return false; }
            var d = Doll(owner, Vector3.zero, true);
            if (d == null) return false;
            Check(nm_agent_task_brace(d.agent, d.world, null));
            (d.Bracing, d.BraceLeft) = (k, 3f);
            return true;
        }

        /// Dragged from a registered car: both hands on its side, kicking and writhing once it moves, letting go at
        /// 12 m/s or after 6 s.
        public bool CarDoor(Transform owner, Transform car)
        {
            int k = cars.FindIndex(c => c.T == car);
            if (k < 0) { Logger.LogWarning($"CarDoor: {car?.name} isn't registered (RegisterCar)"); return false; }
            var d = Doll(owner, Vector3.zero, true);
            if (d == null) return false;
            Cars();
            d.Run(x => nm_agent_task_car_door(x.agent, x.world, k, null));
            return true;
        }

        /// The cars into every ragdoll's world (before its steps); standing characters one is about to hit ragdolled; the
        /// braces aimed at theirs.
        void Cars()
        {
            cars.RemoveAll(c => c.T == null);
            if (cars.Count == 0) return;
            var boxes = new float[cars.Count * 13];
            for (int k = 0; k < cars.Count; k++)
            {
                var c = cars[k];
                var q = c.T.rotation;
                if (Time.deltaTime > 0)
                {
                    c.Vel = (c.T.position - c.Last) / Time.deltaTime;
                    // Its spin: the turn since last frame, the short way round, over the frame.
                    (q * Quaternion.Inverse(c.Turned)).ToAngleAxis(out var deg, out var axis);
                    if (deg > 180f) deg -= 360f;
                    c.Spin = float.IsInfinity(axis.x) || deg == 0f ? Vector3.zero : axis * (deg * Mathf.Deg2Rad / Time.deltaTime);
                }
                (c.Last, c.Turned) = (c.T.position, q);
                var at = c.T.TransformPoint(c.Centre);
                global::Delirium.Ragdoll.Doll.P(at).CopyTo(boxes, k * 13);
                new[] { -q.x, -q.z, -q.y, q.w, c.Half.x, c.Half.z, c.Half.y }.CopyTo(boxes, k * 13 + 3);
                global::Delirium.Ragdoll.Doll.Nm(c.Vel).CopyTo(boxes, k * 13 + 10);
                // A quarter second ahead, a hand's width round it: whoever's in there and still on their feet goes to NM.
                if (c.Vel.magnitude < 2f || !c.KnockDown) continue;
                foreach (var hit in Physics.OverlapBox(at + c.Vel * 0.25f, c.Half + Vector3.one * 0.3f, q))
                {
                    var who = hit.GetComponentInParent<Animator>()?.transform;
                    if (who == null || who.IsChildOf(c.T) || dolls.Any(d => d.Owner == who) || who.GetComponentInChildren<SkinnedMeshRenderer>() == null) continue;
                    var hitBy = Doll(who, Vector3.zero, true);
                    if (hitBy == null || !c.Brace) continue;
                    Check(nm_agent_task_brace(hitBy.agent, hitBy.world, null));
                    (hitBy.Bracing, hitBy.BraceLeft) = (k, 3f);
                }
            }
            var kinds = cars.Select(c => (int)c.Kind | (c.Player ? NM_HIT_PLAYER : 0)).ToArray();
            // Unity's spin (left-handed) as NM's.
            var spins = cars.SelectMany(c => new[] { -c.Spin.x, -c.Spin.z, -c.Spin.y }).ToArray();
            foreach (var w in dolls.Select(d => d.world).Distinct())
            {
                Check(nm_world_set_boxes(w, boxes, cars.Count));
                Check(nm_world_set_box_kinds(w, kinds, cars.Count));
                Check(nm_world_set_box_spins(w, spins, cars.Count));
            }
            foreach (var d in dolls.Where(d => d.Bracing >= 0))
            {
                if ((d.BraceLeft -= Time.deltaTime) <= 0 || d.Bracing >= cars.Count) { d.Bracing = -1; continue; }
                var t = cars[d.Bracing].T;
                var m = global::Delirium.Ragdoll.Doll.Nm(t.right).Concat(global::Delirium.Ragdoll.Doll.Nm(t.forward)).Concat(global::Delirium.Ragdoll.Doll.Nm(t.up)).Concat(global::Delirium.Ragdoll.Doll.P(t.TransformPoint(cars[d.Bracing].Centre))).ToArray();
                Check(nm_agent_task_brace_control(d.agent, d.world, m, null, null));
            }
        }

        /// One of the game's tasks on the character (ragdolled first if it isn't one); false when it couldn't be.
        bool Task(Transform owner, Func<Doll, int> f)
        {
            var d = Doll(owner, Vector3.zero, false);
            d?.Run(f);
            return d != null;
        }

        /// Catching itself as it falls: hands out, then down (the game's catchFall).
        public bool CatchFall(Transform owner) => Task(owner, d => nm_agent_task_catch_fall(d.agent, null));

        /// Pushed off a ledge (a drop, a low wall, stairs): it looks 2 m out along `heading` (0-1 of a turn round from
        /// behind; negative: a random one) and goes over when there's one. False when there's none (it stays as it is).
        public bool FallDown(Transform owner, float heading = -1f)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null) return false;
            int r = nm_agent_task_fall_down(d.agent, d.world, heading);
            Plugin.Check(r);
            return r == 1;
        }

        /// Grabs hold (a ledge, a car's side): each hand reaches for its point (the surface's normal out of it; a zero
        /// normal leaves that hand free), has 1 s to get hold, then holds `hold` s (0: until another reaction).
        public bool Grab(Transform owner, Vector3 left, Vector3 leftNormal, Vector3 right, Vector3 rightNormal, float hold = 0f)
        {
            float[] Hand(Vector3 p, Vector3 n) => n == Vector3.zero ? null : global::Delirium.Ragdoll.Doll.P(p).Concat(global::Delirium.Ragdoll.Doll.Nm(n.normalized)).ToArray();
            return Task(owner, d => nm_agent_task_grab(d.agent, d.world, Hand(left, leftNormal), Hand(right, rightNormal), hold));
        }

        /// Jumping out of a moving car: tucks and rolls (give it the car's speed first: Push or its own movement).
        public bool JumpRoll(Transform owner) => Task(owner, d => nm_agent_task_jump_roll(d.agent, d.world));

        /// Flinching from a threat at `from` (a swing, a near miss).
        public bool Flinch(Transform owner, Vector3 from) => Task(owner, d => nm_agent_task_flinch(d.agent, d.world, null, global::Delirium.Ragdoll.Doll.P(from)));

        /// `owner` dives at `target` (both ragdolled first, within 4 m): its hands latch onto the target's chest (or
        /// thighs, `low`), it drives on and both go down. `keep`: it never lets go by itself.
        public bool Tackle(Transform owner, Transform target, bool low = false, bool keep = false)
        {
            var t = Doll(target, Vector3.zero, false);
            return t != null && Task(owner, d => nm_agent_task_tackle(d.agent, d.world, t.agent, low ? 1 : 0, keep ? 1 : 0));
        }

        /// Water under a ragdoll: its surface's height (world y; NaN: none). Down in it, it floats face up; it stops once
        /// well out. Set it as it moves over water. False when `owner` isn't a ragdoll.
        public bool SetWater(Transform owner, float level)
        {
            var d = dolls.FirstOrDefault(x => x.Owner == owner);
            d?.Run(x => nm_agent_set_water(x.agent, x.world, level));
            return d != null;
        }

        /// Falling from a height: arms out, legs ready (the game starts it when a character drops faster than 6 m/s).
        public bool HighFall(Transform owner, bool player = false)
        {
            var d = Doll(owner, Vector3.zero, false);
            d?.Run(x => nm_agent_task_high_fall(x.agent, x.world, null, player ? 1 : 0));
            return d != null;
        }

        public bool Shoot(Transform owner, Vector3 point, Vector3 dir, float force = 40f, string set = "POWERFUL_WEAPON", float health = 200f)
        {
            var d = Doll(owner, Vector3.zero, false);
            d?.Shoot(point, dir, force, set, health);
            return d != null;
        }

        /// One character's body (Body's names; Random's seed, 0: its own), from its next ragdoll on.
        public void SetBody(Transform owner, string body, int seed = 0) =>
            Shapes[owner] = new Body(body, seed, Shape.Variance, Shape.Symmetric, Shape.Custom);

        /// That character's muscles times `k` (on top of nothing: 0 goes back to Strength's), now and on every ragdoll.
        /// 1.3-2 for a heavy custom body that can't take a hit; under 1 for a weak one.
        public bool SetStrength(Transform owner, float k)
        {
            if (k > 0f) Strong[owner] = k; else Strong.Remove(owner);
            var d = dolls.FirstOrDefault(x => x.Owner == owner);
            return d == null || nm_agent_set_strength(d.agent, k > 0f ? k : Strength) >= 0;
        }

        /// Every `behaviour` message to that character from now on carries `param` as `value` (on/off: 0 or 1). False (the
        /// reason in the log) for a parameter it doesn't take.
        public bool SetOverride(Transform owner, string behaviour, string param, float value)
        {
            var d = dolls.FirstOrDefault(x => x.Owner == owner);
            if (d != null && nm_agent_set_override(d.agent, behaviour, param, value) < 0)
            {
                Logger.LogWarning($"SetOverride: {LastError()}");
                return false;
            }
            if (!Overridden.TryGetValue(owner, out var list)) Overridden[owner] = list = new List<(string, string, float)>();
            list.RemoveAll(o => o.Item1 == behaviour && o.Item2 == param);
            list.Add((behaviour, param, value));
            return true;
        }

        /// Drops that character's SetOverride values and the Overrides setting's for it: its messages go out as sent.
        public void ClearOverrides(Transform owner)
        {
            Overridden[owner] = null;
            var d = dolls.FirstOrDefault(x => x.Owner == owner);
            if (d != null) nm_agent_clear_overrides(d.agent);
        }

        /// Its balance (the game's TaskNMBalance), looking at `look` when given.
        public bool Balance(Transform owner, Vector3? look = null) =>
            Task(owner, d => nm_agent_task_balance(d.agent, d.world, null, look is Vector3 l ? global::Delirium.Ragdoll.Doll.P(l) : null));

        /// Stops the task it's running (Sit, Pose, Script, ...).
        public bool Stop(Transform owner) => Task(owner, d => nm_agent_task_stop(d.agent));

        /// The Playroom's extra reactions, on top of the game's (each its own switch).
        [Flags]
        public enum Reactions { None = 0, Windmill = NM_XR_WINDMILL, CurlUp = NM_XR_CURL_UP, CurlShot = NM_XR_CURL_SHOT, LookHit = NM_XR_LOOK_HIT,
            Ledge = NM_XR_LEDGE, SmartFall = NM_XR_SMART_FALL, Dodge = NM_XR_DODGE, Wall = NM_XR_WALL, ArmsOut = NM_XR_ARMS_OUT }
        public bool SetReactions(Transform owner, Reactions on) => Task(owner, d => nm_agent_set_reactions(d.agent, (int)on));

        /// Shot at again within `gap` seconds of a near miss, it dives (NearMiss).
        public bool SetDive(Transform owner, float gap) => Task(owner, d => nm_agent_set_dive(d.agent, gap));

        /// The Playroom's jump on its balance (loads, pushes off, flies, lands); `dive` head first along `dir` (null: its
        /// facing). False when it can't (not balancing).
        public bool Jump(Transform owner, bool dive = false, Vector3? dir = null)
        {
            var d = Doll(owner, Vector3.zero, true);
            return d != null && nm_agent_jump(d.agent, dive ? 1 : 0, dir is Vector3 v ? global::Delirium.Ragdoll.Doll.Nm(v) : null) == 1;
        }

        /// A bullet that missed it, from `from` along `dir`: close by its head or chest it flinches and looks there.
        /// True when it noticed.
        public bool NearMiss(Transform owner, Vector3 from, Vector3 dir)
        {
            var d = Doll(owner, Vector3.zero, true);
            return d != null && nm_agent_near_miss(d.agent, d.world, global::Delirium.Ragdoll.Doll.P(from), global::Delirium.Ragdoll.Doll.Nm(dir)) == 1;
        }

        /// Something thrown or dropped at it from `at` moving at `vel`: `look` it follows it with its head, `avoid` it
        /// flinches from it.
        public bool ThrownAt(Transform owner, Vector3 at, Vector3 vel, bool look = true, bool avoid = true) =>
            Task(owner, d => nm_agent_thrown_at(d.agent, d.world, global::Delirium.Ragdoll.Doll.P(at), global::Delirium.Ragdoll.Doll.Nm(vel), look ? 1 : 0, avoid ? 1 : 0));

        /// Sits (the game's TaskNMSit; the Animator's pose as its clip), looking at `look`, at `seat` (null: where it is).
        public bool Sit(Transform owner, Vector3? look = null, Vector3? seat = null) => Fed(owner, d =>
            nm_agent_task_sit(d.agent, d.world, look is Vector3 l ? global::Delirium.Ragdoll.Doll.P(l) : null, seat is Vector3 s ? global::Delirium.Ragdoll.Doll.P(s) : null));

        /// Its muscles take the Animator's pose (the game's TaskNMPose) at a body stiffness from `low` to `high`
        /// (negative: 4), until Stop.
        public bool Pose(Transform owner, float low = -1f, float high = -1f) => Fed(owner, d => nm_agent_task_pose(d.agent, d.world, low, high));

        /// Rolls up for `rollMs`, then goes limp at `stiffness` / `damping` (negative: the game's).
        public bool RollUpRelax(Transform owner, float stiffness = -1f, float damping = -1f, int rollMs = 1000) =>
            Task(owner, d => nm_agent_task_roll_up_relax(d.agent, d.world, stiffness, damping, rollMs));

        /// The ragdoll is yours for `ms` (0: until Stop): moved only by the messages you send (React).
        public bool Script(Transform owner, int ms = 0) => Task(owner, d => nm_agent_task_script(d.agent, ms));

        /// Gets up with its own muscles, the Animator's get-up clip as its pose and a spring helping its pelvis at most
        /// `help` of its weight (negative: half); it can fail. `on` false: done, it balances.
        public bool MuscleRise(Transform owner, bool on = true, float help = -1f) => Fed(owner, d => nm_agent_muscle_rise(d.agent, d.world, on ? 1 : 0, help));

        /// Lying, it gets up as the learned network did (`net`, else the GetUpNet setting's): 8 s with its own muscles.
        public bool LearnedGetUp(Transform owner, string net = null) => Task(owner, d => nm_agent_learned_getup(d.agent, Find(net ?? getUpNet)));

        /// Walks under the stick (SetStick) with the learned network tuning its balance (`net`, else the WalkNet
        /// setting's; `cycle` the walk clip's length it learned with). `on` false: its balance as it was.
        public bool LearnedWalk(Transform owner, bool on = true, string net = null, float cycle = 1f) =>
            Task(owner, d => nm_agent_learned_walk(d.agent, d.world, on ? Find(net ?? walkNet) : null, cycle));

        /// The game's drunk control: pushes it along the stick (`x`, `y` -1..1, up the stick forward) from the camera's
        /// heading `camera` (radians about up); `on` false lets go.
        public bool SetStick(Transform owner, bool on, float x = 0f, float y = 0f, float camera = 0f) => Task(owner, d => nm_agent_set_stick(d.agent, on ? 1 : 0, x, y, camera));

        /// Who it's up against: it looks at `attacker`'s head (a ragdoll's, else a person's height at its root) and, armed,
        /// points its gun at it (GunReady says when to fire). Null: no one.
        public bool SetAttacker(Transform owner, Transform attacker, bool armed = false)
        {
            float[] bones = null;
            if (attacker != null)
            {
                var a = dolls.FirstOrDefault(x => x.Owner == attacker);
                var at = a != null ? new[] { 0, 7, 9, 12 }.Select(a.Part) : new[] { 0.95f, 1.15f, 1.35f, 1.6f }.Select(h => attacker.position + Vector3.up * h);
                bones = at.SelectMany(global::Delirium.Ragdoll.Doll.P).ToArray();
            }
            return Task(owner, d => nm_agent_set_attacker(d.agent, bones, armed ? 1 : 0));
        }

        /// Armed against an attacker: true once it's ready to fire (you fire the shot).
        public bool GunReady(Transform owner) => dolls.FirstOrDefault(x => x.Owner == owner) is Doll d && nm_agent_gun_ready(d.agent) == 1;

        /// A behaviour's parameter as it runs now (as last sent, else its default): a float, a Vector3 (NM's space: x, y,
        /// z up) or a string; null when it has none.
        public object GetParam(Transform owner, string message, string param)
        {
            var d = dolls.FirstOrDefault(x => x.Owner == owner);
            if (d == null) return null;
            var v = new float[3];
            switch (nm_agent_get_param(d.agent, message, param, v))
            {
                case 1: return v[0];
                case 3: return new Vector3(v[0], v[1], v[2]);
                case 0:
                    var b = new byte[256];
                    return nm_agent_get_param_string(d.agent, message, param, b, 256) < 0 ? null : System.Text.Encoding.UTF8.GetString(b, 0, Array.IndexOf(b, (byte)0));
                default: return null;
            }
        }

        /// The body part a segment hits first (and where, its normal); -1 when none.
        public int Raycast(Transform owner, Vector3 from, Vector3 to, out Vector3 point, out Vector3 normal)
        {
            (point, normal) = (Vector3.zero, Vector3.zero);
            var d = dolls.FirstOrDefault(x => x.Owner == owner);
            if (d == null) return -1;
            float[] p = new float[3], n = new float[3];
            int k = nm_agent_raycast(d.agent, global::Delirium.Ragdoll.Doll.P(from), global::Delirium.Ragdoll.Doll.P(to), p, n);
            if (k >= 0) (point, normal) = (new Vector3(p[0], p[2], p[1]), new Vector3(n[0], n[2], n[1]));
            return k;
        }

        /// What's off in its setup (one line each; empty: nothing).
        public string CheckSetup(Transform owner) => dolls.FirstOrDefault(x => x.Owner == owner) is Doll d ? Str((b, n) => nm_agent_check(d.agent, b, n)) ?? "" : "";

        /// A message NM takes (React) and its parameters: name, kind (NM_KIND_*), used by the game's behaviours, doc and
        /// default (float, Vector3, else null).
        public class Message { public string Name; public List<(string name, int kind, bool used, string doc, object def)> Params; }
        public static List<Message> Messages() => Enumerable.Range(0, nm_message_count()).Select(m => new Message
        {
            Name = Str((b, n) => nm_message_name(m, b, n)),
            Params = Enumerable.Range(0, Math.Max(nm_message_param_count(m), 0)).Select(p =>
            {
                var v = new float[3];
                int k = nm_message_param_default(m, p, v);
                return (Str((b, n) => nm_message_param_name(m, p, b, n)), nm_message_param_kind(m, p), nm_message_param_used(m, p) == 1,
                    Str((b, n) => nm_message_param_doc(m, p, b, n)), k == 3 ? new Vector3(v[0], v[1], v[2]) : k == 1 ? (object)v[0] : null);
            }).ToList(),
        }).ToList();

        /// A task fed the Animator's pose each step (Sit, Pose, MuscleRise).
        bool Fed(Transform owner, Func<Doll, int> f)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null) return false;
            d.Fed = true;
            d.Run(f);
            return true;
        }

        Doll Doll(Transform owner, Vector3 impulse, bool balanced)
        {
            var map = Overrides();
            var all = owner.GetComponentsInChildren<SkinnedMeshRenderer>().SelectMany(r => r.bones).Where(b => b != null).Distinct().ToList();
            var hips = HipsOf(all, map);
            if (hips == null)
            {
                Logger.LogWarning($"{owner.name}: no hips among its bones (add them to BoneMap): {string.Join(", ", all.Select(b => b.name))}");
                return null;
            }
            return Take(hips, owner, impulse, map, balanced);
        }

        Doll Take(Transform hips, Transform owner, Vector3 impulse, Dictionary<string, int> map, bool balanced = true)
        {
            var doll = dolls.FirstOrDefault(d => d.Hips == hips);
            if (doll == null)
            {
                try { doll = new Doll(hips, owner, build, map, sizeMode); }
                catch (Exception e)
                {
                    Logger.LogWarning($"can't ragdoll {owner.name}: {e.Message}; its bones: {string.Join(", ", hips.GetComponentsInChildren<Transform>().Select(b => b.name))}");
                    return null;
                }
                dolls.Add(doll);
                if (balanced && balance) doll.Send("bodyBalance");
                Logger.LogInfo($"ragdolled {owner.name}");
            }
            if (impulse != Vector3.zero) doll.Push(impulse);
            return doll;
        }

        /// The hips among a skeleton's bones: the one named so, else the thighs' shared parent (Blender rigs name it
        /// "spine").
        static Transform HipsOf(IEnumerable<Transform> bones, Dictionary<string, int> map)
        {
            var list = bones.Where(b => b != null).ToList();
            var hips = list.FirstOrDefault(b => PartOf(b.name, map) == 0);
            if (hips != null) return hips;
            var l = list.FirstOrDefault(b => PartOf(b.name, map) == 1);
            var r = list.FirstOrDefault(b => PartOf(b.name, map) == 4);
            return l != null && r != null && l.parent == r.parent ? l.parent : null;
        }

        Dictionary<string, int> Overrides()
        {
            var parts = Enumerable.Range(0, 21).Select(i => Str((b, n) => nm_part_name(i, b, n))).ToList();
            var map = new Dictionary<string, int>();
            foreach (var pair in boneMap.Split(','))
            {
                var kv = pair.Split('=');
                if (kv.Length != 2) continue;
                var part = kv[1].Trim();
                map[kv[0].Trim()] = part.Equals("None", StringComparison.OrdinalIgnoreCase) ? -1 : parts.IndexOf(part);
            }
            return map;
        }

        internal static int PartOf(string bone, Dictionary<string, int> map) => map.TryGetValue(bone, out int k) ? k : nm_rig_part_for_name(bone);

        internal static void Check(int r)
        {
            if (r < 0) throw new Exception(LastError());
        }
    }

    /// One character on NM: its body fitted to the skeleton under `Hips`, the game's animation and rigidbodies paused.
    class Doll : IDisposable
    {
        static readonly HashSet<string> warned = new HashSet<string>();
        /// Each kind of character's tuned build (nm_build_tune), once its background search is done; kept for the game.
        static readonly Dictionary<string, IntPtr> tunedKinds = new Dictionary<string, IntPtr>();
        /// Worlds ragdolls share, so they push and pile on each other: one per spot (same size, ground within 0.5 m,
        /// within 5 m of where it was made, its level read there), freed with its last ragdoll.
        static readonly List<(IntPtr world, float scale, Vector3 at, int refs, int scene)> shared = new List<(IntPtr, float, Vector3, int, int)>();
        public readonly Transform Hips;
        readonly Transform[] bones;
        internal readonly IntPtr world, agent;
        readonly IntPtr build, rig;
        readonly float[] pose, anim;
        readonly List<Behaviour> paused = new List<Behaviour>();
        readonly List<Rigidbody> frozen = new List<Rigidbody>();
        readonly List<UnityEngine.AI.NavMeshAgent> walkers = new List<UnityEngine.AI.NavMeshAgent>();
        readonly (Vector3 at, Quaternion turn) pinned;
        /// Metres of the game's world per metre of NM's (1 at the character's own size; Size setting). The SDK converts
        /// (nm_build_fit_sized): everything here is the game's units.
        internal readonly float scale = 1f;
        internal static float[] P(Vector3 v) => Nm(v);
        internal static float[] Impulse(Vector3 v) => Nm(v);

        // NM is Z up and right-handed, Unity Y up and left-handed: points swap y and z; rotations (x, y, z, w) become
        // (-x, -z, -y, w), both ways.
        internal static float[] Nm(Vector3 v) => new[] { v.x, v.z, v.y };
        static Vector3 Unity(float[] f, int at) => new Vector3(f[at], f[at + 2], f[at + 1]);
        static Quaternion Turn(float[] f, int at) => new Quaternion(-f[at], -f[at + 2], -f[at + 1], f[at + 3]);

        public Transform Owner { get; }
        public Transform[] Bones => bones;
        /// LyingStill was raised for this rest.
        public bool Told;
        /// Its task takes the Animator's pose each step (Sit, Pose, MuscleRise).
        public bool Fed;
        /// Fell was raised for this fall.
        public bool Down;
        /// CarComing's car it braces against (its index in the cars), else -1; and seconds left on it.
        public int Bracing = -1;
        public float BraceLeft;
        /// EnginePhysics: NM's parts as kinematic Unity rigidbodies (capsules, box feet) where NM has them; null without.
        Rigidbody[] bodies;
        public bool Engine => bodies != null;
        /// Its character or any of its EnginePhysics bodies destroyed by the game.
        public bool Gone => Owner == null || Hips == null || (bodies != null && bodies.Any(b => b == null));
        /// Killed (nm_agent_is_dead): it never gets up by itself.
        public bool Dead => nm_agent_is_dead(agent) == 1;

        public Doll(Transform hips, Transform owner, IntPtr baseBuild, Dictionary<string, int> map, string sizeMode = "Real")
        {
            Hips = hips;
            Owner = owner;
            // Parents before children (GetComponentsInChildren is depth first).
            bones = hips.GetComponentsInChildren<Transform>();
            int n = bones.Length;
            var index = new Dictionary<Transform, int>();
            for (int b = 0; b < n; b++) index[bones[b]] = b;
            var parents = bones.Select(t => t != hips && index.TryGetValue(t.parent, out int p) ? p : -1).ToArray();
            var parts = bones.Select(t => t == hips ? 0 : Plugin.PartOf(t.name, map)).ToArray();
            // The skeleton in its bind pose (the mesh's neutral pose), not the pose it's in now: a body fitted to a
            // mid-stride or arms-out pose comes out bent against its own joint limits, and the physics blows up.
            var bind = Bind(owner, bones, parents, out float grow, out Vector3 bindHip);
            Vector3 At(int b) => bind[b].GetColumn(3);
            var rest = new float[n * 7];
            for (int b = 0; b < n; b++)
            {
                Nm(At(b)).CopyTo(rest, b * 7);
                var q = bind[b].rotation;
                new[] { -q.x, -q.z, -q.y, q.w }.CopyTo(rest, b * 7 + 3);
            }
            // Its 21 joints, missing knees, spine, neck and clavicles filled in, `parts` cleaned (nm_rig_joints).
            var places = new float[n * 3];
            for (int b = 0; b < n; b++) Nm(At(b)).CopyTo(places, b * 3);
            var fit = new float[63];
            // What might make it contort (nm_rig_check), once per kind of character.
            var report = new byte[2048];
            if (nm_rig_check(n, parents, places, parts, report, report.Length) > 0 && warned.Add(Owner.name))
                Debug.LogWarning($"[NM] {Owner.name}'s skeleton: {System.Text.Encoding.UTF8.GetString(report).TrimEnd((char)0)} (BoneMap in the config fixes the map)");
            Plugin.Check(nm_rig_joints(n, parents, places, parts, fit));
            float k = nm_size_scale(baseBuild, fit, NM_SIZE_BASE);
            // DELIRIUM_DUMP=<dir>: each character's 21 fitted joints (NM space, metres), to replay a fit outside the game.
            var dump = Environment.GetEnvironmentVariable("DELIRIUM_DUMP");
            if (!string.IsNullOrEmpty(dump))
                File.WriteAllText(Path.Combine(dump, owner.name + "_" + owner.GetHashCode() + ".txt"),
                    string.Join(" ", fit.Select(f => f.ToString("R", System.Globalization.CultureInfo.InvariantCulture))));
            // ... and its skin as it stands (an .obj per mesh, the mesh's own space), to look at.
            if (!string.IsNullOrEmpty(dump))
                foreach (var r in owner.GetComponentsInChildren<SkinnedMeshRenderer>())
                {
                    var m = new Mesh();
                    r.BakeMesh(m, true);
                    var ci = System.Globalization.CultureInfo.InvariantCulture;
                    File.WriteAllLines(Path.Combine(dump, owner.name + "_" + owner.GetHashCode() + "_" + r.name + ".obj"),
                        m.vertices.Select(v => string.Format(ci, "v {0} {1} {2}", v.x, v.y, v.z))
                         .Concat(Enumerable.Range(0, m.triangles.Length / 3).Select(t => $"f {m.triangles[t * 3] + 1} {m.triangles[t * 3 + 1] + 1} {m.triangles[t * 3 + 2] + 1}")));
                    UnityEngine.Object.Destroy(m);
                }
            if (!string.IsNullOrEmpty(dump))
                File.WriteAllLines(Path.Combine(dump, owner.name + "_" + owner.GetHashCode() + ".bones"),
                    Enumerable.Range(0, n).Select(b => $"{bones[b].name}\t{parents[b]}\t{parts[b]}\t{At(b) - At(0):F3}"));
            // Fitted at the size the Size setting picks: the SDK scales places, pushes and the clock (nm_build_fit_sized).
            build = nm_build_fit_sized(baseBuild, fit, sizeMode == "Homer" ? NM_SIZE_BASE : sizeMode == "Real" ? NM_SIZE_REAL : NM_SIZE_AUTO);
            if (build == IntPtr.Zero) throw new Exception(LastError());
            scale = nm_build_get_scale(build);
            if (scale != 1f) Debug.Log($"[NM] {owner.name}: {k:F2} of a person's leg length, run at a person's (Size {sizeMode})");
            // Its weight from its skin's thickness around its bones, in the bind pose the joints came from; only skins
            // the game left readable (a posed copy comes back at each game's own size and place).
            var verts = new List<float>();
            var vparts = new List<int>();
            foreach (var r in owner.GetComponentsInChildren<SkinnedMeshRenderer>())
            {
                var m = r.sharedMesh;
                if (m == null || !m.isReadable) continue;
                var bw = m.boneWeights;
                var w = r.transform.localToWorldMatrix;
                var mv = m.vertices;
                for (int i = 0; i < mv.Length && i < bw.Length; i++)
                {
                    int bi = bw[i].boneIndex0, b = bi < r.bones.Length && r.bones[bi] != null && index.TryGetValue(r.bones[bi], out int x) ? x : -1;
                    while (b >= 0 && parts[b] < 0) b = parents[b];
                    if (b < 0) continue;
                    verts.AddRange(Nm(bones[0].position + ((Vector3)w.MultiplyPoint3x4(mv[i]) - bindHip) * grow));
                    vparts.Add(parts[b]);
                }
            }
            float was = nm_build_get_weight(build);
            if (!Plugin.Adaptive)
                Debug.Log($"[NM] {owner.name}: {was:F0} kg by its bones (Adaptive off)");
            else if (vparts.Count > 0)
                Debug.Log($"[NM] {owner.name}: {nm_build_weigh_skin(build, fit, verts.ToArray(), vparts.ToArray(), vparts.Count):F0} kg by its skin (fitted {was:F0} kg)");
            else
                Debug.Log($"[NM] {owner.name}: {was:F0} kg by its bones (its skin isn't readable)");
            var shape = Plugin.Shapes.TryGetValue(owner, out var own) ? own : Plugin.Shape;
            if (shape.Name == "Random" && shape.Seed == 0) shape = shape.WithSeed(Environment.TickCount ^ owner.GetHashCode());
            try
            {
                var reshaped = shape.Make(build);
                if (reshaped != IntPtr.Zero)
                {
                    nm_build_destroy(build);
                    build = reshaped;
                }
            }
            catch (Exception e) { Debug.LogWarning($"[NM] {owner.name} body {shape.Name}: {e.Message}"); }
            // Its kind's balance settings: searched once in the background (seconds: it stops the game otherwise), the
            // ones taken before it's done balance on the game's own.
            if (Plugin.Adaptive) lock (tunedKinds)
                if (tunedKinds.TryGetValue(owner.name + shape.Key, out var tuned)) { if (tuned != IntPtr.Zero) nm_build_copy_tune(build, tuned); }
                else
                {
                    tunedKinds[owner.name + shape.Key] = IntPtr.Zero;
                    var (kind, mode, joints) = (owner.name + shape.Key, sizeMode == "Homer" ? NM_SIZE_BASE : sizeMode == "Real" ? NM_SIZE_REAL : NM_SIZE_AUTO, (float[])fit.Clone());
                    System.Threading.ThreadPool.QueueUserWorkItem(_ =>
                    {
                        var t = nm_build_fit_sized(baseBuild, joints, mode);
                        try
                        {
                            var r = t == IntPtr.Zero ? IntPtr.Zero : shape.Make(t);
                            if (r != IntPtr.Zero) { nm_build_destroy(t); t = r; }
                        }
                        catch (Exception) { }
                        int got = t == IntPtr.Zero ? -1 : nm_build_tune(t, 200);
                        lock (tunedKinds) tunedKinds[kind] = t;
                        Debug.Log($"[NM] {kind}: balance tuned, stands through {got} of 3 pushes");
                    });
                }
            // Colliders moved this frame (a character or level just spawned) are where Unity put them only after the next
            // physics step: the ground and level read now would be where they were made.
            Physics.SyncTransforms();
            float ground = Ground(hips, owner);
            var spot = new Vector3(hips.position.x, ground, hips.position.z);
            // Only ragdolls in the same scene share one.
            int sceneId = owner.gameObject.scene.GetHashCode(); // its handle, on every Unity (6.6 made .handle a SceneHandle)
            int near = shared.FindIndex(s => s.scene == sceneId && s.scale == scale && Mathf.Abs(s.at.y - ground) < 0.5f * scale &&
                                             Vector3.Distance(s.at, spot) < 5f * scale);
            if (near >= 0)
            {
                var s = shared[near];
                world = s.world;
                shared[near] = (s.world, s.scale, s.at, s.refs + 1, s.scene);
            }
            else
            {
                world = nm_world_create();
                Plugin.Check(nm_world_set_scale(world, scale));
                shared.Add((world, scale, spot, 1, sceneId));
            }
            // The level around it (walls, stairs, slopes); a floor at its feet when there's none to read.
            // ponytail: the ground under it is always added too, as unreadable meshes (common in shipped games) leave holes;
            // that floor also covers pits next to it, until meshes can be read another way.
            if (near < 0)
            {
                int tris = Environment.GetEnvironmentVariable("DELIRIUM_NOLEVEL") == null ? Level(hips, owner, world) : 0;
                if (!string.IsNullOrEmpty(dump))
                    File.AppendAllText(Path.Combine(dump, "level.txt"), $"{owner.name} {tris} triangles, ground {ground - hips.position.y:F2} below hips\n");
                // The floor of the ground's material under it (ice stays ice past the level's reach).
                byte fm = Surface(GroundCollider(hips, owner));
                if (fm == 0) Plugin.Check(nm_world_add_floor(world, hips.position.x, hips.position.z, ground, 100f * scale));
                else
                {
                    float x = hips.position.x, z = hips.position.z, h = 100f * scale;
                    var v = new[] { x - h, z - h, ground, x + h, z - h, ground, x + h, z + h, ground, x - h, z + h, ground };
                    Plugin.Check(nm_world_add_mesh(world, v, 4, new uint[] { 0, 1, 2, 0, 2, 3 }, 2, new[] { fm, fm }));
                }
            }
            agent = nm_agent_create(build, P(new Vector3(hips.position.x, ground, hips.position.z)), 0f, UnityEngine.Random.value);
            if (agent == IntPtr.Zero) throw new Exception(LastError());
            float strong = Plugin.Strong.TryGetValue(owner, out var ownStrength) ? ownStrength : Plugin.Strength;
            if (strong != 1f) Plugin.Check(nm_agent_set_strength(agent, strong));
            // Arms out early: from 1.3 m (the game's 0.65), predicted 0.45 s ahead (its car set's), every brace it makes.
            if (Plugin.ArmsOutEarly)
            {
                Plugin.Check(nm_agent_set_override(agent, "braceForImpact", "braceDistance", 1.3f));
                Plugin.Check(nm_agent_set_override(agent, "braceForImpact", "targetPredictionTime", 0.45f));
            }
            var mine = Plugin.Overridden.TryGetValue(owner, out var o) ? o : Plugin.AllOverrides.ToList();
            foreach (var (m, p, v) in mine ?? new List<(string, string, float)>())
                if (nm_agent_set_override(agent, m, p, v) < 0) Debug.LogWarning($"[NM] Overrides: {LastError()}");
            rig = nm_rig_create(agent, n, parents, rest, parts);
            if (rig == IntPtr.Zero) throw new Exception(LastError());
            // It starts in the pose its animation has it in now, moving as the character moved: no snap.
            anim = new float[n * 7];
            Plugin.Check(nm_rig_take(rig, agent, Frames(), null, 0f));
            if (Plugin.Recentre) nm_agent_recentre_joint(agent, -1);
            foreach (var (m, v) in Plugin.Holds)
                if (nm_agent_hold(agent, m, v[0], v[1], v[2]) < 0) Debug.LogWarning($"[NM] Hold: {LastError()}");
            foreach (var (m, v) in Plugin.Follows)
                if (nm_agent_follow(agent, m, v[0]) < 0) Debug.LogWarning($"[NM] Follow: {LastError()}");
            var walk = owner.GetComponentInChildren<UnityEngine.AI.NavMeshAgent>();
            var body = owner.GetComponentInChildren<Rigidbody>();
            var cc = owner.GetComponentInChildren<CharacterController>();
            var vel = walk != null && walk.enabled ? walk.velocity : body != null && !body.isKinematic ? body.velocity : cc != null ? cc.velocity : Vector3.zero;
            if (vel != Vector3.zero) Plugin.Check(nm_agent_add_velocity(agent, Nm(vel)));
            pose = new float[n * 4 + 3];
            // The game stops moving this character while NM has it.
            var root = owner;
            // Follow: its animation keeps playing under the ragdoll (Feed), which draws over it (Pose).
            if (Plugin.Follows.Count == 0)
            {
                foreach (var a in root.GetComponentsInChildren<Animator>()) if (a.enabled) { a.enabled = false; paused.Add(a); }
                foreach (var a in root.GetComponentsInChildren<Animation>()) if (a.enabled) { a.enabled = false; paused.Add(a); }
            }
            foreach (var rb in root.GetComponentsInChildren<Rigidbody>()) if (!rb.isKinematic) { rb.isKinematic = true; frozen.Add(rb); }
            // Its AI keeps thinking but can't walk it off: path agents stop moving it and its root stays put (Pose) until
            // it's handed back, where they jump to wherever it got up.
            foreach (var a in root.GetComponentsInChildren<UnityEngine.AI.NavMeshAgent>())
                if (a.updatePosition) { a.updatePosition = a.updateRotation = false; walkers.Add(a); }
            pinned = (owner.position, owner.rotation);
            if (Plugin.EnginePhysics) Build(owner);

        }

        /// Each bone's world matrix in its skinned meshes' bind pose, placed where the character stands; bones no mesh
        /// skins (helpers) keep their current offset from the nearest parent that has one.
        static Matrix4x4[] Bind(Transform owner, Transform[] bones, int[] parents, out float grow, out Vector3 hip)
        {
            var known = new Dictionary<Transform, Matrix4x4>();
            foreach (var r in owner.GetComponentsInChildren<SkinnedMeshRenderer>())
            {
                var poses = r.sharedMesh != null ? r.sharedMesh.bindposes : null;
                if (poses == null) continue;
                for (int i = 0; i < r.bones.Length && i < poses.Length; i++)
                    if (r.bones[i] != null && !known.ContainsKey(r.bones[i]))
                        known[r.bones[i]] = r.transform.localToWorldMatrix * poses[i].inverse;
            }
            var m = new Matrix4x4[bones.Length];
            for (int b = 0; b < bones.Length; b++)
                m[b] = known.TryGetValue(bones[b], out var k) ? k
                     : parents[b] >= 0 ? m[parents[b]] * (bones[parents[b]].worldToLocalMatrix * bones[b].localToWorldMatrix)
                     : bones[b].localToWorldMatrix;
            // The size it's drawn at now (games rescale a skeleton after binding it: the bind pose can be several times
            // too big), with the hips where they are now.
            float now = 0, then = 0;
            for (int b = 1; b < bones.Length; b++)
                if (parents[b] >= 0)
                {
                    now += (bones[b].position - bones[parents[b]].position).magnitude;
                    then += ((Vector3)m[b].GetColumn(3) - (Vector3)m[parents[b]].GetColumn(3)).magnitude;
                }
            grow = then > 1e-4f ? now / then : 1f;
            hip = m[0].GetColumn(3);
            var (g, h) = (grow, hip);
            for (int b = 0; b < bones.Length; b++)
                m[b] = Matrix4x4.TRS(bones[0].position + ((Vector3)m[b].GetColumn(3) - h) * g, m[b].rotation, Vector3.one);
            return m;
        }

        /// The level's static colliders within 15 m, as NM triangles: meshes (readable ones), boxes and terrain.
        /// ponytail: spheres and capsules left out, and moving bodies; the level is read once, when the character's taken.
        static int Level(Transform hips, Transform owner, IntPtr world)
        {
            const float R = 15f;
            var c = hips.position;
            var pts = new List<float>();
            var tris = new List<uint>();
            var mats = new List<byte>();
            byte mat = 0;
            // A mirrored space flips the winding: each triangle's last two corners swap to keep its face outward.
            void Tri(Vector3 a, Vector3 b, Vector3 d)
            {
                uint at = (uint)(pts.Count / 3);
                foreach (var v in new[] { a, d, b }) pts.AddRange(Nm(v));
                tris.AddRange(new[] { at, at + 1, at + 2 });
                mats.Add(mat);
            }
            foreach (var col in Physics.OverlapSphere(c, R))
            {
                if (col.isTrigger || col.transform.IsChildOf(owner) || col.attachedRigidbody != null && !col.attachedRigidbody.isKinematic) continue;
                var t = col.transform;
                mat = Surface(col);
                switch (col)
                {
                    case MeshCollider m when m.sharedMesh != null && m.sharedMesh.isReadable:
                        var v = m.sharedMesh.vertices.Select(t.TransformPoint).ToArray();
                        var ix = m.sharedMesh.triangles;
                        for (int i = 0; i + 2 < ix.Length; i += 3)
                        {
                            Vector3 a = v[ix[i]], b = v[ix[i + 1]], d = v[ix[i + 2]];
                            var mid = (a + b + d) / 3;
                            float size = Mathf.Max((a - mid).magnitude, (b - mid).magnitude, (d - mid).magnitude);
                            if ((mid - c).magnitude < R + size) Tri(a, b, d);
                        }
                        break;
                    case BoxCollider box:
                        // A box around the character is a volume (a room's bounds, a zone), not something to stand on: its
                        // faces would shove the body out through the nearest side.
                        var inside = t.InverseTransformPoint(c) - box.center;
                        if (Mathf.Abs(inside.x) < box.size.x / 2 && Mathf.Abs(inside.y) < box.size.y / 2 && Mathf.Abs(inside.z) < box.size.z / 2)
                        {
                            Debug.Log($"[NM] {owner.name}: skipped box {t.name} around it ({Vector3.Scale(box.size, t.lossyScale)})");
                            break;
                        }
                        var k = new Vector3[8];
                        for (int i = 0; i < 8; i++)
                            k[i] = t.TransformPoint(box.center + Vector3.Scale(box.size / 2, new Vector3((i & 1) * 2 - 1, (i & 2) - 1, (i >> 1 & 2) - 1)));
                        // The 6 faces as corner bits (x 1, y 2, z 4), each wound outward in Unity.
                        int[][] faces = { new[] { 0, 2, 3, 1 }, new[] { 4, 5, 7, 6 }, new[] { 0, 1, 5, 4 }, new[] { 2, 6, 7, 3 }, new[] { 0, 4, 6, 2 }, new[] { 1, 3, 7, 5 } };
                        foreach (var f in faces)
                        {
                            Tri(k[f[0]], k[f[1]], k[f[2]]);
                            Tri(k[f[0]], k[f[2]], k[f[3]]);
                        }
                        break;
                    case TerrainCollider tc when tc.terrainData != null:
                        var td = tc.terrainData;
                        var o = t.position;
                        Vector3 H(float x, float z) => new Vector3(x, o.y + td.GetInterpolatedHeight((x - o.x) / td.size.x, (z - o.z) / td.size.z), z);
                        // A 1 m grid over the square around the character, inside the terrain.
                        for (float x = Mathf.Max(c.x - R, o.x); x < Mathf.Min(c.x + R, o.x + td.size.x - 1); x++)
                            for (float z = Mathf.Max(c.z - R, o.z); z < Mathf.Min(c.z + R, o.z + td.size.z - 1); z++)
                            {
                                Tri(H(x, z), H(x, z + 1), H(x + 1, z + 1));
                                Tri(H(x, z), H(x + 1, z + 1), H(x + 1, z));
                            }
                        break;
                }
            }
            if (tris.Count == 0) return 0;
            Plugin.Check(nm_world_add_mesh(world, pts.ToArray(), pts.Count / 3, tris.ToArray(), tris.Count / 3, mats.ToArray()));
            return tris.Count / 3;
        }

        /// The ground under the hips, not counting the character's own colliders; its lowest bone without one.
        static readonly List<(float, float)> surfaces = new List<(float, float)>();
        /// A collider's physics material as a surface row: 0 Unity's default (friction 0.6, no bounce), else one of rows 200
        /// up, made for it the first time (nm_set_surface: ice's friction 0.05 slides).
        static byte Surface(Collider col)
        {
            var m = col != null ? col.sharedMaterial : null;
            if (m == null || m.dynamicFriction == 0.6f && m.bounciness == 0f) return 0;
            var key = (m.dynamicFriction / 0.6f, m.bounciness);
            int k = surfaces.IndexOf(key);
            if (k < 0)
            {
                surfaces.Add(key);
                k = surfaces.Count - 1;
                // ponytail: 56 distinct materials; past that they share the last row.
                if (k < 56) nm_set_surface(200 + k, key.Item1, Mathf.Clamp01(key.Item2));
            }
            return (byte)(200 + Math.Min(k, 55));
        }

        /// The ground under the hips (its collider), null when none.
        static Collider GroundCollider(Transform hips, Transform owner) =>
            Physics.RaycastAll(hips.position, Vector3.down, 5f).Where(h => !h.transform.IsChildOf(owner)).OrderBy(h => h.distance).FirstOrDefault().collider;

        static float Ground(Transform hips, Transform owner)
        {
            var root = owner;
            var hit = Physics.RaycastAll(hips.position, Vector3.down, 5f).Where(h => !h.transform.IsChildOf(root)).OrderBy(h => h.distance).FirstOrDefault();
            return hit.collider != null ? hit.point.y : hips.GetComponentsInChildren<Transform>().Min(t => t.position.y);
        }

        public void Send(string message)
        {
            Plugin.Check(nm_msg_begin(agent, message));
            Plugin.Check(nm_msg_set_bool(agent, "start", 1));
            Plugin.Check(nm_msg_send(agent, world));
        }

        public void Shoot(Vector3 point, Vector3 dir, float force, string set, float health)
        {
            int part = nm_agent_nearest_part(agent, P(point));
            Plugin.Check(nm_agent_apply_push(agent, P(point), Impulse(dir.normalized * force * Plugin.ShotForce)));
            Plugin.Check(nm_agent_task_shot(agent, world, set, Math.Max(part, 0), P(point), Nm(dir.normalized), health));
        }

        public void Push(Vector3 impulse)
        {
            // At the chest's height, through the hips.
            Plugin.Check(nm_agent_apply_push(agent, P(Hips.position + Vector3.up * 0.35f * scale), Impulse(impulse)));
        }

        /// EnginePhysics: the bodies to where NM has the parts (moved, so they push what's in the way).
        public void Follow()
        {
            float[] p = new float[3], q = new float[4];
            for (int k = 0; k < bodies.Length; k++)
            {
                Plugin.Check(nm_agent_get_part_transform(agent, k, p, q));
                bodies[k].MovePosition(Unity(p, 0));
                bodies[k].MoveRotation(Turn(q, 0));
            }
        }

        /// EnginePhysics: NM's parts as kinematic rigidbodies (NM's mass, capsules and box feet) where NM has them: NM's
        /// own physics moves him (Unity's joints and drives couldn't hold its short or heavy bodies up: PhysX's joints
        /// lack the original engine's armature), the bodies follow (Follow) and push Unity's rigidbodies; loose ones near him go to NM as
        /// movers (MoversNear). They don't meet the character's own colliders.
        void Build(Transform owner)
        {
            int n = nm_agent_part_count(agent);
            var root = new GameObject($"{owner.name} ragdoll (Delirium)").transform;
            bodies = new Rigidbody[n];
            float[] f3 = new float[3], q4 = new float[4], g3 = new float[3], box = new float[3], inertia = new float[3], mass = new float[1];
            for (int k = 0; k < n; k++)
            {
                Plugin.Check(nm_agent_get_part_transform(agent, k, f3, q4));
                var go = new GameObject("part " + k);
                go.transform.SetPositionAndRotation(Unity(f3, 0), Turn(q4, 0));
                go.transform.SetParent(root, true);
                Plugin.Check(nm_build_part_info(build, k, null, g3, q4, out int _));
                int kind = nm_agent_get_part_body(agent, k, mass, inertia, box);
                if (kind == 2) go.AddComponent<BoxCollider>().size = new Vector3(box[0], box[2], box[1]) * 2;
                else
                {
                    var c = go.AddComponent<CapsuleCollider>();
                    // NM's capsules run along their part's Y, which is the converted frame's Z.
                    (c.direction, c.radius, c.height) = (2, g3[0] * scale, 2 * (q4[0] + g3[0]) * scale);
                }
                var rb = go.AddComponent<Rigidbody>();
                rb.mass = mass[0];
                rb.isKinematic = true;
                rb.interpolation = RigidbodyInterpolation.Interpolate;
                rb.collisionDetectionMode = CollisionDetectionMode.ContinuousSpeculative;
                bodies[k] = rb;
            }
            Physics.SyncTransforms();
            var own = owner.GetComponentsInChildren<Collider>();
            foreach (var b in bodies) foreach (var o in own) Physics.IgnoreCollision(b.GetComponent<Collider>(), o);
        }

        /// Seconds lying still (nm_agent_get_lying); `facing` the way to face once up, flat.
        public float Lying(out bool faceUp, out Vector3 facing)
        {
            var head = new float[3];
            float still = nm_agent_get_lying(agent, head, out int up);
            faceUp = up == 1;
            facing = Unity(head, 0) * (faceUp ? -1 : 1);
            return still;
        }

        public float Floor() => Ground(Hips, Owner);

        /// Where a shot along `aim` from `from` strikes the body (nm_agent_aim_hit).
        public Vector3 AimHit(Vector3 from, Vector3 aim)
        {
            var at = new float[3];
            nm_agent_aim_hit(agent, P(from), Nm(aim.normalized), at);
            return Unity(at, 0);
        }

        /// Calls the SDK on this one's body (the plugin's own helpers).
        internal void Run(Func<Doll, int> f) => Plugin.Check(f(this));

        public Vector3 Part(int i)
        {
            var p = new float[3];
            nm_agent_get_part_transform(agent, i, p, null);
            return Unity(p, 0);
        }

        /// Each bone's world frame now, NM's way (nm_rig_take's `now`).
        float[] Frames()
        {
            for (int b = 0; b < bones.Length; b++)
            {
                Nm(bones[b].position).CopyTo(anim, b * 7);
                var q = bones[b].rotation;
                new[] { -q.x, -q.z, -q.y, q.w }.CopyTo(anim, b * 7 + 3);
            }
            return anim;
        }

        /// Follow: the animation's pose this frame (the Animator ran just before LateUpdate) for its joints to aim at.
        public void Feed()
        {
            if (Plugin.Follows.Count > 0 || Fed) Plugin.Check(nm_rig_incoming(rig, agent, Frames()));
        }

        public void Pose()
        {
            Owner.SetPositionAndRotation(pinned.at, pinned.turn);
            int hips = nm_rig_pose(rig, agent, pose);
            // The hips are the rig's root: their turn is a world one, every other bone's is under its parent.
            bones[0].rotation = Turn(pose, 0);
            for (int b = 1; b < bones.Length; b++) bones[b].localRotation = Turn(pose, b * 4);
            bones[hips].position = Unity(pose, bones.Length * 4);
        }

        /// Frees NM's body and gives the character back to the game; returns its Animators, running again.
        public void Dispose() => Release();

        public List<Animator> Release()
        {
            foreach (var b in paused) if (b != null) b.enabled = true;
            foreach (var rb in frozen) if (rb != null) rb.isKinematic = false;
            foreach (var a in walkers)
                if (a != null) { if (a.isActiveAndEnabled && Owner != null) a.Warp(Owner.position); a.updatePosition = a.updateRotation = true; }
            if (bodies != null && bodies[0] != null) UnityEngine.Object.Destroy(bodies[0].transform.parent.gameObject);
            nm_rig_destroy(rig);
            nm_agent_destroy(agent);
            nm_build_destroy(build);
            int i = shared.FindIndex(s => s.world == world);
            if (i >= 0 && shared[i].refs > 1) shared[i] = (world, shared[i].scale, shared[i].at, shared[i].refs - 1, shared[i].scene);
            else
            {
                if (i >= 0) shared.RemoveAt(i);
                nm_world_destroy(world);
            }
            // Its running ones (Follow's never stopped): the get-up trigger goes to them. None when the game destroyed it.
            return Owner == null ? new List<Animator>() : Owner.GetComponentsInChildren<Animator>().Where(a => a.enabled).ToList();
        }
    }
}

namespace Delirium.Ragdoll
{
    /// The Body setting (the Playroom's bodies), for a character and its background tune alike.
    readonly struct Body
    {
        public static readonly string[] Names = { "Fitted", "Small", "Average", "Large", "Lanky", "Fat", "Stocky", "Waddler", "Guard", "Brute", "Brawler", "Bruiser", "Random", "Custom" };
        public readonly string Name, Custom;
        public readonly int Seed;
        public readonly float Variance;
        public readonly bool Symmetric;

        public Body(string name, int seed, float variance, bool symmetric, string custom) =>
            (Name, Seed, Variance, Symmetric, Custom) = (string.IsNullOrEmpty(name) ? "Fitted" : name, seed, variance, symmetric, custom);

        public Body WithSeed(int seed) => new Body(Name, seed, Variance, Symmetric, Custom);

        /// The fitted build `b` reshaped (a new build); zero: Fitted, keep it. Throws for a bad body.
        public IntPtr Make(IntPtr b)
        {
            IntPtr r;
            switch (Name)
            {
                case "Fitted": return IntPtr.Zero;
                case "Random": r = nm_build_random(b, Seed, Variance, Symmetric ? 1 : 0); break;
                case "Custom":
                    var v = Custom.Split(',').Select(x => float.TryParse(x.Trim(), System.Globalization.NumberStyles.Float,
                        System.Globalization.CultureInfo.InvariantCulture, out var f) ? f : float.NaN).ToArray();
                    if (v.Length != 8 || v.Any(float.IsNaN)) throw new Exception("CustomBody needs 8 numbers");
                    r = nm_build_sized(b, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
                    break;
                default: r = nm_build_named(b, Name); break;
            }
            if (r == IntPtr.Zero) throw new Exception(LastError());
            return r;
        }

        /// Kinds tuned apart: the body's own.
        public string Key => Name == "Fitted" ? "" : Name == "Random" ? $" Random {Seed} {Variance} {Symmetric}" : Name == "Custom" ? " Custom " + Custom : " " + Name;

        /// "behaviour.param=value, ...": each one (bad ones in the log).
        public static List<(string, string, float)> ParseOverrides(string s)
        {
            var o = new List<(string, string, float)>();
            foreach (var item in s.Split(',').Select(x => x.Trim()).Where(x => x != ""))
            {
                var kv = item.Split('=');
                var mp = kv[0].Trim().Split('.');
                if (kv.Length == 2 && mp.Length == 2 && float.TryParse(kv[1].Trim(), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var v))
                    o.Add((mp[0], mp[1], v));
                else Debug.LogWarning($"[NM] Overrides: \"{item}\" isn't behaviour.param=value");
            }
            return o;
        }
    }
}
