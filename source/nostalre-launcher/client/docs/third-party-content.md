# Bundled content and attribution

Review date: 2026-10-23. Implementations are retained by the user's decision.
Executable analysis alone is not a content-removal finding. No game code or
lookup implementations were changed during this review.

## Verified content

| Content | Result |
| --- | --- |
| Game archives, official client executables, music | None in the source export; local game data/config/builds remain ignored. |
| README demonstration photo | User supplied on 2026-10-23; downscaled to docs/images/android-handheld.webp with metadata stripped, for documentation. |
| Seventeen compiled Lua fixtures | Table contents match the included authored Lua sources, using the client's parser with the required seed tables. |
| Six generated text fixtures | Match make_fixtures.py output exactly, including synthetic TEST entries. |
| Three Noto fonts | Byte-for-byte copies of upstream baseline b4ec4a4; embedded copyright statements transcribed into FONT-NOTICES.txt, alongside the existing OFL text. |
| Four charm PNGs | Integration commits b2ff3940 and 85fe908d identify these as the user's own artwork; recorded in lib/effects/assets/README.md. Original large drawings are outside the export. |
| winit example images and xsettings fixture | Retained within the vendored winit source tree with its license. |
| Built-in accessory, sprite-name, and pet tables | Inherited resource identifier/name mappings; retained with upstream attribution. |
| Sign-board text | Short lookup keys/labels with locally authored English translations; retained. No large dialogue or narrative dump was identified in this file. |
| Client-message dump | Removed earlier, including its module declaration. No old Git history is imported. |

The fixture comparison checks table contents, rather than claiming every debug
record in compiled chunks is byte-identical to a newly compiled file.

## Notices supplied

- Upstream client: Apache-2.0 license and NOTICE.
- rust-ro: retained MIT license and baseline/modification notice.
- winit: retained Apache-2.0 license and Android modification notice.
- W3C keyboard definitions inside winit: full embedded license/short notice
  also reproduced in LICENSES/winit-W3C.txt.
- Incorporated Korangar code and linked Lua: full MIT notices.
- Fonts: OFL text plus copyright statements from the actual bundled metadata.
- AzzyAI: restored the inherited credit and documented the permission question below.
- rAthena-derived output: source credit, version evidence, and full GPLv3 text;
  distributed under GPL-3.0-or-later (below).

## Open question

### AzzyAI-derived tactic defaults

`lib/ai/src/tactics.rs` says the default tables are ported from reference
homunculus/mercenary/PVP tables. Its distinctive monster IDs, tactic settings,
and weights closely match [AzzyAI H_Tactics.lua](https://github.com/SpenceKonde/AzzyAI/blob/master/USER_AI/H_Tactics.lua),
which supports an adaptation origin rather than inspiration alone.
Upstream's README also credits AzzyAI.

AzzyAI's [AI.lua](https://github.com/SpenceKonde/AzzyAI/blob/master/USER_AI/AI.lua)
and [AI_main.lua](https://github.com/SpenceKonde/AzzyAI/blob/master/USER_AI/AI_main.lua)
contain notices permitting unmodified redistribution; the former asks people
who wish to modify it to contact the author. No separate permission covering
this Rust adaptation was found in the supplied repository evidence. Those
notices are evidence of a permission question, not a determination of the legal
status of individual numeric settings. Added credit alone does not settle it.
Next step: establish applicable permission for the inherited defaults, or
replace those defaults with independently chosen settings if permission is unavailable.
The implementation and defaults remain unchanged.

## Resolved: generated rAthena datasets

Files:

- lib/game/src/item_tables.rs
- lib/game/src/job_tables.rs
- lib/game/src/skill_tree_tables.rs
- rust-ro/models/src/enums/effect_id.rs
- rust-ro/models/src/enums/skill_enums.rs
- lib/network/data/rathena-20250716.tsv
- lib/network/data/rathena-20250716-layout.tsv

The first five match the integration generators against its rAthena checkout
exactly, after the client crate-name adjustment. The TSV headers identify their
rAthena packet-table/layout origin. The integration sync commit 3e629823 records
upstream d4b8e7b plus local server changes. Input fingerprints are recorded in
[rathena-inputs.tsv](rathena-inputs.tsv); the list covers the named primary
inputs, rather than every transitive compiler include.

Confirmed 2026-10-24 against the integration's rAthena checkout: its LICENSE
is GNU GPL version 3, the database headers grant GPL-3.0-or-later, and the source
headers state "Licensed under GNU GPL" with reference to that LICENSE. The local
input hashes match [rathena-inputs.tsv](rathena-inputs.tsv). Full GPLv3 text is
supplied in LICENSES/rathena-GPL-3.0.txt.

Decision: the seven files above are treated as carrying rAthena content and are
distributed under GPL-3.0-or-later. Because they compile into the client, released
binaries (including the APK) are distributed under GPL-3.0-or-later as a whole;
Apache-2.0 and MIT code is compatible with GPLv3 in that direction. The rest of
the source keeps its own license. This repository supplies the corresponding
client source. The datasets and implementations remain unchanged.

References: [GNU output FAQ](https://www.gnu.org/licenses/gpl-faq.html#GPLOutput),
[OFL conditions](https://openfontlicense.org/open-font-license-official-text/).
