#!/bin/bash
set -euo pipefail
S=/src/nostalre-launcher/server
mkdir -p /host-build/zlib-include
cp /opt/devkitpro/portlibs/switch/include/zlib.h /opt/devkitpro/portlibs/switch/include/zconf.h /host-build/zlib-include/
cmake -S "$S" -B /host-build -G Ninja -DCMAKE_BUILD_TYPE=Release \
 -DCMAKE_PROJECT_TOP_LEVEL_INCLUDES=/edition/scripts/host_driver.cmake \
 -DCMAKE_C_FLAGS='-DPRERE -DNOSTALRE_OFFLINE_TEST' -DCMAKE_CXX_FLAGS='-DPRERE -DNOSTALRE_OFFLINE_TEST' \
 -DCMAKE_RUNTIME_OUTPUT_DIRECTORY=/host-build/bin \
 -DZLIB_SYSTEM_LIBRARIES=/usr/lib/x86_64-linux-gnu/libz.so.1 -DZLIB_SYSTEM_INCLUDE_DIRS=/host-build/zlib-include -DWITH_ZLIB=ON \
 -DZLIB_INCLUDE_DIR=/host-build/zlib-include -DZLIB_LIBRARY=/usr/lib/x86_64-linux-gnu/libz.so.1 \
 -DENABLE_SQLITE=ON -DENABLE_WEB_SERVER=OFF -DPACKETVER=20250716 -DBUILD_TOOLS=OFF -DWITH_PCRE=OFF \
 -DLIBCONFIG_INCLUDE_DIRS="$S/3rdparty/libconfig" \
 -DYAML_INCLUDE_DIRS="$S/3rdparty/yaml-cpp/include" -DYAML_SOURCE_DIR="$S/3rdparty/yaml-cpp/src"
cmake --build /host-build --target char-server map-server -j 2
echo CLASSIC_HOST_SERVERS_BUILT
