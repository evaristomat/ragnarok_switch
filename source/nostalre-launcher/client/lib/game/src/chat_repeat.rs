//! kRO's refusal of a line said over and over: its chat input (0x789f10) keeps the last line the player sent (+0x430)
//! and how many times in a row it came again (+0x448), one for public, party, guild, whisper and battleground lines
//! alike; from the third in a row on, 0x7899d0 shows msgstringtable line 49 instead of sending it. The check is off
//! for two run-time globals (0x1226954 at 3, 0x1226950 at 10) whose meaning is not read yet: ours always checks.

/// "You can't type the same word/phrase more than 3 times."
pub const MSG_SAME_LINE: u16 = 49;

#[derive(Default)]
pub struct RepeatGuard {
    last: String,
    repeats: u32,
}

impl RepeatGuard {
    /// Whether `line` may go out. A refused line is the kept one, so the count runs on until a different line
    pub fn admit(&mut self, line: &str) -> bool {
        if line == self.last {
            self.repeats += 1;
            return self.repeats < 2;
        }
        self.repeats = 0;
        self.last.clear();
        self.last.push_str(line);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_third_same_line_in_a_row_and_on_is_refused() {
        let mut guard = RepeatGuard::default();
        assert!(guard.admit("hi") && guard.admit("hi"));
        assert!(!guard.admit("hi") && !guard.admit("hi"));
        assert!(guard.admit("bye"), "another line starts over");
        assert!(guard.admit("hi") && guard.admit("hi") && !guard.admit("hi"));
    }
}
