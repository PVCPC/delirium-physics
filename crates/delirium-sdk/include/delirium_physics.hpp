// delirium_physics.hpp: C++ over the C API (delirium_physics.h): owning handles and exceptions. Header-only.
#pragma once
#include "delirium_physics.h"
#include <array>
#include <memory>
#include <stdexcept>
#include <string>
#include <tuple>
#include <vector>

namespace nm {

using Vec3 = std::array<float, 3>;
using Quat = std::array<float, 4>;

struct Error : std::runtime_error {
    int code;
    Error(int c, const std::string& w) : std::runtime_error(w), code(c) {}
};

inline std::string last_error() {
    std::string s(nm_last_error(nullptr, 0), '\0');
    nm_last_error(s.data(), (int32_t)s.size());
    s.pop_back();
    return s;
}

inline int check(int r) {
    if (r < 0) throw Error(r, last_error());
    return r;
}

/// A buffer call's string (names, docs, errors).
template <class F> std::string text(F f) {
    std::string s(f(nullptr, 0), '\0');
    f(s.data(), (int32_t)s.size());
    s.pop_back();
    return s;
}

inline void unlock(const std::string& game_exe) { check(nm_unlock(game_exe.c_str())); }
inline void init(const std::string& xrt) { check(nm_init(xrt.c_str())); }
inline void load_task_params(const std::string& path) { check(nm_load_task_params(path.c_str())); }
/// Keep Build::tune results in a text file; a body tuned before gets its settings from there at once.
inline void tune_file(const std::string& path) { nm_tune_file(path.c_str()); }
/// Surface row `material` (a mesh triangle's byte) takes this friction and elasticity for every agent; ice ~0.05.
inline void set_surface(int material, float friction, float elasticity) { check(nm_set_surface(material, friction, elasticity)); }

inline int version() { return nm_version(); }
/// An SDK error (a panic) writes delirium_physics-crash.txt in `dir` ("": the working folder) before the host stops.
inline void crash_report(const std::string& dir = "") { check(nm_crash_report(dir.empty() ? nullptr : dir.c_str())); }
/// nm_init from the character file's bytes in memory.
inline void init_memory(const std::vector<uint8_t>& xrt) { check(nm_init_memory(xrt.data(), (int32_t)xrt.size())); }
/// A character's JSON (its body, joints and muscles, nm_character_from_xrt's), or a file of it, in place of nm_init.
inline void init_character(const std::string& json) { check(nm_init_character(json.c_str())); }
inline void init_character_file(const std::string& path) { check(nm_init_character_file(path.c_str())); }
/// The character file `xrt` and build `build` as one character JSON.
inline std::string character_from_xrt(const std::string& xrt, const std::string& build) {
    return text([&](char* b, int32_t n) { return check(nm_character_from_xrt(xrt.c_str(), build.c_str(), b, n)); });
}
inline std::string part_name(int i) { return text([&](char* b, int32_t n) { return check(nm_part_name(i, b, n)); }); }
/// The game's step limit on bodyBalance's steps (on as the game; the Playroom's default is off).
inline void set_step_limit(bool on) { nm_set_step_limit(on); }
/// Off: burning marks him burning without the OnFire reaction.
inline void set_fire_reaction(bool on) { nm_set_fire_reaction(on); }
inline void set_headshot_kills(bool on) { nm_set_headshot_kills(on); }
/// A closed mesh's weight at a body's density (kg).
inline float mesh_weight(const std::vector<Vec3>& v, const std::vector<std::array<uint32_t, 3>>& t) {
    return nm_mesh_weight(v[0].data(), (int32_t)v.size(), t[0].data(), (int32_t)t.size());
}
/// A call that writes lines into a buffer and returns how many: the lines ("" none).
template <class F> std::string lines(F f) {
    std::string s(4096, '\0');
    check(f(s.data(), (int32_t)s.size()));
    s.resize(s.find('\0'));
    return s;
}
/// Why a skeleton might contort (nm_rig_joints' inputs): one line per problem, "" when it should fit.
inline std::string rig_check(const std::vector<int32_t>& parents, const std::vector<std::array<float, 7>>& rest, const std::vector<int32_t>& map) {
    return lines([&](char* b, int32_t n) { return nm_rig_check((int32_t)parents.size(), parents.data(), rest[0].data(), map.data(), b, n); });
}

/// The message list for pickers: NM's messages, their params, kinds (NM_KIND_*), docs and defaults.
namespace messages {
inline int count() { return nm_message_count(); }
inline int find(const char* name) { return nm_message_find(name); }
inline std::string name(int m) { return text([&](char* b, int32_t n) { return check(nm_message_name(m, b, n)); }); }
inline int param_count(int m) { return check(nm_message_param_count(m)); }
inline int param_find(int m, const char* name) { return nm_message_param_find(m, name); }
inline std::string param_name(int m, int p) { return text([&](char* b, int32_t n) { return check(nm_message_param_name(m, p, b, n)); }); }
inline int param_kind(int m, int p) { return check(nm_message_param_kind(m, p)); }
/// Whether the port acts on it.
inline bool param_used(int m, int p) { return check(nm_message_param_used(m, p)) == 1; }
inline std::string param_doc(int m, int p) { return text([&](char* b, int32_t n) { return check(nm_message_param_doc(m, p, b, n)); }); }
/// Its default (up to 3 floats: a vector's); returns how many.
inline int param_default(int m, int p, float* out) { return check(nm_message_param_default(m, p, out)); }
}

/// The weapons table (nm_weapons_*): the default four, your text, or the game's weaponinfo.xml.
struct Weapon {
    std::string name, set;
    /// push, damage, pellets, between, clip, accuracy, range
    std::array<float, 7> v;
};
namespace weapons {
/// "name=SET:push:damage[:pellets...], ..." (empty: the default four); returns how many.
inline int parse(const std::string& t = "") { return check(nm_weapons_parse(t.empty() ? nullptr : t.c_str())); }
inline int load_info(const std::string& path) { return check(nm_weapons_load_info(path.c_str())); }
inline int count() { return nm_weapon_count(); }
inline int find(const char* name) { return nm_weapon_find(name); }
inline Weapon get(int i) {
    Weapon w;
    char n[64], s[64];
    check(nm_weapon_get(i, n, 64, s, 64, w.v.data()));
    w.name = n; w.set = s;
    return w;
}
/// A shotgun's pellets about `dir`: the aim then `pellets` more; `seed` moves on.
inline std::vector<Vec3> shotgun_fan(const Vec3& dir, float accuracy, float range, int pellets, int32_t& seed) {
    std::vector<Vec3> d(pellets + 1);
    d.resize(check(nm_shotgun_fan(dir.data(), accuracy, range, pellets, &seed, d[0].data())));
    return d;
}
}

class World {
    std::unique_ptr<NmWorld, decltype(&nm_world_destroy)> p{nm_world_create(), nm_world_destroy};
public:
    NmWorld* get() const { return p.get(); }
    /// Your units at a sized build's scale (Build::scale): set before adding the level.
    void set_scale(float s) { check(nm_world_set_scale(get(), s)); }
    void add_floor(float x, float y, float z, float half) { check(nm_world_add_floor(get(), x, y, z, half)); }
    void add_mesh(const std::vector<Vec3>& v, const std::vector<std::array<uint32_t, 3>>& t) {
        check(nm_world_add_mesh(get(), v[0].data(), (int32_t)v.size(), t[0].data(), (int32_t)t.size(), nullptr));
    }
    /// Moving boxes (cars, platforms), 13 floats each: centre, quaternion xyzw, half size, velocity. Set before each step.
    void set_boxes(const std::vector<std::array<float, 13>>& b) { check(nm_world_set_boxes(get(), b.empty() ? nullptr : b[0].data(), (int32_t)b.size())); }
    /// The boxes' spins (rad/s, world axes), one per box of the last set_boxes. Set after them, before each step.
    void set_box_spins(const std::vector<Vec3>& s) { check(nm_world_set_box_spins(get(), s.empty() ? nullptr : s[0].data(), (int32_t)s.size())); }
    /// What each box is (NM_HIT_*), one per box of the last set_boxes.
    void set_box_kinds(const std::vector<int32_t>& k) { check(nm_world_set_box_kinds(get(), k.empty() ? nullptr : k.data(), (int32_t)k.size())); }
    /// With per-triangle materials (a byte each, nm::set_surface's rows).
    void add_mesh(const std::vector<Vec3>& v, const std::vector<std::array<uint32_t, 3>>& t, const std::vector<uint8_t>& materials) {
        check(nm_world_add_mesh(get(), v[0].data(), (int32_t)v.size(), t[0].data(), (int32_t)t.size(), materials.data()));
    }
    int triangles() const { return nm_world_triangle_count(get()); }
    /// The level's first hit along `dir` within `len`: its distance and normal, or none.
    bool raycast(const Vec3& from, const Vec3& dir, float len, float& dist, Vec3& normal) const {
        return check(nm_world_raycast(get(), from.data(), dir.data(), len, &dist, normal.data())) == 1;
    }
};

class Build {
    std::unique_ptr<NmBuild, decltype(&nm_build_destroy)> p;
public:
    explicit Build(const std::string& path) : p(nm_build_load(path.c_str()), nm_build_destroy) {
        if (!p) throw Error(NM_E_IO, last_error());
    }
    NmBuild* get() const { return p.get(); }
    int parts() const { return nm_build_part_count(get()); }
    /// This body fitted to another skeleton: its 21 joints (nm_build_fit).
    Build fit(const std::array<Vec3, 21>& joints) const { return Build(nm_build_fit(get(), joints[0].data())); }
    /// fit at the size `mode` picks (NM_SIZE_*): its agents take your units at scale(); give the world the same.
    Build fit_sized(const std::array<Vec3, 21>& joints, int mode = NM_SIZE_AUTO) const { return Build(nm_build_fit_sized(get(), joints[0].data(), mode)); }
    float scale() const { return nm_build_get_scale(get()); }
    /// Searches balance settings this body stands with (seconds: run it off the main thread). Returns 0-3.
    int tune(int tries = 200) { return nm_build_tune(get(), tries); }
    void copy_tune(const Build& from) { check(nm_build_copy_tune(get(), from.get())); }
    float weight() const { return nm_build_get_weight(get()); }
    void set_weight(float kg) { check(nm_build_set_weight(get(), kg)); }
    /// A build from nm_build_load's text, or from a character JSON.
    static Build from_text(const std::string& t) { return Build(nm_build_from_text(t.c_str())); }
    static Build from_character(const std::string& json) { return Build(nm_build_from_character(json.c_str())); }
    /// fit with the neck `neck` of the way up (0-1).
    Build fit_neck(const std::array<Vec3, 21>& joints, float neck) const { return Build(nm_build_fit_neck(get(), joints[0].data(), neck)); }
    /// The Playroom's bodies: its sizes against the average's (1 each; kg, height in m, 0: the average's), a named
    /// profile, or a random one.
    Build sized(float legs, float arms, float trunk, float width, float head, float neck, float kg, float height) const {
        return Build(nm_build_sized(get(), legs, arms, trunk, width, head, neck, kg, height));
    }
    Build named(const char* name) const { return Build(nm_build_named(get(), name)); }
    Build random(int seed, float variance = 1.5f, bool symmetric = true) const { return Build(nm_build_random(get(), seed, variance, symmetric)); }
    /// The head raised (m) and grown (times).
    void set_head(float raise, float grow) { check(nm_build_set_head(get(), raise, grow)); }
    /// Its weight from your skin (vertices in the joints' space, each one's part; empty: the nearest bone's); returns the kg.
    float weigh_skin(const std::array<Vec3, 21>& joints, const std::vector<Vec3>& verts, const std::vector<int32_t>& parts = {}) {
        return nm_build_weigh_skin(get(), joints[0].data(), verts[0].data(), parts.empty() ? nullptr : parts.data(), (int32_t)verts.size());
    }
    /// The scale fit_sized would give (NM_SIZE_*).
    float size_scale(const std::array<Vec3, 21>& joints, int mode = NM_SIZE_AUTO) const { return nm_size_scale(get(), joints[0].data(), mode); }
    struct PartInfo { float mass, radius, half; int parent; };
    PartInfo part(int i) const { PartInfo r; check(nm_build_part_info(get(), i, &r.mass, &r.radius, &r.half, &r.parent)); return r; }
    /// A part's standing frame.
    std::pair<Vec3, Quat> frame(int i) const { Vec3 x; Quat q; check(nm_build_part_frame(get(), i, x.data(), q.data())); return {x, q}; }
private:
    explicit Build(NmBuild* b) : p(b, nm_build_destroy) {
        if (!p) throw Error(NM_E_ARG, last_error());
    }
};

/// A message under construction: agent.message("bodyBalance").set("start", true).set("bodyStiffness", 10.f).send(world).
class Message {
    NmAgent* a;
public:
    Message(NmAgent* a, const char* name) : a(a) { check(nm_msg_begin(a, name)); }
    Message& set(const char* p, float v) { check(nm_msg_set_float(a, p, v)); return *this; }
    Message& set(const char* p, int v) { check(nm_msg_set_int(a, p, v)); return *this; }
    Message& set(const char* p, bool v) { check(nm_msg_set_bool(a, p, v)); return *this; }
    Message& set(const char* p, const Vec3& v) { check(nm_msg_set_vec3(a, p, v.data())); return *this; }
    Message& set(const char* p, const char* v) { check(nm_msg_set_string(a, p, v)); return *this; }
    /// How many params the port accepted without acting on them.
    int send(const World& w) { return check(nm_msg_send(a, w.get())); }
};

class Agent {
    std::unique_ptr<NmAgent, decltype(&nm_agent_destroy)> p;
public:
    Agent(const Build& b, const Vec3& pos, float yaw, float seed = 0.3f) : p(nm_agent_create(b.get(), pos.data(), yaw, seed), nm_agent_destroy) {
        if (!p) throw Error(NM_E_ARG, last_error());
    }
    NmAgent* get() const { return p.get(); }
    Message message(const char* name) { return Message(get(), name); }
    void step(const World& w, float dt = 1.f / 60) { check(nm_agent_step(get(), w.get(), dt)); }
    void set_mode(int mode) { check(nm_agent_set_mode(get(), mode)); }
    int parts() const { return nm_agent_part_count(get()); }
    std::pair<Vec3, Quat> transform(int i) const {
        Vec3 x; Quat q;
        check(nm_agent_get_part_transform(get(), i, x.data(), q.data()));
        return {x, q};
    }
    /// FOLLOW: a part's state from the host's ragdoll.
    void set_state(int i, const Vec3& x, const Quat& q, const Vec3& v, const Vec3& w) {
        check(nm_agent_set_part_state(get(), i, x.data(), q.data(), v.data(), w.data()));
    }
    /// FOLLOW: what to apply to the host's part (force at its centre, torque).
    std::pair<Vec3, Vec3> drive(int i) const {
        Vec3 f, t;
        check(nm_agent_get_part_drive(get(), i, f.data(), t.data()));
        return {f, t};
    }
    void push(const Vec3& at, const Vec3& impulse) { check(nm_agent_apply_push(get(), at.data(), impulse.data())); }
    void shot(const World& w, const char* set, int part, const Vec3& at, const Vec3& dir, float health = 200.f) {
        check(nm_agent_task_shot(get(), w.get(), set, part, at.data(), dir.data(), health));
    }
    /// shot with the weapon's bullet speed (its weaponinfo force) in place of 400.
    void shot(const World& w, const char* set, int part, const Vec3& at, const Vec3& dir, float health, float force) {
        check(nm_agent_task_shot_force(get(), w.get(), set, part, at.data(), dir.data(), health, force));
    }
    bool balancing() const { return nm_agent_is_balancing(get()) == 1; }
    bool fallen() const { return nm_agent_has_fallen(get()) == 1; }
    /// The part a shot along the aim strikes, and where.
    std::pair<int, Vec3> aim_hit(const Vec3& from, const Vec3& dir) const {
        Vec3 at;
        return {check(nm_agent_aim_hit(get(), from.data(), dir.data(), at.data())), at};
    }
    void explosion(const World& w, const Vec3& at, float radius = 5.f, float speed = 15.f) { check(nm_agent_task_explosion(get(), w.get(), at.data(), radius, speed)); }
    void melee_hit(const World& w, const Vec3& at, const Vec3& impulse, const Vec3& attacker_head, bool timid = false) {
        check(nm_agent_task_melee_hit(get(), w.get(), at.data(), impulse.data(), attacker_head.data(), timid));
    }
    void on_fire(const World& w, float health) { check(nm_agent_task_on_fire(get(), w.get(), health)); }
    void set_on_fire(bool burning, float health) { check(nm_agent_set_on_fire(get(), burning, health)); }
    void tase(const World& w, float seconds) { check(nm_agent_tase(get(), w.get(), seconds)); }
    void high_fall(const World& w, bool player = false) { check(nm_agent_task_high_fall(get(), w.get(), nullptr, player)); }
    void jump_roll(const World& w) { check(nm_agent_task_jump_roll(get(), w.get())); }
    /// Pushed off a ledge along `heading` (0-1 of a turn; negative: random): false when there is none that way.
    bool fall_down(const World& w, float heading = -1.0f) { return nm_agent_task_fall_down(get(), w.get(), heading) == 1; }
    /// Hands reach for and hold points (6 floats each: point, surface normal; null: that hand free), `hold` s (0: until the next task).
    void grab(const World& w, const float* left, const float* right, float hold = 0.0f) { check(nm_agent_task_grab(get(), w.get(), left, right, hold)); }
    /// After each step: a moving box's hit, true when one is ready (its damage, box index; the reaction started).
    bool car_hit(const World& w, float health, int agility, bool player_driver, float* damage, int* box = nullptr) { return nm_agent_car_hit(get(), w.get(), health, agility, player_driver ? 1 : 0, damage, box) == 1; }
    /// Holds box `box`'s door (point `at`, or null: the nearest side); dragged once it moves, lets go at 12 m/s.
    void car_door(const World& w, int box, const float* at = nullptr) { check(nm_agent_task_car_door(get(), w.get(), box, at)); }
    /// Cuts part `part` (and those below it) off as a limp piece; its parts read as before. A leg: down for good.
    void sever(const World& w, int part, const float* impulse = nullptr, const float* at = nullptr) { check(nm_agent_sever(get(), w.get(), part, at, impulse)); }
    bool is_severed(int part) const { return nm_agent_is_severed(get(), part) == 1; }
    /// Dives at `target` (within 4 m), hands on his chest or (low) thighs, and takes him down.
    void tackle(const World& w, Agent& target, bool low = false, bool keep = false) { check(nm_agent_task_tackle(get(), w.get(), target.get(), low, keep)); }
    /// 1 holding on, 2 diving, 0 none.
    int tackle_state() const { return nm_agent_tackle_state(get()); }
    /// Water whose surface is at `level` (NaN: none): down in it he floats face up.
    void set_water(const World& w, float level) { check(nm_agent_set_water(get(), w.get(), level)); }
    bool in_water() const { return nm_agent_in_water(get()) == 1; }
    /// Burning (set on fire, or caught from a burning body it touched).
    bool is_burning() const { return nm_agent_is_burning(get()) == 1; }
    bool is_tased() const { return nm_agent_is_tased(get()) == 1; }
    /// Its muscles times `k` (0.1-10; 1 as made): 1.5-2 for a movie giant or a tough enemy.
    void set_strength(float k) { check(nm_agent_set_strength(get(), k)); }
    void relax(float stiffness = -1.f, float damping = -1.f) { check(nm_agent_task_relax(get(), stiffness, damping)); }
    void brace(const World& w) { check(nm_agent_task_brace(get(), w.get(), nullptr)); }
    bool running() const { return nm_agent_task_running(get()) == 1; }
    void stop() { check(nm_agent_task_stop(get())); }
    bool dead() const { return nm_agent_is_dead(get()) == 1; }
    void set_dead(bool d) { check(nm_agent_set_dead(get(), d)); }
    bool should_get_up(float after) const { return nm_agent_should_get_up(get(), after) == 1; }
    int mode() const { return nm_agent_get_mode(get()); }

