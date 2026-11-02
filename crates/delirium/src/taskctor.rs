fn put(img: &mut [u8], at: usize, w: u32) {
    img[at..at + delirium_data::kv!(1 int)].copy_from_slice(&w.to_be_bytes());
}

fn base(img: &mut [u8], min: u32, max: u32) {
    img[delirium_data::kv!(7 int)..delirium_data::kv!(437 int)].copy_from_slice(&(min as u16).to_be_bytes());
    img[delirium_data::kv!(437 int)..delirium_data::kv!(94 int)].copy_from_slice(&(max as u16).to_be_bytes());
    put(img, delirium_data::kv!(94 int), 0);
}

pub fn balance(img: &mut [u8], min: u32, max: u32, entity: u32, r7: u32, dir: Option<[u32; 4]>, lean: u32, helper: bool, set: u32) {
    base(img, min, max);
    put(img, delirium_data::kv!(95 int), entity);
    if !helper {
        put(img, delirium_data::kv!(454 int), r7);
    }
    for (i, w) in dir.unwrap_or([0; 4]).into_iter().enumerate() {
        put(img, delirium_data::kv!(93 int) + delirium_data::kv!(1 int) * i, w);
    }
    put(img, delirium_data::kv!(358 int), lean);
    put(img, delirium_data::kv!(462 int), 0);
    put(img, delirium_data::kv!(428 int), set);
}

pub fn flinch(img: &mut [u8], min: u32, max: u32, at: [u32; 4], entity: u32, state: u32) {
    base(img, min, max);
    put(img, delirium_data::kv!(95 int), state);
    put(img, delirium_data::kv!(409 int), entity);
    for (i, w) in at.into_iter().enumerate() {
        put(img, delirium_data::kv!(93 int) + delirium_data::kv!(1 int) * i, w);
    }
    img[delirium_data::kv!(253 int)] = 0;
    img[delirium_data::kv!(355 int)] = 0;
    put(img, delirium_data::kv!(324 int), 0);
}

pub fn flinch_at(img: &[u8]) -> [f32; 3] {
    std::array::from_fn(|i| f32::from_be_bytes(img[delirium_data::kv!(93 int) + delirium_data::kv!(1 int) * i..delirium_data::kv!(379 int) + delirium_data::kv!(1 int) * i].try_into().unwrap()))
}

pub fn script_start(flag: u8, ped_flags: &mut u32) {
    if flag != 0 {
        *ped_flags |= 1 << delirium_data::kv!(476 int);
    }
}

