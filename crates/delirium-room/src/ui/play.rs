use super::*;

pub fn page(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, page: u8) {
    match page {
        0 => hero(ui, sim, pressed),
        1 => weapons(ui, sim, pressed),
        2 => throw(ui, sim, pressed),
        _ => bonus(ui, sim, pressed),
    }
}

fn hero(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Play");
    info(ui, "Take part yourself, or steer Homer.");
    *pressed = pressed.take().or(tiles(ui, "play", &[("Hero", sim.hero.is_some(), onoff("Play as Hero", sim.hero.is_some())),
        ("walk", sim.walk, onoff("Walk Homer", sim.walk)), ("skate", sim.riding(), onoff("Skate (J)", sim.riding()))]));
    sub(ui, "Hero");
    *pressed = pressed.take().or(switches(ui, &[("shoulder cam", sim.hero_ots, "Shoulder camera".into()),
        ("hero physics", sim.hero_physics, "Hero physics".into()), ("super punch", sim.super_punch, "Super punch".into())]));
    *pressed = pressed.take().or(tiles(ui, "tackle", &[("tackle high", sim.tackle.is_some(), "Tackle high".into()), ("tackle low", sim.tackle.is_some(), "Tackle low".into())]));
    sub(ui, "Homer");
    *pressed = pressed.take().or(tiles(ui, "jump", &[("jump", false, "Jump".into()), ("jump dive", false, "Dive".into())]));
    sub(ui, "Homer fights back");
    *pressed = pressed.take().or(tiles(ui, "fight", &[("passive", !sim.passive, format!("Homer: {}", homer_mode(sim).split(' ').next().unwrap_or(""))),
        ("homer weapon", sim.homer_weapon.is_some(), format!("Weapon: {}", sim.homer_weapon.map_or("fists", |w| WEAPONS[w].0))),
        ("gun ai", sim.gun_ai, format!("Gun AI: {}", if sim.gun_ai { "Game" } else { "Delirium" }))]));
    *pressed = pressed.take().or(switches(ui, &[("even fight", sim.even, "Even fight".into())]));
    fold(ui, "Partial ragdoll", false, |ui| {
        info(ui, "Part of him on an animation, the rest on his ragdoll.");
        *pressed = pressed.take().or(tiles(ui, "partial", &[("partial legs", sim.partial.as_ref().is_some_and(|p| p.1 == PARTIAL_PLANTED), "Legs planted".into()),
            ("partial circle", sim.partial.as_ref().is_some_and(|p| p.1 == PARTIAL_CIRCLE), "Circling".into()),
            ("partial boxer", sim.partial.as_ref().is_some_and(|p| p.1 == PARTIAL_BOXER), "Boxer".into())]));
        *pressed = pressed.take().or(switches(ui, &[("partial run", sim.part.keep_running, "Keep running when hit".into()),
            ("partial break", sim.part.breaks, "Hits break the animation".into())]));
        slide(ui, "Circle size", &mut sim.partial_r, 3.0, 25.0, " m").on_hover_text("Partial ragdoll: circling's radius (press it again to take a new size).");
    });
}