    // Parts, all at once (7 floats each: x, y, z, qx, qy, qz, qw; states 13: + velocity, spin; drives 6).
    std::vector<float> transforms() const { std::vector<float> o(parts() * 7); check(nm_agent_get_part_transforms(get(), o.data(), parts())); return o; }
    void set_states(const std::vector<float>& s) { check(nm_agent_set_part_states(get(), s.data(), (int32_t)(s.size() / 13))); }
    std::vector<float> drives() const { std::vector<float> o(parts() * 6); check(nm_agent_get_part_drives(get(), o.data(), parts())); return o; }
    /// Your animation's pose this frame (7 floats a part), for NMPose / the muscles get-up.
    void set_incoming(const std::vector<float>& t) { check(nm_agent_set_incoming_transforms(get(), t.data(), (int32_t)(t.size() / 7))); }
    std::pair<Vec3, Vec3> velocity(int i) const { Vec3 v, w; check(nm_agent_get_part_velocity(get(), i, v.data(), w.data())); return {v, w}; }
    /// Part `i`'s contact flags this step (0x04 touching last step, 0x08 touching, 0x40 the ground).
    int contact(int i) const { return check(nm_agent_get_part_contact(get(), i)); }
    std::pair<Vec3, Vec3> com() const { Vec3 x, v; check(nm_agent_get_com(get(), x.data(), v.data())); return {x, v}; }
    float mass() const { return nm_agent_get_mass(get()); }
    float kinetic_energy() const { return nm_agent_get_kinetic_energy(get()); }
    /// An impulse on part `i` at `at` (no reaction; push() reacts).
    void impulse(int i, const Vec3& at, const Vec3& j) { check(nm_agent_apply_impulse(get(), i, at.data(), j.data())); }
    void add_velocity(const Vec3& dv) { check(nm_agent_add_velocity(get(), dv.data())); }
    int nearest_part(const Vec3& at) const { return nm_agent_nearest_part(get(), at.data()); }
    /// A leash: point `at` on part `i` kept within `length` of `anchor` (slack inside, no stretch). Again on the same
    /// part: null `at` / `anchor` keep theirs (a new length; shorter reels him in at 2 m/s).
    void leash(int i, const float* at, const float* anchor, float length) { check(nm_agent_leash(get(), i, at, anchor, length)); }
    /// Part `i`'s leash let go (-1: all).
    void unleash(int i = -1) { check(nm_agent_unleash(get(), i)); }
    /// The part a segment hits first (-1: none), where and its normal.
    int raycast(const Vec3& from, const Vec3& to, Vec3& point, Vec3& normal) const { return check(nm_agent_raycast(get(), from.data(), to.data(), point.data(), normal.data())); }
    /// Part `i`'s joint to its parent: `kind` 0 the root, 1 a ball (limits: swing X, swing Y, twist), 2 a hinge (low, high).
    struct Joint { int kind, parent; Vec3 pivot; Quat frame; Vec3 limits; float armature; };
    Joint joint(int i) const {
        Joint j{};
        j.kind = check(nm_agent_get_joint(get(), i, &j.parent, j.pivot.data(), j.frame.data(), j.limits.data(), &j.armature));
        return j;
    }
    /// Part `i`'s body: `kind` 1 a capsule, 2 a box (`box` its half extents).
    struct Body { int kind; float mass; Vec3 inertia, box; };
    Body body(int i) const { Body b{}; b.kind = check(nm_agent_get_part_body(get(), i, &b.mass, b.inertia.data(), b.box.data())); return b; }
    /// Each part's frame in the zero (rest) pose.
    std::vector<float> zero_pose() const {
        std::vector<float> x(parts() * 3), q(parts() * 4);
        check(nm_agent_get_zero_pose(get(), x.data(), q.data(), parts()));
        std::vector<float> o;
        for (int i = 0; i < parts(); i++) { o.insert(o.end(), &x[i * 3], &x[i * 3 + 3]); o.insert(o.end(), &q[i * 4], &q[i * 4 + 4]); }
        return o;
    }

