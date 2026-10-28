"""Actual graphical client + actual offline world on a disposable save under Xvfb/Mesa.

This verifies UI/render/game integration on the PC; it cannot approve Switch FPS.
Run with the kRO installation mounted read-only at /game.
"""
import contextlib
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import socket
import sqlite3
import subprocess
import time

source = Path('/edition/evidence/world/test-world-save.db')
gold = hashlib.sha256(source.read_bytes()).hexdigest()
checkout_config = Path('/src/nostalre-launcher/client/client/config.json')
checkout_config_before = hashlib.sha256(checkout_config.read_bytes()).hexdigest() if checkout_config.exists() else None
root = Path('/tmp/ragswitch-pre-graphics')
if root.exists(): shutil.rmtree(root)
shutil.copytree('/src/nostalre-launcher/client/switch/romfs/server', root)
shutil.copy2(source, root / 'ragnarok.db')
# Require a new successful equip; an item already equipped in the captured save
# must not make this test pass when the scripted equip is actually refused.
with contextlib.closing(sqlite3.connect(root / 'ragnarok.db')) as db:
    assert db.execute('SELECT count(*) FROM char WHERE char_id=150000').fetchone()[0] == 1, 'Run the successful world test before graphics'
    db.execute("UPDATE char SET class=0,base_level=1,job_level=1,base_exp=0,job_exp=0,skill_point=0,last_map='new_1-1',last_x=53,last_y=111 WHERE char_id=150000")
    db.execute('UPDATE inventory SET equip=0 WHERE char_id=150000 AND nameid=1201')
    # Model an existing pre3 character who has never received the new gift.
    db.execute('DELETE FROM inventory WHERE char_id=150000 AND nameid=12887')
    db.execute("DELETE FROM char_reg_num WHERE char_id=150000 AND key='RS_FlyWingGift'")
    db.commit()
    assert db.execute('SELECT count(*) FROM inventory WHERE char_id=150000 AND nameid=1201 AND equip!=0').fetchone()[0] == 0
evidence = Path('/edition/evidence/graphics'); evidence.mkdir(parents=True, exist_ok=True)
runtime = root / 'runtime'; runtime.mkdir(mode=0o700)
profile = root / 'session.bin'
config = json.loads(Path('/src/nostalre-launcher/client/switch/config.json').read_text(encoding='utf-8'))
assert config['solo']['char_id'] is None
config['solo']['char_id'] = 150000
assets = Path('/edition/runtime/switch/ragswitch-pre/game-data')
config.update(grf_paths=[str(assets/'ragswitch-assets.grf'), str(assets/'data.grf'), str(assets / 'official_data.grf')],
              data_dir=None, install_dir=str(assets),
              bgm_enabled=False, sfx_enabled=False, fullscreen=False)
config['hud_layout'] = 'Classic'  # Reproduce a pre4 config trapped outside our menu.
(root / 'config.json').write_text(json.dumps(config), encoding='utf-8')
(evidence / 'test-config.json').write_text(json.dumps(config, indent=2), encoding='utf-8')
script = root / 'window-script.txt'
script.write_text('''wait 2
chat @populate off
wait 1
shot CLASSIC-GROUNDS
wait 1
shot MAP-BEGIN
walk -2 0
wait 1
walk 2 0
wait 1
walk -2 0
wait 1
walk 2 0
wait 1
chat @warp prontera 147 89
wait_map prontera
wait 1
chat @item 1201 1
wait 1
equip 1201
wait 1
chat @monster PORING 1
wait 1
attack
wait 3
shot COMBAT-END
chat @warp prontera 147 89
wait 1
talk
wait 2
shot NPC
chat @warp prontera 150 180
chat @populate on
wait 24
shot CLASSIC-BOTS
wait 1
charselect
wait 2
key enter
wait 2
walk -1 0
wait 1
walk 1 0
wait 1
charselect
wait 2
escape
wait 1
quit
''', encoding='utf-8')
if os.environ.get('NOSTALRE_NPC_QUIT_VALIDATION') == '1':
    scenario = script.read_text(encoding='utf-8')
    assert scenario.endswith('escape\nwait 1\nquit\n')
    script.write_text(scenario.removesuffix('escape\nwait 1\nquit\n') +
                     'key enter\nwait 2\nchat @warp izlude 113 179\nwait_map izlude\nwait 1\ntalk\nwait 1\nquit\n', encoding='utf-8')
