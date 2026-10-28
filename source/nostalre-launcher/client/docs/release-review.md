# Release review

Status: implementations retained; bundled-content review completed with one upstream permission question (AzzyAI) open. rAthena-derived files are distributed under GPL-3.0-or-later.
No original Git history or private executable-research scripts/documents were imported.

## Inventory

[provenance.tsv](provenance.tsv) records every imported source file and its
current disposition. Origin is compared against upstream baseline `b4ec4a4`
where that baseline contains the file; vendored dependencies are identified
separately. `keep` means no issue identified in this pass, not legal clearance.
Repository preparation files added here are described below.

## Completed preparation

- Isolated the client from the integration repository, including current source changes.
- Renamed internal crates, Rust imports, executable names, and visible window/tool branding.
- Removed the unused client-message dump and its module declaration.
- Retained upstream Apache-2.0, rust-ro MIT, winit Apache-2.0, and font OFL notices.
- Added full MIT notices for incorporated Korangar code and linked Lua.
- Added a standalone README and NOTICE; CI targets main and builds with the committed Cargo.lock.
- Excluded local config, data archives, captures, build output, credentials, and assistant settings.
- Started fresh history with one import commit (2026-10-23); no integration history is included.

## Bundled-content and attribution review

The user confirmed retention of the implementations on 2026-10-23. The former
86-file implementation/provenance flag list is closed as a blanket publication
blocker. Current dispositions in provenance.tsv retain those implementations.

The completed content audit is in [third-party-content.md](third-party-content.md).
One upstream permission question remains: the inherited AzzyAI tactic defaults
(lib/ai/src/tactics.rs). The seven rAthena-derived dataset files were confirmed
GPL-3.0-or-later on 2026-10-24 and are distributed under those terms; no code or
data was removed in this pass.
Font copyright details, W3C keyboard notices, and source credits were added.

## Validation

Preparation checks (2026-10-23): `cargo build --workspace --all-targets --locked --offline`
passed. Workspace tests passed 2,407 cases after excluding five local-socket tests
under `emblem_http::tests`. The unfiltered run failed those five because binding
local sockets returned `Operation not permitted` in this sandbox; no test code
was changed to bypass that restriction.

The staged-file scan found no matching secret signatures or forbidden local
files. Apache/MIT license files were checked against the source copies. TOML,
JSON, Python, and shell syntax checks passed. Viewer help output uses nostalRE.
Original source hashes remained unchanged.

Android verification: the optimized `arm64-v8a` client library built successfully
and exports `android_main` and `ANativeActivity_onCreate`. Review found stale
crate/library names in the separate integration repository's APK and desktop
scripts; those scripts now resolve both the old and standalone client names,
including custom Cargo target directories. The existing APK loader name is kept
when staging the standalone library.

All 21 Android Kotlin files, Java resources, and DEX compiled with cached tools.
A signed review APK passed signature, 16 KB ZIP-alignment, manifest, library,
font, and asset checks. Gradle itself cannot start in this sandbox because its
local networking is blocked, so the review APK used SDK/compiler tools directly.
No installation or device runtime test was performed.

A separate checkout of the staged export, without the local lockfile or game data,
resolved dependencies offline, built all targets, and passed the same 2,407 tests
with the five socket tests excluded. CI keeps the unfiltered test command.
Optional asset-dependent tests may return early without supplied resources;
passing them does not validate asset rendering or provenance.

## Publication status (2026-10-24)

The export is pushed to the private repository si-klyde/nostalre, with a v0.1.0
debug-signed test APK published as a pre-release. Before making the repository
public:

- APK source: the rAthena servers' source, with local changes on top of
  d4b8e7b, is published at si-klyde/rathena-nostalre (tag nostalre-v0.1.0,
  2026-10-24). Still unpublished: the bundled roBrowser web client
  (client.zip, GPL-3.0) and the ws-proxy/grf-server bridge. The APK also does
  not yet include the GPL license text or a source link for these parts.
- Resolve the AzzyAI permission question above. companion_ai.json carries
  AzzyAI-style settings inherited from upstream and should be checked with it.