    // Messages and their state.
    /// A console line: "bodyBalance start=true bodyStiffness=10".
    int send(const World& w, const char* line) { return check(nm_msg_send_text(get(), w.get(), line)); }
    /// Drops the message begun and not sent.
    void cancel() { check(nm_msg_cancel(get())); }
    /// A running behaviour's param now (up to 3 floats; returns how many), or its text.
    int param(const char* message, const char* p, float* out) const { return check(nm_agent_get_param(get(), message, p, out)); }
    std::string param_string(const char* message, const char* p) const {
        return text([&](char* b, int32_t n) { return check(nm_agent_get_param_string(get(), message, p, b, n)); });
    }
    /// A behaviour's param held at `value` while it runs.
    void set_override(const char* behaviour, const char* p, float value) { check(nm_agent_set_override(get(), behaviour, p, value)); }
    void clear_overrides() { check(nm_agent_clear_overrides(get())); }
    void recentre_joint(int i) { check(nm_agent_recentre_joint(get(), i)); }
    /// The parts in `mask` held to their pose / following the incoming one.
    void hold(const char* mask, float stiffness, float damping, float oppose_gravity = 0.f) { check(nm_agent_hold(get(), mask, stiffness, damping, oppose_gravity)); }
    void follow(const char* mask, float weight) { check(nm_agent_follow(get(), mask, weight)); }
    /// NM's feedback this step: (name, behaviour slot, value).
    std::vector<std::tuple<std::string, int, float>> feedback() const {
        std::vector<std::tuple<std::string, int, float>> o;
        for (int i = 0, n = nm_agent_feedback_count(get()); i < n; i++) {
            char b[64]; int32_t slot; float v;
            if (nm_agent_feedback_get(get(), i, b, 64, &slot, &v) >= 0) o.emplace_back(b, slot, v);
        }
        return o;
    }
    /// Why he only ragdolls: one line per problem ("" none: he should react).
    std::string check_state() const { return lines([&](char* b, int32_t n) { return nm_agent_check(get(), b, n); }); }

