"""Actual server/protocol regression for class tier, levels, pose and field density."""
from pathlib import Path
import contextlib,json,os,re,shutil,signal,socket,sqlite3,struct,subprocess,tempfile,time,zlib
ROOT=Path('/edition');PAYLOAD=ROOT/'source/nostalre-launcher/client/switch/romfs/server'
OUT=ROOT/('evidence/campaign-pre8-'+os.environ.get('CAMPAIGN_EVIDENCE',os.environ.get('CAMPAIGN_PROFILE','physical')));OUT.mkdir(parents=True,exist_ok=True)
world=Path(tempfile.mkdtemp(prefix='classic-population-review-'));shutil.copytree(PAYLOAD,world,dirs_exist_ok=True)
if os.environ.get('CAMPAIGN_EVIDENCE','').startswith('boss-'):
 # Observe the owned boss and move one existing native healer into range once.
 # This role fixture reproduces foreign healing without changing quest progress.
 (world/'npc/custom/campaign_fixture_probe.txt').write_text('''-\tscript\tCampaignFixtureProbe\t-1,{
OnInit:
 initnpctimer; end;
OnTimer10000:
 .@n=getmapunits(BL_MOB,"moc_fild21",.@ids);
 for (.@i=0;.@i<.@n;.@i++) {
  getunitdata .@ids[.@i],.@u;
  if (.@u[UMOB_CLASS]==1916) {
   debugmes "[BossFixture] id="+.@ids[.@i]+" hp="+.@u[UMOB_HP]+" max="+.@u[UMOB_MAXHP]+" atk="+.@u[UMOB_ATKMIN]+".."+.@u[UMOB_ATKMAX];
   if (!.placed && .@u[UMOB_HP]<.@u[UMOB_MAXHP]) {
    for (.@j=0;.@j<.@n;.@j++) {
     getunitdata .@ids[.@j],.@h;
     if (.@h[UMOB_CLASS]!=1921) continue;
     unitwarp .@ids[.@j],"moc_fild21",.@u[UMOB_X]+3,.@u[UMOB_Y];
     unitskilluseid .@ids[.@j],28,11,.@ids[.@i];
     .placed=1;debugmes "[BossFixture] native healer challenge="+.@ids[.@j];break;
    }
   }
  }
 }
 setnpctimer 0; end;
}
''',encoding='utf8')
 with (world/'npc/scripts_custom.conf').open('a') as f:f.write('\nnpc: npc/custom/campaign_fixture_probe.txt\n')
# Speed up filling only; class selection, positioning and behavior are production paths.
with (world/'conf/import/battle_conf.txt').open('a') as f:f.write('\npopulation_engine_autosummon_batch_size: 50\n')
env=dict(os.environ,RA_KEEP_CWD='1',RA_SQLITE_DIR=str(world),NOSTALRE_TEST_SESSION_PATH=str(world/'session.bin'),NOSTALRE_OFFLINE_SESSION=str(world/'session.bin'))
# Test navigation uses the shipped collision grid, never unseen monster locations.
manifest=json.loads((ROOT/'campaign/pre-renewal/campaign.yml').read_text(encoding='utf8'))
seen=set();warps=[]
def visit_warps(relative):
 if relative in seen:return
 seen.add(relative);path=world/relative
 if not path.is_file():return
 for line in path.read_bytes().decode('cp1252',errors='replace').splitlines():
  line=line.split('//',1)[0].strip()
  include=re.match(r'(?:npc|import):\s*(\S+)',line)
  if include:visit_warps(include[1]);continue
  warp=re.match(r'([\w@-]+),(\d+),(\d+),\d+\twarp\t[^\t]+\t(\d+),(\d+),',line)
  if warp:warps.append((warp[1],*(int(warp[n]) for n in range(2,6))))
visit_warps('npc/pre-re/scripts_main.conf')
for rel in ['db/map_cache.dat','db/pre-re/map_cache.dat','db/import/map_cache.dat']:
 data=(world/rel).read_bytes();_,count=struct.unpack_from('<IH',data);offset=8
 for _ in range(count):
  name,w,h,size=struct.unpack_from('<12shhi',data,offset);offset+=20
  cells=bytearray(zlib.decompress(data[offset:offset+size]));offset+=size
  map_name=name.split(b'\0')[0].decode()
  for area,x,y,rx,ry in warps:
   if area==map_name:
    for py in range(max(0,y-ry),min(h,y+ry+1)):
     for px in range(max(0,x-rx),min(w,x+rx+1)):cells[py*w+px]=1
  for stage in manifest['stages']:
   if stage['mob'] and stage['area']==name.split(b'\0')[0].decode():
    (world/f'campaign-walk-{stage["id"]}.bin').write_bytes(struct.pack('<HH',w,h)+cells)
