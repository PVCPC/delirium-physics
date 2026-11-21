use super::*;

pub fn page(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State, page: u8) {
    match page {
        0 => people(ui, sim, pressed),
        1 => animals(ui, sim, pressed, st),
        _ => appearance(ui, sim, st),
    }
}

type Caps = Vec<(Vec3, Vec3, f32, usize, Color)>;

static BAKED: std::sync::Mutex<Option<(egui::TextureId, u64, f32, f32)>> = std::sync::Mutex::new(None);
static HEIGHTS: std::sync::Mutex<Vec<f32>> = std::sync::Mutex::new(vec![]);
static WANT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const FILL: f32 = 0.9;
const TALL: f32 = 2.2;
const ASPECT: f32 = 0.73;
const PERSON_H: f32 = 150.0;
const BASE: f32 = 0.04;
const GUIDE: f32 = 1.8;
fn bake_eye() -> Vec3 {
    Vec3::new(0.27, -1.0, 0.1).normalize()
}
const PX: u32 = 192;
const COLS: usize = 14;
const THUMB_LAYER: usize = 29;

fn bake_key(sim: &Sim) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{} {:?} {:?} {:?} {:?} {}", sim.ped_color, sim.part_colors, *CUSTOM_SIZE.lock().unwrap(), *CUSTOM_HEIGHT.lock().unwrap(), *CUSTOM_NECK.lock().unwrap(),
            FIGURE_BODY.lock().unwrap().is_some()).hash(&mut h);
    h.finish()
}

#[derive(Component)]
pub struct Baking;

pub static PIPELINES_WAITING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn count_waiting(cache: Res<bevy::render::render_resource::PipelineCache>) {
    PIPELINES_WAITING.store(cache.waiting_pipelines().count(), std::sync::atomic::Ordering::Relaxed);
}

