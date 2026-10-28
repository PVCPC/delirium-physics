use delirium::gjk::tapered;
use delirium::prelude::Vec3;
use std::io::BufRead;

fn main() {
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let w: Vec<f32> = line.split_whitespace().filter_map(|v| u32::from_str_radix(v, 16).ok()).map(f32::from_bits).collect();
        if w.len() < 6 {
            continue;
        }
        let p = tapered(w[0], w[1], w[2]).support(Vec3::new(w[3], w[4], w[5]));
        println!("{:08x} {:08x} {:08x}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits());
    }
}
