use crate::map::hash_bytes;

pub const BUILT_IN: [&str; 101] = [
    "default", "jump_std", "jump_rifle", "ragdoll_trans", "ragdoll_trans_back", "get_up", "get_up_back",
    "cellphone", "cellphone", "climb", "prone", "door", "amb_sit_chair_f", "amb_sit_stool_f", "amb_sit_couch_f",
    "amb_sit_chair_m", "amb_sit_stool_m", "amb_sit_couch_m", "amb_sit_step_m", "amb_sit_wall_m", "arrest", "medic",
    "busted", "AMB@FLEE", "car_bomb", "gun@rifle", "gun@m40a1", "melee", "molotov", "grenade", "CONFIG_SCREEN_F",
    "CONFIG_SCREEN_M", "GUN@COPS", "SWAT_SIGNAL", "cover_l_high_centre", "cover_l_high_corner",
    "cover_l_low_centre", "cover_l_low_corner", "cover_r_high_centre", "cover_r_high_corner", "cover_r_low_centre",
    "cover_r_low_corner", "cover_dive", "injured", "damage_ko", "damage_react", "damage_react_floor",
    "move_m@generic", "move_f@generic", "move_player", "move_rifle", "move_f@armed", "move_combat_strafe",
    "move_combat_strafe_c", "move_crouch", "move_crouch_rifle", "move_rpg", "move_crouch_rpg", "move_melee", "swim",
    "move_crouch_trans", "move_crouch_trans_armed", "move_melee_trans", "move_injured_lower", "move_injured_upper",
    "move_injured_generic", "reaction@male_flee", "gestures@male", "gestures@female", "gestures@car",
    "gestures@car_f", "gestures@hero", "pickup_object", "visemes@m_hi", "visemes@f_hi", "visemes@m_lo",
    "visemes@f_lo", "facials@m_hi", "facials@f_hi", "facials@m_lo", "facials@f_lo", "veh@base", "veh@std",
    "veh@LOW_CONV", "veh@van", "veh@low", "veh@truck", "veh@bus", "veh@train", "veh@HELICOPTER", "veh@boat_speed",
    "veh@busted_std", "veh@busted_low", "veh@busted_van", "veh@busted_truck", "AVOIDS", "EV_DIVES",
    "MOVE_CROUCH_RIFLE", "", "", "",
];

