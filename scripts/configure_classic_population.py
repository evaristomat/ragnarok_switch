"""Private classic occupations and restrained consumable shops, no dynamic drop economy."""
from pathlib import Path
import re,yaml
ROOT=Path(__file__).resolve().parent.parent
source=ROOT/'source/nostalre-launcher/server'
romfs=ROOT/'source/nostalre-launcher/client/switch/romfs/server'
path=source/'db/population_engine.yml'
data=yaml.safe_load(path.read_text(encoding='utf8'))
for profile in data['Body']:
    if profile.get('Profile')=='novice_default':
        profile.update(BaseLevel=[1,3],JobLevel=[1,3],Str=[2,6],Agi=[1,5],Vit=[2,6],Int=[1,5],Dex=[2,7],Luk=[1,4])
    if profile.get('Profile')=='combat_pve_low':
        profile.update(Str=[6,22],Agi=[4,18],Vit=[5,18],Int=[6,24],Dex=[8,24],Luk=[1,8])
vendors=dict(Header=dict(Type='POPULATION_VENDORS_DB',Version=1),Body=[dict(
    VendorKey='classic_supplies',Title='Consumíveis',Type='Static',MaxSlots=6,
    Stock=[dict(Item=item,Amount=amount) for item,amount in [(501,40),(502,25),(506,20),(602,15),(611,20),(1750,100)]])])
settings=dict(population_engine_target_per_map=20,population_engine_shell_skill_interval_ms=4000,population_engine_shell_attack_skill_delay_ms=2000,
    population_engine_shell_basic_attack_chance=85,population_engine_wander_max_per_tick=3,
    population_engine_wander_cooldown_ms=20000,population_engine_wander_cooldown_jitter_ms=70000,
    population_engine_chat_cooldown_ms=240000,population_engine_chat_cooldown_jitter_ms=180000,
    population_engine_vending_enable='on')
config=(source/'conf/battle/population_engine.conf').read_text(encoding='utf8')
for key,value in settings.items():
    config,count=re.subn(rf'(?m)^{key}:.*$',f'{key}: {value}',config)
    assert count==1,key
config=config.replace('// This first integration is ambient/PvE; no generated vendor economy.',
                      '// Classic towns: bounded static consumables only; dynamic drop vendors are disabled.')
for root in [source,romfs]:
    (root/'db/population_engine.yml').write_text('# Independent classic profiles and occupation preset.\n'+yaml.safe_dump(data,sort_keys=False,allow_unicode=True),encoding='utf8')
    (root/'db/population_vendors.yml').write_text('# Fixed ordinary stock; prices use each classic item buy value.\n'+yaml.safe_dump(vendors,sort_keys=False,allow_unicode=True),encoding='utf8')
    (root/'conf/battle/population_engine.conf').write_text(config,encoding='utf8')
print('CLASSIC_POPULATION_CONFIGURED: 50 town / 20 hunting / 6 novice tutorial; static shops only')
