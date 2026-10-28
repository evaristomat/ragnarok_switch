"""Real protocol, classic progression, population and save-isolation checks.

All writes are inside a disposable world; no Renewal source or save is mounted.
"""
from pathlib import Path
import contextlib, hashlib, json, os, re, shutil, signal, socket, sqlite3, subprocess, tempfile, time

ROOT = Path('/edition')
PAYLOAD = Path('/src/nostalre-launcher/client/switch/romfs/server')
EVIDENCE = ROOT / 'evidence/world'
EVIDENCE.mkdir(parents=True, exist_ok=True)
WORLD = Path(tempfile.mkdtemp(prefix='ragswitch-pre-world-'))
SEED_HASH = hashlib.sha256((PAYLOAD / 'ragnarok.db').read_bytes()).hexdigest()
REPORT = {'passed': False, 'installed_on_switch': False, 'native_switch_fps_tested': False}

def connect(path): return contextlib.closing(sqlite3.connect(path, timeout=15))
def environment(world):
    return dict(os.environ, RA_KEEP_CWD='1', RA_SQLITE_DIR=str(world),
        NOSTALRE_TEST_SESSION_PATH=str(world / 'session.bin'),
        NOSTALRE_OFFLINE_SESSION=str(world / 'session.bin'), NOSTALRE_PT_BR='1')

# Unknown/mismatched saves must be refused before a server can modify them.
wrong = Path(tempfile.mkdtemp(prefix='ragswitch-wrong-edition-'))
shutil.copy2(PAYLOAD / 'ragnarok.db', wrong / 'ragnarok.db')
with connect(wrong / 'ragnarok.db') as db:
    db.execute("UPDATE edition_identity SET edition='renewal'")
    db.commit()
before = hashlib.sha256((wrong / 'ragnarok.db').read_bytes()).hexdigest()
bad = subprocess.run(['/host-build/bin/char-server'], cwd=wrong, env=environment(wrong), capture_output=True, timeout=20)
(EVIDENCE / 'wrong-edition.log').write_bytes(bad.stdout + bad.stderr)
assert bad.returncode == 91 and b'Pre-Renewal' in bad.stderr, (bad.returncode, bad.stderr)
assert hashlib.sha256((wrong / 'ragnarok.db').read_bytes()).hexdigest() == before
REPORT['wrong_edition_save_refused_without_writes'] = True

shutil.copytree(PAYLOAD, WORLD, dirs_exist_ok=True)
audit = '-\tscript\tClassicAudit\t-1,{\nOnInit:\n'
for key in ['base_exp_rate', 'job_exp_rate', 'quest_exp_rate', 'item_rate_card', 'item_rate_common']:
    audit += f'debugmes "CLASSIC_AUDIT|{key}|"+getbattleflag("{key}");\n'
for index in range(6):
    audit += f'debugmes "CLASSIC_AUDIT|renewal_{index}|"+checkre({index});\n'
audit += 'debugmes "CLASSIC_AUDIT|poring_base|"+getmonsterinfo(1002,MOB_BASEEXP);\n'
audit += 'debugmes "CLASSIC_AUDIT|poring_job|"+getmonsterinfo(1002,MOB_JOBEXP);\nend;\n}\n'
(WORLD / 'npc/custom/classic-audit.txt').write_text(audit)
with (WORLD / 'npc/scripts_custom.conf').open('a') as stream:
    stream.write('\nnpc: npc/custom/classic-audit.txt\n')

workers, streams = [], []
env = environment(WORLD)
def ready(port):
    deadline = time.monotonic() + 150
    while time.monotonic() < deadline:
        assert all(p.poll() is None for p in workers), 'server exited during startup'
        try:
            with socket.create_connection(('127.0.0.1', port), timeout=.2): return
        except OSError: time.sleep(.2)
    raise AssertionError(f'port {port} not ready')

