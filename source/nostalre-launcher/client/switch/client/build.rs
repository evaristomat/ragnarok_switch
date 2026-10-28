use std::path::PathBuf;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("horizon") { return; }
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let version_path = manifest.join("../VERSION");
    let version = std::fs::read_to_string(&version_path).expect("Missing Switch VERSION");
    println!("cargo:rustc-env=NOSTALRE_SWITCH_VERSION={}", version.trim());
    println!("cargo:rerun-if-changed={}", version_path.display());
    let seed = manifest.join("../romfs/server/ragnarok.db");
    let bytes = std::fs::read(&seed).expect("Missing seed database");
    assert!(bytes.starts_with(b"SQLite format 3\0") && bytes.get(18..20) == Some(&[1, 1]),
        "Switch SQLite omits WAL: seed must use PRAGMA journal_mode=DELETE");
    println!("cargo:rerun-if-changed={}", seed.display());
    cc::Build::new().file(manifest.join("../sqlite_vfs.c"))
        .file(manifest.join("../sqlite_horizon_io.c"))
        .include(manifest.join("../../../server/3rdparty/sqlite"))
        .compile("nostalre_sqlite_vfs");
    println!("cargo:rerun-if-changed=../sqlite_vfs.c");
    println!("cargo:rerun-if-changed=../sqlite_horizon_io.c");
    cc::Build::new().cpp(true).file(manifest.join("../server_bridge.cpp"))
        .flag("-fexceptions").compile("nostalre_server_bridge");
    println!("cargo:rerun-if-changed=../server_bridge.cpp");
    println!("cargo:rerun-if-env-changed=NOSTALRE_CHECK_ONLY");
    if std::env::var_os("NOSTALRE_CHECK_ONLY").is_some() { return; }
    let libraries = manifest.join("../lib");
    for name in ["nostalre_map", "nostalre_char", "sqlite3"] {
        let path = libraries.join(format!("lib{name}.a"));
        assert!(path.is_file(), "Missing server library: {}", path.display());
        println!("cargo:rerun-if-changed={}", path.display());
        println!("cargo:rustc-link-lib=static={name}");
    }
    println!("cargo:rustc-link-search=native={}", libraries.display());
    println!("cargo:rustc-link-lib=z");
    println!("cargo:rustc-link-lib=stdc++");
}
