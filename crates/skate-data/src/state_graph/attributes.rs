use super::GraphAttribute;
use std::collections::BTreeMap;

pub fn key_hash(text: &str) -> u32 {
    if text.is_empty() {
        0
    } else {
        byte_hash(text.as_bytes())
    }
}

pub fn byte_hash(bytes: &[u8]) -> u32 {
    let mut a = 0x9e37_79b9_u32;
    let mut b = a;
    let mut c = 0xabcd_ef00_u32;
    let mut chunks = bytes.chunks_exact(12);
    for chunk in &mut chunks {
        a = a.wrapping_add(u32::from_le_bytes(chunk[0..4].try_into().unwrap()));
        b = b.wrapping_add(u32::from_le_bytes(chunk[4..8].try_into().unwrap()));
        c = c.wrapping_add(u32::from_le_bytes(chunk[8..12].try_into().unwrap()));
        mix(&mut a, &mut b, &mut c);
    }
    c = c.wrapping_add(bytes.len() as u32);
    for (i, &byte) in chunks.remainder().iter().enumerate() {
        match i {
            0..=3 => a = a.wrapping_add((byte as u32) << (8 * i)),
            4..=7 => b = b.wrapping_add((byte as u32) << (8 * (i - 4))),
            _ => c = c.wrapping_add((byte as u32) << (8 * (i - 7))),
        }
    }
    mix(&mut a, &mut b, &mut c);
    c
}

fn mix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *a = a.wrapping_sub(*b).wrapping_sub(*c) ^ (*c >> 13);
    *b = b.wrapping_sub(*c).wrapping_sub(*a) ^ (*a << 8);
    *c = c.wrapping_sub(*a).wrapping_sub(*b) ^ (*b >> 13);
    *a = a.wrapping_sub(*b).wrapping_sub(*c) ^ (*c >> 12);
    *b = b.wrapping_sub(*c).wrapping_sub(*a) ^ (*a << 16);
    *c = c.wrapping_sub(*a).wrapping_sub(*b) ^ (*b >> 5);
    *a = a.wrapping_sub(*b).wrapping_sub(*c) ^ (*c >> 3);
    *b = b.wrapping_sub(*c).wrapping_sub(*a) ^ (*a << 10);
    *c = c.wrapping_sub(*a).wrapping_sub(*b) ^ (*b >> 15);
}

pub struct Attributes<'a> {
    entries: BTreeMap<u32, &'a GraphAttribute>,
}

impl<'a> Attributes<'a> {
    pub fn new(attributes: &'a [GraphAttribute]) -> Self {
        let mut entries = BTreeMap::new();
        for attribute in attributes {
            entries
                .entry(key_hash(&attribute.name))
                .or_insert(attribute);
        }
        Self { entries }
    }
    pub fn get(&self, name: &str) -> Option<&'a GraphAttribute> {
        self.entries.get(&key_hash(name)).copied()
    }
    pub fn text(&self, name: &str) -> Option<&'a str> {
        self.get(name).map(|v| v.text.as_str())
    }
    pub fn boolean_byte(&self, name: &str, default: u8) -> u8 {
        self.get(name).map_or(default, |v| v.boolean_byte)
    }
    pub fn float_bits(&self, name: &str, default: u32) -> u32 {
        self.get(name).map_or(default, |v| v.float_bits)
    }
}
