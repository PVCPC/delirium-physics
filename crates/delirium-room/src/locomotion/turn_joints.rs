use super::*;
pub(super) fn enabled()->bool {
    static ON:OnceLock<bool>=OnceLock::new();*ON.get_or_init(||std::env::var("DLM_TURN_JOINT_CONTROL").as_deref()==Ok("1"))
}
pub(super) fn gate(finite:bool,t:f32)->f32 {if finite {gait_ramp(t-16.)*if jog_stop::enabled(){1.}else{1.-gait_ramp((t-24.)/2.)}}else{0.}}
pub(super) fn support_gain()->f32 {
    static GAIN:OnceLock<f32>=OnceLock::new();*GAIN.get_or_init(||{
        let v=std::env::var("DLM_CUT_SUPPORT_GAIN").map_or(0.,|v|v.parse::<f32>().expect("invalid cut support gain"));
        assert!(v.is_finite()&&(0.0..=0.25).contains(&v));assert!(v==0.||enabled(),"cut support requires retained joint controls");v
    })
}
pub(super) fn push_release()->f32 {
    static GAIN:OnceLock<f32>=OnceLock::new();*GAIN.get_or_init(||{
        let v=std::env::var("DLM_CUT_PUSH_RELEASE").map_or(0.,|v|v.parse::<f32>().expect("invalid cut push release"));
        assert!(v.is_finite()&&(0.0..=1.0).contains(&v));assert!(v==0.||enabled(),"cut push release requires retained joint controls");v
    })
}
fn command(joint:usize,desired:[f32;3],limits:[f32;6],action:&[f32],w:f32)->[f32;3] {
    let (start,n)=match joint {0=>(0,3),1=>(3,1),2=>(4,3),3=>(7,3),4=>(10,1),5=>(11,3),_=>return desired};
    let values=&action[start..start+n];if w==0.||values.iter().all(|&x|x==0.){return desired;}
    let delta=if n==1 {Vec3::new(values[0].clamp(-1.,1.),0.,0.)}else{Vec3::from_slice(values).clamp_length_max(1.)};
    std::array::from_fn(|k|if k<n {(desired[k]+0.3*w*delta[k]).clamp(limits[2*k],limits[2*k+1])}else{desired[k]})
}
pub(super) fn apply(a:&mut Agent,action:&[f32],w:f32){
    if w==0.||action.iter().all(|&x|x==0.){return;}
    assert_eq!(action.len(),14);let ch=a.character().expect("native leg motors required");
    let effectors:Vec<_>=ch.eff.iter().enumerate().map(|(i,e)|
        (command(i,e.desired,ch.limits(i),action,w),e.muscle_stiffness,e.strength,e.damping)).collect();
    a.set_effectors(&effectors);
}
