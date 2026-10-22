use std::sync::OnceLock;
static WEIGHT:OnceLock<f32>=OnceLock::new();
static JOG_WEIGHT:OnceLock<f32>=OnceLock::new();
static JOG_SUPPORT_ONLY:OnceLock<f32>=OnceLock::new();
pub(super) fn support_only()->f32 {
    *JOG_SUPPORT_ONLY.get_or_init(|| {
        let w=std::env::var("DLM_JOG_SUPPORT_ONLY").map_or(0.,|v|v.parse::<f32>().expect("invalid isolated jog support"));
        assert!(w.is_finite()&&(0.0..=0.25).contains(&w));
        assert!(w==0.||jog_weight()==0.,"isolated support cannot also change jog motor gains");w
    })
}
pub(super) fn weight()->f32 {
    *WEIGHT.get_or_init(|| {
        let w=std::env::var("DLM_NATIVE_LEG_OWNERSHIP").map_or(0.,|v|v.parse::<f32>().expect("invalid native leg ownership"));
        assert!(w.is_finite()&&(0.0..=1.0).contains(&w));
        assert!(w==0.||super::phase_support::gain()==0.,"native ownership cannot use sidecar support impulses");w
    })
}
pub(super) fn handover(seconds:f32,gait:f32)->f32 {
    (seconds-1.).clamp(0.,1.)*(1.-gait*2.).clamp(0.,1.)
}
pub(super) fn jog_weight()->f32 {
    *JOG_WEIGHT.get_or_init(|| {
        let w=std::env::var("DLM_JOG_NATIVE_OWNERSHIP").map_or(0.,|v|v.parse::<f32>().expect("invalid jog ownership"));
        assert!(w.is_finite()&&(0.0..=0.25).contains(&w));
        assert!(w==0.||super::phase_support::gain()==0.,"jog ownership cannot use sidecar support");w
    })
}
pub(super) fn jog_handover(gait:f32,jog:f32)->f32 {
    if gait<0.5 {0.} else {jog.clamp(0.,1.)}
}
pub(super) fn configured()->bool {weight()>0.||jog_weight()>0.||support_only()>0.}
