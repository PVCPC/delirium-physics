use std::sync::atomic::{AtomicU8, Ordering};

static KEY_STEP: AtomicU8 = AtomicU8::new(0);

pub fn read(dir: &str, name: &str) -> Result<Vec<u8>, String> {
    if dir.is_empty() {
        return Err("no RDR2 folder set".into());
    }
    let d = std::path::Path::new(dir);
    if !rdr2_data::has_keys(d) {
        if KEY_STEP.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            let d = d.to_path_buf();
            std::thread::spawn(move || {
                if let Err(e) = rdr2_data::capture_keys(&d) {
                    eprintln!("room  RDR2 keys: {e}");
                }
                KEY_STEP.store(2, Ordering::SeqCst);
            });
        }
        return Err("getting RDR2's keys, once for this PC: RDR2 opens for about 40 s and closes by itself, then this loads".into());
    }
    rdr2_data::read_named(d, name)
}

pub fn read_text(dir: &str, name: &str) -> Result<String, String> {
    read(dir, name).map(|b| String::from_utf8_lossy(&b).into_owned())
}

pub fn just_ready() -> bool {
    KEY_STEP.compare_exchange(2, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok()
}
