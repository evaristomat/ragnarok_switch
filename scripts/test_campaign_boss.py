"""Focused boss fixture, separate from the unassisted fresh-character campaign tour."""
from pathlib import Path
import contextlib,os,runpy,shutil,sqlite3,tempfile
root=Path('/edition');profile=os.environ.get('CAMPAIGN_PROFILE','physical')
fixture=Path(tempfile.mkdtemp(prefix='classic-boss-fixture-'))/'boss.db'
shutil.copy2(root/'evidence/world/test-world-save.db',fixture)
with contextlib.closing(sqlite3.connect(fixture)) as db:
 db.execute('CREATE TABLE IF NOT EXISTS ragswitch_campaign(char_id INTEGER PRIMARY KEY,version INTEGER NOT NULL DEFAULT 1,step INTEGER NOT NULL DEFAULT 0,progress INTEGER NOT NULL DEFAULT 0,roster INTEGER NOT NULL DEFAULT 0,revision INTEGER NOT NULL DEFAULT 0)')
 db.execute('INSERT OR REPLACE INTO ragswitch_campaign(char_id,step,roster,revision) VALUES(150000,30,3,30)')
 job={'physical':7,'magic':9,'support':8}[profile]
 strength=90 if profile=='physical' else 10;intelligence=10 if profile=='physical' else 90
 db.execute('UPDATE char SET class=?,base_level=95,job_level=50,base_exp=0,job_exp=0,status_point=0,skill_point=0,str=?,int=?,vit=70,dex=70,agi=20,luk=1,hp=30000,sp=3000',(job,strength,intelligence))
 # A level-95 role fixture retains the ordinary first-class skills it would know.
 db.execute('DELETE FROM skill')
 for skill,level in [(1,9),(142,1),({'physical':5,'magic':19,'support':28}[profile],10)]:db.execute('INSERT INTO skill(char_id,id,lv,flag) VALUES(150000,?,?,0)',(skill,level))
 if profile=='support':db.execute('INSERT INTO skill(char_id,id,lv,flag) VALUES(150000,156,1,0)') # Existing Holy Light quest skill.
 db.commit()
os.environ.update(CAMPAIGN_RESUME=str(fixture),CAMPAIGN_EVIDENCE='boss-'+profile)
runpy.run_path(str(root/'scripts/test_campaign.py'),run_name='__main__')
