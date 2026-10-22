use delirium::{collision::Collision, msg::{self, Message, Value}, nm::Agent, ragdoll::Build};
use bevy::prelude::Vec3;

pub fn registry() -> impl Iterator<Item = &'static Message> {
    msg::MESSAGES.iter().chain(delirium::extras::messages())
}

fn unavailable(name: &str) -> Option<&'static str> {
    Some(match name {
        "activePose" | "incomingTransforms" | "animPose" | "softKeyframe" | "splitBody" => "reference pose is supplied by the joint controller",
        "applyImpulse" | "hardConstraint" => "external forces and pinned bodies are not locomotion actions",
        "defineAttachedObject" | "grab" | "fallOverWall" | "leanTowardsObject" => "requires an actual object or edge target",
        "yanked" | "dragged" | "dangle" | "carried" | "armHang" | "crushReaction" => "requires an external hold or collision context",
        "shot" | "shotRelax" | "shootDodge" | "injury" | "injuredOnGround" | "electrocute" => "requires a matching injury, weapon or shock event",
        "buoyancy" => "requires water",
        "configureBalance" | "configureBalanceReset" | "setStiffness" | "stopAllBehaviours" => "managed by the controller rather than a selectable motor skill",
        "leanInDirection" | "leanToPosition" => "requires a separately bounded continuous lean control",
        _ => return None,
    })
}

pub fn choices() -> Vec<&'static str> {
    std::iter::once("referenceMotors").chain(registry().filter(|m| unavailable(m.name).is_none()).map(|m| m.name)).collect()
}

pub fn catalog() -> String {
    fn field(s: &str) -> String { s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n").replace('\r', "\\r") }
    let mut out = "message\tavailability\tparameter\ttype\timplemented\tdescription\n".to_string();
    for m in registry() {
        let available = if matches!(m.name, "bodyBalance" | "stayUpright") {
            "selectable; coordinated walking task v2: supports ongoing gait rather than replacing it"
        } else { unavailable(m.name).unwrap_or("selectable; native default parameters, targets supplied from the body facing") };
        out.push_str(&format!("{}\t{}\tstart\tBool\ttrue\tNative message start/stop semantics apply\n", m.name, available));
        for p in m.params {
            out.push_str(&format!("{}\t{}\t{}\t{:?}\t{}\t{}\n", m.name, available, p.name, p.kind, p.used, field(p.doc)));
        }
    }
    out
}

#[derive(Default)]
pub struct Tools { pub selected: usize, pub failed: bool }

impl Tools {
    pub fn supports_gait(&self) -> bool {
        matches!(choices()[self.selected], "referenceMotors" | "bodyBalance" | "stayUpright")
    }
    pub fn apply(&mut self, tick: usize, logits: &[f32], a: &mut Agent, build: &Build, world: &Collision) {
        if tick % 12 != 0 { return; }
        let names = choices();
        assert_eq!(logits.len(), names.len());
        let pick = (1..names.len()).fold(0, |best, k| if logits[k] > logits[best] { k } else { best });
        if pick == self.selected { return; }
        a.stop_all_behaviours(); a.msgs = Default::default(); self.failed = false;
        self.selected = pick;
        if pick == 0 { return; }
        let frames: Vec<_> = a.doll.frames().collect();
        let forward = Vec3::Z.cross(frames[4].translation - frames[1].translation).with_z(0.0).normalize_or(Vec3::X);
        let target = frames[9].translation + forward * 2.0;
        let name = names[pick];
        let mut params = vec![("start", Value::Bool(true))];
        match name {
            "headLook" => params.extend([("pos", Value::Vec3(target)), ("alwaysLook", Value::Bool(true))]),
            "pointArm" => params.push(("target", Value::Vec3(target))),
            "braceForImpact" => params.extend([("pos", Value::Vec3(target)), ("look", Value::Vec3(target))]),
            "upperBodyFlinch" => params.push(("pos", Value::Vec3(target))),
            _ => {}
        }
        let mut state = std::mem::take(&mut a.msgs);
        if let Err(error) = msg::send(a, build, world, &mut state, name, &params) {
            eprintln!("motion behavior {name}: {error:?}"); self.failed = true;
            a.stop_all_behaviours(); self.selected = 0; state = Default::default();
        }
        a.msgs = state;
    }
}

