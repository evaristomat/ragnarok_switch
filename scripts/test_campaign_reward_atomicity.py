"""Inject a reward-write failure into a disposable fixture, then retry via real dialogue."""
from pathlib import Path
import contextlib,json,os,shutil,signal,socket,sqlite3,subprocess,tempfile,time
ROOT=Path('/edition');OUT=ROOT/'evidence/campaign-atomicity-pre8';OUT.mkdir(parents=True,exist_ok=True)
world=Path(tempfile.mkdtemp(prefix='classic-campaign-atomicity-'))
shutil.copytree(ROOT/'source/nostalre-launcher/client/switch/romfs/server',world,dirs_exist_ok=True)
shutil.copy2(ROOT/'evidence/world/test-world-save.db',world/'ragnarok.db')
columns='base_level,job_level,base_exp,job_exp,zeny,status_point,skill_point'
with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
 db.execute('CREATE TABLE IF NOT EXISTS ragswitch_campaign(char_id INTEGER PRIMARY KEY,version INTEGER NOT NULL DEFAULT 1,step INTEGER NOT NULL DEFAULT 0,progress INTEGER NOT NULL DEFAULT 0,roster INTEGER NOT NULL DEFAULT 0,revision INTEGER NOT NULL DEFAULT 0)')
 db.execute('INSERT OR REPLACE INTO ragswitch_campaign(char_id,step) VALUES(150000,6)')
 db.execute('UPDATE char SET class=1,base_level=20,job_level=10,base_exp=0,job_exp=0,zeny=123,status_point=50,skill_point=0')
 db.execute("CREATE TRIGGER reject_campaign_reward BEFORE UPDATE ON char WHEN NEW.base_level>20 BEGIN SELECT RAISE(ABORT,'injected campaign reward write failure'); END")
 before=db.execute('SELECT '+columns+' FROM char').fetchone();db.commit()
env=dict(os.environ,RA_KEEP_CWD='1',RA_SQLITE_DIR=str(world),NOSTALRE_TEST_SESSION_PATH=str(world/'session.bin'),NOSTALRE_OFFLINE_SESSION=str(world/'session.bin'))
workers=[];streams=[]
def probe(name,travel=True):
 args=['/test-build/probe/release/nostalre-offline-probe','--seconds','0','--no-initial-walk']
 steps=['wait:1']+(['campaign-menu:4','wait:1'] if travel else [])+['talk:158,246','wait:1']
 for step in steps:args+=['--social',step]
 with (OUT/(name+'.log')).open('wb') as out:p=subprocess.run(args,cwd=world,env=env,stdout=out,stderr=subprocess.STDOUT,timeout=70)
 assert p.returncode==0,(OUT/(name+'.log')).read_text(errors='replace')[-4000:]
try:
 for role,port in [('char',6121),('map',5121)]:
  stream=(OUT/(role+'.log')).open('wb');streams.append(stream)
  workers.append(subprocess.Popen(['/host-build/bin/'+role+'-server'],cwd=world,env=env,stdout=stream,stderr=stream))
  end=time.monotonic()+90
  while True:
   assert all(p.poll() is None for p in workers)
   try:
    with socket.create_connection(('127.0.0.1',port),timeout=.1):break
   except OSError:assert time.monotonic()<end;time.sleep(.1)
 end=time.monotonic()+60
 while 'Map-Server 0 connected:' not in (OUT/'char.log').read_text(errors='replace'):
  assert time.monotonic()<end;time.sleep(.1)
 probe('failed-transaction')
 with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
  after_failure=db.execute('SELECT '+columns+' FROM char').fetchone()
  assert after_failure==before,(before,after_failure)
  assert db.execute('SELECT step,progress,revision FROM ragswitch_campaign').fetchone()==(6,0,0)
  db.execute('DROP TRIGGER reject_campaign_reward');db.commit()
 probe('successful-retry')
 with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
  after_retry=db.execute('SELECT '+columns+' FROM char').fetchone()
  assert after_retry[0]==22 and after_retry[4]==5623,after_retry
  assert db.execute('SELECT step,progress,revision FROM ragswitch_campaign').fetchone()==(7,0,1)
 probe('repeated-contact-after-relog',False)
 with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
  assert db.execute('SELECT '+columns+' FROM char').fetchone()==after_retry
  assert db.execute('SELECT step,progress,revision FROM ragswitch_campaign').fetchone()==(7,0,1)
  assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
 report=dict(passed=True,fixture_stage=6,failure_injected=True,before=before,after_failure=after_failure,after_retry=after_retry,duplicate_reward_prevented=True,relog_preserved=True)
 (OUT/'report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
 print('CAMPAIGN_ATOMICITY_PASSED',json.dumps(report),flush=True)
finally:
 for p in reversed(workers):
  if p.poll() is None:
   p.send_signal(signal.SIGTERM)
   try:p.wait(timeout=25)
   except subprocess.TimeoutExpired:p.kill();p.wait()
 for f in streams:f.close()
