use super::*;

pub mod cast;
mod lab;
mod play;
mod reactions;
mod scene;
mod settings;

pub const PANEL: egui::Color32 = egui::Color32::from_rgb(20, 25, 33);
pub const WINDOW: egui::Color32 = egui::Color32::from_rgb(24, 29, 38);
pub const EXTREME: egui::Color32 = egui::Color32::from_rgb(15, 19, 26);
pub const BTN: egui::Color32 = egui::Color32::from_rgb(34, 42, 55);
pub const HOVER: egui::Color32 = egui::Color32::from_rgb(49, 65, 88);
pub const STROKE: egui::Color32 = egui::Color32::from_rgb(58, 68, 83);
pub const BLACK: egui::Color32 = egui::Color32::from_rgb(0, 0, 0);
pub const RED: egui::Color32 = egui::Color32::from_rgb(255, 110, 160);
pub const LIVE: egui::Color32 = egui::Color32::from_rgb(128, 218, 177);
pub const AMBER: egui::Color32 = egui::Color32::from_rgb(255, 190, 90);

pub fn wip(ui: &mut egui::Ui, why: &str) -> egui::Response {
    ui.label(egui::RichText::new(" WIP ").small().strong().color(BLACK).background_color(AMBER)).on_hover_text(format!("Work in progress: {why}"))
}

pub fn wip_on(ui: &egui::Ui, r: egui::Rect) {
    let g = ui.painter().layout_no_wrap("WIP".into(), egui::FontId::proportional(13.0), BLACK);
    let pill = egui::Rect::from_min_size(r.min + egui::vec2(4.0, 4.0), g.size() + egui::vec2(8.0, 2.0));
    ui.painter().rect_filled(pill, 4.0, AMBER);
    ui.painter().galley(pill.min + egui::vec2(4.0, 1.0), g, BLACK);
}
#[allow(dead_code)]
const CONTRAST_PAIRS: [(&str, &str); 16] = [("INK", "PANEL"), ("INK", "WINDOW"), ("INK", "BTN"), ("INK", "HOVER"), ("INK", "ON"), ("INK", "EXTREME"),
    ("DIM", "PANEL"), ("DIM", "WINDOW"), ("DIM", "BTN"), ("DIM", "EXTREME"), ("HI", "PANEL"), ("HI", "WINDOW"), ("BLACK", "HI"), ("RED", "BTN"), ("LIVE", "PANEL"), ("HI", "BTN")];

const STRIPE: [egui::Color32; 5] = [egui::Color32::from_rgb(255, 140, 100), egui::Color32::from_rgb(250, 220, 110), egui::Color32::from_rgb(120, 225, 175), HI, egui::Color32::from_rgb(185, 155, 250)];

pub const PILL_H: f32 = 34.0;
pub const TAB_H: f32 = 44.0;
pub const ROW_H: f32 = 36.0;
pub const TILE_H: f32 = 104.0;
const TILE_W: f32 = 84.0;

pub const VIEW: u16 = 0x8000;
pub const TABS: [(&str, &str, &[&str]); 7] = [
    ("Scene", "Where: the location, how the room looks, crowds and taxis", &["Location", "Look", "Crowds"]),
    ("Cast", "Who: spawn people and animals, the one you control, how they look", &["People", "Animals", "Appearance"]),
    ("Play", "Play as the Hero, weapons, throwing and pushing, the bonus games", &["Hero", "Weapons", "Throw", "Bonus"]),
    ("Reactions", "How Homer reacts: reactions to start, damage, the reaction engine's parameters", &["Reactions", "Damage", "Parameters"]),
    ("Settings", "Graphics, water, display, sound, camera and interface size", &["Graphics", "Water", "Display", "Audio", "Camera & UI"]),
    ("Lab", "Experimental: new movement, training, extra reactions, inspection tools", &["Movement", "Reactions", "Inspect", "RDR2"]),
    ("Help", "Mouse and keyboard, first steps", &["Help"]),
];
pub const HELP: u8 = 6;
const ICONS: [&str; 7] = ["🗺", "👥", "🎮", "⚡", "⚙", "🔧", "?"];

pub const POP_COLOURS: usize = 0;
pub const POP_TRAINING: usize = 1;
pub const POP_PARAMS: usize = 2;
pub const POP_ANIMAL: usize = 3;
const POPS: [&str; 4] = ["Each part's colour", "Training", "Parameters", "Animal balance"];

#[derive(Default)]
pub struct State {
    page: [u8; 7],
    pub pop: [bool; 4],
    last_scale: f32,
    strip_shut: bool,
    near: Option<f64>,
    away: Option<f64>,
    slid: bool,
    key_open: bool,
}

