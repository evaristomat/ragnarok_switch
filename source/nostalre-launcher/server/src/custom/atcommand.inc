// Copyright (c) rAthena Dev Teams - Licensed under GNU GPL
// For more information, see LICENCE in the main folder

/**
 * Custom Atcommands
 * Place the body of custom atcommands in this file.
 * Format:
 *	ACMD_FUNC(command_name)
 *	{
 *		<code>;
 *	}
 **/

//ACMD_FUNC(newcommand)
//{
//	clif_displaymessage(fd, "It works!");
//	clif_specialeffect(&sd->bl, EF_HEARTCASTING, AREA);
//	return 0;
//}


/* @populate — population engine (runtime PC shells; alias @fakeplayers) */
/**
 * @reloadpopenginedb — Reload db/population_names.yml, db/population_engine.yml, db/population_chat.yml (does not despawn existing fake PCs).
 */
ACMD_FUNC(reloadpopenginedb)
{
	nullpo_retr(-1, sd);

	if (!battle_config.population_engine_enable) {
		clif_displaymessage(fd, "Populacao desativada. Use @populate on para ativar.");
		return -1;
	}


	uint32_t n = 0;
	if (!population_engine_reload_equipment(&n)) {
		clif_displaymessage(fd, "Population engine equipment reload failed (see map-server console).");
		return -1;
	}

	char buf[128];
	snprintf(buf, sizeof(buf), "Population engine equipment reloaded (%u job profiles).", static_cast<unsigned>(n));
	clif_displaymessage(fd, buf);
	return 0;
}

