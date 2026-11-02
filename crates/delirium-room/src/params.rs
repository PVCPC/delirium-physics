//! The Parameters tab: every behaviour's parameters, with its default and what it does. A changed value is sent with
//! that behaviour every time the room sends it (buttons, hits, shots, the console), to every character, in place of
//! the value the room would have used. Nothing changed: every message goes out exactly as before.
use crate::{info, section_title, DIM, HI};
use bevy_egui::egui;
use delirium::extras_rdr2::Reactions;
use delirium::msg::{self, Kind, Message, Param, Value};
use std::collections::BTreeMap;

/// Changed values, by (behaviour, parameter).
pub type Edits = BTreeMap<(&'static str, &'static str), Value>;

/// What each behaviour does (the message reference in docs/modding.md).
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
    ("duck", "A short flinch away.\n• After timeBeforeDuck, his spine and neck curl into a random share of their range.\n• His legs crouch a little.\n• Holds for duckTime, then fades out over blendTime."),
    ("shotSpin", "A hit off his centre spins him round.\n• He turns by the hit's spin and holds that heading.\n• With stepTurnType=1 he then turns to face attackerPos.\n• His feet stay planted, so he eases part way back after."),
    ("shotFallToKnees", "He sinks to his knees.\n• After ftkBalanceTime his balance lets go and his knees and hips bend.\n• He kneels upright for ftkKneelTime.\n• Then he topples forwards (or back) and goes limp."),
    ("meleeReact", "A punch or melee blow.\n• His upper body goes loose for csPeriod and he leans away.\n• He flinches (hands up to guard his face with armsType=1).\n• The part hit snaps along the blow, then a hand goes to the spot."),
    ("shotSnap", "A sharp kick along the bullet's path, then a smaller kick back.\n• Sized by what he's doing: moving, standing or in the air.\n• The joints by the part hit get their own kick.\n• Send it with shot."),
    ("shotFromBehind", "Hit in the back.\n• His spine and neck arch back.\n• His hips push forward and his knees give a little.\n• It swells and fades over sfbPeriod."),
    ("shotInGuts", "Hit in the belly.\n• He hunches forward, knees and hips bent, for sigPeriod.\n• Then eases off over sigBlend."),
    ("shotInjuredLeg", "Hit in the leg.\n• The leg jerks up.\n• Then it stays weak for legInjuryTime while he sags and leans onto it."),
    ("shotInjuredArm", "Hit in the arm.\n• His trunk turns with the bullet and the shoulder drops.\n• Then the arm hangs loose for armLooseTime."),
    ("reachForWound", "A hand goes to the wound.\n• Holds it for grabHoldTime.\n• Both hands for a trunk wound with reachWithOneHand=false.\n• For a leg wound he bends down to it."),
    ("wallCollision", "Running into a wall.\n• At the impact: a snap away from it and weak legs.\n• Back to the wall: he slumps down it.\n• Facing it: both hands brace on it, then he pushes off.\n• Side on: he spins away.\n• Can make the wall solid for him (wallHalfWidth wide)."),
    ("setFallingReaction", "Down onto hands and knees.\n• His balance lets go and he puts his hands to the ground.\n• He holds on all fours, head up, for hkHoldTime.\n• Then his strength fades and he sinks onto his front."),
    ("rdr2Balance", "Extras for his balance, while it runs.\n• Stiffer legs when he moves faster than runningThresh.\n• A foot stepping onto something a little higher is set down flat.\n• Runs until stopped (start=false)."),
    ("leanRandom", "Leans his balance a random way.\n• Glides to a new way every changeTimeMin..changeTimeMax s.\n• Runs until stopped."),
    ("hipsLeanInDirection", "Swings his hips out in a direction.\n• The hips are pushed that way and the chest the other.\n• His feet stay under him; held long, he steps after it.\n• Runs until stopped."),
    ("hipsLeanToPosition", "Swings his hips out towards a point.\n• The hips are pushed that way and the chest the other.\n• His feet stay under him; held long, he steps after it.\n• Runs until stopped."),
    ("hipsLeanTowardsObject", "Swings his hips out towards a point (as leanTowardsObject).\n• The hips are pushed that way and the chest the other.\n• His feet stay under him; held long, he steps after it.\n• Runs until stopped."),
    ("hipsLeanRandom", "Swings his hips out a random way.\n• A new way every changeTimeMin..changeTimeMax s.\n• The hips are pushed that way and the chest the other.\n• Runs until stopped."),
    ("forceLeanInDirection", "Pushes bodyPart in a direction.\n• leanAmount times his weight, while he's on his feet.\n• He leans and steps along with it.\n• Runs until stopped."),
    ("forceLeanToPosition", "Pushes bodyPart towards a point.\n• leanAmount times his weight, while he's on his feet.\n• He leans and steps along with it.\n• Runs until stopped."),
    ("forceLeanTowardsObject", "Pushes bodyPart towards a point (as leanTowardsObject).\n• leanAmount times his weight, while he's on his feet.\n• He leans and steps along with it.\n• Runs until stopped."),
    ("forceLeanRandom", "Pushes bodyPart a random way.\n• A new way every changeTimeMin..changeTimeMax s.\n• leanAmount times his weight, while he's on his feet.\n• Runs until stopped."),
    ("onFire", "On fire.\n• On his feet for staggerTime: he staggers, windmilling his arms over his head.\n• He keeps windmilling as he goes down.\n• Down: he thrashes, and with rollOverFlag rolls back and forth to put the flames out.\n• Runs until stopped."),
    ("rdr2Lasso", "Lassoed and dragged. Send it with dragged, which pulls the rope (ropedBodyPart: 11 neck, 10 chest, 3 a foot).\nBy lassoedtype:\n• 0, the neck: his hands claw at the rope at his throat.\n• 1, the chest and arms: arms pinned to his sides, he wriggles.\n• 2, the feet: legs held straight, arms reaching past his head, clawing at the ground.\nDown, his legs kick and spine writhes, and the ground bumps him; optionally he's stunned first. Runs until stopped."),
    ("keyframe", "Before handing him back to animation.\n• Straightens his spine and neck and brings his hands to his sides over blendTime.\n• Holds it for holdTime, so the animation picks up from a close pose.\n• His legs stay on his balance."),
];