    // State.
    /// How long (s) he has lain still (0 moving or balancing): the moment for your get-up animation; `head` the flat
    /// way from hips to head, `face_up` on his back.
    float lying(Vec3* head = nullptr, bool* face_up = nullptr) const {
        Vec3 h; int32_t f = 0;
        float r = nm_agent_get_lying(get(), h.data(), &f);
        if (head) *head = h;
        if (face_up) *face_up = f == 1;
        return r;
    }
    bool resting(const World& w) const { return nm_agent_is_resting(get(), w.get()) == 1; }
    bool settled() const { return nm_agent_is_settled(get()) == 1; }
    bool relaxing() const { return nm_agent_is_relaxing(get()) == 1; }
    bool catching_fall() const { return nm_agent_is_catching_fall(get()) == 1; }
    bool rolled_up() const { return nm_agent_is_rolled_up(get()) == 1; }
    bool awake() const { return nm_agent_is_awake(get()) == 1; }
    /// Where the balancer stands (between the feet, on the ground); false when not balancing.
    bool stance(Vec3& at) const { return check(nm_agent_get_stance(get(), at.data())) == 1; }

    // Tasks and reactions.
    void balance(const World& w, const char* set = nullptr, const Vec3* look = nullptr) { check(nm_agent_task_balance(get(), w.get(), set, look ? look->data() : nullptr)); }
    void catch_fall(const char* set = nullptr) { check(nm_agent_task_catch_fall(get(), set)); }
    void flinch(const World& w, const Vec3& from, const char* set = nullptr) { check(nm_agent_task_flinch(get(), w.get(), set, from.data())); }
    /// brace's control each frame while the car comes: its matrix (12 floats: x, y, z axes, place), where it will
    /// strike and the driver's head (null: none / the car's place).
    void brace_control(const World& w, const std::array<float, 12>& car, const Vec3* hit = nullptr, const Vec3* look = nullptr) {
        check(nm_agent_task_brace_control(get(), w.get(), car.data(), hit ? hit->data() : nullptr, look ? look->data() : nullptr));
    }
    /// A weapon's shot (weapons::find); returns the health left.
    float shoot(const World& w, int weapon, int part, const Vec3& at, const Vec3& dir, float health) { return nm_agent_shoot_weapon(get(), w.get(), weapon, part, at.data(), dir.data(), health); }
    /// A shotgun's pellets on him at once: 7 floats each (part, at, dir).
    void shotgun(const World& w, const char* set, const std::vector<float>& hits, float health, float force = 400.f) {
        check(nm_agent_task_shotgun(get(), w.get(), set, hits.data(), (int32_t)(hits.size() / 7), health, force));
    }
    /// The Playroom's extra reactions (NM_XR_* bits).
    void set_reactions(int bits) { check(nm_agent_set_reactions(get(), bits)); }
    /// Near misses dive him at this gap (m; 0: never).
    void set_dive(float gap) { check(nm_agent_set_dive(get(), gap)); }
    void jump(bool dive, const Vec3& dir) { check(nm_agent_jump(get(), dive, dir.data())); }
    bool jumping() const { return nm_agent_jumping(get()) == 1; }
    /// A bullet passing close (from, along dir): flinch, look, dive.
    void near_miss(const World& w, const Vec3& from, const Vec3& dir) { check(nm_agent_near_miss(get(), w.get(), from.data(), dir.data())); }
    /// Something thrown at him: look at it and/or flinch from it.
    void thrown_at(const World& w, const Vec3& at, const Vec3& vel, bool look = true, bool avoid = true) { check(nm_agent_thrown_at(get(), w.get(), at.data(), vel.data(), look, avoid)); }
    /// The muscles get-up (your get-up clip through set_incoming); off: he balances.
    void muscle_rise(const World& w, bool on, float help = -1.f) { check(nm_agent_muscle_rise(get(), w.get(), on, help)); }
    /// The learned get-up (getup.net) and walk (walk.net; "": off).
    void learned_getup(const std::string& net) { check(nm_agent_learned_getup(get(), net.c_str())); }
    bool learned_getup_running() const { return nm_agent_learned_getup_running(get()) == 1; }
    void learned_walk(const World& w, const std::string& net, float cycle = 1.f) { check(nm_agent_learned_walk(get(), w.get(), net.empty() ? nullptr : net.c_str(), cycle)); }
    /// Sits looking at `look` (null: straight on) on `seat` (null: where he is), until stop().
    void sit(const World& w, const Vec3* look = nullptr, const Vec3* seat = nullptr) { check(nm_agent_task_sit(get(), w.get(), look ? look->data() : nullptr, seat ? seat->data() : nullptr)); }
    /// NMPose: the incoming pose at muscle stiffness low..high.
    void pose(const World& w, float low, float high) { check(nm_agent_task_pose(get(), w.get(), low, high)); }
    void roll_up_relax(const World& w, float stiffness = -1.f, float damping = -1.f, int roll_ms = -1) { check(nm_agent_task_roll_up_relax(get(), w.get(), stiffness, damping, roll_ms)); }
    /// The ragdoll yours, moved only by your messages, for `ms` (0: until stop()).
    void script(int ms) { check(nm_agent_task_script(get(), ms)); }
    /// Who he's up against: its pelvis, spine, chest and head (12 floats; null: no one); `armed`: he points his gun at it.
    void set_attacker(const float* bones, bool armed) { check(nm_agent_set_attacker(get(), bones, armed)); }
    void gun_ready() { check(nm_agent_gun_ready(get())); }
    /// The player's stick (x, y -1..1) under camera heading `camera`; off lets go.
    void set_stick(bool on, float x = 0.f, float y = 0.f, float camera = 0.f) { check(nm_agent_set_stick(get(), on, x, y, camera)); }
};

/// The agents of `w` stepped at once over `threads` threads (0: one per core; nm_world_step_agents).
inline void step_all(const World& w, const std::vector<Agent*>& agents, float dt = 1.f / 60, int threads = 0) {
    std::vector<NmAgent*> a;
    for (Agent* x : agents) a.push_back(x->get());
    check(nm_world_step_agents(w.get(), a.data(), (int32_t)a.size(), dt, threads));
}

/// Your skeleton on an agent (nm_rig_*): made once with both standing in the same pose, then `pose` each frame
/// gives every bone its rotation under its parent (set only those) and the hips bone its world place.
class Rig {
    std::unique_ptr<NmRig, decltype(&nm_rig_destroy)> p;
    std::vector<float> out;
    int n;
public:
    /// `parents` (-1 for none, parents first), `rest` each bone's world frame (x, y, z, qx, qy, qz, qw), `parts` each
    /// bone's part or -1 (`part_for_name` guesses; one bone on part 0, the hips).
    Rig(const Agent& a, const std::vector<int32_t>& parents, const std::vector<std::array<float, 7>>& rest, const std::vector<int32_t>& parts)
        : p(nm_rig_create(a.get(), (int32_t)parents.size(), parents.data(), rest[0].data(), parts.data()), nm_rig_destroy),
          out(parents.size() * 4 + 3), n((int)parents.size()) {
        if (!p || rest.size() != parents.size() || parts.size() != parents.size()) throw Error(NM_E_ARG, last_error());
    }
    static int part_for_name(const char* name) { return nm_rig_part_for_name(name); }
    /// The 21 joints for nm_build_fit from your skeleton's bind-pose places (gaps filled; `parts` cleaned in place).
    static std::vector<float> joints(const std::vector<int32_t>& parents, const std::vector<Vec3>& places, std::vector<int32_t>& parts) {
        std::vector<float> j(63);
        check(nm_rig_joints((int32_t)parents.size(), parents.data(), places[0].data(), parts.data(), j.data()));
        return j;
    }
    /// Bone `i`'s rotation under its parent after `pose`.
    Quat rotation(int i) const { return {out[i * 4], out[i * 4 + 1], out[i * 4 + 2], out[i * 4 + 3]}; }
    Vec3 hips_place() const { return {out[n * 4], out[n * 4 + 1], out[n * 4 + 2]}; }
    /// Poses the bones from the agent now; returns the hips bone's index.
    int pose(const Agent& a) { return check(nm_rig_pose(p.get(), a.get(), out.data())); }
    /// Takes over from your animation without a snap: the parts onto the bones' world frames now (7 floats each, as
    /// the rest pose), moving as from `before` `dt` s earlier (empty: still). Before the first step.
    void take(Agent& a, const std::vector<float>& now, const std::vector<float>& before = {}, float dt = 1.f / 60) {
        check(nm_rig_take(p.get(), a.get(), now.data(), before.empty() ? nullptr : before.data(), dt));
    }
    /// Your animation's pose (the bones' world frames) as the incoming pose each frame; empty clears it.
    void incoming(Agent& a, const std::vector<float>& now) { check(nm_rig_incoming(p.get(), a.get(), now.empty() ? nullptr : now.data())); }
};

} // namespace nm
