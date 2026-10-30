use delirium_physics::*;
use std::ffi::CString;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let c = |s: &str| CString::new(s).unwrap();
    unsafe {
        if let Ok(e) = std::env::var("GTAIV_EXE") {
            assert_eq!(nm_unlock(c(&e).as_ptr()), 0);
        }
        assert!(nm_init(c(&a[1]).as_ptr()) >= 0);
        if let Some(tp) = a.get(3) {
            nm_load_task_params(c(tp).as_ptr());
        }
        let build = nm_build_load(c(&a[2]).as_ptr());
        assert!(!build.is_null());
        let w = nm_world_create();
        nm_world_add_floor(w, 0.0, 0.0, 0.0, 50.0);
        let mut ms = vec![];
        for (name, shot) in [("standing (bodyBalance)", false), ("shot: stagger and fall", true)] {
            let mut t = vec![];
            for run in 0..20 {
                let g = nm_agent_create(build, [0.0f32; 3].as_ptr(), 0.0, 0.3);
                nm_msg_begin(g, c("bodyBalance").as_ptr());
                nm_msg_set_bool(g, c("start").as_ptr(), 1);
                nm_msg_send(g, w);
                for k in 0..180 {
                    if shot && k == 1 {
                        let mut chest = [0.0f32; 3];
                        nm_agent_get_part_transform(g, 10, chest.as_mut_ptr(), std::ptr::null_mut());
                        nm_agent_apply_push(g, chest.as_ptr(), [0.0f32, 40.0, 0.0].as_ptr());
                        nm_agent_task_shot(g, w, c("POWERFUL_WEAPON").as_ptr(), 9, chest.as_ptr(), [0.0f32, 1.0, 0.0].as_ptr(), 200.0);
                    }
                    if !shot {
                        nm_agent_apply_impulse(g, 0, [0.0f32; 3].as_ptr(), [0.0f32; 3].as_ptr());
                    }
                    let s = Instant::now();
                    nm_agent_step(g, w, 1.0 / 60.0);
                    if run > 0 {
                        t.push(s.elapsed().as_secs_f64() * 1e3);
                    }
                }
                nm_agent_destroy(g);
            }
            t.sort_by(f64::total_cmp);
            let mean = t.iter().sum::<f64>() / t.len() as f64;
            println!("{name:24} mean {mean:.3} ms  median {:.3}  p99 {:.3}  max {:.3}  ({} steps)", t[t.len() / 2], t[t.len() * 99 / 100], t[t.len() - 1], t.len());
            ms.push(mean);
        }
        println!("{} bit; target 0.21 ms per step", usize::BITS);
    }
}
