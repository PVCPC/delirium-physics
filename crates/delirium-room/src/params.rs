use crate::{info, section_title, DIM, HI};
use bevy_egui::egui;
use delirium::msg::{self, Kind, Message, Param, Value};
use std::collections::BTreeMap;

pub type Edits = BTreeMap<(&'static str, &'static str), Value>;

const ABOUT: &[(&str, &str)] = &[
    ("bodyBalance", "Stand, step, recover from pushes."),
    ("configureBalance", "Tune the balancer."),
    ("configureBalanceReset", "Reset balancer tuning."),
    ("stayUpright", "Weak uprightness hold."),
    ("setStiffness", "One-shot body stiffness."),
    ("bodyRelax", "One-shot go limp."),
    ("shotRelax", "One-shot relax after a hit."),
    ("shot", "Bullet or melee reaction."),
    ("upperBodyFlinch", "Cover up, turn away."),
    ("catchFall", "Arms out to break a fall."),
    ("highFall", "Long fall, spread out, land."),
    ("rollDownStairs", "Tumble down a slope."),
    ("braceForImpact", "Brace against an approaching object."),
    ("fallOverWall", "Tip over a low wall."),
    ("grab", "Grab a ledge, rail or box."),
    ("headLook", "Look at a point."),
    ("pointArm", "Point an arm."),
    ("leanInDirection", "One-shot lean."),
    ("leanToPosition", "One-shot lean to a point."),
    ("leanTowardsObject", "One-shot lean to an offset."),
    ("applyImpulse", "One-shot push on a part."),
    ("bodyFoetal", "Curl up."),
    ("bodyRollUp", "Roll into a ball."),
    ("bodyWrithe", "Thrash on the ground."),
    ("armsWindmillAdaptive", "Flail arms to regain balance."),
    ("pedalLegs", "Pedal the legs (cycling or in the air)."),
    ("activePose", "One-shot: follow the supplied pose."),
    ("incomingTransforms", "Toggle using the supplied animation pose."),
    ("defineAttachedObject", "Tell the body it holds a mass."),
    ("stopAllBehaviours", "Clear everything."),
    ("electrocute", "Twitching jolts."),
    ("hardConstraint", "Hold chosen parts on the supplied pose."),
    ("injury", "One-shot: weaken a limb."),
    ("teeter", "Wobble at an edge."),
    ("stumble", "Stagger forward with arms braced."),
    ("staggerFall", "Stagger then fall."),
    ("smartFall", "Choose a fall reaction in the air and on landing."),
    ("balancerCollisionsReaction", "React to an obstacle behind."),
    ("injuredOnGround", "Hurt on the floor."),
    ("learnedCrawl", "Crawl."),
    ("yanked", "Pulled by a point."),
    ("dragged", "Dragged on a rope."),
    ("dangle", "Hang from a point."),
    ("carried", "Being carried."),
    ("buoyancy", "Float in water."),
    ("animPose", "Hold a pose given in text."),
    ("armsWindmill", "Windmill the arms."),
    ("spineTwist", "Twist the spine toward a point."),
    ("wave", "Wave a hand; ends by itself."),
    ("breathe", "Breathing motion."),
    ("softKeyframe", "Soft follow of a target."),
    ("splitBody", "Aim and fire a held weapon."),
    ("shootDodge", "Dodge a shot."),
    ("armHang", "Hang by the arms."),
    ("crushReaction", "React to a crushing object."),
];

const PER_HIT: [&str; 3] = ["bodyPart", "partIndex", "hitPart"];

fn groups() -> [(&'static str, Vec<&'static Message>); 2] {
    [("the game behaviours", msg::MESSAGES.iter().collect()), ("Delirium's extra behaviours", delirium::extras::messages().collect())]
}

fn find(m: &str, p: &str) -> Option<(&'static Message, &'static Param)> {
    let m = msg::find(m)?;
    Some((m, m.params.iter().find(|q| q.name == p)?))
}

fn default(m: &Message, p: &Param) -> Option<Value> {
    if msg::MESSAGES.iter().any(|x| x.name == m.name) {
        return msg::get(&msg::State::default(), m.name, p.name);
    }
    p.doc.match_indices('(').map(|(i, _)| i).collect::<Vec<_>>().into_iter().rev().find_map(|i| {
        let inner = &p.doc[i + 1..i + p.doc[i..].find(')')?];
        parse(p.kind, inner.split([',', ':', ';', ' ']).next()?)
    })
}

