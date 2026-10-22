use bevy::prelude::*;
use std::sync::OnceLock;
static PRIOR:OnceLock<super::Net>=OnceLock::new();
static PATH_GAIN:OnceLock<f32>=OnceLock::new();
static GAIT_PRIOR:OnceLock<super::Net>=OnceLock::new();
static JOG_LEG_AUTHORITY:OnceLock<f32>=OnceLock::new();
static TRANSITION_PRIOR:OnceLock<Option<super::Net>>=OnceLock::new();
static WALK_PRIOR:OnceLock<Option<super::Net>>=OnceLock::new();
static BALANCE_PRIOR:OnceLock<Option<super::Net>>=OnceLock::new();
static ANKLE_PRIOR:OnceLock<Option<super::Net>>=OnceLock::new();
static STRAIGHT_ROUTE:OnceLock<bool>=OnceLock::new();
static JOG_ROUTE_GAIN:OnceLock<f32>=OnceLock::new();
pub(super) fn jog_route_gain(fraction:f32)->f32 {
    let gain=*JOG_ROUTE_GAIN.get_or_init(|| {
        let gain=std::env::var("DLM_JOG_ROUTE_GAIN").map_or(1.,|v|v.parse::<f32>().expect("invalid jog route gain"));
        assert!(gain.is_finite()&&(1.0..=2.0).contains(&gain));gain
    });
    route_gain(fraction,gain)
}
fn route_gain(fraction:f32,gain:f32)->f32 {1.+(gain-1.)*fraction.clamp(0.,1.)}
pub(super) fn straight_route()->bool {
    *STRAIGHT_ROUTE.get_or_init(||std::env::var("DLM_STRAIGHT_ROUTE").as_deref()==Ok("1"))
}
pub(super) fn route_shift(task:Vec3,captured:Vec3,distance:f32)->Vec3 {
    (task-captured)*distance
}
pub(super) fn ankle_prior()->Option<&'static super::Net> {
    ANKLE_PRIOR.get_or_init(||std::env::var("DLM_ANKLE_PRIOR").ok().map(|path| {
        let net=super::Net::load(&path).expect("invalid immutable ankle prior");
        assert_eq!((net.sizes[0],net.sizes[3]),(333,50));net
    })).as_ref()
}
pub(super) fn ankle_correction(action:Vec3,anchor:Vec3)->Vec3 {
    (action-anchor).clamp_length_max(1.0)*0.12
}
pub(super) fn balance_prior()->Option<&'static super::Net> {
    BALANCE_PRIOR.get_or_init(||std::env::var("DLM_BALANCE_PRIOR").ok().map(|path| {
        let net=super::Net::load(&path).expect("invalid immutable balance prior");
        assert!(matches!(net.sizes[0],333|337)&&net.sizes[3]==50);net
    })).as_ref()
}
pub(super) fn balance_action(index:usize,base:f32,action:f32,anchor:f32,jog:f32)->f32 {
    if index>=4 {return base;}
    let delta=0.5*(1.-jog.clamp(0.,1.))*(action-anchor).clamp(-1.,1.);
    if delta==0. {base} else {base+delta}
}

pub(super) fn walk_prior()->Option<&'static super::Net> {
    WALK_PRIOR.get_or_init(||std::env::var("DLM_WALK_REFERENCE_PRIOR").ok().map(|path| {
        let net=super::Net::load(&path).expect("invalid immutable walk-reference prior");
        assert_eq!((net.sizes[0],net.sizes[3]),(333,50));net
    })).as_ref()
}

pub(super) fn walking_weight(brisk:f32)->f32 {0.5*(1.-brisk.clamp(0.,1.))}
pub(super) fn transition_weight(brisk:f32,jog:f32)->f32 {0.25*brisk.clamp(0.,1.)*(1.-jog.clamp(0.,1.))}

pub(super) fn walking_action(index:usize,base:f32,action:f32,anchor:f32,brisk:f32)->f32 {
    if !(28..46).contains(&index) {return base;}
    let delta=walking_weight(brisk)*(action-anchor).clamp(-1.,1.);
    if delta==0. {base} else {base+delta}
}

pub(super) fn transition_prior()->Option<&'static super::Net> {
    TRANSITION_PRIOR.get_or_init(||std::env::var("DLM_TRANSITION_PRIOR").ok().map(|path| {
        let net=super::Net::load(&path).expect("invalid immutable transition prior");
        assert_eq!((net.sizes[0],net.sizes[3]),(333,50));net
    })).as_ref()
}

