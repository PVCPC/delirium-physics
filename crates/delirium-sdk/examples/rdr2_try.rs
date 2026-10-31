//! Tries the RDR2 extra messages on a standing body and prints what they do over time.
//! Usage: GTAIV_EXE=<GTAIV.exe> cargo run --release -p delirium-sdk --example rdr2_try -- <Fred.art|fred.xrt> <build.txt> <message> [name=value...]
//! Values: true/false, x,y,z (a vector), a whole number (an int) or a decimal with a point (a float). Message "none" sends nothing.
use delirium_physics::*;
use std::ffi::CString;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    unsafe {
        if let Ok(e) = std::env::var("GTAIV_EXE") {
            assert_eq!(nm_unlock(c(&e).as_ptr()), 0);
        }
        if a[1].to_ascii_lowercase().ends_with(".art") {
            delirium_data::LE.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        assert!(nm_init(c(&a[1]).as_ptr()) >= 0);
        let build = nm_build_load(c(&a[2]).as_ptr());
        let w = nm_world_create();
        nm_world_add_floor(w, 0.0, 0.0, 0.0, 50.0);
        let g = nm_agent_create(build, [0.0f32; 3].as_ptr(), 0.0, 0.3);
        nm_msg_begin(g, c("bodyBalance").as_ptr());
        nm_msg_set_bool(g, c("start").as_ptr(), 1);
        nm_msg_send(g, w);
        let get = |p: i32| { let mut v = [0.0f32; 3]; nm_agent_get_part_transform(g, p, v.as_mut_ptr(), std::ptr::null_mut()); v };
        for k in 0..std::env::var("STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(240) {
            if k == 60 && a[3] != "none" {
                nm_msg_begin(g, c(&a[3]).as_ptr());
                nm_msg_set_bool(g, c("start").as_ptr(), 1);
                for kv in &a[4..] {
                    let (n, v) = kv.split_once('=').unwrap();
                    match v {
                        "true" | "false" => { nm_msg_set_bool(g, c(n).as_ptr(), (v == "true") as i32); }
                        _ if v.contains(',') => {
                            let f: Vec<f32> = v.split(',').map(|x| x.parse().unwrap()).collect();
                            nm_msg_set_vec3(g, c(n).as_ptr(), f.as_ptr());
                        }
                        _ if !v.contains('.') => { nm_msg_set_int(g, c(n).as_ptr(), v.parse().unwrap()); }
                        _ => { nm_msg_set_float(g, c(n).as_ptr(), v.parse().unwrap()); }
                    }
                }
                let r = nm_msg_send(g, w);
                println!("sent {} -> {r}", a[3]);
            }
            nm_agent_step(g, w, 1.0 / 60.0);
            if k % 6 == 5 {
                let (pel, chest, head) = (get(0), get(10), get(12));
                // facing: across the shoulders (left clavicle minus right), turned a quarter about up
                let (l, r) = (get(13), get(17));
                let yaw = (-(l[0] - r[0])).atan2(l[1] - r[1]).to_degrees();
                println!("t {:.2}  pelvis z {:.3}  chest y {:+.3} z {:.3}  head x {:+.3} y {:+.3} z {:.3}  facing {:+6.1}  balancing {}",
                         (k + 1) as f32 / 60.0, pel[2], chest[1] - pel[1], chest[2], head[0] - pel[0], head[1] - pel[1], head[2],
                         yaw, nm_agent_is_balancing(g));
            }
        }
    }
}
