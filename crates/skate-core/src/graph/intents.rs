use crate::animation::playback_parameters::intent_key;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IntentMap {
    values: BTreeMap<[u32; 6], f32>,
}

impl IntentMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: &str, value: f32) -> Option<f32> {
        self.values.insert(intent_key(name), value)
    }

    pub fn get(&self, name: &str) -> Option<&f32> {
        self.values.get(&intent_key(name))
    }

    pub fn remove(&mut self, name: &str) -> Option<f32> {
        self.values.remove(&intent_key(name))
    }

    pub fn contains_key(&self, name: &str) -> bool {
        self.values.contains_key(&intent_key(name))
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
