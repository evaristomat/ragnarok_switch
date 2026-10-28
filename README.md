# RagSWITCH Pre-Renewal

**English** | [Português brasileiro](docs/README.pt-BR.md)

An offline Ragnarok project for Nintendo Switch, with an embedded local server, bots, and a controller-focused interface. This edition has its own source tree, saves, and maintenance, independent of the Renewal edition.

**Status: 0.1.0-pre12 — experimental, undergoing manual playtesting.** The full campaign and performance on Nintendo Switch have not yet been validated. This repository provides source code; players must supply their own game assets under the applicable permissions.

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
