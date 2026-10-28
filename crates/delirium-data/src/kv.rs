//! The room's and engine's tuning numbers, locked to the player's own game: `kv.bin` opens with a key made from
//! its NM character file (`key`), so this copy carries none of them readable. `kv!(i)` is slot i, typed as the
//! number it stands for; all 0 until `unlock`.
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

const N: usize = 992;
static K64: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static K32: [AtomicU32; N] = [const { AtomicU32::new(0) }; N];
static OPEN: AtomicBool = AtomicBool::new(false);
static LOCKED: &[u8] = include_bytes!("kv.bin");

/// An NM character file's key: a hash of its joints as read (the same on every platform's file).
pub fn key(j: &[crate::nm_asset::Joint]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    for j in j {
        h.update(j.name.as_bytes());
        h.update(&j.index.to_le_bytes());
        h.update(&[j.three as u8]);
        h.update(&j.flip.map(u8::from));
        let mut f: Vec<&f32> = vec![];
        f.extend(j.pos.iter().chain(&j.twist_axis).chain(&j.lean_axis));
        f.extend(j.limits.iter().flat_map(|l| [&l.0, &l.1]));
        f.extend([&j.muscle_stiffness, &j.damping, &j.strength]);
        f.extend(&j.max_torque);
        for v in f {
            h.update(&v.to_bits().to_le_bytes());
        }
    }
    *h.finalize().as_bytes()
}

/// The numbers opened with `j`'s key (an NM character file's joints); false if this game's files aren't one it
/// was locked to.
pub fn unlock(j: &[crate::nm_asset::Joint]) -> bool {
    if OPEN.load(Relaxed) { return true }
    let k = key(j);
    let (Some(n), Some(rest)) = (LOCKED.get(4..8), LOCKED.get(8..)) else { return false };
    let n = u32::from_le_bytes(n.try_into().unwrap()) as usize;
    let check = blake3::keyed_hash(&k, b"check");
    let Some(slot) = rest.chunks(40).take(n).find(|s| s[..8] == check.as_bytes()[..8]) else { return false };
    let wrap = blake3::keyed_hash(&k, b"wrap");
    let d: [u8; 32] = std::array::from_fn(|i| slot[8 + i] ^ wrap.as_bytes()[i]);
    let mut v = rest[n * 40..].to_vec();
    let mut ks = vec![0; v.len()];
    blake3::Hasher::new_keyed(&d).finalize_xof().fill(&mut ks);
    v.iter_mut().zip(ks).for_each(|(a, b)| *a ^= b);
    if v.len() != N * 12 { return false }
    for (i, e) in v.chunks(12).enumerate() {
        K64[i].store(u64::from_le_bytes(e[..8].try_into().unwrap()), Relaxed);
        K32[i].store(u32::from_le_bytes(e[8..].try_into().unwrap()), Relaxed);
    }
    OPEN.store(true, Relaxed);
    true
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
