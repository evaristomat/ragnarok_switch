// Private classic campaign. All mutations run on the map thread.
#pragma once
class map_session_data;
struct mob_data;
struct block_list;
void campaign_init();
int campaign_action(map_session_data* sd, int action, int expected, int npc_id);
void campaign_kill(map_session_data* killer, mob_data* mob);
bool campaign_protect_target(const block_list* source, const block_list* target);
int campaign_roster(map_session_data* sd);

void campaign_heartbeat();

bool campaign_protect_spawn(short map,int mob);
