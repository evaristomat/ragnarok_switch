#!/bin/bash
set -euo pipefail
cd /src/nostalre-launcher/client
cargo +nightly test --release --locked --config .cargo/switch.toml \
 -p nostalre-game -p nostalre-ui-component -p nostalre-client -p nostalre-network \
 --lib -j 1 -- --test-threads=1
cd /src/offline-probe
CARGO_TARGET_DIR=/test-build/probe cargo +nightly build --release --locked -j 1
echo CLASSIC_TESTS_AND_PROBE_BUILT
