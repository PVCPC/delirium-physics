use super::*;
use std::io::Write;

#[test]
#[ignore = "private current-policy phase-specific actuator diagnosis; CPU only"]
fn current_actuator_capacity() {
    let output=std::env::var("DLM_PUSH_OFF_OUTPUT").unwrap();
    let mut csv=std::fs::OpenOptions::new().write(true).create_new(true).open(output).unwrap();
    writeln!(csv,"case,stiffness,phase,step,com_z,com_vz,left_bottom,right_bottom,contact,head_z,target_left,target_right,tilt").unwrap();
    let sim=crate::load(&["sandbox".into()],&[],Some(&std::env::var("DELIRIUM_WALK_GAME").unwrap()),Default::default(),None);
    let motion=Motion::load(&std::env::var("DELIRIUM_WALK_REFERENCE").unwrap(),&sim.build).unwrap();
    let jog=Motion::load(&std::env::var("DELIRIUM_JOG_REFERENCE").unwrap(),&sim.build).unwrap();
    configure_bridge(&sim.build,&motion,&jog).unwrap();
    let net=Net::load(&std::env::var("DELIRIUM_JOG_TEST_POLICY").unwrap()).unwrap();
    let concrete=sim.room.surfaces.iter().position(|s|s.0=="CONCRETE").unwrap_or(0) as u8;
    let mut world=Collision::default();world.add_floor(sim.frames[0].translation.x,sim.frames[0].translation.y,0.,500.,concrete);
    for (case,scale) in [("baseline",1.0),("leg-ms",1.25),("leg-ms",1.5),("leg-ms",2.0),("takeoff-ms",2.0)] {
        for phase in 0..8 {
            let mut c=Controller::walking(&motion,motion.duration*phase as f32/8.,&sim.frames);c.with_jog(&motion,&jog,false);
            let mut a=c.agent_in(&motion,&sim.build,&world);let mut action=vec![0.;c.actions()];
            for n in 0..28*60 {
                if n%2==0 {action=net.forward(&net.w,&c.observation(&a,&sim.build,&motion));}
                c.drive(&mut a,&sim.build,&world,&motion,&action);a.step(&sim.build,&world,DT,&[],&[]);
                let (_,fallen)=c.measure(&a,&sim.build,&motion);assert!(!fallen,"warmup fell phase={phase} step={n}");
            }
            let fr:Vec<_>=a.doll.frames().collect();let up=(fr[12].translation-fr[0].translation).normalize();
            for n in 0..720 {
                if n%2==0 {action=net.forward(&net.w,&c.observation(&a,&sim.build,&motion));}
                c.drive(&mut a,&sim.build,&world,&motion,&action);
                let target=c.target(&motion,c.time+DT);let next=c.target(&motion,c.time+2.*DT);
                let vz=(0..target.len()).map(|k|(next[k].translation.z-target[k].translation.z)/DT*a.doll.part_mass(k)).sum::<f32>()/a.doll.mass();
                if case!="baseline" {
                    let character=a.character().unwrap();
                    let effectors:Vec<_>=character.eff.iter().enumerate().map(|(i,e)| {
                        let gain=if i<6 && (case!="takeoff-ms"||vz>0.1) {scale} else {1.};
                        (e.desired,if i<6 {character.joints[i].muscle_stiffness*gain} else {e.muscle_stiffness},e.strength,e.damping)
                    }).collect();
                    a.set_effectors(&effectors);
                }
                a.step(&sim.build,&world,DT,&[],&[]);
                let fr:Vec<_>=a.doll.frames().collect();let bounds:Vec<_>=a.doll.capsules().collect();let (com,v)=a.doll.com();
                let contact=[3,6].iter().filter(|&&k|a.doll.touch[k]&0x20!=0).count();
                let tilt=up.angle_between((fr[12].translation-fr[0].translation).normalize());
                writeln!(csv,"{case},{scale},{phase},{n},{},{},{},{},{},{},{},{},{tilt}",com.z,v.z,bottom(&sim.build,&bounds,3),bottom(&sim.build,&bounds,6),contact,fr[12].translation.z,bottom(&sim.build,&target,3),bottom(&sim.build,&target,6)).unwrap();
                let (_,fallen)=c.measure(&a,&sim.build,&motion);if fallen {break;}
            }
        }
    }
}

