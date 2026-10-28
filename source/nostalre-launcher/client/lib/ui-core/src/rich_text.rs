//! Text with links and images among its words, wrapped into lines: the words of a span wrap one by one, a `^RRGGBB`
//! colour carries on across lines and spans, and each line's neighbouring words of one kind share a run. Bold words
//! are written in the atlas's bold glyphs (`bold_char`); a line holding an image grows to the image's height.

use crate::draw::{color_code_at, strip_color_codes};
use nostalre_renderer::font_atlas::bold_char;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichSpan<'a> {
    /// Text with its `^RRGGBB` colours; `\n` starts a line
    Text(&'a str),
    /// A link's label; `link` is the caller's
    Link { label: &'a str, link: usize },
    /// An image as wide as `image_width`, a link too
    Image { link: usize },
    /// The words after it bold (true) or not
    Bold(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    Text,
    Link(usize),
    Image(usize),
}

/// Words drawn together: a text run keeps its colour codes, a link's has none
#[derive(Debug, Clone, PartialEq)]
pub struct RichRun {
    pub line: usize,
    pub x: f32,
    pub width: f32,
    pub text: String,
    /// The colour at the run's start
    pub color: [f32; 4],
    pub kind: RunKind,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichLayout {
    pub runs: Vec<RichRun>,
    pub lines: usize,
    /// The lines holding an image, in order
    pub image_lines: Vec<usize>,
}

impl RichLayout {
    /// Where `line` starts below the first, its lines `line_height` high and those holding an image `image_height`
    pub fn line_top(&self, line: usize, line_height: f32, image_height: f32) -> f32 {
        let taller = self.image_lines.iter().filter(|&&l| l < line).count();
        line as f32 * line_height + taller as f32 * (image_height - line_height).max(0.0)
    }

    /// The whole text's height, laid out as [`Self::line_top`]
    pub fn height(&self, line_height: f32, image_height: f32) -> f32 {
        self.line_top(self.lines, line_height, image_height)
    }
}

struct Cursor<M> {
    measure: M,
    max_width: f32,
    space: f32,
    line: usize,
    x: f32,
    /// Spaces written before the next word: kept between words and at a paragraph's start, dropped at a wrap
    spaces: usize,
    color: [f32; 4],
    bold: bool,
    runs: Vec<RichRun>,
    image_lines: Vec<usize>,
}

impl<M: Fn(&str) -> f32> Cursor<M> {
    fn new_line(&mut self) {
        self.line += 1;
        self.x = 0.0;
        self.spaces = 0;
    }

    /// One word, `shown` as it is drawn and `written` with its colour codes
    fn place(&mut self, shown: &str, written: &str, width: f32, kind: RunKind) {
        let mut gap = self.spaces as f32 * self.space;
        if self.x > 0.0 && self.x + gap + width > self.max_width {
            self.new_line();
            gap = 0.0;
        }
        let joins = matches!(self.runs.last(), Some(last) if last.line == self.line && last.kind == kind)
            && !matches!(kind, RunKind::Image(_));
        if joins {
            let last = self.runs.last_mut().unwrap();
            last.text.extend(std::iter::repeat_n(' ', self.spaces));
            last.text.push_str(shown);
            last.width = self.x + gap + width - last.x;
        } else {
            self.runs.push(RichRun {
                line: self.line,
                x: self.x + gap,
                width,
                text: shown.to_string(),
                color: self.color,
                kind,
            });
        }
        if matches!(kind, RunKind::Image(_)) && self.image_lines.last() != Some(&self.line) {
            self.image_lines.push(self.line);
        }
        self.x += gap + width;
        self.spaces = 0;
        self.color = last_color(written, self.color);
    }

    /// `text`'s words, wrapped; a link's are drawn without their colour codes
    fn words(&mut self, text: &str, kind: RunKind) {
        for (n, paragraph) in text.split('\n').enumerate() {
            if n > 0 {
                self.new_line();
            }
            for (n, word) in paragraph.split(' ').enumerate() {
                if n > 0 {
                    self.spaces += 1;
                }
                if word.is_empty() {
                    continue;
                }
                let styled = if self.bold {
                    std::borrow::Cow::Owned(embolden(word))
                } else {
                    std::borrow::Cow::Borrowed(word)
                };
                let bare = strip_color_codes(&styled);
                let width = (self.measure)(&bare);
                let shown: &str = if kind == RunKind::Text {
                    &styled
                } else {
                    &bare
                };
                self.place(shown, word, width, kind);
            }
        }
    }
}

/// `text` in bold glyphs, its colour codes as written
fn embolden(text: &str) -> String {
    let mut bold = String::with_capacity(text.len());
    let mut code_end = 0;
    for (i, c) in text.char_indices() {
        if i >= code_end && color_code_at(text, i, 1.0).is_some() {
            code_end = i + 7;
        }
        bold.push(if i < code_end { c } else { bold_char(c) });
    }
    bold
}

/// The colour after `text`'s codes, starting from `color`
fn last_color(text: &str, color: [f32; 4]) -> [f32; 4] {
    text.match_indices('^')
        .rev()
        .find_map(|(i, _)| color_code_at(text, i, color[3]))
        .unwrap_or(color)
}

pub fn layout(
    spans: &[RichSpan],
    max_width: f32,
    measure: impl Fn(&str) -> f32,
    image_width: f32,
    color: [f32; 4],
) -> RichLayout {
    let space = measure(" ");
    let mut cursor = Cursor {
        measure,
        max_width,
        space,
        line: 0,
        x: 0.0,
        spaces: 0,
        color,
        bold: false,
        runs: Vec::new(),
        image_lines: Vec::new(),
    };
    for span in spans {
        match *span {
            RichSpan::Text(text) => cursor.words(text, RunKind::Text),
            RichSpan::Link { label, link } => cursor.words(label, RunKind::Link(link)),
            RichSpan::Image { link } => cursor.place("", "", image_width, RunKind::Image(link)),
            RichSpan::Bold(bold) => cursor.bold = bold,
        }
    }
    RichLayout {
        lines: cursor.line + 1,
        runs: cursor.runs,
        image_lines: cursor.image_lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

    /// Every character 1 wide
    fn chars(text: &str) -> f32 {
        text.chars().count() as f32
    }

    fn run(line: usize, x: f32, width: f32, text: &str, kind: RunKind) -> RichRun {
        RichRun {
            line,
            x,
            width,
            text: text.into(),
            color: BLACK,
            kind,
        }
    }

    #[test]
    fn a_link_sits_among_the_words_and_wraps_with_them() {
        let spans = [
            RichSpan::Text("Bring a "),
            RichSpan::Link {
                label: "[Red Potion]",
                link: 0,
            },
            RichSpan::Text(", please."),
        ];
        let laid = layout(&spans, 16.0, chars, 4.0, BLACK);
        assert_eq!(
            laid.runs,
            [
                run(0, 0.0, 7.0, "Bring a", RunKind::Text),
                run(0, 8.0, 4.0, "[Red", RunKind::Link(0)),
                run(1, 0.0, 7.0, "Potion]", RunKind::Link(0)),
                run(1, 7.0, 9.0, ", please.", RunKind::Text),
            ]
        );
        assert_eq!(laid.lines, 2);
    }

    #[test]
    fn images_stand_alone_and_lines_break_on_newlines() {
        let spans = [
            RichSpan::Image { link: 0 },
            RichSpan::Text(" "),
            RichSpan::Image { link: 1 },
            RichSpan::Text("\nbelow"),
        ];
        let laid = layout(&spans, 40.0, chars, 4.0, BLACK);
        assert_eq!(
            laid.runs,
            [
                run(0, 0.0, 4.0, "", RunKind::Image(0)),
                run(0, 5.0, 4.0, "", RunKind::Image(1)),
                run(1, 0.0, 5.0, "below", RunKind::Text),
            ]
        );
    }

    #[test]
    fn a_colour_carries_on_to_the_next_line_and_a_links_label_drops_its_codes() {
        let spans = [
            RichSpan::Text("^FF0000aaaa bbbb"),
            RichSpan::Link {
                label: "^000000[x]",
                link: 0,
            },
            RichSpan::Text(" c"),
        ];
        let laid = layout(&spans, 6.0, chars, 4.0, BLACK);
        let colors: Vec<_> = laid
            .runs
            .iter()
            .map(|r| (r.text.as_str(), r.color))
            .collect();
        assert_eq!(
            colors,
            [
                ("^FF0000aaaa", BLACK),
                ("bbbb", RED),
                ("[x]", RED),
                ("c", BLACK)
            ]
        );
    }

    #[test]
    fn spaces_stay_as_written_but_not_at_a_wrap() {
        let laid = layout(&[RichSpan::Text("  a   b cccccc")], 8.0, chars, 4.0, BLACK);
        assert_eq!(
            laid.runs,
            [
                run(0, 2.0, 5.0, "a   b", RunKind::Text),
                run(1, 0.0, 6.0, "cccccc", RunKind::Text),
            ]
        );
    }

    #[test]
    fn bold_words_take_bold_glyphs_and_keep_their_colour_codes() {
        let spans = [
            RichSpan::Text("a "),
            RichSpan::Bold(true),
            RichSpan::Text("^FF0000Bo"),
            RichSpan::Bold(false),
            RichSpan::Text(" c"),
        ];
        let laid = layout(&spans, 40.0, chars, 4.0, BLACK);
        let bold: String = ['B', 'o'].map(bold_char).iter().collect();
        assert_eq!(laid.runs.len(), 1);
        assert_eq!(laid.runs[0].text, format!("a ^FF0000{bold} c"));
    }

    #[test]
    fn a_line_holding_an_image_grows_to_it() {
        let spans = [
            RichSpan::Text("one\n"),
            RichSpan::Image { link: 0 },
            RichSpan::Text("\nthree"),
        ];
        let laid = layout(&spans, 40.0, chars, 4.0, BLACK);
        assert_eq!(laid.image_lines, [1]);
        assert_eq!(laid.line_top(1, 16.0, 24.0), 16.0);
        assert_eq!(laid.line_top(2, 16.0, 24.0), 40.0);
        assert_eq!(laid.height(16.0, 24.0), 56.0);
    }

    #[test]
    fn no_text_is_one_empty_line() {
        assert_eq!(
            layout(&[], 10.0, chars, 4.0, BLACK),
            RichLayout {
                runs: vec![],
                lines: 1,
                image_lines: vec![],
            }
        );
    }
}
