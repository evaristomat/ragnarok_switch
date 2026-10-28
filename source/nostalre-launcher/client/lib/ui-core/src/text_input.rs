use crate::context::UiContext;

pub struct TextInput {
    pub text: String,
    pub cursor_pos: usize,
    pub max_len: usize,
    pub is_password: bool,
    pub numeric_only: bool,
    pub selection_anchor: Option<usize>,
}

impl TextInput {
    pub fn new(max_len: usize, is_password: bool) -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
            max_len,
            is_password,
            numeric_only: false,
            selection_anchor: None,
        }
    }

    pub fn with_numeric_only(mut self, numeric_only: bool) -> Self {
        self.numeric_only = numeric_only;
        self
    }

    pub fn select_all(&mut self) {
        self.selection_anchor = Some(0);
        self.cursor_pos = self.char_count();
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let count = self.char_count();
        let start = anchor.min(self.cursor_pos).min(count);
        let end = anchor.max(self.cursor_pos).min(count);
        (start != end).then_some((start, end))
    }

    fn delete_selection(&mut self) {
        let Some((start, end)) = self.selection_range() else {
            self.selection_anchor = None;
            return;
        };
        let (from, to) = (self.byte_offset(start), self.byte_offset(end));
        self.text.replace_range(from..to, "");
        self.cursor_pos = start;
        self.selection_anchor = None;
    }

    pub fn process_keys(&mut self, ctx: &UiContext) {
        for &ch in &ctx.typed_chars {
            if self.numeric_only && !ch.is_ascii_digit() {
                continue;
            }
            self.delete_selection();
            if self.text.len() < self.max_len {
                let byte_pos = self.byte_offset(self.cursor_pos);
                self.text.insert(byte_pos, ch);
                self.cursor_pos += 1;
            }
        }

        if ctx.key_backspace {
            if self.selection_range().is_some() {
                self.delete_selection();
            } else if self.cursor_pos > 0 {
                self.cursor_pos -= 1;
                let byte_pos = self.byte_offset(self.cursor_pos);
                self.text.remove(byte_pos);
            }
        }

        if ctx.key_delete {
            if self.selection_range().is_some() {
                self.delete_selection();
            } else if self.cursor_pos < self.char_count() {
                let byte_pos = self.byte_offset(self.cursor_pos);
                self.text.remove(byte_pos);
            }
        }

        if ctx.key_left {
            if let Some((start, _)) = self.selection_range() {
                self.cursor_pos = start;
                self.selection_anchor = None;
            } else if self.cursor_pos > 0 {
                self.cursor_pos -= 1;
            }
        }

        if ctx.key_right {
            if let Some((_, end)) = self.selection_range() {
                self.cursor_pos = end;
                self.selection_anchor = None;
            } else if self.cursor_pos < self.char_count() {
                self.cursor_pos += 1;
            }
        }
    }

    pub fn move_cursor_to_end(&mut self) {
        self.cursor_pos = self.char_count();
    }

    pub fn display_text(&self) -> String {
        if self.is_password {
            "*".repeat(self.char_count())
        } else {
            self.text.clone()
        }
    }

    fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_offset(&self, char_index: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_index)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    pub fn display_cursor_offset(&self) -> usize {
        if self.is_password {
            self.cursor_pos
        } else {
            self.byte_offset(self.cursor_pos)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> UiContext {
        UiContext::new(800.0, 600.0)
    }

    #[test]
    fn typing_appends_and_cursor_moves() {
        let mut input = TextInput::new(24, false);
        let mut ctx = make_ctx();
        ctx.typed_chars = vec!['h', 'i'];

        input.process_keys(&ctx);
        assert_eq!(input.text, "hi");
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn space_inserted_in_text() {
        let mut input = TextInput::new(24, false);
        let mut ctx = make_ctx();
        ctx.typed_chars = vec!['h', 'i', ' ', 'y', 'o'];

        input.process_keys(&ctx);
        assert_eq!(input.text, "hi yo");
        assert_eq!(input.cursor_pos, 5);
    }

    #[test]
    fn backspace_removes_before_cursor() {
        let mut input = TextInput::new(24, false);
        input.text = "abc".to_string();
        input.cursor_pos = 3;

        let mut ctx = make_ctx();
        ctx.key_backspace = true;
        input.process_keys(&ctx);
        assert_eq!(input.text, "ab");
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn max_length_enforced() {
        let mut input = TextInput::new(3, false);
        let mut ctx = make_ctx();
        ctx.typed_chars = vec!['a', 'b', 'c', 'd'];

        input.process_keys(&ctx);
        assert_eq!(input.text, "abc");
        assert_eq!(input.cursor_pos, 3);
    }

    #[test]
    fn numeric_only_rejects_non_digits() {
        let mut input = TextInput::new(10, false).with_numeric_only(true);
        let mut ctx = make_ctx();
        ctx.typed_chars = vec!['1', 'a', '2', '-', '3', ' '];

        input.process_keys(&ctx);
        assert_eq!(input.text, "123");
        assert_eq!(input.cursor_pos, 3);
    }

    #[test]
    fn password_masking() {
        let mut input = TextInput::new(24, true);
        input.text = "secret".to_string();
        input.cursor_pos = 6;
        assert_eq!(input.display_text(), "******");
    }

    #[test]
    fn cursor_movement_clamps() {
        let mut input = TextInput::new(24, false);
        input.text = "ab".to_string();
        input.cursor_pos = 0;

        let mut ctx = make_ctx();
        ctx.key_left = true;
        input.process_keys(&ctx);
        assert_eq!(input.cursor_pos, 0);

        ctx.key_left = false;
        ctx.key_right = true;
        input.process_keys(&ctx);
        assert_eq!(input.cursor_pos, 1);

        input.cursor_pos = 2;
        ctx.key_right = true;
        input.process_keys(&ctx);
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn typing_replaces_selected_text() {
        let mut input = TextInput::new(6, false).with_numeric_only(true);
        input.text = "42".to_string();
        input.select_all();

        let mut ctx = make_ctx();
        ctx.typed_chars = vec!['7', '5'];
        input.process_keys(&ctx);
        assert_eq!(input.text, "75");
        assert_eq!(input.cursor_pos, 2);
        assert_eq!(input.selection_range(), None);
    }

    #[test]
    fn backspace_deletes_whole_selection() {
        let mut input = TextInput::new(6, false);
        input.text = "42".to_string();
        input.select_all();

        let mut ctx = make_ctx();
        ctx.key_backspace = true;
        input.process_keys(&ctx);
        assert_eq!(input.text, "");
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn left_arrow_collapses_selection_without_deleting() {
        let mut input = TextInput::new(6, false);
        input.text = "42".to_string();
        input.select_all();

        let mut ctx = make_ctx();
        ctx.key_left = true;
        input.process_keys(&ctx);
        assert_eq!(input.text, "42");
        assert_eq!(input.cursor_pos, 0);
        assert_eq!(input.selection_range(), None);
    }

    #[test]
    fn delete_removes_after_cursor() {
        let mut input = TextInput::new(24, false);
        input.text = "abc".to_string();
        input.cursor_pos = 1;

        let mut ctx = make_ctx();
        ctx.key_delete = true;
        input.process_keys(&ctx);
        assert_eq!(input.text, "ac");
        assert_eq!(input.cursor_pos, 1);
    }
}