#[test]
#[ignore = "private CPU-only joint-motor push-off capacity probe"]
fn stance_extension_capacity() {
    let output=std::env::var("DLM_PUSH_OFF_OUTPUT").unwrap();
    let mut csv=std::fs::OpenOptions::new().write(true).create_new(true).open(&output).unwrap();
    writeln!(csv,"case,stiffness,phase,step,com_z,com_vz,left_bottom,right_bottom,contact,head_z,target_left,target_right,tilt").unwrap();
    let sim=crate::load(&["sandbox".into()],&[],Some(&std::env::var("DELIRIUM_WALK_GAME").unwrap()),Default::default(),None);
    let mut world=Collision::default();world.add_floor(0.,0.,0.,500.,0);
    for axis in 0..3 {for sign in [-1.,1.] {for depth in [0.4,0.8,1.2] {for stiffness in [12.,16.,24.] {for balance in [true,false] {
        let mut a=learn::walk_agent(&sim.build,&world,&sim.frames).unwrap();
        for _ in 0..30 {a.doll.kick(Vec3::ZERO);a.step(&sim.build,&world,DT,&[],&[]);}
        let start:Vec<_>=a.doll.frames().collect();let native=a.limp_rage();
        let initial_torso=(start[12].translation-start[0].translation).normalize();
        if !balance {a.stop_all_behaviours();}
        let case=format!("extension-axis{axis}-sign{sign}-depth{depth}-balance{balance}");
        for n in 0..180 {
            let cycle=n%60;
            let load=if cycle<18 {cycle as f32/18.} else if cycle<24 {1.-(cycle-18) as f32/6.} else {0.};
            let mut rotations:Vec<_>=start.iter().map(|f|f.rotation).collect();
            for k in 1..7 {
                let p=sim.build.parts[k].parent.unwrap();let drive=&native[k];
                let mut angles=if drive.hinge {[drive.desired,0.,0.]} else {[drive.target[2],drive.target[0],drive.target[1]]};
                if k==2||k==5 {angles[0]=-depth*load;}
                if k==1||k==4 {angles[axis]+=sign*depth*0.5*load;}
                rotations[k]=rotations[p]*drive.frame*delirium::nmrs::rotation(angles)*drive.child.inverse();
            }
            a.set_stiffness(stiffness,1.);a.muscle_stiffness_x(1.);
            a.incoming=rotations.into_iter().map(Transform::from_rotation).collect();a.active_pose(None);
            a.doll.kick(Vec3::ZERO);a.step(&sim.build,&world,DT,&[],&[]);
            let frames:Vec<_>=a.doll.frames().collect();let bounds:Vec<_>=a.doll.capsules().collect();
            let (com,v)=a.doll.com();let contact=a.doll.touch.iter().filter(|&&t|t&0x20!=0).count();
            let tilt=initial_torso.angle_between((frames[12].translation-frames[0].translation).normalize());
            writeln!(csv,"{case},{stiffness},0,{n},{},{},{},{},{},{},0,0,{tilt}",com.z,v.z,bottom(&sim.build,&bounds,3),bottom(&sim.build,&bounds,6),contact,frames[12].translation.z).unwrap();
            assert!(com.is_finite()&&v.is_finite());
            if frames[12].translation.z<0.8 {break;}
        }
    }}}}}
}

