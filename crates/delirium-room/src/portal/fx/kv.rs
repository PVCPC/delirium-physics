#[derive(Clone, Debug, PartialEq)]
pub enum KvValue {
    Str(String),
    Block(Kv),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kv {
    pub entries: Vec<(String, KvValue)>,
}

impl Kv {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().rev().find_map(|(k, v)| match v {
            KvValue::Str(s) if k == key => Some(s.as_str()),
            _ => None,
        })
    }

    pub fn block(&self, key: &str) -> Option<&Kv> {
        self.entries.iter().rev().find_map(|(k, v)| match v {
            KvValue::Block(b) if k == key => Some(b),
            _ => None,
        })
    }

    pub fn blocks<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a Kv> + 'a {
        self.entries.iter().filter_map(move |(k, v)| match v {
            KvValue::Block(b) if k == key => Some(b),
            _ => None,
        })
    }

    pub fn root(&self) -> Option<(&str, &Kv)> {
        self.entries.iter().find_map(|(k, v)| match v {
            KvValue::Block(b) => Some((k.as_str(), b)),
            _ => None,
        })
    }

    pub fn set(&mut self, key: &str, value: KvValue) {
        self.entries.retain(|(k, _)| k != key);
        self.entries.push((key.to_string(), value));
    }

    pub fn get_bool(&self, key: &str) -> bool {
        self.get(key).is_some_and(|v| parse_f32(v).unwrap_or(0.0) != 0.0)
    }

    pub fn get_f32(&self, key: &str) -> Option<f32> {
        self.get(key).and_then(parse_f32)
    }
}

pub fn parse_f32(s: &str) -> Option<f32> {
    let s = s.trim();
    s.parse::<f32>().ok().or_else(|| {
        let end = s
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e'))
            .unwrap_or(s.len());
        s[..end].parse().ok()
    })
}

pub fn parse_color(s: &str) -> Option<[f32; 3]> {
    let t = s.trim();
    let (inner, scale) = if let Some(i) = t.strip_prefix('[') {
        (i.trim_end_matches(']'), 1.0)
    } else if let Some(i) = t.strip_prefix('{') {
        (i.trim_end_matches('}'), 1.0 / 255.0)
    } else {
        let v = parse_f32(t)?;
        return Some([v, v, v]);
    };
    let v: Vec<f32> = inner.split_whitespace().filter_map(parse_f32).collect();
    match v.len() {
        0 => None,
        1 | 2 => Some([v[0] * scale; 3]),
        _ => Some([v[0] * scale, v[1] * scale, v[2] * scale]),
    }
}

#[derive(Debug, PartialEq)]
enum Tok<'a> {
    Str(std::borrow::Cow<'a, str>),
    Open,
    Close,
    Cond(&'a str),
}

struct Lexer<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn next(&mut self) -> Option<Tok<'a>> {
        let b = self.s.as_bytes();
        loop {
            while self.pos < b.len() && (b[self.pos].is_ascii_whitespace() || b[self.pos] == 0) {
                self.pos += 1;
            }
            if self.pos + 1 < b.len() && b[self.pos] == b'/' && b[self.pos + 1] == b'/' {
                while self.pos < b.len() && b[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
        if self.pos >= b.len() {
            return None;
        }
        match b[self.pos] {
            b'{' => {
                self.pos += 1;
                Some(Tok::Open)
            }
            b'}' => {
                self.pos += 1;
                Some(Tok::Close)
            }
            b'[' => {
                let start = self.pos + 1;
                while self.pos < b.len() && b[self.pos] != b']' && b[self.pos] != b'\n' {
                    self.pos += 1;
                }
                let t = &self.s[start..self.pos.min(b.len())];
                self.pos = (self.pos + 1).min(b.len());
                Some(Tok::Cond(t))
            }
            b'"' => {
                let start = self.pos + 1;
                let mut i = start;
                let mut has_escape = false;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' && i + 1 < b.len() && b[i + 1] == b'"' {
                        has_escape = true;
                        i += 1;
                    }
                    i += 1;
                }
                let raw = &self.s[start..i.min(b.len())];
                self.pos = (i + 1).min(b.len());
                if has_escape {
                    Some(Tok::Str(raw.replace("\\\"", "\"").into()))
                } else {
                    Some(Tok::Str(raw.into()))
                }
            }
            _ => {
                let start = self.pos;
                while self.pos < b.len()
                    && !b[self.pos].is_ascii_whitespace()
                    && !matches!(b[self.pos], b'{' | b'}' | b'"')
                {
                    if b[self.pos] == b'/' && self.pos + 1 < b.len() && b[self.pos + 1] == b'/' {
                        break;
                    }
                    self.pos += 1;
                }
                Some(Tok::Str(self.s[start..self.pos].into()))
            }
        }
    }
}

fn cond_true(c: &str) -> bool {
    c.split("||").any(|part| {
        let part = part.trim();
        let (neg, name) = match part.strip_prefix('!') {
            Some(n) => (true, n),
            None => (false, part),
        };
        let name = name.trim_start_matches('$').to_ascii_uppercase();
        let v = matches!(name.as_str(), "WIN32" | "WINDOWS" | "PC" | "DX9" | "HDR");
        v != neg
    })
}

pub fn parse(text: &str) -> Kv {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lx = Lexer { s: text, pos: 0 };
    let mut stack: Vec<Kv> = vec![Kv::default()];
    let mut pending_key: Option<String> = None;
    let mut toks: Vec<Tok> = Vec::new();
    while let Some(t) = lx.next() {
        toks.push(t);
    }
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Str(s) => {
                if let Some(key) = pending_key.take() {
                    let keep = match toks.get(i + 1) {
                        Some(Tok::Cond(c)) => {
                            i += 1;
                            cond_true(c)
                        }
                        _ => true,
                    };
                    if keep {
                        stack
                            .last_mut()
                            .unwrap()
                            .entries
                            .push((key, KvValue::Str(s.to_string())));
                    }
                } else {
                    pending_key = Some(s.to_ascii_lowercase());
                }
            }
            Tok::Cond(c) => {
                if !cond_true(c) {
                    if let Some(Tok::Open) = toks.get(i + 1) {
                        let mut depth = 0;
                        i += 1;
                        while i < toks.len() {
                            match toks[i] {
                                Tok::Open => depth += 1,
                                Tok::Close => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            i += 1;
                        }
                        pending_key = None;
                    }
                }
            }
            Tok::Open => {
                let key = pending_key.take().unwrap_or_default();
                stack
                    .last_mut()
                    .unwrap()
                    .entries
                    .push((key, KvValue::Block(Kv::default())));
                stack.push(Kv::default());
            }
            Tok::Close => {
                pending_key = None;
                if stack.len() > 1 {
                    let done = stack.pop().unwrap();
                    let parent = stack.last_mut().unwrap();
                    if let Some((_, KvValue::Block(b))) = parent.entries.last_mut() {
                        *b = done;
                    }
                }
            }
        }
        i += 1;
    }
    while stack.len() > 1 {
        let done = stack.pop().unwrap();
        if let Some((_, KvValue::Block(b))) = stack.last_mut().unwrap().entries.last_mut() {
            *b = done;
        }
    }
    stack.pop().unwrap()
}