fn weapons(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Weapons & click");
    let shoots = sim.click_shoots || sim.aim_gun;
    left_click(ui, sim, pressed);
    if !shoots {
        poke_strength(ui, sim);
    }
    row(ui, "Aim from", |ui| {
        for ((on, name, key), tip) in [(true, "Floating weapon", "#aim gun"), (false, "Camera", "#aim camera")].into_iter().zip(["The weapon floats by the pointer; click to fire or swing.", "You shoot from the camera at what you click."]) {
            if ui.add(egui::Button::selectable(sim.aim_gun == on, name).min_size(egui::vec2(0.0, PILL_H))).on_hover_text(tip).clicked() {
                *pressed = Some(key.into());
            }
        }
    });
    #[cfg(feature = "portal")]
    let (portal, picked) = (sim.portal.on(), sim.portal.gun);
    #[cfg(not(feature = "portal"))]
    let (portal, picked) = (false, false);
    for (k, name) in [(0, "Guns"), (1, "Thrown"), (2, "Melee")] {
        sub(ui, name);
        let items: Vec<(String, bool, String)> = (0..WEAPONS.len()).filter(|&i| kind(i) == k).map(|i| (i.to_string(), sim.weapon == i && !picked, weapon_name(i))).collect();
        let items: Vec<(&str, bool, String)> = items.iter().map(|(a, b, c)| (a.as_str(), *b, c.clone())).collect();
        if let Some(i) = tiles_w(ui, &format!("guns {k}"), &items, 104.0) {
            *pressed = Some(format!("#weapon {i}"));
        }
    }
    if portal {
        sub(ui, "Portal 2");
        if big(ui, "Portal gun", picked, 104.0).on_hover_text("Portal 2's portal gun: left click blue, right click orange").clicked() {
            *pressed = Some("#portalgun".into());
        }
    }
    if !picked && kind(sim.weapon) == 0 {
        let (between, _, pellets, _) = sim.gun_stats();
        let damage = sim.weapons.iter().find(|w| w.name == WEAPONS[sim.weapon].1).map_or(String::new(), |w| format!("Damage {:.0}{} · ", w.damage, if pellets > 1 { " a pellet" } else { "" }));
        info(ui, &format!("{}{}{:.0} shots a minute · {} a clip", damage, if pellets > 1 { format!("{pellets} pellets · ") } else { String::new() },
                          60000.0 / between.max(1.0), sim.clip_size().0));
    }
    if picked {
        info(ui, if sim.hero.is_some() { "Hero: click the view to aim with the mouse over his shoulder (Esc frees it). Left click blue, right click orange." }
                 else if shoots { "Left click: blue portal, right click: orange.  Hold Ctrl: at the pointer." }
                 else { "The portal gun fires with Left click on Shoot." });
    }
    info(ui, if sim.aim_gun { "Click: fire.  Hold Ctrl: the gun stays put and aims at the pointer.  Shift + right click: a fatal shot." }
            else if sim.click_shoots { "Click a body part to shoot it.  Shift + right click: a fatal shot." }
            else { "Shift + click a body part to shoot it.  Shift + right click: a fatal shot." });
    if !shoots {
        fold(ui, "Drag line", true, |ui| {
            slide(ui, "Line stretch > tight", &mut sim.line_k, 0.05, 5.0, "x")
                .on_hover_text("Ctrl + Scroll Wheel: up tighter, down stretchier (the drag's line; with lines tied and no drag, the one you point at, else the newest tied one).");
            switch(ui, &mut sim.line_wrap, "Lines wrap round the map (K)", "Tied lines, rope or leash, catch on walls and corners and hold him from there. Off: straight through.");
            switch(ui, &mut sim.line_leash, "Leash (L): locks at its length", "No stretch at all: he can swing on it but never go further. L switches the drag's line, or with no drag the newest tied one (locked where it is now).");
            if !sim.ties.is_empty() && ui.add(egui::Button::new(format!("Cut all lines ({})", sim.ties.len())).min_size(egui::vec2(0.0, PILL_H))).on_hover_text("Cut every tied line (Shift+T)").clicked() {
                sim.tie(None, true);
            }
            info(ui, "L: leash or stretchy rope. Ctrl + Scroll Wheel: up tight, down stretchy. Alt + Scroll Wheel: longer or shorter. The line shows it: thin, dashed and yellow is a bungee; thick, solid and red a tight rope; a leash pink when taut, dark blue when slack.");
            info(ui, "A tied line is as long as when you tied it: slack (hanging) when he's nearer, holding past that. Point at a line to pick it: L, the scroll wheels and T act on that one, else the newest.");
            info(ui, "While dragging, T ties the line to the map where the pointer is (or right there in the air). Drag him again as much as you like. T cuts the newest line, Shift+T all.");
        });
    }
}

fn left_click(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    let shoots = sim.click_shoots || sim.aim_gun;
    row(ui, "Left click", |ui| {
        for ((on, name, key), tip) in [(false, "Poke", "#click poke"), (true, "Shoot", "#click shoot")].into_iter().zip(["A click pushes the body part", "A click fires the weapon"]) {
            if ui.add(egui::Button::selectable(shoots == on, name).min_size(egui::vec2(90.0, PILL_H))).on_hover_text(tip).clicked() {
                *pressed = Some(key.into());
            }
        }
    });
}

fn poke_strength(ui: &mut egui::Ui, sim: &mut Sim) {
    slide(ui, "Poke strength", &mut sim.poke, 1.0, 200.0, " N·s").on_hover_text("How hard a left click (poke) pushes the body part (N s).");
}

pub fn strip(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, open: &mut bool) {
    let shoots = sim.click_shoots || sim.aim_gun;
    #[cfg(feature = "portal")]
    let portal = sim.portal.gun;
    #[cfg(not(feature = "portal"))]
    let portal = false;
    let weapon = if portal { "Portal gun".to_string() } else { weapon_name(sim.weapon) };
    ui.horizontal(|ui| {
        let head = egui::RichText::new(format!("{}  🎮  Quick play", if *open { "-" } else { "+" })).strong().color(HI);
        if ui.add(egui::Button::new(head).frame(false).min_size(egui::vec2(0.0, PILL_H))).on_hover_text("Show or hide the quick Play controls (they stay under every page; the rest is in Play)").clicked() {
            *open = !*open;
        }
        if !*open {
            ui.label(egui::RichText::new(if shoots { format!("Click: shoot · {weapon}") } else { format!("Click: poke · {:.0} N·s", sim.poke) }).color(DIM));
        }
    });
    if !*open {
        return;
    }
    left_click(ui, sim, pressed);
    if !shoots {
        ui.label(egui::RichText::new("Shift + click: shoot  ·  Shift + right click: fatal shot").small().color(DIM))
            .on_hover_text("Poking or not, Shift + click shoots the body part under the mouse with the weapon (Play > Weapons)");
    }
    if shoots {
        row(ui, "Weapon", |ui| {
            egui::ComboBox::from_id_salt("quick weapon").width(170.0).height(420.0).selected_text(weapon).show_ui(ui, |ui| {
                for i in 0..WEAPONS.len() {
                    if ui.selectable_label(sim.weapon == i && !portal, weapon_name(i)).clicked() {
                        *pressed = Some(format!("#weapon {i}"));
                    }
                }
            }).response.on_hover_text("The weapon a left click fires or swings (all of them and their stats: Play > Weapons)");
        });
    } else {
        poke_strength(ui, sim);
    }
}

