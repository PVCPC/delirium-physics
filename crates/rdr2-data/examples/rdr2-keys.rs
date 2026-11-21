fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(a.get(1).expect("usage: rdr2-keys <RDR2 folder> [archive|path out]"));
    if !rdr2_data::has_keys(dir) {
        println!("RDR2 will open for about 30 seconds to read its keys");
        rdr2_data::capture_keys(dir).unwrap();
    }
    println!("keys: {}", rdr2_data::key_cache().display());
    if let [_, _, spec, out, ..] = &a[..] {
        let d = rdr2_data::Rdr2::open(dir).unwrap().read(spec).unwrap();
        std::fs::write(out, &d).unwrap();
        println!("{spec}: {} bytes", d.len());
    }
}
