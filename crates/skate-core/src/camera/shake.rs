use super::manager_state::clamp;
use super::vector_tracker::{dot,length,refined_reciprocal};

#[derive(Clone,Debug,PartialEq)]
pub struct ShakeSamples {
    pub(super) rotations:Vec<[f32;4]>,
    pub(super) translations:Vec<[f32;4]>,
}
impl ShakeSamples {
    pub fn from_rows(rows:&[[f32;8]])->Result<Self,String> {
        if rows.is_empty() {return Err("Stock camera shake has no samples".into());}
        if rows.iter().flatten().any(|v|!v.is_finite()) {
            return Err("Stock camera shake contains a non-finite sample".into());
        }
        let mut rotations=Vec::with_capacity(rows.len());
        let mut translations=Vec::with_capacity(rows.len());
        let mut rotation_sum=[0.0_f32;4];let mut translation_sum=[0.0_f32;4];
        for row in rows {
            let rotation=[row[0],row[1],row[2],0.0];
            let translation=[row[4],row[5],row[6],0.0];
            rotation_sum=core::array::from_fn(|i|rotation_sum[i]+rotation[i]);
            translation_sum=core::array::from_fn(|i|translation_sum[i]+translation[i]);
            rotations.push(rotation);translations.push(translation);
        }
        let inverse=1.0/(rows.len() as f32);
        let rotation_mean=rotation_sum.map(|v|v*inverse);
        let translation_mean=translation_sum.map(|v|v*inverse);
        for (rotation,translation) in rotations.iter_mut().zip(&mut translations) {
            *rotation=core::array::from_fn(|i|rotation[i]-rotation_mean[i]);
            *translation=core::array::from_fn(|i|translation[i]-translation_mean[i]);
        }
        Ok(Self {rotations,translations})
    }
}

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct ShakeSettings {
    pub amplitude_curve:[[f32;4];4],
    pub frequency_curve:[[f32;4];4],
    pub impulse_magnitude:f32,
    pub impulse_minimum_velocity:f32,
    pub impulse_maximum_velocity:f32,
    pub impulse_frequency:f32,
    pub impulse_decay:f32,
    pub amplitude_minimum:f32,
    pub amplitude_maximum:f32,
    pub amplitude_top_speed:f32,
    pub frequency_minimum:f32,
    pub frequency_maximum:f32,
    pub frequency_top_speed:f32,
    pub data_frames_per_second:f32,
    pub translation_multiplier:f32,
    pub rotation_multiplier:f32,
    pub dutch_multiplier:f32,
}

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct ShakeEffect {
    pub translation:[f32;4],
    pub time:f32,
    pub impulse:f32,
    pub weight:f32,
}
impl ShakeEffect {
    pub fn new()->Self {Self {translation:[0.0;4],time:0.0,impulse:0.0,weight:0.0}}

    pub fn landing_impulse(&mut self,normal:[f32;4],velocity:[f32;4],settings:ShakeSettings)->bool {
        let speed=length(velocity);
        if f32::from_bits(0x3727c5ac)>speed {return false;}
        let inverse=refined_reciprocal(speed);
        let direction=velocity.map(|v|inverse*v);
        let cosine=clamp(dot(normal.map(|v|-v),direction),-1.0,1.0);
        let angle=crate::trigonometry::acos(cosine)*f32::from_bits(0x3f22f983);
        let weighted_speed=speed*(1.0-clamp(angle,0.0,1.0));
        if weighted_speed<=0.0 {return false;}
        let fraction=clamp((weighted_speed-settings.impulse_minimum_velocity)
            /(settings.impulse_maximum_velocity-settings.impulse_minimum_velocity),0.0,1.0);
        let impulse=(fraction*fraction)*settings.impulse_magnitude;
        if impulse>self.impulse {self.impulse=impulse;true} else {false}
    }
}
