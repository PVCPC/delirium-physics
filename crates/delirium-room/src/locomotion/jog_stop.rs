use super::*;
pub(super) fn enabled()->bool {
    static ON:OnceLock<bool>=OnceLock::new();
    *ON.get_or_init(||std::env::var("DLM_JOG_STOP").as_deref()==Ok("1"))
}
pub(super) fn go()->Option<f32> {
    static VALUE:OnceLock<Option<f32>>=OnceLock::new();
    *VALUE.get_or_init(||std::env::var("DLM_STOP_GO").ok().map(|v|v.parse::<f32>().expect("invalid stop go")))
}
pub(super) fn brake_gate(t:f32)->f32 {gait_ramp(t-18.)-go().map_or(0.,|g|gait_ramp(t-g))}
pub(super) fn release(t:f32)->f32 {
    static ON:OnceLock<bool>=OnceLock::new();
    if *ON.get_or_init(||std::env::var("DLM_STOP_RELEASE").as_deref()==Ok("1")) {brake_gate(t)} else {0.}
}
pub(super) fn assist()->f32 {
    static VALUE:OnceLock<f32>=OnceLock::new();
    *VALUE.get_or_init(||std::env::var("DLM_BRAKE_ASSIST").map_or(1.,|v|v.parse::<f32>().expect("invalid brake assist")).clamp(0.,1.))
}
pub(super) fn blend()->f32 {
    static VALUE:OnceLock<f32>=OnceLock::new();
    *VALUE.get_or_init(||std::env::var("DLM_STOP_BLEND").map_or(0.35,|v|v.parse::<f32>().expect("invalid stop blend")))
}

pub(super) fn stop_reward()->f32 {
    static VALUE:OnceLock<f32>=OnceLock::new();
    *VALUE.get_or_init(||std::env::var("DLM_STOP_REWARD").map_or(0.,|v|v.parse::<f32>().expect("invalid stop reward")))
}
pub(super) fn feet()->f32 {
    static VALUE:OnceLock<f32>=OnceLock::new();
    *VALUE.get_or_init(||std::env::var("DLM_STOP_FEET").map_or(0.,|v|v.parse::<f32>().expect("invalid stop feet")))
}
pub(super) fn practice()->bool {
    static ON:OnceLock<bool>=OnceLock::new();
    *ON.get_or_init(||std::env::var("DLM_STOP_PRACTICE").as_deref()==Ok("1"))
}
pub(super) fn braking(desired:Vec3,actual:Vec3,mass:f32,gait:f32)->Vec3 {
    (desired-actual).with_z(0.).clamp_length_max(2.)*(mass*1.5*gait*assist())
}
pub(super) const COLUMNS:[&str;9]=["jog_entry_speed_m_s","stop_latency_s","braking_distance_m","hold_mean_speed_m_s","hold_max_speed_m_s","hold_max_displacement_m","survival_s","max_lateral_drift_m","hold_heading_cost"];
pub(super) fn metrics(frames:&[Vec<Transform>])->[f32;9] {
    let n=frames.len();if n<2{return [0.,41.,0.,10.,10.,10.,n as f32*DT,10.,2.]}
    let origin=frames[0][0].translation;
    let forward=(frames[n.min(60)-1][0].translation-origin).with_z(0.).normalize_or(Vec3::X);
    let side=Vec3::Z.cross(forward);
    let velocity:Vec<_>=frames.windows(2).map(|w|(w[1][0].translation-w[0][0].translation).with_z(0.)/DT).collect();
    let average=|a:usize,b:usize|if b>a {velocity[a..b].iter().map(|v|v.length()).sum::<f32>()/(b-a) as f32}else{0.};
    let entry=average(n.saturating_sub(1).min(14*60),n.saturating_sub(1).min(18*60));
    let cue=19*60;let hold=22*60;
    let (hold,end,still)=if go().is_some() {(1185,1230.min(n),30)} else {(hold,n,60)};
    let stopped=(cue..velocity.len().min(end).saturating_sub(still-1)).find(|&i|velocity[i..i+still].iter().all(|v|v.length()<=0.15));
    let latency=stopped.map_or(41.,|i|(i-cue) as f32*DT);
    let distance=if n>cue {(frames[stopped.unwrap_or(n-1)][0].translation-frames[cue][0].translation).dot(forward).abs()}else{0.};
    let drift=frames.iter().map(|f|(f[0].translation-origin).dot(side).abs()).fold(0.,f32::max);
    let mut result=[entry,latency,distance,10.,10.,10.,n as f32*DT,drift,2.];
    if n>hold+1 {
        let start=frames[hold][0].translation;let velocity=&velocity[..velocity.len().min(end)];let frames=&frames[..end];
        result[3]=average(hold,velocity.len());
        result[4]=velocity[hold..].iter().map(|v|v.length()).fold(0.,f32::max);
        result[5]=frames[hold..].iter().map(|f|(f[0].translation-start).with_z(0.).length()).fold(0.,f32::max);
        result[8]=frames[hold..].iter().map(|f|{
            let heading=f[0].rotation*frames[0][0].rotation.inverse()*forward;
            (1.-heading.dot(forward)).clamp(0.,2.)
        }).sum::<f32>()/(frames.len()-hold) as f32;
    }result
}
