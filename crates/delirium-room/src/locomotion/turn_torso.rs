use super::*;
pub(super) fn gain()->f32 {
    static GAIN:OnceLock<f32>=OnceLock::new();
    *GAIN.get_or_init(||{
        let g=std::env::var("DLM_TURN_TORSO_GAIN").map_or(0.,|v|v.parse::<f32>().expect("invalid turn torso gain"));
        assert!(g.is_finite()&&(0.0..=1.0).contains(&g));g
    })
}
fn weight(finite:bool,t:f32,g:f32)->f32 {if finite {g*(t-8.).clamp(0.,1.)}else{0.}}
pub(super) fn apply(a:&mut Agent,target:&[Transform],finite:bool,t:f32) {
    let w=weight(finite,t,gain());if w==0. {return;}
    let ch=a.character().expect("native spine effectors required");
    let rows=delirium::nmrs::incoming_rows(a.doll.nm,target);
    let effectors:Vec<_>=ch.eff.iter().enumerate().map(|(i,e)|{
        let desired=if delirium::nmrs::SPINE.contains(&i) {
            let l=ch.limits(i);let goal=ch.incoming_angle(i,&rows);
            std::array::from_fn(|j|e.desired[j]+w*(goal[j].clamp(l[2*j],l[2*j+1])-e.desired[j]))
        }else{e.desired};
        (desired,e.muscle_stiffness,e.strength,e.damping)
    }).collect();
    a.set_effectors(&effectors);
}
