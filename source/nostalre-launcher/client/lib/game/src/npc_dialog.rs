use std::time::{Duration, Instant};

/// How long a Next goes unanswered before its button comes back: the server ignores a Next sent from too
/// far from the NPC (npc_checknear) and its script keeps waiting, so the player has to be able to send it
/// again once nearer.
const UNANSWERED_NEXT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcDialogState {
    Idle,
    DisplayingText,
    WaitingForMenu,
    WaitingForNumberInput,
    WaitingForStringInput,
    WaitingForDealType,
}

#[derive(Debug)]
pub struct NpcDialogData {
    pub state: NpcDialogState,
    pub npc_id: u32,
    pub text: String,
    pub next_button: bool,
    pub close_button: bool,
    pub menu_items: Vec<String>,
    pub selected_menu_index: usize,
    pub menu_scroll_offset: usize,
    pub say_visible: bool,
    clear_text_on_next: bool,
    /// When the last Next went out, until the server answers it.
    next_sent_at: Option<Instant>,
}

impl Default for NpcDialogData {
    fn default() -> Self {
        Self::new()
    }
}

impl NpcDialogData {
    pub fn new() -> Self {
        Self {
            state: NpcDialogState::Idle,
            npc_id: 0,
            text: String::new(),
            next_button: false,
            close_button: false,
            menu_items: Vec::new(),
            selected_menu_index: 0,
            menu_scroll_offset: 0,
            say_visible: false,
            clear_text_on_next: false,
            next_sent_at: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.state != NpcDialogState::Idle
    }

    pub fn has_text(&self) -> bool {
        !self.text.is_empty()
    }

    /// A dialog packet from `npc_id` starts a new conversation when it is another NPC's, or comes after the
    /// server ended the last script (its Close showing): the last one's text, buttons and menu go (a warp away
    /// mid-dialog leaves them).
    fn begin(&mut self, npc_id: u32) {
        if self.state != NpcDialogState::Idle && (self.npc_id != npc_id || self.close_button) {
            self.close();
        }
    }

    pub fn open_text(&mut self, npc_id: u32, text: &str) {
        self.begin(npc_id);
        self.next_sent_at = None;
        if self.clear_text_on_next || self.state == NpcDialogState::Idle || self.npc_id != npc_id {
            self.text.clear();
            self.clear_text_on_next = false;
        }
        self.say_visible = true;
        self.npc_id = npc_id;
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(text);
        self.state = NpcDialogState::DisplayingText;
    }

    /// The script's `clear`: its text goes, the dialog stays open and the script carries on.
    pub fn clear_text(&mut self, npc_id: u32) {
        if self.npc_id == npc_id {
            self.text.clear();
        }
    }

    pub fn wait_for_next(&mut self, npc_id: u32) {
        self.begin(npc_id);
        self.next_sent_at = None;
        self.say_visible = true;
        self.npc_id = npc_id;
        if self.state == NpcDialogState::Idle {
            self.state = NpcDialogState::DisplayingText;
        }
        self.next_button = true;
    }

    pub fn wait_for_close(&mut self) {
        self.next_sent_at = None;
        if !self.say_visible {
            self.close();
            return;
        }
        self.close_button = true;
    }

    pub fn show_menu(&mut self, npc_id: u32, items: Vec<String>) {
        self.begin(npc_id);
        self.next_sent_at = None;
        self.npc_id = npc_id;
        self.menu_items = items;
        self.selected_menu_index = 0;
        self.menu_scroll_offset = 0;
        self.next_button = false;
        self.state = NpcDialogState::WaitingForMenu;
    }

    pub fn wait_for_number_input(&mut self, npc_id: u32) {
        self.begin(npc_id);
        self.npc_id = npc_id;
        self.state = NpcDialogState::WaitingForNumberInput;
    }

    pub fn wait_for_string_input(&mut self, npc_id: u32) {
        self.begin(npc_id);
        self.npc_id = npc_id;
        self.state = NpcDialogState::WaitingForStringInput;
    }

    pub fn show_deal_type(&mut self, npc_id: u32) {
        self.begin(npc_id);
        self.npc_id = npc_id;
        self.state = NpcDialogState::WaitingForDealType;
    }

    pub fn advance_next(&mut self) {
        self.advance_next_at(Instant::now());
    }

    fn advance_next_at(&mut self, now: Instant) {
        self.clear_text_on_next = true;
        self.next_button = false;
        self.close_button = false;
        self.state = NpcDialogState::DisplayingText;
        self.next_sent_at = Some(now);
    }

    /// Gives the Next button back when the server never answered the last one (see [`UNANSWERED_NEXT`]).
    pub fn restore_unanswered_next(&mut self, now: Instant) {
        let unanswered = self
            .next_sent_at
            .is_some_and(|sent| now.duration_since(sent) >= UNANSWERED_NEXT);
        if unanswered && self.state == NpcDialogState::DisplayingText {
            self.next_sent_at = None;
            self.next_button = true;
            self.clear_text_on_next = false;
        }
    }

    pub fn close_menu(&mut self) {
        if !self.say_visible {
            self.close();
            return;
        }
        self.menu_items.clear();
        self.selected_menu_index = 0;
        self.menu_scroll_offset = 0;
        self.state = NpcDialogState::DisplayingText;
    }

    pub fn close(&mut self) {
        self.state = NpcDialogState::Idle;
        self.npc_id = 0;
        self.text.clear();
        self.next_button = false;
        self.close_button = false;
        self.menu_items.clear();
        self.selected_menu_index = 0;
        self.menu_scroll_offset = 0;
        self.say_visible = false;
        self.clear_text_on_next = false;
        self.next_sent_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_lifecycle() {
        let mut dialog = NpcDialogData::new();
        assert!(!dialog.is_open());

        dialog.open_text(100, "Hello adventurer!");
        assert!(dialog.is_open());
        assert_eq!(dialog.state, NpcDialogState::DisplayingText);
        assert_eq!(dialog.text, "Hello adventurer!");

        dialog.wait_for_next(100);
        assert!(dialog.next_button);

        dialog.advance_next();
        assert_eq!(dialog.state, NpcDialogState::DisplayingText);
        assert_eq!(dialog.text, "Hello adventurer!");
        assert!(!dialog.next_button);

        dialog.open_text(100, "Choose wisely.");
        assert_eq!(dialog.text, "Choose wisely.");

        dialog.show_menu(100, vec!["Buy".into(), "Sell".into(), "Cancel".into()]);
        assert_eq!(dialog.state, NpcDialogState::WaitingForMenu);
        assert_eq!(dialog.menu_items.len(), 3);

        dialog.wait_for_close();
        assert!(dialog.close_button);

        dialog.close();
        assert!(!dialog.is_open());
        assert!(!dialog.close_button);
    }

    #[test]
    fn clear_empties_the_text_and_keeps_the_dialog() {
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "First page.");
        dialog.clear_text(200);
        assert_eq!(dialog.text, "First page.");
        dialog.clear_text(100);
        assert!(dialog.is_open());
        assert!(!dialog.has_text());
        dialog.open_text(100, "Second page.");
        assert_eq!(dialog.text, "Second page.");
    }

    #[test]
    fn an_unanswered_next_comes_back() {
        let start = Instant::now();
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "Welcome to Izlude.");
        dialog.wait_for_next(100);
        dialog.advance_next_at(start);

        dialog.restore_unanswered_next(start + Duration::from_secs(1));
        assert!(!dialog.next_button);
        dialog.restore_unanswered_next(start + UNANSWERED_NEXT);
        assert!(dialog.next_button);
        // the text stays: nothing replaced it
        assert_eq!(dialog.text, "Welcome to Izlude.");
    }

