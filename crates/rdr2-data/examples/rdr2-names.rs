fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g = rdr2_data::Rdr2::open(std::path::Path::new(&a[1])).expect("open");
    let (top, nested) = a[2].split_once('|').unwrap();
    let r = g.archive(top).unwrap();
    let r = r.sub(nested).unwrap();
    println!("{} entries", r.entries.len());
    for n in &a[3..] {
        if r.entries.contains_key(&rdr2_data::joaat(n)) { println!("have {n}") }
    }
}
