#[derive(Clone)]
pub struct CsvReader<'a> {
    file: &'a [u8],
    at: usize,
    position: i32,
    line: [u8; 1024],
    read: bool,
    len: i32,
    place: i32,
    current: u8,
    previous: u8,
    delimiter: u8,
}

impl<'a> CsvReader<'a> {
    pub fn new(file: &'a [u8], delimiter: u8) -> Self {
        CsvReader { file, at: 0, position: 0, line: [0; 1024], read: false, len: 0, place: 0, current: 0, previous: 0, delimiter }
    }

    pub fn rewind(&mut self) {
        let (file, delimiter) = (self.file, self.delimiter);
        *self = CsvReader::new(file, delimiter);
    }

    pub fn at_end(&self) -> bool {
        self.read && (self.place - self.len).wrapping_add(self.position) >= self.file.len() as i32
    }

    pub fn current(&self) -> u8 {
        self.current
    }
    pub fn previous(&self) -> u8 {
        self.previous
    }

    pub fn next_char(&mut self) -> u8 {
        self.previous = self.current;
        if self.read && self.place < self.len {
            self.place += 1;
        } else {
            let _ = crate::textlines::read_line(&mut self.line, self.file, &mut self.at);
            self.position = self.at as i32;
            self.read = true;
            self.len = self.line.iter().position(|&c| c == 0).unwrap_or(self.line.len()) as i32;
            self.place = 0;
        }
        self.current = self.line[self.place as usize];
        self.current
    }

    pub fn skip_line(&mut self) -> bool {
        if self.current == 0 {
            return true;
        }
        while !self.at_end() {
            if self.next_char() == 0 {
                return true;
            }
        }
        false
    }

    pub fn skip_lines(&mut self, mut n: i32) -> bool {
        while !self.at_end() && n != 0 {
            if self.next_char() == 0 {
                n -= 1;
            }
        }
        !self.at_end()
    }

    pub fn field(&mut self, size: usize, quotes: bool) -> (bool, Vec<u8>) {
        let mut out = vec![];
        let mut quoted = false;
        while !self.at_end() {
            let c = self.next_char();
            if quotes && c == b'"' {
                quoted = !quoted;
                continue;
            }
            if c == b'\r' {
                continue;
            }
            if c == 0 || (!quoted && c == self.delimiter) {
                break;
            }
            out.push(c);
            if out.len() == size {
                out.pop();
                return (false, out);
            }
        }
        (true, out)
    }

    pub fn int(&mut self) -> Option<i32> {
        let (ok, f) = self.field(32, false);
        ok.then(|| crate::textfmt::atoi(&f))
    }

    pub fn float(&mut self) -> Option<f32> {
        let (ok, f) = self.field(32, false);
        ok.then(|| atof(&f) as f32)
    }

    pub fn range(&mut self) -> (bool, f32, f32) {
        let (ok, f) = self.field(64, false);
        if !ok {
            return (false, 0.0, 0.0);
        }
        let s = crate::crt::sscanf(&f, b" %f : %f ");
        (s.count == 2, s.float(0).unwrap_or(0.0), s.float(1).unwrap_or(0.0))
    }
}

pub fn atof(text: &[u8]) -> f64 {
    let s = crate::crt::sscanf(text, b"%lf");
    match s.values.first() {
        Some(crate::crt::Value::Double(v)) => *v,
        _ => 0.0,
    }
}

