#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pad {
    records: Vec<[u32; 4]>,
    count: usize,
}
impl Pad {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            count: 0,
        }
    }

    pub fn from_records(records: Vec<[u32; 4]>) -> Self {
        Self {
            count: records.len(),
            records,
        }
    }
    pub fn from_storage(records: Vec<[u32; 4]>, count: usize) -> Self {
        assert!(count <= records.len());
        Self { records, count }
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn records(&self) -> &[[u32; 4]] {
        &self.records
    }

    pub fn update(&mut self, values: &[f32]) {
        if values.len() > self.count {
            if values.len() > self.records.len() {
                self.records.resize(values.len(), [0; 4]);
            }
            self.records[self.count..values.len()].fill([0; 4]);
        }
        self.count = values.len();
        for (record, &value) in self.records.iter_mut().zip(values) {
            record[0] = value.to_bits();
            let down = u32::from(value > 0.5);
            let held = (record[1] >> 8) & 255;
            if (record[3] as i32) < 3 {
                record[3] = record[3].wrapping_add(1);
                record[1] &= 0xffff;
            } else if down == held {
                record[1] &= 0xffff;
            } else {
                record[1] =
                    (record[1] & 255) | (down << 8) | if down != 0 { 1 << 24 } else { 1 << 16 };
                record[3] = 0;
            }
            if record[1] & 0xff00 != 0 {
                if record[2] == 0 {
                    record[1] = (record[1] & !255) | 1;
                    record[2] = 24;
                } else {
                    record[2] = record[2].wrapping_sub(1);
                    if record[2] == 0 {
                        record[1] = (record[1] & !255) | 1;
                        record[2] = 12;
                    } else {
                        record[1] &= !255;
                    }
                }
            } else {
                record[2] = 0;
                record[1] &= !255;
            }
        }
    }
}
