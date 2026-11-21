use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::time::{Duration, Instant};

type H = *mut c_void;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> H;
    fn GetProcAddress(m: H, name: *const u8) -> *const c_void;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> H;
    fn CloseHandle(h: H) -> i32;
    fn TerminateProcess(h: H, code: u32) -> i32;
    fn WaitForSingleObject(h: H, ms: u32) -> u32;
    fn ReadProcessMemory(h: H, at: *const c_void, buf: *mut c_void, n: usize, got: *mut usize) -> i32;
    fn K32EnumProcesses(ids: *mut u32, bytes: u32, got: *mut u32) -> i32;
    fn K32EnumProcessModulesEx(h: H, mods: *mut H, bytes: u32, got: *mut u32, filter: u32) -> i32;
    fn K32GetModuleBaseNameW(h: H, m: H, name: *mut u16, len: u32) -> u32;
}

type Decompress = unsafe extern "system" fn(
    *const u8, i64, *mut u8, i64, i32, i32, i32, *mut c_void, i64, *mut c_void, *mut c_void, *mut c_void, i64, i32,
) -> i64;

pub struct Oodle(Decompress);
unsafe impl Send for Oodle {}
unsafe impl Sync for Oodle {}

impl Oodle {
    pub fn load(dll: &Path) -> Result<Self, String> {
        use std::os::windows::ffi::OsStrExt;
        let w: Vec<u16> = dll.as_os_str().encode_wide().chain([0]).collect();
        let m = unsafe { LoadLibraryW(w.as_ptr()) };
        let f = if m.is_null() { std::ptr::null() } else { unsafe { GetProcAddress(m, c"OodleLZ_Decompress".as_ptr().cast()) } };
        if f.is_null() {
            return Err(format!("{}: can't load RDR2's Oodle DLL", dll.display()));
        }
        Ok(Oodle(unsafe { std::mem::transmute::<*const c_void, Decompress>(f) }))
    }

    pub fn decompress(&self, src: &[u8], size: usize) -> Result<Vec<u8>, String> {
        let mut out = vec![0u8; size];
        let n = unsafe {
            (self.0)(src.as_ptr(), src.len() as i64, out.as_mut_ptr(), size as i64, 1, 0, 0, std::ptr::null_mut(), 0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0, 3)
        };
        if n <= 0 {
            return Err("rdr2: Oodle couldn't decompress a file".into());
        }
        out.truncate(n as usize);
        Ok(out)
    }
}

const KEY_MAP: u64 = 0x5567e68;
const ALL_KEYS: u32 = 167;
const STEAM_RUN: &str = "steam://rungameid/1174180";

