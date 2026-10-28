use crate::App;
use nostalre_game::entity::EntityState;
use nostalre_game::event::SelfConfigKind;
use nostalre_network::{
    build_action_request_packet, build_change_direction_packet, build_chat_packet,
    build_config_packet, build_doridori_packet, build_emotion_packet, build_exit_room_packet,
    build_give_manner_byname_packet, build_guild_chat_packet, build_leave_party_packet,
    build_lesseffect_packet, build_make_guild, build_make_party_packet, build_party_chat_packet,
    build_party_invite_by_name_packet, build_remember_warppoint_packet,
    build_req_disorganize_guild, build_setting_whisper_pc_packet,
    build_setting_whisper_state_packet, build_stat_change_packet, build_status_gm_packet,
    build_user_count_packet, build_whisper_packet,
};

/// A chat packet's builder, from the line and the packet version
type ChatPacket = fn(&str, u32) -> Vec<u8>;

#[cfg(test)]
mod command_repeat_regression {
    use super::*;
    use crate::{Config,NetworkCommand};

    #[test]
    fn reopening_campaign_and_other_menu_commands_never_uses_chat_repeat_budget() {
        let mut app=App::new(Config::default());
        let (tx,mut rx)=tokio::sync::mpsc::unbounded_channel();
        app.channel.cmd_tx=Some(tx);
        let mut expected=0;
        for command in ["@campanha","@jobchanger","@go 0","@warp prontera 149 178"] {
            for _ in 0..4 {app.run_chat_command(command);expected+=1;}
        }
        let mut sent=0;
        while let Ok(NetworkCommand::SendPacket(_))=rx.try_recv() {sent+=1;}
        assert_eq!(sent,expected,"Menu commands were blocked as repeated public chat");
        app.run_chat_command("oi");app.run_chat_command("oi");
        app.run_chat_command("@campanha");
        app.run_chat_command("oi");
        let mut sent=0;
        while let Ok(NetworkCommand::SendPacket(_))=rx.try_recv() {sent+=1;}
        assert_eq!(sent,3,"Menu actions must neither consume nor reset public chat protection");
    }
}

impl App {
    pub(crate) fn run_chat_command(&mut self, message: &str) {
        if message.is_empty() {
            return;
        }
        if message.starts_with('/') {
            self.handle_slash_command(message);
            return;
        }
        let pv = self.active_packetver;
        let (body, build, public): (&str, ChatPacket, bool) =
            if let Some(party_msg) = message.strip_prefix('%') {
                if self.game.party.is_none() {
                    self.windows
                        .chat_window
                        .add_system("You are not in a party.".to_string());
                    return;
                }
                (party_msg.trim_start(), build_party_chat_packet, false)
            } else if let Some(guild_msg) = message.strip_prefix('$') {
                if self.game.guild.is_none() {
                    self.windows
                        .chat_window
                        .add_system("You are not in a guild.".to_string());
                    return;
                }
                (guild_msg.trim_start(), build_guild_chat_packet, false)
            } else {
                (message, build_chat_packet, true)
            };
        // Server commands are control actions (also used by our controller
        // menus), not public speech. They must not consume or reset its budget.
        let server_command=public && body.starts_with('@');
        if !server_command && !self.admit_chat_line(body) {
            return;
        }
        let char_name = self
            .game
            .session
            .selected_character
            .as_ref()
            .map(|c| c.name.as_str())
            .unwrap_or("Unknown");
        self.channel
            .send_packet(build(&format!("{char_name} : {body}"), pv));
        if public && !server_command {
            self.hear_prayer(message);
        }
    }

    /// Whether a line may be sent: kRO refuses the same line a third time in a row, on any channel, with line 49 in
    /// red (0x7899d0)
    pub(crate) fn admit_chat_line(&mut self, line: &str) -> bool {
        let admitted = self.game.repeated_line.admit(line);
        if !admitted {
            self.add_msg_string_line(
                nostalre_game::chat_repeat::MSG_SAME_LINE,
                &[],
                nostalre_ui_component::helper::colors::RED,
            );
        }
        admitted
    }