if os.environ.get('NOSTALRE_MAP_MEMORY_TEST') == '1':
    # Reproduce the actual ship -> Izlude transition after effect preload.
    # Use only the disposable save; the physical console is never touched.
    route = 'wait 80\n'
    for _ in range(3):
        route += 'chat @warp izlude_c 122 100\nwait_map izlude_c\nwait 2\nchat @warp int_land03 81 104\nwait_map int_land03\nwait 2\n'
    script.write_text(route + script.read_text(encoding='utf-8'), encoding='utf-8')
env = dict(os.environ, RA_KEEP_CWD='1', RA_SQLITE_DIR=str(root),
           NOSTALRE_TEST_SESSION_PATH=str(profile), NOSTALRE_OFFLINE_SESSION=str(profile),
           NOSTALRE_UI_CONFIG=str(root / 'config.json'), NOSTALRE_SCRIPT=str(script),
           CARGO_TARGET_DIR='/test-build/client-host', DISPLAY=':99',
           WINIT_UNIX_BACKEND='x11', XDG_RUNTIME_DIR=str(runtime),
           LIBGL_ALWAYS_SOFTWARE='1', VK_ICD_FILENAMES='/usr/share/vulkan/icd.d/lvp_icd.x86_64.json')
env.update(NOSTALRE_PT_BR='1', NOSTALRE_UI_LAYOUT_PREVIEW='1',
           NOSTALRE_CLASSIC_PORT_VALIDATION='1',
           NOSTALRE_GAMEPLAY_VALIDATION=str(evidence / 'classic-port-checks.json'))
workers = []; streams = []; client = None; shots = set()
expected_shots = {'MAP-BEGIN', 'NPC', 'COMBAT-END', 'CLASSIC-GROUNDS', 'CLASSIC-BOTS'}
expected_shots |= {'CLASSIC-NPC-NAMES', 'CLASSIC-JOURNAL-PTBR'}
expected_shots |= {'CLASSIC-MENU-JOBCHANGER', 'CLASSIC-JOBCHANGER-REQUIREMENTS'}
expected_shots |= {'CLASSIC-JOBCHANGER-SUCCESS'}
expected_shots |= {'CLASSIC-INFINITE-WING'}
expected_shots |= {'CLASSIC-RECEPTION','CLASSIC-CONTROLLER-TARGET','CLASSIC-MERCHANT-SHOP'}
expected_shots |= {'CLASSIC-GROUND-TARGET'}
if os.environ.get('NOSTALRE_QUEST_GUIDE_VALIDATION') == '1':
    expected_shots |= {'QUEST-ACTIVE', 'QUEST-RETURN', 'QUEST-MENU'}
if os.environ.get('NOSTALRE_QUEST_FLOW_VALIDATION') == '1':
    expected_shots |= {'QUEST-RECEIPT-PTBR','QUEST-DELIVERY-PTBR','QUEST-ACADEMY-PTBR'}


def start(name, command, cwd=root):
    stream = (evidence / f'{name}.log').open('wb'); streams.append(stream)
    process = subprocess.Popen(command, cwd=cwd, env=env, stdout=stream, stderr=stream)
    workers.append(process); return process


def ready(port, seconds=60):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        assert all(worker.poll() is None for worker in workers), 'A test worker exited during initialization'
        try:
            with socket.create_connection(('127.0.0.1', port), timeout=.1): return
        except OSError: time.sleep(.05)
    raise AssertionError(f'Local port {port} unavailable')


