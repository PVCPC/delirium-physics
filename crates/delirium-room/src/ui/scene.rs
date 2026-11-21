use super::*;

pub fn page(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, page: u8) {
    match page {
        0 => location(ui, sim, pressed),
        1 => look(ui, sim, pressed),
        _ => crowds(ui, sim, pressed),
    }
}

fn stage_label(l: &str) -> &str {
    match l { "room" => "Room", "big stairs" => "Big stairs", "rooftop" => "Rooftop", "low wall" => "Low wall", "empty" => "Empty", "rail" => "Railing", "street" => "Street", "rope bridge" => "Rope bridge",
              "trampoline" => "Trampoline", "earthquake" => "Earthquake", "mechanical bull" => "Mechanical bull", "cab door" => "Taxi door test", "high ledge" => "High ledge",
              "path tracing test" => "Path tracing test", "coloured lights hall" => "Coloured lights hall", "skate park" => "Skate park", _ => l }
}

fn location(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Location");
    info(ui, "Pick where Homer is. Your settings stay; the scene starts again there.");
    let surfaces = sim.room.surfaces.clone();
    for (name, list) in [("Stages", &STAGES[..]), ("Machines & tests", &EXTRAS[..])] {
        sub(ui, name);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            for &(k, l) in list {
                if thumb(ui, stage_label(l), &format!("stage {k}"), sim.room.stage == k, || editor::stage_pic(&surfaces, k))
                    .on_hover_text(format!("{}: start the scene there", stage_label(l))).clicked() {
                    *pressed = Some(format!("#stage {k}"));
                }
            }
        });
    }
    sub(ui, "Build your own");
    let open = sim.edit_level;
    if ui.add(egui::Button::selectable(open, "✏  Open the level editor").min_size(egui::vec2(ui.available_width(), 44.0)))
        .on_hover_text("Add boxes, ramps, cylinders and spheres; move, turn and size them; save and load levels.").clicked() {
        sim.edit_level = !open;
    }
    sub(ui, "Floor");
    *pressed = pressed.take().or(switches(ui, &[("ice", sim.ice.is_some(), "Ice floor".into())]));
}

fn look(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "How the room looks");
    let looks: Vec<(&str, bool, String)> = LOOKS.iter().map(|(n, l)| (*n, sim.skin == *l, n.to_string())).collect();
    if let Some(n) = tiles_w(ui, "looks", &looks, 120.0) {
        *pressed = Some(format!("#look {n}"));
    }
    sub(ui, "Sky");
    *pressed = pressed.take().or(switches(ui, &[("#photosky", sim.sky == 1, "Photo sky".into()), ("#bevysky", sim.sky == 2, "Bevy sky".into()),
        ("#godrays", sim.god_rays, "God rays".into()), ("#fog", sim.skin.fog, "Fog".into())]));
    sub(ui, "Floor");
    *pressed = pressed.take().or(switches(ui, &[("#grid", sim.skin.grid, "Grid lines".into()), ("#infinite", sim.infinite, "Extend floor".into()),
        ("#ripples", sim.show_ripples, "Floor ripples".into())]));
    sub(ui, "Colours & sun");
    ui.horizontal_wrapped(|ui| {
        let skin = &mut sim.skin;
        for (name, c, tip) in [("Sky colour", &mut skin.sky, "The sky's colour (with no photo or Bevy sky)"), ("Floor colour", &mut skin.floor, "The floor's colour")] {
            let mut k = *c;
            if ui.color_edit_button_rgb(&mut k).on_hover_text(tip).changed() {
                *c = k;
            }
            ui.label(name);
            ui.add_space(12.0);
        }
    });
    ui.horizontal(|ui| {
        let mut k = sim.sun_color;
        if ui.color_edit_button_rgb(&mut k).on_hover_text("The sun's colour: warm for sunset, blue for moonlight.").changed() {
            sim.sun_color = k;
        }
        ui.label("Sun colour");
    });
    slide(ui, "Light intensity", &mut sim.skin.sun, 0.1, 4.0, "×").on_hover_text("How bright the sun and sky light are.");
    ui.add(egui::Slider::new(&mut sim.sun_dir[0], -180.0..=180.0).text("Sun direction").suffix("°")).on_hover_text("Where the sun is round the room (the photo sky turns with it).");
    ui.add(egui::Slider::new(&mut sim.sun_dir[1], 3.0..=89.0).text("Sun height").suffix("°")).on_hover_text("How high the sun is: low for long shadows and a sunset.");
    if ui.button("Reset sun angle").on_hover_text("The sun back where it starts").clicked() {
        sim.sun_dir = SUN_DIR;
    }
}

fn crowds(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Crowds & vehicles");
    sub(ui, "People");
    *pressed = pressed.take().or(tiles(ui, "add", &[("crowd", sim.crowd.len() == 4, "Add 4 people".into()), ("grid", sim.crowd.len() == 24, "Add 24 people".into())]));
    sub(ui, "FPS stress tests");
    *pressed = pressed.take().or(tiles_w(ui, "stress", &[("stress", sim.stress && sim.crowd.len() == 49, "50 Homers".into()),
                                                   ("stress100", sim.stress && sim.crowd.len() == 99, "100 Homers".into()),
                                                   ("stress200", sim.stress && sim.crowd.len() == 199, "200 Homers".into())], 100.0));
    sub(ui, "Taxis");
    ui.horizontal_wrapped(|ui| {
        let w = (ui.available_width() - 8.0) / 2.0;
        if ui.add_enabled(sim.taxis.len() < TAXIS, egui::Button::new("Add one taxi").min_size(egui::vec2(w, 44.0))).on_hover_text("Spawn one drivable taxi. Play as Hero and press F beside it to get in.").clicked() {
            *pressed = Some("add taxi".into());
        }
        if ui.add_sized([w, 44.0], egui::Button::new(egui::RichText::new("Remove taxis").color(RED))).on_hover_text("Takes the parked taxis away.").clicked() {
            *pressed = Some("clear taxis".into());
        }
    });
    info(ui, &format!("Taxis: {} / {TAXIS} · one a click. Hero: F gets in and out, W / S drive, A / D steer.", sim.taxis.len()));
}