    /// A line the player said, kept for a Super Novice's prayer: when it completes it, as kRO's chat input does
    /// (0x789f10), nothing while dead, line 2015 in red at the job's highest level, else 0x01ed
    fn hear_prayer(&mut self, line: &str) {
        use nostalre_game::prayer::{
            MSG_PRAYER_ANGEL, MSG_PRAYER_HELP, MSG_PRAYER_MAX_LEVEL, is_super_novice, level_cap,
        };
        self.game.prayer_lines.push(line);
        let job = self.game.world.entities.player_job();
        if !is_super_novice(job) {
            return;
        }
        let table = &self.game.data_table;
        let (Some(angel), Some(help)) = (
            table.msg(MSG_PRAYER_ANGEL, &[]),
            table.msg(MSG_PRAYER_HELP, &[]),
        ) else {
            return;
        };
        if !self.game.prayer_lines.prayed(&angel, &help) {
            return;
        }
        if !self
            .game
            .world
            .entities
            .player()
            .is_some_and(|p| p.is_alive())
        {
            return;
        }
        if level_cap(job).is_some_and(|cap| self.game.character.base_level >= cap) {
            self.add_msg_string_line(
                MSG_PRAYER_MAX_LEVEL,
                &[],
                nostalre_ui_component::helper::colors::RED,
            );
            return;
        }
        self.channel
            .send_packet(nostalre_network::build_novice_prayer_packet(
                self.active_packetver,
            ));
    }

    fn turn_body(&mut self, step: u8) {
        let pv = self.active_packetver;
        if let Some(entity) = self.game.world.entities.player_mut() {
            entity.set_facing((entity.direction + step) % 8);
            entity.head_dir = 0;
            let (head_dir, dir) = (entity.head_dir, entity.direction);
            self.channel
                .send_packet(build_change_direction_packet(head_dir, dir, pv));
        }
    }

