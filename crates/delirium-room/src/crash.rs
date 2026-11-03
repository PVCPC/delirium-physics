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
        0
    }

    pub fn install() {
        let dir = delirium_data::xex::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("crash.dmp");
        let _ = std::fs::rename(&p, dir.join("crash-previous.dmp"));
        let _ = PATH.set((p.as_os_str().encode_wide().chain([0]).collect(), p.display().to_string()));
        unsafe { SetUnhandledExceptionFilter(Some(filter)) };
    }
}

#[cfg(windows)]
pub use imp::install;
#[cfg(not(windows))]
pub fn install() {}