const EDGE: f32 = 16.0;
pub const PEEK: f32 = 60.0;
const OPEN_WAIT: f64 = 0.15;
const CLOSE_WAIT: f64 = 2.0;
pub const DOCK_KEY: egui::Key = egui::Key::M;

pub fn auto_dock(ctx: &egui::Context, area: egui::Rect, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State, joints_open: &mut bool) {
    let now = ctx.input(|i| i.time);
    let (p, busy) = ctx.input(|i| (i.pointer.hover_pos(), i.pointer.any_down() || i.modifiers.ctrl));
    if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(DOCK_KEY)) {
        st.key_open = !st.key_open;
    }
    let peek = ctx.animate_bool_with_time(egui::Id::new("dock peek"), !st.slid && !busy && p.is_some_and(|p| p.x < area.left() + PEEK && area.y_range().contains(p.y)), 0.12);
    if !st.slid {
        let edge = p.is_some_and(|p| p.x < area.left() + EDGE + 22.0 * peek && area.y_range().contains(p.y));
        st.near = if edge && !busy { st.near.or(Some(now)) } else { None };
        st.slid = st.near.is_some_and(|t| now - t >= OPEN_WAIT);
        st.away = None;
    } else {
        let inside = p.is_some_and(|p| p.x < area.left() + sim.dock_w + 8.0) || ctx.dragged_id().is_some() || egui::Popup::is_any_open(ctx);
        st.away = if inside { None } else { st.away.or(Some(now)) };
        if st.away.is_some_and(|t| now - t >= CLOSE_WAIT) {
            (st.slid, st.near) = (false, None);
        }
    }
    let t = ctx.animate_bool_with_time(egui::Id::new("auto dock"), st.slid || st.key_open, 0.18);
    if t < 0.5 {
        edge_strip(ctx, area, sim, pressed, st, peek);
    }
    if t > 0.0 {
        let w = sim.dock_w;
        let at = area.min - egui::vec2((1.0 - t) * w, 0.0);
        egui::Area::new("auto dock".into()).fixed_pos(at).show(ctx, |ui| {
            egui::Frame::side_top_panel(&ctx.global_style()).inner_margin(14.0).show(ui, |ui| {
                let size = egui::vec2(w - 28.0, area.height() - 28.0);
                (ui.set_min_size(size), ui.set_max_size(size));
                dock(ui, sim, pressed, st, joints_open);
            });
        });
        if t >= 1.0 {
            dock_grip(ctx, egui::Rect::from_min_size(at, egui::vec2(w, area.height())), &mut sim.dock_w);
        }
    }
}

fn edge_strip(ctx: &egui::Context, area: egui::Rect, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State, peek: f32) {
    let edge = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, "dock edge".into()));
    for (i, color) in STRIPE.into_iter().enumerate() {
        let h = area.height() / 5.0;
        edge.rect_filled(egui::Rect::from_min_size(area.min + egui::vec2(0.0, h * i as f32), egui::vec2(8.0, h)), 0.0, color.gamma_multiply(0.9));
    }
    if peek > 0.0 {
        let tab = egui::Rect::from_min_size(egui::pos2(area.left(), area.center().y - 36.0), egui::vec2(8.0 + 22.0 * peek, 72.0));
        edge.rect_filled(tab, egui::CornerRadius { ne: 8, se: 8, ..Default::default() }, WINDOW);
        edge.rect_stroke(tab, egui::CornerRadius { ne: 8, se: 8, ..Default::default() }, egui::Stroke::new(1.0, HI.gamma_multiply(peek)), egui::StrokeKind::Inside);
    }
    let c = egui::pos2(area.left() + 18.0 + 2.0 * peek, area.center().y);
    if peek < 1.0 {
        edge.rect_filled(egui::Rect::from_center_size(c, egui::vec2(18.0, 30.0)), 5.0, EXTREME.gamma_multiply(0.55 * (1.0 - peek)));
    }
    let s = egui::Stroke::new(2.5, HI.gamma_multiply(0.5 + 0.5 * peek));
    edge.line_segment([c + egui::vec2(-4.0, -9.0), c + egui::vec2(4.0, 0.0)], s);
    edge.line_segment([c + egui::vec2(4.0, 0.0), c + egui::vec2(-4.0, 9.0)], s);
    let who = if nm() == delirium::nmrs::MARGE { "Marge" } else { "Homer" };
    let a = &sim.agent;
    let state = [(a.fallen, "fallen"), (a.balancing(), "balancing"), (!a.doll.awake(), "asleep")].iter().find(|s| s.0).map_or("Ready", |s| s.1);
    let hp = if sim.god { 1.0 } else { (sim.health / 200.0).clamp(0.0, 1.0) };
    egui::Area::new("dock strip".into()).fixed_pos(area.min + egui::vec2(14.0, 8.0)).show(ctx, |ui| {
        egui::Frame::new().fill(WINDOW).corner_radius(8.0).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.set_width(200.0);
            let (stripe, _) = ui.allocate_exact_size(egui::vec2(200.0, 4.0), egui::Sense::hover());
            for (i, color) in STRIPE.into_iter().enumerate() {
                let x = stripe.left() + stripe.width() * i as f32 / 5.0;
                ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, stripe.top()), egui::vec2(stripe.width() / 5.0, 4.0)), 0.0, color);
            }
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(who).strong());
                ui.label(egui::RichText::new(if sim.dead { "dead" } else { state }).color(DIM));
            });
            let health = if sim.god { "Health: unlimited".to_string() } else { format!("Health: {:.0} / 200", sim.health) };
            ui.add(egui::ProgressBar::new(hp).desired_height(6.0).corner_radius(3.0).fill(egui::Color32::from_rgb(93, 194, 159))).on_hover_text(health);
            ui.label(egui::RichText::new(format!("Panel: left edge or {}", DOCK_KEY.name())).small().color(DIM));
        });
    });
    egui::Area::new("quick play edge".into()).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(egui::pos2(area.left() + 14.0, area.bottom() - 8.0)).show(ctx, |ui| {
        egui::Frame::new().fill(WINDOW).corner_radius(10.0).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
            ui.set_width(360.0);
            let mut open = !st.strip_shut;
            play::strip(ui, sim, pressed, &mut open);
            st.strip_shut = !open;
        });
    });
}

