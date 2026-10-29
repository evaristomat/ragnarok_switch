# RagSWITCH Pre-Renewal

**English** | [Português brasileiro](docs/README.pt-BR.md)

An offline Ragnarok project for Nintendo Switch, with an embedded local server, bots, and a controller-focused interface. This edition has its own source tree, saves, and maintenance, independent of the Renewal edition.

**Status: 0.1.0-pre12 — experimental, undergoing manual playtesting.** The full campaign and performance on Nintendo Switch have not yet been validated. A prebuilt Switch executable is available below; players must supply their own compatible game assets under the applicable permissions.

## Download and play on Nintendo Switch

**[Download 0.1.0-pre12 (.nro and SHA256)](https://github.com/evaristomat/ragnarok_switch/releases/tag/v0.1.0-pre12)** · **[Guia de instalação em português](docs/INSTALLATION.md)**

The `.nro` is the executable, not a complete game-data bundle. You do not need to compile it, run a PC server, install the Android APK, or download a personal save. You need a Nintendo Switch already configured to run homebrew through [hbmenu](https://github.com/switchbrew/nx-hbmenu), a microSD card, and the following files:

| File or folder | Where to obtain it | Approximate size / purpose |
| --- | --- | --- |
| `ragswitch-pre.nro` | This project's release above | 149 MB; includes the offline server and initial database |
| `game-data/data.grf` | Your compatible kRO **2025-07-16** client; see the upstream [WARP0716 client instructions](https://github.com/CrazyBebop/WARP0716#getting-the-client) | 3.66 GB in the maintainer's setup; base game resources |
| `game-data/official_data.grf` | ROenglishRE's [custom officials instructions](https://llchrisll.github.io/ROTPDocs/misc/custom_officials/) | 783 MB in the maintainer's setup; additional models and sprites |
| `game-data/ragswitch-assets.grf` | [Download from this release](https://github.com/evaristomat/ragnarok_switch/releases/download/v0.1.0-pre12/ragswitch-assets.grf) | 21 MB; required classic maps and edition overrides |
| `game-data/System/`, `SystemEN/`, `data/` | Preserve these folders from the compatible client/data preparation | Supporting tables and loose resources |
| `game-data/BGM/` | Your compatible client's music folder | Optional music |

**Download both project files:** `ragswitch-pre.nro` and `ragswitch-assets.grf` are attached to the release. You still need to obtain the compatible `data.grf`, `official_data.grf`, and supporting client folders separately. The default configuration checks for all three GRFs. Arbitrary modern or private-server clients are not a verified substitute. See [asset requirements](docs/ASSETS.md).

1. Download `ragswitch-pre.nro`, `ragswitch-assets.grf`, and `SHA256SUMS.txt` from the release's **Assets** section, not the automatic **Source code** ZIP. Allow at least 6 GB free on the SD for the base files, plus space for music, extracted server data and saves.
2. Put the executable at `/switch/ragswitch-pre/ragswitch-pre.nro` and the resources above inside `/switch/ragswitch-pre/game-data/`. Keep those folder names; the executable uses this fixed location.
3. Open hbmenu in **application mode**: hold **R** while launching an installed game, then select RagSWITCH Pre-Renewal. Album/applet mode may have insufficient memory.
4. Let the first start extract the embedded server and create your own save. Play offline; no external server account is needed.

For updates, close the game, back up the edition folder, and preserve `ragnarok.db`, its auxiliary files, and `config.json`. See the [full installation and troubleshooting guide](docs/INSTALLATION.md). This release is experimental; installation was verified on the maintainer's Switch, but full hardware gameplay and campaign validation remain pending.

## Personal, unofficial project

**This is a personal, experimental, non-commercial project intended for study, development, and individual offline play. It is not an official Ragnarok Online release.** These goals do not add non-commercial or offline-only restrictions to the existing software licenses.

The maintainer **is not the author of Ragnarok Online** and does not claim authorship or ownership of the original game, its universe, characters, trademarks, music, or assets. Those rights remain with **Gravity Co., Ltd. and the respective rights holders**. The source projects used here retain their own licenses, credits, and authorship.

This project **is not affiliated with, sponsored, approved, or endorsed by Gravity, its licensees/distributors, or Nintendo**. References to Ragnarok Online and Nintendo Switch identify the game and target platform; they do not imply a partnership. This project does not operate an official service, provide support on behalf of those companies, or present itself as a replacement for official services.

The public source export does not provide the commercial game package, GRF archives, or personal saves. Users must obtain and use their own resources in accordance with the applicable permissions and terms. Publishing this source code **does not grant permission to use or redistribute third-party content**.

Read the [full independence, rights, and warranty disclaimer](docs/DISCLAIMER.md), which includes an English summary. The project's personal/offline nature and this notice do not replace licenses or permissions, or guarantee immunity from third-party claims.

## Features

- Pre-Renewal mechanics, a maximum Base Level of 99, and classic first, second, transcendent, and expanded classes. No third/fourth classes or Doram.
- 15x Base EXP, Job EXP, and drop rates; zero SP cost for skills. Edition settings are recorded in [edition.json](edition.json).
- **Echoes of Rune-Midgard** (in-game title: **Ecos de Rune-Midgard**), an original 33-stage solo campaign with HUD objectives and route guidance. It uses existing maps and content. The Job Changer accounts for the character's current class.
- Bots whose progression follows the player, town merchants, and ambient conversations. Population settings: 50 in towns, 20 on hunting maps, with a global cap of 50.
- A custom controller HUD, right-stick camera controls, and monster selection with ZL/ZR. Ground-targeted skills have a separate confirmation step.
- A travel menu, Job Changer, and an Infinite Fly Wing granted when each character first enters a map, including existing characters after updating.
- Brazilian Portuguese interface and game content. NPC names and the Novice Training Grounds received additional translation review in pre12; other dialogue may still need revision.

The client protocol is 20250716 for asset compatibility. This is not a certified recreation of a specific official episode; some maps use modern visuals.

**Language note:** this README is in English. The in-game interface/content and most supporting guides are currently in Brazilian Portuguese.

## Getting started

| Goal | Guide |
| --- | --- |
| Install on Switch and preserve saves | [Installation](docs/INSTALLATION.md) |
| Build from source | [Build instructions and toolchain](docs/BUILDING.md) |
| Supply game assets and translations | [Assets and local data](docs/ASSETS.md) |
| Understand the client, server, bots, and campaign | [Architecture](docs/ARCHITECTURE.md) |
| Test changes and report bugs | [Testing](docs/TESTING.md) and [contributing](CONTRIBUTING.md) |
| Check licenses and unresolved questions | [Licenses and credits](docs/LICENSING.md) |
| Prepare or publish a fork | [GitHub publishing](docs/PUBLISHING.md) |
| Review limitations and recent changes | [Current status](docs/STATUS.md) and [changelog](CHANGELOG.md) |

## Repository layout

~~~text
config/       SQL seed database, with no characters or personal data
campaign/     campaign stage definitions
docs/         public guides and references
scripts/      builds, checks, and tools (see scripts/README.md)
source/       independent client, server, and graphics runtime sources
vendor/       supporting dependencies; the export policy selects public content
toolchain/    test environment recipes and environment identification
.github/      lightweight checks and contribution templates
~~~

Local builds, game assets, reports, and saves live in build/, runtime/, outputs/, and evidence/. Those directories, development memories, and private integration history are excluded from the public export.

## Licensing and provenance

The GPLv3 text is provided in [LICENSE](LICENSE). This project combines components with their own licenses, including GPL, Apache, MIT, and OFL: see [NOTICE](NOTICE) and the [license inventory](docs/LICENSING.md). Original notices have been preserved. No permission to redistribute proprietary game assets is implied.

An inherited permission question concerning AzzyAI-derived default tactic tables remains documented in the [licensing guide](docs/LICENSING.md). Organizing or publishing this repository does not resolve that question; review it before redistributing the affected material.