pub fn bake(mut commands: Commands, sim: NonSend<Sim>, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>,
            mut mats: ResMut<Assets<StandardMaterial>>, mut tex: ResMut<bevy_egui::EguiUserTextures>, temp: Query<Entity, With<Baking>>,
            mut st: Local<Option<(Handle<Image>, u64, f32, f32, u16)>>, mut old: Local<Option<Handle<Image>>>) {
    use bevy::camera::visibility::RenderLayers;
    if let Some((img, key, cell, k, frames)) = st.as_mut() {
        *frames += 1;
        if *frames >= 90 && PIPELINES_WAITING.load(std::sync::atomic::Ordering::Relaxed) > 0 && *frames < 1200 {
            *frames = 80;
        }
        if *frames >= 90 {
            let mut n = 0;
            for e in &temp {
                commands.entity(e).despawn();
                n += 1;
            }
            eprintln!("thumbs  baked: {n} entities (camera, lights, capsules) despawned");
            if let Some(o) = old.take() {
                tex.remove_image(&o);
            }
            *BAKED.lock().unwrap() = Some((tex.add_image(bevy_egui::EguiTextureHandle::Strong(img.clone())), *key, *cell, *k));
            *old = Some(img.clone());
            *st = None;
        }
        return;
    }
    if !WANT.swap(false, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let key = bake_key(&sim);
    if BAKED.lock().unwrap().is_some_and(|b| b.1 == key) {
        return;
    }
    let colour = |k: usize| match sim.part_colors.get(k).filter(|_| sim.ped_color == OWN_COLOURS) {
        Some(&[r, g, b]) => Color::srgb(r, g, b),
        None => part_color(sim.ped_color, delirium_data::nm_asset::PARTS.get(k).copied().unwrap_or(""), 1.0),
    };
    let top = |caps: &Caps| caps.iter().map(|c| c.0.z.max(c.1.z) + c.2).fold(0.0, f32::max);
    let people: Vec<_> = (0..BODIES().len()).map(|i| person_fig(&sim.base, i)).collect();
    *HEIGHTS.lock().unwrap() = people.iter().map(|p| p.1).collect();
    let tallest = people.iter().enumerate().filter(|(i, p)| *i != RANDOM && p.1 <= TALL).map(|(_, p)| top(&p.0)).fold(1.0, f32::max);
    let cell = tallest / FILL;
    let animals: Vec<_> = (0..rdr2frag::ANIMALS.len()).map(animal_fig).collect();
    let k = cell * FILL / animals.iter().map(top).fold(0.5, f32::max);
    let figs = people.into_iter().enumerate().map(|(i, p)| (i, 0, p.0, true))
        .chain(animals.into_iter().enumerate().map(|(i, a)| (i, 1, a.into_iter().map(|c| (c.0 * k, c.1 * k, c.2 * k, c.3, c.4)).collect(), false)));
    let img = images.add(Image::new_target_texture(2 * COLS as u32 * PX, 2 * PX, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, None));
    let layer = RenderLayers::layer(THUMB_LAYER);
    let o = Vec3::new(0.0, 0.0, 800.0);
    let eye = bake_eye();
    let (right, up) = (Vec3::Z.cross(eye).normalize(), eye.cross(Vec3::Z.cross(eye).normalize()));
    commands.spawn((Camera3d::default(), Camera { order: -10, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                    bevy::camera::RenderTarget::Image(img.clone().into()), Transform::from_translation(o + eye * 100.0).looking_at(o, Vec3::Z),
                    Projection::from(OrthographicProjection { scaling_mode: bevy::camera::ScalingMode::Fixed { width: 2.0 * COLS as f32 * cell, height: 2.0 * cell },
                                                              ..OrthographicProjection::default_3d() }),
                    Msaa::Sample4, bevy::light::AmbientLight { brightness: 400.0, ..default() }, layer.clone(), Baking));
    for (from, lux) in [(Vec3::new(1.0, -1.5, 2.0), 7000.0), (Vec3::new(-1.5, 0.5, 0.5), 2000.0)] {
        commands.spawn((DirectionalLight { illuminance: lux, shadow_maps_enabled: false, ..default() }, Transform::from_translation(from).looking_at(Vec3::ZERO, Vec3::Z),
                        layer.clone(), Baking));
    }
    for (i, row, caps, recolour) in figs {
        let w = if row == 0 { ASPECT * cell } else { cell };
        let feet = o + right * ((2.0 * i as f32 + 0.5) * w - COLS as f32 * cell) + up * ((0.5 - row as f32) * cell - (0.5 - BASE) * cell);
        for (a, b, r, k, c) in caps {
            let len = a.distance(b);
            commands.spawn((Mesh3d(meshes.add(Capsule3d::new(r, len))),
                            MeshMaterial3d(mats.add(StandardMaterial { base_color: if recolour { colour(k) } else { c }, perceptual_roughness: 0.7, ..default() })),
                            Transform::from_translation(feet + (a + b) / 2.0).with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a).normalize_or(Vec3::Y))),
                            layer.clone(), bevy::light::NotShadowCaster, Baking));
        }
    }
    *st = Some((img, key, cell, k, 0));
}

