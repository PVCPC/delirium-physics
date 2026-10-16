use crate::csv::CsvReader;
use crate::map::hash_bytes;

pub const ACTION_COLUMNS: [&str; 41] = [
    "Name", "Priority", "Bias", "Type", "AIPlayerfilter", "AvailableFromIdle", "MustBeRecievingHit", "MustBeBlocking",
    "MustHaveJustStruckSomething", "HealthRange", "attackMultRange", "defenseMultRange", "martialLevelRange", "WeaponClass",
    "ImpulseOp", "CheckImpulse", "Impulse", "ImpulseState", "ImpulseRange", "ImpulseInterval", "CheckImpulse", "Impulse",
    "ImpulseState", "ImpulseRange", "ImpulseInterval", "RequireTarget", "RequireLockOn", "RequiredTargetType",
    "RequiredOpponentRes", "CheckOpponentAnimTime", "OpponentAnimTime", "RequiredOpponentPose", "CheckRelPos", "AngleRange",
    "DistanceRange", "HeightRange", "OpponentsActionToTest", "SpecialTest", "AdditionalActionTest", "Move", "Prefer Ragdol",
];

pub const RESULT_COLUMNS: [&str; 42] = [
    "Name", "Priority", "AnimGroup", "AnimName", "SyncStartTime", "StartTime", "Playback Rate", "PlayerPlaybackMult",
    "IsLoop", "BlendInFactor", "BlendOutFactor", "IsAnIdle", "IsABlock", "PlayerLockEnablesMeleeMode", "HomingTargetType",
    "AngleRange", "DistanceRange", "HeightRange", "stopOnCirtFrame", "stopTime", "DesiredAngleAndRate",
    "DesiredDistanceAndRate", "DesiredHeightAndRate", "ForceImmediateReaction", "SelfDamage", "DropWeapon",
    "DisarmOpponent", "StrikeBone0", "StrikeBone0Radius", "StrikeBone1", "StrikeBone1Radius", "OnHitDamage",
    "AppliedPedMoveMomentum", "ObjectDamageMult", "Branch0Action", "Branch0StartEnd", "Branch1Action", "Branch1StartEnd",
    "Branch2Action", "Branch2StartEnd", "SpecialResult", "SoundName",
];

const IMPULSES: [&str; 9] =
    ["attack", "attack2", "attack3", "attack4", "kick", "block", "analogLeftRight", "analogUpDown", "analogFromCenter"];
const STATES: [&str; 6] = ["heldDown", "heldUp", "pressedAndHeld", "releasedAndHeld", "pressed", "released"];
const POSES: [&str; 7] = ["standing", "crouched", "onGround", "onFront", "onRHS", "onBack", "onLHS"];
const POINTS: [&str; 16] = [
    "entityRoot", "entityClosestBoxPoint", "pedRoot", "pedClosestBoxPoint", "pedHead", "pedChest", "pedKneeLeft",
    "pedKneeRight", "vehicleRoot", "vehicleClosestBoxPoint", "vehicleBonnet", "vehicleBoot", "vehicleClosestDoor",
    "vehicleClosestWindow", "vehicleClosestLight", "vehicleClosestTyre",
];
const BONES: [(&str, u32); 14] = [
    ("PELVIS", 0x1A1), ("HEAD", 0x4B5), ("R_UPPERARM", 0x4C8), ("R_FOREARM", 0x4C9), ("R_HAND", 0x4D0), ("L_UPPERARM", 0x4C1),
    ("L_FOREARM", 0x4C2), ("L_HAND", 0x4C3), ("L_THIGH", 0x1A2), ("L_CALF", 0x1A3), ("L_FOOT", 0x1A4), ("R_THIGH", 0x1A7),
    ("R_CALF", 0x1A8), ("R_FOOT", 0x1A9),
];

fn h(s: &[u8]) -> u32 {
    hash_bytes(s, 0)
}

pub fn none() -> u32 {
    h(b"none")
}

fn place(list: &[&str], s: &[u8]) -> Option<usize> {
    let x = h(s);
    list.iter().position(|n| h(n.as_bytes()) == x)
}

fn who(s: &[u8]) -> u32 {
    match place(&["all", "player", "ai"], s) {
        Some(i) => i as u32,
        None => 3,
    }
}

fn point(s: &[u8]) -> u32 {
    place(&POINTS, s).map_or(16, |i| i as u32)
}

