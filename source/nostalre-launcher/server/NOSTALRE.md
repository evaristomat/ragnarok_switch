# nostalRE fork of rAthena

This fork carries the rAthena changes used by
[nostalRE](https://github.com/si-klyde/nostalre). Its
[launcher](https://github.com/si-klyde/nostalre-launcher) takes this branch as its
`server/` submodule and builds the Android app and the desktop servers from it
(`scripts/build-server.sh`, `scripts/build-apk.sh`). The app bundles
login, char and map servers built from this source, together with this tree's
`conf/`, `db/` and `npc/` folders and a SQLite database made from `sql-files/`.

rAthena is licensed under the GNU General Public License version 3
([LICENSE](LICENSE)). These changes are distributed under the same terms.

## Baseline and modifications

- Upstream: [rathena/rathena](https://github.com/rathena/rathena) at
  `d4b8e7b8f16061cc2496d377ac8f777ce72a4f39` (2026-10-06).
- Modified between 2026-10-08 and 2026-10-20 in the Ragnarok Solo integration
  project. The `nostalre` branch's first commit after the baseline holds every
  change; `git diff d4b8e7b8 -- <path>` shows any file's modifications.
- Most behavior changes carry a `Ragnarok Solo` comment.

Summary:

- Optional embedded SQLite backend instead of MySQL: `ENABLE_SQLITE` CMake
  option, `src/common/sql_sqlite.cpp`, the SQLite amalgamation in
  `3rdparty/sqlite` (public domain), `tools/mysql2sqlite.py`, and the
  `sql-compat-test` tool.
- Query fixes for SQLite operator precedence and trailing-space login IDs.
- `RA_KEEP_CWD` keeps the working directory when binaries live apart from
  `conf/`; overridable CMake output directories for cross builds.
- Monster HP bar for every packet version; packet obfuscation off.
- Hair style limits per sex and race; elemental ATK never negative.
- Single-player configuration: local-only IPs, instant character delete,
  pincode off, MVP EXP message on, homunculus growth message off.
- NPC and warp positions for current client map data (Renewal Izlude, no
  airship docks).

## Building the Android servers

The app's servers are built for Renewal at packet version 20250716, with the
Android NDK (r30, `30.0.16248370`), CMake and Ninja:

```sh
NDK="$ANDROID_NDK_HOME"
cmake -S . -B build-android -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DENABLE_SQLITE=ON -DENABLE_WEB_SERVER=OFF -DPACKETVER=20250716 \
  -DCMAKE_TOOLCHAIN_FILE="$NDK/build/cmake/android.toolchain.cmake" \
  -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-26 \
  -DCMAKE_FIND_ROOT_PATH_MODE_INCLUDE=BOTH \
  -DHAVE_TLS_EXITCODE=0 -DHAVE_MONOTONIC_CLOCK_EXITCODE=0 \
  -DRT_LIBRARY="$NDK/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/aarch64-linux-android/26/libc.so" \
  -DCMAKE_RUNTIME_OUTPUT_DIRECTORY="$PWD/build-android/bin"
ninja -C build-android login-server char-server map-server
for exe in login-server char-server map-server; do
  "$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" "build-android/bin/$exe"
done
```

The app ships them renamed as `libloginserver.so`, `libcharserver.so` and
`libmapserver.so`.

Server data and database, as packed into the app:

```sh
mkdir -p stage
git ls-files -z conf db npc | tar --null -T - -cf - | tar -xf - -C stage
for t in conf/import-tmpl conf/msg_conf/import-tmpl db/import-tmpl; do
  [ -d "stage/${t%-tmpl}" ] || cp -r "stage/$t" "stage/${t%-tmpl}"
done
(cd stage && python3 -m zipfile -c ../server.zip conf db npc)
python3 tools/mysql2sqlite.py sql-files/main.sql sql-files/logs.sql \
  sql-files/web.sql sql-files/roulette_default_data.sql | sqlite3 -bail ragnarok.db
```

At runtime the app sets `RA_KEEP_CWD=1` and `RA_SQLITE_DIR` to the folder
holding `ragnarok.db`.

A host build uses the same CMake options without the toolchain arguments.

## Tags

- `nostalre-v0.1.0`: source of the servers in nostalRE v0.1.0.
- `nostalre-v0.1.1`: source of the servers in nostalRE v0.1.1 (drops go straight to the inventory).