fn figure_tile(ui: &mut egui::Ui, label: &str, on: bool, cell: (usize, usize)) -> egui::Response {
    WANT.store(true, std::sync::atomic::Ordering::Relaxed);
    let baked = *BAKED.lock().unwrap();
    let person = cell.1 == 0;
    let height = HEIGHTS.lock().unwrap().get(cell.0).copied().filter(|_| person);
    let label = match height {
        _ if person && cell.0 == RANDOM => format!("{label}\nvaries"),
        Some(m) => format!("{label}\n{m:.2} m"),
        None => label.to_string(),
    };
    let draw = |p: &egui::Painter, r: egui::Rect| if let Some((id, _, h, k)) = baked {
        let aspect = if person { ASPECT } else { 1.0 };
        let s = r.height().min(r.width() / aspect);
        let pic = egui::Rect::from_center_size(r.center(), egui::vec2(s * aspect, s));
        let (du, v) = (0.5 / COLS as f32, 0.5);
        let uv = egui::Rect::from_min_size(egui::pos2(cell.0 as f32 * 2.0 * du * aspect, cell.1 as f32 * v), egui::vec2(du * aspect, v));
        let e = bake_eye();
        let guide = GUIDE * if person { 1.0 } else { k };
        let y = pic.bottom() - (BASE + guide * (1.0 - e.z * e.z).sqrt() / h) * s;
        p.hline(r.x_range(), y, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(130, 210, 255, 60)));
        p.image(id, pic, uv, egui::Color32::WHITE);
        if let Some(m) = height.filter(|m| *m > TALL) {
            let g = p.layout_no_wrap(format!("{m:.2} m"), egui::FontId::proportional(13.0), INK);
            let b = egui::Rect::from_min_size(r.min + egui::vec2(2.0, 1.0), egui::vec2(g.size().x + 22.0, g.size().y + 4.0));
            p.rect_filled(b, 6.0, egui::Color32::from_rgba_unmultiplied(20, 26, 36, 220));
            let (c, w) = (egui::pos2(b.left() + 9.0, b.center().y), 4.5);
            p.add(egui::Shape::convex_polygon(vec![c + egui::vec2(0.0, -w), c + egui::vec2(w, w * 0.8), c + egui::vec2(-w, w * 0.8)], HI, egui::Stroke::NONE));
            p.galley(egui::pos2(b.left() + 17.0, b.top() + 2.0), g, INK);
        }
    };
    if person { tile_sized(ui, &label, on, PERSON_H, draw) } else { tile(ui, &label, on, draw) }
}

fn holo_on_hover(r: egui::Response, key: usize) -> egui::Response {
    let now = std::time::Instant::now();
    let mut h = HOVER_HOLO.lock().unwrap();
    if r.hovered() && h.is_none_or(|h| h.1 < now + std::time::Duration::from_secs(1)) {
        *h = Some((key, now + std::time::Duration::from_millis(100)));
    }
    r
}

fn person_fig(base: &(Build, Vec<Transform>), i: usize) -> (Caps, f32) {
    let (b, f) = match i {
        RANDOM => {
            let (len, girth) = roll_body(&base.0);
            base.0.rolled(&base.1, &len, &girth)
        }
        FIGURE if FIGURE_BODY.lock().unwrap().is_none() => (base.0.clone(), base.1.clone()),
        _ => {
            let (b, f, _) = body_of(&base.0, &base.1, i);
            (b, f)
        }
    };
    if f.len() != b.parts.len() || f.len() <= HAND_R {
        return (vec![], 0.0);
    }
    let doll = Ragdoll::new(&b, &f, Vec3::ZERO);
    let side = Side::new(f.iter().map(|t| t.translation).collect());
    let turn = Quat::from_rotation_arc(side.fwd.with_z(0.0).normalize_or(Vec3::Y), Vec3::new(0.0, -1.0, 0.0).normalize());
    (feet_at_0(doll.capsules().zip(&b.parts).enumerate().map(|(k, (t, p))| {
        let a = t.rotation * Vec3::Y * p.half;
        (turn * (t.translation - a), turn * (t.translation + a), p.radius, k, Color::WHITE)
    }).collect()), b.height(&f))
}

fn feet_at_0(caps: Caps) -> Caps {
    let floor = caps.iter().map(|c| c.0.z.min(c.1.z) - c.2).fold(f32::MAX, f32::min);
    let mid = caps.iter().fold(Vec3::ZERO, |s, c| s + c.0 + c.1) / (2 * caps.len().max(1)) as f32;
    let o = mid.with_z(floor);
    caps.into_iter().map(|c| (c.0 - o, c.1 - o, c.2, c.3, c.4)).collect()
}

