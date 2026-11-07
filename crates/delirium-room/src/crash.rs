#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::OnceLock;

    #[repr(C, packed(4))]
    struct ExceptionInfo {
        thread: u32,
        pointers: *mut c_void,
        client: i32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetUnhandledExceptionFilter(f: Option<unsafe extern "system" fn(*mut c_void) -> i32>) -> *mut c_void;
        fn GetCurrentProcess() -> *mut c_void;
        fn GetCurrentProcessId() -> u32;
        fn GetCurrentThreadId() -> u32;
        fn CreateFileW(name: *const u16, access: u32, share: u32, sec: *mut c_void, how: u32, flags: u32, template: *mut c_void) -> *mut c_void;
        fn CloseHandle(h: *mut c_void) -> i32;
    }
    #[link(name = "dbghelp")]
    unsafe extern "system" {
        fn MiniDumpWriteDump(process: *mut c_void, pid: u32, file: *mut c_void, kind: u32, ex: *const ExceptionInfo, user: *const c_void, cb: *const c_void) -> i32;
    }

    static PATH: OnceLock<(Vec<u16>, String)> = OnceLock::new();

    unsafe extern "system" fn filter(pointers: *mut c_void) -> i32 {
        if let Some((wide, shown)) = PATH.get() {
            let f = unsafe { CreateFileW(wide.as_ptr(), 0x4000_0000, 0, std::ptr::null_mut(), 2, 0x80, std::ptr::null_mut()) };
            if f as isize != -1 {
                let ex = ExceptionInfo { thread: unsafe { GetCurrentThreadId() }, pointers, client: 0 };
                let ok = unsafe { MiniDumpWriteDump(GetCurrentProcess(), GetCurrentProcessId(), f, 0x1 | 0x40 | 0x1000, &ex, std::ptr::null(), std::ptr::null()) };
                unsafe { CloseHandle(f) };
                if ok != 0 {
                    eprintln!("\nThe room crashed. Please send this file with your report: {shown}");
                }
            }
        }
        let _ = std::panic::catch_unwind(|| super::save(&format!("The room crashed (the system stopped it; crash.dmp has where)\n\n{}", super::system())));
        0
    }

    pub fn install() {
        let dir = delirium_data::xex::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("crash.dmp");
        let _ = std::fs::rename(&p, dir.join("crash-previous.dmp"));
        let _ = PATH.set((p.as_os_str().encode_wide().chain([0]).collect(), p.display().to_string()));
        unsafe { SetUnhandledExceptionFilter(Some(filter)) };
        super::panics();
    }
}

fn panics() {
    let dirs = [std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())), Some(delirium_data::xex::config_dir())];
    for d in dirs.iter().flatten() {
        let p = d.join("crash.txt");
        if let Ok(t) = std::fs::read_to_string(&p) {
            let _ = std::fs::rename(&p, d.join("crash-previous.txt"));
            *pending() = Some(t);
        }
    }
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::remove_file(crate::getup::config_dir().join(crate::STARTING));
        save(&format!("The room crashed: {info}\n\n{}\n{}", system(), std::backtrace::Backtrace::force_capture()));
        default(info);
    }));
}

fn save(text: &str) {
    let dirs = [std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())), Some(delirium_data::xex::config_dir())];
    if let Some(p) = dirs.iter().flatten().map(|d| d.join("crash.txt")).find(|p| std::fs::write(p, text).is_ok()) {
        eprintln!("\nThe room crashed. It saved a report: {} (the next start offers to send it)", p.display());
    }
}

pub fn render_error(e: &bevy::render::error_handler::RenderError, main: &mut bevy::prelude::World, _: &mut bevy::prelude::World)
    -> bevy::render::error_handler::RenderErrorPolicy {
    save(&format!("The room crashed: graphics error ({:?}): {}\n\n{}", e.ty, e.description, system()));
    main.write_message(bevy::app::AppExit::error());
    bevy::render::error_handler::RenderErrorPolicy::StopRendering
}

static LIVE: std::sync::Mutex<(String, Vec<(f32, String)>)> = std::sync::Mutex::new((String::new(), vec![]));

pub fn note_settings(now: &str) {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let t = START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32();
    let mut l = LIVE.lock().unwrap_or_else(|e| e.into_inner());
    if l.0 == now {
        return;
    }
    if !l.0.is_empty() {
        let was: std::collections::HashMap<&str, &str> = l.0.lines().filter_map(|x| x.split_once(' ')).collect();
        let changed: Vec<String> = now.lines().filter_map(|x| x.split_once(' ')).filter(|(k, v)| was.get(k) != Some(v))
            .map(|(k, v)| format!("{k} {} -> {v}", was.get(k).unwrap_or(&"?"))).collect();
        if !changed.is_empty() {
            l.1.push((t, changed.join(", ")));
            let n = l.1.len().saturating_sub(10);
            l.1.drain(..n);
        }
    }
    l.0 = now.to_string();
}

static GPU: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn note_gpu(gpu: String) {
    let _ = GPU.set(gpu);
}

pub fn pending() -> std::sync::MutexGuard<'static, Option<String>> {
    static LAST: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    LAST.lock().unwrap_or_else(|e| e.into_inner())
}

fn system() -> String {
    let run = |c: &str, a: &[&str]| std::process::Command::new(c).args(a).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let os = if cfg!(windows) { run("cmd", &["/c", "ver"]) } else if cfg!(target_os = "macos") { run("sw_vers", &["-productVersion"]).map(|v| format!("macOS {v}")) } else { run("uname", &["-sr"]) };
    let (settings, changes) = match LIVE.try_lock() {
        Ok(l) if !l.0.is_empty() => (l.0.clone(), l.1.iter().rev().map(|(t, c)| format!("  {t:.1} s: {c}\n")).collect::<String>()),
        _ => (std::fs::read_to_string(delirium_data::xex::config_dir().join("settings.txt")).unwrap_or_default(), String::new()),
    };
    let changes = if changes.is_empty() { "  none\n".into() } else { changes };
    format!("System: {} ({})\nGraphics: {}\n\nLast settings changed (newest first, seconds after start):\n{changes}\nSettings:\n{settings}",
        os.unwrap_or_default(), std::env::consts::ARCH, GPU.get().map_or("not started", |g| g))
}

pub fn report(text: &str) {
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    let first = text.lines().next().unwrap_or("The room crashed");
    let body: String = text.chars().take(5000).collect();
    let url = format!("https://github.com/PVCPC/delirium-physics/issues/new?title={}&body={}", enc(first), enc(&format!("```\n{body}\n```")));
    let _ = if cfg!(windows) {
        std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&url).spawn()
    };
}

#[cfg(windows)]
pub use imp::install;
#[cfg(not(windows))]
pub fn install() {
    panics();
}
