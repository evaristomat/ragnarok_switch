use crate::App;
use models::enums::skill_enums::SkillEnum;
use nostalre_game::banner::BannerKind;
use nostalre_game::data_table::msg_string_table::MsgLine;
use nostalre_game::entity::ChatBubbleState;
use nostalre_game::event::{FameKind, MvpFeedbackKind, SpecialItemSource};
use nostalre_game::skill_msg::{SKILL_MSG_COLOR, skill_msg_line};
use nostalre_ui_component::game::chat_room_member_window::{OTHER_MSG_COLOR, OWN_MSG_COLOR};
use nostalre_ui_component::game::chat_window::ChatChannel;
use nostalre_ui_component::helper::colors::{CYAN, GREEN, RED, YELLOW};

const MSI_PLAYERS_CONNECTED: u16 = 178;
/// "%s has opened [%s] and received [%s].": a box's or a monster's announced item (0x07fd's handler 0x7b7ef0)
const MSG_SPECIAL_ITEM_FROM: u16 = 0x65c;
/// "Congratulation %s,  Acquired [%s] !": an NPC's announced item (0x7b7ef0)
const MSG_SPECIAL_ITEM_GIVEN: u16 = 0x74c;

const MSI_IMPOSSIBLE_TELEPORT_AREA: u16 = 500;
const MSI_IMPOSSIBLE_MEMO_AREA: u16 = 501;
const MSI_IMPOSSIBLE_SKILL_AREA: u16 = 1334;
const MSI_IMPOSSIBLE_USEITEM_AREA: u16 = 1335;

const MSI_ERR_ATTACK_ARROW: u16 = 242;
const MSI_ERR_ATTACK_WEIGHT: u16 = 243;
const MSI_ERR_SKILL_WEIGHT: u16 = 244;
const MSI_ARROW_EQUIPMENT_SUCCESS: u16 = 245;
const MSI_KNIFE_EQUIPMENT_SUCCESS: u16 = 1040;
const MSI_BULLET_EQUIPMENT_SUCCESS: u16 = 1175;

pub(super) const ARROWFAIL_SUCCESS: i16 = 3;

const JOB_ASSASSIN: u16 = 12;
const JOB_GUNSLINGER: u16 = 24;
const JOB_NINJA: u16 = 25;
const JOB_ASSASSIN_CROSS: u16 = 4013;

const PALE_CYAN: [f32; 4] = [100.0 / 255.0, 1.0, 1.0, 1.0];
/// rAthena's note on 0x07e6: purple, 0xCDCDFF
const SERVER_SKILL_MSG_COLOR: [f32; 4] = [205.0 / 255.0, 205.0 / 255.0, 1.0, 1.0];

fn bgr(color: u32) -> [f32; 4] {
    [
        (color & 0xff) as f32 / 255.0,
        ((color >> 8) & 0xff) as f32 / 255.0,
        ((color >> 16) & 0xff) as f32 / 255.0,
        1.0,
    ]
}

fn map_info_notice(atype: i16) -> Option<(u16, [f32; 4])> {
    match atype {
        0 => Some((MSI_IMPOSSIBLE_TELEPORT_AREA, RED)),
        1 => Some((MSI_IMPOSSIBLE_MEMO_AREA, CYAN)),
        2 => Some((MSI_IMPOSSIBLE_SKILL_AREA, CYAN)),
        3 => Some((MSI_IMPOSSIBLE_USEITEM_AREA, CYAN)),
        _ => None,
    }
}

fn action_failure_notice(error_code: i16, job: u16) -> Option<(u16, [f32; 4])> {
    match error_code {
        0 => Some((MSI_ERR_ATTACK_ARROW, RED)),
        1 => Some((MSI_ERR_ATTACK_WEIGHT, PALE_CYAN)),
        2 => Some((MSI_ERR_SKILL_WEIGHT, PALE_CYAN)),
        // the job line names the ammunition: a Guillotine Cross's knives, a Rebellion's bullets
        ARROWFAIL_SUCCESS => match nostalre_game::sprite_path::classic_job(job) {
            JOB_ASSASSIN | JOB_ASSASSIN_CROSS => Some((MSI_KNIFE_EQUIPMENT_SUCCESS, PALE_CYAN)),
            JOB_GUNSLINGER => Some((MSI_BULLET_EQUIPMENT_SUCCESS, PALE_CYAN)),
            JOB_NINJA => None,
            _ => Some((MSI_ARROW_EQUIPMENT_SUCCESS, PALE_CYAN)),
        },
        _ => None,
    }
}

