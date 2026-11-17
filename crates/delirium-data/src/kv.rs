//! The room's and engine's tuning numbers, read from the player's own game: `kv.map` says where in the PC
//! GTAIV.exe each one is, `kv-disc.map` where in the disc's default.xex (opened); the few that aren't the game's
//! (ours, math constants) are in them as they are, so this copy carries none of the game's. `kv!(i)` is slot i, typed
//! as the number it stands for; all 0 until `unlock`.
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

const N: usize = 1097;
static K64: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static K32: [AtomicU32; N] = [const { AtomicU32::new(0) }; N];
static OPEN: AtomicBool = AtomicBool::new(false);
static MAPS: &[&[u8]] = &[include_bytes!("kv.map"), include_bytes!("kv-disc.map"), include_bytes!("kv-pc1.map"), include_bytes!("kv-pc2.map")];
static RVAS: &[u8] = include_bytes!("kv-rva.map");

/// The release reads its numbers from the game's program (the PC GTAIV.exe or the disc's default.xex), so a PS3 game won't do.
pub const FROM_GAME: bool = true;

/// Opens the numbers from the player's own game program: the PC GTAIV.exe or the disc's default.xex opened
/// (its bytes); false if it isn't a version they're read from.
pub fn unlock(prog: &[u8]) -> bool {
    if OPEN.load(Relaxed) { return true }
    let Some(map) = MAPS.into_iter().find(|m| m.len() == 44 + N * 13 && le(m, 4, 8) == prog.len() as u64 && m[12..44] == *blake3::hash(prog).as_bytes())
    else { return moved(prog) };
    let be = map[..4] == *b"DKX1";
    let rd = |at: usize, k: usize| if be { prog[at..at + k].iter().fold(0u64, |v, &x| v << 8 | x as u64) } else { le(prog, at, k) };
    for (i, r) in map[44..].chunks(13).enumerate() {
        let (w, h) = slot(r, le(r, 1, 4) as usize, &rd);
        K64[i].store(w, Relaxed);
        K32[i].store(h, Relaxed);
    }
    OPEN.store(true, Relaxed);
    true
}

fn le(b: &[u8], at: usize, k: usize) -> u64 { b[at..at + k].iter().rev().fold(0u64, |v, &x| v << 8 | x as u64) }

/// One slot's words from its 13-byte record, its number read at `at` (`rd`: k bytes at an offset).
fn slot(r: &[u8], at: usize, rd: &dyn Fn(usize, usize) -> u64) -> (u64, u32) {
    // the kind (the 0x80 bit: a decimal stored with its sign flipped)
    let neg = if r[0] & 0x80 != 0 { 1u32 << 31 } else { 0 };
    match r[0] & 0x7f {
        0 => (le(r, 1, 8), le(r, 9, 4) as u32),  // as it is
        1 => (rd(at, 4), rd(at, 4) as u32),  // a whole number
        2 => { let h = rd(at, 4) as u32 ^ neg; (wide(h, r[5]), h) }  // a decimal, and how its double is made
        3 => { let w = rd(at, 8) ^ (neg as u64) << 32; (w, (f64::from_bits(w) as f32).to_bits()) }  // a double
        4 => (rd(at, 2), rd(at, 2) as u32),  // a 16-bit whole number
        _ => {  // a whole number built by two instructions: its top half, then its bottom (added signed if r[9])
            let lo = rd(le(r, 5, 4) as usize, 2);
            let v = (rd(at, 2) << 16).wrapping_add(if r[9] == 1 { lo as u16 as i16 as u64 } else { lo }) & 0xffff_ffff;
            (v, v as u32)
        }
    }
}

