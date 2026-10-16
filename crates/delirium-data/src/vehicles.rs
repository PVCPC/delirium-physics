use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Handling {
    pub mass: f32,
    pub drag: f32,
    pub percent_submerged: i32,
    pub buoyancy: f32,
    pub centre_of_mass: [f32; 3],
    pub drive_bias: [f32; 2],
    pub gears: u8,
    pub drive_force: f32,
    pub drive_inertia: f32,
    pub drive_max_velocity: f32,
    pub max_flat_velocity: f32,
    pub gear_ratios: Vec<f32>,
    pub brake_force: f32,
    pub brake_bias: [f32; 2],
    pub handbrake: f32,
    pub steering_lock: f32,
    pub traction_max: f32,
    pub traction_min: f32,
    pub traction_lateral: f32,
    pub traction_spring_delta_max: f32,
    pub traction_bias: [f32; 2],
    pub suspension_force: f32,
    pub suspension_comp_damp: f32,
    pub suspension_rebound_damp: f32,
    pub suspension_upper: f32,
    pub suspension_lower: f32,
    pub suspension_raise: f32,
    pub suspension_bias: [f32; 2],
    pub damage: [f32; 4],
    pub seat_offset: f32,
    pub value: i32,
    pub model_flags: u32,
    pub handling_flags: u32,
}

fn split(bias: f32) -> [f32; 2] {
    if bias < 0.1 {
        [0.0, 1.0]
    } else if bias > 0.9 {
        [1.0, 0.0]
    } else {
        [bias * 2.0, (1.0 - bias) * 2.0]
    }
}

pub fn gear_ratios(gears: u8) -> Vec<f32> {
    let n = gears.clamp(1, 7) as usize;
    let mut r = vec![-10.0f32 / 3.0; n + 1];
    r[n] = 1.0;
    if n >= 2 {
        r[1] = 10.0 / 3.0;
        let step = (r[1] / r[n]).powf(1.0 / (n - 1) as f32);
        for k in 2..=n {
            r[k] = r[k - 1] / step;
        }
    }
    r
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(|l| l.split(['#', ';']).next().unwrap_or("").trim()).filter(|l| !l.is_empty())
}

pub fn read_handling(text: &str) -> HashMap<String, Handling> {
    lines(text)
        .filter(|l| l.starts_with(|c: char| c.is_ascii_alphabetic()))
        .filter_map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            let f = |i: usize| t.get(i)?.parse::<f32>().ok();
            let n = |i: usize| t.get(i)?.parse::<f32>().ok().map(|v| v as i32);
            let hex = |i: usize| u32::from_str_radix(t.get(i)?, 16).ok();
            let handling_flags = hex(35)?;
            let damp = if handling_flags & 0x2000_0000 != 0 { 0.3 } else { 0.1 };
            let gears = n(8)?.clamp(1, 7) as u8;
            let percent_submerged = n(3)?;
            let top = f(11)? / 3.6;
            let h = Handling {
                mass: f(1)?,
                drag: f(2)? * 1e-4,
                percent_submerged,
                buoyancy: 100.0 / percent_submerged.max(1) as f32,
                centre_of_mass: [f(4)?, f(5)?, f(6)?],
                drive_bias: split(f(7)?),
                gears,
                drive_force: f(9)?,
                drive_inertia: f(10)?,
                drive_max_velocity: top,
                max_flat_velocity: top * 1.2,
                gear_ratios: gear_ratios(gears),
                brake_force: f(12)?,
                brake_bias: split(f(13)?),
                handbrake: f(14)?,
                steering_lock: f(15)?.to_radians(),
                traction_max: f(16)?,
                traction_min: f(17)?,
                traction_lateral: f(18)?.to_radians(),
                traction_spring_delta_max: f(19)?,
                traction_bias: split(f(20)?),
                suspension_force: f(21)?,
                suspension_comp_damp: f(22)? * damp,
                suspension_rebound_damp: f(23)? * damp,
                suspension_upper: f(24)?,
                suspension_lower: f(25)?,
                suspension_raise: f(26)?,
                suspension_bias: split(f(27)?),
                damage: [f(28)?, f(29)?, f(30)?, f(31)?],
                seat_offset: f(32)?,
                value: n(33)?,
                model_flags: hex(34)?,
                handling_flags,
            };
            Some((t[0].to_ascii_uppercase(), h))
        })
        .collect()
}

pub fn read_flying(text: &str) -> HashMap<String, [f32; 22]> {
    lines(text)
        .filter_map(|l| {
            let mut t = l.strip_prefix('$')?.split_whitespace();
            let id = t.next()?.to_ascii_uppercase();
            Some((id, t.filter_map(|v| v.parse().ok()).collect::<Vec<f32>>().try_into().ok()?))
        })
        .collect()
}

pub fn read_bike(text: &str) -> HashMap<String, [f32; 15]> {
    lines(text)
        .filter_map(|l| {
            let mut t = l.strip_prefix('!')?.split_whitespace();
            let id = t.next()?.to_ascii_uppercase();
            let mut b: [f32; 15] = t.filter_map(|v| v.parse().ok()).collect::<Vec<f32>>().try_into().ok()?;
            for i in [4, 5, 10, 11] {
                b[i] = b[i].to_radians();
            }
            Some((id, b))
        })
        .collect()
}

pub fn read_ide_game_names(text: &str) -> HashMap<String, String> {
    let mut cars = false;
    lines(text).filter_map(|l| {
        match l {
            "cars" => cars = true,
            "end" => cars = false,
            _ if cars => {
                let t: Vec<&str> = l.split(',').map(str::trim).collect();
                return Some((t.first()?.to_ascii_lowercase(), t.get(4)?.to_string()));
            }
            _ => {}
        }
        None
    }).collect()
}

