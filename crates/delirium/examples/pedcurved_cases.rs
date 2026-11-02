use delirium::curved::{from_blobs, ped_part};
use std::io::BufRead;

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn line(b: &[u32]) -> String {
    b.iter().map(|v| format!("{v:08x}")).collect::<Vec<_>>().join(" ")
}

fn main() {
    for l in std::io::stdin().lock().lines().map_while(Result::ok) {
        let w: Vec<&str> = l.split_whitespace().collect();
        if w.first() == Some(&"support") && w.len() == 7 {
            let f = |i: usize| f32::from_bits(u32::from_str_radix(w[i], 16).unwrap());
            let p = ped_part(f(1), f(2), f(3)).support(delirium::prelude::Vec3::new(f(4), f(5), f(6)));
            println!("{} {:?}", line(&[p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]), p);
            continue;
        }
        if w.len() < 3 {
            continue;
        }
        let f = |i: usize| f32::from_bits(u32::from_str_radix(w[i], 16).unwrap());
        let mut out = line(&ped_part(f(0), f(1), f(2)).bits());
        if w.len() >= 6 {
            let game = from_blobs(&hex(w[3]), &hex(w[4]), &hex(w[5])).unwrap();
            out = format!("{out} | {}", line(&game.bits()));
        }
        println!("{out}");
    }
}