/// The notice window's colour for an item announce: 0x7b7ef0 sets it to COLORREF 0x62b0ff, #FFB062
const ANNOUNCE_ORANGE: [f32; 4] = [1.0, 0xb0 as f32 / 255.0, 0x62 as f32 / 255.0, 1.0];

impl App {
    pub(super) fn handle_broadcast_message(
        &mut self,
        message: String,
        color: [f32; 4],
        banner: BannerKind,
    ) {
        match banner {
            BannerKind::Once => {
                self.game.broadcast.banner.enqueue(message, 1);
            }
            BannerKind::Repeat(times) => {
                self.game.broadcast.banner.enqueue(message.clone(), times);
                self.windows
                    .chat_window
                    .add_message(message, color, ChatChannel::System);
            }
            BannerKind::None => {
                self.game.broadcast.notice.show(message.clone(), color);
                self.windows
                    .chat_window
                    .add_message(message, color, ChatChannel::System);
            }
        }
    }

    pub(super) fn handle_chat_message(&mut self, gid: u32, message: String) {
        if let Some(bubble_text) = message.split(" : ").nth(1)
            && let Some(entity) = self.game.world.entities.get_mut(gid)
        {
            entity.chat_bubble = Some(ChatBubbleState::new(bubble_text.to_string()));
        }
        if self.windows.chat_room_member_window.is_open()
            && let Some((sender, _)) = message.split_once(" : ")
            && self.windows.chat_room_member_window.has_member(sender)
        {
            self.windows
                .chat_room_member_window
                .push_message(message.clone(), OTHER_MSG_COLOR);
        }
        let is_own = self
            .game
            .session
            .selected_character
            .as_ref()
            .zip(message.split_once(" : "))
            .is_some_and(|(c, (sender, _))| sender == c.name);
        if is_own || !self.game.prefs.hide_public_chat {
            if self.config.is_gm_account(gid) {
                self.windows
                    .chat_window
                    .add_chat_colored(message, nostalre_game::targeting::GM_TEXT_COLOR);
            } else {
                self.windows.chat_window.add_chat(message);
            }
        }
    }

    pub(crate) fn show_ranking_once_named(&mut self) {
        let character = &mut self.game.character;
        let Some(lines) = character
            .pending_ranking
            .as_ref()
            .and_then(|r| r.lines(&character.char_names))
        else {
            return;
        };
        character.pending_ranking = None;
        for line in lines {
            self.windows.chat_window.add_system(line);
        }
    }

    pub(super) fn handle_own_chat_message(&mut self, message: String) {
        if let Some(bubble_text) = message.split(" : ").nth(1)
            && let Some(player_id) = self.game.world.entities.player_id()
            && let Some(entity) = self.game.world.entities.get_mut(player_id)
        {
            entity.chat_bubble = Some(ChatBubbleState::new(bubble_text.to_string()));
        }
        if self.windows.chat_room_member_window.is_open() {
            self.windows
                .chat_room_member_window
                .push_message(message.clone(), OWN_MSG_COLOR);
        }
        let self_is_gm = self
            .game
            .world
            .entities
            .player_id()
            .and_then(|id| self.game.world.entities.get(id))
            .is_some_and(|e| e.is_gm);
        if self_is_gm {
            self.windows
                .chat_window
                .add_chat_colored(message, nostalre_game::targeting::GM_TEXT_COLOR);
        } else {
            self.windows.chat_window.add_own_chat(message);
        }
    }

    pub(super) fn handle_guild_chat_message(&mut self, message: String) {
        self.windows.chat_window.add_guild(message);
    }

    pub(super) fn handle_whisper_received(&mut self, sender: String, message: String) {
        self.windows.chat_window.remember_whisper(sender.clone());
        self.windows.chat_window.add_whisper_in(sender, message);
    }

