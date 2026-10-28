"""Audit real solo boss fights, including the native-angel healing regression."""
from pathlib import Path
import json,re
root=Path(__file__).resolve().parent.parent
profiles={}
skills={'physical':'SmBash','magic':'MgFirebolt','support':'AlHolylight'}
for profile,skill in skills.items():
    folder=root/f'evidence/campaign-pre8-boss-{profile}'
    report=json.loads((folder/'report.json').read_text())
    assert report['passed'] and report['resumed_from_stage']==30,profile
    assert report['canonical_stage']==33 and report['completion_survived_relog'],profile
    text=(folder/'probe.log').read_text(errors='replace')
    # Each role fixture engages only its owned Morocc; use the actual successful
    # player spell's target AID, never an assumed allocation order.
    targets=set(re.findall(r'SkillDamage \{ skill: '+skill+r', src_gid: 2000000, target_gid: (\d+), damage: [1-9]\d*',text))
    assert len(targets)==1,(profile,targets)
    buffs=[(source,target) for source,target in re.findall(r'SkillNoDamage \{ skill: (?:AlHeal|HpAssumptio), src_gid: (\d+), target_gid: (\d+)',text) if target in targets and source!=target]
    assert not buffs,(profile,buffs)
    assert 'Retire-se do confronto antes de alterar seus companheiros.' in text,profile
    claims=re.findall(r'Campaign claim: char=150000 step=(\d+) revision=\d+',(folder/'map.log').read_text(errors='replace'))
    assert claims==['31','32','33'],(profile,claims)
    assert '[BossFixture] native healer challenge=' in (folder/'map.log').read_text(errors='replace'),profile
    profiles[profile]=dict(passed=True,owned_boss_won=True,external_heals_and_buffs_absent=True,roster_change_locked_during_battle=True,epilogue_and_relog_passed=True,fixture_starts_at_stage=30)
result=dict(passed=True,profiles=profiles)
(root/'evidence/campaign-bosses-pre8.json').write_text(json.dumps(result,indent=2))
print('CAMPAIGN_BOSSES_PASSED',json.dumps(result),flush=True)
