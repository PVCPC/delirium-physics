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

inline void init(const std::string& xrt) { check(nm_init(xrt.c_str())); }
inline void load_task_params(const std::string& path) { check(nm_load_task_params(path.c_str())); }
/// Surface row `material` (a mesh triangle's byte) takes this friction and elasticity for every agent; ice ~0.05.
inline void set_surface(int material, float friction, float elasticity) { check(nm_set_surface(material, friction, elasticity)); }

class World {
    std::unique_ptr<NmWorld, decltype(&nm_world_destroy)> p{nm_world_create(), nm_world_destroy};
public:
    NmWorld* get() const { return p.get(); }
    void add_floor(float x, float y, float z, float half) { check(nm_world_add_floor(get(), x, y, z, half)); }
    void add_mesh(const std::vector<Vec3>& v, const std::vector<std::array<uint32_t, 3>>& t) {
        check(nm_world_add_mesh(get(), v[0].data(), (int32_t)v.size(), t[0].data(), (int32_t)t.size(), nullptr));
    }
    /// Moving boxes (cars, platforms), 13 floats each: centre, quaternion xyzw, half size, velocity. Set before each step.
    void set_boxes(const std::vector<std::array<float, 13>>& b) { check(nm_world_set_boxes(get(), b.empty() ? nullptr : b[0].data(), (int32_t)b.size())); }
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
    bool balancing() const { return nm_agent_is_balancing(get()) == 1; }
    bool fallen() const { return nm_agent_has_fallen(get()) == 1; }
};

} // namespace nm
