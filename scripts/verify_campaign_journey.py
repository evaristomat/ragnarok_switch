"""Audit the fresh-character journey across legitimate development relogs/resumes."""
from pathlib import Path
import contextlib,json,re,sqlite3
ROOT=Path(__file__).resolve().parent.parent;OUT=ROOT/'evidence/campaign-pre8-fresh-physical'
last=json.loads((OUT/'report.json').read_text(encoding='utf8'))
assert last['passed'] and last['canonical_stage']==33 and last['completion_survived_relog']
claims=[];sources=[]
for path in sorted(OUT.glob('part*-map.log'))+[OUT/'map.log']:
 rows=[int(n) for n in re.findall(r'Campaign claim: char=150000 step=(\d+) revision=\d+',path.read_text(errors='replace'))]
 if rows:sources.append(dict(file=path.name,completed_steps=rows));claims+=rows
assert sorted(claims)==list(range(1,34)),claims
probe_files=sorted(OUT.glob('part*-probe.log'))+[OUT/'probe.log']
assert any('created CampaignReview' in p.read_text(errors='replace') for p in probe_files)
for path in probe_files:
 assert not re.search(r'social: say:@(?:lvup|joblvup|item\d*|setquest|jobchange|zeny)(?:\s|$)',path.read_text(errors='replace')),path
with contextlib.closing(sqlite3.connect('file:'+str(OUT/'last-save.db').replace('\\','/')+'?mode=ro',uri=True)) as db:
 assert db.execute('SELECT step,progress,revision FROM ragswitch_campaign').fetchone()==(33,0,33)
 hero=db.execute('SELECT name,class,base_level,job_level FROM char').fetchone()
 assert hero[0]=='CampaignReview' and hero[2]==99,hero
 assert db.execute('SELECT count(*) FROM char').fetchone()[0]==1
 assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
report=dict(passed=True,fresh_character=True,stages_verified=33,each_stage_reward_committed_once=True,
 development_relogs=bool(list(OUT.glob('part*-map.log'))),progress_commands_used=False,
 paid_equipment_used=True,travel_uses_the_exposed_offline_warp_menu_command=True,
 optional_second_class_used=True,hero=hero,claim_sources=sources,
 completion_survived_relog=last['completion_survived_relog'])
(ROOT/'evidence/campaign-journey-pre8.json').write_text(json.dumps(report,indent=2,ensure_ascii=False),encoding='utf8')
print('CAMPAIGN_JOURNEY_PASSED',json.dumps(report,ensure_ascii=True))
