fn be32(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    pub name: String,
    pub offset: u32,
    pub size: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub version: u32,
    pub data: Vec<u8>,
    pub strings: Vec<String>,
    pub objects: Vec<Object>,
    pub offsets_a: Vec<u32>,
    pub offsets_b: Vec<u32>,
}

impl Metadata {
    pub fn read(raw: &[u8]) -> Option<Metadata> {
        let version = be32(raw, 0)?;
        let data_size = be32(raw, 4)? as usize;
        let data = raw.get(8..8 + data_size)?.to_vec();
        let mut at = 8 + data_size;
        let table_size = be32(raw, at)? as usize;
        let table = raw.get(at + 4..at + 4 + table_size)?;
        at += 4 + table_size;
        let count = be32(table, 0)? as usize;
        let base = 4 + 4 * count;
        let strings = (0..count)
            .map(|i| {
                let s = table.get(base + be32(table, 4 + 4 * i)? as usize..)?;
                Some(String::from_utf8_lossy(s.split(|&c| c == 0).next()?).into_owned())
            })
            .collect::<Option<Vec<_>>>()?;
        let object_count = be32(raw, at)? as usize;
        at += 8;
        let mut objects = Vec::with_capacity(object_count.min(1 << 20));
        for _ in 0..object_count {
            let n = *raw.get(at)? as usize;
            let name = String::from_utf8_lossy(raw.get(at + 1..at + 1 + n)?).into_owned();
            objects.push(Object { name, offset: be32(raw, at + 1 + n)?, size: be32(raw, at + 5 + n)? });
            at += 9 + n;
        }
        let mut list = || -> Option<Vec<u32>> {
            let n = be32(raw, at)? as usize;
            let v = (0..n).map(|i| be32(raw, at + 4 + 4 * i)).collect::<Option<Vec<_>>>()?;
            at += 4 + 4 * n;
            Some(v)
        };
        let (offsets_a, offsets_b) = (list()?, list()?);
        Some(Metadata { version, data, strings, objects, offsets_a, offsets_b })
    }

    pub fn bytes(&self, o: &Object) -> &[u8] {
        self.data.get(o.offset as usize..(o.offset + o.size) as usize).unwrap_or(&[])
    }