    pub(super) fn handle_whisper_ack(&mut self, result: u8) {
        let sent = self.game.pending_confirms.pending_whisper_out.take();
        let text = match result {
            0 => {
                if let Some((name, message)) = sent {
                    self.windows.chat_window.add_whisper_out(name, message);
                }
                return;
            }
            1 => "The target character is not logged in.",
            2 => "The target character is ignoring whispers.",
            3 => "The target character is ignoring everyone.",
            _ => "Failed to send whisper.",
        };
        let text = match sent {
            Some((name, _)) => format!("({name}) {text}"),
            None => text.to_string(),
        };
        self.windows.chat_window.add_error(text);
    }

    pub(super) fn handle_exp_gained(
        &mut self,
        aid: u32,
        amount: i64,
        is_base: bool,
        is_quest: bool,
    ) {
        if !self.game.prefs.show_exp {
            return;
        }
        let percentage = self.game.character.exp_gain_percentage(amount, is_base);
        let kind = if is_base { "base" } else { "job" };
        self.windows.chat_window.add_message(
            format!("Gained {amount} {kind} experience ({percentage:.2}%)"),
            GREEN,
            ChatChannel::System,
        );
    }

    pub(super) fn handle_whisper_setting_result(&mut self, allow: bool, result: u8, all: bool) {
        let pending = self.game.pending_confirms.pending_whisper_block.take();
        let block = !allow;
        if all {
            let message = match (result, block) {
                (0, true) => "Blocking all whispers.",
                (0, false) => "Accepting all whispers.",
                _ => "Could not change your whisper setting.",
            };
            self.windows.chat_window.add_system(message.to_string());
            return;
        }
        let Some((name, _)) = pending else {
            return;
        };
        match result {
            0 => {
                self.game.prefs.blocked_whispers.retain(|n| n != &name);
                if block {
                    self.game.prefs.blocked_whispers.push(name.clone());
                }
                let verb = if block { "Blocked" } else { "Unblocked" };
                self.windows
                    .chat_window
                    .add_system(format!("{verb} whispers from {name}."));
            }
            2 => self
                .windows
                .chat_window
                .add_error("Your block list is full.".to_string()),
            _ => self
                .windows
                .chat_window
                .add_error(format!("Could not change whisper setting for {name}.")),
        }
    }

    pub(super) fn handle_memo_result(&mut self, result: u8) {
        match result {
            0 => self
                .windows
                .chat_window
                .add_system("Saved location as a Memo Point for Warp skill.".to_string()),
            1 => self
                .windows
                .chat_window
                .add_error("Skill Level is not high enough.".to_string()),
            _ => self
                .windows
                .chat_window
                .add_error("You haven't learned Warp.".to_string()),
        }
    }

    /// A msgstringtable line the server named, with its arguments; logged when the table lacks it
    pub(super) fn server_msg_line(&self, msg_id: u16, args: &[&str]) -> Option<String> {
        let line = self.game.data_table.msg(msg_id, args);
        if line.is_none() {
            tracing::debug!("Unknown server msg id {msg_id}");
        }
        line
    }

    pub(super) fn handle_server_msg(&mut self, msg_id: u16, args: &[&str]) {
        let Some(message) = self.server_msg_line(msg_id, args) else {
            return;
        };
        if nostalre_game::data_table::msg_string_table::is_error_msg(msg_id) {
            self.windows.chat_window.add_error(message);
        } else {
            self.windows.chat_window.add_notice(message);
        }
    }

    pub(super) fn handle_server_msg_color(&mut self, msg_id: u16, color: u32) {
        self.add_msg_string_line(msg_id, &[], bgr(color));
    }

    /// "[Skill] message", as rAthena documents 0x07e6; an id no skill has shows the message alone
    pub(super) fn handle_server_skill_msg(&mut self, skill_id: u16, msg_id: u16) {
        let Some(message) = self.server_msg_line(msg_id, &[]) else {
            return;
        };
        let line = match SkillEnum::try_from_value(u32::from(skill_id)) {
            Ok(skill) => format!("[{}] {message}", self.game.data_table.skill_name_for(skill)),
            Err(_) => message,
        };
        self.windows
            .chat_window
            .add_message(line, SERVER_SKILL_MSG_COLOR, ChatChannel::System);
    }