    #[test]
    fn an_answered_next_stays_answered() {
        let start = Instant::now();
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "Page one.");
        dialog.wait_for_next(100);
        dialog.advance_next_at(start);
        dialog.open_text(100, "Page two.");

        dialog.restore_unanswered_next(start + UNANSWERED_NEXT * 2);
        assert!(!dialog.next_button);
        assert_eq!(dialog.text, "Page two.");
    }

    #[test]
    fn another_npcs_text_starts_without_the_last_ones_buttons() {
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "[Young Man]");
        dialog.wait_for_next(100);

        dialog.open_text(200, "[Wise Bull Horn]");
        dialog.wait_for_close();

        assert!(!dialog.next_button);
        assert!(dialog.close_button);
        assert_eq!(
            (dialog.npc_id, dialog.text.as_str()),
            (200, "[Wise Bull Horn]")
        );
    }

    #[test]
    fn another_npcs_menu_starts_without_the_last_ones_text() {
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "[Young Man]");
        dialog.wait_for_close();

        dialog.show_menu(200, vec!["Yes".into(), "No".into()]);

        assert!(dialog.text.is_empty());
        assert!(!dialog.close_button);
        assert_eq!(dialog.npc_id, 200);
    }

    #[test]
    fn the_same_npc_after_its_close_starts_over() {
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "Goodbye.");
        dialog.wait_for_close();

        dialog.open_text(100, "Hello again.");

        assert_eq!(dialog.text, "Hello again.");
        assert!(!dialog.close_button);
    }

    #[test]
    fn background_close_keeps_next_button_and_npc_id() {
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "Take this quest?");
        dialog.wait_for_next(100);

        dialog.wait_for_close();

        assert!(dialog.next_button);
        assert!(dialog.close_button);
        assert_eq!(dialog.npc_id, 100);
    }

    #[test]
    fn menu_keeps_the_say_dialog_and_its_text() {
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "Choose:");
        dialog.wait_for_next(100);
        dialog.advance_next();

        dialog.show_menu(100, vec!["Yes".into(), "No".into()]);
        assert_eq!(dialog.text, "Choose:");
        assert!(dialog.say_visible);
        assert!(!dialog.next_button);

        dialog.close_menu();
        assert!(dialog.menu_items.is_empty());
        assert_eq!(dialog.state, NpcDialogState::DisplayingText);
        assert_eq!(dialog.text, "Choose:");
        assert!(dialog.say_visible);

        dialog.open_text(100, "You chose.");
        assert_eq!(dialog.text, "You chose.");

        let mut bare = NpcDialogData::new();
        bare.show_menu(100, vec!["Yes".into()]);
        assert!(!bare.say_visible);
        bare.close_menu();
        assert!(!bare.is_open());
    }

    #[test]
    fn text_accumulates_within_same_npc() {
        let mut dialog = NpcDialogData::new();
        dialog.open_text(100, "Line 1");
        dialog.open_text(100, "Line 2");
        assert_eq!(dialog.text, "Line 1\nLine 2");
    }
}