try:
    server_directory = os.environ.get('NOSTALRE_GRAPHICAL_SERVER_DIR', '/host-build/bin')
    start('char', [server_directory + '/char-server']); ready(6121)
    start('map', [server_directory + '/map-server']); ready(5121, 90)
    deadline = time.monotonic() + 90
    while 'Map-Server 0 connected:' not in (evidence / 'char.log').read_text(encoding='utf-8', errors='replace'):
        assert time.monotonic() < deadline, 'Map registration did not complete'
        time.sleep(.05)
    start('xvfb', ['Xvfb', ':99', '-screen', '0', '1280x720x24', '-nolisten', 'tcp'])
    deadline = time.monotonic() + 40
    while not Path('/tmp/.X11-unix/X99').exists():
        assert time.monotonic() < deadline; time.sleep(.05)
    # Cargo normally runs library tests in the package's source directory.
    # Build first, then run the exact reported executable in the disposable root.
    build = subprocess.run(['cargo', '+nightly', 'test', '--release', '--locked', '--config', '.cargo/switch.toml',
                            '-p', 'nostalre-client', '--lib', '--no-run', '--message-format=json-render-diagnostics', '-j', '1'],
                           cwd='/src/nostalre-launcher/client', env=env, capture_output=True, timeout=600)
    (evidence / 'build.jsonl').write_bytes(build.stdout)
    (evidence / 'build.log').write_bytes(build.stderr)
    assert build.returncode == 0, 'Graphical test build failed'
    executables = []
    for line in build.stdout.decode('utf-8', errors='replace').splitlines():
        try: message = json.loads(line)
        except json.JSONDecodeError: continue
        if message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('name') == 'nostalre_client' and message.get('executable'):
            executables.append(message['executable'])
    assert len(executables) == 1, 'Cargo did not identify exactly one client test executable'
    executable = Path(executables[0]).resolve()
    assert executable.is_relative_to(Path('/test-build/client-host'))
    client = start('client', [str(executable), 'full_window_offline_character_map_walk_and_actions',
                              '--ignored', '--nocapture', '--test-threads=1'])
    deadline = time.monotonic() + (330 if os.environ.get('NOSTALRE_GAMEPLAY_VALIDATION') else 240)
    while client.poll() is None:
        assert time.monotonic() < deadline, 'Graphical test timed out'
        try:
            text = (evidence / 'client.log').read_text(encoding='utf-8', errors='replace')
        except OSError as error:
            # Docker's Windows bind may temporarily return ENODATA while the App writes.
            if error.errno != 61: raise
            time.sleep(.05)
            continue
        for shot in sorted(expected_shots):
            if shot not in shots and f'script shot {shot}' in text:
                raw = evidence / f'{shot}.xwd'; png = evidence / f'{shot}.png'
                subprocess.run(['xwd', '-root', '-silent', '-out', str(raw)], env=env, check=True, timeout=10)
                subprocess.run(['convert', str(raw), str(png)], env=env, check=True, timeout=10)
                shots.add(shot); print('SCREENSHOT', png.name, flush=True)
        time.sleep(.05)
    assert client.returncode == 0, 'Complete graphical client test failed'
    server_text = (evidence / 'map.log').read_text(encoding='utf-8', errors='replace')
    assert 'failed npc_checknear' not in server_text, 'Remote NPC conversation failed its server distance check'
    assert 'script error' not in server_text.lower(), 'Script error in the classic world'
    if os.environ.get('NOSTALRE_NPC_QUIT_VALIDATION') == '1':
        text = (evidence / 'client.log').read_text(encoding='utf-8', errors='replace')
        assert 'NPC-EXIT-CHECK dialog_open=true request=QuitGame' in text, 'Quit did not begin with a real NPC dialog open'
        assert 'NPC-EXIT-CHECK disconnect_allowed=true' in text, 'The real server did not acknowledge exit'
    if os.environ.get('NOSTALRE_MAP_MEMORY_TEST') == '1':
        text = (evidence / 'client.log').read_text(encoding='utf-8', errors='replace')
        assert text.count('Map: izlude_c ') >= 3, 'The actual crash destination was not loaded three times'
        assert text.count('Map: int_land03 ') >= 4, 'Ship return route did not complete'
    assert shots == expected_shots, 'Graphical script did not complete all visual scenarios'
