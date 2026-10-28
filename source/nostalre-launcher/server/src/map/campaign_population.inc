// Included once by population_engine.cpp. Companion actors remain ephemeral.
#include "campaign.hpp"
static std::unordered_map<uint32,int> campaign_assist_targets;
static int campaign_defend_candidate(block_list* bl,va_list args) {
 auto* owner=va_arg(args,map_session_data*);auto** found=va_arg(args,mob_data**);auto* best=va_arg(args,int*);
 auto* md=BL_CAST(BL_MOB,bl);if(!md || status_isdead(*md) || campaign_protect_target(md,owner))return 0;
 auto* victim=map_id2sd(md->target_id);
 if(!victim || (victim!=owner && victim->pop.campaign_owner!=owner->status.char_id))return 0;
 int gap=distance_bl(owner,md);if(gap<*best){*best=gap;*found=md;}return 0;
}
static uint16_t campaign_companion_job(map_session_data* owner,int slot) {
 const int job=pc_jobid2mapid(owner->status.class_);
 if((job&MAPID_FIRSTMASK)==MAPID_NOVICE)return JOB_NOVICE;
 const bool second=(job&JOBL_2)!=0;
 static const uint16_t first[]={JOB_SWORDMAN,JOB_MAGE,JOB_ACOLYTE};
 static const uint16_t advanced[]={JOB_KNIGHT,JOB_WIZARD,JOB_PRIEST};
 return second?advanced[slot]:first[slot];
}
void population_engine_campaign_recall(map_session_data* owner,bool revive) {
 if(!owner)return;
 campaign_assist_targets.erase(owner->status.char_id);
 std::vector<map_session_data*> old;
 for(auto* bot:g_population_engine_pcs)if(bot && bot->pop.campaign_owner==owner->status.char_id)old.push_back(bot);
 if(revive)for(auto* bot:old)population_engine_shell_release(bot);
 else for(auto* bot:old)if(!pc_isdead(bot)) {
  pc_setpos(bot,owner->mapindex,owner->x,owner->y,CLR_TELEPORT);population_engine_finish_map_placement(bot);
 }
}
static void campaign_companion_sync(map_session_data* owner) {
 if(!owner || population_engine_is_population_pc(owner->id) || !owner->state.active || owner->prev==nullptr)return;
 int desired=campaign_roster(owner);
 for(int slot=0;slot<3;slot++) {
  map_session_data* bot=nullptr;
  for(auto* b:g_population_engine_pcs)if(b && b->pop.campaign_owner==owner->status.char_id && b->pop.campaign_slot==slot){bot=b;break;}
  uint16_t job=campaign_companion_job(owner,slot);
  // A level/class change must not resurrect a casualty; recall handles recovery.
  if(bot && pc_isdead(bot) && slot<desired)continue;
  if(bot && (slot>=desired || bot->status.class_!=job || bot->status.base_level!=owner->status.base_level)) {
   population_engine_shell_release(bot);bot=nullptr;
  }
  if(slot>=desired)continue;
  if(!bot) {
   const size_t cap=std::max(1,battle_config.population_engine_max_count);
   // Reserve within the global cap by retiring one ambient actor, never adding over it.
   if(g_population_engine_count.load()>=cap) {
    map_session_data* ambient=nullptr;
    for(auto* b:g_population_engine_pcs)if(b && !b->pop.campaign_owner){ambient=b;break;}
    if(!ambient)continue;
    population_engine_shell_release(ambient);
   }
   int16 x=owner->x,y=owner->y;
   if(!map_search_freecell(owner,owner->m,&x,&y,3,3,1))continue;
   auto equipment=population_engine_db().find(job);
   const uint32_t index=population_engine_allocate_index();if(!index)continue;
   bot=population_engine_spawn_shell(owner->m,x,y,index,job,slot==1?'M':'F',slot+1,0,get_job_weapon(job),0,0,0,0,0,0,0,equipment?equipment->script:nullptr,false,equipment?equipment.get():nullptr,3,PopulationDbSource::Main,true);
   if(!bot)continue;
   bot->pop.campaign_owner=owner->status.char_id;bot->pop.campaign_slot=slot;
   bot->pop.classic_town_role=0;bot->pop.arena_team=0;bot->pop.flags|=PSF::Mortal|PSF::CombatActive;
   bot->status.party_id=0;bot->status.base_level=owner->status.base_level;
   bot->status.job_level=std::min<uint32>(owner->status.job_level,50);
   static const char* names[]={"Lira","Caio","Nara"};safestrncpy(bot->status.name,names[slot],sizeof(bot->status.name));
   int level=std::min<uint32>(99,owner->status.base_level);
   bot->status.str=slot==0?std::max(2,level):10;
   bot->status.int_=slot>0?std::max(2,level):10;
   bot->status.vit=std::max(2,slot==0?level*3/4:level/3);
   bot->status.dex=std::max(2,level*3/4);bot->status.agi=std::max(1,level/2);
   status_calc_pc(bot,SCO_FORCE);
   bot->status.hp=bot->battle_status.hp=bot->battle_status.max_hp;
   bot->status.sp=bot->battle_status.sp=bot->battle_status.max_sp;
   g_population_engine_pcs.push_back(bot);g_population_engine_count++;
   g_population_engine_stats.total_created++;g_population_engine_stats.active_units++;
   clif_name_area(bot);
   ShowInfo("[CampaignCompanion] owner=%u id=%d slot=%d job=%u base=%u chat=%d hp=%u\n",owner->status.char_id,bot->id,slot,bot->status.class_,bot->status.base_level,bot->chatID,bot->battle_status.hp);
  }
  if(!pc_isdead(bot) && (bot->m!=owner->m || distance_bl(bot,owner)>14)) {
   pc_setpos(bot,owner->mapindex,owner->x,owner->y,CLR_TELEPORT);population_engine_finish_map_placement(bot);
  }
 }
}
static int campaign_companion_sync_pc(map_session_data* owner,va_list) {campaign_companion_sync(owner);return 0;}
static void campaign_companion_sweep() {
 for(auto it=campaign_assist_targets.begin();it!=campaign_assist_targets.end();) {if(!map_charid2sd(it->first))it=campaign_assist_targets.erase(it);else ++it;}
 std::vector<map_session_data*> abandoned;
 for(auto* b:g_population_engine_pcs)if(b && b->pop.campaign_owner && !map_charid2sd(b->pop.campaign_owner))abandoned.push_back(b);
 for(auto* b:abandoned)population_engine_shell_release(b);
 map_foreachpc(campaign_companion_sync_pc);
}
bool population_engine_campaign_tick(map_session_data* bot) {
 if(!bot || !bot->pop.campaign_owner)return false;
 auto* owner=map_charid2sd(bot->pop.campaign_owner);
 if(!owner || pc_isdead(owner) || pc_isdead(bot) || owner->m!=bot->m)return true;
 auto tick=gettick();
 if(bot->ud.skilltimer!=INVALID_TIMER)return true; // Finish the current cast before following/changing targets.
 // Support targets only its linked roster, with a cooldown, and remains vulnerable.
 map_session_data* heal=owner;
 for(auto* ally:g_population_engine_pcs)if(ally && ally->pop.campaign_owner==owner->status.char_id && ally->m==bot->m && !pc_isdead(ally) && (int64)ally->battle_status.hp*heal->battle_status.max_hp<(int64)heal->battle_status.hp*ally->battle_status.max_hp)heal=ally;
 if(bot->pop.campaign_slot==2 && bot->status.class_!=JOB_NOVICE && tick>bot->pop.reactive_buff_cd && heal->battle_status.hp<heal->battle_status.max_hp*8/10) {
  if(distance_bl(bot,heal)>8)unit_walktobl(bot,heal,5,0);
  else unit_skilluse_id(bot,heal->id,AL_HEAL,(pc_jobid2mapid(owner->status.class_)&JOBL_2)?10:std::min<int>(10,std::max(1,(int)owner->status.job_level)));
  bot->pop.reactive_buff_cd=tick+2200;return true;
 }
 int target=owner->ud.target;
 if(!target)target=owner->ud.skilltarget;
 auto* md=map_id2md(target);
 if(md && md->m==owner->m && !status_isdead(*md) && distance_bl(owner,md)<=AREA_SIZE)campaign_assist_targets[owner->status.char_id]=target;
 else {
  auto it=campaign_assist_targets.find(owner->status.char_id);
  md=it==campaign_assist_targets.end()?nullptr:map_id2md(it->second);
  if(md && md->m==owner->m && !status_isdead(*md) && distance_bl(owner,md)<=AREA_SIZE)target=md->id;
  else {
   campaign_assist_targets.erase(owner->status.char_id);md=nullptr;int nearest=AREA_SIZE+1;
   map_foreachinrange(campaign_defend_candidate,owner,AREA_SIZE,BL_MOB,owner,&md,&nearest);
   if(md){target=md->id;campaign_assist_targets[owner->status.char_id]=target;}
  }
 }
 if(md && md->m==bot->m && !status_isdead(*md) && distance_bl(owner,md)<=AREA_SIZE) {
  bot->pop.target_id=target;
  if(bot->pop.campaign_slot==0 && bot->status.class_!=JOB_NOVICE && md->target_id!=bot->id && status_get_class_(md)!=CLASS_BOSS && tick>bot->pop.reactive_buff_cd) {
   unit_skilluse_id(bot,target,SM_PROVOKE,std::min<int>(10,std::max(1,(int)owner->status.job_level)));
   bot->pop.reactive_buff_cd=tick+5000;
  }
  if(bot->pop.campaign_slot==1 && bot->status.class_!=JOB_NOVICE) {
   // Preserve the cast across AI ticks instead of alternating it with melee.
   if(bot->ud.skilltimer==INVALID_TIMER && tick>bot->pop.skill_cd) {
    if(distance_bl(bot,md)>8)unit_walktobl(bot,md,6,0);
    else {
     int result=unit_skilluse_id(bot,target,MG_FIREBOLT,std::min<int>(10,std::max(1,(int)owner->status.base_level/10)));
     if(!bot->pop.campaign_cast_logged) {ShowInfo("[CampaignCast] id=%d firebolt=%d chat=%d sit=%d vending=%d blocked=%d disallowed=%d\n",bot->id,result,bot->chatID,pc_issit(bot),bot->state.vending,bot->ud.state.blockedskill,skill_isNotOk(MG_FIREBOLT,*bot));bot->pop.campaign_cast_logged=true;}
    }
    bot->pop.skill_cd=tick+2000;
   }
  } else if(bot->pop.campaign_slot!=2 || bot->status.class_==JOB_NOVICE) {
   if(distance_bl(bot,md)>1)unit_walktobl(bot,md,1,0);
   else unit_attack(bot,target,1);
  }
 } else {
  unit_stop_attack(bot);bot->pop.target_id=0;
  if(distance_bl(bot,owner)>3 && tick>bot->pop.last_move+700) {unit_walktobl(bot,owner,2,0);bot->pop.last_move=tick;}
 }
 return true;
}
