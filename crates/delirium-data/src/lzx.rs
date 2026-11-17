pub const MIRROR: usize = 261;
pub const MIRRORED: usize = 257;
pub const MAX_WINDOW: usize = 0x20_0000;
pub const OUT_STORE: usize = 0x8000;
pub const PARTIAL_STORE: usize = 38917;
pub const SECOND_STORE: usize = 0x8000;
pub const HEAD_BYTES: usize = 20;
pub const STATE_BYTES: usize = 12304;
pub const DEFAULT_WINDOW: usize = 0x2_0000;
pub const DEFAULT_PARTITION: u32 = 0x8_0000;
pub const GAME_PARAMS: LzxParams = LzxParams { flags: 0, window: 0x2_0000, partition: 0x8_0000 };
pub const TRANSLATED_BLOCKS: usize = 0x8000;

pub const EXTRA_BITS: [u8; 52] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15,
    16, 16, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
];
pub const POSITION_BASE: [u32; 51] = [
    0xFFFF_FFFE, 0xFFFF_FFFF, 0, 1, 2, 4, 6, 0xA, 0xE, 0x16, 0x1E, 0x2E, 0x3E, 0x5E, 0x7E, 0xBE, 0xFE, 0x17E, 0x1FE,
    0x2FE, 0x3FE, 0x5FE, 0x7FE, 0xBFE, 0xFFE, 0x17FE, 0x1FFE, 0x2FFE, 0x3FFE, 0x5FFE, 0x7FFE, 0xBFFE, 0xFFFE,
    0x1_7FFE, 0x1_FFFE, 0x2_FFFE, 0x3_FFFE, 0x5_FFFE, 0x7_FFFE, 0x9_FFFE, 0xB_FFFE, 0xD_FFFE, 0xF_FFFE, 0x11_FFFE,
    0x13_FFFE, 0x15_FFFE, 0x17_FFFE, 0x19_FFFE, 0x1B_FFFE, 0x1D_FFFE, 0x1F_FFFE,
];

