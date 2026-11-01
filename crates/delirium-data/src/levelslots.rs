#[derive(Clone, Debug)]
pub struct Slots {
    pub ring: Vec<u16>,
    pub head: usize,
    pub tail: usize,
    pub count: u16,
    pub later: Vec<u16>,
}

impl Slots {
    pub fn new(size: usize) -> Slots {
        Slots { ring: (0..size as u16).collect(), head: 0, tail: 0, count: 0, later: vec![] }
    }

    pub fn take(&mut self, lifo: bool) -> u16 {
        let n = self.ring.len();
        let i = if lifo {
            self.tail = (self.tail + n - 1) % n;
            self.ring[self.tail]
        } else {
            let i = self.ring[self.head];
            self.head = (self.head + 1) % n;
            i
        };
        self.count = self.count.wrapping_add(1);
        i
    }

    pub fn free(&mut self, i: u16) {
        self.count = self.count.wrapping_sub(1);
        self.ring[self.tail] = i;
        self.tail = (self.tail + 1) % self.ring.len();
    }

    pub fn free_later(&mut self, i: u16) {
        if self.later.len() == 511 {
            self.flush();
        }
        self.later.push(i);
    }

    pub fn remove_held(&mut self, held: &mut i32, inst: Option<u16>) {
        if let (true, Some(i)) = (*held != -1, inst.filter(|&i| i != 0xFFFF)) {
            self.free(i);
        }
        *held = -1;
    }

    pub fn flush(&mut self) {
        for i in std::mem::take(&mut self.later) {
            self.free(i);
        }
    }
}

impl Default for Slots {
    fn default() -> Slots {
        Slots::new(6000)
    }
}