pub const fn code(tab: u8, page: u8) -> u16 {
    VIEW | (tab as u16) << 4 | page as u16
}

pub fn from_old(bits: u16) -> (u8, u8) {
    match active_tab(bits).trailing_zeros() {
        0 => (3, 0),
        1 => (2, 1),
        2 => (0, 0),
        3 => (1, 0),
        4 => (4, 0),
        5 => (HELP, 0),
        6 => (5, 2),
        8 => (4, 2),
        9 => (3, 2),
        _ => (3, 0),
    }
}

pub fn view(tab: u16) -> (u8, u8) {
    if tab & VIEW == 0 {
        return from_old(tab);
    }
    let t = ((tab >> 4) & 0xf).min(TABS.len() as u16 - 1) as usize;
    (t as u8, ((tab & 0xf) as usize).min(TABS[t].2.len() - 1) as u8)
}

pub fn named(n: &str) -> u16 {
    let key = |s: &str| s.to_lowercase().replace([' ', '&'], "");
    let (t, p) = n.split_once('/').unwrap_or((n, ""));
    let Some(ti) = TABS.iter().position(|x| key(x.0) == key(t)) else { return code(1, 0) };
    code(ti as u8, TABS[ti].2.iter().position(|x| key(x) == key(p)).unwrap_or(0) as u8)
}

pub fn theme(ctx: &egui::Context, sim: &mut Sim, st: &mut State) {
    if (sim.ui_scale - st.last_scale).abs() > 1e-4 {
        ctx.set_zoom_factor(sim.ui_scale);
    } else if (ctx.zoom_factor() - sim.ui_scale).abs() > 1e-4 {
        sim.ui_scale = ctx.zoom_factor().clamp(UI_SCALE.0, UI_SCALE.1);
        ctx.set_zoom_factor(sim.ui_scale);
    }
    st.last_scale = sim.ui_scale;
    ctx.global_style_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        s.text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
        s.text_styles.insert(egui::TextStyle::Heading, egui::FontId::proportional(21.0));
        s.text_styles.insert(egui::TextStyle::Small, egui::FontId::proportional(13.0));
        s.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 8.0);
        s.spacing.interact_size.y = 34.0;
        s.spacing.slider_width = 150.0;
        s.spacing.combo_height = 400.0;
        s.spacing.scroll = egui::style::ScrollStyle::solid();
        let mut v = egui::Visuals::dark();
        v.override_text_color = Some(INK);
        v.window_fill = WINDOW;
        v.panel_fill = PANEL;
        v.faint_bg_color = egui::Color32::from_rgb(27, 33, 44);
        v.window_corner_radius = egui::CornerRadius::same(10);
        v.window_stroke = egui::Stroke::new(1.0_f32, STROKE);
        v.extreme_bg_color = EXTREME;
        v.selection.bg_fill = ON;
        v.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);
        v.hyperlink_color = HI;
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = egui::CornerRadius::same(6);
            w.fg_stroke = egui::Stroke::new(1.0_f32, INK);
        }
        v.widgets.inactive.weak_bg_fill = BTN;
        v.widgets.inactive.bg_fill = BTN;
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(65, 77, 95));
        v.widgets.hovered.weak_bg_fill = HOVER;
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, HI);
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, INK);
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, STROKE);
        s.visuals = v;
    });
}