    pub fn find(&self, name: &str) -> Option<&Object> {
        self.objects.iter().find(|o| o.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub hash: u32,
    pub bank: String,
    pub variations: u16,
    pub variation_data: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voice {
    pub hash: u32,
    pub contexts: Vec<Context>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Speech {
    pub variation_data: Vec<u8>,
    pub voices: Vec<Voice>,
}

impl Speech {
    pub fn read(raw: &[u8]) -> Option<Speech> {
        let a = be32(raw, 0)? as usize;
        let variation_data = raw.get(4..4 + a)?.to_vec();
        let contexts_at = 8 + a;
        let context_count = be32(raw, 4 + a)? as usize;
        let voices_at = contexts_at + context_count * 14 + 4;
        let voice_count = be32(raw, voices_at - 4)? as usize;
        let strings_at = voices_at + voice_count * 10 + 4;
        let string_count = be32(raw, strings_at - 4)? as usize;
        let text_at = strings_at + 4 * string_count;
        let string = |i: usize| -> Option<String> {
            if i >= string_count {
                return None;
            }
            let s = raw.get(text_at + be32(raw, strings_at + 4 * i)? as usize..)?;
            Some(String::from_utf8_lossy(s.split(|&c| c == 0).next()?).into_owned())
        };
        let voices = (0..voice_count)
            .map(|v| {
                let r = voices_at + 10 * v;
                let first = contexts_at + be32(raw, r)? as usize;
                let n = u16::from_be_bytes([*raw.get(r + 8)?, *raw.get(r + 9)?]) as usize;
                let contexts = (0..n)
                    .map(|k| {
                        let c = first + 14 * k;
                        let data = be32(raw, c + 4)? as i32;
                        Some(Context {
                            bank: string(be32(raw, c)? as usize)?,
                            variation_data: (data >= 0).then_some(data as u32),
                            hash: be32(raw, c + 8)?,
                            variations: u16::from_be_bytes([*raw.get(c + 12)?, *raw.get(c + 13)?]),
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(Voice { hash: be32(raw, r + 4)?, contexts })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Speech { variation_data, voices })
    }

    pub fn find(&self, voice: u32, context: u32) -> Option<&Context> {
        let v = &self.voices[self.voices.binary_search_by_key(&voice, |v| v.hash).ok()?];
        v.contexts.iter().find(|c| c.hash == context)
    }
}

pub fn name_hash(name: &str) -> u32 {
    let b = name.as_bytes();
    let quoted = b.first() == Some(&b'"');
    let mut h: u32 = 0;
    for &c in &b[quoted as usize..] {
        if quoted && c == b'"' {
            break;
        }
        let c = match c {
            b'A'..=b'Z' => c + 32,
            b'\\' => b'/',
            _ => c,
        };
        h = h.wrapping_add(c as i8 as u32);
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub flags: u32,
    pub byte_09: u8,
    pub byte_0a: u8,
    pub volume_db: f32,
    pub volume_db_other: f32,
    pub word_0f: u16,
    pub word_11: u16,
    pub word_13: u16,
    pub scale: f32,
    pub scale_other: f32,
    pub factors: [f32; 5],
    pub children: Vec<u32>,
}

impl Category {
    pub fn read(b: &[u8]) -> Option<Category> {
        let u16_at = |o: usize| b.get(o..o + 2).map(|x| u16::from_be_bytes([x[0], x[1]]));
        let hundredth = |o: usize| u16_at(o).map(|v| v as f32 * 0.01);
        let n = *b.get(0x23)? as usize;
        Some(Category {
            flags: be32(b, 5)?,
            byte_09: b[9],
            byte_0a: b[10],
            volume_db: u16_at(0x0B)? as i16 as f32 * 0.01,
            volume_db_other: u16_at(0x0D)? as i16 as f32 * 0.01,
            word_0f: u16_at(0x0F)?,
            word_11: u16_at(0x11)?,
            word_13: u16_at(0x13)?,
            scale: hundredth(0x15)?,
            scale_other: hundredth(0x17)?,
            factors: [hundredth(0x19)?, hundredth(0x1B)?, hundredth(0x1D)?, hundredth(0x1F)?, hundredth(0x21)?],
            children: (0..n).map(|i| be32(b, 0x24 + 4 * i)).collect::<Option<_>>()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub clamp_input: bool,
    pub min_input: f32,
    pub max_input: f32,
    pub shape: Shape,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Constant(f32),
    Linear { x0: f32, y0: f32, x1: f32, y1: f32 },
    LinearToDecibels { x0: f32, y0: f32, x1: f32, y1: f32 },
    Points(Vec<[f32; 2]>),
    Table(Vec<f32>),
    QuarterSine { flip_input: bool, flip_output: bool },
    Power { flip_input: bool, flip_output: bool, exponent: f32 },
    Exponential { k: f32, normalised: bool },
    Gaussian { k: f32, normalised: bool },
    Sine { amplitude: f32, start: f32, end: f32, offset: f32, span_if_end_negative: f32 },
    Reciprocal { k: f32, normalised: bool },
    ReciprocalSquare { k: f32, normalised: bool },
    Polynomial,
    TableTenths(Vec<f32>),
    Other(u8),
}

fn be_f32(b: &[u8], at: usize) -> Option<f32> {
    be32(b, at).map(f32::from_bits)
}

impl Curve {
    pub fn read(b: &[u8]) -> Option<Curve> {
        let line = |b: &[u8]| Some((be_f32(b, 0x15)?, be_f32(b, 0x19)?, be_f32(b, 0x1D)?, be_f32(b, 0x21)?));
        let shape = match *b.first()? {
            1 => Shape::Constant(be_f32(b, 0x15)?),
            2 => line(b).map(|(x0, y0, x1, y1)| Shape::Linear { x0, y0, x1, y1 })?,
            3 => line(b).map(|(x0, y0, x1, y1)| Shape::LinearToDecibels { x0, y0, x1, y1 })?,
            4 => {
                let n = be32(b, 0x15)? as usize;
                Shape::Points((0..n).map(|i| Some([be_f32(b, 0x19 + 8 * i)?, be_f32(b, 0x1D + 8 * i)?])).collect::<Option<_>>()?)
            }
            6 => {
                let n = u16::from_be_bytes([*b.get(0x15)?, *b.get(0x16)?]) as usize;
                Shape::Table((0..n).map(|i| be_f32(b, 0x17 + 4 * i)).collect::<Option<_>>()?)
            }
            5 => Shape::QuarterSine { flip_input: b.get(0x15)? & 1 != 0, flip_output: b.get(0x15)? & 2 != 0 },
            7 => Shape::Power { flip_input: b.get(0x15)? & 1 != 0, flip_output: b.get(0x15)? & 2 != 0, exponent: be_f32(b, 0x16)? },
            8 => Shape::Exponential { k: be_f32(b, 0x15)?, normalised: be32(b, 5)? & 0xC != 0 },
            9 => Shape::Gaussian { k: be_f32(b, 0x15)?, normalised: be32(b, 5)? & 0xC != 0 },
            10 => Shape::Sine {
                start: be_f32(b, 0x15)?,
                end: be_f32(b, 0x19)?,
                span_if_end_negative: be_f32(b, 0x1D)?,
                amplitude: be_f32(b, 0x21)?,
                offset: be_f32(b, 0x25)?,
            },
            11 => Shape::Reciprocal { k: be_f32(b, 0x15)?, normalised: be32(b, 5)? & 0xC != 0 },
            12 => Shape::ReciprocalSquare { k: be_f32(b, 0x15)?, normalised: be32(b, 5)? & 0xC != 0 },
            13 => Shape::Polynomial,
            14 => {
                let n = u16::from_be_bytes([*b.get(0x15)?, *b.get(0x16)?]) as usize;
                Shape::TableTenths((0..n).map(|i| be_f32(b, 0x17 + 4 * i)).collect::<Option<_>>()?)
            }
            t => Shape::Other(t),
        };
        Some(Curve { clamp_input: be32(b, 5)? & 3 == 1, min_input: be_f32(b, 0x0D)?, max_input: be_f32(b, 0x11)?, shape })
    }

    pub fn value(&self, x: f32) -> Option<f32> {
        if let Shape::Points(p) = &self.shape {
            return Some(points_value(p, x));
        }
        let x = if self.clamp_input {
            let x = if x - self.min_input < 0.0 { self.min_input } else { x };
            if x - self.max_input < 0.0 { x } else { self.max_input }
        } else {
            x
        };
        let line = |x0: f32, y0: f32, x1: f32, y1: f32| {
            if x <= x0 {
                y0
            } else if x1 <= x {
                y1
            } else if y0 == y1 {
                y0
            } else {
                ((x - x0) / (x1 - x0)).mul_add(y1 - y0, y0)
            }
        };
        Some(match &self.shape {
            Shape::Constant(v) => *v,
            Shape::Linear { x0, y0, x1, y1 } => line(*x0, *y0, *x1, *y1),
            Shape::LinearToDecibels { x0, y0, x1, y1 } => {
                let v = line(*x0, *y0, *x1, *y1);
                if v - 0.000_01 < 0.0 { -100.0 } else { v.log2() * f32::from_bits(0x3E9A_1CAC) * 20.0 }
            }
            Shape::Table(t) => {
                if t.is_empty() {
                    return Some(0.0);
                }
                let i = (x.floor() as i32).clamp(0, t.len() as i32 - 1);
                t[i as usize]
            }
            Shape::QuarterSine { flip_input, flip_output } => {
                let x = if *flip_input { 1.0 - x } else { x };
                let x = if x >= 0.0 { x } else { 0.0 };
                let x = if x as f64 - 1.0 >= 0.0 { 1.0 } else { x };
                let s = crate::mathrt::sin((x * std::f32::consts::FRAC_PI_2) as f64) as f32;
                if *flip_output { 1.0 - s } else { s }
            }
            Shape::Power { flip_input, flip_output, exponent } => {
                let x = if *flip_input { 1.0 - x } else { x };
                let p = x.powf(*exponent);
                if *flip_output { 1.0 - p } else { p }
            }
            Shape::Exponential { k, normalised } => {
                let e = f32::from_bits(0x402D_F854);
                let end = e.powf(k * -1.0);
                let v = e.powf((k * x) * -1.0);
                if *normalised { (v - end) / (1.0 - end) } else { v }
            }
            Shape::Gaussian { k, normalised } => {
                let e = f32::from_bits(0x402D_F854);
                let end = e.powf((k * k) * -1.0);
                let kx = k * x;
                let v = e.powf((kx * kx) * -1.0);
                if *normalised { (v - end) / (1.0 - end) } else { v }
            }
            Shape::Sine { amplitude, start, end, offset, span_if_end_negative } => {
                let span = if *end >= 0.0 { end - start } else { *span_if_end_negative };
                let angle = span.mul_add(x, *start) * f32::from_bits(0x3C8E_FA35);
                (crate::mathrt::sin(angle as f64) as f32).mul_add(*amplitude, *offset)
            }
            Shape::Reciprocal { k, normalised } => {
                let v = 1.0 / (k * x + 1.0);
                let end = 1.0 / (k + 1.0);
                if *normalised { (v - end) / (1.0 - end) } else { v }
            }
            Shape::ReciprocalSquare { k, normalised } => {
                let d = k * x + 1.0;
                let v = 1.0 / (d * d);
                let end = 1.0 / (k + 1.0);
                let end = end * end;
                if *normalised { (v - end) / (1.0 - end) } else { v }
            }
            Shape::Polynomial => {
                let t = (x - 63.95) * f32::from_bits(0x3CDD_9DC1);
                let t2 = t * t;
                let mut acc = t.mul_add(-17.0421, -62.4395);
                acc = t2.mul_add(4.3724, acc);
                let t3 = t2 * t;
                acc = t3.mul_add(-2.5018, acc);
                let t4 = t3 * t;
                acc = t4.mul_add(0.715, acc);
                let t5 = t4 * t;
                acc = t5.mul_add(0.9942, acc);
                let t6 = t5 * t;
                acc = t6.mul_add(-0.2507, acc);
                let t7 = t6 * t;
                let t8 = t7 * t;
                acc = t7.mul_add(-0.5388, acc);
                acc = t8.mul_add(0.2201, acc);
                let v = if x - 4.99 >= 0.0 { acc } else { 0.0 };
                if x - 128.0 >= 0.0 { -100.0 } else { v }
            }
            Shape::TableTenths(t) => {
                if t.is_empty() {
                    return Some(0.0);
                }
                let i = ((x * 10.0).floor() as i32).clamp(0, t.len() as i32 - 1);
                t[i as usize]
            }
            Shape::Points(_) => unreachable!(),
            Shape::Other(_) => return None,
        })
    }
}

fn points_value(p: &[[f32; 2]], x: f32) -> f32 {
    if p.len() <= 3 {
        let (x0, y0) = p.first().map_or((0.0, 0.0), |a| (a[0], a[1]));
        let (x1, y1) = p.get(1).map_or((x0 + 1.0, y0), |a| (a[0], a[1]));
        let (x2, y2) = p.get(2).map_or((x1 + 2.0, y1), |a| (a[0], a[1]));
        let (r0, r1) = (1.0 / (x1 - x0), 1.0 / (x2 - x1));
        let a = if x - x0 < 0.0 { y0 } else { (r0 * (x - x0)).mul_add(y1 - y0, y0) };
        let b = if x - x1 < 0.0 { a } else { (r1 * (x - x1)).mul_add(y2 - y1, y1) };
        return if x - x2 < 0.0 { b } else { y2 };
    }
    let last = p[p.len() - 1][0];
    let x = if x - last < 0.0 { x } else { last };
    let x = if p[0][0] - x < 0.0 { x } else { p[0][0] };
    let mut i = 1;
    while i < p.len() && x > p[i][0] {
        i += 1;
    }
    let i = i.min(p.len() - 1);
    ((x - p[i - 1][0]) / (p[i][0] - p[i - 1][0])).mul_add(p[i][1] - p[i - 1][1], p[i - 1][1])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SoundKind {
    Looping = 1,
    Envelope = 2,
    TwinLoop = 3,
    Speech = 4,
    OnStop = 5,
    Wrapper = 6,
    Sequential = 7,
    Streaming = 8,
    RetriggeredOverlapped = 9,
    Crossfade = 10,
    CollapsingStereo = 11,
    Simple = 12,
    Multitrack = 13,
    Randomized = 14,
    Environment = 15,
    Switch = 16,
    VariableCurve = 17,
    VariablePrintValue = 18,
    Assert = 19,
    VariableSetTime = 20,
    VariableBlock = 21,
    If = 22,
    ForLoop = 23,
    MathOperation = 24,
}

impl SoundKind {
    pub const ALL: [SoundKind; 24] = {
        use SoundKind::*;
        [
            Looping, Envelope, TwinLoop, Speech, OnStop, Wrapper, Sequential, Streaming, RetriggeredOverlapped, Crossfade,
            CollapsingStereo, Simple, Multitrack, Randomized, Environment, Switch, VariableCurve, VariablePrintValue, Assert, VariableSetTime,
            VariableBlock, If, ForLoop, MathOperation,
        ]
    };

    pub fn from_type(t: u8) -> Option<SoundKind> {
        Self::ALL.into_iter().find(|k| *k as u8 == t)
    }

    pub fn class(self) -> (&'static str, u32) {
        use SoundKind::*;
        match self {
            Looping => ("audLoopingSound", 0xD0),
            Envelope => ("audEnvelopeSound", 0xF0),
            TwinLoop => ("audTwinLoopSound", 0xEC),
            Speech => ("audSpeechSound", 0xAC),
            OnStop => ("audOnStopSound", 0xA8),
            Wrapper => ("audWrapperSound", 0xA4),
            Sequential => ("audSequentialSound", 0xC8),
            Streaming => ("audStreamingSound", 0xDC),
            RetriggeredOverlapped => ("audRetriggeredOverlappedSound", 0xD8),
            Crossfade => ("audCrossfadeSound", 0xA8),
            CollapsingStereo => ("audCollapsingStereoSound", 0xC4),
            Simple => ("audSimpleSound", 0xB0),
            Multitrack => ("audMultitrackSound", 0xAC),
            Randomized => ("audRandomizedSound", 0xA4),
            Environment => ("audEnvironmentSound", 0xE4),
            Switch => ("audSwitchSound", 0xE8),
            VariableCurve => ("audVariableCurveSound", 0xD4),
            VariablePrintValue => ("audVariablePrintValueSound", 0xA8),
            Assert => ("audAssertSound", 0xA4),
            VariableSetTime => ("audVariableSetTimeSound", 0xA8),
            VariableBlock => ("audVariableBlockSound", 0xB0),
            If => ("audIfSound", 0xBC),
            ForLoop => ("audForLoopSound", 0xDC),
            MathOperation => ("audMathOperationSound", 0xAC),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundHeader {
    pub kind: u8,
    pub word_01: u32,
    pub flags: u32,
    pub inherit: [u8; 2],
    pub present: u32,
    pub volume: i16,
    pub volume_variance: u16,
    pub pitch: i16,
    pub pitch_variance: u16,
    pub pan: i16,
    pub pan_variance: u16,
    pub pre_delay: u16,
    pub pre_delay_variance: u16,
    pub start_offset: i32,
    pub start_offset_variance: u32,
    pub attack_time: u16,
    pub release_time: i16,
    pub doppler_percent: u16,
    pub category: u32,
    pub rolloff_curve: u32,
    pub rolloff_scale: u16,
    pub speaker_mask: u8,
    pub route: u8,
    pub volume_variable: u32,
    pub pitch_variable: u32,
    pub pan_variable: u32,
    pub pre_delay_variable: u32,
    pub start_offset_variable: u32,
    pub cutoff_variable: u32,
}

impl Default for SoundHeader {
    fn default() -> Self {
        SoundHeader {
            kind: 0xFF,
            word_01: 0xFFFF_FFFF,
            flags: 0xAAAA_AAAA,
            inherit: [0; 2],
            present: 0xFFFF_FFFF,
            volume: 0,
            volume_variance: 0,
            pitch: 0,
            pitch_variance: 0,
            pan: -1,
            pan_variance: 0,
            pre_delay: 0,
            pre_delay_variance: 0,
            start_offset: 0,
            start_offset_variance: 0,
            attack_time: 0,
            release_time: -1,
            doppler_percent: 100,
            category: 0,
            rolloff_curve: 0,
            rolloff_scale: 100,
            speaker_mask: 0,
            route: 0,
            volume_variable: 0,
            pitch_variable: 0,
            pan_variable: 0,
            pre_delay_variable: 0,
            start_offset_variable: 0,
            cutoff_variable: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimpleSound {
    pub slot: u32,
    pub bank: u32,
    pub wave: u32,
}

impl SimpleSound {
    pub fn read(body: &[u8]) -> Option<SimpleSound> {
        Some(SimpleSound { slot: be32(body, 0)?, bank: be32(body, 4)?, wave: be32(body, 8)? })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundStart {
    pub volume: f32,
    pub pitch: i16,
    pub pan: i16,
    pub pre_delay: u32,
    pub start_offset: i32,
}

impl SoundHeader {
    pub fn read(b: &[u8]) -> Option<(SoundHeader, usize)> {
        let mut h = SoundHeader { kind: *b.first()?, word_01: be32(b, 1)?, flags: be32(b, 5)?, inherit: [*b.get(9)?, *b.get(10)?], present: be32(b, 11)?, ..Default::default() };
        let mut at = 15;
        let mut take = |n: usize| -> Option<u32> {
            let v = b.get(at..at + n)?.iter().fold(0u32, |a, &c| a << 8 | c as u32);
            at += n;
            Some(v)
        };
        let p = h.present;
        macro_rules! field {
            ($bit:expr, $name:ident, $n:expr) => {
                if p & (1 << $bit) != 0 {
                    h.$name = take($n)? as _;
                }
            };
        }
        field!(0, volume, 2);
        field!(1, volume_variance, 2);
        field!(2, pitch, 2);
        field!(3, pitch_variance, 2);
        field!(4, pan, 2);
        field!(5, pan_variance, 2);
        field!(6, pre_delay, 2);
        field!(7, pre_delay_variance, 2);
        field!(8, start_offset, 4);
        field!(9, start_offset_variance, 4);
        field!(10, attack_time, 2);
        field!(11, release_time, 2);
        field!(12, doppler_percent, 2);
        field!(13, category, 4);
        field!(14, rolloff_curve, 4);
        field!(15, rolloff_scale, 2);
        field!(16, speaker_mask, 1);
        field!(17, route, 1);
        field!(18, volume_variable, 4);
        field!(19, pitch_variable, 4);
        field!(20, pan_variable, 4);
        field!(21, pre_delay_variable, 4);
        field!(22, start_offset_variable, 4);
        field!(23, cutoff_variable, 4);
        Some((h, at))
    }

    pub fn start(&self, rng: &mut crate::rng::AudioRand) -> SoundStart {
        let mut draw = |range: i32| if range == 0 { 0 } else { rng.int_in_range(range.wrapping_neg(), range) };
        let v = draw(self.volume_variance as i32);
        let volume = (self.volume as f32).mul_add(0.01, v as f32 * 0.01);
        let pitch = (self.pitch as u16).wrapping_add(draw(self.pitch_variance as i32) as u16) as i16;
        let pan = if self.pan == -1 {
            -1
        } else {
            let d = draw(self.pan_variance as i32) as i16 as i32;
            ((d + self.pan as i32 + 360) % 360) as i16
        };
        let pre_delay = (self.pre_delay as u32).wrapping_add(draw(self.pre_delay_variance as i32) as u32);
        let start_offset = self.start_offset.wrapping_add(draw(self.start_offset_variance as i32));
        SoundStart { volume, pitch, pan, pre_delay, start_offset }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Sound,
    Variable,
    Curve,
    I16,
    U16,
    U8,
    Word,
    Bytes(u8),
}

impl Field {
    pub fn size(self) -> usize {
        match self {
            Field::I16 | Field::U16 => 2,
            Field::U8 => 1,
            Field::Bytes(n) => n as usize,
            _ => 4,
        }
    }
}

impl SoundKind {
    pub fn fixed_layout(self) -> Option<&'static [(u8, Field, &'static str)]> {
        use Field::*;
        Some(match self {
            SoundKind::Looping => &[(0, I16, "+0xBC, sign kept"), (2, U16, "+0xC0"), (4, Sound, "the sound looped; kept at +0xC8")],
            SoundKind::OnStop => &[(0, Sound, "CHILD"), (4, Sound, "ONSTOP"), (8, Sound, "ONFINISHED")],
            SoundKind::Wrapper => &[(0, Sound, "the sound wrapped")],
            SoundKind::RetriggeredOverlapped => {
                &[(0, I16, "+0xC4, sign kept"), (2, U16, "+0xC0"), (4, Variable, "+0xD0"), (8, Variable, "+0xCC"), (12, Sound, "the sound retriggered; kept at +0xC8")]
            }
            SoundKind::Crossfade => &[
                (0, Sound, "NEAR"),
                (4, Sound, "FAR"),
                (8, U8, "helper +0x72; not 0 sets the sound's +0x3A bit 5"),
                (9, Word, "helper +0x48"),
                (13, Word, "helper +0x4C"),
                (17, Word, "helper +0x50"),
                (21, Variable, "helper +0x54"),
                (25, Variable, "helper +0x58"),
                (29, Word, "not read by the set-up"),
                (33, Variable, "helper +0x60"),
                (37, Curve, "helper +0x00"),
            ],
            SoundKind::CollapsingStereo => &[
                (0, Sound, "first child (slots 0 and 2)"),
                (4, Sound, "second child (slots 1 and 3)"),
                (8, Word, "+0xA4"),
                (12, Word, "+0xA8"),
                (16, Variable, "+0xAC"),
                (20, Variable, "+0xB0"),
                (24, Variable, "+0xB4"),
                (28, Variable, "+0xB8: the first child's pan, with the next; without both: 270 and 90"),
                (32, Variable, "+0xBC: the second child's pan"),
                (36, U8, "+0xC1"),
            ],
            SoundKind::Simple => &[(0, Word, "the wave's slot once found"), (4, Word, "the bank's name hash"), (8, Word, "the wave's name hash")],
            SoundKind::Envelope => &[
                (0, U16, "+0xE4: the attack, ms"),
                (2, U16, "+0xE6: the decay, ms"),
                (4, U8, "+0xE8: the sustain level, percent"),
                (5, Word, "+0xDC: the hold, ms (signed; negative: until stopped)"),
                (9, Word, "+0xE0: the release, ms (signed; negative: none)"),
                (13, Curve, "the attack's shape; slot record +0x00"),
                (17, Curve, "the decay's shape; slot record +0x28"),
                (21, Curve, "the release's shape; slot record +0x50"),
                (25, Variable, "+0xC8: the attack, seconds"),
                (29, Variable, "+0xCC: the decay, seconds"),
                (33, Variable, "+0xD4: the sustain level, percent"),
                (37, Variable, "+0xD0: the hold, seconds"),
                (41, Variable, "+0xD8: the release, seconds"),
                (45, Sound, "the sound shaped; -1 or 0: none"),
            ],
            SoundKind::VariableCurve => &[(0, Sound, "the sound played"), (4, Variable, "+0xA4: read"), (8, Variable, "+0xA8: written"), (12, Curve, "+0xAC")],
            SoundKind::VariablePrintValue => &[(0, Variable, "+0xA4"), (4, Bytes(15), "a counted string; not read by the set-up")],
            SoundKind::VariableSetTime => &[(0, Variable, "+0xA4")],
            SoundKind::If => &[
                (0, Sound, "played when the condition holds"),
                (4, Sound, "played when it does not"),
                (8, Variable, "+0xA4: the left side"),
                (12, U8, "+0xB0: the condition"),
                (13, Word, "+0xA8: the right side, a float, used if there is no variable for it"),
                (17, Variable, "+0xAC: the right side"),
            ],
            SoundKind::ForLoop => &[
                (0, Sound, "the sound played; kept at +0xBC"),
                (4, Word, "+0xD4"),
                (8, Variable, "+0xC4"),
                (12, Word, "+0xD0"),
                (16, Variable, "+0xCC"),
                (20, Word, "+0xD8"),
                (24, Variable, "+0xC8"),
                (28, Variable, "+0xC0: has to be there"),
            ],
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Less = 0,
    LessOrEqual = 1,
    Greater = 2,
    GreaterOrEqual = 3,
    Equal = 4,
    NotEqual = 5,
}

impl Condition {
    pub fn from_byte(b: u8) -> Option<Condition> {
        use Condition::*;
        [Less, LessOrEqual, Greater, GreaterOrEqual, Equal, NotEqual].into_iter().find(|c| *c as u8 == b)
    }

    pub fn holds(self, left: f32, right: f32) -> bool {
        const MARGIN: f32 = 0.0001;
        match self {
            Condition::Less => !(right - MARGIN <= left),
            Condition::LessOrEqual => !(right + MARGIN <= left),
            Condition::Greater => !(left <= right + MARGIN),
            Condition::GreaterOrEqual => !(left <= right - MARGIN),
            Condition::Equal => !(left <= right - MARGIN || right + MARGIN <= left),
            Condition::NotEqual => !(right - MARGIN < left && left < right + MARGIN),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MathOperation {
    pub operation: u8,
    pub operands: [(u32, u32); 3],
    pub result: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CountedSound {
    Multitrack(Vec<(u32, u32)>),
    Sequential(Vec<(u32, u32)>),
    Streaming { word: u32, children: Vec<(u32, u32)> },
    Switch { variable: u32, sounds: Vec<u32> },
    Randomized(Randomized),
    MathOperation { sound: u32, operations: Vec<MathOperation> },
    VariableBlock { sound: u32, variables: Vec<(u32, u32, u8)> },
    TwinLoop { times: [u16; 4], crossfade: u32, variables: [u32; 4], children: Vec<(u32, u32)> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Randomized {
    pub variable: u32,
    pub position: u8,
    pub history: Vec<u8>,
    pub choices: Vec<(u32, f32)>,
}

impl Randomized {
    pub fn pick(&self, rng: &mut crate::rng::AudioRand) -> u8 {
        let all = self.choices.len() <= self.history.len();
        let allowed = |i: usize| all || !self.history.iter().any(|&h| h as i8 as i32 == (i & 0xFF) as i32);
        let mut total = 0.0f32;
        for (i, c) in self.choices.iter().enumerate() {
            if allowed(i) {
                total += c.1;
            }
        }
        let mut left = rng.float_in_range(0.0, total);
        for (i, c) in self.choices.iter().enumerate() {
            if allowed(i) {
                left -= c.1;
                if left <= 0.0 {
                    return i as u8;
                }
            }
        }
        0
    }

    pub fn next(&mut self, rng: &mut crate::rng::AudioRand) -> u8 {
        let pick = self.pick(rng);
        if !self.history.is_empty() {
            let n = self.history.len();
            if let Some(slot) = self.history.get_mut(self.position as usize) {
                *slot = pick;
            }
            self.position = ((self.position as usize + 1) % n) as u8;
        }
        pick
    }
}

impl CountedSound {
    pub fn read(kind: SoundKind, b: &[u8]) -> Option<(CountedSound, usize, Vec<usize>)> {
        let mut refs = vec![];
        let mut pairs = |at: usize, n: usize| -> Option<Vec<(u32, u32)>> {
            (0..n)
                .map(|i| {
                    refs.push(at + 8 * i);
                    Some((be32(b, at + 8 * i)?, be32(b, at + 8 * i + 4)?))
                })
                .collect()
        };
        let (data, size) = match kind {
            SoundKind::Multitrack | SoundKind::Sequential => {
                let n = *b.first()? as usize;
                let children = pairs(1, n)?;
                (if kind == SoundKind::Multitrack { CountedSound::Multitrack(children) } else { CountedSound::Sequential(children) }, 1 + 8 * n)
            }
            SoundKind::Streaming => {
                let n = *b.get(4)? as usize;
                (CountedSound::Streaming { word: be32(b, 0)?, children: pairs(5, n)? }, 5 + 8 * n)
            }
            SoundKind::TwinLoop => {
                let n = *b.get(0x1C)? as usize;
                let u = |at: usize| Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]));
                let data = CountedSound::TwinLoop {
                    times: [u(0)?, u(2)?, u(4)?, u(6)?],
                    crossfade: be32(b, 8)?,
                    variables: [be32(b, 12)?, be32(b, 16)?, be32(b, 20)?, be32(b, 24)?],
                    children: pairs(0x1D, n)?,
                };
                (data, 0x1D + 8 * n)
            }
            SoundKind::Switch => {
                let n = *b.get(4)? as usize;
                let sounds = (0..n).map(|i| be32(b, 5 + 4 * i)).collect::<Option<Vec<_>>>()?;
                refs.extend((0..n).map(|i| 5 + 4 * i));
                (CountedSound::Switch { variable: be32(b, 0)?, sounds }, 5 + 4 * n)
            }
            SoundKind::Randomized => {
                let h = *b.get(5)? as usize;
                let n = *b.get(6 + h)? as usize;
                let at = 7 + h;
                let choices = (0..n).map(|i| Some((be32(b, at + 8 * i)?, f32::from_bits(be32(b, at + 8 * i + 4)?)))).collect::<Option<Vec<_>>>()?;
                refs.extend((0..n).map(|i| at + 8 * i));
                let r = Randomized { variable: be32(b, 0)?, position: *b.get(4)?, history: b.get(6..6 + h)?.to_vec(), choices };
                (CountedSound::Randomized(r), at + 8 * n)
            }
            SoundKind::MathOperation => {
                let n = *b.get(4)? as usize;
                let operations = (0..n)
                    .map(|i| {
                        let at = 5 + 29 * i;
                        Some(MathOperation {
                            operation: *b.get(at)?,
                            operands: [(be32(b, at + 1)?, be32(b, at + 5)?), (be32(b, at + 9)?, be32(b, at + 13)?), (be32(b, at + 17)?, be32(b, at + 21)?)],
                            result: be32(b, at + 25)?,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                refs.push(0);
                (CountedSound::MathOperation { sound: be32(b, 0)?, operations }, 5 + 29 * n)
            }
            SoundKind::VariableBlock => {
                let n = *b.get(4)? as usize;
                let variables = (0..n).map(|i| Some((be32(b, 5 + 9 * i)?, be32(b, 9 + 9 * i)?, *b.get(13 + 9 * i)?))).collect::<Option<Vec<_>>>()?;
                refs.push(0);
                (CountedSound::VariableBlock { sound: be32(b, 0)?, variables }, 5 + 9 * n)
            }
            _ => return None,
        };
        Some((data, size, refs))
    }
}

fn fsel(test: f32, at_least_zero: f32, below: f32) -> f32 {
    if test >= 0.0 { at_least_zero } else { below }
}

impl MathOperation {
    pub fn evaluate(operation: u8, a: f32, b: f32, c: f32, time_ms: u32, rng: &mut crate::rng::AudioRand) -> Option<f32> {
        let floor = |x: f32| crate::mathrt::floor(x as f64) as f32;
        let made = |v: f32| if v.is_nan() && !a.is_nan() && !b.is_nan() { f32::from_bits(0x7FC0_0000) } else { v };
        Some(match operation {
            0 => made(b + a),
            1 => made(a - b),
            2 => made(b * a),
            3 => made(a / b),
            4 => a,
            5 => {
                let (x, y) = (a as i64 as u32, b as i64 as u32);
                if y == 0 {
                    return None;
                }
                (x % y) as f32
            }
            6 => fsel(a - b, b, a),
            7 => fsel(a - b, a, b),
            8 | 15 => a.abs(),
            9 => fsel(a, fsel(-a, 0.0, 1.0), -1.0),
            10 => floor(a),
            11 => crate::mathrt::ceil(a as f64) as f32,
            12 => rng.float_in_range(a, b),
            13 => crate::mathrt::sin(a as f64) as f32,
            14 => crate::mathrt::cos(a as f64) as f32,
            16 => fsel(a + 100.0, 10.0f32.powf(a * 0.05), 0.0),
            17 => fsel(a - 0.000_01, (a.log2() * f32::from_bits(0x3E9A_1CAC)) * 20.0, -100.0),
            18 => 2.0f32.powf((a as i32) as f32 * f32::from_bits(0x3A5A_740E)),
            19 => ((((a as f64).log10() as f32 * f32::from_bits(0x4054_9A78)) * 1200.0) as i32) as f32,
            20 => time_ms as f32 * 0.001,
            21 => fsel(a, b, c),
            22 => (c - b).mul_add(a, b),
            23 => {
                let larger = fsel(a - b, a, b);
                if larger as f64 - c as f64 >= 0.0 { c } else { larger }
            }
            24 => a.powf(b),
            25 => ((if a >= 0.0 { a + 0.5 } else { a - 0.5 }) as i32) as f32,
            26 => (crate::mathrt::sin((a * f32::from_bits(0x40C9_0FDB)) as f64) as f32 + 1.0) * 0.5,
            27 => {
                let part = a - floor(a);
                let past_half = part - 0.5;
                fsel(past_half, -past_half.mul_add(2.0, -1.0), part * 2.0)
            }
            28 => a - floor(a),
            29 => fsel((a - floor(a)) - 0.5, 0.0, 1.0),
            30 => {
                let d = b - a;
                let sign = fsel(d, fsel(-d, 0.0, 1.0), -1.0);
                let step = fsel(d.abs() - c, c, d.abs());
                -sign.mul_add(step, -b)
            }
            31 => {
                let x = (time_ms as f32 / a) * 0.001;
                x - floor(x)
            }
            _ => 0.0,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectKind {
    Null = 1,
    Reverb = 2,
    BiquadFilter = 3,
    Convolution = 4,
    Compressor = 5,
    Waveshaper = 6,
    Delay = 7,
}

impl EffectKind {
    pub fn from_type(t: u8) -> Option<EffectKind> {
        use EffectKind::*;
        [Null, Reverb, BiquadFilter, Convolution, Compressor, Waveshaper, Delay].into_iter().find(|k| *k as u8 == t)
    }

    pub fn class(self) -> (&'static str, u32) {
        match self {
            EffectKind::Null => ("audNullEffect", 0x74),
            EffectKind::Reverb => ("audReverbEffect", 0xB8),
            EffectKind::BiquadFilter => ("audBiquadFilterEffect", 0xBC),
            EffectKind::Convolution => ("audConvolutionEffect", 0x74),
            EffectKind::Compressor => ("audCompressorEffect", 0xE0),
            EffectKind::Waveshaper => ("audWaveshaperEffect", 0xB0),
            EffectKind::Delay => ("audDelayEffect", 0x150),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub kind: u8,
    pub bytes_01: [u8; 9],
    pub next: Option<u32>,
    pub byte_0e: u8,
    pub reverb: Option<[u32; 4]>,
    pub rest: Vec<u8>,
}

impl Effect {
    pub fn read(b: &[u8]) -> Option<Effect> {
        let next = be32(b, 10)?;
        let reverb = if b[0] == EffectKind::Reverb as u8 { Some([be32(b, 15)?, be32(b, 19)?, be32(b, 23)?, be32(b, 27)?]) } else { None };
        Some(Effect {
            kind: b[0],
            bytes_01: b.get(1..10)?.try_into().unwrap(),
            next: (next != u32::MAX).then_some(next),
            byte_0e: *b.get(14)?,
            reverb,
            rest: b.get(if reverb.is_some() { 31 } else { 15 }..)?.to_vec(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Submix {
    pub name: &'static str,
    pub slot: u8,
    pub level: u8,
    pub outputs: &'static [&'static str],
    pub gains: &'static [f32],
}

pub const SUBMIXES: [Submix; 9] = [
    Submix { name: "MASTER", slot: 8, level: 5, outputs: &[], gains: &[1.0] },
    Submix { name: "SFX", slot: 7, level: 4, outputs: &["MASTER"], gains: &[1.0] },
    Submix { name: "MUSIC", slot: 3, level: 3, outputs: &["MASTER"], gains: &[1.0] },
    Submix { name: "FE", slot: 4, level: 3, outputs: &["SFX"], gains: &[1.0] },
    Submix { name: "LISTENER", slot: 6, level: 3, outputs: &["SFX"], gains: &[1.0] },
    Submix { name: "PRE_LISTENER", slot: 5, level: 2, outputs: &["LISTENER", "SFX"], gains: &[0.0, 1.0] },
    Submix { name: "SOURCE_REVERB_SMALL", slot: 0, level: 1, outputs: &["PRE_LISTENER"], gains: &[1.0] },
    Submix { name: "SOURCE_REVERB_MEDIUM", slot: 1, level: 1, outputs: &["PRE_LISTENER"], gains: &[1.0] },
    Submix { name: "SOURCE_REVERB_LARGE", slot: 2, level: 1, outputs: &["PRE_LISTENER"], gains: &[1.0] },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GameObjectKind {
    Vehicle = 0,
    CollisionMaterial = 1,
    StaticEmitter = 2,
    EntityEmitter = 3,
    Helicopter = 4,
    MeleeCombat = 5,
    SpeechContexts = 6,
    Boat = 7,
    Weapon = 8,
    Footsteps = 9,
    RadioStationList = 10,
    RadioStation = 11,
    RadioTrackList = 12,
    RadioTrackCategoryWeights = 13,
    ScannerCrime = 14,
    Clothing = 15,
    Ped = 16,
    AmbientEmitterList = 17,
    ScannerScriptedReport = 18,
    AmbientZone = 19,
    AmbientRule = 20,
    AmbientZoneList = 21,
    ScannerArea = 22,
    Cutscene = 23,
    Interior = 24,
    Door = 25,
}

impl GameObjectKind {
    pub fn from_type(t: u8) -> Option<GameObjectKind> {
        use GameObjectKind::*;
        const ALL: [GameObjectKind; 26] = [
            Vehicle, CollisionMaterial, StaticEmitter, EntityEmitter, Helicopter, MeleeCombat, SpeechContexts, Boat, Weapon, Footsteps,
            RadioStationList, RadioStation, RadioTrackList, RadioTrackCategoryWeights, ScannerCrime, Clothing, Ped, AmbientEmitterList,
            ScannerScriptedReport, AmbientZone, AmbientRule, AmbientZoneList, ScannerArea, Cutscene, Interior, Door,
        ];
        ALL.get(t as usize).copied()
    }
}

impl Metadata {
    pub fn find_hash(&self, hash: u32) -> Option<&Object> {
        if hash == 0 {
            return None;
        }
        self.objects.iter().find(|o| name_hash(&o.name) == hash)
    }
}

pub const SECOND_LIST_CONTEXTS: [&str; 9] =
    ["RUN_FROM_FIGHT", "FIGHT", "DODGE", "BUMP", "GUN_RUN", "GUN_COOL", "TRAPPED", "VEHICLE_ATTACKED", "SURPRISED"];
pub const SECOND_LIST_DRIVER_CONTEXTS: [&str; 15] = [
    "RUN_FROM_FIGHT", "FIGHT", "DODGE", "BUMP", "GUN_RUN", "GUN_COOL", "TRAPPED", "VEHICLE_ATTACKED", "SURPRISED", "CRASH_CAR",
    "BLOCKED_VEHICLE", "BLOCKED_PED", "BLOCKED_GENERIC", "JACKED_IN_CAR", "JACK_CAR_BACK",
];
pub const THIRD_LIST_CONTEXTS: [&str; 52] = [
    "ABUSE_DRIVER", "BEEN_SHOT_SUBMIT", "BLOCKED_PED", "BLOCKED_VEHICLE", "BUMP", "CONV_GANG_RESP", "CONV_GANG_STATE", "CONV_SMOKE_RESP",
    "CONV_SMOKE_STATE", "COVER_ME", "CRASH_BIKE", "CRASH_CAR", "CRASH_GENERIC", "DODGE", "DUCK", "FIGHT", "FOLLOWED_NO",
    "GANG_ASK_PLAYER_LEAVE", "GANG_ATTACK_WARNING", "GANG_BUMP", "GANG_CHASE", "GANG_DODGE_WARNING", "GANG_FIGHT_CHEER", "GANG_INTERVENE",
    "GANG_WATCH_THIS_GUY", "GANG_WATCH_THIS_GUY_RESP", "GANG_WATCH_THIS_GUY_SOLO", "GANG_WEAPON_WARNING", "GANG_YOU_DROP_WEAPON",
    "GENERIC_BYE", "GENERIC_FUCK_OFF", "GENERIC_HI", "GENERIC_NO", "GENERIC_YES", "INTIMIDATE_RESP", "JACKED_IN_CAR", "JACKED_ON_STREET",
    "JACKING_CAR_BACK", "JACKING_GENERIC_BACK", "MOVE_IN", "OVER_HANDLEBARS", "PLAYER_OVER_THERE", "PLAYER_UP_THERE", "SHIT",
    "SHOT_IN_LEG", "SHUT_UP_HORN", "SURROUNDED", "TAKE_COVER", "TARGET", "THANKS", "TRAPPED", "VEHICLE_ATTACKED",
];

pub fn voice_class(speech: &Speech, voice: u32) -> u8 {
    let has = |context: &str| speech.find(voice, name_hash(context)).is_some();
    if has("CONV_GANG_STATE") {
        3
    } else if has("HOOKER_SEX") {
        2
    } else {
        has("CRASH_CAR") as u8
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PedVoices {
    pub lists: [Vec<(u32, i32)>; 3],
}

impl PedVoices {
    pub fn read(b: &[u8]) -> Option<PedVoices> {
        let mut at = 10;
        let mut lists: [Vec<(u32, i32)>; 3] = Default::default();
        for list in &mut lists {
            let n = *b.get(at)? as usize;
            at += 1;
            for _ in 0..n {
                list.push((be32(b, at)?, be32(b, at + 4)? as i32));
                at += 8;
            }
        }
        (at == b.len()).then_some(PedVoices { lists })
    }

    pub fn first(&self) -> Option<u32> {
        self.lists[0].first().map(|v| v.0)
    }

    pub fn pick(&self, list: usize, rng: &mut crate::rng::AudioRand, mut usable: impl FnMut(u32) -> bool) -> Option<(u32, i32)> {
        let l = &self.lists[list];
        if l.is_empty() {
            return None;
        }
        let start = rng.int_in_range(0, l.len() as i32 - 1);
        let (mut best, mut fewest) = (None, -1);
        for i in 0..l.len() as i32 {
            let v = l[((i + start) % l.len() as i32) as usize];
            if (fewest == -1 || v.1 < fewest) && usable(v.0) {
                fewest = v.1;
                best = Some(v);
            }
        }
        best
    }

    pub fn choose(&self, context: &str, speech: &Speech, rng: &mut crate::rng::AudioRand) -> Option<u32> {
        let is = |list: &[&str]| list.iter().any(|c| name_hash(c) == name_hash(context));
        let known = |v: u32| speech.voices.binary_search_by_key(&v, |x| x.hash).is_ok();
        let second = if is(&SECOND_LIST_CONTEXTS) { self.pick(1, rng, known) } else { None };
        let drivers = if is(&SECOND_LIST_DRIVER_CONTEXTS) { self.pick(1, rng, |v| voice_class(speech, v) == 1 && known(v)) } else { None };
        let third = if is(&THIRD_LIST_CONTEXTS) { self.pick(2, rng, known) } else { None };
        let first = self.pick(0, rng, known);
        let mut other: Option<(u32, i32)> = None;
        for c in [second, drivers, third].into_iter().flatten() {
            if c.1 > -1 && other.is_none_or(|o| c.1 < o.1) {
                other = Some(c);
            }
        }
        match (first.filter(|f| f.1 >= 0), other) {
            (Some(f), Some(o)) => Some(if o.1 <= f.1 { o.0 } else { f.0 }),
            (Some(f), None) => Some(f.0),
            (None, o) => o.map(|o| o.0),
        }
    }

    pub fn count_use(&mut self, voice: u32, by: i32) {
        if voice == 0 {
            return;
        }
        for list in &mut self.lists {
            if let Some(v) = list.iter_mut().find(|v| v.0 == voice) {
                v.1 = v.1.wrapping_add(by);
                return;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Footsteps {
    pub groups: [(u8, [u32; 5]); 5],
}

impl Footsteps {
    pub fn read(b: &[u8]) -> Option<Footsteps> {
        if b.len() != 115 {
            return None;
        }
        let mut groups = [(0u8, [0u32; 5]); 5];
        for (g, group) in groups.iter_mut().enumerate() {
            let at = 10 + 21 * g;
            *group = (b[at], std::array::from_fn(|i| be32(b, at + 1 + 4 * i).unwrap()));
        }
        Some(Footsteps { groups })
    }

    pub fn landing_sound(&self, footwear: u32) -> Option<u32> {
        let group = match footwear {
            16.. => return None,
            2 | 15 => 3,
            3 => 1,
            4 => 0,
            6 if self.groups[4].0 == 5 => 4,
            _ => 2,
        };
        Some(self.groups[group].1[2])
    }

    pub fn step_sound(&self, step: u32, footwear: u32, special: bool) -> Option<u32> {
        let mut sound = if step == 14 || step == 15 { 4 } else { (step >> 1) as usize };
        if special && step != 16 && step != 17 {
            if step > 1 {
                return None;
            }
            sound = 3;
        } else if step == 16 || step == 17 {
            sound = 3;
        }
        let group = match footwear {
            16.. => return None,
            2 | 15 => 3,
            3 => 1,
            4 => 0,
            6 if self.groups[4].0 == 5 => 4,
            _ => 2,
        };
        let mut bytes = vec![0u8; 10];
        for g in &self.groups {
            bytes.push(g.0);
            g.1.iter().for_each(|w| bytes.extend(w.to_be_bytes()));
        }
        be32(&bytes, 10 + 21 * group + 1 + 4 * sound)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Door {
    pub pass: u32,
    pub limit: u32,
    pub hit_sound: u32,
    pub shut: u32,
    pub threshold: f32,
}

impl Door {
    pub fn read(b: &[u8]) -> Option<Door> {
        let w = |at: usize| be32(b, at).unwrap();
        (b.len() == 30).then(|| Door { pass: w(10), limit: w(14), hit_sound: w(18), shut: w(22), threshold: f32::from_bits(w(26)) })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clothing {
    pub rustle: u32,
    pub legs_loop: u32,
}

impl Clothing {
    pub fn read(b: &[u8]) -> Option<Clothing> {
        Some(Clothing { rustle: be32(b, 10)?, legs_loop: be32(b, 18)? })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScannerCrime {
    pub opening: Vec<(u32, f32)>,
    pub crime: Vec<(u32, f32)>,
}

impl ScannerCrime {
    pub fn read(b: &[u8]) -> Option<ScannerCrime> {
        let mut at = 10;
        let mut list = || -> Option<Vec<(u32, f32)>> {
            let n = *b.get(at)? as usize;
            let v = (0..n).map(|i| Some((be32(b, at + 1 + 8 * i)?, f32::from_bits(be32(b, at + 5 + 8 * i)?)))).collect::<Option<Vec<_>>>()?;
            at += 1 + 8 * n;
            Some(v)
        };
        let (opening, crime) = (list()?, list()?);
        (at == b.len()).then_some(ScannerCrime { opening, crime })
    }

    pub fn pick(list: &[(u32, f32)], mut x: f32) -> u32 {
        for &(sound, weight) in list {
            x -= weight;
            if x <= 0.0 {
                return sound;
            }
        }
        0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScannerPhrase {
    pub sound: u32,
    pub slot: u8,
    pub gap_ms: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannerScriptedReport {
    pub phrases: Vec<ScannerPhrase>,
}

impl ScannerScriptedReport {
    pub fn read(b: &[u8]) -> Option<ScannerScriptedReport> {
        let n = u16::from_be_bytes(b.get(10..12)?.try_into().ok()?) as usize;
        if b.len() != 12 + 7 * n {
            return None;
        }
        let phrases = (0..n)
            .map(|i| {
                let at = 12 + 7 * i;
                ScannerPhrase { sound: be32(b, at).unwrap(), slot: b[at + 4], gap_ms: u16::from_be_bytes([b[at + 5], b[at + 6]]) }
            })
            .collect();
        Some(ScannerScriptedReport { phrases })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannerArea {
    pub name: u32,
    pub transfer: u32,
    pub this_train: u32,
    pub approaching: u32,
    pub sounds_1a: [u32; 2],
    pub display: String,
}

impl ScannerArea {
    pub fn read(b: &[u8]) -> Option<ScannerArea> {
        let n = *b.get(0x22)? as usize;
        let display = String::from_utf8_lossy(b.get(0x23..0x23 + n)?).into_owned();
        (b.len() == 0x23 + n).then(|| ScannerArea {
            name: be32(b, 0x0A).unwrap(),
            transfer: be32(b, 0x0E).unwrap(),
            this_train: be32(b, 0x12).unwrap(),
            approaching: be32(b, 0x16).unwrap(),
            sounds_1a: [be32(b, 0x1A).unwrap(), be32(b, 0x1E).unwrap()],
            display,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeleeCombat {
    pub swing: u32,
    pub hit: u32,
    pub hit_ped: u32,
    pub sound_16: u32,
}

impl MeleeCombat {
    pub fn read(b: &[u8]) -> Option<MeleeCombat> {
        (b.len() == 26).then(|| MeleeCombat { swing: be32(b, 10).unwrap(), hit: be32(b, 14).unwrap(), hit_ped: be32(b, 18).unwrap(), sound_16: be32(b, 22).unwrap() })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weapon {
    pub fire: u32,
    pub echo: u32,
    pub casing_bounce: u32,
    pub swing: u32,
    pub hit: u32,
    pub hit_ped: u32,
    pub heft: u32,
    pub put_down: u32,
    pub rattle: u32,
    pub pickup: u32,
    pub casing: u8,
    pub safety_on: u32,
    pub safety_off: u32,
}

impl Weapon {
    pub fn read(b: &[u8]) -> Option<Weapon> {
        if b.len() != 59 {
            return None;
        }
        let w = |at: usize| be32(b, at).unwrap();
        Some(Weapon {
            fire: w(10),
            echo: w(14),
            casing_bounce: w(18),
            swing: w(22),
            hit: w(26),
            hit_ped: w(30),
            heft: w(34),
            put_down: w(38),
            rattle: w(42),
            pickup: w(46),
            casing: b[50],
            safety_on: w(51),
            safety_off: w(55),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollisionMaterial {
    pub flags: u32,
    pub impact_sound: u32,
    pub scrape_sound: u32,
    pub break_sound: u32,
    pub bullet_sound: u32,
    pub impulse_percent: u8,
    pub min_impulse: u8,
    pub impulse_range: u8,
    pub max_scrape_speed: u8,
    pub weight: u16,
    pub bullet_hit_percent: u8,
    pub footsteps: u32,
    pub footstep_volume: u8,
    pub footstep_impact: u8,
    pub strength_curve: u32,
    pub volume_curve: u32,
    pub slide_pitch_curve: u32,
    pub slide_volume_curve: u32,
    pub sounds_37: [u32; 11],
    pub climb_surface: u8,
    pub door: u32,
}

pub fn ricochet_chance(d: [f32; 3], n: [f32; 3]) -> f32 {
    let dot = -(n[0] * d[0] + n[1] * d[1] + n[2] * d[2]);
    let a = dot.clamp(-0.99999, 0.99999).acos();
    -(a * 57.295776 - 90.0) * 0.011111111 * 0.65 + 0.25
}

pub fn ricochet_out(d: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    let inv_d = 1.0 / (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let inv_n = 1.0 / (n[1] * n[1] + n[2] * n[2] + n[0] * n[0]);
    std::array::from_fn(|i| -(n[i] * n[i] * d[i] * inv_n * 2.0 - d[i]) * inv_d)
}

impl CollisionMaterial {
    pub fn ricochets(&self) -> bool {
        self.flags & 3 == 1
    }

    pub fn read(b: &[u8]) -> Option<CollisionMaterial> {
        if b.len() != 104 {
            return None;
        }
        let w = |at: usize| be32(b, at).unwrap();
        Some(CollisionMaterial {
            flags: w(5),
            impact_sound: w(10),
            scrape_sound: w(14),
            break_sound: w(0x12),
            bullet_sound: w(0x16),
            impulse_percent: b[0x1A],
            min_impulse: b[0x1B],
            impulse_range: b[0x1C],
            max_scrape_speed: b[0x1D],
            weight: u16::from_be_bytes([b[0x1E], b[0x1F]]),
            bullet_hit_percent: b[0x20],
            footsteps: w(0x21),
            footstep_volume: b[0x25],
            footstep_impact: b[0x26],
            strength_curve: w(0x27),
            volume_curve: w(0x2B),
            slide_pitch_curve: w(0x2F),
            slide_volume_curve: w(0x33),
            sounds_37: std::array::from_fn(|i| w(0x37 + 4 * i)),
            climb_surface: b[0x63],
            door: w(0x64),
        })
    }

    pub fn strength(&self, impulse: f32) -> Option<f32> {
        let w = self.weight as f32 * 0.01;
        let over = impulse - self.min_impulse as f32 / w;
        let over = if over < 0.0 { 0.0 } else { over };
        let t = over / (self.impulse_range as f32 / w);
        let t = if t - 1.0 < 0.0 { t } else { 1.0 };
        if t <= 0.0 { None } else { Some(t) }
    }

    pub fn roll_sound(&self) -> u32 {
        self.sounds_37[6]
    }

    pub fn scrape_strength(&self, speed: f32) -> f32 {
        let t = speed / self.max_scrape_speed as f32;
        if t - 1.0 >= 0.0 { 1.0 } else { t }
    }
}

pub fn roll_strength(turning: f32) -> f32 {
    let t = turning / 3.0;
    if t - 1.0 >= 0.0 { 1.0 } else { t }
}

pub fn pitch_cents(ratio: f32) -> i32 {
    (((ratio as f64).log10() as f32 * f32::from_bits(0x4054_9A78)) * 1200.0) as i32
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResonantImpact {
    pub variation: usize,
    pub volume: f32,
    pub pitch: i32,
    pub impact: Option<u32>,
    pub resonance: Option<u32>,
}

pub fn resonant_impact(randomized: &SoundHeader, variations: &[(SoundHeader, Vec<(u32, u32)>)], rng: &mut crate::rng::AudioRand) -> Option<ResonantImpact> {
    if variations.is_empty() {
        return None;
    }
    let range = |rng: &mut crate::rng::AudioRand, v: u16| rng.int_in_range(-(v as i32), v as i32);
    let volume_draw = range(rng, randomized.volume_variance) as f32;
    let pitch = range(rng, randomized.pitch_variance) + randomized.pitch as i32;
    let variation = rng.int_in_range(0, variations.len() as i32 - 1) as usize;
    let (header, tracks) = &variations[variation];
    let own_volume_draw = range(rng, header.volume_variance) as f32;
    let pitch = range(rng, header.pitch_variance) + (header.pitch as i32 + pitch);
    let tag = name_hash("Impact");
    let (mut impact, mut resonance) = (None, None);
    for &(sound, word) in tracks {
        if word == tag {
            impact = Some(sound);
        } else {
            resonance = Some(sound);
        }
    }
    let volume = ((own_volume_draw + header.volume as f32) + volume_draw) + randomized.volume as f32;
    Some(ResonantImpact { variation, volume, pitch, impact, resonance })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechContext {
    pub hash: u32,
    pub repeat_time: i32,
    pub wait_time: i32,
    pub byte_0c: u8,
    pub nogender: u32,
    pub drunk: u32,
    pub next_time: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpeechContexts {
    pub contexts: Vec<SpeechContext>,
}

impl SpeechContexts {
    pub fn read(b: &[u8]) -> Option<SpeechContexts> {
        let n = u16::from_be_bytes([*b.get(10)?, *b.get(11)?]) as usize;
        if b.len() != 12 + 25 * n {
            return None;
        }
        let contexts = (0..n)
            .map(|i| {
                let at = 12 + 25 * i;
                let w = |o: usize| be32(b, at + o).unwrap();
                SpeechContext { hash: w(0), repeat_time: w(4) as i32, wait_time: w(8) as i32, byte_0c: b[at + 12], nogender: w(13), drunk: w(17), next_time: w(21) }
            })
            .collect();
        Some(SpeechContexts { contexts })
    }

    pub fn index(&self, hash: u32) -> Option<usize> {
        self.contexts.iter().position(|c| c.hash == hash)
    }

    fn get(&self, index: i32) -> Option<&SpeechContext> {
        usize::try_from(index).ok().and_then(|i| self.contexts.get(i))
    }

    pub fn next_time(&self, index: i32) -> Option<u32> {
        self.get(index).filter(|c| c.repeat_time >= 0).map(|c| c.next_time)
    }

    pub fn said(&mut self, index: i32, now: u32, rng: &mut crate::rng::AudioRand) {
        let Some(c) = usize::try_from(index).ok().and_then(|i| self.contexts.get_mut(i)) else { return };
        if c.repeat_time < 0 {
            return;
        }
        let most = (c.repeat_time as f32 * 1.25) as i32;
        c.next_time = (rng.int_in_range(c.repeat_time, most) as u32).wrapping_add(now);
    }

    pub fn wait(&self, index: i32, rng: &mut crate::rng::AudioRand) -> u32 {
        match self.get(index).filter(|c| c.wait_time >= 0) {
            Some(c) => {
                let t = (rng.float_in_range(0.8, 1.2) * c.wait_time as f32) as i64 as u32;
                t.min(50_000)
            }
            None => rng.int_in_range(6500, 10000) as u32,
        }
    }

    pub fn nogender(&self, index: i32) -> bool {
        self.get(index).is_some_and(|c| c.nogender != 0)
    }
    pub fn drunk(&self, index: i32) -> bool {
        self.get(index).is_some_and(|c| c.drunk != 0)
    }
    pub fn byte_0c(&self, index: i32) -> u8 {
        self.get(index).map_or(0, |c| c.byte_0c)
    }
}

