use crate::Exit;
use bevy::prelude::*;

#[allow(non_snake_case)]
#[inline]
pub fn MEAT_DAMAGE() -> f32 { delirium_data::kv!(74) }
#[allow(non_snake_case)]
#[inline]
pub fn MEAT_RED() -> &'static LinearRgba { static V: std::sync::OnceLock<LinearRgba> = std::sync::OnceLock::new(); let f = || LinearRgba::rgb(delirium_data::kv!(20), delirium_data::kv!(64), 0.0); if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }

#[allow(non_snake_case)]
#[inline]
pub fn SHOT_EXIT() -> &'static Exit { static V: std::sync::OnceLock<Exit> = std::sync::OnceLock::new(); let f = || Exit { min: 1.0, max: delirium_data::kv!(27), speed: Some(delirium_data::kv!(57)), fail: false }; if delirium_data::kv::open() { V.get_or_init(f) } else { Box::leak(Box::new(f())) } }
pub const EXIT_SLOW: f32 = 2.0;

pub fn exit(b: &str) -> Option<Exit> {
    let e = |min, max, speed, fail| Some(Exit { min, max, speed, fail });
    match b {
        "balance" | "drunk" => e(1.0, delirium_data::kv!(27), Some(delirium_data::kv!(57)), true),
        "highfall" => e(1.0, delirium_data::kv!(27), None, false),
        "relax" => e(1.0, delirium_data::kv!(27), Some(delirium_data::kv!(73)), false),
        "rollup" => e(2.0, delirium_data::kv!(5), Some(delirium_data::kv!(73)), false),
        "brace" => e(2.0, delirium_data::kv!(80), None, false),
        "flinch" => e(1.0, delirium_data::kv!(27), Some(delirium_data::kv!(57)), false),
        "catchfall" | "stairs" => e(2.0, delirium_data::kv!(27), Some(delirium_data::kv!(57)), false),
        _ => None,
    }
}

#[allow(non_snake_case)]
#[inline]
pub fn LYING_HEAD() -> f32 { delirium_data::kv!(24) }
#[allow(non_snake_case)]
#[inline]
pub fn DYING() -> f32 { delirium_data::kv!(28) }
#[allow(non_snake_case)]
#[inline]
pub fn FATAL() -> f32 { delirium_data::kv!(76) }
#[allow(non_snake_case)]
#[inline]
pub fn HIGHFALL_SPEED() -> f32 { delirium_data::kv!(32) }
#[allow(non_snake_case)]
#[inline]
pub fn BALANCE_ROLL() -> f32 { delirium_data::kv!(70) }
#[allow(non_snake_case)]
#[inline]
pub fn BUMP_LEAN() -> f32 { delirium_data::kv!(106) }
pub const BUMP_LEAN_SECS: f32 = 1.0;
#[allow(non_snake_case)]
#[inline]
pub fn BULLET_BACK() -> f32 { delirium_data::kv!(5) }

#[allow(non_snake_case)]
#[inline]
pub fn ROCKET_SPEED() -> f32 { delirium_data::kv!(80) }
#[allow(non_snake_case)]
#[inline]
pub fn ROCKET_LIFE() -> f32 { delirium_data::kv!(104) }
#[allow(non_snake_case)]
#[inline]
pub fn PELLET_SPREAD() -> f32 { delirium_data::kv!(106) }
#[allow(non_snake_case)]
#[inline]
pub fn BURN_DAMAGE() -> f32 { delirium_data::kv!(35) }

pub fn gun_spread(kind: &str, held: f32, len: f32, acc: f32, mut rnd: impl FnMut() -> f32) -> (f32, f32) {
    let full = match kind { "pistol" | "deagle" => delirium_data::kv!(833), "smg" => delirium_data::kv!(12), "ak47" => delirium_data::kv!(5), _ => f32::INFINITY };
    let ped = 2.0 * (held / full).min(1.0);
    let p = ped * (delirium_data::kv!(73) + rnd());
    let w = rnd() * acc * if p < 0.5 { (2.0 * p).max(delirium_data::kv!(57)) } else { 1.0 };
    let most = ped * delirium_data::kv!(128) + acc * if ped * delirium_data::kv!(128) < 0.5 { (delirium_data::kv!(531) * ped).max(delirium_data::kv!(57)) } else { 1.0 };
    (delirium_data::kv!(109) * len * (p + w), delirium_data::kv!(109) * len * most)
}

#[allow(non_snake_case)]
#[inline]
pub fn GRAB_STRENGTH() -> f32 { delirium_data::kv!(167) }
#[allow(non_snake_case)]
#[inline]
pub fn GRAB_STIFFNESS() -> f32 { delirium_data::kv!(27) }
#[allow(non_snake_case)]
#[inline]
pub fn GRAB_DIST() -> f32 { delirium_data::kv!(168) }
#[allow(non_snake_case)]
#[inline]
pub fn REACH_ANGLE() -> f32 { delirium_data::kv!(169) }
#[allow(non_snake_case)]
#[inline]
pub fn ONE_SIDE_REACH() -> f32 { delirium_data::kv!(170) }
#[allow(non_snake_case)]
#[inline]
pub fn SHOVE_PART() -> i32 { delirium_data::kv!(77 int) }
#[allow(non_snake_case)]
#[inline]
pub fn LIFT_IMPULSE() -> f32 { delirium_data::kv!(724) }
#[allow(non_snake_case)]
#[inline]
pub fn STUN_MAG() -> f32 { delirium_data::kv!(106) }
#[allow(non_snake_case)]
#[inline]
pub fn DRUNK_TIMEOUT() -> f32 { delirium_data::kv!(974) }
#[allow(non_snake_case)]
#[inline]
pub fn DRUNK_LEAN() -> f32 { delirium_data::kv!(57) }
#[allow(non_snake_case)]
#[inline]
pub fn LEAN_BUTTON() -> f32 { delirium_data::kv!(106) }
#[allow(non_snake_case)]
#[inline]
pub fn STAYUP_STIFFNESS() -> f32 { delirium_data::kv!(5) }
#[allow(non_snake_case)]
#[inline]
pub fn DODGE_LEAN() -> f32 { delirium_data::kv!(70) }
#[allow(non_snake_case)]
#[inline]
pub fn BRACE_LEAN() -> f32 { delirium_data::kv!(110) }
#[allow(non_snake_case)]
#[inline]
pub fn ARMS_OUT_TIMEOUT() -> f32 { delirium_data::kv!(128) }