fn parse(kind: Kind, v: &str) -> Option<Value> {
    Some(match kind {
        Kind::Float => Value::Float(v.parse().ok()?),
        Kind::Int => Value::Int(v.parse().ok()?),
        Kind::Bool => Value::Bool(match v {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return None,
        }),
        Kind::Vec3 | Kind::Str => return None,
    })
}

fn show(v: &Value) -> String {
    match v {
        Value::Float(x) => format!("{x:.3}").trim_end_matches('0').trim_end_matches('.').to_string(),
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Vec3(v) => format!("{},{},{}", v.x, v.y, v.z),
        Value::Str(s) => s.clone(),
    }
}

fn editable(p: &Param) -> bool {
    p.used && matches!(p.kind, Kind::Float | Kind::Int | Kind::Bool) && !PER_HIT.contains(&p.name)
}

pub fn apply(edits: &Edits, name: &str, ps: &mut Vec<(&str, Value)>) -> Vec<String> {
    let mut done = vec![];
    for ((_, k), v) in edits.iter().filter(|((m, _), _)| *m == name) {
        match ps.iter_mut().find(|(n, _)| n == k) {
            Some(slot) => slot.1 = v.clone(),
            None => ps.push((k, v.clone())),
        }
        done.push(format!("{k}={}", show(v)));
    }
    done
}

pub fn save(edits: &Edits) -> String {
    edits.iter().map(|((m, p), v)| format!("param {m}.{p} {}
", show(v))).collect()
}

pub fn load_param(edits: &mut Edits, v: &str) {
    let Some((key, val)) = v.trim().split_once(' ') else { return };
    let Some((m, p)) = key.split_once('.') else { return };
    if let Some((m, p)) = find(m, p).filter(|(_, p)| editable(p)) && let Some(x) = parse(p.kind, val.trim()) {
        edits.insert((m.name, p.name), x);
    }
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Float => "a number",
        Kind::Int => "a whole number",
        Kind::Bool => "on or off",
        Kind::Vec3 => "a position or direction",
        Kind::Str => "text",
    }
}

fn about(m: &str) -> String {
    ABOUT.iter().find(|a| a.0 == m).map_or("", |a| a.1).to_string()
}

fn hover(ui: &mut egui::Ui, title: &str, sections: &[String]) {
    ui.set_max_width(380.0);
    ui.label(egui::RichText::new(title).strong().size(15.0));
    for s in sections.iter().filter(|s| !s.is_empty()) {
        ui.add_space(6.0);
        for line in s.lines() {
            match line.strip_prefix("• ") {
                Some(b) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(8.0);
                        ui.label("•");
                        ui.label(b);
                    });
                }
                None => {
                    ui.label(line);
                }
            }
        }
    }
}

fn behaviour_hover(ui: &mut egui::Ui, m: &str) {
    hover(ui, m, &[about(m)]);
}