struct Proc(H);
impl Drop for Proc {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
impl Proc {
    fn read(&self, at: u64, n: usize) -> Option<Vec<u8>> {
        let mut b = vec![0u8; n];
        let mut got = 0;
        (unsafe { ReadProcessMemory(self.0, at as *const c_void, b.as_mut_ptr().cast(), n, &mut got) } != 0 && got == n).then_some(b)
    }
    fn u64(&self, at: u64) -> Option<u64> {
        self.read(at, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    }
    fn exited(&self) -> bool {
        unsafe { WaitForSingleObject(self.0, 0) == 0 }
    }
    fn base(&self) -> Option<u64> {
        let mut m: H = std::ptr::null_mut();
        let mut got = 0;
        (unsafe { K32EnumProcessModulesEx(self.0, &mut m, 8, &mut got, 3) } != 0 && !m.is_null()).then_some(m as u64)
    }
}

fn find() -> Option<Proc> {
    let mut ids = vec![0u32; 8192];
    let mut got = 0;
    unsafe { K32EnumProcesses(ids.as_mut_ptr(), (ids.len() * 4) as u32, &mut got) };
    ids.truncate(got as usize / 4);
    ids.into_iter().find_map(|pid| {
        let h = unsafe { OpenProcess(0x0010_0411, 0, pid) };
        if h.is_null() {
            return None;
        }
        let p = Proc(h);
        let mut name = [0u16; 260];
        let n = unsafe { K32GetModuleBaseNameW(h, std::ptr::null_mut(), name.as_mut_ptr(), 260) } as usize;
        String::from_utf16_lossy(&name[..n]).eq_ignore_ascii_case("rdr2.exe").then_some(p)
    })
}

fn keys(p: &Proc, base: u64, want: u32) -> Option<HashMap<u8, Vec<u8>>> {
    let map = p.u64(base + KEY_MAP).filter(|&m| m != 0)?;
    let m = p.read(map, 0x18)?;
    let (buckets, nb, count) = (u64::from_le_bytes(m[..8].try_into().unwrap()), u32::from_le_bytes(m[8..12].try_into().unwrap()), u32::from_le_bytes(m[16..20].try_into().unwrap()));
    if count < want || nb == 0 || nb > 4096 {
        return None;
    }
    let heads = p.read(buckets, nb as usize * 8)?;
    let mut out = HashMap::new();
    for h in heads.chunks_exact(8) {
        let mut node = u64::from_le_bytes(h.try_into().unwrap());
        while node != 0 && out.len() < 4096 {
            let n = p.read(node, 0x18)?;
            let (id, key) = (u64::from_le_bytes(n[..8].try_into().unwrap()), u64::from_le_bytes(n[8..16].try_into().unwrap()));
            if id < 256 && key != 0 {
                out.insert(id as u8, p.read(key, crate::KEY_LEN)?);
            }
            node = u64::from_le_bytes(n[16..24].try_into().unwrap());
        }
    }
    Some(out)
}

fn image(p: &Proc, base: u64) -> Option<Vec<u8>> {
    let h = p.read(base, 0x1000)?;
    let pe = u32::from_le_bytes(h.get(0x3c..0x40)?.try_into().ok()?) as usize;
    let size = u32::from_le_bytes(h.get(pe + 0x50..pe + 0x54)?.try_into().ok()?) as usize;
    let mut out = vec![0u8; size];
    for at in (0..size).step_by(0x10000) {
        let n = 0x10000.min(size - at);
        match p.read(base + at as u64, n) {
            Some(b) => out[at..at + n].copy_from_slice(&b),
            None => for pg in (at..at + n).step_by(0x1000) {
                if let Some(b) = p.read(base + pg as u64, 0x1000.min(size - pg)) {
                    out[pg..pg + b.len()].copy_from_slice(&b);
                }
            },
        }
    }
    Some(out)
}

pub fn capture() -> Result<(HashMap<u8, Vec<u8>>, Vec<u8>), String> {
    let running = find();
    let ours = running.is_none();
    if ours {
        std::process::Command::new("cmd").args(["/C", "start", "", STEAM_RUN]).spawn().map_err(|e| format!("can't start RDR2 through Steam: {e}"))?;
    }
    let t0 = Instant::now();
    let p = match running {
        Some(p) => p,
        None => loop {
            if let Some(p) = find() {
                break p;
            }
            if t0.elapsed() > Duration::from_secs(300) {
                return Err("RDR2 didn't start (is Steam running and RDR2 installed?)".into());
            }
            std::thread::sleep(Duration::from_millis(200));
        },
    };
    let started = Instant::now();
    let mut prev: Option<HashMap<u8, Vec<u8>>> = None;
    let got = loop {
        let want = if started.elapsed() > Duration::from_secs(90) { 1 } else { ALL_KEYS };
        if let Some(k) = p.base().and_then(|b| keys(&p, b, want)) {
            if prev.as_ref() == Some(&k) {
                break Some(k);
            }
            prev = Some(k);
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }
        if p.exited() || started.elapsed() > Duration::from_secs(120) {
            break None;
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let img = got.as_ref().and_then(|_| p.base()).and_then(|b| image(&p, b));
    if ours {
        unsafe { TerminateProcess(p.0, 0) };
        unsafe { WaitForSingleObject(p.0, 10_000) };
    }
    let keys = got.ok_or_else(|| "RDR2 closed before its keys loaded".to_string())?;
    Ok((keys, img.ok_or("couldn't read RDR2's memory")?))
}
