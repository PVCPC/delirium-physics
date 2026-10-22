use super::*;
use crate::nmrs::{Character, ANKLE, HIP, KNEE};

#[derive(Clone, Copy, Debug)]
pub struct LocomotionDrive {
    pub targets: [[f32; 3]; 6],
    pub stance: [bool; 2],
    pub weight: f32,
    pub gain_weight: f32,
    pub leg_stiffness: [f32; 2],
    pub response: [f32; 2],
}

pub(super) struct NativeTerms {
    pub torque: Vec<Vec3>,
    pub push: Vec<Vec3>,
    pub yaw: [f32;2],
    pub attached: Option<(usize,Vec3,f32)>,
}

impl LocomotionDrive {
    fn valid(&self)->bool {
        self.weight.is_finite()&&(0.0..=1.0).contains(&self.weight)
            &&self.gain_weight.is_finite()&&(0.0..=self.weight).contains(&self.gain_weight)
            &&self.targets.iter().flatten().all(|x|x.is_finite())
            &&self.leg_stiffness.iter().all(|x|x.is_finite()&&(6.0..=24.0).contains(x))
            &&self.response.iter().all(|x|x.is_finite()&&(0.5..=2.0).contains(x))
    }
}

impl Agent {
    pub fn set_locomotion_drive(&mut self, drive:Option<LocomotionDrive>)->Result<(), &'static str> {
        self.locomotion_drive=None;
        if let Some(d)=drive {
            if !d.valid() {return Err("invalid locomotion drive");}
            self.locomotion_tracking=true;
            if d.weight>0. {
                if !self.ch.as_ref().is_some_and(|c|c.eff.len()>=6&&c.joints.len()>=6) {return Err("locomotion requires native leg effectors");}
                self.locomotion_drive=Some(d);
            }
        }
        Ok(())
    }

    pub(super) fn take_locomotion_drive(&mut self)->Option<LocomotionDrive> {
        let d=self.locomotion_drive.take()?;
        let recovery=self.fallen||self.balance.is_none()||self.nm_shot.is_some()||self.nm_hit.is_some()
            ||self.catch_fall.is_some()||self.nm_cf.as_ref().is_some_and(|f|f.active)
            ||self.nm_ru.active||self.nm_rds.active||self.nm_pd.active||self.nm_bf.active
            ||self.nm_hf.active()||self.nm_br.active||self.nm_bw.as_ref().is_some_and(|w|w.active)
            ||self.roll_up.is_some()||!self.extras.on.is_empty()||self.extras.hard.is_some();
        (!recovery).then_some(d)
    }

    pub(super) fn apply_locomotion_drive(&mut self,held:&mut crate::ragdoll::Kept,build:&Build,dt:f32) {
        let Some(d)=self.take_locomotion_drive() else {return;};
        let Some(native)=self.locomotion_terms.take() else {return;};
        for (k,p) in native.push.iter().enumerate().filter(|(_,p)|**p!=Vec3::ZERO) {
            self.doll.impulse(k,self.doll.centre(k),-*p);
        }
        let ch=self.ch.as_mut().expect("validated native character");
        let stance=actual_stance(ch,&self.doll,d.stance);
        apply_targets(ch,d,stance);
        let mut phase=support_character(ch,stance);phase.attached=native.attached;
        let frames:Vec<_>=self.doll.frames().collect();
        let body=crate::nmdbm::Body{ch:&phase,build,doll:&self.doll,frames:&frames};
        let mut yaw=native.yaw;
        let mut torque=crate::nmdbm::foot_yaw(&body,&mut yaw,dt);
        for i in 0..2 {if !stance[i] {torque[phase.joints[ANKLE[i]].child]=Vec3::ZERO;}}
        for (t,g) in torque.iter_mut().zip(crate::nmdbm::oppose_gravity(&body,phase.attached)) {*t+=g;}
        let desired_push=ch.stabilise.map_or_else(||vec![Vec3::ZERO;build.parts.len()],|(k,_)|stabilise_on(&body,k,dt,stance));
        if held.torque.len()<torque.len() {held.torque.resize(torque.len(),Vec3::ZERO);}
        for (i,t) in torque.into_iter().enumerate() {
            let old=native.torque.get(i).copied().unwrap_or(Vec3::ZERO);
            held.torque[i]+=(t-old)*d.weight;
        }
        for (k,want) in desired_push.into_iter().enumerate() {
            let old=native.push.get(k).copied().unwrap_or(Vec3::ZERO);
            let p=old+(want-old)*d.weight;
            if p!=Vec3::ZERO {self.doll.impulse(k,self.doll.centre(k),p);}
        }
        ch.foot_yaw=yaw;
        if d.weight==1. {ch.support=phase.support;}
    }
}

pub(super) fn actual_stance(ch:&Character,doll:&Ragdoll,wanted:[bool;2])->[bool;2] {
    std::array::from_fn(|i|wanted[i]&&doll.touch[ch.joints[ANKLE[i]].child]&0x20!=0)
}

pub(super) fn apply_targets(ch:&mut Character,d:LocomotionDrive,stance:[bool;2]) {
    for i in 0..6 {
        let limits=ch.limits(i);
        ch.eff[i].desired=std::array::from_fn(|j|d.targets[i][j].clamp(limits[2*j],limits[2*j+1]));
        let leg=i/3;let e=&mut ch.eff[i];let k=d.leg_stiffness[leg];
        e.strength+=(k*k-e.strength)*d.gain_weight;
        e.damping+=(2.*k-e.damping)*d.gain_weight;
        let response=if ANKLE.contains(&i) {if stance[leg] {10.} else {1.}}
            else if KNEE.contains(&i) {if stance[leg] {1.5} else {2.}} else {1.5};
        e.muscle_stiffness+=(response*d.response[leg]-e.muscle_stiffness)*d.gain_weight;
    }
}

pub(super) fn support_character(ch:&Character,stance:[bool;2])->Character {
    let mut phase=ch.clone();
    phase.support=std::array::from_fn(|i|if stance[i] {ch.joints[ANKLE[i]].child as i32} else {-1});
    phase
}

pub(super) fn stabilise_on(body:&crate::nmdbm::Body,k:f32,dt:f32,stance:[bool;2])->Vec<Vec3> {
    let feet:Vec<_>=(0..2).filter(|&i|stance[i]).map(|i|body.ch.joints[ANKLE[i]].child)
        .filter(|&f|body.doll.touch[f]&0x28==0x20).collect();
    if feet.len()==2 {return crate::nmdbm::stabilise(body,k,dt);}
    let mut out=vec![Vec3::ZERO;body.build.parts.len()];
    let Some(&foot)=feet.first() else {return out;};
    let up=body.ch.up;let (com,vel)=body.doll.com();let mid=body.doll.centre(foot);
    let fv=body.doll.motion(foot).0;
    let s=((com-mid).normalize_or_zero().dot(up).max(0.).powi(4)*k).powi(2);
    let f=(com+vel*0.1-mid)*s+(vel-fv)*(0.2*s);
    let f=(f-up*f.dot(up))*(dt*60.);
    out[foot]+=f;out[body.ch.joints[HIP[0]].parent]-=f;out
}