    pub(super) fn handle_map_info_notice(&mut self, atype: i16) {
        let Some((msg_id, color)) = map_info_notice(atype) else {
            return;
        };
        self.add_msg_string_line(msg_id, &[], color);
    }

    pub(super) fn handle_action_failure(&mut self, error_code: i16) {
        let job = self.game.world.entities.player_job();
        let Some((msg_id, color)) = action_failure_notice(error_code, job) else {
            return;
        };
        self.add_msg_string_line(msg_id, &[], color);
    }

    pub(super) fn handle_server_colored_message(
        &mut self,
        account_id: u32,
        color: u32,
        message: String,
    ) {
        let key = self.game.world.entities.resolve_key(account_id);
        if let Some(entity) = self.game.world.entities.get_mut(key) {
            entity.chat_bubble = Some(ChatBubbleState::new(message.clone()));
        }
        self.windows
            .chat_window
            .add_message(message, bgr(color), ChatChannel::System);
    }

    /// A line in kRO's own colour for it, as its exe shows it
    pub(super) fn add_msg_line(&mut self, line: MsgLine, args: &[&str]) {
        self.add_msg_string_line(line.msg_id, args, line.color);
    }

    /// An NPC's navigation guide (0x08e2), a first version of ours: where it points, said in chat, and its cell marked
    /// on the minimap while on its map (kRO's own is its navigation window, 0x685be0, with routes from its data)
    pub(super) fn handle_navigation_guide(
        &mut self,
        navigation: nostalre_game::navigation::Navigation,
    ) {
        use nostalre_game::navigation::NavTarget;
        let map = self
            .game
            .data_table
            .map_name
            .as_ref()
            .and_then(|t| t.display_name(&navigation.map))
            .unwrap_or(&navigation.map)
            .to_string();
        let line = match navigation.target {
            NavTarget::Cell { x, y } => format!("Navigation: {map} ({x}, {y})"),
            NavTarget::Map => format!("Navigation: {map}"),
            NavTarget::Monster { mob_id } => format!("Navigation: monster {mob_id} in {map}"),
        };
        self.windows.chat_window.add_system(line);
        self.game.navigation = Some(navigation);
    }

    pub(crate) fn add_msg_string_line(&mut self, msg_id: u16, args: &[&str], color: [f32; 4]) {
        let Some(line) = self.server_msg_line(msg_id, args) else {
            return;
        };
        self.windows
            .chat_window
            .add_message(line, color, ChatChannel::System);
    }

    /// kRO's exe says an announced item in yellow, with the box item's or the monster's name between the player's
    /// and the item's, and writes it in its notice window in orange (0x7b7ef0); refine is not in its line
    pub(super) fn handle_special_item_obtained(
        &mut self,
        player: &str,
        item_id: u32,
        source: SpecialItemSource,
    ) {
        let data = &self.game.data_table;
        let item = data.item_name_for(item_id, true);
        let (msg_id, from) = match source {
            SpecialItemSource::Box(box_id) => (
                MSG_SPECIAL_ITEM_FROM,
                Some(data.item_name_for(box_id, true)),
            ),
            SpecialItemSource::Monster(name) => (MSG_SPECIAL_ITEM_FROM, Some(name)),
            SpecialItemSource::Npc => (MSG_SPECIAL_ITEM_GIVEN, None),
        };
        let args: Vec<&str> = std::iter::once(player)
            .chain(from.as_deref())
            .chain(std::iter::once(item.as_str()))
            .collect();
        let Some(line) = self.server_msg_line(msg_id, &args) else {
            return;
        };
        self.game
            .broadcast
            .notice
            .show(line.clone(), ANNOUNCE_ORANGE);
        self.windows
            .chat_window
            .add_message(line, YELLOW, ChatChannel::System);
    }

