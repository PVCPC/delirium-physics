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
    ("duck", "Short flinch away: after timeBeforeDuck the spine and neck bend into a random share of their remaining range, each at its own rate, and the legs crouch through the balancer, for duckTime; it fades out over blendTime."),
    ("shotSpin", "A hit off his centre spins him round: a damped yaw torque pulls him onto a heading that swings round by the hit's spin and holds it, the balancer told to face it; then, with stepTurnType=1, he turns to face attackerPos. The feet stay planted, so after the hold he eases part way back."),
    ("shotFallToKnees", "Sinks to his knees: after ftkBalanceTime the balancer lets go, knees and hips bend at ftkBendRate; once the knees are down he kneels upright for ftkKneelTime, his centre held over his knees; then he topples forwards (or back) and goes loose."),
    ("meleeReact", "A punch or melee blow: the upper body goes loose for csPeriod, a push leans him away, he flinches (hands up to guard his face with armsType=1), the hit part snaps along the blow, then a hand reaches for the spot hit."),
    ("shotSnap", "A sharp kick across the chosen joints along the bullet's path, then a smaller counter-kick back (send it with shot). Sized by his state (moving, standing, airborne) and per joint; the hit part's joints get their own kick."),
    ("shotFromBehind", "Hit in the back: the spine arches back, the neck goes back, the hips push forward and the knees give a little; it swells and goes over sfbPeriod."),
    ("shotInGuts", "Hit in the belly: he hunches forward, knees and hips bent, for sigPeriod, then eases off over sigBlend."),
    ("shotInjuredLeg", "Hit in the leg: it jerks up, then stays weak for legInjuryTime while he sags and leans onto it."),
    ("shotInjuredArm", "Hit in the arm: the trunk turns with the bullet and the hit shoulder drops, then the arm hangs loose for armLooseTime."),
    ("reachForWound", "A hand goes to the wound and holds it for grabHoldTime (both hands for a trunk wound with reachWithOneHand=false); for a leg wound he bends down to it."),
    ("wallCollision", "Run into a wall: a snap away from it and weak legs at the impact; his back to it: he slumps down it; facing it: both hands brace on it, then he pushes off; side on: a spin away. Can make the wall solid for his body (a plane, or wallHalfWidth wide)."),
    ("setFallingReaction", "Down onto hands and knees: the balancer lets go, he folds and puts his hands to the ground; once hands and a knee are down (or after 1.5 s) he holds there on all fours, head up, for hkHoldTime, then his strength fades and he sinks onto his front."),
    ("rdr2Balance", "Balancer extras while it runs: stiffer legs when he moves faster than runningThresh, and a foot stepping up onto something only a little higher is turned flat on it. Runs until stopped (start=false)."),
    ("leanRandom", "Lean his whole balance this way (as leanInDirection); a new way, gliding to it, every changeTimeMin..changeTimeMax s. Runs until stopped."),
    ("hipsLeanInDirection", "In a direction: the hips swing out that way (pushed one way, the chest the other) while his feet stay under him; a sustained lean makes the balancer step after it. Runs until stopped."),
    ("hipsLeanToPosition", "Towards a point: the hips swing out that way (pushed one way, the chest the other) while his feet stay under him; a sustained lean makes the balancer step after it. Runs until stopped."),
    ("hipsLeanTowardsObject", "Towards a point (as leanTowardsObject): the hips swing out that way (pushed one way, the chest the other) while his feet stay under him; a sustained lean makes the balancer step after it. Runs until stopped."),
    ("hipsLeanRandom", "A random way, changing every changeTimeMin..changeTimeMax s: the hips swing out that way (pushed one way, the chest the other) while his feet stay under him; a sustained lean makes the balancer step after it. Runs until stopped."),
    ("forceLeanInDirection", "In a direction: push bodyPart that way, leanAmount times his weight, while he's on his feet (he leans and steps along). Runs until stopped."),
    ("forceLeanToPosition", "Towards a point: push bodyPart that way, leanAmount times his weight, while he's on his feet (he leans and steps along). Runs until stopped."),
    ("forceLeanTowardsObject", "Towards a point (as leanTowardsObject): push bodyPart that way, leanAmount times his weight, while he's on his feet (he leans and steps along). Runs until stopped."),
    ("forceLeanRandom", "A random way, changing every changeTimeMin..changeTimeMax s: push bodyPart that way, leanAmount times his weight, while he's on his feet (he leans and steps along). Runs until stopped."),
    ("onFire", "On fire: on his feet for staggerTime he staggers, his lean swinging forwards and back, arms flailing between a windmill and a writhe; then he goes down, writhes between curled up and thrashing, and (rollOverFlag) rolls back and forth to put the flames out. Runs until stopped."),
    ("keyframe", "Before handing the body back to animation: blends spine and neck straight and the hands to his sides over blendTime, damping the upper body, and holds it for holdTime, so the animation picks up from a pose close to its own. The legs stay the balancer's."),
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

fn about(m: &str) -> String {
    let mut s = format!("{m}: {}", ABOUT.iter().find(|a| a.0 == m).map_or("", |a| a.1));
    if let Some(u) = RDR2_USE.iter().find(|u| u.0 == m) {
        s += &format!("\nIn Red Dead Redemption 2 reaction style, started by {}.", u.1);
    }
    s
}

fn tip(m: &Message, p: &Param, def: &Option<Value>) -> String {
    let mut s = format!("{}\n{}\n\nIt is {}. Default: {}.", p.name, if p.doc.is_empty() { "No description yet." } else { p.doc },
                        kind_name(p.kind), def.as_ref().map_or("not listed".into(), show));
    if !p.used {
        s += "\nAccepted, but Delirium doesn't use it (yet): changing it does nothing.";
    } else if PER_HIT.contains(&p.name) {
        s += "\nSet by the room for each hit (the part hit); send it from the console to choose one.";
    } else if matches!(p.kind, Kind::Vec3 | Kind::Str) {
        s += "\nSet by the room each time (where, which way, which parts); send it from the console to choose one.";
    }
    s + "\n\nBehaviour: " + &about(m.name)
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
                header.header_response.on_hover_text(about(m.name));
            }
        });
    }
}

fn row(ui: &mut egui::Ui, edits: &mut Edits, m: &'static Message, p: &'static Param) {
    let key = (m.name, p.name);
    let def = default(m, p);
    let changed = edits.contains_key(&key);
    let name = egui::RichText::new(p.name);
    ui.label(if changed { name.color(HI) } else if editable(p) { name } else { name.color(DIM) }).on_hover_text(tip(m, p, &def));
    if !editable(p) {
        let why = if !p.used { "not used" } else if PER_HIT.contains(&p.name) { "set per hit" } else { "set by the room" };
        ui.label(egui::RichText::new(why).small().color(DIM)).on_hover_text(tip(m, p, &def));
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
