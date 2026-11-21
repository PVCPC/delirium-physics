fn main() {
    let a: Vec<String> = std::env::args().collect();
    let d = std::fs::read(&a[1]).expect("image");
    let p = rdr2_data::params::scan(&d);
    println!("{} messages, {} parameters", p.len(), p.values().map(|m| m.len()).sum::<usize>());
    println!("configurebalance legsApartRestep {:?}", p.get("configurebalance").and_then(|m| m.get(&rdr2_data::joaat("legsApartRestep"))));
    if let Some(o) = a.get(2) {
        std::fs::write(o, rdr2_data::params::to_text(&p)).unwrap();
    }
}