pub const UI_SCALE: (f32, f32) = (0.75, 2.0);

pub fn head(ui: &mut egui::Ui, t: &str) {
    ui.add_space(4.0);
    ui.label(section_title(t));
}

pub fn sub(ui: &mut egui::Ui, t: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(t).strong().size(16.0).color(HI));
}

pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new().fill(WINDOW).stroke(egui::Stroke::new(1.0, STROKE)).corner_radius(10.0).inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        }).inner
}

pub fn fold(ui: &mut egui::Ui, name: &str, open: bool, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(4.0);
    egui::CollapsingHeader::new(egui::RichText::new(name).strong().size(16.0)).default_open(open).show_unindented(ui, add);
}

pub fn switch_row(ui: &mut egui::Ui, on: bool, name: &str, tip: &str) -> egui::Response {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, ROW_H), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, name));
    if ui.is_rect_visible(r) {
        let p = ui.painter();
        if resp.hovered() || resp.has_focus() {
            p.rect_filled(r, 6.0, BTN);
        }
        if resp.has_focus() {
            p.rect_stroke(r, 6.0, egui::Stroke::new(1.5, HI), egui::StrokeKind::Inside);
        }
        let ink = if ui.is_enabled() { INK } else { DIM };
        p.text(r.left_center() + egui::vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(15.0), ink);
        let t = egui::Rect::from_center_size(egui::pos2(r.right() - 30.0, r.center().y), egui::vec2(42.0, 22.0));
        let k = ui.ctx().animate_bool_responsive(resp.id, on);
        p.rect_filled(t, 11.0, if on { ON } else { egui::Color32::from_rgb(65, 77, 95) });
        p.circle_filled(egui::pos2(egui::lerp(t.left() + 11.0..=t.right() - 11.0, k), t.center().y), 8.0, if ui.is_enabled() { INK } else { DIM });
    }
    let resp = resp.on_hover_text(tip);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn switch(ui: &mut egui::Ui, on: &mut bool, name: &str, tip: &str) -> egui::Response {
    let mut r = switch_row(ui, *on, name, tip);
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    r
}

pub fn switch_cmd(ui: &mut egui::Ui, key: &str, on: bool, name: &str) -> Option<String> {
    let tip = TIPS.iter().find(|t| t.0 == key).map_or(name, |t| t.1);
    switch_row(ui, on, name, tip).clicked().then(|| key.to_string())
}

pub fn onoff(b: &str, on: bool) -> String {
    format!("{b}: {}", if on { "On" } else { "Off" })
}

pub fn mono(s: String) -> egui::RichText {
    egui::RichText::new(s).monospace()
}

pub fn switch_if(ui: &mut egui::Ui, enabled: bool, on: &mut bool, name: &str, tip: &str, why: &str) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| switch(ui, on, name, tip)).inner.on_disabled_hover_text(why)
}

pub fn switches(ui: &mut egui::Ui, items: &[(&str, bool, String)]) -> Option<String> {
    let mut hit = None;
    for (k, on, name) in items {
        hit = hit.or(switch_cmd(ui, k, *on, name));
    }
    hit
}

pub fn pills(ui: &mut egui::Ui, names: &[&str], now: usize, tips: Option<&[&str]>) -> Option<usize> {
    let mut hit = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
        for (k, name) in names.iter().enumerate() {
            let on = now == k;
            let text = egui::RichText::new(*name).size(14.5).color(if on { BLACK } else { INK });
            let b = egui::Button::new(text).fill(if on { HI } else { BTN }).corner_radius(17.0).min_size(egui::vec2(0.0, PILL_H)).selected(on);
            let r = ui.add(b).on_hover_text(tips.and_then(|t| t.get(k)).copied().unwrap_or(name));
            if r.clicked() {
                hit = Some(k);
            }
        }
    });
    hit
}

pub fn tiles(ui: &mut egui::Ui, id: &str, items: &[(&str, bool, String)]) -> Option<String> {
    tiles_w(ui, id, items, 150.0)
}

pub fn tiles_w(ui: &mut egui::Ui, id: &str, items: &[(&str, bool, String)], w: f32) -> Option<String> {
    let mut hit = None;
    let per = ((ui.available_width() + 8.0) / (w + 8.0)).floor().max(1.0) as usize;
    let w = (ui.available_width() - 8.0 * (per - 1) as f32) / per as f32;
    egui::Grid::new(id).spacing([8.0, 8.0]).min_col_width(w).max_col_width(w).show(ui, |ui| {
        for (i, (b, on, label)) in items.iter().enumerate() {
            let tip = TIPS.iter().find(|t| t.0 == *b).map_or(label.as_str(), |t| t.1);
            if big(ui, label, *on, w).on_hover_text(tip).clicked() {
                hit = Some(b.to_string());
            }
            if i % per == per - 1 {
                ui.end_row();
            }
        }
    });
    hit
}