/// When Red Dead Redemption 2 style starts a behaviour by itself.
const RDR2_USE: &[(&str, &str)] = &[
    ("rdr2Balance", "every shot and punch"),
    ("shotSnap", "every shot"),
    ("shotSpin", "a shot in the trunk, by chance (the picker's spin chance)"),
    ("shotFallToKnees", "a shot in the belly, pelvis or legs, by chance (the picker's knees chance)"),
    ("shotFromBehind", "a shot in the back"),
    ("shotInGuts", "a shot in the belly"),
    ("shotInjuredLeg", "a shot in a thigh or shin"),
    ("shotInjuredArm", "a shot in an arm"),
    ("reachForWound", "most shots, by chance (the picker's reach chance)"),
    ("meleeReact", "a punch or melee blow"),
];

/// Set by the room for each hit (which part was hit), so not edited here.
const PER_HIT: [&str; 3] = ["bodyPart", "partIndex", "hitPart"];

fn groups() -> [(&'static str, Vec<&'static Message>); 3] {
    use delirium::{extras, extras_fall, extras_held, extras_rdr2};
    [("GTA IV behaviours", msg::MESSAGES.iter().collect()),
     ("Delirium's extra behaviours", extras::MESSAGES.iter().chain(&extras_fall::MESSAGES).chain(&extras_held::MESSAGES).collect()),
     ("Red Dead Redemption 2 behaviours", extras_rdr2::MESSAGES.iter().collect())]
}

fn find(m: &str, p: &str) -> Option<(&'static Message, &'static Param)> {
    let m = msg::find(m)?;
    Some((m, m.params.iter().find(|q| q.name == p)?))
}

/// The value it has when nothing sets it: the engine's for GTA IV's behaviours, the one in brackets in an extra's
/// description otherwise (None when it doesn't say).
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

/// Puts the changed values for behaviour `name` into `ps` (replacing what the line set); what it changed, for the log.
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

/// settings.txt lines: `param behaviour.parameter value` for each change, and `rdr2_pick ...` when the picker isn't on
/// its defaults (nothing at all when nothing changed).
pub fn save(edits: &Edits, pick: &Reactions) -> String {
    let mut s: String = edits.iter().map(|((m, p), v)| format!("param {m}.{p} {}\n", show(v))).collect();
    if *pick != Reactions::default() {
        let b = |x: bool| x as u8;
        s += &format!("rdr2_pick {} {} {} {} {} {} {}\n", b(pick.snap), pick.spin, pick.knees, b(pick.hit_location), pick.reach, b(pick.melee), b(pick.balance));
    }
    s
}