start=0
resume=os.environ.get('CAMPAIGN_RESUME')
if resume:
    shutil.copy2(resume,world/'ragnarok.db')
    with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:start=db.execute('SELECT step FROM ragswitch_campaign WHERE char_id=150000').fetchone()[0]
workers=[];streams=[];report={'passed':False,'resumed_from_stage':start};failures=[]
try:
    for role,port in [('char',6121),('map',5121)]:
        stream=(OUT/(role+'.log')).open('wb');streams.append(stream)
        workers.append(subprocess.Popen(['/host-build/bin/'+role+'-server'],cwd=world,env=env,stdout=stream,stderr=stream))
        end=time.monotonic()+90
        while True:
            assert all(x.poll() is None for x in workers)
            try:
                with socket.create_connection(('127.0.0.1',port),timeout=.1):break
            except OSError:assert time.monotonic()<end;time.sleep(.05)
    end=time.monotonic()+60
    while 'Map-Server 0 connected:' not in (OUT/'char.log').read_text(errors='replace'):
        assert time.monotonic()<end;time.sleep(.05)
    cmd=['/test-build/probe/release/nostalre-offline-probe','--seconds','0','--no-initial-walk','--dump','Skill' if os.environ.get('CAMPAIGN_EVIDENCE','').startswith('boss-') else 'hat']
    if not resume:cmd+=['--create','CampaignReview']
    manifest=json.loads((ROOT/'campaign/pre-renewal/campaign.yml').read_text(encoding='utf8'))
    limit=int(os.environ.get('CAMPAIGN_STAGES','33'));profile=os.environ.get('CAMPAIGN_PROFILE','physical')
    def paid_preparation():
        # This is the same exposed warp command as the offline game's travel menu.
        # Existing equipment is bought with quest-earned zeny, never created by GM.
        return ['say:@warp prt_in 173 130','wait:1','shop-equip:172,130,1119','shop-equip:172,132,2309','shop-equip:172,132,2503','shop-equip:172,132,2403']
    steps=['wait:1']
    if resume and start>=14 and profile=='physical' and not os.environ.get('CAMPAIGN_EVIDENCE','').startswith('boss-'):
        with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
            if not db.execute('SELECT 1 FROM inventory WHERE nameid=1119').fetchone():steps+=paid_preparation()
    for i,stage in list(enumerate(manifest['stages'][:limit]))[start:]:
        steps+=['campaign-menu:4','wait:1']
        if stage['kind']=='class':
            steps+=['learn-basic','job-menu:'+{'physical':'1,1','magic':'2,1','support':'4,1'}[profile],'wait:1']
        if stage['kind']=='hunt':
            steps+=['allocate:'+profile,'learn-job:'+profile,f'fight-objective:{stage["id"]},{stage["mob"]},{stage["count"]},900','campaign-menu:4','wait:1']
        if stage['kind']=='boss':
            steps+=[f'talk-pick:{stage["x"]},{stage["y"]},1','wait:2','campaign-menu:2','allocate:'+profile,f'fight-objective:{stage["id"]},{stage["mob"]},1,900','campaign-menu:4','wait:1',f'talk-pick:{stage["x"]},{stage["y"]},2']
        else:steps+=[f'talk:{stage["x"]},{stage["y"]}','wait:1']
        if i==4:steps+=['campaign-menu:2','campaign-menu:2','campaign-menu:2','wait:3']
        if i==13:
            steps+=['learn-job:'+profile,'job-menu:1,1','wait:3'] # Optional second class via the existing changer, after Job40.
        if i==14 and profile=='physical':steps+=paid_preparation() # The next payment covers all four purchases.
    steps+=['wait:2','logout']
    for step in steps:cmd+=['--social',step]
    with (OUT/'probe.log').open('wb') as f:
        probe=subprocess.Popen(cmd,cwd=world,env=env,stdout=f,stderr=subprocess.STDOUT)
        end=time.monotonic()+7200;captured=set()
        while probe.poll() is None:
            assert time.monotonic()<end,'campaign tour timed out'
            with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
                row=db.execute('SELECT step FROM ragswitch_campaign WHERE char_id=150000').fetchone()
                if row and row[0] in (5,14,23,28,30,31,33) and row[0] not in captured:
                    cp=OUT/f'checkpoint-{row[0]}.db'
                    with contextlib.closing(sqlite3.connect(cp)) as target:db.backup(target)
                    captured.add(row[0]);print('CAMPAIGN_CHECKPOINT',row[0],flush=True)
            time.sleep(.25)
        p=probe
    text=(OUT/'probe.log').read_text(errors='replace')
    assert p.returncode==0 and text.rstrip().endswith('clean'),text[-4000:]
    with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
        state=db.execute('SELECT step,progress,revision FROM ragswitch_campaign WHERE char_id=150000').fetchone()
        assert state==(limit,0,limit),state
        saved=db.execute('SELECT base_level,job_level FROM char WHERE char_id=150000').fetchone()
        assert saved[0]>=3 and saved[1]>=3,saved
        if start<=30<limit:assert db.execute('SELECT roster FROM ragswitch_campaign').fetchone()[0]==3
    logs=(OUT/'map.log').read_text(errors='replace')
    assert 'script error' not in logs.lower() and '[Error]' not in logs,logs[-5000:]
    if os.environ.get('CAMPAIGN_EVIDENCE','').startswith('boss-') and start==30:
        boss_ids=set(re.findall(r'\[BossFixture\] id=(\d+)',logs))
        assert boss_ids,'Owned boss fixture was never observed'
        assert '[BossFixture] native healer challenge=' in logs
        foreign_buffs=[(source,target) for source,target in re.findall(r'SkillNoDamage \{ skill: (?:AlHeal|HpAssumptio), src_gid: (\d+), target_gid: (\d+)',text) if target in boss_ids and source!=target]
        assert not foreign_buffs,foreign_buffs
        assert 'Retire-se do confronto antes de alterar seus companheiros.' in text
        report.update(owned_boss_won=True,external_heals_and_buffs_absent=True,roster_change_locked_during_battle=True)
    if limit==33:
        # The probe exits via the real logout protocol. Wait until the char
        # server observes the map disconnect before opening a new connection.
        deadline=time.monotonic()+15
        while True:
            with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:online=db.execute('SELECT online FROM char WHERE char_id=150000').fetchone()[0]
            if not online:break
            assert time.monotonic()<deadline,'Character remained online after acknowledged logout'
            time.sleep(.1)
        with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
            final_character=db.execute('SELECT class,base_level,job_level,zeny,base_exp,job_exp,skill_point FROM char').fetchone()
        relog=['/test-build/probe/release/nostalre-offline-probe','--seconds','0','--no-initial-walk','--dump','UiOpen']
        for step in ['wait:1','campaign-menu:1','campaign-menu:4','campaign-menu:1']:relog+=['--social',step]
        with (OUT/'completion-relog.log').open('wb') as stream:p=subprocess.run(relog,cwd=world,env=env,stdout=stream,stderr=subprocess.STDOUT,timeout=60)
        text=(OUT/'completion-relog.log').read_text(errors='replace')
        assert p.returncode==0 and 'CAMPANHA CONCLU\u00cdDA' in text and 'quest_id: 900999' in text,text[-3000:]
        with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
            assert db.execute('SELECT class,base_level,job_level,zeny,base_exp,job_exp,skill_point FROM char').fetchone()==final_character
            assert db.execute('SELECT step,progress,revision FROM ragswitch_campaign').fetchone()==(33,0,33)
            assert db.execute('SELECT count(*) FROM quest WHERE quest_id BETWEEN 900500 AND 900532').fetchone()[0]==33
            assert db.execute('SELECT count(*) FROM quest WHERE quest_id=900999').fetchone()[0]==1
        report.update(completion_survived_relog=True,completion_opens_final_journal_entry=True,repeated_menu_did_not_duplicate_rewards=True)
    report.update(passed=True,stages_without_gm_progress=limit-start,stages_verified_this_run=limit-start,canonical_stage=limit,profile=profile,state=state,fixture=os.environ.get('CAMPAIGN_EVIDENCE','').startswith('boss-'))
finally:
    for p in reversed(workers):
        if p.poll() is None:
            p.send_signal(signal.SIGTERM)
            try:p.wait(timeout=20)
            except subprocess.TimeoutExpired:p.kill();p.wait()
    for f in streams:f.close()
    with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
        with contextlib.closing(sqlite3.connect(OUT/'last-save.db')) as target:db.backup(target)
    (OUT/'report.json').write_text(json.dumps(report,indent=2))
print('CAMPAIGN_REVIEW',json.dumps({k:v for k,v in report.items() if k!='samples'}),flush=True)
assert report['passed'],failures