pub fn big(ui: &mut egui::Ui, label: &str, on: bool, w: f32) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding.x = 4.0;
        ui.add_sized([w, 44.0], egui::Button::new(egui::RichText::new(label).color(INK)).selected(on).wrap().corner_radius(8.0))
    }).inner
}

pub fn thumb(ui: &mut egui::Ui, label: &str, key: &str, on: bool, make: impl FnOnce() -> Vec<([Vec3; 3], Color)>) -> egui::Response {
    tile(ui, label, on, |p, pic| editor::pic(p, pic, key, make))
}

pub fn tile(ui: &mut egui::Ui, label: &str, on: bool, draw: impl FnOnce(&egui::Painter, egui::Rect)) -> egui::Response {
    tile_sized(ui, label, on, TILE_H, draw)
}

pub fn tile_sized(ui: &mut egui::Ui, label: &str, on: bool, h: f32, draw: impl FnOnce(&egui::Painter, egui::Rect)) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(TILE_W, h), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, label));
    if ui.is_rect_visible(r) {
        let p = ui.painter_at(r.expand(2.0));
        let lit = resp.hovered() || resp.has_focus();
        let fill = if resp.is_pointer_button_down_on() { HOVER } else if lit { egui::Color32::from_rgb(40, 52, 70) } else if on { egui::Color32::from_rgb(31, 46, 68) } else { EXTREME };
        let stroke = if on { egui::Stroke::new(2.0, HI) } else if lit { egui::Stroke::new(1.5, HI) } else { egui::Stroke::new(1.0, STROKE) };
        p.rect(r, 10.0, fill, stroke, egui::StrokeKind::Inside);
        let g = p.layout(label.to_string(), egui::FontId::proportional(13.0), if on { HI } else { INK }, TILE_W - 8.0);
        let pic = egui::Rect::from_min_max(r.min + egui::vec2(5.0, 6.0), egui::pos2(r.max.x - 5.0, r.max.y - 10.0 - g.size().y));
        draw(&p, pic);
        p.galley(egui::pos2(r.center().x - g.size().x / 2.0, r.max.y - 6.0 - g.size().y), g, INK);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn row<R>(ui: &mut egui::Ui, name: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(96.0, PILL_H), egui::Sense::hover());
        ui.painter().text(r.left_center(), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(14.0), DIM);
        add(ui)
    }).inner
}

pub fn pop_head(ui: &mut egui::Ui, st: &mut State, k: usize, name: &str) -> bool {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(name).strong().size(16.0).color(HI));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let out = st.pop[k];
            if ui.add(egui::Button::new(if out { "⤓ Dock back" } else { "🗗 Pop out" }).min_size(egui::vec2(0.0, PILL_H)))
                .on_hover_text(if out { "Put it back in the panel" } else { "Open it in its own window over the room, to watch while you play (close the window to put it back)" }).clicked() {
                st.pop[k] = !out;
            }
        });
    });
    if st.pop[k] {
        info(ui, "Out in its own window: drag it where you like.");
    }
    !st.pop[k]
}

pub static PICKED: std::sync::Mutex<Option<(u8, String)>> = std::sync::Mutex::new(None);
pub const PICK_BODY: u8 = 0;
pub const PICK_IMPORT: u8 = 1;
static PICKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const MODEL_EXTS: [&str; 9] = ["glb", "gltf", "obj", "fbx", "usd", "usdz", "usdc", "usda", "ply"];

pub fn pick_model(why: u8) {
    if PICKING.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    std::thread::spawn(move || {
        #[cfg(windows)]
        {
            let title = if why == PICK_BODY { "Pick a 3D model to be Homer's body (rigged or not: an unrigged one is auto-fitted)" } else { "Import a model or splat" };
            if let Some(p) = rfd::FileDialog::new().set_title(title).add_filter("3D models", &MODEL_EXTS).add_filter("All files", &["*"]).pick_file() {
                *PICKED.lock().unwrap() = Some((why, p.to_string_lossy().into_owned()));
            }
        }
        #[cfg(not(windows))]
        let _ = (why, MODEL_EXTS);
        PICKING.store(false, std::sync::atomic::Ordering::Relaxed);
    });
}

pub const DOCK_W: (f32, f32, f32) = (340.0, 700.0, 420.0);

