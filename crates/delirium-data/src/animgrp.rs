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

pub fn read(file: &[u8]) -> Vec<Group> {
    let mut lines = Lines { buf: [0; 512], at: 0 };
    let mut out: Vec<Group> = vec![];
    let slot = |k: &[u8]| -> Option<usize> {
        Some(match k {
            b"WeaponGroup" => 0x20,
            b"Version" => 0x2C,
            b"VehicleDrivebyGroup" => 0x30,
            b"GestureGroup" => 0x34,
            b"VehicleGroup" => 0x3C,
            b"FacialGroup" => 0x40,
            b"MovementGroup" => 0x44,
            b"BikeGroup" => 0x4C,
            b"WeaponProjectileGroup" => 0x50,
            _ => return None,
        })
    };
    let mut written = 0;
    let is = |kind: &[u8], k: &[u8], written: usize| kind == k && slot(k).is_none_or(|s| written <= s);
    while let Some(line) = lines.next(file) {
        if written > 0x38 {
            break;
        }
        if matches!(line.first(), None | Some(b'#')) {
            continue;
        }
        let mut t = tokens(&line);
        let kind = t.next().unwrap_or_default().to_vec();
        if is(&kind, b"Version", written) {
            continue;
        }
        t.next();
        let name = t.next().unwrap_or_default().to_vec();
        written = written.max(name.len() + 1);
        let kind = if name.len() >= 0x60 { name[0x60..].to_vec() } else { kind };
        if ADDS.iter().any(|k| is(&kind, k, written)) {
            out.push(Group::stored(&name, &name));
        } else if kind == b"VehicleDrivebyGroupV2" || is(&kind, b"VehicleDrivebyGroup", written) {
            let known = BUILT_IN.iter().map(|n| n.as_bytes()).chain(out.iter().map(|g| &g.name[..]))
                .any(|n| n.eq_ignore_ascii_case(&name));
            let dictionary = if kind == b"VehicleDrivebyGroupV2" { t.next().unwrap_or_default().to_vec() } else { name.clone() };
            if !known {
                out.push(Group::stored(&name, &dictionary));
            }
        }
    }
    out
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

