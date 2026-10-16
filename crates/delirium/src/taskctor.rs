fn put(img: &mut [u8], at: usize, w: u32) {
    img[at..at + 4].copy_from_slice(&w.to_be_bytes());
}

fn base(img: &mut [u8], min: u32, max: u32) {
    img[32..34].copy_from_slice(&(min as u16).to_be_bytes());
    img[34..36].copy_from_slice(&(max as u16).to_be_bytes());
    put(img, 36, 0);
}

pub fn balance(img: &mut [u8], min: u32, max: u32, entity: u32, r7: u32, dir: Option<[u32; 4]>, lean: u32, helper: bool, set: u32) {
    base(img, min, max);
    put(img, 40, entity);
    if !helper {
        put(img, 192, r7);
    }
    for (i, w) in dir.unwrap_or([0; 4]).into_iter().enumerate() {
        put(img, 48 + 4 * i, w);
    }
    put(img, 80, lean);
    put(img, 84, 0);
    put(img, 224, set);
}

pub fn flinch(img: &mut [u8], min: u32, max: u32, at: [u32; 4], entity: u32, state: u32) {
    base(img, min, max);
    put(img, 40, state);
    put(img, 44, entity);
    for (i, w) in at.into_iter().enumerate() {
        put(img, 48 + 4 * i, w);
    }
    img[64] = 0;
    img[65] = 0;
    put(img, 68, 0);
}

pub fn flinch_at(img: &[u8]) -> [f32; 3] {
    std::array::from_fn(|i| f32::from_be_bytes(img[48 + 4 * i..52 + 4 * i].try_into().unwrap()))
}

pub fn script_start(flag: u8, ped_flags: &mut u32) {
    if flag != 0 {
        *ped_flags |= 1 << 29;
    }
}

