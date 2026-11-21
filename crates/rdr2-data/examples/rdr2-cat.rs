fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    for n in &a[2..] {
        let got = if n.contains("|#") {
            let p: Vec<&str> = n.split('|').collect();
            let g = rdr2_data::Rdr2::open(dir).expect("open");
            let h = |s: &str| u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap();
            let r = g.archive(p[0]).expect("archive");
            let mut r = r;
            for q in &p[1..p.len() - 1] {
                r = r.sub_hash(h(q)).expect("nested");
            }
            if p[p.len() - 1] == "*" {
                for (&fh, e) in &r.entries {
                    if !e.is_archive() {
                        if let Ok(d) = r.read_hash(fh) {
                            println!("  {fh:08x} {} bytes", d.len());
                            if let Ok(out) = std::env::var("CAT_OUT") {
                                let _ = std::fs::write(format!("{out}/{fh:08x}.bin"), &d);
                            }
                        }
                    } else {
                        println!("  {fh:08x} (archive)");
                    }
                }
                continue;
            }
            r.read_hash(h(p[p.len() - 1])).map_err(|e| e.to_string())
        } else {
            rdr2_data::read_named(dir, n).map_err(|e| e.to_string())
        };
        match got {
            Ok(d) => {
                println!("have {n} ({} bytes)", d.len());
                if let Ok(out) = std::env::var("CAT_OUT") {
                    let _ = std::fs::write(format!("{out}/{}", n.replace(['|', '/'], "_")), &d);
                }
            }
            Err(_) => {}
        }
    }
}