pub(super) fn transition_action(index:usize,base:f32,action:f32,anchor:f32,brisk:f32,jog:f32)->f32 {
    if !(28..46).contains(&index) {return base;}
    let delta=transition_weight(brisk,jog)*(action-anchor).clamp(-1.,1.);
    if delta==0. {base} else {base+delta}
}

pub(super) fn gait_prior()->&'static super::Net {
    GAIT_PRIOR.get_or_init(|| {
        let path=std::env::var("DLM_GAIT_PRIOR").expect("jog leg learning requires its preserved walking controller");
        let net=super::Net::load(&path).expect("invalid preserved gait controller");
        assert_eq!((net.sizes[0],net.sizes[3]),(331,48));net
    })
}

pub(super) fn protected_leg_action(index:usize,prior:f32,action:f32,fraction:f32)->f32 {
    let gain=*JOG_LEG_AUTHORITY.get_or_init(|| {
        let gain=std::env::var("DLM_JOG_LEG_AUTHORITY").map_or(0.25,|x|x.parse::<f32>().expect("invalid jog leg authority"));
        assert!(gain.is_finite()&&(0.25..=1.0).contains(&gain));gain
    });
    protected_leg_action_with_gain(index,prior,action,fraction,gain)
}

fn protected_leg_action_with_gain(index:usize,prior:f32,action:f32,fraction:f32,gain:f32)->f32 {
    if index>=46 {return action;}
    if index<28||fraction<=0. {return prior;}
    prior+gain*fraction.clamp(0.,1.)*(action-prior)
}

pub(super) fn path_gain()->f32 {
    *PATH_GAIN.get_or_init(||std::env::var("DLM_PATH_GAIN").ok().and_then(|x|x.parse::<f32>().ok())
        .filter(|x|x.is_finite()&&(0.0..=1.0).contains(x)).unwrap_or(1.0))
}

pub(super) fn steering(previous:f32,cross_track:f32,lateral_velocity:f32,gain:f32)->f32 {
    let desired=(-0.08*cross_track-0.25*lateral_velocity).clamp(-0.14,0.14)*gain;
    previous+(desired-previous).clamp(-0.003,0.003)
}

pub(super) fn prior()->&'static super::Net {
    PRIOR.get_or_init(|| {
        let path=std::env::var("DLM_ALIGNMENT_PRIOR").expect("alignment requires a preserved walking policy");
        let net=super::Net::load(&path).expect("invalid alignment walking prior");
        assert_eq!((net.sizes[0],net.sizes[3]),(323,46));net
    })
}

pub(super) fn bounded_action(prior:f32, action:f32, fraction:f32)->f32 {
    let authority=0.20+0.60*fraction.clamp(0.,1.);
    (prior+authority*(action-prior).clamp(-1.,1.)).clamp(-1.,1.)
}

pub(super) fn knee(fr:&[Transform],hip:usize,knee:usize,foot:usize,side:Vec3)->f32 {
    let h=fr[hip].translation;let line=fr[foot].translation-h;
    let v=fr[knee].translation-h;
    let bend=v-line*(v.dot(line)/line.length_squared().max(1e-6));
    bend.dot(side)
}

pub(super) fn joint_knee(hip:Vec3,knee:Vec3,ankle:Vec3,side:Vec3)->f32 {
    let line=ankle-hip;let v=knee-hip;
    (v-line*(v.dot(line)/line.length_squared().max(1e-6))).dot(side)
}

pub(super) fn terms(fr:&[Transform],target:&[Transform],actual_heading:Quat,target_heading:Quat,
    task_forward:Vec3,lateral_velocity:f32,slip:f32)->[f32;4] {
    let side=actual_heading*Vec3::Y;let desired_side=target_heading*Vec3::Y;
    let knee_error=[(1,2,3),(4,5,6)].iter().map(|&(h,k,f)|
        (knee(fr,h,k,f,side)-knee(target,h,k,f,desired_side)).powi(2)).sum::<f32>()*0.5;
    let track=(fr[0].translation-target[0].translation).dot(Vec3::Z.cross(task_forward));
    let forward=actual_heading*Vec3::X;let desired=target_heading*Vec3::X;
    let heading=forward.truncate().normalize_or_zero().angle_to(desired.truncate().normalize_or_zero());
    [knee_error,track*track,heading*heading,slip+0.2*lateral_velocity*lateral_velocity]
}

pub(super) fn cost(t:[f32;4])->f32 {
    20.0*t[0].min(0.01)+0.15*(t[1]/0.09).min(4.0)+0.5*t[2].min(0.5)+0.3*t[3].min(2.0)
}

pub(super) fn placement_cost(actual:f32,reference:f32)->f32 {
    0.12*((actual-reference)/0.16).clamp(-2.,2.).powi(2)
}