ACMD_FUNC(populate)
{
	nullpo_retr(-1, sd);

    if (message && strcmpi(message, "skillcost") == 0) {
        unsigned count=0, nonzero=0;
        for (const auto& entry : skill_db) {
            ++count;
            for (unsigned lv=0; lv<MAX_SKILL_LEVEL; ++lv)
                if (entry.second->require.sp[lv] || entry.second->require.sp_rate[lv]) ++nonzero;
        }
        for (const auto id : {SM_BASH, AL_HEAL, MG_FIREWALL, MO_EXTREMITYFIST, CH_CHAINCRUSH}) {
            const auto req=skill_get_requirement(sd,id,1);
            if (req.sp || req.sp_rate) ++nonzero;
        }
        char line[128];
        snprintf(line,sizeof(line),"[SkillCostAudit] skills=%u nonzero=%u",count,nonzero);
        clif_displaymessage(fd,line);
        return 0;
    }

    // RagSWITCH: on/off are usable even when the engine is disabled. Reload
    // from a clean state so timers and the occupancy cache cannot multiply.
    if (message && (strcmpi(message, "on") == 0 || strcmpi(message, "off") == 0)) {
        const bool enable = strcmpi(message, "on") == 0;
        do_final_population_engine();
        battle_config.population_engine_enable = enable ? 1 : 0;
        if (enable) do_init_population_engine_load_databases();
        clif_displaymessage(fd, enable ? "Populacao ativada: criacao automatica no mapa atual."
                                       : "Populacao desativada e removida.");
        return 0;
    }

	if (!battle_config.population_engine_enable) {
		clif_displaymessage(fd, "Populacao desativada. Use @populate on para ativar.");
		return -1;
	}


	char cmd[50], param[256];

	if (!message || !*message) {
		clif_displaymessage(fd, "Uso: @populate on | off | stats | timing");
		clif_displaymessage(fd, "  Spawns population-engine PCs (visible shells, no autocombat).");
		clif_displaymessage(fd, "  @populate stop | stats | status  (alias: @fakeplayers)");
		clif_displaymessage(fd, "  @populate timing | timingreset  (wander tick profiler)");
		return -1;
	}

	const char *msg_ptr = message;
	while (*msg_ptr == ' ' || *msg_ptr == '\t')
		msg_ptr++;
	sscanf(msg_ptr, "%49s %255[^\n]", cmd, param);

	if (strcmpi(cmd, "stop") == 0) {
		if (!population_engine_is_running()) {
			clif_displaymessage(fd, "Population engine is not running");
			return -1;
		}
		population_engine_stop();
		clif_displaymessage(fd, "Population engine stopped and cleaned up");
	} else if (strcmpi(cmd, "stats") == 0) {
		PopulationEngineStats stats = population_engine_get_stats();
		const size_t live_count = population_engine_get_count();
		char msg[256];
		clif_displaymessage(fd, "=== Population engine stats ===");
		snprintf(msg, sizeof(msg), "  Active / created / errors : %zu / %u / %u",
			live_count, stats.total_created, stats.errors);
		clif_displaymessage(fd, msg);
		snprintf(msg, sizeof(msg), "  Chat lines emitted        : %u", stats.chat_lines_emitted);
		clif_displaymessage(fd, msg);
		snprintf(msg, sizeof(msg), "  Walk failures             : %u", stats.walk_failures);
		clif_displaymessage(fd, msg);
		snprintf(msg, sizeof(msg), "  Name blocklist retries    : %u", stats.name_retries);
		clif_displaymessage(fd, msg);
        unsigned vendors=0,chats=0,resting=0,strolling=0,walking=0;
        extern std::vector<map_session_data*> g_population_engine_pcs;
        for (const auto* bot : g_population_engine_pcs) {
            if (!bot || bot->m!=sd->m) continue;
            if (bot->pop.classic_town_role==1 && bot->state.vending) ++vendors;
            if (bot->pop.classic_town_role==2 && bot->chatID) ++chats;
            if (bot->pop.classic_town_role==3) ++resting;
            if (bot->pop.classic_town_role==4) {++strolling;if (bot->ud.walktimer!=INVALID_TIMER) ++walking;}
        }
        snprintf(msg,sizeof(msg),"Cidade: lojas=%u salas=%u repouso=%u passeio=%u andando=%u",vendors,chats,resting,strolling,walking);
        clif_displaymessage(fd,msg);
		// Memory estimate: each live bot holds one map_session_data allocation.
		// sizeof is a compile-time constant so this is safe in a sprintf context.
		const size_t est_kb = (live_count * sizeof(map_session_data) + 512) / 1024;
		snprintf(msg, sizeof(msg), "  Est. heap (map_session_data x %zu) : ~%zu KB",
			live_count, est_kb);
		clif_displaymessage(fd, msg);
		clif_displaymessage(fd, "================================");
	} else if (strcmpi(cmd, "audit") == 0) {
        char msg[256];
        extern std::vector<map_session_data*> g_population_engine_pcs;
        for (const auto* bot : g_population_engine_pcs) {
            if (!bot || bot->m!=sd->m) continue;
            snprintf(msg,sizeof(msg),"[PopAudit] id=%u job=%u base=%u joblevel=%u role=%u direction=%u sitting=%u",
                bot->status.account_id,bot->status.class_,bot->status.base_level,bot->status.job_level,
                bot->pop.classic_town_role,bot->ud.dir,pc_issit(bot)?1:0);
            clif_displaymessage(fd,msg);
        }
	} else if (strcmpi(cmd, "status") == 0) {
		if (population_engine_is_running())
			clif_displaymessage(fd, "Population engine is RUNNING");
		else
			clif_displaymessage(fd, "Population engine is NOT running");
	} else if (strcmpi(cmd, "timing") == 0) {
		const WanderTimingSnapshot ts = population_engine_path_get_timing_snapshot();
		char msg[256];
		clif_displaymessage(fd, "=== Population wander tick profile ===");
		snprintf(msg, sizeof(msg), "  Ticks recorded / window      : %" PRIu64 " / %" PRIu64,
			ts.ticks_recorded, ts.window_size);
		clif_displaymessage(fd, msg);
		if (ts.window_size == 0) {
			clif_displaymessage(fd, "  (no samples yet \u2014 wait a few seconds)");
		} else {
			snprintf(msg, sizeof(msg), "  Avg total / tick             : %.1f \u00b5s   (%.2f ms)",
				ts.avg_total_us, ts.avg_total_us / 1000.0);
			clif_displaymessage(fd, msg);
			snprintf(msg, sizeof(msg), "    avg map_search_freecell    : %.1f \u00b5s   (%.1f%%)",
				ts.avg_freecell_us,
				ts.avg_total_us > 0 ? 100.0 * ts.avg_freecell_us / ts.avg_total_us : 0.0);
			clif_displaymessage(fd, msg);
			snprintf(msg, sizeof(msg), "    avg fallback path_search   : %.1f \u00b5s   (%.1f%%)",
				ts.avg_fallback_us,
				ts.avg_total_us > 0 ? 100.0 * ts.avg_fallback_us / ts.avg_total_us : 0.0);
			clif_displaymessage(fd, msg);
			snprintf(msg, sizeof(msg), "    avg unit_walktoxy          : %.1f \u00b5s   (%.1f%%)",
				ts.avg_unit_walktoxy_us,
				ts.avg_total_us > 0 ? 100.0 * ts.avg_unit_walktoxy_us / ts.avg_total_us : 0.0);
			clif_displaymessage(fd, msg);
			snprintf(msg, sizeof(msg), "  Avg shells examined/walked   : %.1f / %.1f",
				ts.avg_examined, ts.avg_processed);
			clif_displaymessage(fd, msg);
			snprintf(msg, sizeof(msg), "  Last tick total              : %" PRIu64 " \u00b5s  (proc %u, exam %u)",
				ts.last_total_us, ts.last_processed, ts.last_examined);
			clif_displaymessage(fd, msg);
			snprintf(msg, sizeof(msg), "    last freecell/fallback/uw  : %" PRIu64 " / %" PRIu64 " / %" PRIu64 " \u00b5s",
				ts.last_freecell_us, ts.last_fallback_us, ts.last_unit_walktoxy_us);
			clif_displaymessage(fd, msg);
			snprintf(msg, sizeof(msg), "  Worst tick total             : %" PRIu64 " \u00b5s  (proc %u)",
				ts.worst_total_us, ts.worst_processed);
			clif_displaymessage(fd, msg);
			snprintf(msg, sizeof(msg), "  Calls (cumulative) fc/fb/uw  : %" PRIu64 " / %" PRIu64 " / %" PRIu64,
				ts.freecell_calls, ts.fallback_calls, ts.unit_walktoxy_calls);
			clif_displaymessage(fd, msg);
		}
		clif_displaymessage(fd, "=======================================");
		// pe_perf generic section snapshot (chat/spawn/etc).
		{
			pe_perf::SectionStat sects[pe_perf::kMaxSections];
			size_t n = 0;
			pe_perf::snapshot(sects, n);
			clif_displaymessage(fd, "=== Population perf sections ===");
			if (n == 0) {
				clif_displaymessage(fd, "  (no sections recorded yet)");
			} else {
				char hdr[256];
				snprintf(hdr, sizeof(hdr), "  %-28s %10s %12s %10s %10s %10s",
					"section", "calls", "total_us", "avg_us", "last_us", "worst_us");
				clif_displaymessage(fd, hdr);
				for (size_t i = 0; i < n; ++i) {
					const auto &s = sects[i];
					const double avg = s.calls > 0 ? double(s.total_us) / double(s.calls) : 0.0;
					char row[256];
					snprintf(row, sizeof(row), "  %-28s %10" PRIu64 " %12" PRIu64 " %10.1f %10" PRIu64 " %10" PRIu64,
						s.name ? s.name : "?", s.calls, s.total_us, avg, s.last_us, s.worst_us);
					clif_displaymessage(fd, row);
				}
			}
			clif_displaymessage(fd, "================================");
		}
	} else if (strcmpi(cmd, "timingreset") == 0) {
		population_engine_path_reset_timing();
		pe_perf::reset();
		clif_displaymessage(fd, "Population wander timing reset.");
	} else {
		uint32_t quantity = 0;
		char map_name[32] = "";
		int parsed = sscanf(msg_ptr, "%u %31s", &quantity, map_name);

		if (parsed < 1 || quantity == 0) {
			clif_displaymessage(fd, "Invalid quantity. Uso: @populate on | off | stats | timing");
			return -1;
		}
		if (population_engine_is_running()) {
			clif_displaymessage(fd, "Population engine is already running. Stop with @populate stop");
			return -1;
		}
		if (quantity > 5000) {
			clif_displaymessage(fd, "Quantity too large (max 5000)");
			return -1;
		}
		int16_t map_id = sd->m;
		if (map_name[0]) {
			map_id = map_mapname2mapid(map_name);
			if (map_id < 0) {
				clif_displaymessage(fd, "Invalid map name");
				return -1;
			}
		}
		PopulationEngineConfig config;
		config.num_units = quantity;
		config.map_id = map_id;
		config.spread_units = true;
		config.spawn_x = 0;
		config.spawn_y = 0;
		PopulationEngineStats stats;
		if (population_engine_start(config, stats)) {
			char msg[256];
			sprintf(msg, "Population PCs spawned: %u on map %d", stats.total_created, map_id);
			clif_displaymessage(fd, msg);
		} else {
			clif_displaymessage(fd, "Failed to spawn population PCs");
			return -1;
		}
	}
	return 0;
}
