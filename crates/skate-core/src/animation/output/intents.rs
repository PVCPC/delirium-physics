use crate::input::graph_intents::IntentMutation;

pub const BUCKET_COUNT: usize = 61;
pub const ENTRY_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentName(pub [u32; 6]);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intent {
    pub name: IntentName,
    pub value: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntentCapacityExceeded;

pub struct IntentMap {
    buckets: [Vec<Intent>; BUCKET_COUNT],
    len: usize,
}

impl Default for IntentMap {
    fn default() -> Self {
        Self {
            buckets: std::array::from_fn(|_| Vec::new()),
            len: 0,
        }
    }
}

impl IntentMap {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, name: IntentName) -> Option<f32> {
        let hash = name.0.iter().fold(0u32, |sum, word| sum.wrapping_add(*word));
        self.buckets[hash as usize % BUCKET_COUNT]
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.value)
    }

    pub fn entries(&self) -> impl Iterator<Item = &Intent> {
        self.buckets.iter().flat_map(|bucket| bucket.iter())
    }

    pub fn insert(&mut self, intent: Intent) -> Result<bool, IntentCapacityExceeded> {
        let hash = intent
            .name
            .0
            .iter()
            .fold(0u32, |sum, word| sum.wrapping_add(*word));
        let bucket = &mut self.buckets[hash as usize % BUCKET_COUNT];
        if bucket.iter().any(|entry| entry.name == intent.name) {
            return Ok(false);
        }
        if self.len == ENTRY_CAPACITY {
            return Err(IntentCapacityExceeded);
        }
        bucket.insert(0, intent);
        self.len += 1;
        Ok(true)
    }

    pub fn set(&mut self, name: IntentName, value: f32) -> Result<bool, IntentCapacityExceeded> {
        let hash = name
            .0
            .iter()
            .fold(0u32, |sum, word| sum.wrapping_add(*word));
        let bucket = &mut self.buckets[hash as usize % BUCKET_COUNT];
        if let Some(entry) = bucket.iter_mut().find(|entry| entry.name == name) {
            entry.value = value;
            return Ok(false);
        }
        self.insert(Intent { name, value })
    }

    pub fn remove(&mut self, name: IntentName) -> bool {
        let hash = name
            .0
            .iter()
            .fold(0u32, |sum, word| sum.wrapping_add(*word));
        let bucket = &mut self.buckets[hash as usize % BUCKET_COUNT];
        let Some(index) = bucket.iter().position(|entry| entry.name == name) else {
            return false;
        };
        bucket.remove(index);
        self.len -= 1;
        true
    }

    pub fn apply(
        &mut self,
        name: IntentName,
        mutation: IntentMutation,
    ) -> Result<bool, IntentCapacityExceeded> {
        match mutation {
            IntentMutation::None => Ok(false),
            IntentMutation::Remove => Ok(self.remove(name)),
            IntentMutation::Set(value) => self.set(name, value),
        }
    }

    pub fn clear(&mut self) {
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.len = 0;
    }

    pub fn replace_from(&mut self, source: &Self) {
        self.clear();
        for &entry in source.entries() {
            self.insert(entry).expect("bounded intent map assignment");
        }
    }
}

