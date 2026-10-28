"""Short real-client Comodo memory comparison; disposable save, no campaign journey."""
from pathlib import Path
import contextlib, hashlib, json, os, shutil, signal, socket, sqlite3, subprocess, tempfile, time

root=Path('/edition')
phase=os.environ.get('COMODO_REPORT','before')
assert phase in ('before','after')
out=root/f'evidence/comodo-memory-{phase}';out.mkdir(parents=True,exist_ok=True)
source=root/'evidence/world/test-world-save.db'
gold=hashlib.sha256(source.read_bytes()).hexdigest()
work=Path(tempfile.mkdtemp(prefix='comodo-memory-'))
shutil.copytree(root/'source/nostalre-launcher/client/switch/romfs/server',work,dirs_exist_ok=True)
shutil.copy2(source,work/'ragnarok.db')
with contextlib.closing(sqlite3.connect(work/'ragnarok.db')) as db:
    db.execute("UPDATE char SET class=0,base_level=1,job_level=1,hp=100,last_map='comodo',last_x=195,last_y=150 WHERE char_id=150000")
    db.commit()
assets=root/'runtime/switch/ragswitch-pre/game-data'
config=json.loads((root/'source/nostalre-launcher/client/switch/config.json').read_text())
config['solo']['char_id']=150000
config.update(grf_paths=[str(assets/n) for n in ['ragswitch-assets.grf','data.grf','official_data.grf']],data_dir=None,install_dir=str(assets),bgm_enabled=False,sfx_enabled=False,fullscreen=False)
(work/'config.json').write_text(json.dumps(config))
(work/'session-runtime').mkdir(mode=0o700)
route='wait 2\nchat @populate off\nwait 3\n'
for _ in range(3): route+='walk -4 0\nwait 1\nwalk 4 0\nwait 1\n'
route+='shot COMODO-OFF\nchat @populate on\nwait 24\n'
for _ in range(5):route+='walk -4 0\nwait 1\nwalk 4 0\nwait 1\n'
if os.environ.get('COMODO_CAMERA_TOUR') == '1':
    route+='turn 90\nwait 1\nturn 90\nwait 1\nzoom 2\nwait 1\nturn 90\nwait 1\nzoom -2\nturn 90\nwait 1\n'
route+='shot COMODO-ON\nwait 2\nquit\n'
(work/'route.txt').write_text(route)
env=dict(os.environ,RA_KEEP_CWD='1',RA_SQLITE_DIR=str(work),NOSTALRE_TEST_SESSION_PATH=str(work/'session.bin'),NOSTALRE_OFFLINE_SESSION=str(work/'session.bin'),NOSTALRE_UI_CONFIG=str(work/'config.json'),NOSTALRE_SCRIPT=str(work/'route.txt'),NOSTALRE_PT_BR='1',NOSTALRE_UI_LAYOUT_PREVIEW='1',NOSTALRE_COMODO_MEMORY_TEST='1',DISPLAY=':99',WINIT_UNIX_BACKEND='x11',XDG_RUNTIME_DIR=str(work/'session-runtime'),LIBGL_ALWAYS_SOFTWARE='1',VK_ICD_FILENAMES='/usr/share/vulkan/icd.d/lvp_icd.x86_64.json')
build=subprocess.run(['cargo','+nightly','test','--release','--locked','--config','.cargo/switch.toml','-p','nostalre-client','--lib','--no-run','--message-format=json-render-diagnostics','-j','1'],cwd='/src/nostalre-launcher/client',env=env,capture_output=True,timeout=600)
(out/'build.jsonl').write_bytes(build.stdout);(out/'build.log').write_bytes(build.stderr)
assert build.returncode==0,'client build failed'
messages=[json.loads(l) for l in build.stdout.decode().splitlines() if l.startswith('{')]
executables=[m['executable'] for m in messages if m.get('reason')=='compiler-artifact' and m.get('target',{}).get('name')=='nostalre_client' and m.get('executable')]
assert len(executables)==1
workers=[];streams=[];samples=[];client=None
def start(name,args):
    stream=(out/(name+'.log')).open('wb');streams.append(stream)
    p=subprocess.Popen(args,cwd=work,env=env,stdout=stream,stderr=stream);workers.append(p);return p
def ready(port):
    deadline=time.monotonic()+90
    while time.monotonic()<deadline:
        assert all(p.poll() is None for p in workers)
        try:
            with socket.create_connection(('127.0.0.1',port),timeout=.1):return
        except OSError:time.sleep(.1)
    raise TimeoutError(port)
try:
    start('char',['/host-build/bin/char-server']);ready(6121)
    start('map',['/host-build/bin/map-server']);ready(5121)
    deadline=time.monotonic()+60
    while 'Map-Server 0 connected:' not in (out/'char.log').read_text(errors='replace'):
        assert time.monotonic()<deadline;time.sleep(.1)
    start('xvfb',['Xvfb',':99','-screen','0','1280x720x24','-nolisten','tcp']);time.sleep(1)
    client=start('client',[executables[0],'full_window_offline_character_map_walk_and_actions','--ignored','--nocapture','--test-threads=1'])
    deadline=time.monotonic()+140;shots=set()
    while client.poll() is None:
        assert time.monotonic()<deadline,'Comodo client timed out'
        status=Path(f'/proc/{client.pid}/status').read_text()
        samples.append({k:int(line.split()[1]) for line in status.splitlines() for k in ['VmRSS','VmHWM'] if line.startswith(k+':')})
        text=(out/'client.log').read_text(errors='replace')
        for shot in ('COMODO-OFF','COMODO-ON'):
            if shot not in shots and f'script shot {shot}' in text:
                raw=out/(shot+'.xwd');png=out/(shot+'.png')
                subprocess.run(['xwd','-root','-silent','-out',str(raw)],env=env,check=True)
                subprocess.run(['convert',str(raw),str(png)],env=env,check=True)
                shots.add(shot)
        time.sleep(.5)
    report=dict(client_exit=client.returncode,peak_rss_kib=max(s.get('VmHWM',0) for s in samples),source_save_preserved=hashlib.sha256(source.read_bytes()).hexdigest()==gold,shots=sorted(shots),pc_only=True)
    (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report),flush=True)
    assert client.returncode==0 and len(shots)==2 and report['source_save_preserved']
finally:
    for p in reversed(workers):
        if p.poll() is None:
            p.send_signal(signal.SIGTERM)
            try:p.wait(timeout=10)
            except subprocess.TimeoutExpired:p.kill();p.wait()
    for f in streams:f.close()