fn animal_fig(i: usize) -> Caps {
    let Some(&(_, text)) = rdr2frag::ANIMALS.get(i) else { return vec![] };
    let Ok(h) = rdr2frag::Frag::from_text(text).and_then(|f| horse::Horse::new(rdr2frag::BUILT_IN_XML, f, Vec3::ZERO, -0.5)) else { return vec![] };
    let caps = h.capsules();
    let at = |n: &str| h.build.parts.iter().position(|p| p.name.to_lowercase().contains(n)).map(|k| (caps[k].0 + caps[k].1) / 2.0);
    let fwd = at("head").zip(at("pelvis")).map_or(Vec3::Y, |(a, b)| (a - b).with_z(0.0).normalize_or(Vec3::Y));
    let turn = Quat::from_rotation_arc(fwd, Vec3::new(1.0, 0.45, 0.0).normalize());
    feet_at_0(caps.iter().zip(&h.build.parts).enumerate().filter(|(_, (_, p))| !((3..=6).contains(&i) && p.name.starts_with("Tail")))
        .map(|(k, (c, p))| (turn * c.0, turn * c.1, c.2, k, horse::coat(i, k, &p.name))).collect())
}

fn people(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    head(ui, "People");
    info(ui, "Pick a body, then Spawn: another Homer 2 m in front of the one you control.");
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        for (i, (_, n, h, m, _)) in BODIES().iter().enumerate() {
            let model = FIGURE_BODY.lock().unwrap().is_some();
            let tip = if i == FIGURE && !model {
                "Work in progress: pick a 3D model to be Homer's body (rigged or not: an unrigged one is auto-fitted)".to_string()
            } else if i == FIGURE {
                "Work in progress: Homer's body from a 3D model (picked: click again for another file)".to_string()
            } else if *h > 0.0 {
                format!("{n}: {h:.2} m, {m:.0} kg")
            } else {
                format!("{n} body")
            };
            let tile = figure_tile(ui, n, sim.spawn_body == i, (i, 0));
            if i == FIGURE {
                wip_on(ui, tile.rect);
            }
            if holo_on_hover(tile, i).on_hover_text(tip).clicked() {
                if i == FIGURE && (!model || sim.spawn_body == FIGURE) {
                    pick_model(PICK_BODY);
                }
                sim.spawn_body = i;
            }
        }
    });
    if sim.spawn_body == RANDOM {
        card(ui, randomness);
    }
    if sim.spawn_body == CUSTOM {
        card(ui, |ui| {
            ui.label(egui::RichText::new("Custom body · the next spawn's sizes").strong());
            ui.vertical(custom_ui);
        });
    }
    ui.add_space(6.0);
    let room = sim.bench.len() + 1 < HOMERS;
    if ui.add_enabled(room, egui::Button::new(egui::RichText::new(format!("+  Spawn {}", BODIES()[sim.spawn_body].1)).strong().size(16.0).color(BLACK)).fill(HI)
            .min_size(egui::vec2(ui.available_width(), 46.0)).corner_radius(10.0))
        .on_hover_text(format!("Another Homer of this body, 2 m in front of the one you control, balancing (Shift+N); up to {HOMERS}")).clicked() {
        *pressed = Some(format!("spawn {}", BODIES()[sim.spawn_body].0));
    }
    sub(ui, "The one you control");
    card(ui, |ui| {
        row(ui, "Body", |ui| {
            egui::ComboBox::from_id_salt("controlled body").width(170.0).height(420.0).selected_text(BODIES()[sim.body].1).show_ui(ui, |ui| {
                for (i, (_, n, h, m, _)) in BODIES().iter().enumerate() {
                    let tip = if sim.bench.is_empty() { format!("{h:.2} m, {m:.0} kg (restarts the scene)") } else { format!("{h:.2} m, {m:.0} kg: the Homer you control, where he stands") };
                    if ui.selectable_label(sim.body == i, *n).on_hover_text(tip).clicked() && sim.body != i {
                        if sim.bench.is_empty() {
                            sim.body = i;
                            sim.switch_to = Some((sim.scene.clone().unwrap_or_else(|| "room".into()), nm() == delirium::nmrs::MARGE));
                        } else {
                            *pressed = Some(format!("body {}", BODIES()[i].0));
                        }
                    }
                }
            }).response.on_hover_text("The controlled Homer's body. Alone in the room it restarts the scene; with others he changes where he stands.");
        });
        if ui.add(egui::Button::new("New random body").min_size(egui::vec2(0.0, PILL_H))).on_hover_text("Reroll: the Homer you control gets a fresh random body in place (Random, drawn anew)").clicked() {
            *pressed = Some(format!("body {}", BODIES()[RANDOM].0));
        }
        if sim.body == RANDOM {
            randomness(ui);
        }
        slide(ui, "Strength", &mut sim.agent.strength, 0.25, 3.0, "×").on_hover_text("His muscles' strength against the game's (1x).");
        ui.horizontal(|ui| {
            if ui.add_enabled(!sim.bench.is_empty(), egui::Button::new("Next (N)").min_size(egui::vec2(0.0, PILL_H))).on_hover_text("Control the next Homer: keys, pokes, drags, buttons and camera (N)").clicked() {
                *pressed = Some("next homer".into());
            }
            if ui.add_enabled(sim.can_remove(), egui::Button::new(egui::RichText::new("Remove him").color(RED)).min_size(egui::vec2(0.0, PILL_H))).on_hover_text("Remove the Homer you control, a spawned one; control passes to the next (Ctrl+N)").clicked() {
                *pressed = Some("remove homer".into());
            }
        });
    });
    if !sim.bench.is_empty() {
        sub(ui, "In the room");
        let mut all: Vec<(usize, usize, bool)> = std::iter::once((sim.homer_id, sim.body, true)).chain(sim.bench.iter().map(|b| (b.homer_id, b.body, false))).collect();
        all.sort();
        for (id, body, on) in all {
            ui.horizontal(|ui| {
                ui.label(format!("Homer {} · {}", id + 1, BODIES()[body].1));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if on {
                        ui.label(egui::RichText::new("controlled").color(HI));
                    } else {
                        if ui.add(egui::Button::new("Remove").min_size(egui::vec2(0.0, PILL_H))).on_hover_text(format!("Take Homer {} away", id + 1)).clicked() { *pressed = Some(format!("unspawn {}", id + 1)); }
                        if ui.add(egui::Button::new("Control").min_size(egui::vec2(0.0, PILL_H))).on_hover_text(format!("Control Homer {}", id + 1)).clicked() { *pressed = Some(format!("control {}", id + 1)); }
                    }
                });
            });
        }
    }
    info(ui, &format!("Up to {HOMERS}. N, or a click on one, controls it: the keys, pokes, drags, buttons and camera follow. Shift+N spawns, Ctrl+N removes the controlled one. Each costs a physics step."));
}

