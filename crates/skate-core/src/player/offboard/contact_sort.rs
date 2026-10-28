use super::{contact_records::Record, contact_segments::Candidate};
pub trait Distance: Copy {
    fn distance(self) -> f32;
}
impl Distance for Record {
    fn distance(self) -> f32 {
        self.distance
    }
}
impl Distance for Candidate {
    fn distance(self) -> f32 {
        self.position_distance
    }
}
fn less<T: Distance>(a: T, b: T) -> bool {
    a.distance() < b.distance()
}

pub fn sort<T: Distance>(records: &mut [T]) {
    if records.is_empty() {
        return;
    }
    let depth = 2 * (usize::BITS - 1 - records.len().leading_zeros());
    introsort(records, depth);
    insertion(records);
}
fn insertion<T: Distance>(a: &mut [T]) {
    for index in 1..a.len() {
        let value = a[index];
        let mut hole = index;
        while hole > 0 && less(value, a[hole - 1]) {
            a[hole] = a[hole - 1];
            hole -= 1;
        }
        a[hole] = value;
    }
}
fn introsort<T: Distance>(mut a: &mut [T], mut depth: u32) {
    while a.len() > 28 && depth > 0 {
        let first = a[0];
        let middle = a[a.len() / 2];
        let last = a[a.len() - 1];
        let pivot = if less(first, middle) {
            if less(middle, last) {
                middle
            } else if less(first, last) {
                last
            } else {
                first
            }
        } else if less(first, last) {
            first
        } else if less(middle, last) {
            last
        } else {
            middle
        };
        let mut left = 0;
        let mut right = a.len();
        let cut = loop {
            while less(a[left], pivot) {
                left += 1;
            }
            right -= 1;
            while less(pivot, a[right]) {
                right -= 1;
            }
            if left >= right {
                break left;
            }
            a.swap(left, right);
            left += 1;
        };
        depth -= 1;
        let (lower, upper) = a.split_at_mut(cut);
        introsort(upper, depth);
        a = lower;
    }
    if depth == 0 {
        heap_sort(a);
    }
}
fn heap_sort<T: Distance>(a: &mut [T]) {
    if a.len() < 2 {
        return;
    }
    for index in (0..=(a.len() - 2) / 2).rev() {
        let value = a[index];
        adjust_heap(a, index, value);
    }
    for end in (1..a.len()).rev() {
        let value = a[end];
        a[end] = a[0];
        adjust_heap(&mut a[..end], 0, value);
    }
}
fn adjust_heap<T: Distance>(a: &mut [T], top: usize, value: T) {
    let mut hole = top;
    let mut child = 2 * (hole + 1);
    while child < a.len() {
        if less(a[child], a[child - 1]) {
            child -= 1;
        }
        a[hole] = a[child];
        hole = child;
        child = 2 * (hole + 1);
    }
    if child == a.len() {
        a[hole] = a[child - 1];
        hole = child - 1;
    }
    while hole > top {
        let parent = (hole - 1) / 2;
        if !less(a[parent], value) {
            break;
        }
        a[hole] = a[parent];
        hole = parent;
    }
    a[hole] = value;
}

