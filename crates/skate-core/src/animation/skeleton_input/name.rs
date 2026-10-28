use crate::animation::output::attributes::AttributeName;

pub const fn encode(text: &[u8]) -> AttributeName {
    let mut words = [0u32; 5];
    let mut position = 0;
    while position < 30 && position < text.len() && text[position] != 0 {
        let mut weight = 79_235_168u32;
        let word = position / 6;
        let mut count = 0;
        while count < 6 && position < text.len() && text[position] != 0 {
            let byte = text[position] as i8 as i32;
            let digit = if byte >= 97 {
                byte - 86
            } else if byte > 90 {
                byte - 58
            } else if byte > 57 {
                byte - 54
            } else {
                byte - 47
            };
            words[word] = words[word].wrapping_add((digit as u32).wrapping_mul(weight));
            weight /= 38;
            count += 1;
            position += 1;
        }
    }
    AttributeName(words)
}
