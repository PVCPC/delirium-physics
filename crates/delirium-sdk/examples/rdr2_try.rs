//! Tries the RDR2 extra messages on a standing body and prints what they do over time.
//! Usage: GTAIV_EXE=<GTAIV.exe> cargo run --release -p delirium-sdk --example rdr2_try -- <Fred.art|fred.xrt> <build.txt> <message> [name=value...]
//! Values: true/false, x,y,z (a vector), a whole number (an int) or a decimal with a point (a float). Message "none" sends nothing.
//! Message "rdr2shot" (part=N, back=true for a shot from behind) shoots him through the SDK as a game would:
//! nm_agent_task_shot, the bullet's push, then nm_agent_rdr2_shot (RDR2's reactions to it).
use delirium_physics::rdr2::nm_agent_rdr2_shot;
use delirium_physics::*;
use std::ffi::CString;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    unsafe {
        if a[1].to_ascii_lowercase().ends_with(".art") {
            delirium_data::LE.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if let Ok(e) = std::env::var("GTAIV_EXE") {
            assert_eq!(nm_unlock(c(&e).as_ptr()), 0);
        }
        assert!(nm_init(c(&a[1]).as_ptr()) >= 0);
        let build = nm_build_load(c(&a[2]).as_ptr());
        let w = nm_world_create();
        if let Some(r) = std::env::var("PEDESTAL").ok().and_then(|v| v.parse::<f32>().ok()) {
            // PEDESTAL=r: flat ground within r of the origin (a square), 0.6 m lower beyond it (a ledge all round)
            let n = 41usize; let step = 0.1f32; let half = (n - 1) as f32 * step * 0.5;
            let mut pos = vec![];
            for iy in 0..n { for ix in 0..n {
                let (x, y) = (ix as f32 * step - half, iy as f32 * step - half);
                pos.extend([x, y, if x.abs() <= r && y.abs() <= r { 0.0 } else { -0.6 }]);
            } }
            let mut tri: Vec<u32> = vec![];
            for iy in 0..n - 1 { for ix in 0..n - 1 {
                let a0 = (iy * n + ix) as u32; let (a1, b0) = (a0 + 1, a0 + n as u32); let b1 = b0 + 1;
                tri.extend([a0, a1, b1, a0, b1, b0]);
            } }
            let mat = vec![0u8; tri.len() / 3];
            assert!(nm_world_add_mesh(w, pos.as_ptr(), (n * n) as i32, tri.as_ptr(), (tri.len() / 3) as i32, mat.as_ptr()) >= 0);
            nm_world_add_floor(w, 0.0, 0.0, -0.6, 50.0);
        } else {
            nm_world_add_floor(w, 0.0, 0.0, 0.0, 50.0);
        }
        let g = nm_agent_create(build, [0.0f32; 3].as_ptr(), 0.0, 0.3);
        nm_msg_begin(g, c("bodyBalance").as_ptr());
        nm_msg_set_bool(g, c("start").as_ptr(), 1);
        nm_msg_send(g, w);
        let get = |p: i32| { let mut v = [0.0f32; 3]; nm_agent_get_part_transform(g, p, v.as_mut_ptr(), std::ptr::null_mut()); v };
        for k in 0..std::env::var("STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(240) {
            if k == 30 {
                // PRE=<message>: another message (start=true, its defaults) sent first
                if let Ok(pre) = std::env::var("PRE") {
                    let mut it = pre.split_whitespace();
                    let m = it.next().unwrap_or("").to_string();
                    nm_msg_begin(g, c(&m).as_ptr());
                    nm_msg_set_bool(g, c("start").as_ptr(), 1);
                    for kv in it {
                        let (n, v) = kv.split_once('=').unwrap();
                        nm_msg_set_float(g, c(n).as_ptr(), v.parse().unwrap());
                    }
                    println!("sent {m} -> {}", nm_msg_send(g, w));
                }
            }
            if k == 60 && a[3] == "rdr2shot" {
                let arg = |n: &str| a[4..].iter().find_map(|kv| kv.strip_prefix(&format!("{n}=")).map(str::to_string));
                let part: i32 = arg("part").and_then(|v| v.parse().ok()).unwrap_or(10);
                let (l, r) = (get(13), get(17));
                let f = { let d = [l[1] - r[1], -(l[0] - r[0])]; let n = (d[0] * d[0] + d[1] * d[1]).sqrt(); [d[0] / n, d[1] / n, 0.0f32] };
                // the bullet travels against his facing (from in front), or along it from behind
                let s = if arg("back").as_deref() == Some("true") { 1.0 } else { -1.0 };
                let dir = [f[0] * s, f[1] * s, 0.0f32];
                let at = get(part);
                nm_agent_task_shot(g, w, c("*").as_ptr(), part, at.as_ptr(), dir.as_ptr(), 200.0);
                nm_agent_apply_push(g, at.as_ptr(), [dir[0] * 40.0, dir[1] * 40.0, 0.0f32].as_ptr());
                println!("shot part {part}: nm_agent_rdr2_shot started {}", nm_agent_rdr2_shot(g, w, part, at.as_ptr(), dir.as_ptr()));
            } else if k == 60 && a[3] != "none" {
                nm_msg_begin(g, c(&a[3]).as_ptr());
                nm_msg_set_bool(g, c("start").as_ptr(), 1);
                // "@f,l,u": a point forward / left / up of the pelvis (his facing); "@n:f,l,u": a direction that way
                let (pel, l, r) = (get(0), get(13), get(17));
                let lv = { let d = [l[0] - r[0], l[1] - r[1]]; let n = (d[0] * d[0] + d[1] * d[1]).sqrt(); [d[0] / n, d[1] / n] };
                let fv = [lv[1], -lv[0]];
                let rel = |s: &str, point: bool| -> [f32; 3] {
                    let x: Vec<f32> = s.split(',').map(|x| x.parse().unwrap()).collect();
                    let o = if point { pel } else { [0.0; 3] };
                    [o[0] + fv[0] * x[0] + lv[0] * x[1], o[1] + fv[1] * x[0] + lv[1] * x[1], o[2] + x[2]]
                };
                for kv in &a[4..] {
                    let (n, v) = kv.split_once('=').unwrap();
                    if let Some(rest) = v.strip_prefix('@') {
                        let f = if let Some(d) = rest.strip_prefix("n:") { rel(d, false) } else { rel(rest, true) };
                        nm_msg_set_vec3(g, c(n).as_ptr(), f.as_ptr());
                        continue;
                    }
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
                println!("sent {} -> {r}  (pelvis {:.3},{:.3}  facing {:.3},{:.3})", a[3], pel[0], pel[1], fv[0], fv[1]);
            }
            nm_agent_step(g, w, 1.0 / 60.0);
            if let Ok(path) = std::env::var("DUMP") {
                // every part's position each 0.1 s, one line per sample: t then x y z for parts 0..20
                if k % 6 == 5 {
                    use std::io::Write;
                    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).unwrap();
                    let mut line = format!("{:.2}", (k + 1) as f32 / 60.0);
                    for p in 0..21 { let v = get(p); line += &format!(" {:.3} {:.3} {:.3}", v[0], v[1], v[2]); }
                    writeln!(f, "{line}").unwrap();
                }
            }
            if k % 6 == 5 {
                let (pel, chest, head) = (get(0), get(10), get(12));
                // facing: across the shoulders (left clavicle minus right), turned a quarter about up
                let (l, r) = (get(13), get(17));
                let yaw = (-(l[0] - r[0])).atan2(l[1] - r[1]).to_degrees();
                println!("t {:.2}  pelvis z {:.3}  chest y {:+.3} z {:.3}  head x {:+.3} y {:+.3} z {:.3}  facing {:+6.1}  balancing {}  pelvis at {:+.2} {:+.2}",
                         (k + 1) as f32 / 60.0, pel[2], chest[1] - pel[1], chest[2], head[0] - pel[0], head[1] - pel[1], head[2],
                         yaw, nm_agent_is_balancing(g), pel[0], pel[1]);
                if std::env::var("LEAN").is_ok() {
                    // forward (across the shoulders x up): how far chest and head are ahead of the pelvis, and knee heights
                    let f = [l[1] - r[1], -(l[0] - r[0])];
                    let n = (f[0] * f[0] + f[1] * f[1]).sqrt().max(1e-6);
                    let ahead = |p: [f32; 3]| ((p[0] - pel[0]) * f[0] + (p[1] - pel[1]) * f[1]) / n;
                    let (lk, rk) = (get(2), get(5));
                    println!("      ahead  chest {:+.3} head {:+.3}   knees L z {:.2} R z {:.2}   feet L z {:.2} R z {:.2}", ahead(chest), ahead(head), lk[2], rk[2], get(3)[2], get(6)[2]);
                }
                if let Ok(wv) = std::env::var("WALLAT") {
                    // distance of pelvis, chest and head from a wall plane: WALLAT=px,py,nx,ny (world)
                    let w: Vec<f32> = wv.split(',').map(|x| x.parse().unwrap()).collect();
                    let d = |p: [f32; 3]| (p[0] - w[0]) * w[2] + (p[1] - w[1]) * w[3];
                    let (lh, rh) = (get(16), get(20));
                    println!("      wall   pelvis {:.2} chest {:.2} head {:.2}   heights {:.2} {:.2} {:.2}   hands L {:.2} at {:.2} R {:.2} at {:.2}", d(pel), d(chest), d(head), pel[2], chest[2], head[2], d(lh), lh[2], d(rh), rh[2]);
                }
                if std::env::var("HANDS").is_ok() {
                    let (lh, rh) = (get(16), get(20));
                    println!("      hands  L x {:+.2} y {:+.2} z {:.2}   R x {:+.2} y {:+.2} z {:.2}", lh[0] - pel[0], lh[1] - pel[1], lh[2], rh[0] - pel[0], rh[1] - pel[1], rh[2]);
                }
            }
        }
    }
}
