use super::*;
fn setup(mixed:bool)->Learner {
    let actor=Mlp{s:[349,4,4,54],tanh_out:true};let critic=Mlp{s:[349,4,4,1],tanh_out:false};
    let n=actor.len()+54+critic.len();let learner=Learner::new(actor,critic,vec![0.02;n],1);
    unsafe {
        let mut x=vec![0.1;349];x[311]=if mixed {0.5}else{0.};let act=vec![0.1;54];
        let mu=learner.actor.forward(&learner.p.get()[..learner.actor.len()],&x,1).pop().unwrap();
        let old=row_log_prob(&learner.actor,&mu,&act,&learner.p.get()[learner.actor.len()..learner.actor.len()+54],&x);
        *learner.batch.get()=Batch{x,act,old:vec![old],adv:vec![1.],ret:vec![2.]};
    }
    learner
}
#[test] fn walking_ignores_old50_and_jogging_ignores_only_teacher28() {
    let l=setup(false);let actor=&l.actor;let mu=vec![0.;54];let std=vec![-1.;54];let act=vec![0.1;54];
    for mixed in [false,true] {
        let mut obs=vec![0.;349];obs[311]=if mixed {1.}else{0.};
        let baseline=row_log_prob(actor,&mu,&act,&std,&obs);
        let mut noise=act.clone();noise[..if mixed {28}else{50}].fill(30.);
        assert_eq!(baseline,row_log_prob(actor,&mu,&noise,&std,&obs));
        noise[50]=0.4;assert_ne!(baseline,row_log_prob(actor,&mu,&noise,&std,&obs));
        if mixed {noise=act.clone();noise[28]=0.4;assert_ne!(baseline,row_log_prob(actor,&mu,&noise,&std,&obs));}
    }
    unsafe {
        let b=l.batch.get();let n=l.p.get().len();let mut g=vec![0.;n];let mut h=vec![0.;n];
        chunk_grad(actor,&l.critic,l.p.get(),b,0,1,1.,&mut g);
        let mut changed=Batch{x:b.x.clone(),act:b.act.clone(),old:b.old.clone(),adv:b.adv.clone(),ret:b.ret.clone()};changed.act[..50].fill(80.);
        chunk_grad(actor,&l.critic,l.p.get(),&changed,0,1,1.,&mut h);assert_eq!(g,h);
    }
}
#[test] fn masks_skip_unused_leg_momentum_and_preserve_teacher_heads() {
    for mixed in [false,true] {
        let l=setup(mixed);unsafe {
            l.m.get().fill(0.1);l.v.get().fill(0.02);*l.t.get()=100;
            let p=l.p.get().clone();let m=l.m.get().clone();let v=l.v.get().clone();l.step();
            for i in 0..p.len() {if !l.trainable(i) {
                assert_eq!(p[i].to_bits(),l.p.get()[i].to_bits());assert_eq!(m[i].to_bits(),l.m.get()[i].to_bits());assert_eq!(v[i].to_bits(),l.v.get()[i].to_bits());
            }}
            let head=l.actor.off(2);let bias=head+54*4;
            assert_eq!(p[head..head+28*4],l.p.get()[head..head+28*4]);
            assert_eq!(p[bias..bias+28],l.p.get()[bias..bias+28]);
            if mixed {assert_ne!(p[head+28*4..head+50*4],l.p.get()[head+28*4..head+50*4]);}
            else {assert_eq!(p[head+28*4..head+50*4],l.p.get()[head+28*4..head+50*4]);}
            assert_ne!(p[head+50*4..bias],l.p.get()[head+50*4..bias]);
            assert_ne!(p[..head],l.p.get()[..head]);
            assert_ne!(p[l.actor.len()+54..],l.p.get()[l.actor.len()+54..]);
            assert_eq!(entropy_share(&l.actor,l.batch.get(),28,l.live.get()),if mixed {1.}else{0.});
            assert_eq!(entropy_share(&l.actor,l.batch.get(),50,l.live.get()),1.);
        }
    }
}
#[test] fn mixed_batch_actor_gradient_matches_finite_difference() {
    let l=setup(true);unsafe {
        let b=l.batch.get();b.x.extend(vec![0.1;349]);b.x[349+311]=0.;
        b.act.extend(vec![0.1;54]);b.adv.push(-0.3);b.ret.push(2.);
        let mu=l.actor.forward(&l.p.get()[..l.actor.len()],&b.x,2).pop().unwrap();
        b.old.push(row_log_prob(&l.actor,&mu[54..],&b.act[54..],&l.p.get()[l.actor.len()..l.actor.len()+54],&b.x[349..]));
        let mut gradient=vec![0.;l.p.get().len()];chunk_grad(&l.actor,&l.critic,l.p.get(),b,0,2,2.,&mut gradient);
        let objective=|p:&[f32]| {
            let mu=l.actor.forward(&p[..l.actor.len()],&b.x,2).pop().unwrap();let mut loss=0.;
            for r in 0..2 {
                let start=if r==0 {28}else{50};
                let lp=log_prob(&mu[r*54+start..(r+1)*54],&b.act[r*54+start..(r+1)*54],&p[l.actor.len()+start..l.actor.len()+54]);
                let ratio=(lp-b.old[r]).exp();loss-=((ratio*b.adv[r]).min(ratio.clamp(0.8,1.2)*b.adv[r]))*0.5;
            }
            loss
        };
        let head=l.actor.off(2);
        for i in [head+28*4,head+50*4,311] {
            let mut p=l.p.get().clone();let eps=0.001;p[i]+=eps;let plus=objective(&p);p[i]-=2.*eps;let minus=objective(&p);
            let numeric=(plus-minus)/(2.*eps);assert!((numeric-gradient[i]).abs()<0.0015,"index{i}: numerical{numeric}, gradient{}",gradient[i]);
        }
    }
}
