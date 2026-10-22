use super::*;
static REFERENCE: OnceLock<Motion> = OnceLock::new();
struct FootShape{rotation:Quat,radius:f32,half:f32,boxed:Option<Vec3>}
static FEET:OnceLock<[FootShape;2]>=OnceLock::new();

pub(super) fn configure(build:&Build, walk:&Motion)->Result<(),String>{
    if jog_stop::enabled()&&(tempo_practice()||!turn_joints::enabled()){return Err("Jog stop requires joint authority and no cut tempo warp".into())}
    if (tempo_practice()||walk_expert::ankle_expert().is_some())&&!turn_joints::enabled(){return Err("tempo practice and walking ankle retention require the finite cut controller".into())}
    if turn_joints::enabled()&&!whole_body::enabled(){return Err("cut joint control requires whole-body finite reference".into())}
    if reset_balance_enabled()&&(!whole_body::enabled()||std::env::var("DLM_TURN_REFERENCE").map_or(true,|s|s.is_empty())) {return Err("reference balance reset requires explicit whole-body finite-turn practice".into())}
    if whole_body::enabled()&&(!preview_enabled()||!signal_enabled()||std::env::var("DLM_TURN_REFERENCE").map_or(true,|s|s.is_empty())) {return Err("whole-body control needs explicit finite reference, preview and turn signal".into())}
    if duration()!=ORIGINAL_DURATION&&std::env::var("DLM_TURN_REFERENCE").map_or(true,|s|s.is_empty()) {return Err("turn duration needs an explicit finite reference".into())}
    if preview_enabled()&&std::env::var("DLM_TURN_REFERENCE").map_or(true,|s|s.is_empty()){
        return Err("turn preview needs an explicit finite reference".into())
    }
    let Ok(path)=std::env::var("DLM_TURN_REFERENCE") else {return Ok(())};
    if path.is_empty(){return Ok(())}
    if tracking::mode()!=17||!foot_feedback::enabled(){return Err("finite turn requires the341-input foot-state controller".into())}
    let mut motion=Motion::load(&path,build)?;
    if motion.duration<59.9 {return Err("finite turn reference must cover the full60-second trial".into())}
    let entry=(motion.frames[30][0].translation-motion.frames[0][0].translation).with_z(0.).normalize_or_zero();
    if entry.length()<0.5{return Err("finite turn has no initial travel heading".into())}
    let turn=Quat::from_rotation_z(walk.stride.y.atan2(walk.stride.x)-entry.y.atan2(entry.x));
    for frame in &mut motion.frames {for body in frame {body.translation=turn*body.translation;body.rotation=turn*body.rotation;}}
    let agent=learn::agent(build,&walk.frames[0]);let body:Vec<_>=agent.doll.frames().collect();let bounds:Vec<_>=agent.doll.capsules().collect();
    FEET.set([3,6].map(|k|FootShape{rotation:body[k].rotation.inverse()*bounds[k].rotation,
        radius:build.parts[k].radius,half:build.parts[k].half,boxed:build.parts[k].boxed})).map_err(|_|"turn foot calibration already configured")?;
    REFERENCE.set(motion).map_err(|_|"turn reference already configured".into())
}
pub(super) fn enabled()->bool {REFERENCE.get().is_some()}
pub(super) fn reset_balance_enabled()->bool{
    static ON:OnceLock<bool>=OnceLock::new();*ON.get_or_init(||std::env::var("DLM_REFERENCE_BALANCE_RESET").as_deref()==Ok("1"))
}
pub(super) fn signal_enabled()->bool{
    static VALUE:OnceLock<bool>=OnceLock::new();*VALUE.get_or_init(||std::env::var("DLM_TURN_SIGNAL_REWARD").as_deref()==Ok("1"))
}
fn learning_value(terms:[f32;4],heading:f32,support_cost:f32,cross:f32,jitter:f32,progress:f32,posture:f32)->f32{
    let pose=0.5*(terms[0]+terms[2]);
    let style=0.35*pose+0.20*terms[1]+0.15*terms[3]+0.15*heading.clamp(0.,1.)
        +0.10/(1.+support_cost.max(0.))+0.05/(1.+cross*cross);
    0.05+(0.2+0.8*progress.clamp(0.,1.))*style*posture.clamp(0.,1.)-0.03*jitter.clamp(0.,1.)
}
pub(super) fn learning_reward(a:&Agent,build:&Build,c:&Controller,motion:&Motion,q:&Quality)->f32{
    let t=c.elapsed as f32*DT+c.jog.as_ref().map_or(0.,|j|j.offset);
    if !signal_enabled()||!c.turning()||t<=16. {return q.reward}
    let target=c.target(motion,c.time);let next=c.target(motion,c.time+DT);
    let frames:Vec<_>=a.doll.frames().collect();let bounds:Vec<_>=a.doll.capsules().collect();
    let forward=c.task_forward(&target);let actual=c.body_heading(&frames)*Vec3::X;
    let desired_velocity=(next[0].translation-target[0].translation)/DT;
    let velocity=a.doll.motion(0).0;
    let requested=desired_velocity.with_z(0.);
    let progress=if requested.length_squared()<0.25 {(-(velocity-requested).with_z(0.).length_squared()).exp()}
        else {velocity.dot(requested)/requested.length_squared()};
    let mut planned=target.clone();let mut mismatch=0.;
    for k in [3,6]{planned[k].rotation*=frames[k].rotation.inverse()*bounds[k].rotation;
        mismatch+=0.25*f32::from((a.doll.touch[k]&0x20!=0)!=(bottom(build,&planned,k)<0.025));}
    let desired_vz=(next[0].translation.z-target[0].translation.z)/DT;
    let support=mismatch+0.6*(velocity.z-desired_vz).clamp(-2.,2.).powi(2);
    let posture=(frames[12].translation-frames[0].translation).normalize_or_zero().dot(
        (target[12].translation-target[0].translation).normalize_or_zero());
    let cross=(frames[0].translation-target[0].translation).dot(Vec3::Z.cross(forward));
    let value=learning_value(q.tracking,0.5*(1.+actual.dot(forward)),support,cross,q.jitter,progress,posture);
    let stop=if jog_stop::enabled()&&jog_stop::stop_reward()>0. {
        let feet:f32=[3,6].iter().map(|&k|(a.doll.motion(k).0-(next[k].translation-target[k].translation)/DT).with_z(0.).length_squared()).sum();
        jog_stop::stop_reward()*jog_stop::brake_gate(t)*(-4.*(velocity-requested).with_z(0.).length_squared()-jog_stop::feet()*feet).exp()
    } else {0.};
    q.reward+(value-q.reward)*gait_ramp(t-16.)+stop
}
pub(super) const ORIGINAL_DURATION:f32=52./30.;
pub(super) fn duration()->f32{
    static VALUE:OnceLock<f32>=OnceLock::new();*VALUE.get_or_init(||{
        let d=std::env::var("DLM_TURN_DURATION").map_or(ORIGINAL_DURATION,|v|v.parse::<f32>().expect("invalid turn duration"));
        assert!(d.is_finite()&&(ORIGINAL_DURATION..=3.).contains(&d),"turn duration outside captured-to3s curriculum");d
    })
}
pub(super) fn tempo_practice()->bool {
    static ON:OnceLock<bool>=OnceLock::new();*ON.get_or_init(||std::env::var("DLM_TURN_TEMPO_PRACTICE").as_deref()==Ok("1"))
}
pub(super) fn practice_duration(index:usize,resets:usize)->f32 {
    if !tempo_practice(){return duration()}
    [3.,2.5,3.,ORIGINAL_DURATION][(index+resets)%4]
}
fn source_time_for(t:f32,d:f32)->f32{
    if d==ORIGINAL_DURATION||t<=18. {return t}
    if t>=18.+d {return t-(d-ORIGINAL_DURATION)}
    let u=(t-18.)/d;let a=2.*(ORIGINAL_DURATION/d-1.);
    18.+d*(u+a*(0.5*u-(std::f32::consts::TAU*u).sin()/(2.*std::f32::consts::TAU)))
}
pub(super) fn return_step()->usize {20*60+((duration()-ORIGINAL_DURATION)*60.).round() as usize}
pub(super) fn preview_enabled()->bool{
    static VALUE:OnceLock<bool>=OnceLock::new();*VALUE.get_or_init(||std::env::var("DLM_TURN_PREVIEW_FEEDBACK").as_deref()==Ok("1"))
}
pub(super) fn preview(control:&Controller,motion:&Motion,inverse:Quat)->[f32;8]{
    let mut result=[0.;8];
    for (i,ahead) in [0.25,0.5].into_iter().enumerate(){
        let target=control.target(motion,control.time+ahead);
        let next=control.target(motion,control.time+ahead+DT);
        let heading=inverse*(control.body_heading(&target)*Vec3::X);
        let velocity=inverse*((next[0].translation-target[0].translation)/DT)*0.2;
        result[i*4..i*4+4].copy_from_slice(&[heading.x,heading.y,velocity.x,velocity.y]);
    }
    result.map(|v|v.clamp(-4.,4.))
}
pub(super) fn entry_control()->bool{
    static VALUE:OnceLock<bool>=OnceLock::new();*VALUE.get_or_init(||std::env::var("DLM_TURN_ENTRY_CONTROL").as_deref()==Ok("1"))
}
pub(super) fn route_anchor()->bool{
    static VALUE:OnceLock<bool>=OnceLock::new();*VALUE.get_or_init(||std::env::var("DLM_TURN_ROUTE_ANCHOR").as_deref()==Ok("1"))
}
pub(super) fn entry_window(t:f32)->f32{gait_ramp(t-17.)*(1.-gait_ramp(t-20.))}
pub(super) fn entry_gain(t:f32,actual:Vec3,wanted:Vec3)->f32{
    let aligned=actual.dot(wanted).clamp(0.,1.);let window=entry_window(t);
    1.-window+window*aligned*aligned
}
pub(super) fn native_input(a:&mut Agent,build:&Build,world:&Collision,state:&mut delirium::msg::State,
    direction:Vec3,wander:&mut delirium::playerctl::Wander,rand:&mut delirium_data::rng::Rand,gain:f32){
    let ped=a.doll.frames().next().unwrap().translation;
    let (force,message)=delirium::playerctl::frame((0,-127),(-direction.x).atan2(direction.y),ped,a.doll.mass(),wander,rand);
    a.push.push((0,ped,force*gain));delirium::shottask::apply(a,build,world,state,&[message]);
}
fn fit_floor(frames:&mut [Transform],weight:f32){
    let min=[3,6].into_iter().zip(FEET.get().unwrap()).map(|(k,p)|{
        let up=(frames[k].rotation*p.rotation).inverse()*Vec3::Z;
        frames[k].translation.z-p.boxed.map_or(p.radius+p.half*up.y.abs(),|half|up.abs().dot(half))
    }).fold(f32::INFINITY,f32::min);
    let lift=(0.002-min).max(0.);
    assert!(lift<0.12,"finite target needs more than12cm calibrated floor correction: {lift}");
    for f in frames{f.translation.z+=weight*lift;}
}
pub(super) fn mix(t:f32)->f32 {gait_ramp((t-8.)/6.)}
pub(super) fn sample(motion:&Motion,time:f32)->Vec<Transform>{super::memo(motion.id,1,time,||sample_(motion,time))}
fn sample_(motion:&Motion,time:f32)->Vec<Transform>{
    let t=(time/DT).clamp(0.,(motion.frames.len()-1) as f32);
    let i=(t.floor() as usize).min(motion.frames.len()-2);let beta=(t-i as f32).clamp(0.,1.);
    let mut out:Vec<_>=motion.frames[i].iter().zip(&motion.frames[i+1]).enumerate().map(|(k,(a,b))|if super::parented(&motion.anchors,k){Transform::IDENTITY}else{
        Transform::from_translation(a.translation.lerp(b.translation,beta)).with_rotation(a.rotation.slerp(b.rotation,beta))}).collect();
    for (k,&(parent,ap,ac)) in motion.anchors.iter().enumerate(){if let Some(p)=parent{
        let a=motion.frames[i][p].rotation.inverse()*motion.frames[i][k].rotation;
        let b=motion.frames[i+1][p].rotation.inverse()*motion.frames[i+1][k].rotation;
        out[k].rotation=out[p].rotation*a.slerp(b,beta);
        out[k].translation=out[p].translation+out[p].rotation*ap-out[k].rotation*ac;
    }}out
}
pub(super) fn target(control:&Controller,walk:&Motion,time:f32)->Vec<Transform>{
    let motion=REFERENCE.get().unwrap();let jog=control.jog.as_ref().unwrap();
    let duration=if jog.practice {jog.practice_duration}else{duration()};
    let t=control.elapsed as f32*DT+jog.offset+time-control.time;
    let origin=motion.frames[0][0].translation.with_z(0.);
    let shift=control.walk.as_ref().unwrap().start[0].translation.with_z(0.)+control.turn_origin.unwrap_or(Vec3::ZERO);
    let finite:Vec<_>=sample(motion,source_time_for(t,duration)).into_iter().map(|f|Transform::from_translation(control.heading*(f.translation-origin)+shift)
        .with_rotation(control.heading*f.rotation)).collect();
    let dynamic=control.straight_target(walk,time);
    if t<=8. {return dynamic}
    let desired=control.body_heading(&finite);let current=control.body_heading(&dynamic);
    let route_mix=gait_ramp((t-8.)/2.);
    let yaw=Quat::IDENTITY.slerp(desired*current.inverse(),route_mix);
    let root=dynamic[0].translation.lerp(finite[0].translation.with_z(dynamic[0].translation.z),route_mix);
    let mut locomotion:Vec<_>=dynamic.iter().map(|f|Transform::from_translation(
        yaw*(f.translation-dynamic[0].translation)+root)
        .with_rotation((yaw*f.rotation).normalize())).collect();
    let exit=if duration==ORIGINAL_DURATION {19.383333}else{18.+duration-0.35};
    let beta=if jog_stop::enabled(){let w=jog_stop::blend();if w==0.35 {gait_ramp((t-17.65)/0.35)}else{gait_ramp((t-(18.-w))/w)}}else{gait_ramp((t-18.)/0.35)*(1.-gait_ramp((t-exit)/0.35))};
    if beta==0.{fit_floor(&mut locomotion,route_mix);return locomotion}
    let mut out=locomotion.clone();
    out[0].translation=locomotion[0].translation.lerp(finite[0].translation,beta);
    out[0].rotation=locomotion[0].rotation.slerp(finite[0].rotation,beta).normalize();
    for (k,&(parent,ap,ac)) in walk.anchors.iter().enumerate(){if let Some(p)=parent{
        let a=locomotion[p].rotation.inverse()*locomotion[k].rotation;
        let b=finite[p].rotation.inverse()*finite[k].rotation;
        out[k].rotation=(out[p].rotation*a.slerp(b,beta)).normalize();
        out[k].translation=out[p].translation+out[p].rotation*ap-out[k].rotation*ac;
    }}fit_floor(&mut out,1.);out
}
pub(super) fn offset(index:usize,resets:usize)->f32{
    if let Some(go)=jog_stop::go().filter(|_|jog_stop::enabled()&&jog_stop::practice()) {[14.,17.,18.,19.,20.5,22.,go-1.,0.][(index+resets)%8]}
    else if jog_stop::enabled()&&jog_stop::practice(){[14.,17.,18.,18.,19.,19.,22.,0.][(index+resets)%8]}
    else if jog_stop::enabled(){[0.,14.,0.,18.,0.,19.,0.,22.][(index+resets)%8]}else{[0.,14.,0.,16.,0.,18.,0.,20.][(index+resets)%8]}
}