fn randomness(ui: &mut egui::Ui) {
    let mut on = SYMMETRIC.load(std::sync::atomic::Ordering::Relaxed);
    if switch(ui, &mut on, "Symmetric", "Random bodies alike on both sides; off, each side drawn on its own").changed() {
        SYMMETRIC.store(on, std::sync::atomic::Ordering::Relaxed);
    }
    let mut v = f32::from_bits(VARIANCE.load(std::sync::atomic::Ordering::Relaxed));
    let ((l0, l1), _) = spread(v);
    if ui.add(egui::Slider::new(&mut v, 1.0..=4.0).text("Variance").suffix("x")).on_hover_text(format!("How far a random body strays from his: bones {l0:.2}x to {l1:.2}x his length (1x: his own; 4x: 0.2x to 4x). The next roll's")).changed() {
        VARIANCE.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
}

fn animals(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State) {
    head(ui, "Animals");
    let linked = std::path::Path::new(&sim.rdr2_dir).join("RDR2.exe").is_file();
    info(ui, if linked { "Horse: RDR2's own horse body (your RDR2, Play > Bonus). The others: the Farm Animals pack." } else { "The Farm Animals pack. Link your RDR2 (Play > Bonus) for RDR2's own horse body." });
    let b = sim.horse_body;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        for (i, a) in rdr2frag::ANIMALS.iter().enumerate() {
            let name = if i == 0 && linked { "Horse (RDR2)".to_string() } else { a.0.to_string() };
            if holo_on_hover(figure_tile(ui, &name, b as usize == i + 2, (i, 1)), HOLO_ANIMAL + i).on_hover_text(format!("{name}: the next animal's body")).clicked() {
                *pressed = Some(format!("horse_body {}", i + 2));
            }
        }
    });
    ui.add_space(6.0);
    let next = rdr2frag::ANIMALS[(b as usize).saturating_sub(2).min(rdr2frag::ANIMALS.len() - 1)].0;
    if ui.add(egui::Button::new(egui::RichText::new(format!("+  Spawn {next}")).strong().size(16.0).color(BLACK)).fill(HI)
            .min_size(egui::vec2(ui.available_width(), 46.0)).corner_radius(10.0))
        .on_hover_text("Another animal of the picked body beside Homer, clear of the others; the ones out stay").clicked() {
        *pressed = Some("animal add".into());
    }
    if sim.horse.is_some() {
        sub(ui, "In the room");
        let name = |h: &horse::Horse| rdr2frag::ANIMALS[h.animal.min(rdr2frag::ANIMALS.len() - 1)].0;
        let mut all: Vec<(u32, &str, bool)> = sim.horse.iter().map(|h| (sim.horse_id, name(h), true)).chain(sim.herd.iter().map(|(i, h)| (*i, name(h), false))).collect();
        all.sort();
        for (id, n, on) in all {
            ui.horizontal(|ui| {
                ui.label(format!("{n} {id}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("Remove").color(RED)).min_size(egui::vec2(0.0, PILL_H))).on_hover_text(format!("Take {n} {id} away")).clicked() {
                        *pressed = if on { Some("horse".into()) } else { Some(format!("animal off {id}")) };
                    }
                    if on {
                        ui.label(egui::RichText::new("selected").color(HI)).on_hover_text("The settings below, the camera, pokes and scripts act on this one; click another animal (or Select) to change");
                    } else if ui.add(egui::Button::new("Select").min_size(egui::vec2(0.0, PILL_H))).on_hover_text(format!("{n} {id}: the one the settings, camera and scripts act on")).clicked() {
                        *pressed = Some(format!("animal pick {id}"));
                    }
                });
            });
        }
    }
    sub(ui, "Brain");
    let l = sim.horse_loalco;
    *pressed = pressed.take().or(tiles(ui, "hbrain", &[("horse_brain 0", !l, "Balances (RDR2 core)".into()), ("horse_brain 1", l, "Held pose".into())]));
    let mut freeze = horse::FREEZE.load(std::sync::atomic::Ordering::Relaxed);
    if switch(ui, &mut freeze, "Freeze when standing still", "On: after it spawns, after Reset pose and after a second standing still, the animal holds its pose until touched. Off: it balances on its own the whole time.").changed() {
        horse::FREEZE.store(freeze, std::sync::atomic::Ordering::Relaxed);
    }
    if pop_head(ui, st, POP_ANIMAL, "Balance readout") {
        animal_readout(ui, sim);
    }
}

