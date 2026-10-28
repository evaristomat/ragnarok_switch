#!/bin/bash
set -euo pipefail
cd /src/nostalre-launcher/client
STD_LIB="$(rustc +nightly --print sysroot)/lib/rustlib/src/rust/library"
cargo +nightly update --config .cargo/switch.toml -p libc --manifest-path "$STD_LIB/Cargo.toml"
cargo +nightly build --locked --release --config .cargo/switch.toml -j 1 \
 -Z build-std=core,alloc,std,panic_abort -Z json-target-spec \
 --target /src/bevy-horizon/aarch64-nintendo-horizon.json \
 --package nostalre-switch --bin nostalre
cp /build/aarch64-nintendo-horizon/release/nostalre /edition/outputs/ragswitch-pre.elf
bash /edition/scripts/package_client.sh
