"""Actual server/protocol regression for class tier, levels, pose and field density."""
from pathlib import Path
import contextlib,json,os,re,shutil,signal,socket,sqlite3,subprocess,tempfile,time
ROOT=Path('/edition');PAYLOAD=ROOT/'source/nostalre-launcher/client/switch/romfs/server'
OUT=ROOT/'evidence/population-pre8';OUT.mkdir(parents=True,exist_ok=True)
world=Path(tempfile.mkdtemp(prefix='classic-population-review-'));shutil.copytree(PAYLOAD,world,dirs_exist_ok=True)
# Speed up filling only; class selection, positioning and behavior are production paths.
with (world/'conf/import/battle_conf.txt').open('a') as f:f.write('\npopulation_engine_autosummon_batch_size: 50\n')
env=dict(os.environ,RA_KEEP_CWD='1',RA_SQLITE_DIR=str(world),NOSTALRE_TEST_SESSION_PATH=str(world/'session.bin'),NOSTALRE_OFFLINE_SESSION=str(world/'session.bin'))
workers=[];streams=[];report={'passed':False};failures=[]
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
    cmd=['/test-build/probe/release/nostalre-offline-probe','--create','PopulationReview','--seconds','0','--dump','hat']
    steps=['wait:1','say:@populate skillcost','wait:1','say:@populate off','say:@jobchange 1','say:@lvup 19','say:@warp prontera 156 191',
           'wait:1','say:@populate on','wait:5','say:@populate audit','wait:1',
           'say:@warp prt_fild08 170 180','wait:5','say:@populate audit','wait:1',
           'say:@jobchange 7','say:@lvup 30','wait:5','say:@populate audit','wait:1',
           'say:@jobchange 1','say:@lvup -30','wait:5','say:@populate audit','wait:1']
    for step in steps:cmd+=['--social',step]
    with (OUT/'probe.log').open('wb') as f:
        p=subprocess.run(cmd,cwd=world,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=90)
    text=(OUT/'probe.log').read_text(errors='replace');assert p.returncode==0 and text.rstrip().endswith('clean')
    groups=[]
    for block in text.split('social: say:@populate audit')[1:]:
        # Each event also has raw packet bytes; inspect only decoded messages.
        rows=re.findall(r'\[PopAudit\] id=(\d+) job=(\d+) base=(\d+) joblevel=(\d+) role=(\d+) direction=(\d+) sitting=(\d+)',block.split('social:',1)[0])
        groups.append([dict(zip(['id','job','base','joblevel','role','direction','sitting'],map(int,row))) for row in rows])
    assert len(groups)==4, len(groups)
    first={0,1,2,3,4,5,6,24,25,4046};second=set(range(7,24))|{4046,4047,4048,4049,4024,4045}
    def check(name,value):
        report[name]=bool(value)
        if not value:failures.append(name)
    cost=re.search(r'\[SkillCostAudit\] skills=(\d+) nonzero=(\d+)',text)
    check('all_loaded_skill_levels_have_zero_sp_cost',cost is not None and int(cost[1])>500 and int(cost[2])==0)
    town,field,advanced,returned=groups
    check('town_50',len(town)==50)
    check('first_class_town_has_no_second_classes',bool(town) and all(x['job'] in first for x in town))
    check('first_class_level_band',bool(town) and all(14<=x['base']<=23 for x in town))
    check('town_faces_at_least_four_directions',len({x['direction'] for x in town})>=4)
    check('town_mixes_standing_and_sitting',3<=sum(x['sitting'] for x in town)<=18)
    check('field_20',len(field)==20)
    check('first_class_field_has_no_second_classes',bool(field) and all(x['job'] in first for x in field))
    check('second_class_updates_existing_field_population',len(advanced)==20 and any(x['job'] in second for x in advanced) and all(44<=x['base']<=53 for x in advanced))
    check('return_to_first_class_downgrades_population',len(returned)==20 and all(x['job'] in first and 14<=x['base']<=23 for x in returned))
    check('positive_class_levels',all(1<=x['base']<=99 and x['joblevel']>=1 for group in groups for x in group))
    report['samples']=groups
    time.sleep(1)
    with contextlib.closing(sqlite3.connect(world/'ragnarok.db')) as db:
        check('no_bot_characters_saved',db.execute('SELECT count(*) FROM char').fetchone()[0]==1)
        check('save_integrity',db.execute('PRAGMA quick_check').fetchone()[0]=='ok')
    report['passed']=not failures;report['failures']=failures
finally:
    for p in reversed(workers):
        if p.poll() is None:
            p.send_signal(signal.SIGTERM)
            try:p.wait(timeout=20)
            except subprocess.TimeoutExpired:p.kill();p.wait()
    for f in streams:f.close()
    (OUT/'report.json').write_text(json.dumps(report,indent=2))
print('POPULATION_REVIEW',json.dumps({k:v for k,v in report.items() if k!='samples'}),flush=True)
assert report['passed'],failures
