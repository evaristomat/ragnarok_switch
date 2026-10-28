# nostalRE

A native Renewal client for desktop and Android. Built with Rust and wgpu,
with touch, controller, keyboard, and mouse input. Android app builds run the
client and local servers together for offline play.

![Rust](https://img.shields.io/badge/Rust-1.97.1-DEA584?style=flat&logo=rust&logoColor=white)
![wgpu](https://img.shields.io/badge/Rendering-wgpu_28-5E4AE3?style=flat)
![winit](https://img.shields.io/badge/Windowing-winit_0.30-455A64?style=flat)
![Android](https://img.shields.io/badge/Android-11%2B-3DDC84?style=flat&logo=android&logoColor=white)
![Protocol](https://img.shields.io/badge/Protocol-20250716-1976D2?style=flat)
[![Based on nostalro-client](https://img.shields.io/badge/Based_on-nostalro--client-607D8B?style=flat)](https://github.com/nmeylan/nostalro-client)
[![AI-assisted development](https://img.shields.io/badge/Development-AI--assisted-8E44AD?style=flat)](#ai-use-disclosure)

**[Download test APK](https://github.com/si-klyde/nostalre/releases)** · [Install](#play-on-android) · [Contribute](#contributing) · [Credits](#attribution) · [License](#license) · [AI disclosure](#ai-use-disclosure)

![nostalRE gameplay on an Android handheld](docs/images/android-handheld.webp)

*A mobile-first client. User-provided photo; game resources are supplied separately.*

## Play on Android

### Download

Open **[GitHub Releases](https://github.com/si-klyde/nostalre/releases)** and
select the ARM64 `.apk` under a release's **Assets**. Read its notes for the
build's status and known issues.

> [!NOTE]
> Current APKs are debug-signed **test builds** published as pre-releases.
> Expect bugs and missing features, and back up your game folder.

| You need | Requirement |
| --- | --- |
| Device | Android 11+ (API 30), 64-bit ARM (`arm64-v8a`) |
| Resources | Your own compatible game folder containing `DATA.INI` and its referenced GRF files |

### Install

1. Open the downloaded APK and allow installation from your download app if Android asks.
2. Copy your compatible game folder to the device. Keep `DATA.INI` and its referenced archives together.
3. Launch the app, grant the requested file access, and select that folder.
4. Choose your local username, create or select a character, and play.

The app runs its servers locally; no computer or separate server setup is
needed for the APK. Optional `BGM/` music and loose `data/` overrides can stay
in your game folder.

**Bring your own resources.** No original game archives, official client
executables, or music are supplied in this repository or the APK. The photo
above is documentation, not a bundled game-resource pack.

### Report a problem

[Open an issue](https://github.com/si-klyde/nostalre/issues) with the APK
version, device model, Android version, and reproduction steps. Include relevant
logs or a recording when available.

## Contributing

This repository contains the native Rust client and resource tools. The Android
launcher, local servers, and APK packaging are maintained in a separate integration
project. Building the native library here is one part of producing an APK.

See [Features.md](Features.md) and the [protocol report](docs/packet-status-20250716.md)
for current implementation details.

### 1. Prepare a checkout

Install Rust through rustup; [rust-toolchain.toml](rust-toolchain.toml) pins
Rust `1.97.1`. Desktop play requires your own resources and a compatible rAthena
Renewal server built with `PACKETVER=20250716`.

On Debian/Ubuntu:

```sh
sudo apt-get install pkg-config libasound2-dev libx11-dev libxkbcommon-dev libwayland-dev
```

```sh
git clone https://github.com/si-klyde/nostalre.git
cd nostalre
cp config.sample.json config.json
```

### 2. Configure and run

Edit `config.json` for your server and resource installation:

```json
{
  "login_servers": [
    { "name": "Local", "host": "127.0.0.1", "port": 6900 }
  ],
  "grf_paths": ["/path/to/install/data.grf"],
  "install_dir": "/path/to/install",
  "bgm_path": "/path/to/install/BGM"
}
```

Run from the repository root:

```sh
cargo run --release --bin nostalre-client
```

The client reads and saves `config.json` in the working directory. Local config,
`data/`, and build output are ignored by Git.

<details>
<summary>Resource paths and configuration</summary>

- `install_dir`: the installation folder containing `System` and `SystemEN` tables.
- `grf_paths`: archives in priority order; the first wins.
- `data_dir`: optional loose overrides, with paths relative to the archive's `data/` folder.

See [configuration](docs/configuration.md) for display, audio, input, and other settings.

</details>

### 3. Test your changes

```sh
cargo build --workspace --all-targets
cargo test --workspace
```

Some integration tests require locally supplied resources and return early when
those files are unavailable. Keep changes focused, use short commit messages,
and explain what changed and how it was tested. Rendering and input changes
benefit from reproduction steps and screenshots or recordings.

Never commit game archives, extracted game assets, credentials, or personal configuration.

<details>
<summary>Build the Android native library</summary>

Install the Android NDK and set `ANDROID_NDK_HOME` to its directory, then:

```sh
rustup target add aarch64-linux-android
cargo install cargo-ndk
cargo ndk -t arm64-v8a -P 26 build --release -p nostalre-client --lib
```

Output: `target/aarch64-linux-android/release/libnostalre_client.so`.
`CARGO_TARGET_DIR` can select another build directory. The library's build API
level is separate from the packaged app's Android 11 minimum. APK packaging
also requires the separate integration project.

</details>

<details>
<summary>Resource tools</summary>

```sh
cargo run --release --bin sprite-viewer -- --grf data/data.grf
cargo run --release --bin nostalre-grf-editor
cargo run --release --bin grf-verify -- data/data.grf
```

Additional viewers and archive tools live in `tools/`. Hot-reload scripts
require `cargo-watch`.

</details>

| Documentation | Covers |
| --- | --- |
| [Architecture](docs/internal/architecture.md) | Crates, state, and event flow |
| [Rendering](docs/internal/rendering.md) | Rendering pipeline |
| [Configuration](docs/configuration.md) | Runtime settings |
| [Profiling](docs/profiling.md) | Performance analysis |

## Attribution

Based on **[nostalro-client](https://github.com/nmeylan/nostalro-client)** by
**Nicolas Meylan (nmeylan)**. nostalRE develops that work independently, extending
Renewal protocol support, rendering, interface, input, and platform integration.

<details>
<summary>Projects and contributors credited</summary>

| Project | Contribution |
| --- | --- |
| [nostalro-client](https://github.com/nmeylan/nostalro-client) — Nicolas Meylan | Upstream native client; Apache-2.0 |
| [rust-ro](https://github.com/nmeylan/rust-ro) — Nicolas Meylan | Models, packets, movement, macros, and packet generator; MIT |
| [Korangar](https://github.com/vE5li/korangar) — Lucas Pütz | Referenced RSM model/animation implementations; MIT |
| [winit](https://github.com/rust-windowing/winit) | Windowing and input, including the local Android modification; Apache-2.0, with W3C notices |
| [Lua](https://www.lua.org/) — Lua.org / PUC-Rio | Lua 5.1 runtime; MIT |
| Noto Project Authors / Adobe | Bundled fonts; SIL Open Font License |
| [rAthena](https://github.com/rathena/rathena) — rAthena Development Team | Protocol and generated server datasets; GPL-3.0-or-later |
| [AzzyAI](https://github.com/SpenceKonde/AzzyAI) — Dr. Azzy / Spence Konde | Design credit and tactic defaults inherited through nostalro-client |

</details>

See [LICENSE.md](LICENSE.md), [NOTICE](NOTICE), and [LICENSES](LICENSES) for
license texts and component details. The [bundled-content review](docs/third-party-content.md)
records the remaining permission question for inherited tactic defaults.

The repository also includes original charm artwork, OFL fonts, authored parser
fixtures, vendored examples/test data, and the user-provided README photo.
Game-specific compatibility tables and resource names remain in the source.

## License

nostalRE combines code under several licenses. Each part keeps its upstream terms:

| Part | License |
| --- | --- |
| Client, libraries, and tools (default) | Apache-2.0 — [LICENSE.md](LICENSE.md) |
| `rust-ro/` | MIT — [rust-ro/LICENSE.md](rust-ro/LICENSE.md) |
| `vendor/winit/` | Apache-2.0 — [vendor/winit/LICENSE](vendor/winit/LICENSE), with [W3C notices](LICENSES/winit-W3C.txt) |
| Korangar-derived RSM code | MIT — [LICENSES/korangar-MIT.txt](LICENSES/korangar-MIT.txt) |
| Linked Lua 5.1 runtime | MIT — [LICENSES/lua-MIT.txt](LICENSES/lua-MIT.txt) |
| Bundled Noto fonts | SIL OFL 1.1 — [OFL-LICENSE.txt](lib/renderer/src/fonts/OFL-LICENSE.txt) |
| [rAthena-derived tables and packet data](docs/third-party-content.md#resolved-generated-rathena-datasets) (7 files) | GPL-3.0-or-later — [LICENSES/rathena-GPL-3.0.txt](LICENSES/rathena-GPL-3.0.txt) |

The rAthena-derived tables are compiled into the client, so released binaries,
including the APK, are distributed under GPL-3.0-or-later as a whole. The
Apache-2.0, MIT, and OFL parts remain available under their own terms in source form.
[NOTICE](NOTICE) lists attribution and modification details.

The APK's login, char, and map servers are built from
[si-klyde/rathena-nostalre](https://github.com/si-klyde/rathena-nostalre), a GPL-3.0
fork of rAthena; tag `nostalre-v0.1.0` matches the v0.1.0 APK.

## AI use disclosure

AI tools assist with implementation, debugging, protocol and rendering work,
tests, refactoring, and documentation. This README was also written with AI assistance.

Validation includes builds, automated tests, and manual play checks. Contributors
should verify and understand generated changes before submitting them. AI assistance
does not replace attribution or licensing requirements.
