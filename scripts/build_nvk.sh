#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Adapted from source/bevy-horizon/scripts/{build_nvk,localize_nvk}.sh.
# Uses the already patched vendored Mesa tree, without nested Git repositories.
set -euo pipefail
JOBS="${JOBS:-2}"
MESA=/edition/build/nvk/mesa
OUT=/src/bevy-horizon/nvk
mkdir -p "$MESA"
cp -a /src/mesa-switch/. "$MESA/"
cd "$MESA"
mkdir -p /usr/local/libexec
cp bindgen-switch-wrapper.sh /usr/local/libexec/bindgen
cp rustc-switch-wrapper.sh /usr/local/libexec/rustc
cp bindgen-atomic-shim.h /usr/local/libexec/
chmod +x /usr/local/libexec/bindgen /usr/local/libexec/rustc
export MESA_SWITCH_RUSTC=/usr/local/cargo/bin/rustc
export MESA_SWITCH_BINDGEN=/usr/local/cargo/bin/bindgen
export MESA_SWITCH_RUST_TARGET=aarch64-unknown-linux-gnu
cp src/gallium/winsys/nouveau/drm/nouveau.h src/nouveau/headers/nv_device_info.h \
    /opt/devkitpro/portlibs/switch/include/
if [ ! -d /usr/lib/llvm-15/lib/clang/15/include ]; then
    ln -sf /usr/lib/llvm-15/lib/clang/15.0.6/include /usr/lib/llvm-15/lib/clang/15/include
fi
echo 'void __mesa_switch_stub_lib(void) {}' > /tmp/stub_posix.c
aarch64-none-elf-gcc -c /tmp/stub_posix.c -o /tmp/stub_posix.o
for lib in dl rt util; do
    [ -f "/opt/devkitpro/portlibs/switch/lib/lib$lib.a" ] ||
        aarch64-none-elf-ar rcs "/opt/devkitpro/portlibs/switch/lib/lib$lib.a" /tmp/stub_posix.o
done
setup() {
    local dir=$1
    shift
    if [ -f "$dir/meson-private/coredata.dat" ]; then
        meson setup "$dir" --reconfigure "$@"
    else
        meson setup "$dir" "$@"
    fi
}
setup builddir-native \
    -Dvulkan-drivers= -Dgallium-drivers= -Dshader-cache=enabled -Dplatforms= \
    -Dglx=disabled -Degl=disabled -Dopengl=false -Dgles1=disabled -Dgles2=disabled \
    -Dtools=[] -Dllvm=enabled -Dmesa-clc=enabled -Dprecomp-compiler=enabled \
    -Dinstall-mesa-clc=true
ninja -C builddir-native -j "$JOBS" src/compiler/clc/mesa_clc src/compiler/spirv/vtn_bindgen2
export PATH="/usr/local/libexec:$MESA/builddir-native/src/compiler/clc:$MESA/builddir-native/src/compiler/spirv:$PATH"
printf "[binaries]\nrust = '%s'\n" "$MESA_SWITCH_RUSTC" > /usr/local/libexec/nvk-native.txt
MESA_COMMIT=$(python3 -c 'import json;print(json.load(open("/edition/toolchain/environment.json"))["mesa_upstream_commit"])')
setup builddir-switch \
    --cross-file switch_cross_file.txt --native-file /usr/local/libexec/nvk-native.txt \
    --default-library=static --buildtype=release -Doptimization=2 -Db_lto=false -Db_ndebug=true \
    -Dvulkan-drivers=nouveau -Dgallium-drivers= -Dgallium-rusticl=false \
    -Dplatforms=switch -Dglx=disabled -Degl=disabled -Dopengl=false \
    -Dgles1=disabled -Dgles2=disabled -Dvideo-codecs= \
    -Dshader-cache=enabled -Dxmlconfig=enabled -Dexpat=enabled -Dtools=[] \
    -Dllvm=disabled -Dshared-glapi=disabled -Dshared-llvm=disabled \
    -Dmesa-clc=system -Dprecomp-compiler=system -Dcpp_rtti=false \
    -Dbuild-tests=false -Dnvk-build-id="$MESA_COMMIT"
ninja -C builddir-switch -j "$JOBS" src/nouveau/vulkan/libvulkan.a
BIN=/opt/devkitpro/devkitA64/bin/aarch64-none-elf
"$BIN-ranlib" builddir-switch/src/nouveau/vulkan/libvulkan.a
work=$(mktemp -d /edition/build/nvk/stage.XXXXXX)
trap 'rm -rf "$work"' EXIT
cp builddir-switch/src/nouveau/vulkan/libvulkan.a "$work/"
cd "$work"
KEEP_RE='^(vk[A-Z]|nouveau_horizon_region_(alloc|free|stats)$|nouveau_horizon_tune$)'
"$BIN-nm" -g --defined-only libvulkan.a 2>/dev/null \
    | awk -v re="$KEEP_RE" 'NF == 3 && $2 ~ /^[TDRBVW]$/ && $3 ~ re { print $3 }' \
    | sort -u > keep.txt
test -s keep.txt
"$BIN-ld" -r --force-group-allocation -o vulkan_bh.o $(sed 's/^/-u /' keep.txt) libvulkan.a
"$BIN-objcopy" --keep-global-symbols=keep.txt vulkan_bh.o
"$BIN-nm" -g --defined-only vulkan_bh.o | awk -v re="$KEEP_RE" '$3 !~ re' > leftover.txt
test ! -s leftover.txt
if "$BIN-readelf" -g vulkan_bh.o | grep -q 'COMDAT group'; then
    echo 'COMDAT groups survived localization' >&2
    exit 1
fi
"$BIN-ar" rcs libvulkan_bh.a vulkan_bh.o
mkdir -p "$OUT/lib" "$OUT/include"
cp libvulkan.a libvulkan_bh.a "$OUT/lib/"
cp -r "$MESA/include/vulkan" "$MESA/include/vk_video" "$OUT/include/"
{
    echo "mesa-switch baseline: $MESA_COMMIT; vendored patches already included"
    dkp-pacman -Q devkitA64 libnx switch-libexpat switch-libzstd switch-zlib
    rustc +nightly -V
    meson --version
} > "$OUT/BUILD_INFO"
echo CLASSIC_NVK_BUILT
