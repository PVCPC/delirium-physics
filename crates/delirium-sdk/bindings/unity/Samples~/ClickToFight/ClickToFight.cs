// Click to fight: put this on your camera (with DeliriumRagdolls in the scene) and aim at a character.
// Left click shoots (he staggers or falls; the fourth shot kills), F punches (within 3 m), G throws a blast where you aim.
// Downed characters get up after 3 s. Characters are any object with an Animator; no colliders needed.
using System.Collections.Generic;
using UnityEngine;

namespace Delirium.Ragdoll.Samples
{
    public class ClickToFight : MonoBehaviour
    {
        public float shotDamage = 40f, punch = 30f, blastRadius = 5f;
        readonly Dictionary<Transform, float> health = new Dictionary<Transform, float>();

        void Start()
        {
            var nm = DeliriumRagdolls.Instance;
            if (nm == null) { Debug.LogWarning("ClickToFight: add DeliriumRagdolls to the scene"); enabled = false; return; }
            if (nm.getUpAfter <= 0) nm.getUpAfter = 3f;
        }

        void Update()
        {
            if (Input.GetMouseButtonDown(0)) Fire(transform.position, transform.forward);
            if (Input.GetKeyDown(KeyCode.F)) Punch(transform.position, transform.forward);
            if (Input.GetKeyDown(KeyCode.G)) Blast(transform.position, transform.forward);
        }

        /// Shoots the character nearest the aim; its health drops by shotDamage (200 to start).
        public bool Fire(Vector3 from, Vector3 aim)
        {
            var who = Aimed(from, aim, out _);
            if (who == null) return false;
            float hp = health.TryGetValue(who, out var h) ? h : 200f;
            health[who] = hp = Mathf.Max(0, hp - shotDamage);
            return DeliriumRagdolls.Instance.ShootAlong(who, from, aim, 40f, "POWERFUL_WEAPON", hp);
        }

        /// Punches the character nearest the aim, when it's within 3 m.
        public bool Punch(Vector3 from, Vector3 aim)
        {
            var who = Aimed(from, aim, out var chest);
            if (who == null || Vector3.Distance(from, chest) > 3f) return false;
            return DeliriumRagdolls.Instance.MeleeHit(who, chest, aim.normalized * punch, from);
        }

        /// A blast where the aim meets the level (10 m ahead if it meets nothing), throwing everyone in range.
        public int Blast(Vector3 from, Vector3 aim)
        {
            var at = Physics.Raycast(from, aim, out var hit, 100f) ? hit.point : from + aim.normalized * 10f;
            int n = 0;
            foreach (var a in FindObjectsOfType<Animator>())
                if (Vector3.Distance(Chest(a.transform), at) < blastRadius && DeliriumRagdolls.Instance.Explode(a.transform, at, blastRadius)) n++;
            return n;
        }

        /// The character whose chest is nearest the aim line (within 1.5 m of it).
        Transform Aimed(Vector3 from, Vector3 aim, out Vector3 chest)
        {
            Transform best = null;
            float bestOff = 1.5f;
            chest = default;
            aim.Normalize();
            foreach (var a in FindObjectsOfType<Animator>())
            {
                var c = Chest(a.transform);
                var to = c - from;
                float off = Vector3.Cross(aim, to).magnitude;
                if (Vector3.Dot(to, aim) > 0 && off < bestOff) (best, bestOff, chest) = (a.transform, off, c);
            }
            return best;
        }

        // Its head while ragdolled (the body's moved off its root), else a metre above its root.
        static Vector3 Chest(Transform who) => DeliriumRagdolls.Instance.Head(who) ?? who.position + Vector3.up;
    }
}
