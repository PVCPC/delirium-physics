pub fn backwards(n: [f32; 3], b: [f32; 3], ground_flag: bool, mirrored: bool) -> bool {
    let side = (-n[0]).mul_add(b[2], n[2] * b[0]);
    let positive_or_unordered = !(side <= 0.0);
    positive_or_unordered ^ ground_flag ^ mirrored
}
