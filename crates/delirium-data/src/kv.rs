//! The room's and engine's tuning numbers, read from the player's own game: `kv.map` says where in the PC
//! GTAIV.exe each one is, `kv-disc.map` where in the disc's default.xex (opened); the few that aren't the game's
//! (ours, math constants) are in them as they are, so this copy carries none of the game's. `kv!(i)` is slot i, typed
//! as the number it stands for; all 0 until `unlock`.
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

const N: usize = 1001;
static K64: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static K32: [AtomicU32; N] = [const { AtomicU32::new(0) }; N];
static OPEN: AtomicBool = AtomicBool::new(false);
static MAPS: [&[u8]; 2] = [include_bytes!("kv.map"), include_bytes!("kv-disc.map")];

/// The release reads its numbers from the game's program (the PC GTAIV.exe or the disc's default.xex), so a PS3 game won't do.
pub const FROM_GAME: bool = true;

/// Opens the numbers from the player's own game program: the PC GTAIV.exe or the disc's default.xex opened
/// (its bytes); false if it isn't a version they're read from.
pub fn unlock(prog: &[u8]) -> bool {
    if OPEN.load(Relaxed) { return true }
    let le = |b: &[u8], at: usize, k: usize| b[at..at + k].iter().rev().fold(0u64, |v, &x| v << 8 | x as u64);
    let Some(map) = MAPS.into_iter().find(|m| m.len() == 44 + N * 13 && le(m, 4, 8) == prog.len() as u64 && m[12..44] == *blake3::hash(prog).as_bytes())
    else { return false };
    let be = map[..4] == *b"DKX1";
    let rd = |at: usize, k: usize| if be { prog[at..at + k].iter().fold(0u64, |v, &x| v << 8 | x as u64) } else { le(prog, at, k) };
    for (i, r) in map[44..].chunks(13).enumerate() {
        // the kind (the 0x80 bit: a decimal stored with its sign flipped) and where
        let (at, neg) = (le(r, 1, 4) as usize, if r[0] & 0x80 != 0 { 1u32 << 31 } else { 0 });
        let (w, h) = match r[0] & 0x7f {
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
        };
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
