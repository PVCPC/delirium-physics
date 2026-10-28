use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=delirium.ico");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let Some(rc) = rc() else {
        println!("cargo:warning=no rc.exe (the Windows SDK): the room's exe has no icon or name");
        return;
    };
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let v = env::var("CARGO_PKG_VERSION").unwrap();
    let n: Vec<&str> = v.split(['.', '-']).chain(["0"; 4]).take(4).collect();
    let ico = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("delirium.ico");
    let script = format!(r#"1 ICON "{}"
1 VERSIONINFO
FILEVERSION {n},0
PRODUCTVERSION {n},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "FileDescription", "Delirium Physics"
      VALUE "ProductName", "Delirium Physics"
      VALUE "FileVersion", "{v}"
      VALUE "ProductVersion", "{v}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#, ico.display().to_string().replace('\\', "\\\\"), n = n[..3].join(","));
    fs::write(out.join("delirium.rc"), script).unwrap();
    let res = out.join("delirium.res");
    let ok = Command::new(rc).arg("/nologo").arg("/fo").arg(&res).arg(out.join("delirium.rc")).status().is_ok_and(|s| s.success());
    if ok {
        println!("cargo:rustc-link-arg-bins={}", res.display());
    } else {
        println!("cargo:warning=rc.exe failed: the room's exe has no icon or name");
    }
}

fn rc() -> Option<PathBuf> {
    let kits = PathBuf::from(env::var("ProgramFiles(x86)").ok()?).join("Windows Kits").join("10").join("bin");
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    let mut found: Vec<PathBuf> = fs::read_dir(kits).ok()?.flatten().map(|e| e.path().join(arch).join("rc.exe")).filter(|p| p.exists()).collect();
    found.sort();
    found.pop()
}