pub const MAIN_LENGTHS: usize = 672;
pub const LENGTH_LENGTHS: usize = 252;
pub const LENGTH_SYMBOLS: usize = 249;
pub const MAIN_ROOT: usize = 1024;
pub const MAIN_TREE: usize = 2688;
pub const LENGTH_ROOT: usize = 256;
pub const LENGTH_TREE: usize = 996;
pub const ALIGNED_ROOT: usize = 128;
const MAIN_ROOT_BITS: u32 = 10;
const LENGTH_ROOT_BITS: u32 = 8;
const PRE_ROOT_BITS: u32 = 8;
const ALIGNED_ROOT_BITS: u32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LzxError {
    OutOfInput,
    BadBlockKind,
    BadTree,
    Overrun { by: usize, near: bool },
    BadPosition,
    BadWindow,
    NotLzx,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LzxParams {
    pub flags: u32,
    pub window: usize,
    pub partition: u32,
}

impl LzxParams {
    pub fn filled(from: Option<LzxParams>) -> LzxParams {
        let mut p = from.unwrap_or_default();
        if p.window == 0 {
            p.window = DEFAULT_WINDOW;
        }
        if p.partition == 0 {
            p.partition = DEFAULT_PARTITION;
        }
        p
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Verbatim,
    Aligned,
    Uncompressed,
}

impl BlockKind {
    pub fn from_bits(v: u32) -> Option<BlockKind> {
        match v {
            1 => Some(BlockKind::Verbatim),
            2 => Some(BlockKind::Aligned),
            3 => Some(BlockKind::Uncompressed),
            _ => None,
        }
    }
    pub fn bits(self) -> u32 {
        match self {
            BlockKind::Verbatim => 1,
            BlockKind::Aligned => 2,
            BlockKind::Uncompressed => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockState {
    NewBlock,
    InBlock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Block,
    Input(usize),
    Partial(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    None,
    Caller(usize),
    Store,
}

#[derive(Clone, Debug)]
pub struct LzxDecoder {
    pub window: Vec<u8>,
    pub window_size: usize,
    pub repeats: [usize; 3],
    pub main_root: Vec<u16>,
    pub length_root: Vec<u16>,
    pub main_lengths: Vec<u8>,
    pub length_lengths: Vec<u8>,
    pub aligned_root: [u8; ALIGNED_ROOT],
    pub aligned_lengths: [u8; 8],
    pub main_tree: Vec<u16>,
    pub length_tree: Vec<u16>,
    pub input: Vec<u8>,
    pub in_pos: usize,
    pub in_end: usize,
    pub input_origin: Origin,
    pub target: Target,
    pub total_out: u64,
    pub last_main_lengths: Vec<u8>,
    pub last_length_lengths: Vec<u8>,
    pub bits: u32,
    pub bit_count: i32,
    pub slots: usize,
    pub first_block: bool,
    pub out_of_input: bool,
    pub position: usize,
    pub file_size: u32,
    pub translated: usize,
    pub blocks: usize,
    pub block_size: usize,
    pub block_left: usize,
    pub block_kind: Option<BlockKind>,
    pub block_state: BlockState,
    pub spare: [u32; 3],
    pub extra_bits: [u8; 52],
    pub position_base: [u32; 51],
    pub uncached: bool,
    pub out_store: Vec<u8>,
    pub out_read: usize,
    pub out_left: usize,
    pub partial: Vec<u8>,
    pub partial_fill: usize,
    pub counts: [u32; 4],
    pub second_store: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct Cursor {
    pos: usize,
    bits: u32,
    count: i32,
}

fn top_bits(bits: u32, n: u32) -> u32 {
    bits.checked_shr(32 - n.min(32)).unwrap_or(0)
}

fn drop_top(bits: u32, n: u32) -> u32 {
    bits.checked_shl(n).unwrap_or(0)
}

#[derive(Clone, Copy)]
struct Tables<'a> {
    root: &'a [u16],
    tree: &'a [u16],
    root_bits: u32,
}

impl Tables<'_> {
    fn symbol(&self, bits: u32) -> Result<usize, LzxError> {
        let mut entry = *self.root.get(top_bits(bits, self.root_bits) as usize).ok_or(LzxError::BadTree)? as i16;
        let mut bit = 31 - self.root_bits;
        while entry < 0 {
            let node = (-(entry as i32)) as usize * 2 + ((bits >> bit) & 1) as usize;
            entry = *self.tree.get(node).ok_or(LzxError::BadTree)? as i16;
            bit = bit.checked_sub(1).ok_or(LzxError::BadTree)?;
        }
        Ok(entry as usize)
    }
}

pub fn make_table(lengths: &[u8], bits: u32, root: &mut [u16], tree: &mut [u16]) -> Result<(), LzxError> {
    let b = bits as usize;
    let mut count = [0u32; 17];
    let mut start = [0u32; 18];
    for &l in lengths {
        *count.get_mut(l as usize).ok_or(LzxError::BadTree)? += 1;
    }
    for l in 1..=16 {
        start[l + 1] = (count[l] << (16 - l)) + start[l];
    }
    match start[17] {
        0x1_0000 => {}
        0 => {
            let entries = (1 << b).min(root.len());
            root[..entries].fill(0);
            return Ok(());
        }
        _ => return Err(LzxError::BadTree),
    }
    let rest = 16 - b;
    for l in 1..=b {
        count[l] = 1 << (b - l);
        start[l] >>= rest;
    }
    for l in b + 1..=16 {
        count[l] = 1 << (16 - l);
    }
    let used = (start[b + 1] >> rest) as usize;
    if used != 0x1_0000 {
        let full = (1usize << b).min(root.len());
        if used < full {
            root[used..full].fill(0);
        }
    }
    let mut next = lengths.len();
    for (sym, &l) in lengths.iter().enumerate() {
        let l = l as usize;
        if l == 0 {
            continue;
        }
        let first = start[l];
        let end = count[l] + first;
        start[l] = end;
        if l <= b {
            if end > 1 << b {
                return Err(LzxError::BadTree);
            }
            root.get_mut(first as usize..end as usize).ok_or(LzxError::BadTree)?.fill(sym as u16);
            continue;
        }
        let mut code = first << b;
        let mut at = Entry::Root((first >> rest) as usize);
        for _ in 0..l - b {
            if at.get(root, tree)? == 0 {
                tree.get_mut(2 * next..2 * next + 2).ok_or(LzxError::BadTree)?.fill(0);
                at.set(root, tree, (next as u16).wrapping_neg())?;
                next += 1;
            }
            let node = (at.get(root, tree)? as i16 as i32).unsigned_abs() as usize;
            at = Entry::Tree(node * 2 + (code & 0x8000 != 0) as usize);
            code <<= 1;
        }
        at.set(root, tree, sym as u16)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Entry {
    Root(usize),
    Tree(usize),
}

impl Entry {
    fn get(self, root: &[u16], tree: &[u16]) -> Result<u16, LzxError> {
        match self {
            Entry::Root(i) => root.get(i),
            Entry::Tree(i) => tree.get(i),
        }
        .copied()
        .ok_or(LzxError::BadTree)
    }
    fn set(self, root: &mut [u16], tree: &mut [u16], v: u16) -> Result<(), LzxError> {
        let e = match self {
            Entry::Root(i) => root.get_mut(i),
            Entry::Tree(i) => tree.get_mut(i),
        };
        *e.ok_or(LzxError::BadTree)? = v;
        Ok(())
    }
}

pub fn make_aligned_table(lengths: &[u8; 8], table: &mut [u8; ALIGNED_ROOT]) -> Result<(), LzxError> {
    let mut count = [0u16; 17];
    let mut start = [0u16; 18];
    for &l in lengths {
        *count.get_mut(l as usize).ok_or(LzxError::BadTree)? += 1;
    }
    for l in 1..=16 {
        start[l + 1] = (count[l] << (16 - l)).wrapping_add(start[l]);
    }
    if start[17] != 0 {
        return Err(LzxError::BadTree);
    }
    let rest = 16 - ALIGNED_ROOT_BITS as usize;
    for l in 1..=7 {
        count[l] = 1 << (7 - l);
        start[l] >>= rest;
    }
    for l in 8..=16 {
        count[l] = 1 << (16 - l);
    }
    table.fill(0);
    for (s, &l) in lengths.iter().enumerate() {
        if l == 0 {
            continue;
        }
        let first = start[l as usize] as usize;
        let end = count[l as usize] as usize + first;
        if end > ALIGNED_ROOT {
            return Err(LzxError::BadTree);
        }
        table[first..end].fill(s as u8);
        start[l as usize] = end as u16;
    }
    Ok(())
}

impl LzxDecoder {
    pub fn empty() -> Self {
        LzxDecoder {
            window: Vec::new(),
            window_size: 0,
            repeats: [0; 3],
            main_root: vec![0; MAIN_ROOT],
            length_root: vec![0; LENGTH_ROOT],
            main_lengths: vec![0; MAIN_LENGTHS],
            length_lengths: vec![0; LENGTH_LENGTHS],
            aligned_root: [0; ALIGNED_ROOT],
            aligned_lengths: [0; 8],
            main_tree: vec![0; MAIN_TREE],
            length_tree: vec![0; LENGTH_TREE],
            input: Vec::new(),
            in_pos: 0,
            in_end: 0,
            input_origin: Origin::Block,
            target: Target::None,
            total_out: 0,
            last_main_lengths: vec![0; MAIN_LENGTHS],
            last_length_lengths: vec![0; LENGTH_LENGTHS],
            bits: 0,
            bit_count: 0,
            slots: 0,
            first_block: false,
            out_of_input: false,
            position: 0,
            file_size: 0,
            translated: 0,
            blocks: 0,
            block_size: 0,
            block_left: 0,
            block_kind: None,
            block_state: BlockState::NewBlock,
            spare: [0; 3],
            extra_bits: [0; 52],
            position_base: [0; 51],
            uncached: false,
            out_store: Vec::new(),
            out_read: 0,
            out_left: 0,
            partial: Vec::new(),
            partial_fill: 0,
            counts: [0; 4],
            second_store: Vec::new(),
        }
    }

    fn wrapped(&self, at: usize) -> usize {
        at & (self.window_size - 1)
    }

    fn input_byte(&self, at: usize) -> u8 {
        self.input.get(at).copied().unwrap_or(0)
    }

    fn input_half(&self, at: usize) -> u32 {
        u16::from_le_bytes([self.input_byte(at), self.input_byte(at + 1)]) as u32
    }

    pub fn write_constants(&mut self) {
        self.extra_bits = EXTRA_BITS;
        self.position_base = POSITION_BASE;
    }

    pub fn start_bits(&mut self) {
        if self.block_kind == Some(BlockKind::Uncompressed) || self.in_pos + 4 > self.in_end {
            return;
        }
        self.bits = self.input_half(self.in_pos) << 16 | self.input_half(self.in_pos + 2);
        self.in_pos += 4;
        self.bit_count = 16;
    }

    pub fn drop_bits(&mut self, n: u32) {
        self.bits = drop_top(self.bits, n);
        self.bit_count -= n as i32;
        for _ in 0..2 {
            if self.bit_count > 0 {
                return;
            }
            if self.in_pos >= self.in_end {
                self.out_of_input = true;
                return;
            }
            self.bits |= drop_top(self.input_half(self.in_pos), (-self.bit_count) as u32);
            self.in_pos += 2;
            self.bit_count += 16;
        }
    }

    pub fn read_bits(&mut self, n: u32) -> u32 {
        let v = top_bits(self.bits, n);
        self.drop_bits(n);
        v
    }

    fn cursor(&self) -> Cursor {
        Cursor { pos: self.in_pos, bits: self.bits, count: self.bit_count }
    }

    fn keep(&mut self, c: Cursor) {
        self.bit_count = c.count;
        self.bits = c.bits;
        self.in_pos = c.pos;
    }

    fn refill(&self, c: &mut Cursor) {
        c.bits |= drop_top(self.input_half(c.pos), (-c.count) as u32);
        c.pos += 2;
        c.count += 16;
    }

    fn drop1(&self, c: &mut Cursor, n: u32) {
        c.count -= n as i32;
        c.bits = drop_top(c.bits, n);
        if c.count <= 0 {
            self.refill(c);
        }
    }

    fn take(&self, c: &mut Cursor, n: u32) -> u32 {
        let v = top_bits(c.bits, n);
        c.count -= n as i32;
        c.bits = drop_top(c.bits, n);
        for _ in 0..2 {
            if c.count > 0 {
                break;
            }
            self.refill(c);
        }
        v
    }

    fn main_tables(&self) -> Tables<'_> {
        Tables { root: &self.main_root, tree: &self.main_tree, root_bits: MAIN_ROOT_BITS }
    }

    fn length_tables(&self) -> Tables<'_> {
        Tables { root: &self.length_root, tree: &self.length_tree, root_bits: LENGTH_ROOT_BITS }
    }

    fn main_count(&self) -> usize {
        (self.slots + 32) * 8
    }

    pub fn clear_lengths(&mut self) {
        let n = self.main_count();
        self.main_lengths[..n].fill(0);
        self.last_main_lengths[..n].fill(0);
        self.length_lengths[..LENGTH_SYMBOLS].fill(0);
        self.last_length_lengths[..LENGTH_SYMBOLS].fill(0);
    }

    pub fn reset_state(&mut self) {
        self.position = 0;
        self.repeats = [1; 3];
        self.total_out = 0;
        self.block_state = BlockState::NewBlock;
        self.block_left = 0;
        self.block_kind = None;
        self.first_block = true;
        self.file_size = 0;
        self.out_of_input = false;
    }

    pub fn clear_translation(&mut self) {
        self.translated = 0;
    }

    pub fn reset(&mut self) {
        self.clear_lengths();
        self.reset_state();
        self.clear_translation();
        self.blocks = 0;
    }

    pub fn undo_e8(&mut self, run: &mut [u8]) {
        let n = run.len();
        let start = self.translated;
        self.translated += n;
        if n <= 6 {
            return;
        }
        let tail = n - 6;
        let mut keep = [0u8; 6];
        keep.copy_from_slice(&run[tail..]);
        run[tail..].fill(0xE8);
        let stop = (start + n).saturating_sub(10);
        let mut at = 0;
        while let Some(far) = run[at..].iter().position(|&b| b == 0xE8) {
            let place = start + at + far;
            if place >= stop {
                break;
            }
            let operand_at = at + far + 1;
            let operand = u32::from_le_bytes(run[operand_at..operand_at + 4].try_into().unwrap());
            if let Some(relative) = e8_relative(operand, place as u32, self.file_size) {
                run[operand_at..operand_at + 4].copy_from_slice(&relative.to_le_bytes());
            }
            at = operand_at + 4;
        }
        run[tail..].copy_from_slice(&keep);
    }

    pub fn read_raw_repeats(&mut self) -> Result<(), LzxError> {
        let p = self.in_pos - 2;
        self.in_pos = p;
        if p + 4 >= self.in_end {
            return Err(LzxError::OutOfInput);
        }
        for k in 0..3 {
            let at = p + 4 * k;
            self.repeats[k] = (self.input_half(at) | self.input_half(at + 2) << 16) as usize;
        }
        self.in_pos = p + 12;
        Ok(())
    }

    fn win(&self, at: usize) -> u8 {
        self.window.get(at).copied().unwrap_or(0)
    }
    fn set_win(&mut self, at: usize, v: u8) {
        if let Some(e) = self.window.get_mut(at) {
            *e = v;
        }
    }

    pub fn copy_raw(&mut self, pos: usize, n: usize) -> Result<(), LzxError> {
        let from = self.in_pos;
        if from + n > self.in_end {
            return Err(LzxError::OutOfInput);
        }
        for k in 0..n {
            let v = self.input_byte(from + k);
            self.set_win(pos + k, v);
        }
        self.in_pos = from + n;
        let end = pos + n;
        for at in pos..end.min(MIRRORED) {
            let v = self.win(at);
            self.set_win(self.window_size + at, v);
        }
        self.position = self.wrapped(end);
        Ok(())
    }

    fn verbatim_offset(&mut self, c: &mut Cursor, slot: usize) -> usize {
        if slot <= 2 {
            return self.repeat(slot);
        }
        let extra = self.take(c, self.extra_bits[slot] as u32);
        let off = (self.position_base[slot] + extra) as usize;
        self.repeats = [off, self.repeats[0], self.repeats[1]];
        off
    }

    fn repeat(&mut self, k: usize) -> usize {
        let off = self.repeats[k];
        self.repeats[k] = self.repeats[0];
        self.repeats[0] = off;
        off
    }

    fn aligned_offset(&mut self, c: &mut Cursor, slot: usize) -> Result<usize, LzxError> {
        if slot <= 2 {
            return Ok(self.repeat(slot));
        }
        let base = self.position_base[slot];
        let n = self.extra_bits[slot] as u32;
        let extra = if n >= 3 {
            let high = self.take_one_refill(c, n - 3);
            let s = self.aligned_root[top_bits(c.bits, ALIGNED_ROOT_BITS) as usize];
            self.drop1(c, *self.aligned_lengths.get(s as usize).ok_or(LzxError::BadTree)? as u32);
            (high << 3) + s as u32
        } else {
            let v = top_bits(c.bits, n);
            if n != 0 {
                self.drop1(c, n);
            }
            v
        };
        let off = (base + extra) as usize;
        self.repeats = [off, self.repeats[0], self.repeats[1]];
        Ok(off)
    }

    fn take_one_refill(&self, c: &mut Cursor, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = top_bits(c.bits, n);
        self.drop1(c, n);
        v
    }

    fn decode_symbols(&mut self, pos: usize, n: usize, near: bool, aligned: bool) -> Result<usize, LzxError> {
        let stop = pos + n;
        let mut c = self.cursor();
        let mut at = pos;
        while at < stop {
            let s = self.main_tables().symbol(c.bits)?;
            if c.pos >= self.in_end {
                return Err(LzxError::OutOfInput);
            }
            let l = *self.main_lengths.get(s).ok_or(LzxError::BadTree)? as u32;
            self.drop1(&mut c, l);
            if s < 256 {
                self.set_win(at, s as u8);
                if near {
                    self.set_win(self.window_size + at, s as u8);
                }
                at += 1;
                continue;
            }
            let m = s - 256;
            let mut len = m & 7;
            if len == 7 {
                let t = self.length_tables().symbol(c.bits)?;
                let l = *self.length_lengths.get(t).ok_or(LzxError::BadTree)? as u32;
                self.drop1(&mut c, l);
                len = t + 7;
            }
            let slot = m >> 3;
            if slot >= self.slots {
                return Err(LzxError::BadTree);
            }
            let off = if aligned { self.aligned_offset(&mut c, slot)? } else { self.verbatim_offset(&mut c, slot) };
            let len = len + 2;
            if near {
                for k in 0..len {
                    let v = self.win(self.wrapped((at + k).wrapping_sub(off)));
                    self.set_win(at + k, v);
                    if at + k < MIRRORED {
                        self.set_win(self.window_size + at + k, v);
                    }
                }
            } else {
                let from = self.wrapped(at.wrapping_sub(off));
                for k in 0..len {
                    let v = self.win(from + k);
                    self.set_win(at + k, v);
                }
            }
            at += len;
        }
        self.keep(c);
        if near {
            return Ok(at);
        }
        self.position = self.wrapped(at);
        Ok(at - stop)
    }

    pub fn verbatim_near(&mut self, pos: usize, n: usize) -> Result<usize, LzxError> {
        self.decode_symbols(pos, n, true, false)
    }
    pub fn verbatim_past(&mut self, pos: usize, n: usize) -> Result<usize, LzxError> {
        self.decode_symbols(pos, n, false, false)
    }
    pub fn aligned_near(&mut self, pos: usize, n: usize) -> Result<usize, LzxError> {
        self.decode_symbols(pos, n, true, true)
    }
    pub fn aligned_past(&mut self, pos: usize, n: usize) -> Result<usize, LzxError> {
        self.decode_symbols(pos, n, false, true)
    }

    fn decode_compressed(&mut self, pos: usize, n: usize, aligned: bool) -> Result<(), LzxError> {
        let (mut pos, mut n) = (pos, n);
        if pos < MIRRORED {
            let part = (MIRRORED - pos).min(n);
            let end = if aligned { self.aligned_near(pos, part)? } else { self.verbatim_near(pos, part)? };
            self.position = end;
            if end >= pos + n {
                return match end - (pos + n) {
                    0 => Ok(()),
                    by => Err(LzxError::Overrun { by, near: true }),
                };
            }
            n = pos + n - end;
            pos = end;
        }
        match if aligned { self.aligned_past(pos, n)? } else { self.verbatim_past(pos, n)? } {
            0 => Ok(()),
            by => Err(LzxError::Overrun { by, near: false }),
        }
    }

    pub fn decode_verbatim(&mut self, pos: usize, n: usize) -> Result<(), LzxError> {
        self.decode_compressed(pos, n, false)
    }
    pub fn decode_aligned(&mut self, pos: usize, n: usize) -> Result<(), LzxError> {
        self.decode_compressed(pos, n, true)
    }

    fn refill_checked(&self, c: &mut Cursor) -> Result<(), LzxError> {
        for _ in 0..2 {
            if c.count > 0 {
                return Ok(());
            }
            if c.pos >= self.in_end {
                return Err(LzxError::OutOfInput);
            }
            self.refill(c);
        }
        Ok(())
    }

    pub fn read_lengths(&mut self, n: usize, old: &[u8], out: &mut [u8]) -> Result<(), LzxError> {
        let mut pre = [0u8; 20];
        for p in pre.iter_mut() {
            *p = self.read_bits(4) as u8;
        }
        if self.out_of_input {
            return Err(LzxError::OutOfInput);
        }
        let mut root = [0u16; 1 << PRE_ROOT_BITS];
        let mut tree = [0u16; 96];
        make_table(&pre, PRE_ROOT_BITS, &mut root, &mut tree)?;
        let pre_tree = Tables { root: &root, tree: &tree, root_bits: PRE_ROOT_BITS };
        let mut c = self.cursor();
        let delta = |i: usize, s: usize| -> Result<u8, LzxError> {
            if s > 16 {
                return Err(LzxError::BadTree);
            }
            Ok(((old.get(i).copied().unwrap_or(0) as usize + 17 - s) % 17) as u8)
        };
        let mut out_of_input = self.out_of_input;
        let mut i = 0;
        while i < n {
            let s = pre_tree.symbol(c.bits)?;
            let l = pre[s] as u32;
            c.count -= l as i32;
            c.bits = drop_top(c.bits, l);
            out_of_input |= self.refill_checked(&mut c).is_err();
            if out_of_input {
                break;
            }
            match s {
                17 | 18 => {
                    let (w, least) = if s == 17 { (4, 4) } else { (5, 20) };
                    let v = top_bits(c.bits, w);
                    c.count -= w as i32;
                    c.bits = drop_top(c.bits, w);
                    out_of_input |= self.refill_checked(&mut c).is_err();
                    let k = (v as usize + least).min(n - i);
                    out[i..i + k].fill(0);
                    i += k;
                }
                19 => {
                    let v = top_bits(c.bits, 1);
                    c.count -= 1;
                    c.bits = drop_top(c.bits, 1);
                    out_of_input |= self.refill_checked(&mut c).is_err();
                    let k = (v as usize + 4).min(n - i);
                    let s2 = pre_tree.symbol(c.bits)?;
                    let l = pre[s2] as u32;
                    c.count -= l as i32;
                    c.bits = drop_top(c.bits, l);
                    out_of_input |= self.refill_checked(&mut c).is_err();
                    let v = delta(i, s2)?;
                    out[i..i + k].fill(v);
                    i += k;
                }
                _ => {
                    out[i] = delta(i, s)?;
                    i += 1;
                }
            }
        }
        let result = if out_of_input { Err(LzxError::OutOfInput) } else { Ok(()) };
        self.in_pos = c.pos;
        self.out_of_input = out_of_input;
        self.bits = c.bits;
        self.bit_count = c.count;
        result
    }

    pub fn read_trees(&mut self) -> Result<(), LzxError> {
        let old = std::mem::take(&mut self.last_main_lengths);
        let mut lengths = std::mem::take(&mut self.main_lengths);
        let mut read = self.read_lengths(256, &old[..256], &mut lengths[..256]);
        if read.is_ok() {
            let n = self.slots * 8;
            read = self.read_lengths(n, &old[256..256 + n], &mut lengths[256..256 + n]);
        }
        self.last_main_lengths = old;
        self.main_lengths = lengths;
        read?;
        let n = self.main_count();
        make_table(&self.main_lengths[..n], MAIN_ROOT_BITS, &mut self.main_root, &mut self.main_tree)?;
        let old = std::mem::take(&mut self.last_length_lengths);
        let mut lengths = std::mem::take(&mut self.length_lengths);
        let read = self.read_lengths(LENGTH_SYMBOLS, &old[..LENGTH_SYMBOLS], &mut lengths[..LENGTH_SYMBOLS]);
        self.last_length_lengths = old;
        self.length_lengths = lengths;
        read?;
        make_table(&self.length_lengths[..LENGTH_SYMBOLS], LENGTH_ROOT_BITS, &mut self.length_root, &mut self.length_tree)
    }

    pub fn read_aligned(&mut self) -> Result<(), LzxError> {
        for k in 0..8 {
            self.aligned_lengths[k] = self.read_bits(3) as u8;
        }
        if self.out_of_input {
            return Err(LzxError::OutOfInput);
        }
        make_aligned_table(&self.aligned_lengths, &mut self.aligned_root)
    }

    fn put(&mut self, out: &mut [u8], t: Target, data: &[u8]) {
        let dest: &mut [u8] = match t {
            Target::None => return,
            Target::Caller(at) => out.get_mut(at..).unwrap_or(&mut []),
            Target::Store => &mut self.out_store,
        };
        let n = data.len().min(dest.len());
        dest[..n].copy_from_slice(&data[..n]);
    }

    fn undo_e8_at(&mut self, out: &mut [u8], t: Target, n: usize) {
        match t {
            Target::None => {}
            Target::Caller(at) => {
                let end = (at + n).min(out.len());
                if let Some(run) = out.get_mut(at..end) {
                    self.undo_e8(run);
                }
            }
            Target::Store => {
                let mut store = std::mem::take(&mut self.out_store);
                let end = n.min(store.len());
                self.undo_e8(&mut store[..end]);
                self.out_store = store;
            }
        }
    }

    pub fn hand_out(&mut self, out: &mut [u8], run: &[u8]) {
        let target = self.target;
        if target == Target::None {
            return;
        }
        let translate = self.file_size != 0 && self.blocks < TRANSLATED_BLOCKS;
        if self.uncached && !translate {
            self.put(out, target, run);
            return;
        }
        let dest = if self.uncached { Target::Store } else { target };
        self.put(out, dest, run);
        if !translate {
            return;
        }
        self.undo_e8_at(out, dest, run.len());
        if self.uncached {
            let store = self.out_store[..run.len().min(self.out_store.len())].to_vec();
            self.put(out, target, &store);
        }
    }

    fn read_block_header(&mut self) -> Result<(), LzxError> {
        if self.first_block {
            self.first_block = false;
            self.file_size = if self.read_bits(1) != 0 {
                let high = self.read_bits(16);
                let low = self.read_bits(16);
                high << 16 | low
            } else {
                0
            };
        }
        if self.block_kind == Some(BlockKind::Uncompressed) {
            self.block_kind = None;
            self.start_bits();
        }
        let kind = self.read_bits(3);
        let size = (self.read_bits(8) << 16 | self.read_bits(8) << 8 | self.read_bits(8)) as usize;
        self.block_size = size;
        self.block_left = size;
        self.block_kind = BlockKind::from_bits(kind);
        match self.block_kind.ok_or(LzxError::BadBlockKind)? {
            BlockKind::Aligned => {
                self.read_aligned()?;
                self.read_trees_for_block()?;
            }
            BlockKind::Verbatim => self.read_trees_for_block()?,
            BlockKind::Uncompressed => self.read_raw_repeats()?,
        }
        self.block_state = BlockState::InBlock;
        Ok(())
    }

    fn read_trees_for_block(&mut self) -> Result<(), LzxError> {
        let n = self.main_count();
        self.last_main_lengths[..n].copy_from_slice(&self.main_lengths[..n]);
        self.last_length_lengths[..LENGTH_SYMBOLS].copy_from_slice(&self.length_lengths[..LENGTH_SYMBOLS]);
        self.read_trees()
    }

    pub fn decode(&mut self, out: &mut [u8], want: usize) -> Result<usize, LzxError> {
        let mut left = want;
        let mut made = 0;
        while left > 0 {
            if self.block_state == BlockState::NewBlock {
                self.read_block_header()?;
            }
            while self.block_left > 0 && left > 0 {
                let n = self.block_left.min(left);
                let pos = self.position;
                match self.block_kind.ok_or(LzxError::BadBlockKind)? {
                    BlockKind::Aligned => self.decode_aligned(pos, n)?,
                    BlockKind::Verbatim => self.decode_verbatim(pos, n)?,
                    BlockKind::Uncompressed => self.copy_raw(pos, n)?,
                }
                self.block_left -= n;
                left -= n;
                made += n;
            }
            if self.block_left == 0 {
                self.block_state = BlockState::NewBlock;
            }
            if left == 0 {
                self.start_bits();
            }
        }
        let end = if self.position == 0 { self.window_size } else { self.position };
        let from = end.checked_sub(made).ok_or(LzxError::BadPosition)?;
        let run = self.window[from..from + made].to_vec();
        self.hand_out(out, &run);
        Ok(made)
    }

    pub fn decode_block(&mut self, out: &mut [u8], size: usize, block: &[u8], n: usize, target: Target, uncached: bool) -> Result<usize, LzxError> {
        self.input = block.to_vec();
        self.input_origin = Origin::Block;
        self.in_pos = 0;
        self.in_end = n + 4;
        self.target = target;
        self.uncached = uncached;
        self.start_bits();
        let made = self.decode(out, size);
        self.blocks += 1;
        let made = made?;
        self.total_out += made as u64;
        Ok(made)
    }

    fn source_byte(&self, input: &[u8], s: Source, k: usize) -> u8 {
        match s {
            Source::Input(at) => input.get(at + k).copied().unwrap_or(0),
            Source::Partial(at) => self.partial.get(at + k).copied().unwrap_or(0),
        }
    }

    fn source_half(&self, input: &[u8], s: Source, k: usize) -> usize {
        (self.source_byte(input, s, k) as usize) << 8 | self.source_byte(input, s, k + 1) as usize
    }

    fn source_bytes(&self, input: &[u8], s: Source, k: usize, n: usize) -> Vec<u8> {
        (0..n).map(|j| self.source_byte(input, s, k + j)).collect()
    }

    fn source_tail(&self, input: &[u8], s: Source, k: usize) -> Vec<u8> {
        let (buf, at) = match s {
            Source::Input(at) => (input, at),
            Source::Partial(at) => (self.partial.as_slice(), at),
        };
        buf.get(at + k..).map_or_else(Vec::new, |b| b.to_vec())
    }

    fn keep_partial(&mut self, at: usize, data: &[u8]) {
        for (k, &b) in data.iter().enumerate() {
            if let Some(e) = self.partial.get_mut(at + k) {
                *e = b;
            }
        }
    }

    pub fn decompress(&mut self, out: &mut [u8], room: usize, input: &[u8], input_len: usize, uncached: &mut dyn FnMut(Target) -> bool) -> Result<(usize, usize), LzxError> {
        let mut used = 0;
        let mut made = 0;
        if self.out_left != 0 {
            made = self.out_left.min(room);
            let from = self.out_read;
            let run: Vec<u8> = (0..made).map(|k| self.out_store.get(from + k).copied().unwrap_or(0)).collect();
            let n = run.len().min(out.len());
            out[..n].copy_from_slice(&run[..n]);
            self.out_left -= made;
            self.out_read += made;
        }
        if input_len == 0 && self.partial_fill <= 1 {
            return Ok((made, used));
        }
        let fill = self.partial_fill;
        let mut rest = input_len;
        let mut src = Source::Input(0);
        if fill != 0 {
            src = Source::Partial(0);
            let (head, size_at) = if self.partial.first().copied() == Some(255) { (5, 3) } else { (2, 0) };
            if fill < head {
                let n = (head - fill).min(input_len);
                let data = self.source_bytes(input, Source::Input(0), 0, n);
                self.keep_partial(fill, &data);
                self.partial_fill = fill + n;
                used = n;
                rest = input_len - n;
            }
            if rest != 0 {
                let total = self.source_half(input, src, size_at) + head;
                let have = self.partial_fill;
                let n = total.saturating_sub(have).min(input_len - used);
                let data = self.source_bytes(input, Source::Input(used), 0, n);
                self.keep_partial(have, &data);
                self.partial_fill = have + n;
                used += n;
                rest = if self.partial_fill >= total { total } else { 0 };
            }
        }
        if self.out_left != 0 {
            return Ok((made, used));
        }
        let mut block = 0;
        while rest != 0 {
            let (head, total) = if self.source_byte(input, src, 0) == 255 {
                if rest < 5 {
                    (5, 5)
                } else {
                    block = self.source_half(input, src, 1);
                    (5, self.source_half(input, src, 3) + 5)
                }
            } else if rest < 2 {
                (2, 2)
            } else {
                block = 0x8000;
                (2, self.source_half(input, src, 0) + 2)
            };
            if rest < total {
                let data = self.source_bytes(input, src, 0, rest);
                self.keep_partial(0, &data);
                used += rest;
                self.partial_fill = rest;
                rest = 0;
            } else {
                let space = room - made;
                let target = if space < block { Target::Store } else { Target::Caller(made) };
                let bytes = self.source_tail(input, src, head);
                let is_uncached = uncached(target);
                let got = self.decode_block(out, block, &bytes, total - head, target, is_uncached)?;
                self.input_origin = match src {
                    Source::Input(at) => Origin::Input(at + head),
                    Source::Partial(at) => Origin::Partial(at + head),
                };
                if target == Target::Store {
                    let run = self.out_store[..space.min(self.out_store.len())].to_vec();
                    self.put(out, Target::Caller(made), &run);
                    made += space;
                    self.out_read = space;
                    self.out_left = got - space;
                } else {
                    made += got;
                }
                match src {
                    Source::Partial(_) => {
                        src = Source::Input(used);
                        self.partial_fill = 0;
                    }
                    Source::Input(at) => {
                        src = Source::Input(at + total);
                        used += total;
                    }
                }
                rest = input_len - used;
            }
            if self.out_left != 0 {
                break;
            }
        }
        Ok((made, used))
    }

    pub fn context_bytes(window: usize, head: usize, flags: ContextFlags) -> usize {
        let store = if flags.stream {
            PARTIAL_STORE
        } else if flags.second_store {
            SECOND_STORE
        } else {
            0
        };
        window + MIRROR + head + STATE_BYTES + store + OUT_STORE
    }

    pub fn lay_out(&mut self, window: usize, flags: ContextFlags) -> Result<(), LzxError> {
        if window > MAX_WINDOW {
            return Err(LzxError::BadWindow);
        }
        self.write_constants();
        self.window_size = window;
        self.spare = [0; 3];
        self.slots = 4;
        let mut covered = 4;
        loop {
            covered += 1usize << self.extra_bits[self.slots];
            self.slots += 1;
            if covered >= window {
                break;
            }
        }
        self.window = vec![0; window + MIRROR];
        self.out_read = 0;
        self.out_left = 0;
        self.partial_fill = 0;
        self.counts = [0; 4];
        self.out_store.resize(OUT_STORE, 0);
        if flags.stream {
            self.partial.resize(PARTIAL_STORE, 0);
        } else {
            self.partial = Vec::new();
            if flags.second_store {
                self.second_store.resize(SECOND_STORE, 0);
            }
        }
        Ok(())
    }
}

fn e8_relative(operand: u32, place: u32, size: u32) -> Option<u32> {
    if operand < size {
        Some(operand.wrapping_sub(place))
    } else if operand.wrapping_neg() <= place {
        Some(operand.wrapping_add(size))
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Input(usize),
    Partial(usize),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextFlags {
    pub stream: bool,
    pub second_store: bool,
    pub made_here: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Default,
    Lzx,
    Other,
}

#[derive(Clone, Debug)]
pub struct LzxContext {
    pub is_lzx: bool,
    pub flags: ContextFlags,
    pub param_flags: u32,
    pub decoder: LzxDecoder,
}

impl LzxContext {
    pub fn size(params: Option<LzxParams>, flags: ContextFlags) -> usize {
        let p = LzxParams::filled(params);
        LzxDecoder::context_bytes(p.window, HEAD_BYTES, flags)
    }

    pub fn set_up(&mut self, params: Option<LzxParams>, flags: ContextFlags) -> Result<(), LzxError> {
        let p = LzxParams::filled(params);
        self.decoder.lay_out(p.window, flags)?;
        self.param_flags = p.flags;
        if flags.stream {
            self.reset_inner();
        }
        Ok(())
    }

    pub fn blank() -> LzxContext {
        LzxContext { is_lzx: false, flags: ContextFlags::default(), param_flags: 0, decoder: LzxDecoder::empty() }
    }

    pub fn init(&mut self, params: Option<LzxParams>, flags: ContextFlags) -> Result<(), LzxError> {
        self.flags = flags;
        self.is_lzx = true;
        self.set_up(params, flags)
    }

    pub fn new(params: Option<LzxParams>, flags: ContextFlags) -> Result<LzxContext, LzxError> {
        let mut c = LzxContext::blank();
        c.init(params, flags)?;
        Ok(c)
    }

    pub fn reset_inner(&mut self) {
        let d = &mut self.decoder;
        d.reset();
        if !self.flags.stream {
            if !self.flags.second_store {
                return;
            }
            d.partial = Vec::new();
            d.counts = [0; 4];
        }
        d.partial_fill = 0;
        d.out_left = 0;
        d.out_read = 0;
    }

    pub fn reset_checked(&mut self) {
        self.reset_inner();
    }

    pub fn decompress_checked(&mut self, out: &mut [u8], room: usize, input: &[u8], input_len: usize, uncached: &mut dyn FnMut(Target) -> bool) -> Result<(usize, usize), LzxError> {
        self.decoder.decompress(out, room, input, input_len, uncached)
    }

    pub fn create(codec: Codec, params: Option<LzxParams>, flags: ContextFlags) -> Result<Box<LzxContext>, LzxError> {
        LzxContext::create_from(LzxContext::blank(), codec, params, flags)
    }

    pub fn create_from(mut c: LzxContext, codec: Codec, params: Option<LzxParams>, flags: ContextFlags) -> Result<Box<LzxContext>, LzxError> {
        if codec != Codec::Other {
            c.init(params, flags)?;
        }
        c.flags.made_here = true;
        Ok(Box::new(c))
    }

    pub fn reset(&mut self) -> Result<(), LzxError> {
        if !self.is_lzx {
            return Err(LzxError::NotLzx);
        }
        self.reset_checked();
        Ok(())
    }

    pub fn decompress(&mut self, out: &mut [u8], room: usize, input: &[u8], input_len: usize, uncached: &mut dyn FnMut(Target) -> bool) -> Result<(usize, usize), LzxError> {
        if !self.is_lzx {
            return Err(LzxError::NotLzx);
        }
        self.decompress_checked(out, room, input, input_len, uncached)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LzxStream {
    pub input_left: usize,
    pub input_at: usize,
    pub block_used: usize,
    pub block_size: usize,
    pub output_room: usize,
    pub output_at: usize,
    pub output_made: usize,
    pub output_size: usize,
}

pub const STREAM_BLOCK_HEAD: usize = 8;

impl LzxStream {
    pub fn make_context() -> Box<LzxContext> {
        let flags = ContextFlags { stream: true, ..ContextFlags::default() };
        LzxContext::create(Codec::Lzx, Some(GAME_PARAMS), flags).expect("the game's window is a valid LZX window")
    }

    pub fn reset(&mut self, ctx: &mut LzxContext, block: usize, size: usize) {
        let _ = ctx.reset();
        *self = LzxStream { block_size: block, output_size: size, ..LzxStream::default() };
    }

    pub fn step(&mut self, ctx: &mut LzxContext, input: &[u8], out: &mut [u8], uncached: &mut dyn FnMut(Target) -> bool) -> Result<(), LzxError> {
        if self.block_used == 0 {
            let at = self.input_at;
            let head = input.get(at..at + STREAM_BLOCK_HEAD).ok_or(LzxError::OutOfInput)?;
            self.block_size = u32::from_be_bytes([head[4], head[5], head[6], head[7]]) as usize;
            self.input_left = self.input_left.checked_sub(STREAM_BLOCK_HEAD).ok_or(LzxError::OutOfInput)?;
            self.input_at += STREAM_BLOCK_HEAD;
        }
        let in_size = self.input_left.min(self.block_size - self.block_used);
        let input_from = input.get(self.input_at..).unwrap_or(&[]);
        let out_from = out.get_mut(self.output_at..).unwrap_or(&mut []);
        let (made, read) = ctx.decompress(out_from, self.output_room, input_from, in_size, uncached)?;
        self.output_at += made;
        self.input_at += read;
        self.output_room -= made;
        self.input_left -= read;
        self.output_made += made;
        self.block_used += read;
        Ok(())
    }
}