fn weapon_name(i: usize) -> String {
    let w = WEAPONS[i].0;
    match w { "deagle" => "Desert Eagle", "smg" => "SMG", "ak47" => "AK-47", "rpg" => "RPG", "pistol" => "Pistol", "sniper" => "Sniper", "shotgun" => "Shotgun", "grenade" => "Grenade", "molotov" => "Molotov", "bat" => "Bat", "knife" => "Knife", "taser" => "Taser", "baton" => "Stun baton", "flamethrower" => "Flamethrower", _ => w }.to_string()
}

fn throw(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Throw & push");
    info(ui, "At Homer; hold Ctrl and point to aim it (keys 1, 2, 3 throw at the pointer).");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        for (key, name, pic) in [("throw ball", "Ball", "ball"), ("throw box", "Box", "box"), ("drop crate", "Crate", "crate")] {
            let tip = TIPS.iter().find(|t| t.0 == key).map_or(name, |t| t.1);
            if thumb(ui, name, &format!("kind {pic}"), false, || editor::kind_pic(pic)).on_hover_text(tip).clicked() {
                *pressed = Some(key.into());
            }
        }
    });
    slide(ui, "Object weight", &mut sim.room.weight, 0.25, 4.0, "×").on_hover_text("How heavy the thrown things are against their usual weight.");
    *pressed = pressed.take().or(tiles(ui, "throw", &[("clear", false, "Clear objects".into())]));
    sub(ui, "Impact tests");
    *pressed = pressed.take().or(tiles(ui, "impact tests", &[("push front", false, "Push from front".into()), ("push back", false, "Push from behind".into()), ("push fast", false, "Box from front (fast)".into()),
        ("car", false, "Car impact".into()), ("fall", false, "Drop Homer".into()), ("high fall", false, "High drop".into()),
        ("explosion pile", false, "Explosion pile".into()), ("wrecking ball", false, "Wrecking ball".into())]));
}

fn bonus(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "Bonus games");
    info(ui, "Optional: link your own copies of these games for their extras.");
    #[cfg(feature = "portal")]
    fold(ui, "Portal 2", true, |ui| {
        let on = sim.portal.on();
        *pressed = pressed.take().or(switches(ui, &[("#portal2", on, "Portal 2".into())]));
        portal::settings(ui, sim, pressed);
    });
    fold(ui, "Skate 3", true, |ui| {
        skate_files::settings(ui, &mut sim.skate_link, pressed);
        info(ui, "Bails: Skate 3's own, or Homer onto his ragdoll (Delirium physics). Riding, walking and the board are Skate 3's either way.");
        let s3 = sim.skate3_bails;
        *pressed = pressed.take().or(tiles(ui, "skatephys", &[("#skate3_bails 1", s3, "Skate 3 physics".into()), ("#skate3_bails 0", !s3, "Delirium physics".into())]));
    });
    fold(ui, "Red Dead Redemption 2", true, |ui| {
        info(ui, "Link your own RDR2 here and the Horse (Cast > Animals) is RDR2's own horse body; without it, the Farm Animals horse.");
        ui.label("RDR2 folder");
        ui.add(egui::TextEdit::singleline(&mut sim.rdr2_dir).desired_width(f32::INFINITY).min_size(egui::vec2(0.0, PILL_H)).hint_text("e.g. F:/SteamLibrary/steamapps/common/Red Dead Redemption 2"))
            .on_hover_text("The folder with RDR2.exe in it");
        let dir = sim.rdr2_dir.trim().trim_matches('"').to_string();
        if dir != sim.rdr2_dir {
            sim.rdr2_dir = dir;
        }
        let found = std::path::Path::new(&sim.rdr2_dir).join("RDR2.exe").is_file();
        if !sim.rdr2_dir.is_empty() && !found {
            ui.colored_label(HI, "No RDR2.exe in that folder. Choose the folder with RDR2.exe in it.");
        }
        if found {
            info(ui, "Linked. The first time on this PC, RDR2 opens for about 40 s and closes by itself (its keys); then the horse comes out.");
        }
    });
}
