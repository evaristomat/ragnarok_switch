#!/bin/bash
set -euo pipefail
S=/src/nostalre-launcher/server
cmake -S "$S" -B /server-build -G Ninja \
 -DCMAKE_TOOLCHAIN_FILE=/src/nostalre-launcher/scripts/switch-toolchain.cmake \
 -DCMAKE_BUILD_TYPE=Release -DNOSTALRE_SWITCH=ON \
 -U CMAKE_C_FLAGS -U CMAKE_CXX_FLAGS \
 -DENABLE_SQLITE=ON -DENABLE_WEB_SERVER=OFF -DPACKETVER=20250716 \
 -DBUILD_TOOLS=OFF -DWITH_PCRE=OFF \
 -DRT_LIBRARY=/opt/devkitpro/devkitA64/aarch64-none-elf/lib/pic/libc.a \
 -DLIBCONFIG_INCLUDE_DIRS="$S/3rdparty/libconfig" \
 -DHAVE_SETRLIMIT=OFF -DHAVE_GETTID=OFF \
 -DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=/server-build/lib \
 -DCMAKE_LIBRARY_OUTPUT_DIRECTORY=/server-build/lib \
 -DCMAKE_RUNTIME_OUTPUT_DIRECTORY=/server-build/bin \
 -DYAML_INCLUDE_DIRS="$S/3rdparty/yaml-cpp/include" \
 -DYAML_SOURCE_DIR="$S/3rdparty/yaml-cpp/src"
cmake --build /server-build --target char-server map-server -j 1
BIN=/opt/devkitpro/devkitA64/bin/aarch64-none-elf
OUT=/src/nostalre-launcher/client/switch/lib
mkdir -p "$OUT"
for role in char map; do
 dir="/server-build/isolated-$role"
 mkdir -p "$dir"
 "$BIN-ld" -r --force-group-allocation --whole-archive \
  /server-build/lib/lib${role}-serverr.a \
  /server-build/lib/libcommonr.a /server-build/lib/libcommon_baser.a \
  /server-build/lib/librymlr.a /server-build/lib/libyaml-cppr.a \
  --no-whole-archive -o "$dir/server.o"
 "$BIN-objcopy" --redefine-sym main=nostalre_${role}_main \
  --redefine-sym nostalre_server_stop=nostalre_${role}_stop \
  --redefine-sym nostalre_server_status=nostalre_${role}_status "$dir/server.o"
 printf 'nostalre_%s_main\nnostalre_%s_stop\nnostalre_%s_status\n' "$role" "$role" "$role" > "$dir/keep.txt"
 if [ "$role" = char ]; then echo nostalre_offline_initialize >> "$dir/keep.txt"; fi
 "$BIN-objcopy" --keep-global-symbols="$dir/keep.txt" "$dir/server.o"
 "$BIN-ar" rcs "$OUT/libnostalre_${role}.a" "$dir/server.o"
done
cp /server-build/lib/libsqlite3r.a "$OUT/libsqlite3.a"
echo CLASSIC_SWITCH_SERVERS_BUILT
