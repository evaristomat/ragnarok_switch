// Included once by quest.cpp, keeping the campaign in the existing quest module.
#include "campaign.hpp"
#include "population_engine.hpp"
#include "npc.hpp"
#include "unit.hpp"
#include "status.hpp"
#include "script.hpp"
#include "skill.hpp"
#include <common/sql.hpp>
#include <unordered_map>
#include <algorithm>
struct CampaignStage { const char* npc; const char* map; int x,y,kind; const char* area; int mob,count,item,goal,jobgoal,zeny,ax,ay; };
#include "campaign_data.inc"
static constexpr int campaign_size=sizeof(campaign_stages)/sizeof(campaign_stages[0]);
struct CampaignSave { int step=0, count=0, roster=0; uint32 revision=0; bool valid=false; };
struct CampaignAttempt { int mob=0; bool entered=false; };
static std::unordered_map<uint32,CampaignAttempt> campaign_attempts;
static std::unordered_map<uint32,CampaignSave> campaign_active;
void campaign_init() {
 if (Sql_Query(mmysql_handle,"CREATE TABLE IF NOT EXISTS ragswitch_campaign (char_id INTEGER PRIMARY KEY, version INTEGER NOT NULL DEFAULT 1, step INTEGER NOT NULL DEFAULT 0, progress INTEGER NOT NULL DEFAULT 0, roster INTEGER NOT NULL DEFAULT 0, revision INTEGER NOT NULL DEFAULT 0)")==SQL_ERROR) Sql_ShowDebug(mmysql_handle);
}
static CampaignSave campaign_load(map_session_data* sd) {
 CampaignSave out;
 if (!sd || population_engine_is_population_pc(sd->id)) return out;
 if (Sql_Query(mmysql_handle,"SELECT step,progress,roster,revision FROM ragswitch_campaign WHERE char_id=%u",sd->status.char_id)==SQL_ERROR)return out;
 if(Sql_NextRow(mmysql_handle)==SQL_SUCCESS) {
  char* v=nullptr;
  Sql_GetData(mmysql_handle,0,&v,nullptr);out.step=atoi(v);
  Sql_GetData(mmysql_handle,1,&v,nullptr);out.count=atoi(v);
  Sql_GetData(mmysql_handle,2,&v,nullptr);out.roster=atoi(v);
  Sql_GetData(mmysql_handle,3,&v,nullptr);out.revision=strtoul(v,nullptr,10);
  out.valid=out.step>=0 && out.step<=campaign_size && out.roster>=0 && out.roster<=3;
 }
 Sql_FreeResult(mmysql_handle);return out;
}
static void campaign_sync(map_session_data* sd,const CampaignSave& save) {
 if (!save.valid)return;
 campaign_active[sd->status.char_id]=save;
 pc_setreg2(sd,"RS_CampStage",save.step);
 // Only our own active quests may be removed. Completed receipts remain in the journal.
 for(int i=0;i<sd->num_quests;) {
  int id=sd->quest_log[i].quest_id;
  if(id>=900000 && id<900000+campaign_size && id!=900000+save.step) {quest_delete(sd,id);continue;}
  ++i;
 }
 for(int i=0;i<save.step;i++) {
  int id=900500+i;
  if(quest_check(sd,id,HAVEQUEST)<0){quest_add(sd,id);quest_update_status(sd,id,Q_INACTIVE);}
 }
 int id=save.step<campaign_size ? 900000+save.step:900999;
 if(quest_check(sd,id,HAVEQUEST)<0)quest_add(sd,id);
 if(save.step==campaign_size)quest_update_status(sd,id,Q_INACTIVE);
 else {
  for(int i=0;i<sd->avail_quests;i++) if(sd->quest_log[i].quest_id==id && campaign_stages[save.step].mob) {
   if(sd->quest_log[i].count[0]!=save.count){sd->quest_log[i].count[0]=save.count;clif_quest_update_objective(sd,&sd->quest_log[i]);}
  }
 }
 pc_show_questinfo(sd);
}
static bool campaign_at_contact(map_session_data* sd,const CampaignStage& st,int npc_id) {
 auto* npc=map_id2nd(npc_id);
 return npc && npc->m==sd->m && std::string(npc->name)==std::string(st.npc).substr(0,std::string(st.npc).find("::")) && sd->m==map_mapname2mapid(st.map) && distance_xy(sd->x,sd->y,npc->x,npc->y)<=6;
}
static bool campaign_can_claim(map_session_data* sd,const CampaignSave& s,const CampaignStage& st,int npc) {
 // Remote job changer is allowed for class and epilogue; no hunt can be delivered remotely.
 if (!campaign_at_contact(sd,st,npc) && !(sd->npc_id==npc && std::string(st.npc)=="Mestre das Classes"))return false;
 if(pc_isdead(sd))return false;
 if((st.kind==1 || st.kind==4) && s.count<st.count)return false;
 if(st.kind==2 && ([&](){ int count=0;for(int n=0;n<MAX_INVENTORY;n++)if(sd->inventory.u.items_inventory[n].nameid==(uint32)st.item)count+=sd->inventory.u.items_inventory[n].amount;return count;})()<st.count)return false;
 if(st.kind==3 && (sd->status.class_==JOB_NOVICE || sd->status.class_==JOB_NOVICE_HIGH || sd->status.class_==JOB_BABY))return false;
 return true;
}
static bool campaign_claim(map_session_data* sd,CampaignSave& s,const CampaignStage& st) {
 // A revision travels with every normal character save. The char server rejects stale
 // autosaves queued before this synchronous transaction; reward and stage cannot diverge.
 if(Sql_Query(mmysql_handle,"BEGIN IMMEDIATE")==SQL_ERROR)return false;
 if(Sql_Query(mmysql_handle,"UPDATE ragswitch_campaign SET step=step+1,progress=0,revision=revision+1 WHERE char_id=%u AND step=%d AND revision=%u",sd->status.char_id,s.step,s.revision)==SQL_ERROR || Sql_NumRowsAffected(mmysql_handle)!=1) {Sql_Query(mmysql_handle,"ROLLBACK");return false;}
 const auto previous=sd->status;
 // Adaptive preparation grants the remaining EXP to the chapter's reference level.
 // These are already 15x quest budgets: param EXP prevents a second multiplier.
 for(int n=0;n<99 && sd->status.base_level<(uint32)st.goal && !pc_is_maxbaselv(sd);n++) {
  auto needed=pc_nextbaseexp(sd);if(needed<=sd->status.base_exp)break;
  pc_gainexp(sd,nullptr,needed-sd->status.base_exp,0,3);
 }
 for(int n=0;n<70 && sd->status.job_level<(uint32)st.jobgoal && !pc_is_maxjoblv(sd);n++) {
  auto needed=pc_nextjobexp(sd);if(needed<=sd->status.job_exp)break;
  pc_gainexp(sd,nullptr,0,needed-sd->status.job_exp,3);
 }
 sd->status.zeny=std::min<int64>(MAX_ZENY,(int64)sd->status.zeny+st.zeny);
 sd->status.campaign_revision=s.revision+1;
 auto& p=sd->status;
 bool ok=Sql_Query(mmysql_handle,"UPDATE char SET base_level=%u,job_level=%u,base_exp=%" PRIu64 ",job_exp=%" PRIu64 ",zeny=%d,status_point=%u,skill_point=%u WHERE char_id=%u",p.base_level,p.job_level,p.base_exp,p.job_exp,p.zeny,p.status_point,p.skill_point,p.char_id)!=SQL_ERROR;
 if(!ok || Sql_Query(mmysql_handle,"COMMIT")==SQL_ERROR) {
  Sql_Query(mmysql_handle,"ROLLBACK");sd->status=previous;status_calc_pc(sd,SCO_FORCE);
  for(auto field:{SP_ZENY,SP_BASELEVEL,SP_JOBLEVEL,SP_BASEEXP,SP_JOBEXP,SP_STATUSPOINT,SP_SKILLPOINT})clif_updatestatus(*sd,field);
  return false;
 }
 ++s.step;s.count=0;++s.revision;
 clif_updatestatus(*sd,SP_ZENY);campaign_sync(sd,s);chrif_save(sd,CSAVE_NORMAL);
 ShowInfo("Campaign claim: char=%u step=%d revision=%u\n",p.char_id,s.step,s.revision);
 return true;
}
static int campaign_find_morocc(block_list* bl,va_list ap) {
 auto* md=BL_CAST(BL_MOB,bl);auto* found=va_arg(ap,int*);
 if(md && md->mob_id==1916 && !status_isdead(*md)) *found=md->id;
 return 0;
}
static void campaign_cancel_attempt(uint32 owner) {
 auto it=campaign_attempts.find(owner);if(it==campaign_attempts.end())return;
 if(auto* md=map_id2md(it->second.mob))unit_free(md,CLR_OUTSIGHT);
 campaign_attempts.erase(it);
}
static bool campaign_begin(map_session_data* sd) {
 auto s=campaign_load(sd);if(!s.valid || s.step>=campaign_size || campaign_stages[s.step].kind!=4 || s.count>=campaign_stages[s.step].count || pc_isdead(sd))return false;
 auto& st=campaign_stages[s.step];
 campaign_cancel_attempt(sd->status.char_id);
 int mid=map_mapname2mapid(st.area),found=0;
 map_foreachinmap(campaign_find_morocc,mid,BL_MOB,&found);
 // Never replace, delete or duplicate an unrelated public encounter.
 if(found){clif_displaymessage(sd->fd,"Já existe um Satan Morocc nesta área. Aguarde o confronto em andamento terminar.");return false;}
 int id=mob_once_spawn(sd,mid,58,193,"Satan Morocc",1916,1,"",0,AI_NONE);
 auto* md=map_id2md(id);if(!md)return false;
 // Scale only this actor. Database, public MVPs, IDs and artwork stay classic.
 md->status.max_hp=md->status.hp=180000;
 md->status.rhw.atk=650;md->status.rhw.atk2=900;
 md->status.matk_min=100;md->status.matk_max=160;
 md->status.def=20;md->status.mdef=20;
 md->status.str=60;md->status.agi=40;md->status.vit=50;md->status.int_=40;md->status.dex=70;md->status.luk=10;
 // Changing AGI/DEX alone leaves the raid's derived flee and 76ms attack delay.
 md->status.hit=169;md->status.flee=139;md->status.flee2=10;
 md->status.def2=50;md->status.mdef2=65;md->status.batk=60;
 md->status.adelay=1200;md->status.amotion=600;md->status.dmotion=300;md->status.speed=180;
 // Clone only this actor's database row before reducing raid-scale spell levels.
 // Keep the classic visual/ID and two telegraphed attacks; public Morocc stays untouched.
 auto original=md->db;md->db=std::make_shared<s_mob_db>(*original);md->db->skill.clear();
 for(const auto& spell:original->skill) {
  if(!spell || (spell->skill_id!=NPC_PULSESTRIKE && spell->skill_id!=WZ_METEOR))continue;
  auto scaled=std::make_shared<s_mob_skill>(*spell);scaled->skill_lv=1;md->db->skill.push_back(scaled);
 }
 md->db->status=md->status;
 if(md->base_status)*md->base_status=md->status;
 md->special_state.ai=AI_NONE;
 campaign_attempts[sd->status.char_id]={id,false};
 return true;
}
bool campaign_protect_spawn(int16 map,int mob) {
 if(mob!=1916 || map!=map_mapname2mapid("moc_fild21"))return false;
 int found=0;map_foreachinmap(campaign_find_morocc,map,BL_MOB,&found);return found!=0;
}
void campaign_heartbeat() {
 for(auto it=campaign_active.begin();it!=campaign_active.end();) {
  if(!map_charid2sd(it->first))it=campaign_active.erase(it);else ++it;
 }
 std::vector<uint32> cancel;
 for(auto& pair:campaign_attempts) {
  auto* owner=map_charid2sd(pair.first);auto* mob=map_id2md(pair.second.mob);
  if(!owner || !mob || pc_isdead(owner)) {cancel.push_back(pair.first);continue;}
  if(owner->m==mob->m)pair.second.entered=true;
  else if(pair.second.entered)cancel.push_back(pair.first);
 }
 for(auto owner:cancel)campaign_cancel_attempt(owner);
}
int campaign_roster(map_session_data* sd) {auto s=campaign_load(sd);return s.valid?s.roster:0;}
int campaign_action(map_session_data* sd,int action,int expected,int npc) {
 if(!sd || population_engine_is_population_pc(sd->id))return -1;
 auto s=campaign_load(sd);
 if(!s.valid && action==0) {
  bool grounds=strncmp(map_getmapdata(sd->m)->name,"new_",4)==0;
  int start=grounds?0:((sd->status.class_==JOB_NOVICE || sd->status.class_==JOB_NOVICE_HIGH || sd->status.class_==JOB_BABY)?4:5);
  if(Sql_Query(mmysql_handle,"INSERT OR IGNORE INTO ragswitch_campaign(char_id,step) VALUES(%u,%d)",sd->status.char_id,start)==SQL_ERROR)return -1;
  s=campaign_load(sd);
 }
 if(!s.valid)return -1;
 if(action==0) {campaign_sync(sd,s);clif_quest_send_list(sd);return s.step;}
 if(action==1) {
  if(expected!=s.step || s.step>=campaign_size)return 0;
  const auto& st=campaign_stages[s.step];
  return campaign_can_claim(sd,s,st,npc) && campaign_claim(sd,s,st);
 }
 if(action==2) {
  campaign_heartbeat();if(campaign_attempts.count(sd->status.char_id))return -1;
  int roster=(s.roster+1)%4;
  if(Sql_Query(mmysql_handle,"UPDATE ragswitch_campaign SET roster=%d WHERE char_id=%u",roster,sd->status.char_id)==SQL_ERROR)return 0;
  population_engine_campaign_recall(sd,true);
  std::string message="Companheiros recrutados: "+std::to_string(roster)+"/3 (Lira, Caio, Nara).";
  clif_displaymessage(sd->fd,message.c_str());return roster;
 }
 if(action==3)return s.step<campaign_size && campaign_at_contact(sd,campaign_stages[s.step],npc) && campaign_begin(sd);
 if(action==5) {
  if(s.step>=campaign_size || pc_isdead(sd))return 0;
  auto& st=campaign_stages[s.step];
  bool hunt=st.kind==1 && s.count<st.count;
  const char* destination=hunt?st.area:st.map;int16 x=hunt?st.ax:st.x,y=hunt?st.ay:st.y;
  int mid=map_mapname2mapid(destination);
  if(mid<0)return 0;
  bool found=false;const int ax=x,ay=y;
  // Arrival must avoid warp rectangles and OnTouch scripts, not only solid walls.
  for(int radius=hunt?0:1;radius<=6 && !found;radius++) {
   for(int dy=-radius;dy<=radius && !found;dy++)for(int dx=-radius;dx<=radius;dx++) {
    if(std::max(abs(dx),abs(dy))!=radius)continue;
    if(map_getcell(mid,ax+dx,ay+dy,CELL_CHKPASS) && !map_getcell(mid,ax+dx,ay+dy,CELL_CHKNPC)) {x=ax+dx;y=ay+dy;found=true;break;}
   }
  }
  if(!found)return 0;
  campaign_cancel_attempt(sd->status.char_id);
  population_engine_campaign_recall(sd,false);
  return pc_setpos(sd,mapindex_name2id(destination),x,y,CLR_TELEPORT)==SETPOS_OK;
 }
 if(action==4){campaign_heartbeat();population_engine_campaign_recall(sd,!campaign_attempts.count(sd->status.char_id));return 1;}
 return 0;
}
void campaign_kill(map_session_data* killer,mob_data* md) {
 if(!killer || !md)return;
 auto* sd=population_engine_is_population_pc(killer->id)?map_charid2sd(killer->pop.campaign_owner):killer;
 if(!sd || sd->m!=md->m || pc_isdead(sd) || distance_bl(sd,md)>AREA_SIZE)return;
 auto s=campaign_load(sd);if(!s.valid || s.step>=campaign_size)return;
 auto& st=campaign_stages[s.step];
 if(st.mob!=md->mob_id || sd->m!=map_mapname2mapid(st.area) || s.count>=st.count)return;
 if(st.kind==4) {
  auto it=campaign_attempts.find(sd->status.char_id);
  if(it==campaign_attempts.end() || it->second.mob!=md->id)return;
  campaign_attempts.erase(it);
 }
 if(Sql_Query(mmysql_handle,"UPDATE ragswitch_campaign SET progress=progress+1 WHERE char_id=%u AND step=%d AND progress<%d",sd->status.char_id,s.step,st.count)==SQL_ERROR)return;
 ++s.count;campaign_sync(sd,s);
 ShowInfo("[CampaignCredit] char=%u step=%d count=%d companion=%u\n",sd->status.char_id,s.step,s.count,killer->pop.campaign_owner);
}
bool campaign_protect_target(const block_list* src,const block_list* target) {
 if(!src || !target)return false;
 // Native Morocc angels heal/buff nearby monsters. Their friendly skills must
 // not enter the owned encounter any more than their attacks can enter it.
 if(src->type==BL_MOB && target->type==BL_MOB) {
  for(const auto& entry:campaign_attempts)
   if(target->id==entry.second.mob && src->id!=target->id)return true;
 }
 if(src->type==BL_MOB && target->type==BL_PC) {
  const auto* pc=BL_CAST(BL_PC,target);
  uint32 owner=pc->pop.campaign_owner?pc->pop.campaign_owner:pc->status.char_id;
  for(const auto& entry:campaign_attempts) {
   if(src->id==entry.second.mob)return owner!=entry.first;
   if(owner==entry.first) {
    const auto* boss=map_id2md(entry.second.mob);
    // The owned encounter uses this existing map without drawing its unrelated
    // raid monsters or ambient bots into the player's solo fight.
    if(boss && boss->m==src->m)return true;
   }
  }
 }
 if(target->type!=BL_MOB)return false;
 auto* pc=BL_CAST(BL_PC,src);
 if(!pc || !population_engine_is_population_pc(pc->id) || pc->pop.campaign_owner)return false;
 for(auto& pair:campaign_attempts)if(pair.second.mob==target->id)return true;
 const auto* md=BL_CAST(BL_MOB,target);
 for(const auto& entry:campaign_active) {
  const auto& save=entry.second;
  if(save.step>=campaign_size)continue;
  const auto& stage=campaign_stages[save.step];
  if(stage.mob!=md->mob_id || save.count>=stage.count || map_mapname2mapid(stage.area)!=md->m)continue;
  const auto* owner=map_charid2sd(entry.first);
  // In the solo edition, keep the current objective species available across this
  // map. Ambient hunters must not clear it before the owner can walk into view.
  if(owner && owner->m==md->m && !pc_isdead(owner))return true;
 }
 return false;
}
