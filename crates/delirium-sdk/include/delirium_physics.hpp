// delirium_physics.hpp: C++ over the C API (delirium_physics.h): owning handles and exceptions. Header-only.
#pragma once
#include "delirium_physics.h"
#include <array>
#include <memory>
#include <stdexcept>
#include <string>
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
    /// Its muscles times `k` (0.1-10; 1 as made): 1.5-2 for a movie giant or a tough enemy.
    void set_strength(float k) { check(nm_agent_set_strength(get(), k)); }
    void relax(float stiffness = -1.f, float damping = -1.f) { check(nm_agent_task_relax(get(), stiffness, damping)); }
    void brace(const World& w) { check(nm_agent_task_brace(get(), w.get(), nullptr)); }
    bool running() const { return nm_agent_task_running(get()) == 1; }
    void stop() { check(nm_agent_task_stop(get())); }
    bool dead() const { return nm_agent_is_dead(get()) == 1; }
    void set_dead(bool d) { check(nm_agent_set_dead(get(), d)); }
    bool should_get_up(float after) const { return nm_agent_should_get_up(get(), after) == 1; }
};

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
};

} // namespace nm
