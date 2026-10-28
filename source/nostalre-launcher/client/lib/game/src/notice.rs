//! kRO's notice window over the top of the screen (a window of class 0x522b40): the broadcast handlers (0x009a at
//! 0x7b7080, 0x01c3 at 0x7b6b70) and the special item announce (0x07fd at 0x7b7ef0) write their line there, the
//! latest replacing the last, in the packet's colour; the game mode closes it 20 seconds after the last line came
//! (0x77b9f8: `+0x2d0 + 0x4e20`)

const LIFETIME_SECS: f32 = 20.0;

#[derive(Debug, Default)]
pub struct NoticeWindow {
    current: Option<Notice>,
}

#[derive(Debug)]
struct Notice {
    text: String,
    color: [f32; 4],
    age: f32,
}

impl NoticeWindow {
    pub fn show(&mut self, text: String, color: [f32; 4]) {
        self.current = Some(Notice {
            text,
            color,
            age: 0.0,
        });
    }

    pub fn tick(&mut self, dt: f32) {
        if let Some(notice) = &mut self.current {
            notice.age += dt;
            if notice.age >= LIFETIME_SECS {
                self.current = None;
            }
        }
    }

    /// The line shown and its colour
    pub fn current(&self) -> Option<(&str, [f32; 4])> {
        self.current
            .as_ref()
            .map(|notice| (notice.text.as_str(), notice.color))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORANGE: [f32; 4] = [1.0, 0.69, 0.38, 1.0];
    const WHITE: [f32; 4] = [1.0; 4];

    #[test]
    fn the_latest_line_replaces_the_last_and_goes_after_20_seconds() {
        let mut notice = NoticeWindow::default();
        notice.show("first".into(), WHITE);
        notice.tick(15.0);
        notice.show("second".into(), ORANGE);
        notice.tick(15.0);
        assert_eq!(notice.current(), Some(("second", ORANGE)));
        notice.tick(5.0);
        assert_eq!(notice.current(), None);
    }
}
