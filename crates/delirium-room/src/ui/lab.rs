use super::*;

pub fn page(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State, page: u8, joints_open: &mut bool) {
    match page {
        0 => movement(ui, sim, pressed, st),
        1 => reactions(ui, sim, pressed),
        2 => inspect(ui, sim, joints_open),
        _ => rdr2(ui, sim),
    }
}

fn movement(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State) {
    head(ui, "Experimental movement");
    *pressed = pressed.take().or(tiles_w(ui, "preview_move", &[("experimental walk", sim.preview_on, "Experimental walk · W walk, Shift jog".into())], 300.0));
    if sim.watch.is_some() || [learn::path(), learn::walk_path(false), learn::walk_path(true)].iter().any(|p|
        std::path::Path::new(p).exists() || std::path::Path::new(&learn::live_path(p)).exists()) {
        if pop_head(ui, st, POP_TRAINING, "Learned movement · local training") {
            training(ui, sim, pressed);
        }
    } else {
        info(ui, "Learned movement shows here once a local training run has written its networks.");
    }
}

pub fn training(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    *pressed = pressed.take().or(tiles(ui, "learned_move", &[
        ("walk learned", sim.walk_learned.as_ref().is_some_and(|w| !w.3), "Learned WASD walk".into()),
        ("run learned", sim.walk_learned.as_ref().is_some_and(|w| w.3), "Learned WASD run".into())]));
    let watches: Vec<_> = [("watch training", "Watch get-up", learn::path()),
        ("watch walk training", "Watch walk", learn::walk_path(false)), ("watch run training", "Watch run", learn::walk_path(true))]
        .into_iter().map(|(key, label, path)| (key, sim.watch.as_ref().is_some_and(|w| w.path == path), label.into())).collect();
    *pressed = pressed.take().or(tiles(ui, "train", &watches));
    info(ui, &match sim.watch.as_ref() {
        Some(w) if w.stamp.is_some() => format!("{} · generation {}: best try {:.2}. The 8 tries of the latest generation, side by side.", std::path::Path::new(&w.path).file_stem().unwrap_or_default().to_string_lossy(), w.generation,
                                                w.scores.iter().copied().fold(f32::MIN, f32::max)),
        Some(_) => "Waiting for the trainer...".into(),
        None => "Watch the trainer's latest generation live.".into(),
    });
}

fn reactions(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Experimental reactions");
    switch(ui, &mut sim.extra, "Additional experimental reactions", "Our own uses of the reaction engine's moves, beyond the game's: pick which below.");
    let on = sim.extra;
    ui.add_enabled_ui(on, |ui| {
        fold(ui, "Which reactions", on, |ui| {
            for (i, (name, tip)) in XR.iter().enumerate() {
                let mut b = sim.extra_pick & (1 << i) != 0;
                if switch(ui, &mut b, name, tip).changed() {
                    sim.extra_pick ^= 1 << i;
                }
            }
        });
    });
    sub(ui, "Delirium's own reactions");
    info(ui, "Start one, then poke or shoot him.");
    let items: Vec<(&str, bool, String)> = delirium::extras::messages().map(|m| {
        let mut label = String::new();
        for (k, c) in m.name.chars().enumerate() {
            if k == 0 { label.extend(c.to_uppercase()); }
            else {
                if c.is_uppercase() { label.push(' '); }
                label.push(c.to_ascii_lowercase());
            }
        }
        (m.name, false, label)
    }).collect();
    *pressed = pressed.take().or(tiles_w(ui, "additional reactions", &items, 145.0));
}

fn inspect(ui: &mut egui::Ui, sim: &mut Sim, joints_open: &mut bool) {
    head(ui, "Inspect");
    info(ui, "Technical information for comparing and diagnosing the simulation.");
    if ui.add(egui::Button::selectable(*joints_open, "Open joint inspector").min_size(egui::vec2(0.0, PILL_H))).on_hover_text("Each joint's angle against its target, in its own window").clicked() { *joints_open = true; }
    let a = &sim.agent;
    let (com, v) = a.doll.com();
    let joints = delirium::nmrs::ASSET[nm()].get().map_or(0, |j| j.len());
    let parts = &sim.build.parts;
    let table = joints + 1 == parts.len() && parts.iter().skip(1).all(|p| p.parent.is_some());
    let masses = delirium::nmrs::MASSES[nm()].get().is_some_and(|m| m.len() == parts.len() && m.iter().enumerate().all(|(k, a)| (a - sim.agent.doll.part_mass(k)).abs() < 1e-3));
    let yes = |b: bool| if b { "yes" } else { "NO" };
    let checks = format!("joint table matches Fred.xrt: {} ({joints} joints)\npart masses match: {}", yes(table), yes(masses));
    let ours = format!("ours  t {:.2}s  {}x{}\nCOM ({:.2}, {:.2}, {:.2})  |v| {:.2} m/s\nmass {:.1} kg  KE {:.0} J", sim.steps as f32 / 60.0, sim.speed,
                       if sim.paused { " paused" } else { "" }, com.x, com.y, com.z, v.length(), a.doll.mass(), a.doll.kinetic());
    card(ui, |ui| {
        ui.label(mono(checks).color(DIM));
        ui.label(mono(ours));
        ui.label(mono(format!("world: {} triangles, {} objects{}", sim.world.len(), sim.room.objs.len(),
                              if sim.look && !sim.room.objs.is_empty() { format!(", looking at {}", sim.room.objs.len() - 1) } else { String::new() })).color(DIM));
    });
}

fn rdr2(ui: &mut egui::Ui, sim: &mut Sim) {
    head(ui, "RDR2 horse reference");
    info(ui, "RDR2's own horse skeleton (yellow) with our capsules on it (blue), played on a loop beside the horse. Local reference only; link RDR2 in Play > Bonus.");
    horse_ghost::panel(ui, sim);
}
