use std::{fs::File,io::Write,sync::{Mutex,OnceLock}};
static TRACE:OnceLock<Option<Mutex<File>>>=OnceLock::new();

pub(super) fn trace(values:[f32;20]) {
    let Some(file)=TRACE.get_or_init(||std::env::var_os("DLM_REWARD_TRACE").map(|path| {
        let mut file=File::options().write(true).create_new(true).open(path).expect("new reward trace path required");
        writeln!(file,"step,fraction,practice,dense,support_cost,contact_cost,lateral,jitter,alignment_cost,reward,actual_vz,target_vz,left_height,right_height,target_left_height,target_right_height,left_world_touch,right_world_touch,torso_upright,placement_cost").unwrap();Mutex::new(file)
    })) else {return;};
    let mut file=file.lock().expect("reward trace lock");
    for (i,x) in values.iter().enumerate() {if i>0 {write!(file,",").unwrap();}write!(file,"{x:.8}").unwrap();}
    writeln!(file).unwrap();
}
