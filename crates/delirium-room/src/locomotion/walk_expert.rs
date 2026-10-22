use std::sync::OnceLock;
static EXPERT:OnceLock<Option<super::Net>>=OnceLock::new();
static ANKLES:OnceLock<Option<super::Net>>=OnceLock::new();
pub(super) fn ankle_expert()->Option<&'static super::Net>{
    ANKLES.get_or_init(||std::env::var("DLM_WALKING_ANKLE_EXPERT").ok().map(|path|{
        let n=super::Net::load(&path).expect("invalid walking ankle expert");assert_eq!((n.sizes[0],n.sizes[3]),(351,4));n
    })).as_ref()
}
pub(super) fn ankle_action(student:f32,expert:f32,gait:f32)->f32 {
    let w=learned_share(gait);if w==0. {expert}else if w==1. {student}else{expert+w*(student-expert)}
}
pub(super) fn expert()->Option<&'static super::Net> {
    EXPERT.get_or_init(||std::env::var("DLM_WALKING_EXPERT").ok().map(|path| {
        let net=super::Net::load(&path).expect("invalid walking expert");
        assert_eq!((net.sizes[0],net.sizes[3]),(333,50));net
    })).as_ref()
}
pub(super) fn learned(gait:f32)->bool {learned_share(gait)==1.}
fn learned_share(gait:f32)->f32 {let x=(gait*2.).clamp(0.,1.);x*x*(3.-2.*x)}
pub(super) fn blend(action:&[f32],expert:&[f32],gait:f32)->Vec<f32> {
    assert!(matches!(action.len(),50|54|90|104));assert_eq!(expert.len(),50);
    let w=learned_share(gait);
    action.iter().enumerate().map(|(i,&a)|if i<28||i>=50||w==1. {a} else if w==0. {expert[i]} else {expert[i]+w*(a-expert[i])}).collect()
}
