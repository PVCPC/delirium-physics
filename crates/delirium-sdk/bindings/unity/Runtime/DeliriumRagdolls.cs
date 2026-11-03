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
    [BepInPlugin("delirium_physics.ragdoll", "NM Ragdoll", "0.6.0")]
    public class Plugin : BaseUnityPlugin
    {
        string gameExe, xrt, buildFile, taskParams, boneMap, getUpFront, getUpBack, sizeMode;
        KeyCode ragdollKey, releaseKey;
        float push, getUpAfter, getUpBlend;
        bool balance;
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
        [Tooltip("A shot whose nearest body part is the head kills.")]
        public bool headshotKills = true;
        [Tooltip("Auto, Real (the character's own size) or Homer (a person-sized body, the world scaled to it).")]
        public string sizeMode = "Auto";
        [Header("Force")]
        public float shotForce = 1f, meleeForce = 1f, explosionForce = 1f;
        [Tooltip("ShootWeapon's table: name=set:push:damage, comma separated.")]
        public string weaponTable = "pistol=PISTOL_WEAPON:30:25, smg=SMG_WEAPON:25:15, rifle=POWERFUL_WEAPON:45:35, shotgun=POWERFUL_WEAPON:90:80";
        [Header("Body")]
        [Tooltip("Fit each kind of character to its own shape (weight and balance).")]
        public bool adaptive = true;
        [Tooltip("Every body's muscles times this.")]
        public float strength = 1f;

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
        /// The Weapons setting: name -> taskparams set, bullet push (N s), health it takes.
        readonly Dictionary<string, (string set, float force, float damage)> weapons = new Dictionary<string, (string, float, float)>(StringComparer.OrdinalIgnoreCase);
        IntPtr build;
        readonly List<Doll> dolls = new List<Doll>();
        readonly List<Blend> blends = new List<Blend>();

        /// A ragdoll has lain still for half a second: (owner, lying face up, the way it should face once up: its
        /// feet's side when on its back, its head's when on its front; flat). Start your own get-up animation and call
        /// GetUp(owner), or leave it lying.
        public event Action<Transform, bool, Vector3> LyingStill;
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
            balance = Config.Bind("Feel", "Balance", true, "Ragdolls try to keep their feet (off: they go limp).").Value;
            ShotForce = Config.Bind("Force", "Shot", 1f, "Every bullet's push times this (1: as the mod or Weapons table gives it).").Value;
            MeleeForce = Config.Bind("Force", "Melee", 1f, "Every punch's, kick's and melee weapon's push times this.").Value;
            ExplosionForce = Config.Bind("Force", "Explosion", 1f, "Every explosion's throw speed times this.").Value;
            var weaponTable = Config.Bind("Force", "Weapons", "pistol=PISTOL_WEAPON:30:25, smg=SMG_WEAPON:25:15, rifle=POWERFUL_WEAPON:45:35, shotgun=POWERFUL_WEAPON:90:80",
                "Weapons for ShootWeapon, name=set:push:damage, comma separated. set: the reaction (PISTOL_WEAPON, SMG_WEAPON, POWERFUL_WEAPON, * the defaults); " +
                "push: the bullet's, N s (40 is a solid hit); damage: the health it takes (a ped has 100-200; under 100 left goes down, under 50 is a kill).").Value;
            Adaptive = Config.Bind("Body", "Adaptive", true, "Fit each kind of character to its own shape: its weight from its skin and its " +
                "balance settings searched once in the background (a few seconds). Off: the game man's weight share and balance on its bones.").Value;
            Strength = Config.Bind("Body", "Strength", 1f, new ConfigDescription("Every body's muscles times this (1: as made). 1.5-2 for tough " +
                "enemies or a movie giant: at 2 a person stands a push of 80 N s instead of 60.", new AcceptableValueRange<float>(0.1f, 10f))).Value;
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
            foreach (var w in weaponTable.Split(','))
            {
                var kv = w.Split('=');
                var v = kv.Length == 2 ? kv[1].Split(':') : new string[0];
                if (v.Length == 3 && float.TryParse(v[1], System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var f)
                    && float.TryParse(v[2], System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var dmg))
                    weapons[kv[0].Trim()] = (v[0].Trim(), f, dmg);
                else if (w.Trim() != "") Logger.LogWarning($"Weapons: can't read \"{w.Trim()}\" (name=set:push:damage)");
            }
            string Find(string p) => p == "" || Path.IsPathRooted(p) ? p : Path.Combine(dir, p);
            try
            {
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

        void Update()
        {
            if (Pressed(releaseKey))
            {
                foreach (var d in dolls) d.Dispose();
                dolls.Clear();
            }
            if (Pressed(ragdollKey)) Grab();
            // NM steps at 60 Hz whatever the game's own rate; at most 4 a frame so a hitch can't snowball.
            clock = Mathf.Min(clock + Time.deltaTime, 4 / 60f);
            for (; clock >= 1 / 60f; clock -= 1 / 60f)
                foreach (var d in dolls) d.Step(1 / 60f);
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
            foreach (var d in dolls) d.Pose();
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
            if (!weapons.TryGetValue(weapon, out var w))
            {
                Logger.LogWarning($"ShootWeapon: no weapon \"{weapon}\" in the Weapons setting; shot with the defaults, no damage");
                w = ("*", 40f, 0f);
            }
            float left = Math.Max(health - w.damage, 0f);
            return ShootAlong(owner, from, aim, w.force, w.set, left) ? left : -1f;
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

        /// Goes limp and stays down (a kill or a knock-out): it never gets up by itself.
        public bool Kill(Transform owner)
        {
            var d = Doll(owner, Vector3.zero, false);
            if (d == null) return false;
            d.Run(x => nm_agent_set_dead(x.agent, 1));
            d.Run(x => nm_agent_task_relax(x.agent, -1f, -1f));
            return true;
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
        static readonly List<(IntPtr world, float scale, Vector3 at, int refs)> shared = new List<(IntPtr, float, Vector3, int)>();
        public readonly Transform Hips;
        readonly Transform[] bones;
        internal readonly IntPtr world, agent;
        readonly IntPtr build, rig;
        readonly float[] pose;
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
                File.WriteAllText(Path.Combine(dump, owner.name + "_" + owner.GetInstanceID() + ".txt"),
                    string.Join(" ", fit.Select(f => f.ToString("R", System.Globalization.CultureInfo.InvariantCulture))));
            // ... and its skin as it stands (an .obj per mesh, the mesh's own space), to look at.
            if (!string.IsNullOrEmpty(dump))
                foreach (var r in owner.GetComponentsInChildren<SkinnedMeshRenderer>())
                {
                    var m = new Mesh();
                    r.BakeMesh(m, true);
                    var ci = System.Globalization.CultureInfo.InvariantCulture;
                    File.WriteAllLines(Path.Combine(dump, owner.name + "_" + owner.GetInstanceID() + "_" + r.name + ".obj"),
                        m.vertices.Select(v => string.Format(ci, "v {0} {1} {2}", v.x, v.y, v.z))
                         .Concat(Enumerable.Range(0, m.triangles.Length / 3).Select(t => $"f {m.triangles[t * 3] + 1} {m.triangles[t * 3 + 1] + 1} {m.triangles[t * 3 + 2] + 1}")));
                    UnityEngine.Object.Destroy(m);
                }
            if (!string.IsNullOrEmpty(dump))
                File.WriteAllLines(Path.Combine(dump, owner.name + "_" + owner.GetInstanceID() + ".bones"),
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
            // Its kind's balance settings: searched once in the background (seconds: it stops the game otherwise), the
            // ones taken before it's done balance on the game's own.
            if (Plugin.Adaptive) lock (tunedKinds)
                if (tunedKinds.TryGetValue(owner.name, out var tuned)) { if (tuned != IntPtr.Zero) nm_build_copy_tune(build, tuned); }
                else
                {
                    tunedKinds[owner.name] = IntPtr.Zero;
                    var (kind, mode, joints) = (owner.name, sizeMode == "Homer" ? NM_SIZE_BASE : sizeMode == "Real" ? NM_SIZE_REAL : NM_SIZE_AUTO, (float[])fit.Clone());
                    System.Threading.ThreadPool.QueueUserWorkItem(_ =>
                    {
                        var t = nm_build_fit_sized(baseBuild, joints, mode);
                        int got = t == IntPtr.Zero ? -1 : nm_build_tune(t, 200);
                        lock (tunedKinds) tunedKinds[kind] = t;
                        Debug.Log($"[NM] {kind}: balance tuned, stands through {got} of 3 pushes");
                    });
                }
            float ground = Ground(hips, owner);
            var spot = new Vector3(hips.position.x, ground, hips.position.z);
            int near = shared.FindIndex(s => s.scale == scale && Mathf.Abs(s.at.y - ground) < 0.5f * scale &&
                                             Vector3.Distance(s.at, spot) < 5f * scale);
            if (near >= 0)
            {
                var s = shared[near];
                world = s.world;
                shared[near] = (s.world, s.scale, s.at, s.refs + 1);
            }
            else
            {
                world = nm_world_create();
                Plugin.Check(nm_world_set_scale(world, scale));
                shared.Add((world, scale, spot, 1));
            }
            // The level around it (walls, stairs, slopes); a floor at its feet when there's none to read.
            // ponytail: the ground under it is always added too, as unreadable meshes (common in shipped games) leave holes;
            // that floor also covers pits next to it, until meshes can be read another way.
            if (near < 0)
            {
                int tris = Environment.GetEnvironmentVariable("DELIRIUM_NOLEVEL") == null ? Level(hips, owner, world) : 0;
                if (!string.IsNullOrEmpty(dump))
                    File.AppendAllText(Path.Combine(dump, "level.txt"), $"{owner.name} {tris} triangles, ground {ground - hips.position.y:F2} below hips\n");
                Plugin.Check(nm_world_add_floor(world, hips.position.x, hips.position.z, ground, 100f * scale));
            }
            agent = nm_agent_create(build, P(new Vector3(hips.position.x, ground, hips.position.z)), 0f, UnityEngine.Random.value);
            if (agent == IntPtr.Zero) throw new Exception(LastError());
            if (Plugin.Strength != 1f) Plugin.Check(nm_agent_set_strength(agent, Plugin.Strength));
            rig = nm_rig_create(agent, n, parents, rest, parts);
            if (rig == IntPtr.Zero) throw new Exception(LastError());
            // It starts in the pose its animation has it in now, moving as the character moved: no snap.
            var now = new float[n * 7];
            for (int b = 0; b < n; b++)
            {
                Nm(bones[b].position).CopyTo(now, b * 7);
                var q = bones[b].rotation;
                new[] { -q.x, -q.z, -q.y, q.w }.CopyTo(now, b * 7 + 3);
            }
            Plugin.Check(nm_rig_take(rig, agent, now, null, 0f));
            var walk = owner.GetComponentInChildren<UnityEngine.AI.NavMeshAgent>();
            var body = owner.GetComponentInChildren<Rigidbody>();
            var cc = owner.GetComponentInChildren<CharacterController>();
            var vel = walk != null && walk.enabled ? walk.velocity : body != null && !body.isKinematic ? body.velocity : cc != null ? cc.velocity : Vector3.zero;
            if (vel != Vector3.zero) Plugin.Check(nm_agent_add_velocity(agent, Nm(vel)));
            pose = new float[n * 4 + 3];
            // The game stops moving this character while NM has it.
            var root = owner;
            foreach (var a in root.GetComponentsInChildren<Animator>()) if (a.enabled) { a.enabled = false; paused.Add(a); }
            foreach (var a in root.GetComponentsInChildren<Animation>()) if (a.enabled) { a.enabled = false; paused.Add(a); }
            foreach (var rb in root.GetComponentsInChildren<Rigidbody>()) if (!rb.isKinematic) { rb.isKinematic = true; frozen.Add(rb); }
            // Its AI keeps thinking but can't walk it off: path agents stop moving it and its root stays put (Pose) until
            // it's handed back, where they jump to wherever it got up.
            foreach (var a in root.GetComponentsInChildren<UnityEngine.AI.NavMeshAgent>())
                if (a.updatePosition) { a.updatePosition = a.updateRotation = false; walkers.Add(a); }
            pinned = (owner.position, owner.rotation);

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
            // A mirrored space flips the winding: each triangle's last two corners swap to keep its face outward.
            void Tri(Vector3 a, Vector3 b, Vector3 d)
            {
                uint at = (uint)(pts.Count / 3);
                foreach (var v in new[] { a, d, b }) pts.AddRange(Nm(v));
                tris.AddRange(new[] { at, at + 1, at + 2 });
            }
            foreach (var col in Physics.OverlapSphere(c, R))
            {
                if (col.isTrigger || col.transform.IsChildOf(owner) || col.attachedRigidbody != null && !col.attachedRigidbody.isKinematic) continue;
                var t = col.transform;
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
            Plugin.Check(nm_world_add_mesh(world, pts.ToArray(), pts.Count / 3, tris.ToArray(), tris.Count / 3, null));
            return tris.Count / 3;
        }

        /// The ground under the hips, not counting the character's own colliders; its lowest bone without one.
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

        public void Step(float dt) => Plugin.Check(nm_agent_step(agent, world, dt));

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
                if (a != null) { if (a.isActiveAndEnabled) a.Warp(Owner.position); a.updatePosition = a.updateRotation = true; }
            nm_rig_destroy(rig);
            nm_agent_destroy(agent);
            nm_build_destroy(build);
            int i = shared.FindIndex(s => s.world == world);
            if (i >= 0 && shared[i].refs > 1) shared[i] = (world, shared[i].scale, shared[i].at, shared[i].refs - 1);
            else
            {
                if (i >= 0) shared.RemoveAt(i);
                nm_world_destroy(world);
            }
            return paused.OfType<Animator>().Where(a => a != null).ToList();
        }
    }
}