    /// A special pop-up (0x0bbe) as the pinned client's table has it: its lines in chat in its colour; its floating
    /// kind not drawn yet
    pub(super) fn handle_special_popup(&mut self, id: i32) {
        let popup = u32::try_from(id)
            .ok()
            .and_then(|id| self.game.data_table.special_popups.get(&id))
            .cloned();
        let Some(popup) = popup else {
            tracing::debug!(id, "special popup the table has not");
            return;
        };
        if popup.kind.in_chat() {
            for line in popup.lines {
                self.windows
                    .chat_window
                    .add_message(line, popup.color, ChatChannel::System);
            }
        }
        if popup.kind.floating() {
            tracing::debug!(id, "floating special popup not drawn yet");
        }
    }

    pub(super) fn handle_user_count(&mut self, count: i32) {
        let line = self
            .game
            .data_table
            .msg(MSI_PLAYERS_CONNECTED, &[&count.to_string()])
            .unwrap_or_else(|| format!("There are {count} Players Currently Connected."));
        self.windows.chat_window.add_notice(line);
    }

    pub(super) fn handle_skill_msg(&mut self, msg_no: i32) {
        let Some(line) = skill_msg_line(msg_no) else {
            tracing::debug!("Unknown skill msg {msg_no}");
            return;
        };
        self.windows.chat_window.add_message(
            line.to_string(),
            SKILL_MSG_COLOR,
            ChatChannel::System,
        );
        self.game.broadcast.poptip.push(line.to_string());
    }

    pub(super) fn handle_mvp_feedback(&mut self, kind: MvpFeedbackKind) {
        let (line, color) = match kind {
            MvpFeedbackKind::Item { item_id } => {
                let name = self.item_name(item_id);
                (
                    format!("Congratulations! You are the MVP! Your reward item is {name} !!"),
                    CYAN,
                )
            }
            MvpFeedbackKind::ItemDropped => (
                "You are the MVP, but cannot obtain the reward because you are overweight."
                    .to_string(),
                RED,
            ),
        };
        self.windows
            .chat_window
            .add_message(line, color, ChatChannel::System);
    }

    pub(super) fn handle_fame_points_gained(&mut self, kind: FameKind, point: i32, total: i32) {
        self.windows
            .chat_window
            .add_system(kind.point_line(point, total));
    }

    pub(super) fn handle_pvp_points_received(&mut self, win: i32, lose: i32, point: i32) {
        self.windows.chat_window.add_message(
            format!("You have {win} win(s), {lose} loss(es) and {point} PvP point(s)."),
            CYAN,
            ChatChannel::System,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_info_notice_pairs_each_type_with_its_line_and_colour() {
        assert_eq!(map_info_notice(0), Some((500, RED)));
        assert_eq!(map_info_notice(1), Some((501, CYAN)));
        assert_eq!(map_info_notice(2), Some((1334, CYAN)));
        assert_eq!(map_info_notice(3), Some((1335, CYAN)));
        assert_eq!(map_info_notice(4), None);
    }

    #[test]
    fn action_failure_notice_names_the_ammunition_of_the_job() {
        assert_eq!(action_failure_notice(0, JOB_ASSASSIN), Some((242, RED)));
        assert_eq!(action_failure_notice(1, JOB_NINJA), Some((243, PALE_CYAN)));
        assert_eq!(action_failure_notice(2, JOB_NINJA), Some((244, PALE_CYAN)));

        assert_eq!(action_failure_notice(3, 3), Some((245, PALE_CYAN)));
        assert_eq!(
            action_failure_notice(3, JOB_ASSASSIN),
            Some((1040, PALE_CYAN))
        );
        // the baby and later jobs of a line load its ammunition: a Baby Assassin, a Guillotine Cross, a Rebellion,
        // a Kagerou
        for (job, notice) in [
            (4035, Some(1040)),
            (4059, Some(1040)),
            (JOB_GUNSLINGER, Some(1175)),
            (4215, Some(1175)),
            (JOB_NINJA, None),
            (4211, None),
        ] {
            assert_eq!(
                action_failure_notice(3, job),
                notice.map(|id| (id, PALE_CYAN)),
                "job {job}"
            );
        }

        assert_eq!(action_failure_notice(4, JOB_NINJA), None);
    }

    #[test]
    fn bgr_reads_blue_from_the_high_byte() {
        assert_eq!(bgr(0x00_00_00_ff), RED);
        assert_eq!(bgr(0x00_ff_ff_00), CYAN);
    }
}