pub fn read_ide_handling(text: &str) -> HashMap<String, (String, [f32; 2])> {
    let mut out = HashMap::new();
    let mut cars = false;
    for l in lines(text) {
        match l {
            "cars" => cars = true,
            "end" => cars = false,
            _ if cars => {
                let t: Vec<&str> = l.split(',').map(str::trim).collect();
                if let [model, _, _, handling, ..] = t.as_slice() {
                    let rim = |i: usize| t.get(i).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                    out.insert(model.to_ascii_lowercase(), (handling.to_ascii_uppercase(), [rim(9), rim(10)]));
                }
            }
            _ => {}
        }
    }
    out
}

pub struct CarColours {
    pub table: Vec<[u8; 3]>,
    pub cars: HashMap<String, Vec<Vec<usize>>>,
}

impl CarColours {
    pub fn combo(&self, model: &str, k: usize) -> Option<Vec<[u8; 3]>> {
        let combos = self.cars.get(&model.to_ascii_lowercase()).filter(|c| !c.is_empty())?;
        combos[k % combos.len()].iter().map(|&i| self.table.get(i).copied()).collect()
    }
}

pub fn read_carcols(text: &str) -> CarColours {
    let (mut table, mut cars) = (vec![], HashMap::new());
    let mut section = "";
    for l in lines(text) {
        match l {
            "col" | "car3" | "car4" | "end" => section = l,
            _ => {
                let t: Vec<&str> = l.split([',', ' ', '\t']).filter(|s| !s.is_empty()).collect();
                match section {
                    "col" => {
                        if let [r, g, b, ..] = t.as_slice() {
                            if let (Ok(r), Ok(g), Ok(b)) = (r.parse(), g.parse(), b.parse()) {
                                table.push([r, g, b]);
                            }
                        }
                    }
                    "car3" | "car4" => {
                        let n = if section == "car3" { 3 } else { 4 };
                        let Some((model, rest)) = t.split_first() else { continue };
                        let ids: Vec<usize> = rest.iter().filter_map(|s| s.parse().ok()).collect();
                        let combos = ids.chunks_exact(n).map(<[usize]>::to_vec).collect();
                        cars.insert(model.to_ascii_lowercase(), combos);
                    }
                    _ => {}
                }
            }
        }
    }
    CarColours { table, cars }
}

pub fn read_materials(text: &str) -> Vec<[f32; 2]> {
    text_rows(text)
        .map(|l| {
            let f: Vec<f32> = l.split_whitespace().skip(3).take(5).map(|v| v.parse().unwrap_or(f32::NAN)).collect();
            match f[..] {
                [_, _, _, grip, wet] if grip.is_finite() && wet.is_finite() => [grip, wet],
                _ => [1.0, 0.0],
            }
        })
        .collect()
}

pub fn read_material_names(text: &str) -> Vec<String> {
    text_rows(text).map(|l| l.split_whitespace().next().unwrap_or_default().to_string()).collect()
}

pub fn read_friction(text: &str) -> Vec<[f32; 2]> {
    text_rows(text)
        .map(|l| {
            let f: Vec<f32> = l.split_whitespace().skip(3).take(2).filter_map(|v| v.parse().ok()).collect();
            if f.len() == 2 { [f[0], f[1]] } else { [1.0, 0.1] }
        })
        .collect()
}

pub fn read_burn(text: &str) -> Vec<[f32; 3]> {
    text_rows(text)
        .map(|l| {
            let f: Vec<f32> = l.split_whitespace().skip(10).take(3).filter_map(|v| v.parse().ok()).collect();
            if f.len() == 3 { [f[0], f[1], f[2]] } else { [0.0; 3] }
        })
        .collect()
}

pub fn material_row(text: &str, name: &str) -> Option<usize> {
    text_rows(text).position(|l| l.split_whitespace().next() == Some(name))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelFx {
    pub skid: f32,
    pub friction: [f32; 2],
    pub group: u16,
}

pub const NO_FX: WheelFx = WheelFx { skid: 9999.9, friction: [9999.9; 2], group: 0 };

pub fn read_wheel_fx(materials: &str, vehiclefx: &str) -> Vec<[WheelFx; 4]> {
    let mut groups = HashMap::<String, [WheelFx; 4]>::new();
    let (mut section, mut row) = (None, 0);
    for l in vehiclefx.lines().map(str::trim) {
        if let Some(name) = l.strip_prefix("VEHFX_INFO_").filter(|n| n.ends_with("START")) {
            section = ["START", "BURST_START", "WET_START", "BURST_WET_START"].iter().position(|s| name == *s);
            row = 0;
        } else if l.starts_with("VEHFX_INFO_") {
            section = None;
        } else if let (Some(k), true) = (section, l.starts_with(|c: char| c.is_ascii_alphabetic())) {
            let f: Vec<&str> = l.split_whitespace().collect();
            let num = |i: usize| f.get(i).and_then(|v| v.parse().ok()).unwrap_or(9999.9);
            let fx = WheelFx { skid: num(1), friction: [num(11), num(12)], group: row };
            groups.entry(f[0].to_ascii_uppercase()).or_insert([NO_FX; 4])[k] = fx;
            row += 1;
        }
    }
    text_rows(materials)
        .map(|l| l.split_whitespace().nth(1).and_then(|g| groups.get(&g.to_ascii_uppercase())).copied().unwrap_or([NO_FX; 4]))
        .collect()
}

fn text_rows(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|l| l.starts_with(|c: char| c.is_ascii_alphabetic()))
}