pub fn animal_readout(ui: &mut egui::Ui, sim: &mut Sim) {
    match sim.horse.as_ref() {
        Some(h) => {
            ui.label(egui::RichText::new(h.state()).monospace().color(HI));
            if let Some(f) = h.doll.frames().next() {
                ui.label(egui::RichText::new(format!("pelvis {:.2} {:.2} {:.2} m", f.translation.x, f.translation.y, f.translation.z)).monospace().color(DIM));
            }
        }
        None => info(ui, "No animal out: Spawn animal (Cast > Animals)."),
    }
}

fn appearance(ui: &mut egui::Ui, sim: &mut Sim, st: &mut State) {
    head(ui, "Appearance");
    let tip = |k: &str| TIPS.iter().find(|t| t.0 == k).map_or("", |t| t.1);
    sub(ui, "Body colour");
    let shown: Vec<usize> = (0..PED_COLORS.len()).filter(|&i| i < 3 || i == OWN_COLOURS).collect();
    let names: Vec<&str> = shown.iter().map(|&i| PED_COLORS[i].1).collect();
    let tips: Vec<&str> = shown.iter().map(|&i| tip(PED_COLORS[i].0)).collect();
    if let Some(k) = pills(ui, &names, shown.iter().position(|&i| i == sim.ped_color).unwrap_or(usize::MAX), Some(&tips)) {
        sim.ped_color = shown[k];
    }
    if sim.ped_color == OWN_COLOURS {
        ui.horizontal(|ui| {
            let mut all = sim.part_colors[0];
            if ui.color_edit_button_rgb(&mut all).on_hover_text("Every part this colour").changed() {
                sim.part_colors = [all; 21];
            }
            ui.label("All of him");
        });
        if pop_head(ui, st, POP_COLOURS, "Each part") {
            part_colours(ui, sim);
        }
    }
    sub(ui, "Bodies");
    switch(ui, &mut sim.bone_skin, "Skeleton body", "Play as the skeleton: the Homers drawn as their bones.");
    let mut see = 1.0 - sim.body_alpha;
    if ui.add_enabled(!sim.clean, egui::Slider::new(&mut see, 0.0..=0.9).text("See-through").custom_formatter(|x, _| if x <= 0.0 { "solid".into() } else { format!("{:.0}%", x * 100.0) }))
        .on_hover_text("How much you see through the bodies. Solid hides the skeleton inside; Clean view makes them solid too.").changed() {
        sim.body_alpha = 1.0 - see;
    }
    let mut see = 1.0 - sim.hero_alpha;
    if ui.add_enabled(!sim.clean, egui::Slider::new(&mut see, 0.0..=0.9).text("Hero see-through").custom_formatter(|x, _| if x <= 0.0 { "solid".into() } else { format!("{:.0}%", x * 100.0) }))
        .on_hover_text("How much you see through the Hero.").changed() {
        sim.hero_alpha = 1.0 - see;
    }
    sub(ui, "Show");
    switch(ui, &mut sim.eyes, "Eyes", tip("#eyes"));
    switch(ui, &mut sim.brows, "Eyebrows", tip("#brows"));
    switch(ui, &mut sim.skel, "Skeleton lines", tip("#skel"));
    if sim.skel {
        row(ui, "Style", |ui| {
            egui::ComboBox::from_id_salt("skel_style").width(150.0).selected_text(SKEL_STYLES[sim.skel_style as usize]).show_ui(ui, |ui| {
                for k in 0..SKEL_STYLES.len() as u8 {
                    ui.selectable_value(&mut sim.skel_style, k, SKEL_STYLES[k as usize]);
                }
            }).response.on_hover_text("Lines: white bones, coloured joints. Muscles: solid red bones and yellow joints. Stick: all yellow. Simple: bones and small joint dots, lighter with a crowd. Sticks only: just the bones' lines, the lightest.");
        });
    }
    switch(ui, &mut sim.clean, "Clean view", tip("#clean"));
}

pub fn part_colours(ui: &mut egui::Ui, sim: &mut Sim) {
    egui::Grid::new("part colours").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
        for (k, name) in delirium_data::nm_asset::PARTS.iter().enumerate() {
            ui.color_edit_button_rgb(&mut sim.part_colors[k]);
            ui.label(name.replace('_', " "));
            if k % 2 == 1 {
                ui.end_row();
            }
        }
    });
}
