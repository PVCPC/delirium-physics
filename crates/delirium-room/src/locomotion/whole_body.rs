use super::*;
pub(super) fn enabled()->bool {
    static ON:OnceLock<bool>=OnceLock::new();
    *ON.get_or_init(||std::env::var("DLM_WHOLE_BODY_TURN").as_deref()==Ok("1"))
}
pub(super) fn gate(finite:bool,t:f32)->f32 {if finite {(t-8.).clamp(0.,1.)}else{0.}}
pub(super) fn arm(delta:&[f32],w:f32)->Quat {
    Quat::from_scaled_axis(Vec3::from_slice(delta).clamp_length_max(1.)*(0.35*w))
}
pub(super) fn spine(a:&mut Agent,delta:&[f32],w:f32) {
    if w==0.||delta.iter().all(|&x|x==0.) {return;}
    let ch=a.character().expect("native spine motors required");
    let effectors:Vec<_>=ch.eff.iter().enumerate().map(|(i,e)|{
        let mut desired=e.desired;
        if let Some(k)=delirium::nmrs::SPINE.iter().position(|&j|j==i) {
            let d=Vec3::from_slice(&delta[3*k..3*k+3]).clamp_length_max(1.)*(0.25*w);
            let l=ch.limits(i);
            for j in 0..3 {desired[j]=(desired[j]+d[j]).clamp(l[2*j],l[2*j+1]);}
        }
        (desired,e.muscle_stiffness,e.strength,e.damping)
    }).collect();
    a.set_effectors(&effectors);
}
