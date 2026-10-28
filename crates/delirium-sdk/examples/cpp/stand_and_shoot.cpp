// A ped stands under bodyBalance, then takes a POWERFUL_WEAPON shot. usage: stand_and_shoot <fred.xrt> <build.txt> [taskparams.txt]
#include "delirium_physics.hpp"
#include <cstdio>

int main(int argc, char** argv) try {
    if (argc < 3) { std::puts("usage: stand_and_shoot <fred.xrt> <build.txt> [taskparams.txt]"); return 2; }
    nm::init(argv[1]);
    if (argc > 3) nm::load_task_params(argv[3]);
    nm::Build build(argv[2]);
    nm::World world;
    world.add_floor(0, 0, 0, 50);
    nm::Agent agent(build, {0, 0, 0}, 0.f);
    agent.message("bodyBalance").set("start", true).set("bodyStiffness", 10.f).send(world);
    for (int i = 0; i < 60; i++) agent.step(world);
    auto [chest, q] = agent.transform(10);
    agent.push(chest, {0, 40, 0});
    agent.shot(world, "POWERFUL_WEAPON", 9, chest, {0, 1, 0});
    for (int i = 0; i < 120; i++) {
        agent.step(world);
        if (i % 30 == 29) {
            auto [x, r] = agent.transform(0);
            std::printf("%.1fs pelvis %.2f %.2f %.2f balancing %d fallen %d\n", (i + 1) / 60.f, x[0], x[1], x[2], agent.balancing(), agent.fallen());
        }
    }
    return 0;
} catch (const nm::Error& e) {
    std::printf("delirium_physics %d: %s\n", e.code, e.what());
    return 1;
}