/// Reads a `param` line's value (`behaviour.parameter value`).
pub fn load_param(edits: &mut Edits, v: &str) {
    let Some((key, val)) = v.trim().split_once(' ') else { return };
    let Some((m, p)) = key.split_once('.') else { return };
    if let Some((m, p)) = find(m, p).filter(|(_, p)| editable(p)) && let Some(x) = parse(p.kind, val.trim()) {
        edits.insert((m.name, p.name), x);
    }
}

/// Reads the `rdr2_pick` line's value.
pub fn load_pick(pick: &mut Reactions, v: &str) {
    let n: Vec<f32> = v.split_whitespace().map_while(|x| x.parse().ok()).collect();
    if let [snap, spin, knees, hit, reach, melee, balance] = n[..] {
        *pick = Reactions { snap: snap != 0.0, spin: spin.clamp(0.0, 1.0), knees: knees.clamp(0.0, 1.0), hit_location: hit != 0.0,
                            reach: reach.clamp(0.0, 1.0), melee: melee != 0.0, balance: balance != 0.0 };
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

/// What a behaviour does, and when RDR2 style starts it.
fn about(m: &str) -> String {
    let mut s = ABOUT.iter().find(|a| a.0 == m).map_or("", |a| a.1).to_string();
    if let Some(u) = RDR2_USE.iter().find(|u| u.0 == m) {
        s += &format!("

In Red Dead Redemption 2 reaction style, started by {}.", u.1);
    }
    s
}

/// A hover box: the title in bold, then each section, a line at a time ("• " lines indented), kept narrow.
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

/// The Parameters tab.
pub fn tab(ui: &mut egui::Ui, edits: &mut Edits, search: &mut String, changed_only: &mut bool, pick: &mut Reactions, rdr2: bool) {
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

    let picker = "Red Dead Redemption 2 reaction picker";
    let picker_changed = *pick != Reactions::default();
    if if *changed_only { picker_changed } else { hit(picker) || hit("rdr2") } {
        let title = if picker_changed { egui::RichText::new(format!("{picker}  (changed)")).strong().color(HI) } else { egui::RichText::new(picker).strong() };
        egui::CollapsingHeader::new(title).id_salt(picker).open(open).show(ui, |ui| {
            info(ui, if rdr2 { "Which RDR2 reactions a shot or punch starts." }
                     else { "Which RDR2 reactions a shot or punch starts, in Red Dead Redemption 2 reaction style (Reactions tab)." });
            let chance = |ui: &mut egui::Ui, x: &mut f32, label: &str, tip: &str| {
                ui.add(egui::Slider::new(x, 0.0..=1.0).text(label).custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).on_hover_text(tip);
            };
            chance(ui, &mut pick.spin, "Spin chance", "A shot in the trunk spins him round (shotSpin).");
            chance(ui, &mut pick.knees, "Fall to knees chance", "A shot in the belly, pelvis or legs drops him to his knees (shotFallToKnees), when he didn't spin.");
            chance(ui, &mut pick.reach, "Reach for the wound chance", "A hand goes to the wound (reachForWound); not for the head, neck, hands or feet.");
            ui.checkbox(&mut pick.snap, "Snap").on_hover_text("Every shot: the hit part jerks along the bullet and snaps back (shotSnap).");
            ui.checkbox(&mut pick.hit_location, "Where he's hit matters").on_hover_text("shotFromBehind (the back), shotInGuts (the belly), shotInjuredLeg, shotInjuredArm.");
            ui.checkbox(&mut pick.melee, "Melee reaction").on_hover_text("A punch starts meleeReact instead of the usual flinch.");
            ui.checkbox(&mut pick.balance, "RDR2 balance").on_hover_text("rdr2Balance from the first hit: stiffer legs when he runs, feet set down flat.");
            if ui.add_enabled(picker_changed, egui::Button::new("Reset picker")).clicked() {
                *pick = Reactions::default();
            }
        });
    }

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
        egui::CollapsingHeader::new(title).id_salt(group).open(open).show(ui, |ui| {
            for (m, ps) in shown {
                let n = edits.keys().filter(|k| k.0 == m.name).count();
                let head = if n > 0 { egui::RichText::new(format!("{}  ({n} changed)", m.name)).color(HI) } else { egui::RichText::new(m.name) };
                let header = egui::CollapsingHeader::new(head).id_salt(("params", m.name)).open(open).show(ui, |ui| {
                    egui::Grid::new(("param grid", m.name)).num_columns(3).spacing([10.0, 4.0]).striped(true).show(ui, |ui| {
                        for p in ps {
                            row(ui, edits, m, p);
                            ui.end_row();
                        }
                    });
                });
                header.header_response.on_hover_ui(|ui| behaviour_hover(ui, m.name));
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
