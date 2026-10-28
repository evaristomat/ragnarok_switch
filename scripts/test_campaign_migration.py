"""Existing-save and Job Changer compatibility on disposable real server worlds."""
from pathlib import Path
import contextlib,hashlib,json,os,shutil,signal,socket,sqlite3,subprocess,tempfile,time
ROOT=Path('/edition');PAYLOAD=ROOT/'source/nostalre-launcher/client/switch/romfs/server';OUT=ROOT/'evidence/campaign-migration-pre8';OUT.mkdir(parents=True,exist_ok=True)
results=[]
def run(name,novice=False):
 world=Path(tempfile.mkdtemp(prefix='classic-campaign-migration-'));shutil.copytree(PAYLOAD,world,dirs_exist_ok=True)
 shutil.copy2(ROOT/'evidence/world/test-world-save.db',world/'ragnarok.db')
 with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
  db.execute('DROP TABLE IF EXISTS ragswitch_campaign') # Reproduce a genuine pre-campaign schema, only in a fixture.
  db.execute('DELETE FROM quest WHERE quest_id BETWEEN 900000 AND 900999')
  db.execute("DELETE FROM char_reg_num WHERE key='RS_CampStage'")
  if novice:
   db.execute('UPDATE char SET class=0,base_level=20,job_level=10,skill_point=0,base_exp=0,job_exp=0')
   db.execute('DELETE FROM skill');db.execute('INSERT INTO skill(char_id,id,lv,flag) VALUES(150000,1,9,0)')
  db.execute('INSERT OR IGNORE INTO quest(char_id,quest_id,state,time) VALUES(150000,1002,0,0)')
  before=db.execute('SELECT class,base_level,job_level,zeny FROM char').fetchone();db.commit()
 env=dict(os.environ,RA_KEEP_CWD='1',RA_SQLITE_DIR=str(world),NOSTALRE_TEST_SESSION_PATH=str(world/'session.bin'),NOSTALRE_OFFLINE_SESSION=str(world/'session.bin'))
 workers=[];streams=[]
 try:
  for role,port in [('char',6121),('map',5121)]:
   stream=(OUT/(name+'-'+role+'.log')).open('wb');streams.append(stream)
   workers.append(subprocess.Popen(['/host-build/bin/'+role+'-server'],cwd=world,env=env,stdout=stream,stderr=stream))
   end=time.monotonic()+120
   while True:
    assert all(p.poll() is None for p in workers),'server exited'
    try:
     with socket.create_connection(('127.0.0.1',port),timeout=.1):break
    except OSError:assert time.monotonic()<end;time.sleep(.1)
  end=time.monotonic()+60
  while 'Map-Server 0 connected:' not in (OUT/(name+'-char.log')).read_text(errors='replace'):
   assert time.monotonic()<end;time.sleep(.1)
  steps=['wait:1','campaign-menu:4','wait:1']
  if novice:steps+=['job-menu:1,1','job-menu:1','talk:150,180','wait:1']
  args=['/test-build/probe/release/nostalre-offline-probe','--seconds','0','--no-initial-walk']
  for step in steps:args+=['--social',step]
  with (OUT/(name+'-probe.log')).open('wb') as out:probe=subprocess.run(args,cwd=world,env=env,stdout=out,stderr=subprocess.STDOUT,timeout=100)
  assert probe.returncode==0,(OUT/(name+'-probe.log')).read_text(errors='replace')[-5000:]
  # Relog to load canonical progress rather than keeping a client-local flag.
  args=['/test-build/probe/release/nostalre-offline-probe','--seconds','0','--no-initial-walk','--social','wait:1','--social','campaign-menu:1']
  with (OUT/(name+'-relog.log')).open('wb') as out:probe=subprocess.run(args,cwd=world,env=env,stdout=out,stderr=subprocess.STDOUT,timeout=50)
  assert probe.returncode==0
 finally:
  for p in reversed(workers):
   if p.poll() is None:
    p.send_signal(signal.SIGTERM)
    try:p.wait(timeout=25)
    except subprocess.TimeoutExpired:p.kill();p.wait()
  for f in streams:f.close()
 with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
  after=db.execute('SELECT class,base_level,job_level,zeny FROM char').fetchone()
  state=db.execute('SELECT step,progress,revision FROM ragswitch_campaign WHERE char_id=150000').fetchone()
  assert state==(5,0,1 if novice else 0),state
  if novice:assert after[:3]==(1,20,1),(before,after)
  else:assert after==before,(before,after)
  assert db.execute('SELECT count(*) FROM char').fetchone()[0]==1
  assert db.execute('SELECT amount,expire_time FROM inventory WHERE char_id=150000 AND nameid=12887').fetchall()==[(1,0)]
  assert db.execute('SELECT count(*) FROM quest WHERE quest_id=1002').fetchone()[0]==1
  assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
 log=(OUT/(name+'-map.log')).read_text(errors='replace');assert '[Error]' not in log and 'script error' not in log.lower()
 results.append(dict(name=name,passed=True,before=before,after=after,state=state,wing_not_duplicated=True,classic_quest_preserved=True,relog_preserved=True))
run('already-transcendent')
run('job-changer-before-delivery',True)
(OUT/'report.json').write_text(json.dumps(dict(passed=True,checks=results),indent=2),encoding='utf8')
print('CAMPAIGN_MIGRATION_PASSED',json.dumps(results),flush=True)
