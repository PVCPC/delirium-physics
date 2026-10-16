use aes::Aes256;
use aes::cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray};

pub struct ArchiveKey(Aes256);

impl ArchiveKey {

    pub fn from_bytes(key: &[u8; 32]) -> Self {
        Self(Aes256::new(GenericArray::from_slice(key)))
    }

    pub fn find_by_trial(image: &[u8], rpf: &[u8]) -> Option<(Self, [u8; 32])> {
        let le = |d: &[u8], at: usize| u32::from_le_bytes(d[at..at + 4].try_into().unwrap());
        if rpf.get(0..4) != Some(b"RPF2") || rpf.len() < 0x810 || le(rpf, 16) == 0 {
            return None;
        }
        let count = le(rpf, 8);
        let root: [u8; 16] = rpf[0x800..0x810].try_into().unwrap();
        (0..image.len().saturating_sub(32)).step_by(4).find_map(|i| {
            let k: [u8; 32] = image[i..i + 32].try_into().unwrap();
            let key = Self::from_bytes(&k);
            let mut e = root;
            key.decrypt(&mut e);
            let n = le(&e, 12);
            (le(&e, 0) == 0 && le(&e, 8) == 0x8000_0001 && n >= 1 && n < count).then_some((key, k))
        })
    }

    pub fn decrypt(&self, data: &mut [u8]) {
        for block in data.chunks_exact_mut(16) {
            let b = GenericArray::from_mut_slice(block);
            for _ in 0..16 {
                self.0.decrypt_block(b);
            }
        }
    }
}

