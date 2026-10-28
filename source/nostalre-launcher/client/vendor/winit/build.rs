use cfg_aliases::cfg_aliases;

fn main() {
    // The script doesn't depend on our code.
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("horizon") {
        println!("cargo:rerun-if-changed=src/horizon/native.c");
        cc::Build::new().file("src/horizon/native.c")
            .include("/opt/devkitpro/libnx/include").compile("nostalre_hid");
    }
    // Setup cfg aliases.
    cfg_aliases! {
        // Systems.
        android_platform: { target_os = "android" },
        web_platform: { all(target_family = "wasm", target_os = "unknown") },
        macos_platform: { target_os = "macos" },
        ios_platform: { target_os = "ios" },
        windows_platform: { target_os = "windows" },
        apple: { any(target_os = "ios", target_os = "macos") },
        free_unix: { all(unix, not(apple), not(android_platform), not(target_os = "emscripten"), not(target_os = "horizon")) },
        redox: { target_os = "redox" },

        // Native displays.
        x11_platform: { all(feature = "x11", free_unix, not(redox)) },
        wayland_platform: { all(feature = "wayland", free_unix, not(redox)) },
        orbital_platform: { redox },
    }

    // Winit defined cfgs.
    println!("cargo:rustc-check-cfg=cfg(unreleased_changelogs)");
}
