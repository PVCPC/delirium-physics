use std::cell::RefCell;

#[derive(Clone)]
pub struct LineBuffer {
    pub bytes: [u8; 1024],
}

impl Default for LineBuffer {
    fn default() -> Self {
        LineBuffer { bytes: [0; 1024] }
    }
}

impl LineBuffer {
    pub fn read(&mut self, file: &[u8], at: &mut usize, blank: bool) -> Option<usize> {
        read_line(&mut self.bytes, file, at)?;
        let b = &mut self.bytes;
        if blank {
            let mut j = 0;
            while b[j] != 0 {
                if (b[j] < b' ' && (b[j] as i8) >= 0) || b[j] == b',' {
                    b[j] = b' ';
                }
                j += 1;
            }
        }
        let mut start = 0;
        while b[start] == b' ' {
            start += 1;
        }
        Some(start)
    }

    pub fn text(&self, start: usize) -> &[u8] {
        let m = &self.bytes[start..];
        &m[..m.iter().position(|&c| c == 0).unwrap_or(m.len())]
    }

    pub fn memory(&self, start: usize) -> &[u8] {
        &self.bytes[start..]
    }
}

pub fn read_line(b: &mut [u8], file: &[u8], at: &mut usize) -> Option<()> {
    let rest = &file[(*at).min(file.len())..];
    let limit = b.len() - 2;
    let take = rest.iter().take(limit).position(|&c| c == b'\n').map_or(rest.len().min(limit), |i| i + 1);
    b[..take].copy_from_slice(&rest[..take]);
    b[take] = 0;
    *at += take;
    let mut n = take;
    while n != 0 && (b[n - 1] as i8) <= 0x20 {
        n -= 1;
    }
    b[n] = 0;
    if n == 0 && *at >= file.len() {
        return None;
    }
    let mut i = 0;
    let mut zero = false;
    while i < n {
        match b[i] {
            0 => {
                zero = true;
                break;
            }
            b'\n' => break,
            b'\r' => b[i] = 0,
            _ => {}
        }
        i += 1;
    }
    if !zero {
        b[i] = 0;
    }
    Some(())
}

thread_local! {
    static SHARED: RefCell<LineBuffer> = RefCell::new(LineBuffer::default());
}

pub fn each_line(file: &[u8], blank: bool, mut f: impl FnMut(&[u8])) {
    let mut at = 0;
    loop {
        let line = SHARED.with(|s| {
            let mut s = s.borrow_mut();
            s.read(file, &mut at, blank).map(|start| s.memory(start).to_vec())
        });
        let Some(line) = line else { break };
        f(&line);
    }
}

pub fn lines(file: &[u8], blank: bool) -> Vec<Vec<u8>> {
    let mut out = vec![];
    each_line(file, blank, |m| out.push(m[..m.iter().position(|&c| c == 0).unwrap_or(m.len())].to_vec()));
    out
}