#[test]
#[ignore = "requires private rig, references and preserved policy; CPU-only diagnostic"]
fn isolated_push_off() {
    let output = std::env::var("DLM_PUSH_OFF_OUTPUT").unwrap();
    let mut csv = std::fs::OpenOptions::new().write(true).create_new(true).open(&output).unwrap();
    writeln!(csv,"case,stiffness,phase,step,com_z,com_vz,left_bottom,right_bottom,contact,head_z,target_left,target_right").unwrap();
    let sim = crate::load(&["sandbox".into()], &[], Some(&std::env::var("DELIRIUM_WALK_GAME").unwrap()), Default::default(), None);
    let motion = Motion::load(&std::env::var("DELIRIUM_WALK_REFERENCE").unwrap(), &sim.build).unwrap();
    let jog = Motion::load(&std::env::var("DELIRIUM_JOG_REFERENCE").unwrap(), &sim.build).unwrap();
    configure_bridge(&sim.build, &motion, &jog).unwrap();
    let net = Net::load(&std::env::var("DELIRIUM_JOG_TEST_POLICY").unwrap()).unwrap();
    let warmup=std::env::var("DLM_PUSH_OFF_WARMUP").is_ok();
    let concrete = sim.room.surfaces.iter().position(|s| s.0 == "CONCRETE").unwrap_or(0) as u8;
    let mut world = Collision::default(); world.add_floor(0.0, 0.0, 0.0, 500.0, concrete);
    for (case, stiffness) in [("learned",12.0),("learned-no-balance",12.0),("learned-no-upright",12.0),("learned-no-host-force",12.0),
        ("learned-teacher-legs",12.0),("learned-teacher-legs",16.0),("learned-stiffness",16.0),
        ("learned-quarter-residual",14.0),("learned-quarter-residual",16.0),
        ("teacher-balanced",12.0),("teacher-motors",8.0),("teacher-motors",12.0),("teacher-motors",16.0),("teacher-static-start",16.0)] {
        let mut recordings = vec![];
        for phase in 0..8 {
            let mut c = Controller::walking(&motion, motion.duration * phase as f32 / 8.0, &sim.frames);
            c.with_jog(&motion, &jog, false); if !warmup {c.jog.as_mut().unwrap().offset = 28.0;}
            let mut a = c.agent_in(&motion, &sim.build, &world);
            let mut action=vec![0.0;46];
            if warmup {
                for n in 0..28*60 {
                    if n%2==0 {action=net.forward(&net.w,&c.observation(&a,&sim.build,&motion));}
                    c.drive(&mut a,&sim.build,&world,&motion,&action);a.step(&sim.build,&world,DT,&[],&[]);
                    let (_,fallen)=c.measure(&a,&sim.build,&motion);
                    assert!(!fallen,"source policy fell during warmup phase {phase} at {n}");
                }
            }
            if case == "teacher-static-start" {
                let frames: Vec<_> = a.doll.frames().collect();
                for (k,f) in frames.iter().enumerate() { a.doll.set_body(k,f.translation,f.rotation,Vec3::ZERO,Vec3::ZERO); }
            }
            if case.starts_with("teacher") && case != "teacher-balanced" { a.stop_all_behaviours(); }
            let mut recording = vec![];
            for n in 0..if warmup {720} else {180} {
                if case.starts_with("learned") {
                    if n%2==0 {action=net.forward(&net.w,&c.observation(&a,&sim.build,&motion));}
                    c.drive(&mut a,&sim.build,&world,&motion,&action);
                    if case == "learned-no-balance" { a.body_balance_stop(); }
                    if case == "learned-no-upright" { a.stay_upright(false,0.0,0.5); }
                    if case == "learned-no-host-force" { a.push.clear(); }
                    if case == "learned-stiffness" {a.set_stiffness_mask(stiffness,1.0,"lb");}
                    if case == "learned-quarter-residual" {
                        let target=c.target(&motion,c.time+DT);let before=a.incoming.clone();
                        for k in 1..7 {
                            let p=sim.build.parts[k].parent.unwrap();
                            let local=target[p].rotation.inverse()*target[k].rotation;
                            let residual=before[p].rotation.inverse()*before[k].rotation;
                            a.incoming[k].rotation=a.incoming[p].rotation*local.slerp(residual,0.25);
                        }
                        a.set_stiffness_mask(stiffness,1.0,"lb");a.active_pose(Some("lb"));
                    }
                    if case == "learned-teacher-legs" {
                        let target=c.target(&motion,c.time+DT);
                        a.set_stiffness_mask(stiffness,1.0,"lb");
                        a.incoming=target.iter().map(|f|Transform::from_rotation(f.rotation)).collect();
                        a.active_pose(Some("lb"));
                    }
                } else {
                    let fraction=c.jog.as_ref().unwrap().fraction(c.elapsed as f32*DT+c.jog.as_ref().unwrap().offset);
                    let brisk=c.jog.as_ref().unwrap().brisk.as_ref().unwrap();
                    let rate=motion.duration*((1.0-fraction)/brisk.0.duration+fraction/jog.duration);
                    c.walk.as_mut().unwrap().phase=(c.walk.as_ref().unwrap().phase+DT*rate).rem_euclid(motion.duration);
                    let target=c.target(&motion,c.time+DT);
                    a.set_stiffness(stiffness,1.0); a.muscle_stiffness_x(1.0);
                    a.incoming=target.iter().map(|f|Transform::from_rotation(f.rotation)).collect();
                    a.active_pose(None); a.doll.kick(Vec3::ZERO);
                }
                a.step(&sim.build,&world,DT,&[],&[]);
                let frames: Vec<_>=a.doll.frames().collect();
                assert!(frames.iter().all(|f|f.translation.is_finite()));
                let mut target=c.target(&motion,c.time+DT);
                let bounds: Vec<_>=a.doll.capsules().collect();
                for k in [3,6] {target[k].rotation*=frames[k].rotation.inverse()*bounds[k].rotation;}
                let contact=a.doll.touch.iter().filter(|&&t|t&0x20!=0).count();
                let (com,v)=a.doll.com();
                writeln!(csv,"{case},{stiffness},{phase},{n},{},{},{},{},{},{},{},{}",com.z,v.z,bottom(&sim.build,&bounds,3),bottom(&sim.build,&bounds,6),contact,frames[12].translation.z,bottom(&sim.build,&target,3),bottom(&sim.build,&target,6)).unwrap();
                recording.push(frames);
                let (_,fallen)=c.measure(&a,&sim.build,&motion);
                if fallen { break; }
            }
            recordings.push(recording);
        }
        let path=format!("{output}-{case}-{stiffness}");
        learn::save_live(&path,0,&vec![0.0;8],&recordings);
    }
}