pub const BUILT_IN_SLOTS: [(usize, &str); 13] = [
    (47, "move_m@generic"), (48, "move_f@generic"), (49, "move_player"), (50, "move_rifle"), (51, "move_f@armed"),
    (52, "move_combat_strafe"), (53, "move_combat_strafe_c"), (54, "move_crouch"), (55, "move_crouch_rifle"),
    (56, "move_rpg"), (57, "move_crouch_rpg"), (58, "move_melee"), (59, "swimming"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub name: Vec<u8>,
    pub dictionary: Vec<u8>,
}

impl Group {
    pub fn stored(name: &[u8], dictionary: &[u8]) -> Group {
        let dictionary = dictionary[..dictionary.len().min(24)].to_vec();
        let name = if name.len() >= 32 { [&name[..32], &dictionary[..]].concat() } else { name.to_vec() };
        Group { name, dictionary }
    }
}

const KIND_AT: [(&[u8], usize, u32); 11] = [
    (b"WeaponGroup", 0x20, 0), (b"Version", 0x2C, 0), (b"VehicleDrivebyGroup", 0x30, 0),
    (b"GestureGroup", 0x34, 0), (b"VehicleGroup", 0x3C, 0), (b"FacialGroup", 0x40, 0),
    (b"MovementGroup", 0x44, 0), (b"%d", 0x48, 0), (b"BikeGroup", 0x4C, 0),
    (b"WeaponProjectileGroup", 0x50, 0), (b"VehicleToDrivebyLink", 0x54, 0),
];

const ADDS: [&[u8]; 7] =
    [b"MovementGroup", b"WeaponGroup", b"WeaponProjectileGroup", b"GestureGroup", b"FacialGroup", b"VehicleGroup", b"BikeGroup"];

pub(crate) struct Lines {
    pub(crate) buf: [u8; 512],
    pub(crate) at: usize,
}

impl Lines {
    pub(crate) fn next(&mut self, file: &[u8]) -> Option<Vec<u8>> {
        crate::textlines::read_line(&mut self.buf, file, &mut self.at)?;
        let b = &mut self.buf;
        let mut i = 0;
        while b[i] != 0 {
            if (b[i] as i8) < 0x20 || b[i] == b',' {
                b[i] = b' ';
            }
            i += 1;
        }
        let mut start = 0;
        while b[start] != 0 && (b[start] as i8) < 0x21 {
            start += 1;
        }
        let end = b[start..].iter().position(|&c| c == 0).map_or(b.len(), |e| start + e);
        Some(b[start..end].to_vec())
    }
}

pub(crate) fn tokens(line: &[u8]) -> impl Iterator<Item = &[u8]> {
    line.split(|c| b" ,\t()".contains(c)).filter(|t| !t.is_empty())
}

fn format(token: &[u8]) -> Option<Vec<u8>> {
    #[derive(PartialEq, Clone, Copy)]
    enum St { Normal, Percent, Flag, Width, Dot, Precis, Size }
    let (mut out, mut st, mut i) = (vec![], St::Normal, 0);
    while i < token.len() {
        let c = token[i];
        i += 1;
        st = match (st, c) {
            (St::Normal, b'%') => St::Percent,
            (St::Percent | St::Flag, b' ' | b'+' | b'-' | b'#' | b'0') => St::Flag,
            (St::Percent | St::Flag | St::Width, b'1'..=b'9') | (St::Width, b'0') => St::Width,
            (St::Dot | St::Precis, b'0'..=b'9') => St::Precis,
            (St::Percent | St::Flag | St::Width, b'.') => St::Dot,
            (St::Percent | St::Flag | St::Dot, b'*') => return None,
            (St::Normal, _) => {
                out.push(c);
                St::Normal
            }
            (_, b'h' | b'l' | b'L' | b'w' | b'F' | b'N') => St::Size,
            (_, b'I') if token[i..].starts_with(b"64") || token[i..].starts_with(b"32") => {
                i += 2;
                St::Size
            }
            (_, b'I') if token.get(i).is_some_and(|n| b"diouxX".contains(n)) => St::Size,
            (_, b'c' | b'C' | b'd' | b'i' | b'o' | b'u' | b'x' | b'X' | b'e' | b'E' | b'f' | b'g' | b'G' | b'a' | b'A' | b'n'
               | b'p' | b's' | b'S' | b'Z') => return None,
            _ => {
                out.push(c);
                St::Normal
            }
        };
    }
    Some(out)
}

fn scan_int(t: &[u8], old: i32) -> i32 {
    let t = t.trim_ascii_start();
    let (neg, t) = match t.first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits = t.iter().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return old;
    }
    let n = t[..digits].iter().fold(0i32, |n, d| n.wrapping_mul(10).wrapping_add((d - b'0') as i32));
    if neg { n.wrapping_neg() } else { n }
}

