//! Regression fingerprint: runs fixed scenes through the C interface and hashes every part's transform bits on every
//! step, so a change that should leave the existing behaviours alone can be checked bit for bit.
//! Usage: GTAIV_EXE=<GTAIV.exe> cargo run --release -p delirium-sdk --example regress -- <fred.xrt|Fred.art> <build.txt> [taskparams]
//! Prints one hash per scene and a combined one; compare the output before and after a change.
use delirium_physics::*;
use std::ffi::CString;

/// FNV-1a over the bytes of every part's position and rotation.
fn mix(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

type Scene = (&'static str, fn(*mut NmAgent, *mut NmWorld, usize));

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

unsafe fn start(g: *mut NmAgent, w: *mut NmWorld, msg: &str, floats: &[(&str, f32)]) {
    unsafe {
        nm_msg_begin(g, c(msg).as_ptr());
        nm_msg_set_bool(g, c("start").as_ptr(), 1);
        for (k, v) in floats {
            nm_msg_set_float(g, c(k).as_ptr(), *v);
        }
        nm_msg_send(g, w);
    }
}

unsafe fn chest(g: *mut NmAgent) -> [f32; 3] {
    let mut p = [0.0f32; 3];
    unsafe { nm_agent_get_part_transform(g, 10, p.as_mut_ptr(), std::ptr::null_mut()) };
    p
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenes: [Scene; 10] = [
        ("stand", |g, w, k| unsafe { if k == 0 { start(g, w, "bodyBalance", &[]) } }),
        ("push", |g, w, k| unsafe {
            if k == 0 { start(g, w, "bodyBalance", &[]) }
            if k == 30 { let p = chest(g); nm_agent_apply_push(g, p.as_ptr(), [0.0f32, 60.0, 0.0].as_ptr()); }
        }),
        ("shot", |g, w, k| unsafe {
            if k == 0 { start(g, w, "bodyBalance", &[]) }
            if k == 30 {
                let p = chest(g);
                nm_agent_apply_push(g, p.as_ptr(), [0.0f32, 40.0, 0.0].as_ptr());
                nm_agent_task_shot(g, w, c("POWERFUL_WEAPON").as_ptr(), 10, p.as_ptr(), [0.0f32, 1.0, 0.0].as_ptr(), 200.0);
            }
        }),
        ("shot_back", |g, w, k| unsafe {
            if k == 0 { start(g, w, "bodyBalance", &[]) }
            if k == 30 {
                let p = chest(g);
                nm_agent_task_shot(g, w, c("*").as_ptr(), 10, p.as_ptr(), [0.0f32, -1.0, 0.0].as_ptr(), 200.0);
            }
        }),
        ("catchfall", |g, w, k| unsafe {
            if k == 0 { start(g, w, "catchFall", &[]) }
            if k == 1 { let p = chest(g); nm_agent_apply_push(g, p.as_ptr(), [0.0f32, 80.0, 0.0].as_ptr()); }
        }),
        ("writhe", |g, w, k| unsafe { if k == 0 { start(g, w, "bodyWrithe", &[]) } }),
        ("rollup", |g, w, k| unsafe { if k == 0 { start(g, w, "bodyRollUp", &[]) } }),
        ("windmill_pedal", |g, w, k| unsafe {
            if k == 0 { start(g, w, "armsWindmillAdaptive", &[]); start(g, w, "pedalLegs", &[]) }
        }),
        ("stagger", |g, w, k| unsafe {
            if k == 0 { start(g, w, "staggerFall", &[]) }
            if k == 1 { let p = chest(g); nm_agent_apply_push(g, p.as_ptr(), [0.0f32, 50.0, 0.0].as_ptr()); }
        }),
        ("electrocute", |g, w, k| unsafe { if k == 0 { start(g, w, "electrocute", &[]) } }),
    ];
    unsafe {
        if let Ok(e) = std::env::var("GTAIV_EXE") {
            assert_eq!(nm_unlock(c(&e).as_ptr()), 0, "nm_unlock failed");
        }
        // the PC game's body file (Fred.art) is little-endian; the console fred.xrt is big-endian
        if a[1].to_ascii_lowercase().ends_with(".art") {
            delirium_data::LE.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        assert!(nm_init(c(&a[1]).as_ptr()) >= 0);
        if let Some(tp) = a.get(3) {
            nm_load_task_params(c(tp).as_ptr());
        }
        let build = nm_build_load(c(&a[2]).as_ptr());
        assert!(!build.is_null());
        let w = nm_world_create();
        nm_world_add_floor(w, 0.0, 0.0, 0.0, 50.0);
        let mut all = 0xcbf29ce484222325u64;
        for (name, f) in scenes {
            let g = nm_agent_create(build, [0.0f32; 3].as_ptr(), 0.0, 0.3);
            let mut h = 0xcbf29ce484222325u64;
            for k in 0..240 {
                f(g, w, k);
                nm_agent_step(g, w, 1.0 / 60.0);
                for p in 0..nm_agent_part_count(g) {
                    let (mut pos, mut rot) = ([0.0f32; 3], [0.0f32; 4]);
                    nm_agent_get_part_transform(g, p, pos.as_mut_ptr(), rot.as_mut_ptr());
                    for v in pos.iter().chain(rot.iter()) {
                        mix(&mut h, &v.to_bits().to_le_bytes());
                    }
                }
            }
            let pelvis = { let mut p = [0.0f32; 3]; nm_agent_get_part_transform(g, 0, p.as_mut_ptr(), std::ptr::null_mut()); p };
            println!("{name:16} {h:016x}  pelvis z {:.3}  fallen {}", pelvis[2], nm_agent_has_fallen(g));
            mix(&mut all, &h.to_le_bytes());
            nm_agent_destroy(g);
        }
        println!("combined         {all:016x}");
    }
}