fn param_hover(ui: &mut egui::Ui, m: &Message, p: &Param, def: &Option<Value>) {
    let doc = if p.doc.is_empty() { "No description yet.".to_string() } else { p.doc.to_string() };
    let mut kind = format!("Type: {}
Default: {}", kind_name(p.kind), def.as_ref().map_or("not listed".into(), show));
    if !p.used {
        kind += "
Accepted, but Delirium doesn't use it (yet): changing it does nothing.";
    } else if PER_HIT.contains(&p.name) {
        kind += "
Set by the room for each hit (the part hit); send it from the console to choose one.";
    } else if matches!(p.kind, Kind::Vec3 | Kind::Str) {
        kind += "
Set by the room each time (where, which way, which parts); send it from the console to choose one.";
    }
    hover(ui, p.name, &[doc, kind, format!("Part of {}:
{}", m.name, about(m.name))]);
}

pub fn tab(ui: &mut egui::Ui, edits: &mut Edits, search: &mut String, changed_only: &mut bool, pressed: &mut Option<String>) {
    ui.add_space(6.0);
    ui.label(section_title("Parameters"));
    info(ui, "Change any behaviour's settings. A changed value is used every time that behaviour starts, from a button, \
              a hit, a shot or the console, for every character, in place of the value the room would pick. Hover a \
              name for what it does and which behaviour it belongs to.");
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(search).hint_text("Search behaviours or parameters").desired_width(220.0));
        ui.checkbox(changed_only, "Changed only");
        if ui.add_enabled(!edits.is_empty(), egui::Button::new("Reset all")).on_hover_text("Every behaviour back to its defaults.").clicked() {
            edits.clear();
        }
    });
    let count = match edits.len() {
        0 => "Nothing changed.".to_string(),
        1 => "1 value changed.".to_string(),
        n => format!("{n} values changed."),
    };
    ui.label(egui::RichText::new(count).small().color(DIM));
    let q = search.trim().to_ascii_lowercase();
    let hit = |s: &str| q.is_empty() || s.to_ascii_lowercase().contains(&q);
    let open = (!q.is_empty() || *changed_only).then_some(true);

    for (group, messages) in groups() {
        let shown: Vec<(&'static Message, Vec<&'static Param>)> = messages.into_iter().filter_map(|m| {
            let ps: Vec<&'static Param> = m.params.iter()
                .filter(|p| (hit(m.name) || hit(p.name)) && (!*changed_only || edits.contains_key(&(m.name, p.name)))).collect();
            (!ps.is_empty()).then_some((m, ps))
        }).collect();
        if shown.is_empty() {
            continue;
        }
        let changed: usize = shown.iter().map(|(m, _)| edits.keys().filter(|k| k.0 == m.name).count()).sum();
        let title = if changed > 0 { egui::RichText::new(format!("{group}  ({changed} changed)")).strong().color(HI) } else { egui::RichText::new(group).strong() };
        egui::CollapsingHeader::new(title).id_salt(group).default_open(true).open(open).show(ui, |ui| {
            for (m, ps) in shown {
                let n = edits.keys().filter(|k| k.0 == m.name).count();
                let head = if n > 0 { egui::RichText::new(format!("{}  ({n} changed)", m.name)).color(HI) } else { egui::RichText::new(m.name) };
                let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(("params", m.name)), false);
                if let Some(o) = open {
                    state.set_open(o);
                }
                state.show_header(ui, |ui| {
                    ui.label(head).on_hover_ui(|ui| behaviour_hover(ui, m.name));
                    if let Some((b, label)) = crate::reaction_for(m.name) {
                        if ui.small_button(format!("▶ {label}")).on_hover_text("Start this reaction (as its Reactions button).").clicked() {
                            *pressed = Some(b.to_string());
                        }
                    }
                }).body(|ui| {
                    egui::Grid::new(("param grid", m.name)).num_columns(3).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
                        for p in ps {
                            row(ui, edits, m, p);
                            ui.end_row();
                        }
                    });
                });
            }
        });
    }
}

fn row(ui: &mut egui::Ui, edits: &mut Edits, m: &'static Message, p: &'static Param) {
    let key = (m.name, p.name);
    let def = default(m, p);
    let changed = edits.contains_key(&key);
    let name = egui::RichText::new(p.name);
    ui.label(if changed { name.color(HI) } else if editable(p) { name } else { name.color(DIM) }).on_hover_ui(|ui| param_hover(ui, m, p, &def));
    if !editable(p) {
        let why = if !p.used { "not used" } else if PER_HIT.contains(&p.name) { "set per hit" } else { "set by the room" };
        ui.label(egui::RichText::new(why).small().color(DIM)).on_hover_ui(|ui| param_hover(ui, m, p, &def));
        ui.label("");
        return;
    }
    let now = edits.get(&key).cloned().or_else(|| def.clone()).unwrap_or(match p.kind {
        Kind::Float => Value::Float(0.0),
        Kind::Int => Value::Int(0),
        _ => Value::Bool(false),
    });
    let new = match now {
        Value::Float(mut x) => {
            let speed = (x.abs().max(1.0) * 0.01) as f64;
            ui.add(egui::DragValue::new(&mut x).speed(speed).max_decimals(3)).changed().then_some(Value::Float(x))
        }
        Value::Int(mut i) => ui.add(egui::DragValue::new(&mut i).speed(0.1)).changed().then_some(Value::Int(i)),
        Value::Bool(mut b) => ui.checkbox(&mut b, "").changed().then_some(Value::Bool(b)),
        _ => None,
    };
    if let Some(v) = new {
        edits.insert(key, v);
    }
    let back = format!("Back to {}.", def.as_ref().map_or("its default".into(), show));
    if ui.add_enabled(changed, egui::Button::new("Reset").small()).on_hover_text(back).clicked() {
        edits.remove(&key);
    }
}