    fn handle_slash_command(&mut self, command: &str) {
        use nostalre_game::chat_command::{ChatCommand, parse_chat_command};
        let pv = self.active_packetver;
        match parse_chat_command(command) {
            ChatCommand::Sit => {
                if self.player_hidden() {
                    return;
                }
                if let Some(entity) = self.game.world.entities.player() {
                    let action = if entity.state() == EntityState::Sitting {
                        3u8
                    } else {
                        2u8
                    };
                    self.channel
                        .send_packet(build_action_request_packet(0, action, pv));
                }
            }
            ChatCommand::Stand => {
                if self.player_hidden() {
                    return;
                }
                let sitting = self
                    .game
                    .world
                    .entities
                    .player()
                    .is_some_and(|e| e.state() == EntityState::Sitting);
                if sitting {
                    self.channel
                        .send_packet(build_action_request_packet(0, 3u8, pv));
                }
            }
            ChatCommand::Doridori => {
                let Some(entity) = self.game.world.entities.player_mut() else {
                    return;
                };
                entity.head_dir = if entity.head_dir == 1 { 2 } else { 1 };
                let (head_dir, dir) = (entity.head_dir, entity.direction);
                let sitting = entity.state() == EntityState::Sitting;
                self.channel
                    .send_packet(build_change_direction_packet(head_dir, dir, pv));
                if sitting {
                    let now_ms = self.start_time.elapsed().as_millis() as u32;
                    if self.game.session.doridori.record_flip(now_ms) {
                        self.channel.send_packet(build_doridori_packet(pv));
                        self.game.session.doridori.reset();
                    }
                }
            }
            ChatCommand::BingBing => self.turn_body(1),
            ChatCommand::BangBang => self.turn_body(7),
            ChatCommand::Where => {
                match (
                    self.game.session.current_map.as_ref(),
                    self.game.world.entities.player(),
                ) {
                    (Some(map_name), Some(player)) => {
                        let (x, y) = player.movement.cell_position();
                        let message = format!("{map_name}.gat ({x}, {y})");
                        self.windows.chat_window.add_system(message);
                    }
                    _ => {
                        self.windows
                            .chat_window
                            .add_system("You are not in a map yet.".to_string());
                    }
                }
            }
            ChatCommand::Memo => {
                self.channel
                    .send_packet(build_remember_warppoint_packet(pv));
            }
            ChatCommand::ExitRoom => {
                self.channel.send_packet(build_exit_room_packet(pv));
            }
            ChatCommand::LeaveParty => {
                if self.game.party.is_some() {
                    self.channel.send_packet(build_leave_party_packet(pv));
                } else {
                    self.windows
                        .chat_window
                        .add_system("You are not in a party.".to_string());
                }
            }
            ChatCommand::MakeParty(name) => {
                if name.is_empty() {
                    self.windows
                        .chat_window
                        .add_system("Usage: /organize <party name>".to_string());
                } else if self.game.party.is_some() {
                    self.windows
                        .chat_window
                        .add_system("You are already in a party.".to_string());
                } else {
                    self.channel.send_packet(build_make_party_packet(&name, pv));
                }
            }
            ChatCommand::InviteParty(name) => {
                if name.is_empty() {
                    self.windows
                        .chat_window
                        .add_system("Usage: /invite <character name>".to_string());
                } else {
                    self.channel
                        .send_packet(build_party_invite_by_name_packet(&name, pv));
                }
            }
            ChatCommand::MakeGuild(name) => {
                const EMPERIUM_ITEM_ID: u32 = 714;
                let has_emperium = self
                    .game
                    .character
                    .inventory
                    .all_items()
                    .iter()
                    .any(|i| i.item_id == EMPERIUM_ITEM_ID);
                let gid = self
                    .game
                    .session
                    .login_session
                    .as_ref()
                    .map(|s| s.account_id)
                    .unwrap_or(0);
                if name.is_empty() {
                    self.windows
                        .chat_window
                        .add_system("Usage: /guild <guild name>".to_string());
                } else if self.game.guild.is_some() {
                    self.windows
                        .chat_window
                        .add_system("You are already in a guild.".to_string());
                } else if !has_emperium {
                    self.windows
                        .chat_window
                        .add_system("You need an Emperium to create a guild.".to_string());
                } else {
                    self.channel.send_packet(build_make_guild(gid, &name, pv));
                }
            }
            ChatCommand::BreakGuild(name) => {
                if name.is_empty() {
                    self.windows
                        .chat_window
                        .add_system("Usage: /breakguild <guild name>".to_string());
                } else if self.game.guild.is_none() {
                    self.windows
                        .chat_window
                        .add_system("You are not in a guild.".to_string());
                } else {
                    self.channel
                        .send_packet(build_req_disorganize_guild(&name, pv));
                }
            }
            ChatCommand::StatUp { status_id, amount } => {
                self.channel.send_packet(build_stat_change_packet(
                    status_id,
                    amount.min(u8::MAX as u32) as u8,
                    pv,
                ));
            }
            ChatCommand::ToggleEffect => {
                let show = !self.config.show_skill_effects;
                self.apply_graphics_settings(
                    nostalre_game::display::GraphicsSettings {
                        show_skill_effects: show,
                        ..self.config.graphics()
                    },
                    true,
                );
                self.channel.send_packet(build_lesseffect_packet(!show, pv));
                let status = if show { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("Skill effects: {status}"));
            }
            ChatCommand::ToggleFog => {
                let fog = !self.config.fog;
                self.apply_graphics_settings(
                    nostalre_game::display::GraphicsSettings {
                        fog: fog,
                        ..self.config.graphics()
                    },
                    true,
                );
                let status = if fog { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("Fog: {status}"));
            }
            ChatCommand::ToggleLightmap => {
                if let Some(renderer) = &mut self.renderer {
                    let enabled = renderer.toggle_lightmap();
                    let status = if enabled { "ON" } else { "OFF" };
                    self.windows
                        .chat_window
                        .add_system(format!("Lightmap: {status}"));
                }
            }
            ChatCommand::ToggleHdWater => {
                if let Some(renderer) = &mut self.renderer {
                    let enabled = renderer.toggle_hd_water();
                    let status = if enabled { "HD" } else { "classic" };
                    self.windows
                        .chat_window
                        .add_system(format!("Water: {status}"));
                }
            }
            ChatCommand::ToggleAura => {
                let mut display = self.config.display.clone();
                display.show_level_aura = !display.show_level_aura;
                let status = if display.show_level_aura { "ON" } else { "OFF" };
                self.apply_graphics_settings(
                    nostalre_game::display::GraphicsSettings {
                        display: display,
                        ..self.config.graphics()
                    },
                    true,
                );
                self.windows
                    .chat_window
                    .add_system(format!("Level aura: {status}"));
            }
            ChatCommand::ToggleNoTrade => {
                let refuse = !self.config.refuse_trade;
                self.apply_graphics_settings(
                    nostalre_game::display::GraphicsSettings {
                        refuse_trade: refuse,
                        ..self.config.graphics()
                    },
                    true,
                );
                let status = if refuse { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("Refuse trade requests: {status}"));
            }
            ChatCommand::RefuseParty(refuse) => {
                self.apply_graphics_settings(
                    nostalre_game::display::GraphicsSettings {
                        refuse_party_invite: refuse,
                        ..self.config.graphics()
                    },
                    true,
                );
                let status = if refuse { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("Refuse party invites: {status}"));
            }
            ChatCommand::ToggleNoShift => {
                self.game.prefs.noshift_mode = !self.game.prefs.noshift_mode;
                let status = if self.game.prefs.noshift_mode {
                    "ON"
                } else {
                    "OFF"
                };
                self.windows
                    .chat_window
                    .add_system(format!("No-shift mode: {status}"));
            }
            ChatCommand::ToggleNoCtrl => {
                self.game.prefs.noctrl_mode = !self.game.prefs.noctrl_mode;
                let status = if self.game.prefs.noctrl_mode {
                    "ON"
                } else {
                    "OFF"
                };
                self.windows
                    .chat_window
                    .add_system(format!("No-ctrl mode: {status}"));
            }
            ChatCommand::ToggleBgm => {
                self.config.bgm_enabled = !self.config.bgm_enabled;
                self.sound.set_volumes(
                    self.config.effective_bgm_volume(),
                    self.config.effective_sfx_volume(),
                );
                self.config.save("config.json");
                let status = if self.config.bgm_enabled { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("Background music: {status}"));
            }
            ChatCommand::ToggleSound => {
                self.config.sfx_enabled = !self.config.sfx_enabled;
                self.sound.set_volumes(
                    self.config.effective_bgm_volume(),
                    self.config.effective_sfx_volume(),
                );
                self.config.save("config.json");
                let status = if self.config.sfx_enabled { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("Sound effects: {status}"));
            }
            ChatCommand::SetBgmVolume(vol) => {
                self.config.bgm_volume = vol as f32 / 127.0;
                self.sound.set_volumes(
                    self.config.effective_bgm_volume(),
                    self.config.effective_sfx_volume(),
                );
                self.config.save("config.json");
                self.windows
                    .chat_window
                    .add_system(format!("BGM volume: {vol}"));
            }
            ChatCommand::SetSfxVolume(vol) => {
                self.config.sfx_volume = vol as f32 / 127.0;
                self.sound.set_volumes(
                    self.config.effective_bgm_volume(),
                    self.config.effective_sfx_volume(),
                );
                self.config.save("config.json");
                self.windows
                    .chat_window
                    .add_system(format!("Sound volume: {vol}"));
            }
            ChatCommand::ToggleShowExp => {
                self.game.prefs.show_exp = !self.game.prefs.show_exp;
                let status = if self.game.prefs.show_exp {
                    "ON"
                } else {
                    "OFF"
                };
                self.windows
                    .chat_window
                    .add_system(format!("Experience messages: {status}"));
            }
            ChatCommand::ToggleHidePublicChat => {
                self.game.prefs.hide_public_chat = !self.game.prefs.hide_public_chat;
                let status = if self.game.prefs.hide_public_chat {
                    "OFF"
                } else {
                    "ON"
                };
                self.windows
                    .chat_window
                    .add_system(format!("Public chat: {status}"));
            }
            ChatCommand::BattleMode => {
                self.game.character.hotkeys.toggle_battle_mode();
                let status = if self.game.character.hotkeys.battle_mode() {
                    "ON"
                } else {
                    "OFF"
                };
                self.windows
                    .chat_window
                    .add_system(format!("Battle Mode {status}"));
            }
            ChatCommand::Ranking(kind) => {
                self.channel
                    .send_packet(nostalre_network::build_ranking_request_packet(kind, pv));
            }
            ChatCommand::ToggleMiss => {
                self.game.prefs.show_miss = !self.game.prefs.show_miss;
                let status = if self.game.prefs.show_miss {
                    "ON"
                } else {
                    "OFF"
                };
                self.windows
                    .chat_window
                    .add_system(format!("Miss text: {status}"));
            }
            ChatCommand::ToggleMonsterSnap => {
                self.config.snap.monster_no_skill = !self.config.snap.monster_no_skill;
                let status = if self.config.snap.monster_no_skill {
                    "ON"
                } else {
                    "OFF"
                };
                self.config.save("config.json");
                self.windows
                    .chat_window
                    .add_system(format!("Monster cursor snap: {status}"));
            }
            ChatCommand::ToggleSkillSnap => {
                self.config.snap.monster_skill = !self.config.snap.monster_skill;
                let status = if self.config.snap.monster_skill {
                    "ON"
                } else {
                    "OFF"
                };
                self.config.save("config.json");
                self.windows
                    .chat_window
                    .add_system(format!("Skill cursor snap: {status}"));
            }
            ChatCommand::ToggleItemSnap => {
                self.config.snap.item = !self.config.snap.item;
                let status = if self.config.snap.item { "ON" } else { "OFF" };
                self.config.save("config.json");
                self.windows
                    .chat_window
                    .add_system(format!("Item cursor snap: {status}"));
            }
            ChatCommand::ToggleEqOpen => {
                // The server's setting, as the equipment window's Show Equip box shows it
                let open = !self.game.prefs.self_config.open_equipment_window;
                self.channel.send_packet(build_config_packet(
                    SelfConfigKind::OpenEquipmentWindow.config_id(),
                    open as i32,
                    pv,
                ));
                let status = if open { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("Equipment visible to others: {status}"));
            }
            ChatCommand::GuildChat(msg) => {
                if msg.is_empty() {
                    self.windows
                        .chat_window
                        .add_system("Usage: /gc <message>".to_string());
                } else if self.game.guild.is_none() {
                    self.windows
                        .chat_window
                        .add_system("You are not in a guild.".to_string());
                } else {
                    let char_name = self
                        .game
                        .session
                        .selected_character
                        .as_ref()
                        .map(|c| c.name.as_str())
                        .unwrap_or("Unknown");
                    let full_msg = format!("{char_name} : {msg}");
                    self.channel
                        .send_packet(build_guild_chat_packet(&full_msg, pv));
                }
            }
            ChatCommand::WhisperFriends(text) => {
                if text.is_empty() {
                    self.windows
                        .chat_window
                        .add_system("Usage: /hi <message>".to_string());
                } else {
                    let targets: Vec<String> = self
                        .game
                        .friends
                        .friends
                        .iter()
                        .filter(|f| f.online)
                        .map(|f| f.name.clone())
                        .collect();
                    if targets.is_empty() {
                        self.windows
                            .chat_window
                            .add_system("No friends are online.".to_string());
                    } else {
                        for name in targets {
                            self.channel
                                .send_packet(build_whisper_packet(&name, &text, pv));
                        }
                    }
                }
            }
            ChatCommand::WhisperBlock { name, block } => {
                if name.is_empty() {
                    self.windows
                        .chat_window
                        .add_system("Usage: /ex <character name>".to_string());
                } else {
                    self.channel
                        .send_packet(build_setting_whisper_pc_packet(&name, block, pv));
                    self.game.pending_confirms.pending_whisper_block = Some((name, block));
                }
            }
            ChatCommand::WhisperBlockAll(block) => {
                self.channel
                    .send_packet(build_setting_whisper_state_packet(block, pv));
            }
            ChatCommand::WhisperListBlocked => {
                if self.game.prefs.blocked_whispers.is_empty() {
                    self.windows
                        .chat_window
                        .add_system("No blocked players.".to_string());
                } else {
                    let list = self.game.prefs.blocked_whispers.join(", ");
                    self.windows
                        .chat_window
                        .add_system(format!("Blocked: {list}"));
                }
            }
            ChatCommand::OpenChatCreate => {
                self.windows.chat_room_create_window.toggle();
            }
            ChatCommand::OpenEmotionList => {
                self.windows.emotion_window.toggle();
            }
            ChatCommand::OpenCompanionAi { mercenary } => {
                self.windows
                    .companion_ai_config_window
                    .open_at_tab(if mercenary { 1 } else { 0 });
            }
            ChatCommand::Unsupported => {
                self.windows
                    .chat_window
                    .add_system("This command is not supported yet.".to_string());
            }
            ChatCommand::Emote(emote_type) => {
                self.channel
                    .send_packet(build_emotion_packet(emote_type, pv));
            }
            ChatCommand::UserCount => {
                self.channel.send_packet(build_user_count_packet(pv));
            }
            ChatCommand::CheckStatus(name) => {
                self.game.pending_confirms.pending_gm_check = Some(name.clone());
                self.channel.send_packet(build_status_gm_packet(&name, pv));
            }
            ChatCommand::ReportManner(name) => {
                self.channel
                    .send_packet(build_give_manner_byname_packet(&name, pv));
            }
            ChatCommand::ToggleShowPing => {
                self.game.show_ping = !self.game.show_ping;
                let status = if self.game.show_ping { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("Ping overlay: {status}"));
            }
            ChatCommand::ToggleShowFps => {
                self.game.show_fps = !self.game.show_fps;
                let status = if self.game.show_fps { "ON" } else { "OFF" };
                self.windows
                    .chat_window
                    .add_system(format!("FPS overlay: {status}"));
            }
            ChatCommand::Help => {
                self.windows.chat_window.add_system("Commands:".to_string());
                for (cmd, desc) in nostalre_game::chat_command::COMMAND_HELP {
                    self.windows
                        .chat_window
                        .add_system(format!("{cmd} - {desc}"));
                }
            }
            ChatCommand::Outdated => {
                self.windows
                    .chat_window
                    .add_system("This command is no longer available.".to_string());
            }
            ChatCommand::Unknown => {
                self.reply_unknown_command(command);
            }
        }
    }

    fn reply_unknown_command(&mut self, command: &str) {
        let cmd = command.split_whitespace().next().unwrap_or("");
        self.windows
            .chat_window
            .add_system(format!("Unknown command: {cmd}"));
    }
}