fn bone(s: &[u8]) -> u32 {
    let x = h(s);
    BONES.iter().find(|b| h(b.0.as_bytes()) == x).map_or(0, |b| b.1)
}

fn hash_or_0(s: &[u8]) -> u32 {
    let x = h(s);
    if x == none() { 0 } else { x }
}

fn rad(d: f32) -> f32 {
    d * f32::from_bits(0x3C8E_FA35)
}

fn rad_end(d: f32) -> f32 {
    if d == 180.0 {
        std::f32::consts::PI
    } else if d == -180.0 {
        -std::f32::consts::PI
    } else {
        rad(d)
    }
}

pub fn wrap(mut x: f32) -> f32 {
    let (pi, two_pi) = (std::f32::consts::PI, f32::from_bits(0x40C9_0FDB));
    let low = if x >= -25.0 {
        if x > 25.0 || x > pi {
            if x > 25.0 {
                x = 25.0;
            }
            loop {
                x -= two_pi;
                if !(x > pi) {
                    break;
                }
            }
        }
        x < -pi
    } else {
        x = -25.0;
        true
    };
    if low {
        loop {
            x += two_pi;
            if !(x < -pi) {
                break;
            }
        }
    }
    x
}

fn sorted(r: [f32; 2]) -> [f32; 2] {
    if r[0] > r[1] { [r[1], r[0]] } else { r }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Impulse {
    pub check: bool,
    pub impulse: u32,
    pub state: u32,
    pub range: [f32; 2],
    pub interval: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Action {
    pub name: u32,
    pub priority: i32,
    pub bias: f32,
    pub ai_player_filter: u32,
    pub flags: [bool; 4],
    pub ranges: [[f32; 2]; 4],
    pub weapon_class: u32,
    pub kind: u32,
    pub available_from_idle: bool,
    pub impulse_op: u32,
    pub impulses: [Impulse; 2],
    pub require_target: u32,
    pub require_lock_on: u32,
    pub required_target_type: u32,
    pub opponent_res_not: bool,
    pub opponent_res: u32,
    pub check_opponent_anim_time: bool,
    pub opponent_anim_time: [f32; 2],
    pub opponent_pose: u32,
    pub opponents_action_to_test: u32,
    pub check_rel_pos: bool,
    pub rel_point: u32,
    pub height_range: [f32; 2],
    pub distance_range: [f32; 2],
    pub angle_range: [f32; 2],
    pub special_test: u32,
    pub additional_action_test: u32,
    pub move_: u32,
    pub prefer_ragdoll: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResultDef {
    pub name: u32,
    pub priority: i32,
    pub anim_group: i32,
    pub anim: i32,
    pub sync_start_time: u32,
    pub start_time: f32,
    pub playback_rate: f32,
    pub player_playback_mult: f32,
    pub blend_in: f32,
    pub blend_out: f32,
    pub is_loop: bool,
    pub is_an_idle: bool,
    pub is_a_block: bool,
    pub player_lock_enables_melee_mode: bool,
    pub homing: bool,
    pub homing_target_type: u32,
    pub height_range: [f32; 2],
    pub distance_range: [f32; 2],
    pub angle_range: [f32; 2],
    pub stop_on_crit_frame: bool,
    pub stop_time: f32,
    pub desired_height: [f32; 2],
    pub desired_distance: [f32; 2],
    pub desired_angle: [f32; 2],
    pub force_immediate_reaction: bool,
    pub self_damage: f32,
    pub drop_weapon: bool,
    pub disarm_opponent: bool,
    pub on_hit_damage: f32,
    pub applied_ped_move_momentum: f32,
    pub object_damage_mult: f32,
    pub special_result: u32,
    pub sound: u32,
    pub branches: [(bool, u32, [f32; 2]); 3],
    pub strike_bones: [(bool, u32, f32); 2],
}

#[derive(Default)]
struct Kept {
    ints: std::collections::HashMap<u16, i32>,
    floats: std::collections::HashMap<u16, f32>,
}

impl Kept {
    fn int(&mut self, r: &mut CsvReader, at: u16) -> i32 {
        if let Some(v) = r.int() {
            self.ints.insert(at, v);
        }
        self.ints.get(&at).copied().unwrap_or(0)
    }
    fn float(&mut self, r: &mut CsvReader, at: u16) -> f32 {
        if let Some(v) = r.float() {
            self.floats.insert(at, v);
        }
        self.floats.get(&at).copied().unwrap_or(0.0)
    }
}

fn s(r: &mut CsvReader) -> Vec<u8> {
    r.field(128, true).1
}

fn range(r: &mut CsvReader) -> [f32; 2] {
    let (_, lo, hi) = r.range();
    [lo, hi]
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActionTable {
    pub actions: Vec<Action>,
    pub results: Vec<ResultDef>,
    pub completed: bool,
}

pub fn read(file: &[u8], group: impl Fn(&[u8]) -> i32, anim: impl Fn(i32, &[u8]) -> i32) -> ActionTable {
    let mut r = CsvReader::new(file, b',');
    let mut out = ActionTable::default();
    let mut count: u32 = 1;
    if !r.at_end() {
        loop {
            if !r.skip_lines(1) {
                break;
            }
            count += 1;
            if r.at_end() {
                break;
            }
        }
    }
    r.rewind();
    let mut actions = true;
    let mut k = Kept::default();
    let mut i = 0;
    while i < count {
        if r.at_end() {
            break;
        }
        let name = s(&mut r);
        let name_hash = h(&name);
        if name.starts_with(b"//") {
            if name_hash == h(b"// ACTIONS:") {
                actions = true;
            }
            if name_hash == h(b"// RESULTS:") {
                actions = false;
            }
            r.skip_lines(1);
        } else if actions {
            match read_action(&mut r, &mut k, name_hash) {
                Some(a) => out.actions.push(a),
                None => return out,
            }
        } else {
            out.results.push(read_result(&mut r, &mut k, name_hash, &group, &anim));
        }
        i += 1;
    }
    out.completed = true;
    out
}

fn read_action(r: &mut CsvReader, k: &mut Kept, name: u32) -> Option<Action> {
    let priority = k.int(r, 576);
    let bias = k.float(r, 504);
    let kind = s(r);
    let filter = s(r);
    let flags = [k.int(r, 256), k.int(r, 288), k.int(r, 556), k.int(r, 304)].map(|v| v != 0);
    let health = range(r);
    let attack = range(r);
    let defense = range(r);
    let martial = range(r);
    let weapon = s(r);
    let op = s(r);
    let mut raw = [(0i32, vec![], vec![], [0f32; 2], 0f32), (0, vec![], vec![], [0.0; 2], 0.0)];
    for (n, imp) in raw.iter_mut().enumerate() {
        imp.0 = k.int(r, 864 + 4 * n as u16);
        imp.1 = s(r);
        imp.2 = s(r);
        imp.3 = range(r);
        imp.4 = k.float(r, 920 + 4 * n as u16);
    }
    let require_target = s(r);
    let require_lock_on = s(r);
    let target_type = s(r);
    let opponent_res = s(r);
    let check_anim_time = k.int(r, 416);
    let anim_time = range(r);
    let pose = s(r);
    let check_rel_pos = k.int(r, 488);
    let angle = range(r);
    let distance = range(r);
    let height = range(r);
    let opponents_action = s(r);
    let special = s(r);
    let additional = s(r);
    let move_ = s(r);
    let prefer_ragdoll = k.int(r, 632);
    r.skip_line();
    let op = match place(&["and", "or", "xor"], &op) {
        Some(0) => 1,
        Some(1) => 0,
        Some(2) => 2,
        _ => return None,
    };
    let impulses = raw.map(|(check, imp, state, range, interval)| {
        let ih = h(&imp);
        let impulse = if ih == none() { 0 } else { place(&IMPULSES, &imp).map_or(0, |i| i as u32 + 1) };
        let sh = h(&state);
        let state = if sh == none() { 0 } else { place(&STATES, &state).map_or(0, |i| i as u32 + 1) };
        Impulse { check: check != 0, impulse, state, range, interval }
    });
    let (not, res) = match opponent_res.strip_prefix(b"!") {
        Some(rest) => (true, rest.to_vec()),
        None => (false, opponent_res.clone()),
    };
    let ph = h(&pose);
    let opponent_pose = if ph == none() {
        0
    } else if ph == h(b"any") {
        1
    } else {
        place(&POSES, &pose).map_or(0, |i| i as u32 + 2)
    };
    let point = point(&target_type);
    Some(Action {
        name,
        priority,
        bias,
        ai_player_filter: who(&filter),
        flags,
        ranges: [health, attack, defense, martial],
        weapon_class: h(&weapon),
        kind: h(&kind),
        available_from_idle: flags[0],
        impulse_op: op,
        impulses,
        require_target: who(&require_target),
        require_lock_on: who(&require_lock_on),
        required_target_type: point,
        opponent_res_not: not,
        opponent_res: hash_or_0(&res),
        check_opponent_anim_time: check_anim_time != 0,
        opponent_anim_time: anim_time,
        opponent_pose,
        opponents_action_to_test: hash_or_0(&opponents_action),
        check_rel_pos: check_rel_pos != 0,
        rel_point: point,
        height_range: sorted(height),
        distance_range: sorted(distance),
        angle_range: [wrap(rad_end(angle[0])), wrap(rad_end(angle[1]))],
        special_test: hash_or_0(&special),
        additional_action_test: hash_or_0(&additional),
        move_: hash_or_0(&move_),
        prefer_ragdoll: prefer_ragdoll != 0,
    })
}

fn read_result(
    r: &mut CsvReader, k: &mut Kept, name: u32, group: &impl Fn(&[u8]) -> i32, anim: &impl Fn(i32, &[u8]) -> i32,
) -> ResultDef {
    let priority = k.int(r, 640);
    let group_name = s(r);
    let anim_name = s(r);
    let sync = k.int(r, 512);
    let start_time = k.float(r, 508);
    let playback_rate = k.float(r, 500);
    let player_playback_mult = k.float(r, 492);
    let is_loop = k.int(r, 584);
    let blend_in = k.float(r, 484);
    let blend_out = k.float(r, 568);
    let is_an_idle = k.int(r, 520);
    let is_a_block = k.int(r, 648);
    let lock = k.int(r, 528);
    let homing = s(r);
    let angle = range(r);
    let distance = range(r);
    let height = range(r);
    let stop_on_crit = k.int(r, 616);
    let stop_time = k.float(r, 608);
    let desired_angle = range(r);
    let desired_distance = range(r);
    let desired_height = range(r);
    let force = k.int(r, 592);
    let self_damage = k.float(r, 560);
    let drop = k.int(r, 536);
    let disarm = k.int(r, 624);
    let mut bones = [(vec![], 0f32), (vec![], 0.0)];
    for (n, b) in bones.iter_mut().enumerate() {
        b.0 = s(r);
        b.1 = k.float(r, 912 + 4 * n as u16);
    }
    let on_hit = k.float(r, 552);
    let momentum = k.float(r, 600);
    let object_damage = k.float(r, 544);
    let mut branches = [(vec![], [0f32; 2]), (vec![], [0.0; 2]), (vec![], [0.0; 2])];
    for b in branches.iter_mut() {
        b.0 = s(r);
        b.1 = range(r);
    }
    let special = s(r);
    let sound = s(r);
    r.skip_line();
    let (mut g, mut a) = (-1, -1);
    if h(&group_name) != none() {
        g = group(&group_name);
        if h(&anim_name) != none() {
            a = anim(g, &anim_name);
        }
    }
    let homing_type = point(&homing);
    ResultDef {
        name,
        priority,
        anim_group: g,
        anim: a,
        sync_start_time: match sync {
            0 => 0,
            1 => 1,
            -1 => 2,
            _ => 0,
        },
        start_time,
        playback_rate,
        player_playback_mult,
        blend_in,
        blend_out,
        is_loop: is_loop != 0,
        is_an_idle: is_an_idle != 0,
        is_a_block: is_a_block != 0,
        player_lock_enables_melee_mode: lock != 0,
        homing: homing_type != 16,
        homing_target_type: homing_type,
        height_range: sorted(height),
        distance_range: sorted(distance),
        angle_range: [wrap(rad(angle[0])), wrap(rad(angle[1]))],
        stop_on_crit_frame: stop_on_crit != 0,
        stop_time,
        desired_height,
        desired_distance,
        desired_angle: [rad(desired_angle[0]), rad(desired_angle[1])],
        force_immediate_reaction: force != 0,
        self_damage,
        drop_weapon: drop != 0,
        disarm_opponent: disarm != 0,
        on_hit_damage: on_hit,
        applied_ped_move_momentum: momentum,
        object_damage_mult: object_damage,
        special_result: h(&special),
        sound: h(&sound),
        branches: branches.map(|(n, rg)| {
            let x = h(&n);
            (x != none(), if x != none() { x } else { 0 }, rg)
        }),
        strike_bones: bones.map(|(n, radius)| {
            let some = h(&n) != none();
            (some, if some { bone(&n) } else { 0 }, radius)
        }),
    }
}