try:
    for role, port in [('char', 6121), ('map', 5121)]:
        stream = (EVIDENCE / f'{role}.log').open('wb'); streams.append(stream)
        workers.append(subprocess.Popen([f'/host-build/bin/{role}-server'], cwd=WORLD, env=env, stdout=stream, stderr=stream))
        ready(port)
    deadline = time.monotonic() + 90
    while 'Map-Server 0 connected:' not in (EVIDENCE / 'char.log').read_text(errors='replace'):
        assert time.monotonic() < deadline, 'map not registered'
        assert all(p.poll() is None for p in workers)
        time.sleep(.2)
    print('CLASSIC_SERVERS_READY', flush=True)
    time.sleep(3)
    initial_map_log = (EVIDENCE / 'map.log').read_text(errors='replace')
    audit_values = dict(re.findall(r'CLASSIC_AUDIT\|([^|\s]+)\|(\d+)', initial_map_log))
    assert all(audit_values[f'renewal_{n}'] == '0' for n in range(6)), audit_values
    assert all(audit_values[key] == '1500' for key in ['base_exp_rate','job_exp_rate','quest_exp_rate','item_rate_card','item_rate_common']), audit_values
    assert audit_values['poring_base'] == '30' and audit_values['poring_job'] == '15', audit_values
    REPORT['real_server_audit'] = audit_values

    # Fresh creation must begin at the classic grounds, then a real NPC grants first class.
    args = ['/test-build/probe/release/nostalre-offline-probe', '--create', 'ClassicTester', '--seconds', '0', '--dump', 'hat']
    steps = [
        'wait:1', 'say:@warp prontera 149 180', 'wait:1',
        'say:@joblvup 9', 'say:@skillpoint -9999', 'wait:1', 'talk:150,180',
        'wait:1', 'say:@jobchange 4008', 'say:@lvup 999', 'say:@joblvup 999',
        'say:@jobchange 4054', 'say:@jobchange 4252', 'say:@jobchange 4218',
        'wait:24', 'say:@populate stats',
        'say:@warp prt_fild08 170 180', 'wait:8', 'say:@populate stats',
        'say:@recallall', 'wait:8', 'say:@populate stats',
        'say:@doom', 'wait:8', 'say:@populate stats',
        'say:@warp prontera 149 180', 'wait:24', 'say:@populate stats',
        'say:@populate off', 'wait:4', 'say:@populate on', 'wait:24', 'say:@populate stats',
    ]
    for step in steps: args += ['--social', step]
    with (EVIDENCE / 'probe.log').open('wb') as out:
        probe = subprocess.run(args, cwd=WORLD, env=env | {'NOSTALRE_PROBE_NPC_CHOICES': '1,1'}, stdout=out, stderr=subprocess.STDOUT, timeout=260)
    text = (EVIDENCE / 'probe.log').read_text(errors='replace')
    assert probe.returncode == 0 and text.rstrip().endswith('clean'), text[-5000:]
    assert 'new_1-1' in text, 'fresh character did not enter the classic grounds'
    assert 'Sua nova classe' in text, 'classic first-class NPC was not exercised'
    rows = [tuple(map(int,row)) for row in re.findall(r'Active / created / errors : (\d+) / (\d+) / (\d+)', text)]
    assert [r[0] for r in rows] == [50,20,20,20,50,50] and all(r[2] == 0 for r in rows), rows
    assert rows[1][1] == rows[2][1] == rows[3][1], 'recall/respawn replaced shell identities'
    town_roles=[tuple(map(int,row)) for row in re.findall(r'Cidade: lojas=(\d+) salas=(\d+) repouso=(\d+) passeio=(\d+) andando=(\d+)',text)]
    assert len(town_roles)==6 and all(row[:4]==(8,6,24,12) for row in [town_roles[0],town_roles[4],town_roles[5]]),town_roles
    assert all(row[4]<=4 for row in town_roles),town_roles
    time.sleep(5)
    with connect(WORLD / 'ragnarok.db') as db:
        saved = db.execute('SELECT class,base_level,job_level,online,last_map FROM char WHERE name=?', ('ClassicTester',)).fetchone()
        assert saved[:4] == (4008,99,70,0), saved
        assert db.execute('SELECT count(*) FROM char').fetchone()[0] == 1, 'bots leaked into the save'
        assert db.execute('PRAGMA quick_check').fetchone()[0] == 'ok'
        assert db.execute('SELECT amount,expire_time FROM inventory WHERE char_id=? AND nameid=12887', (150000,)).fetchall() == [(1,0)]
    map_log = (EVIDENCE / 'map.log').read_text(errors='replace')
    bot_rows = [tuple(map(int, row)) for row in re.findall(r'\[ClassicBot\] job=(\d+) base=(\d+) joblevel=(\d+) maxjob=(\d+)', map_log)]
    allowed_jobs = set(json.loads((ROOT / 'edition.json').read_text())['classic_job_ids'])
    assert bot_rows and all(job in allowed_jobs and 1 <= base <= 99 and 1 <= level <= limit for job, base, level, limit in bot_rows), bot_rows
    counts = [int(x) for x in re.findall(r'\[Population\] active=(\d+) cap=50', map_log)]
    assert counts[-1] == 0 and max(counts) == 50, counts
    respawns = re.findall(r'\[PopulationTest\] respawn id=(\d+) alive=(\d) placed=(\d)', map_log)
    assert len(respawns) >= 20 and all(a == p == '1' for _,a,p in respawns), respawns
    assert 'script error' not in map_log.lower(), 'script errors in classic payload'
    REPORT.update(passed=True, new_character_starts_at_classic_grounds=True,
        first_class_npc_verified=True, modern_classes_refused=True, final_class_base_job=list(saved[:3]),
        population_counts=[r[0] for r in rows], population_errors=[r[2] for r in rows],
        classic_bot_class_and_level_samples=len(bot_rows),
        recall_respawn_preserve_bots=True, logout_removes_bots=True, no_bots_saved_as_characters=True,
        database_quick_check='ok')
    REPORT['native_town_roles_samples']=town_roles
    REPORT['new_character_received_permanent_infinite_wing']=True
finally:
    for worker in reversed(workers):
        if worker.poll() is None:
            worker.send_signal(signal.SIGTERM)
            try: worker.wait(timeout=25)
            except subprocess.TimeoutExpired: worker.kill(); worker.wait()
    for stream in streams: stream.close()
    REPORT['server_exit_codes'] = [p.returncode for p in workers]
    REPORT['seed_unchanged'] = hashlib.sha256((PAYLOAD / 'ragnarok.db').read_bytes()).hexdigest() == SEED_HASH
    if (WORLD / 'ragnarok.db').is_file(): shutil.copy2(WORLD / 'ragnarok.db', EVIDENCE / 'test-world-save.db')
    (EVIDENCE / 'report.json').write_text(json.dumps(REPORT, indent=2))
assert REPORT['passed'] and REPORT['seed_unchanged'] and REPORT['server_exit_codes'] == [0,0]
print('PASS: classic formulas/creation/classes/99-70, 50/20 bots, recall/respawn/logout and independent save.', flush=True)
