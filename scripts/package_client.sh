#!/bin/bash
set -euo pipefail
CLIENT=/src/nostalre-launcher/client
VERSION="$(cat "$CLIENT/switch/VERSION" | tr -d '\r\n')"
test -s /edition/outputs/ragswitch-pre.elf
grep -aFq "$VERSION" /edition/outputs/ragswitch-pre.elf
mkdir -p /edition/outputs/switch/ragswitch-pre
nacptool --create 'RagSWITCH Pre-Renewal' 'RagSWITCH / classic edition' "$VERSION" /edition/outputs/ragswitch-pre.nacp
elf2nro /edition/outputs/ragswitch-pre.elf /edition/outputs/switch/ragswitch-pre/ragswitch-pre.nro \
 --nacp=/edition/outputs/ragswitch-pre.nacp --icon=/opt/devkitpro/libnx/default_icon.jpg \
 --romfsdir="$CLIENT/switch/romfs"
echo CLASSIC_NRO_BUILT