pub fn dock_grip(ctx: &egui::Context, r: egui::Rect, w: &mut f32) {
    let hit = egui::Rect::from_x_y_ranges(r.right() - 4.0..=r.right() + 4.0, r.y_range());
    let resp = egui::Area::new("dock grip".into()).fixed_pos(hit.min).order(egui::Order::Foreground).show(ctx, |ui| {
        let resp = ui.allocate_exact_size(hit.size(), egui::Sense::drag()).1.on_hover_text("Drag: the panel wider or narrower");
        let lit = resp.hovered() || resp.dragged();
        let p = ui.painter();
        p.vline(r.right(), r.y_range(), egui::Stroke::new(if lit { 2.0 } else { 1.0 }, if lit { HI } else { STROKE }));
        for k in -1..=1 {
            p.circle_filled(egui::pos2(r.right(), r.center().y + k as f32 * 7.0), 2.0, if lit { HI } else { DIM });
        }
        resp
    }).inner;
    if resp.hovered() || resp.dragged() {
        ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if resp.dragged() {
        *w = (*w + resp.drag_delta().x).clamp(DOCK_W.0, DOCK_W.1);
    }
}

pub fn dock(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State, joints_open: &mut bool) {
    let (tab, page) = view(sim.tab);
    st.page[tab as usize] = page;
    status_card(ui, sim);
    ui.add_space(6.0);
    let w = (ui.available_width() - 12.0) / 3.0;
    egui::Grid::new("tabs").num_columns(3).spacing([6.0, 6.0]).show(ui, |ui| {
        for (i, (name, tip, _)) in TABS.iter().enumerate().take(6) {
            let on = tab as usize == i;
            let text = egui::RichText::new(format!("{}  {name}", ICONS[i])).size(15.0).strong().color(if on { BLACK } else { INK });
            let b = egui::Button::new(text).fill(if on { HI } else { BTN }).corner_radius(8.0).selected(on);
            if ui.add_sized([w, TAB_H], b).on_hover_text(*tip).clicked() {
                sim.tab = code(i as u8, st.page[i]);
            }
            if i % 3 == 2 {
                ui.end_row();
            }
        }
    });
    let (tab, page) = view(sim.tab);
    let pages = TABS[tab as usize].2;
    if pages.len() > 1 {
        ui.add_space(4.0);
        if let Some(k) = pills(ui, pages, page as usize, None) {
            sim.tab = code(tab, k as u8);
        }
    }
    ui.add_space(2.0);
    ui.separator();
    let (tab, page) = view(sim.tab);
    if tab != 2 {
        egui::Panel::bottom("quick play").frame(egui::Frame::new().fill(WINDOW).inner_margin(egui::Margin::symmetric(8, 6)).corner_radius(10.0))
            .show(ui, |ui| {
                let mut open = !st.strip_shut;
                play::strip(ui, sim, pressed, &mut open);
                st.strip_shut = !open;
            });
    }
    egui::ScrollArea::vertical().id_salt(("page", sim.tab)).auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width() - ui.spacing().scroll.allocated_width());
        match tab {
            0 => scene::page(ui, sim, pressed, page),
            1 => cast::page(ui, sim, pressed, st, page),
            2 => play::page(ui, sim, pressed, page),
            3 => reactions::page(ui, sim, pressed, st, page),
            4 => settings::page(ui, sim, pressed, page),
            5 => lab::page(ui, sim, pressed, st, page, joints_open),
            _ => help(ui, sim),
        }
        ui.add_space(24.0);
    });
}

