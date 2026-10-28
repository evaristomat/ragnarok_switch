"""Keep GM destinations represented by the active classic NPC/warp/spawn scripts."""
from pathlib import Path
import json, re
ROOT = Path(__file__).resolve().parent.parent
SERVER = ROOT / 'source/nostalre-launcher/server'
catalog = ROOT / 'source/nostalre-launcher/client/lib/ui-component/src/menu/travel_catalog.rs'
visited, maps = set(), set()
def visit(relative):
    if relative in visited: return
    visited.add(relative)
    path = SERVER / relative
    if not path.is_file(): return
    for line in path.read_text(encoding='cp1252', errors='replace').splitlines():
        active = line.split('//',1)[0].strip()
        include = re.match(r'(?:npc|import):\s*(\S+)', active)
        if include:
            visit(include[1])
        else:
            match = re.match(r'^([a-z0-9_@-]+)(?:,\d+[^\t]*)?\t(?:script|warp|monster|boss_monster|duplicate\()', active)
            if match: maps.add(match[1])
visit('npc/pre-re/scripts_main.conf')
assert {'prontera', 'new_1-1', 'prt_fild08', 'iz_dun02'} <= maps
text = catalog.read_text(encoding='utf-8')
blocks = re.findall(r'    TravelGroup \{\n(.*?)(?=\n    TravelGroup \{|\n\];)', text, re.S)
kept = []
old_count = 0
for block in blocks:
    category = int(re.search(r'category: (\d+)', block)[1])
    label = re.search(r'label: "([^"]+)"', block)[1]
    destinations = []
    pattern = r'Destination \{\s*label: "([^"]+)",\s*map: "([^"]+)",\s*x: (\d+),\s*y: (\d+),\s*go: (None|Some\(\d+\)),'
    for name, map_name, x, y, go in re.findall(pattern, block):
        old_count += 1
        if map_name in maps:
            destinations.append(dict(label=name, map=map_name, x=int(x), y=int(y), go=go))
    if destinations: kept.append(dict(category=category, label=label, destinations=destinations))
assert len(kept) > 20, 'Unexpectedly empty classic catalog'
lines = ['// RagSWITCH Pre-Renewal: destinations from active classic NPC/warp/spawn scripts.',
    '// Original Warper by Euphy and rAthena contributors, GPL.',
    'use super::travel::{Destination, TravelGroup};',
    'pub(super) static GROUPS: &[TravelGroup] = &[']
for group in kept:
    lines += ['    TravelGroup {', f'        category: {group["category"]},', f'        label: {json.dumps(group["label"], ensure_ascii=False)},', '        destinations: &[']
    for d in group['destinations']:
        lines += ['            Destination {', f'                label: {json.dumps(d["label"], ensure_ascii=False)},', f'                map: "{d["map"]}",', f'                x: {d["x"]}, y: {d["y"]}, go: {d["go"]},', '            },']
    lines += ['        ],', '    },']
lines += ['];', '']
catalog.write_text('\n'.join(lines), encoding='utf-8', newline='\n')
(ROOT / 'evidence/classic-travel.json').write_text(json.dumps(dict(
    active_npc_files=len(visited), classic_maps=len(maps), old_destinations=old_count,
    retained_destinations=sum(len(g['destinations']) for g in kept), groups=kept), indent=2, ensure_ascii=False), encoding='utf-8')
print('CLASSIC_TRAVEL_READY', len(kept), 'groups;', old_count - sum(len(g['destinations']) for g in kept), 'unavailable destinations removed')
