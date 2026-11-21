use rdr2_data::{Rdr2, Rpf};
use std::path::Path;

fn walk(a: &Rpf, chain: &str, needle: &[u8]) {
    for (&h, e) in &a.entries {
        if e.is_archive() {
            if let Ok(s) = a.sub_hash(h) {
                walk(&s, &format!("{chain}|{h:08x}"), needle);
            }
        } else if let Ok(d) = a.read_hash(h) {
            if let Some(at) = d.windows(needle.len()).position(|w| w.eq_ignore_ascii_case(needle)) {
                let around = String::from_utf8_lossy(&d[at.saturating_sub(120)..(at + 160).min(d.len())]).replace(['\n', '\r'], " ");
                println!("{chain}|{h:08x} ({} bytes): ...{around}...", d.len());
                if let Ok(out) = std::env::var("GREP_OUT") {
                    let _ = std::fs::write(format!("{out}/{}_{h:08x}.bin", chain.replace(['|', ' '], "_")), &d);
                }
            }
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g = Rdr2::open(Path::new(&a[1])).expect("open");
    let mut rpfs: Vec<_> = std::fs::read_dir(&a[1]).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".rpf") && !["levels", "textures", "movies", "anim_", "shaders", "hd_"].iter().any(|s| n.starts_with(s))).collect();
    rpfs.sort();
    for name in rpfs {
        match g.archive(&name) {
            Ok(r) => match a[2].strip_prefix('#') {
                Some(n) => walk(&r, &name, &rdr2_data::joaat(n).to_le_bytes()),
                None => walk(&r, &name, a[2].as_bytes()),
            },
            Err(e) => eprintln!("{name}: {e}"),
        }
    }
}