fn status_card(ui: &mut egui::Ui, sim: &mut Sim) {
    let a = &sim.agent;
    let state = [(a.balancing(), "balancing"), (a.fallen, "fallen"), (a.settled(), "settled"), (a.relaxing(), "relaxing"), (a.catching(), "catching his fall"),
                 (a.rolled_up(), "rolled up"), (!a.doll.awake(), "asleep")].iter().filter(|s| s.0).map(|s| s.1).collect::<Vec<_>>().join(", ");
    let who = if nm() == delirium::nmrs::MARGE { "Marge" } else { "Homer" };
    let (tab, _) = view(sim.tab);
    let (stripe, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), egui::Sense::hover());
    for (i, color) in STRIPE.into_iter().enumerate() {
        let start = stripe.left() + stripe.width() * i as f32 / 5.0;
        ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(start, stripe.top()), egui::vec2(stripe.width() / 5.0, 5.0)), 0.0, color);
    }
    card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(section_title(&if sim.body == 1 { who.to_string() } else { format!("{who} · {}", BODIES()[sim.body].1) }));
            ui.label(egui::RichText::new(if state.is_empty() { "Ready" } else { &state }).color(DIM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let on = tab == HELP;
                if ui.add(egui::Button::new(egui::RichText::new("?").strong().size(17.0).color(if on { BLACK } else { INK })).fill(if on { HI } else { BTN })
                        .corner_radius(17.0).min_size(egui::vec2(PILL_H, PILL_H))).on_hover_text("Help: mouse and keyboard, first steps").clicked() {
                    sim.tab = if on { code(1, 0) } else { code(HELP, 0) };
                }
                let pin = !sim.sidebar_auto;
                let tip = if pin { "Pinned open: click to let the panel hide at the left edge" } else { "Pin the panel open (it hides at the left edge now)" };
                if ui.add(egui::Button::new(egui::RichText::new("📌").size(15.0).color(if pin { BLACK } else { INK })).fill(if pin { HI } else { BTN }).corner_radius(17.0)
                        .min_size(egui::vec2(PILL_H, PILL_H)).selected(pin)).on_hover_text(tip).clicked() {
                    sim.sidebar_auto = pin;
                }
            });
        });
        let hp = if sim.god { 1.0 } else { (sim.health / 200.0).clamp(0.0, 1.0) };
        let health = if sim.god { "Health: unlimited".to_string() } else { format!("Health: {:.0} / 200", sim.health) };
        ui.add(egui::ProgressBar::new(hp).desired_height(10.0).corner_radius(5.0).fill(egui::Color32::from_rgb(93, 194, 159))).on_hover_text(&health);
        ui.label(egui::RichText::new(health).small().color(DIM));
        let held = sim.held;
        if held {
            ui.label(egui::RichText::new("Pose held. Start a reaction (Reactions) or poke him.").color(HI));
        }
        if sim.dead {
            ui.label(egui::RichText::new("Dead. Reset pose (top bar, R) revives him.").color(HI));
        } else if sim.health < 100.0 && !sim.god {
            ui.label(egui::RichText::new("Low health: more shots can make him collapse.").color(HI));
        }
    });
}

fn help(ui: &mut egui::Ui, sim: &mut Sim) {
    head(ui, "Help & shortcuts");
    info(ui, "Pick a location (Scene), spawn people or animals (Cast), start a reaction (Reactions), then poke, shoot, throw or play as the Hero (Play). Pause or reset any time from the top bar.");
    if sim.help && ui.add(egui::Button::new("Got it — start exploring").min_size(egui::vec2(0.0, PILL_H))).on_hover_text("Hide these first-start tips").clicked() {
        sim.help = false;
        if let Some(p) = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join(HELP_SEEN))) {
            let _ = std::fs::write(p, "");
        }
        sim.tab = 1;
    }
    sub(ui, "Mouse & keyboard");
    egui::Grid::new("keys").max_col_width(230.0).spacing([12.0, 8.0]).striped(true).show(ui, |ui| {
        for (k, what) in [("click", if sim.aim_gun { "fire or swing the floating weapon" } else if sim.click_shoots { "shoot a body part" } else { "poke a body part" }), ("drag", "pull a body part (camera aim)"), ("T (dragging)", "tie the line to the map there; T cuts the newest, Shift+T all"),
                          ("F11", "fullscreen on / off"), ("L", "the line: leash (locked length) or stretchy rope"), ("K", "tied lines wrap round walls and corners on / off"), ("Ctrl + Scroll Wheel", "the line: up tight, down stretchy"), ("Alt + Scroll Wheel", "the newest tied line longer / shorter"), ("Shift + click", "shoot a body part (camera aim)"), ("Shift + right click", "fatal shot"),
                          ("right drag", "orbit the camera"), ("wheel", "zoom"), ("WASD", "walk Homer when Walk Homer is on; otherwise move only in free camera"), ("middle drag", "pan (enters the detached camera)"), ("C  or  Tab", "follow Homer / detached camera: WASD fly, Q/E down/up, hold right mouse to look, wheel zoom, Shift + wheel sets speed, Shift fast, Ctrl slow; the Hero stays put"),
                          ("Space", "pause"), (".", "one step"), ("[  ]", "slower / faster (1/16 to 4x)"), ("1  2  3", "throw a ball / a box / drop a crate at the pointer"), ("R", "reset pose"), ("X", "stop all"),
                          ("Ctrl + point", "aim throws there (and the floating gun)"), ("F", "get in / out of a taxi"), ("Enter", "type a command"), ("H", "hide panels"), ("M", "slide the side panel out or back"),
                          ("P", "photo mode"), ("Ctrl +  /  Ctrl -", "the interface bigger / smaller (Settings, Camera & UI)"), ("Hero (Play)", "WASD walk, Shift run, Ctrl sprint, click punch")] {
            ui.label(egui::RichText::new(k).strong());
            ui.add(egui::Label::new(what).wrap());
            ui.end_row();
        }
    });
}

