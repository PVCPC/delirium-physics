use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit, generic_array::GenericArray};
use aes::{Aes128, Aes256};

pub type AppKey = (u16, [u8; 32], [u8; 16]);

pub fn app_keys(src: &str) -> Vec<AppKey> {
    src.split("sk_APP_arr.emplace_back(").skip(1).filter_map(|e| {
        let f: Vec<&str> = e.split(',').map(|s| s.trim().trim_matches('"')).collect();
        let rev = u16::from_str_radix(f.get(2)?.trim_start_matches("0x"), 16).ok()?;
        Some((rev, hex(f.get(4)?)?, hex(f.get(5)?)?))
    }).collect()
}

fn hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    (s.len() == N * 2).then(|| std::array::from_fn(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap_or(0)))
}

fn be<const N: usize>(d: &[u8], at: usize) -> Result<u64, String> {
    let b = d.get(at..at + N).ok_or("SELF: truncated")?;
    Ok(b.iter().fold(0u64, |a, &x| a << 8 | x as u64))
}

fn ctr(key: &[u8], iv: &[u8], data: &mut [u8]) {
    let k = Aes128::new(GenericArray::from_slice(key));
    let mut c = u128::from_be_bytes(iv.try_into().unwrap());
    for chunk in data.chunks_mut(16) {
        let mut b = GenericArray::from(c.to_be_bytes());
        k.encrypt_block(&mut b);
        chunk.iter_mut().zip(b.iter()).for_each(|(x, k)| *x ^= k);
        c = c.wrapping_add(1);
    }
}

pub fn decrypt(d: &[u8], keys: &[AppKey]) -> Result<Vec<u8>, String> {
    if d.get(..4) != Some(b"SCE\0") {
        return Err("not a PS3 program (SELF)".into());
    }
    let (rev, meta) = (be::<2>(d, 8)? as u16, be::<4>(d, 12)? as usize);
    let phdr = be::<8>(d, 0x38)? as usize;
    let (_, erk, riv) = keys.iter().find(|k| k.0 == rev).ok_or(format!("no key for this program's revision {rev}"))?;
    let mi = 0x20 + meta;
    let mut info: [u8; 0x40] = d.get(mi..mi + 0x40).ok_or("SELF: truncated")?.try_into().unwrap();
    let k = Aes256::new(GenericArray::from_slice(erk));
    let mut prev = *riv;
    for b in info.chunks_mut(16) {
        let c: [u8; 16] = b.try_into().unwrap();
        let mut x = GenericArray::from(c);
        k.decrypt_block(&mut x);
        b.iter_mut().zip(x.iter().zip(prev)).for_each(|(o, (x, p))| *o = x ^ p);
        prev = c;
    }
    let (mkey, miv) = (&info[..16], &info[32..48]);
    let mut head = d.get(mi + 0x40..mi + 0x60).ok_or("SELF: truncated")?.to_vec();
    ctr(mkey, miv, &mut head);
    let (nsec, nkey) = (be::<4>(&head, 12)? as usize, be::<4>(&head, 16)? as usize);
    let mut all = d.get(mi + 0x40..mi + 0x60 + nsec * 0x30 + nkey * 0x10).ok_or("SELF: truncated")?.to_vec();
    ctr(mkey, miv, &mut all);
    let (secs, ks) = (&all[0x20..0x20 + nsec * 0x30], &all[0x20 + nsec * 0x30..]);
    let key = |i: usize| ks.get(i * 16..i * 16 + 16).ok_or("SELF: bad key index");
    let mut img = vec![];
    for s in secs.chunks(0x30) {
        let (off, size, kind, idx) = (be::<8>(s, 0)? as usize, be::<8>(s, 8)? as usize, be::<4>(s, 16)?, be::<4>(s, 20)? as usize);
        let (enc, ki, ivi, comp) = (be::<4>(s, 32)?, be::<4>(s, 36)? as usize, be::<4>(s, 40)? as usize, be::<4>(s, 44)?);
        if kind != 2 {
            continue;
        }
        let mut x = d.get(off..off + size).ok_or("SELF: truncated segment")?.to_vec();
        if enc == 3 {
            ctr(key(ki)?, key(ivi)?, &mut x);
        }
        if comp == 2 {
            x = miniz_oxide::inflate::decompress_to_vec_zlib(&x).map_err(|e| format!("SELF segment: {e:?}"))?;
        }
        let at = be::<8>(d, phdr + idx * 56 + 8)? as usize;
        if img.len() < at + x.len() {
            img.resize(at + x.len(), 0);
        }
        img[at..at + x.len()].copy_from_slice(&x);
    }
    Ok(img)
}