pub fn read(file: &[u8], exe: (u32, &[u8])) -> Option<Vec<Group>> {
    let mut lines = Lines { buf: [0; 512], at: 0 };
    let mut out: Vec<Group> = vec![];
    let mut stack = [0u8; 0x60];
    for (_, s, at) in KIND_AT {
        stack[s..s + 4].copy_from_slice(&at.to_be_bytes());
    }
    let mut written = 0;
    let mut version = 0;
    let is = |stack: &[u8; 0x60], kind: &[u8], k: &[u8]| -> Option<bool> {
        let Some(&(_, s, at)) = KIND_AT.iter().find(|e| e.0 == k) else { return Some(kind == k) };
        let p = u32::from_be_bytes(stack[s..s + 4].try_into().unwrap());
        if p == at {
            return Some(kind == k);
        }
        let from = exe.1.get(p.checked_sub(exe.0)? as usize..)?;
        let n = from.iter().position(|&c| c == 0)?;
        Some(kind == &from[..n])
    };
    while let Some(line) = lines.next(file) {
        if written > 0x38 {
            break;
        }
        if matches!(line.first(), None | Some(b'#')) {
            continue;
        }
        let mut t = tokens(&line);
        let kind = format(t.next()?)?;
        if is(&stack, &kind, b"Version")? {
            version = scan_int(t.next()?, version);
            continue;
        }
        t.next()?;
        let name = format(t.next()?)?;
        written = written.max(name.len() + 1);
        for (d, s) in stack.iter_mut().zip(name.iter().chain([&0])) {
            *d = *s;
        }
        let kind = if name.len() >= 0x60 { name[0x60..].to_vec() } else { kind };
        let known = |out: &[Group], n: &[u8]| {
            BUILT_IN.iter().map(|n| n.as_bytes()).chain(out.iter().map(|g| &g.name[..])).any(|g| g.eq_ignore_ascii_case(n))
        };
        let mut adds = false;
        for k in ADDS {
            if is(&stack, &kind, k)? {
                adds = true;
                break;
            }
        }
        if adds {
            out.push(Group::stored(&name, &name));
            if kind == b"BikeGroup" {
                for _ in 0..6 {
                    t.next()?;
                }
            }
        } else if is(&stack, &kind, b"VehicleToDrivebyLink")? {
            let other = format(t.next()?)?;
            if known(&out, &name) && known(&out, &other) {
                format(t.next()?)?;
            }
        } else if kind == b"VehicleDrivebyGroupV2" || is(&stack, &kind, b"VehicleDrivebyGroup")? {
            let dictionary = if kind == b"VehicleDrivebyGroupV2" { format(&name).and(format(t.next()?))? } else { format(&name)? };
            if !known(&out, &name) {
                out.push(Group::stored(&name, &dictionary));
            }
            for _ in 0..if version > 1 { 8 } else { 6 } {
                t.next()?;
            }
        }
    }
    Some(out)
}

pub fn group_of(groups: &[Group], anims: &[u8]) -> i32 {
    let slot = (!anims.eq_ignore_ascii_case(b"null")).then(|| hash_bytes(anims, 0));
    for k in 0..BUILT_IN.len() {
        let own = BUILT_IN_SLOTS.iter().find(|(i, _)| *i == k).map(|(_, d)| hash_bytes(d.as_bytes(), 0));
        if own == slot {
            return k as i32;
        }
    }
    groups.iter().position(|g| Some(hash_bytes(&g.dictionary, 0)) == slot).map_or(-1, |i| (BUILT_IN.len() + i) as i32)
}

pub fn ambient_groups(file: &[u8], before: &[Vec<u8>]) -> Option<Vec<Group>> {
    const HEADERS: [(&[u8], Option<u8>); 6] = [(b"PROP_SETS:", Some(2)), (b"AMBIENT_GROUP_SETS:", Some(3)), (b"AMBIENT_GROUPS:", Some(0)),
                                               (b"MOVEMENT_GROUPS:", Some(1)), (b"ANIM_GROUP_DEFIN", Some(4)), (b"SetTiming_", None)];
    let mut lines = Lines { buf: [0; 512], at: 0 };
    let (mut mode, mut out) = (0, Vec::<Group>::new());
    while let Some(line) = lines.next(file) {
        if line.first().is_none_or(|&c| c == b'#') {
            continue;
        }
        if let Some(&(_, m)) = HEADERS.iter().find(|h| line.starts_with(h.0)) {
            mode = m.unwrap_or(mode);
            continue;
        }
        let mut t = tokens(&line);
        match mode {
            0 => {
                let name = t.next()?;
                if !before.iter().chain(out.iter().map(|g| &g.name)).any(|n| n.eq_ignore_ascii_case(name)) {
                    out.push(Group::stored(name, name));
                }
            }
            4 => out.push(Group::stored(t.next()?, t.next()?)),
            _ => {}
        }
    }
    Some(out)
}