pub fn top_bar(ui: &mut egui::Ui, sim: &mut Sim, pressed: &mut Option<String>) {
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("DELIRIUM").family(egui::FontFamily::Name("big".into())).size(20.0).color(HI));
        ui.label(egui::RichText::new("Playroom").color(DIM));
        let open = sim.edit_level;
        if ui.add(egui::Button::new(egui::RichText::new("✏ Level editor").strong().color(if open { BLACK } else { HI }))
                .fill(if open { HI } else { HI.gamma_multiply(0.15) }).stroke(egui::Stroke::new(1.0, HI)))
            .on_hover_text("Work in progress: the level editor: shapes, models and splats (or drop a model file on the window), the stage's own walls and stairs, Homer's start").clicked() {
            *pressed = Some("#editlevel".into());
        }
        wip(ui, "the level editor and the model importer are still being built; expect rough edges");
        if sim.safe {
            ui.label(egui::RichText::new("Safe Mode").strong().color(HI))
                .on_hover_text("Started in Safe Mode: every graphics setting at its lowest, and your saved settings untouched until you change one here. Start the regular launcher for your own settings.");
        }
        ui.separator();
        if ui.button(if sim.paused { "▶ Resume" } else { "⏸ Pause" }).on_hover_text("Pause or resume (Space)").clicked() {
            sim.paused = !sim.paused;
        }
        if ui.add_enabled(sim.paused, egui::Button::new("Step")).on_hover_text("Advance one simulation step (.)").clicked() {
            (sim.paused, sim.acc) = (true, 1.0 / 60.0);
        }
        if ui.button("⟲ Reset").on_hover_text(RESET_TIP).clicked() { *pressed = Some("RESET pose".into()); }
        if ui.button("Reset here").on_hover_text("Stand Homer back up where he is, facing his way (Shift+R)").clicked() { *pressed = Some("RESET here".into()); }
        if ui.button("Stop reactions").on_hover_text("Stop every active behaviour (X)").clicked() { *pressed = Some("STOP all".into()); }
        ui.separator();
        if ui.add_enabled(sim.bench.len() + 1 < HOMERS, egui::Button::new("+ Spawn Homer"))
            .on_hover_text(format!("Another Homer, {} (Cast > People picks the body), 2 m in front of the one you control (Shift+N); up to {HOMERS}", BODIES()[sim.spawn_body].1)).clicked() {
            *pressed = Some(format!("spawn {}", BODIES()[sim.spawn_body].0));
        }
        if ui.add_enabled(!sim.bench.is_empty(), egui::Button::new("Next (N)")).on_hover_text("Control the next Homer: keys, pokes, drags, buttons and camera (N)").clicked() {
            *pressed = Some("next homer".into());
        }
        ui.separator();
        if ui.add_enabled(sim.speed > SPEEDS.0, egui::Button::new("◀◀")).on_hover_text("Slower: halve simulation speed ([)").clicked() { *pressed = Some("#slower".into()); }
        ui.label(egui::RichText::new(format!("{}×", sim.speed)).strong().color(HI))
            .on_hover_text(if sim.speed == 1.0 { "Real time" } else if sim.speed < 1.0 { "Slow motion" } else { "Fast forward" });
        if ui.add_enabled(sim.speed != 1.0, egui::Button::new("1×")).on_hover_text("Return to real-time speed").clicked() { *pressed = Some("#realtime".into()); }
        if ui.add_enabled(sim.speed < SPEEDS.1, egui::Button::new("▶▶")).on_hover_text("Faster: double simulation speed (])").clicked() { *pressed = Some("#faster".into()); }
    });
}

pub fn windows(ctx: &egui::Context, sim: &mut Sim, pressed: &mut Option<String>, st: &mut State) {
    for k in 0..POPS.len() {
        if !st.pop[k] {
            continue;
        }
        let mut open = true;
        egui::Window::new(POPS[k]).id(egui::Id::new(("pop", k))).open(&mut open).default_pos([440.0, 90.0 + 40.0 * k as f32])
            .default_width(if k == POP_PARAMS { 460.0 } else { 340.0 }).resizable(true).show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| match k {
                    POP_COLOURS => cast::part_colours(ui, sim),
                    POP_TRAINING => lab::training(ui, sim, pressed),
                    POP_PARAMS => {
                        let s = &mut *sim;
                        params::tab(ui, &mut s.param_edits, &mut s.param_search, &mut s.param_changed_only, pressed);
                    }
                    _ => cast::animal_readout(ui, sim),
                });
            });
        st.pop[k] = open;
    }
}