/// A PC GTAIV.exe packed differently from the builds the maps were made from (another shop's, an unwrapped one):
/// each number read where each known build keeps it in memory, found in this file through its own section table;
/// taken only if every group of `kv-rva.map`'s numbers comes out exactly as the known set (its fingerprints).
fn moved(prog: &[u8]) -> bool {
    let r = RVAS;
    if r.len() < 12 || r[..4] != *b"DKR1" || prog.len() < 0x40 || prog[..2] != *b"MZ" { return false }
    let (m, c) = (le(r, 4, 4) as usize, le(r, 8, 4) as usize);
    let groups = N.div_ceil(c);
    if c == 0 || r.len() != 12 + 4 * m + 4 * m * N + 16 * groups { return false }
    // this exe's sections: (virtual address, file offset, size in the file)
    let pe = le(prog, 0x3c, 4) as usize;
    if pe + 24 > prog.len() { return false }
    let (ns, opt) = (le(prog, pe + 6, 2) as usize, le(prog, pe + 20, 2) as usize);
    let secs: Vec<(u64, u64, u64)> = (0..ns).map(|k| pe + 24 + opt + k * 40).take_while(|&o| o + 40 <= prog.len())
        .map(|o| (le(prog, o + 12, 4), le(prog, o + 20, 4), le(prog, o + 16, 4))).collect();
    let first = secs.iter().map(|s| s.0).min().unwrap_or(0);
    let off = |a: u64| secs.iter().find(|s| s.0 <= a && a < s.0 + s.2).map(|s| s.1 + a - s.0)
        .or(if a < first { Some(a) } else { None }).map(|o| o as usize).filter(|&o| o + 8 <= prog.len());
    let rd = |at: usize, k: usize| le(prog, at, k);
    // each slot's number as each known build has it (None: not there)
    let mut per: Vec<Vec<Option<(u64, u32)>>> = Vec::new();
    for k in 0..m {
        let Some(map) = MAPS.get(le(r, 12 + 4 * k, 4) as usize) else { return false };
        if map.len() != 44 + N * 13 { continue }
        per.push(map[44..].chunks(13).enumerate().map(|(i, rec)| {
            let a = le(r, 12 + 4 * m + 4 * (k * N + i), 4);
            match rec[0] & 0x7f {
                0 => Some(slot(rec, 0, &rd)),
                5 => None,
                _ if a == 0xffff_ffff => None,
                _ => off(a).map(|o| slot(rec, o, &rd)),
            }
        }).collect());
    }
    let check = |g: usize, pick: &[(u64, u32)]| {
        let bytes: Vec<u8> = pick.iter().flat_map(|&(a, b)| a.to_le_bytes().into_iter().chain(b.to_le_bytes())).collect();
        blake3::hash(&bytes).as_bytes()[..16] == r[r.len() - 16 * groups + 16 * g..][..16]
    };
    let mut got = vec![(0u64, 0u32); N];
    for g in 0..groups {
        let span = g * c..((g + 1) * c).min(N);
        // one build's numbers for the whole group (the others' where it has none), else every mix (up to 4096)
        let whole = (0..per.len()).find_map(|k| {
            let pick: Option<Vec<(u64, u32)>> = span.clone().map(|i| per[k][i].or_else(|| per.iter().find_map(|p| p[i]))).collect();
            pick.filter(|p| check(g, p))
        });
        let hit = whole.or_else(|| {
            let cand: Vec<Vec<(u64, u32)>> = span.clone().map(|i| {
                let mut v: Vec<(u64, u32)> = Vec::new();
                for p in &per { if let Some(x) = p[i] { if !v.contains(&x) { v.push(x) } } }
                v
            }).collect();
            let ways = cand.iter().try_fold(1usize, |n, v| n.checked_mul(v.len()).filter(|&n| n <= 4096))?;
            (0..ways).find_map(|mut w| {
                let pick: Vec<(u64, u32)> = cand.iter().map(|v| { let x = v[w % v.len()]; w /= v.len(); x }).collect();
                check(g, &pick).then_some(pick)
            })
        });
        let Some(pick) = hit else { return false };
        got[span].copy_from_slice(&pick);
    }
    for (i, &(w, h)) in got.iter().enumerate() {
        K64[i].store(w, Relaxed);
        K32[i].store(h, Relaxed);
    }
    OPEN.store(true, Relaxed);
    true
}

/// A decimal's 64-bit word from its 32-bit one: widened (0), or its shortest decimal (1) or 9 digits (2) read as a double.
fn wide(h: u32, how: u8) -> u64 {
    let f = f32::from_bits(h);
    match how { 0 => f as f64, 1 => format!("{f}").parse::<f64>().unwrap(), _ => format!("{f:.8e}").parse::<f64>().unwrap() }.to_bits()
}
/// Opened (`unlock`).
pub fn open() -> bool { OPEN.load(Relaxed) }

pub trait Kv: Copy { fn at(i: usize) -> Self; }
impl Kv for f32 { #[inline(always)] fn at(i: usize) -> f32 { f32::from_bits(K32[i].load(Relaxed)) } }
impl Kv for f64 { #[inline(always)] fn at(i: usize) -> f64 { f64::from_bits(K64[i].load(Relaxed)) } }
macro_rules! whole { ($($t:ty)*) => { $(impl Kv for $t { #[inline(always)] fn at(i: usize) -> $t { K64[i].load(Relaxed) as $t } })* } }
whole!(i8 i16 i32 i64 isize u8 u16 u32 u64 usize);
/// Slot i as `_`'s type.
#[inline(always)]
pub fn t<T: Kv>(_: T, i: usize) -> T { T::at(i) }

#[macro_export]
macro_rules! kv {
    ($i:literal) => { $crate::kv::t(0.0, $i) };
    ($i:literal int) => { $crate::kv::t(0, $i) };
    ($i:literal $s:ident) => { $crate::kv::t(0 as $s, $i) };
}
