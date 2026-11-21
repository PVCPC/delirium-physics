use super::*;

pub fn page(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State, page: u8) {
    match page {
        0 => reactions(ui, sim, pressed),
        1 => damage(ui, sim, pressed),
        _ => {
            if pop_head(ui, st, POP_PARAMS, "Reaction engine parameters") {
                let s = &mut *sim;
                params::tab(ui, &mut s.param_edits, &mut s.param_search, &mut s.param_changed_only, pressed);
            }
        }
    }
}

fn reactions(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Reactions");
    info(ui, "Start a reaction, then poke, shoot or pull Homer to see how he responds.");
    let items: Vec<(&str, bool, String)> = BEHAVIOURS.iter().filter(|b| !b.starts_with("STOP")).map(|b| (*b, false, reaction_label(b).to_string())).collect();
    *pressed = pressed.take().or(tiles_w(ui, "beh", &items, 112.0));
    sub(ui, "How he reacts");
    switch(ui, &mut sim.step_limit, "Step limit", "As in the game: a stumbling Homer falls after so many steps. Off: he keeps stepping until he finds his balance.");
    switch(ui, &mut sim.carry_pushes, "Carry pushes", "A shot or push on a ragdoll that was just moved far (through a portal) lands where it hit. Off, as in the game: it can spin him.");
    switch(ui, &mut sim.fire_react, "Fire reaction", "Set alight, he flails and tries to put himself out. Off: he burns (and chars, and gets hurt) without reacting.");
    switch(ui, &mut sim.dive, "Dive when shot at", "Shot at again soon after a near miss, he dives out of the line of fire (once a burst: he won't keep diving under automatic fire).");
    ui.add_enabled(sim.dive, egui::Slider::new(&mut sim.dodge_gap, 0.0..=10.0).text("Dive if shot at again within").custom_formatter(|x, _| if x <= 0.0 { "never".into() } else { format!("{x:.1} s") }))
        .on_hover_text("A first near miss: he flinches. Another within this time: he dives out of the line. Rounds in the first half second (one click of an automatic) count as one.");
    sub(ui, "Automatic reactions");
    *pressed = pressed.take().or(switches(ui, &[("look", sim.look, "Look at objects".into()), ("avoid", sim.avoid, "Avoid objects".into()),
        ("game tasks", sim.tasks, "Animation mode".into()), ("infinite health", sim.god, "Unlimited health".into()), ("weak", sim.weak, "Weaken muscles".into())]));
    *pressed = pressed.take().or(tiles(ui, "getup", &[("get-up", sim.getup_mode != 0, format!("Get-up: {}", GETUP_MODES[sim.getup_mode as usize]))]));
    info(ui, "More reactions of Delirium's own: Lab > Reactions.");
}

fn damage(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Damage");
    *pressed = pressed.take().or(switches(ui, &[("#blood", sim.pixel_blood, "Pixel blood".into()), ("#gore", sim.gore, "Extra bloody".into()),
        ("#layers", sim.layers, "Layered damage".into()), ("#pops", sim.show_pops, "Damage numbers".into()), ("#meat", sim.meat_on, "Hall of Meat".into())]));
    sub(ui, "Sever");
    let hat = matches!(sim.hat, Some(sever::Hat::On));
    *pressed = pressed.take().or(switches(ui, &[("dismember", sim.dismember, "Dismember".into())]));
    let mut cut: Vec<(&str, bool, String)> = vec![("hat", hat, if hat { "Knock hat off".into() } else { "Hat on".into() })];
    cut.extend(sever::CUTS.iter().map(|&(n, k)| (n, sim.agent.doll.gone >> k & 1 != 0, format!("Cut {}", &n[6..]))));
    *pressed = pressed.take().or(tiles_w(ui, "sever", &cut, 112.0));
}
