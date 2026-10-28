use crate::App;
use nostalre_game::quest::{MARKER_NONE, Quest, QuestHuntEntry, QuestMarker, marker_sprite_action};
use nostalre_ui_component::game::chat_window::ChatChannel;

impl App {
    pub(super) fn handle_quest_list_received(&mut self, quests: Vec<Quest>) {
        self.game.quest_log.replace(quests);
        if let Some(character)=&self.game.session.selected_character {
            let path=format!("quest-follow-pre-{}.txt",character.gid);
            if let Ok(text)=std::fs::read_to_string(path) {
                if let Ok(id)=text.trim().parse::<u32>() {self.game.quest_log.follow(id);}
            }
        }
    }

    pub(super) fn handle_quest_added(&mut self, quest: Quest) {
        let title = self
            .game
            .data_table
            .quest_display
            .as_ref()
            .map(|t| t.title(quest.id))
            .unwrap_or_else(|| "Missão desconhecida".to_string());
        self.game.quest_log.add(quest);
        self.windows.chat_window.add_message(
            format!("Missão recebida: {title}"),
            [1.0, 1.0, 1.0, 1.0],
            ChatChannel::Public,
        );
    }

    pub(super) fn handle_quest_removed(&mut self, quest_id: u32) {
        self.game.quest_log.remove(quest_id);
    }

    pub(super) fn handle_quest_hunt_updated(&mut self, entries: Vec<QuestHuntEntry>) {
        for entry in entries {
            let before = self
                .game
                .quest_log
                .objective(entry.hunt_id)
                .map(|(quest_id, o)| (quest_id, o.current, o.name.clone()));
            self.game.quest_log.update_hunt(entry);
            if let Some((quest_id, old_current, name)) = before
                && entry.current > old_current
            {
                let title = self
                    .game
                    .data_table
                    .quest_display
                    .as_ref()
                    .map(|t| t.title(quest_id))
                    .unwrap_or_default();
                self.windows.chat_window.add_message(
                    format!(
                        "Missão [{title}]: {name} derrotado. ({}/{})",
                        entry.current, entry.required
                    ),
                    [1.0, 1.0, 1.0, 1.0],
                    ChatChannel::Public,
                );
            }
        }
    }

    pub(super) fn handle_quest_active_changed(&mut self, quest_id: u32, active: bool) {
        self.game.quest_log.set_active(quest_id, active);
    }

    pub(super) fn handle_quest_npc_marker(
        &mut self,
        npc_id: u32,
        x: u16,
        y: u16,
        effect: i16,
        color: u8,
    ) {
        if effect == MARKER_NONE {
            self.game.quest_markers.remove(&npc_id);
        } else {
            if marker_sprite_action(effect).is_none() {
                tracing::debug!(
                    npc_id,
                    effect,
                    "quest marker icon kRO draws with another effect, not drawn"
                );
            }
            self.game.quest_markers.insert(
                npc_id,
                QuestMarker {
                    x,
                    y,
                    effect,
                    color,
                },
            );
        }
    }
}