finally:
    for process in reversed(workers):
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
            try: process.wait(timeout=20)
            except subprocess.TimeoutExpired: process.kill(); process.wait()
    for stream in streams: stream.close()
    with contextlib.closing(sqlite3.connect(root / 'ragnarok.db')) as db:
        integrity = db.execute('PRAGMA quick_check').fetchone()[0]
        preserved = db.execute('SELECT count(*) FROM char WHERE char_id=150000').fetchone()[0] == 1
        equipped = db.execute('SELECT count(*) FROM inventory WHERE char_id=150000 AND nameid=1201 AND equip!=0').fetchone()[0] > 0
        wing_rows=db.execute('SELECT amount,expire_time FROM inventory WHERE char_id=150000 AND nameid=12887').fetchall()
        assert wing_rows == [(1,0)], wing_rows
        if os.environ.get('NOSTALRE_QUEST_FLOW_VALIDATION') == '1':
            states=dict(db.execute('SELECT quest_id,state FROM quest WHERE char_id=150000 AND quest_id IN (7474,7475,7476,7477,4269)').fetchall())
            tutorial_flow=dict(tour_completed=states.get(7474)=='2',
                               visit_receipts_erased=all(id not in states for id in [7475,7476,7477]),
                               academy_registered=states.get(4269)=='2')
    report = dict(graphical_test_exit=None if client is None else client.poll(), screenshots=sorted(shots),
                  database_quick_check=integrity, original_avatar_preserved=preserved, knife_equipped=equipped,
                  knife_initially_unequipped_in_disposable_fixture=True,
                  infinite_wing_quantity_after_reentry=1, infinite_wing_has_no_expiration=True,
                  test_source_save_unchanged=hashlib.sha256(source.read_bytes()).hexdigest() == gold,
                  graphics_backend='Mesa software Vulkan on Xvfb (PC)', switch_fps_approved=False)
    report['source_checkout_config_preserved'] = (hashlib.sha256(checkout_config.read_bytes()).hexdigest() if checkout_config.exists() else None) == checkout_config_before
    if os.environ.get('NOSTALRE_QUEST_FLOW_VALIDATION') == '1':
        report['tutorial_flow_database_checks']=tutorial_flow
    report['minimap_original_and_floor_plan_assertions_passed'] = client is not None and client.poll() == 0
    if env.get('NOSTALRE_GAMEPLAY_VALIDATION'):
        gameplay_path = Path(env['NOSTALRE_GAMEPLAY_VALIDATION'])
        gameplay = json.loads(gameplay_path.read_text(encoding='utf-8')) if gameplay_path.exists() else {}
        report['extended_gameplay_finished'] = gameplay.get('finished', False)
        report['extended_regression_failures'] = [case['name'] for case in gameplay.get('checks', []) if case['kind'] == 'regression' and not case['passed']]
        report['extended_proposal_failures'] = [case['name'] for case in gameplay.get('checks', []) if case['kind'] == 'proposal' and not case['passed']]
    (evidence / 'report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
assert integrity == 'ok' and preserved and equipped and report['test_source_save_unchanged'] and report['source_checkout_config_preserved']
if os.environ.get('NOSTALRE_QUEST_FLOW_VALIDATION') == '1':
    assert all(tutorial_flow.values()), f'Actual tutorial script progression failed: {tutorial_flow}'
if env.get('NOSTALRE_GAMEPLAY_VALIDATION'):
    assert report['extended_gameplay_finished'], 'Extended gameplay did not reach its final checkpoint'
    assert not report['extended_regression_failures'], f"Extended gameplay failed: {report['extended_regression_failures']}"
print('PASS: actual graphical App entered the world, walked, warped, showed NPC and equipped knife; source save preserved. Switch FPS unapproved.', flush=True)
